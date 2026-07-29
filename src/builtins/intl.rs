//! ECMA-402 locale canonicalization and the `%Intl%` intrinsic.

use super::intl_aliases::{TRANSFORM_VALUE_ALIASES, UNICODE_TYPE_ALIASES};
use super::intl_locale_info::{
    CALENDAR_PREFERENCES, HOUR_CYCLES, SCRIPT_DIRECTIONS, SUPPORTED_VALUE_CALENDARS,
    SUPPORTED_VALUE_NUMBERING_SYSTEMS, SUPPORTED_VALUE_TIME_ZONES, SUPPORTED_VALUE_UNITS,
    TIME_ZONES, WEEK_INFORMATION,
};
use super::{
    accessor_get_prop, const_prop, data_prop, make_value_array_in_current_realm,
    native_constructor_prototype_with_default,
};
use crate::error::{self, Error};
use crate::value::{
    GcIdx, HeapObj, IntlLocaleData, IntlLocaleRecord, NativeConstructMode, ObjectData,
    PropertyDescriptor, PropertyKey, Value,
};
use crate::vm::{NativeFn, Vm};
use icu_locale::extensions::unicode;
use icu_locale::{Locale, LocaleCanonicalizer, LocaleExpander};
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

fn locale_tag(vm: &Vm, value: &Value) -> Option<Arc<str>> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::IntlLocale(locale) => locale.record.get().map(|record| record.locale.clone()),
        _ => None,
    })
}

fn require_locale_tag(vm: &Vm, this: Option<Value>) -> error::Result<Arc<str>> {
    let receiver = this.unwrap_or(Value::Undefined);
    locale_tag(vm, &receiver)
        .ok_or_else(|| Error::type_err("Intl.Locale method called on incompatible receiver"))
}

