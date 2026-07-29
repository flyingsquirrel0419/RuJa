//! ECMA-402 locale canonicalization and the `%Intl%` intrinsic.

use super::intl_aliases::{TRANSFORM_VALUE_ALIASES, UNICODE_TYPE_ALIASES};
use super::{data_prop, make_value_array_in_current_realm};
use crate::error::{self, Error};
use crate::value::{GcIdx, HeapObj, ObjectData, PropertyDescriptor, PropertyKey, Value};
use crate::vm::{NativeFn, Vm};
use icu_locale::extensions::unicode;
use icu_locale::{Locale, LocaleCanonicalizer};
use indexmap::{IndexMap, IndexSet};
use parking_lot::Mutex;
use std::borrow::Cow;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const MAX_SAFE_LENGTH: f64 = 9_007_199_254_740_991.0;

fn generated_alias<'a>(
    tables: &'a [(&str, &[(&str, &str)])],
    key: &str,
    value: &str,
) -> Option<&'a str> {
    let key_index = tables
        .binary_search_by_key(&key, |(candidate, _)| *candidate)
        .ok()?;
    let aliases = tables.get(key_index)?.1;
    let alias_index = aliases
        .binary_search_by_key(&value, |(candidate, _)| *candidate)
        .ok()?;
    aliases.get(alias_index).map(|(_, canonical)| *canonical)
}

fn replace_legacy_tag_prefix(tag: &str) -> Cow<'_, str> {
    const LEGACY_TAGS: &[(&str, &str)] = &[
        ("art-lojban", "jbo"),
        ("cel-gaulish", "xtg"),
        ("zh-guoyu", "zh"),
        ("zh-hakka", "hak"),
        ("zh-xiang", "hsn"),
    ];

    let lower = tag.to_ascii_lowercase();
    for (legacy, replacement) in LEGACY_TAGS {
        if lower == *legacy {
            return Cow::Owned((*replacement).to_string());
        }
        if lower
            .strip_prefix(legacy)
            .is_some_and(|suffix| suffix.starts_with('-'))
        {
            return Cow::Owned(format!("{replacement}{}", &tag[legacy.len()..]));
        }
    }
    Cow::Borrowed(tag)
}

#[derive(Default)]
struct LongLanguageSubtags {
    primary: Option<String>,
    transform: Option<String>,
}

type NumericExtensions = Vec<Vec<String>>;

#[derive(Default)]
struct TransformExtension {
    language: Option<String>,
    fields: Vec<(String, String)>,
}

fn is_transform_key(subtag: &str) -> bool {
    let bytes = subtag.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1].is_ascii_digit()
}

fn extract_transform_extension(tag: &str) -> (Cow<'_, str>, Option<TransformExtension>) {
    let subtags: Vec<&str> = tag.split('-').collect();
    let Some(start) = subtags
        .iter()
        .take_while(|subtag| !subtag.eq_ignore_ascii_case("x"))
        .position(|subtag| subtag.eq_ignore_ascii_case("t"))
    else {
        return (Cow::Borrowed(tag), None);
    };
    let end = subtags[start + 1..]
        .iter()
        .position(|subtag| subtag.len() == 1)
        .map_or(subtags.len(), |offset| start + 1 + offset);
    let contents = &subtags[start + 1..end];
    let first_field = contents
        .iter()
        .position(|subtag| is_transform_key(subtag))
        .unwrap_or(contents.len());
    let language = (first_field != 0).then(|| contents[..first_field].join("-"));
    let mut fields = Vec::new();
    let mut index = first_field;
    while index < contents.len() {
        let key = contents[index].to_ascii_lowercase();
        let value_start = index + 1;
        index = value_start;
        while index < contents.len() && !is_transform_key(contents[index]) {
            index += 1;
        }
        fields.push((
            key,
            contents[value_start..index].join("-").to_ascii_lowercase(),
        ));
    }
    let mut kept = Vec::with_capacity(subtags.len() - (end - start));
    kept.extend_from_slice(&subtags[..start]);
    kept.extend_from_slice(&subtags[end..]);
    (
        Cow::Owned(kept.join("-")),
        Some(TransformExtension { language, fields }),
    )
}

