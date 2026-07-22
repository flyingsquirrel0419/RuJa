//! Built-in objects and globals for the RuJa VM.
//!
//! All built-in constructors, prototypes, and global functions are registered
//! here. Native functions follow the `NativeFn` signature used by the VM.

pub(crate) mod global;
pub(crate) mod json;
pub(crate) mod math;

pub(crate) mod call_arguments;

pub(crate) mod array;
pub(crate) use array::*;

pub(crate) mod string;
pub(crate) use string::*;

pub(crate) mod collections;
pub(crate) use collections::*;
pub(crate) mod regexp;
pub(crate) use regexp::*;
pub(crate) mod function;
pub(crate) mod proxy;
pub(crate) mod typed_array;
pub(crate) use function::*;
pub(crate) use global::{
    async_function_constructor, async_generator_function_constructor, bigint_as_int_n,
    bigint_as_uint_n, bigint_to_string, bigint_value_of, function_constructor,
    generator_function_constructor, global_bigint, global_eval, global_is_finite, global_is_nan,
    global_parse_float, global_parse_int,
};
pub(crate) use json::{
    build_json, build_reflect, build_reflect_in_env, date_constructor, date_get_component,
    date_get_time, date_get_timezone_offset, date_now, date_parse, date_set_component,
    date_to_iso_string, date_to_json, date_to_primitive, date_to_string, date_to_temporal_instant,
    date_utc,
};
pub(crate) use math::{build_console, build_math_in_env};
pub(crate) use proxy::*;
pub(crate) use typed_array::*;

use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{
    ArrayData, BindingKind, CollectionIteratorData, CollectionIteratorKind, FunctionData,
    FunctionKind, GcIdx, HeapObj, IteratorConcatIterable, IteratorHelperData, IteratorHelperInner,
    IteratorHelperKind, IteratorZipMode, MapData, MapKey, NativeConstructMode, ObjectData,
    PropertyDescriptor, PropertyKey, RegExpStringIteratorData, SetData, Value,
};
use crate::vm::{NativeFn, Vm};
use indexmap::{IndexMap, IndexSet};
use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_rational::Ratio;
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};
use regex::{Regex as RustRegex, RegexBuilder as RustRegexBuilder};
use regex_syntax::hir::{Class, ClassUnicode, ClassUnicodeRange, Hir, HirKind};
use regex_syntax::ParserBuilder as RegexSyntaxParserBuilder;
use std::borrow::Cow;

#[derive(Clone, Copy)]
struct CompiledMatch<'t> {
    text: &'t str,
    start: usize,
    end: usize,
}

impl<'t> CompiledMatch<'t> {
    fn as_str(self) -> &'t str {
        self.text
    }

    fn start(self) -> usize {
        self.start
    }

    fn end(self) -> usize {
        self.end
    }
}

struct CompiledCaptures<'t> {
    groups: Vec<Option<CompiledMatch<'t>>>,
}

struct RegexCaptureName {
    name: Arc<str>,
    index: usize,
}

impl<'t> CompiledCaptures<'t> {
    fn get(&self, index: usize) -> Option<CompiledMatch<'t>> {
        self.groups.get(index).copied().flatten()
    }

    fn iter(&self) -> impl Iterator<Item = Option<CompiledMatch<'t>>> + '_ {
        self.groups.iter().copied()
    }

    fn len(&self) -> usize {
        self.groups.len()
    }
}

enum CompiledRegex {
    Rust(RustRegex),
    Fancy(fancy_regex::Regex),
    CaptureCorrected {
        fast: RustRegex,
        captures: fancy_regex::Regex,
    },
}

#[derive(Clone, Copy)]
struct RegexModifierState {
    dot_all: bool,
    ignore_case: bool,
}

/// Compile a regex pattern applying ES flags: `i` (case-insensitive),
/// `m` (multiline ^/$), `s` (dotall). Other flags (`g`/`y`/`u`) do not affect
/// the regex engine here and are handled by the caller.
fn compile_regex(source: &str, flags: &str) -> Result<CompiledRegex, String> {
    compile_regex_with_input_mode(source, flags, false)
}

fn compile_regex_for_code_units(source: &str, flags: &str) -> Result<CompiledRegex, String> {
    compile_regex_with_input_mode(source, flags, true)
}

fn compile_regex_with_input_mode(
    source: &str,
    flags: &str,
    code_unit_input: bool,
) -> Result<CompiledRegex, String> {
    let capture_count = regex_capture_count(source);
    let capture_names = regex_capture_names(source, flags)?;
    let capture_indices = regex_capture_indices_by_name(&capture_names);
    let rewritten_source = rewrite_named_regex_groups_for_backend(source, flags, &capture_indices)?;
    let uses_backreference = regex_uses_backreference(
        &rewritten_source,
        capture_count,
        !capture_indices.is_empty(),
    );
    let uses_lookaround = regex_uses_lookaround(&rewritten_source);
    let needs_capture_correction = regex_contains_quantified_capture_group(&rewritten_source);
    if uses_backreference || uses_lookaround {
        let normalized = normalize_regex_for_backend(
            &rewritten_source,
            flags,
            capture_count,
            code_unit_input,
            true,
            &capture_indices,
        )?;
        let mut b = fancy_regex::RegexBuilder::new(&normalized.source);
        b.case_insensitive(flags.contains('i'));
        b.multi_line(flags.contains('m'));
        b.dot_matches_new_line(flags.contains('s'));
        b.ecmascript_mode(true);
        b.ecmascript_unicode_mode(flags.contains('u') || flags.contains('v'));
        b.ecmascript_backref_sets(normalized.backref_sets);
        return b
            .build()
            .map(CompiledRegex::Fancy)
            .map_err(|e| e.to_string());
    }

    let rust_normalized = normalize_regex_for_backend(
        &rewritten_source,
        flags,
        capture_count,
        code_unit_input,
        false,
        &capture_indices,
    )?;
    let mut b = RustRegexBuilder::new(&rust_normalized.source);
    b.case_insensitive(flags.contains('i'));
    b.multi_line(flags.contains('m'));
    b.dot_matches_new_line(flags.contains('s'));
    let fast = b.build().map_err(|e| e.to_string())?;
    if !needs_capture_correction {
        return Ok(CompiledRegex::Rust(fast));
    }

    let capture_normalized = normalize_regex_for_backend(
        &rewritten_source,
        flags,
        capture_count,
        code_unit_input,
        true,
        &capture_indices,
    )?;
    let mut b = fancy_regex::RegexBuilder::new(&capture_normalized.source);
    b.case_insensitive(flags.contains('i'));
    b.multi_line(flags.contains('m'));
    b.dot_matches_new_line(flags.contains('s'));
    b.ecmascript_mode(true);
    b.ecmascript_unicode_mode(flags.contains('u') || flags.contains('v'));
    b.ecmascript_backref_sets(capture_normalized.backref_sets);
    let captures = b.build().map_err(|e| e.to_string())?;
    Ok(CompiledRegex::CaptureCorrected { fast, captures })
}

impl CompiledRegex {
    fn find<'t>(&self, input: &'t str) -> error::Result<Option<CompiledMatch<'t>>> {
        self.find_at(input, 0)
    }

    fn find_at<'t>(
        &self,
        input: &'t str,
        start: usize,
    ) -> error::Result<Option<CompiledMatch<'t>>> {
        match self {
            CompiledRegex::Rust(re) | CompiledRegex::CaptureCorrected { fast: re, .. } => {
                Ok(re.find_at(input, start).map(CompiledMatch::from))
            }
            CompiledRegex::Fancy(re) => re
                .find_from_pos(input, start)
                .map(|m| m.map(CompiledMatch::from))
                .map_err(regex_runtime_error),
        }
    }

    fn find_iter<'t>(&self, input: &'t str) -> error::Result<Vec<CompiledMatch<'t>>> {
        match self {
            CompiledRegex::Rust(re) | CompiledRegex::CaptureCorrected { fast: re, .. } => {
                Ok(re.find_iter(input).map(CompiledMatch::from).collect())
            }
            CompiledRegex::Fancy(re) => {
                let mut matches = Vec::new();
                let mut pos = 0;
                while pos <= input.len() {
                    let Some(m) = re.find_from_pos(input, pos).map_err(regex_runtime_error)? else {
                        break;
                    };
                    let start = m.start();
                    let end = m.end();
                    matches.push(CompiledMatch::from(m));
                    if end == pos {
                        match input[end..].chars().next() {
                            Some(ch) => pos = end + ch.len_utf8(),
                            None => break,
                        }
                    } else {
                        pos = end.max(start + 1);
                    }
                }
                Ok(matches)
            }
        }
    }

    fn captures<'t>(&self, input: &'t str) -> error::Result<Option<CompiledCaptures<'t>>> {
        self.captures_at(input, 0)
    }

    fn captures_at<'t>(
        &self,
        input: &'t str,
        start: usize,
    ) -> error::Result<Option<CompiledCaptures<'t>>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.captures_at(input, start).map(CompiledCaptures::from)),
            CompiledRegex::Fancy(re) => re
                .captures_from_pos(input, start)
                .map(|caps| caps.map(CompiledCaptures::from))
                .map_err(regex_runtime_error),
            CompiledRegex::CaptureCorrected { fast, captures } => {
                let Some(expected) = fast.find_at(input, start) else {
                    return Ok(None);
                };
                corrected_captures(captures, input, expected.start(), expected.end()).map(Some)
            }
        }
    }

    fn captures_iter<'t>(&self, input: &'t str) -> error::Result<Vec<CompiledCaptures<'t>>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re
                .captures_iter(input)
                .map(CompiledCaptures::from)
                .collect()),
            CompiledRegex::Fancy(re) => {
                let mut captures = Vec::new();
                let mut pos = 0;
                while pos <= input.len() {
                    let Some(caps) = re
                        .captures_from_pos(input, pos)
                        .map_err(regex_runtime_error)?
                    else {
                        break;
                    };
                    let Some(m) = caps.get(0) else {
                        break;
                    };
                    let end = m.end();
                    captures.push(CompiledCaptures::from(caps));
                    if end == pos {
                        match input[end..].chars().next() {
                            Some(ch) => pos = end + ch.len_utf8(),
                            None => break,
                        }
                    } else {
                        pos = end;
                    }
                }
                Ok(captures)
            }
            CompiledRegex::CaptureCorrected { fast, captures } => fast
                .find_iter(input)
                .map(|expected| {
                    corrected_captures(captures, input, expected.start(), expected.end())
                })
                .collect(),
        }
    }

    fn replace<'t>(&self, input: &'t str, replacement: &str) -> error::Result<Cow<'t, str>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.replace(input, replacement)),
            CompiledRegex::Fancy(_) | CompiledRegex::CaptureCorrected { .. } => {
                self.replace_fancy(input, replacement, false)
            }
        }
    }

    fn replace_all<'t>(&self, input: &'t str, replacement: &str) -> error::Result<Cow<'t, str>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.replace_all(input, replacement)),
            CompiledRegex::Fancy(_) | CompiledRegex::CaptureCorrected { .. } => {
                self.replace_fancy(input, replacement, true)
            }
        }
    }

    fn replace_fancy<'t>(
        &self,
        input: &'t str,
        replacement: &str,
        global: bool,
    ) -> error::Result<Cow<'t, str>> {
        let mut result = String::new();
        let mut last_end = 0;
        let mut replaced = false;
        for caps in self.captures_iter(input)? {
            let Some(m) = caps.get(0) else {
                continue;
            };
            result.push_str(&input[last_end..m.start()]);
            result.push_str(replacement);
            last_end = m.end();
            replaced = true;
            if !global {
                break;
            }
        }
        if !replaced {
            return Ok(Cow::Borrowed(input));
        }
        result.push_str(&input[last_end..]);
        Ok(Cow::Owned(result))
    }
}

fn corrected_captures<'t>(
    re: &fancy_regex::Regex,
    input: &'t str,
    expected_start: usize,
    expected_end: usize,
) -> error::Result<CompiledCaptures<'t>> {
    let caps = re
        .captures_from_pos(input, expected_start)
        .map_err(regex_runtime_error)?
        .ok_or_else(|| Error::internal("capture backend lost a prefiltered RegExp match"))?;
    let actual = caps
        .get(0)
        .ok_or_else(|| Error::internal("capture backend omitted RegExp group zero"))?;
    if actual.start() != expected_start || actual.end() != expected_end {
        return Err(Error::internal(
            "capture backend disagreed with the prefiltered RegExp match",
        ));
    }
    Ok(CompiledCaptures::from(caps))
}

impl<'t> From<regex::Match<'t>> for CompiledMatch<'t> {
    fn from(value: regex::Match<'t>) -> Self {
        Self {
            text: value.as_str(),
            start: value.start(),
            end: value.end(),
        }
    }
}

impl<'t> From<fancy_regex::Match<'t>> for CompiledMatch<'t> {
    fn from(value: fancy_regex::Match<'t>) -> Self {
        Self {
            text: value.as_str(),
            start: value.start(),
            end: value.end(),
        }
    }
}

impl<'t> From<regex::Captures<'t>> for CompiledCaptures<'t> {
    fn from(value: regex::Captures<'t>) -> Self {
        Self {
            groups: (0..value.len())
                .map(|index| value.get(index).map(CompiledMatch::from))
                .collect(),
        }
    }
}

impl<'t> From<fancy_regex::Captures<'t>> for CompiledCaptures<'t> {
    fn from(value: fancy_regex::Captures<'t>) -> Self {
        Self {
            groups: (0..value.len())
                .map(|index| value.get(index).map(CompiledMatch::from))
                .collect(),
        }
    }
}

fn regex_runtime_error(error: fancy_regex::Error) -> Arc<Error> {
    Error::syntax(format!("Invalid regex match: {error}"))
}

// Rust regex uses Unicode Nd/White_Space; ECMAScript uses ASCII digits and
// its lexical WhiteSpace plus LineTerminator set (including FEFF, excluding 0085).
const ECMASCRIPT_WHITESPACE_CLASS_BODY: &str = r"\x09-\x0d\x20\u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}-\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";
const ECMASCRIPT_WORD_CLASS_BODY: &str = "A-Za-z0-9_";
const ECMASCRIPT_UNICODE_IGNORE_CASE_WORD_CLASS_BODY: &str = r"A-Za-z0-9_\u{17f}\u{212a}";

fn ecmascript_word_class_body(unicode_mode: bool) -> &'static str {
    if unicode_mode {
        ECMASCRIPT_UNICODE_IGNORE_CASE_WORD_CLASS_BODY
    } else {
        ECMASCRIPT_WORD_CLASS_BODY
    }
}

fn push_ecmascript_word_escape_for_backend(
    out: &mut String,
    escape: char,
    in_class: bool,
    unicode_mode: bool,
) {
    out.pop();
    if in_class && !unicode_mode {
        escape_annex_b_hyphen_before_class_set(out);
    }
    if !in_class {
        out.push_str("(?-i:");
    }
    out.push('[');
    if escape == 'W' {
        out.push('^');
    }
    out.push_str(ecmascript_word_class_body(unicode_mode));
    out.push(']');
    if !in_class {
        out.push(')');
    }
}

fn push_ecmascript_word_boundary_for_backend(
    out: &mut String,
    escape: char,
    unicode_ignore_case: bool,
) {
    out.pop();
    let word = ecmascript_word_class_body(unicode_ignore_case);
    out.push_str("(?-i:(?:");
    match escape {
        'b' => {
            out.push_str("(?<=[");
            out.push_str(word);
            out.push_str("])(?![");
            out.push_str(word);
            out.push_str("])|(?<![");
            out.push_str(word);
            out.push_str("])(?=[");
            out.push_str(word);
            out.push_str("])");
        }
        'B' => {
            out.push_str("(?<=[");
            out.push_str(word);
            out.push_str("])(?=[");
            out.push_str(word);
            out.push_str("])|(?<![");
            out.push_str(word);
            out.push_str("])(?![");
            out.push_str(word);
            out.push_str("])");
        }
        _ => unreachable!(),
    }
    out.push_str("))");
}

fn legacy_regex_canonicalize_code_unit(unit: u16) -> u16 {
    if (0xd800..=0xdfff).contains(&unit) {
        return unit;
    }
    let Some(ch) = char::from_u32(unit as u32) else {
        return unit;
    };
    let mut uppercase = ch.to_uppercase();
    let Some(mapped) = uppercase.next() else {
        return unit;
    };
    if uppercase.next().is_some() || mapped.len_utf16() != 1 {
        return unit;
    }
    let mapped = mapped as u32 as u16;
    if unit >= 0x80 && mapped < 0x80 {
        unit
    } else {
        mapped
    }
}

fn legacy_regex_canonical_code_units() -> &'static [u16] {
    static CODE_UNITS: OnceLock<Box<[u16]>> = OnceLock::new();
    CODE_UNITS.get_or_init(|| {
        (0..=u16::MAX)
            .map(legacy_regex_canonicalize_code_unit)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    })
}

fn internal_char_for_code_unit(unit: u16) -> char {
    if (0xd800..=0xdfff).contains(&unit) {
        char::from_u32(0xf0000 + (unit as u32 - 0xd800)).unwrap()
    } else {
        char::from_u32(unit as u32).unwrap()
    }
}

type LegacyCaseFoldGroup = (u16, Box<[u16]>);
type LegacyCaseFoldGroups = Box<[LegacyCaseFoldGroup]>;

fn legacy_case_fold_groups() -> &'static [LegacyCaseFoldGroup] {
    static GROUPS: OnceLock<LegacyCaseFoldGroups> = OnceLock::new();
    GROUPS.get_or_init(|| {
        let mut pairs = (0..=u16::MAX)
            .map(|unit| (legacy_regex_canonical_code_units()[unit as usize], unit))
            .collect::<Vec<_>>();
        pairs.sort_unstable();
        let mut groups = Vec::new();
        let mut start = 0;
        while start < pairs.len() {
            let mut end = start + 1;
            while end < pairs.len() && pairs[end].0 == pairs[start].0 {
                end += 1;
            }
            if end - start > 1 {
                groups.push((
                    pairs[start].0,
                    pairs[start..end]
                        .iter()
                        .map(|(_, unit)| *unit)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ));
            }
            start = end;
        }
        groups.into_boxed_slice()
    })
}

fn class_contains_char(class: &ClassUnicode, ch: char) -> bool {
    class
        .ranges()
        .binary_search_by(|range| {
            if ch < range.start() {
                std::cmp::Ordering::Greater
            } else if ch > range.end() {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn legacy_case_fold_class(class: &ClassUnicode) -> ClassUnicode {
    let mut additions = Vec::new();
    for (_, group) in legacy_case_fold_groups() {
        if group
            .iter()
            .any(|unit| class_contains_char(class, internal_char_for_code_unit(*unit)))
        {
            additions.extend(group.iter().map(|unit| {
                let ch = internal_char_for_code_unit(*unit);
                ClassUnicodeRange::new(ch, ch)
            }));
        }
    }
    let mut folded = class.clone();
    folded.union(&ClassUnicode::new(additions));
    folded
}

fn push_legacy_case_fold_atom_for_backend(out: &mut String, unit: u16) {
    let canonical = legacy_regex_canonical_code_units()[unit as usize];
    let group = legacy_case_fold_groups()
        .binary_search_by_key(&canonical, |(key, _)| *key)
        .ok()
        .map(|index| legacy_case_fold_groups()[index].1.as_ref());
    let ranges = group
        .unwrap_or(core::slice::from_ref(&unit))
        .iter()
        .map(|member| {
            let ch = internal_char_for_code_unit(*member);
            ClassUnicodeRange::new(ch, ch)
        });
    let class = Hir::class(Class::Unicode(ClassUnicode::new(ranges))).to_string();
    out.push_str("(?-i:");
    out.push_str(&class);
    out.push(')');
}

fn unicode_sets_class_needs_native_fallback<I>(chars: &std::iter::Peekable<I>) -> bool
where
    I: Iterator<Item = char> + Clone,
{
    let mut chars = chars.clone();
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if escaped {
            if matches!(ch, 'p' | 'P' | 'q') {
                return true;
            }
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '[' {
            return true;
        }
        if ch == ']' {
            return false;
        }
        if (ch == '&' && chars.peek() == Some(&'&')) || (ch == '-' && chars.peek() == Some(&'-')) {
            return true;
        }
    }
    false
}

fn materialize_active_word_class(
    backend_class: &str,
    unicode_mode: bool,
) -> Result<String, String> {
    const MAX_CACHE_ENTRIES: usize = 128;
    const MAX_CACHED_SOURCE_BYTES: usize = 512;
    const MAX_CACHED_RESULT_BYTES: usize = 4096;
    static CACHE: OnceLock<Mutex<std::collections::HashMap<(String, bool), String>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let cache_key = (backend_class.len() <= MAX_CACHED_SOURCE_BYTES)
        .then(|| (backend_class.to_string(), unicode_mode));
    if let Some(cached) = cache_key
        .as_ref()
        .and_then(|key| cache.lock().get(key).cloned())
    {
        return Ok(cached);
    }

    let outer_negated = backend_class.starts_with("[^");
    let positive_class = if outer_negated {
        format!("[{}", &backend_class[2..])
    } else {
        backend_class.to_string()
    };
    let hir = RegexSyntaxParserBuilder::new()
        .unicode(true)
        .utf8(true)
        .build()
        .parse(&positive_class)
        .map_err(|error| error.to_string())?;
    let mut class = match hir.into_kind() {
        HirKind::Class(Class::Unicode(class)) => class,
        HirKind::Literal(literal) => {
            let text = std::str::from_utf8(&literal.0).map_err(|error| error.to_string())?;
            let mut chars = text.chars();
            let Some(ch) = chars.next() else {
                return Err("ignore-case class normalized to an empty literal".to_string());
            };
            if chars.next().is_some() {
                return Err("ignore-case class normalized to multiple literals".to_string());
            }
            ClassUnicode::new([ClassUnicodeRange::new(ch, ch)])
        }
        _ => return Err("ignore-case class did not normalize to a Unicode class".to_string()),
    };

    // CharacterSetMatcher compares Canonicalize results. Materializing that
    // equivalence closure lets the backend run this class with `i` disabled.
    class = if unicode_mode {
        class.case_fold_simple();
        class
    } else {
        legacy_case_fold_class(&class)
    };
    if outer_negated {
        class.negate();
    }
    let materialized = Hir::class(Class::Unicode(class)).to_string();
    if let Some(cache_key) = cache_key.filter(|_| materialized.len() <= MAX_CACHED_RESULT_BYTES) {
        let mut cache = cache.lock();
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(cache_key, materialized.clone());
    }
    Ok(materialized)
}

fn escape_annex_b_hyphen_before_class_set(out: &mut String) {
    if !out.ends_with('-') {
        return;
    }
    let preceding_backslashes = out[..out.len() - 1]
        .chars()
        .rev()
        .take_while(|ch| *ch == '\\')
        .count();
    if preceding_backslashes % 2 == 0 {
        out.pop();
        out.push_str(r"\-");
    }
}

fn push_regex_capture_set_backreference_for_backend(
    out: &mut String,
    indices: &[usize],
    backref_set_id: Option<usize>,
) {
    debug_assert!(!indices.is_empty());
    if indices.len() > 1 {
        out.push_str("(?@");
        out.push_str(
            &backref_set_id
                .expect("duplicate named captures must have a registered backend set")
                .to_string(),
        );
        out.push(')');
        return;
    }
    for capture_index in indices {
        out.push_str("(?(");
        out.push_str(&capture_index.to_string());
        out.push_str(")\\");
        out.push_str(&capture_index.to_string());
        out.push('|');
    }
    for _ in indices {
        out.push(')');
    }
}

fn push_ecmascript_class_escape_for_backend(
    out: &mut String,
    escape: char,
    in_class: bool,
    unicode_mode: bool,
) {
    out.pop();
    if in_class {
        // Annex B treats a range with either multi-character endpoint as the
        // union of both endpoints and a literal hyphen in non-Unicode mode.
        if !unicode_mode {
            escape_annex_b_hyphen_before_class_set(out);
        }
        match escape {
            'd' => out.push_str("[:digit:]"),
            'D' => out.push_str("[:^digit:]"),
            's' | 'S' => {
                out.push('[');
                if escape == 'S' {
                    out.push('^');
                }
                out.push_str(ECMASCRIPT_WHITESPACE_CLASS_BODY);
                out.push(']');
            }
            _ => unreachable!(),
        }
        return;
    }

    out.push('[');
    if matches!(escape, 'D' | 'S') {
        out.push('^');
    }
    match escape {
        'd' | 'D' => out.push_str("0-9"),
        's' | 'S' => out.push_str(ECMASCRIPT_WHITESPACE_CLASS_BODY),
        _ => unreachable!(),
    }
    out.push(']');
}

struct NormalizedRegex {
    source: String,
    backref_sets: Vec<Vec<usize>>,
}

fn normalize_regex_for_backend(
    source: &str,
    flags: &str,
    capture_count: usize,
    code_unit_input: bool,
    fancy_backend: bool,
    capture_indices: &IndexMap<Arc<str>, Vec<usize>>,
) -> Result<NormalizedRegex, String> {
    let unicode_mode = flags.contains('u') || flags.contains('v');
    if source == "[]" {
        return Ok(NormalizedRegex {
            source: r"[^\s\S]".to_string(),
            backref_sets: Vec::new(),
        });
    }
    if source == "[^]" {
        let source = if unicode_mode {
            "(?s:.)".to_string()
        } else {
            r"[\x00-\u{ffff}\u{f0000}-\u{f07ff}]".to_string()
        };
        return Ok(NormalizedRegex {
            source,
            backref_sets: Vec::new(),
        });
    }
    let mut out = String::with_capacity(source.len());
    let mut backref_set_ids: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut backref_sets = Vec::new();
    let mut chars = source.chars().peekable();
    let mut in_class = false;
    let mut unicode_sets_class_depth = 0usize;
    let mut escaped = false;
    let mut class_output_start = None;
    let mut class_has_active_word_escape = false;
    let mut materialize_current_word_class = true;
    let mut capture_index = 0usize;
    let mut open_captures: Vec<Option<usize>> = Vec::new();
    let mut lookbehind_context = Vec::new();
    let mut modifier_stack = vec![RegexModifierState {
        dot_all: flags.contains('s'),
        ignore_case: flags.contains('i'),
    }];

    while let Some(ch) = chars.next() {
        if escaped {
            if ch == 'k' && !in_class && !capture_indices.is_empty() && chars.peek() == Some(&'<') {
                out.pop();
                chars.next();
                let mut raw_name = String::new();
                let mut terminated = false;
                for next in chars.by_ref() {
                    if next == '>' {
                        terminated = true;
                        break;
                    }
                    raw_name.push(next);
                }
                if !terminated {
                    return Err("unterminated regular expression group name".to_string());
                }
                let name = crate::lexer::decode_regex_group_name(&raw_name)?;
                let Some(indices) = capture_indices.get(name.as_str()) else {
                    return Err(format!("unknown regular expression group name '{name}'"));
                };
                let references_open_capture = open_captures
                    .iter()
                    .flatten()
                    .any(|capture| indices.contains(capture));
                if !references_open_capture {
                    if indices.len() == 1 && lookbehind_context.last().copied().unwrap_or(false) {
                        // Keep the established fixed-width lookbehind route.
                        // Backend conditionals can make an otherwise valid
                        // lookbehind appear variable length.
                        out.push('\\');
                        out.push_str(&indices[0].to_string());
                    } else {
                        let backref_set_id = if indices.len() > 1 {
                            if let Some(set_id) = backref_set_ids.get(&name) {
                                Some(*set_id)
                            } else {
                                let set_id = backref_sets.len();
                                backref_sets.push(indices.clone());
                                backref_set_ids.insert(name.clone(), set_id);
                                Some(set_id)
                            }
                        } else {
                            None
                        };
                        push_regex_capture_set_backreference_for_backend(
                            &mut out,
                            &indices,
                            backref_set_id,
                        );
                    }
                }
            } else if ch.is_ascii_digit() && ch != '0' {
                let mut digits = String::from(ch);
                while matches!(chars.peek(), Some(next) if next.is_ascii_digit()) {
                    digits.push(chars.next().unwrap());
                }
                let value = digits.parse::<usize>().unwrap_or(usize::MAX);
                if !in_class && value > 0 && value <= capture_count {
                    let references_open_capture = open_captures
                        .iter()
                        .flatten()
                        .any(|capture| *capture == value);
                    if references_open_capture {
                        out.pop();
                    } else if !lookbehind_context.last().copied().unwrap_or(false) {
                        out.pop();
                        out.push_str("(?(");
                        out.push_str(&digits);
                        out.push_str(")\\");
                        out.push_str(&digits);
                        out.push_str("|)");
                    } else {
                        out.push_str(&digits);
                    }
                } else if flags.contains('u') {
                    out.push_str(&digits);
                } else {
                    out.pop();
                    push_legacy_decimal_escape_for_backend(&mut out, &digits);
                }
            } else if matches!(ch, 'd' | 'D' | 's' | 'S') {
                push_ecmascript_class_escape_for_backend(&mut out, ch, in_class, unicode_mode);
            } else if flags.contains('u')
                && ch == 'P'
                && !in_class
                && modifier_stack.last().is_some_and(|state| state.ignore_case)
                && consume_uppercase_letter_property_name(&mut chars)
            {
                out.pop();
                out.push_str("(?s:.)");
            } else if flags.contains('u')
                && ch == 'P'
                && in_class
                && modifier_stack.last().is_some_and(|state| state.ignore_case)
                && consume_uppercase_letter_property_name(&mut chars)
            {
                out.pop();
                out.push_str(r"\s\S");
            } else if in_class
                && matches!(ch, 'w' | 'W')
                && modifier_stack.last().is_some_and(|state| state.ignore_case)
                && materialize_current_word_class
            {
                class_has_active_word_escape = true;
                push_ecmascript_word_escape_for_backend(&mut out, ch, true, unicode_mode);
            } else if !in_class
                && matches!(ch, 'w' | 'W')
                && modifier_stack.last().is_some_and(|state| state.ignore_case)
            {
                push_ecmascript_word_escape_for_backend(&mut out, ch, false, unicode_mode);
            } else if !in_class && matches!(ch, 'b' | 'B') && fancy_backend {
                let unicode_ignore_case =
                    unicode_mode && modifier_stack.last().is_some_and(|state| state.ignore_case);
                push_ecmascript_word_boundary_for_backend(&mut out, ch, unicode_ignore_case);
            } else if !in_class
                && matches!(ch, 'b' | 'B')
                && modifier_stack.last().is_some_and(|state| state.ignore_case)
                && !unicode_mode
            {
                out.pop();
                match ch {
                    'b' => out.push_str(r"(?-iu:\b)"),
                    'B' => out.push_str(r"(?-iu:\B)"),
                    _ => unreachable!(),
                }
            } else if in_class
                && matches!(ch, 'w' | 'W')
                && !modifier_stack.last().is_some_and(|state| state.ignore_case)
            {
                out.pop();
                match ch {
                    'w' => out.push_str("[:word:]"),
                    'W' => out.push_str("[:^word:]"),
                    _ => unreachable!(),
                }
            } else if !in_class
                && matches!(ch, 'w' | 'W' | 'b' | 'B')
                && !modifier_stack.last().is_some_and(|state| state.ignore_case)
            {
                out.pop();
                match ch {
                    'w' => out.push_str(r"(?-i:[A-Za-z0-9_])"),
                    'W' => out.push_str(r"(?-i:[^A-Za-z0-9_])"),
                    'b' => out.push_str(r"(?-iu:\b)"),
                    'B' => out.push_str(r"(?-iu:\B)"),
                    _ => unreachable!(),
                }
            } else if ch == 'u' && has_exact_hex_escape(&chars, 4) {
                let mut lead_hex = String::new();
                for _ in 0..4 {
                    lead_hex.push(chars.next().unwrap());
                }
                let lead = u32::from_str_radix(&lead_hex, 16).unwrap_or(0);
                let mut lookahead = chars.clone();
                let mut trail_hex = String::new();
                let has_trail_escape =
                    lookahead.next() == Some('\\') && lookahead.next() == Some('u');
                if has_trail_escape {
                    for _ in 0..4 {
                        match lookahead.next() {
                            Some(next) if next.is_ascii_hexdigit() => trail_hex.push(next),
                            _ => break,
                        }
                    }
                }
                if (0xd800..=0xdbff).contains(&lead) && trail_hex.len() == 4 {
                    let trail = u32::from_str_radix(&trail_hex, 16).unwrap_or(0);
                    if (0xdc00..=0xdfff).contains(&trail) {
                        chars = lookahead;
                        if code_unit_input && !unicode_mode {
                            push_surrogate_code_unit_escape_for_backend(&mut out, lead, in_class);
                            out.push('\\');
                            push_surrogate_code_unit_escape_for_backend(&mut out, trail, in_class);
                        } else {
                            let scalar = 0x10000 + ((lead - 0xd800) << 10) + (trail - 0xdc00);
                            out.pop();
                            out.push_str("\\u{");
                            out.push_str(&format!("{scalar:x}"));
                            out.push('}');
                        }
                    } else if unicode_mode {
                        push_surrogate_sentinel_escape_for_backend(&mut out, lead);
                    } else {
                        push_surrogate_code_unit_escape_for_backend(&mut out, lead, in_class);
                    }
                } else if (0xd800..=0xdfff).contains(&lead) {
                    if unicode_mode {
                        push_surrogate_sentinel_escape_for_backend(&mut out, lead);
                    } else {
                        push_surrogate_code_unit_escape_for_backend(&mut out, lead, in_class);
                    }
                } else if !in_class
                    && !unicode_mode
                    && modifier_stack.last().is_some_and(|state| state.ignore_case)
                    && (lead > 0x7f || (lead as u8 as char).is_ascii_alphabetic())
                {
                    out.pop();
                    push_legacy_case_fold_atom_for_backend(&mut out, lead as u16);
                } else {
                    out.push('u');
                    out.push_str(&lead_hex);
                }
            } else if ch == '0' && !chars.peek().is_some_and(|next| next.is_ascii_digit()) {
                out.push_str("x00");
            } else if in_class && ch == 'b' {
                out.pop();
                out.push_str(r"\x08");
            } else if ch == 'c' && chars.peek().is_some_and(|next| next.is_ascii_alphabetic()) {
                let control = chars.next().unwrap() as u8 % 32;
                out.pop();
                out.push_str("\\x");
                out.push_str(&format!("{control:02x}"));
            } else if (ch == 'x' && !has_exact_hex_escape(&chars, 2))
                || (ch == 'u' && !unicode_mode)
            {
                out.pop();
                push_regex_literal_for_backend(&mut out, ch);
            } else if !in_class
                && !unicode_mode
                && modifier_stack.last().is_some_and(|state| state.ignore_case)
                && ch == 'x'
            {
                let mut hex = String::new();
                for _ in 0..2 {
                    match chars.peek().copied() {
                        Some(next) if next.is_ascii_hexdigit() => {
                            hex.push(chars.next().unwrap());
                        }
                        _ => break,
                    }
                }
                if hex.len() == 2 {
                    let code = u32::from_str_radix(&hex, 16).unwrap_or(0);
                    if code > 0x7f || (code as u8 as char).is_ascii_alphabetic() {
                        out.pop();
                        push_legacy_case_fold_atom_for_backend(&mut out, code as u16);
                    } else {
                        out.push('x');
                        out.push_str(&hex);
                    }
                } else {
                    out.push(ch);
                    out.push_str(&hex);
                }
            } else if !flags.contains('u') && !regex_backend_escape_passthrough(ch, chars.peek()) {
                out.pop();
                if !in_class
                    && !unicode_mode
                    && modifier_stack.last().is_some_and(|state| state.ignore_case)
                    && (ch.is_ascii_alphabetic() || !ch.is_ascii())
                {
                    if let Ok(unit) = u16::try_from(ch as u32) {
                        push_legacy_case_fold_atom_for_backend(&mut out, unit);
                    } else {
                        push_regex_literal_for_backend(&mut out, ch);
                    }
                } else {
                    push_regex_literal_for_backend(&mut out, ch);
                }
            } else {
                out.push(ch);
            }
            escaped = false;
            continue;
        }
        if ch == '\\' {
            out.push(ch);
            escaped = true;
            continue;
        }
        if ch == '[' {
            if in_class && flags.contains('v') {
                unicode_sets_class_depth += 1;
                out.push(ch);
                continue;
            }
            if !in_class {
                class_output_start = Some(out.len());
                class_has_active_word_escape = false;
                materialize_current_word_class =
                    !flags.contains('v') || !unicode_sets_class_needs_native_fallback(&chars);
                unicode_sets_class_depth = 1;
            }
            in_class = true;
            out.push(ch);
            continue;
        }
        if ch == ']' && in_class {
            if flags.contains('v') && unicode_sets_class_depth > 1 {
                unicode_sets_class_depth -= 1;
                out.push(ch);
                continue;
            }
            in_class = false;
            unicode_sets_class_depth = 0;
            out.push(ch);
            let legacy_ignore_case =
                !unicode_mode && modifier_stack.last().is_some_and(|state| state.ignore_case);
            if class_has_active_word_escape || legacy_ignore_case {
                let start = class_output_start
                    .take()
                    .expect("active word class must have an output start");
                let backend_class = out[start..].to_string();
                let materialized = materialize_active_word_class(&backend_class, unicode_mode)?;
                out.truncate(start);
                out.push_str("(?-i:");
                out.push_str(&materialized);
                out.push(')');
            } else {
                class_output_start = None;
            }
            materialize_current_word_class = true;
            continue;
        }

        if !in_class && ch == '.' && !flags.contains('u') {
            if modifier_stack.last().is_some_and(|state| state.dot_all) {
                out.push_str(r"[\x00-\u{ffff}\u{f0000}-\u{f07ff}]");
            } else {
                out.push_str(
                    r"[\x00-\x09\x0b\x0c\x0e-\u{2027}\u{202a}-\u{ffff}\u{f0000}-\u{f07ff}]",
                );
            }
            continue;
        }

        if !in_class && ch == '(' && chars.peek() == Some(&'?') {
            let mut lookahead = chars.clone();
            lookahead.next();
            let starts_lookbehind =
                lookahead.next() == Some('<') && matches!(lookahead.next(), Some('=' | '!'));
            lookbehind_context
                .push(lookbehind_context.last().copied().unwrap_or(false) || starts_lookbehind);
            open_captures.push(None);
            out.push(ch);
            out.push(chars.next().unwrap());
            let mut add_modifiers = String::new();
            while matches!(chars.peek(), Some('i' | 'm' | 's')) {
                add_modifiers.push(chars.next().unwrap());
            }
            let mut remove_modifiers = String::new();
            if chars.peek() == Some(&'-') {
                chars.next();
                while matches!(chars.peek(), Some('i' | 'm' | 's')) {
                    remove_modifiers.push(chars.next().unwrap());
                }
                if chars.peek() == Some(&':') {
                    let mut state = *modifier_stack.last().unwrap();
                    if add_modifiers.contains('s') {
                        state.dot_all = true;
                    }
                    if add_modifiers.contains('i') {
                        state.ignore_case = true;
                    }
                    if remove_modifiers.contains('s') {
                        state.dot_all = false;
                    }
                    if remove_modifiers.contains('i') {
                        state.ignore_case = false;
                    }
                    modifier_stack.push(state);
                    out.push_str(&add_modifiers);
                    if !remove_modifiers.is_empty() {
                        out.push('-');
                        out.push_str(&remove_modifiers);
                    }
                    chars.next();
                    out.push(':');
                    continue;
                }
                out.push_str(&add_modifiers);
                out.push('-');
                out.push_str(&remove_modifiers);
                continue;
            }
            if !add_modifiers.is_empty() && chars.peek() == Some(&':') {
                let mut state = *modifier_stack.last().unwrap();
                if add_modifiers.contains('s') {
                    state.dot_all = true;
                }
                if add_modifiers.contains('i') {
                    state.ignore_case = true;
                }
                modifier_stack.push(state);
            } else {
                modifier_stack.push(*modifier_stack.last().unwrap());
            }
            out.push_str(&add_modifiers);
            continue;
        }

        if !in_class && ch == '(' {
            capture_index += 1;
            open_captures.push(Some(capture_index));
            lookbehind_context.push(lookbehind_context.last().copied().unwrap_or(false));
            modifier_stack.push(*modifier_stack.last().unwrap());
            out.push(ch);
            continue;
        }

        if !in_class && ch == ')' {
            open_captures.pop();
            lookbehind_context.pop();
            if modifier_stack.len() > 1 {
                modifier_stack.pop();
            }
            out.push(ch);
            continue;
        }

        // The Rust regex backends match Unicode scalars. Non-Unicode ES regexes
        // instead consume UTF-16 code units, so preserve each half explicitly.
        if code_unit_input
            && !unicode_mode
            && crate::value::utf16_single_unit_from_internal_char(ch).is_none()
        {
            let mut units = [0; 2];
            for unit in ch.encode_utf16(&mut units) {
                push_surrogate_sentinel_atom_for_backend(&mut out, *unit as u32);
            }
            continue;
        }

        if !in_class
            && !unicode_mode
            && modifier_stack.last().is_some_and(|state| state.ignore_case)
            && (ch.is_ascii_alphabetic() || !ch.is_ascii())
        {
            if let Ok(unit) = u16::try_from(ch as u32) {
                push_legacy_case_fold_atom_for_backend(&mut out, unit);
            } else {
                out.push_str("(?-i:");
                out.push(ch);
                out.push(')');
            }
        } else {
            out.push(ch);
        }
    }

    Ok(NormalizedRegex {
        source: out,
        backref_sets,
    })
}

fn regex_capture_count(source: &str) -> usize {
    let mut count = 0;
    let mut chars = source.chars().peekable();
    let mut in_class = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => {
                if chars.peek() != Some(&'?') {
                    count += 1;
                    continue;
                }
                let mut lookahead = chars.clone();
                lookahead.next();
                if lookahead.next() == Some('<') && !matches!(lookahead.peek(), Some('=' | '!')) {
                    count += 1;
                }
            }
            _ => {}
        }
    }
    count
}

fn regex_group_name_end(chars: &[char], start: usize) -> Result<usize, String> {
    let mut end = start;
    while chars.get(end).is_some_and(|ch| *ch != '>') {
        end += 1;
    }
    if chars.get(end) == Some(&'>') {
        Ok(end)
    } else {
        Err("unterminated regular expression group name".to_string())
    }
}

fn regex_capture_names(source: &str, flags: &str) -> Result<Vec<RegexCaptureName>, String> {
    crate::lexer::scan_regex_named_captures(source, flags).map(|captures| {
        captures
            .into_iter()
            .map(|(name, index)| RegexCaptureName {
                name: Arc::from(name.as_str()),
                index,
            })
            .collect()
    })
}

fn regex_capture_indices_by_name(captures: &[RegexCaptureName]) -> IndexMap<Arc<str>, Vec<usize>> {
    let mut indices = IndexMap::new();
    for capture in captures {
        indices
            .entry(capture.name.clone())
            .or_insert_with(Vec::new)
            .push(capture.index);
    }
    indices
}

fn rewrite_named_regex_groups_for_backend(
    source: &str,
    flags: &str,
    capture_indices: &IndexMap<Arc<str>, Vec<usize>>,
) -> Result<String, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0usize;
    let mut in_class = false;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '\\' {
            if !in_class && chars.get(index + 1) == Some(&'k') && chars.get(index + 2) == Some(&'<')
            {
                if capture_indices.is_empty() && !flags.contains('u') && !flags.contains('v') {
                    out.push('\\');
                    out.push('k');
                    index += 2;
                    continue;
                }
                let end = regex_group_name_end(&chars, index + 3)?;
                let raw_name: String = chars[index + 3..end].iter().collect();
                let name = crate::lexer::decode_regex_group_name(&raw_name)?;
                if !capture_indices.contains_key(name.as_str()) {
                    return Err(format!("unknown regular expression group name '{name}'"));
                }
                // Keep the source-level reference intact until normalization,
                // where all same-name capture indices can be lowered into one
                // backend conditional without losing capture-stack context.
                out.extend(chars[index..=end].iter().copied());
                index = end + 1;
                continue;
            }
            if capture_indices.is_empty() || chars.get(index + 1) != Some(&'k') {
                out.push(ch);
                if let Some(next) = chars.get(index + 1) {
                    out.push(*next);
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }
            return Err("invalid regular expression named backreference".to_string());
        }

        if ch == '[' && !in_class {
            in_class = true;
            out.push(ch);
            index += 1;
            continue;
        }
        if ch == ']' && in_class {
            in_class = false;
            out.push(ch);
            index += 1;
            continue;
        }

        if !in_class
            && ch == '('
            && chars.get(index + 1) == Some(&'?')
            && chars.get(index + 2) == Some(&'<')
            && !matches!(chars.get(index + 3), Some('=' | '!'))
        {
            let end = regex_group_name_end(&chars, index + 3)?;
            let raw_name: String = chars[index + 3..end].iter().collect();
            crate::lexer::decode_regex_group_name(&raw_name)?;
            out.push('(');
            index = end + 1;
            continue;
        }

        out.push(ch);
        index += 1;
    }

    Ok(out)
}

fn named_capture_indices(names: &[RegexCaptureName], name: &str) -> Vec<usize> {
    names
        .iter()
        .filter(|capture| capture.name.as_ref() == name)
        .map(|capture| capture.index)
        .collect()
}

fn make_regexp_groups_object(
    vm: &mut Vm,
    caps: &CompiledCaptures<'_>,
    names: &[RegexCaptureName],
) -> error::Result<Value> {
    if names.is_empty() {
        return Ok(Value::Undefined);
    }
    let obj_idx = vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(None),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Object")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.heap.with_obj(obj_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let mut matched_names = IndexSet::new();
        for capture in names {
            let value = caps
                .get(capture.index)
                .map(|m| Value::String(canonicalize_regexp_match_text(m.as_str())))
                .unwrap_or(Value::Undefined);
            if matched_names.contains(&capture.name) {
                debug_assert!(value.is_undefined());
                continue;
            }
            if !value.is_undefined() {
                matched_names.insert(capture.name.clone());
            }
            props.insert(
                PropertyKey::from(capture.name.clone()),
                PropertyDescriptor::data(value),
            );
        }
    });
    Ok(Value::Object(obj_idx))
}

fn canonicalize_regexp_match_text(text: &str) -> Arc<str> {
    let text = crate::value::utf16_to_string(&crate::value::utf16_from_str(text));
    Arc::from(text.as_str())
}

fn regex_contains_quantified_capture_group(source: &str) -> bool {
    let chars: Vec<char> = source.chars().collect();
    let mut capture_count_at_group_start = Vec::new();
    let mut capture_count = 0;
    let mut in_class = false;
    let mut escaped = false;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => {
                let captures_before_group = capture_count;
                if regex_group_is_capturing_chars(&chars, i) {
                    capture_count += 1;
                }
                capture_count_at_group_start.push(captures_before_group);
            }
            ')' if !in_class => {
                if capture_count_at_group_start.pop().is_some_and(|before| {
                    capture_count > before && regex_quantifier_starts_at_chars(&chars, i + 1)
                }) {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

fn regex_group_is_capturing_chars(chars: &[char], idx: usize) -> bool {
    if chars.get(idx) != Some(&'(') {
        return false;
    }
    if chars.get(idx + 1) != Some(&'?') {
        return true;
    }
    chars.get(idx + 2) == Some(&'<') && !matches!(chars.get(idx + 3), Some('=' | '!'))
}

fn regex_quantifier_starts_at_chars(chars: &[char], idx: usize) -> bool {
    match chars.get(idx) {
        Some('*' | '+' | '?') => true,
        Some('{') => {
            let mut i = idx + 1;
            let mut saw_digit = false;
            while matches!(chars.get(i), Some(ch) if ch.is_ascii_digit()) {
                saw_digit = true;
                i += 1;
            }
            if !saw_digit {
                return false;
            }
            if chars.get(i) == Some(&',') {
                i += 1;
                while matches!(chars.get(i), Some(ch) if ch.is_ascii_digit()) {
                    i += 1;
                }
            }
            chars.get(i) == Some(&'}')
        }
        _ => false,
    }
}

fn regex_uses_backreference(source: &str, capture_count: usize, has_named_captures: bool) -> bool {
    if capture_count == 0 {
        return false;
    }
    let mut chars = source.chars().peekable();
    let mut in_class = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if escaped {
            if !in_class && has_named_captures && ch == 'k' && chars.peek() == Some(&'<') {
                return true;
            }
            if !in_class && ch.is_ascii_digit() && ch != '0' {
                let mut digits = String::from(ch);
                while matches!(chars.peek(), Some(next) if next.is_ascii_digit()) {
                    digits.push(chars.next().unwrap());
                }
                if digits
                    .parse::<usize>()
                    .is_ok_and(|value| value <= capture_count)
                {
                    return true;
                }
            }
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            _ => {}
        }
    }
    false
}

fn regex_uses_lookaround(source: &str) -> bool {
    let chars: Vec<char> = source.chars().collect();
    let mut in_class = false;
    let mut escaped = false;
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if escaped {
            escaped = false;
            index += 1;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class && chars.get(index + 1) == Some(&'?') => {
                if matches!(chars.get(index + 2), Some('=' | '!'))
                    || (chars.get(index + 2) == Some(&'<')
                        && matches!(chars.get(index + 3), Some('=' | '!')))
                {
                    return true;
                }
            }
            _ => {}
        }
        index += 1;
    }
    false
}

fn push_legacy_decimal_escape_for_backend(out: &mut String, digits: &str) {
    let mut chars = digits.chars();
    let Some(first) = chars.next() else {
        return;
    };
    if !matches!(first, '1'..='7') {
        out.push(first);
        out.extend(chars);
        return;
    }

    let mut value = first.to_digit(8).unwrap();
    let mut used = first.len_utf8();
    for ch in digits[first.len_utf8()..].chars() {
        let Some(digit) = ch.to_digit(8) else {
            break;
        };
        let next = value * 8 + digit;
        if next > 0xff {
            break;
        }
        value = next;
        used += ch.len_utf8();
    }
    out.push_str("\\x");
    out.push_str(&format!("{value:02x}"));
    out.push_str(&digits[used..]);
}

fn regex_backend_escape_passthrough(ch: char, next: Option<&char>) -> bool {
    matches!(
        ch,
        '0' | 'b'
            | 'B'
            | 'd'
            | 'D'
            | 'f'
            | 'n'
            | 'r'
            | 's'
            | 'S'
            | 't'
            | 'u'
            | 'v'
            | 'w'
            | 'W'
            | 'x'
    ) || (ch == 'c' && next.is_some_and(|next| next.is_ascii_alphabetic()))
}

fn has_exact_hex_escape<I>(chars: &std::iter::Peekable<I>, len: usize) -> bool
where
    I: Iterator<Item = char> + Clone,
{
    let mut lookahead = chars.clone();
    (0..len).all(|_| lookahead.next().is_some_and(|ch| ch.is_ascii_hexdigit()))
}

fn push_regex_literal_for_backend(out: &mut String, ch: char) {
    let literal = ch.to_string();
    out.push_str(&regex::escape(&literal));
}

fn push_surrogate_sentinel_escape_for_backend(out: &mut String, surrogate: u32) {
    debug_assert!((0xd800..=0xdfff).contains(&surrogate));
    let sentinel = 0xf0000 + (surrogate - 0xd800);
    out.pop();
    out.push_str("\\u{");
    out.push_str(&format!("{sentinel:x}"));
    out.push('}');
}

fn push_surrogate_code_unit_escape_for_backend(out: &mut String, surrogate: u32, in_class: bool) {
    debug_assert!((0xd800..=0xdfff).contains(&surrogate));
    out.pop();
    if !in_class {
        out.push('[');
    }
    push_surrogate_sentinel_atom_for_backend(out, surrogate);
    if (0xd800..=0xdbff).contains(&surrogate) {
        let high_offset = surrogate - 0xd800;
        let start = 0x10000 + (high_offset << 10);
        let end = start + 0x3ff;
        push_unicode_code_point_atom_for_backend(out, start);
        out.push('-');
        push_unicode_code_point_atom_for_backend(out, end);
    } else {
        let low_offset = surrogate - 0xdc00;
        for high_offset in 0..=0x3ff {
            let scalar = 0x10000 + (high_offset << 10) + low_offset;
            push_unicode_code_point_atom_for_backend(out, scalar);
        }
    }
    if !in_class {
        out.push(']');
    }
}

fn push_surrogate_sentinel_atom_for_backend(out: &mut String, surrogate: u32) {
    let sentinel = 0xf0000 + (surrogate - 0xd800);
    push_unicode_code_point_atom_for_backend(out, sentinel);
}

fn push_unicode_code_point_atom_for_backend(out: &mut String, code_point: u32) {
    out.push_str("\\u{");
    out.push_str(&format!("{code_point:x}"));
    out.push('}');
}

fn consume_uppercase_letter_property_name<I>(chars: &mut std::iter::Peekable<I>) -> bool
where
    I: Iterator<Item = char> + Clone,
{
    let mut lookahead = chars.clone();
    if lookahead.next() != Some('{') {
        return false;
    }
    let mut name = String::new();
    while let Some(ch) = lookahead.next() {
        if ch == '}' {
            if matches!(
                name.as_str(),
                "Lu" | "Uppercase_Letter"
                    | "General_Category=Lu"
                    | "General_Category=Uppercase_Letter"
                    | "gc=Lu"
                    | "gc=Uppercase_Letter"
            ) {
                *chars = lookahead;
                return true;
            }
            return false;
        }
        name.push(ch);
    }
    false
}
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use std::sync::{Arc, OnceLock};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn data_prop(value: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value,
        writable: true,
        enumerable: false,
        configurable: true,
        get: None,
        set: None,
        is_accessor: false,
    }
}

pub(crate) fn builtin_function_own_props(
    name: &str,
    length: usize,
) -> IndexMap<PropertyKey, PropertyDescriptor> {
    let mut props = IndexMap::new();
    let mut length_desc = PropertyDescriptor::data(Value::Number(length as f64));
    length_desc.writable = false;
    length_desc.enumerable = false;
    length_desc.configurable = true;
    props.insert(PropertyKey::from("length"), length_desc);

    let mut name_desc = PropertyDescriptor::data(Value::String(Arc::from(name)));
    name_desc.writable = false;
    name_desc.enumerable = false;
    name_desc.configurable = true;
    props.insert(PropertyKey::from("name"), name_desc);
    props
}

/// Create a non-writable, non-enumerable, non-configurable data property
/// descriptor (for built-in constants like Number.MAX_VALUE).
pub(crate) fn const_prop(value: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value,
        writable: false,
        enumerable: false,
        configurable: false,
        get: None,
        set: None,
        is_accessor: false,
    }
}

fn accessor_get_prop(get: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value: Value::Undefined,
        writable: false,
        enumerable: false,
        configurable: true,
        get: Some(get),
        set: None,
        is_accessor: true,
    }
}

fn accessor_prop(get: Value, set: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value: Value::Undefined,
        writable: false,
        enumerable: false,
        configurable: true,
        get: Some(get),
        set: Some(set),
        is_accessor: true,
    }
}

fn set_function_object_proto(vm: &mut Vm, function_idx: GcIdx, proto: &Value) {
    vm.heap.with_obj(function_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            *f.proto.lock() = Some(proto.clone());
        }
    });
}

pub(crate) fn throw_type_error_intrinsic(vm: &mut Vm, realm: GcIdx) -> error::Result<Value> {
    let realm = env::global_env_root(&vm.heap, realm);
    if let Some(value) = vm.realm_throw_type_errors.get(&realm.0) {
        return Ok(value.clone());
    }

    let thrower_idx = vm.new_native_function_in_env("", function_throw_type_error, 0, realm)?;
    vm.heap.with_obj(thrower_idx.0, |obj| {
        if let HeapObj::Function(function) = obj {
            let mut props = function.props.lock();
            let mut length_desc = PropertyDescriptor::data(Value::Number(0.0));
            length_desc.writable = false;
            length_desc.enumerable = false;
            length_desc.configurable = false;
            props.insert(PropertyKey::from("length"), length_desc);

            let mut name_desc = PropertyDescriptor::data(Value::String(Arc::from("")));
            name_desc.writable = false;
            name_desc.enumerable = false;
            name_desc.configurable = false;
            props.insert(PropertyKey::from("name"), name_desc);

            function.extensible.store(false, Ordering::Relaxed);
        }
    });
    let thrower = Value::Object(thrower_idx);
    vm.realm_throw_type_errors.insert(realm.0, thrower.clone());
    Ok(thrower)
}

pub(crate) fn restricted_throw_type_error_accessor(thrower: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value: Value::Undefined,
        writable: false,
        enumerable: false,
        configurable: false,
        get: Some(thrower.clone()),
        set: Some(thrower),
        is_accessor: true,
    }
}

pub(crate) fn install_symbol_static_properties(
    vm: &Vm,
    props: &mut IndexMap<PropertyKey, PropertyDescriptor>,
) {
    let symbols = &vm.well_known_symbols;
    for (name, id) in [
        ("asyncDispose", symbols.async_dispose),
        ("asyncIterator", symbols.async_iterator),
        ("dispose", symbols.dispose),
        ("hasInstance", symbols.has_instance),
        ("isConcatSpreadable", symbols.is_concat_spreadable),
        ("iterator", symbols.iterator),
        ("match", symbols.r#match),
        ("matchAll", symbols.match_all),
        ("replace", symbols.replace),
        ("search", symbols.search),
        ("species", symbols.species),
        ("split", symbols.split),
        ("toPrimitive", symbols.to_primitive),
        ("toStringTag", symbols.to_string_tag),
        ("unscopables", symbols.unscopables),
    ] {
        props.insert(PropertyKey::from(name), const_prop(Value::Symbol(id)));
    }
}

pub(crate) fn native_constructor_prototype(vm: &mut Vm, fallback: Value) -> error::Result<Value> {
    native_constructor_prototype_with_default(vm, "Object", fallback)
}

pub(crate) fn native_constructor_prototype_with_default(
    vm: &mut Vm,
    intrinsic: &str,
    fallback: Value,
) -> error::Result<Value> {
    if let Some(proto) = vm.current_native_new_target_prototype().cloned() {
        if matches!(proto, Value::Object(_)) {
            return Ok(proto);
        }
    }
    if let Some(realm) = vm.current_native_new_target_fallback_realm() {
        return vm.realm_default_prototype(realm, intrinsic, fallback);
    }
    if let Some(new_target) = vm.current_native_new_target().cloned() {
        let proto = vm.get_property_by_key(&new_target, &PropertyKey::from("prototype"))?;
        if matches!(proto, Value::Object(_)) {
            return Ok(proto);
        }
    }
    if let Some(new_target) = vm.current_native_new_target().cloned() {
        return vm.constructor_realm_default_prototype(&new_target, intrinsic, fallback);
    }
    Ok(fallback)
}

pub(crate) fn install_methods(vm: &mut Vm, proto: &Value, methods: &[(Arc<str>, Value)]) {
    if let Value::Object(idx) = proto {
        vm.heap.with_obj(idx.0, |obj| {
            let props = obj.props();
            for (name, func) in methods {
                props
                    .lock()
                    .insert(PropertyKey::from(name.clone()), data_prop(func.clone()));
            }
        });
    }
}

pub(crate) fn is_array(value: &Value, heap: &Heap) -> bool {
    match value {
        Value::Object(idx) => heap.with_obj(idx.0, |obj| match obj {
            HeapObj::Array(a) => !a.is_arguments.load(Ordering::Relaxed),
            // Tagged-template objects are ordinary objects with class_name "Array"
            // and Array.prototype, so they are recognized as arrays.
            HeapObj::Object(o) => o.class_name.as_deref() == Some("Array"),
            _ => false,
        }),
        _ => false,
    }
}

pub(crate) fn is_array_or_throw(vm: &mut Vm, value: &Value) -> error::Result<bool> {
    enum Step {
        Done(bool),
        Proxy(Value),
        Revoked,
    }

    let mut current = value.clone();
    loop {
        let Value::Object(idx) = &current else {
            return Ok(false);
        };
        match vm.heap.with_obj(idx.0, |obj| match obj {
            HeapObj::Array(array) => Step::Done(!array.is_arguments.load(Ordering::Relaxed)),
            HeapObj::Object(object) if object.class_name.as_deref() == Some("Array") => {
                Step::Done(true)
            }
            HeapObj::Proxy(proxy) if *proxy.revoked.lock() => Step::Revoked,
            HeapObj::Proxy(proxy) => Step::Proxy(proxy.target.clone()),
            _ => Step::Done(false),
        }) {
            Step::Done(is_array) => return Ok(is_array),
            Step::Proxy(target) => {
                vm.consume_fuel()?;
                current = target;
            }
            Step::Revoked => {
                return Err(Error::type_err(
                    "Cannot determine whether a revoked Proxy is an array",
                ));
            }
        }
    }
}

pub(crate) fn is_callable(value: &Value, heap: &Heap) -> bool {
    let Value::Object(idx) = value else {
        return false;
    };
    heap.with_obj(idx.0, |obj| match obj {
        HeapObj::Function(_) => true,
        HeapObj::Proxy(proxy) => proxy.callable,
        _ => false,
    })
}

pub(crate) fn object_to_string(
    vm: &mut Vm,
    this: Option<Value>,
    class_hint: Option<&str>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_null() {
        return Ok(Value::String(Arc::from("[object Null]")));
    }
    if this.is_undefined() {
        return Ok(Value::String(Arc::from("[object Undefined]")));
    }
    let object = vm.to_object(&this)?;
    let builtin_tag = if is_array_or_throw(vm, &object)? {
        "Array".to_string()
    } else if is_callable(&object, &vm.heap) {
        "Function".to_string()
    } else {
        match &object {
            Value::Object(index) => class_hint.map(str::to_string).unwrap_or_else(|| {
                vm.heap.with_obj(index.0, |object| match object {
                    HeapObj::Array(array) if array.is_arguments.load(Ordering::Relaxed) => {
                        "Arguments".to_string()
                    }
                    HeapObj::Object(data) => {
                        if let Some(primitive) = data.primitive.lock().as_ref() {
                            return match primitive {
                                Value::String(_) => "String".to_string(),
                                Value::Number(_) => "Number".to_string(),
                                Value::Bool(_) => "Boolean".to_string(),
                                _ => "Object".to_string(),
                            };
                        }
                        match data.class_name.as_deref() {
                            Some("Date") => "Date".to_string(),
                            Some("RegExp") => "RegExp".to_string(),
                            Some(
                                "Error" | "EvalError" | "RangeError" | "ReferenceError"
                                | "SyntaxError" | "TypeError" | "URIError" | "AggregateError",
                            ) => "Error".to_string(),
                            _ => "Object".to_string(),
                        }
                    }
                    _ => "Object".to_string(),
                })
            }),
            _ => "Object".to_string(),
        }
    };

    let pin_count = vm.pin(&object);
    let tag = vm.get_property_by_key(
        &object,
        &PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
    );
    vm.unpin_many(pin_count);
    let tag = tag?;
    let tag = match tag {
        Value::String(tag) => tag.to_string(),
        _ => builtin_tag,
    };
    Ok(Value::String(Arc::from(format!("[object {tag}]").as_str())))
}

// ---------------------------------------------------------------------------
// Built-in builders
// ---------------------------------------------------------------------------

pub(crate) fn make_builtin_constructor(
    vm: &mut Vm,
    name: &str,
    methods: &[(&str, NativeFn, usize)],
) -> error::Result<(GcIdx, GcIdx)> {
    let proto_value = vm.object_proto.clone();

    let mut method_props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    for (n, f, len) in methods {
        let func_idx = vm.new_native_function(n, *f, *len)?;
        method_props.insert(PropertyKey::from(*n), data_prop(Value::Object(func_idx)));
    }

    let proto_obj = HeapObj::Object(ObjectData {
        props: Mutex::new(method_props),
        proto: Mutex::new(Some(proto_value.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from(name)),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj)?);

    let ctor_func = FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Native {
            func: object_constructor,
            length: 1,
            construct_mode: Some(NativeConstructMode::InternalDeferredPrototype),
        },
        closure: vm.global,
        lexical_new_target: Value::Undefined,
        home_object: Mutex::new(None),
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(Some(Value::Object(proto_idx))),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(builtin_function_own_props(name, 1)),
        extensible: AtomicBool::new(true),
        private_fields: Mutex::new(std::collections::HashMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func))?);
    // constructor.prototype
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(Value::Object(proto_idx)),
        );
    });
    // prototype.constructor
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
    });

    Ok((ctor_idx, proto_idx))
}

fn install_typed_array_constructor(
    vm: &mut Vm,
    name: &str,
    constructor: NativeFn,
    kind: crate::value::TypedArrayKind,
    typed_array_ctor: &Value,
    typed_array_proto: &Value,
) -> error::Result<()> {
    install_typed_array_constructor_in_env(
        vm,
        vm.global,
        None,
        (name, constructor, kind),
        typed_array_ctor,
        typed_array_proto,
    )
}

fn install_typed_array_constructor_in_env(
    vm: &mut Vm,
    env: GcIdx,
    global: Option<&Value>,
    entry: (&str, NativeFn, crate::value::TypedArrayKind),
    typed_array_ctor: &Value,
    typed_array_proto: &Value,
) -> error::Result<()> {
    let (name, constructor, kind) = entry;
    let ctor_idx = vm.new_native_constructor_in_env(
        name,
        constructor,
        3,
        env,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    let proto_idx = GcIdx(vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(typed_array_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from(name)),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?);
    vm.heap.with_obj(ctor_idx.0, |o| {
        if let HeapObj::Function(f) = o {
            *f.proto.lock() = Some(typed_array_ctor.clone());
            *f.prototype.lock() = Some(Value::Object(proto_idx));
            f.props.lock().insert(
                PropertyKey::from("prototype"),
                const_prop(Value::Object(proto_idx)),
            );
        }
    });
    let element_size = Value::Number(kind.element_size() as f64);
    vm.heap.with_obj(proto_idx.0, |o| {
        let mut props = o.props().lock();
        props.insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
        props.insert(
            PropertyKey::from("BYTES_PER_ELEMENT"),
            const_prop(element_size.clone()),
        );
    });
    vm.heap.with_obj(ctor_idx.0, |o| {
        if let HeapObj::Function(f) = o {
            f.props.lock().insert(
                PropertyKey::from("BYTES_PER_ELEMENT"),
                const_prop(element_size),
            );
        }
    });
    let ctor = Value::Object(ctor_idx);
    let realm = crate::environment::global_env_root(&vm.heap, env);
    vm.realm_typed_array_constructors
        .insert((realm.0, kind), ctor.clone());
    if let Some(global) = global {
        define_realm_global(vm, env, global, name, ctor);
    } else {
        define_global(vm, name, ctor);
    }
    Ok(())
}

fn typed_array_constructor_entries() -> [(&'static str, NativeFn, crate::value::TypedArrayKind); 11]
{
    [
        (
            "Int8Array",
            int8array_constructor as NativeFn,
            crate::value::TypedArrayKind::Int8,
        ),
        (
            "Uint8Array",
            uint8array_constructor as NativeFn,
            crate::value::TypedArrayKind::Uint8,
        ),
        (
            "Uint8ClampedArray",
            uint8clampedarray_constructor as NativeFn,
            crate::value::TypedArrayKind::Uint8Clamped,
        ),
        (
            "Int16Array",
            int16array_constructor as NativeFn,
            crate::value::TypedArrayKind::Int16,
        ),
        (
            "Uint16Array",
            uint16array_constructor as NativeFn,
            crate::value::TypedArrayKind::Uint16,
        ),
        (
            "Int32Array",
            int32array_constructor as NativeFn,
            crate::value::TypedArrayKind::Int32,
        ),
        (
            "Uint32Array",
            uint32array_constructor as NativeFn,
            crate::value::TypedArrayKind::Uint32,
        ),
        (
            "Float32Array",
            float32array_constructor as NativeFn,
            crate::value::TypedArrayKind::Float32,
        ),
        (
            "Float64Array",
            float64array_constructor as NativeFn,
            crate::value::TypedArrayKind::Float64,
        ),
        (
            "BigInt64Array",
            bigint64array_constructor as NativeFn,
            crate::value::TypedArrayKind::BigInt64,
        ),
        (
            "BigUint64Array",
            biguint64array_constructor as NativeFn,
            crate::value::TypedArrayKind::BigUint64,
        ),
    ]
}

fn make_typed_array_intrinsic_in_env(vm: &mut Vm, env: GcIdx) -> error::Result<(Value, Value)> {
    let realm = crate::environment::global_env_root(&vm.heap, env);
    let object_proto = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.object_proto.clone());
    let typed_array_ctor = Value::Object(vm.new_native_constructor_in_env(
        "TypedArray",
        typed_array_intrinsic_constructor,
        0,
        env,
        NativeConstructMode::InternalDeferredPrototype,
    )?);
    let typed_array_proto =
        Value::Object(GcIdx(vm.heap.allocate(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(object_proto)),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("TypedArray")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?));
    if let Value::Object(idx) = &typed_array_ctor {
        let typed_array_from_fn =
            vm.new_native_function_in_env("from", typed_array_from, 1, env)?;
        let typed_array_of_fn = vm.new_native_function_in_env("of", typed_array_of, 0, env)?;
        let typed_array_species_getter = vm.new_native_function_in_env(
            "get [Symbol.species]",
            array_buffer_species_get,
            0,
            env,
        )?;
        let species_symbol = vm.well_known_symbols.species;
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Function(f) = o {
                *f.prototype.lock() = Some(typed_array_proto.clone());
                f.props.lock().insert(
                    PropertyKey::from("prototype"),
                    const_prop(typed_array_proto.clone()),
                );
                f.props.lock().insert(
                    PropertyKey::from("from"),
                    data_prop(Value::Object(typed_array_from_fn)),
                );
                f.props.lock().insert(
                    PropertyKey::from("of"),
                    data_prop(Value::Object(typed_array_of_fn)),
                );
                f.props.lock().insert(
                    PropertyKey::Symbol(species_symbol),
                    accessor_get_prop(Value::Object(typed_array_species_getter)),
                );
            }
        });
    }
    let typed_array_buffer_getter =
        vm.new_native_function_in_env("get buffer", typed_array_buffer_get, 0, env)?;
    let typed_array_byte_length_getter =
        vm.new_native_function_in_env("get byteLength", typed_array_byte_length_get, 0, env)?;
    let typed_array_byte_offset_getter =
        vm.new_native_function_in_env("get byteOffset", typed_array_byte_offset_get, 0, env)?;
    let typed_array_length_getter =
        vm.new_native_function_in_env("get length", typed_array_length_get, 0, env)?;
    let typed_array_to_string_tag_getter = vm.new_native_function_in_env(
        "get [Symbol.toStringTag]",
        typed_array_to_string_tag_get,
        0,
        env,
    )?;
    let typed_array_subarray_fn =
        vm.new_native_function_in_env("subarray", typed_array_subarray, 2, env)?;
    let typed_array_set_fn = vm.new_native_function_in_env("set", typed_array_set, 1, env)?;
    let typed_array_copy_within_fn =
        vm.new_native_function_in_env("copyWithin", typed_array_copy_within, 2, env)?;
    let typed_array_slice_fn = vm.new_native_function_in_env("slice", typed_array_slice, 2, env)?;
    let typed_array_find_fn = vm.new_native_function_in_env("find", typed_array_find, 1, env)?;
    let typed_array_find_index_fn =
        vm.new_native_function_in_env("findIndex", typed_array_find_index, 1, env)?;
    let typed_array_find_last_fn =
        vm.new_native_function_in_env("findLast", typed_array_find_last, 1, env)?;
    let typed_array_find_last_index_fn =
        vm.new_native_function_in_env("findLastIndex", typed_array_find_last_index, 1, env)?;
    let typed_array_some_fn = vm.new_native_function_in_env("some", typed_array_some, 1, env)?;
    let typed_array_every_fn = vm.new_native_function_in_env("every", typed_array_every, 1, env)?;
    let typed_array_for_each_fn =
        vm.new_native_function_in_env("forEach", typed_array_for_each, 1, env)?;
    let typed_array_includes_fn =
        vm.new_native_function_in_env("includes", typed_array_includes, 1, env)?;
    let typed_array_index_of_fn =
        vm.new_native_function_in_env("indexOf", typed_array_index_of, 1, env)?;
    let typed_array_last_index_of_fn =
        vm.new_native_function_in_env("lastIndexOf", typed_array_last_index_of, 1, env)?;
    let typed_array_reduce_right_fn =
        vm.new_native_function_in_env("reduceRight", typed_array_reduce_right, 1, env)?;
    let typed_array_reduce_fn =
        vm.new_native_function_in_env("reduce", typed_array_reduce, 1, env)?;
    let typed_array_map_fn = vm.new_native_function_in_env("map", typed_array_map, 1, env)?;
    let typed_array_filter_fn =
        vm.new_native_function_in_env("filter", typed_array_filter, 1, env)?;
    let typed_array_sort_fn = vm.new_native_function_in_env("sort", typed_array_sort, 1, env)?;
    let typed_array_to_sorted_fn =
        vm.new_native_function_in_env("toSorted", typed_array_to_sorted, 1, env)?;
    let typed_array_with_fn = vm.new_native_function_in_env("with", typed_array_with, 2, env)?;
    let typed_array_join_fn = vm.new_native_function_in_env("join", typed_array_join, 1, env)?;
    let typed_array_to_locale_string_fn =
        vm.new_native_function_in_env("toLocaleString", typed_array_to_locale_string, 0, env)?;
    let typed_array_reverse_fn =
        vm.new_native_function_in_env("reverse", typed_array_reverse, 0, env)?;
    let typed_array_to_reversed_fn =
        vm.new_native_function_in_env("toReversed", typed_array_to_reversed, 0, env)?;
    let typed_array_fill_fn = vm.new_native_function_in_env("fill", typed_array_fill, 1, env)?;
    let typed_array_at_fn = vm.new_native_function_in_env("at", typed_array_at, 1, env)?;
    let typed_array_values_fn =
        vm.new_native_function_in_env("values", typed_array_values, 0, env)?;
    let typed_array_keys_fn = vm.new_native_function_in_env("keys", typed_array_keys, 0, env)?;
    let typed_array_entries_fn =
        vm.new_native_function_in_env("entries", typed_array_entries, 0, env)?;
    if let Value::Object(idx) = &typed_array_proto {
        vm.heap.with_obj(idx.0, |obj| {
            let mut props = obj.props().lock();
            props.insert(
                PropertyKey::from("constructor"),
                data_prop(typed_array_ctor.clone()),
            );
            props.insert(
                PropertyKey::from("buffer"),
                accessor_get_prop(Value::Object(typed_array_buffer_getter)),
            );
            props.insert(
                PropertyKey::from("byteLength"),
                accessor_get_prop(Value::Object(typed_array_byte_length_getter)),
            );
            props.insert(
                PropertyKey::from("byteOffset"),
                accessor_get_prop(Value::Object(typed_array_byte_offset_getter)),
            );
            props.insert(
                PropertyKey::from("length"),
                accessor_get_prop(Value::Object(typed_array_length_getter)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
                accessor_get_prop(Value::Object(typed_array_to_string_tag_getter)),
            );
            props.insert(
                PropertyKey::from("subarray"),
                data_prop(Value::Object(typed_array_subarray_fn)),
            );
            props.insert(
                PropertyKey::from("set"),
                data_prop(Value::Object(typed_array_set_fn)),
            );
            props.insert(
                PropertyKey::from("copyWithin"),
                data_prop(Value::Object(typed_array_copy_within_fn)),
            );
            props.insert(
                PropertyKey::from("slice"),
                data_prop(Value::Object(typed_array_slice_fn)),
            );
            props.insert(
                PropertyKey::from("find"),
                data_prop(Value::Object(typed_array_find_fn)),
            );
            props.insert(
                PropertyKey::from("findIndex"),
                data_prop(Value::Object(typed_array_find_index_fn)),
            );
            props.insert(
                PropertyKey::from("findLast"),
                data_prop(Value::Object(typed_array_find_last_fn)),
            );
            props.insert(
                PropertyKey::from("findLastIndex"),
                data_prop(Value::Object(typed_array_find_last_index_fn)),
            );
            props.insert(
                PropertyKey::from("some"),
                data_prop(Value::Object(typed_array_some_fn)),
            );
            props.insert(
                PropertyKey::from("every"),
                data_prop(Value::Object(typed_array_every_fn)),
            );
            props.insert(
                PropertyKey::from("forEach"),
                data_prop(Value::Object(typed_array_for_each_fn)),
            );
            props.insert(
                PropertyKey::from("includes"),
                data_prop(Value::Object(typed_array_includes_fn)),
            );
            props.insert(
                PropertyKey::from("indexOf"),
                data_prop(Value::Object(typed_array_index_of_fn)),
            );
            props.insert(
                PropertyKey::from("lastIndexOf"),
                data_prop(Value::Object(typed_array_last_index_of_fn)),
            );
            props.insert(
                PropertyKey::from("reduceRight"),
                data_prop(Value::Object(typed_array_reduce_right_fn)),
            );
            props.insert(
                PropertyKey::from("reduce"),
                data_prop(Value::Object(typed_array_reduce_fn)),
            );
            props.insert(
                PropertyKey::from("map"),
                data_prop(Value::Object(typed_array_map_fn)),
            );
            props.insert(
                PropertyKey::from("filter"),
                data_prop(Value::Object(typed_array_filter_fn)),
            );
            props.insert(
                PropertyKey::from("sort"),
                data_prop(Value::Object(typed_array_sort_fn)),
            );
            props.insert(
                PropertyKey::from("toSorted"),
                data_prop(Value::Object(typed_array_to_sorted_fn)),
            );
            props.insert(
                PropertyKey::from("with"),
                data_prop(Value::Object(typed_array_with_fn)),
            );
            props.insert(
                PropertyKey::from("join"),
                data_prop(Value::Object(typed_array_join_fn)),
            );
            props.insert(
                PropertyKey::from("toLocaleString"),
                data_prop(Value::Object(typed_array_to_locale_string_fn)),
            );
            props.insert(
                PropertyKey::from("reverse"),
                data_prop(Value::Object(typed_array_reverse_fn)),
            );
            props.insert(
                PropertyKey::from("toReversed"),
                data_prop(Value::Object(typed_array_to_reversed_fn)),
            );
            props.insert(
                PropertyKey::from("fill"),
                data_prop(Value::Object(typed_array_fill_fn)),
            );
            props.insert(
                PropertyKey::from("at"),
                data_prop(Value::Object(typed_array_at_fn)),
            );
            props.insert(
                PropertyKey::from("values"),
                data_prop(Value::Object(typed_array_values_fn)),
            );
            props.insert(
                PropertyKey::from("keys"),
                data_prop(Value::Object(typed_array_keys_fn)),
            );
            props.insert(
                PropertyKey::from("entries"),
                data_prop(Value::Object(typed_array_entries_fn)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.iterator),
                data_prop(Value::Object(typed_array_values_fn)),
            );
        });
    }

    Ok((typed_array_ctor, typed_array_proto))
}

fn install_typed_array_to_string_alias(vm: &mut Vm, typed_array_proto: &Value, to_string: Value) {
    if let Value::Object(index) = typed_array_proto {
        vm.heap.with_obj(index.0, |object| {
            object
                .props()
                .lock()
                .insert(PropertyKey::from("toString"), data_prop(to_string));
        });
    }
}

fn install_array_buffer_constructor_in_env(
    vm: &mut Vm,
    env: GcIdx,
    global: Option<&Value>,
    update_vm_slot: bool,
) -> error::Result<(Value, Value)> {
    let (array_buffer_ctor, array_buffer_proto) = make_builtin_constructor_with_in_env(
        vm,
        "ArrayBuffer",
        1,
        array_buffer_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("slice", array_buffer_slice, 2),
            ("resize", array_buffer_resize, 1),
            ("sliceToImmutable", array_buffer_slice_to_immutable, 2),
            ("transfer", array_buffer_transfer, 0),
            (
                "transferToFixedLength",
                array_buffer_transfer_to_fixed_length,
                0,
            ),
            ("transferToImmutable", array_buffer_transfer_to_immutable, 0),
        ],
        env,
    )?;
    let array_buffer_ctor = Value::Object(array_buffer_ctor);
    let array_buffer_proto = Value::Object(array_buffer_proto);
    let realm = crate::environment::global_env_root(&vm.heap, env);
    vm.realm_array_buffer_prototypes
        .insert(realm.0, array_buffer_proto.clone());
    if update_vm_slot {
        vm.array_buffer_proto = array_buffer_proto.clone();
    }
    let array_buffer_byte_length_getter =
        vm.new_native_function_in_env("get byteLength", array_buffer_byte_length_get, 0, env)?;
    let array_buffer_immutable_getter =
        vm.new_native_function_in_env("get immutable", array_buffer_immutable_get, 0, env)?;
    let array_buffer_detached_getter =
        vm.new_native_function_in_env("get detached", array_buffer_detached_get, 0, env)?;
    let array_buffer_resizable_getter =
        vm.new_native_function_in_env("get resizable", array_buffer_resizable_get, 0, env)?;
    let array_buffer_max_byte_length_getter = vm.new_native_function_in_env(
        "get maxByteLength",
        array_buffer_max_byte_length_get,
        0,
        env,
    )?;
    let array_buffer_is_view_fn =
        vm.new_native_function_in_env("isView", array_buffer_is_view, 1, env)?;
    let array_buffer_species_getter =
        vm.new_native_function_in_env("get [Symbol.species]", array_buffer_species_get, 0, env)?;
    if let Value::Object(idx) = &array_buffer_ctor {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                let mut props = f.props.lock();
                props.insert(
                    PropertyKey::from("isView"),
                    data_prop(Value::Object(array_buffer_is_view_fn)),
                );
                props.insert(
                    PropertyKey::Symbol(vm.well_known_symbols.species),
                    accessor_get_prop(Value::Object(array_buffer_species_getter)),
                );
            }
        });
    }
    if let Value::Object(idx) = &array_buffer_proto {
        vm.heap.with_obj(idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            props.insert(
                PropertyKey::from("byteLength"),
                accessor_get_prop(Value::Object(array_buffer_byte_length_getter)),
            );
            props.insert(
                PropertyKey::from("immutable"),
                accessor_get_prop(Value::Object(array_buffer_immutable_getter)),
            );
            props.insert(
                PropertyKey::from("detached"),
                accessor_get_prop(Value::Object(array_buffer_detached_getter)),
            );
            props.insert(
                PropertyKey::from("resizable"),
                accessor_get_prop(Value::Object(array_buffer_resizable_getter)),
            );
            props.insert(
                PropertyKey::from("maxByteLength"),
                accessor_get_prop(Value::Object(array_buffer_max_byte_length_getter)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
                PropertyDescriptor {
                    value: Value::String(Arc::from("ArrayBuffer")),
                    writable: false,
                    enumerable: false,
                    configurable: true,
                    get: None,
                    set: None,
                    is_accessor: false,
                },
            );
        });
    }
    if let Some(global) = global {
        define_realm_global(vm, env, global, "ArrayBuffer", array_buffer_ctor.clone());
    } else {
        define_global(vm, "ArrayBuffer", array_buffer_ctor.clone());
    }
    Ok((array_buffer_ctor, array_buffer_proto))
}

fn install_shared_array_buffer_constructor_in_env(
    vm: &mut Vm,
    env: GcIdx,
    global: Option<&Value>,
) -> error::Result<(Value, Value)> {
    let (constructor, prototype) = make_builtin_constructor_with_in_env(
        vm,
        "SharedArrayBuffer",
        1,
        shared_array_buffer_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("slice", shared_array_buffer_slice, 2),
            ("grow", shared_array_buffer_grow, 1),
        ],
        env,
    )?;
    let byte_length_getter = vm.new_native_function_in_env(
        "get byteLength",
        shared_array_buffer_byte_length_get,
        0,
        env,
    )?;
    let growable_getter =
        vm.new_native_function_in_env("get growable", shared_array_buffer_growable_get, 0, env)?;
    let max_byte_length_getter = vm.new_native_function_in_env(
        "get maxByteLength",
        shared_array_buffer_max_byte_length_get,
        0,
        env,
    )?;
    let species_getter =
        vm.new_native_function_in_env("get [Symbol.species]", array_buffer_species_get, 0, env)?;
    let function_proto = vm
        .realm_function_prototypes
        .get(&env.0)
        .cloned()
        .unwrap_or_else(|| vm.function_proto.clone());
    for function in [
        constructor,
        byte_length_getter,
        growable_getter,
        max_byte_length_getter,
        species_getter,
    ] {
        set_function_object_proto(vm, function, &function_proto);
    }
    for name in ["slice", "grow"] {
        let method = vm.heap.with_obj(prototype.0, |obj| {
            obj.props()
                .lock()
                .get(&PropertyKey::from(name))
                .map(|descriptor| descriptor.value.clone())
        });
        if let Some(Value::Object(method)) = method {
            set_function_object_proto(vm, method, &function_proto);
        }
    }
    vm.heap.with_obj(constructor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(species_getter)),
        );
    });
    vm.heap.with_obj(prototype.0, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("byteLength"),
            accessor_get_prop(Value::Object(byte_length_getter)),
        );
        props.insert(
            PropertyKey::from("growable"),
            accessor_get_prop(Value::Object(growable_getter)),
        );
        props.insert(
            PropertyKey::from("maxByteLength"),
            accessor_get_prop(Value::Object(max_byte_length_getter)),
        );
        let mut tag = data_prop(Value::String(Arc::from("SharedArrayBuffer")));
        tag.writable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });

    let constructor = Value::Object(constructor);
    let prototype = Value::Object(prototype);
    if let Some(global) = global {
        define_realm_global(vm, env, global, "SharedArrayBuffer", constructor.clone());
    } else {
        define_global(vm, "SharedArrayBuffer", constructor.clone());
    }
    Ok((constructor, prototype))
}

fn install_data_view_constructor_in_env(
    vm: &mut Vm,
    env: GcIdx,
    global: Option<&Value>,
) -> error::Result<(Value, Value)> {
    let (data_view_ctor, data_view_proto) = make_builtin_constructor_with_in_env(
        vm,
        "DataView",
        1,
        data_view_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("getFloat16", data_view_get_float16, 1),
            ("getFloat32", data_view_get_float32, 1),
            ("getFloat64", data_view_get_float64, 1),
            ("getBigInt64", data_view_get_bigint64, 1),
            ("getBigUint64", data_view_get_biguint64, 1),
            ("getInt16", data_view_get_int16, 1),
            ("getInt32", data_view_get_int32, 1),
            ("getInt8", data_view_get_int8, 1),
            ("getUint16", data_view_get_uint16, 1),
            ("getUint32", data_view_get_uint32, 1),
            ("getUint8", data_view_get_uint8, 1),
            ("setFloat16", data_view_set_float16, 2),
            ("setFloat32", data_view_set_float32, 2),
            ("setFloat64", data_view_set_float64, 2),
            ("setBigInt64", data_view_set_bigint64, 2),
            ("setBigUint64", data_view_set_biguint64, 2),
            ("setInt16", data_view_set_int16, 2),
            ("setInt32", data_view_set_int32, 2),
            ("setInt8", data_view_set_int8, 2),
            ("setUint16", data_view_set_uint16, 2),
            ("setUint32", data_view_set_uint32, 2),
            ("setUint8", data_view_set_uint8, 2),
        ],
        env,
    )?;
    let data_view_ctor = Value::Object(data_view_ctor);
    let data_view_proto = Value::Object(data_view_proto);
    let data_view_buffer_getter =
        vm.new_native_function_in_env("get buffer", data_view_buffer_get, 0, env)?;
    let data_view_byte_length_getter =
        vm.new_native_function_in_env("get byteLength", data_view_byte_length_get, 0, env)?;
    let data_view_byte_offset_getter =
        vm.new_native_function_in_env("get byteOffset", data_view_byte_offset_get, 0, env)?;
    if let Value::Object(idx) = &data_view_proto {
        vm.heap.with_obj(idx.0, |obj| {
            let mut props = obj.props().lock();
            props.insert(
                PropertyKey::from("buffer"),
                accessor_get_prop(Value::Object(data_view_buffer_getter)),
            );
            props.insert(
                PropertyKey::from("byteLength"),
                accessor_get_prop(Value::Object(data_view_byte_length_getter)),
            );
            props.insert(
                PropertyKey::from("byteOffset"),
                accessor_get_prop(Value::Object(data_view_byte_offset_getter)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
                PropertyDescriptor {
                    value: Value::String(Arc::from("DataView")),
                    writable: false,
                    enumerable: false,
                    configurable: true,
                    get: None,
                    set: None,
                    is_accessor: false,
                },
            );
        });
    }
    if let Some(global) = global {
        define_realm_global(vm, env, global, "DataView", data_view_ctor.clone());
    } else {
        define_global(vm, "DataView", data_view_ctor.clone());
    }
    Ok((data_view_ctor, data_view_proto))
}

fn install_weak_ref_constructor_in_env(
    vm: &mut Vm,
    env: GcIdx,
    global: Option<&Value>,
) -> error::Result<(Value, Value)> {
    let (constructor, prototype) = make_builtin_constructor_with_in_env(
        vm,
        "WeakRef",
        1,
        weak_ref_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[("deref", weak_ref_deref, 0)],
        env,
    )?;
    let function_proto = vm
        .realm_function_prototypes
        .get(&env.0)
        .cloned()
        .unwrap_or_else(|| vm.function_proto.clone());
    set_function_object_proto(vm, constructor, &function_proto);
    let deref = vm.heap.with_obj(prototype.0, |obj| {
        obj.props()
            .lock()
            .get(&PropertyKey::from("deref"))
            .map(|descriptor| descriptor.value.clone())
    });
    if let Some(Value::Object(deref)) = deref {
        set_function_object_proto(vm, deref, &function_proto);
    }
    vm.heap.with_obj(prototype.0, |obj| {
        let mut props = obj.props().lock();
        let mut tag = data_prop(Value::String(Arc::from("WeakRef")));
        tag.writable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });

    let constructor = Value::Object(constructor);
    let prototype = Value::Object(prototype);
    if let Some(global) = global {
        define_realm_global(vm, env, global, "WeakRef", constructor.clone());
    } else {
        define_global(vm, "WeakRef", constructor.clone());
    }
    Ok((constructor, prototype))
}

fn install_finalization_registry_constructor_in_env(
    vm: &mut Vm,
    env: GcIdx,
    global: Option<&Value>,
) -> error::Result<(Value, Value)> {
    let (constructor, prototype) = make_builtin_constructor_with_in_env(
        vm,
        "FinalizationRegistry",
        1,
        finalization_registry_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("register", finalization_registry_register, 2),
            ("unregister", finalization_registry_unregister, 1),
        ],
        env,
    )?;
    let function_proto = vm
        .realm_function_prototypes
        .get(&env.0)
        .cloned()
        .unwrap_or_else(|| vm.function_proto.clone());
    set_function_object_proto(vm, constructor, &function_proto);
    let methods = vm.heap.with_obj(prototype.0, |obj| {
        let props = obj.props().lock();
        ["register", "unregister"]
            .iter()
            .filter_map(|name| {
                props
                    .get(&PropertyKey::from(*name))
                    .map(|descriptor| descriptor.value.clone())
            })
            .collect::<Vec<_>>()
    });
    for method in methods {
        if let Value::Object(method) = method {
            set_function_object_proto(vm, method, &function_proto);
        }
    }
    vm.heap.with_obj(prototype.0, |obj| {
        let mut tag = data_prop(Value::String(Arc::from("FinalizationRegistry")));
        tag.writable = false;
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });

    let constructor = Value::Object(constructor);
    let prototype = Value::Object(prototype);
    if let Some(global) = global {
        define_realm_global(vm, env, global, "FinalizationRegistry", constructor.clone());
    } else {
        define_global(vm, "FinalizationRegistry", constructor.clone());
    }
    Ok((constructor, prototype))
}

pub(crate) fn make_error_constructor(vm: &mut Vm, name: &str) -> error::Result<(GcIdx, GcIdx)> {
    make_error_constructor_in_env(vm, name, vm.global)
}

fn make_error_constructor_in_env(
    vm: &mut Vm,
    name: &str,
    env: GcIdx,
) -> error::Result<(GcIdx, GcIdx)> {
    let realm = crate::environment::global_env_root(&vm.heap, env);
    let object_proto = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.object_proto.clone());
    let function_proto = vm
        .realm_function_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.function_proto.clone());
    let proto_parent = if name == "Error" {
        object_proto.clone()
    } else if let Some(error_proto) = vm
        .realm_error_prototypes
        .get(&(realm.0, Arc::from("Error")))
        .cloned()
    {
        error_proto
    } else if matches!(vm.error_proto, Value::Object(_)) {
        vm.error_proto.clone()
    } else {
        object_proto
    };
    let proto_obj = HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto_parent)),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj)?);

    let ctor_native = if name == "AggregateError" {
        aggregate_error_constructor
    } else {
        error_constructor
    };
    let ctor_length = if name == "AggregateError" { 2 } else { 1 };
    let ctor_func = FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Native {
            func: ctor_native,
            length: ctor_length,
            construct_mode: Some(NativeConstructMode::InternalEagerPrototype),
        },
        closure: env,
        lexical_new_target: Value::Undefined,
        home_object: Mutex::new(None),
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(Some(Value::Object(proto_idx))),
        proto: Mutex::new(match function_proto {
            Value::Object(_) => Some(function_proto),
            _ => None,
        }),
        props: Mutex::new(builtin_function_own_props(name, ctor_length)),
        extensible: AtomicBool::new(true),
        private_fields: Mutex::new(std::collections::HashMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func))?);
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(Value::Object(proto_idx)),
        );
    });
    if name == "Error" {
        let is_error_fn = vm.new_native_function_in_env("isError", error_is_error, 1, env)?;
        vm.heap.with_obj(ctor_idx.0, |obj| {
            obj.props().lock().insert(
                PropertyKey::from("isError"),
                data_prop(Value::Object(is_error_fn)),
            );
        });
    }
    let ts_fn = vm.new_native_function_in_env("toString", error_to_string, 0, env)?;
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
        obj.props().lock().insert(
            PropertyKey::from("name"),
            data_prop(Value::String(Arc::from(name))),
        );
        obj.props().lock().insert(
            PropertyKey::from("message"),
            data_prop(Value::String(Arc::from(""))),
        );
        obj.props().lock().insert(
            PropertyKey::from("toString"),
            data_prop(Value::Object(ts_fn)),
        );
    });
    if name == "Error" {
        let stack_get = vm.new_native_function_in_env("get stack", error_stack_get, 0, env)?;
        let stack_set = vm.new_native_function_in_env("set stack", error_stack_set, 1, env)?;
        vm.heap.with_obj(proto_idx.0, |obj| {
            obj.props().lock().insert(
                PropertyKey::from("stack"),
                accessor_prop(Value::Object(stack_get), Value::Object(stack_set)),
            );
        });
    }
    vm.realm_error_prototypes
        .insert((env.0, Arc::from(name)), Value::Object(proto_idx));

    Ok((ctor_idx, proto_idx))
}

pub(crate) fn define_global(vm: &mut Vm, name: &str, value: Value) {
    env::declare(&vm.heap, vm.global, name, value.clone(), BindingKind::Var);
    define_global_property(vm, name, data_prop(value));
}

pub(crate) fn define_global_const(vm: &mut Vm, name: &str, value: Value) {
    env::declare(&vm.heap, vm.global, name, value.clone(), BindingKind::Var);
    define_global_property(vm, name, const_prop(value));
}

fn define_global_property(vm: &mut Vm, name: &str, desc: PropertyDescriptor) {
    if let Value::Object(idx) = &vm.global_this {
        vm.heap.with_obj(idx.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(name), desc.clone());
        });
    }
}

fn init_global_this(vm: &mut Vm) -> error::Result<()> {
    let globalthis_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("global")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.global_this = Value::Object(GcIdx(globalthis_idx));

    for (name, value) in env::own_bindings(&vm.heap, vm.global) {
        define_global_property(vm, &name, data_prop(value));
    }
    define_global(vm, "globalThis", vm.global_this.clone());
    Ok(())
}

fn define_realm_global(vm: &mut Vm, env: GcIdx, global: &Value, name: &str, value: Value) {
    crate::environment::declare(&vm.heap, env, name, value.clone(), BindingKind::Var);
    if let Value::Object(idx) = global {
        vm.heap.with_obj(idx.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(name), data_prop(value));
        });
    }
}

fn make_regexp_constructor_in_env(vm: &mut Vm, env: GcIdx) -> error::Result<(GcIdx, GcIdx)> {
    let (regex_ctor, regex_proto) = make_builtin_constructor_with_proto_class_in_env(
        vm,
        "RegExp",
        2,
        (
            regexp_constructor,
            NativeConstructMode::InternalDeferredPrototype,
        ),
        &[
            ("test", regexp_test, 1),
            ("exec", regexp_exec, 1),
            ("toString", regexp_to_string, 0),
        ],
        env,
        None,
    )?;
    let source_getter = vm.new_native_function_in_env("get source", regexp_source_get, 0, env)?;
    let flags_getter = vm.new_native_function_in_env("get flags", regexp_flags_get, 0, env)?;
    let has_indices_getter =
        vm.new_native_function_in_env("get hasIndices", regexp_has_indices_get, 0, env)?;
    let global_getter = vm.new_native_function_in_env("get global", regexp_global_get, 0, env)?;
    let ignore_case_getter =
        vm.new_native_function_in_env("get ignoreCase", regexp_ignore_case_get, 0, env)?;
    let multiline_getter =
        vm.new_native_function_in_env("get multiline", regexp_multiline_get, 0, env)?;
    let dot_all_getter = vm.new_native_function_in_env("get dotAll", regexp_dot_all_get, 0, env)?;
    let unicode_getter =
        vm.new_native_function_in_env("get unicode", regexp_unicode_get, 0, env)?;
    let unicode_sets_getter =
        vm.new_native_function_in_env("get unicodeSets", regexp_unicode_sets_get, 0, env)?;
    let sticky_getter = vm.new_native_function_in_env("get sticky", regexp_sticky_get, 0, env)?;
    let match_fn = vm.new_native_function_in_env("[Symbol.match]", regexp_symbol_match, 1, env)?;
    let match_all_fn =
        vm.new_native_function_in_env("[Symbol.matchAll]", regexp_symbol_match_all, 1, env)?;
    let search_fn =
        vm.new_native_function_in_env("[Symbol.search]", regexp_symbol_search, 1, env)?;
    let replace_fn =
        vm.new_native_function_in_env("[Symbol.replace]", regexp_symbol_replace, 2, env)?;
    let split_fn = vm.new_native_function_in_env("[Symbol.split]", regexp_symbol_split, 2, env)?;
    vm.heap.with_obj(regex_proto.0, |o| {
        if let HeapObj::Object(obj) = o {
            let mut props = obj.props.lock();
            props.insert(
                PropertyKey::from("__regex_proto__"),
                data_prop(Value::Bool(true)),
            );
            props.insert(
                PropertyKey::from("source"),
                accessor_get_prop(Value::Object(source_getter)),
            );
            props.insert(
                PropertyKey::from("flags"),
                accessor_get_prop(Value::Object(flags_getter)),
            );
            props.insert(
                PropertyKey::from("hasIndices"),
                accessor_get_prop(Value::Object(has_indices_getter)),
            );
            props.insert(
                PropertyKey::from("global"),
                accessor_get_prop(Value::Object(global_getter)),
            );
            props.insert(
                PropertyKey::from("ignoreCase"),
                accessor_get_prop(Value::Object(ignore_case_getter)),
            );
            props.insert(
                PropertyKey::from("multiline"),
                accessor_get_prop(Value::Object(multiline_getter)),
            );
            props.insert(
                PropertyKey::from("dotAll"),
                accessor_get_prop(Value::Object(dot_all_getter)),
            );
            props.insert(
                PropertyKey::from("unicode"),
                accessor_get_prop(Value::Object(unicode_getter)),
            );
            props.insert(
                PropertyKey::from("unicodeSets"),
                accessor_get_prop(Value::Object(unicode_sets_getter)),
            );
            props.insert(
                PropertyKey::from("sticky"),
                accessor_get_prop(Value::Object(sticky_getter)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.r#match),
                data_prop(Value::Object(match_fn)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.match_all),
                data_prop(Value::Object(match_all_fn)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.search),
                data_prop(Value::Object(search_fn)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.replace),
                data_prop(Value::Object(replace_fn)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.split),
                data_prop(Value::Object(split_fn)),
            );
        }
    });
    let regexp_species_getter =
        vm.new_native_function_in_env("get [Symbol.species]", promise_species_get, 0, env)?;
    let regexp_escape_fn = vm.new_native_function_in_env("escape", regexp_escape, 1, env)?;
    vm.heap.with_obj(regex_ctor.0, |o| {
        if let HeapObj::Function(f) = o {
            f.props.lock().insert(
                PropertyKey::from("escape"),
                data_prop(Value::Object(regexp_escape_fn)),
            );
            f.props.lock().insert(
                PropertyKey::from("__proto__"),
                data_prop(Value::Object(regex_proto)),
            );
            f.props.lock().insert(
                PropertyKey::Symbol(vm.well_known_symbols.species),
                accessor_get_prop(Value::Object(regexp_species_getter)),
            );
        }
    });
    Ok((regex_ctor, regex_proto))
}

const ARRAY_PROTOTYPE_METHODS: &[(&str, NativeFn, usize)] = &[
    ("push", array_push, 1),
    ("pop", array_pop, 0),
    ("join", array_join, 1),
    ("map", array_map, 1),
    ("filter", array_filter, 1),
    ("reduce", array_reduce, 1),
    ("reduceRight", array_reduce_right, 1),
    ("toReversed", array_to_reversed, 0),
    ("toSorted", array_to_sorted, 1),
    ("toSpliced", array_to_spliced, 2),
    ("with", array_with, 2),
    ("forEach", array_for_each, 1),
    ("indexOf", array_index_of, 1),
    ("includes", array_includes, 1),
    ("slice", array_slice, 2),
    ("concat", array_concat, 1),
    ("find", array_find, 1),
    ("findIndex", array_find_index, 1),
    ("findLast", array_find_last, 1),
    ("findLastIndex", array_find_last_index, 1),
    ("fill", array_fill, 1),
    ("some", array_some, 1),
    ("every", array_every, 1),
    ("reverse", array_reverse, 0),
    ("sort", array_sort, 1),
    ("shift", array_shift, 0),
    ("unshift", array_unshift, 1),
    ("splice", array_splice, 2),
    ("lastIndexOf", array_last_index_of, 1),
    ("at", array_at, 1),
    ("flat", array_flat, 0),
    ("flatMap", array_flat_map, 1),
    ("copyWithin", array_copy_within, 2),
    ("keys", array_keys, 0),
    ("values", array_values, 0),
    ("entries", array_entries, 0),
    ("toString", array_to_string, 0),
    ("toLocaleString", array_to_locale_string, 0),
];

fn install_array_intrinsic_in_env(
    vm: &mut Vm,
    env: GcIdx,
    realm_global: Option<&Value>,
) -> error::Result<(GcIdx, GcIdx)> {
    let (constructor, prototype) = make_builtin_constructor_with_array_prototype_in_env(
        vm,
        "Array",
        1,
        array_constructor,
        NativeConstructMode::InternalEagerPrototype,
        ARRAY_PROTOTYPE_METHODS,
        env,
    )?;
    let constructor_value = Value::Object(constructor);
    let prototype_value = Value::Object(prototype);
    let mut pin_count = vm.pin(&constructor_value);
    pin_count += vm.pin(&prototype_value);

    let function_prototype = vm
        .realm_function_prototypes
        .get(&env.0)
        .cloned()
        .unwrap_or_else(|| vm.function_proto.clone());
    set_function_object_proto(vm, constructor, &function_prototype);
    let object_prototype = vm
        .realm_object_prototypes
        .get(&env.0)
        .cloned()
        .unwrap_or_else(|| vm.object_proto.clone());
    vm.heap.with_obj(prototype.0, |object| {
        *object.proto().lock() = Some(object_prototype);
        let mut length = PropertyDescriptor::data(Value::Number(0.0));
        length.enumerable = false;
        length.configurable = false;
        object
            .props()
            .lock()
            .insert(PropertyKey::from("length"), length);
    });

    let values = vm.get_property(&prototype_value, "values")?;
    vm.realm_array_values_functions
        .insert(env.0, values.clone());
    vm.heap.with_obj(prototype.0, |object| {
        object.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.iterator),
            data_prop(values),
        );
    });

    for (name, function, length) in [
        ("isArray", array_is_array as NativeFn, 1),
        ("from", array_from as NativeFn, 1),
        ("fromAsync", array_from_async as NativeFn, 1),
        ("of", array_of as NativeFn, 0),
    ] {
        let method = vm.new_native_function_in_env(name, function, length, env)?;
        vm.heap.with_obj(constructor.0, |object| {
            object
                .props()
                .lock()
                .insert(PropertyKey::from(name), data_prop(Value::Object(method)));
        });
    }
    let species =
        vm.new_native_function_in_env("get [Symbol.species]", promise_species_get, 0, env)?;
    vm.heap.with_obj(constructor.0, |object| {
        object.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(species)),
        );
    });

    vm.realm_array_constructors
        .insert(env.0, constructor_value.clone());
    vm.realm_array_prototypes
        .insert(env.0, prototype_value.clone());
    if env == vm.global {
        define_global(vm, "Array", constructor_value);
    } else if let Some(global) = realm_global {
        define_realm_global(vm, env, global, "Array", constructor_value);
    }
    vm.unpin_many(pin_count);
    Ok((constructor, prototype))
}

fn install_promise_intrinsic_in_env(
    vm: &mut Vm,
    env: GcIdx,
    realm_global: Option<&Value>,
) -> error::Result<(GcIdx, GcIdx)> {
    let realm = crate::environment::global_env_root(&vm.heap, env);
    let (constructor, prototype) = make_builtin_constructor_with_in_env(
        vm,
        "Promise",
        1,
        promise_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("then", promise_then, 2),
            ("catch", promise_catch, 1),
            ("finally", promise_finally, 1),
        ],
        realm,
    )?;
    let constructor_value = Value::Object(constructor);
    let prototype_value = Value::Object(prototype);
    let pin_count = vm.pin_many(&[constructor_value.clone(), prototype_value.clone()]);

    vm.heap.with_obj(prototype.0, |object| {
        let mut tag = data_prop(Value::String(Arc::from("Promise")));
        tag.writable = false;
        object.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });
    for (name, function, length) in [
        ("resolve", promise_static_resolve as NativeFn, 1),
        ("reject", promise_static_reject as NativeFn, 1),
        ("all", promise_static_all as NativeFn, 1),
        ("allKeyed", promise_static_all_keyed as NativeFn, 1),
        ("race", promise_static_race as NativeFn, 1),
        ("allSettled", promise_static_all_settled as NativeFn, 1),
        (
            "allSettledKeyed",
            promise_static_all_settled_keyed as NativeFn,
            1,
        ),
        ("any", promise_static_any as NativeFn, 1),
        ("try", promise_static_try as NativeFn, 1),
        ("withResolvers", promise_with_resolvers as NativeFn, 0),
    ] {
        let method = vm.new_native_function_in_env(name, function, length, realm)?;
        vm.heap.with_obj(constructor.0, |object| {
            object
                .props()
                .lock()
                .insert(PropertyKey::from(name), data_prop(Value::Object(method)));
        });
    }
    let species =
        vm.new_native_function_in_env("get [Symbol.species]", promise_species_get, 0, realm)?;
    vm.heap.with_obj(constructor.0, |object| {
        object.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(species)),
        );
    });

    vm.realm_promise_constructors
        .insert(realm.0, constructor_value.clone());
    vm.realm_promise_prototypes.insert(realm.0, prototype_value);
    if realm == vm.global {
        define_global(vm, "Promise", constructor_value);
    } else if let Some(global) = realm_global {
        define_realm_global(vm, realm, global, "Promise", constructor_value);
    }
    vm.unpin_many(pin_count);
    Ok((constructor, prototype))
}

fn install_generator_intrinsics_in_env(
    vm: &mut Vm,
    env: GcIdx,
    iterator_prototype: Value,
    function_prototype: Value,
    function_constructor: GcIdx,
) -> error::Result<(GcIdx, GcIdx)> {
    let realm = crate::environment::global_env_root(&vm.heap, env);
    let generator_prototype = vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(iterator_prototype)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Generator")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let generator_prototype_value = Value::Object(generator_prototype);
    let mut pins = vm.pin(&generator_prototype_value);
    let next = vm.new_native_function_in_env("next", generator_next, 1, realm)?;
    let return_method = vm.new_native_function_in_env("return", generator_return, 1, realm)?;
    let throw = vm.new_native_function_in_env("throw", generator_throw, 1, realm)?;
    vm.heap.with_obj(generator_prototype.0, |object| {
        let mut props = object.props().lock();
        props.insert(PropertyKey::from("next"), data_prop(Value::Object(next)));
        props.insert(
            PropertyKey::from("return"),
            data_prop(Value::Object(return_method)),
        );
        props.insert(PropertyKey::from("throw"), data_prop(Value::Object(throw)));
        let mut tag = data_prop(Value::String(Arc::from("Generator")));
        tag.writable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });

    let constructor = vm.new_native_constructor_in_env(
        "GeneratorFunction",
        generator_function_constructor,
        1,
        realm,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    let constructor_value = Value::Object(constructor);
    pins += vm.pin(&constructor_value);
    set_function_object_proto(vm, constructor, &Value::Object(function_constructor));

    let function_prototype_idx = vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(function_prototype)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("GeneratorFunction")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let function_prototype_value = Value::Object(function_prototype_idx);
    pins += vm.pin(&function_prototype_value);
    vm.heap.with_obj(function_prototype_idx.0, |object| {
        let mut props = object.props().lock();
        let mut constructor_desc = data_prop(constructor_value.clone());
        constructor_desc.writable = false;
        props.insert(PropertyKey::from("constructor"), constructor_desc);
        let mut prototype_desc = data_prop(generator_prototype_value.clone());
        prototype_desc.writable = false;
        props.insert(PropertyKey::from("prototype"), prototype_desc);
        let mut tag = data_prop(Value::String(Arc::from("GeneratorFunction")));
        tag.writable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });
    vm.heap.with_obj(generator_prototype.0, |object| {
        let mut constructor_desc = data_prop(function_prototype_value.clone());
        constructor_desc.writable = false;
        object
            .props()
            .lock()
            .insert(PropertyKey::from("constructor"), constructor_desc);
    });
    vm.heap.with_obj(constructor.0, |object| {
        if let HeapObj::Function(function) = object {
            *function.prototype.lock() = Some(function_prototype_value.clone());
        }
        object.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(function_prototype_value.clone()),
        );
    });

    vm.realm_generator_prototypes
        .insert(realm.0, generator_prototype_value);
    vm.realm_generator_function_constructors
        .insert(realm.0, constructor_value);
    vm.realm_generator_function_prototypes
        .insert(realm.0, function_prototype_value);
    vm.unpin_many(pins);
    Ok((generator_prototype, function_prototype_idx))
}

fn install_async_generator_intrinsics_in_env(
    vm: &mut Vm,
    env: GcIdx,
    object_prototype: Value,
    function_prototype: Value,
    function_constructor: GcIdx,
) -> error::Result<(GcIdx, GcIdx, GcIdx)> {
    let realm = crate::environment::global_env_root(&vm.heap, env);
    let async_iterator_prototype = vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(object_prototype)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("AsyncIterator")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let async_iterator_prototype_value = Value::Object(async_iterator_prototype);
    let mut pins = vm.pin(&async_iterator_prototype_value);
    let async_iterator = vm.new_native_function_in_env(
        "[Symbol.asyncIterator]",
        collections::collection_iterator_this,
        0,
        realm,
    )?;
    let async_dispose =
        vm.new_native_function_in_env("[Symbol.asyncDispose]", async_iterator_dispose, 0, realm)?;
    vm.heap.with_obj(async_iterator_prototype.0, |object| {
        let mut props = object.props().lock();
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.async_iterator),
            data_prop(Value::Object(async_iterator)),
        );
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.async_dispose),
            data_prop(Value::Object(async_dispose)),
        );
    });

    let async_generator_prototype = vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(async_iterator_prototype_value.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("AsyncGenerator")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let async_generator_prototype_value = Value::Object(async_generator_prototype);
    pins += vm.pin(&async_generator_prototype_value);
    let next = vm.new_native_function_in_env("next", async_generator_next, 1, realm)?;
    let return_method =
        vm.new_native_function_in_env("return", async_generator_return, 1, realm)?;
    let throw = vm.new_native_function_in_env("throw", async_generator_throw, 1, realm)?;
    vm.heap.with_obj(async_generator_prototype.0, |object| {
        let mut props = object.props().lock();
        props.insert(PropertyKey::from("next"), data_prop(Value::Object(next)));
        props.insert(
            PropertyKey::from("return"),
            data_prop(Value::Object(return_method)),
        );
        props.insert(PropertyKey::from("throw"), data_prop(Value::Object(throw)));
        let mut tag = data_prop(Value::String(Arc::from("AsyncGenerator")));
        tag.writable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });

    let constructor = vm.new_native_constructor_in_env(
        "AsyncGeneratorFunction",
        async_generator_function_constructor,
        1,
        realm,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    let constructor_value = Value::Object(constructor);
    pins += vm.pin(&constructor_value);
    set_function_object_proto(vm, constructor, &Value::Object(function_constructor));

    let function_prototype_idx = vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(function_prototype)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("AsyncGeneratorFunction")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let function_prototype_value = Value::Object(function_prototype_idx);
    pins += vm.pin(&function_prototype_value);
    vm.heap.with_obj(function_prototype_idx.0, |object| {
        let mut props = object.props().lock();
        let mut constructor_desc = data_prop(constructor_value.clone());
        constructor_desc.writable = false;
        props.insert(PropertyKey::from("constructor"), constructor_desc);
        let mut prototype_desc = data_prop(async_generator_prototype_value.clone());
        prototype_desc.writable = false;
        props.insert(PropertyKey::from("prototype"), prototype_desc);
        let mut tag = data_prop(Value::String(Arc::from("AsyncGeneratorFunction")));
        tag.writable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });
    vm.heap.with_obj(async_generator_prototype.0, |object| {
        let mut constructor_desc = data_prop(function_prototype_value.clone());
        constructor_desc.writable = false;
        object
            .props()
            .lock()
            .insert(PropertyKey::from("constructor"), constructor_desc);
    });
    vm.heap.with_obj(constructor.0, |object| {
        if let HeapObj::Function(function) = object {
            *function.prototype.lock() = Some(function_prototype_value.clone());
        }
        object.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(function_prototype_value.clone()),
        );
    });

    vm.realm_async_iterator_prototypes
        .insert(realm.0, async_iterator_prototype_value);
    vm.realm_async_generator_prototypes
        .insert(realm.0, async_generator_prototype_value);
    vm.realm_async_generator_function_constructors
        .insert(realm.0, constructor_value);
    vm.realm_async_generator_function_prototypes
        .insert(realm.0, function_prototype_value);
    vm.unpin_many(pins);
    Ok((
        async_iterator_prototype,
        async_generator_prototype,
        function_prototype_idx,
    ))
}

fn install_proxy_intrinsic_in_env(
    vm: &mut Vm,
    env: GcIdx,
    global: Option<&Value>,
) -> error::Result<Value> {
    let constructor = Value::Object(vm.new_native_constructor_in_env(
        "Proxy",
        proxy_constructor,
        2,
        env,
        NativeConstructMode::InternalDeferredPrototype,
    )?);
    let pin_count = vm.pin(&constructor);
    let result = (|| {
        let revocable = vm.new_native_function_in_env("revocable", proxy_revocable, 2, env)?;
        let Value::Object(constructor_idx) = constructor else {
            unreachable!();
        };
        vm.heap.with_obj(constructor_idx.0, |object| {
            object.props().lock().insert(
                PropertyKey::from("revocable"),
                data_prop(Value::Object(revocable)),
            );
        });
        let constructor = Value::Object(constructor_idx);
        if let Some(global) = global {
            define_realm_global(vm, env, global, "Proxy", constructor.clone());
        } else {
            define_global(vm, "Proxy", constructor.clone());
        }
        Ok(constructor)
    })();
    vm.unpin_many(pin_count);
    result
}

fn populate_test262_realm(vm: &mut Vm, realm_env: GcIdx) -> error::Result<Value> {
    let global_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(None),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("realm-global")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let global = Value::Object(GcIdx(global_idx));
    vm.realm_globals.insert(realm_env.0, global.clone());

    crate::environment::declare(
        &vm.heap,
        realm_env,
        "this",
        global.clone(),
        BindingKind::Const,
    );
    define_realm_global(vm, realm_env, &global, "globalThis", global.clone());

    let eval_idx = vm.new_native_function_in_env("eval", global_eval, 1, realm_env)?;
    let eval_value = Value::Object(eval_idx);
    vm.realm_eval_functions
        .insert(realm_env.0, eval_value.clone());
    define_realm_global(vm, realm_env, &global, "eval", eval_value);

    let parse_int_idx =
        vm.new_native_function_in_env("parseInt", global_parse_int, 2, realm_env)?;
    define_realm_global(
        vm,
        realm_env,
        &global,
        "parseInt",
        Value::Object(parse_int_idx),
    );
    let realm_function_proto_idx =
        vm.new_native_function_in_env("Function.prototype", function_proto_noop, 0, realm_env)?;
    let realm_function_proto = Value::Object(realm_function_proto_idx);
    vm.heap.with_obj(realm_function_proto_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            *f.proto.lock() = None;
        }
    });
    vm.realm_function_prototypes
        .insert(realm_env.0, realm_function_proto.clone());
    for function in [eval_idx, parse_int_idx] {
        set_function_object_proto(vm, function, &realm_function_proto);
    }
    install_proxy_intrinsic_in_env(vm, realm_env, Some(&global))?;

    let function_ctor_idx = vm.new_native_constructor_in_env(
        "Function",
        function_constructor,
        1,
        realm_env,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    set_function_object_proto(vm, function_ctor_idx, &realm_function_proto);
    let call_fn = vm.new_native_function_in_env("call", function_call, 1, realm_env)?;
    let apply_fn = vm.new_native_function_in_env("apply", function_apply, 2, realm_env)?;
    let bind_fn = vm.new_native_function_in_env("bind", function_bind, 1, realm_env)?;
    let tostring_fn =
        vm.new_native_function_in_env("toString", function_to_string, 0, realm_env)?;
    let has_instance_fn = vm.new_native_function_in_env(
        "[Symbol.hasInstance]",
        function_symbol_has_instance,
        1,
        realm_env,
    )?;
    for idx in [call_fn, apply_fn, bind_fn, tostring_fn, has_instance_fn] {
        set_function_object_proto(vm, idx, &realm_function_proto);
    }
    let throw_type_error_fn = throw_type_error_intrinsic(vm, realm_env)?;
    install_methods(
        vm,
        &realm_function_proto,
        &[
            (Arc::from("call"), Value::Object(call_fn)),
            (Arc::from("apply"), Value::Object(apply_fn)),
            (Arc::from("bind"), Value::Object(bind_fn)),
            (Arc::from("toString"), Value::Object(tostring_fn)),
        ],
    );
    vm.heap.with_obj(function_ctor_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            f.prototype.lock().replace(realm_function_proto.clone());
        }
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(realm_function_proto.clone()),
        );
    });
    vm.heap.with_obj(realm_function_proto_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(function_ctor_idx)),
        );
        let mut has_instance_desc = PropertyDescriptor::data(Value::Object(has_instance_fn));
        has_instance_desc.writable = false;
        has_instance_desc.enumerable = false;
        has_instance_desc.configurable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.has_instance),
            has_instance_desc,
        );
        let restricted = PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: false,
            configurable: true,
            get: Some(throw_type_error_fn.clone()),
            set: Some(throw_type_error_fn),
            is_accessor: true,
        };
        props.insert(PropertyKey::from("caller"), restricted.clone());
        props.insert(PropertyKey::from("arguments"), restricted);
    });
    define_realm_global(
        vm,
        realm_env,
        &global,
        "Function",
        Value::Object(function_ctor_idx),
    );
    let realm_object_prototype_idx = vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(None),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Object")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let realm_object_prototype = Value::Object(realm_object_prototype_idx);
    let mut object_pins = vm.pin(&realm_object_prototype);
    let realm_object_idx = vm.new_native_constructor_in_env(
        "Object",
        object_constructor,
        1,
        realm_env,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    let realm_object = Value::Object(realm_object_idx);
    object_pins += vm.pin(&realm_object);
    vm.heap.with_obj(realm_object_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(realm_object_prototype.clone()),
        );
        if let HeapObj::Function(function) = obj {
            *function.prototype.lock() = Some(realm_object_prototype.clone());
        }
    });
    set_function_object_proto(vm, realm_object_idx, &realm_function_proto);
    install_object_static_methods_in_env(vm, realm_object_idx, realm_env)?;
    install_object_prototype_methods_in_env(vm, realm_object_prototype_idx, realm_env)?;
    vm.heap.with_obj(realm_object_prototype_idx.0, |object| {
        object.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(realm_object.clone()),
        );
    });
    install_object_proto_accessor_in_env(vm, realm_object_prototype_idx, realm_env)?;
    define_realm_global(vm, realm_env, &global, "Object", realm_object);
    vm.register_realm_object_prototype(realm_env, realm_object_prototype.clone());
    vm.heap.with_obj(global_idx, |object| {
        *object.proto().lock() = Some(realm_object_prototype.clone());
    });
    vm.heap.with_obj(realm_function_proto_idx.0, |object| {
        *object.proto().lock() = Some(realm_object_prototype.clone());
    });
    vm.unpin_many(object_pins);

    let realm_reflect = build_reflect_in_env(vm, realm_env, realm_object_prototype.clone())?;
    define_realm_global(vm, realm_env, &global, "Reflect", realm_reflect);

    let realm_math = build_math_in_env(vm, realm_env, realm_object_prototype.clone())?;
    define_realm_global(vm, realm_env, &global, "Math", realm_math);

    let (realm_error_ctor, realm_error_proto) =
        make_error_constructor_in_env(vm, "Error", realm_env)?;
    define_realm_global(
        vm,
        realm_env,
        &global,
        "Error",
        Value::Object(realm_error_ctor),
    );
    let realm_error_ctor_value = Value::Object(realm_error_ctor);
    let realm_error_proto_value = Value::Object(realm_error_proto);
    for name in [
        "EvalError",
        "RangeError",
        "ReferenceError",
        "SyntaxError",
        "TypeError",
        "URIError",
        "AggregateError",
    ] {
        let (ctor, proto) = make_error_constructor_in_env(vm, name, realm_env)?;
        vm.heap.with_obj(ctor.0, |obj| {
            if let HeapObj::Function(f) = obj {
                *f.proto.lock() = Some(realm_error_ctor_value.clone());
            }
        });
        vm.heap.with_obj(proto.0, |obj| {
            *obj.proto().lock() = Some(realm_error_proto_value.clone());
        });
        define_realm_global(vm, realm_env, &global, name, Value::Object(ctor));
    }

    install_async_function_intrinsic(vm, realm_env, &realm_function_proto, function_ctor_idx)?;
    let object_proto = vm
        .realm_object_prototypes
        .get(&realm_env.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Object prototype intrinsic"))?;
    let realm_iterator_proto =
        install_iterator_intrinsic_in_env(vm, realm_env, Some(&global), object_proto.clone())?;
    setup_regexp_string_iterator_proto_in_env(vm, realm_env, realm_iterator_proto.clone())?;
    install_generator_intrinsics_in_env(
        vm,
        realm_env,
        realm_iterator_proto.clone(),
        realm_function_proto.clone(),
        function_ctor_idx,
    )?;
    install_async_generator_intrinsics_in_env(
        vm,
        realm_env,
        object_proto.clone(),
        realm_function_proto.clone(),
        function_ctor_idx,
    )?;
    install_array_intrinsic_in_env(vm, realm_env, Some(&global))?;
    setup_array_iterator_proto_in_env(vm, realm_env, realm_iterator_proto.clone())?;
    let (str_ctor, str_proto) = make_builtin_constructor_with_in_env(
        vm,
        "String",
        1,
        string_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("valueOf", string_value_of, 0),
            ("toString", string_proto_to_string, 0),
        ],
        realm_env,
    )?;
    let realm_string_proto = Value::Object(str_proto);
    vm.set_primitive(&realm_string_proto, Value::String(Arc::from("")));
    vm.heap.with_obj(str_proto.0, |obj| {
        obj.props()
            .lock()
            .insert(PropertyKey::from("length"), const_prop(Value::Number(0.0)));
    });
    setup_string_iterator_proto_in_env(vm, realm_env, &realm_string_proto, realm_iterator_proto)?;
    define_realm_global(vm, realm_env, &global, "String", Value::Object(str_ctor));

    let (num_ctor, num_proto) = make_builtin_constructor_with_in_env(
        vm,
        "Number",
        1,
        number_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("toString", num_proto_to_string, 1),
            ("toLocaleString", num_proto_to_locale_string, 0),
            ("valueOf", number_value_of, 0),
        ],
        realm_env,
    )?;
    vm.set_primitive(&Value::Object(num_proto), Value::Number(0.0));
    define_realm_global(vm, realm_env, &global, "Number", Value::Object(num_ctor));

    let (bool_ctor, bool_proto) = make_builtin_constructor_with_in_env(
        vm,
        "Boolean",
        1,
        boolean_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("valueOf", boolean_value_of, 0),
            ("toString", boolean_to_string, 0),
        ],
        realm_env,
    )?;
    vm.set_primitive(&Value::Object(bool_proto), Value::Bool(false));
    define_realm_global(vm, realm_env, &global, "Boolean", Value::Object(bool_ctor));

    let bigint_idx = vm.new_native_constructor_in_env(
        "BigInt",
        global_bigint,
        1,
        realm_env,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    let bigint_as_int_n = vm.new_native_function_in_env("asIntN", bigint_as_int_n, 2, realm_env)?;
    let bigint_as_uint_n =
        vm.new_native_function_in_env("asUintN", bigint_as_uint_n, 2, realm_env)?;
    let bigint_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("BigInt")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let realm_bigint_proto = Value::Object(GcIdx(bigint_proto_idx));
    let bigint_to_string =
        vm.new_native_function_in_env("toString", bigint_to_string, 0, realm_env)?;
    let bigint_to_locale_string = vm.new_native_function_in_env(
        "toLocaleString",
        bigint_proto_to_locale_string,
        0,
        realm_env,
    )?;
    let bigint_value_of =
        vm.new_native_function_in_env("valueOf", bigint_value_of, 0, realm_env)?;
    vm.heap.with_obj(bigint_idx.0, |obj| {
        if let HeapObj::Function(function) = obj {
            *function.prototype.lock() = Some(realm_bigint_proto.clone());
        }
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("prototype"),
            const_prop(realm_bigint_proto.clone()),
        );
        props.insert(
            PropertyKey::from("asIntN"),
            data_prop(Value::Object(bigint_as_int_n)),
        );
        props.insert(
            PropertyKey::from("asUintN"),
            data_prop(Value::Object(bigint_as_uint_n)),
        );
    });
    vm.heap.with_obj(bigint_proto_idx, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(bigint_idx)),
        );
        props.insert(
            PropertyKey::from("toString"),
            data_prop(Value::Object(bigint_to_string)),
        );
        props.insert(
            PropertyKey::from("toLocaleString"),
            data_prop(Value::Object(bigint_to_locale_string)),
        );
        let mut tag = data_prop(Value::String(Arc::from("BigInt")));
        tag.writable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
        props.insert(
            PropertyKey::from("valueOf"),
            data_prop(Value::Object(bigint_value_of)),
        );
    });
    define_realm_global(vm, realm_env, &global, "BigInt", Value::Object(bigint_idx));

    let (regexp_ctor, regexp_proto) = make_regexp_constructor_in_env(vm, realm_env)?;
    vm.realm_regexp_constructors
        .insert(realm_env.0, Value::Object(regexp_ctor));
    vm.realm_regexp_prototypes
        .insert(realm_env.0, Value::Object(regexp_proto));
    define_realm_global(vm, realm_env, &global, "RegExp", Value::Object(regexp_ctor));
    let symbol_idx = vm.new_native_constructor_in_env(
        "Symbol",
        symbol_constructor,
        0,
        realm_env,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    let symbol_for_idx = vm.new_native_function_in_env("for", symbol_for, 1, realm_env)?;
    let symbol_key_for_idx =
        vm.new_native_function_in_env("keyFor", symbol_key_for, 1, realm_env)?;
    let symbol_to_string_idx =
        vm.new_native_function_in_env("toString", symbol_to_string, 0, realm_env)?;
    let symbol_value_of_idx =
        vm.new_native_function_in_env("valueOf", symbol_value_of, 0, realm_env)?;
    let symbol_to_primitive_idx =
        vm.new_native_function_in_env("[Symbol.toPrimitive]", symbol_to_primitive, 1, realm_env)?;
    let symbol_description_idx =
        vm.new_native_function_in_env("get description", symbol_description_get, 0, realm_env)?;
    let mut symbol_proto_props = IndexMap::new();
    symbol_proto_props.insert(
        PropertyKey::from("toString"),
        data_prop(Value::Object(symbol_to_string_idx)),
    );
    symbol_proto_props.insert(
        PropertyKey::from("valueOf"),
        data_prop(Value::Object(symbol_value_of_idx)),
    );
    symbol_proto_props.insert(
        PropertyKey::from("description"),
        accessor_get_prop(Value::Object(symbol_description_idx)),
    );
    symbol_proto_props.insert(
        PropertyKey::Symbol(vm.well_known_symbols.to_primitive),
        PropertyDescriptor {
            value: Value::Object(symbol_to_primitive_idx),
            writable: false,
            enumerable: false,
            configurable: true,
            get: None,
            set: None,
            is_accessor: false,
        },
    );
    symbol_proto_props.insert(
        PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
        PropertyDescriptor {
            value: Value::String(Arc::from("Symbol")),
            writable: false,
            enumerable: false,
            configurable: true,
            get: None,
            set: None,
            is_accessor: false,
        },
    );
    symbol_proto_props.insert(
        PropertyKey::from("constructor"),
        data_prop(Value::Object(symbol_idx)),
    );
    let symbol_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(symbol_proto_props),
        proto: Mutex::new(Some(object_proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Symbol")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let realm_symbol_proto = Value::Object(GcIdx(symbol_proto_idx));
    vm.heap.with_obj(symbol_idx.0, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("for"),
            data_prop(Value::Object(symbol_for_idx)),
        );
        props.insert(
            PropertyKey::from("keyFor"),
            data_prop(Value::Object(symbol_key_for_idx)),
        );
        install_symbol_static_properties(vm, &mut props);
        props.insert(
            PropertyKey::from("prototype"),
            const_prop(realm_symbol_proto.clone()),
        );
        drop(props);
        if let HeapObj::Function(function) = obj {
            *function.prototype.lock() = Some(realm_symbol_proto.clone());
        }
    });
    define_realm_global(vm, realm_env, &global, "Symbol", Value::Object(symbol_idx));
    for (kind, prototype) in [
        (
            crate::vm::PrimitivePrototypeKind::String,
            Value::Object(str_proto),
        ),
        (
            crate::vm::PrimitivePrototypeKind::Number,
            Value::Object(num_proto),
        ),
        (
            crate::vm::PrimitivePrototypeKind::Boolean,
            Value::Object(bool_proto),
        ),
        (
            crate::vm::PrimitivePrototypeKind::BigInt,
            realm_bigint_proto,
        ),
        (
            crate::vm::PrimitivePrototypeKind::Symbol,
            realm_symbol_proto,
        ),
    ] {
        vm.realm_primitive_prototypes
            .insert((realm_env.0, kind), prototype);
    }

    install_date_intrinsic_in_env(vm, realm_env, Some(&global))?;

    install_array_buffer_constructor_in_env(vm, realm_env, Some(&global), false)?;
    install_shared_array_buffer_constructor_in_env(vm, realm_env, Some(&global))?;
    install_data_view_constructor_in_env(vm, realm_env, Some(&global))?;
    let (typed_array_ctor, typed_array_proto) = make_typed_array_intrinsic_in_env(vm, realm_env)?;
    let realm_array_prototype = vm
        .realm_array_prototypes
        .get(&realm_env.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Array prototype intrinsic"))?;
    let realm_array_to_string = vm.get_property(&realm_array_prototype, "toString")?;
    install_typed_array_to_string_alias(vm, &typed_array_proto, realm_array_to_string);
    for entry in typed_array_constructor_entries() {
        install_typed_array_constructor_in_env(
            vm,
            realm_env,
            Some(&global),
            entry,
            &typed_array_ctor,
            &typed_array_proto,
        )?;
    }
    install_promise_intrinsic_in_env(vm, realm_env, Some(&global))?;
    install_atomics_in_env(vm, realm_env, Some(&global))?;
    install_weak_ref_constructor_in_env(vm, realm_env, Some(&global))?;
    install_finalization_registry_constructor_in_env(vm, realm_env, Some(&global))?;
    vm.install_heap_limit_error_in_realm(realm_env)?;

    Ok(global)
}

fn install_date_intrinsic_in_env(
    vm: &mut Vm,
    realm_env: GcIdx,
    realm_global: Option<&Value>,
) -> error::Result<()> {
    let (constructor, prototype) = make_builtin_constructor_with_proto_class_in_env(
        vm,
        "Date",
        7,
        (
            date_constructor,
            NativeConstructMode::InternalDeferredPrototype,
        ),
        &[
            ("valueOf", date_get_time, 0),
            ("getTime", date_get_time, 0),
            ("getFullYear", date_get_component, 0),
            ("getUTCFullYear", date_get_component, 0),
            ("getMonth", date_get_component, 0),
            ("getUTCMonth", date_get_component, 0),
            ("getDate", date_get_component, 0),
            ("getUTCDate", date_get_component, 0),
            ("getDay", date_get_component, 0),
            ("getUTCDay", date_get_component, 0),
            ("getHours", date_get_component, 0),
            ("getUTCHours", date_get_component, 0),
            ("getMinutes", date_get_component, 0),
            ("getUTCMinutes", date_get_component, 0),
            ("getSeconds", date_get_component, 0),
            ("getUTCSeconds", date_get_component, 0),
            ("getMilliseconds", date_get_component, 0),
            ("getUTCMilliseconds", date_get_component, 0),
            ("setTime", date_set_component, 1),
            ("setMilliseconds", date_set_component, 1),
            ("setUTCMilliseconds", date_set_component, 1),
            ("setSeconds", date_set_component, 2),
            ("setUTCSeconds", date_set_component, 2),
            ("setMinutes", date_set_component, 3),
            ("setUTCMinutes", date_set_component, 3),
            ("setHours", date_set_component, 4),
            ("setUTCHours", date_set_component, 4),
            ("setDate", date_set_component, 1),
            ("setUTCDate", date_set_component, 1),
            ("setMonth", date_set_component, 2),
            ("setUTCMonth", date_set_component, 2),
            ("setFullYear", date_set_component, 3),
            ("setUTCFullYear", date_set_component, 3),
            ("toString", date_to_string, 0),
            ("toLocaleString", date_to_string, 0),
            ("toUTCString", date_to_string, 0),
            ("toTimeString", date_to_string, 0),
            ("toDateString", date_to_string, 0),
            ("toLocaleDateString", date_to_string, 0),
            ("toLocaleTimeString", date_to_string, 0),
            ("toISOString", date_to_iso_string, 0),
            ("toJSON", date_to_json, 1),
            ("toTemporalInstant", date_to_temporal_instant, 0),
            ("getTimezoneOffset", date_get_timezone_offset, 0),
        ],
        realm_env,
        None,
    )?;
    let constructor_value = Value::Object(constructor);
    let prototype_value = Value::Object(prototype);
    let mut pin_count = vm.pin_many(&[constructor_value.clone(), prototype_value.clone()]);
    let result = (|| -> error::Result<()> {
        let to_primitive =
            vm.new_native_function_in_env("[Symbol.toPrimitive]", date_to_primitive, 1, realm_env)?;
        pin_count += vm.pin(&Value::Object(to_primitive));
        let now = vm.new_native_function_in_env("now", date_now, 0, realm_env)?;
        pin_count += vm.pin(&Value::Object(now));
        let parse = vm.new_native_function_in_env("parse", date_parse, 1, realm_env)?;
        pin_count += vm.pin(&Value::Object(parse));
        let utc = vm.new_native_function_in_env("UTC", date_utc, 7, realm_env)?;
        pin_count += vm.pin(&Value::Object(utc));

        vm.heap.with_obj(prototype.0, |object| {
            object.props().lock().insert(
                PropertyKey::Symbol(vm.well_known_symbols.to_primitive),
                PropertyDescriptor {
                    value: Value::Object(to_primitive),
                    writable: false,
                    enumerable: false,
                    configurable: true,
                    get: None,
                    set: None,
                    is_accessor: false,
                },
            );
        });
        vm.heap.with_obj(constructor.0, |object| {
            let mut props = object.props().lock();
            props.insert(PropertyKey::from("now"), data_prop(Value::Object(now)));
            props.insert(PropertyKey::from("parse"), data_prop(Value::Object(parse)));
            props.insert(PropertyKey::from("UTC"), data_prop(Value::Object(utc)));
        });

        vm.realm_date_prototypes
            .insert(realm_env.0, prototype_value.clone());
        if realm_env == vm.global {
            vm.date_proto = prototype_value;
            define_global(vm, "Date", constructor_value);
        } else if let Some(global) = realm_global {
            define_realm_global(vm, realm_env, global, "Date", constructor_value);
        }
        Ok(())
    })();
    vm.unpin_many(pin_count);
    result
}

fn make_test262_realm(vm: &mut Vm) -> error::Result<Value> {
    make_test262_realm_transaction(vm, |_| {})
}

fn make_test262_realm_transaction(
    vm: &mut Vm,
    before_population: impl FnOnce(&mut Vm),
) -> error::Result<Value> {
    let pin_base = vm.gc_pins.len();
    let realm_env = crate::environment::new_env(&vm.heap, None, true)?;
    // Realm installers use fallible, stack-disciplined temporary pins. The
    // transaction owns their entire suffix so an early return cannot retain a
    // partially initialized Realm. Pin the environment itself until published
    // functions make it reachable through the provisional registry graph.
    vm.gc_pins.push(realm_env.0);
    before_population(vm);
    let result = (|| {
        let global = populate_test262_realm(vm, realm_env)?;
        let realm = vm.new_object()?;
        vm.heap.with_obj(realm.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from("global"), data_prop(global));
        });
        Ok(Value::Object(realm))
    })();
    if result.is_ok() {
        debug_assert_eq!(
            vm.gc_pins.len(),
            pin_base + 1,
            "successful Realm installation must release all nested pins"
        );
    }
    vm.gc_pins.truncate(pin_base);
    if result.is_err() {
        vm.remove_realm_registry_entries(realm_env);
    }
    result
}

#[cfg(test)]
pub(crate) fn make_test262_realm_after_environment_gc(vm: &mut Vm) -> error::Result<Value> {
    // At this point the explicit pin is the environment's only GC root.
    make_test262_realm_transaction(vm, |vm| vm.gc())
}

fn test262_create_realm(vm: &mut Vm, _args: &[Value], _: Option<Value>) -> error::Result<Value> {
    make_test262_realm(vm)
}

fn test262_eval_script(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let src = match args.first().cloned().unwrap_or(Value::Undefined) {
        Value::String(s) => s.to_string(),
        _ => return Ok(Value::Undefined),
    };
    vm.eval_script_global(&src)
}

fn test262_detach_array_buffer(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let buffer = args.first().cloned().unwrap_or(Value::Undefined);
    match buffer {
        Value::Object(idx) => {
            let detached = vm.heap.with_obj(idx.0, |obj| {
                if let HeapObj::ArrayBuffer(array_buffer) = obj {
                    array_buffer
                        .detached
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    array_buffer.bytes.lock().clear();
                    true
                } else {
                    false
                }
            });
            if detached {
                Ok(Value::Undefined)
            } else {
                Err(Error::type_err(
                    "$262.detachArrayBuffer called on non-ArrayBuffer",
                ))
            }
        }
        _ => Err(Error::type_err(
            "$262.detachArrayBuffer called on non-object",
        )),
    }
}

fn test262_agent_start(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let source = vm.to_string(args.first().unwrap_or(&Value::Undefined))?;
    let cluster = vm.agent_cluster.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    cluster.broadcasts.lock().push(sender);
    std::thread::Builder::new()
        .name("ruja-test262-agent".to_string())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut worker = match Vm::new() {
                Ok(worker) => worker,
                Err(error) => {
                    cluster
                        .reports
                        .lock()
                        .push_back(format!("agent initialization failed: {error}"));
                    return;
                }
            };
            worker.agent_cluster = cluster.clone();
            worker.agent_broadcast_rx = Some(receiver);
            worker.agent_can_block = true;
            let result = worker
                .run(&source)
                .and_then(|_| worker.run_external_jobs_until_idle());
            if let Err(error) = result {
                cluster
                    .reports
                    .lock()
                    .push_back(format!("agent execution failed: {error}"));
            }
        })
        .map_err(|error| Error::internal(format!("failed to spawn test262 agent: {error}")))?;
    Ok(Value::Undefined)
}

fn test262_agent_broadcast(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let value = args.first().unwrap_or(&Value::Undefined);
    let Value::Object(idx) = value else {
        return Err(Error::type_err(
            "$262.agent.broadcast requires SharedArrayBuffer",
        ));
    };
    let broadcast = vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return None;
        };
        if !buffer.shared {
            return None;
        }
        Some(crate::vm::AgentBroadcast {
            bytes: buffer.bytes.clone(),
            waiters: buffer.waiters.clone(),
            max_byte_length: buffer.max_byte_length,
        })
    });
    let broadcast = broadcast
        .ok_or_else(|| Error::type_err("$262.agent.broadcast requires SharedArrayBuffer"))?;
    vm.agent_cluster
        .broadcasts
        .lock()
        .retain(|sender| sender.send(broadcast.clone()).is_ok());
    Ok(Value::Undefined)
}

fn test262_agent_receive_broadcast(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let callback = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err(
            "$262.agent.receiveBroadcast requires a callback",
        ));
    }
    let receiver = vm.agent_broadcast_rx.take().ok_or_else(|| {
        Error::type_err("$262.agent.receiveBroadcast is only available in worker agents")
    })?;
    let broadcast = receiver
        .recv()
        .map_err(|_| Error::internal("test262 agent broadcast channel closed"))?;
    vm.agent_broadcast_rx = Some(receiver);
    let shared = shared_array_buffer_from_agent_broadcast(vm, broadcast)?;
    vm.call_function(&callback, &[shared], Some(Value::Undefined))?;
    Ok(Value::Undefined)
}

fn test262_agent_report(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let report = vm.to_string(args.first().unwrap_or(&Value::Undefined))?;
    vm.agent_cluster
        .reports
        .lock()
        .push_back(report.to_string());
    Ok(Value::Undefined)
}

fn test262_agent_get_report(
    vm: &mut Vm,
    _args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    Ok(vm
        .agent_cluster
        .reports
        .lock()
        .pop_front()
        .map(|report| Value::String(Arc::from(report)))
        .unwrap_or(Value::Null))
}

fn test262_agent_sleep(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let milliseconds = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    if milliseconds.is_finite() && milliseconds > 0.0 {
        std::thread::sleep(std::time::Duration::from_secs_f64(milliseconds / 1000.0));
    }
    Ok(Value::Undefined)
}

fn test262_agent_monotonic_now(
    _vm: &mut Vm,
    _args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    Ok(Value::Number(
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f64()
            * 1000.0,
    ))
}

fn test262_agent_leaving(_vm: &mut Vm, _args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::Undefined)
}

fn install_test262_host(vm: &mut Vm) -> error::Result<()> {
    let host = vm.new_object()?;
    let create_realm = vm.new_native_function("createRealm", test262_create_realm, 0)?;
    let eval_script = vm.new_native_function("evalScript", test262_eval_script, 1)?;
    let detach_array_buffer =
        vm.new_native_function("detachArrayBuffer", test262_detach_array_buffer, 1)?;
    let agent = vm.new_object()?;
    let agent_methods: &[(&str, NativeFn, usize)] = &[
        ("start", test262_agent_start, 1),
        ("broadcast", test262_agent_broadcast, 1),
        ("receiveBroadcast", test262_agent_receive_broadcast, 1),
        ("report", test262_agent_report, 1),
        ("getReport", test262_agent_get_report, 0),
        ("sleep", test262_agent_sleep, 1),
        ("monotonicNow", test262_agent_monotonic_now, 0),
        ("leaving", test262_agent_leaving, 0),
    ];
    let mut installed_agent_methods = Vec::with_capacity(agent_methods.len());
    for (name, function, length) in agent_methods {
        installed_agent_methods.push((*name, vm.new_native_function(name, *function, *length)?));
    }
    vm.heap.with_obj(agent.0, |obj| {
        let mut props = obj.props().lock();
        for (name, function) in installed_agent_methods {
            props.insert(PropertyKey::from(name), data_prop(Value::Object(function)));
        }
    });
    vm.heap.with_obj(host.0, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("createRealm"),
            data_prop(Value::Object(create_realm)),
        );
        props.insert(
            PropertyKey::from("evalScript"),
            data_prop(Value::Object(eval_script)),
        );
        props.insert(
            PropertyKey::from("detachArrayBuffer"),
            data_prop(Value::Object(detach_array_buffer)),
        );
        props.insert(
            PropertyKey::from("global"),
            data_prop(vm.global_this.clone()),
        );
        props.insert(PropertyKey::from("agent"), data_prop(Value::Object(agent)));
    });
    define_global(vm, "$262", Value::Object(host));
    Ok(())
}

pub(crate) fn get_arg(args: &[Value], idx: usize) -> Value {
    args.get(idx).cloned().unwrap_or(Value::Undefined)
}

// ---------------------------------------------------------------------------
// Object constructor / prototype
// ---------------------------------------------------------------------------

fn new_object_with_prototype(vm: &mut Vm, prototype: Value) -> error::Result<Value> {
    let pin_count = vm.pin(&prototype);
    let result = vm
        .alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

fn new_object_in_current_realm(vm: &mut Vm) -> error::Result<Value> {
    let realm = vm.current_realm_global_env();
    let prototype = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.object_proto.clone());
    new_object_with_prototype(vm, prototype)
}

fn object_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let has_distinct_new_target = match (
        vm.current_native_new_target().cloned(),
        vm.current_native_callee().cloned(),
    ) {
        (Some(new_target), Some(active_function)) => !vm.strict_eq(&new_target, &active_function),
        _ => false,
    };
    if has_distinct_new_target {
        let fallback = vm.object_proto.clone();
        let prototype = native_constructor_prototype(vm, fallback)?;
        return new_object_with_prototype(vm, prototype);
    }

    let first = args.first().unwrap_or(&Value::Undefined);
    match first {
        Value::Undefined | Value::Null => new_object_in_current_realm(vm),
        Value::Bool(_)
        | Value::Number(_)
        | Value::String(_)
        | Value::Symbol(_)
        | Value::BigInt(_) => vm.to_object(first),
        Value::Object(_) => Ok(first.clone()),
        Value::PrivateName(_) => Err(Error::type_err("Private name is not an object".to_string())),
        Value::Reference(_) => Err(Error::type_err("Reference is not an object".to_string())),
    }
}

fn object_to_string_native(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    object_to_string(vm, this, None)
}

fn object_to_locale_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let to_string = vm.get_property(&this, "toString")?;
    if !is_callable(&to_string, &vm.heap) {
        return Err(Error::type_err("toString is not a function".to_string()));
    }
    vm.call_function(&to_string, &[], Some(this))
}

fn object_has_own_key(vm: &Vm, obj: &Value, key: &PropertyKey) -> error::Result<bool> {
    if let Value::Object(idx) = obj {
        if let Some(target) = vm.heap.with_obj(idx.0, |heap_obj| {
            if let HeapObj::Proxy(proxy) = heap_obj {
                if *proxy.revoked.lock() {
                    None
                } else {
                    Some(proxy.target.clone())
                }
            } else {
                None
            }
        }) {
            return object_has_own_key(vm, &target, key);
        }

        if let Some(desc) = vm.typed_array_integer_index_own_property_descriptor(obj, key) {
            return Ok(desc.is_some());
        }
    }

    match obj {
        Value::Object(idx) => {
            let namespace_binding = vm.heap.with_obj(idx.0, |heap_obj| {
                if let HeapObj::ModuleNamespace(namespace) = heap_obj {
                    return key
                        .as_str()
                        .and_then(|name| namespace.exports.lock().get(name).cloned());
                }
                None
            });
            if let Some((env, name)) = namespace_binding {
                return match crate::environment::get_checked(&vm.heap, env, &name) {
                    Ok(_) => Ok(true),
                    Err(true) => Err(Error::reference(format!(
                        "Cannot access '{}' before initialization",
                        name
                    ))),
                    Err(false) => Ok(false),
                };
            }
            Ok(vm.heap.with_obj(idx.0, |heap_obj| {
                if let HeapObj::ModuleNamespace(namespace) = heap_obj {
                    if key
                        .as_str()
                        .is_some_and(|name| namespace.exports.lock().contains_key(name))
                    {
                        return true;
                    }
                }
                if heap_obj.props().lock().contains_key(key) {
                    return true;
                }
                if let HeapObj::Array(a) = heap_obj {
                    if key.as_str() == Some("length") {
                        return !a.is_arguments.load(Ordering::Relaxed);
                    }
                    if let Some(name) = key.as_str() {
                        if let Some(i) = crate::value::parse_array_index(name) {
                            return a.is_dense_present(i);
                        }
                    }
                }
                if let HeapObj::Object(od) = heap_obj {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        if key.as_str() == Some("length") {
                            return true;
                        }
                        return canonical_string_index(key)
                            .is_some_and(|i| i < crate::value::utf16_len(&s));
                    }
                }
                false
            }))
        }
        Value::String(s) => {
            if key.as_str() == Some("length") {
                return Ok(true);
            }
            Ok(canonical_string_index(key).is_some_and(|i| i < crate::value::utf16_len(s)))
        }
        _ => Ok(false),
    }
}

fn to_property_key_descriptor(vm: &mut Vm, value: &Value) -> error::Result<PropertyKey> {
    match vm.to_property_key_value(value)? {
        Value::String(s) => Ok(PropertyKey::from_rc(s)),
        Value::Symbol(id) => Ok(PropertyKey::Symbol(id)),
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    }
}

pub(crate) fn property_key_to_value(key: &PropertyKey) -> Value {
    match key {
        PropertyKey::Str(s) => Value::String(s.clone()),
        PropertyKey::Symbol(id) => Value::Symbol(*id),
    }
}

fn object_has_own_property(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    let key = to_property_key_descriptor(vm, args.first().unwrap_or(&Value::Undefined))?;
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&this)?;
    Ok(Value::Bool(
        own_property_descriptor_for_key_or_throw(vm, &object, &key)?.is_some(),
    ))
}

fn object_has_own(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let key = to_property_key_descriptor(vm, args.get(1).unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(
        own_property_descriptor_for_key_or_throw(vm, &obj, &key)?.is_some(),
    ))
}

fn object_property_is_enumerable(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    let key = to_property_key_descriptor(vm, args.first().unwrap_or(&Value::Undefined))?;
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&this)?;
    Ok(Value::Bool(
        own_property_descriptor_for_key_or_throw(vm, &object, &key)?
            .is_some_and(|descriptor| descriptor.enumerable),
    ))
}

fn object_value_of(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    vm.to_object(&this)
}

fn legacy_accessor_descriptor(vm: &mut Vm, slot: &str, accessor: Value) -> error::Result<Value> {
    let descriptor = new_object_in_current_realm(vm)?;
    if let Value::Object(idx) = &descriptor {
        vm.heap.with_obj(idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            props.insert(PropertyKey::from(slot), data_prop(accessor));
            props.insert(
                PropertyKey::from("enumerable"),
                data_prop(Value::Bool(true)),
            );
            props.insert(
                PropertyKey::from("configurable"),
                data_prop(Value::Bool(true)),
            );
        });
    }
    Ok(descriptor)
}

fn object_define_legacy_accessor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    slot: &str,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&this)?;
    let accessor = get_arg(args, 1);
    if !is_callable(&accessor, &vm.heap) {
        return Err(Error::type_err("Accessor must be a function".to_string()));
    }
    let mut pin_count = vm.pin(&object);
    pin_count += vm.pin(&accessor);
    let result = (|| {
        let key = to_property_key_descriptor(vm, &get_arg(args, 0))?;
        let key = property_key_to_value(&key);
        let descriptor = legacy_accessor_descriptor(vm, slot, accessor)?;
        object_define_property_result(vm, &[object.clone(), key, descriptor], true)?;
        Ok(Value::Undefined)
    })();
    vm.unpin_many(pin_count);
    result
}

fn object_define_getter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    object_define_legacy_accessor(vm, args, this, "get")
}

fn object_define_setter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    object_define_legacy_accessor(vm, args, this, "set")
}

fn object_lookup_legacy_accessor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    slot: &str,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let mut object = vm.to_object(&this)?;
    let key = to_property_key_descriptor(vm, &get_arg(args, 0))?;
    loop {
        vm.consume_fuel()?;
        let object_pin = vm.pin(&object);
        let descriptor = own_property_descriptor_for_key_or_throw(vm, &object, &key);
        match descriptor {
            Err(error) => {
                vm.unpin_many(object_pin);
                return Err(error);
            }
            Ok(Some(desc)) => {
                let result = if desc.is_accessor {
                    match slot {
                        "get" => desc.get.unwrap_or(Value::Undefined),
                        "set" => desc.set.unwrap_or(Value::Undefined),
                        _ => Value::Undefined,
                    }
                } else {
                    Value::Undefined
                };
                vm.unpin_many(object_pin);
                return Ok(result);
            }
            Ok(None) => {}
        }
        let next = vm.get_prototype_of(&object);
        vm.unpin_many(object_pin);
        match next? {
            Some(next) => object = next,
            None => return Ok(Value::Undefined),
        }
    }
}

fn object_lookup_getter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    object_lookup_legacy_accessor(vm, args, this, "get")
}

fn object_lookup_setter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    object_lookup_legacy_accessor(vm, args, this, "set")
}

fn global_decode_uri(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let input = vm.to_string(args.first().unwrap_or(&Value::Undefined))?;
    uri_decode(&input, URI_RESERVED_SET)
}

fn global_decode_uri_component(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let input = vm.to_string(args.first().unwrap_or(&Value::Undefined))?;
    uri_decode(&input, "")
}

fn global_encode_uri(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let input = vm.to_string(args.first().unwrap_or(&Value::Undefined))?;
    uri_encode(&input, URI_UNESCAPED_SET_WITH_RESERVED)
}

fn global_encode_uri_component(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let input = vm.to_string(args.first().unwrap_or(&Value::Undefined))?;
    uri_encode(&input, URI_UNESCAPED_SET)
}

const URI_UNESCAPED_SET: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!~*'()";
const URI_RESERVED_SET: &str = ";/?:@&=+$,#";
const URI_UNESCAPED_SET_WITH_RESERVED: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!~*'();/?:@&=+$,#";

fn uri_encode(input: &str, unescaped_set: &str) -> error::Result<Value> {
    let units = crate::value::utf16_from_str(input);
    let mut out = String::new();
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        let code_point = if (0xd800..=0xdbff).contains(&unit) {
            let Some(&low) = units.get(index + 1) else {
                return Err(Error::uri("malformed URI sequence"));
            };
            if !(0xdc00..=0xdfff).contains(&low) {
                return Err(Error::uri("malformed URI sequence"));
            }
            index += 2;
            0x10000 + (((unit as u32 - 0xd800) << 10) | (low as u32 - 0xdc00))
        } else if (0xdc00..=0xdfff).contains(&unit) {
            return Err(Error::uri("malformed URI sequence"));
        } else {
            index += 1;
            unit as u32
        };

        if code_point <= 0x7f {
            let ch = char::from_u32(code_point).unwrap();
            if unescaped_set.contains(ch) {
                out.push(ch);
                continue;
            }
        }

        let ch = char::from_u32(code_point).ok_or_else(|| Error::uri("malformed URI sequence"))?;
        let mut buf = [0; 4];
        for byte in ch.encode_utf8(&mut buf).as_bytes() {
            out.push('%');
            out.push(hex_digit(byte >> 4));
            out.push(hex_digit(byte & 0x0f));
        }
    }
    Ok(Value::String(Arc::from(out.as_str())))
}

fn uri_decode(input: &str, reserved_set: &str) -> error::Result<Value> {
    let mut out = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '%' {
            out.push(chars[index]);
            index += 1;
            continue;
        }

        let first = parse_uri_hex_byte(&chars, index)?;
        let utf8_len = uri_utf8_sequence_len(first)?;
        let mut bytes = Vec::with_capacity(utf8_len);
        let mut raw = String::with_capacity(utf8_len * 3);
        for offset in 0..utf8_len {
            let triplet_index = index + offset * 3;
            let byte = parse_uri_hex_byte(&chars, triplet_index)?;
            bytes.push(byte);
            raw.push('%');
            raw.push(chars[triplet_index + 1]);
            raw.push(chars[triplet_index + 2]);
        }

        let decoded =
            std::str::from_utf8(&bytes).map_err(|_| Error::uri("malformed URI sequence"))?;
        if decoded.chars().count() != 1 {
            return Err(Error::uri("malformed URI sequence"));
        }
        let decoded_char = decoded.chars().next().unwrap();
        if reserved_set.contains(decoded_char) {
            out.push_str(&raw);
        } else {
            push_decoded_uri_char(&mut out, decoded_char);
        }
        index += utf8_len * 3;
    }
    Ok(Value::String(Arc::from(out.as_str())))
}

fn push_decoded_uri_char(out: &mut String, ch: char) {
    let mut units = [0; 2];
    let encoded = ch.encode_utf16(&mut units);
    out.push_str(&crate::value::utf16_from_codes(encoded));
}

fn parse_uri_hex_byte(chars: &[char], index: usize) -> error::Result<u8> {
    if chars.get(index) != Some(&'%') {
        return Err(Error::uri("malformed URI sequence"));
    }
    let high = chars
        .get(index + 1)
        .and_then(|ch| ch.to_digit(16))
        .ok_or_else(|| Error::uri("malformed URI sequence"))?;
    let low = chars
        .get(index + 2)
        .and_then(|ch| ch.to_digit(16))
        .ok_or_else(|| Error::uri("malformed URI sequence"))?;
    Ok(((high << 4) | low) as u8)
}

fn uri_utf8_sequence_len(first: u8) -> error::Result<usize> {
    match first {
        0x00..=0x7f => Ok(1),
        0xc2..=0xdf => Ok(2),
        0xe0..=0xef => Ok(3),
        0xf0..=0xf4 => Ok(4),
        _ => Err(Error::uri("malformed URI sequence")),
    }
}

fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + (nibble - 10)) as char,
        _ => unreachable!(),
    }
}

fn this_string_value(vm: &Vm, this: Option<Value>) -> error::Result<Arc<str>> {
    match this {
        Some(Value::String(s)) => Ok(s),
        Some(Value::Object(idx)) => {
            let prim = vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    od.primitive.lock().clone()
                } else {
                    None
                }
            });
            if let Some(Value::String(s)) = prim {
                Ok(s)
            } else {
                Err(Error::type_err(
                    "String method called on incompatible receiver",
                ))
            }
        }
        _ => Err(Error::type_err(
            "String method called on incompatible receiver",
        )),
    }
}

/// `String.prototype.toString`: return the string primitive for `this`.
fn string_proto_to_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::String(this_string_value(vm, this)?))
}

/// `String.prototype.valueOf`: return the string primitive for `this`.
fn string_value_of(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(this_string_value(vm, this)?))
}

fn this_number_value(vm: &Vm, this: Option<Value>) -> error::Result<f64> {
    match this {
        Some(Value::Number(n)) => Ok(n),
        Some(Value::Object(idx)) => {
            let prim = vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    od.primitive.lock().clone()
                } else {
                    None
                }
            });
            if let Some(Value::Number(n)) = prim {
                Ok(n)
            } else {
                Err(Error::type_err(
                    "Number method called on incompatible receiver",
                ))
            }
        }
        _ => Err(Error::type_err(
            "Number method called on incompatible receiver",
        )),
    }
}

fn this_boolean_value(vm: &Vm, this: Option<Value>) -> error::Result<bool> {
    match this {
        Some(Value::Bool(b)) => Ok(b),
        Some(Value::Object(idx)) => {
            let prim = vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    od.primitive.lock().clone()
                } else {
                    None
                }
            });
            if let Some(Value::Bool(b)) = prim {
                Ok(b)
            } else {
                Err(Error::type_err(
                    "Boolean method called on incompatible receiver",
                ))
            }
        }
        _ => Err(Error::type_err(
            "Boolean method called on incompatible receiver",
        )),
    }
}

fn boolean_value_of(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(this_boolean_value(vm, this)?))
}

/// `Boolean.prototype.toString`: return "true" or "false".
fn boolean_to_string(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = this_boolean_value(vm, this)?;
    Ok(Value::String(Arc::from(if val { "true" } else { "false" })))
}

/// `Number.prototype.toString(radix)`: convert number to string in given radix.
fn num_proto_to_string(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = this_number_value(vm, this)?;
    let radix = match args.first() {
        None | Some(Value::Undefined) => 10.0,
        Some(value) => {
            let number = vm.to_number(value)?;
            if number.is_nan() {
                0.0
            } else {
                number.trunc()
            }
        }
    };
    if radix == 10.0 {
        let s = vm.to_string(&Value::Number(n))?;
        return Ok(Value::String(s));
    }
    if !(2.0..=36.0).contains(&radix) {
        return Err(Error::range("toString() radix must be between 2 and 36"));
    }
    let radix = radix as u32;
    let s = if n.is_nan() {
        "NaN".to_string()
    } else if n.is_infinite() {
        if n > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        }
    } else {
        format_radix(n, radix)
    };
    Ok(Value::String(Arc::from(s.as_str())))
}

fn num_proto_to_locale_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let number = this_number_value(vm, this)?;
    Ok(Value::String(vm.to_string(&Value::Number(number))?))
}

fn bigint_proto_to_locale_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    bigint_to_string(vm, &[], this)
}

/// `Number.prototype.valueOf`: return the number primitive for `this`.
fn number_value_of(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Number(this_number_value(vm, this)?))
}

/// Format a number in a given radix (2-36). Handles integers and fractions.
fn format_radix(n: f64, radix: u32) -> String {
    if n == 0.0 {
        return "0".to_string();
    }
    let neg = n < 0.0;
    let n = n.abs();
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut int_part = n.trunc() as u64;
    let frac_part = n.fract();
    let mut int_str = String::new();
    if int_part == 0 {
        int_str.push('0');
    } else {
        while int_part > 0 {
            let d = (int_part % radix as u64) as usize;
            int_str.insert(0, digits[d] as char);
            int_part /= radix as u64;
        }
    }
    let mut result = int_str;
    if frac_part > 0.0 {
        result.push('.');
        let mut f = frac_part;
        for _ in 0..52 {
            f *= radix as f64;
            let d = f.trunc() as usize;
            if d >= radix as usize {
                break;
            }
            result.push(digits[d] as char);
            f -= d as f64;
            if f < 1e-15 {
                break;
            }
        }
    }
    if neg {
        format!("-{}", result)
    } else {
        result
    }
}

fn array_index_key(name: &str) -> Option<u32> {
    if name.is_empty()
        || !name.bytes().all(|b| b.is_ascii_digit())
        || (name.len() > 1 && name.starts_with('0'))
    {
        return None;
    }
    name.parse::<u32>()
        .ok()
        .filter(|n| (*n as u64) < (1u64 << 32) - 1)
}

fn push_unique_key(
    keys: &mut Vec<PropertyKey>,
    seen: &mut IndexSet<PropertyKey>,
    key: PropertyKey,
) {
    if seen.insert(key.clone()) {
        keys.push(key);
    }
}

fn ordinary_own_property_keys(
    vm: &mut Vm,
    obj: &Value,
    enumerable_only: bool,
    include_strings: bool,
    include_symbols: bool,
) -> error::Result<Vec<PropertyKey>> {
    let mut keys = Vec::new();
    let mut seen = IndexSet::new();
    let typed_array_index_count = include_strings
        .then(|| vm.typed_array_integer_index_own_property_key_count(obj))
        .flatten();

    // Charge before materializing native key collections. The byte length is
    // a conservative O(1) upper bound for a string's UTF-16 key count.
    let mut scan_work = typed_array_index_count.unwrap_or(0);
    match obj {
        Value::Object(idx) => {
            scan_work = scan_work.saturating_add(vm.heap.with_obj(idx.0, |o| {
                let mut work = o.props().lock().len();
                if include_strings {
                    if let HeapObj::Array(array) = o {
                        work = work.saturating_add(array.present.lock().len());
                    }
                    if let HeapObj::Object(object) = o {
                        if let Some(Value::String(string)) = object.primitive.lock().as_ref() {
                            work = work.saturating_add(string.len());
                        }
                    }
                    if let HeapObj::ModuleNamespace(namespace) = o {
                        work = work.saturating_add(namespace.exports.lock().len());
                    }
                }
                work
            }));
        }
        Value::String(string) if include_strings => {
            scan_work = scan_work.saturating_add(string.len());
        }
        _ => {}
    }
    if vm.fuel_remaining().is_some() {
        for _ in 0..scan_work {
            vm.consume_fuel()?;
        }
    }

    match obj {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            let mut index_keys: Vec<u32> = Vec::new();
            let mut string_keys: Vec<PropertyKey> = Vec::new();
            let mut symbol_keys: Vec<PropertyKey> = Vec::new();

            if let Some(count) = typed_array_index_count {
                for i in 0..count {
                    if let Ok(index) = u32::try_from(i) {
                        index_keys.push(index);
                    }
                }
            }

            if let HeapObj::Array(a) = o {
                if include_strings {
                    for (i, present) in a.present.lock().iter().copied().enumerate() {
                        if present {
                            index_keys.push(i as u32);
                        }
                    }
                    if !enumerable_only {
                        string_keys.push(PropertyKey::from("length"));
                    }
                }
            }

            if let HeapObj::Object(od) = o {
                if include_strings {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        for i in 0..crate::value::utf16_len(&s) {
                            index_keys.push(i as u32);
                        }
                        if !enumerable_only {
                            string_keys.push(PropertyKey::from("length"));
                        }
                    }
                }
            }

            if let HeapObj::ModuleNamespace(namespace) = o {
                if include_strings {
                    for name in namespace.exports.lock().keys() {
                        string_keys.push(PropertyKey::from(name.clone()));
                    }
                }
            }

            for (k, desc) in o.props().lock().iter() {
                if enumerable_only && !desc.enumerable {
                    continue;
                }
                match k {
                    PropertyKey::Str(s) if include_strings => {
                        if let Some(index) = array_index_key(s) {
                            index_keys.push(index);
                        } else {
                            string_keys.push(PropertyKey::from(s.clone()));
                        }
                    }
                    PropertyKey::Symbol(id) if include_symbols => {
                        symbol_keys.push(PropertyKey::Symbol(*id));
                    }
                    _ => {}
                }
            }

            index_keys.sort_unstable();
            index_keys.dedup();
            for n in index_keys {
                push_unique_key(
                    &mut keys,
                    &mut seen,
                    PropertyKey::from(n.to_string().as_str()),
                );
            }
            for key in string_keys {
                push_unique_key(&mut keys, &mut seen, key);
            }
            for key in symbol_keys {
                push_unique_key(&mut keys, &mut seen, key);
            }
        }),
        Value::String(s) if include_strings => {
            for i in 0..crate::value::utf16_len(s) {
                push_unique_key(
                    &mut keys,
                    &mut seen,
                    PropertyKey::from(i.to_string().as_str()),
                );
            }
            if !enumerable_only {
                push_unique_key(&mut keys, &mut seen, PropertyKey::from("length"));
            }
        }
        _ => {}
    }
    Ok(keys)
}

pub(crate) fn make_value_array(vm: &mut Vm, items: Vec<Value>) -> error::Result<Value> {
    make_value_array_in_env(vm, items, vm.global)
}

pub(crate) fn make_value_array_in_env(
    vm: &mut Vm,
    items: Vec<Value>,
    env: GcIdx,
) -> error::Result<Value> {
    let prototype = vm.array_prototype_for_env(env);
    let required_roots =
        items
            .iter()
            .try_fold(Vm::value_root_count(&prototype), |count, item| {
                count
                    .checked_add(Vm::value_root_count(item))
                    .ok_or_else(|| Error::range("temporary root set is too large"))
            })?;
    vm.try_reserve_gc_pins(required_roots)?;
    let pin_count = vm.pin_many(&items) + vm.pin(&prototype);
    let arr = HeapObj::Array(ArrayData::new(items, Some(prototype)));
    let result = vm.alloc(arr).map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn make_value_array_in_current_realm(
    vm: &mut Vm,
    items: Vec<Value>,
) -> error::Result<Value> {
    make_value_array_in_env(vm, items, vm.current_realm_global_env())
}

pub(crate) fn make_str_array(vm: &mut Vm, strs: Vec<Arc<str>>) -> error::Result<Value> {
    let items: Vec<Value> = strs.into_iter().map(Value::String).collect();
    let arr = HeapObj::Array(ArrayData::new(items, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr)?)))
}

fn object_keys(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let keys = own_property_keys_or_throw(vm, &obj, false, true, false)?;
    let mut strings = Vec::new();
    for key in keys {
        if own_property_descriptor_for_key_or_throw(vm, &obj, &key)?
            .is_some_and(|desc| desc.enumerable)
        {
            if let PropertyKey::Str(name) = key {
                strings.push(name);
            }
        }
    }
    let realm = vm.current_realm_global_env();
    create_array_from_values_in_realm(vm, strings.into_iter().map(Value::String).collect(), realm)
}

fn object_values(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let keys = own_property_keys_or_throw(vm, &obj, false, true, false)?;
    let mut vals = Vec::with_capacity(keys.len());
    let mut value_pins = 0;
    let result = (|| {
        for key in &keys {
            let Some(k) = key.as_str() else {
                continue;
            };
            if !own_property_descriptor_for_key_or_throw(vm, &obj, key)?
                .is_some_and(|desc| desc.enumerable)
            {
                continue;
            }
            let value = vm.get_property(&obj, k)?;
            value_pins += vm.pin(&value);
            vals.push(value);
        }
        let realm = vm.current_realm_global_env();
        create_array_from_values_in_realm(vm, vals, realm)
    })();
    vm.unpin_many(value_pins);
    result
}

fn object_entries(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let keys = own_property_keys_or_throw(vm, &obj, false, true, false)?;
    let realm = vm.current_realm_global_env();
    let mut pairs = Vec::new();
    let mut pair_pins = 0;
    let result = (|| {
        for k in keys {
            if !own_property_descriptor_for_key_or_throw(vm, &obj, &k)?
                .is_some_and(|desc| desc.enumerable)
            {
                continue;
            }
            let Some(name) = k.as_str() else {
                continue;
            };
            let value = vm.get_property(&obj, name)?;
            let pair = create_array_from_values_in_realm(
                vm,
                vec![Value::String(Arc::from(name)), value],
                realm,
            )?;
            pair_pins += vm.pin(&pair);
            pairs.push(pair);
        }
        create_array_from_values_in_realm(vm, pairs, realm)
    })();
    vm.unpin_many(pair_pins);
    result
}

fn object_group_by(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let items = args.first().cloned().unwrap_or(Value::Undefined);
    if items.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let callback = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err("Object.groupBy callback must be callable"));
    }

    let iterator = vm.make_iterator(&items)?;
    let iterator_pin = vm.pin(&iterator);
    let mut groups: IndexMap<PropertyKey, Vec<Value>> = IndexMap::new();
    let mut group_pins = 0;
    let result = (|| {
        let mut k = 0usize;
        loop {
            let (value, done) = vm.iterator_next(&iterator)?;
            if done {
                break;
            }
            group_pins += vm.pin(&value);
            let key_value = match vm.call_function(
                &callback,
                &[value.clone(), Value::Number(k as f64)],
                Some(Value::Undefined),
            ) {
                Ok(value) => value,
                Err(err) => {
                    vm.iterator_close(&iterator)?;
                    return Err(err);
                }
            };
            group_pins += vm.pin(&key_value);
            let key = match to_property_key_descriptor(vm, &key_value) {
                Ok(key) => key,
                Err(err) => {
                    vm.iterator_close(&iterator)?;
                    return Err(err);
                }
            };
            groups.entry(key).or_default().push(value);
            k += 1;
        }

        let realm = vm.current_realm_global_env();
        let obj_idx = vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(None),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("Object")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?;
        let result = Value::Object(obj_idx);
        let result_pin = vm.pin(&result);
        let completion = (|| {
            for (key, values) in groups {
                let array = create_array_from_values_in_realm(vm, values, realm)?;
                vm.heap.with_obj(obj_idx.0, |o| {
                    o.props().lock().insert(
                        key,
                        PropertyDescriptor {
                            value: array,
                            writable: true,
                            enumerable: true,
                            configurable: true,
                            get: None,
                            set: None,
                            is_accessor: false,
                        },
                    );
                });
            }
            Ok(result)
        })();
        vm.unpin_many(result_pin);
        completion
    })();
    vm.unpin_many(group_pins + iterator_pin);
    result
}

fn object_assign(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if target.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let to = vm.to_object(&target)?;
    let to_pin = vm.pin(&to);
    let result = (|| -> error::Result<Value> {
        for src in &args[1..] {
            if src.is_nullish() {
                continue;
            }
            let from = vm.to_object(src)?;
            let from_pin = vm.pin(&from);
            let source_result = (|| -> error::Result<()> {
                let keys = own_property_keys_or_throw(vm, &from, false, true, true)?;
                for k in keys {
                    if !own_property_descriptor_for_key_or_throw(vm, &from, &k)?
                        .is_some_and(|desc| desc.enumerable)
                    {
                        continue;
                    }
                    let v = vm.get_property_by_key(&from, &k)?;
                    if !vm.try_set_property_key_with_receiver(&to, &k, v, &to)? {
                        return Err(Error::type_err("Cannot assign to read only property"));
                    }
                }
                Ok(())
            })();
            vm.unpin_many(from_pin);
            source_result?;
        }
        Ok(to.clone())
    })();
    vm.unpin_many(to_pin);
    result
}

fn object_is(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let a = args.first().cloned().unwrap_or(Value::Undefined);
    let b = args.get(1).cloned().unwrap_or(Value::Undefined);
    // Object.is: SameValue (distinguishes -0/+0 and treats NaN as equal)
    let same = match (&a, &b) {
        (Value::Number(x), Value::Number(y)) => {
            if x.is_nan() && y.is_nan() {
                true
            } else if *x == 0.0 && *y == 0.0 {
                x.is_sign_negative() == y.is_sign_negative()
            } else {
                x == y
            }
        }
        _ => vm.strict_eq(&a, &b),
    };
    Ok(Value::Bool(same))
}
fn object_from_entries(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let entries = args.first().cloned().unwrap_or(Value::Undefined);
    if entries.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let realm = vm.current_realm_global_env();
    let prototype = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.object_proto.clone());
    let allocation_pins = vm.pin_many(&[entries.clone(), prototype.clone()]);
    let allocation = vm.alloc(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }));
    vm.unpin_many(allocation_pins);
    let obj_idx = allocation?;
    let object = Value::Object(obj_idx);
    let object_pin = vm.pin(&object);
    let result = (|| {
        // Accept an array (or array-like) of [key, value] pairs.
        if let Value::Object(arr_idx) = &entries {
            let pairs: Vec<Value> = vm.heap.with_obj(arr_idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    a.items.lock().clone()
                } else {
                    Vec::new()
                }
            });
            for pair in &pairs {
                // Each entry object is read through Get(entry, "0") / Get(entry, "1").
                if !matches!(pair, Value::Object(_)) {
                    return Err(Error::type_err("Iterator value is not an entry object"));
                }
                let mut entry_pins = 0;
                let entry_result: error::Result<()> = (|| {
                    let key = vm.get_property_by_key(pair, &PropertyKey::from("0"))?;
                    entry_pins += vm.pin(&key);
                    let value = vm.get_property_by_key(pair, &PropertyKey::from("1"))?;
                    entry_pins += vm.pin(&value);
                    let key = to_property_key_descriptor(vm, &key)?;
                    vm.heap.with_obj(obj_idx.0, |o| {
                        if let HeapObj::Object(obj) = o {
                            // Own enumerable data property (data_prop is
                            // non-enumerable, which would hide it from
                            // Object.keys / JSON.stringify).
                            obj.props.lock().insert(
                                key,
                                PropertyDescriptor {
                                    value,
                                    writable: true,
                                    enumerable: true,
                                    configurable: true,
                                    get: None,
                                    set: None,
                                    is_accessor: false,
                                },
                            );
                        }
                    });
                    Ok(())
                })();
                vm.unpin_many(entry_pins);
                entry_result?;
            }
        }
        Ok(object)
    })();
    vm.unpin_many(object_pin);
    result
}
fn object_create(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let proto = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(proto, Value::Object(_) | Value::Null) {
        return Err(Error::type_err(
            "Object prototype may only be an Object or null",
        ));
    }
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(if proto.is_null() { None } else { Some(proto) }),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let obj = Value::Object(GcIdx(obj_idx));
    if let Some(props) = args.get(1) {
        if !props.is_undefined() {
            object_define_properties(vm, &[obj.clone(), props.clone()], None)?;
        }
    }
    Ok(obj)
}
fn object_get_own_property_names(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let keys: Vec<Arc<str>> = own_property_keys_or_throw(vm, &obj, false, true, false)?
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::Str(s) => Some(s),
            PropertyKey::Symbol(_) => None,
        })
        .collect();
    let realm = vm.current_realm_global_env();
    create_array_from_values_in_realm(vm, keys.into_iter().map(Value::String).collect(), realm)
}

fn object_get_own_property_symbols(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let symbols: Vec<Value> = own_property_keys_or_throw(vm, &obj, false, false, true)?
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::Symbol(id) => Some(Value::Symbol(id)),
            PropertyKey::Str(_) => None,
        })
        .collect();
    let realm = vm.current_realm_global_env();
    create_array_from_values_in_realm(vm, symbols, realm)
}

fn object_get_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if obj.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&obj)?;
    Ok(vm.get_prototype_of(&object)?.unwrap_or(Value::Null))
}

pub(crate) fn object_set_prototype_of(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let proto = args.get(1).cloned().unwrap_or(Value::Undefined);
    if obj.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let p = prototype_arg(proto)?;
    if matches!(obj, Value::Object(_)) && !vm.set_prototype_of(&obj, p)? {
        return Err(Error::type_err("Cannot mutate object prototype"));
    }
    Ok(obj)
}

pub(crate) fn reflect_set_prototype_of_result(vm: &mut Vm, args: &[Value]) -> error::Result<bool> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(obj, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.setPrototypeOf target must be an object",
        ));
    }
    let proto = args.get(1).cloned().unwrap_or(Value::Undefined);
    vm.set_prototype_of(&obj, prototype_arg(proto)?)
}

fn prototype_arg(proto: Value) -> error::Result<Option<Value>> {
    if proto.is_null() {
        Ok(None)
    } else if matches!(proto, Value::Object(_)) {
        Ok(Some(proto))
    } else {
        Err(Error::type_err(
            "Object prototype may only be an Object or null",
        ))
    }
}

fn object_proto_get(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&this)?;
    Ok(vm.get_prototype_of(&object)?.unwrap_or(Value::Null))
}

fn object_proto_set(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let proto = args.first().cloned().unwrap_or(Value::Undefined);
    let p = match proto {
        Value::Object(_) => Some(proto),
        Value::Null => None,
        _ => return Ok(Value::Undefined),
    };
    if !matches!(this, Value::Object(_)) {
        return Ok(Value::Undefined);
    }
    if !vm.set_prototype_of(&this, p)? {
        return Err(Error::type_err("Cannot mutate object prototype"));
    }
    Ok(Value::Undefined)
}

pub(crate) fn reflect_get_prototype_of_strict(vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(obj, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.getPrototypeOf target must be an object",
        ));
    }
    object_get_prototype_of(vm, args, None)
}

fn object_prevent_extensions(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if matches!(obj, Value::Object(_)) && !vm.prevent_extensions(&obj)? {
        return Err(Error::type_err(
            "Object.preventExtensions failed to prevent extensions",
        ));
    }
    Ok(obj)
}

fn descriptor_is_frozen(desc: &PropertyDescriptor) -> bool {
    !desc.configurable && (desc.is_accessor || !desc.writable)
}

fn array_length(array: &ArrayData) -> usize {
    let dense_len = array.items.lock().len();
    let sparse_len = array.sparse_max.lock().unwrap_or(0);
    dense_len.max(sparse_len)
}

fn materialize_array_index_descriptors(array: &ArrayData, freeze: bool) {
    let items = array.items.lock();
    let present = array.present.lock();
    let mut props = array.props.lock();
    let is_arguments = array.is_arguments.load(Ordering::Relaxed);
    for (index, value) in items.iter().enumerate() {
        if !present.get(index).copied().unwrap_or(false) {
            continue;
        }
        let key = PropertyKey::from_string(index.to_string());
        let desc = props
            .entry(key)
            .or_insert_with(|| PropertyDescriptor::data(value.clone()));
        if freeze && !desc.is_accessor {
            desc.writable = false;
            if is_arguments {
                if let Some(map) = array.arguments_map.lock().as_mut() {
                    if let Some(slot) = map.names.get_mut(index) {
                        *slot = None;
                    }
                }
            }
        }
        desc.configurable = false;
    }
    if !is_arguments {
        let length_key = PropertyKey::from("length");
        let sparse_len = array.sparse_max.lock().unwrap_or(0);
        let length = Value::Number(items.len().max(sparse_len) as f64);
        let desc = props.entry(length_key).or_insert_with(|| {
            let mut desc = PropertyDescriptor::data(length);
            desc.enumerable = false;
            desc.configurable = false;
            desc
        });
        if freeze && !desc.is_accessor {
            desc.writable = false;
        }
        desc.configurable = false;
    }
}

fn array_integrity(array: &ArrayData, frozen: bool) -> bool {
    if array.extensible.load(Ordering::Relaxed) {
        return false;
    }
    let length = array_length(array);
    let items = array.items.lock();
    let present = array.present.lock();
    let props = array.props.lock();
    for index in 0..items.len() {
        if !present.get(index).copied().unwrap_or(false) {
            continue;
        }
        let key = PropertyKey::from_string(index.to_string());
        let Some(desc) = props.get(&key) else {
            return false;
        };
        if frozen {
            if !descriptor_is_frozen(desc) {
                return false;
            }
        } else if desc.configurable {
            return false;
        }
    }
    if !array.is_arguments.load(Ordering::Relaxed) {
        let length_key = PropertyKey::from("length");
        if let Some(desc) = props.get(&length_key) {
            if desc.configurable {
                return false;
            }
            if desc.value != Value::Number(length as f64) {
                return false;
            }
        }
    }
    let is_arguments = array.is_arguments.load(Ordering::Relaxed);
    props.iter().all(|(key, desc)| {
        if !is_arguments && key.as_str() == Some("length") {
            return true;
        }
        if frozen {
            descriptor_is_frozen(desc)
        } else {
            !desc.configurable
        }
    })
}

fn is_proxy_value(vm: &Vm, obj: &Value) -> bool {
    matches!(obj, Value::Object(idx) if vm.heap.with_obj(idx.0, |o| matches!(o, HeapObj::Proxy(_))))
}

fn uses_specialized_integrity_path(vm: &Vm, obj: &Value) -> bool {
    matches!(
        obj,
        Value::Object(idx)
            if vm.heap.with_obj(idx.0, |object| matches!(
                object,
                HeapObj::Object(_)
                    | HeapObj::Array(_)
                    | HeapObj::Function(_)
                    | HeapObj::IteratorHelper(_)
            ))
    )
}

#[cfg(test)]
fn take_proxy_own_keys_reservation_failure(
    vm: &mut Vm,
    site: crate::vm::ProxyOwnKeysReservationSite,
) -> bool {
    let Some((configured_site, remaining)) = vm.fail_proxy_own_keys_reservation else {
        return false;
    };
    if configured_site != site {
        return false;
    }
    if remaining != 0 {
        vm.fail_proxy_own_keys_reservation = Some((configured_site, remaining - 1));
        return false;
    }
    vm.fail_proxy_own_keys_reservation = None;
    true
}

fn reserve_proxy_own_keys_trap_result_key(
    _vm: &mut Vm,
    keys: &mut Vec<PropertyKey>,
) -> error::Result<()> {
    #[cfg(test)]
    if take_proxy_own_keys_reservation_failure(
        _vm,
        crate::vm::ProxyOwnKeysReservationSite::TrapResultKey,
    ) {
        return Err(Error::range("Proxy ownKeys trap result is too large"));
    }
    keys.try_reserve(1)
        .map_err(|_| Error::range("Proxy ownKeys trap result is too large"))
}

fn reserve_proxy_own_keys_seen_key(
    _vm: &mut Vm,
    seen: &mut IndexSet<PropertyKey>,
) -> error::Result<()> {
    #[cfg(test)]
    if take_proxy_own_keys_reservation_failure(_vm, crate::vm::ProxyOwnKeysReservationSite::SeenKey)
    {
        return Err(Error::range("Proxy ownKeys duplicate set is too large"));
    }
    seen.try_reserve(1)
        .map_err(|_| Error::range("Proxy ownKeys duplicate set is too large"))
}

fn proxy_own_keys_from_array_like(
    vm: &mut Vm,
    key_list: &Value,
) -> error::Result<Vec<PropertyKey>> {
    const MAX_SAFE_LENGTH: f64 = 9_007_199_254_740_991.0;

    if !matches!(key_list, Value::Object(_)) {
        return Err(Error::type_err(
            "Proxy ownKeys trap result must be an object",
        ));
    }
    let list_pin = vm.pin(key_list);
    let result = (|| -> error::Result<Vec<PropertyKey>> {
        let length_value = vm.get_property(key_list, "length")?;
        let length_pin = vm.pin(&length_value);
        let length_result = vm.to_number(&length_value);
        vm.unpin_many(length_pin);
        let length_number = length_result?;
        let length = if length_number.is_nan() || length_number <= 0.0 {
            0
        } else {
            length_number.trunc().min(MAX_SAFE_LENGTH) as usize
        };
        let mut keys = Vec::new();
        for index in 0..length {
            vm.consume_fuel()?;
            let item = vm.get_property(key_list, &index.to_string())?;
            let key = match item {
                Value::String(value) => PropertyKey::Str(value),
                Value::Symbol(id) => PropertyKey::Symbol(id),
                _ => {
                    return Err(Error::type_err(
                        "Proxy ownKeys trap entries must be strings or symbols",
                    ));
                }
            };
            reserve_proxy_own_keys_trap_result_key(vm, &mut keys)?;
            keys.push(key);
        }
        Ok(keys)
    })();
    vm.unpin_many(list_pin);
    result
}

pub(crate) fn own_property_keys_or_throw(
    vm: &mut Vm,
    obj: &Value,
    enumerable_only: bool,
    include_strings: bool,
    include_symbols: bool,
) -> error::Result<Vec<PropertyKey>> {
    struct PendingProxyKeys {
        object: Value,
        target: Value,
        trap_keys: Vec<PropertyKey>,
        seen: IndexSet<PropertyKey>,
        extensible_target: bool,
        enumerable_only: bool,
        include_strings: bool,
        include_symbols: bool,
    }

    enum ProxyKeysStep {
        Forward(Value),
        Validate {
            target: Value,
            trap_keys: Vec<PropertyKey>,
            seen: IndexSet<PropertyKey>,
            extensible_target: bool,
        },
    }

    let root_pin = vm.pin(obj);
    let mut pending = Vec::new();
    let mut pending_pins = 0;
    let mut current = obj.clone();
    let mut current_filters = (enumerable_only, include_strings, include_symbols);
    let result = (|| {
        let mut keys = loop {
            let proxy_result = match &current {
                Value::Object(idx) => vm.heap.with_obj(idx.0, |heap_obj| {
                    if let HeapObj::Proxy(proxy) = heap_obj {
                        if *proxy.revoked.lock() {
                            return Some(Err(Error::type_err(
                                "Cannot perform 'ownKeys' on a proxy that has been revoked",
                            )));
                        }
                        Some(Ok((proxy.target.clone(), proxy.handler.clone())))
                    } else {
                        None
                    }
                }),
                _ => None,
            };
            let Some(proxy_result) = proxy_result else {
                break ordinary_own_property_keys(
                    vm,
                    &current,
                    current_filters.0,
                    current_filters.1,
                    current_filters.2,
                )?;
            };

            let (target, handler) = proxy_result?;
            vm.consume_fuel()?;
            let proxy_pins = vm.pin_many(&[target.clone(), handler.clone()]);
            let step = (|| {
                let trap = vm.get_proxy_method(&handler, "ownKeys")?;
                if trap.is_nullish() {
                    return Ok(ProxyKeysStep::Forward(target.clone()));
                }
                let key_list =
                    vm.call_function(&trap, std::slice::from_ref(&target), Some(handler.clone()))?;
                let trap_keys = proxy_own_keys_from_array_like(vm, &key_list)?;
                let mut seen = IndexSet::new();
                for key in &trap_keys {
                    if seen.contains(key) {
                        return Err(Error::type_err(
                            "Proxy ownKeys trap returned duplicate entries",
                        ));
                    }
                    reserve_proxy_own_keys_seen_key(vm, &mut seen)?;
                    seen.insert(key.clone());
                }
                let extensible_target = vm.is_extensible(&target)?;
                Ok(ProxyKeysStep::Validate {
                    target: target.clone(),
                    trap_keys,
                    seen,
                    extensible_target,
                })
            })();
            vm.unpin_many(proxy_pins);

            match step? {
                ProxyKeysStep::Forward(target) => current = target,
                ProxyKeysStep::Validate {
                    target,
                    trap_keys,
                    seen,
                    extensible_target,
                } => {
                    pending_pins += vm.pin_many(&[current.clone(), target.clone()]);
                    pending.push(PendingProxyKeys {
                        object: current,
                        target: target.clone(),
                        trap_keys,
                        seen,
                        extensible_target,
                        enumerable_only: current_filters.0,
                        include_strings: current_filters.1,
                        include_symbols: current_filters.2,
                    });
                    current = target;
                    current_filters = (false, true, true);
                }
            }
        };

        while let Some(frame) = pending.pop() {
            let mut omitted_non_configurable = false;
            for target_key in &keys {
                vm.consume_fuel()?;
                let descriptor =
                    own_property_descriptor_for_key_or_throw(vm, &frame.target, target_key)?;
                if descriptor.is_some_and(|descriptor| !descriptor.configurable)
                    && !frame.seen.contains(target_key)
                {
                    omitted_non_configurable = true;
                }
            }
            if omitted_non_configurable {
                return Err(Error::type_err(
                    "Proxy ownKeys trap omitted a non-configurable key",
                ));
            }
            if !frame.extensible_target {
                let target_key_set: IndexSet<_> = keys.iter().cloned().collect();
                if target_key_set != frame.seen {
                    return Err(Error::type_err(
                        "Proxy ownKeys trap does not match a non-extensible target",
                    ));
                }
            }

            let mut filtered = Vec::new();
            for key in frame.trap_keys {
                vm.consume_fuel()?;
                let included = matches!(&key, PropertyKey::Str(_) if frame.include_strings)
                    || matches!(&key, PropertyKey::Symbol(_) if frame.include_symbols);
                if !included {
                    continue;
                }
                if frame.enumerable_only
                    && !own_property_descriptor_for_key_or_throw(vm, &frame.object, &key)?
                        .is_some_and(|desc| desc.enumerable)
                {
                    continue;
                }
                filtered.push(key);
            }
            keys = filtered;
        }
        Ok(keys)
    })();
    vm.unpin_many(pending_pins);
    vm.unpin(root_pin);
    result
}

struct IntegrityDescriptor {
    configurable: bool,
    writable: Option<bool>,
}

fn integrity_descriptor_from_object(
    vm: &mut Vm,
    desc_obj: &Value,
) -> error::Result<IntegrityDescriptor> {
    let configurable = if vm.has_own(desc_obj, "configurable") {
        vm.get_property(desc_obj, "configurable")?.is_truthy()
    } else {
        false
    };
    let writable = if vm.has_own(desc_obj, "writable") {
        Some(vm.get_property(desc_obj, "writable")?.is_truthy())
    } else {
        None
    };
    Ok(IntegrityDescriptor {
        configurable,
        writable,
    })
}

fn integrity_descriptor_for_key(
    vm: &mut Vm,
    obj: &Value,
    key: &PropertyKey,
) -> error::Result<Option<IntegrityDescriptor>> {
    if is_proxy_value(vm, obj) {
        let key_value = property_key_to_value(key);
        let desc = object_get_own_property_descriptor(vm, &[obj.clone(), key_value], None)?;
        if desc.is_undefined() {
            return Ok(None);
        }
        return integrity_descriptor_from_object(vm, &desc).map(Some);
    }
    Ok(
        own_property_descriptor_for_key(vm, obj, key).map(|desc| IntegrityDescriptor {
            configurable: desc.configurable,
            writable: if desc.is_accessor {
                None
            } else {
                Some(desc.writable)
            },
        }),
    )
}

fn integrity_define_descriptor(vm: &mut Vm, writable: Option<bool>) -> error::Result<Value> {
    let desc_obj = vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let mut props = IndexMap::new();
    props.insert(
        PropertyKey::from("configurable"),
        PropertyDescriptor::data(Value::Bool(false)),
    );
    if let Some(writable) = writable {
        props.insert(
            PropertyKey::from("writable"),
            PropertyDescriptor::data(Value::Bool(writable)),
        );
    }
    vm.heap.with_obj(desc_obj.0, |o| {
        if let HeapObj::Object(od) = o {
            *od.props.lock() = props;
        }
    });
    Ok(Value::Object(desc_obj))
}

fn set_integrity_level(vm: &mut Vm, obj: &Value, frozen: bool) -> error::Result<bool> {
    let operation_pin = vm.pin(obj);
    let result = (|| {
        if !vm.prevent_extensions(obj)? {
            return Ok(false);
        }
        let keys = own_property_keys_or_throw(vm, obj, false, true, true)?;
        for key in keys {
            let writable = if frozen {
                let Some(desc) = integrity_descriptor_for_key(vm, obj, &key)? else {
                    continue;
                };
                desc.writable.map(|_| false)
            } else {
                None
            };
            let desc_obj = integrity_define_descriptor(vm, writable)?;
            let key_value = property_key_to_value(&key);
            if !object_define_property_result(vm, &[obj.clone(), key_value, desc_obj], false)? {
                return Ok(false);
            }
        }
        Ok(true)
    })();
    vm.unpin_many(operation_pin);
    result
}

fn test_integrity_level(vm: &mut Vm, obj: &Value, frozen: bool) -> error::Result<bool> {
    let operation_pin = vm.pin(obj);
    let result = (|| {
        if vm.is_extensible(obj)? {
            return Ok(false);
        }
        let keys = own_property_keys_or_throw(vm, obj, false, true, true)?;
        for key in keys {
            let Some(desc) = integrity_descriptor_for_key(vm, obj, &key)? else {
                continue;
            };
            if desc.configurable {
                return Ok(false);
            }
            if frozen && desc.writable.unwrap_or(false) {
                return Ok(false);
            }
        }
        Ok(true)
    })();
    vm.unpin_many(operation_pin);
    result
}

fn object_is_extensible(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    vm.is_extensible(&obj).map(Value::Bool)
}

fn object_seal(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if matches!(obj, Value::Object(_)) && !uses_specialized_integrity_path(vm, &obj) {
        if !set_integrity_level(vm, &obj, false)? {
            return Err(Error::type_err("Object.seal failed to seal object"));
        }
        return Ok(obj);
    }
    if matches!(obj, Value::Object(_)) && !vm.prevent_extensions(&obj)? {
        return Err(Error::type_err("Object.seal failed to prevent extensions"));
    }
    if let Value::Object(idx) = &obj {
        vm.heap.with_obj(idx.0, |o| match o {
            HeapObj::Object(od) => {
                for d in od.props.lock().values_mut() {
                    d.configurable = false;
                }
            }
            HeapObj::Array(a) => {
                materialize_array_index_descriptors(a, false);
                for d in a.props.lock().values_mut() {
                    d.configurable = false;
                }
            }
            HeapObj::Function(f) => {
                for d in f.props.lock().values_mut() {
                    d.configurable = false;
                }
            }
            HeapObj::IteratorHelper(helper) => {
                for d in helper.props.lock().values_mut() {
                    d.configurable = false;
                }
            }
            _ => {}
        });
    }
    Ok(obj)
}

fn object_is_sealed(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if matches!(obj, Value::Object(_)) && !uses_specialized_integrity_path(vm, &obj) {
        return test_integrity_level(vm, &obj, false).map(Value::Bool);
    }
    if let Value::Object(idx) = &obj {
        let sealed = vm.heap.with_obj(idx.0, |o| match o {
            HeapObj::Object(od) => {
                !od.extensible.load(Ordering::Relaxed)
                    && od.props.lock().values().all(|d| !d.configurable)
            }
            HeapObj::Array(a) => array_integrity(a, false),
            HeapObj::Function(f) => {
                !f.extensible.load(Ordering::Relaxed)
                    && f.props.lock().values().all(|d| !d.configurable)
            }
            HeapObj::IteratorHelper(helper) => {
                !helper.extensible.load(Ordering::Relaxed)
                    && helper.props.lock().values().all(|d| !d.configurable)
            }
            _ => !o.is_extensible(),
        });
        return Ok(Value::Bool(sealed));
    }
    Ok(Value::Bool(true))
}

fn object_is_frozen(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if matches!(obj, Value::Object(_)) && !uses_specialized_integrity_path(vm, &obj) {
        return test_integrity_level(vm, &obj, true).map(Value::Bool);
    }
    if let Value::Object(idx) = &obj {
        let frozen = vm.heap.with_obj(idx.0, |o| match o {
            HeapObj::Object(od) => {
                let ext = od.extensible.load(Ordering::Relaxed);
                let all_frozen = od.props.lock().values().all(descriptor_is_frozen);
                !ext && all_frozen
            }
            HeapObj::Array(a) => array_integrity(a, true),
            HeapObj::Function(f) => {
                !f.extensible.load(Ordering::Relaxed)
                    && f.props.lock().values().all(descriptor_is_frozen)
            }
            HeapObj::IteratorHelper(helper) => {
                !helper.extensible.load(Ordering::Relaxed)
                    && helper.props.lock().values().all(descriptor_is_frozen)
            }
            HeapObj::ModuleNamespace(namespace) => namespace.exports.lock().is_empty(),
            _ => !o.is_extensible(),
        });
        return Ok(Value::Bool(frozen));
    }
    Ok(Value::Bool(true))
}

fn object_get_own_property_descriptors(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let realm = vm.current_realm_global_env();
    let prototype = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.object_proto.clone());
    let allocation_pins = vm.pin_many(&[obj.clone(), prototype.clone()]);
    let allocation = vm.alloc(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }));
    let result_idx = match allocation {
        Ok(result_idx) => result_idx,
        Err(error) => {
            vm.unpin_many(allocation_pins);
            return Err(error);
        }
    };
    let result_value = Value::Object(result_idx);
    let result_pin = vm.pin(&result_value);
    let mut descriptor_pins = 0;
    let result = (|| {
        let keys = own_property_keys_or_throw(vm, &obj, false, true, true)?;
        let mut props = IndexMap::new();
        for key in keys {
            if let Some(desc) = own_property_descriptor_for_key_or_throw(vm, &obj, &key)? {
                let descriptor = from_property_descriptor(vm, desc)?;
                descriptor_pins += vm.pin(&descriptor);
                props.insert(key, PropertyDescriptor::data(descriptor));
            }
        }
        vm.heap.with_obj(result_idx.0, |o| {
            if let HeapObj::Object(od) = o {
                *od.props.lock() = props;
            }
        });
        Ok(result_value)
    })();
    vm.unpin_many(descriptor_pins + result_pin + allocation_pins);
    result
}

fn normalize_property_descriptor_object(vm: &mut Vm, desc: &Value) -> error::Result<Value> {
    if !matches!(desc, Value::Object(_)) {
        return Err(Error::type_err("Property description must be an object"));
    }
    let realm = vm
        .native_callee_closure()
        .map(|closure| env::global_env_root(&vm.heap, closure))
        .unwrap_or(vm.global);
    let prototype = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Object prototype intrinsic"))?;
    let base_pins = vm.pin_many(&[desc.clone(), prototype.clone()]);
    let normalized_idx = match vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    })) {
        Ok(idx) => idx,
        Err(error) => {
            vm.unpin_many(base_pins);
            return Err(error);
        }
    };
    let normalized = Value::Object(normalized_idx);
    let normalized_pin = vm.pin(&normalized);
    let result = (|| -> error::Result<()> {
        let mut has_data = false;
        let mut has_accessor = false;
        for name in [
            "enumerable",
            "configurable",
            "value",
            "writable",
            "get",
            "set",
        ] {
            let key = PropertyKey::from(name);
            if !vm.has_property_with_free_ordinary_edge(desc, name)? {
                continue;
            }
            let mut value = vm.get_property_by_key(desc, &key)?;
            match name {
                "enumerable" | "configurable" | "writable" => {
                    value = Value::Bool(vm.to_boolean(&value));
                }
                "value" => has_data = true,
                "get" | "set" => {
                    has_accessor = true;
                    if !value.is_undefined() && !is_callable(&value, &vm.heap) {
                        return Err(Error::type_err(if name == "get" {
                            "Getter must be a function"
                        } else {
                            "Setter must be a function"
                        }));
                    }
                }
                _ => unreachable!(),
            }
            if name == "writable" {
                has_data = true;
            }
            vm.heap.with_obj(normalized_idx.0, |object| {
                object
                    .props()
                    .lock()
                    .insert(key, PropertyDescriptor::data(value));
            });
        }
        if has_data && has_accessor {
            return Err(Error::type_err(
                "Invalid property descriptor. Cannot both specify accessors and a value or writable attribute",
            ));
        }
        let order: &[&str] = if has_data {
            &["value", "writable", "enumerable", "configurable"]
        } else if has_accessor {
            &["get", "set", "enumerable", "configurable"]
        } else {
            &["enumerable", "configurable"]
        };
        vm.heap.with_obj(normalized_idx.0, |object| {
            let props = object.props();
            let mut props = props.lock();
            let mut ordered = IndexMap::new();
            for name in order {
                let key = PropertyKey::from(*name);
                if let Some(descriptor) = props.get(&key).cloned() {
                    ordered.insert(key, descriptor);
                }
            }
            *props = ordered;
        });
        Ok(())
    })();
    vm.unpin_many(normalized_pin + base_pins);
    result.map(|_| normalized)
}

fn descriptor_same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            (left.is_nan() && right.is_nan()) || left.to_bits() == right.to_bits()
        }
        _ => left == right,
    }
}

fn object_define_properties(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Object.defineProperties target must be an object",
        ));
    }
    let properties = vm.to_object(&args.get(1).cloned().unwrap_or(Value::Undefined))?;
    let base_pins = vm.pin_many(&[target.clone(), properties.clone()]);
    let mut descriptor_pins = 0;
    let result = (|| -> error::Result<Value> {
        let keys = own_property_keys_or_throw(vm, &properties, false, true, true)?;
        let mut descriptors = Vec::new();
        for key in keys {
            if !own_property_descriptor_for_key_or_throw(vm, &properties, &key)?
                .is_some_and(|descriptor| descriptor.enumerable)
            {
                continue;
            }
            let descriptor_object = vm.get_property_by_key(&properties, &key)?;
            let descriptor = normalize_property_descriptor_object(vm, &descriptor_object)?;
            descriptor_pins += vm.pin(&descriptor);
            descriptors.push((key, descriptor));
        }
        for (key, descriptor) in descriptors {
            object_define_property_result(
                vm,
                &[target.clone(), property_key_to_value(&key), descriptor],
                true,
            )?;
        }
        Ok(target.clone())
    })();
    vm.unpin_many(descriptor_pins);
    vm.unpin_many(base_pins);
    result
}

pub(crate) fn canonical_string_index(key: &PropertyKey) -> Option<usize> {
    canonical_string_index_name(key.as_str()?)
}

pub(crate) fn canonical_string_index_name(name: &str) -> Option<usize> {
    let index = name.parse::<usize>().ok()?;
    if index.to_string() == name {
        Some(index)
    } else {
        None
    }
}

fn string_exotic_own_property_descriptor(s: &str, key: &PropertyKey) -> Option<PropertyDescriptor> {
    if key.as_str() == Some("length") {
        let mut desc = PropertyDescriptor::data(Value::Number(crate::value::utf16_len(s) as f64));
        desc.writable = false;
        desc.enumerable = false;
        desc.configurable = false;
        return Some(desc);
    }

    let index = canonical_string_index(key)?;
    let unit = crate::value::utf16_get(s, index)?;
    let mut desc = PropertyDescriptor::data(Value::String(Arc::from(
        crate::value::utf16_to_string(&[unit]).as_str(),
    )));
    desc.writable = false;
    desc.enumerable = true;
    desc.configurable = false;
    Some(desc)
}

fn own_property_descriptor_for_key(
    vm: &mut Vm,
    obj: &Value,
    key: &PropertyKey,
) -> Option<PropertyDescriptor> {
    if let Value::Object(idx) = obj {
        if let Some(target) = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Proxy(proxy) = o {
                if *proxy.revoked.lock() {
                    None
                } else {
                    Some(proxy.target.clone())
                }
            } else {
                None
            }
        }) {
            return own_property_descriptor_for_key(vm, &target, key);
        }

        if let Some(desc) = vm.typed_array_integer_index_own_property_descriptor(obj, key) {
            return desc;
        }

        let array_descriptor = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                if key.as_str() == Some("length") {
                    if let Some(desc) = a.props.lock().get(key).cloned() {
                        return Some(desc);
                    }
                    if a.is_arguments.load(Ordering::Relaxed) {
                        return None;
                    }
                    let length = a.items.lock().len().max(a.sparse_max.lock().unwrap_or(0));
                    let mut desc = PropertyDescriptor::data(Value::Number(length as f64));
                    desc.writable = true;
                    desc.enumerable = false;
                    desc.configurable = false;
                    return Some(desc);
                }
            }
            None
        });
        if let Some(desc) = array_descriptor {
            return Some(desc);
        }
        let namespace_binding = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ModuleNamespace(namespace) = o {
                return key
                    .as_str()
                    .and_then(|name| namespace.exports.lock().get(name).cloned());
            }
            None
        });
        if let Some((env, name)) = namespace_binding {
            let value = crate::environment::get_checked(&vm.heap, env, &name)
                .ok()
                .flatten()
                .unwrap_or(Value::Undefined);
            let mut desc = PropertyDescriptor::data(value);
            desc.writable = true;
            desc.enumerable = true;
            desc.configurable = false;
            return Some(desc);
        }
        let is_array = vm.heap.with_obj(idx.0, |o| matches!(o, HeapObj::Array(_)));
        if is_array {
            if let Some(i) = canonical_string_index(key) {
                return vm.array_index_own_property_descriptor(idx.0, i, key);
            }
        }
    }

    match obj {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            let ordinary = o.props().lock().get(key).cloned();
            if ordinary.is_some() {
                return ordinary;
            }

            if let HeapObj::Object(od) = o {
                if let Some(Value::String(s)) = od.primitive.lock().clone() {
                    return string_exotic_own_property_descriptor(&s, key);
                }
            }

            None
        }),
        Value::String(s) => string_exotic_own_property_descriptor(s, key),
        _ => None,
    }
}

fn property_descriptor_from_object(vm: &mut Vm, desc: &Value) -> error::Result<PropertyDescriptor> {
    if !matches!(desc, Value::Object(_)) {
        return Err(Error::type_err(
            "Proxy getOwnPropertyDescriptor trap must return an object or undefined",
        ));
    }

    let mut pin_count = vm.pin(desc);
    let result = (|| {
        let mut value = Value::Undefined;
        let mut writable = false;
        let mut enumerable = false;
        let mut configurable = false;
        let mut get = None;
        let mut set = None;
        let mut has_value = false;
        let mut has_writable = false;
        let mut has_get = false;
        let mut has_set = false;

        if vm.has_property_with_free_ordinary_edge(desc, "enumerable")? {
            enumerable = vm.get_property(desc, "enumerable")?.is_truthy();
        }
        if vm.has_property_with_free_ordinary_edge(desc, "configurable")? {
            configurable = vm.get_property(desc, "configurable")?.is_truthy();
        }
        if vm.has_property_with_free_ordinary_edge(desc, "value")? {
            value = vm.get_property(desc, "value")?;
            pin_count += vm.pin(&value);
            has_value = true;
        }
        if vm.has_property_with_free_ordinary_edge(desc, "writable")? {
            writable = vm.get_property(desc, "writable")?.is_truthy();
            has_writable = true;
        }
        if vm.has_property_with_free_ordinary_edge(desc, "get")? {
            let getter = vm.get_property(desc, "get")?;
            pin_count += vm.pin(&getter);
            if !getter.is_undefined() && !is_callable(&getter, &vm.heap) {
                return Err(Error::type_err("Getter must be a function"));
            }
            get = if getter.is_undefined() {
                None
            } else {
                Some(getter)
            };
            has_get = true;
        }
        if vm.has_property_with_free_ordinary_edge(desc, "set")? {
            let setter = vm.get_property(desc, "set")?;
            pin_count += vm.pin(&setter);
            if !setter.is_undefined() && !is_callable(&setter, &vm.heap) {
                return Err(Error::type_err("Setter must be a function"));
            }
            set = if setter.is_undefined() {
                None
            } else {
                Some(setter)
            };
            has_set = true;
        }

        let is_accessor = has_get || has_set;
        let is_data = has_value || has_writable;
        if is_accessor && is_data {
            return Err(Error::type_err(
                "Invalid property descriptor. Cannot both specify accessors and a value or writable attribute",
            ));
        }

        Ok(if is_accessor {
            PropertyDescriptor {
                value: Value::Undefined,
                writable: false,
                enumerable,
                configurable,
                get,
                set,
                is_accessor: true,
            }
        } else {
            PropertyDescriptor {
                value,
                writable,
                enumerable,
                configurable,
                get: None,
                set: None,
                is_accessor: false,
            }
        })
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn own_property_descriptor_for_key_or_throw(
    vm: &mut Vm,
    obj: &Value,
    key: &PropertyKey,
) -> error::Result<Option<PropertyDescriptor>> {
    let root_pin = vm.pin(obj);
    let mut current = obj.clone();
    let mut pending = Vec::new();
    let mut pending_pin_count = 0;
    let result = (|| {
        let mut target_desc = loop {
            let Value::Object(idx) = &current else {
                break own_property_descriptor_for_key(vm, &current, key);
            };
            let namespace_binding = vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::ModuleNamespace(namespace) = o {
                    return key
                        .as_str()
                        .and_then(|name| namespace.exports.lock().get(name).cloned());
                }
                None
            });
            if let Some((env, name)) = namespace_binding {
                let value = match crate::environment::get_checked(&vm.heap, env, &name) {
                    Ok(Some(value)) => value,
                    Ok(None) | Err(false) => Value::Undefined,
                    Err(true) => {
                        return Err(Error::reference(format!(
                            "Cannot access '{}' before initialization",
                            name
                        )))
                    }
                };
                let mut desc = PropertyDescriptor::data(value);
                desc.writable = true;
                desc.enumerable = true;
                desc.configurable = false;
                break Some(desc);
            }
            let proxy_result = vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Proxy(proxy) = o {
                    if *proxy.revoked.lock() {
                        return Some(Err(Error::type_err(
                            "Cannot perform 'getOwnPropertyDescriptor' on a proxy that has been revoked",
                        )));
                    }
                    Some(Ok((proxy.target.clone(), proxy.handler.clone())))
                } else {
                    None
                }
            });
            let Some(proxy_result) = proxy_result else {
                break own_property_descriptor_for_key(vm, &current, key);
            };
            let (target, handler) = proxy_result?;
            vm.consume_fuel()?;
            let proxy_pins = vm.pin_many(&[target.clone(), handler.clone()]);
            let trap = match vm.get_proxy_method(&handler, "getOwnPropertyDescriptor") {
                Ok(trap) => trap,
                Err(error) => {
                    vm.unpin_many(proxy_pins);
                    return Err(error);
                }
            };
            if trap.is_nullish() {
                vm.unpin_many(proxy_pins);
                current = target;
                continue;
            }
            let key_value = property_key_to_value(key);
            let trap_pin = vm.pin(&trap);
            let trap_result = vm.call_function(&trap, &[target.clone(), key_value], Some(handler));
            vm.unpin(trap_pin);
            let trap_result = match trap_result {
                Ok(result) => result,
                Err(error) => {
                    vm.unpin_many(proxy_pins);
                    return Err(error);
                }
            };
            if !trap_result.is_undefined() && !matches!(trap_result, Value::Object(_)) {
                vm.unpin_many(proxy_pins);
                return Err(Error::type_err(
                    "Proxy getOwnPropertyDescriptor trap must return an object or undefined",
                ));
            }
            vm.unpin_many(proxy_pins);
            pending_pin_count += vm.pin(&target);
            pending_pin_count += vm.pin(&trap_result);
            pending.push((target.clone(), trap_result));
            current = target;
        };

        while let Some((target, trap_result)) = pending.pop() {
            target_desc = validate_proxy_get_own_property_descriptor_result(
                vm,
                &target,
                &trap_result,
                target_desc,
            )?;
        }
        Ok(target_desc)
    })();
    vm.unpin_many(pending_pin_count);
    vm.unpin(root_pin);
    result
}

fn validate_proxy_get_own_property_descriptor_result(
    vm: &mut Vm,
    target: &Value,
    result: &Value,
    target_desc: Option<PropertyDescriptor>,
) -> error::Result<Option<PropertyDescriptor>> {
    let mut descriptor_roots = Vec::new();
    if let Some(target_desc) = target_desc.as_ref() {
        descriptor_roots.push(target_desc.value.clone());
        descriptor_roots.extend(target_desc.get.iter().cloned());
        descriptor_roots.extend(target_desc.set.iter().cloned());
    }
    let descriptor_pins = vm.pin_many(&descriptor_roots);
    let validation = (|| {
        if result.is_undefined() {
            let Some(target_desc) = target_desc.as_ref() else {
                return Ok(None);
            };
            if !target_desc.configurable || !vm.is_extensible(target)? {
                return Err(Error::type_err(
                    "Proxy getOwnPropertyDescriptor trap cannot hide the target property",
                ));
            }
            return Ok(None);
        }
        let extensible_target = vm.is_extensible(target)?;
        let result_desc = property_descriptor_from_object(vm, result)?;
        if !vm.is_compatible_property_descriptor(
            target_desc.as_ref(),
            &result_desc,
            extensible_target,
        ) {
            return Err(Error::type_err(
                "Proxy getOwnPropertyDescriptor trap returned an incompatible descriptor",
            ));
        }
        if !result_desc.configurable {
            let Some(target_desc) = target_desc.as_ref() else {
                return Err(Error::type_err(
                    "Proxy getOwnPropertyDescriptor trap cannot report a new non-configurable property",
                ));
            };
            if target_desc.configurable
                || (!result_desc.is_accessor
                    && !result_desc.writable
                    && !target_desc.is_accessor
                    && target_desc.writable)
            {
                return Err(Error::type_err(
                    "Proxy getOwnPropertyDescriptor trap cannot tighten the target descriptor",
                ));
            }
        }
        Ok(Some(result_desc))
    })();
    vm.unpin_many(descriptor_pins);
    validation
}

fn from_property_descriptor(vm: &mut Vm, desc: PropertyDescriptor) -> error::Result<Value> {
    let realm = vm.current_realm_global_env();
    let prototype = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.object_proto.clone());
    let mut roots = vec![prototype.clone(), desc.value.clone()];
    roots.extend(desc.get.iter().cloned());
    roots.extend(desc.set.iter().cloned());
    let pin_count = vm.pin_many(&roots);
    let mut props = IndexMap::new();
    if desc.is_accessor {
        props.insert(
            PropertyKey::from("get"),
            PropertyDescriptor::data(desc.get.unwrap_or(Value::Undefined)),
        );
        props.insert(
            PropertyKey::from("set"),
            PropertyDescriptor::data(desc.set.unwrap_or(Value::Undefined)),
        );
    } else {
        props.insert(
            PropertyKey::from("value"),
            PropertyDescriptor::data(desc.value),
        );
        props.insert(
            PropertyKey::from("writable"),
            PropertyDescriptor::data(Value::Bool(desc.writable)),
        );
    }
    props.insert(
        PropertyKey::from("enumerable"),
        PropertyDescriptor::data(Value::Bool(desc.enumerable)),
    );
    props.insert(
        PropertyKey::from("configurable"),
        PropertyDescriptor::data(Value::Bool(desc.configurable)),
    );
    let result = vm
        .alloc(HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(props),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

fn object_get_own_property_descriptor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let key = to_property_key_descriptor(vm, args.get(1).unwrap_or(&Value::Undefined))?;
    match own_property_descriptor_for_key_or_throw(vm, &obj, &key)? {
        Some(desc) => from_property_descriptor(vm, desc),
        None => Ok(Value::Undefined),
    }
}

fn object_freeze(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if matches!(target, Value::Object(_)) && !uses_specialized_integrity_path(vm, &target) {
        if !set_integrity_level(vm, &target, true)? {
            return Err(Error::type_err("Object.freeze failed to freeze object"));
        }
        return Ok(target);
    }
    if matches!(target, Value::Object(_)) && !vm.prevent_extensions(&target)? {
        return Err(Error::type_err(
            "Object.freeze failed to prevent extensions",
        ));
    }
    if let Value::Object(idx) = &target {
        let has_namespace_exports = vm.heap.with_obj(idx.0, |object| {
            matches!(object, HeapObj::ModuleNamespace(namespace) if !namespace.exports.lock().is_empty())
        });
        if has_namespace_exports {
            return Err(Error::type_err(
                "Cannot freeze a module namespace with writable exports",
            ));
        }
    }
    if let Value::Object(idx) = &target {
        vm.heap.with_obj(idx.0, |obj| match obj {
            HeapObj::Object(o) => {
                for d in o.props.lock().values_mut() {
                    if !d.is_accessor {
                        d.writable = false;
                    }
                    d.configurable = false;
                }
            }
            HeapObj::Array(a) => {
                materialize_array_index_descriptors(a, true);
                for d in a.props.lock().values_mut() {
                    if !d.is_accessor {
                        d.writable = false;
                    }
                    d.configurable = false;
                }
            }
            HeapObj::Function(f) => {
                for d in f.props.lock().values_mut() {
                    if !d.is_accessor {
                        d.writable = false;
                    }
                    d.configurable = false;
                }
            }
            HeapObj::IteratorHelper(helper) => {
                for d in helper.props.lock().values_mut() {
                    if !d.is_accessor {
                        d.writable = false;
                    }
                    d.configurable = false;
                }
            }
            _ => {}
        });
    }
    Ok(target)
}

fn object_define_property(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    object_define_property_result(vm, args, true)?;
    Ok(target)
}

pub(crate) fn object_define_property_result(
    vm: &mut Vm,
    args: &[Value],
    throw_on_failure: bool,
) -> error::Result<bool> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key_input = args.get(1).cloned().unwrap_or(Value::Undefined);
    let desc_input = args.get(2).cloned().unwrap_or(Value::Undefined);
    let argument_pins = vm.pin_many(&[target.clone(), key_input.clone(), desc_input.clone()]);
    let result = (|| -> error::Result<bool> {
        if !matches!(target, Value::Object(_)) {
            return Err(Error::type_err(
                "Object.defineProperty target must be an object",
            ));
        }
        let key = to_property_key_descriptor(vm, &key_input)?;
        let desc = normalize_property_descriptor_object(vm, &desc_input)?;
        let desc_pin = vm.pin(&desc);
        let define_result = (|| -> error::Result<bool> {
            if let Value::Object(idx) = target {
                let mut value = Value::Undefined;
                let mut writable = false;
                let mut enumerable = false;
                let mut configurable = false;
                let mut get = None;
                let mut set = None;
                let mut has_value = false;
                let mut has_writable = false;
                let mut has_enumerable = false;
                let mut has_configurable = false;
                let mut has_get = false;
                let mut has_set = false;
                if let Value::Object(_) = desc {
                    // Presence of each field is determined by an OWN property on the
                    // descriptor object, mirroring ToPropertyDescriptor: a missing
                    // field must NOT flip the has_* flags, otherwise a plain
                    // `{value: 1, writable: false}` descriptor would be misread as
                    // an accessor (get/set absent but `get_property` returns
                    // `Ok(undefined)`).
                    if vm.has_own(&desc, "value") {
                        value = vm.get_property(&desc, "value")?;
                        has_value = true;
                    }
                    if vm.has_own(&desc, "writable") {
                        writable = vm.get_property(&desc, "writable")?.is_truthy();
                        has_writable = true;
                    }
                    if vm.has_own(&desc, "get") {
                        let v = vm.get_property(&desc, "get")?;
                        if !v.is_undefined() && !is_callable(&v, &vm.heap) {
                            return Err(Error::type_err("Getter must be a function"));
                        }
                        get = if v.is_undefined() { None } else { Some(v) };
                        has_get = true;
                    }
                    if vm.has_own(&desc, "set") {
                        let v = vm.get_property(&desc, "set")?;
                        if !v.is_undefined() && !is_callable(&v, &vm.heap) {
                            return Err(Error::type_err("Setter must be a function"));
                        }
                        set = if v.is_undefined() { None } else { Some(v) };
                        has_set = true;
                    }
                    if vm.has_own(&desc, "enumerable") {
                        enumerable = vm.get_property(&desc, "enumerable")?.is_truthy();
                        has_enumerable = true;
                    }
                    if vm.has_own(&desc, "configurable") {
                        configurable = vm.get_property(&desc, "configurable")?.is_truthy();
                        has_configurable = true;
                    }
                }
                // A descriptor is an accessor descriptor if it has get/set, and a
                // data descriptor if it has value/writable. Mixing the two is a
                // TypeError per [[DefineOwnProperty]].
                let is_accessor = has_get || has_set;
                let is_data = has_value || has_writable;
                if is_accessor && is_data {
                    return Err(Error::type_err(
                "Invalid property descriptor. Cannot both specify accessors and a value or writable attribute",
            ));
                }
                let proxy_descriptor = crate::vm::ProxyDefinePropertyDescriptor {
                    descriptor: PropertyDescriptor {
                        value: value.clone(),
                        writable,
                        enumerable,
                        configurable,
                        get: get.clone(),
                        set: set.clone(),
                        is_accessor,
                    },
                    has_value,
                    has_writable,
                    has_enumerable,
                    has_configurable,
                    has_get,
                    has_set,
                };
                let ordinary_target = match vm.proxy_define_own_property(
                    &target,
                    &key,
                    &proxy_descriptor,
                    Some(&desc),
                )? {
                    crate::vm::ProxyDefinePropertyOutcome::Ordinary(target) => target,
                    crate::vm::ProxyDefinePropertyOutcome::Complete(result) => {
                        if !result && throw_on_failure {
                            return Err(Error::type_err(
                                "Proxy defineProperty trap returned false",
                            ));
                        }
                        return Ok(result);
                    }
                };
                let idx = match ordinary_target.clone() {
                    Value::Object(idx) => idx,
                    _ => unreachable!("DefineOwnProperty target remains an object"),
                };
                let target = ordinary_target;
                let is_array_length = key.as_str() == Some("length")
                    && vm.heap.with_obj(idx.0, |object| {
                        matches!(object, HeapObj::Array(array) if !array.is_arguments.load(Ordering::Relaxed))
                    });
                if is_array_length {
                    let success = vm.define_array_length_property(
                        idx.0,
                        has_value.then(|| value.clone()),
                        has_writable,
                        writable,
                        has_enumerable,
                        enumerable,
                        has_configurable,
                        configurable,
                        is_accessor,
                    )?;
                    if !success && throw_on_failure {
                        return Err(Error::type_err("Cannot redefine Array length"));
                    }
                    return Ok(success);
                }
                let is_namespace = vm.heap.with_obj(idx.0, |object| {
                    matches!(object, HeapObj::ModuleNamespace(_))
                });
                if is_namespace {
                    let current = own_property_descriptor_for_key_or_throw(vm, &target, &key)?;
                    let success = current.is_some_and(|current| {
                        !is_accessor
                            && (!has_value || descriptor_same_value(&value, &current.value))
                            && (!has_writable || writable == current.writable)
                            && (!has_enumerable || enumerable == current.enumerable)
                            && (!has_configurable || configurable == current.configurable)
                    });
                    if !success && throw_on_failure {
                        return Err(Error::type_err("Cannot redefine module namespace property"));
                    }
                    return Ok(success);
                }
                if let Some(success) = vm.define_typed_array_integer_index_property(
                    &target,
                    &key,
                    crate::vm::TypedArrayDefineDescriptor {
                        value: has_value.then_some(&value),
                        has_configurable,
                        configurable,
                        has_enumerable,
                        enumerable,
                        is_accessor,
                        has_writable,
                        writable,
                    },
                )? {
                    if !success && throw_on_failure {
                        return Err(Error::type_err("Cannot define TypedArray integer index"));
                    }
                    return Ok(success);
                }
                if key
                    .as_str()
                    .and_then(crate::value::parse_array_index)
                    .is_some_and(|index| {
                        vm.array_index_blocked_by_non_writable_length(idx.0, index)
                    })
                {
                    if throw_on_failure {
                        return Err(Error::type_err(
                            "Cannot define Array index with non-writable length",
                        ));
                    }
                    return Ok(false);
                }
                let current = own_property_descriptor_for_key(vm, &target, &key);
                let mapped_arguments_index = key
                    .as_str()
                    .and_then(crate::value::parse_array_index)
                    .and_then(|i| {
                        vm.arguments_mapped_binding_for_index(idx.0, i)
                            .map(|mapped| (i, mapped))
                    });
                if current.is_none() {
                    let extensible = vm.heap.with_obj(idx.0, |obj| obj.is_extensible());
                    if !extensible {
                        if throw_on_failure {
                            return Err(Error::type_err(format!(
                                "Cannot define property '{}', object is not extensible",
                                key.as_str().unwrap_or("Symbol")
                            )));
                        }
                        return Ok(false);
                    }
                }
                let map_value = value.clone();
                let descriptor = if let Some(mut current) = current {
                    if !current.configurable {
                        if has_configurable && configurable {
                            if throw_on_failure {
                                return Err(Error::type_err(
                                    "Cannot redefine non-configurable property",
                                ));
                            }
                            return Ok(false);
                        }
                        if has_enumerable && enumerable != current.enumerable {
                            if throw_on_failure {
                                return Err(Error::type_err(
                                    "Cannot redefine non-configurable property",
                                ));
                            }
                            return Ok(false);
                        }
                        if is_accessor != current.is_accessor && (is_accessor || is_data) {
                            if throw_on_failure {
                                return Err(Error::type_err(
                                    "Cannot redefine non-configurable property",
                                ));
                            }
                            return Ok(false);
                        }
                        if current.is_accessor {
                            if has_get && get != current.get {
                                if throw_on_failure {
                                    return Err(Error::type_err(
                                        "Cannot redefine non-configurable property",
                                    ));
                                }
                                return Ok(false);
                            }
                            if has_set && set != current.set {
                                if throw_on_failure {
                                    return Err(Error::type_err(
                                        "Cannot redefine non-configurable property",
                                    ));
                                }
                                return Ok(false);
                            }
                        } else if is_data && !current.writable {
                            if has_writable && writable {
                                if throw_on_failure {
                                    return Err(Error::type_err(
                                        "Cannot redefine non-configurable property",
                                    ));
                                }
                                return Ok(false);
                            }
                            if has_value && !descriptor_same_value(&value, &current.value) {
                                if throw_on_failure {
                                    return Err(Error::type_err(
                                        "Cannot redefine non-configurable property",
                                    ));
                                }
                                return Ok(false);
                            }
                        }
                    }
                    if has_enumerable {
                        current.enumerable = enumerable;
                    }
                    if has_configurable {
                        current.configurable = configurable;
                    }
                    if is_accessor {
                        current.value = Value::Undefined;
                        current.writable = false;
                        if has_get {
                            current.get = get;
                        }
                        if has_set {
                            current.set = set;
                        }
                        current.is_accessor = true;
                    } else if is_data {
                        if has_value {
                            current.value = value;
                        }
                        if has_writable {
                            current.writable = writable;
                        }
                        current.get = None;
                        current.set = None;
                        current.is_accessor = false;
                    }
                    current
                } else if is_accessor {
                    PropertyDescriptor {
                        value: Value::Undefined,
                        writable: false,
                        enumerable,
                        configurable,
                        get,
                        set,
                        is_accessor: true,
                    }
                } else if is_data {
                    PropertyDescriptor {
                        value,
                        writable,
                        enumerable,
                        configurable,
                        get: None,
                        set: None,
                        is_accessor: false,
                    }
                } else {
                    // Generic descriptor (only enumerable/configurable).
                    PropertyDescriptor {
                        value: Value::Undefined,
                        writable: false,
                        enumerable,
                        configurable,
                        get: None,
                        set: None,
                        is_accessor: false,
                    }
                };
                let array_index = key.as_str().and_then(crate::value::parse_array_index);
                vm.heap.with_obj(idx.0, |obj| {
                    if let HeapObj::Array(a) = obj {
                        if let Some(i) = array_index {
                            if i >= a.items.lock().len() {
                                let new_len = i + 1;
                                if new_len <= crate::value::MAX_DENSE_ARRAY_LEN {
                                    let mut items = a.items.lock();
                                    let mut present = a.present.lock();
                                    while items.len() < new_len {
                                        items.push(Value::Undefined);
                                        present.push(false);
                                    }
                                    let dense_length = items.len();
                                    let mut sparse_max = a.sparse_max.lock();
                                    if sparse_max.is_some_and(|sparse| sparse <= dense_length) {
                                        *sparse_max = None;
                                    }
                                } else {
                                    let mut sparse_max = a.sparse_max.lock();
                                    if sparse_max.is_none_or(|current| new_len > current) {
                                        *sparse_max = Some(new_len);
                                    }
                                }
                            }
                        }
                    }
                    obj.props().lock().insert(key.clone(), descriptor);
                });
                if array_index.is_some() {
                    vm.sync_array_length_descriptor_after_index(idx.0);
                }
                if let Some((i, (env, name))) = mapped_arguments_index {
                    if is_accessor {
                        vm.remove_arguments_mapping_for_index(idx.0, i);
                    } else {
                        if has_value {
                            crate::environment::set(&vm.heap, env, &name, map_value);
                        }
                        if has_writable && !writable {
                            vm.remove_arguments_mapping_for_index(idx.0, i);
                        }
                    }
                }
                if let Some(key) = key.as_str() {
                    vm.ic_invalidate(idx.0, key);
                }
            }
            Ok(true)
        })();
        vm.unpin_many(desc_pin);
        define_result
    })();
    vm.unpin_many(argument_pins);
    result
}

// Minimal stubs to keep the crate compiling while parser/lexer work is in progress.

fn active_error_constructor_prototype(vm: &mut Vm) -> error::Result<Value> {
    if let Some(callee) = vm.current_native_callee().cloned() {
        let proto = vm.get_property_by_key(&callee, &PropertyKey::from("prototype"))?;
        if matches!(proto, Value::Object(_)) {
            return Ok(proto);
        }
    }
    Ok(vm.error_proto.clone())
}

fn active_error_constructor_name(vm: &mut Vm) -> Arc<str> {
    let Some(Value::Object(idx)) = vm.current_native_callee() else {
        return Arc::from("Error");
    };
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            f.name.clone().unwrap_or_else(|| Arc::from("Error"))
        } else {
            Arc::from("Error")
        }
    })
}

fn active_error_intrinsic_prototype(vm: &mut Vm, name: &Arc<str>) -> error::Result<Value> {
    if let Some(env) = vm.native_callee_closure() {
        if let Some(proto) = vm.realm_error_prototypes.get(&(env.0, name.clone())) {
            return Ok(proto.clone());
        }
    }
    active_error_constructor_prototype(vm)
}

fn new_target_error_constructor_prototype(vm: &mut Vm) -> error::Result<Value> {
    let name = active_error_constructor_name(vm);
    let fallback = active_error_intrinsic_prototype(vm, &name)?;
    native_constructor_prototype_with_default(vm, &name, fallback)
}

fn new_error_object(vm: &mut Vm, proto: Value) -> error::Result<GcIdx> {
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Error")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(GcIdx(vm.heap.allocate(obj)?))
}

fn error_object_for_constructor(vm: &mut Vm, this: Option<Value>) -> error::Result<GcIdx> {
    // Use the `this` provided by `construct` (already linked to <Error>.prototype).
    // When called as a plain function (Error(msg) without `new`), `this` is
    // undefined (strict) or the global object (sloppy). In sloppy mode we
    // detect the global object by checking its class_name; in strict mode
    // `this` is None. Both cases create a fresh object. But `construct`
    // passes a fresh object with class_name=None, so we must NOT treat
    // that as "not an error" — only reject the global object.
    match this {
        Some(Value::Object(i)) => {
            // Check if `this` is the global object (sloppy-mode plain call).
            // The global object has class_name "global". A fresh object from
            // `construct` has class_name None.
            let is_global = vm.heap.with_obj(i.0, |obj| {
                if let HeapObj::Object(o) = obj {
                    o.class_name.as_deref() == Some("global")
                } else {
                    false
                }
            });
            if is_global {
                let proto = active_error_constructor_prototype(vm)?;
                Ok(new_error_object(vm, proto)?)
            } else if vm.current_native_new_target().is_some() {
                let proto = new_target_error_constructor_prototype(vm)?;
                Ok(new_error_object(vm, proto)?)
            } else {
                Ok(i)
            }
        }
        _ => {
            // Called as Error(msg) or TypeError(msg) without new: create a
            // fresh object from the active constructor's prototype.
            let proto = if vm.current_native_new_target().is_some() {
                new_target_error_constructor_prototype(vm)?
            } else {
                active_error_constructor_prototype(vm)?
            };
            Ok(new_error_object(vm, proto)?)
        }
    }
}

fn install_error_message_and_cause(
    vm: &mut Vm,
    idx: GcIdx,
    msg: Option<Arc<str>>,
    options: Option<&Value>,
) -> error::Result<()> {
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Object(o) = obj {
            let mut props = o.props.lock();
            if let Some(msg) = msg {
                props.insert(PropertyKey::from("message"), data_prop(Value::String(msg)));
            }
        }
    });
    if let Some(options @ Value::Object(_)) = options {
        if vm.has_property(options, "cause")? {
            let cause = vm.get_property(options, "cause")?;
            vm.define_own_property_or_throw(
                &Value::Object(idx),
                PropertyKey::from("cause"),
                data_prop(cause),
            )?;
        }
    }
    Ok(())
}

fn error_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let idx = error_object_for_constructor(vm, this)?;
    let msg = match args.first() {
        Some(Value::Undefined) | None => None,
        Some(v) => Some(vm.to_string(v)?),
    };
    install_error_message_and_cause(vm, idx, msg, args.get(1))?;
    Ok(Value::Object(idx))
}

fn aggregate_error_constructor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let idx = error_object_for_constructor(vm, this)?;
    let msg = match args.get(1) {
        Some(Value::Undefined) | None => None,
        Some(v) => Some(vm.to_string(v)?),
    };
    install_error_message_and_cause(vm, idx, msg, args.get(2))?;

    let errors = args.first().cloned().unwrap_or(Value::Undefined);
    let iterator = vm.make_iterator(&errors)?;
    let mut list = Vec::new();
    loop {
        let (value, done) = vm.iterator_next(&iterator)?;
        if done {
            break;
        }
        list.push(value);
    }
    let errors_array = make_value_array(vm, list)?;
    vm.define_own_property_or_throw(
        &Value::Object(idx),
        PropertyKey::from("errors"),
        data_prop(errors_array),
    )?;
    Ok(Value::Object(idx))
}

const OBJECT_STATIC_METHODS: &[(&str, NativeFn, usize)] = &[
    ("keys", object_keys, 1),
    ("values", object_values, 1),
    ("entries", object_entries, 1),
    ("assign", object_assign, 2),
    ("is", object_is, 2),
    ("hasOwn", object_has_own, 2),
    ("fromEntries", object_from_entries, 1),
    ("groupBy", object_group_by, 2),
    ("create", object_create, 2),
    ("freeze", object_freeze, 1),
    ("getOwnPropertyNames", object_get_own_property_names, 1),
    ("getOwnPropertySymbols", object_get_own_property_symbols, 1),
    (
        "getOwnPropertyDescriptor",
        object_get_own_property_descriptor,
        2,
    ),
    ("defineProperty", object_define_property, 3),
    ("defineProperties", object_define_properties, 2),
    ("getPrototypeOf", object_get_prototype_of, 1),
    ("setPrototypeOf", object_set_prototype_of, 2),
    ("preventExtensions", object_prevent_extensions, 1),
    ("isExtensible", object_is_extensible, 1),
    ("seal", object_seal, 1),
    ("isSealed", object_is_sealed, 1),
    ("isFrozen", object_is_frozen, 1),
    (
        "getOwnPropertyDescriptors",
        object_get_own_property_descriptors,
        1,
    ),
];

const OBJECT_PROTOTYPE_METHODS: &[(&str, NativeFn, usize)] = &[
    ("toString", object_to_string_native, 0),
    ("toLocaleString", object_to_locale_string, 0),
    ("hasOwnProperty", object_has_own_property, 1),
    ("isPrototypeOf", object_is_prototype_of, 1),
    ("propertyIsEnumerable", object_property_is_enumerable, 1),
    ("valueOf", object_value_of, 0),
    ("__defineGetter__", object_define_getter, 2),
    ("__defineSetter__", object_define_setter, 2),
    ("__lookupGetter__", object_lookup_getter, 1),
    ("__lookupSetter__", object_lookup_setter, 1),
];

fn install_object_prototype_methods_in_env(
    vm: &mut Vm,
    object_proto: GcIdx,
    env: GcIdx,
) -> error::Result<()> {
    let prototype = Value::Object(object_proto);
    let pin_count = vm.pin(&prototype);
    let result = (|| {
        for &(name, function, length) in OBJECT_PROTOTYPE_METHODS {
            let method = vm.new_native_function_in_env(name, function, length, env)?;
            vm.heap.with_obj(object_proto.0, |object| {
                object
                    .props()
                    .lock()
                    .insert(PropertyKey::from(name), data_prop(Value::Object(method)));
            });
        }
        Ok(())
    })();
    vm.unpin_many(pin_count);
    result
}

fn install_object_proto_accessor_in_env(
    vm: &mut Vm,
    object_proto: GcIdx,
    env: GcIdx,
) -> error::Result<()> {
    let prototype = Value::Object(object_proto);
    let mut pin_count = vm.pin(&prototype);
    let result = (|| {
        let proto_get = vm.new_native_function_in_env("get __proto__", object_proto_get, 0, env)?;
        pin_count += vm.pin(&Value::Object(proto_get));
        let proto_set = vm.new_native_function_in_env("set __proto__", object_proto_set, 1, env)?;
        vm.heap.with_obj(object_proto.0, |object| {
            object.props().lock().insert(
                PropertyKey::from("__proto__"),
                accessor_prop(Value::Object(proto_get), Value::Object(proto_set)),
            );
        });
        Ok(())
    })();
    vm.unpin_many(pin_count);
    result
}

fn install_object_static_methods_in_env(
    vm: &mut Vm,
    object_ctor: GcIdx,
    env: GcIdx,
) -> error::Result<()> {
    let constructor = Value::Object(object_ctor);
    let pin_count = vm.pin(&constructor);
    let result = (|| {
        for &(name, function, length) in OBJECT_STATIC_METHODS {
            let method = vm.new_native_function_in_env(name, function, length, env)?;
            vm.heap.with_obj(object_ctor.0, |object| {
                object
                    .props()
                    .lock()
                    .insert(PropertyKey::from(name), data_prop(Value::Object(method)));
            });
        }
        Ok(())
    })();
    vm.unpin_many(pin_count);
    result
}

pub fn setup(vm: &mut Vm) -> error::Result<()> {
    let (object_ctor, object_proto) = make_builtin_constructor(vm, "Object", &[])?;
    install_object_static_methods_in_env(vm, object_ctor, vm.global)?;
    define_global(vm, "Object", Value::Object(object_ctor));
    vm.object_proto = Value::Object(object_proto);
    vm.heap.with_obj(object_proto.0, |obj| {
        *obj.proto().lock() = None;
        obj.props()
            .lock()
            .shift_remove(&PropertyKey::from("constructor"));
    });
    install_object_prototype_methods_in_env(vm, object_proto, vm.global)?;
    vm.heap.with_obj(object_proto.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(object_ctor)),
        );
    });
    install_object_proto_accessor_in_env(vm, object_proto, vm.global)?;

    let (error_ctor, error_proto) = make_error_constructor(vm, "Error")?;
    vm.error_proto = Value::Object(error_proto);
    define_global(vm, "Error", Value::Object(error_ctor));
    let native_error_ctor_parent = Value::Object(error_ctor);
    for name in [
        "TypeError",
        "RangeError",
        "ReferenceError",
        "SyntaxError",
        "EvalError",
        "URIError",
        "AggregateError",
    ] {
        let (ctor, _) = make_error_constructor(vm, name)?;
        vm.heap.with_obj(ctor.0, |obj| {
            if let HeapObj::Function(f) = obj {
                *f.proto.lock() = Some(native_error_ctor_parent.clone());
            }
        });
        define_global(vm, name, Value::Object(ctor));
    }
    vm.install_heap_limit_error_in_realm(vm.global)?;
    Ok(())
}

// =========================================================================
// Extended setup
// =========================================================================
fn install_async_function_intrinsic(
    vm: &mut Vm,
    env: GcIdx,
    function_proto: &Value,
    function_ctor: GcIdx,
) -> error::Result<()> {
    let constructor = vm.new_native_constructor_in_env(
        "AsyncFunction",
        async_function_constructor,
        1,
        env,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    set_function_object_proto(vm, constructor, &Value::Object(function_ctor));
    let prototype = Value::Object(GcIdx(vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(function_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("AsyncFunction")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?));
    if let Value::Object(prototype_idx) = &prototype {
        vm.heap.with_obj(prototype_idx.0, |obj| {
            let mut props = obj.props().lock();
            let mut constructor_desc = data_prop(Value::Object(constructor));
            constructor_desc.writable = false;
            props.insert(PropertyKey::from("constructor"), constructor_desc);
            let mut tag_desc = data_prop(Value::String(Arc::from("AsyncFunction")));
            tag_desc.writable = false;
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
                tag_desc,
            );
        });
    }
    vm.heap.with_obj(constructor.0, |obj| {
        if let HeapObj::Function(f) = obj {
            *f.prototype.lock() = Some(prototype.clone());
            f.props.lock().insert(
                PropertyKey::from("prototype"),
                const_prop(prototype.clone()),
            );
        }
    });
    vm.realm_async_function_prototypes.insert(env.0, prototype);
    Ok(())
}

fn active_iterator_realm(vm: &Vm) -> GcIdx {
    vm.native_callee_closure()
        .map(|closure| env::global_env_root(&vm.heap, closure))
        .unwrap_or(vm.global)
}

fn iterator_constructor(
    vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let Some(new_target) = vm.current_native_new_target().cloned() else {
        return Err(Error::type_err("Iterator must be subclassed"));
    };
    if vm
        .current_native_callee()
        .is_some_and(|callee| *callee == new_target)
    {
        return Err(Error::type_err("Iterator must be subclassed"));
    }
    let realm = active_iterator_realm(vm);
    let fallback = vm
        .realm_iterator_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.iterator_base_proto.clone());
    let proto = native_constructor_prototype_with_default(vm, "Iterator", fallback)?;
    let pin_count = vm.pin(&proto);
    let object = HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Iterator")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let result = vm.alloc(object).map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

fn iterator_identity(_vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(this.unwrap_or(Value::Undefined))
}

fn iterator_from(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let input = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(input, Value::Object(_) | Value::String(_)) {
        return Err(Error::type_err(
            "Iterator.from requires an object or string",
        ));
    }

    let iterator_key = PropertyKey::Symbol(vm.well_known_symbols.iterator);
    let method = vm.get_property_by_key(&input, &iterator_key)?;
    let iterator = if method.is_nullish() {
        input
    } else {
        if !is_callable(&method, &vm.heap) {
            return Err(Error::type_err("iterator method is not callable"));
        }
        let method_pin = vm.pin(&method);
        let result = vm.call_function(&method, &[], Some(input));
        vm.unpin_many(method_pin);
        result?
    };
    if !matches!(iterator, Value::Object(_)) {
        return Err(Error::type_err("iterator method must return an object"));
    }

    let mut pin_count = vm.pin(&iterator);
    let result = (|| -> error::Result<Value> {
        let next = vm.get_property(&iterator, "next")?;
        pin_count += vm.pin(&next);
        let realm = active_iterator_realm(vm);
        let constructor = vm
            .realm_iterator_constructors
            .get(&realm.0)
            .cloned()
            .ok_or_else(|| Error::internal("missing Iterator intrinsic"))?;
        if vm.ordinary_has_instance(&constructor, &iterator)? {
            return Ok(iterator.clone());
        }
        let proto = vm
            .realm_wrap_for_valid_iterator_prototypes
            .get(&realm.0)
            .cloned()
            .ok_or_else(|| Error::internal("missing valid Iterator wrapper prototype"))?;
        let wrapper = Value::Object(vm.alloc(HeapObj::CollectionIterator(
            CollectionIteratorData {
                source: Mutex::new(iterator.clone()),
                next_method: Mutex::new(Some(next)),
                kind: CollectionIteratorKind::WrappedIterator,
                index: Mutex::new(0),
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(Some(proto)),
                extensible: AtomicBool::new(true),
            },
        ))?);
        vm.keep_during_job(&wrapper);
        Ok(wrapper)
    })();
    vm.unpin_many(pin_count);
    result
}

fn valid_iterator_wrapper_record(vm: &Vm, this: Option<Value>) -> error::Result<(Value, Value)> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err(
            "valid Iterator wrapper method called on incompatible receiver",
        ));
    };
    vm.heap
        .with_obj(idx.0, |obj| {
            let HeapObj::CollectionIterator(iterator) = obj else {
                return None;
            };
            if iterator.kind != CollectionIteratorKind::WrappedIterator {
                return None;
            }
            iterator
                .next_method
                .lock()
                .clone()
                .map(|next| (iterator.source.lock().clone(), next))
        })
        .ok_or_else(|| {
            Error::type_err("valid Iterator wrapper method called on incompatible receiver")
        })
}

fn valid_iterator_wrapper_next(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (iterator, next) = valid_iterator_wrapper_record(vm, this)?;
    vm.call_function(&next, &[], Some(iterator))
}

fn valid_iterator_wrapper_return(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (iterator, _) = valid_iterator_wrapper_record(vm, this)?;
    let return_method = vm.get_property(&iterator, "return")?;
    if return_method.is_nullish() {
        let realm = active_iterator_realm(vm);
        let proto = vm
            .realm_object_prototypes
            .get(&realm.0)
            .cloned()
            .ok_or_else(|| Error::internal("missing Object prototype intrinsic"))?;
        let result = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::from([
                (
                    PropertyKey::from("value"),
                    PropertyDescriptor::data(Value::Undefined),
                ),
                (
                    PropertyKey::from("done"),
                    PropertyDescriptor::data(Value::Bool(true)),
                ),
            ])),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        vm.keep_during_job(&result);
        return Ok(result);
    }
    if !is_callable(&return_method, &vm.heap) {
        return Err(Error::type_err("Iterator return is not callable"));
    }
    let return_pin = vm.pin(&return_method);
    let result = vm.call_function(&return_method, &[], Some(iterator));
    vm.unpin_many(return_pin);
    result
}

fn create_iterator_result_in_realm(
    vm: &mut Vm,
    value: Value,
    done: bool,
    realm: GcIdx,
) -> error::Result<Value> {
    let proto = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Object prototype intrinsic"))?;
    let pin_count = vm.pin_many(&[value.clone(), proto.clone()]);
    let result = vm
        .alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::from([
                (PropertyKey::from("value"), PropertyDescriptor::data(value)),
                (
                    PropertyKey::from("done"),
                    PropertyDescriptor::data(Value::Bool(done)),
                ),
            ])),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

fn create_iterator_result(vm: &mut Vm, value: Value, done: bool) -> error::Result<Value> {
    create_iterator_result_in_realm(vm, value, done, active_iterator_realm(vm))
}

fn close_iterator_preserving_abrupt(vm: &mut Vm, iterator: &Value) -> error::Result<()> {
    let return_method = match vm.get_property(iterator, "return") {
        Ok(method) => method,
        Err(error) if error.catchable() => return Ok(()),
        Err(error) => return Err(error),
    };
    if return_method.is_nullish() || !is_callable(&return_method, &vm.heap) {
        return Ok(());
    }
    match vm.call_function(&return_method, &[], Some(iterator.clone())) {
        Ok(_) => Ok(()),
        Err(error) if error.catchable() => Ok(()),
        Err(error) => Err(error),
    }
}

fn close_iterator_after_error<T>(
    vm: &mut Vm,
    iterator: &Value,
    error: Arc<Error>,
) -> error::Result<T> {
    let error_pin = error
        .thrown_value
        .as_ref()
        .map(|value| vm.pin(value))
        .unwrap_or(0);
    let close_result = close_iterator_preserving_abrupt(vm, iterator);
    vm.unpin_many(error_pin);
    close_result?;
    Err(error)
}

fn close_iterator_normally(vm: &mut Vm, iterator: &Value) -> error::Result<()> {
    let return_method = vm.get_property(iterator, "return")?;
    if return_method.is_nullish() {
        return Ok(());
    }
    if !is_callable(&return_method, &vm.heap) {
        return Err(Error::type_err("Iterator return is not callable"));
    }
    let result = vm.call_function(&return_method, &[], Some(iterator.clone()))?;
    if !matches!(result, Value::Object(_)) {
        return Err(Error::type_err("Iterator return must return an object"));
    }
    Ok(())
}

fn close_iterator_records(
    vm: &mut Vm,
    records: &[IteratorHelperInner],
    mut completion: error::Result<()>,
) -> error::Result<()> {
    let mut roots = Vec::with_capacity(records.len() * 2);
    for record in records {
        roots.push(record.iterator.clone());
        roots.push(record.next_method.clone());
    }
    let pin_count = vm.pin_many(&roots);
    let result = (|| {
        for record in records.iter().rev() {
            if let Err(error) = &completion {
                if !error.catchable() {
                    return completion;
                }
            }
            vm.consume_fuel()?;
            completion = match completion {
                Ok(()) => close_iterator_normally(vm, &record.iterator),
                Err(error) => {
                    let error_pin = error
                        .thrown_value
                        .as_ref()
                        .map(|value| vm.pin(value))
                        .unwrap_or(0);
                    let close_result = close_iterator_preserving_abrupt(vm, &record.iterator);
                    vm.unpin_many(error_pin);
                    match close_result {
                        Ok(()) => Err(error),
                        Err(close_error) => Err(close_error),
                    }
                }
            };
        }
        completion
    })();
    vm.unpin_many(pin_count);
    result
}

fn create_array_from_values_in_realm(
    vm: &mut Vm,
    values: Vec<Value>,
    realm: GcIdx,
) -> error::Result<Value> {
    let prototype = vm
        .realm_array_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Array prototype intrinsic"))?;
    let mut roots = Vec::with_capacity(values.len() + 1);
    roots.extend(values.iter().cloned());
    roots.push(prototype.clone());
    let pin_count = vm.pin_many(&roots);
    let result = vm
        .alloc(HeapObj::Array(ArrayData::new(values, Some(prototype))))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

fn create_keyed_object_from_values(
    vm: &mut Vm,
    keys: &[PropertyKey],
    values: Vec<Value>,
) -> error::Result<Value> {
    if keys.len() != values.len() {
        return Err(Error::internal("Iterator.zipKeyed result length mismatch"));
    }
    let pin_count = vm.pin_many(&values);
    let props = keys
        .iter()
        .cloned()
        .zip(values)
        .map(|(key, value)| (key, PropertyDescriptor::data(value)))
        .collect();
    let result = vm
        .alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(props),
            proto: Mutex::new(None),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

fn allocate_iterator_helper(
    vm: &mut Vm,
    iterator: Value,
    next_method: Value,
    callback: Option<Value>,
    kind: IteratorHelperKind,
    remaining: Option<BigUint>,
) -> error::Result<Value> {
    let realm = active_iterator_realm(vm);
    let proto = vm
        .realm_iterator_helper_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Iterator Helper prototype intrinsic"))?;
    let mut roots = vec![iterator.clone(), next_method.clone(), proto.clone()];
    if let Some(callback) = &callback {
        roots.push(callback.clone());
    }
    let pin_count = vm.pin_many(&roots);
    let helper = vm
        .alloc(HeapObj::IteratorHelper(IteratorHelperData {
            resume_realm: realm,
            iterator,
            next_method,
            callback,
            kind,
            counter: Mutex::new(BigUint::zero()),
            inner_iterator: Mutex::new(None),
            concat_iterables: Box::new([]),
            concat_index: AtomicUsize::new(0),
            zip_iterators: Mutex::new(Box::new([])),
            zip_open_count: AtomicUsize::new(0),
            zip_padding: Box::new([]),
            zip_keys: Box::new([]),
            zip_mode: IteratorZipMode::Shortest,
            remaining: Mutex::new(remaining),
            state: std::sync::atomic::AtomicU8::new(0),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    helper
}

fn allocate_iterator_concat(
    vm: &mut Vm,
    iterables: Vec<IteratorConcatIterable>,
) -> error::Result<Value> {
    let realm = active_iterator_realm(vm);
    let proto = vm
        .realm_iterator_helper_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Iterator Helper prototype intrinsic"))?;
    let mut roots = Vec::with_capacity(iterables.len() * 2 + 1);
    roots.push(proto.clone());
    for record in &iterables {
        roots.push(record.iterable.clone());
        roots.push(record.open_method.clone());
    }
    let pin_count = vm.pin_many(&roots);
    let helper = vm
        .alloc(HeapObj::IteratorHelper(IteratorHelperData {
            resume_realm: realm,
            iterator: Value::Undefined,
            next_method: Value::Undefined,
            callback: None,
            kind: IteratorHelperKind::Concat,
            counter: Mutex::new(BigUint::zero()),
            inner_iterator: Mutex::new(None),
            concat_iterables: iterables.into_boxed_slice(),
            concat_index: AtomicUsize::new(0),
            zip_iterators: Mutex::new(Box::new([])),
            zip_open_count: AtomicUsize::new(0),
            zip_padding: Box::new([]),
            zip_keys: Box::new([]),
            zip_mode: IteratorZipMode::Shortest,
            remaining: Mutex::new(Some(BigUint::zero())),
            state: std::sync::atomic::AtomicU8::new(0),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    helper
}

fn allocate_iterator_zip(
    vm: &mut Vm,
    iterators: Vec<IteratorHelperInner>,
    mode: IteratorZipMode,
    padding: Vec<Value>,
    kind: IteratorHelperKind,
    keys: Vec<PropertyKey>,
) -> error::Result<Value> {
    if !matches!(kind, IteratorHelperKind::Zip | IteratorHelperKind::ZipKeyed) {
        return Err(Error::internal("invalid Iterator zip helper kind"));
    }
    if kind == IteratorHelperKind::ZipKeyed && keys.len() != iterators.len() {
        return Err(Error::internal("Iterator.zipKeyed key count mismatch"));
    }
    let realm = active_iterator_realm(vm);
    let proto = vm
        .realm_iterator_helper_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Iterator Helper prototype intrinsic"))?;
    let mut roots = Vec::with_capacity(iterators.len() * 2 + padding.len() + 1);
    roots.push(proto.clone());
    for record in &iterators {
        roots.push(record.iterator.clone());
        roots.push(record.next_method.clone());
    }
    roots.extend(padding.iter().cloned());
    let pin_count = vm.pin_many(&roots);
    let open_count = iterators.len();
    let slots = iterators
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let helper = vm
        .alloc(HeapObj::IteratorHelper(IteratorHelperData {
            resume_realm: realm,
            iterator: Value::Undefined,
            next_method: Value::Undefined,
            callback: None,
            kind,
            counter: Mutex::new(BigUint::zero()),
            inner_iterator: Mutex::new(None),
            concat_iterables: Box::new([]),
            concat_index: AtomicUsize::new(0),
            zip_iterators: Mutex::new(slots),
            zip_open_count: AtomicUsize::new(open_count),
            zip_padding: padding.into_boxed_slice(),
            zip_keys: keys.into_boxed_slice(),
            zip_mode: mode,
            remaining: Mutex::new(Some(BigUint::zero())),
            state: std::sync::atomic::AtomicU8::new(0),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    helper
}

fn get_sync_iterator(vm: &mut Vm, iterable: Value) -> error::Result<IteratorHelperInner> {
    let iterable_pin = vm.pin(&iterable);
    let result = (|| {
        let iterator_key = PropertyKey::Symbol(vm.well_known_symbols.iterator);
        let method = vm.get_property_by_key(&iterable, &iterator_key)?;
        if method.is_nullish() || !is_callable(&method, &vm.heap) {
            return Err(Error::type_err("value is not iterable"));
        }
        let method_pin = vm.pin(&method);
        let iterator_result = vm.call_function(&method, &[], Some(iterable.clone()));
        vm.unpin_many(method_pin);
        let iterator = iterator_result?;
        if !matches!(iterator, Value::Object(_)) {
            return Err(Error::type_err("iterator method must return an object"));
        }
        let iterator_pin = vm.pin(&iterator);
        let next_result = vm.get_property(&iterator, "next");
        vm.unpin_many(iterator_pin);
        Ok(IteratorHelperInner {
            iterator,
            next_method: next_result?,
        })
    })();
    vm.unpin_many(iterable_pin);
    result
}

fn close_iterator_records_after_error<T>(
    vm: &mut Vm,
    records: &[IteratorHelperInner],
    error: Arc<Error>,
) -> error::Result<T> {
    close_iterator_records(vm, records, Err(error))?;
    unreachable!("an abrupt completion cannot become normal")
}

fn iterator_joint_options(
    vm: &mut Vm,
    args: &[Value],
    name: &str,
) -> error::Result<(IteratorZipMode, Value)> {
    let options = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !matches!(options, Value::Undefined | Value::Object(_)) {
        return Err(Error::type_err(format!("{name} options must be an object")));
    }
    let mode_value = if matches!(options, Value::Undefined) {
        Value::Undefined
    } else {
        vm.get_property(&options, "mode")?
    };
    let mode = match mode_value {
        Value::Undefined => IteratorZipMode::Shortest,
        Value::String(value) if value.as_ref() == "shortest" => IteratorZipMode::Shortest,
        Value::String(value) if value.as_ref() == "longest" => IteratorZipMode::Longest,
        Value::String(value) if value.as_ref() == "strict" => IteratorZipMode::Strict,
        _ => return Err(Error::type_err(format!("invalid {name} mode"))),
    };
    let padding = if mode == IteratorZipMode::Longest && !matches!(options, Value::Undefined) {
        vm.get_property(&options, "padding")?
    } else {
        Value::Undefined
    };
    if mode == IteratorZipMode::Longest && !matches!(padding, Value::Undefined | Value::Object(_)) {
        return Err(Error::type_err(format!(
            "{name} padding must be an object or undefined"
        )));
    }
    Ok((mode, padding))
}

fn iterator_zip(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let iterables = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(iterables, Value::Object(_)) {
        return Err(Error::type_err("Iterator.zip requires an object iterable"));
    }

    let (mode, padding_option) = iterator_joint_options(vm, args, "Iterator.zip")?;

    let mut root_pins = vm.pin(&padding_option);
    let outer = match get_sync_iterator(vm, iterables) {
        Ok(outer) => outer,
        Err(error) => {
            vm.unpin_many(root_pins);
            return Err(error);
        }
    };
    root_pins += vm.pin_many(&[outer.iterator.clone(), outer.next_method.clone()]);
    let mut iterators = Vec::new();
    let setup = (|| -> error::Result<()> {
        loop {
            let item = match iterator_helper_step(vm, &outer.iterator, &outer.next_method, true) {
                Ok(Some(item)) => item,
                Ok(None) => break,
                Err(error) => {
                    return close_iterator_records_after_error(vm, &iterators, error);
                }
            };
            let record = match get_iterator_flattenable_reject_primitives(
                vm,
                item,
                "Iterator.zip inputs must be objects",
            ) {
                Ok((iterator, next_method)) => IteratorHelperInner {
                    iterator,
                    next_method,
                },
                Err(error) => {
                    let mut records = Vec::with_capacity(iterators.len() + 1);
                    records.push(outer.clone());
                    records.extend(iterators.iter().cloned());
                    return close_iterator_records_after_error(vm, &records, error);
                }
            };
            root_pins += vm.pin_many(&[record.iterator.clone(), record.next_method.clone()]);
            iterators.push(record);
        }
        Ok(())
    })();
    if let Err(error) = setup {
        vm.unpin_many(root_pins);
        return Err(error);
    }

    let mut padding = Vec::with_capacity(iterators.len());
    let padding_result = (|| -> error::Result<()> {
        if matches!(padding_option, Value::Undefined) {
            padding.resize(iterators.len(), Value::Undefined);
            return Ok(());
        }

        let padding_iterator = match get_sync_iterator(vm, padding_option) {
            Ok(iterator) => iterator,
            Err(error) => {
                return close_iterator_records_after_error(vm, &iterators, error);
            }
        };
        root_pins += vm.pin_many(&[
            padding_iterator.iterator.clone(),
            padding_iterator.next_method.clone(),
        ]);
        (|| -> error::Result<()> {
            let mut using_iterator = true;
            for _ in 0..iterators.len() {
                if using_iterator {
                    match iterator_helper_step(
                        vm,
                        &padding_iterator.iterator,
                        &padding_iterator.next_method,
                        true,
                    ) {
                        Ok(Some(value)) => {
                            root_pins += vm.pin(&value);
                            padding.push(value);
                            continue;
                        }
                        Ok(None) => using_iterator = false,
                        Err(error) => {
                            return close_iterator_records_after_error(vm, &iterators, error);
                        }
                    }
                }
                padding.push(Value::Undefined);
            }
            if using_iterator {
                if let Err(error) = close_iterator_normally(vm, &padding_iterator.iterator) {
                    return close_iterator_records_after_error(vm, &iterators, error);
                }
            }
            Ok(())
        })()
    })();
    if let Err(error) = padding_result {
        vm.unpin_many(root_pins);
        return Err(error);
    }

    let result = allocate_iterator_zip(
        vm,
        iterators,
        mode,
        padding,
        IteratorHelperKind::Zip,
        Vec::new(),
    );
    vm.unpin_many(root_pins);
    result
}

fn iterator_zip_keyed(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let iterables = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(iterables, Value::Object(_)) {
        return Err(Error::type_err("Iterator.zipKeyed requires an object"));
    }
    let (mode, padding_option) = iterator_joint_options(vm, args, "Iterator.zipKeyed")?;
    let mut root_pins = vm.pin_many(&[iterables.clone(), padding_option.clone()]);
    let all_keys = match own_property_keys_or_throw(vm, &iterables, false, true, true) {
        Ok(keys) => keys,
        Err(error) => {
            vm.unpin_many(root_pins);
            return Err(error);
        }
    };

    let mut keys = Vec::new();
    let mut iterators = Vec::new();
    let setup = (|| -> error::Result<()> {
        for key in all_keys {
            vm.consume_fuel()?;
            let descriptor = match own_property_descriptor_for_key_or_throw(vm, &iterables, &key) {
                Ok(descriptor) => descriptor,
                Err(error) => {
                    return close_iterator_records_after_error(vm, &iterators, error);
                }
            };
            if !descriptor.is_some_and(|descriptor| descriptor.enumerable) {
                continue;
            }
            let value = match vm.get_property_by_key(&iterables, &key) {
                Ok(value) => value,
                Err(error) => {
                    return close_iterator_records_after_error(vm, &iterators, error);
                }
            };
            if value.is_undefined() {
                continue;
            }
            keys.push(key);
            let record = match get_iterator_flattenable_reject_primitives(
                vm,
                value,
                "Iterator.zipKeyed inputs must be objects",
            ) {
                Ok((iterator, next_method)) => IteratorHelperInner {
                    iterator,
                    next_method,
                },
                Err(error) => {
                    return close_iterator_records_after_error(vm, &iterators, error);
                }
            };
            root_pins += vm.pin_many(&[record.iterator.clone(), record.next_method.clone()]);
            iterators.push(record);
        }
        Ok(())
    })();
    if let Err(error) = setup {
        vm.unpin_many(root_pins);
        return Err(error);
    }

    let mut padding = Vec::with_capacity(keys.len());
    let padding_result = (|| -> error::Result<()> {
        if padding_option.is_undefined() {
            padding.resize(keys.len(), Value::Undefined);
            return Ok(());
        }
        for key in &keys {
            vm.consume_fuel()?;
            let value = match vm.get_property_by_key(&padding_option, key) {
                Ok(value) => value,
                Err(error) => {
                    return close_iterator_records_after_error(vm, &iterators, error);
                }
            };
            root_pins += vm.pin(&value);
            padding.push(value);
        }
        Ok(())
    })();
    if let Err(error) = padding_result {
        vm.unpin_many(root_pins);
        return Err(error);
    }

    let result = allocate_iterator_zip(
        vm,
        iterators,
        mode,
        padding,
        IteratorHelperKind::ZipKeyed,
        keys,
    );
    vm.unpin_many(root_pins);
    result
}

fn iterator_concat(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let iterator_key = PropertyKey::Symbol(vm.well_known_symbols.iterator);
    let mut iterables = Vec::with_capacity(args.len());
    let mut pin_count = 0;
    let result = (|| -> error::Result<Value> {
        for item in args {
            vm.consume_fuel()?;
            if !matches!(item, Value::Object(_)) {
                return Err(Error::type_err("Iterator.concat requires object iterables"));
            }
            let method = vm.get_property_by_key(item, &iterator_key)?;
            if method.is_nullish() || !is_callable(&method, &vm.heap) {
                return Err(Error::type_err(
                    "Iterator.concat iterator method is not callable",
                ));
            }
            pin_count += vm.pin_many(&[item.clone(), method.clone()]);
            iterables.push(IteratorConcatIterable {
                iterable: item.clone(),
                open_method: method,
            });
        }
        allocate_iterator_concat(vm, iterables)
    })();
    vm.unpin_many(pin_count);
    result
}

fn iterator_callback_helper_start(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    kind: IteratorHelperKind,
) -> error::Result<Value> {
    let Some(iterator @ Value::Object(_)) = this else {
        return Err(Error::type_err("Iterator helper requires an object"));
    };
    let callback = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        close_iterator_preserving_abrupt(vm, &iterator)?;
        return Err(Error::type_err("Iterator helper callback is not callable"));
    }
    let next_method = vm.get_property(&iterator, "next")?;
    allocate_iterator_helper(
        vm,
        iterator,
        next_method,
        Some(callback),
        kind,
        Some(BigUint::zero()),
    )
}

fn iterator_limit_helper_start(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    kind: IteratorHelperKind,
) -> error::Result<Value> {
    let Some(iterator @ Value::Object(_)) = this else {
        return Err(Error::type_err("Iterator helper requires an object"));
    };
    let limit = args.first().cloned().unwrap_or(Value::Undefined);
    let number = match vm.to_number(&limit) {
        Ok(number) => number,
        Err(error) => {
            let error_pin = error
                .thrown_value
                .as_ref()
                .map(|value| vm.pin(value))
                .unwrap_or(0);
            let close_result = close_iterator_preserving_abrupt(vm, &iterator);
            vm.unpin_many(error_pin);
            close_result?;
            return Err(error);
        }
    };
    let integer = number.trunc();
    if number.is_nan() || integer < 0.0 {
        close_iterator_preserving_abrupt(vm, &iterator)?;
        return Err(Error::range("Iterator helper limit must be non-negative"));
    }
    let remaining = if integer == f64::INFINITY {
        None
    } else {
        Some(
            BigUint::from_f64(integer)
                .ok_or_else(|| Error::internal("invalid finite Iterator helper limit"))?,
        )
    };
    let next_method = vm.get_property(&iterator, "next")?;
    allocate_iterator_helper(vm, iterator, next_method, None, kind, remaining)
}

fn iterator_map(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    iterator_callback_helper_start(vm, args, this, IteratorHelperKind::Map)
}

fn iterator_filter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    iterator_callback_helper_start(vm, args, this, IteratorHelperKind::Filter)
}

fn iterator_flat_map(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    iterator_callback_helper_start(vm, args, this, IteratorHelperKind::FlatMap)
}

fn iterator_take(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    iterator_limit_helper_start(vm, args, this, IteratorHelperKind::Take)
}

fn iterator_drop(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    iterator_limit_helper_start(vm, args, this, IteratorHelperKind::Drop)
}

fn iterator_helper_set_state(vm: &Vm, idx: GcIdx, state: u8) {
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::IteratorHelper(helper) = obj {
            helper.state.store(state, Ordering::Relaxed);
        }
    });
}

fn iterator_helper_consume_remaining(vm: &Vm, idx: GcIdx) -> bool {
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return false;
        };
        let mut slot = helper.remaining.lock();
        let Some(remaining) = slot.as_mut() else {
            return true;
        };
        if remaining.is_zero() {
            false
        } else {
            *remaining -= 1u8;
            true
        }
    })
}

fn iterator_helper_inner(vm: &Vm, idx: GcIdx) -> Option<(Value, Value)> {
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return None;
        };
        helper
            .inner_iterator
            .lock()
            .as_ref()
            .map(|inner| (inner.iterator.clone(), inner.next_method.clone()))
    })
}

fn iterator_helper_set_inner(vm: &Vm, idx: GcIdx, inner: Option<(Value, Value)>) {
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::IteratorHelper(helper) = obj {
            *helper.inner_iterator.lock() =
                inner.map(|(iterator, next_method)| IteratorHelperInner {
                    iterator,
                    next_method,
                });
        }
    });
}

fn iterator_helper_counter_number(vm: &Vm, idx: GcIdx) -> f64 {
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return 0.0;
        };
        helper.counter.lock().to_f64().unwrap_or(f64::INFINITY)
    })
}

fn iterator_helper_increment_counter(vm: &Vm, idx: GcIdx) {
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::IteratorHelper(helper) = obj {
            *helper.counter.lock() += 1u8;
        }
    });
}

fn iterator_helper_concat_record(vm: &Vm, idx: GcIdx) -> Option<(Value, Value)> {
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return None;
        };
        let index = helper.concat_index.load(Ordering::Relaxed);
        helper
            .concat_iterables
            .get(index)
            .map(|record| (record.iterable.clone(), record.open_method.clone()))
    })
}

fn iterator_helper_advance_concat(vm: &Vm, idx: GcIdx) {
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::IteratorHelper(helper) = obj {
            helper.concat_index.fetch_add(1, Ordering::Relaxed);
        }
    });
}

fn iterator_helper_zip_snapshot(
    vm: &Vm,
    idx: GcIdx,
) -> Option<(IteratorZipMode, IteratorHelperKind, usize)> {
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return None;
        };
        Some((
            helper.zip_mode,
            helper.kind,
            helper.zip_iterators.lock().len(),
        ))
    })
}

fn iterator_helper_zip_keys(vm: &Vm, idx: GcIdx) -> Vec<PropertyKey> {
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return Vec::new();
        };
        helper.zip_keys.to_vec()
    })
}

fn iterator_helper_zip_padding(vm: &Vm, idx: GcIdx, index: usize) -> Value {
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return Value::Undefined;
        };
        helper
            .zip_padding
            .get(index)
            .cloned()
            .unwrap_or(Value::Undefined)
    })
}

fn iterator_helper_zip_record(vm: &Vm, idx: GcIdx, index: usize) -> Option<IteratorHelperInner> {
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return None;
        };
        helper
            .zip_iterators
            .lock()
            .get(index)
            .and_then(Clone::clone)
    })
}

fn iterator_helper_zip_mark_done(vm: &Vm, idx: GcIdx, index: usize) {
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::IteratorHelper(helper) = obj {
            if let Some(slot) = helper.zip_iterators.lock().get_mut(index) {
                if slot.take().is_some() {
                    helper.zip_open_count.fetch_sub(1, Ordering::Relaxed);
                }
            }
        }
    });
}

fn iterator_helper_zip_has_open(vm: &Vm, idx: GcIdx) -> bool {
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return false;
        };
        helper.zip_open_count.load(Ordering::Relaxed) != 0
    })
}

fn iterator_helper_take_zip_open(
    vm: &mut Vm,
    idx: GcIdx,
) -> error::Result<Vec<IteratorHelperInner>> {
    let len = vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return 0;
        };
        helper.zip_iterators.lock().len()
    });
    let mut records = Vec::new();
    for _ in 0..len {
        vm.consume_fuel()?;
    }
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::IteratorHelper(helper) = obj {
            records.extend(
                helper
                    .zip_iterators
                    .lock()
                    .iter_mut()
                    .filter_map(Option::take),
            );
            helper.zip_open_count.store(0, Ordering::Relaxed);
        }
    });
    Ok(records)
}

fn iterator_concat_open_inner(
    vm: &mut Vm,
    iterable: Value,
    open_method: Value,
) -> error::Result<(Value, Value)> {
    let base_pin = vm.pin_many(&[iterable.clone(), open_method.clone()]);
    let iterator_result = vm.call_function(&open_method, &[], Some(iterable));
    vm.unpin_many(base_pin);
    let iterator = iterator_result?;
    if !matches!(iterator, Value::Object(_)) {
        return Err(Error::type_err(
            "Iterator.concat iterator method must return an object",
        ));
    }
    let iterator_pin = vm.pin(&iterator);
    let next_method = vm.get_property(&iterator, "next");
    vm.unpin_many(iterator_pin);
    next_method.map(|next_method| (iterator, next_method))
}

fn get_iterator_flattenable_reject_primitives(
    vm: &mut Vm,
    mapped: Value,
    primitive_error: &'static str,
) -> error::Result<(Value, Value)> {
    if !matches!(mapped, Value::Object(_)) {
        return Err(Error::type_err(primitive_error));
    }
    let mapped_pin = vm.pin(&mapped);
    let iterator_key = PropertyKey::Symbol(vm.well_known_symbols.iterator);
    let method = match vm.get_property_by_key(&mapped, &iterator_key) {
        Ok(method) => method,
        Err(error) => {
            vm.unpin_many(mapped_pin);
            return Err(error);
        }
    };
    let iterator = if method.is_nullish() {
        mapped.clone()
    } else {
        if !is_callable(&method, &vm.heap) {
            vm.unpin_many(mapped_pin);
            return Err(Error::type_err("iterator method is not callable"));
        }
        let method_pin = vm.pin(&method);
        let result = vm.call_function(&method, &[], Some(mapped.clone()));
        vm.unpin_many(method_pin);
        match result {
            Ok(iterator @ Value::Object(_)) => iterator,
            Ok(_) => {
                vm.unpin_many(mapped_pin);
                return Err(Error::type_err("iterator method must return an object"));
            }
            Err(error) => {
                vm.unpin_many(mapped_pin);
                return Err(error);
            }
        }
    };
    let iterator_pin = vm.pin(&iterator);
    let next_result = vm.get_property(&iterator, "next");
    vm.unpin_many(iterator_pin + mapped_pin);
    next_result.map(|next_method| (iterator, next_method))
}

fn iterator_helper_step(
    vm: &mut Vm,
    iterator: &Value,
    next_method: &Value,
    read_value: bool,
) -> error::Result<Option<Value>> {
    vm.consume_fuel()?;
    let step = vm.call_function(next_method, &[], Some(iterator.clone()))?;
    if !matches!(step, Value::Object(_)) {
        return Err(Error::type_err("Iterator result is not an object"));
    }
    let step_pin = vm.pin(&step);
    let result = (|| -> error::Result<Option<Value>> {
        let done = vm.get_property(&step, "done")?;
        if vm.to_boolean(&done) {
            return Ok(None);
        }
        if read_value {
            Ok(Some(vm.get_property(&step, "value")?))
        } else {
            Ok(Some(Value::Undefined))
        }
    })();
    vm.unpin_many(step_pin);
    result
}

fn iterator_zip_step(vm: &mut Vm, idx: GcIdx, realm: GcIdx) -> error::Result<Option<Value>> {
    let (mode, kind, iter_count) = iterator_helper_zip_snapshot(vm, idx)
        .ok_or_else(|| Error::internal("Iterator.zip helper state is missing"))?;
    if iter_count == 0 {
        return Ok(None);
    }

    let mut values = Vec::with_capacity(iter_count);
    let mut value_pins = 0;
    let result = (|| -> error::Result<Option<Value>> {
        for index in 0..iter_count {
            let Some(record) = iterator_helper_zip_record(vm, idx, index) else {
                if mode == IteratorZipMode::Longest {
                    vm.consume_fuel()?;
                    let value = iterator_helper_zip_padding(vm, idx, index);
                    value_pins += vm.pin(&value);
                    values.push(value);
                    continue;
                }
                return Err(Error::internal(
                    "inactive iterator in non-longest zip helper",
                ));
            };
            let record_pin = vm.pin_many(&[record.iterator.clone(), record.next_method.clone()]);
            let step = iterator_helper_step(vm, &record.iterator, &record.next_method, true);
            vm.unpin_many(record_pin);
            match step {
                Ok(Some(value)) => {
                    value_pins += vm.pin(&value);
                    values.push(value);
                }
                Err(error) => {
                    iterator_helper_zip_mark_done(vm, idx, index);
                    let open = iterator_helper_take_zip_open(vm, idx)?;
                    return close_iterator_records_after_error(vm, &open, error);
                }
                Ok(None) => {
                    iterator_helper_zip_mark_done(vm, idx, index);
                    match mode {
                        IteratorZipMode::Longest => {
                            if !iterator_helper_zip_has_open(vm, idx) {
                                return Ok(None);
                            }
                            let value = iterator_helper_zip_padding(vm, idx, index);
                            value_pins += vm.pin(&value);
                            values.push(value);
                        }
                        IteratorZipMode::Shortest => {
                            let open = iterator_helper_take_zip_open(vm, idx)?;
                            close_iterator_records(vm, &open, Ok(()))?;
                            return Ok(None);
                        }
                        IteratorZipMode::Strict if index != 0 => {
                            let open = iterator_helper_take_zip_open(vm, idx)?;
                            return close_iterator_records_after_error(
                                vm,
                                &open,
                                Error::type_err("Iterator.zip inputs have different lengths"),
                            );
                        }
                        IteratorZipMode::Strict => {
                            for remaining_index in 1..iter_count {
                                let Some(remaining) =
                                    iterator_helper_zip_record(vm, idx, remaining_index)
                                else {
                                    continue;
                                };
                                let remaining_pin = vm.pin_many(&[
                                    remaining.iterator.clone(),
                                    remaining.next_method.clone(),
                                ]);
                                let remaining_step = iterator_helper_step(
                                    vm,
                                    &remaining.iterator,
                                    &remaining.next_method,
                                    false,
                                );
                                vm.unpin_many(remaining_pin);
                                match remaining_step {
                                    Ok(None) => {
                                        iterator_helper_zip_mark_done(vm, idx, remaining_index)
                                    }
                                    Err(error) => {
                                        iterator_helper_zip_mark_done(vm, idx, remaining_index);
                                        let open = iterator_helper_take_zip_open(vm, idx)?;
                                        return close_iterator_records_after_error(
                                            vm, &open, error,
                                        );
                                    }
                                    Ok(Some(_)) => {
                                        let open = iterator_helper_take_zip_open(vm, idx)?;
                                        return close_iterator_records_after_error(
                                            vm,
                                            &open,
                                            Error::type_err(
                                                "Iterator.zip inputs have different lengths",
                                            ),
                                        );
                                    }
                                }
                            }
                            return Ok(None);
                        }
                    }
                }
            }
        }
        match kind {
            IteratorHelperKind::Zip => {
                create_array_from_values_in_realm(vm, values, realm).map(Some)
            }
            IteratorHelperKind::ZipKeyed => {
                let keys = iterator_helper_zip_keys(vm, idx);
                create_keyed_object_from_values(vm, &keys, values).map(Some)
            }
            _ => Err(Error::internal("invalid Iterator zip helper kind")),
        }
    })();
    vm.unpin_many(value_pins);
    result
}

fn iterator_helper_next(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err(
            "Iterator Helper next called on incompatible receiver",
        ));
    };
    let record = vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return None;
        };
        let state = helper.state.load(Ordering::Relaxed);
        if matches!(state, 0 | 3) {
            helper.state.store(1, Ordering::Relaxed);
        }
        Some((
            state,
            helper.resume_realm,
            helper.iterator.clone(),
            helper.next_method.clone(),
            helper.callback.clone(),
            helper.kind,
        ))
    });
    let Some((state, realm, iterator, next_method, callback, kind)) = record else {
        return Err(Error::type_err(
            "Iterator Helper next called on incompatible receiver",
        ));
    };
    if state == 1 {
        return Err(Error::type_err("Iterator Helper is already running"));
    }
    if state == 2 {
        return create_iterator_result(vm, Value::Undefined, true);
    }

    let result = (|| -> error::Result<Value> {
        loop {
            let yielded = match kind {
                IteratorHelperKind::Take => {
                    if !iterator_helper_consume_remaining(vm, idx) {
                        close_iterator_normally(vm, &iterator)?;
                        iterator_helper_set_state(vm, idx, 2);
                        return create_iterator_result(vm, Value::Undefined, true);
                    }
                    iterator_helper_step(vm, &iterator, &next_method, true)?
                }
                IteratorHelperKind::Drop => {
                    while iterator_helper_consume_remaining(vm, idx) {
                        if iterator_helper_step(vm, &iterator, &next_method, false)?.is_none() {
                            iterator_helper_set_state(vm, idx, 2);
                            return create_iterator_result(vm, Value::Undefined, true);
                        }
                    }
                    iterator_helper_step(vm, &iterator, &next_method, true)?
                }
                IteratorHelperKind::Map | IteratorHelperKind::Filter => {
                    let Some(value) = iterator_helper_step(vm, &iterator, &next_method, true)?
                    else {
                        iterator_helper_set_state(vm, idx, 2);
                        return create_iterator_result(vm, Value::Undefined, true);
                    };
                    let callback = callback.as_ref().ok_or_else(|| {
                        Error::internal("Iterator callback helper is missing its callback")
                    })?;
                    let callback_result = vm.call_function(
                        callback,
                        &[
                            value.clone(),
                            Value::Number(iterator_helper_counter_number(vm, idx)),
                        ],
                        Some(Value::Undefined),
                    );
                    let selected = match callback_result {
                        Ok(result) => result,
                        Err(error) => return close_iterator_after_error(vm, &iterator, error),
                    };
                    iterator_helper_increment_counter(vm, idx);
                    if kind == IteratorHelperKind::Map {
                        Some(selected)
                    } else if vm.to_boolean(&selected) {
                        Some(value)
                    } else {
                        None
                    }
                }
                IteratorHelperKind::FlatMap => loop {
                    if let Some((inner_iterator, inner_next)) = iterator_helper_inner(vm, idx) {
                        match iterator_helper_step(vm, &inner_iterator, &inner_next, true) {
                            Ok(Some(value)) => break Some(value),
                            Ok(None) => {
                                iterator_helper_set_inner(vm, idx, None);
                                iterator_helper_increment_counter(vm, idx);
                                continue;
                            }
                            Err(error) => {
                                iterator_helper_set_inner(vm, idx, None);
                                return close_iterator_after_error(vm, &iterator, error);
                            }
                        }
                    }

                    let Some(value) = iterator_helper_step(vm, &iterator, &next_method, true)?
                    else {
                        iterator_helper_set_state(vm, idx, 2);
                        return create_iterator_result(vm, Value::Undefined, true);
                    };
                    let callback = callback.as_ref().ok_or_else(|| {
                        Error::internal("Iterator flatMap helper is missing its mapper")
                    })?;
                    let mapped = match vm.call_function(
                        callback,
                        &[
                            value,
                            Value::Number(iterator_helper_counter_number(vm, idx)),
                        ],
                        Some(Value::Undefined),
                    ) {
                        Ok(mapped) => mapped,
                        Err(error) => return close_iterator_after_error(vm, &iterator, error),
                    };
                    let inner = match get_iterator_flattenable_reject_primitives(
                        vm,
                        mapped,
                        "Iterator.prototype.flatMap mapper must return an object",
                    ) {
                        Ok(inner) => inner,
                        Err(error) => return close_iterator_after_error(vm, &iterator, error),
                    };
                    iterator_helper_set_inner(vm, idx, Some(inner));
                },
                IteratorHelperKind::Concat => loop {
                    if let Some((inner_iterator, inner_next)) = iterator_helper_inner(vm, idx) {
                        match iterator_helper_step(vm, &inner_iterator, &inner_next, true)? {
                            Some(value) => break Some(value),
                            None => {
                                iterator_helper_set_inner(vm, idx, None);
                                iterator_helper_advance_concat(vm, idx);
                                continue;
                            }
                        }
                    }

                    let Some((iterable, open_method)) = iterator_helper_concat_record(vm, idx)
                    else {
                        iterator_helper_set_state(vm, idx, 2);
                        return create_iterator_result(vm, Value::Undefined, true);
                    };
                    let inner = iterator_concat_open_inner(vm, iterable, open_method)?;
                    iterator_helper_set_inner(vm, idx, Some(inner));
                },
                IteratorHelperKind::Zip | IteratorHelperKind::ZipKeyed => {
                    match iterator_zip_step(vm, idx, realm)? {
                        Some(values) => Some(values),
                        None => {
                            iterator_helper_set_state(vm, idx, 2);
                            return create_iterator_result(vm, Value::Undefined, true);
                        }
                    }
                }
            };
            if let Some(yielded) = yielded {
                let result = create_iterator_result_in_realm(vm, yielded, false, realm)?;
                iterator_helper_set_state(vm, idx, 3);
                return Ok(result);
            } else if matches!(kind, IteratorHelperKind::Take | IteratorHelperKind::Drop) {
                iterator_helper_set_state(vm, idx, 2);
                return create_iterator_result(vm, Value::Undefined, true);
            }
        }
    })();
    if result.is_err() {
        if kind == IteratorHelperKind::Concat {
            iterator_helper_set_inner(vm, idx, None);
        }
        iterator_helper_set_state(vm, idx, 2);
    }
    result.map_err(|error| vm.materialize_error_in_realm(error, realm))
}

fn iterator_helper_return(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err(
            "Iterator Helper return called on incompatible receiver",
        ));
    };
    let record = vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::IteratorHelper(helper) = obj else {
            return None;
        };
        let previous = helper.state.load(Ordering::Relaxed);
        match previous {
            0 => helper.state.store(2, Ordering::Relaxed),
            3 => helper.state.store(1, Ordering::Relaxed),
            _ => {}
        }
        let inner = if matches!(previous, 0 | 3) {
            helper
                .inner_iterator
                .lock()
                .take()
                .map(|inner| (inner.iterator, inner.next_method))
        } else {
            None
        };
        Some((
            previous,
            helper.resume_realm,
            helper.iterator.clone(),
            inner,
            helper.kind,
        ))
    });
    let Some((previous, realm, iterator, inner, kind)) = record else {
        return Err(Error::type_err(
            "Iterator Helper return called on incompatible receiver",
        ));
    };
    if previous == 1 {
        return Err(Error::type_err("Iterator Helper is already running"));
    }
    if previous == 2 {
        return create_iterator_result(vm, Value::Undefined, true);
    }
    if previous == 0 && kind == IteratorHelperKind::Concat {
        return create_iterator_result(vm, Value::Undefined, true);
    }
    let zip_open = if matches!(kind, IteratorHelperKind::Zip | IteratorHelperKind::ZipKeyed) {
        match iterator_helper_take_zip_open(vm, idx) {
            Ok(open) => open,
            Err(error) => {
                iterator_helper_set_state(vm, idx, 2);
                return Err(error);
            }
        }
    } else {
        Vec::new()
    };
    let inner_pin = inner
        .as_ref()
        .map(|(inner_iterator, inner_next)| {
            vm.pin_many(&[inner_iterator.clone(), inner_next.clone()])
        })
        .unwrap_or(0);
    let close_result = if matches!(kind, IteratorHelperKind::Zip | IteratorHelperKind::ZipKeyed) {
        close_iterator_records(vm, &zip_open, Ok(()))
    } else if kind == IteratorHelperKind::Concat {
        if let Some((inner_iterator, _)) = inner {
            close_iterator_normally(vm, &inner_iterator)
        } else {
            Ok(())
        }
    } else if let Some((inner_iterator, _)) = inner {
        match close_iterator_normally(vm, &inner_iterator) {
            Ok(()) => close_iterator_normally(vm, &iterator),
            Err(error) => close_iterator_after_error(vm, &iterator, error),
        }
    } else {
        close_iterator_normally(vm, &iterator)
    };
    vm.unpin_many(inner_pin);
    iterator_helper_set_state(vm, idx, 2);
    if previous == 0 {
        close_result?;
        return create_iterator_result(vm, Value::Undefined, true);
    }
    match close_result {
        Ok(()) => create_iterator_result(vm, Value::Undefined, true),
        Err(error) => Err(vm.materialize_error_in_realm(error, realm)),
    }
}

fn iterator_reduce(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let Some(iterator @ Value::Object(_)) = this else {
        return Err(Error::type_err(
            "Iterator.prototype.reduce requires an object",
        ));
    };
    let reducer = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&reducer, &vm.heap) {
        return close_iterator_after_error(
            vm,
            &iterator,
            Error::type_err("Iterator.prototype.reduce reducer is not callable"),
        );
    }

    let mut base_pins = vm.pin_many(&[iterator.clone(), reducer.clone()]);
    let mut accumulator_pin = 0;
    let result = (|| -> error::Result<Value> {
        let next_method = vm.get_property(&iterator, "next")?;
        base_pins += vm.pin(&next_method);

        let (mut accumulator, mut counter) = if args.len() > 1 {
            (args[1].clone(), BigUint::zero())
        } else {
            let Some(value) = iterator_helper_step(vm, &iterator, &next_method, true)? else {
                return Err(Error::type_err(
                    "Iterator.prototype.reduce requires an initial value for an empty iterator",
                ));
            };
            (value, BigUint::from(1u8))
        };
        accumulator_pin = vm.pin(&accumulator);

        loop {
            let Some(value) = iterator_helper_step(vm, &iterator, &next_method, true)? else {
                return Ok(accumulator);
            };
            let value_pin = vm.pin(&value);
            let reduced = match vm.call_function(
                &reducer,
                &[
                    accumulator.clone(),
                    value,
                    Value::Number(counter.to_f64().unwrap_or(f64::INFINITY)),
                ],
                Some(Value::Undefined),
            ) {
                Ok(reduced) => reduced,
                Err(error) => {
                    let outcome = close_iterator_after_error(vm, &iterator, error);
                    vm.unpin_many(value_pin);
                    return outcome;
                }
            };
            let reduced_pin = vm.pin(&reduced);
            vm.unpin_many(reduced_pin + value_pin + accumulator_pin);
            accumulator = reduced;
            accumulator_pin = vm.pin(&accumulator);
            counter += 1u8;
        }
    })();
    vm.unpin_many(accumulator_pin + base_pins);
    result
}

fn iterator_for_each(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let Some(iterator @ Value::Object(_)) = this else {
        return Err(Error::type_err(
            "Iterator.prototype.forEach requires an object",
        ));
    };
    let procedure = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&procedure, &vm.heap) {
        return close_iterator_after_error(
            vm,
            &iterator,
            Error::type_err("Iterator.prototype.forEach callback is not callable"),
        );
    }

    let mut pin_count = vm.pin_many(&[iterator.clone(), procedure.clone()]);
    let result = (|| -> error::Result<Value> {
        let next_method = vm.get_property(&iterator, "next")?;
        pin_count += vm.pin(&next_method);
        let mut counter = BigUint::zero();
        loop {
            let Some(value) = iterator_helper_step(vm, &iterator, &next_method, true)? else {
                return Ok(Value::Undefined);
            };
            let value_pin = vm.pin(&value);
            let call_result = vm.call_function(
                &procedure,
                &[
                    value,
                    Value::Number(counter.to_f64().unwrap_or(f64::INFINITY)),
                ],
                Some(Value::Undefined),
            );
            match call_result {
                Ok(_) => vm.unpin_many(value_pin),
                Err(error) => {
                    let outcome = close_iterator_after_error(vm, &iterator, error);
                    vm.unpin_many(value_pin);
                    return outcome;
                }
            }
            counter += 1u8;
        }
    })();
    vm.unpin_many(pin_count);
    result
}

fn iterator_some(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let Some(iterator @ Value::Object(_)) = this else {
        return Err(Error::type_err(
            "Iterator.prototype.some requires an object",
        ));
    };
    let predicate = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&predicate, &vm.heap) {
        return close_iterator_after_error(
            vm,
            &iterator,
            Error::type_err("Iterator.prototype.some predicate is not callable"),
        );
    }

    let mut pin_count = vm.pin_many(&[iterator.clone(), predicate.clone()]);
    let result = (|| -> error::Result<Value> {
        let next_method = vm.get_property(&iterator, "next")?;
        pin_count += vm.pin(&next_method);
        let mut counter = BigUint::zero();
        loop {
            let Some(value) = iterator_helper_step(vm, &iterator, &next_method, true)? else {
                return Ok(Value::Bool(false));
            };
            let value_pin = vm.pin(&value);
            let selected = match vm.call_function(
                &predicate,
                &[
                    value,
                    Value::Number(counter.to_f64().unwrap_or(f64::INFINITY)),
                ],
                Some(Value::Undefined),
            ) {
                Ok(selected) => selected,
                Err(error) => {
                    let outcome = close_iterator_after_error(vm, &iterator, error);
                    vm.unpin_many(value_pin);
                    return outcome;
                }
            };
            if vm.to_boolean(&selected) {
                let close_result = close_iterator_normally(vm, &iterator);
                vm.unpin_many(value_pin);
                close_result?;
                return Ok(Value::Bool(true));
            }
            vm.unpin_many(value_pin);
            counter += 1u8;
        }
    })();
    vm.unpin_many(pin_count);
    result
}

fn iterator_every(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let Some(iterator @ Value::Object(_)) = this else {
        return Err(Error::type_err(
            "Iterator.prototype.every requires an object",
        ));
    };
    let predicate = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&predicate, &vm.heap) {
        return close_iterator_after_error(
            vm,
            &iterator,
            Error::type_err("Iterator.prototype.every predicate is not callable"),
        );
    }

    let mut pin_count = vm.pin_many(&[iterator.clone(), predicate.clone()]);
    let result = (|| -> error::Result<Value> {
        let next_method = vm.get_property(&iterator, "next")?;
        pin_count += vm.pin(&next_method);
        let mut counter = BigUint::zero();
        loop {
            let Some(value) = iterator_helper_step(vm, &iterator, &next_method, true)? else {
                return Ok(Value::Bool(true));
            };
            let value_pin = vm.pin(&value);
            let selected = match vm.call_function(
                &predicate,
                &[
                    value,
                    Value::Number(counter.to_f64().unwrap_or(f64::INFINITY)),
                ],
                Some(Value::Undefined),
            ) {
                Ok(selected) => selected,
                Err(error) => {
                    let outcome = close_iterator_after_error(vm, &iterator, error);
                    vm.unpin_many(value_pin);
                    return outcome;
                }
            };
            if !vm.to_boolean(&selected) {
                let close_result = close_iterator_normally(vm, &iterator);
                vm.unpin_many(value_pin);
                close_result?;
                return Ok(Value::Bool(false));
            }
            vm.unpin_many(value_pin);
            counter += 1u8;
        }
    })();
    vm.unpin_many(pin_count);
    result
}

fn iterator_find(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let Some(iterator @ Value::Object(_)) = this else {
        return Err(Error::type_err(
            "Iterator.prototype.find requires an object",
        ));
    };
    let predicate = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&predicate, &vm.heap) {
        return close_iterator_after_error(
            vm,
            &iterator,
            Error::type_err("Iterator.prototype.find predicate is not callable"),
        );
    }

    let mut pin_count = vm.pin_many(&[iterator.clone(), predicate.clone()]);
    let result = (|| -> error::Result<Value> {
        let next_method = vm.get_property(&iterator, "next")?;
        pin_count += vm.pin(&next_method);
        let mut counter = BigUint::zero();
        loop {
            let Some(value) = iterator_helper_step(vm, &iterator, &next_method, true)? else {
                return Ok(Value::Undefined);
            };
            let value_pin = vm.pin(&value);
            let selected = match vm.call_function(
                &predicate,
                &[
                    value.clone(),
                    Value::Number(counter.to_f64().unwrap_or(f64::INFINITY)),
                ],
                Some(Value::Undefined),
            ) {
                Ok(selected) => selected,
                Err(error) => {
                    let outcome = close_iterator_after_error(vm, &iterator, error);
                    vm.unpin_many(value_pin);
                    return outcome;
                }
            };
            if vm.to_boolean(&selected) {
                let close_result = close_iterator_normally(vm, &iterator);
                vm.unpin_many(value_pin);
                close_result?;
                return Ok(value);
            }
            vm.unpin_many(value_pin);
            counter += 1u8;
        }
    })();
    vm.unpin_many(pin_count);
    result
}

fn iterator_to_array(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    const MAX_ITERATOR_TO_ARRAY_LEN: usize = 1 << 16;

    let Some(iterator @ Value::Object(_)) = this else {
        return Err(Error::type_err(
            "Iterator.prototype.toArray requires an object",
        ));
    };
    let mut pin_count = vm.pin(&iterator);
    let result = (|| -> error::Result<Value> {
        let next = vm.get_property(&iterator, "next")?;
        pin_count += vm.pin(&next);
        let mut values = Vec::new();
        loop {
            let result = vm.call_function(&next, &[], Some(iterator.clone()))?;
            if !matches!(result, Value::Object(_)) {
                return Err(Error::type_err("Iterator result is not an object"));
            }
            let result_pin = vm.pin(&result);
            let done_result = (|| -> error::Result<Option<Value>> {
                let done = vm.get_property(&result, "done")?;
                if vm.to_boolean(&done) {
                    return Ok(None);
                }
                Ok(Some(vm.get_property(&result, "value")?))
            })();
            vm.unpin_many(result_pin);
            let Some(value) = done_result? else {
                break;
            };
            if values.len() >= MAX_ITERATOR_TO_ARRAY_LEN {
                if let Ok(return_method) = vm.get_property(&iterator, "return") {
                    if is_callable(&return_method, &vm.heap) {
                        let return_pin = vm.pin(&return_method);
                        let _ = vm.call_function(&return_method, &[], Some(iterator.clone()));
                        vm.unpin_many(return_pin);
                    }
                }
                return Err(Error::range("Invalid array length"));
            }
            pin_count += vm.pin(&value);
            values.push(value);
        }
        let realm = active_iterator_realm(vm);
        let prototype = vm
            .realm_array_prototypes
            .get(&realm.0)
            .cloned()
            .ok_or_else(|| Error::internal("missing Array prototype intrinsic"))?;
        vm.alloc(HeapObj::Array(ArrayData::new(values, Some(prototype))))
            .map(Value::Object)
    })();
    vm.unpin_many(pin_count);
    result
}

fn iterator_dispose(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let return_method = vm.get_property(&receiver, "return")?;
    if !return_method.is_nullish() {
        vm.call_function(&return_method, &[], Some(receiver))?;
    }
    Ok(Value::Undefined)
}

fn async_iterator_dispose_unwrap(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Undefined)
}

fn reject_async_iterator_dispose(
    vm: &mut Vm,
    capability: &collections::PromiseCapability,
    error: &Arc<Error>,
    realm: GcIdx,
) -> error::Result<()> {
    let reason = vm.promise_rejection_reason_in_realm(error, realm)?;
    let pins = vm.pin_many(&[
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
        reason.clone(),
    ]);
    let result = vm.call_function(
        &capability.reject,
        std::slice::from_ref(&reason),
        Some(Value::Undefined),
    );
    vm.unpin_many(pins);
    result.map(|_| ())
}

fn async_iterator_dispose(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    let realm = env::global_env_root(&vm.heap, realm);
    let constructor = vm.promise_constructor_for_env(realm);
    let capability = new_promise_capability_in_env(vm, constructor, realm)?;
    let promise = capability.promise.clone();
    let receiver = this.unwrap_or(Value::Undefined);
    let pins = vm.pin_many(&[
        receiver.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);

    let result = (|| -> error::Result<()> {
        let return_method = match vm.get_property(&receiver, "return") {
            Ok(method) => method,
            Err(error) => {
                reject_async_iterator_dispose(vm, &capability, &error, realm)?;
                return Ok(());
            }
        };
        if return_method.is_nullish() {
            vm.call_function(
                &capability.resolve,
                &[Value::Undefined],
                Some(Value::Undefined),
            )?;
            return Ok(());
        }
        if !is_callable(&return_method, &vm.heap) {
            let error = Error::type_err("Async iterator return method is not callable");
            reject_async_iterator_dispose(vm, &capability, &error, realm)?;
            return Ok(());
        }

        let return_pin = vm.pin(&return_method);
        let returned = vm.call_function(&return_method, &[], Some(receiver.clone()));
        vm.unpin(return_pin);
        let returned = match returned {
            Ok(value) => value,
            Err(error) => {
                reject_async_iterator_dispose(vm, &capability, &error, realm)?;
                return Ok(());
            }
        };
        let returned_pin = vm.pin(&returned);
        let wrapper = vm.promise_resolve_intrinsic_in_env(returned, realm);
        vm.unpin(returned_pin);
        let wrapper = match wrapper {
            Ok(wrapper) => wrapper,
            Err(error) => {
                reject_async_iterator_dispose(vm, &capability, &error, realm)?;
                return Ok(());
            }
        };
        let wrapper_pin = vm.pin(&Value::Object(wrapper));
        let attach_result = (|| -> error::Result<()> {
            let unwrap =
                vm.new_native_function_in_env("", async_iterator_dispose_unwrap, 1, realm)?;
            let handler = crate::value::PromiseHandler {
                on_fulfilled: Value::Object(unwrap),
                on_rejected: Value::Undefined,
                derived: Some(crate::value::PromiseReactionCapability {
                    promise: capability.promise.clone(),
                    resolve: capability.resolve.clone(),
                    reject: capability.reject.clone(),
                }),
                continuation: None,
            };
            let state = vm.heap.with_obj(wrapper.0, |object| {
                if let HeapObj::Promise(data) = object {
                    *data.state.lock()
                } else {
                    crate::value::PromiseStatus::Fulfilled
                }
            });
            if state == crate::value::PromiseStatus::Pending {
                vm.heap.with_obj(wrapper.0, |object| {
                    if let HeapObj::Promise(data) = object {
                        data.handlers.lock().push(handler);
                    }
                });
            } else {
                let realm = match state {
                    crate::value::PromiseStatus::Fulfilled => {
                        vm.promise_reaction_job_realm(&handler.on_fulfilled)?
                    }
                    crate::value::PromiseStatus::Rejected => {
                        vm.promise_reaction_job_realm(&handler.on_rejected)?
                    }
                    crate::value::PromiseStatus::Pending => None,
                };
                vm.microtask_queue.push_back(crate::vm::Microtask::Then {
                    promise: wrapper,
                    on_fulfilled: handler.on_fulfilled,
                    on_rejected: handler.on_rejected,
                    derived: handler.derived,
                    continuation: None,
                    realm,
                });
            }
            Ok(())
        })();
        vm.unpin(wrapper_pin);
        attach_result?;
        Ok(())
    })();
    vm.unpin_many(pins);
    result?;
    Ok(promise)
}

fn iterator_constructor_get(
    vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let realm = active_iterator_realm(vm);
    vm.realm_iterator_constructors
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Iterator intrinsic"))
}

fn iterator_to_string_tag_get(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::String(Arc::from("Iterator")))
}

fn iterator_proto_set_property(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    key: PropertyKey,
) -> error::Result<Value> {
    let Some(receiver @ Value::Object(_)) = this else {
        return Err(Error::type_err(
            "Iterator prototype setter requires an object",
        ));
    };
    let realm = active_iterator_realm(vm);
    let home = vm
        .realm_iterator_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Iterator prototype intrinsic"))?;
    if receiver == home {
        return Err(Error::type_err(
            "Cannot assign to Iterator prototype intrinsic",
        ));
    }
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    if own_property_descriptor_for_key_or_throw(vm, &receiver, &key)?.is_none() {
        vm.define_own_property_or_throw(&receiver, key, PropertyDescriptor::data(value))?;
    } else if !vm.try_set_property_key_with_receiver(&receiver, &key, value, &receiver)? {
        return Err(Error::type_err("Cannot assign Iterator prototype property"));
    }
    Ok(Value::Undefined)
}

fn iterator_constructor_set(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    iterator_proto_set_property(vm, args, this, PropertyKey::from("constructor"))
}

fn iterator_to_string_tag_set(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    iterator_proto_set_property(
        vm,
        args,
        this,
        PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
    )
}

fn install_iterator_intrinsic_in_env(
    vm: &mut Vm,
    realm: GcIdx,
    realm_global: Option<&Value>,
    object_proto: Value,
) -> error::Result<Value> {
    let prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(object_proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Iterator")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?);
    let mut pin_count = vm.pin(&prototype);

    let iterator_fn =
        vm.new_native_function_in_env("[Symbol.iterator]", iterator_identity, 0, realm)?;
    pin_count += vm.pin(&Value::Object(iterator_fn));
    let dispose_fn =
        vm.new_native_function_in_env("[Symbol.dispose]", iterator_dispose, 0, realm)?;
    pin_count += vm.pin(&Value::Object(dispose_fn));
    let constructor_get =
        vm.new_native_function_in_env("get constructor", iterator_constructor_get, 0, realm)?;
    pin_count += vm.pin(&Value::Object(constructor_get));
    let constructor_set =
        vm.new_native_function_in_env("set constructor", iterator_constructor_set, 1, realm)?;
    pin_count += vm.pin(&Value::Object(constructor_set));
    let tag_get = vm.new_native_function_in_env(
        "get [Symbol.toStringTag]",
        iterator_to_string_tag_get,
        0,
        realm,
    )?;
    pin_count += vm.pin(&Value::Object(tag_get));
    let tag_set = vm.new_native_function_in_env(
        "set [Symbol.toStringTag]",
        iterator_to_string_tag_set,
        1,
        realm,
    )?;
    pin_count += vm.pin(&Value::Object(tag_set));
    let wrapper_next =
        vm.new_native_function_in_env("next", valid_iterator_wrapper_next, 0, realm)?;
    pin_count += vm.pin(&Value::Object(wrapper_next));
    let wrapper_return =
        vm.new_native_function_in_env("return", valid_iterator_wrapper_return, 0, realm)?;
    pin_count += vm.pin(&Value::Object(wrapper_return));
    let wrapper_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?);
    pin_count += vm.pin(&wrapper_prototype);
    if let Value::Object(wrapper_idx) = &wrapper_prototype {
        vm.heap.with_obj(wrapper_idx.0, |obj| {
            let mut props = obj.props().lock();
            props.insert(
                PropertyKey::from("next"),
                data_prop(Value::Object(wrapper_next)),
            );
            props.insert(
                PropertyKey::from("return"),
                data_prop(Value::Object(wrapper_return)),
            );
        });
    }
    let helper_next = vm.new_native_function_in_env("next", iterator_helper_next, 0, realm)?;
    pin_count += vm.pin(&Value::Object(helper_next));
    let helper_return =
        vm.new_native_function_in_env("return", iterator_helper_return, 0, realm)?;
    pin_count += vm.pin(&Value::Object(helper_return));
    let mut helper_tag = data_prop(Value::String(Arc::from("Iterator Helper")));
    helper_tag.writable = false;
    let helper_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::from([
            (
                PropertyKey::from("next"),
                data_prop(Value::Object(helper_next)),
            ),
            (
                PropertyKey::from("return"),
                data_prop(Value::Object(helper_return)),
            ),
            (
                PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
                helper_tag,
            ),
        ])),
        proto: Mutex::new(Some(prototype.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?);
    pin_count += vm.pin(&helper_prototype);
    let from = vm.new_native_function_in_env("from", iterator_from, 1, realm)?;
    pin_count += vm.pin(&Value::Object(from));
    let map = vm.new_native_function_in_env("map", iterator_map, 1, realm)?;
    pin_count += vm.pin(&Value::Object(map));
    let filter = vm.new_native_function_in_env("filter", iterator_filter, 1, realm)?;
    pin_count += vm.pin(&Value::Object(filter));
    let flat_map = vm.new_native_function_in_env("flatMap", iterator_flat_map, 1, realm)?;
    pin_count += vm.pin(&Value::Object(flat_map));
    let take = vm.new_native_function_in_env("take", iterator_take, 1, realm)?;
    pin_count += vm.pin(&Value::Object(take));
    let drop = vm.new_native_function_in_env("drop", iterator_drop, 1, realm)?;
    pin_count += vm.pin(&Value::Object(drop));
    let reduce = vm.new_native_function_in_env("reduce", iterator_reduce, 1, realm)?;
    pin_count += vm.pin(&Value::Object(reduce));
    let for_each = vm.new_native_function_in_env("forEach", iterator_for_each, 1, realm)?;
    pin_count += vm.pin(&Value::Object(for_each));
    let some = vm.new_native_function_in_env("some", iterator_some, 1, realm)?;
    pin_count += vm.pin(&Value::Object(some));
    let every = vm.new_native_function_in_env("every", iterator_every, 1, realm)?;
    pin_count += vm.pin(&Value::Object(every));
    let find = vm.new_native_function_in_env("find", iterator_find, 1, realm)?;
    pin_count += vm.pin(&Value::Object(find));
    let to_array = vm.new_native_function_in_env("toArray", iterator_to_array, 0, realm)?;
    pin_count += vm.pin(&Value::Object(to_array));
    let concat = vm.new_native_function_in_env("concat", iterator_concat, 0, realm)?;
    pin_count += vm.pin(&Value::Object(concat));
    let zip = vm.new_native_function_in_env("zip", iterator_zip, 1, realm)?;
    pin_count += vm.pin(&Value::Object(zip));
    let zip_keyed = vm.new_native_function_in_env("zipKeyed", iterator_zip_keyed, 1, realm)?;
    pin_count += vm.pin(&Value::Object(zip_keyed));
    let constructor = vm.new_native_constructor_in_env(
        "Iterator",
        iterator_constructor,
        0,
        realm,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    let constructor_value = Value::Object(constructor);
    pin_count += vm.pin(&constructor_value);

    vm.heap.with_obj(constructor.0, |obj| {
        if let HeapObj::Function(function) = obj {
            *function.prototype.lock() = Some(prototype.clone());
        }
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(prototype.clone()),
        );
        obj.props()
            .lock()
            .insert(PropertyKey::from("from"), data_prop(Value::Object(from)));
        obj.props().lock().insert(
            PropertyKey::from("concat"),
            data_prop(Value::Object(concat)),
        );
        obj.props()
            .lock()
            .insert(PropertyKey::from("zip"), data_prop(Value::Object(zip)));
        obj.props().lock().insert(
            PropertyKey::from("zipKeyed"),
            data_prop(Value::Object(zip_keyed)),
        );
    });
    if let Value::Object(prototype_idx) = &prototype {
        vm.heap.with_obj(prototype_idx.0, |obj| {
            let mut props = obj.props().lock();
            props.insert(
                PropertyKey::from("constructor"),
                accessor_prop(
                    Value::Object(constructor_get),
                    Value::Object(constructor_set),
                ),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
                accessor_prop(Value::Object(tag_get), Value::Object(tag_set)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.iterator),
                data_prop(Value::Object(iterator_fn)),
            );
            props.insert(
                PropertyKey::Symbol(vm.well_known_symbols.dispose),
                data_prop(Value::Object(dispose_fn)),
            );
            props.insert(PropertyKey::from("map"), data_prop(Value::Object(map)));
            props.insert(
                PropertyKey::from("filter"),
                data_prop(Value::Object(filter)),
            );
            props.insert(
                PropertyKey::from("flatMap"),
                data_prop(Value::Object(flat_map)),
            );
            props.insert(PropertyKey::from("take"), data_prop(Value::Object(take)));
            props.insert(PropertyKey::from("drop"), data_prop(Value::Object(drop)));
            props.insert(
                PropertyKey::from("reduce"),
                data_prop(Value::Object(reduce)),
            );
            props.insert(
                PropertyKey::from("forEach"),
                data_prop(Value::Object(for_each)),
            );
            props.insert(PropertyKey::from("some"), data_prop(Value::Object(some)));
            props.insert(PropertyKey::from("every"), data_prop(Value::Object(every)));
            props.insert(PropertyKey::from("find"), data_prop(Value::Object(find)));
            props.insert(
                PropertyKey::from("toArray"),
                data_prop(Value::Object(to_array)),
            );
        });
    }

    vm.realm_iterator_constructors
        .insert(realm.0, constructor_value.clone());
    vm.realm_iterator_prototypes
        .insert(realm.0, prototype.clone());
    vm.realm_wrap_for_valid_iterator_prototypes
        .insert(realm.0, wrapper_prototype);
    vm.realm_iterator_helper_prototypes
        .insert(realm.0, helper_prototype);
    if realm == vm.global {
        vm.iterator_base_proto = prototype.clone();
        define_global(vm, "Iterator", constructor_value);
    } else if let Some(global) = realm_global {
        define_realm_global(vm, realm, global, "Iterator", constructor_value);
    }
    vm.unpin_many(pin_count);
    Ok(prototype)
}

pub fn setup_full(vm: &mut Vm) -> error::Result<()> {
    // Allocate Function.prototype first so that every function created during
    // the rest of bootstrap inherits call/apply/bind via its [[Prototype]].
    let function_proto_idx =
        vm.new_native_function("Function.prototype", function_proto_noop, 0)?;
    vm.function_proto = Value::Object(function_proto_idx);
    setup(vm)?;
    vm.register_realm_object_prototype(vm.global, vm.object_proto.clone());
    // Per spec, Function.prototype's [[Prototype]] is Object.prototype.
    // (Function.prototype is itself a function, but it inherits Object.prototype
    // methods like isPrototypeOf, hasOwnProperty, toString, etc.)
    vm.heap.with_obj(function_proto_idx.0, |obj| {
        *obj.proto().lock() = Some(vm.object_proto.clone());
    });
    init_global_this(vm)?;
    // Math
    let math = build_math_in_env(vm, vm.global, vm.object_proto.clone())?;
    define_global(vm, "Math", math);
    // console
    let console = build_console(vm)?;
    define_global(vm, "console", console);
    // JSON
    let json = build_json(vm)?;
    define_global(vm, "JSON", json);
    // Reflect
    let reflect = build_reflect(vm)?;
    define_global(vm, "Reflect", reflect);

    install_proxy_intrinsic_in_env(vm, vm.global, None)?;

    install_array_buffer_constructor_in_env(vm, vm.global, None, true)?;
    install_shared_array_buffer_constructor_in_env(vm, vm.global, None)?;
    install_data_view_constructor_in_env(vm, vm.global, None)?;
    let (typed_array_ctor, typed_array_proto) = make_typed_array_intrinsic_in_env(vm, vm.global)?;
    for (name, constructor, kind) in typed_array_constructor_entries() {
        install_typed_array_constructor(
            vm,
            name,
            constructor,
            kind,
            &typed_array_ctor,
            &typed_array_proto,
        )?;
    }
    install_atomics_in_env(vm, vm.global, None)?;
    install_date_intrinsic_in_env(vm, vm.global, None)?;
    let object_proto = vm.object_proto.clone();
    install_iterator_intrinsic_in_env(vm, vm.global, None, object_proto)?;
    setup_array_iterator_proto(vm)?;
    setup_regexp_string_iterator_proto(vm)?;
    // Array
    let (_, array_proto) = install_array_intrinsic_in_env(vm, vm.global, None)?;
    vm.array_proto = Value::Object(array_proto);
    vm.array_to_string_fn = vm.get_property(&vm.array_proto.clone(), "toString")?;
    install_typed_array_to_string_alias(vm, &typed_array_proto, vm.array_to_string_fn.clone());
    // String
    let (str_ctor, str_proto) = make_builtin_constructor_with(
        vm,
        "String",
        1,
        string_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("charAt", str_char_at, 1),
            ("charCodeAt", str_char_code_at, 1),
            ("indexOf", str_index_of, 1),
            ("lastIndexOf", str_last_index_of, 1),
            ("valueOf", string_value_of, 0),
            ("slice", str_slice, 2),
            ("toUpperCase", str_to_upper, 0),
            ("toLowerCase", str_to_lower, 0),
            ("toLocaleUpperCase", str_to_upper, 0),
            ("toLocaleLowerCase", str_to_lower, 0),
            ("localeCompare", str_locale_compare, 1),
            ("trim", str_trim, 0),
            ("split", str_split, 2),
            ("replace", str_replace, 2),
            ("includes", str_includes, 1),
            ("startsWith", str_starts_with, 1),
            ("endsWith", str_ends_with, 1),
            ("repeat", str_repeat, 1),
            ("match", str_match, 1),
            ("matchAll", str_match_all, 1),
            ("padStart", str_pad_start, 1),
            ("padEnd", str_pad_end, 1),
            ("at", str_at, 1),
            ("trimStart", str_trim_start, 0),
            ("trimEnd", str_trim_end, 0),
            ("replaceAll", str_replace_all, 2),
            ("normalize", str_normalize, 0),
            ("substring", str_substring, 2),
            ("substr", str_substr, 2),
            ("codePointAt", str_code_point_at, 1),
            ("isWellFormed", str_is_well_formed, 0),
            ("toWellFormed", str_to_well_formed, 0),
            ("concat", str_concat, 1),
            ("search", str_search, 1),
            ("toString", string_proto_to_string, 0),
        ],
    )?;
    vm.string_proto = Value::Object(str_proto);
    vm.set_primitive(&vm.string_proto.clone(), Value::String(Arc::from("")));
    vm.heap.with_obj(str_proto.0, |obj| {
        obj.props()
            .lock()
            .insert(PropertyKey::from("length"), const_prop(Value::Number(0.0)));
    });
    let string_proto = vm.string_proto.clone();
    let iterator_base_proto = vm.iterator_base_proto.clone();
    setup_string_iterator_proto_in_env(vm, vm.global, &string_proto, iterator_base_proto)?;
    define_global(vm, "String", Value::Object(str_ctor));
    // String static methods
    let raw_fn = vm.new_native_function("raw", string_raw, 1)?;
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props()
            .lock()
            .insert(PropertyKey::from("raw"), data_prop(Value::Object(raw_fn)));
    });
    let fcp_fn = vm.new_native_function("fromCodePoint", string_from_code_point, 1)?;
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("fromCodePoint"),
            data_prop(Value::Object(fcp_fn)),
        );
    });
    // String statics
    let from_char_code_fn = vm.new_native_function("fromCharCode", str_from_char_code, 1)?;
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("fromCharCode"),
            data_prop(Value::Object(from_char_code_fn)),
        );
    });
    // Number
    let (num_ctor, num_proto) = make_builtin_constructor_with(
        vm,
        "Number",
        1,
        number_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("toFixed", num_to_fixed, 1),
            ("toPrecision", num_to_precision, 1),
            ("toExponential", num_to_exponential, 1),
            ("toString", num_proto_to_string, 1),
            ("toLocaleString", num_proto_to_locale_string, 0),
            ("valueOf", number_value_of, 0),
        ],
    )?;
    vm.number_proto = Value::Object(num_proto);
    vm.set_primitive(&vm.number_proto.clone(), Value::Number(0.0));
    let parse_int_value = Value::Object(vm.new_native_function("parseInt", global_parse_int, 2)?);
    let parse_float_value =
        Value::Object(vm.new_native_function("parseFloat", global_parse_float, 1)?);
    // Number static methods + constants
    let statics: &[(&str, NativeFn, usize)] = &[
        ("isInteger", number_is_integer, 1),
        ("isFinite", number_is_finite, 1),
        ("isNaN", number_is_nan, 1),
        ("isSafeInteger", number_is_safe_integer, 1),
    ];
    let mut static_methods: Vec<(Arc<str>, Value)> = Vec::new();
    for (name, fnp, len) in statics {
        let idx = vm.new_native_function(name, *fnp, *len)?;
        static_methods.push((Arc::from(*name), Value::Object(idx)));
    }
    static_methods.push((Arc::from("parseInt"), parse_int_value.clone()));
    static_methods.push((Arc::from("parseFloat"), parse_float_value.clone()));
    let static_constants: Vec<(Arc<str>, Value)> = vec![
        (
            Arc::from("MAX_SAFE_INTEGER"),
            Value::Number(9007199254740991.0),
        ),
        (
            Arc::from("MIN_SAFE_INTEGER"),
            Value::Number(-9007199254740991.0),
        ),
        (Arc::from("EPSILON"), Value::Number(f64::EPSILON)),
        (Arc::from("MAX_VALUE"), Value::Number(f64::MAX)),
        (Arc::from("MIN_VALUE"), Value::Number(5e-324f64)),
        (Arc::from("POSITIVE_INFINITY"), Value::Number(f64::INFINITY)),
        (
            Arc::from("NEGATIVE_INFINITY"),
            Value::Number(f64::NEG_INFINITY),
        ),
        (Arc::from("NaN"), Value::Number(f64::NAN)),
    ];
    vm.heap.with_obj(num_ctor.0, |o| {
        if let HeapObj::Function(f) = o {
            for (name, val) in &static_methods {
                f.props
                    .lock()
                    .insert(PropertyKey::from(name.clone()), data_prop(val.clone()));
            }
            for (name, val) in &static_constants {
                f.props
                    .lock()
                    .insert(PropertyKey::from(name.clone()), const_prop(val.clone()));
            }
        }
    });
    define_global(vm, "Number", Value::Object(num_ctor));
    // Boolean
    let (bool_ctor, bool_proto) = make_builtin_constructor_with(
        vm,
        "Boolean",
        1,
        boolean_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("valueOf", boolean_value_of, 0),
            ("toString", boolean_to_string, 0),
        ],
    )?;
    vm.boolean_proto = Value::Object(bool_proto);
    vm.set_primitive(&vm.boolean_proto.clone(), Value::Bool(false));
    define_global(vm, "Boolean", Value::Object(bool_ctor));
    // globals
    define_global(vm, "parseInt", parse_int_value);
    define_global(vm, "parseFloat", parse_float_value);
    let idx = vm.new_native_function("isNaN", global_is_nan, 1)?;
    define_global(vm, "isNaN", Value::Object(idx));
    let idx = vm.new_native_function("isFinite", global_is_finite, 1)?;
    define_global(vm, "isFinite", Value::Object(idx));
    let eval_idx = vm.new_native_function("eval", global_eval, 1)?;
    let eval_value = Value::Object(eval_idx);
    vm.realm_eval_functions
        .insert(vm.global.0, eval_value.clone());
    define_global(vm, "eval", eval_value);
    for (name, func) in [
        ("decodeURI", global_decode_uri as NativeFn),
        (
            "decodeURIComponent",
            global_decode_uri_component as NativeFn,
        ),
        ("encodeURI", global_encode_uri as NativeFn),
        (
            "encodeURIComponent",
            global_encode_uri_component as NativeFn,
        ),
    ] {
        let idx = vm.new_native_function(name, func, 1)?;
        define_global(vm, name, Value::Object(idx));
    }
    define_global_const(vm, "NaN", Value::Number(f64::NAN));
    define_global_const(vm, "Infinity", Value::Number(f64::INFINITY));
    define_global_const(vm, "undefined", Value::Undefined);
    // BigInt has [[Construct]] for extends/newTarget checks, but its body
    // rejects construction before argument coercion.
    let bigint_idx = vm.new_native_constructor(
        "BigInt",
        global_bigint,
        1,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    let as_int_n = vm.new_native_function("asIntN", bigint_as_int_n, 2)?;
    let as_uint_n = vm.new_native_function("asUintN", bigint_as_uint_n, 2)?;
    vm.heap.with_obj(bigint_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            let mut props = f.props.lock();
            props.insert(
                PropertyKey::from("asIntN"),
                data_prop(Value::Object(as_int_n)),
            );
            props.insert(
                PropertyKey::from("asUintN"),
                data_prop(Value::Object(as_uint_n)),
            );
        }
    });
    define_global(vm, "BigInt", Value::Object(bigint_idx));
    // BigInt prototype with minimal members.
    {
        let bp_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("BigInt")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?;
        let bproto = Value::Object(GcIdx(bp_idx));
        vm.bigint_proto = bproto.clone();
        {
            let bi = bigint_idx;
            vm.heap.with_obj(bi.0, |obj| {
                if let HeapObj::Function(f) = obj {
                    *f.prototype.lock() = Some(bproto.clone());
                    f.props
                        .lock()
                        .insert(PropertyKey::from("prototype"), const_prop(bproto.clone()));
                }
            });
            let to_str = vm.new_native_function("toString", bigint_to_string, 0)?;
            let to_locale_str =
                vm.new_native_function("toLocaleString", bigint_proto_to_locale_string, 0)?;
            let value_of = vm.new_native_function("valueOf", bigint_value_of, 0)?;
            if let Value::Object(pi) = bproto {
                vm.heap.with_obj(pi.0, |obj| {
                    obj.props().lock().insert(
                        crate::value::PropertyKey::from("constructor"),
                        data_prop(Value::Object(bi)),
                    );
                    obj.props().lock().insert(
                        crate::value::PropertyKey::from("toString"),
                        data_prop(Value::Object(to_str)),
                    );
                    obj.props().lock().insert(
                        crate::value::PropertyKey::from("toLocaleString"),
                        data_prop(Value::Object(to_locale_str)),
                    );
                    let mut tag = data_prop(Value::String(Arc::from("BigInt")));
                    tag.writable = false;
                    obj.props().lock().insert(
                        PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
                        tag,
                    );
                    obj.props().lock().insert(
                        crate::value::PropertyKey::from("valueOf"),
                        data_prop(Value::Object(value_of)),
                    );
                });
            }
        }
    }
    // Promise
    let (promise_ctor, promise_proto) = install_promise_intrinsic_in_env(vm, vm.global, None)?;
    vm.promise_ctor = Value::Object(promise_ctor);
    vm.promise_proto = Value::Object(promise_proto);
    // RegExp
    let (regex_ctor, regex_proto) = make_regexp_constructor_in_env(vm, vm.global)?;
    vm.regexp_proto = Value::Object(regex_proto);
    vm.realm_regexp_constructors
        .insert(vm.global.0, Value::Object(regex_ctor));
    vm.realm_regexp_prototypes
        .insert(vm.global.0, vm.regexp_proto.clone());
    define_global(vm, "RegExp", Value::Object(regex_ctor));
    // Function constructor: new Function(p0, ..., body)
    let function_ctor_idx = vm.new_native_constructor(
        "Function",
        function_constructor,
        1,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    vm.heap.with_obj(function_ctor_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            f.prototype
                .lock()
                .replace(Value::Object(function_proto_idx));
        }
    });
    define_global(vm, "Function", Value::Object(function_ctor_idx));
    // %AsyncFunction% is not a global binding. Async function objects inherit
    // from its Realm's prototype, whose constructor exposes the intrinsic.
    let function_proto = vm.function_proto.clone();
    install_async_function_intrinsic(vm, vm.global, &function_proto, function_ctor_idx)?;
    let iterator_prototype = vm.iterator_base_proto.clone();
    let function_prototype = vm.function_proto.clone();
    let (generator_prototype, generator_function_prototype) = install_generator_intrinsics_in_env(
        vm,
        vm.global,
        iterator_prototype,
        function_prototype,
        function_ctor_idx,
    )?;
    vm.generator_proto = Value::Object(generator_prototype);
    vm.generator_function_proto = Value::Object(generator_function_prototype);
    let object_prototype = vm.object_proto.clone();
    let function_prototype = vm.function_proto.clone();
    let (async_iterator, async_generator, async_generator_function) =
        install_async_generator_intrinsics_in_env(
            vm,
            vm.global,
            object_prototype,
            function_prototype,
            function_ctor_idx,
        )?;
    vm.async_iterator_proto = Value::Object(async_iterator);
    vm.async_generator_proto = Value::Object(async_generator);
    vm.async_generator_function_proto = Value::Object(async_generator_function);
    // Install call/apply/bind on Function.prototype (allocated at the top of
    // setup_full) so every function inherits them via its [[Prototype]].
    let call_fn = vm.new_native_function("call", function_call, 1)?;
    let apply_fn = vm.new_native_function("apply", function_apply, 2)?;
    let bind_fn = vm.new_native_function("bind", function_bind, 1)?;
    let tostring_fn = vm.new_native_function("toString", function_to_string, 0)?;
    let has_instance_fn =
        vm.new_native_function("[Symbol.hasInstance]", function_symbol_has_instance, 1)?;
    let throw_type_error_fn = throw_type_error_intrinsic(vm, vm.global)?;
    vm.realm_function_prototypes
        .insert(vm.global.0, Value::Object(function_proto_idx));
    install_methods(
        vm,
        &Value::Object(function_proto_idx),
        &[
            (Arc::from("call"), Value::Object(call_fn)),
            (Arc::from("apply"), Value::Object(apply_fn)),
            (Arc::from("bind"), Value::Object(bind_fn)),
            (Arc::from("toString"), Value::Object(tostring_fn)),
        ],
    );
    // Function.prototype points to the function prototype object.
    vm.heap.with_obj(function_ctor_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(Value::Object(function_proto_idx)),
        );
    });
    // The function prototype's `constructor` is the Function constructor.
    vm.heap.with_obj(function_proto_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(function_ctor_idx)),
        );
        let mut has_instance_desc = PropertyDescriptor::data(Value::Object(has_instance_fn));
        has_instance_desc.writable = false;
        has_instance_desc.enumerable = false;
        has_instance_desc.configurable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.has_instance),
            has_instance_desc,
        );
        let restricted = PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: false,
            configurable: true,
            get: Some(throw_type_error_fn.clone()),
            set: Some(throw_type_error_fn),
            is_accessor: true,
        };
        props.insert(PropertyKey::from("caller"), restricted.clone());
        props.insert(PropertyKey::from("arguments"), restricted);
    });
    setup_collections(vm)?;
    install_weak_ref_constructor_in_env(vm, vm.global, None)?;
    install_finalization_registry_constructor_in_env(vm, vm.global, None)?;
    install_test262_host(vm)?;
    vm.realm_globals.insert(vm.global.0, vm.global_this.clone());
    for (kind, prototype) in [
        (
            crate::vm::PrimitivePrototypeKind::String,
            vm.string_proto.clone(),
        ),
        (
            crate::vm::PrimitivePrototypeKind::Number,
            vm.number_proto.clone(),
        ),
        (
            crate::vm::PrimitivePrototypeKind::BigInt,
            vm.bigint_proto.clone(),
        ),
        (
            crate::vm::PrimitivePrototypeKind::Boolean,
            vm.boolean_proto.clone(),
        ),
        (
            crate::vm::PrimitivePrototypeKind::Symbol,
            vm.symbol_proto.clone(),
        ),
    ] {
        vm.realm_primitive_prototypes
            .insert((vm.global.0, kind), prototype);
    }
    Ok(())
}

// =========================================================================

fn object_is_prototype_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    let arg = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(arg, Value::Object(_)) {
        return Ok(Value::Bool(false));
    }
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let this_obj = vm.to_object(&this)?;
    let this_pin = vm.pin(&this_obj);
    let result = (|| {
        let mut current = arg;
        loop {
            vm.consume_fuel()?;
            let current_pin = vm.pin(&current);
            let next = vm.get_prototype_of(&current);
            vm.unpin_many(current_pin);
            let Some(next) = next? else {
                return Ok(Value::Bool(false));
            };
            if vm.strict_eq(&next, &this_obj) {
                return Ok(Value::Bool(true));
            }
            current = next;
        }
    })();
    vm.unpin_many(this_pin);
    result
}

fn error_is_error(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let is_error = match args.first() {
        Some(Value::Object(idx)) => vm.heap.with_obj(idx.0, |obj| {
            matches!(obj, HeapObj::Object(data) if data.class_name.as_deref() == Some("Error"))
        }),
        _ => false,
    };
    Ok(Value::Bool(is_error))
}

fn error_has_error_data(vm: &Vm, value: &Value) -> bool {
    match value {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |obj| {
            matches!(obj, HeapObj::Object(data) if data.class_name.as_deref() == Some("Error"))
        }),
        _ => false,
    }
}

fn error_stack_home_prototype(vm: &mut Vm) -> Value {
    let env = vm.native_callee_closure().unwrap_or(vm.global);
    let Some(Value::Object(error_ctor)) = crate::environment::get(&vm.heap, env, "Error") else {
        return vm.error_proto.clone();
    };
    vm.heap
        .with_obj(error_ctor.0, |obj| {
            obj.props()
                .lock()
                .get(&PropertyKey::from("prototype"))
                .map(|desc| desc.value.clone())
        })
        .unwrap_or_else(|| vm.error_proto.clone())
}

fn error_stack_get(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if !this.is_object() {
        return Err(Error::type_err(
            "Error.prototype.stack getter called on non-object",
        ));
    }
    if !error_has_error_data(vm, &this) {
        return Ok(Value::Undefined);
    }
    Ok(Value::String(Arc::from("Error")))
}

fn error_stack_set(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if !this.is_object() {
        return Err(Error::type_err(
            "Error.prototype.stack setter called on non-object",
        ));
    }
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(value, Value::String(_)) {
        return Err(Error::type_err(
            "Error.prototype.stack setter requires a string",
        ));
    }
    if this == error_stack_home_prototype(vm) {
        return Err(Error::type_err("Cannot set Error.prototype.stack"));
    }

    let key = PropertyKey::from("stack");
    if own_property_descriptor_for_key_or_throw(vm, &this, &key)?.is_none() {
        let mut desc = PropertyDescriptor::data(value);
        desc.writable = true;
        desc.enumerable = true;
        desc.configurable = true;
        let desc_obj = from_property_descriptor(vm, desc)?;
        object_define_property_result(
            vm,
            &[this, Value::String(Arc::from("stack")), desc_obj],
            true,
        )?;
    } else {
        let success = vm.try_set_property_key_with_receiver(&this, &key, value, &this)?;
        if !success {
            return Err(Error::type_err("Cannot set Error.prototype.stack"));
        }
    }
    Ok(Value::Undefined)
}

fn error_to_string(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if !this.is_object() {
        return Err(Error::type_err(
            "Error.prototype.toString called on non-object",
        ));
    }
    let name = vm.get_property(&this, "name")?;
    let name_str = if name.is_undefined() {
        "Error".to_string()
    } else {
        vm.to_string(&name)?.to_string()
    };
    let msg = vm.get_property(&this, "message")?;
    let msg_str = if msg.is_undefined() {
        String::new()
    } else {
        vm.to_string(&msg)?.to_string()
    };
    if name_str.is_empty() {
        Ok(Value::String(Arc::from(msg_str)))
    } else if msg_str.is_empty() {
        Ok(Value::String(Arc::from(name_str)))
    } else {
        Ok(Value::String(Arc::from(format!(
            "{}: {}",
            name_str, msg_str
        ))))
    }
}