fn locale_string_slot(
    vm: &Vm,
    this: Option<Value>,
    select: impl FnOnce(&IntlLocaleRecord) -> Option<Arc<str>>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let Value::Object(index) = receiver else {
        return Err(Error::type_err(
            "Intl.Locale accessor called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::IntlLocale(locale) => locale
            .record
            .get()
            .map(select)
            .map(|value| value.map(Value::String).unwrap_or(Value::Undefined))
            .ok_or_else(|| Error::type_err("Intl.Locale object is not initialized")),
        _ => Err(Error::type_err(
            "Intl.Locale accessor called on incompatible receiver",
        )),
    })
}

#[derive(Clone)]
struct LocaleTagParts {
    language: String,
    script: Option<String>,
    region: Option<String>,
    variants: Vec<String>,
    suffix: Vec<String>,
}

fn is_script_subtag(value: &str) -> bool {
    value.len() == 4 && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_region_subtag(value: &str) -> bool {
    (value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic()))
        || (value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_variant_subtag(value: &str) -> bool {
    ((5..=8).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        || (value.len() == 4
            && value.as_bytes()[0].is_ascii_digit()
            && value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
}

fn locale_tag_parts(tag: &str) -> error::Result<LocaleTagParts> {
    let subtags: Vec<&str> = tag.split('-').collect();
    let base_end = subtags
        .iter()
        .position(|subtag| subtag.len() == 1)
        .unwrap_or(subtags.len());
    let base = &subtags[..base_end];
    let language = base
        .first()
        .ok_or_else(|| Error::internal("canonical locale has no language"))?
        .to_string();
    let mut index = 1;
    let script = base
        .get(index)
        .filter(|value| is_script_subtag(value))
        .map(|value| {
            index += 1;
            (*value).to_string()
        });
    let region = base
        .get(index)
        .filter(|value| is_region_subtag(value))
        .map(|value| {
            index += 1;
            (*value).to_string()
        });
    let variants = base[index..]
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    let suffix = subtags[base_end..]
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    Ok(LocaleTagParts {
        language,
        script,
        region,
        variants,
        suffix,
    })
}

fn locale_tag_parts_metered(vm: &mut Vm, tag: &str) -> error::Result<LocaleTagParts> {
    vm.consume_fuel_units(tag.len().div_ceil(64).min(i64::MAX as usize) as i64)?;
    locale_tag_parts(tag)
}

fn compose_locale_tag(parts: &LocaleTagParts) -> String {
    let mut subtags = vec![parts.language.clone()];
    if let Some(script) = &parts.script {
        subtags.push(script.clone());
    }
    if let Some(region) = &parts.region {
        subtags.push(region.clone());
    }
    subtags.extend(parts.variants.iter().cloned());
    subtags.extend(parts.suffix.iter().cloned());
    subtags.join("-")
}

fn valid_language_option(value: &str) -> bool {
    ((2..=3).contains(&value.len()) || (5..=8).contains(&value.len()))
        && value.bytes().all(|byte| byte.is_ascii_alphabetic())
        && !value.eq_ignore_ascii_case("root")
}

fn valid_variants_option(value: &str) -> bool {
    let mut seen = IndexSet::new();
    !value.is_empty()
        && value
            .split('-')
            .all(|variant| is_variant_subtag(variant) && seen.insert(variant.to_ascii_lowercase()))
}

fn valid_unicode_type(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|subtag| {
            (3..=8).contains(&subtag.len())
                && subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn get_string_option(
    vm: &mut Vm,
    options: Option<&Value>,
    name: &str,
) -> error::Result<Option<String>> {
    let Some(options) = options else {
        return Ok(None);
    };
    let value = vm.get_property(options, name)?;
    if value.is_undefined() {
        return Ok(None);
    }
    let pin_count = vm.pin(&value);
    let result = vm.to_string(&value).and_then(|value| {
        vm.consume_fuel_units(value.len().div_ceil(64).min(i64::MAX as usize) as i64)?;
        Ok(value.to_string())
    });
    vm.unpin_many(pin_count);
    result.map(Some)
}

fn get_boolean_option(
    vm: &mut Vm,
    options: Option<&Value>,
    name: &str,
) -> error::Result<Option<bool>> {
    let Some(options) = options else {
        return Ok(None);
    };
    let value = vm.get_property(options, name)?;
    if value.is_undefined() {
        Ok(None)
    } else {
        Ok(Some(vm.to_boolean(&value)))
    }
}

fn update_language_id(vm: &mut Vm, tag: &str, options: Option<&Value>) -> error::Result<Arc<str>> {
    let mut parts = locale_tag_parts(tag)?;
    if let Some(language) = get_string_option(vm, options, "language")? {
        if !valid_language_option(&language) {
            return Err(Error::range("Invalid language option"));
        }
        parts.language = language;
    }
    if let Some(script) = get_string_option(vm, options, "script")? {
        if !is_script_subtag(&script) {
            return Err(Error::range("Invalid script option"));
        }
        parts.script = Some(script);
    }
    if let Some(region) = get_string_option(vm, options, "region")? {
        if !is_region_subtag(&region) {
            return Err(Error::range("Invalid region option"));
        }
        parts.region = Some(region);
    }
    if let Some(variants) = get_string_option(vm, options, "variants")? {
        if !valid_variants_option(&variants) {
            return Err(Error::range("Invalid variants option"));
        }
        parts.variants = variants.split('-').map(str::to_string).collect();
    }
    canonicalize_locale_metered(vm, &compose_locale_tag(&parts))
}

#[derive(Default)]
struct UnicodeExtension {
    attributes: Vec<String>,
    keywords: IndexMap<String, String>,
}

fn remove_unicode_extension(tag: &str) -> (String, UnicodeExtension) {
    let subtags: Vec<&str> = tag.split('-').collect();
    let Some(start) = subtags
        .iter()
        .take_while(|subtag| !subtag.eq_ignore_ascii_case("x"))
        .position(|subtag| subtag.eq_ignore_ascii_case("u"))
    else {
        return (tag.to_string(), UnicodeExtension::default());
    };
    let end = subtags[start + 1..]
        .iter()
        .position(|subtag| subtag.len() == 1)
        .map_or(subtags.len(), |offset| start + 1 + offset);
    let contents = &subtags[start + 1..end];
    let first_key = contents
        .iter()
        .position(|subtag| subtag.len() == 2)
        .unwrap_or(contents.len());
    let attributes = contents[..first_key]
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    let mut keywords = IndexMap::new();
    let mut index = first_key;
    while index < contents.len() {
        let key = contents[index].to_string();
        let value_start = index + 1;
        index = value_start;
        while index < contents.len() && contents[index].len() != 2 {
            index += 1;
        }
        keywords.insert(key, contents[value_start..index].join("-"));
    }
    let mut kept = Vec::with_capacity(subtags.len() - (end - start));
    kept.extend_from_slice(&subtags[..start]);
    kept.extend_from_slice(&subtags[end..]);
    (
        kept.join("-"),
        UnicodeExtension {
            attributes,
            keywords,
        },
    )
}

fn restore_unicode_extension(base: String, mut extension: UnicodeExtension) -> String {
    if extension.attributes.is_empty() && extension.keywords.is_empty() {
        return base;
    }
    extension.attributes.sort();
    let mut keywords: Vec<_> = extension.keywords.into_iter().collect();
    keywords.sort_by(|left, right| left.0.cmp(&right.0));
    let mut rendered = vec!["u".to_string()];
    rendered.extend(extension.attributes);
    for (key, value) in keywords {
        rendered.push(key);
        if value != "true" && !value.is_empty() {
            rendered.extend(value.split('-').map(str::to_string));
        }
    }
    let mut subtags: Vec<String> = base.split('-').map(str::to_string).collect();
    let insertion = subtags
        .iter()
        .position(|subtag| subtag.len() == 1 && subtag.as_str() > "u")
        .unwrap_or(subtags.len());
    subtags.splice(insertion..insertion, rendered);
    subtags.join("-")
}

fn update_unicode_extension(
    vm: &mut Vm,
    tag: &str,
    options: Option<&Value>,
) -> error::Result<Arc<str>> {
    let calendar = get_string_option(vm, options, "calendar")?;
    if calendar
        .as_deref()
        .is_some_and(|value| !valid_unicode_type(value))
    {
        return Err(Error::range("Invalid calendar option"));
    }
    let collation = get_string_option(vm, options, "collation")?;
    if collation
        .as_deref()
        .is_some_and(|value| !valid_unicode_type(value))
    {
        return Err(Error::range("Invalid collation option"));
    }
    let first_day_of_week =
        get_string_option(vm, options, "firstDayOfWeek")?.map(|value| match value.as_str() {
            "0" | "7" => "sun".to_string(),
            "1" => "mon".to_string(),
            "2" => "tue".to_string(),
            "3" => "wed".to_string(),
            "4" => "thu".to_string(),
            "5" => "fri".to_string(),
            "6" => "sat".to_string(),
            _ => value,
        });
    if first_day_of_week
        .as_deref()
        .is_some_and(|value| !valid_unicode_type(value))
    {
        return Err(Error::range("Invalid firstDayOfWeek option"));
    }
    let hour_cycle = get_string_option(vm, options, "hourCycle")?;
    if hour_cycle
        .as_deref()
        .is_some_and(|value| !matches!(value, "h11" | "h12" | "h23" | "h24"))
    {
        return Err(Error::range("Invalid hourCycle option"));
    }
    let case_first = get_string_option(vm, options, "caseFirst")?;
    if case_first
        .as_deref()
        .is_some_and(|value| !matches!(value, "upper" | "lower" | "false"))
    {
        return Err(Error::range("Invalid caseFirst option"));
    }
    let numeric = get_boolean_option(vm, options, "numeric")?;
    let numbering_system = get_string_option(vm, options, "numberingSystem")?;
    if numbering_system
        .as_deref()
        .is_some_and(|value| !valid_unicode_type(value))
    {
        return Err(Error::range("Invalid numberingSystem option"));
    }

    let (base, mut extension) = remove_unicode_extension(tag);
    for (key, value) in [
        ("ca", calendar),
        ("co", collation),
        ("fw", first_day_of_week),
        ("hc", hour_cycle),
        ("kf", case_first),
        ("kn", numeric.map(|value| value.to_string())),
        ("nu", numbering_system),
    ] {
        if let Some(value) = value {
            extension.keywords.insert(key.to_string(), value);
        }
    }
    canonicalize_locale_metered(vm, &restore_unicode_extension(base, extension))
}

fn locale_record(tag: Arc<str>) -> IntlLocaleRecord {
    let (_, extension) = remove_unicode_extension(&tag);
    let value = |key: &str| {
        extension.keywords.get(key).map(|value| {
            Arc::from(if value.is_empty() && key != "kf" {
                "true"
            } else {
                value
            })
        })
    };
    let numeric = extension
        .keywords
        .get("kn")
        .is_some_and(|value| value.is_empty() || value == "true");
    IntlLocaleRecord {
        locale: tag,
        calendar: value("ca"),
        case_first: value("kf"),
        collation: value("co"),
        first_day_of_week: value("fw"),
        hour_cycle: value("hc"),
        numbering_system: value("nu"),
        numeric,
    }
}

fn transform_likely_subtags(vm: &mut Vm, tag: &str, maximize: bool) -> error::Result<Arc<str>> {
    vm.consume_fuel_units(tag.len().div_ceil(64).min(i64::MAX as usize) as i64)?;
    let parts = locale_tag_parts(tag)?;
    let subtags = tag.bytes().filter(|byte| *byte == b'-').count() + 1;
    vm.consume_fuel_units(subtags.saturating_mul(subtags).min(i64::MAX as usize) as i64)?;
    let suffix = parts.suffix.join("-");
    let mut base_parts = parts;
    base_parts.suffix.clear();
    let base = compose_locale_tag(&base_parts);
    let (input, replacements) = substitute_long_language_subtags(&base);
    let mut locale = input
        .parse::<Locale>()
        .map_err(|_| Error::range(format!("Invalid language tag: {tag}")))?;
    let expander = LocaleExpander::new_extended();
    if maximize {
        expander.maximize(&mut locale.id);
    } else {
        expander.minimize(&mut locale.id);
    }
    let mut transformed = restore_long_language_subtags(locale.to_string(), replacements);
    if !suffix.is_empty() {
        transformed.push('-');
        transformed.push_str(&suffix);
    }
    canonicalize_locale_metered(vm, &transformed)
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
    if let Some(tag) = locale_tag(vm, locales) {
        return Ok(vec![Value::String(tag)]);
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
                let canonical = if let Some(tag) = locale_tag(vm, &value) {
                    tag
                } else {
                    let value_pin = vm.pin(&value);
                    let tag_result = vm.to_string(&value);
                    vm.unpin_many(value_pin);
                    canonicalize_locale_metered(vm, &tag_result?)?
                };
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

fn intl_locale_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err(
            "Intl.Locale constructor must be called with new",
        ));
    }
    let tag = args.first().cloned().unwrap_or(Value::Undefined);
    let raw_options = args.get(1).cloned().unwrap_or(Value::Undefined);
    vm.try_reserve_gc_pins(5)?;
    let mut pin_count = vm.pin_many(&[tag.clone(), raw_options.clone()]);
    let result = (|| {
        let realm = vm.current_realm_global_env();
        let fallback = vm
            .realm_intl_locale_prototypes
            .get(&realm.0)
            .cloned()
            .ok_or_else(|| Error::internal("Intl.Locale prototype is not installed"))?;
        let prototype = native_constructor_prototype_with_default(vm, "Intl.Locale", fallback)?;
        pin_count += vm.pin(&prototype);
        let index = vm.alloc(HeapObj::IntlLocale(IntlLocaleData {
            record: std::sync::OnceLock::new(),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
        }))?;
        let object = Value::Object(index);
        pin_count += vm.pin(&object);

        if !matches!(tag, Value::String(_) | Value::Object(_)) {
            return Err(Error::type_err(
                "Intl.Locale tag must be a string or object",
            ));
        }
        let input = if let Some(locale) = locale_tag(vm, &tag) {
            locale
        } else {
            vm.to_string(&tag)?
        };
        let options = if raw_options.is_undefined() {
            None
        } else {
            let object = vm.to_object(&raw_options)?;
            pin_count += vm.pin(&object);
            Some(object)
        };
        let canonical = canonicalize_locale_metered(vm, &input)?;
        let canonical = update_language_id(vm, &canonical, options.as_ref())?;
        let canonical = update_unicode_extension(vm, &canonical, options.as_ref())?;
        let record = locale_record(canonical);
        vm.heap.with_obj(index.0, |heap_object| {
            let HeapObj::IntlLocale(locale) = heap_object else {
                unreachable!("new Intl.Locale object changed representation")
            };
            locale
                .record
                .set(record)
                .map_err(|_| Error::internal("Intl.Locale initialized twice"))
        })?;
        Ok(object)
    })();
    vm.unpin_many(pin_count);
    result
}

fn locale_base_name(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let tag = require_locale_tag(vm, this)?;
    let mut parts = locale_tag_parts_metered(vm, &tag)?;
    parts.suffix.clear();
    Ok(Value::String(Arc::from(compose_locale_tag(&parts))))
}

fn locale_language(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let tag = require_locale_tag(vm, this)?;
    Ok(Value::String(Arc::from(
        locale_tag_parts_metered(vm, &tag)?.language,
    )))
}

fn locale_script(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let tag = require_locale_tag(vm, this)?;
    Ok(locale_tag_parts_metered(vm, &tag)?
        .script
        .map(|value| Value::String(Arc::from(value)))
        .unwrap_or(Value::Undefined))
}

fn locale_region(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let tag = require_locale_tag(vm, this)?;
    Ok(locale_tag_parts_metered(vm, &tag)?
        .region
        .map(|value| Value::String(Arc::from(value)))
        .unwrap_or(Value::Undefined))
}

fn locale_variants(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let tag = require_locale_tag(vm, this)?;
    let variants = locale_tag_parts_metered(vm, &tag)?.variants;
    Ok(if variants.is_empty() {
        Value::Undefined
    } else {
        Value::String(Arc::from(variants.join("-")))
    })
}

fn locale_calendar(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    locale_string_slot(vm, this, |record| record.calendar.clone())
}

fn locale_case_first(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    locale_string_slot(vm, this, |record| record.case_first.clone())
}

fn locale_collation(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    locale_string_slot(vm, this, |record| record.collation.clone())
}

fn locale_hour_cycle(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    locale_string_slot(vm, this, |record| record.hour_cycle.clone())
}

fn locale_numbering_system(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    locale_string_slot(vm, this, |record| record.numbering_system.clone())
}

fn locale_numeric(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let Value::Object(index) = receiver else {
        return Err(Error::type_err(
            "Intl.Locale accessor called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::IntlLocale(locale) => locale
            .record
            .get()
            .map(|record| Value::Bool(record.numeric))
            .ok_or_else(|| Error::type_err("Intl.Locale object is not initialized")),
        _ => Err(Error::type_err(
            "Intl.Locale accessor called on incompatible receiver",
        )),
    })
}

fn locale_first_day_of_week(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    locale_string_slot(vm, this, |record| record.first_day_of_week.clone())
}

fn require_locale_record(vm: &Vm, this: Option<Value>) -> error::Result<IntlLocaleRecord> {
    let receiver = this.unwrap_or(Value::Undefined);
    let Value::Object(index) = receiver else {
        return Err(Error::type_err(
            "Intl.Locale method called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::IntlLocale(locale) => locale
            .record
            .get()
            .cloned()
            .ok_or_else(|| Error::type_err("Intl.Locale object is not initialized")),
        _ => Err(Error::type_err(
            "Intl.Locale method called on incompatible receiver",
        )),
    })
}

fn string_list_data(
    table: &'static [(&'static str, &'static [&'static str])],
    key: &str,
) -> Option<&'static [&'static str]> {
    table
        .binary_search_by_key(&key, |(candidate, _)| *candidate)
        .ok()
        .and_then(|index| table.get(index).map(|(_, values)| *values))
}

fn intl_string_list_value(vm: &mut Vm, values: &[&str]) -> error::Result<Value> {
    let work = values.iter().try_fold(values.len(), |work, value| {
        work.checked_add(value.len().div_ceil(64))
            .ok_or_else(|| Error::range("Intl result is too large"))
    })?;
    vm.consume_fuel_units(work.min(i64::MAX as usize) as i64)?;
    let mut items = Vec::new();
    items
        .try_reserve_exact(values.len())
        .map_err(|_| Error::range("Intl result is too large"))?;
    items.extend(values.iter().map(|value| Value::String(Arc::from(*value))));
    make_value_array_in_current_realm(vm, items)
}

fn locale_info_object(vm: &mut Vm, properties: Vec<(&'static str, Value)>) -> error::Result<Value> {
    vm.try_reserve_gc_pins(properties.len())?;
    let values: Vec<Value> = properties.iter().map(|(_, value)| value.clone()).collect();
    let pin_count = vm.pin_many(&values);
    let result = vm.new_object_in_current_realm().map(|index| {
        vm.heap.with_obj(index.0, |object| {
            let mut props = object.props().lock();
            for (name, value) in properties {
                let mut descriptor = PropertyDescriptor::data(value);
                descriptor.writable = true;
                descriptor.enumerable = true;
                descriptor.configurable = true;
                props.insert(PropertyKey::from(name), descriptor);
            }
        });
        Value::Object(index)
    });
    vm.unpin_many(pin_count);
    result
}

fn canonical_subdivision_region(tag: &str, key: &str) -> Option<String> {
    let (_, extension) = remove_unicode_extension(tag);
    let subdivision = extension.keywords.get(key)?;
    if !(3..=8).contains(&subdivision.len())
        || !subdivision.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return None;
    }
    let prefix_len = if subdivision
        .as_bytes()
        .get(..2)
        .is_some_and(|prefix| prefix.iter().all(u8::is_ascii_alphabetic))
    {
        2
    } else if subdivision
        .as_bytes()
        .get(..3)
        .is_some_and(|prefix| prefix.iter().all(u8::is_ascii_digit))
    {
        3
    } else {
        return None;
    };
    if !(1..=4).contains(&(subdivision.len() - prefix_len)) {
        return None;
    }
    let canonical = canonicalize_locale(&format!("und-{}", &subdivision[..prefix_len])).ok()?;
    locale_tag_parts(&canonical).ok()?.region
}

struct RegionPreference {
    language: String,
    region: String,
    region_override: Option<String>,
}

fn likely_subtag_parts(
    vm: &mut Vm,
    parts: &LocaleTagParts,
) -> error::Result<Option<LocaleTagParts>> {
    let mut base_parts = parts.clone();
    base_parts.suffix.clear();
    let base = compose_locale_tag(&base_parts);
    let subtags = base.bytes().filter(|byte| *byte == b'-').count() + 1;
    vm.consume_fuel_units(subtags.saturating_mul(subtags).min(i64::MAX as usize) as i64)?;
    let (input, replacements) = substitute_long_language_subtags(&base);
    let Ok(mut locale) = input.parse::<Locale>() else {
        return Ok(None);
    };
    LocaleExpander::new_extended().maximize(&mut locale.id);
    let maximal = restore_long_language_subtags(locale.to_string(), replacements);
    Ok(locale_tag_parts(&maximal).ok())
}

fn region_preference(vm: &mut Vm, tag: &str) -> error::Result<RegionPreference> {
    let parts = locale_tag_parts_metered(vm, tag)?;
    let region = if let Some(region) = parts.region.clone() {
        region
    } else if let Some(region) = canonical_subdivision_region(tag, "sd") {
        region
    } else {
        likely_subtag_parts(vm, &parts)?
            .and_then(|maximal| maximal.region)
            .unwrap_or_else(|| "001".to_string())
    };
    Ok(RegionPreference {
        language: parts.language,
        region,
        region_override: canonical_subdivision_region(tag, "rg"),
    })
}

fn preferred_region_data(
    table: &'static [(&'static str, &'static [&'static str])],
    preference: &RegionPreference,
) -> Option<&'static [&'static str]> {
    let mut regions = [
        preference.region_override.as_deref(),
        Some(&preference.region),
    ];
    for region in regions.iter_mut().filter_map(Option::take) {
        let locale = format!("{}-{region}", preference.language);
        if let Some(values) = string_list_data(table, &locale) {
            return Some(values);
        }
        if let Some(values) = string_list_data(table, region) {
            return Some(values);
        }
        if let Some(values) = string_list_data(table, "001") {
            return Some(values);
        }
    }
    None
}