fn restore_transform_extension(
    canonical: String,
    extension: Option<TransformExtension>,
) -> error::Result<String> {
    let Some(mut extension) = extension else {
        return Ok(canonical);
    };
    let mut rendered = vec!["t".to_string()];
    if let Some(language) = extension.language {
        rendered.extend(
            canonicalize_locale(&language)?
                .split('-')
                .map(str::to_ascii_lowercase),
        );
    }
    for (key, value) in &mut extension.fields {
        if let Some(replacement) = generated_alias(TRANSFORM_VALUE_ALIASES, key, value) {
            *value = replacement.to_string();
        }
    }
    extension.fields.sort_by(|left, right| left.0.cmp(&right.0));
    for (key, value) in extension.fields {
        rendered.push(key);
        rendered.extend(value.split('-').map(str::to_string));
    }

    let mut subtags: Vec<String> = canonical.split('-').map(str::to_string).collect();
    let insertion = subtags
        .iter()
        .position(|subtag| subtag.len() == 1 && subtag.as_str() > "t")
        .unwrap_or(subtags.len());
    subtags.splice(insertion..insertion, rendered);
    Ok(subtags.join("-"))
}

fn strip_numeric_extensions(tag: &str) -> Result<(Cow<'_, str>, NumericExtensions), ()> {
    let subtags: Vec<&str> = tag.split('-').collect();
    let mut kept = Vec::with_capacity(subtags.len());
    let mut numeric = NumericExtensions::new();
    let mut seen = 0u16;
    let mut index = 0;
    while index < subtags.len() {
        let subtag = subtags[index];
        if subtag.eq_ignore_ascii_case("x") {
            kept.extend_from_slice(&subtags[index..]);
            break;
        }
        if subtag.len() == 1 && subtag.as_bytes()[0].is_ascii_digit() {
            if index == 0 {
                return Err(());
            }
            let digit = (subtag.as_bytes()[0] - b'0') as u16;
            let bit = 1u16 << digit;
            if seen & bit != 0 {
                return Err(());
            }
            seen |= bit;
            let mut end = index + 1;
            while end < subtags.len() && subtags[end].len() != 1 {
                let value = subtags[end];
                if !(2..=8).contains(&value.len())
                    || !value.bytes().all(|byte| byte.is_ascii_alphanumeric())
                {
                    return Err(());
                }
                end += 1;
            }
            if end == index + 1 {
                return Err(());
            }
            numeric.push(
                subtags[index..end]
                    .iter()
                    .map(|value| value.to_ascii_lowercase())
                    .collect(),
            );
            index = end;
            continue;
        }
        kept.push(subtag);
        index += 1;
    }

    if numeric.is_empty() {
        Ok((Cow::Borrowed(tag), numeric))
    } else {
        Ok((Cow::Owned(kept.join("-")), numeric))
    }
}

fn restore_numeric_extensions(canonical: String, mut numeric: NumericExtensions) -> String {
    if numeric.is_empty() {
        return canonical;
    }
    numeric.sort_by(|left, right| left[0].cmp(&right[0]));
    let base_end = canonical
        .split('-')
        .scan(0usize, |offset, subtag| {
            let start = *offset;
            *offset += subtag.len() + 1;
            Some((start, subtag))
        })
        .find_map(|(start, subtag)| (subtag.len() == 1).then_some(start))
        .unwrap_or(canonical.len());

    let mut result = canonical[..base_end].trim_end_matches('-').to_string();
    for extension in numeric {
        result.push('-');
        result.push_str(&extension.join("-"));
    }
    if base_end < canonical.len() {
        result.push('-');
        result.push_str(&canonical[base_end..]);
    }
    result
}