fn locale_get_calendars(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let record = require_locale_record(vm, this)?;
    if let Some(calendar) = record.calendar {
        return make_value_array_in_current_realm(vm, vec![Value::String(calendar)]);
    }
    let preference = region_preference(vm, &record.locale)?;
    intl_string_list_value(
        vm,
        preferred_region_data(CALENDAR_PREFERENCES, &preference).unwrap_or(&["gregory"]),
    )
}

fn locale_get_collations(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let record = require_locale_record(vm, this)?;
    if let Some(collation) = record.collation {
        return make_value_array_in_current_realm(vm, vec![Value::String(collation)]);
    }
    intl_string_list_value(vm, &["emoji", "eor"])
}

fn locale_get_hour_cycles(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let record = require_locale_record(vm, this)?;
    if let Some(hour_cycle) = record.hour_cycle {
        return make_value_array_in_current_realm(vm, vec![Value::String(hour_cycle)]);
    }
    let preference = region_preference(vm, &record.locale)?;
    intl_string_list_value(
        vm,
        preferred_region_data(HOUR_CYCLES, &preference).unwrap_or(&["h23"]),
    )
}

fn locale_get_numbering_systems(
    vm: &mut Vm,
    _: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let record = require_locale_record(vm, this)?;
    if let Some(numbering_system) = record.numbering_system {
        return make_value_array_in_current_realm(vm, vec![Value::String(numbering_system)]);
    }
    intl_string_list_value(vm, &["latn"])
}

fn locale_get_time_zones(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let record = require_locale_record(vm, this)?;
    let Some(region) = locale_tag_parts_metered(vm, &record.locale)?.region else {
        return Ok(Value::Undefined);
    };
    intl_string_list_value(vm, string_list_data(TIME_ZONES, &region).unwrap_or(&[]))
}

fn locale_get_text_info(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let record = require_locale_record(vm, this)?;
    let parts = locale_tag_parts_metered(vm, &record.locale)?;
    let script = if parts.script.is_some() {
        parts.script.clone()
    } else {
        likely_subtag_parts(vm, &parts)?.and_then(|maximal| maximal.script)
    };
    let direction = script
        .as_deref()
        .and_then(|script| {
            SCRIPT_DIRECTIONS
                .binary_search_by_key(&script, |(candidate, _)| *candidate)
                .ok()
                .and_then(|index| SCRIPT_DIRECTIONS.get(index).map(|(_, value)| *value))
        })
        .map(|value| Value::String(Arc::from(value)))
        .unwrap_or(Value::Undefined);
    locale_info_object(vm, vec![("direction", direction)])
}

fn locale_get_week_info(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let record = require_locale_record(vm, this)?;
    let preference = region_preference(vm, &record.locale)?;
    let lookup_region = preference
        .region_override
        .as_deref()
        .unwrap_or(&preference.region);
    let week_data = week_information(lookup_region)
        .or_else(|| week_information("001"))
        .ok_or_else(|| Error::internal("CLDR world week data is missing"))?;
    let first_day = record
        .first_day_of_week
        .as_deref()
        .and_then(weekday_number)
        .unwrap_or(week_data.0);
    let weekend = make_value_array_in_current_realm(
        vm,
        week_data
            .1
            .iter()
            .map(|day| Value::Number(f64::from(*day)))
            .collect(),
    )?;
    locale_info_object(
        vm,
        vec![
            ("firstDay", Value::Number(f64::from(first_day))),
            ("weekend", weekend),
        ],
    )
}