fn is_long_language_subtag(subtag: &str) -> bool {
    (5..=8).contains(&subtag.len()) && subtag.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn substitute_long_language_subtags(tag: &str) -> (Cow<'_, str>, LongLanguageSubtags) {
    let mut subtags: Vec<&str> = tag.split('-').collect();
    let mut replaced = LongLanguageSubtags::default();
    if subtags
        .first()
        .is_some_and(|subtag| is_long_language_subtag(subtag))
    {
        replaced.primary = subtags.first().map(|subtag| subtag.to_ascii_lowercase());
        subtags[0] = "qaa";
    }

    let mut index = 1;
    while index < subtags.len() {
        if subtags[index].eq_ignore_ascii_case("x") {
            break;
        }
        if subtags[index].eq_ignore_ascii_case("t")
            && subtags
                .get(index + 1)
                .is_some_and(|subtag| is_long_language_subtag(subtag))
        {
            replaced.transform = subtags
                .get(index + 1)
                .map(|subtag| subtag.to_ascii_lowercase());
            subtags[index + 1] = "qab";
            break;
        }
        index += 1;
    }

    if replaced.primary.is_none() && replaced.transform.is_none() {
        (Cow::Borrowed(tag), replaced)
    } else {
        (Cow::Owned(subtags.join("-")), replaced)
    }
}

fn restore_long_language_subtags(canonical: String, replacements: LongLanguageSubtags) -> String {
    if replacements.primary.is_none() && replacements.transform.is_none() {
        return canonical;
    }
    let mut subtags: Vec<&str> = canonical.split('-').collect();
    if replacements.primary.is_some() && subtags.first() == Some(&"qaa") {
        subtags[0] = replacements.primary.as_deref().unwrap_or("qaa");
    }
    if let Some(transform) = replacements.transform.as_deref() {
        let mut index = 1;
        while index + 1 < subtags.len() {
            if subtags[index] == "x" {
                break;
            }
            if subtags[index] == "t" && subtags[index + 1] == "qab" {
                subtags[index + 1] = transform;
                break;
            }
            index += 1;
        }
    }
    subtags.join("-")
}

fn canonicalize_extension_aliases(locale: &mut Locale) {
    for (key_name, _) in UNICODE_TYPE_ALIASES {
        let key = key_name
            .parse::<unicode::Key>()
            .expect("generated CLDR Unicode key must be valid");
        let replacement = locale
            .extensions
            .unicode
            .keywords
            .get(&key)
            .and_then(|value| generated_alias(UNICODE_TYPE_ALIASES, key_name, &value.to_string()));
        if let Some(replacement) = replacement {
            let value = unicode::Value::try_from_str(replacement)
                .expect("generated CLDR Unicode value must be valid");
            locale.extensions.unicode.keywords.set(key, value);
        }
    }
}

fn canonicalize_locale(tag: &str) -> error::Result<Arc<str>> {
    let (validation_without_numeric, _) = strip_numeric_extensions(tag)
        .map_err(|_| Error::range(format!("Invalid language tag: {tag}")))?;
    let (validation_input, _) = substitute_long_language_subtags(&validation_without_numeric);
    validation_input
        .parse::<Locale>()
        .map_err(|_| Error::range(format!("Invalid language tag: {tag}")))?;

    let legacy = replace_legacy_tag_prefix(tag);
    let (without_numeric, numeric_extensions) = strip_numeric_extensions(&legacy)
        .map_err(|_| Error::range(format!("Invalid language tag: {tag}")))?;
    let (without_transform, transform_extension) = extract_transform_extension(&without_numeric);
    let (input, long_languages) = substitute_long_language_subtags(&without_transform);
    let mut locale = input
        .parse::<Locale>()
        .map_err(|_| Error::range(format!("Invalid language tag: {tag}")))?;
    LocaleCanonicalizer::new_extended().canonicalize(&mut locale);
    canonicalize_extension_aliases(&mut locale);
    let canonical = restore_long_language_subtags(locale.to_string(), long_languages);
    let canonical = restore_transform_extension(canonical, transform_extension)?;
    Ok(Arc::from(restore_numeric_extensions(
        canonical,
        numeric_extensions,
    )))
}

fn canonicalize_locale_metered(vm: &mut Vm, tag: &str) -> error::Result<Arc<str>> {
    vm.consume_fuel_units(tag.len().div_ceil(64).min(i64::MAX as usize) as i64)?;
    let subtags = tag.bytes().filter(|byte| *byte == b'-').count() + 1;
    let quadratic = subtags.saturating_mul(subtags);
    vm.consume_fuel_units(quadratic.min(i64::MAX as usize) as i64)?;
    canonicalize_locale(tag)
}

fn to_length(vm: &mut Vm, value: &Value) -> error::Result<u64> {
    let number = vm.to_number(value)?;
    if number.is_nan() || number <= 0.0 {
        return Ok(0);
    }
    if number.is_infinite() {
        return Ok(MAX_SAFE_LENGTH as u64);
    }
    Ok(number.trunc().min(MAX_SAFE_LENGTH) as u64)
}

fn canonicalize_list(vm: &mut Vm, locales: &Value) -> error::Result<Vec<Value>> {
    let mut seen = IndexSet::<Arc<str>>::new();
    if locales.is_undefined() {
        return Ok(Vec::new());
    }
    if let Value::String(tag) = locales {
        let canonical = canonicalize_locale_metered(vm, tag)?;
        return Ok(vec![Value::String(canonical)]);
    }

    let object = vm.to_object(locales)?;
    let object_pin = vm.pin(&object);
    let completion = (|| {
        let length_value = vm.get_property(&object, "length")?;
        let length_pin = vm.pin(&length_value);
        let length_result = to_length(vm, &length_value);
        vm.unpin_many(length_pin);
        let length = length_result?;

        let mut index = 0;
        while index < length {
            vm.consume_fuel()?;
            let key = PropertyKey::from_integer_index(index);
            if vm.has_property_key(&object, &key)? {
                let value = vm.get_property_by_key(&object, &key)?;
                if !matches!(value, Value::String(_) | Value::Object(_)) {
                    return Err(Error::type_err(
                        "locale list elements must be strings or objects",
                    ));
                }
                let value_pin = vm.pin(&value);
                let tag_result = vm.to_string(&value);
                vm.unpin_many(value_pin);
                let canonical = canonicalize_locale_metered(vm, &tag_result?)?;
                if !seen.contains(&canonical) {
                    seen.try_reserve(1)
                        .map_err(|_| Error::range("locale list is too large"))?;
                    seen.insert(canonical);
                }
            }
            index += 1;
        }

        let mut result = Vec::new();
        result
            .try_reserve_exact(seen.len())
            .map_err(|_| Error::range("locale list is too large"))?;
        result.extend(seen.into_iter().map(Value::String));
        Ok(result)
    })();
    vm.unpin_many(object_pin);
    completion
}

fn intl_get_canonical_locales(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let locales = args.first().cloned().unwrap_or(Value::Undefined);
    let locales_pin = vm.pin(&locales);
    let list = canonicalize_list(vm, &locales);
    vm.unpin_many(locales_pin);
    make_value_array_in_current_realm(vm, list?)
}

pub(crate) fn build_intl_in_env(
    vm: &mut Vm,
    env: GcIdx,
    object_proto: Value,
) -> error::Result<Value> {
    let mut pin_count = vm.pin(&object_proto);
    let result = (|| {
        let method_idx = vm.new_native_function_in_env_with_gc_retry(
            "getCanonicalLocales",
            intl_get_canonical_locales as NativeFn,
            1,
            env,
        )?;
        let method = Value::Object(method_idx);
        pin_count += vm.pin(&method);

        let mut props = IndexMap::new();
        props.insert(PropertyKey::from("getCanonicalLocales"), data_prop(method));
        let mut tag = PropertyDescriptor::data(Value::String(Arc::from("Intl")));
        tag.writable = false;
        tag.enumerable = false;
        tag.configurable = true;
        props.insert(
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
        let object = HeapObj::Object(ObjectData {
            props: Mutex::new(props),
            proto: Mutex::new(Some(object_proto)),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("Intl")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        vm.alloc(object).map(Value::Object)
    })();
    vm.unpin_many(pin_count);
    result
}

#[cfg(test)]
mod tests {
    use super::canonicalize_locale;

    #[test]
    fn canonicalizes_cldr_aliases_and_extensions() {
        let cases = [
            ("CMN-hANS", "zh-Hans"),
            ("sgn-GR", "gss"),
            ("sh", "sr-Latn"),
            ("hy-SU", "hy-AM"),
            ("ja-latn-hepburn-heploc", "ja-Latn-alalc97"),
            ("art-lojban", "jbo"),
            ("posix", "posix"),
            ("posix-Latn-US", "posix-Latn-US"),
            ("en-t-enochian-latn", "en-t-enochian-latn"),
            ("und-u-ca-islamicc", "und-u-ca-islamic-civil"),
            ("und-u-kb-yes", "und-u-kb"),
            ("und-u-tz-eire", "und-u-tz-iedub"),
            (
                "und-Latn-t-und-hani-m0-names",
                "und-Latn-t-und-hani-m0-prprname",
            ),
            ("en-t-de-DD", "en-t-de-de"),
            ("en-t-art-lojban", "en-t-jbo"),
            ("en-t-sh", "en-t-sr-latn"),
            ("en-t-m0-zeta-m0-alpha", "en-t-m0-zeta-m0-alpha"),
            (
                "en-u-ca-gregory-t-m0-names-h0-hybrid",
                "en-t-h0-hybrid-m0-prprname-u-ca-gregory",
            ),
            ("en-0-foo", "en-0-foo"),
            ("EN-9-ABC-0-DEF", "en-0-def-9-abc"),
            ("en-0-foo-x-0-BAR", "en-0-foo-x-0-bar"),
        ];
        for (input, expected) in cases {
            assert_eq!(canonicalize_locale(input).unwrap().as_ref(), expected);
        }
    }

    #[test]
    fn rejects_non_uts35_language_tags() {
        for invalid in [
            "",
            "x-foo",
            "i-klingon",
            "de_DE",
            "de-u",
            "en-",
            "art-lojban-US",
            "en-0",
            "en-0-foo-0-bar",
        ] {
            assert!(canonicalize_locale(invalid).is_err(), "accepted {invalid}");
        }
    }
}