fn week_information(region: &str) -> Option<(u8, &'static [u8])> {
    WEEK_INFORMATION
        .binary_search_by_key(&region, |(candidate, _, _)| *candidate)
        .ok()
        .and_then(|index| {
            WEEK_INFORMATION
                .get(index)
                .map(|(_, first_day, weekend)| (*first_day, *weekend))
        })
}

fn weekday_number(value: &str) -> Option<u8> {
    match value {
        "mon" => Some(1),
        "tue" => Some(2),
        "wed" => Some(3),
        "thu" => Some(4),
        "fri" => Some(5),
        "sat" => Some(6),
        "sun" => Some(7),
        _ => None,
    }
}

fn locale_to_string(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    require_locale_tag(vm, this).map(Value::String)
}

fn construct_intrinsic_locale(vm: &mut Vm, tag: Arc<str>) -> error::Result<Value> {
    let realm = vm.current_realm_global_env();
    let constructor = vm
        .realm_intl_locale_constructors
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("Intl.Locale constructor is not installed"))?;
    let pin_count = vm.pin(&constructor);
    let result = vm.construct(&constructor, &[Value::String(tag)]);
    vm.unpin_many(pin_count);
    result
}

fn locale_maximize(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let tag = require_locale_tag(vm, this)?;
    let transformed = transform_likely_subtags(vm, &tag, true)?;
    construct_intrinsic_locale(vm, transformed)
}

fn locale_minimize(vm: &mut Vm, _: &[Value], this: Option<Value>) -> error::Result<Value> {
    let tag = require_locale_tag(vm, this)?;
    let transformed = transform_likely_subtags(vm, &tag, false)?;
    construct_intrinsic_locale(vm, transformed)
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

fn intl_supported_values_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let key = vm.to_string(args.first().unwrap_or(&Value::Undefined))?;
    let values = match key.as_ref() {
        "calendar" => SUPPORTED_VALUE_CALENDARS,
        // Formatter-dependent lists remain empty until their service
        // constructors establish which values RuJa actually supports.
        "collation" | "currency" => &[],
        "numberingSystem" => SUPPORTED_VALUE_NUMBERING_SYSTEMS,
        "timeZone" => SUPPORTED_VALUE_TIME_ZONES,
        "unit" => SUPPORTED_VALUE_UNITS,
        _ => return Err(Error::range("Invalid key for Intl.supportedValuesOf")),
    };
    intl_string_list_value(vm, values)
}

fn build_locale_intrinsic_in_env(
    vm: &mut Vm,
    env: GcIdx,
    object_proto: Value,
) -> error::Result<Value> {
    const METHODS: [(&str, NativeFn, usize); 10] = [
        ("toString", locale_to_string as NativeFn, 0),
        ("maximize", locale_maximize as NativeFn, 0),
        ("minimize", locale_minimize as NativeFn, 0),
        ("getCalendars", locale_get_calendars as NativeFn, 0),
        ("getCollations", locale_get_collations as NativeFn, 0),
        ("getHourCycles", locale_get_hour_cycles as NativeFn, 0),
        (
            "getNumberingSystems",
            locale_get_numbering_systems as NativeFn,
            0,
        ),
        ("getTimeZones", locale_get_time_zones as NativeFn, 0),
        ("getTextInfo", locale_get_text_info as NativeFn, 0),
        ("getWeekInfo", locale_get_week_info as NativeFn, 0),
    ];
    const GETTERS: [(&str, NativeFn); 12] = [
        ("baseName", locale_base_name as NativeFn),
        ("calendar", locale_calendar as NativeFn),
        ("caseFirst", locale_case_first as NativeFn),
        ("collation", locale_collation as NativeFn),
        ("firstDayOfWeek", locale_first_day_of_week as NativeFn),
        ("hourCycle", locale_hour_cycle as NativeFn),
        ("language", locale_language as NativeFn),
        ("numberingSystem", locale_numbering_system as NativeFn),
        ("numeric", locale_numeric as NativeFn),
        ("region", locale_region as NativeFn),
        ("script", locale_script as NativeFn),
        ("variants", locale_variants as NativeFn),
    ];

    // Provisional functions must survive a GC retry until both intrinsic
    // objects and their Realm registry roots have been published.
    vm.try_reserve_gc_pins(1 + METHODS.len() + GETTERS.len() + 2)?;
    let mut pin_count = vm.pin(&object_proto);
    let result = (|| {
        let mut prototype_props = IndexMap::new();
        prototype_props
            .try_reserve(METHODS.len() + GETTERS.len() + 2)
            .map_err(|_| Error::range("Intl.Locale prototype is too large"))?;

        for (name, function, length) in METHODS {
            let method = Value::Object(
                vm.new_native_function_in_env_with_gc_retry(name, function, length, env)?,
            );
            pin_count += vm.pin(&method);
            prototype_props.insert(PropertyKey::from(name), data_prop(method));
        }
        for (property, getter) in GETTERS {
            let name = format!("get {property}");
            let function =
                Value::Object(vm.new_native_function_in_env_with_gc_retry(&name, getter, 0, env)?);
            pin_count += vm.pin(&function);
            prototype_props.insert(PropertyKey::from(property), accessor_get_prop(function));
        }

        let mut tag = PropertyDescriptor::data(Value::String(Arc::from("Intl.Locale")));
        tag.writable = false;
        tag.enumerable = false;
        tag.configurable = true;
        prototype_props.insert(
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );

        let prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(prototype_props),
            proto: Mutex::new(Some(object_proto)),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&prototype);

        let constructor = Value::Object(vm.new_native_constructor_in_env_with_gc_retry(
            "Locale",
            intl_locale_constructor as NativeFn,
            1,
            env,
            NativeConstructMode::InternalEagerPrototype,
        )?);
        pin_count += vm.pin(&constructor);

        let Value::Object(constructor_index) = &constructor else {
            unreachable!("native constructor allocation returned a non-object")
        };
        vm.heap.with_obj(constructor_index.0, |object| {
            let HeapObj::Function(function) = object else {
                unreachable!("native constructor allocation returned a non-function")
            };
            *function.prototype.lock() = Some(prototype.clone());
            function.props.lock().insert(
                PropertyKey::from("prototype"),
                const_prop(prototype.clone()),
            );
        });
        let Value::Object(prototype_index) = &prototype else {
            unreachable!("Locale prototype allocation returned a non-object")
        };
        vm.heap.with_obj(prototype_index.0, |object| {
            object.props().lock().insert(
                PropertyKey::from("constructor"),
                data_prop(constructor.clone()),
            );
        });

        vm.realm_intl_locale_constructors
            .insert(env.0, constructor.clone());
        vm.realm_intl_locale_prototypes.insert(env.0, prototype);
        Ok(constructor)
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn build_intl_in_env(
    vm: &mut Vm,
    env: GcIdx,
    object_proto: Value,
) -> error::Result<Value> {
    vm.try_reserve_gc_pins(4)?;
    let mut pin_count = vm.pin(&object_proto);
    let result = (|| {
        let locale = build_locale_intrinsic_in_env(vm, env, object_proto.clone())?;
        pin_count += vm.pin(&locale);
        let canonical_locales_idx = vm.new_native_function_in_env_with_gc_retry(
            "getCanonicalLocales",
            intl_get_canonical_locales as NativeFn,
            1,
            env,
        )?;
        let canonical_locales = Value::Object(canonical_locales_idx);
        pin_count += vm.pin(&canonical_locales);
        let supported_values_idx = vm.new_native_function_in_env_with_gc_retry(
            "supportedValuesOf",
            intl_supported_values_of as NativeFn,
            1,
            env,
        )?;
        let supported_values = Value::Object(supported_values_idx);
        pin_count += vm.pin(&supported_values);

        let mut props = IndexMap::new();
        props
            .try_reserve(4)
            .map_err(|_| Error::range("Intl namespace is too large"))?;
        props.insert(PropertyKey::from("Locale"), data_prop(locale));
        props.insert(
            PropertyKey::from("getCanonicalLocales"),
            data_prop(canonical_locales),
        );
        props.insert(
            PropertyKey::from("supportedValuesOf"),
            data_prop(supported_values),
        );
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
    use super::{
        canonical_subdivision_region, canonicalize_locale, region_preference, string_list_data,
        week_information, CALENDAR_PREFERENCES, HOUR_CYCLES, TIME_ZONES,
    };
    use crate::value::Value;
    use crate::vm::OwnKeyConsumerReservationSite;
    use crate::vm::Vm;
    use std::sync::Arc;

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
    fn supported_values_publication_failures_release_roots_and_recover() {
        let mut vm = Vm::new().expect("failed to initialize VM");
        let baseline_pins = vm.gc_pins.len();

        vm.fail_own_key_consumer_reservation =
            Some((OwnKeyConsumerReservationSite::ArrayPresence, 0));
        let reserve_error = vm
            .run("Intl.supportedValuesOf('calendar')")
            .expect_err("Array presence reservation failure should propagate");
        assert!(reserve_error
            .to_string()
            .contains("Array presence bitmap is too large"));
        assert_eq!(vm.gc_pins.len(), baseline_pins);

        vm.gc();
        let exact_cap = vm.heap.live_count();
        vm.set_max_heap_objects(Some(exact_cap));
        let cap_error = vm
            .run("Intl.supportedValuesOf('calendar')")
            .expect_err("exact live-object cap should reject the result Array");
        assert!(cap_error.to_string().contains("heap limit exceeded"));
        assert_eq!(vm.gc_pins.len(), baseline_pins);

        vm.set_max_heap_objects(None);
        assert_eq!(
            vm.run("Intl.supportedValuesOf('calendar')[0]")
                .expect("VM should recover after publication failures"),
            Value::String(Arc::from("buddhist"))
        );
        assert_eq!(vm.gc_pins.len(), baseline_pins);
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

    #[test]
    fn locale_info_tables_are_sorted_and_cover_world_fallbacks() {
        for table in [CALENDAR_PREFERENCES, HOUR_CYCLES, TIME_ZONES] {
            assert!(table.windows(2).all(|pair| pair[0].0 < pair[1].0));
        }
        assert_eq!(
            string_list_data(CALENDAR_PREFERENCES, "TH"),
            Some(&["buddhist", "gregory"][..])
        );
        assert_eq!(
            string_list_data(HOUR_CYCLES, "US"),
            Some(&["h12", "h23"][..])
        );
        assert!(string_list_data(TIME_ZONES, "US")
            .is_some_and(|zones| zones.windows(2).all(|pair| pair[0] < pair[1])));
        assert_eq!(week_information("001"), Some((1, &[6, 7][..])));
    }

    #[test]
    fn region_preference_validates_subdivisions_and_obeys_priority() {
        assert_eq!(
            canonical_subdivision_region("fa-u-sd-thabcd", "sd").as_deref(),
            Some("TH")
        );
        assert_eq!(canonical_subdivision_region("fa-u-sd-thabcde", "sd"), None);

        let mut vm = Vm::new().expect("VM should initialize");
        let override_preference = region_preference(&mut vm, "fa-JP-u-sd-inka-rg-thzzzz")
            .expect("region preference should resolve");
        assert_eq!(override_preference.region, "JP");
        assert_eq!(override_preference.region_override.as_deref(), Some("TH"));

        let subdivision_preference = region_preference(&mut vm, "fa-u-sd-inka")
            .expect("subdivision preference should resolve");
        assert_eq!(subdivision_preference.region, "IN");

        let likely_preference =
            region_preference(&mut vm, "fa").expect("likely region should resolve");
        assert_eq!(likely_preference.region, "IR");
    }
}
