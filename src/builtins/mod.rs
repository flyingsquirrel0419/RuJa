//! Built-in objects and globals for the RuJa VM.
//!
//! All built-in constructors, prototypes, and global functions are registered
//! here. Native functions follow the `NativeFn` signature used by the VM.

pub(crate) mod global;
pub(crate) mod intl;
mod intl_aliases;
mod intl_locale_info;
pub(crate) mod json;
pub(crate) mod math;
mod temporal;

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
    generator_function_constructor, global_bigint, global_escape, global_eval, global_is_finite,
    global_is_nan, global_parse_float, global_parse_int, global_unescape,
};
pub(crate) use json::{
    build_json, build_json_in_env, build_reflect, build_reflect_in_env, date_constructor,
    date_get_component, date_get_time, date_get_timezone_offset, date_now, date_parse,
    date_set_component, date_to_iso_string, date_to_json, date_to_primitive, date_to_string,
    date_to_temporal_instant, date_utc,
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
    PropertyDescriptor, PropertyKey, RegExpStringIteratorData, SetData, TemporalData,
    TemporalDurationFields, TemporalKind, TemporalPlainDateFields, TemporalPlainDateTimeFields,
    TemporalPlainMonthDayFields, TemporalPlainTimeFields, TemporalPlainYearMonthFields,
    TemporalTimeZone, TemporalTimeZoneKind, Value,
};
use crate::vm::{NativeFn, Vm};
use indexmap::{IndexMap, IndexSet};
use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_rational::Ratio;
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};
use regex::{Regex as RustRegex, RegexBuilder as RustRegexBuilder};
use regex_automata::{
    hybrid::{dfa as AutomataHybridDfa, regex as AutomataHybrid},
    nfa::thompson::{self as AutomataThompson, pikevm as AutomataPike, WhichCaptures},
    util::syntax as AutomataSyntax,
    Anchored as AutomataAnchored, Input as AutomataInput, MatchKind as AutomataMatchKind,
};
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

pub(crate) struct BoundedRustRegex {
    regex: AutomataHybrid::Regex,
    cache: Mutex<AutomataHybrid::Cache>,
    pike: AutomataPike::PikeVM,
    pike_only: AtomicBool,
    retained_charge: usize,
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

#[derive(Clone)]
pub(crate) enum CompiledRegex {
    Rust(Arc<RustRegex>),
    BoundedRust(Arc<BoundedRustRegex>),
    Fancy(Arc<fancy_regex::Regex>),
    LogicalUtf16(Arc<regress::Regex>),
    // Assertion erasure is rejection-only. The capture-erased exact linear
    // matcher may select language bounds, but never supplies capture slots.
    PrefilteredExact {
        prefilter: Arc<RustRegex>,
        boundary_fast: Arc<RustRegex>,
        exact: Arc<fancy_regex::Regex>,
        linear_exact: Option<Arc<fancy_regex::Regex>>,
        needs_capture_correction: bool,
    },
    CaptureCorrected {
        fast: Arc<RustRegex>,
        captures: Arc<fancy_regex::Regex>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegExpCompileMode {
    ScalarPreferred,
    Utf16CodeUnits,
    LogicalUtf16Required,
}

pub(crate) struct RegExpMatcherCacheEntry {
    source: Arc<str>,
    compile_flags: u8,
    mode: RegExpCompileMode,
    matcher: CompiledRegex,
    matcher_charge: usize,
}

#[derive(Default)]
pub(crate) struct RegExpMatcherCache {
    entries: std::collections::VecDeque<RegExpMatcherCacheEntry>,
    source_bytes: usize,
    matcher_bytes: usize,
}

impl RegExpMatcherCache {
    fn debug_assert_invariants(&self) {
        debug_assert!(self.entries.len() <= MAX_REGEXP_MATCHER_CACHE_ENTRIES);
        debug_assert!(self.source_bytes <= MAX_REGEXP_MATCHER_CACHE_SOURCE_BYTES);
        debug_assert!(self.matcher_bytes <= MAX_REGEXP_MATCHER_CACHE_BYTES);
        debug_assert_eq!(
            self.source_bytes,
            self.entries
                .iter()
                .map(|entry| entry.source.len())
                .sum::<usize>()
        );
        debug_assert_eq!(
            self.matcher_bytes,
            self.entries
                .iter()
                .map(|entry| entry.matcher_charge)
                .sum::<usize>()
        );
    }

    #[cfg(test)]
    pub(crate) fn clear_for_test(&mut self) {
        self.entries.clear();
        self.source_bytes = 0;
        self.matcher_bytes = 0;
    }

    #[cfg(test)]
    pub(crate) fn len_for_test(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty_for_test(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn source_bytes_for_test(&self) -> usize {
        self.source_bytes
    }

    #[cfg(test)]
    pub(crate) fn matcher_bytes_for_test(&self) -> usize {
        self.matcher_bytes
    }

    #[cfg(test)]
    pub(crate) fn contains_source_for_test(&self, source: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.source.as_ref() == source)
    }
}

#[derive(Debug)]
enum RegexCompileError {
    Syntax(String),
    Resource(String),
}

fn regexp_compile_error(error: RegexCompileError) -> Arc<Error> {
    match error {
        RegexCompileError::Syntax(message) => Error::syntax(format!("Invalid regex: {message}")),
        RegexCompileError::Resource(message) => Error::range(format!("Invalid regex: {message}")),
    }
}

#[derive(Clone, Copy)]
struct RegexModifierState {
    dot_all: bool,
    ignore_case: bool,
}

/// Compile a regex pattern applying ES flags: `i` (case-insensitive),
/// `m` (multiline ^/$), `s` (dotall), and Unicode semantics (`u`/`v`). The
/// caller handles state-only flags (`d`/`g`/`y`) outside the matcher.
fn compile_regex(source: &str, flags: &str) -> Result<CompiledRegex, RegexCompileError> {
    compile_regex_with_input_mode(source, flags, false)
}

pub(crate) const MAX_REGEXP_MATCHER_CACHE_ENTRIES: usize = 16;
const MAX_REGEXP_MATCHER_CACHE_SOURCE_BYTES: usize = 256 * 1024;
const MAX_REGEXP_MATCHER_CACHE_SINGLE_SOURCE_BYTES: usize = 64 * 1024;
const MAX_REGEXP_MATCHER_CACHE_BYTES: usize = 128 * 1024 * 1024;
const MAX_REGEXP_MATCHER_CACHE_SINGLE_BYTES: usize = 128 * 1024 * 1024;
const RUST_REGEX_NFA_SIZE_LIMIT: usize = 10 * 1024 * 1024;
const RUST_REGEX_DFA_CACHE_LIMIT: usize = 2 * 1024 * 1024;
const RUST_REGEX_RETAINED_DFA_CACHE_LIMIT: usize = 512 * 1024;
const RUST_REGEX_DFA_RETAINED_MULTIPLIER: usize = 4;

fn regexp_compile_flags(flags: &str) -> u8 {
    u8::from(flags.contains('i'))
        | (u8::from(flags.contains('m')) << 1)
        | (u8::from(flags.contains('s')) << 2)
        | (u8::from(flags.contains('u')) << 3)
        | (u8::from(flags.contains('v')) << 4)
}

fn cache_budget_exceeded(current: usize, additional: usize, maximum: usize) -> bool {
    current
        .checked_add(additional)
        .is_none_or(|total| total > maximum)
}

fn regexp_compile_mode_for_input(
    flags: &str,
    input: &str,
    force_code_units: bool,
) -> RegExpCompileMode {
    if force_code_units {
        return RegExpCompileMode::Utf16CodeUnits;
    }
    let unicode_mode = flags.contains('u') || flags.contains('v');
    let contains_surrogate_backing = unicode_mode
        && input.chars().any(|ch| {
            crate::value::utf16_single_unit_from_internal_char(ch)
                .is_some_and(|unit| (0xd800..=0xdfff).contains(&unit))
        });
    if contains_surrogate_backing {
        RegExpCompileMode::LogicalUtf16Required
    } else {
        RegExpCompileMode::ScalarPreferred
    }
}

fn compile_regex_for_mode(
    source: &str,
    flags: &str,
    mode: RegExpCompileMode,
) -> Result<CompiledRegex, RegexCompileError> {
    let logical_on_backend_syntax = |result| match result {
        Ok(compiled) => Ok(compiled),
        Err(RegexCompileError::Syntax(_)) => compile_logical_utf16_regex(source, flags),
        Err(resource @ RegexCompileError::Resource(_)) => Err(resource),
    };
    match mode {
        RegExpCompileMode::ScalarPreferred if flags.contains('u') || flags.contains('v') => {
            compile_regex(source, flags).or_else(|_| compile_logical_utf16_regex(source, flags))
        }
        RegExpCompileMode::ScalarPreferred => {
            logical_on_backend_syntax(compile_regex(source, flags))
        }
        RegExpCompileMode::Utf16CodeUnits => {
            logical_on_backend_syntax(compile_regex_for_code_units(source, flags))
        }
        RegExpCompileMode::LogicalUtf16Required => compile_logical_utf16_regex(source, flags),
    }
}

fn regexp_matcher_cache_get(
    vm: &mut Vm,
    source: &str,
    flags: &str,
    mode: RegExpCompileMode,
) -> Option<CompiledRegex> {
    let compile_flags = regexp_compile_flags(flags);
    let position = vm.regexp_matcher_cache.entries.iter().position(|entry| {
        entry.mode == mode
            && entry.compile_flags == compile_flags
            && entry.source.as_ref() == source
    })?;
    let entry = vm
        .regexp_matcher_cache
        .entries
        .remove(position)
        .expect("RegExp cache position must remain valid");
    let matcher = entry.matcher.clone();
    vm.regexp_matcher_cache.entries.push_back(entry);
    #[cfg(test)]
    {
        vm.regexp_matcher_cache_hit_count += 1;
    }
    Some(matcher)
}

fn regexp_matcher_cache_put(
    vm: &mut Vm,
    source: Arc<str>,
    flags: &str,
    mode: RegExpCompileMode,
    matcher: CompiledRegex,
) -> bool {
    if source.len() > MAX_REGEXP_MATCHER_CACHE_SINGLE_SOURCE_BYTES {
        return false;
    }
    let Some(matcher_charge) = matcher.cache_charge() else {
        return false;
    };
    if matcher_charge > MAX_REGEXP_MATCHER_CACHE_SINGLE_BYTES {
        return false;
    }
    let compile_flags = regexp_compile_flags(flags);
    if let Some(position) = vm.regexp_matcher_cache.entries.iter().position(|entry| {
        entry.mode == mode
            && entry.compile_flags == compile_flags
            && entry.source.as_ref() == source.as_ref()
    }) {
        let old = vm
            .regexp_matcher_cache
            .entries
            .remove(position)
            .expect("RegExp cache position must remain valid");
        vm.regexp_matcher_cache.source_bytes = vm
            .regexp_matcher_cache
            .source_bytes
            .checked_sub(old.source.len())
            .expect("RegExp cache source accounting must remain balanced");
        vm.regexp_matcher_cache.matcher_bytes = vm
            .regexp_matcher_cache
            .matcher_bytes
            .checked_sub(old.matcher_charge)
            .expect("RegExp cache matcher accounting must remain balanced");
    }

    while vm.regexp_matcher_cache.entries.len() >= MAX_REGEXP_MATCHER_CACHE_ENTRIES
        || cache_budget_exceeded(
            vm.regexp_matcher_cache.source_bytes,
            source.len(),
            MAX_REGEXP_MATCHER_CACHE_SOURCE_BYTES,
        )
        || cache_budget_exceeded(
            vm.regexp_matcher_cache.matcher_bytes,
            matcher_charge,
            MAX_REGEXP_MATCHER_CACHE_BYTES,
        )
    {
        let Some(evicted) = vm.regexp_matcher_cache.entries.pop_front() else {
            break;
        };
        vm.regexp_matcher_cache.source_bytes = vm
            .regexp_matcher_cache
            .source_bytes
            .checked_sub(evicted.source.len())
            .expect("RegExp cache source accounting must remain balanced");
        vm.regexp_matcher_cache.matcher_bytes = vm
            .regexp_matcher_cache
            .matcher_bytes
            .checked_sub(evicted.matcher_charge)
            .expect("RegExp cache matcher accounting must remain balanced");
    }

    if vm.regexp_matcher_cache.entries.len() == vm.regexp_matcher_cache.entries.capacity() {
        #[cfg(test)]
        if vm.fail_next_regexp_matcher_cache_reservation {
            vm.fail_next_regexp_matcher_cache_reservation = false;
            return false;
        }
        if vm.regexp_matcher_cache.entries.try_reserve(1).is_err() {
            return false;
        }
    }
    vm.regexp_matcher_cache.source_bytes = vm
        .regexp_matcher_cache
        .source_bytes
        .checked_add(source.len())
        .expect("bounded RegExp cache source accounting cannot overflow");
    vm.regexp_matcher_cache.matcher_bytes = vm
        .regexp_matcher_cache
        .matcher_bytes
        .checked_add(matcher_charge)
        .expect("bounded RegExp cache matcher accounting cannot overflow");
    vm.regexp_matcher_cache
        .entries
        .push_back(RegExpMatcherCacheEntry {
            source,
            compile_flags,
            mode,
            matcher,
            matcher_charge,
        });
    vm.regexp_matcher_cache.debug_assert_invariants();
    true
}

fn compile_regex_cached(
    vm: &mut Vm,
    source: Arc<str>,
    flags: &str,
    mode: RegExpCompileMode,
) -> Result<CompiledRegex, RegexCompileError> {
    if let Some(matcher) = regexp_matcher_cache_get(vm, &source, flags, mode) {
        return Ok(matcher);
    }
    #[cfg(test)]
    {
        vm.regexp_matcher_compile_count += 1;
    }
    let matcher = compile_regex_for_mode(&source, flags, mode)?;
    regexp_matcher_cache_put(vm, source, flags, mode, matcher.clone());
    Ok(matcher)
}

fn compile_regex_for_input(
    source: &str,
    flags: &str,
    input: &str,
) -> Result<CompiledRegex, RegexCompileError> {
    compile_regex_for_mode(
        source,
        flags,
        regexp_compile_mode_for_input(flags, input, false),
    )
}

fn compile_regex_for_input_cached(
    vm: &mut Vm,
    source: Arc<str>,
    flags: &str,
    input: &str,
) -> Result<CompiledRegex, RegexCompileError> {
    let mode = regexp_compile_mode_for_input(flags, input, false);
    compile_regex_cached(vm, source, flags, mode)
}

fn validate_logical_utf16_source_length(source: &str) -> Result<(), String> {
    let mut utf16_units = 0usize;
    for ch in source.chars() {
        utf16_units = utf16_units.saturating_add(
            crate::value::utf16_single_unit_from_internal_char(ch).map_or(ch.len_utf16(), |_| 1),
        );
        if utf16_units > REGEX_LOGICAL_UTF16_SOURCE_LIMIT {
            return Err("logical UTF-16 regex program is too large".to_string());
        }
    }
    Ok(())
}

fn validate_logical_utf16_source(source: &str) -> Result<(), String> {
    validate_logical_utf16_source_length(source)?;

    let bytes = source.as_bytes();
    let mut property_escapes = 0usize;
    for index in 1..bytes.len().saturating_sub(1) {
        if !matches!(bytes[index], b'p' | b'P') || bytes[index + 1] != b'{' {
            continue;
        }
        let preceding_slashes = bytes[..index]
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\\')
            .count();
        if preceding_slashes % 2 == 1 {
            property_escapes += 1;
            if property_escapes > REGEX_LOGICAL_UTF16_PROPERTY_LIMIT {
                return Err("logical UTF-16 regex has too many property operands".to_string());
            }
        }
    }
    Ok(())
}

fn logical_utf16_pattern_code_points(source: &str) -> Vec<u32> {
    crate::value::utf16_code_points_from_str(source)
}

fn logical_utf16_flags(flags: &str) -> regress::Flags {
    let mut logical_flags = regress::Flags::from(flags);
    if flags.contains('v') {
        logical_flags.unicode = true;
        logical_flags.unicode_sets = true;
    }
    logical_flags
}

fn validate_logical_utf16_construction_limits(
    source: &str,
    flags: &str,
) -> Result<(), RegexCompileError> {
    validate_logical_utf16_source(source).map_err(RegexCompileError::Resource)?;
    let code_points = logical_utf16_pattern_code_points(source);
    regress::Regex::validate_unicode_resource_limits(
        code_points.into_iter(),
        logical_utf16_flags(flags),
    )
    .map_err(|error| {
        if error.is_resource_limit() {
            RegexCompileError::Resource(error.to_string())
        } else {
            RegexCompileError::Syntax(error.to_string())
        }
    })
}

fn compile_logical_utf16_regex(
    source: &str,
    flags: &str,
) -> Result<CompiledRegex, RegexCompileError> {
    validate_logical_utf16_source(source).map_err(RegexCompileError::Resource)?;
    let code_points = logical_utf16_pattern_code_points(source);
    let logical_flags = logical_utf16_flags(flags);
    let regex =
        regress::Regex::from_unicode(code_points.into_iter(), logical_flags).map_err(|error| {
            if error.is_resource_limit() {
                RegexCompileError::Resource(error.to_string())
            } else {
                RegexCompileError::Syntax(error.to_string())
            }
        })?;
    if regex.bounded_execution_state_cost() > REGEX_LOGICAL_UTF16_WORK_LIMIT {
        return Err(RegexCompileError::Resource(
            "logical UTF-16 regex program is too large".to_string(),
        ));
    }
    Ok(CompiledRegex::LogicalUtf16(Arc::new(regex)))
}

fn compile_regex_for_code_units(
    source: &str,
    flags: &str,
) -> Result<CompiledRegex, RegexCompileError> {
    compile_regex_with_input_mode(source, flags, true)
}

fn compile_regex_with_input_mode(
    source: &str,
    flags: &str,
    code_unit_input: bool,
) -> Result<CompiledRegex, RegexCompileError> {
    let capture_count = regex_capture_count(source);
    let capture_names = regex_capture_names(source, flags).map_err(RegexCompileError::Syntax)?;
    let capture_indices = regex_capture_indices_by_name(&capture_names);
    let rewritten_source = rewrite_named_regex_groups_for_backend(source, flags, &capture_indices)
        .map_err(RegexCompileError::Syntax)?;
    let uses_backreference = regex_uses_backreference(
        &rewritten_source,
        capture_count,
        !capture_indices.is_empty(),
    );
    let uses_lookaround = regex_uses_lookaround(&rewritten_source);
    let needs_capture_correction = regex_contains_quantified_capture_group(&rewritten_source);
    let quantifiers = crate::lexer::regex_quantifier_metadata(source, flags)
        .map_err(RegexCompileError::Syntax)?;
    let requires_counter_backend = quantifiers.requires_counter_backend;
    if uses_backreference || uses_lookaround || requires_counter_backend {
        let normalized = normalize_regex_for_backend(
            &rewritten_source,
            flags,
            capture_count,
            code_unit_input,
            true,
            false,
            &capture_indices,
        )
        .map_err(RegexCompileError::Syntax)?;
        return build_fancy_regex_with_repeat_fallback(
            &normalized,
            flags,
            requires_counter_backend,
            quantifiers.has_braced,
        )
        .map(Arc::new)
        .map(CompiledRegex::Fancy);
    }

    let rust_normalized = normalize_regex_for_backend(
        &rewritten_source,
        flags,
        capture_count,
        code_unit_input,
        false,
        true,
        &capture_indices,
    )
    .map_err(RegexCompileError::Syntax)?;
    if capture_count == 0
        && !needs_capture_correction
        && !rust_normalized.relaxed_unicode_word_boundary
    {
        if let Some(regex) = build_bounded_rust_regex(&rust_normalized.source, flags) {
            return Ok(CompiledRegex::BoundedRust(Arc::new(regex)));
        }
    }
    let mut b = RustRegexBuilder::new(&rust_normalized.source);
    b.size_limit(RUST_REGEX_NFA_SIZE_LIMIT)
        .dfa_size_limit(RUST_REGEX_DFA_CACHE_LIMIT);
    b.case_insensitive(flags.contains('i'));
    b.multi_line(flags.contains('m'));
    b.dot_matches_new_line(flags.contains('s'));
    let fast = match b.build() {
        Ok(regex) => regex,
        Err(regex::Error::CompiledTooBig(_)) if quantifiers.has_braced => {
            let normalized = normalize_regex_for_backend(
                &rewritten_source,
                flags,
                capture_count,
                code_unit_input,
                true,
                false,
                &capture_indices,
            )
            .map_err(RegexCompileError::Syntax)?;
            return build_fancy_regex(&normalized, flags, true)
                .map(Arc::new)
                .map(CompiledRegex::Fancy)
                .map_err(fancy_regex_compile_error);
        }
        Err(error) => return Err(rust_regex_compile_error(error, &rust_normalized.source)),
    };
    if rust_normalized.relaxed_unicode_word_boundary {
        let boundary_normalized = normalize_regex_for_backend(
            &rewritten_source,
            flags,
            capture_count,
            code_unit_input,
            false,
            false,
            &capture_indices,
        )
        .map_err(RegexCompileError::Syntax)?;
        let mut boundary_builder = RustRegexBuilder::new(&boundary_normalized.source);
        boundary_builder
            .size_limit(RUST_REGEX_NFA_SIZE_LIMIT)
            .dfa_size_limit(RUST_REGEX_DFA_CACHE_LIMIT);
        boundary_builder.case_insensitive(flags.contains('i'));
        boundary_builder.multi_line(flags.contains('m'));
        boundary_builder.dot_matches_new_line(flags.contains('s'));
        let boundary_fast = boundary_builder
            .build()
            .map_err(|error| rust_regex_compile_error(error, &boundary_normalized.source))?;
        let exact_normalized = normalize_regex_for_backend(
            &rewritten_source,
            flags,
            capture_count,
            code_unit_input,
            true,
            false,
            &capture_indices,
        )
        .map_err(RegexCompileError::Syntax)?;
        let exact = Arc::new(build_fancy_regex_with_repeat_fallback(
            &exact_normalized,
            flags,
            false,
            quantifiers.has_braced,
        )?);
        let linear_exact = if needs_capture_correction {
            let normalized = NormalizedRegex {
                source: erase_backend_capture_groups(&exact_normalized.source),
                backref_sets: Vec::new(),
                relaxed_unicode_word_boundary: false,
            };
            Some(Arc::new(build_fancy_regex_with_repeat_fallback(
                &normalized,
                flags,
                false,
                quantifiers.has_braced,
            )?))
        } else {
            None
        };
        return Ok(CompiledRegex::PrefilteredExact {
            prefilter: Arc::new(fast),
            boundary_fast: Arc::new(boundary_fast),
            exact,
            linear_exact,
            needs_capture_correction,
        });
    }
    if !needs_capture_correction {
        return Ok(CompiledRegex::Rust(Arc::new(fast)));
    }

    let capture_normalized = normalize_regex_for_backend(
        &rewritten_source,
        flags,
        capture_count,
        code_unit_input,
        true,
        false,
        &capture_indices,
    )
    .map_err(RegexCompileError::Syntax)?;
    let captures = Arc::new(build_fancy_regex_with_repeat_fallback(
        &capture_normalized,
        flags,
        false,
        quantifiers.has_braced,
    )?);
    Ok(CompiledRegex::CaptureCorrected {
        fast: Arc::new(fast),
        captures,
    })
}

fn build_fancy_regex(
    normalized: &NormalizedRegex,
    flags: &str,
    non_delegated_repeats: bool,
) -> Result<fancy_regex::Regex, fancy_regex::Error> {
    let mut b = fancy_regex::RegexBuilder::new(&normalized.source);
    b.case_insensitive(flags.contains('i'));
    b.multi_line(flags.contains('m'));
    b.dot_matches_new_line(flags.contains('s'));
    b.ecmascript_mode(true);
    b.ecmascript_unicode_mode(flags.contains('u') || flags.contains('v'));
    b.ecmascript_backref_sets(normalized.backref_sets.clone());
    b.ecmascript_non_delegated_repeats(non_delegated_repeats);
    b.build()
}

fn build_fancy_regex_with_repeat_fallback(
    normalized: &NormalizedRegex,
    flags: &str,
    non_delegated_repeats: bool,
    has_braced_repeat: bool,
) -> Result<fancy_regex::Regex, RegexCompileError> {
    match build_fancy_regex(normalized, flags, non_delegated_repeats) {
        Ok(regex) => Ok(regex),
        Err(error)
            if !non_delegated_repeats
                && has_braced_repeat
                && fancy_regex_size_limit_exceeded(&error) =>
        {
            build_fancy_regex(normalized, flags, true).map_err(fancy_regex_compile_error)
        }
        Err(error) => Err(fancy_regex_compile_error(error)),
    }
}

fn rust_regex_compile_error(error: regex::Error, source: &str) -> RegexCompileError {
    match error {
        regex::Error::CompiledTooBig(_) => RegexCompileError::Resource(error.to_string()),
        regex::Error::Syntax(_) if rust_regex_syntax_resource_limit(source) => {
            RegexCompileError::Resource(error.to_string())
        }
        regex::Error::Syntax(_) => RegexCompileError::Syntax(error.to_string()),
        _ => RegexCompileError::Syntax(error.to_string()),
    }
}

fn build_bounded_rust_regex(source: &str, flags: &str) -> Option<BoundedRustRegex> {
    let syntax = AutomataSyntax::Config::new()
        .case_insensitive(flags.contains('i'))
        .multi_line(flags.contains('m'))
        .dot_matches_new_line(flags.contains('s'))
        .utf8(true);
    let mut pike_builder = AutomataPike::PikeVM::builder();
    pike_builder.syntax(syntax).thompson(
        AutomataThompson::Config::new()
            .nfa_size_limit(Some(RUST_REGEX_NFA_SIZE_LIMIT))
            .utf8(true)
            .which_captures(WhichCaptures::Implicit),
    );
    let pike = pike_builder.build(source).ok()?;

    let mut builder = AutomataHybrid::Regex::builder();
    builder
        .syntax(syntax)
        .thompson(
            AutomataThompson::Config::new()
                .nfa_size_limit(Some(RUST_REGEX_NFA_SIZE_LIMIT))
                .utf8(true)
                .which_captures(WhichCaptures::None),
        )
        .dfa(
            AutomataHybridDfa::Config::new()
                .cache_capacity(RUST_REGEX_RETAINED_DFA_CACHE_LIMIT)
                .minimum_cache_clear_count(Some(3))
                .minimum_bytes_per_state(Some(10))
                .match_kind(AutomataMatchKind::LeftmostFirst),
        );
    let regex = builder.build(source).ok()?;
    let cache = regex.create_cache();
    // regex-automata's budget uses approximate live sizes, so charge a
    // conservative multiplier for Vec slack and hash-table buckets. PikeVM
    // fallback scratch is created per call and never retained.
    let cache_charge = regex
        .forward()
        .get_config()
        .get_cache_capacity()
        .checked_add(regex.reverse().get_config().get_cache_capacity())
        .and_then(|bytes| bytes.checked_mul(RUST_REGEX_DFA_RETAINED_MULTIPLIER))?;
    let retained_charge = regex
        .forward()
        .get_nfa()
        .memory_usage()
        .checked_add(regex.reverse().get_nfa().memory_usage())
        .and_then(|bytes| bytes.checked_add(pike.get_nfa().memory_usage()))
        .and_then(|bytes| bytes.checked_add(cache_charge))
        .and_then(|bytes| bytes.checked_add(source.len()))
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<AutomataHybrid::Regex>()))
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<AutomataHybrid::Cache>()))
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<AutomataPike::PikeVM>()))
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<BoundedRustRegex>()))?;
    Some(BoundedRustRegex {
        regex,
        cache: Mutex::new(cache),
        pike,
        pike_only: AtomicBool::new(false),
        retained_charge,
    })
}

impl BoundedRustRegex {
    fn find_at<'t>(
        &self,
        input: &'t str,
        start: usize,
        anchored: bool,
    ) -> error::Result<Option<CompiledMatch<'t>>> {
        let mut automata_input = AutomataInput::new(input).span(start..input.len());
        if anchored {
            automata_input = automata_input.anchored(AutomataAnchored::Yes);
        }
        if !self.pike_only.load(Ordering::Acquire) {
            let mut cache = self.cache.lock();
            // A waiter may have observed false before another search gave up.
            // Recheck under the cache lock so a failed cache is never reused.
            if !self.pike_only.load(Ordering::Acquire) {
                match self.regex.try_search(&mut cache, &automata_input) {
                    Ok(matched) => {
                        return Ok(matched.map(|matched| CompiledMatch {
                            text: &input[matched.start()..matched.end()],
                            start: matched.start(),
                            end: matched.end(),
                        }));
                    }
                    Err(_) => {
                        self.pike_only.store(true, Ordering::Release);
                    }
                }
            }
        }
        let mut cache = self.pike.create_cache();
        Ok(self
            .pike
            .find(&mut cache, automata_input)
            .map(|matched| CompiledMatch {
                text: &input[matched.start()..matched.end()],
                start: matched.start(),
                end: matched.end(),
            }))
    }

    fn captures_at<'t>(
        &self,
        input: &'t str,
        start: usize,
        anchored: bool,
    ) -> error::Result<Option<CompiledCaptures<'t>>> {
        self.find_at(input, start, anchored).map(|matched| {
            matched.map(|matched| CompiledCaptures {
                groups: vec![Some(matched)],
            })
        })
    }
}

fn rust_regex_syntax_resource_limit(source: &str) -> bool {
    let mut parser = regex_syntax::ast::parse::ParserBuilder::new().build();
    parser.parse(source).is_err_and(|error| {
        matches!(
            error.kind(),
            regex_syntax::ast::ErrorKind::CaptureLimitExceeded
                | regex_syntax::ast::ErrorKind::NestLimitExceeded(_)
        )
    })
}

fn fancy_regex_compile_error(error: fancy_regex::Error) -> RegexCompileError {
    if fancy_regex_size_limit_exceeded(&error)
        || matches!(
            error,
            fancy_regex::Error::ParseError(_, fancy_regex::ParseError::RecursionExceeded)
        )
    {
        RegexCompileError::Resource(error.to_string())
    } else {
        RegexCompileError::Syntax(error.to_string())
    }
}

fn fancy_regex_size_limit_exceeded(error: &fancy_regex::Error) -> bool {
    let fancy_regex::Error::CompileError(error) = error else {
        return false;
    };
    let fancy_regex::CompileError::InnerError(error) = error.as_ref() else {
        return false;
    };
    error.size_limit().is_some()
}

impl CompiledRegex {
    fn cache_charge(&self) -> Option<usize> {
        match self {
            CompiledRegex::Rust(_) => None,
            CompiledRegex::BoundedRust(regex) => Some(regex.retained_charge),
            // fancy-regex delegates to scratch-pool-owning regex-automata
            // matchers and does not expose a finite retained-cache bound.
            CompiledRegex::Fancy(_) => None,
            CompiledRegex::LogicalUtf16(regex) => regex
                .memory_usage()
                .checked_add(core::mem::size_of::<regress::Regex>()),
            CompiledRegex::PrefilteredExact { .. } | CompiledRegex::CaptureCorrected { .. } => None,
        }
    }

    fn find<'t>(&self, input: &'t str) -> error::Result<Option<CompiledMatch<'t>>> {
        self.find_at(input, 0)
    }

    fn find_at<'t>(
        &self,
        input: &'t str,
        start: usize,
    ) -> error::Result<Option<CompiledMatch<'t>>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.find_at(input, start).map(CompiledMatch::from)),
            CompiledRegex::BoundedRust(re) => re.find_at(input, start, false),
            CompiledRegex::LogicalUtf16(_) => {
                Ok(self.captures_at(input, start)?.and_then(|caps| caps.get(0)))
            }
            CompiledRegex::Fancy(re) => re
                .find_from_pos(input, start)
                .map(|m| m.map(CompiledMatch::from))
                .map_err(regex_runtime_error),
            CompiledRegex::PrefilteredExact {
                prefilter,
                boundary_fast: _,
                exact,
                linear_exact,
                needs_capture_correction: _,
            } => {
                if let Some(linear_exact) = linear_exact {
                    let Some(candidate) = linear_exact
                        .find_from_pos(input, start)
                        .map_err(regex_runtime_error)?
                    else {
                        return Ok(None);
                    };
                    return Ok(corrected_captures(exact, input, candidate.start())?.get(0));
                }
                if prefilter.find_at(input, start).is_none() {
                    return Ok(None);
                }
                exact
                    .find_from_pos(input, start)
                    .map(|matched| matched.map(CompiledMatch::from))
                    .map_err(regex_runtime_error)
            }
            CompiledRegex::CaptureCorrected { fast, captures } => {
                let Some(candidate) = fast.find_at(input, start) else {
                    return Ok(None);
                };
                let corrected = corrected_captures(captures, input, candidate.start())?;
                Ok(corrected.get(0))
            }
        }
    }

    fn find_iter<'t>(&self, input: &'t str) -> error::Result<Vec<CompiledMatch<'t>>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.find_iter(input).map(CompiledMatch::from).collect()),
            CompiledRegex::BoundedRust(_) => Ok(self
                .captures_iter(input)?
                .into_iter()
                .filter_map(|captures| captures.get(0))
                .collect()),
            CompiledRegex::LogicalUtf16(_) => Ok(self
                .captures_iter(input)?
                .into_iter()
                .filter_map(|caps| caps.get(0))
                .collect()),
            CompiledRegex::Fancy(re) => fancy_find_iter(re, input),
            CompiledRegex::PrefilteredExact {
                prefilter,
                boundary_fast,
                exact,
                linear_exact,
                needs_capture_correction,
            } => {
                if let Some(linear_exact) = linear_exact {
                    return Ok(fancy_corrected_captures_iter(linear_exact, exact, input)?
                        .into_iter()
                        .filter_map(|captures| captures.get(0))
                        .collect());
                }
                if rust_and_ecmascript_unicode_word_classes_agree(input) {
                    if !needs_capture_correction {
                        return Ok(boundary_fast
                            .find_iter(input)
                            .map(CompiledMatch::from)
                            .collect());
                    }
                    return Ok(corrected_captures_iter(boundary_fast, exact, input)?
                        .into_iter()
                        .filter_map(|captures| captures.get(0))
                        .collect());
                }
                if prefilter.find(input).is_none() {
                    return Ok(Vec::new());
                }
                fancy_find_iter(exact, input)
            }
            CompiledRegex::CaptureCorrected { .. } => Ok(self
                .captures_iter(input)?
                .into_iter()
                .filter_map(|caps| caps.get(0))
                .collect()),
        }
    }

    fn find_iter_metered<'t, F>(
        &self,
        input: &'t str,
        mut before_push: F,
    ) -> error::Result<Vec<CompiledMatch<'t>>>
    where
        F: FnMut() -> error::Result<()>,
    {
        if let CompiledRegex::LogicalUtf16(regex) = self {
            return Ok(
                logical_utf16_captures_iter_metered(regex, input, before_push)?
                    .into_iter()
                    .filter_map(|captures| captures.get(0))
                    .collect(),
            );
        }

        let mut matches = Vec::new();
        let mut position = 0usize;
        while position <= input.len() {
            let Some(matched) = self.find_at(input, position)? else {
                break;
            };
            before_push()?;
            let Some(next) = next_regex_iteration_position(input, matched.start(), matched.end())
            else {
                matches.push(matched);
                break;
            };
            matches.push(matched);
            position = next;
        }
        Ok(matches)
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
            CompiledRegex::BoundedRust(re) => re.captures_at(input, start, false),
            CompiledRegex::LogicalUtf16(re) => {
                let units = crate::value::utf16_from_str(input);
                let start = crate::value::utf16_len(&input[..start]);
                let mut work_remaining = logical_utf16_work_limit(re, units.len());
                re.find_from_utf16_bounded(&units, start, &mut work_remaining)
                    .map_err(logical_utf16_runtime_error)?
                    .map(|matched| logical_utf16_captures(input, matched))
                    .transpose()
            }
            CompiledRegex::Fancy(re) => re
                .captures_from_pos(input, start)
                .map(|caps| caps.map(CompiledCaptures::from))
                .map_err(regex_runtime_error),
            CompiledRegex::PrefilteredExact {
                prefilter,
                boundary_fast: _,
                exact,
                linear_exact,
                needs_capture_correction: _,
            } => {
                if let Some(linear_exact) = linear_exact {
                    let Some(expected) = linear_exact
                        .find_from_pos(input, start)
                        .map_err(regex_runtime_error)?
                    else {
                        return Ok(None);
                    };
                    return corrected_captures(exact, input, expected.start()).map(Some);
                }
                if prefilter.find_at(input, start).is_none() {
                    return Ok(None);
                }
                exact
                    .captures_from_pos(input, start)
                    .map(|captures| captures.map(CompiledCaptures::from))
                    .map_err(regex_runtime_error)
            }
            CompiledRegex::CaptureCorrected { fast, captures } => {
                let Some(expected) = fast.find_at(input, start) else {
                    return Ok(None);
                };
                corrected_captures(captures, input, expected.start()).map(Some)
            }
        }
    }

    fn captures_iter<'t>(&self, input: &'t str) -> error::Result<Vec<CompiledCaptures<'t>>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re
                .captures_iter(input)
                .map(CompiledCaptures::from)
                .collect()),
            CompiledRegex::BoundedRust(_) => {
                let mut captures = Vec::new();
                let mut position = 0usize;
                while position <= input.len() {
                    let Some(matched) = self.captures_at(input, position)? else {
                        break;
                    };
                    let Some(group) = matched.get(0) else {
                        break;
                    };
                    let next = next_regex_iteration_position(input, group.start(), group.end());
                    captures.push(matched);
                    let Some(next) = next else {
                        break;
                    };
                    position = next;
                }
                Ok(captures)
            }
            CompiledRegex::LogicalUtf16(re) => logical_utf16_captures_iter(re, input),
            CompiledRegex::Fancy(re) => fancy_captures_iter(re, input),
            CompiledRegex::PrefilteredExact {
                prefilter,
                boundary_fast,
                exact,
                linear_exact,
                needs_capture_correction,
            } => {
                if let Some(linear_exact) = linear_exact {
                    return fancy_corrected_captures_iter(linear_exact, exact, input);
                }
                if rust_and_ecmascript_unicode_word_classes_agree(input) {
                    if !needs_capture_correction {
                        return Ok(boundary_fast
                            .captures_iter(input)
                            .map(CompiledCaptures::from)
                            .collect());
                    }
                    return corrected_captures_iter(boundary_fast, exact, input);
                }
                if prefilter.find(input).is_none() {
                    return Ok(Vec::new());
                }
                fancy_captures_iter(exact, input)
            }
            CompiledRegex::CaptureCorrected { fast, captures } => {
                corrected_captures_iter(fast, captures, input)
            }
        }
    }

    fn captures_exact_at<'t>(
        &self,
        input: &'t str,
        start: usize,
    ) -> error::Result<Option<CompiledCaptures<'t>>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re
                .captures_at(input, start)
                .filter(|captures| {
                    captures
                        .get(0)
                        .is_some_and(|matched| matched.start() == start)
                })
                .map(CompiledCaptures::from)),
            CompiledRegex::BoundedRust(re) => re.captures_at(input, start, true),
            CompiledRegex::LogicalUtf16(re) => {
                let units = crate::value::utf16_from_str(input);
                let start = crate::value::utf16_len(&input[..start]);
                let mut work_remaining = logical_utf16_work_limit(re, units.len());
                re.find_at_utf16_bounded(&units, start, &mut work_remaining)
                    .map_err(logical_utf16_runtime_error)?
                    .map(|matched| logical_utf16_captures(input, matched))
                    .transpose()
            }
            CompiledRegex::Fancy(re) => re
                .captures_at_pos(input, start)
                .map(|captures| captures.map(CompiledCaptures::from))
                .map_err(regex_runtime_error),
            CompiledRegex::PrefilteredExact {
                exact,
                linear_exact,
                ..
            } => {
                if let Some(linear_exact) = linear_exact {
                    let Some(expected) = linear_exact
                        .find_at_pos(input, start)
                        .map_err(regex_runtime_error)?
                    else {
                        return Ok(None);
                    };
                    debug_assert_eq!(expected.start(), start);
                }
                exact
                    .captures_at_pos(input, start)
                    .map(|captures| captures.map(CompiledCaptures::from))
                    .map_err(regex_runtime_error)
            }
            CompiledRegex::CaptureCorrected { fast, captures } => {
                let Some(candidate) = fast.find_at(input, start) else {
                    return Ok(None);
                };
                if candidate.start() != start {
                    return Ok(None);
                }
                corrected_captures(captures, input, start).map(Some)
            }
        }
    }

    fn replace<'t>(&self, input: &'t str, replacement: &str) -> error::Result<Cow<'t, str>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.replace(input, replacement)),
            CompiledRegex::BoundedRust(_)
            | CompiledRegex::Fancy(_)
            | CompiledRegex::LogicalUtf16(_)
            | CompiledRegex::PrefilteredExact { .. }
            | CompiledRegex::CaptureCorrected { .. } => {
                self.replace_fancy(input, replacement, false)
            }
        }
    }

    fn replace_all<'t>(&self, input: &'t str, replacement: &str) -> error::Result<Cow<'t, str>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.replace_all(input, replacement)),
            CompiledRegex::BoundedRust(_)
            | CompiledRegex::Fancy(_)
            | CompiledRegex::LogicalUtf16(_)
            | CompiledRegex::PrefilteredExact { .. }
            | CompiledRegex::CaptureCorrected { .. } => {
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
        if !global {
            let Some(caps) = self.captures(input)? else {
                return Ok(Cow::Borrowed(input));
            };
            let Some(matched) = caps.get(0) else {
                return Ok(Cow::Borrowed(input));
            };
            let mut result =
                String::with_capacity(input.len() - matched.as_str().len() + replacement.len());
            result.push_str(&input[..matched.start()]);
            result.push_str(replacement);
            result.push_str(&input[matched.end()..]);
            return Ok(Cow::Owned(result));
        }
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
        }
        if !replaced {
            return Ok(Cow::Borrowed(input));
        }
        result.push_str(&input[last_end..]);
        Ok(Cow::Owned(result))
    }
}

const REGEX_LOGICAL_UTF16_WORK_LIMIT: usize = 1_000_000;
const REGEX_LOGICAL_UTF16_MAX_WORK_LIMIT: usize = 32_000_000;
const REGEX_LOGICAL_UTF16_SOURCE_LIMIT: usize = 262_144;
const REGEX_LOGICAL_UTF16_PROPERTY_LIMIT: usize = 64;

fn logical_utf16_work_limit(regex: &regress::Regex, input_units: usize) -> usize {
    let work_per_unit = regex.bounded_execution_state_cost().clamp(256, 8192);
    REGEX_LOGICAL_UTF16_WORK_LIMIT
        .saturating_add(input_units.saturating_mul(work_per_unit))
        .min(REGEX_LOGICAL_UTF16_MAX_WORK_LIMIT)
}

fn meter_logical_regex_input(vm: &mut Vm, regex: &CompiledRegex, input: &str) -> error::Result<()> {
    if !matches!(regex, CompiledRegex::LogicalUtf16(_)) {
        return Ok(());
    }
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        vm.consume_fuel()?;
        if crate::value::utf16_single_unit_from_internal_char(ch)
            .is_some_and(|unit| (0xd800..=0xdbff).contains(&unit))
            && chars.peek().is_some_and(|next| {
                crate::value::utf16_single_unit_from_internal_char(*next)
                    .is_some_and(|unit| (0xdc00..=0xdfff).contains(&unit))
            })
        {
            chars.next();
        }
    }
    Ok(())
}

fn logical_utf16_runtime_error(error: regress::RuntimeError) -> Arc<Error> {
    Error::fuel(format!("Invalid regex match: {error}"))
}

fn logical_utf16_match<'t>(
    input: &'t str,
    range: regress::Range,
    byte_offsets: &std::collections::HashMap<usize, usize>,
) -> error::Result<CompiledMatch<'t>> {
    let start = byte_offsets
        .get(&range.start)
        .copied()
        .ok_or_else(|| Error::internal("logical RegExp returned an invalid UTF-16 match start"))?;
    let end = byte_offsets
        .get(&range.end)
        .copied()
        .ok_or_else(|| Error::internal("logical RegExp returned an invalid UTF-16 match end"))?;
    Ok(CompiledMatch {
        text: &input[start..end],
        start,
        end,
    })
}

fn logical_utf16_captures<'t>(
    input: &'t str,
    matched: regress::Match,
) -> error::Result<CompiledCaptures<'t>> {
    logical_utf16_captures_many(input, vec![matched])?
        .pop()
        .ok_or_else(|| Error::internal("logical RegExp omitted a successful match"))
}

fn logical_utf16_captures_many<'t>(
    input: &'t str,
    matches: Vec<regress::Match>,
) -> error::Result<Vec<CompiledCaptures<'t>>> {
    let mut endpoints: Vec<usize> = matches
        .iter()
        .flat_map(|matched| {
            std::iter::once(&matched.range)
                .chain(matched.captures.iter().filter_map(Option::as_ref))
                .flat_map(|range| [range.start, range.end])
        })
        .collect();
    endpoints.sort_unstable();
    endpoints.dedup();

    let mut byte_offsets = std::collections::HashMap::with_capacity(endpoints.len());
    let mut endpoint_index = 0usize;
    let mut utf16 = 0usize;
    for (byte, ch) in input.char_indices() {
        while endpoints.get(endpoint_index) == Some(&utf16) {
            byte_offsets.insert(utf16, byte);
            endpoint_index += 1;
        }
        if endpoints
            .get(endpoint_index)
            .is_some_and(|endpoint| *endpoint < utf16)
        {
            return Err(Error::internal(
                "logical RegExp returned a non-boundary UTF-16 offset",
            ));
        }
        utf16 += crate::value::utf16_single_unit_from_internal_char(ch)
            .map_or_else(|| ch.len_utf16(), |_| 1);
    }
    while endpoints.get(endpoint_index) == Some(&utf16) {
        byte_offsets.insert(utf16, input.len());
        endpoint_index += 1;
    }
    if endpoint_index != endpoints.len() {
        return Err(Error::internal(
            "logical RegExp returned an out-of-range UTF-16 offset",
        ));
    }

    matches
        .into_iter()
        .map(|matched| {
            let mut groups = Vec::with_capacity(matched.captures.len() + 1);
            groups.push(Some(logical_utf16_match(
                input,
                matched.range,
                &byte_offsets,
            )?));
            groups.extend(
                matched
                    .captures
                    .into_iter()
                    .map(|range| {
                        range
                            .map(|range| logical_utf16_match(input, range, &byte_offsets))
                            .transpose()
                    })
                    .collect::<error::Result<Vec<_>>>()?,
            );
            Ok(CompiledCaptures { groups })
        })
        .collect()
}

fn next_logical_utf16_position(units: &[u16], position: usize) -> Option<usize> {
    let high = *units.get(position)?;
    if (0xd800..=0xdbff).contains(&high)
        && units
            .get(position + 1)
            .is_some_and(|low| (0xdc00..=0xdfff).contains(low))
    {
        Some(position + 2)
    } else {
        Some(position + 1)
    }
}

fn logical_utf16_captures_iter<'t>(
    regex: &regress::Regex,
    input: &'t str,
) -> error::Result<Vec<CompiledCaptures<'t>>> {
    logical_utf16_captures_iter_metered(regex, input, || Ok(()))
}

fn logical_utf16_captures_iter_metered<'t, F>(
    regex: &regress::Regex,
    input: &'t str,
    mut before_push: F,
) -> error::Result<Vec<CompiledCaptures<'t>>>
where
    F: FnMut() -> error::Result<()>,
{
    let units = crate::value::utf16_from_str(input);
    let mut matches = Vec::new();
    let mut position = 0;
    let mut work_remaining = logical_utf16_work_limit(regex, units.len());
    while position <= units.len() {
        let Some(matched) = regex
            .find_from_utf16_bounded(&units, position, &mut work_remaining)
            .map_err(logical_utf16_runtime_error)?
        else {
            break;
        };
        let start = matched.range.start;
        let end = matched.range.end;
        before_push()?;
        matches.push(matched);
        if start != end {
            position = end;
        } else if let Some(next) = next_logical_utf16_position(&units, end) {
            position = next;
        } else {
            break;
        }
    }
    logical_utf16_captures_many(input, matches)
}

fn next_regex_iteration_position(input: &str, start: usize, end: usize) -> Option<usize> {
    if start != end {
        return Some(end);
    }
    input[end..].chars().next().map(|ch| end + ch.len_utf8())
}

fn rust_and_ecmascript_unicode_word_classes_agree(input: &str) -> bool {
    static RUST_ONLY_WORD: OnceLock<RustRegex> = OnceLock::new();
    let rust_only_word = RUST_ONLY_WORD.get_or_init(|| {
        RustRegex::new(r"[\w&&[^A-Za-z0-9_\u{017F}\u{212A}]]")
            .expect("static Unicode word-class difference must compile")
    });
    !rust_only_word.is_match(input)
}

fn erase_backend_capture_groups(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut escaped = false;
    let mut in_class = false;
    while let Some(ch) = chars.next() {
        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            output.push(ch);
            escaped = true;
            continue;
        }
        if ch == '[' && !in_class {
            in_class = true;
        } else if ch == ']' && in_class {
            in_class = false;
        }
        if ch == '(' && !in_class && chars.peek() != Some(&'?') {
            output.push_str("(?:");
        } else {
            output.push(ch);
        }
    }
    output
}

fn fancy_find_iter<'t>(
    regex: &fancy_regex::Regex,
    input: &'t str,
) -> error::Result<Vec<CompiledMatch<'t>>> {
    let mut matches = Vec::new();
    let mut position = 0;
    while position <= input.len() {
        let Some(matched) = regex
            .find_from_pos(input, position)
            .map_err(regex_runtime_error)?
        else {
            break;
        };
        let start = matched.start();
        let end = matched.end();
        matches.push(CompiledMatch::from(matched));
        let Some(next) = next_regex_iteration_position(input, start, end) else {
            break;
        };
        position = next;
    }
    Ok(matches)
}

fn fancy_captures_iter<'t>(
    regex: &fancy_regex::Regex,
    input: &'t str,
) -> error::Result<Vec<CompiledCaptures<'t>>> {
    let mut captures = Vec::new();
    let mut position = 0;
    while position <= input.len() {
        let Some(groups) = regex
            .captures_from_pos(input, position)
            .map_err(regex_runtime_error)?
        else {
            break;
        };
        let matched = groups
            .get(0)
            .ok_or_else(|| Error::internal("exact RegExp backend omitted group zero"))?;
        let start = matched.start();
        let end = matched.end();
        captures.push(CompiledCaptures::from(groups));
        let Some(next) = next_regex_iteration_position(input, start, end) else {
            break;
        };
        position = next;
    }
    Ok(captures)
}

fn corrected_captures<'t>(
    re: &fancy_regex::Regex,
    input: &'t str,
    expected_start: usize,
) -> error::Result<CompiledCaptures<'t>> {
    let caps = re
        .captures_at_pos(input, expected_start)
        .map_err(regex_runtime_error)?
        .ok_or_else(|| Error::internal("capture backend lost a prefiltered RegExp match"))?;
    let actual = caps
        .get(0)
        .ok_or_else(|| Error::internal("capture backend omitted RegExp group zero"))?;
    if actual.start() != expected_start {
        return Err(Error::internal(
            "capture backend disagreed with the prefiltered RegExp start",
        ));
    }
    Ok(CompiledCaptures::from(caps))
}

fn corrected_captures_iter<'t>(
    fast: &RustRegex,
    exact: &fancy_regex::Regex,
    input: &'t str,
) -> error::Result<Vec<CompiledCaptures<'t>>> {
    let mut matches = Vec::new();
    let mut position = 0;
    while position <= input.len() {
        let Some(candidate) = fast.find_at(input, position) else {
            break;
        };
        let corrected = corrected_captures(exact, input, candidate.start())?;
        let actual = corrected
            .get(0)
            .ok_or_else(|| Error::internal("capture backend omitted RegExp group zero"))?;
        matches.push(corrected);
        let Some(next) = next_regex_iteration_position(input, actual.start(), actual.end()) else {
            break;
        };
        position = next;
    }
    Ok(matches)
}

fn fancy_corrected_captures_iter<'t>(
    linear_exact: &fancy_regex::Regex,
    exact: &fancy_regex::Regex,
    input: &'t str,
) -> error::Result<Vec<CompiledCaptures<'t>>> {
    let mut matches = Vec::new();
    let mut position = 0;
    while position <= input.len() {
        let Some(candidate) = linear_exact
            .find_from_pos(input, position)
            .map_err(regex_runtime_error)?
        else {
            break;
        };
        let corrected = corrected_captures(exact, input, candidate.start())?;
        let actual = corrected
            .get(0)
            .ok_or_else(|| Error::internal("capture backend omitted RegExp group zero"))?;
        matches.push(corrected);
        let Some(next) = next_regex_iteration_position(input, actual.start(), actual.end()) else {
            break;
        };
        position = next;
    }
    Ok(matches)
}

#[cfg(test)]
mod compiled_regex_tests {
    use super::*;

    #[test]
    fn compiler_and_runtime_errors_preserve_resource_kind() {
        assert!(matches!(
            rust_regex_compile_error(regex::Error::CompiledTooBig(1), "a"),
            RegexCompileError::Resource(_)
        ));
        let nested = format!("{}a{}", "(".repeat(251), ")".repeat(251));
        let nested_error = RustRegexBuilder::new(&nested)
            .build()
            .expect_err("backend nesting limit should reject the pattern");
        assert!(matches!(
            rust_regex_compile_error(nested_error, &nested),
            RegexCompileError::Resource(_)
        ));

        let normalized = NormalizedRegex {
            source: "(".to_string(),
            backref_sets: Vec::new(),
            relaxed_unicode_word_boundary: false,
        };
        let syntax = build_fancy_regex(&normalized, "", false)
            .expect_err("invalid source should fail compilation");
        assert!(matches!(
            fancy_regex_compile_error(syntax),
            RegexCompileError::Syntax(_)
        ));
        let nested = NormalizedRegex {
            source: format!("{}a{}", "(?=".repeat(65), ")".repeat(65)),
            backref_sets: Vec::new(),
            relaxed_unicode_word_boundary: false,
        };
        let nested_error = build_fancy_regex(&nested, "u", false)
            .expect_err("fancy backend nesting limit should reject the pattern");
        assert!(matches!(
            fancy_regex_compile_error(nested_error),
            RegexCompileError::Resource(_)
        ));

        let runtime = regex_runtime_error(fancy_regex::Error::RuntimeError(
            fancy_regex::RuntimeError::BacktrackLimitExceeded,
        ));
        assert_eq!(runtime.kind, crate::error::ErrorKind::Fuel);
        assert!(!runtime.catchable());
    }

    #[test]
    fn exec_compile_injection_preserves_real_errors_and_targets_success() {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.fail_regexp_exec_compile = Some(0);
        assert!(matches!(
            regexp::compile_regexp_for_exec(&mut vm, Arc::from("("), "", ""),
            Err(RegexCompileError::Syntax(_))
        ));
        assert_eq!(vm.fail_regexp_exec_compile, Some(0));

        assert!(matches!(
            regexp::compile_regexp_for_exec(&mut vm, Arc::from("a"), "", "a"),
            Err(RegexCompileError::Resource(_))
        ));
        assert_eq!(vm.fail_regexp_exec_compile, None);
    }

    #[test]
    fn exec_compile_backend_fixtures_select_expected_variants() {
        assert!(matches!(
            regexp::compile_regexp_for_exec(
                &mut Vm::new().expect("VM should initialize"),
                Arc::from("a"),
                "",
                "a"
            ),
            Ok(CompiledRegex::BoundedRust(_))
        ));
        assert!(matches!(
            regexp::compile_regexp_for_exec(
                &mut Vm::new().expect("VM should initialize"),
                Arc::from("(?=a)a"),
                "",
                "a"
            ),
            Ok(CompiledRegex::Fancy(_))
        ));
        assert!(matches!(
            regexp::compile_regexp_for_exec(
                &mut Vm::new().expect("VM should initialize"),
                Arc::from("(a){1,1000000}"),
                "",
                "a"
            ),
            Ok(CompiledRegex::Fancy(_))
        ));
        let lone_surrogate = crate::value::utf16_to_string(&[0xd800]);
        assert!(matches!(
            regexp::compile_regexp_for_exec(
                &mut Vm::new().expect("VM should initialize"),
                Arc::from("."),
                "u",
                &lone_surrogate
            ),
            Ok(CompiledRegex::LogicalUtf16(_))
        ));
        assert!(matches!(
            regexp::compile_regexp_for_exec(
                &mut Vm::new().expect("VM should initialize"),
                Arc::from(r"\p{RGI_Emoji}"),
                "v",
                "😀"
            ),
            Ok(CompiledRegex::LogicalUtf16(_))
        ));
        assert!(matches!(
            compile_regex_for_mode(r"[\uDC00-\uDC0B]", "", RegExpCompileMode::Utf16CodeUnits),
            Ok(CompiledRegex::LogicalUtf16(_))
        ));
    }

    #[test]
    fn matcher_cache_charge_covers_every_compiled_backend_variant() {
        let fixtures = [
            compile_regex("a", "").expect("Rust fixture should compile"),
            compile_regex("(?=a)a", "").expect("fancy fixture should compile"),
            compile_regex("(a){1,1000000}", "").expect("counter fallback fixture should compile"),
            compile_logical_utf16_regex(".", "u").expect("logical fixture should compile"),
            compile_regex("(a?b??)*", "").expect("capture-corrected fixture should compile"),
            compile_regex(r"\b(a?b??)*", "iu").expect("prefiltered fixture should compile"),
        ];

        assert!(matches!(fixtures[0], CompiledRegex::BoundedRust(_)));
        assert!(matches!(fixtures[1], CompiledRegex::Fancy(_)));
        assert!(matches!(fixtures[2], CompiledRegex::Fancy(_)));
        assert!(matches!(fixtures[3], CompiledRegex::LogicalUtf16(_)));
        assert!(matches!(
            fixtures[4],
            CompiledRegex::CaptureCorrected { .. }
        ));
        assert!(matches!(
            fixtures[5],
            CompiledRegex::PrefilteredExact { .. }
        ));

        for (index, matcher) in fixtures.into_iter().enumerate() {
            let charge = matcher.cache_charge();
            let cacheable = matches!(index, 0 | 3);
            assert_eq!(charge.is_some(), cacheable);
            let mut vm = Vm::new().expect("VM should initialize");
            assert_eq!(
                regexp_matcher_cache_put(
                    &mut vm,
                    Arc::from(format!("backend-{index}")),
                    "",
                    RegExpCompileMode::ScalarPreferred,
                    matcher,
                ),
                cacheable
            );
            if let Some(charge) = charge {
                assert!(charge > 0);
                assert!(charge <= MAX_REGEXP_MATCHER_CACHE_SINGLE_BYTES);
                assert_eq!(vm.regexp_matcher_cache.matcher_bytes_for_test(), charge);
            }
        }

        let captured = compile_regex("(a)", "").expect("captured Rust fixture should compile");
        assert!(matches!(captured, CompiledRegex::Rust(_)));
        assert!(!regexp_matcher_cache_put(
            &mut Vm::new().expect("VM should initialize"),
            Arc::from("(a)"),
            "",
            RegExpCompileMode::ScalarPreferred,
            captured,
        ));
    }

    fn logical_cache_accounting_matcher() -> CompiledRegex {
        compile_logical_utf16_regex("", "u").expect("logical accounting fixture should compile")
    }

    #[test]
    fn matcher_cache_semantic_flags_miss_once_then_hit() {
        for (source, flags, input, expected) in [
            ("a", "i", "A", true),
            ("^.$", "m", "\na\n", true),
            ("^.$", "s", "\n", true),
            (".", "u", "😀", true),
            (".", "v", "😀", true),
        ] {
            let mut vm = Vm::new().expect("VM should initialize");
            compile_regex_cached(
                &mut vm,
                Arc::from(source),
                "",
                RegExpCompileMode::ScalarPreferred,
            )
            .expect("unflagged comparison matcher should compile");
            let matcher = compile_regex_cached(
                &mut vm,
                Arc::from(source),
                flags,
                RegExpCompileMode::ScalarPreferred,
            )
            .expect("flagged matcher should compile separately");
            assert_eq!(
                matcher
                    .find(input)
                    .expect("matcher should execute")
                    .is_some(),
                expected
            );
            compile_regex_cached(
                &mut vm,
                Arc::from(source),
                flags,
                RegExpCompileMode::ScalarPreferred,
            )
            .expect("second flagged compilation should hit");
            assert_eq!(vm.regexp_matcher_compile_count, 2, "flag {flags}");
            assert_eq!(vm.regexp_matcher_cache_hit_count, 1, "flag {flags}");
        }
    }

    #[test]
    fn finite_rust_and_logical_matchers_coexist_and_hit() {
        let mut vm = Vm::new().expect("VM should initialize");
        let fallback_source: Arc<str> = Arc::from(r"[\uDC00-\uDC0B]");
        assert!(matches!(
            compile_regex_cached(
                &mut vm,
                fallback_source.clone(),
                "",
                RegExpCompileMode::Utf16CodeUnits,
            ),
            Ok(CompiledRegex::LogicalUtf16(_))
        ));
        assert!(matches!(
            compile_regex_cached(
                &mut vm,
                Arc::from(r"\w"),
                "",
                RegExpCompileMode::Utf16CodeUnits,
            ),
            Ok(CompiledRegex::BoundedRust(_))
        ));
        assert!(matches!(
            compile_regex_cached(
                &mut vm,
                Arc::from(r"\w"),
                "",
                RegExpCompileMode::Utf16CodeUnits,
            ),
            Ok(CompiledRegex::BoundedRust(_))
        ));
        assert!(matches!(
            compile_regex_cached(
                &mut vm,
                fallback_source,
                "",
                RegExpCompileMode::Utf16CodeUnits,
            ),
            Ok(CompiledRegex::LogicalUtf16(_))
        ));

        assert_eq!(vm.regexp_matcher_compile_count, 2);
        assert_eq!(vm.regexp_matcher_cache_hit_count, 2);
        assert!(vm
            .regexp_matcher_cache
            .contains_source_for_test(r"[\uDC00-\uDC0B]"));
        assert!(vm.regexp_matcher_cache.contains_source_for_test(r"\w"));
        assert!(vm.regexp_matcher_cache.matcher_bytes_for_test() < MAX_REGEXP_MATCHER_CACHE_BYTES);
    }

    #[test]
    fn bounded_rust_charge_covers_lazy_dfa_cache_growth() {
        let matcher = compile_regex(r"(?:a|ab|abc|abcd|abcde)*z", "")
            .expect("bounded Rust fixture should compile");
        let CompiledRegex::BoundedRust(regex) = matcher else {
            panic!("capture-free regular fixture should use bounded Rust");
        };
        let cache_capacity = regex
            .regex
            .forward()
            .get_config()
            .get_cache_capacity()
            .checked_add(regex.regex.reverse().get_config().get_cache_capacity())
            .expect("configured cache capacities should fit");
        assert!(regex.cache.lock().memory_usage() <= cache_capacity);

        let input = format!("{}z", "abcde".repeat(2_000));
        assert!(regex
            .find_at(&input, 0, false)
            .expect("bounded search should complete")
            .is_some());
        assert!(regex.cache.lock().memory_usage() <= cache_capacity);

        let conservative_cache_charge = cache_capacity
            .checked_mul(RUST_REGEX_DFA_RETAINED_MULTIPLIER)
            .expect("conservative cache charge should fit");
        let immutable_charge = regex
            .regex
            .forward()
            .get_nfa()
            .memory_usage()
            .checked_add(regex.regex.reverse().get_nfa().memory_usage())
            .and_then(|bytes| bytes.checked_add(regex.pike.get_nfa().memory_usage()))
            .and_then(|bytes| bytes.checked_add(conservative_cache_charge))
            .expect("bounded matcher charge should fit");
        assert!(regex.retained_charge >= immutable_charge);
    }

    #[test]
    fn bounded_rust_cache_saturation_permanently_uses_pike_fallback() {
        let source = r"[aβ]{100}";
        let mut pike_builder = AutomataPike::PikeVM::builder();
        pike_builder
            .syntax(AutomataSyntax::Config::new().utf8(true))
            .thompson(
                AutomataThompson::Config::new()
                    .nfa_size_limit(Some(RUST_REGEX_NFA_SIZE_LIMIT))
                    .utf8(true)
                    .which_captures(WhichCaptures::Implicit),
            );
        let pike = pike_builder
            .build(source)
            .expect("Pike fallback should compile");
        let mut builder = AutomataHybrid::Regex::builder();
        builder
            .syntax(AutomataSyntax::Config::new().utf8(true))
            .thompson(
                AutomataThompson::Config::new()
                    .nfa_size_limit(Some(RUST_REGEX_NFA_SIZE_LIMIT))
                    .utf8(true)
                    .which_captures(WhichCaptures::None),
            )
            .dfa(
                AutomataHybridDfa::Config::new()
                    .skip_cache_capacity_check(true)
                    .cache_capacity(0)
                    .minimum_cache_clear_count(Some(0))
                    .match_kind(AutomataMatchKind::LeftmostFirst),
            );
        let hybrid = builder.build(source).expect("hybrid probe should compile");
        let input = "a".repeat(101);
        let mut probe_cache = hybrid.create_cache();
        assert!(hybrid
            .try_search(&mut probe_cache, &AutomataInput::new(&input))
            .is_err());

        let bounded = BoundedRustRegex {
            cache: Mutex::new(hybrid.create_cache()),
            regex: hybrid,
            pike,
            pike_only: AtomicBool::new(false),
            retained_charge: 0,
        };
        for _ in 0..2 {
            let matched = bounded
                .find_at(&input, 0, false)
                .expect("Pike fallback search should complete")
                .expect("Pike fallback should match");
            assert_eq!((matched.start(), matched.end()), (0, 100));
            assert!(bounded.pike_only.load(Ordering::Acquire));
        }
    }

    #[test]
    fn matcher_cache_keys_eviction_and_best_effort_publication_are_bounded() {
        let mut vm = Vm::new().expect("VM should initialize");
        let source: Arc<str> = Arc::from("a");
        compile_regex_cached(
            &mut vm,
            source.clone(),
            "gyd",
            RegExpCompileMode::Utf16CodeUnits,
        )
        .expect("first matcher should compile");
        assert_eq!(vm.regexp_matcher_compile_count, 1);
        assert_eq!(vm.regexp_matcher_cache.len_for_test(), 1);

        compile_regex_cached(
            &mut vm,
            source.clone(),
            "",
            RegExpCompileMode::Utf16CodeUnits,
        )
        .expect("non-compiling flags should share a matcher");
        assert_eq!(vm.regexp_matcher_compile_count, 1);
        assert_eq!(vm.regexp_matcher_cache_hit_count, 1);

        compile_regex_cached(
            &mut vm,
            source.clone(),
            "i",
            RegExpCompileMode::Utf16CodeUnits,
        )
        .expect("ignoreCase needs a distinct matcher");
        compile_regex_cached(&mut vm, source, "", RegExpCompileMode::ScalarPreferred)
            .expect("scalar input needs a distinct matcher");
        for flags in ["m", "s", "u", "v"] {
            compile_regex_cached(
                &mut vm,
                Arc::from("a"),
                flags,
                RegExpCompileMode::ScalarPreferred,
            )
            .expect("each matcher-semantic flag needs a distinct entry");
        }
        assert_eq!(vm.regexp_matcher_compile_count, 7);
        assert_eq!(vm.regexp_matcher_cache.len_for_test(), 7);

        let mut failed = Vm::new().expect("VM should initialize");
        failed.fail_next_regexp_matcher_cache_reservation = true;
        compile_regex_cached(
            &mut failed,
            Arc::from("failure"),
            "",
            RegExpCompileMode::ScalarPreferred,
        )
        .expect("cache allocation failure must not fail compilation");
        assert!(failed.regexp_matcher_cache.is_empty_for_test());
        assert!(!failed.fail_next_regexp_matcher_cache_reservation);
        assert_eq!(failed.regexp_matcher_compile_count, 1);
        compile_regex_cached(
            &mut failed,
            Arc::from("failure"),
            "",
            RegExpCompileMode::ScalarPreferred,
        )
        .expect("second compilation should publish after reservation recovers");
        compile_regex_cached(
            &mut failed,
            Arc::from("failure"),
            "",
            RegExpCompileMode::ScalarPreferred,
        )
        .expect("third compilation should hit the cache");
        assert_eq!(failed.regexp_matcher_compile_count, 2);
        assert_eq!(failed.regexp_matcher_cache_hit_count, 1);

        let matcher = compile_regex("a", "").expect("fixture should compile");
        let oversized: Arc<str> =
            Arc::from("a".repeat(MAX_REGEXP_MATCHER_CACHE_SINGLE_SOURCE_BYTES + 1));
        assert!(!regexp_matcher_cache_put(
            &mut failed,
            oversized,
            "",
            RegExpCompileMode::ScalarPreferred,
            matcher
        ));

        let mut lru = Vm::new().expect("VM should initialize");
        let accounting_matcher = logical_cache_accounting_matcher();
        for index in 0..MAX_REGEXP_MATCHER_CACHE_ENTRIES {
            let source: Arc<str> = Arc::from(format!("lru-{index}"));
            assert!(regexp_matcher_cache_put(
                &mut lru,
                source,
                "",
                RegExpCompileMode::ScalarPreferred,
                accounting_matcher.clone(),
            ));
        }
        assert_eq!(
            lru.regexp_matcher_cache.len_for_test(),
            MAX_REGEXP_MATCHER_CACHE_ENTRIES
        );
        regexp_matcher_cache_get(&mut lru, "lru-0", "", RegExpCompileMode::ScalarPreferred)
            .expect("touching the oldest matcher should refresh it");
        let overflow_source: Arc<str> = Arc::from("lru-overflow");
        assert!(regexp_matcher_cache_put(
            &mut lru,
            overflow_source,
            "",
            RegExpCompileMode::ScalarPreferred,
            accounting_matcher.clone(),
        ));
        assert_eq!(
            lru.regexp_matcher_cache.len_for_test(),
            MAX_REGEXP_MATCHER_CACHE_ENTRIES
        );
        assert!(lru.regexp_matcher_cache.contains_source_for_test("lru-0"));
        assert!(!lru.regexp_matcher_cache.contains_source_for_test("lru-1"));
        assert!(
            lru.regexp_matcher_cache.source_bytes_for_test()
                <= MAX_REGEXP_MATCHER_CACHE_SOURCE_BYTES
        );
        assert!(
            lru.regexp_matcher_cache.matcher_bytes_for_test() <= MAX_REGEXP_MATCHER_CACHE_BYTES
        );

        let active =
            regexp_matcher_cache_get(&mut lru, "lru-0", "", RegExpCompileMode::ScalarPreferred)
                .expect("active matcher should be retained independently of the cache");
        for index in 0..=MAX_REGEXP_MATCHER_CACHE_ENTRIES {
            let source: Arc<str> = Arc::from(format!("evict-{index}"));
            assert!(regexp_matcher_cache_put(
                &mut lru,
                source,
                "",
                RegExpCompileMode::ScalarPreferred,
                accounting_matcher.clone(),
            ));
        }
        assert!(!lru.regexp_matcher_cache.contains_source_for_test("lru-0"));
        assert!(active
            .find("")
            .expect("evicted active matcher should remain usable")
            .is_some());

        let mut matcher_budget = Vm::new().expect("VM should initialize");
        for index in 0..5 {
            compile_regex_cached(
                &mut matcher_budget,
                Arc::from(format!("rust{index}")),
                "",
                RegExpCompileMode::ScalarPreferred,
            )
            .expect("Rust matcher budget fixture should compile");
        }
        assert_eq!(matcher_budget.regexp_matcher_cache.len_for_test(), 5);
        assert!(matcher_budget
            .regexp_matcher_cache
            .contains_source_for_test("rust0"));
        assert!(
            matcher_budget.regexp_matcher_cache.matcher_bytes_for_test()
                < MAX_REGEXP_MATCHER_CACHE_BYTES
        );

        let mut mixed_budget = Vm::new().expect("VM should initialize");
        let finite = logical_cache_accounting_matcher();
        assert!(regexp_matcher_cache_put(
            &mut mixed_budget,
            Arc::from("finite-0"),
            "",
            RegExpCompileMode::Utf16CodeUnits,
            finite.clone(),
        ));
        assert!(regexp_matcher_cache_put(
            &mut mixed_budget,
            Arc::from("finite-1"),
            "",
            RegExpCompileMode::Utf16CodeUnits,
            finite.clone(),
        ));
        let rust = compile_regex("small", "").expect("Rust fixture should compile");
        assert!(regexp_matcher_cache_put(
            &mut mixed_budget,
            Arc::from("small"),
            "",
            RegExpCompileMode::Utf16CodeUnits,
            rust,
        ));
        assert!(mixed_budget
            .regexp_matcher_cache
            .contains_source_for_test("finite-0"));
        assert!(mixed_budget
            .regexp_matcher_cache
            .contains_source_for_test("finite-1"));
        assert!(mixed_budget
            .regexp_matcher_cache
            .contains_source_for_test("small"));

        let mut finite_preempts_rust = Vm::new().expect("VM should initialize");
        compile_regex_cached(
            &mut finite_preempts_rust,
            Arc::from("small"),
            "",
            RegExpCompileMode::Utf16CodeUnits,
        )
        .expect("Rust fixture should compile and publish");
        assert!(regexp_matcher_cache_put(
            &mut finite_preempts_rust,
            Arc::from("finite"),
            "",
            RegExpCompileMode::Utf16CodeUnits,
            finite,
        ));
        assert!(finite_preempts_rust
            .regexp_matcher_cache
            .contains_source_for_test("small"));
        assert!(finite_preempts_rust
            .regexp_matcher_cache
            .contains_source_for_test("finite"));
        assert!(
            finite_preempts_rust
                .regexp_matcher_cache
                .matcher_bytes_for_test()
                < MAX_REGEXP_MATCHER_CACHE_BYTES
        );

        let mut source_budget = Vm::new().expect("VM should initialize");
        let accounting_matcher = logical_cache_accounting_matcher();
        for index in 0..5 {
            let mut source = "x".repeat(MAX_REGEXP_MATCHER_CACHE_SINGLE_SOURCE_BYTES - 1);
            source.push(char::from(b'0' + index));
            assert!(regexp_matcher_cache_put(
                &mut source_budget,
                Arc::from(source),
                "",
                RegExpCompileMode::ScalarPreferred,
                accounting_matcher.clone(),
            ));
        }
        assert_eq!(source_budget.regexp_matcher_cache.len_for_test(), 4);
        assert_eq!(
            source_budget.regexp_matcher_cache.source_bytes_for_test(),
            MAX_REGEXP_MATCHER_CACHE_SOURCE_BYTES
        );
    }

    #[test]
    fn unicode_word_class_agreement_detects_rust_only_characters() {
        assert!(rust_and_ecmascript_unicode_word_classes_agree("a_9ſK"));
        assert!(!rust_and_ecmascript_unicode_word_classes_agree("é"));
        assert!(!rust_and_ecmascript_unicode_word_classes_agree("中"));
        assert!(!rust_and_ecmascript_unicode_word_classes_agree("\u{0660}"));

        let hir = RegexSyntaxParserBuilder::new()
            .ecmascript_unicode_word_boundary(true)
            .build()
            .parse(r"\b")
            .expect("custom word boundary should parse");
        assert!(matches!(
            hir.kind(),
            HirKind::Look(regex_syntax::hir::Look::WordEcmaUnicodeIgnoreCase)
        ));
    }

    #[test]
    fn logical_utf16_source_limits_are_escape_aware() {
        let properties = r"\p{Letter}".repeat(REGEX_LOGICAL_UTF16_PROPERTY_LIMIT + 1);
        assert!(validate_logical_utf16_source(&properties)
            .expect_err("too many property operands must be rejected")
            .contains("too many property operands"));

        let escaped_literals = r"\\p{2}".repeat(REGEX_LOGICAL_UTF16_PROPERTY_LIMIT + 1);
        validate_logical_utf16_source(&escaped_literals)
            .expect("escaped property-like literals are not property operands");

        let oversized = "é".repeat(REGEX_LOGICAL_UTF16_SOURCE_LIMIT + 1);
        assert!(validate_logical_utf16_source(&oversized)
            .expect_err("oversized non-ASCII sources must be rejected")
            .contains("too large"));
    }

    #[test]
    fn capture_corrected_apis_use_ecmascript_ends_and_iteration() {
        let re = compile_regex("(a?b??)*", "").expect("nullable capture pattern should compile");

        let found = re
            .find_at("ab", 0)
            .expect("find_at should execute")
            .expect("find_at should match");
        assert_eq!((found.as_str(), found.start(), found.end()), ("ab", 0, 2));

        let found_iter = re.find_iter("ab").expect("find_iter should execute");
        assert_eq!(
            found_iter
                .iter()
                .map(|matched| (matched.as_str(), matched.start(), matched.end()))
                .collect::<Vec<_>>(),
            vec![("ab", 0, 2), ("", 2, 2)]
        );

        let captures = re
            .captures_at("ab", 0)
            .expect("captures_at should execute")
            .expect("captures_at should match");
        assert_eq!(captures.get(0).map(CompiledMatch::as_str), Some("ab"));
        assert_eq!(captures.get(1).map(CompiledMatch::as_str), Some("b"));

        let captures_iter = re
            .captures_iter("ab")
            .expect("captures_iter should execute");
        assert_eq!(
            captures_iter
                .iter()
                .map(|captures| captures.get(0).map(CompiledMatch::as_str))
                .collect::<Vec<_>>(),
            vec![Some("ab"), Some("")]
        );

        let empty = compile_regex("(a?)*?", "").expect("empty capture pattern should compile");
        let empty_find_iter = empty.find_iter("😀x").expect("find_iter should advance");
        assert_eq!(
            empty_find_iter
                .iter()
                .map(|matched| (matched.start(), matched.end()))
                .collect::<Vec<_>>(),
            vec![(0, 0), (4, 4), (5, 5)]
        );
        let empty_captures_iter = empty
            .captures_iter("😀x")
            .expect("captures_iter should advance");
        assert_eq!(
            empty_captures_iter
                .iter()
                .map(|captures| {
                    let matched = captures.get(0).expect("group zero should exist");
                    (matched.start(), matched.end())
                })
                .collect::<Vec<_>>(),
            vec![(0, 0), (4, 4), (5, 5)]
        );
    }

    #[test]
    fn unicode_boundary_find_uses_exact_nullable_repeat_bounds() {
        let re = compile_regex(r"\b(a?b??)*", "iu")
            .expect("Unicode boundary nullable capture pattern should compile");

        let found = re
            .find_at("ab", 0)
            .expect("find_at should execute")
            .expect("find_at should match");
        assert_eq!((found.as_str(), found.start(), found.end()), ("ab", 0, 2));

        let found_iter = re.find_iter("ab").expect("find_iter should execute");
        assert_eq!(
            found_iter
                .iter()
                .map(|matched| (matched.as_str(), matched.start(), matched.end()))
                .collect::<Vec<_>>(),
            vec![("ab", 0, 2), ("", 2, 2)]
        );
    }

    #[test]
    fn unicode_boundary_position_calls_do_not_rescan_the_whole_input() {
        let re = compile_regex(r"\ba", "iu").expect("Unicode boundary pattern should compile");
        let input = "a ".repeat(20_000);
        let mut position = 0;
        let mut count = 0;
        while let Some(captures) = re
            .captures_at(&input, position)
            .expect("position match should execute")
        {
            let matched = captures.get(0).expect("group zero should exist");
            count += 1;
            position = matched.end();
        }
        assert_eq!(count, 20_000);
    }

    #[test]
    fn capture_corrected_no_match_stays_on_the_linear_prefilter() {
        let re = compile_regex("(a+)+$", "").expect("nested repeat should compile");
        let input = format!("{}!", "a".repeat(4_096));
        let CompiledRegex::CaptureCorrected { captures, .. } = &re else {
            panic!("nested quantified capture should use the hybrid backend");
        };
        assert!(captures
            .find(&input)
            .expect("non-nullable repeated captures should stay linear")
            .is_none());
        assert!(re
            .find(&input)
            .expect("the linear prefilter should reject without a backend error")
            .is_none());
    }

    #[test]
    fn unicode_word_boundary_prefilter_is_superset_only() {
        let non_boundary =
            compile_regex(r"^\B(a)*", "iu").expect("Unicode ignore-case boundary should compile");
        let CompiledRegex::PrefilteredExact {
            prefilter, exact, ..
        } = &non_boundary
        else {
            panic!("Unicode ignore-case boundary should use the exact hybrid backend");
        };
        assert!(prefilter.find("é").is_some());
        assert!(exact
            .find("é")
            .expect("exact backend should execute")
            .is_some());

        let captures = non_boundary
            .captures_at("é", 0)
            .expect("captures should execute")
            .expect("non-boundary should match");
        let whole = captures.get(0).expect("group zero should exist");
        assert_eq!((whole.start(), whole.end()), (0, 0));
        assert_eq!(captures.get(1).map(CompiledMatch::as_str), None);

        let boundary =
            compile_regex(r"^\b(a)*", "iu").expect("Unicode ignore-case boundary should compile");
        let CompiledRegex::PrefilteredExact { prefilter, .. } = &boundary else {
            panic!("Unicode ignore-case boundary should use the exact hybrid backend");
        };
        assert!(
            prefilter.find("é").is_some(),
            "relaxed assertion may produce a false positive"
        );
        assert!(boundary
            .find("é")
            .expect("exact backend should execute")
            .is_none());

        let transition =
            compile_regex(r"é\b(a)*", "iu").expect("Unicode ignore-case transition should compile");
        let matched = transition
            .find("éa")
            .expect("transition should execute")
            .expect("non-word to word boundary should match");
        assert_eq!(
            (matched.as_str(), matched.start(), matched.end()),
            ("éa", 0, 3)
        );
    }

    #[test]
    fn unicode_word_boundary_exact_iteration_advances_from_actual_empty_match() {
        let re =
            compile_regex(r"\B(a)*", "iu").expect("Unicode ignore-case boundary should compile");
        let found = re.find_iter("é😀").expect("find iteration should execute");
        assert_eq!(
            found
                .iter()
                .map(|matched| (matched.start(), matched.end()))
                .collect::<Vec<_>>(),
            vec![(0, 0), (2, 2), (6, 6)]
        );

        let later =
            compile_regex(r"\b", "iu").expect("Unicode ignore-case boundary should compile");
        assert_eq!(
            later
                .find_iter("éK")
                .expect("later empty matches should execute")
                .iter()
                .map(|matched| (matched.start(), matched.end()))
                .collect::<Vec<_>>(),
            vec![(2, 2), (5, 5)]
        );
        let captures = re
            .captures_iter("é😀")
            .expect("capture iteration should execute");
        assert_eq!(
            captures
                .iter()
                .map(|groups| {
                    let matched = groups.get(0).expect("group zero should exist");
                    (matched.start(), matched.end())
                })
                .collect::<Vec<_>>(),
            vec![(0, 0), (2, 2), (6, 6)]
        );
        assert_eq!(
            re.replace_all("é😀", "X")
                .expect("replacement should execute"),
            "XéX😀X"
        );

        let hard = compile_regex(r"(?=)\B(a)*", "iu")
            .expect("hard Unicode ignore-case boundary should compile");
        assert!(matches!(&hard, CompiledRegex::Fancy(_)));
        let hard_found = hard
            .find_iter("é😀")
            .expect("hard find iteration should execute");
        assert_eq!(
            hard_found
                .iter()
                .map(|matched| (matched.start(), matched.end()))
                .collect::<Vec<_>>(),
            vec![(0, 0), (2, 2), (6, 6)]
        );
    }

    #[test]
    fn unicode_word_boundary_prefilter_preserves_hostile_no_match_rejection() {
        let re = compile_regex(r"^(a+)+\b$", "iu").expect("nested boundary pattern should compile");
        let input = format!("{}!", "a".repeat(4_096));
        let CompiledRegex::PrefilteredExact { exact, .. } = &re else {
            panic!("Unicode ignore-case boundary should use the exact hybrid backend");
        };
        assert!(exact
            .find(&input)
            .expect("non-nullable repeated captures should stay linear")
            .is_none());
        assert!(re
            .find(&input)
            .expect("linear superset prefilter should reject")
            .is_none());

        let false_positive =
            compile_regex(r"^(a+)+\B$", "iu").expect("nested non-boundary pattern should compile");
        assert!(false_positive
            .find(&"a".repeat(4_096))
            .expect("the exact boundary fast path should reject linearly")
            .is_none());

        let disagreement = compile_regex(r"^(é+)+\b$", "iu")
            .expect("nested non-ASCII boundary pattern should compile");
        let CompiledRegex::PrefilteredExact { exact, .. } = &disagreement else {
            panic!("Unicode ignore-case boundary should use the exact hybrid backend");
        };
        let disagreement_input = "é".repeat(4_096);
        assert!(exact
            .find(&disagreement_input)
            .expect("non-ASCII non-nullable repeats should stay linear")
            .is_none());
        assert!(disagreement
            .find(&disagreement_input)
            .expect("non-ASCII exact no-match should not exhaust work")
            .is_none());
    }

    #[test]
    fn unicode_word_boundary_large_linear_scan_does_not_hit_work_limit() {
        let re = compile_regex(r"\b", "iu").expect("boundary should compile");
        assert!(re
            .find(&"é".repeat(1_000_001))
            .expect("linear position scanning is not backtracking work")
            .is_none());

        let repeated_capture =
            compile_regex(r"^(a)*\b$", "iu").expect("long repeated capture should compile");
        let repeated_input = "a".repeat(100_001);
        let CompiledRegex::PrefilteredExact {
            exact,
            linear_exact: Some(linear_exact),
            ..
        } = &repeated_capture
        else {
            panic!("repeated boundary capture should use both exact matchers");
        };
        assert!(
            linear_exact
                .find(&repeated_input)
                .expect("linear exact matcher should execute")
                .is_some(),
            "linear pattern {:?}",
            linear_exact.as_str()
        );
        assert!(
            exact
                .captures(&repeated_input)
                .expect("capture matcher should execute")
                .is_some(),
            "capture pattern {:?}",
            exact.as_str()
        );
        let captures = repeated_capture
            .captures(&repeated_input)
            .expect("deterministic repeated captures should stay within bounded storage")
            .expect("the repeated capture should match");
        assert_eq!(
            captures.get(0).map(CompiledMatch::as_str),
            Some(repeated_input.as_str())
        );
        assert_eq!(captures.get(1).map(CompiledMatch::as_str), Some("a"));

        let alternating = compile_regex(r"^(?:(a)|(b))+\b$", "iu")
            .expect("alternating repeated captures should compile");
        let alternating = alternating
            .captures("ab")
            .expect("alternating captures should execute")
            .expect("alternating captures should match");
        assert_eq!(alternating.get(1).map(CompiledMatch::as_str), None);
        assert_eq!(alternating.get(2).map(CompiledMatch::as_str), Some("b"));
    }

    #[test]
    fn exact_position_and_first_replace_do_not_search_the_suffix() {
        let sticky = compile_regex(r"\b(a+)+$", "iu").expect("sticky probe should compile");
        let sticky_input = format!("é{}", "a".repeat(4_096));
        assert!(sticky
            .captures_at(&sticky_input, 0)
            .expect("ordinary search should execute")
            .is_some());
        assert!(sticky
            .captures_exact_at(&sticky_input, 0)
            .expect("exact-position search should execute")
            .is_none());

        let replace =
            compile_regex(r"^\B|(a+)+b", "iu").expect("first-replacement probe should compile");
        let replace_input = format!("é{}!", "a".repeat(4_096));
        let replaced = replace
            .replace(&replace_input, "X")
            .expect("non-global replacement should not inspect later matches");
        assert_eq!(replaced, format!("X{replace_input}"));
    }
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
    match error {
        fancy_regex::Error::RuntimeError(_) => Error::fuel(format!("Invalid regex match: {error}")),
        _ => Error::syntax(format!("Invalid regex match: {error}")),
    }
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
    match escape {
        'b' => out.push_str(r"\b"),
        'B' => out.push_str(r"\B"),
        _ => unreachable!(),
    }
    if unicode_ignore_case {
        out.push_str("{ruja-ecma-unicode-i}");
    } else {
        out.push_str("{ruja-ecma}");
    }
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
    relaxed_unicode_word_boundary: bool,
}

fn empty_ecmascript_class_backend_atom(negated: bool, unicode_mode: bool) -> &'static str {
    if !negated {
        r"[^\s\S]"
    } else if unicode_mode {
        "(?s:.)"
    } else {
        r"[\x00-\u{ffff}\u{f0000}-\u{f07ff}]"
    }
}

fn normalize_regex_for_backend(
    source: &str,
    flags: &str,
    capture_count: usize,
    code_unit_input: bool,
    fancy_backend: bool,
    relax_unicode_word_boundaries: bool,
    capture_indices: &IndexMap<Arc<str>, Vec<usize>>,
) -> Result<NormalizedRegex, String> {
    debug_assert!(!(fancy_backend && relax_unicode_word_boundaries));
    let unicode_mode = flags.contains('u') || flags.contains('v');
    if source == "[]" {
        return Ok(NormalizedRegex {
            source: empty_ecmascript_class_backend_atom(false, unicode_mode).to_string(),
            backref_sets: Vec::new(),
            relaxed_unicode_word_boundary: false,
        });
    }
    if source == "[^]" {
        return Ok(NormalizedRegex {
            source: empty_ecmascript_class_backend_atom(true, unicode_mode).to_string(),
            backref_sets: Vec::new(),
            relaxed_unicode_word_boundary: false,
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
    let mut relaxed_unicode_word_boundary = false;
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
                } else if unicode_mode {
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
            {
                // Complex v-classes stay as backend set algebra, but each word
                // operand must still use ECMAScript's narrow inventory.
                class_has_active_word_escape |= materialize_current_word_class;
                push_ecmascript_word_escape_for_backend(&mut out, ch, true, unicode_mode);
            } else if !in_class
                && matches!(ch, 'w' | 'W')
                && modifier_stack.last().is_some_and(|state| state.ignore_case)
            {
                push_ecmascript_word_escape_for_backend(&mut out, ch, false, unicode_mode);
            } else if !in_class
                && matches!(ch, 'b' | 'B')
                && unicode_mode
                && modifier_stack.last().is_some_and(|state| state.ignore_case)
                && relax_unicode_word_boundaries
            {
                out.pop();
                out.push_str("(?:)");
                relaxed_unicode_word_boundary = true;
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
            } else if ch == 'c' {
                let control = chars.peek().copied().and_then(|next| {
                    crate::lexer::regex_control_escape_value(next, in_class, unicode_mode)
                });
                out.pop();
                if let Some(control) = control {
                    chars.next();
                    out.push_str("\\x");
                    out.push_str(&format!("{control:02x}"));
                } else if !unicode_mode {
                    // Annex B parses an otherwise incomplete `\c` as a
                    // literal reverse solidus followed by a separate `c` atom.
                    push_regex_literal_for_backend(&mut out, '\\');
                    out.push('c');
                } else {
                    out.push_str("\\c");
                }
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
            } else if !unicode_mode && !regex_backend_escape_passthrough(ch) {
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

        // Canonical internal strings keep scalars that overlap the sentinel
        // range as two sentinel-backed UTF-16 units. Unicode patterns consume
        // that valid pair as one code point, including inside character
        // classes. Adjacent lone high/low surrogates have the same JS string
        // value and therefore follow the same rule.
        if unicode_mode {
            if let Some(high) = crate::value::utf16_single_unit_from_internal_char(ch)
                .filter(|unit| (0xd800..=0xdbff).contains(unit))
            {
                if let Some(low) = chars
                    .peek()
                    .and_then(|next| crate::value::utf16_single_unit_from_internal_char(*next))
                    .filter(|unit| (0xdc00..=0xdfff).contains(unit))
                {
                    chars.next();
                    let scalar = 0x10000 + (((high as u32 - 0xd800) << 10) | (low as u32 - 0xdc00));
                    out.push(char::from_u32(scalar).expect("valid surrogate pair scalar"));
                    continue;
                }
            }
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
            let empty_class = class_output_start.and_then(|start| match &out[start..] {
                "[]" => Some(false),
                "[^]" => Some(true),
                _ => None,
            });
            if let Some(negated) = empty_class {
                let start = class_output_start
                    .take()
                    .expect("empty class must have an output start");
                out.truncate(start);
                out.push_str(empty_ecmascript_class_backend_atom(negated, unicode_mode));
                class_has_active_word_escape = false;
                materialize_current_word_class = true;
                continue;
            }
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

        if !in_class && ch == '.' {
            if unicode_mode {
                if modifier_stack.last().is_some_and(|state| state.dot_all) {
                    out.push_str("(?s:.)");
                } else {
                    out.push_str(r"[^\n\r\u{2028}\u{2029}]");
                }
            } else if modifier_stack.last().is_some_and(|state| state.dot_all) {
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
        relaxed_unicode_word_boundary,
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

fn regex_backend_escape_passthrough(ch: char) -> bool {
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
    )
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
                                | "SyntaxError" | "TypeError" | "URIError" | "AggregateError"
                                | "SuppressedError",
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
        &PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
                    PropertyKey::symbol(species_symbol),
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
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
                PropertyKey::symbol(vm.well_known_symbols.iterator),
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
                    PropertyKey::symbol(vm.well_known_symbols.species),
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
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
            PropertyKey::symbol(vm.well_known_symbols.species),
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
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });

    let constructor = Value::Object(constructor);
    let prototype = Value::Object(prototype);
    let realm = crate::environment::global_env_root(&vm.heap, env);
    vm.realm_weakref_prototypes
        .insert(realm.0, prototype.clone());
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
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });

    let constructor = Value::Object(constructor);
    let prototype = Value::Object(prototype);
    let realm = crate::environment::global_env_root(&vm.heap, env);
    vm.realm_finalization_registry_prototypes
        .insert(realm.0, prototype.clone());
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

    let (ctor_native, ctor_length): (NativeFn, usize) = match name {
        "AggregateError" => (aggregate_error_constructor, 2),
        "SuppressedError" => (suppressed_error_constructor, 3),
        _ => (error_constructor, 1),
    };
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
    });
    if name == "Error" {
        let ts_fn = vm.new_native_function_in_env("toString", error_to_string, 0, env)?;
        let stack_get = vm.new_native_function_in_env("get stack", error_stack_get, 0, env)?;
        let stack_set = vm.new_native_function_in_env("set stack", error_stack_set, 1, env)?;
        vm.heap.with_obj(proto_idx.0, |obj| {
            obj.props().lock().insert(
                PropertyKey::from("toString"),
                data_prop(Value::Object(ts_fn)),
            );
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

fn define_realm_global_const(vm: &mut Vm, env: GcIdx, global: &Value, name: &str, value: Value) {
    crate::environment::declare(&vm.heap, env, name, value.clone(), BindingKind::Const);
    if let Value::Object(index) = global {
        vm.heap.with_obj(index.0, |object| {
            object
                .props()
                .lock()
                .insert(PropertyKey::from(name), const_prop(value));
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
            ("compile", regexp_compile, 2),
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
                PropertyKey::symbol(vm.well_known_symbols.r#match),
                data_prop(Value::Object(match_fn)),
            );
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.match_all),
                data_prop(Value::Object(match_all_fn)),
            );
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.search),
                data_prop(Value::Object(search_fn)),
            );
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.replace),
                data_prop(Value::Object(replace_fn)),
            );
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.split),
                data_prop(Value::Object(split_fn)),
            );
        }
    });
    let regexp_species_getter =
        vm.new_native_function_in_env("get [Symbol.species]", promise_species_get, 0, env)?;
    let regexp_escape_fn = vm.new_native_function_in_env("escape", regexp_escape, 1, env)?;
    install_regexp_legacy_static_properties(vm, regex_ctor, env)?;
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
                PropertyKey::symbol(vm.well_known_symbols.species),
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

const ANNEX_B_STRING_HTML_METHODS: &[(&str, NativeFn, usize)] = &[
    ("anchor", str_anchor, 1),
    ("big", str_big, 0),
    ("blink", str_blink, 0),
    ("bold", str_bold, 0),
    ("fixed", str_fixed, 0),
    ("fontcolor", str_fontcolor, 1),
    ("fontsize", str_fontsize, 1),
    ("italics", str_italics, 0),
    ("link", str_link, 1),
    ("small", str_small, 0),
    ("strike", str_strike, 0),
    ("sub", str_sub, 0),
    ("sup", str_sup, 0),
];

fn install_annex_b_string_methods_in_env(
    vm: &mut Vm,
    realm: GcIdx,
    prototype: GcIdx,
) -> error::Result<()> {
    vm.try_reserve_gc_pins(1)?;
    let prototype_value = Value::Object(prototype);
    let prototype_pin = vm.pin(&prototype_value);
    let result = (|| {
        for &(name, function, length) in ANNEX_B_STRING_HTML_METHODS {
            let method = vm.new_native_function_in_env(name, function, length, realm)?;
            vm.heap.with_obj(prototype.0, |object| {
                object
                    .props()
                    .lock()
                    .insert(PropertyKey::from(name), data_prop(Value::Object(method)));
            });
        }

        let (trim_start, trim_end) = vm.heap.with_obj(prototype.0, |object| {
            let properties = object.props();
            let properties = properties.lock();
            (
                properties
                    .get(&PropertyKey::from("trimStart"))
                    .map(|descriptor| descriptor.value.clone()),
                properties
                    .get(&PropertyKey::from("trimEnd"))
                    .map(|descriptor| descriptor.value.clone()),
            )
        });
        let trim_start = trim_start
            .ok_or_else(|| Error::internal("missing String.prototype.trimStart intrinsic"))?;
        let trim_end = trim_end
            .ok_or_else(|| Error::internal("missing String.prototype.trimEnd intrinsic"))?;
        vm.heap.with_obj(prototype.0, |object| {
            let properties = object.props();
            let mut properties = properties.lock();
            properties.insert(PropertyKey::from("trimLeft"), data_prop(trim_start));
            properties.insert(PropertyKey::from("trimRight"), data_prop(trim_end));
        });
        Ok(())
    })();
    vm.unpin_many(prototype_pin);
    result
}

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
    vm.try_reserve_gc_pins(3)?;
    let mut pin_count = vm.pin_many(&[constructor_value.clone(), prototype_value.clone()]);

    let setup = (|| -> error::Result<Value> {
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

        let unscopables = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(None),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&unscopables);
        // `with` is absent because it is a reserved word and cannot be an
        // unqualified identifier inside a with statement.
        for name in [
            "at",
            "copyWithin",
            "entries",
            "fill",
            "find",
            "findIndex",
            "findLast",
            "findLastIndex",
            "flat",
            "flatMap",
            "includes",
            "keys",
            "toReversed",
            "toSorted",
            "toSpliced",
            "values",
        ] {
            vm.define_own_property_or_throw(
                &unscopables,
                PropertyKey::from(name),
                PropertyDescriptor::data(Value::Bool(true)),
            )?;
        }
        let mut descriptor = data_prop(unscopables.clone());
        descriptor.writable = false;
        vm.define_own_property_or_throw(
            &prototype_value,
            PropertyKey::symbol(vm.well_known_symbols.unscopables),
            descriptor,
        )?;

        let values = vm.get_property(&prototype_value, "values")?;
        vm.heap.with_obj(prototype.0, |object| {
            object.props().lock().insert(
                PropertyKey::symbol(vm.well_known_symbols.iterator),
                data_prop(values.clone()),
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
                PropertyKey::symbol(vm.well_known_symbols.species),
                accessor_get_prop(Value::Object(species)),
            );
        });

        Ok(values)
    })();
    vm.unpin_many(pin_count);
    let values = setup?;

    vm.realm_array_values_functions.insert(env.0, values);
    vm.realm_array_constructors
        .insert(env.0, constructor_value.clone());
    vm.realm_array_prototypes
        .insert(env.0, prototype_value.clone());
    if env == vm.global {
        define_global(vm, "Array", constructor_value);
    } else if let Some(global) = realm_global {
        define_realm_global(vm, env, global, "Array", constructor_value);
    }
    Ok((constructor, prototype))
}

#[cfg(test)]
pub(crate) fn reinstall_array_intrinsic_for_test(vm: &mut Vm, env: GcIdx) -> error::Result<()> {
    install_array_intrinsic_in_env(vm, env, None).map(|_| ())
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
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
            PropertyKey::symbol(vm.well_known_symbols.species),
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
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
            PropertyKey::symbol(vm.well_known_symbols.async_iterator),
            data_prop(Value::Object(async_iterator)),
        );
        props.insert(
            PropertyKey::symbol(vm.well_known_symbols.async_dispose),
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
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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

pub(crate) fn install_temporal_namespace_in_env(
    vm: &mut Vm,
    env: GcIdx,
    global: Option<&Value>,
    object_proto: Value,
) -> error::Result<Value> {
    vm.try_reserve_gc_pins(177)?;
    let mut pin_count = 0;
    let result = (|| {
        let instant_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&instant_prototype);

        let instant_constructor = Value::Object(vm.new_native_constructor_in_env_with_gc_retry(
            "Instant",
            temporal_instant_constructor,
            1,
            env,
            NativeConstructMode::InternalDeferredPrototype,
        )?);
        pin_count += vm.pin(&instant_constructor);
        let from_epoch_milliseconds = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "fromEpochMilliseconds",
            temporal_instant_from_epoch_milliseconds,
            1,
            env,
        )?);
        pin_count += vm.pin(&from_epoch_milliseconds);
        let from_epoch_nanoseconds = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "fromEpochNanoseconds",
            temporal_instant_from_epoch_nanoseconds,
            1,
            env,
        )?);
        pin_count += vm.pin(&from_epoch_nanoseconds);
        let epoch_milliseconds = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "get epochMilliseconds",
            temporal_instant_epoch_milliseconds,
            0,
            env,
        )?);
        pin_count += vm.pin(&epoch_milliseconds);
        let epoch_nanoseconds = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "get epochNanoseconds",
            temporal_instant_epoch_nanoseconds,
            0,
            env,
        )?);
        pin_count += vm.pin(&epoch_nanoseconds);
        let equals = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "equals",
            temporal_instant_equals,
            1,
            env,
        )?);
        pin_count += vm.pin(&equals);
        let from = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "from",
            temporal_instant_from,
            1,
            env,
        )?);
        pin_count += vm.pin(&from);
        let value_of = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "valueOf",
            temporal_instant_value_of,
            0,
            env,
        )?);
        pin_count += vm.pin(&value_of);
        let to_string = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "toString",
            temporal_instant_to_string,
            0,
            env,
        )?);
        pin_count += vm.pin(&to_string);
        let compare = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "compare",
            temporal_instant_compare,
            2,
            env,
        )?);
        pin_count += vm.pin(&compare);

        let zoned_date_time_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&zoned_date_time_prototype);
        let zoned_date_time_constructor =
            Value::Object(vm.new_native_constructor_in_env_with_gc_retry(
                "ZonedDateTime",
                temporal_zoned_date_time_constructor,
                2,
                env,
                NativeConstructMode::InternalDeferredPrototype,
            )?);
        pin_count += vm.pin(&zoned_date_time_constructor);
        let zoned_epoch_milliseconds = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "get epochMilliseconds",
            temporal_zoned_date_time_epoch_milliseconds,
            0,
            env,
        )?);
        pin_count += vm.pin(&zoned_epoch_milliseconds);
        let zoned_epoch_nanoseconds = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "get epochNanoseconds",
            temporal_zoned_date_time_epoch_nanoseconds,
            0,
            env,
        )?);
        pin_count += vm.pin(&zoned_epoch_nanoseconds);
        let time_zone_id = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "get timeZoneId",
            temporal_zoned_date_time_time_zone_id,
            0,
            env,
        )?);
        pin_count += vm.pin(&time_zone_id);
        let calendar_id = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "get calendarId",
            temporal_zoned_date_time_calendar_id,
            0,
            env,
        )?);
        pin_count += vm.pin(&calendar_id);

        macro_rules! alloc_zoned_native {
            ($binding:ident, $name:literal, $native:ident, $length:expr) => {
                let $binding = Value::Object(
                    vm.new_native_function_in_env_with_gc_retry($name, $native, $length, env)?,
                );
                pin_count += vm.pin(&$binding);
            };
        }

        alloc_zoned_native!(zoned_from, "from", temporal_zoned_date_time_from, 1);
        alloc_zoned_native!(
            zoned_compare,
            "compare",
            temporal_zoned_date_time_compare,
            2
        );
        alloc_zoned_native!(zoned_equals, "equals", temporal_zoned_date_time_equals, 1);
        alloc_zoned_native!(
            zoned_with_time_zone,
            "withTimeZone",
            temporal_zoned_date_time_with_time_zone,
            1
        );
        alloc_zoned_native!(
            zoned_with_calendar,
            "withCalendar",
            temporal_zoned_date_time_with_calendar,
            1
        );
        alloc_zoned_native!(zoned_era, "get era", temporal_zoned_date_time_era, 0);
        alloc_zoned_native!(
            zoned_era_year,
            "get eraYear",
            temporal_zoned_date_time_era_year,
            0
        );
        alloc_zoned_native!(zoned_year, "get year", temporal_zoned_date_time_year, 0);
        alloc_zoned_native!(zoned_month, "get month", temporal_zoned_date_time_month, 0);
        alloc_zoned_native!(
            zoned_month_code,
            "get monthCode",
            temporal_zoned_date_time_month_code,
            0
        );
        alloc_zoned_native!(zoned_day, "get day", temporal_zoned_date_time_day, 0);
        alloc_zoned_native!(zoned_hour, "get hour", temporal_zoned_date_time_hour, 0);
        alloc_zoned_native!(
            zoned_minute,
            "get minute",
            temporal_zoned_date_time_minute,
            0
        );
        alloc_zoned_native!(
            zoned_second,
            "get second",
            temporal_zoned_date_time_second,
            0
        );
        alloc_zoned_native!(
            zoned_millisecond,
            "get millisecond",
            temporal_zoned_date_time_millisecond,
            0
        );
        alloc_zoned_native!(
            zoned_microsecond,
            "get microsecond",
            temporal_zoned_date_time_microsecond,
            0
        );
        alloc_zoned_native!(
            zoned_nanosecond,
            "get nanosecond",
            temporal_zoned_date_time_nanosecond,
            0
        );
        alloc_zoned_native!(
            zoned_day_of_week,
            "get dayOfWeek",
            temporal_zoned_date_time_day_of_week,
            0
        );
        alloc_zoned_native!(
            zoned_day_of_year,
            "get dayOfYear",
            temporal_zoned_date_time_day_of_year,
            0
        );
        alloc_zoned_native!(
            zoned_week_of_year,
            "get weekOfYear",
            temporal_zoned_date_time_week_of_year,
            0
        );
        alloc_zoned_native!(
            zoned_year_of_week,
            "get yearOfWeek",
            temporal_zoned_date_time_year_of_week,
            0
        );
        alloc_zoned_native!(
            zoned_hours_in_day,
            "get hoursInDay",
            temporal_zoned_date_time_hours_in_day,
            0
        );
        alloc_zoned_native!(
            zoned_days_in_week,
            "get daysInWeek",
            temporal_zoned_date_time_days_in_week,
            0
        );
        alloc_zoned_native!(
            zoned_days_in_month,
            "get daysInMonth",
            temporal_zoned_date_time_days_in_month,
            0
        );
        alloc_zoned_native!(
            zoned_days_in_year,
            "get daysInYear",
            temporal_zoned_date_time_days_in_year,
            0
        );
        alloc_zoned_native!(
            zoned_months_in_year,
            "get monthsInYear",
            temporal_zoned_date_time_months_in_year,
            0
        );
        alloc_zoned_native!(
            zoned_in_leap_year,
            "get inLeapYear",
            temporal_zoned_date_time_in_leap_year,
            0
        );
        alloc_zoned_native!(
            zoned_offset_nanoseconds,
            "get offsetNanoseconds",
            temporal_zoned_date_time_offset_nanoseconds,
            0
        );
        alloc_zoned_native!(
            zoned_offset,
            "get offset",
            temporal_zoned_date_time_offset,
            0
        );
        alloc_zoned_native!(
            zoned_to_instant,
            "toInstant",
            temporal_zoned_date_time_to_instant,
            0
        );
        alloc_zoned_native!(
            zoned_to_plain_date_time,
            "toPlainDateTime",
            temporal_zoned_date_time_to_plain_date_time,
            0
        );
        alloc_zoned_native!(
            zoned_to_string,
            "toString",
            temporal_zoned_date_time_to_string,
            0
        );
        alloc_zoned_native!(zoned_to_json, "toJSON", temporal_zoned_date_time_to_json, 0);
        alloc_zoned_native!(
            zoned_value_of,
            "valueOf",
            temporal_zoned_date_time_value_of,
            0
        );
        alloc_zoned_native!(
            zoned_start_of_day,
            "startOfDay",
            temporal_zoned_date_time_start_of_day,
            0
        );

        let duration_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&duration_prototype);
        let duration_constructor = Value::Object(vm.new_native_constructor_in_env_with_gc_retry(
            "Duration",
            temporal_duration_constructor,
            0,
            env,
            NativeConstructMode::InternalDeferredPrototype,
        )?);
        pin_count += vm.pin(&duration_constructor);

        macro_rules! alloc_duration_getter {
            ($binding:ident, $name:literal, $native:ident) => {
                let $binding = Value::Object(
                    vm.new_native_function_in_env_with_gc_retry($name, $native, 0, env)?,
                );
                pin_count += vm.pin(&$binding);
            };
        }

        alloc_duration_getter!(duration_years, "get years", temporal_duration_years);
        alloc_duration_getter!(duration_months, "get months", temporal_duration_months);
        alloc_duration_getter!(duration_weeks, "get weeks", temporal_duration_weeks);
        alloc_duration_getter!(duration_days, "get days", temporal_duration_days);
        alloc_duration_getter!(duration_hours, "get hours", temporal_duration_hours);
        alloc_duration_getter!(duration_minutes, "get minutes", temporal_duration_minutes);
        alloc_duration_getter!(duration_seconds, "get seconds", temporal_duration_seconds);
        alloc_duration_getter!(
            duration_milliseconds,
            "get milliseconds",
            temporal_duration_milliseconds
        );
        alloc_duration_getter!(
            duration_microseconds,
            "get microseconds",
            temporal_duration_microseconds
        );
        alloc_duration_getter!(
            duration_nanoseconds,
            "get nanoseconds",
            temporal_duration_nanoseconds
        );
        alloc_duration_getter!(duration_sign, "get sign", temporal_duration_sign);
        alloc_duration_getter!(duration_blank, "get blank", temporal_duration_blank);

        let plain_date_time_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&plain_date_time_prototype);
        let plain_date_time_constructor =
            Value::Object(vm.new_native_constructor_in_env_with_gc_retry(
                "PlainDateTime",
                temporal_plain_date_time_constructor,
                3,
                env,
                NativeConstructMode::InternalDeferredPrototype,
            )?);
        pin_count += vm.pin(&plain_date_time_constructor);
        let plain_date_time_from = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "from",
            temporal_plain_date_time_from,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_date_time_from);
        let plain_date_time_compare = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "compare",
            temporal_plain_date_time_compare,
            2,
            env,
        )?);
        pin_count += vm.pin(&plain_date_time_compare);

        macro_rules! alloc_plain_date_time_getter {
            ($binding:ident, $name:literal, $native:ident) => {
                let $binding = Value::Object(
                    vm.new_native_function_in_env_with_gc_retry($name, $native, 0, env)?,
                );
                pin_count += vm.pin(&$binding);
            };
        }

        alloc_plain_date_time_getter!(
            plain_calendar_id,
            "get calendarId",
            temporal_plain_date_time_calendar_id
        );
        alloc_plain_date_time_getter!(plain_era, "get era", temporal_plain_date_time_era);
        alloc_plain_date_time_getter!(
            plain_era_year,
            "get eraYear",
            temporal_plain_date_time_era_year
        );
        alloc_plain_date_time_getter!(plain_year, "get year", temporal_plain_date_time_year);
        alloc_plain_date_time_getter!(plain_month, "get month", temporal_plain_date_time_month);
        alloc_plain_date_time_getter!(
            plain_month_code,
            "get monthCode",
            temporal_plain_date_time_month_code
        );
        alloc_plain_date_time_getter!(plain_day, "get day", temporal_plain_date_time_day);
        alloc_plain_date_time_getter!(plain_hour, "get hour", temporal_plain_date_time_hour);
        alloc_plain_date_time_getter!(plain_minute, "get minute", temporal_plain_date_time_minute);
        alloc_plain_date_time_getter!(plain_second, "get second", temporal_plain_date_time_second);
        alloc_plain_date_time_getter!(
            plain_millisecond,
            "get millisecond",
            temporal_plain_date_time_millisecond
        );
        alloc_plain_date_time_getter!(
            plain_microsecond,
            "get microsecond",
            temporal_plain_date_time_microsecond
        );
        alloc_plain_date_time_getter!(
            plain_nanosecond,
            "get nanosecond",
            temporal_plain_date_time_nanosecond
        );
        alloc_plain_date_time_getter!(
            plain_day_of_week,
            "get dayOfWeek",
            temporal_plain_date_time_day_of_week
        );
        alloc_plain_date_time_getter!(
            plain_day_of_year,
            "get dayOfYear",
            temporal_plain_date_time_day_of_year
        );
        alloc_plain_date_time_getter!(
            plain_week_of_year,
            "get weekOfYear",
            temporal_plain_date_time_week_of_year
        );
        alloc_plain_date_time_getter!(
            plain_year_of_week,
            "get yearOfWeek",
            temporal_plain_date_time_year_of_week
        );
        alloc_plain_date_time_getter!(
            plain_days_in_week,
            "get daysInWeek",
            temporal_plain_date_time_days_in_week
        );
        alloc_plain_date_time_getter!(
            plain_days_in_month,
            "get daysInMonth",
            temporal_plain_date_time_days_in_month
        );
        alloc_plain_date_time_getter!(
            plain_days_in_year,
            "get daysInYear",
            temporal_plain_date_time_days_in_year
        );
        alloc_plain_date_time_getter!(
            plain_months_in_year,
            "get monthsInYear",
            temporal_plain_date_time_months_in_year
        );
        alloc_plain_date_time_getter!(
            plain_in_leap_year,
            "get inLeapYear",
            temporal_plain_date_time_in_leap_year
        );
        let plain_value_of = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "valueOf",
            temporal_plain_date_time_value_of,
            0,
            env,
        )?);
        pin_count += vm.pin(&plain_value_of);
        let plain_equals = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "equals",
            temporal_plain_date_time_equals,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_equals);

        let plain_date_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&plain_date_prototype);
        let plain_date_constructor =
            Value::Object(vm.new_native_constructor_in_env_with_gc_retry(
                "PlainDate",
                temporal_plain_date_constructor,
                3,
                env,
                NativeConstructMode::InternalDeferredPrototype,
            )?);
        pin_count += vm.pin(&plain_date_constructor);
        let plain_date_from = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "from",
            temporal_plain_date_from,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_date_from);
        let plain_date_compare = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "compare",
            temporal_plain_date_compare,
            2,
            env,
        )?);
        pin_count += vm.pin(&plain_date_compare);

        macro_rules! alloc_plain_date_getter {
            ($binding:ident, $name:literal, $native:ident) => {
                let $binding = Value::Object(
                    vm.new_native_function_in_env_with_gc_retry($name, $native, 0, env)?,
                );
                pin_count += vm.pin(&$binding);
            };
        }

        alloc_plain_date_getter!(
            plain_date_calendar_id,
            "get calendarId",
            temporal_plain_date_calendar_id
        );
        alloc_plain_date_getter!(plain_date_era, "get era", temporal_plain_date_era);
        alloc_plain_date_getter!(
            plain_date_era_year,
            "get eraYear",
            temporal_plain_date_era_year
        );
        alloc_plain_date_getter!(plain_date_year, "get year", temporal_plain_date_year);
        alloc_plain_date_getter!(plain_date_month, "get month", temporal_plain_date_month);
        alloc_plain_date_getter!(
            plain_date_month_code,
            "get monthCode",
            temporal_plain_date_month_code
        );
        alloc_plain_date_getter!(plain_date_day, "get day", temporal_plain_date_day);
        alloc_plain_date_getter!(
            plain_date_day_of_week,
            "get dayOfWeek",
            temporal_plain_date_day_of_week
        );
        alloc_plain_date_getter!(
            plain_date_day_of_year,
            "get dayOfYear",
            temporal_plain_date_day_of_year
        );
        alloc_plain_date_getter!(
            plain_date_week_of_year,
            "get weekOfYear",
            temporal_plain_date_week_of_year
        );
        alloc_plain_date_getter!(
            plain_date_year_of_week,
            "get yearOfWeek",
            temporal_plain_date_year_of_week
        );
        alloc_plain_date_getter!(
            plain_date_days_in_week,
            "get daysInWeek",
            temporal_plain_date_days_in_week
        );
        alloc_plain_date_getter!(
            plain_date_days_in_month,
            "get daysInMonth",
            temporal_plain_date_days_in_month
        );
        alloc_plain_date_getter!(
            plain_date_days_in_year,
            "get daysInYear",
            temporal_plain_date_days_in_year
        );
        alloc_plain_date_getter!(
            plain_date_months_in_year,
            "get monthsInYear",
            temporal_plain_date_months_in_year
        );
        alloc_plain_date_getter!(
            plain_date_in_leap_year,
            "get inLeapYear",
            temporal_plain_date_in_leap_year
        );
        let plain_date_value_of = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "valueOf",
            temporal_plain_date_value_of,
            0,
            env,
        )?);
        pin_count += vm.pin(&plain_date_value_of);
        let plain_date_equals = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "equals",
            temporal_plain_date_equals,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_date_equals);
        let plain_date_to_string = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "toString",
            temporal_plain_date_to_string,
            0,
            env,
        )?);
        pin_count += vm.pin(&plain_date_to_string);
        let plain_date_to_json = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "toJSON",
            temporal_plain_date_to_json,
            0,
            env,
        )?);
        pin_count += vm.pin(&plain_date_to_json);
        let plain_date_to_plain_date_time =
            Value::Object(vm.new_native_function_in_env_with_gc_retry(
                "toPlainDateTime",
                temporal_plain_date_to_plain_date_time,
                0,
                env,
            )?);
        pin_count += vm.pin(&plain_date_to_plain_date_time);

        let plain_time_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&plain_time_prototype);
        let plain_time_constructor =
            Value::Object(vm.new_native_constructor_in_env_with_gc_retry(
                "PlainTime",
                temporal_plain_time_constructor,
                0,
                env,
                NativeConstructMode::InternalDeferredPrototype,
            )?);
        pin_count += vm.pin(&plain_time_constructor);
        let plain_time_from = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "from",
            temporal_plain_time_from,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_time_from);

        macro_rules! alloc_plain_time_getter {
            ($binding:ident, $name:literal, $native:ident) => {
                let $binding = Value::Object(
                    vm.new_native_function_in_env_with_gc_retry($name, $native, 0, env)?,
                );
                pin_count += vm.pin(&$binding);
            };
        }

        alloc_plain_time_getter!(plain_time_hour, "get hour", temporal_plain_time_hour);
        alloc_plain_time_getter!(plain_time_minute, "get minute", temporal_plain_time_minute);
        alloc_plain_time_getter!(plain_time_second, "get second", temporal_plain_time_second);
        alloc_plain_time_getter!(
            plain_time_millisecond,
            "get millisecond",
            temporal_plain_time_millisecond
        );
        alloc_plain_time_getter!(
            plain_time_microsecond,
            "get microsecond",
            temporal_plain_time_microsecond
        );
        alloc_plain_time_getter!(
            plain_time_nanosecond,
            "get nanosecond",
            temporal_plain_time_nanosecond
        );
        let plain_time_value_of = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "valueOf",
            temporal_plain_time_value_of,
            0,
            env,
        )?);
        pin_count += vm.pin(&plain_time_value_of);
        let plain_time_equals = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "equals",
            temporal_plain_time_equals,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_time_equals);
        let plain_time_compare = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "compare",
            temporal_plain_time_compare,
            2,
            env,
        )?);
        pin_count += vm.pin(&plain_time_compare);
        let plain_time_to_string = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "toString",
            temporal_plain_time_to_string,
            0,
            env,
        )?);
        pin_count += vm.pin(&plain_time_to_string);
        let plain_time_to_json = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "toJSON",
            temporal_plain_time_to_json,
            0,
            env,
        )?);
        pin_count += vm.pin(&plain_time_to_json);
        let plain_time_round = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "round",
            temporal_plain_time_round,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_time_round);
        let plain_time_with = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "with",
            temporal_plain_time_with,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_time_with);
        let duration_from = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "from",
            temporal_duration_from,
            1,
            env,
        )?);
        pin_count += vm.pin(&duration_from);
        let plain_time_add = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "add",
            temporal_plain_time_add,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_time_add);
        let plain_time_subtract = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "subtract",
            temporal_plain_time_subtract,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_time_subtract);
        let duration_with = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "with",
            temporal_duration_with,
            1,
            env,
        )?);
        pin_count += vm.pin(&duration_with);
        let duration_negated = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "negated",
            temporal_duration_negated,
            0,
            env,
        )?);
        pin_count += vm.pin(&duration_negated);
        let duration_abs = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "abs",
            temporal_duration_abs,
            0,
            env,
        )?);
        pin_count += vm.pin(&duration_abs);
        let duration_value_of = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "valueOf",
            temporal_duration_value_of,
            0,
            env,
        )?);
        pin_count += vm.pin(&duration_value_of);
        let duration_to_string = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "toString",
            temporal_duration_to_string,
            0,
            env,
        )?);
        pin_count += vm.pin(&duration_to_string);
        let duration_to_json = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "toJSON",
            temporal_duration_to_json,
            0,
            env,
        )?);
        pin_count += vm.pin(&duration_to_json);
        let duration_total = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "total",
            temporal_duration_total,
            1,
            env,
        )?);
        pin_count += vm.pin(&duration_total);

        let plain_month_day_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&plain_month_day_prototype);
        let plain_month_day_constructor =
            Value::Object(vm.new_native_constructor_in_env_with_gc_retry(
                "PlainMonthDay",
                temporal_plain_month_day_constructor,
                2,
                env,
                NativeConstructMode::InternalDeferredPrototype,
            )?);
        pin_count += vm.pin(&plain_month_day_constructor);
        let plain_month_day_from = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "from",
            temporal_plain_month_day_from,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_month_day_from);

        macro_rules! alloc_plain_month_day_native {
            ($binding:ident, $name:literal, $native:ident) => {
                let $binding = Value::Object(
                    vm.new_native_function_in_env_with_gc_retry($name, $native, 0, env)?,
                );
                pin_count += vm.pin(&$binding);
            };
        }
        alloc_plain_month_day_native!(
            plain_month_day_calendar_id,
            "get calendarId",
            temporal_plain_month_day_calendar_id
        );
        alloc_plain_month_day_native!(
            plain_month_day_month_code,
            "get monthCode",
            temporal_plain_month_day_month_code
        );
        alloc_plain_month_day_native!(plain_month_day_day, "get day", temporal_plain_month_day_day);
        alloc_plain_month_day_native!(
            plain_month_day_value_of,
            "valueOf",
            temporal_plain_month_day_value_of
        );
        alloc_plain_month_day_native!(
            plain_month_day_to_string,
            "toString",
            temporal_plain_month_day_to_string
        );
        let plain_month_day_with = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "with",
            temporal_plain_month_day_with,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_month_day_with);

        let plain_year_month_prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&plain_year_month_prototype);
        let plain_year_month_constructor =
            Value::Object(vm.new_native_constructor_in_env_with_gc_retry(
                "PlainYearMonth",
                temporal_plain_year_month_constructor,
                2,
                env,
                NativeConstructMode::InternalDeferredPrototype,
            )?);
        pin_count += vm.pin(&plain_year_month_constructor);
        let plain_year_month_from = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "from",
            temporal_plain_year_month_from,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_year_month_from);

        macro_rules! alloc_plain_year_month_native {
            ($binding:ident, $name:literal, $native:ident) => {
                let $binding = Value::Object(
                    vm.new_native_function_in_env_with_gc_retry($name, $native, 0, env)?,
                );
                pin_count += vm.pin(&$binding);
            };
        }
        alloc_plain_year_month_native!(
            plain_year_month_calendar_id,
            "get calendarId",
            temporal_plain_year_month_calendar_id
        );
        alloc_plain_year_month_native!(
            plain_year_month_era,
            "get era",
            temporal_plain_year_month_era
        );
        alloc_plain_year_month_native!(
            plain_year_month_era_year,
            "get eraYear",
            temporal_plain_year_month_era_year
        );
        alloc_plain_year_month_native!(
            plain_year_month_year,
            "get year",
            temporal_plain_year_month_year
        );
        alloc_plain_year_month_native!(
            plain_year_month_month,
            "get month",
            temporal_plain_year_month_month
        );
        alloc_plain_year_month_native!(
            plain_year_month_month_code,
            "get monthCode",
            temporal_plain_year_month_month_code
        );
        alloc_plain_year_month_native!(
            plain_year_month_days_in_month,
            "get daysInMonth",
            temporal_plain_year_month_days_in_month
        );
        alloc_plain_year_month_native!(
            plain_year_month_days_in_year,
            "get daysInYear",
            temporal_plain_year_month_days_in_year
        );
        alloc_plain_year_month_native!(
            plain_year_month_months_in_year,
            "get monthsInYear",
            temporal_plain_year_month_months_in_year
        );
        alloc_plain_year_month_native!(
            plain_year_month_in_leap_year,
            "get inLeapYear",
            temporal_plain_year_month_in_leap_year
        );
        alloc_plain_year_month_native!(
            plain_year_month_value_of,
            "valueOf",
            temporal_plain_year_month_value_of
        );
        alloc_plain_year_month_native!(
            plain_year_month_to_string,
            "toString",
            temporal_plain_year_month_to_string
        );
        let plain_year_month_with = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "with",
            temporal_plain_year_month_with,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_year_month_with);
        let plain_year_month_add = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "add",
            temporal_plain_year_month_add,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_year_month_add);
        let plain_year_month_subtract =
            Value::Object(vm.new_native_function_in_env_with_gc_retry(
                "subtract",
                temporal_plain_year_month_subtract,
                1,
                env,
            )?);
        pin_count += vm.pin(&plain_year_month_subtract);
        let plain_year_month_equals = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "equals",
            temporal_plain_year_month_equals,
            1,
            env,
        )?);
        pin_count += vm.pin(&plain_year_month_equals);
        let plain_year_month_compare = Value::Object(vm.new_native_function_in_env_with_gc_retry(
            "compare",
            temporal_plain_year_month_compare,
            2,
            env,
        )?);
        pin_count += vm.pin(&plain_year_month_compare);
        alloc_plain_year_month_native!(
            plain_year_month_to_json,
            "toJSON",
            temporal_plain_year_month_to_json
        );
        let plain_year_month_to_plain_date =
            Value::Object(vm.new_native_function_in_env_with_gc_retry(
                "toPlainDate",
                temporal_plain_year_month_to_plain_date,
                1,
                env,
            )?);
        pin_count += vm.pin(&plain_year_month_to_plain_date);

        let Value::Object(instant_constructor_index) = instant_constructor.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(instant_constructor_index.0, |object| {
            let HeapObj::Function(function) = object else {
                unreachable!()
            };
            *function.prototype.lock() = Some(instant_prototype.clone());
            function.props.lock().insert(
                PropertyKey::from("prototype"),
                const_prop(instant_prototype.clone()),
            );
            function.props.lock().insert(
                PropertyKey::from("fromEpochMilliseconds"),
                data_prop(from_epoch_milliseconds),
            );
            function.props.lock().insert(
                PropertyKey::from("fromEpochNanoseconds"),
                data_prop(from_epoch_nanoseconds),
            );
            function
                .props
                .lock()
                .insert(PropertyKey::from("from"), data_prop(from));
            function
                .props
                .lock()
                .insert(PropertyKey::from("compare"), data_prop(compare));
        });
        let Value::Object(instant_prototype_index) = instant_prototype.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(instant_prototype_index.0, |object| {
            let mut props = object.props().lock();
            props.insert(
                PropertyKey::from("constructor"),
                data_prop(instant_constructor.clone()),
            );
            props.insert(
                PropertyKey::from("epochMilliseconds"),
                accessor_get_prop(epoch_milliseconds),
            );
            props.insert(
                PropertyKey::from("epochNanoseconds"),
                accessor_get_prop(epoch_nanoseconds),
            );
            props.insert(PropertyKey::from("equals"), data_prop(equals));
            props.insert(PropertyKey::from("toString"), data_prop(to_string));
            props.insert(PropertyKey::from("valueOf"), data_prop(value_of));
            let mut tag = data_prop(Value::String(Arc::from("Temporal.Instant")));
            tag.writable = false;
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                tag,
            );
        });

        let Value::Object(duration_constructor_index) = duration_constructor.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(duration_constructor_index.0, |object| {
            let HeapObj::Function(function) = object else {
                unreachable!()
            };
            *function.prototype.lock() = Some(duration_prototype.clone());
            function.props.lock().insert(
                PropertyKey::from("prototype"),
                const_prop(duration_prototype.clone()),
            );
            function
                .props
                .lock()
                .insert(PropertyKey::from("from"), data_prop(duration_from));
        });
        let Value::Object(duration_prototype_index) = duration_prototype.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(duration_prototype_index.0, |object| {
            let mut props = object.props().lock();
            props.insert(
                PropertyKey::from("constructor"),
                data_prop(duration_constructor.clone()),
            );
            props.insert(
                PropertyKey::from("years"),
                accessor_get_prop(duration_years),
            );
            props.insert(
                PropertyKey::from("months"),
                accessor_get_prop(duration_months),
            );
            props.insert(
                PropertyKey::from("weeks"),
                accessor_get_prop(duration_weeks),
            );
            props.insert(PropertyKey::from("days"), accessor_get_prop(duration_days));
            props.insert(
                PropertyKey::from("hours"),
                accessor_get_prop(duration_hours),
            );
            props.insert(
                PropertyKey::from("minutes"),
                accessor_get_prop(duration_minutes),
            );
            props.insert(
                PropertyKey::from("seconds"),
                accessor_get_prop(duration_seconds),
            );
            props.insert(
                PropertyKey::from("milliseconds"),
                accessor_get_prop(duration_milliseconds),
            );
            props.insert(
                PropertyKey::from("microseconds"),
                accessor_get_prop(duration_microseconds),
            );
            props.insert(
                PropertyKey::from("nanoseconds"),
                accessor_get_prop(duration_nanoseconds),
            );
            props.insert(PropertyKey::from("sign"), accessor_get_prop(duration_sign));
            props.insert(
                PropertyKey::from("blank"),
                accessor_get_prop(duration_blank),
            );
            props.insert(PropertyKey::from("with"), data_prop(duration_with));
            props.insert(PropertyKey::from("negated"), data_prop(duration_negated));
            props.insert(PropertyKey::from("abs"), data_prop(duration_abs));
            props.insert(PropertyKey::from("total"), data_prop(duration_total));
            props.insert(PropertyKey::from("toString"), data_prop(duration_to_string));
            props.insert(PropertyKey::from("toJSON"), data_prop(duration_to_json));
            props.insert(PropertyKey::from("valueOf"), data_prop(duration_value_of));
            let mut tag = data_prop(Value::String(Arc::from("Temporal.Duration")));
            tag.writable = false;
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                tag,
            );
        });

        let Value::Object(plain_date_time_constructor_index) = plain_date_time_constructor.clone()
        else {
            unreachable!()
        };
        vm.heap
            .with_obj(plain_date_time_constructor_index.0, |object| {
                let HeapObj::Function(function) = object else {
                    unreachable!()
                };
                *function.prototype.lock() = Some(plain_date_time_prototype.clone());
                function.props.lock().insert(
                    PropertyKey::from("prototype"),
                    const_prop(plain_date_time_prototype.clone()),
                );
                function
                    .props
                    .lock()
                    .insert(PropertyKey::from("from"), data_prop(plain_date_time_from));
                function.props.lock().insert(
                    PropertyKey::from("compare"),
                    data_prop(plain_date_time_compare),
                );
            });
        let Value::Object(plain_date_time_prototype_index) = plain_date_time_prototype.clone()
        else {
            unreachable!()
        };
        vm.heap
            .with_obj(plain_date_time_prototype_index.0, |object| {
                let mut props = object.props().lock();
                props.insert(
                    PropertyKey::from("constructor"),
                    data_prop(plain_date_time_constructor.clone()),
                );
                for (name, getter) in [
                    ("calendarId", plain_calendar_id),
                    ("era", plain_era),
                    ("eraYear", plain_era_year),
                    ("year", plain_year),
                    ("month", plain_month),
                    ("monthCode", plain_month_code),
                    ("day", plain_day),
                    ("hour", plain_hour),
                    ("minute", plain_minute),
                    ("second", plain_second),
                    ("millisecond", plain_millisecond),
                    ("microsecond", plain_microsecond),
                    ("nanosecond", plain_nanosecond),
                    ("dayOfWeek", plain_day_of_week),
                    ("dayOfYear", plain_day_of_year),
                    ("weekOfYear", plain_week_of_year),
                    ("yearOfWeek", plain_year_of_week),
                    ("daysInWeek", plain_days_in_week),
                    ("daysInMonth", plain_days_in_month),
                    ("daysInYear", plain_days_in_year),
                    ("monthsInYear", plain_months_in_year),
                    ("inLeapYear", plain_in_leap_year),
                ] {
                    props.insert(PropertyKey::from(name), accessor_get_prop(getter));
                }
                props.insert(PropertyKey::from("equals"), data_prop(plain_equals));
                props.insert(PropertyKey::from("valueOf"), data_prop(plain_value_of));
                let mut tag = data_prop(Value::String(Arc::from("Temporal.PlainDateTime")));
                tag.writable = false;
                props.insert(
                    PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                    tag,
                );
            });

        let Value::Object(plain_date_constructor_index) = plain_date_constructor.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(plain_date_constructor_index.0, |object| {
            let HeapObj::Function(function) = object else {
                unreachable!()
            };
            *function.prototype.lock() = Some(plain_date_prototype.clone());
            function.props.lock().insert(
                PropertyKey::from("prototype"),
                const_prop(plain_date_prototype.clone()),
            );
            function
                .props
                .lock()
                .insert(PropertyKey::from("from"), data_prop(plain_date_from));
            function
                .props
                .lock()
                .insert(PropertyKey::from("compare"), data_prop(plain_date_compare));
        });
        let Value::Object(plain_date_prototype_index) = plain_date_prototype.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(plain_date_prototype_index.0, |object| {
            let mut props = object.props().lock();
            props.insert(
                PropertyKey::from("constructor"),
                data_prop(plain_date_constructor.clone()),
            );
            for (name, getter) in [
                ("calendarId", plain_date_calendar_id),
                ("era", plain_date_era),
                ("eraYear", plain_date_era_year),
                ("year", plain_date_year),
                ("month", plain_date_month),
                ("monthCode", plain_date_month_code),
                ("day", plain_date_day),
                ("dayOfWeek", plain_date_day_of_week),
                ("dayOfYear", plain_date_day_of_year),
                ("weekOfYear", plain_date_week_of_year),
                ("yearOfWeek", plain_date_year_of_week),
                ("daysInWeek", plain_date_days_in_week),
                ("daysInMonth", plain_date_days_in_month),
                ("daysInYear", plain_date_days_in_year),
                ("monthsInYear", plain_date_months_in_year),
                ("inLeapYear", plain_date_in_leap_year),
            ] {
                props.insert(PropertyKey::from(name), accessor_get_prop(getter));
            }
            props.insert(PropertyKey::from("equals"), data_prop(plain_date_equals));
            props.insert(
                PropertyKey::from("toPlainDateTime"),
                data_prop(plain_date_to_plain_date_time),
            );
            props.insert(
                PropertyKey::from("toString"),
                data_prop(plain_date_to_string),
            );
            props.insert(PropertyKey::from("toJSON"), data_prop(plain_date_to_json));
            props.insert(PropertyKey::from("valueOf"), data_prop(plain_date_value_of));
            let mut tag = data_prop(Value::String(Arc::from("Temporal.PlainDate")));
            tag.writable = false;
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                tag,
            );
        });

        let Value::Object(plain_time_constructor_index) = plain_time_constructor.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(plain_time_constructor_index.0, |object| {
            let HeapObj::Function(function) = object else {
                unreachable!()
            };
            *function.prototype.lock() = Some(plain_time_prototype.clone());
            function.props.lock().insert(
                PropertyKey::from("prototype"),
                const_prop(plain_time_prototype.clone()),
            );
            function
                .props
                .lock()
                .insert(PropertyKey::from("from"), data_prop(plain_time_from));
            function
                .props
                .lock()
                .insert(PropertyKey::from("compare"), data_prop(plain_time_compare));
        });
        let Value::Object(plain_time_prototype_index) = plain_time_prototype.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(plain_time_prototype_index.0, |object| {
            let mut props = object.props().lock();
            props.insert(
                PropertyKey::from("constructor"),
                data_prop(plain_time_constructor.clone()),
            );
            for (name, getter) in [
                ("hour", plain_time_hour),
                ("minute", plain_time_minute),
                ("second", plain_time_second),
                ("millisecond", plain_time_millisecond),
                ("microsecond", plain_time_microsecond),
                ("nanosecond", plain_time_nanosecond),
            ] {
                props.insert(PropertyKey::from(name), accessor_get_prop(getter));
            }
            props.insert(PropertyKey::from("equals"), data_prop(plain_time_equals));
            props.insert(
                PropertyKey::from("toString"),
                data_prop(plain_time_to_string),
            );
            props.insert(PropertyKey::from("toJSON"), data_prop(plain_time_to_json));
            props.insert(PropertyKey::from("round"), data_prop(plain_time_round));
            props.insert(PropertyKey::from("with"), data_prop(plain_time_with));
            props.insert(PropertyKey::from("add"), data_prop(plain_time_add));
            props.insert(
                PropertyKey::from("subtract"),
                data_prop(plain_time_subtract),
            );
            props.insert(PropertyKey::from("valueOf"), data_prop(plain_time_value_of));
            let mut tag = data_prop(Value::String(Arc::from("Temporal.PlainTime")));
            tag.writable = false;
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                tag,
            );
        });

        let Value::Object(zoned_constructor_index) = zoned_date_time_constructor.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(zoned_constructor_index.0, |object| {
            let HeapObj::Function(function) = object else {
                unreachable!()
            };
            *function.prototype.lock() = Some(zoned_date_time_prototype.clone());
            function.props.lock().insert(
                PropertyKey::from("prototype"),
                const_prop(zoned_date_time_prototype.clone()),
            );
            function
                .props
                .lock()
                .insert(PropertyKey::from("from"), data_prop(zoned_from));
            function
                .props
                .lock()
                .insert(PropertyKey::from("compare"), data_prop(zoned_compare));
        });
        let Value::Object(zoned_prototype_index) = zoned_date_time_prototype.clone() else {
            unreachable!()
        };
        vm.heap.with_obj(zoned_prototype_index.0, |object| {
            let mut props = object.props().lock();
            props.insert(
                PropertyKey::from("constructor"),
                data_prop(zoned_date_time_constructor.clone()),
            );
            props.insert(
                PropertyKey::from("epochMilliseconds"),
                accessor_get_prop(zoned_epoch_milliseconds),
            );
            props.insert(
                PropertyKey::from("epochNanoseconds"),
                accessor_get_prop(zoned_epoch_nanoseconds),
            );
            props.insert(
                PropertyKey::from("timeZoneId"),
                accessor_get_prop(time_zone_id),
            );
            props.insert(
                PropertyKey::from("calendarId"),
                accessor_get_prop(calendar_id),
            );
            props.insert(PropertyKey::from("era"), accessor_get_prop(zoned_era));
            props.insert(
                PropertyKey::from("eraYear"),
                accessor_get_prop(zoned_era_year),
            );
            props.insert(PropertyKey::from("year"), accessor_get_prop(zoned_year));
            props.insert(PropertyKey::from("month"), accessor_get_prop(zoned_month));
            props.insert(
                PropertyKey::from("monthCode"),
                accessor_get_prop(zoned_month_code),
            );
            props.insert(PropertyKey::from("day"), accessor_get_prop(zoned_day));
            props.insert(PropertyKey::from("hour"), accessor_get_prop(zoned_hour));
            props.insert(PropertyKey::from("minute"), accessor_get_prop(zoned_minute));
            props.insert(PropertyKey::from("second"), accessor_get_prop(zoned_second));
            props.insert(
                PropertyKey::from("millisecond"),
                accessor_get_prop(zoned_millisecond),
            );
            props.insert(
                PropertyKey::from("microsecond"),
                accessor_get_prop(zoned_microsecond),
            );
            props.insert(
                PropertyKey::from("nanosecond"),
                accessor_get_prop(zoned_nanosecond),
            );
            props.insert(
                PropertyKey::from("dayOfWeek"),
                accessor_get_prop(zoned_day_of_week),
            );
            props.insert(
                PropertyKey::from("dayOfYear"),
                accessor_get_prop(zoned_day_of_year),
            );
            props.insert(
                PropertyKey::from("weekOfYear"),
                accessor_get_prop(zoned_week_of_year),
            );
            props.insert(
                PropertyKey::from("yearOfWeek"),
                accessor_get_prop(zoned_year_of_week),
            );
            props.insert(
                PropertyKey::from("hoursInDay"),
                accessor_get_prop(zoned_hours_in_day),
            );
            props.insert(
                PropertyKey::from("daysInWeek"),
                accessor_get_prop(zoned_days_in_week),
            );
            props.insert(
                PropertyKey::from("daysInMonth"),
                accessor_get_prop(zoned_days_in_month),
            );
            props.insert(
                PropertyKey::from("daysInYear"),
                accessor_get_prop(zoned_days_in_year),
            );
            props.insert(
                PropertyKey::from("monthsInYear"),
                accessor_get_prop(zoned_months_in_year),
            );
            props.insert(
                PropertyKey::from("inLeapYear"),
                accessor_get_prop(zoned_in_leap_year),
            );
            props.insert(
                PropertyKey::from("offsetNanoseconds"),
                accessor_get_prop(zoned_offset_nanoseconds),
            );
            props.insert(PropertyKey::from("offset"), accessor_get_prop(zoned_offset));
            props.insert(
                PropertyKey::from("withTimeZone"),
                data_prop(zoned_with_time_zone),
            );
            props.insert(
                PropertyKey::from("withCalendar"),
                data_prop(zoned_with_calendar),
            );
            props.insert(
                PropertyKey::from("startOfDay"),
                data_prop(zoned_start_of_day),
            );
            props.insert(PropertyKey::from("equals"), data_prop(zoned_equals));
            props.insert(PropertyKey::from("toInstant"), data_prop(zoned_to_instant));
            props.insert(
                PropertyKey::from("toPlainDateTime"),
                data_prop(zoned_to_plain_date_time),
            );
            props.insert(PropertyKey::from("toString"), data_prop(zoned_to_string));
            props.insert(PropertyKey::from("toJSON"), data_prop(zoned_to_json));
            props.insert(PropertyKey::from("valueOf"), data_prop(zoned_value_of));
            let mut tag = data_prop(Value::String(Arc::from("Temporal.ZonedDateTime")));
            tag.writable = false;
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                tag,
            );
        });

        let Value::Object(plain_month_day_constructor_index) = plain_month_day_constructor.clone()
        else {
            unreachable!()
        };
        vm.heap
            .with_obj(plain_month_day_constructor_index.0, |object| {
                let HeapObj::Function(function) = object else {
                    unreachable!()
                };
                *function.prototype.lock() = Some(plain_month_day_prototype.clone());
                function.props.lock().insert(
                    PropertyKey::from("prototype"),
                    const_prop(plain_month_day_prototype.clone()),
                );
                function
                    .props
                    .lock()
                    .insert(PropertyKey::from("from"), data_prop(plain_month_day_from));
            });
        let Value::Object(plain_month_day_prototype_index) = plain_month_day_prototype.clone()
        else {
            unreachable!()
        };
        vm.heap
            .with_obj(plain_month_day_prototype_index.0, |object| {
                let mut props = object.props().lock();
                props.insert(
                    PropertyKey::from("constructor"),
                    data_prop(plain_month_day_constructor.clone()),
                );
                props.insert(
                    PropertyKey::from("calendarId"),
                    accessor_get_prop(plain_month_day_calendar_id),
                );
                props.insert(
                    PropertyKey::from("monthCode"),
                    accessor_get_prop(plain_month_day_month_code),
                );
                props.insert(
                    PropertyKey::from("day"),
                    accessor_get_prop(plain_month_day_day),
                );
                props.insert(
                    PropertyKey::from("valueOf"),
                    data_prop(plain_month_day_value_of),
                );
                props.insert(
                    PropertyKey::from("toString"),
                    data_prop(plain_month_day_to_string),
                );
                props.insert(PropertyKey::from("with"), data_prop(plain_month_day_with));
                let mut tag = data_prop(Value::String(Arc::from("Temporal.PlainMonthDay")));
                tag.writable = false;
                props.insert(
                    PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                    tag,
                );
            });

        let Value::Object(plain_year_month_constructor_index) =
            plain_year_month_constructor.clone()
        else {
            unreachable!()
        };
        vm.heap
            .with_obj(plain_year_month_constructor_index.0, |object| {
                let HeapObj::Function(function) = object else {
                    unreachable!()
                };
                *function.prototype.lock() = Some(plain_year_month_prototype.clone());
                function.props.lock().insert(
                    PropertyKey::from("prototype"),
                    const_prop(plain_year_month_prototype.clone()),
                );
                function
                    .props
                    .lock()
                    .insert(PropertyKey::from("from"), data_prop(plain_year_month_from));
                function.props.lock().insert(
                    PropertyKey::from("compare"),
                    data_prop(plain_year_month_compare),
                );
            });
        let Value::Object(plain_year_month_prototype_index) = plain_year_month_prototype.clone()
        else {
            unreachable!()
        };
        vm.heap
            .with_obj(plain_year_month_prototype_index.0, |object| {
                let mut props = object.props().lock();
                props.insert(
                    PropertyKey::from("constructor"),
                    data_prop(plain_year_month_constructor.clone()),
                );
                for (name, getter) in [
                    ("calendarId", plain_year_month_calendar_id),
                    ("era", plain_year_month_era),
                    ("eraYear", plain_year_month_era_year),
                    ("year", plain_year_month_year),
                    ("month", plain_year_month_month),
                    ("monthCode", plain_year_month_month_code),
                    ("daysInMonth", plain_year_month_days_in_month),
                    ("daysInYear", plain_year_month_days_in_year),
                    ("monthsInYear", plain_year_month_months_in_year),
                    ("inLeapYear", plain_year_month_in_leap_year),
                ] {
                    props.insert(PropertyKey::from(name), accessor_get_prop(getter));
                }
                props.insert(
                    PropertyKey::from("valueOf"),
                    data_prop(plain_year_month_value_of),
                );
                props.insert(
                    PropertyKey::from("toString"),
                    data_prop(plain_year_month_to_string),
                );
                props.insert(
                    PropertyKey::from("toJSON"),
                    data_prop(plain_year_month_to_json),
                );
                props.insert(
                    PropertyKey::from("toPlainDate"),
                    data_prop(plain_year_month_to_plain_date),
                );
                props.insert(PropertyKey::from("with"), data_prop(plain_year_month_with));
                props.insert(PropertyKey::from("add"), data_prop(plain_year_month_add));
                props.insert(
                    PropertyKey::from("subtract"),
                    data_prop(plain_year_month_subtract),
                );
                props.insert(
                    PropertyKey::from("equals"),
                    data_prop(plain_year_month_equals),
                );
                let mut tag = data_prop(Value::String(Arc::from("Temporal.PlainYearMonth")));
                tag.writable = false;
                props.insert(
                    PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                    tag,
                );
            });

        let mut now_tag = data_prop(Value::String(Arc::from("Temporal.Now")));
        now_tag.writable = false;
        let now = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::from([(
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                now_tag,
            )])),
            proto: Mutex::new(Some(object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("Temporal.Now")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);
        pin_count += vm.pin(&now);
        let mut temporal_tag = data_prop(Value::String(Arc::from("Temporal")));
        temporal_tag.writable = false;
        let temporal = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::from([
                (PropertyKey::from("Now"), data_prop(now)),
                (
                    PropertyKey::from("Instant"),
                    data_prop(instant_constructor.clone()),
                ),
                (
                    PropertyKey::from("Duration"),
                    data_prop(duration_constructor.clone()),
                ),
                (
                    PropertyKey::from("PlainDate"),
                    data_prop(plain_date_constructor.clone()),
                ),
                (
                    PropertyKey::from("PlainMonthDay"),
                    data_prop(plain_month_day_constructor.clone()),
                ),
                (
                    PropertyKey::from("PlainTime"),
                    data_prop(plain_time_constructor.clone()),
                ),
                (
                    PropertyKey::from("PlainDateTime"),
                    data_prop(plain_date_time_constructor.clone()),
                ),
                (
                    PropertyKey::from("PlainYearMonth"),
                    data_prop(plain_year_month_constructor.clone()),
                ),
                (
                    PropertyKey::from("ZonedDateTime"),
                    data_prop(zoned_date_time_constructor.clone()),
                ),
                (
                    PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                    temporal_tag,
                ),
            ])),
            proto: Mutex::new(Some(object_proto)),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("Temporal")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?);

        vm.realm_temporal_instant_constructors
            .insert(env.0, instant_constructor);
        vm.realm_temporal_instant_prototypes
            .insert(env.0, instant_prototype);
        vm.realm_temporal_duration_constructors
            .insert(env.0, duration_constructor);
        vm.realm_temporal_duration_prototypes
            .insert(env.0, duration_prototype);
        vm.realm_temporal_plain_date_constructors
            .insert(env.0, plain_date_constructor);
        vm.realm_temporal_plain_date_prototypes
            .insert(env.0, plain_date_prototype);
        vm.realm_temporal_plain_month_day_constructors
            .insert(env.0, plain_month_day_constructor);
        vm.realm_temporal_plain_month_day_prototypes
            .insert(env.0, plain_month_day_prototype);
        vm.realm_temporal_plain_time_constructors
            .insert(env.0, plain_time_constructor);
        vm.realm_temporal_plain_time_prototypes
            .insert(env.0, plain_time_prototype);
        vm.realm_temporal_plain_date_time_constructors
            .insert(env.0, plain_date_time_constructor);
        vm.realm_temporal_plain_date_time_prototypes
            .insert(env.0, plain_date_time_prototype);
        vm.realm_temporal_plain_year_month_constructors
            .insert(env.0, plain_year_month_constructor);
        vm.realm_temporal_plain_year_month_prototypes
            .insert(env.0, plain_year_month_prototype);
        vm.realm_temporal_zoned_date_time_constructors
            .insert(env.0, zoned_date_time_constructor);
        vm.realm_temporal_zoned_date_time_prototypes
            .insert(env.0, zoned_date_time_prototype);

        if let Some(global) = global {
            define_realm_global(vm, env, global, "Temporal", temporal.clone());
        } else {
            define_global(vm, "Temporal", temporal.clone());
        }
        Ok(temporal)
    })();
    vm.unpin_many(pin_count);
    result
}

const TEMPORAL_INSTANT_LIMIT_MILLISECONDS: i64 = 8_640_000_000_000_000;

fn temporal_instant_limit_nanoseconds() -> BigInt {
    BigInt::from(TEMPORAL_INSTANT_LIMIT_MILLISECONDS) * BigInt::from(1_000_000_i64)
}

fn temporal_instant_epoch(vm: &Vm, this: Option<Value>) -> error::Result<Arc<BigInt>> {
    let Value::Object(index) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(
            "Temporal.Instant method called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind: TemporalKind::Instant { epoch_nanoseconds },
            ..
        }) => Ok(epoch_nanoseconds.clone()),
        _ => Err(Error::type_err(
            "Temporal.Instant method called on incompatible receiver",
        )),
    })
}

fn create_temporal_instant(
    vm: &mut Vm,
    epoch_nanoseconds: Arc<BigInt>,
    prototype: Value,
) -> error::Result<Value> {
    if epoch_nanoseconds.as_ref().abs() > temporal_instant_limit_nanoseconds() {
        return Err(Error::range(
            "Temporal.Instant epoch nanoseconds out of range",
        ));
    }
    vm.try_reserve_gc_pins(1)?;
    let pin_count = vm.pin(&prototype);
    let result = vm.alloc(HeapObj::Temporal(TemporalData {
        kind: TemporalKind::Instant { epoch_nanoseconds },
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    result.map(Value::Object)
}

pub(crate) fn create_temporal_instant_in_realm(
    vm: &mut Vm,
    epoch_nanoseconds: Arc<BigInt>,
    realm: GcIdx,
) -> error::Result<Value> {
    let prototype = vm
        .realm_temporal_instant_prototypes
        .get(&env::global_env_root(&vm.heap, realm).0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.Instant prototype is not installed"))?;
    create_temporal_instant(vm, epoch_nanoseconds, prototype)
}

fn temporal_duration_sign_value(fields: &TemporalDurationFields) -> i8 {
    for value in [
        fields.years,
        fields.months,
        fields.weeks,
        fields.days,
        fields.hours,
        fields.minutes,
        fields.seconds,
        fields.milliseconds,
        fields.microseconds,
        fields.nanoseconds,
    ] {
        if value < 0.0 {
            return -1;
        }
        if value > 0.0 {
            return 1;
        }
    }
    0
}

fn temporal_duration_is_valid(fields: &TemporalDurationFields) -> bool {
    let values = [
        fields.years,
        fields.months,
        fields.weeks,
        fields.days,
        fields.hours,
        fields.minutes,
        fields.seconds,
        fields.milliseconds,
        fields.microseconds,
        fields.nanoseconds,
    ];
    if values
        .iter()
        .any(|value| !value.is_finite() || value.fract() != 0.0)
    {
        return false;
    }
    let sign = temporal_duration_sign_value(fields);
    if values
        .iter()
        .any(|value| (*value < 0.0 && sign > 0) || (*value > 0.0 && sign < 0))
    {
        return false;
    }
    let integers = values
        .map(|value| BigInt::from_f64(value).expect("finite integral f64 must convert to BigInt"));
    let date_unit_limit = BigInt::from(1_u64 << 32);
    if integers[..3]
        .iter()
        .any(|value| value.abs() >= date_unit_limit)
    {
        return false;
    }
    let normalized_nanoseconds = &integers[3] * BigInt::from(86_400_000_000_000_i64)
        + &integers[4] * BigInt::from(3_600_000_000_000_i64)
        + &integers[5] * BigInt::from(60_000_000_000_i64)
        + &integers[6] * BigInt::from(1_000_000_000_i64)
        + &integers[7] * BigInt::from(1_000_000_i64)
        + &integers[8] * BigInt::from(1_000_i64)
        + &integers[9];
    let time_limit = BigInt::from(1_000_000_000_i64) * BigInt::from(1_u64 << 53);
    normalized_nanoseconds.abs() < time_limit
}

fn temporal_duration_slots(vm: &Vm, this: Option<Value>) -> error::Result<TemporalDurationFields> {
    let Value::Object(index) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(
            "Temporal.Duration method called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind: TemporalKind::Duration { fields },
            ..
        }) => Ok(*fields),
        _ => Err(Error::type_err(
            "Temporal.Duration method called on incompatible receiver",
        )),
    })
}

fn temporal_duration_slots_if_present(vm: &Vm, value: &Value) -> Option<TemporalDurationFields> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind: TemporalKind::Duration { fields },
            ..
        }) => Some(*fields),
        _ => None,
    })
}

fn create_temporal_duration(
    vm: &mut Vm,
    fields: TemporalDurationFields,
    prototype: Value,
) -> error::Result<Value> {
    if !temporal_duration_is_valid(&fields) {
        return Err(Error::range("Invalid Temporal.Duration fields"));
    }
    vm.try_reserve_gc_pins(1)?;
    let pin_count = vm.pin(&prototype);
    let result = vm.alloc(HeapObj::Temporal(TemporalData {
        kind: TemporalKind::Duration { fields },
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    result.map(Value::Object)
}

fn create_temporal_duration_in_realm(
    vm: &mut Vm,
    fields: TemporalDurationFields,
    realm: GcIdx,
) -> error::Result<Value> {
    let prototype = vm
        .realm_temporal_duration_prototypes
        .get(&env::global_env_root(&vm.heap, realm).0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.Duration prototype is not installed"))?;
    create_temporal_duration(vm, fields, prototype)
}

#[derive(Clone, Copy)]
struct TemporalPartialDurationFields {
    years: Option<f64>,
    months: Option<f64>,
    weeks: Option<f64>,
    days: Option<f64>,
    hours: Option<f64>,
    minutes: Option<f64>,
    seconds: Option<f64>,
    milliseconds: Option<f64>,
    microseconds: Option<f64>,
    nanoseconds: Option<f64>,
}

impl TemporalPartialDurationFields {
    fn merge(self, fallback: TemporalDurationFields) -> TemporalDurationFields {
        TemporalDurationFields {
            years: self.years.unwrap_or(fallback.years),
            months: self.months.unwrap_or(fallback.months),
            weeks: self.weeks.unwrap_or(fallback.weeks),
            days: self.days.unwrap_or(fallback.days),
            hours: self.hours.unwrap_or(fallback.hours),
            minutes: self.minutes.unwrap_or(fallback.minutes),
            seconds: self.seconds.unwrap_or(fallback.seconds),
            milliseconds: self.milliseconds.unwrap_or(fallback.milliseconds),
            microseconds: self.microseconds.unwrap_or(fallback.microseconds),
            nanoseconds: self.nanoseconds.unwrap_or(fallback.nanoseconds),
        }
    }
}

fn temporal_zero_duration_fields() -> TemporalDurationFields {
    TemporalDurationFields {
        years: 0.0,
        months: 0.0,
        weeks: 0.0,
        days: 0.0,
        hours: 0.0,
        minutes: 0.0,
        seconds: 0.0,
        milliseconds: 0.0,
        microseconds: 0.0,
        nanoseconds: 0.0,
    }
}

fn temporal_partial_duration_fields_rooted(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPartialDurationFields> {
    let numeric = |vm: &mut Vm, name: &str| -> error::Result<Option<f64>> {
        match vm.get_property(item, name)? {
            Value::Undefined => Ok(None),
            value => temporal_integer_if_integral(vm, value).map(Some),
        }
    };
    let days = numeric(vm, "days")?;
    let hours = numeric(vm, "hours")?;
    let microseconds = numeric(vm, "microseconds")?;
    let milliseconds = numeric(vm, "milliseconds")?;
    let minutes = numeric(vm, "minutes")?;
    let months = numeric(vm, "months")?;
    let nanoseconds = numeric(vm, "nanoseconds")?;
    let seconds = numeric(vm, "seconds")?;
    let weeks = numeric(vm, "weeks")?;
    let years = numeric(vm, "years")?;
    if [
        days,
        hours,
        microseconds,
        milliseconds,
        minutes,
        months,
        nanoseconds,
        seconds,
        weeks,
        years,
    ]
    .into_iter()
    .all(|field| field.is_none())
    {
        return Err(Error::type_err(
            "Temporal.Duration property bag requires a duration field",
        ));
    }
    let partial = TemporalPartialDurationFields {
        years,
        months,
        weeks,
        days,
        hours,
        minutes,
        seconds,
        milliseconds,
        microseconds,
        nanoseconds,
    };
    Ok(partial)
}

fn temporal_duration_property_fields_rooted(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalDurationFields> {
    let fields =
        temporal_partial_duration_fields_rooted(vm, item)?.merge(temporal_zero_duration_fields());
    temporal_duration_is_valid(&fields)
        .then_some(fields)
        .ok_or_else(|| Error::range("Invalid Temporal.Duration fields"))
}

fn to_temporal_duration_fields(vm: &mut Vm, item: &Value) -> error::Result<TemporalDurationFields> {
    if let Some(fields) = temporal_duration_slots_if_present(vm, item) {
        return Ok(fields);
    }
    if matches!(item, Value::Object(_)) {
        return temporal_with_rooted_value(vm, item.clone(), |vm, item| {
            temporal_duration_property_fields_rooted(vm, item)
        });
    }
    let Value::String(source) = item else {
        return Err(Error::type_err(
            "Temporal.Duration input must be a String or object",
        ));
    };
    vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
    let parsed = temporal::parse_duration_string(source)
        .ok_or_else(|| Error::range("Invalid Temporal.Duration string"))?;
    let fields = TemporalDurationFields {
        years: parsed.years,
        months: parsed.months,
        weeks: parsed.weeks,
        days: parsed.days,
        hours: parsed.hours,
        minutes: parsed.minutes,
        seconds: parsed.seconds,
        milliseconds: parsed.milliseconds,
        microseconds: parsed.microseconds,
        nanoseconds: parsed.nanoseconds,
    };
    temporal_duration_is_valid(&fields)
        .then_some(fields)
        .ok_or_else(|| Error::range("Invalid Temporal.Duration fields"))
}

fn temporal_duration_from(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let fields = to_temporal_duration_fields(vm, args.first().unwrap_or(&Value::Undefined))?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_duration_in_realm(vm, fields, realm)
}

fn temporal_duration_with(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = temporal_duration_slots(vm, this)?;
    let item = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(item, Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.Duration.prototype.with requires an object",
        ));
    }
    let partial = temporal_with_rooted_value(vm, item, |vm, item| {
        temporal_partial_duration_fields_rooted(vm, item)
    })?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_duration_in_realm(vm, partial.merge(receiver), realm)
}

#[derive(Clone, Copy)]
enum TemporalDurationSignTransform {
    Abs,
    Negate,
}

fn temporal_duration_sign_transform(
    vm: &mut Vm,
    this: Option<Value>,
    operation: TemporalDurationSignTransform,
) -> error::Result<Value> {
    let fields = temporal_duration_slots(vm, this)?;
    let transform = |value: f64| {
        if value == 0.0 {
            0.0
        } else {
            match operation {
                TemporalDurationSignTransform::Abs => value.abs(),
                TemporalDurationSignTransform::Negate => -value,
            }
        }
    };
    let transformed = TemporalDurationFields {
        years: transform(fields.years),
        months: transform(fields.months),
        weeks: transform(fields.weeks),
        days: transform(fields.days),
        hours: transform(fields.hours),
        minutes: transform(fields.minutes),
        seconds: transform(fields.seconds),
        milliseconds: transform(fields.milliseconds),
        microseconds: transform(fields.microseconds),
        nanoseconds: transform(fields.nanoseconds),
    };
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_duration_in_realm(vm, transformed, realm)
}

fn temporal_duration_abs(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_duration_sign_transform(vm, this, TemporalDurationSignTransform::Abs)
}

fn temporal_duration_negated(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_duration_sign_transform(vm, this, TemporalDurationSignTransform::Negate)
}

fn temporal_duration_integer_values(fields: TemporalDurationFields) -> error::Result<[i128; 10]> {
    let values = [
        fields.years,
        fields.months,
        fields.weeks,
        fields.days,
        fields.hours,
        fields.minutes,
        fields.seconds,
        fields.milliseconds,
        fields.microseconds,
        fields.nanoseconds,
    ]
    .map(|value| {
        BigInt::from_f64(value)
            .and_then(|integer| integer.to_i128())
            .ok_or_else(|| Error::range("Invalid Temporal.Duration fields"))
    });
    let [years, months, weeks, days, hours, minutes, seconds, milliseconds, microseconds, nanoseconds] =
        values;
    Ok([
        years?,
        months?,
        weeks?,
        days?,
        hours?,
        minutes?,
        seconds?,
        milliseconds?,
        microseconds?,
        nanoseconds?,
    ])
}

fn temporal_duration_fields_from_integer_values(
    values: [i128; 10],
) -> error::Result<TemporalDurationFields> {
    let numbers = values.map(|value| {
        let number = value
            .to_f64()
            .ok_or_else(|| Error::range("Invalid rounded Temporal.Duration"))?;
        if BigInt::from_f64(number).as_ref() != Some(&BigInt::from(value)) {
            return Err(Error::range("Invalid rounded Temporal.Duration"));
        }
        Ok(number)
    });
    let [years, months, weeks, days, hours, minutes, seconds, milliseconds, microseconds, nanoseconds] =
        numbers;
    let fields = TemporalDurationFields {
        years: years?,
        months: months?,
        weeks: weeks?,
        days: days?,
        hours: hours?,
        minutes: minutes?,
        seconds: seconds?,
        milliseconds: milliseconds?,
        microseconds: microseconds?,
        nanoseconds: nanoseconds?,
    };
    temporal_duration_is_valid(&fields)
        .then_some(fields)
        .ok_or_else(|| Error::range("Invalid rounded Temporal.Duration"))
}

fn temporal_duration_round_for_string(
    fields: TemporalDurationFields,
    increment: i128,
    rounding_mode: temporal::InstantRoundingMode,
) -> error::Result<TemporalDurationFields> {
    let mut values = temporal_duration_integer_values(fields)?;
    let largest = values.iter().position(|value| *value != 0).unwrap_or(9);
    let rounded_largest = largest.min(6);
    let total = temporal_duration_time_nanoseconds(fields)?;
    let rounded = temporal::round_signed_to_increment(total, increment, rounding_mode)
        .ok_or_else(|| Error::range("Temporal.Duration rounding failed"))?;

    values[4..].fill(0);
    let mut remainder = rounded;
    if rounded_largest <= 3 {
        let carried_days = remainder / 86_400_000_000_000_i128;
        values[3] = values[3]
            .checked_add(carried_days)
            .ok_or_else(|| Error::range("Temporal.Duration rounding failed"))?;
        remainder %= 86_400_000_000_000_i128;
    }
    for (index, unit) in [
        (4, 3_600_000_000_000_i128),
        (5, 60_000_000_000_i128),
        (6, 1_000_000_000_i128),
        (7, 1_000_000_i128),
        (8, 1_000_i128),
        (9, 1_i128),
    ] {
        if index >= rounded_largest.max(4) {
            values[index] = remainder / unit;
            remainder %= unit;
        }
    }
    temporal_duration_fields_from_integer_values(values)
}

fn temporal_duration_precision_increment(
    precision: temporal::InstantPrecision,
) -> error::Result<i128> {
    match precision {
        temporal::InstantPrecision::Auto => Ok(1),
        temporal::InstantPrecision::Digits(digits) if digits <= 9 => {
            Ok(10_i128.pow(u32::from(9 - digits)))
        }
        temporal::InstantPrecision::Minute | temporal::InstantPrecision::Digits(_) => {
            Err(Error::range("Invalid Temporal.Duration precision"))
        }
    }
}

fn temporal_duration_to_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let fields = temporal_duration_slots(vm, this)?;
    let options = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(options, Value::Undefined | Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.Duration.prototype.toString options must be an object",
        ));
    }

    vm.try_reserve_value_roots(std::slice::from_ref(&options))?;
    let options_pin = vm.pin(&options);
    let result = (|| {
        let get_option = |vm: &mut Vm, name: &str| {
            if options.is_undefined() {
                Ok(Value::Undefined)
            } else {
                vm.get_property(&options, name)
            }
        };
        let fractional_second_digits = match get_option(vm, "fractionalSecondDigits")? {
            Value::Undefined => InstantFractionalSecondDigits::Auto,
            Value::Number(number) => InstantFractionalSecondDigits::Number(number),
            value => InstantFractionalSecondDigits::String(temporal_option_to_string(vm, &value)?),
        };
        let fractional_second_digits =
            temporal_instant_fractional_second_digits(fractional_second_digits)?;
        let rounding_mode_value = get_option(vm, "roundingMode")?;
        let rounding_mode = if rounding_mode_value.is_undefined() {
            None
        } else {
            Some(temporal_option_to_string(vm, &rounding_mode_value)?)
        };
        let rounding_mode = temporal_instant_rounding_mode(rounding_mode.as_deref())?;
        let smallest_unit_value = get_option(vm, "smallestUnit")?;
        let smallest_unit = if smallest_unit_value.is_undefined() {
            None
        } else {
            Some(temporal_option_to_string(vm, &smallest_unit_value)?)
        };
        let smallest_unit = temporal_instant_smallest_unit(smallest_unit.as_deref())?;
        let precision = match smallest_unit {
            None => fractional_second_digits.map_or(
                temporal::InstantPrecision::Auto,
                temporal::InstantPrecision::Digits,
            ),
            Some(InstantSmallestUnit::Second) => temporal::InstantPrecision::Digits(0),
            Some(InstantSmallestUnit::Millisecond) => temporal::InstantPrecision::Digits(3),
            Some(InstantSmallestUnit::Microsecond) => temporal::InstantPrecision::Digits(6),
            Some(InstantSmallestUnit::Nanosecond) => temporal::InstantPrecision::Digits(9),
            Some(InstantSmallestUnit::DateOrHour | InstantSmallestUnit::Minute) => {
                return Err(Error::range(
                    "Invalid Temporal.Duration smallestUnit option",
                ));
            }
        };
        let increment = temporal_duration_precision_increment(precision)?;
        let rounded = if increment == 1 {
            fields
        } else {
            temporal_duration_round_for_string(fields, increment, rounding_mode)?
        };
        let integer_values = temporal_duration_integer_values(rounded)?;
        temporal::format_duration(integer_values, precision)
            .map(Arc::<str>::from)
            .map(Value::String)
            .ok_or_else(|| Error::range("Temporal.Duration string formatting failed"))
    })();
    vm.unpin_many(options_pin);
    result
}

fn temporal_duration_to_json(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let fields = temporal_duration_slots(vm, this)?;
    let integer_values = temporal_duration_integer_values(fields)?;
    temporal::format_duration(integer_values, temporal::InstantPrecision::Auto)
        .map(Arc::<str>::from)
        .map(Value::String)
        .ok_or_else(|| Error::range("Temporal.Duration JSON formatting failed"))
}

#[derive(Clone, Copy)]
enum TemporalDurationTotalUnit {
    Year,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
}

impl TemporalDurationTotalUnit {
    fn parse(value: &str) -> error::Result<Self> {
        Ok(match value {
            "year" | "years" => Self::Year,
            "month" | "months" => Self::Month,
            "week" | "weeks" => Self::Week,
            "day" | "days" => Self::Day,
            "hour" | "hours" => Self::Hour,
            "minute" | "minutes" => Self::Minute,
            "second" | "seconds" => Self::Second,
            "millisecond" | "milliseconds" => Self::Millisecond,
            "microsecond" | "microseconds" => Self::Microsecond,
            "nanosecond" | "nanoseconds" => Self::Nanosecond,
            _ => return Err(Error::range("Invalid Temporal total unit option")),
        })
    }

    fn is_calendar(self) -> bool {
        matches!(self, Self::Year | Self::Month | Self::Week)
    }

    fn nanoseconds(self) -> Option<i128> {
        Some(match self {
            Self::Day => 86_400_000_000_000,
            Self::Hour => 3_600_000_000_000,
            Self::Minute => 60_000_000_000,
            Self::Second => 1_000_000_000,
            Self::Millisecond => 1_000_000,
            Self::Microsecond => 1_000,
            Self::Nanosecond => 1,
            Self::Year | Self::Month | Self::Week => return None,
        })
    }
}

enum TemporalDurationRelativeTo {
    Plain {
        date: TemporalPlainDateFields,
        calendar_identifier: Arc<str>,
    },
    Zoned {
        epoch_nanoseconds: Arc<BigInt>,
        time_zone: TemporalTimeZone,
        calendar_identifier: Arc<str>,
    },
}

fn temporal_duration_plain_date_from_property_fields(
    fields: &TemporalZonedDateTimePropertyFields,
) -> error::Result<TemporalPlainDateFields> {
    let year = fields
        .year
        .as_ref()
        .ok_or_else(|| Error::type_err("Temporal property bag requires year"))?
        .to_i128()
        .ok_or_else(|| Error::range("Temporal year is out of range"))?;
    if fields.month.is_none() && fields.month_code.is_none() {
        return Err(Error::type_err(
            "Temporal property bag requires month or monthCode",
        ));
    }
    let mut month = fields
        .month
        .clone()
        .unwrap_or_else(|| BigInt::from(fields.month_code.unwrap().0));
    if let Some((month_code, leap)) = fields.month_code {
        if leap || !(1..=12).contains(&month_code) {
            return Err(Error::range("Invalid monthCode for ISO 8601 calendar"));
        }
        if fields
            .month
            .as_ref()
            .is_some_and(|value| value != &BigInt::from(month_code))
        {
            return Err(Error::range("month and monthCode do not agree"));
        }
        month = BigInt::from(month_code);
    }
    if month <= BigInt::zero() {
        return Err(Error::range("Temporal month is out of range"));
    }
    let month = month
        .min(BigInt::from(12))
        .to_i128()
        .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    let day = fields
        .day
        .as_ref()
        .ok_or_else(|| Error::type_err("Temporal property bag requires day"))?;
    if day <= &BigInt::zero() {
        return Err(Error::range("Temporal day is out of range"));
    }
    let maximum_day = temporal::days_in_month(year, month)
        .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    let day = day
        .min(&BigInt::from(maximum_day))
        .to_i128()
        .ok_or_else(|| Error::range("Temporal day is out of range"))?;
    temporal_plain_date_fields([BigInt::from(year), BigInt::from(month), BigInt::from(day)])
        .ok_or_else(|| Error::range("Temporal relativeTo date is out of range"))
}

fn temporal_duration_relative_to(
    vm: &mut Vm,
    value: &Value,
) -> error::Result<Option<TemporalDurationRelativeTo>> {
    if value.is_undefined() {
        return Ok(None);
    }
    if let Some((date, calendar_identifier)) = temporal_plain_date_slots_if_present(vm, value) {
        return Ok(Some(TemporalDurationRelativeTo::Plain {
            date,
            calendar_identifier,
        }));
    }
    if let Some((date_time, calendar_identifier)) =
        temporal_plain_date_time_slots_if_present(vm, value)
    {
        return Ok(Some(TemporalDurationRelativeTo::Plain {
            date: TemporalPlainDateFields {
                year: date_time.year,
                month: date_time.month,
                day: date_time.day,
            },
            calendar_identifier,
        }));
    }
    if let Some((epoch_nanoseconds, time_zone, calendar_identifier)) =
        temporal_zoned_date_time_slots_if_present(vm, value)
    {
        return Ok(Some(TemporalDurationRelativeTo::Zoned {
            epoch_nanoseconds,
            time_zone,
            calendar_identifier,
        }));
    }
    if matches!(value, Value::Object(_)) {
        let fields = temporal_zoned_date_time_property_fields(vm, value)?;
        if fields.time_zone.is_some() {
            let (epoch_nanoseconds, time_zone, calendar_identifier) =
                temporal_zoned_date_time_from_property_fields(
                    fields,
                    TemporalZonedDateTimeFromOptions {
                        offset: temporal::ZonedDateTimeOffsetOption::Reject,
                        overflow: TemporalOverflow::Constrain,
                    },
                )?;
            return Ok(Some(TemporalDurationRelativeTo::Zoned {
                epoch_nanoseconds,
                time_zone,
                calendar_identifier,
            }));
        }
        let date = temporal_duration_plain_date_from_property_fields(&fields)?;
        return Ok(Some(TemporalDurationRelativeTo::Plain {
            date,
            calendar_identifier: fields.calendar_identifier,
        }));
    }
    let Value::String(source) = value else {
        return Err(Error::type_err(
            "Temporal relativeTo must be a String or object",
        ));
    };
    vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
    if let Some(parsed) = temporal::parse_zoned_date_time_string(source) {
        let epoch_nanoseconds = temporal::resolve_zoned_date_time_epoch(
            &parsed,
            temporal::ZonedDateTimeOffsetOption::Reject,
        )
        .ok_or_else(|| Error::range("Temporal relativeTo offset does not match"))?;
        if epoch_nanoseconds.abs() > temporal_instant_limit_nanoseconds() {
            return Err(Error::range("Temporal relativeTo is out of range"));
        }
        return Ok(Some(TemporalDurationRelativeTo::Zoned {
            epoch_nanoseconds: Arc::new(epoch_nanoseconds),
            time_zone: temporal_time_zone_from_identifier(
                parsed.time_zone_identifier,
                parsed.offset_minutes,
            ),
            calendar_identifier: parsed.calendar_identifier,
        }));
    }
    if temporal::has_time_zone_annotation(source) {
        return Err(Error::range("Named Temporal time zones are not available"));
    }
    let parsed = temporal::parse_plain_date_string(source)
        .ok_or_else(|| Error::range("Invalid Temporal relativeTo string"))?;
    let date = temporal_plain_date_fields([
        BigInt::from(parsed.year),
        BigInt::from(parsed.month),
        BigInt::from(parsed.day),
    ])
    .ok_or_else(|| Error::range("Temporal relativeTo is out of range"))?;
    Ok(Some(TemporalDurationRelativeTo::Plain {
        date,
        calendar_identifier: parsed.calendar_identifier,
    }))
}

const TEMPORAL_DURATION_DAY_NANOSECONDS: i128 = 86_400_000_000_000;

fn temporal_duration_iso_date_add(
    date: TemporalPlainDateFields,
    years: i128,
    months: i128,
    weeks: i128,
    days: i128,
) -> error::Result<i128> {
    let month_index = i128::from(date.year)
        .checked_mul(12)
        .and_then(|value| value.checked_add(i128::from(date.month) - 1))
        .and_then(|value| value.checked_add(years.checked_mul(12)?))
        .and_then(|value| value.checked_add(months))
        .ok_or_else(|| Error::range("Temporal relative date is out of range"))?;
    let year = month_index.div_euclid(12);
    let month = month_index.rem_euclid(12) + 1;
    let day = i128::from(date.day).min(
        temporal::days_in_month(year, month)
            .ok_or_else(|| Error::range("Temporal relative date is out of range"))?,
    );
    let epoch_day = temporal::days_from_civil(year, month, day)
        .and_then(|value| value.checked_add(weeks.checked_mul(7)?))
        .and_then(|value| value.checked_add(days))
        .ok_or_else(|| Error::range("Temporal relative date is out of range"))?;
    let (year, month, day) = temporal::civil_from_days(epoch_day)
        .ok_or_else(|| Error::range("Temporal relative date is out of range"))?;
    temporal_plain_date_fields([BigInt::from(year), BigInt::from(month), BigInt::from(day)])
        .ok_or_else(|| Error::range("Temporal relative date is out of range"))?;
    Ok(epoch_day)
}

fn temporal_duration_calendar_boundary_nanoseconds(
    date: TemporalPlainDateFields,
    unit: TemporalDurationTotalUnit,
    count: i128,
) -> error::Result<i128> {
    let (years, months) = match unit {
        TemporalDurationTotalUnit::Year => (count, 0),
        TemporalDurationTotalUnit::Month => (0, count),
        _ => return Err(Error::internal("Invalid Temporal calendar boundary unit")),
    };
    temporal_duration_iso_date_add(date, years, months, 0, 0)?
        .checked_mul(TEMPORAL_DURATION_DAY_NANOSECONDS)
        .ok_or_else(|| Error::range("Temporal relative date is out of range"))
}

fn temporal_duration_total_calendar_unit(
    date: TemporalPlainDateFields,
    destination_nanoseconds: i128,
    unit: TemporalDurationTotalUnit,
) -> error::Result<f64> {
    let start_day = temporal::days_from_civil(
        i128::from(date.year),
        i128::from(date.month),
        i128::from(date.day),
    )
    .ok_or_else(|| Error::internal("Invalid Temporal relative date"))?;
    let start_nanoseconds = start_day
        .checked_mul(TEMPORAL_DURATION_DAY_NANOSECONDS)
        .ok_or_else(|| Error::range("Temporal relative date is out of range"))?;
    let direction = match destination_nanoseconds.cmp(&start_nanoseconds) {
        std::cmp::Ordering::Less => -1_i128,
        std::cmp::Ordering::Equal => return Ok(0.0),
        std::cmp::Ordering::Greater => 1_i128,
    };
    let destination_day = destination_nanoseconds.div_euclid(TEMPORAL_DURATION_DAY_NANOSECONDS);
    let (destination_year, destination_month, _) = temporal::civil_from_days(destination_day)
        .ok_or_else(|| Error::range("Temporal destination is out of range"))?;
    let mut whole = match unit {
        TemporalDurationTotalUnit::Year => destination_year - i128::from(date.year),
        TemporalDurationTotalUnit::Month => destination_year
            .checked_mul(12)
            .and_then(|value| value.checked_add(destination_month - 1))
            .and_then(|value| {
                value.checked_sub(i128::from(date.year) * 12 + (i128::from(date.month) - 1))
            })
            .ok_or_else(|| Error::range("Temporal total is out of range"))?,
        _ => return Err(Error::internal("Invalid Temporal calendar total unit")),
    };
    loop {
        let boundary = temporal_duration_calendar_boundary_nanoseconds(date, unit, whole)?;
        let overshot = if direction > 0 {
            boundary > destination_nanoseconds
        } else {
            boundary < destination_nanoseconds
        };
        if !overshot {
            break;
        }
        whole = whole
            .checked_sub(direction)
            .ok_or_else(|| Error::range("Temporal total is out of range"))?;
    }
    loop {
        let candidate = whole
            .checked_add(direction)
            .ok_or_else(|| Error::range("Temporal total is out of range"))?;
        let boundary = temporal_duration_calendar_boundary_nanoseconds(date, unit, candidate)?;
        let fits = if direction > 0 {
            boundary <= destination_nanoseconds
        } else {
            boundary >= destination_nanoseconds
        };
        if !fits {
            break;
        }
        whole = candidate;
    }
    let boundary = temporal_duration_calendar_boundary_nanoseconds(date, unit, whole)?;
    let next = temporal_duration_calendar_boundary_nanoseconds(
        date,
        unit,
        whole
            .checked_add(direction)
            .ok_or_else(|| Error::range("Temporal total is out of range"))?,
    )?;
    let span = next
        .checked_sub(boundary)
        .and_then(i128::checked_abs)
        .ok_or_else(|| Error::range("Temporal total is out of range"))?;
    let progress = destination_nanoseconds
        .checked_sub(boundary)
        .and_then(i128::checked_abs)
        .ok_or_else(|| Error::range("Temporal total is out of range"))?;
    let numerator =
        BigInt::from(whole) * BigInt::from(span) + BigInt::from(direction) * BigInt::from(progress);
    Ratio::new(numerator, BigInt::from(span))
        .to_f64()
        .ok_or_else(|| Error::range("Temporal total is out of range"))
}

fn temporal_duration_total_with_relative_to(
    fields: TemporalDurationFields,
    unit: TemporalDurationTotalUnit,
    relative_to: TemporalDurationRelativeTo,
) -> error::Result<f64> {
    let values = temporal_duration_integer_values(fields)?;
    if values.iter().all(|value| *value == 0) {
        return Ok(0.0);
    }
    let (date, zoned_epoch_nanoseconds, plain_relative) = match relative_to {
        TemporalDurationRelativeTo::Plain {
            date,
            calendar_identifier,
        } => {
            if calendar_identifier.as_ref() != "iso8601" {
                return Err(Error::range("Non-ISO Temporal calendars are not available"));
            }
            (date, None, true)
        }
        TemporalDurationRelativeTo::Zoned {
            epoch_nanoseconds,
            time_zone,
            calendar_identifier,
        } => {
            if calendar_identifier.as_ref() != "iso8601" {
                return Err(Error::range("Non-ISO Temporal calendars are not available"));
            }
            if matches!(time_zone.kind, TemporalTimeZoneKind::Named(_)) {
                return Err(Error::range("Named Temporal time zones are not available"));
            }
            (
                temporal_zoned_date_time_plain_date_fields(&epoch_nanoseconds, &time_zone)?,
                Some(epoch_nanoseconds),
                false,
            )
        }
    };
    let start_day = temporal::days_from_civil(
        i128::from(date.year),
        i128::from(date.month),
        i128::from(date.day),
    )
    .ok_or_else(|| Error::internal("Invalid Temporal relative date"))?;
    let start_nanoseconds = start_day
        .checked_mul(TEMPORAL_DURATION_DAY_NANOSECONDS)
        .ok_or_else(|| Error::range("Temporal relative date is out of range"))?;
    let minimum_plain_nanoseconds = temporal::days_from_civil(-271_821, 4, 20)
        .and_then(|value| value.checked_mul(TEMPORAL_DURATION_DAY_NANOSECONDS))
        .ok_or_else(|| Error::internal("Invalid Temporal minimum date"))?;
    let maximum_plain_nanoseconds = temporal::days_from_civil(275_760, 9, 14)
        .and_then(|value| value.checked_mul(TEMPORAL_DURATION_DAY_NANOSECONDS))
        .ok_or_else(|| Error::internal("Invalid Temporal maximum date"))?;
    if plain_relative
        && !(minimum_plain_nanoseconds..maximum_plain_nanoseconds).contains(&start_nanoseconds)
    {
        return Err(Error::range("Temporal relative date-time is out of range"));
    }
    let end_day = temporal_duration_iso_date_add(date, values[0], values[1], values[2], values[3])?;
    let time_nanoseconds = temporal_duration_time_nanoseconds(fields)?;
    let destination_nanoseconds = end_day
        .checked_mul(TEMPORAL_DURATION_DAY_NANOSECONDS)
        .and_then(|value| value.checked_add(time_nanoseconds))
        .ok_or_else(|| Error::range("Temporal destination is out of range"))?;
    if plain_relative
        && !(minimum_plain_nanoseconds..maximum_plain_nanoseconds)
            .contains(&destination_nanoseconds)
    {
        return Err(Error::range("Temporal destination is out of range"));
    }
    let destination = temporal::iso_date_time(&BigInt::from(destination_nanoseconds), 0)
        .ok_or_else(|| Error::range("Temporal destination is out of range"))?;
    temporal_plain_date_time_fields_from_iso(temporal::IsoDateTimeFields {
        year: destination.year,
        month: destination.month,
        day: destination.day,
        hour: destination.hour,
        minute: destination.minute,
        second: destination.second,
        millisecond: destination.millisecond,
        microsecond: destination.microsecond,
        nanosecond: destination.nanosecond,
    })?;
    let total_nanoseconds = destination_nanoseconds
        .checked_sub(start_nanoseconds)
        .ok_or_else(|| Error::range("Temporal total is out of range"))?;
    if let Some(epoch_nanoseconds) = zoned_epoch_nanoseconds {
        let target = epoch_nanoseconds.as_ref() + BigInt::from(total_nanoseconds);
        if target.abs() > temporal_instant_limit_nanoseconds() {
            return Err(Error::range("Temporal target epoch is out of range"));
        }
    }
    if matches!(
        unit,
        TemporalDurationTotalUnit::Year | TemporalDurationTotalUnit::Month
    ) {
        return temporal_duration_total_calendar_unit(date, destination_nanoseconds, unit);
    }
    let divisor = match unit {
        TemporalDurationTotalUnit::Week => TEMPORAL_DURATION_DAY_NANOSECONDS * 7,
        _ => unit
            .nanoseconds()
            .ok_or_else(|| Error::internal("Invalid Temporal total unit"))?,
    };
    Ratio::new(BigInt::from(total_nanoseconds), BigInt::from(divisor))
        .to_f64()
        .ok_or_else(|| Error::range("Temporal total is out of range"))
}

fn temporal_duration_total_without_relative_to(
    fields: TemporalDurationFields,
    unit: TemporalDurationTotalUnit,
) -> error::Result<f64> {
    let values = temporal_duration_integer_values(fields)?;
    if values[..3].iter().any(|value| *value != 0) || unit.is_calendar() {
        return Err(Error::range(
            "Temporal.Duration calendar units require relativeTo",
        ));
    }
    let total_nanoseconds = values[3]
        .checked_mul(86_400_000_000_000)
        .and_then(|value| value.checked_add(values[4].checked_mul(3_600_000_000_000)?))
        .and_then(|value| value.checked_add(values[5].checked_mul(60_000_000_000)?))
        .and_then(|value| value.checked_add(values[6].checked_mul(1_000_000_000)?))
        .and_then(|value| value.checked_add(values[7].checked_mul(1_000_000)?))
        .and_then(|value| value.checked_add(values[8].checked_mul(1_000)?))
        .and_then(|value| value.checked_add(values[9]))
        .ok_or_else(|| Error::range("Temporal.Duration total is out of range"))?;
    let divisor = unit
        .nanoseconds()
        .ok_or_else(|| Error::range("Temporal.Duration calendar units require relativeTo"))?;
    Ratio::new(BigInt::from(total_nanoseconds), BigInt::from(divisor))
        .to_f64()
        .ok_or_else(|| Error::range("Temporal.Duration total is out of range"))
}

fn temporal_duration_total(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let fields = temporal_duration_slots(vm, this)?;
    let total_of = args.first().cloned().unwrap_or(Value::Undefined);
    if total_of.is_undefined() {
        return Err(Error::type_err(
            "Temporal.Duration.prototype.total requires an argument",
        ));
    }

    let (relative_to, unit) = match &total_of {
        Value::String(unit) => (None, TemporalDurationTotalUnit::parse(unit)?),
        Value::Object(_) => {
            vm.try_reserve_value_roots(std::slice::from_ref(&total_of))?;
            let options_pin = vm.pin(&total_of);
            let result = (|| {
                let relative_to = vm.get_property(&total_of, "relativeTo")?;
                vm.try_reserve_value_roots(std::slice::from_ref(&relative_to))?;
                let relative_pin = vm.pin(&relative_to);
                let unit_result = (|| {
                    let relative_to = temporal_duration_relative_to(vm, &relative_to)?;
                    let unit = vm.get_property(&total_of, "unit")?;
                    if unit.is_undefined() {
                        return Err(Error::range(
                            "Temporal.Duration.prototype.total requires a unit",
                        ));
                    }
                    let unit = temporal_option_to_string(vm, &unit)?;
                    Ok((relative_to, TemporalDurationTotalUnit::parse(&unit)?))
                })();
                vm.unpin_many(relative_pin);
                unit_result
            })();
            vm.unpin_many(options_pin);
            result?
        }
        _ => {
            return Err(Error::type_err(
                "Temporal.Duration.prototype.total argument must be a String or object",
            ));
        }
    };

    if let Some(relative_to) = relative_to {
        return temporal_duration_total_with_relative_to(fields, unit, relative_to)
            .map(Value::Number);
    }
    temporal_duration_total_without_relative_to(fields, unit).map(Value::Number)
}

fn temporal_duration_value_of(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Err(Error::type_err(
        "Temporal.Duration.prototype.valueOf always throws",
    ))
}

fn temporal_duration_time_nanoseconds(fields: TemporalDurationFields) -> error::Result<i128> {
    if !temporal_duration_is_valid(&fields) {
        return Err(Error::range("Invalid Temporal.Duration fields"));
    }
    let integer = |value: f64| {
        BigInt::from_f64(value).ok_or_else(|| Error::range("Invalid Temporal.Duration fields"))
    };
    let total = integer(fields.hours)? * BigInt::from(3_600_000_000_000_i64)
        + integer(fields.minutes)? * BigInt::from(60_000_000_000_i64)
        + integer(fields.seconds)? * BigInt::from(1_000_000_000_i64)
        + integer(fields.milliseconds)? * BigInt::from(1_000_000_i64)
        + integer(fields.microseconds)? * BigInt::from(1_000_i64)
        + integer(fields.nanoseconds)?;
    total
        .to_i128()
        .ok_or_else(|| Error::range("Temporal.Duration time fields are out of range"))
}

fn temporal_duration_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Temporal.Duration requires 'new'"));
    }
    let mut values = [0.0; 10];
    for (index, output) in values.iter_mut().enumerate() {
        let value = args.get(index).cloned().unwrap_or(Value::Undefined);
        if !matches!(value, Value::Undefined) {
            *output = temporal_integer_if_integral(vm, value)?;
        }
    }
    let fields = TemporalDurationFields {
        years: values[0],
        months: values[1],
        weeks: values[2],
        days: values[3],
        hours: values[4],
        minutes: values[5],
        seconds: values[6],
        milliseconds: values[7],
        microseconds: values[8],
        nanoseconds: values[9],
    };
    if !temporal_duration_is_valid(&fields) {
        return Err(Error::range("Invalid Temporal.Duration fields"));
    }

    let realm = env::global_env_root(&vm.heap, vm.native_callee_closure().unwrap_or(vm.global));
    let fallback = vm
        .realm_temporal_duration_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.Duration prototype is not installed"))?;
    let prototype = native_constructor_prototype_with_default(vm, "Temporal.Duration", fallback)?;
    create_temporal_duration(vm, fields, prototype)
}

macro_rules! temporal_duration_getter {
    ($name:ident, $field:ident) => {
        fn $name(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
            temporal_duration_slots(vm, this).map(|fields| Value::Number(fields.$field))
        }
    };
}

temporal_duration_getter!(temporal_duration_years, years);
temporal_duration_getter!(temporal_duration_months, months);
temporal_duration_getter!(temporal_duration_weeks, weeks);
temporal_duration_getter!(temporal_duration_days, days);
temporal_duration_getter!(temporal_duration_hours, hours);
temporal_duration_getter!(temporal_duration_minutes, minutes);
temporal_duration_getter!(temporal_duration_seconds, seconds);
temporal_duration_getter!(temporal_duration_milliseconds, milliseconds);
temporal_duration_getter!(temporal_duration_microseconds, microseconds);
temporal_duration_getter!(temporal_duration_nanoseconds, nanoseconds);

fn temporal_duration_sign(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_duration_slots(vm, this)
        .map(|fields| Value::Number(f64::from(temporal_duration_sign_value(&fields))))
}

fn temporal_duration_blank(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_duration_slots(vm, this)
        .map(|fields| Value::Bool(temporal_duration_sign_value(&fields) == 0))
}

fn temporal_plain_date_time_slots(
    vm: &Vm,
    this: Option<Value>,
) -> error::Result<(TemporalPlainDateTimeFields, Arc<str>)> {
    let Value::Object(index) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(
            "Temporal.PlainDateTime method called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::PlainDateTime {
                    fields,
                    calendar_identifier,
                },
            ..
        }) => Ok((*fields, calendar_identifier.clone())),
        _ => Err(Error::type_err(
            "Temporal.PlainDateTime method called on incompatible receiver",
        )),
    })
}

fn temporal_plain_date_time_slots_if_present(
    vm: &Vm,
    value: &Value,
) -> Option<(TemporalPlainDateTimeFields, Arc<str>)> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::PlainDateTime {
                    fields,
                    calendar_identifier,
                },
            ..
        }) => Some((*fields, calendar_identifier.clone())),
        _ => None,
    })
}

fn temporal_plain_date_time_fields(values: [BigInt; 9]) -> Option<TemporalPlainDateTimeFields> {
    let [year, month, day, hour, minute, second, millisecond, microsecond, nanosecond] = values;
    let (year, month, day, hour, minute, second, millisecond, microsecond, nanosecond) = (
        year.to_i128()?,
        month.to_i128()?,
        day.to_i128()?,
        hour.to_i128()?,
        minute.to_i128()?,
        second.to_i128()?,
        millisecond.to_i128()?,
        microsecond.to_i128()?,
        nanosecond.to_i128()?,
    );
    if day < 1
        || day > temporal::days_in_month(year, month)?
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
        || !(0..=999).contains(&millisecond)
        || !(0..=999).contains(&microsecond)
        || !(0..=999).contains(&nanosecond)
    {
        return None;
    }
    let local_nanoseconds =
        temporal::iso_date_time_to_local_nanoseconds(temporal::IsoDateTimeFields {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
            microsecond,
            nanosecond,
        })?;
    let instant_limit = i128::from(TEMPORAL_INSTANT_LIMIT_MILLISECONDS).checked_mul(1_000_000)?;
    let day_nanoseconds = 86_400_i128.checked_mul(1_000_000_000)?;
    let limit = instant_limit.checked_add(day_nanoseconds)?;
    if local_nanoseconds <= -limit || local_nanoseconds >= limit {
        return None;
    }
    Some(TemporalPlainDateTimeFields {
        year: i32::try_from(year).ok()?,
        month: u8::try_from(month).ok()?,
        day: u8::try_from(day).ok()?,
        hour: u8::try_from(hour).ok()?,
        minute: u8::try_from(minute).ok()?,
        second: u8::try_from(second).ok()?,
        millisecond: u16::try_from(millisecond).ok()?,
        microsecond: u16::try_from(microsecond).ok()?,
        nanosecond: u16::try_from(nanosecond).ok()?,
    })
}

fn temporal_plain_date_time_fields_from_iso(
    fields: temporal::IsoDateTimeFields,
) -> error::Result<TemporalPlainDateTimeFields> {
    temporal_plain_date_time_fields([
        BigInt::from(fields.year),
        BigInt::from(fields.month),
        BigInt::from(fields.day),
        BigInt::from(fields.hour),
        BigInt::from(fields.minute),
        BigInt::from(fields.second),
        BigInt::from(fields.millisecond),
        BigInt::from(fields.microsecond),
        BigInt::from(fields.nanosecond),
    ])
    .ok_or_else(|| Error::range("Temporal.PlainDateTime fields are out of range"))
}

fn temporal_plain_date_time_is_valid(fields: TemporalPlainDateTimeFields) -> bool {
    let validated = temporal_plain_date_time_fields([
        BigInt::from(fields.year),
        BigInt::from(fields.month),
        BigInt::from(fields.day),
        BigInt::from(fields.hour),
        BigInt::from(fields.minute),
        BigInt::from(fields.second),
        BigInt::from(fields.millisecond),
        BigInt::from(fields.microsecond),
        BigInt::from(fields.nanosecond),
    ]);
    validated.is_some_and(|validated| validated == fields)
}

fn temporal_plain_date_time_iso(
    fields: TemporalPlainDateTimeFields,
) -> error::Result<temporal::IsoDateTime> {
    let local_nanoseconds =
        temporal::iso_date_time_to_local_nanoseconds(temporal::IsoDateTimeFields {
            year: i128::from(fields.year),
            month: i128::from(fields.month),
            day: i128::from(fields.day),
            hour: i128::from(fields.hour),
            minute: i128::from(fields.minute),
            second: i128::from(fields.second),
            millisecond: i128::from(fields.millisecond),
            microsecond: i128::from(fields.microsecond),
            nanosecond: i128::from(fields.nanosecond),
        })
        .ok_or_else(|| Error::internal("Invalid Temporal.PlainDateTime slots"))?;
    temporal::iso_date_time(&BigInt::from(local_nanoseconds), 0)
        .ok_or_else(|| Error::internal("Invalid Temporal.PlainDateTime slots"))
}

fn create_temporal_plain_date_time(
    vm: &mut Vm,
    fields: TemporalPlainDateTimeFields,
    calendar_identifier: Arc<str>,
    prototype: Value,
) -> error::Result<Value> {
    if !temporal_plain_date_time_is_valid(fields) {
        return Err(Error::range("Invalid Temporal.PlainDateTime fields"));
    }
    vm.try_reserve_gc_pins(1)?;
    let pin_count = vm.pin(&prototype);
    let result = vm.alloc(HeapObj::Temporal(TemporalData {
        kind: TemporalKind::PlainDateTime {
            fields,
            calendar_identifier,
        },
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    result.map(Value::Object)
}

pub(crate) fn create_temporal_plain_date_time_in_realm(
    vm: &mut Vm,
    fields: TemporalPlainDateTimeFields,
    calendar_identifier: Arc<str>,
    realm: GcIdx,
) -> error::Result<Value> {
    let prototype = vm
        .realm_temporal_plain_date_time_prototypes
        .get(&env::global_env_root(&vm.heap, realm).0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainDateTime prototype is not installed"))?;
    create_temporal_plain_date_time(vm, fields, calendar_identifier, prototype)
}

fn temporal_constructor_calendar(
    vm: &mut Vm,
    value: Value,
    type_name: &str,
) -> error::Result<Arc<str>> {
    let source = match value {
        Value::Undefined => return Ok(Arc::from("iso8601")),
        Value::String(source) => source,
        _ => {
            return Err(Error::type_err(format!(
                "Temporal.{type_name} calendar must be a String"
            )));
        }
    };
    vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
    source
        .eq_ignore_ascii_case("iso8601")
        .then(|| Arc::from("iso8601"))
        .ok_or_else(|| Error::range("Invalid Temporal calendar identifier"))
}

fn temporal_plain_date_time_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Temporal.PlainDateTime requires 'new'"));
    }
    let mut values: [BigInt; 9] = std::array::from_fn(|_| BigInt::zero());
    for (index, output) in values.iter_mut().enumerate() {
        let value = args.get(index).cloned().unwrap_or(Value::Undefined);
        if index < 3 || !value.is_undefined() {
            *output = temporal_integer_with_truncation(vm, value)?;
        }
    }
    let calendar_identifier = temporal_constructor_calendar(
        vm,
        args.get(9).cloned().unwrap_or(Value::Undefined),
        "PlainDateTime",
    )?;
    let fields = temporal_plain_date_time_fields(values)
        .ok_or_else(|| Error::range("Invalid Temporal.PlainDateTime fields"))?;
    let realm = env::global_env_root(&vm.heap, vm.native_callee_closure().unwrap_or(vm.global));
    let fallback = vm
        .realm_temporal_plain_date_time_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainDateTime prototype is not installed"))?;
    let prototype =
        native_constructor_prototype_with_default(vm, "Temporal.PlainDateTime", fallback)?;
    create_temporal_plain_date_time(vm, fields, calendar_identifier, prototype)
}

macro_rules! temporal_plain_date_time_number_getter {
    ($name:ident, $field:ident) => {
        fn $name(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
            temporal_plain_date_time_slots(vm, this)
                .map(|(fields, _)| Value::Number(f64::from(fields.$field)))
        }
    };
}

temporal_plain_date_time_number_getter!(temporal_plain_date_time_year, year);
temporal_plain_date_time_number_getter!(temporal_plain_date_time_month, month);
temporal_plain_date_time_number_getter!(temporal_plain_date_time_day, day);
temporal_plain_date_time_number_getter!(temporal_plain_date_time_hour, hour);
temporal_plain_date_time_number_getter!(temporal_plain_date_time_minute, minute);
temporal_plain_date_time_number_getter!(temporal_plain_date_time_second, second);
temporal_plain_date_time_number_getter!(temporal_plain_date_time_millisecond, millisecond);
temporal_plain_date_time_number_getter!(temporal_plain_date_time_microsecond, microsecond);
temporal_plain_date_time_number_getter!(temporal_plain_date_time_nanosecond, nanosecond);

fn temporal_plain_date_time_calendar_id(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_date_time_slots(vm, this).map(|(_, calendar)| Value::String(calendar))
}

fn temporal_plain_date_time_era(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_date_time_slots(vm, this).map(|_| Value::Undefined)
}

fn temporal_plain_date_time_era_year(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_date_time_slots(vm, this).map(|_| Value::Undefined)
}

fn temporal_plain_date_time_month_code(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_date_time_slots(vm, this)
        .map(|(fields, _)| Value::String(Arc::from(format!("M{:02}", fields.month))))
}

fn temporal_plain_date_time_value_of(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Err(Error::type_err(
        "Temporal.PlainDateTime.prototype.valueOf always throws",
    ))
}

#[derive(Clone, Copy)]
enum TemporalPlainDateTimeComputedField {
    DayOfWeek,
    DayOfYear,
    WeekOfYear,
    YearOfWeek,
    DaysInWeek,
    DaysInMonth,
    DaysInYear,
    MonthsInYear,
    InLeapYear,
}

fn temporal_plain_date_time_computed_field(
    vm: &mut Vm,
    this: Option<Value>,
    field: TemporalPlainDateTimeComputedField,
) -> error::Result<Value> {
    let (fields, _) = temporal_plain_date_time_slots(vm, this)?;
    let date_time = temporal_plain_date_time_iso(fields)?;
    temporal_iso_date_computed_field(date_time, field)
}

fn temporal_iso_date_computed_field(
    date_time: temporal::IsoDateTime,
    field: TemporalPlainDateTimeComputedField,
) -> error::Result<Value> {
    Ok(match field {
        TemporalPlainDateTimeComputedField::DayOfWeek => {
            Value::Number(temporal::iso_day_of_week(date_time.epoch_days) as f64)
        }
        TemporalPlainDateTimeComputedField::DayOfYear => Value::Number(
            temporal::iso_day_of_year(date_time)
                .ok_or_else(|| Error::internal("Invalid Temporal.PlainDateTime slots"))?
                as f64,
        ),
        TemporalPlainDateTimeComputedField::WeekOfYear => Value::Number(
            temporal::iso_week_of_year(date_time)
                .ok_or_else(|| Error::internal("Invalid Temporal.PlainDateTime slots"))?
                .0 as f64,
        ),
        TemporalPlainDateTimeComputedField::YearOfWeek => Value::Number(
            temporal::iso_week_of_year(date_time)
                .ok_or_else(|| Error::internal("Invalid Temporal.PlainDateTime slots"))?
                .1 as f64,
        ),
        TemporalPlainDateTimeComputedField::DaysInWeek => Value::Number(7.0),
        TemporalPlainDateTimeComputedField::DaysInMonth => Value::Number(
            temporal::days_in_month(date_time.year, date_time.month)
                .ok_or_else(|| Error::internal("Invalid Temporal.PlainDateTime slots"))?
                as f64,
        ),
        TemporalPlainDateTimeComputedField::DaysInYear => {
            Value::Number(if temporal::leap_year(date_time.year) {
                366.0
            } else {
                365.0
            })
        }
        TemporalPlainDateTimeComputedField::MonthsInYear => Value::Number(12.0),
        TemporalPlainDateTimeComputedField::InLeapYear => {
            Value::Bool(temporal::leap_year(date_time.year))
        }
    })
}

macro_rules! temporal_plain_date_time_computed_getter {
    ($name:ident, $field:ident) => {
        fn $name(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
            temporal_plain_date_time_computed_field(
                vm,
                this,
                TemporalPlainDateTimeComputedField::$field,
            )
        }
    };
}

temporal_plain_date_time_computed_getter!(temporal_plain_date_time_day_of_week, DayOfWeek);
temporal_plain_date_time_computed_getter!(temporal_plain_date_time_day_of_year, DayOfYear);
temporal_plain_date_time_computed_getter!(temporal_plain_date_time_week_of_year, WeekOfYear);
temporal_plain_date_time_computed_getter!(temporal_plain_date_time_year_of_week, YearOfWeek);
temporal_plain_date_time_computed_getter!(temporal_plain_date_time_days_in_week, DaysInWeek);
temporal_plain_date_time_computed_getter!(temporal_plain_date_time_days_in_month, DaysInMonth);
temporal_plain_date_time_computed_getter!(temporal_plain_date_time_days_in_year, DaysInYear);
temporal_plain_date_time_computed_getter!(temporal_plain_date_time_months_in_year, MonthsInYear);
temporal_plain_date_time_computed_getter!(temporal_plain_date_time_in_leap_year, InLeapYear);

fn temporal_plain_date_slots(
    vm: &Vm,
    this: Option<Value>,
) -> error::Result<(TemporalPlainDateFields, Arc<str>)> {
    let Value::Object(index) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(
            "Temporal.PlainDate method called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::PlainDate {
                    fields,
                    calendar_identifier,
                },
            ..
        }) => Ok((*fields, calendar_identifier.clone())),
        _ => Err(Error::type_err(
            "Temporal.PlainDate method called on incompatible receiver",
        )),
    })
}

fn temporal_plain_date_slots_if_present(
    vm: &Vm,
    value: &Value,
) -> Option<(TemporalPlainDateFields, Arc<str>)> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::PlainDate {
                    fields,
                    calendar_identifier,
                },
            ..
        }) => Some((*fields, calendar_identifier.clone())),
        _ => None,
    })
}

fn temporal_plain_date_fields(values: [BigInt; 3]) -> Option<TemporalPlainDateFields> {
    let [year, month, day] = values;
    let date_time = temporal_plain_date_time_fields([
        year,
        month,
        day,
        BigInt::from(12),
        BigInt::zero(),
        BigInt::zero(),
        BigInt::zero(),
        BigInt::zero(),
        BigInt::zero(),
    ])?;
    Some(TemporalPlainDateFields {
        year: date_time.year,
        month: date_time.month,
        day: date_time.day,
    })
}

fn temporal_plain_date_is_valid(fields: TemporalPlainDateFields) -> bool {
    temporal_plain_date_fields([
        BigInt::from(fields.year),
        BigInt::from(fields.month),
        BigInt::from(fields.day),
    ])
    .is_some_and(|validated| validated == fields)
}

fn create_temporal_plain_date(
    vm: &mut Vm,
    fields: TemporalPlainDateFields,
    calendar_identifier: Arc<str>,
    prototype: Value,
) -> error::Result<Value> {
    if !temporal_plain_date_is_valid(fields) {
        return Err(Error::range("Invalid Temporal.PlainDate fields"));
    }
    vm.try_reserve_gc_pins(1)?;
    let pin_count = vm.pin(&prototype);
    let result = vm.alloc(HeapObj::Temporal(TemporalData {
        kind: TemporalKind::PlainDate {
            fields,
            calendar_identifier,
        },
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    result.map(Value::Object)
}

fn create_temporal_plain_date_in_realm(
    vm: &mut Vm,
    fields: TemporalPlainDateFields,
    calendar_identifier: Arc<str>,
    realm: GcIdx,
) -> error::Result<Value> {
    let prototype = vm
        .realm_temporal_plain_date_prototypes
        .get(&env::global_env_root(&vm.heap, realm).0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainDate prototype is not installed"))?;
    create_temporal_plain_date(vm, fields, calendar_identifier, prototype)
}

fn temporal_plain_date_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Temporal.PlainDate requires 'new'"));
    }
    let values = [
        temporal_integer_with_truncation(vm, args.first().cloned().unwrap_or(Value::Undefined))?,
        temporal_integer_with_truncation(vm, args.get(1).cloned().unwrap_or(Value::Undefined))?,
        temporal_integer_with_truncation(vm, args.get(2).cloned().unwrap_or(Value::Undefined))?,
    ];
    let calendar_identifier = temporal_constructor_calendar(
        vm,
        args.get(3).cloned().unwrap_or(Value::Undefined),
        "PlainDate",
    )?;
    let fields = temporal_plain_date_fields(values)
        .ok_or_else(|| Error::range("Invalid Temporal.PlainDate fields"))?;
    let realm = env::global_env_root(&vm.heap, vm.native_callee_closure().unwrap_or(vm.global));
    let fallback = vm
        .realm_temporal_plain_date_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainDate prototype is not installed"))?;
    let prototype = native_constructor_prototype_with_default(vm, "Temporal.PlainDate", fallback)?;
    create_temporal_plain_date(vm, fields, calendar_identifier, prototype)
}

macro_rules! temporal_plain_date_number_getter {
    ($name:ident, $field:ident) => {
        fn $name(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
            temporal_plain_date_slots(vm, this)
                .map(|(fields, _)| Value::Number(f64::from(fields.$field)))
        }
    };
}

temporal_plain_date_number_getter!(temporal_plain_date_year, year);
temporal_plain_date_number_getter!(temporal_plain_date_month, month);
temporal_plain_date_number_getter!(temporal_plain_date_day, day);

fn temporal_plain_date_calendar_id(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_date_slots(vm, this).map(|(_, calendar)| Value::String(calendar))
}

fn temporal_plain_date_era(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_date_slots(vm, this).map(|_| Value::Undefined)
}

fn temporal_plain_date_era_year(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_date_slots(vm, this).map(|_| Value::Undefined)
}

fn temporal_plain_date_month_code(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_date_slots(vm, this)
        .map(|(fields, _)| Value::String(Arc::from(format!("M{:02}", fields.month))))
}

fn temporal_plain_date_value_of(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Err(Error::type_err(
        "Temporal.PlainDate.prototype.valueOf always throws",
    ))
}

fn temporal_plain_date_iso(
    fields: TemporalPlainDateFields,
) -> error::Result<temporal::IsoDateTime> {
    temporal_plain_date_time_iso(TemporalPlainDateTimeFields {
        year: fields.year,
        month: fields.month,
        day: fields.day,
        hour: 12,
        minute: 0,
        second: 0,
        millisecond: 0,
        microsecond: 0,
        nanosecond: 0,
    })
    .map_err(|_| Error::internal("Invalid Temporal.PlainDate slots"))
}

fn temporal_plain_date_computed_field(
    vm: &mut Vm,
    this: Option<Value>,
    field: TemporalPlainDateTimeComputedField,
) -> error::Result<Value> {
    let (fields, _) = temporal_plain_date_slots(vm, this)?;
    temporal_iso_date_computed_field(temporal_plain_date_iso(fields)?, field)
}

macro_rules! temporal_plain_date_computed_getter {
    ($name:ident, $field:ident) => {
        fn $name(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
            temporal_plain_date_computed_field(vm, this, TemporalPlainDateTimeComputedField::$field)
        }
    };
}

temporal_plain_date_computed_getter!(temporal_plain_date_day_of_week, DayOfWeek);
temporal_plain_date_computed_getter!(temporal_plain_date_day_of_year, DayOfYear);
temporal_plain_date_computed_getter!(temporal_plain_date_week_of_year, WeekOfYear);
temporal_plain_date_computed_getter!(temporal_plain_date_year_of_week, YearOfWeek);
temporal_plain_date_computed_getter!(temporal_plain_date_days_in_week, DaysInWeek);
temporal_plain_date_computed_getter!(temporal_plain_date_days_in_month, DaysInMonth);
temporal_plain_date_computed_getter!(temporal_plain_date_days_in_year, DaysInYear);
temporal_plain_date_computed_getter!(temporal_plain_date_months_in_year, MonthsInYear);
temporal_plain_date_computed_getter!(temporal_plain_date_in_leap_year, InLeapYear);

fn temporal_instant_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Temporal.Instant requires 'new'"));
    }
    let epoch_nanoseconds = vm.coerce_bigint_shared(args.first().unwrap_or(&Value::Undefined))?;
    if epoch_nanoseconds.as_ref().abs() > temporal_instant_limit_nanoseconds() {
        return Err(Error::range(
            "Temporal.Instant epoch nanoseconds out of range",
        ));
    }
    let realm = env::global_env_root(&vm.heap, vm.native_callee_closure().unwrap_or(vm.global));
    let fallback = vm
        .realm_temporal_instant_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.Instant prototype is not installed"))?;
    let prototype = native_constructor_prototype_with_default(vm, "Temporal.Instant", fallback)?;
    create_temporal_instant(vm, epoch_nanoseconds, prototype)
}

fn temporal_zoned_date_time_slots(
    vm: &Vm,
    this: Option<Value>,
) -> error::Result<(Arc<BigInt>, TemporalTimeZone, Arc<str>)> {
    let Value::Object(index) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(
            "Temporal.ZonedDateTime method called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::ZonedDateTime {
                    epoch_nanoseconds,
                    time_zone,
                    calendar_identifier,
                },
            ..
        }) => Ok((
            epoch_nanoseconds.clone(),
            time_zone.clone(),
            calendar_identifier.clone(),
        )),
        _ => Err(Error::type_err(
            "Temporal.ZonedDateTime method called on incompatible receiver",
        )),
    })
}

fn temporal_zoned_date_time_slots_if_present(
    vm: &Vm,
    value: &Value,
) -> Option<(Arc<BigInt>, TemporalTimeZone, Arc<str>)> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::ZonedDateTime {
                    epoch_nanoseconds,
                    time_zone,
                    calendar_identifier,
                },
            ..
        }) => Some((
            epoch_nanoseconds.clone(),
            time_zone.clone(),
            calendar_identifier.clone(),
        )),
        _ => None,
    })
}

fn create_temporal_zoned_date_time(
    vm: &mut Vm,
    epoch_nanoseconds: Arc<BigInt>,
    time_zone: TemporalTimeZone,
    calendar_identifier: Arc<str>,
    prototype: Value,
) -> error::Result<Value> {
    vm.try_reserve_gc_pins(1)?;
    let pin_count = vm.pin(&prototype);
    let result = vm.alloc(HeapObj::Temporal(TemporalData {
        kind: TemporalKind::ZonedDateTime {
            epoch_nanoseconds,
            time_zone,
            calendar_identifier,
        },
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    result.map(Value::Object)
}

fn temporal_zoned_date_time_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Temporal.ZonedDateTime requires 'new'"));
    }
    let epoch_nanoseconds = vm.coerce_bigint_shared(args.first().unwrap_or(&Value::Undefined))?;
    if epoch_nanoseconds.as_ref().abs() > temporal_instant_limit_nanoseconds() {
        return Err(Error::range(
            "Temporal.ZonedDateTime epoch nanoseconds out of range",
        ));
    }

    let Value::String(time_zone_source) = args.get(1).unwrap_or(&Value::Undefined) else {
        return Err(Error::type_err(
            "Temporal.ZonedDateTime time zone must be a String",
        ));
    };
    vm.consume_fuel_units(time_zone_source.len().min(i64::MAX as usize) as i64)?;
    let (time_zone_identifier, offset_minutes) =
        temporal::parse_time_zone_identifier(time_zone_source)
            .ok_or_else(|| Error::range("Invalid Temporal time zone identifier"))?;
    let time_zone = temporal_time_zone_from_identifier(time_zone_identifier, offset_minutes);

    let calendar_identifier = match args.get(2).unwrap_or(&Value::Undefined) {
        Value::Undefined => Arc::from("iso8601"),
        Value::String(source) => {
            vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
            if !source.eq_ignore_ascii_case("iso8601") {
                return Err(Error::range("Invalid Temporal calendar identifier"));
            }
            Arc::from("iso8601")
        }
        _ => {
            return Err(Error::type_err(
                "Temporal.ZonedDateTime calendar must be a String",
            ))
        }
    };

    let realm = env::global_env_root(&vm.heap, vm.native_callee_closure().unwrap_or(vm.global));
    let fallback = vm
        .realm_temporal_zoned_date_time_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.ZonedDateTime prototype is not installed"))?;
    let prototype =
        native_constructor_prototype_with_default(vm, "Temporal.ZonedDateTime", fallback)?;
    create_temporal_zoned_date_time(
        vm,
        epoch_nanoseconds,
        time_zone,
        calendar_identifier,
        prototype,
    )
}

fn temporal_zoned_date_time_epoch_nanoseconds(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_zoned_date_time_slots(vm, this).map(|(epoch, _, _)| Value::BigInt(epoch))
}

fn temporal_zoned_date_time_epoch_milliseconds(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (epoch_nanoseconds, _, _) = temporal_zoned_date_time_slots(vm, this)?;
    let milliseconds = epoch_nanoseconds
        .as_ref()
        .div_floor(&BigInt::from(1_000_000_i64))
        .to_f64()
        .ok_or_else(|| Error::range("Temporal epoch milliseconds out of Number range"))?;
    Ok(Value::Number(milliseconds))
}

fn temporal_zoned_date_time_time_zone_id(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_zoned_date_time_slots(vm, this)
        .map(|(_, time_zone, _)| Value::String(time_zone.identifier))
}

fn temporal_zoned_date_time_calendar_id(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_zoned_date_time_slots(vm, this)
        .map(|(_, _, calendar_identifier)| Value::String(calendar_identifier))
}

fn temporal_time_zone_offset_nanoseconds(
    time_zone: &TemporalTimeZone,
    _epoch_nanoseconds: &BigInt,
) -> error::Result<i128> {
    match &time_zone.kind {
        TemporalTimeZoneKind::Utc => Ok(0),
        TemporalTimeZoneKind::FixedOffset(minutes) => Ok(i128::from(*minutes) * 60 * 1_000_000_000),
        TemporalTimeZoneKind::Named(identifier) => Err(Error::range(format!(
            "Named Temporal time zone is not available: {identifier}"
        ))),
    }
}

fn create_temporal_zoned_date_time_in_realm(
    vm: &mut Vm,
    epoch_nanoseconds: Arc<BigInt>,
    time_zone: TemporalTimeZone,
    calendar_identifier: Arc<str>,
    realm: GcIdx,
) -> error::Result<Value> {
    let prototype = vm
        .realm_temporal_zoned_date_time_prototypes
        .get(&env::global_env_root(&vm.heap, realm).0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.ZonedDateTime prototype is not installed"))?;
    create_temporal_zoned_date_time(
        vm,
        epoch_nanoseconds,
        time_zone,
        calendar_identifier,
        prototype,
    )
}

fn temporal_zoned_date_time_from(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let item = args.first().unwrap_or(&Value::Undefined);
    let (epoch_nanoseconds, time_zone, calendar_identifier) =
        to_temporal_zoned_date_time(vm, item, args.get(1))?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_zoned_date_time_in_realm(
        vm,
        epoch_nanoseconds,
        time_zone,
        calendar_identifier,
        realm,
    )
}

fn to_temporal_zoned_date_time(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(Arc<BigInt>, TemporalTimeZone, Arc<str>)> {
    let (epoch_nanoseconds, time_zone, calendar_identifier) = if let Some(slots) =
        temporal_zoned_date_time_slots_if_present(vm, item)
    {
        temporal_zoned_date_time_from_options(vm, options)?;
        slots
    } else if matches!(item, Value::Object(_)) {
        temporal_zoned_date_time_from_property_bag(vm, item, options)?
    } else {
        let Value::String(source) = item else {
            return Err(Error::type_err(
                "Temporal.ZonedDateTime input must be a String or object",
            ));
        };
        vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
        let parsed = temporal::parse_zoned_date_time_string(source)
            .ok_or_else(|| Error::range("Invalid Temporal.ZonedDateTime string"))?;
        let options = temporal_zoned_date_time_from_options(vm, options)?;
        let epoch_nanoseconds = temporal::resolve_zoned_date_time_epoch(&parsed, options.offset)
            .ok_or_else(|| Error::range("Temporal.ZonedDateTime offset does not match"))?;
        if epoch_nanoseconds.abs() > temporal_instant_limit_nanoseconds() {
            return Err(Error::range(
                "Temporal.ZonedDateTime epoch nanoseconds out of range",
            ));
        }
        (
            Arc::new(epoch_nanoseconds),
            temporal_time_zone_from_identifier(parsed.time_zone_identifier, parsed.offset_minutes),
            parsed.calendar_identifier,
        )
    };
    Ok((epoch_nanoseconds, time_zone, calendar_identifier))
}

fn temporal_zoned_date_time_equals(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (epoch_nanoseconds, time_zone, calendar_identifier) =
        temporal_zoned_date_time_slots(vm, this)?;
    let (other_epoch, other_time_zone, other_calendar) =
        to_temporal_zoned_date_time(vm, args.first().unwrap_or(&Value::Undefined), None)?;
    Ok(Value::Bool(
        epoch_nanoseconds == other_epoch
            && temporal_time_zone_equals(&time_zone, &other_time_zone)
            && temporal_calendar_equals(&calendar_identifier, &other_calendar),
    ))
}

fn temporal_zoned_date_time_compare(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let (one, _, _) =
        to_temporal_zoned_date_time(vm, args.first().unwrap_or(&Value::Undefined), None)?;
    let (two, _, _) =
        to_temporal_zoned_date_time(vm, args.get(1).unwrap_or(&Value::Undefined), None)?;
    let result = match one.cmp(&two) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    };
    Ok(Value::Number(result))
}

fn temporal_zoned_date_time_with_time_zone(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (epoch_nanoseconds, _, calendar_identifier) = temporal_zoned_date_time_slots(vm, this)?;
    let time_zone =
        temporal_time_zone_from_value(vm, args.first().cloned().unwrap_or(Value::Undefined))?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_zoned_date_time_in_realm(
        vm,
        epoch_nanoseconds,
        time_zone,
        calendar_identifier,
        realm,
    )
}

fn temporal_zoned_date_time_with_calendar(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (epoch_nanoseconds, time_zone, _) = temporal_zoned_date_time_slots(vm, this)?;
    let calendar_identifier = temporal_calendar_identifier_from_value(
        vm,
        args.first().cloned().unwrap_or(Value::Undefined),
    )?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_zoned_date_time_in_realm(
        vm,
        epoch_nanoseconds,
        time_zone,
        calendar_identifier,
        realm,
    )
}

fn temporal_zoned_date_time_start_of_day(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (epoch_nanoseconds, time_zone, calendar_identifier) =
        temporal_zoned_date_time_slots(vm, this)?;
    let offset_nanoseconds = temporal_time_zone_offset_nanoseconds(&time_zone, &epoch_nanoseconds)?;
    let start_epoch =
        temporal::fixed_offset_start_of_day_epoch(epoch_nanoseconds.as_ref(), offset_nanoseconds)
            .ok_or_else(|| Error::range("Temporal.ZonedDateTime start of day is out of range"))?;
    if start_epoch.abs() > temporal_instant_limit_nanoseconds() {
        return Err(Error::range(
            "Temporal.ZonedDateTime start of day is out of range",
        ));
    }

    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_zoned_date_time_in_realm(
        vm,
        Arc::new(start_epoch),
        time_zone,
        calendar_identifier,
        realm,
    )
}

fn temporal_time_zone_equals(one: &TemporalTimeZone, two: &TemporalTimeZone) -> bool {
    one.identifier == two.identifier
}

fn temporal_calendar_equals(one: &str, two: &str) -> bool {
    one == two
}

#[derive(Clone, Copy)]
enum TemporalOverflow {
    Constrain,
    Reject,
}

struct TemporalZonedDateTimeFromOptions {
    offset: temporal::ZonedDateTimeOffsetOption,
    overflow: TemporalOverflow,
}

struct TemporalZonedDateTimePropertyFields {
    year: Option<BigInt>,
    month: Option<BigInt>,
    month_code: Option<(u8, bool)>,
    day: Option<BigInt>,
    hour: BigInt,
    minute: BigInt,
    second: BigInt,
    millisecond: BigInt,
    microsecond: BigInt,
    nanosecond: BigInt,
    offset_nanoseconds: Option<i128>,
    time_zone: Option<TemporalTimeZone>,
    calendar_identifier: Arc<str>,
}

fn temporal_with_rooted_value<T>(
    vm: &mut Vm,
    value: Value,
    operation: impl FnOnce(&mut Vm, &Value) -> error::Result<T>,
) -> error::Result<T> {
    vm.try_reserve_value_roots(std::slice::from_ref(&value))?;
    let pin_count = vm.pin(&value);
    let result = operation(vm, &value);
    vm.unpin_many(pin_count);
    result
}

fn temporal_integer_with_truncation(vm: &mut Vm, value: Value) -> error::Result<BigInt> {
    temporal_with_rooted_value(vm, value, |vm, value| {
        let primitive = if matches!(value, Value::Object(_)) {
            vm.to_primitive_number(value)?
        } else {
            value.clone()
        };
        if let Value::String(source) = &primitive {
            vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
        }
        let number = vm.to_number(&primitive)?;
        if !number.is_finite() {
            return Err(Error::range("Temporal field must be a finite number"));
        }
        BigInt::from_f64(number.trunc())
            .ok_or_else(|| Error::range("Temporal field is out of range"))
    })
}

fn temporal_integer_if_integral(vm: &mut Vm, value: Value) -> error::Result<f64> {
    temporal_with_rooted_value(vm, value, |vm, value| {
        let primitive = if matches!(value, Value::Object(_)) {
            vm.to_primitive_number(value)?
        } else {
            value.clone()
        };
        if let Value::String(source) = &primitive {
            vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
        }
        let number = vm.to_number(&primitive)?;
        if !number.is_finite() || number.fract() != 0.0 {
            return Err(Error::range("Temporal.Duration field must be an integer"));
        }
        Ok(if number == 0.0 { 0.0 } else { number })
    })
}

fn temporal_positive_integer_with_truncation(vm: &mut Vm, value: Value) -> error::Result<BigInt> {
    let integer = temporal_integer_with_truncation(vm, value)?;
    if integer <= BigInt::zero() {
        return Err(Error::range("Temporal field must be positive"));
    }
    Ok(integer)
}

fn temporal_string_primitive(vm: &mut Vm, value: Value, field: &str) -> error::Result<Arc<str>> {
    temporal_with_rooted_value(vm, value, |vm, value| {
        let primitive = vm.to_primitive_hint(value, true)?;
        let Value::String(source) = primitive else {
            return Err(Error::type_err(format!(
                "Temporal {field} must convert to a String"
            )));
        };
        vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
        Ok(source)
    })
}

fn temporal_month_code(vm: &mut Vm, value: Value) -> error::Result<(u8, bool)> {
    let source = temporal_string_primitive(vm, value, "monthCode")?;
    let bytes = source.as_bytes();
    let leap = bytes.len() == 4 && bytes[3] == b'L';
    if !((bytes.len() == 3 || leap)
        && bytes[0] == b'M'
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit())
    {
        return Err(Error::range("Invalid Temporal monthCode"));
    }
    let month = (bytes[1] - b'0') * 10 + bytes[2] - b'0';
    if month == 0 && !leap {
        return Err(Error::range("Invalid Temporal monthCode"));
    }
    Ok((month, leap))
}

fn temporal_calendar_from_value(vm: &mut Vm, value: Value) -> error::Result<Arc<str>> {
    if value.is_undefined() {
        return Ok(Arc::from("iso8601"));
    }
    temporal_calendar_identifier_from_value(vm, value)
}

fn temporal_calendar_slot_if_present(vm: &Vm, value: &Value) -> Option<Arc<str>> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::PlainDate {
                    calendar_identifier,
                    ..
                }
                | TemporalKind::PlainMonthDay {
                    calendar_identifier,
                    ..
                }
                | TemporalKind::PlainDateTime {
                    calendar_identifier,
                    ..
                }
                | TemporalKind::PlainYearMonth {
                    calendar_identifier,
                    ..
                }
                | TemporalKind::ZonedDateTime {
                    calendar_identifier,
                    ..
                },
            ..
        }) => Some(calendar_identifier.clone()),
        _ => None,
    })
}

fn temporal_calendar_identifier_from_value(vm: &mut Vm, value: Value) -> error::Result<Arc<str>> {
    if let Some(calendar) = temporal_calendar_slot_if_present(vm, &value) {
        return Ok(calendar);
    }
    let Value::String(source) = value else {
        return Err(Error::type_err(
            "Temporal calendar must be a String or Temporal object",
        ));
    };
    vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
    temporal::parse_calendar_identifier(&source)
        .ok_or_else(|| Error::range("Invalid Temporal calendar identifier"))
}

struct TemporalPlainDatePropertyFields {
    year: Option<BigInt>,
    month: Option<BigInt>,
    month_code: Option<(u8, bool)>,
    day: Option<BigInt>,
    calendar_identifier: Arc<str>,
}

fn temporal_plain_date_property_fields(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainDatePropertyFields> {
    vm.try_reserve_value_roots(std::slice::from_ref(item))?;
    let item_pins = vm.pin(item);
    let result = (|| {
        let calendar_identifier =
            if let Some(calendar) = temporal_calendar_slot_if_present(vm, item) {
                calendar
            } else {
                let calendar = vm.get_property(item, "calendar")?;
                temporal_calendar_from_value(vm, calendar)?
            };
        let numeric = |vm: &mut Vm, name: &str| -> error::Result<Option<BigInt>> {
            match vm.get_property(item, name)? {
                Value::Undefined => Ok(None),
                value => temporal_integer_with_truncation(vm, value).map(Some),
            }
        };
        let day = numeric(vm, "day")?;
        let month = numeric(vm, "month")?;
        let month_code = match vm.get_property(item, "monthCode")? {
            Value::Undefined => None,
            value => Some(temporal_month_code(vm, value)?),
        };
        let year = numeric(vm, "year")?;
        Ok(TemporalPlainDatePropertyFields {
            year,
            month,
            month_code,
            day,
            calendar_identifier,
        })
    })();
    vm.unpin_many(item_pins);
    result
}

fn temporal_plain_date_from_property_bag(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(TemporalPlainDateFields, Arc<str>)> {
    let fields = temporal_plain_date_property_fields(vm, item)?;
    let overflow = temporal_from_overflow(vm, options)?;

    let year = fields
        .year
        .ok_or_else(|| Error::type_err("Temporal property bag requires year"))?;
    if fields.month.is_none() && fields.month_code.is_none() {
        return Err(Error::type_err(
            "Temporal property bag requires month or monthCode",
        ));
    }
    let day = fields
        .day
        .ok_or_else(|| Error::type_err("Temporal property bag requires day"))?;

    if let Some((month_code, leap)) = fields.month_code {
        if leap || !(1..=12).contains(&month_code) {
            return Err(Error::range("Invalid monthCode for ISO 8601 calendar"));
        }
        if fields
            .month
            .as_ref()
            .is_some_and(|month| month != &BigInt::from(month_code))
        {
            return Err(Error::range("month and monthCode do not agree"));
        }
    }

    let mut month = fields
        .month
        .unwrap_or_else(|| BigInt::from(fields.month_code.unwrap().0));
    if month <= BigInt::zero() {
        return Err(Error::range("Temporal month is out of range"));
    }
    if month > BigInt::from(12) {
        match overflow {
            TemporalOverflow::Constrain if fields.month_code.is_none() => month = BigInt::from(12),
            TemporalOverflow::Constrain | TemporalOverflow::Reject => {
                return Err(Error::range("Temporal month is out of range"));
            }
        }
    }
    let year = year
        .to_i128()
        .ok_or_else(|| Error::range("Temporal year is out of range"))?;
    let month = month
        .to_i128()
        .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    if day <= BigInt::zero() {
        return Err(Error::range("Temporal day is out of range"));
    }
    let maximum_day = temporal::days_in_month(year, month)
        .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    let day = if day > BigInt::from(maximum_day) {
        match overflow {
            TemporalOverflow::Constrain => maximum_day,
            TemporalOverflow::Reject => {
                return Err(Error::range("Temporal day is out of range"));
            }
        }
    } else {
        day.to_i128()
            .ok_or_else(|| Error::range("Temporal day is out of range"))?
    };

    let resolved =
        temporal_plain_date_fields([BigInt::from(year), BigInt::from(month), BigInt::from(day)])
            .ok_or_else(|| Error::range("Temporal.PlainDate fields are out of range"))?;
    Ok((resolved, fields.calendar_identifier))
}

struct TemporalPlainDateTimePropertyFields {
    year: Option<BigInt>,
    month: Option<BigInt>,
    month_code: Option<(u8, bool)>,
    day: Option<BigInt>,
    hour: BigInt,
    minute: BigInt,
    second: BigInt,
    millisecond: BigInt,
    microsecond: BigInt,
    nanosecond: BigInt,
    calendar_identifier: Arc<str>,
}

fn temporal_plain_date_time_property_fields(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainDateTimePropertyFields> {
    vm.try_reserve_value_roots(std::slice::from_ref(item))?;
    let item_pins = vm.pin(item);
    let result = (|| {
        let calendar_identifier =
            if let Some(calendar) = temporal_calendar_slot_if_present(vm, item) {
                calendar
            } else {
                let calendar = vm.get_property(item, "calendar")?;
                temporal_calendar_from_value(vm, calendar)?
            };
        let numeric = |vm: &mut Vm, name: &str| -> error::Result<Option<BigInt>> {
            match vm.get_property(item, name)? {
                Value::Undefined => Ok(None),
                value => temporal_integer_with_truncation(vm, value).map(Some),
            }
        };
        let numeric_or_zero = |vm: &mut Vm, name: &str| -> error::Result<BigInt> {
            numeric(vm, name).map(|value| value.unwrap_or_else(BigInt::zero))
        };
        let day = numeric(vm, "day")?;
        let hour = numeric_or_zero(vm, "hour")?;
        let microsecond = numeric_or_zero(vm, "microsecond")?;
        let millisecond = numeric_or_zero(vm, "millisecond")?;
        let minute = numeric_or_zero(vm, "minute")?;
        let month = numeric(vm, "month")?;
        let month_code = match vm.get_property(item, "monthCode")? {
            Value::Undefined => None,
            value => Some(temporal_month_code(vm, value)?),
        };
        let nanosecond = numeric_or_zero(vm, "nanosecond")?;
        let second = numeric_or_zero(vm, "second")?;
        let year = numeric(vm, "year")?;
        Ok(TemporalPlainDateTimePropertyFields {
            year,
            month,
            month_code,
            day,
            hour,
            minute,
            second,
            millisecond,
            microsecond,
            nanosecond,
            calendar_identifier,
        })
    })();
    vm.unpin_many(item_pins);
    result
}

fn temporal_from_overflow(vm: &mut Vm, options: Option<&Value>) -> error::Result<TemporalOverflow> {
    let options = options.cloned().unwrap_or(Value::Undefined);
    if !matches!(options, Value::Undefined | Value::Object(_)) {
        return Err(Error::type_err("Temporal options must be an object"));
    }
    vm.try_reserve_value_roots(std::slice::from_ref(&options))?;
    let pin_count = vm.pin(&options);
    let result = (|| {
        let overflow = if options.is_undefined() {
            None
        } else {
            match vm.get_property(&options, "overflow")? {
                Value::Undefined => None,
                value => Some(temporal_option_to_string(vm, &value)?),
            }
        };
        match overflow.as_deref().unwrap_or("constrain") {
            "constrain" => Ok(TemporalOverflow::Constrain),
            "reject" => Ok(TemporalOverflow::Reject),
            _ => Err(Error::range("Invalid Temporal overflow option")),
        }
    })();
    vm.unpin_many(pin_count);
    result
}

fn temporal_plain_date_time_from_property_bag(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(TemporalPlainDateTimeFields, Arc<str>)> {
    let fields = temporal_plain_date_time_property_fields(vm, item)?;
    let overflow = temporal_from_overflow(vm, options)?;

    let year = fields
        .year
        .ok_or_else(|| Error::type_err("Temporal property bag requires year"))?;
    if fields.month.is_none() && fields.month_code.is_none() {
        return Err(Error::type_err(
            "Temporal property bag requires month or monthCode",
        ));
    }
    let day = fields
        .day
        .ok_or_else(|| Error::type_err("Temporal property bag requires day"))?;

    if let Some((month_code, leap)) = fields.month_code {
        if leap || !(1..=12).contains(&month_code) {
            return Err(Error::range("Invalid monthCode for ISO 8601 calendar"));
        }
        if fields
            .month
            .as_ref()
            .is_some_and(|month| month != &BigInt::from(month_code))
        {
            return Err(Error::range("month and monthCode do not agree"));
        }
    }

    let mut month = fields
        .month
        .unwrap_or_else(|| BigInt::from(fields.month_code.unwrap().0));
    if month <= BigInt::zero() {
        return Err(Error::range("Temporal month is out of range"));
    }
    if month > BigInt::from(12) {
        match overflow {
            TemporalOverflow::Constrain if fields.month_code.is_none() => month = BigInt::from(12),
            TemporalOverflow::Constrain | TemporalOverflow::Reject => {
                return Err(Error::range("Temporal month is out of range"));
            }
        }
    }
    let year = year
        .to_i128()
        .ok_or_else(|| Error::range("Temporal year is out of range"))?;
    let month = month
        .to_i128()
        .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    if day <= BigInt::zero() {
        return Err(Error::range("Temporal day is out of range"));
    }
    let maximum_day = temporal::days_in_month(year, month)
        .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    let day = if day > BigInt::from(maximum_day) {
        match overflow {
            TemporalOverflow::Constrain => maximum_day,
            TemporalOverflow::Reject => {
                return Err(Error::range("Temporal day is out of range"));
            }
        }
    } else {
        day.to_i128()
            .ok_or_else(|| Error::range("Temporal day is out of range"))?
    };

    let iso_fields = temporal::IsoDateTimeFields {
        year,
        month,
        day,
        hour: temporal_regulate_field(fields.hour, 23, overflow)?,
        minute: temporal_regulate_field(fields.minute, 59, overflow)?,
        second: temporal_regulate_field(fields.second, 59, overflow)?,
        millisecond: temporal_regulate_field(fields.millisecond, 999, overflow)?,
        microsecond: temporal_regulate_field(fields.microsecond, 999, overflow)?,
        nanosecond: temporal_regulate_field(fields.nanosecond, 999, overflow)?,
    };
    Ok((
        temporal_plain_date_time_fields_from_iso(iso_fields)?,
        fields.calendar_identifier,
    ))
}

fn temporal_zoned_date_time_plain_fields(
    epoch_nanoseconds: &BigInt,
    time_zone: &TemporalTimeZone,
) -> error::Result<TemporalPlainDateTimeFields> {
    let offset_nanoseconds = temporal_time_zone_offset_nanoseconds(time_zone, epoch_nanoseconds)?;
    let date_time = temporal::iso_date_time(epoch_nanoseconds, offset_nanoseconds)
        .ok_or_else(|| Error::range("Temporal.ZonedDateTime local date is out of range"))?;
    temporal_plain_date_time_fields_from_iso(temporal::IsoDateTimeFields {
        year: date_time.year,
        month: date_time.month,
        day: date_time.day,
        hour: date_time.hour,
        minute: date_time.minute,
        second: date_time.second,
        millisecond: date_time.millisecond,
        microsecond: date_time.microsecond,
        nanosecond: date_time.nanosecond,
    })
    .map_err(|_| Error::range("Temporal.ZonedDateTime local date is out of range"))
}

fn temporal_zoned_date_time_plain_date_fields(
    epoch_nanoseconds: &BigInt,
    time_zone: &TemporalTimeZone,
) -> error::Result<TemporalPlainDateFields> {
    let offset_nanoseconds = temporal_time_zone_offset_nanoseconds(time_zone, epoch_nanoseconds)?;
    let date_time = temporal::iso_date_time(epoch_nanoseconds, offset_nanoseconds)
        .ok_or_else(|| Error::range("Temporal.ZonedDateTime local date is out of range"))?;
    temporal_plain_date_fields([
        BigInt::from(date_time.year),
        BigInt::from(date_time.month),
        BigInt::from(date_time.day),
    ])
    .ok_or_else(|| Error::range("Temporal.ZonedDateTime local date is out of range"))
}

fn to_temporal_plain_date(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(TemporalPlainDateFields, Arc<str>)> {
    if let Some(slots) = temporal_plain_date_slots_if_present(vm, item) {
        temporal_from_overflow(vm, options)?;
        Ok(slots)
    } else if let Some((epoch_nanoseconds, time_zone, calendar_identifier)) =
        temporal_zoned_date_time_slots_if_present(vm, item)
    {
        let fields = temporal_zoned_date_time_plain_date_fields(&epoch_nanoseconds, &time_zone)?;
        temporal_from_overflow(vm, options)?;
        Ok((fields, calendar_identifier))
    } else if let Some((date_time, calendar_identifier)) =
        temporal_plain_date_time_slots_if_present(vm, item)
    {
        temporal_from_overflow(vm, options)?;
        Ok((
            TemporalPlainDateFields {
                year: date_time.year,
                month: date_time.month,
                day: date_time.day,
            },
            calendar_identifier,
        ))
    } else if matches!(item, Value::Object(_)) {
        temporal_plain_date_from_property_bag(vm, item, options)
    } else {
        let Value::String(source) = item else {
            return Err(Error::type_err(
                "Temporal.PlainDate input must be a String or object",
            ));
        };
        vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
        let parsed = temporal::parse_plain_date_string(source)
            .ok_or_else(|| Error::range("Invalid Temporal.PlainDate string"))?;
        temporal_from_overflow(vm, options)?;
        let fields = temporal_plain_date_fields([
            BigInt::from(parsed.year),
            BigInt::from(parsed.month),
            BigInt::from(parsed.day),
        ])
        .ok_or_else(|| Error::range("Temporal.PlainDate fields are out of range"))?;
        Ok((fields, parsed.calendar_identifier))
    }
}

fn temporal_plain_date_from(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) =
        to_temporal_plain_date(vm, args.first().unwrap_or(&Value::Undefined), args.get(1))?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_date_in_realm(vm, fields, calendar_identifier, realm)
}

fn temporal_plain_date_compare(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let (one, _) = to_temporal_plain_date(vm, args.first().unwrap_or(&Value::Undefined), None)?;
    let (two, _) = to_temporal_plain_date(vm, args.get(1).unwrap_or(&Value::Undefined), None)?;
    let result = match (one.year, one.month, one.day).cmp(&(two.year, two.month, two.day)) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    };
    Ok(Value::Number(result))
}

fn temporal_plain_date_equals(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) = temporal_plain_date_slots(vm, this)?;
    let (other_fields, other_calendar) =
        to_temporal_plain_date(vm, args.first().unwrap_or(&Value::Undefined), None)?;
    Ok(Value::Bool(
        fields == other_fields && temporal_calendar_equals(&calendar_identifier, &other_calendar),
    ))
}

fn temporal_calendar_name_to_string_option(
    vm: &mut Vm,
    args: &[Value],
    receiver_name: &str,
) -> error::Result<temporal::AnnotationDisplay> {
    let options = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(options, Value::Undefined | Value::Object(_)) {
        return Err(Error::type_err(format!(
            "Temporal.{receiver_name}.prototype.toString options must be an object"
        )));
    }
    vm.try_reserve_value_roots(std::slice::from_ref(&options))?;
    let options_pin = vm.pin(&options);
    let result = (|| {
        let calendar_name = if options.is_undefined() {
            None
        } else {
            match vm.get_property(&options, "calendarName")? {
                Value::Undefined => None,
                value => Some(temporal_option_to_string(vm, &value)?),
            }
        };
        temporal_annotation_display(calendar_name.as_deref(), "calendarName", true)
    })();
    vm.unpin_many(options_pin);
    result
}

fn temporal_plain_date_to_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) = temporal_plain_date_slots(vm, this)?;
    let calendar_name = temporal_calendar_name_to_string_option(vm, args, "PlainDate")?;
    temporal_plain_date_format(fields, &calendar_identifier, calendar_name)
}

fn temporal_plain_date_to_json(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) = temporal_plain_date_slots(vm, this)?;
    temporal_plain_date_format(
        fields,
        &calendar_identifier,
        temporal::AnnotationDisplay::Auto,
    )
}

fn temporal_plain_month_day_slots(
    vm: &Vm,
    this: Option<Value>,
) -> error::Result<(TemporalPlainMonthDayFields, Arc<str>)> {
    let Value::Object(index) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(
            "Temporal.PlainMonthDay method called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::PlainMonthDay {
                    fields,
                    calendar_identifier,
                },
            ..
        }) => Ok((*fields, calendar_identifier.clone())),
        _ => Err(Error::type_err(
            "Temporal.PlainMonthDay method called on incompatible receiver",
        )),
    })
}

fn temporal_plain_month_day_slots_if_present(
    vm: &Vm,
    value: &Value,
) -> Option<(TemporalPlainMonthDayFields, Arc<str>)> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::PlainMonthDay {
                    fields,
                    calendar_identifier,
                },
            ..
        }) => Some((*fields, calendar_identifier.clone())),
        _ => None,
    })
}

fn temporal_is_date_or_time_object(vm: &Vm, value: &Value) -> bool {
    let Value::Object(index) = value else {
        return false;
    };
    vm.heap.with_obj(index.0, |object| {
        matches!(
            object,
            HeapObj::Temporal(TemporalData {
                kind: TemporalKind::PlainDate { .. }
                    | TemporalKind::PlainMonthDay { .. }
                    | TemporalKind::PlainTime { .. }
                    | TemporalKind::PlainDateTime { .. }
                    | TemporalKind::PlainYearMonth { .. }
                    | TemporalKind::ZonedDateTime { .. },
                ..
            })
        )
    })
}

fn temporal_reject_partial_calendar_or_time_zone(
    vm: &mut Vm,
    item: &Value,
    receiver_name: &str,
) -> error::Result<()> {
    if temporal_is_date_or_time_object(vm, item) {
        return Err(Error::type_err(format!(
            "Temporal.{receiver_name}.prototype.with requires a partial Temporal object"
        )));
    }
    if !vm.get_property(item, "calendar")?.is_undefined() {
        return Err(Error::type_err(format!(
            "Temporal.{receiver_name}.prototype.with rejects calendar"
        )));
    }
    if !vm.get_property(item, "timeZone")?.is_undefined() {
        return Err(Error::type_err(format!(
            "Temporal.{receiver_name}.prototype.with rejects timeZone"
        )));
    }
    Ok(())
}

fn create_temporal_plain_month_day(
    vm: &mut Vm,
    fields: TemporalPlainMonthDayFields,
    calendar_identifier: Arc<str>,
    prototype: Value,
) -> error::Result<Value> {
    let date = temporal_plain_date_fields([
        BigInt::from(fields.reference_iso_year),
        BigInt::from(fields.month),
        BigInt::from(fields.day),
    ]);
    if date.is_none() {
        return Err(Error::range("Invalid Temporal.PlainMonthDay fields"));
    }
    vm.try_reserve_gc_pins(1)?;
    let pin_count = vm.pin(&prototype);
    let result = vm.alloc(HeapObj::Temporal(TemporalData {
        kind: TemporalKind::PlainMonthDay {
            fields,
            calendar_identifier,
        },
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    result.map(Value::Object)
}

fn create_temporal_plain_month_day_in_realm(
    vm: &mut Vm,
    fields: TemporalPlainMonthDayFields,
    calendar_identifier: Arc<str>,
    realm: GcIdx,
) -> error::Result<Value> {
    let prototype = vm
        .realm_temporal_plain_month_day_prototypes
        .get(&env::global_env_root(&vm.heap, realm).0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainMonthDay prototype is not installed"))?;
    create_temporal_plain_month_day(vm, fields, calendar_identifier, prototype)
}

fn temporal_plain_month_day_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Temporal.PlainMonthDay requires 'new'"));
    }
    let month =
        temporal_integer_with_truncation(vm, args.first().cloned().unwrap_or(Value::Undefined))?;
    let day =
        temporal_integer_with_truncation(vm, args.get(1).cloned().unwrap_or(Value::Undefined))?;
    let calendar_identifier = temporal_constructor_calendar(
        vm,
        args.get(2).cloned().unwrap_or(Value::Undefined),
        "PlainMonthDay",
    )?;
    let reference_iso_year = match args.get(3) {
        None | Some(Value::Undefined) => BigInt::from(1972),
        Some(value) => temporal_integer_with_truncation(vm, value.clone())?,
    };
    let date = temporal_plain_date_fields([reference_iso_year, month, day])
        .ok_or_else(|| Error::range("Invalid Temporal.PlainMonthDay fields"))?;
    let fields = TemporalPlainMonthDayFields {
        reference_iso_year: date.year,
        month: date.month,
        day: date.day,
    };
    let realm = env::global_env_root(&vm.heap, vm.native_callee_closure().unwrap_or(vm.global));
    let fallback = vm
        .realm_temporal_plain_month_day_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainMonthDay prototype is not installed"))?;
    let prototype =
        native_constructor_prototype_with_default(vm, "Temporal.PlainMonthDay", fallback)?;
    create_temporal_plain_month_day(vm, fields, calendar_identifier, prototype)
}

struct TemporalPlainMonthDayPropertyFields {
    year: Option<BigInt>,
    month: Option<BigInt>,
    month_code: Option<(u8, bool)>,
    day: Option<BigInt>,
    calendar_identifier: Arc<str>,
}

fn temporal_plain_month_day_property_fields(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainMonthDayPropertyFields> {
    vm.try_reserve_value_roots(std::slice::from_ref(item))?;
    let item_pins = vm.pin(item);
    let result = (|| {
        let calendar_identifier =
            if let Some(calendar) = temporal_calendar_slot_if_present(vm, item) {
                calendar
            } else {
                let calendar = vm.get_property(item, "calendar")?;
                temporal_calendar_from_value(vm, calendar)?
            };
        let positive = |vm: &mut Vm, name: &str| -> error::Result<Option<BigInt>> {
            match vm.get_property(item, name)? {
                Value::Undefined => Ok(None),
                value => temporal_positive_integer_with_truncation(vm, value).map(Some),
            }
        };
        let day = positive(vm, "day")?;
        let month = positive(vm, "month")?;
        let month_code = match vm.get_property(item, "monthCode")? {
            Value::Undefined => None,
            value => Some(temporal_month_code(vm, value)?),
        };
        let year = match vm.get_property(item, "year")? {
            Value::Undefined => None,
            value => Some(temporal_integer_with_truncation(vm, value)?),
        };
        Ok(TemporalPlainMonthDayPropertyFields {
            year,
            month,
            month_code,
            day,
            calendar_identifier,
        })
    })();
    vm.unpin_many(item_pins);
    result
}

fn temporal_bigint_leap_year(year: &BigInt) -> bool {
    (year % 4_u8).is_zero() && (!(year % 100_u8).is_zero() || (year % 400_u16).is_zero())
}

fn temporal_bigint_days_in_month(year: &BigInt, month: i128) -> Option<i128> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if temporal_bigint_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    })
}

fn temporal_plain_month_day_from_property_bag(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(TemporalPlainMonthDayFields, Arc<str>)> {
    let fields = temporal_plain_month_day_property_fields(vm, item)?;
    let overflow = temporal_from_overflow(vm, options)?;

    if fields.month.is_none() && fields.month_code.is_none() {
        return Err(Error::type_err(
            "Temporal property bag requires month or monthCode",
        ));
    }
    let day = fields
        .day
        .ok_or_else(|| Error::type_err("Temporal property bag requires day"))?;

    if let Some((month_code, leap)) = fields.month_code {
        if leap || !(1..=12).contains(&month_code) {
            return Err(Error::range("Invalid monthCode for ISO 8601 calendar"));
        }
        if fields
            .month
            .as_ref()
            .is_some_and(|month| month != &BigInt::from(month_code))
        {
            return Err(Error::range("month and monthCode do not agree"));
        }
    }

    let mut month = fields
        .month
        .unwrap_or_else(|| BigInt::from(fields.month_code.unwrap().0));
    if month <= BigInt::zero() {
        return Err(Error::range("Temporal month is out of range"));
    }
    if month > BigInt::from(12) {
        match overflow {
            TemporalOverflow::Constrain if fields.month_code.is_none() => month = BigInt::from(12),
            TemporalOverflow::Constrain | TemporalOverflow::Reject => {
                return Err(Error::range("Temporal month is out of range"));
            }
        }
    }
    let overflow_year = fields.year.unwrap_or_else(|| BigInt::from(1972));
    let month = month
        .to_i128()
        .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    if day <= BigInt::zero() {
        return Err(Error::range("Temporal day is out of range"));
    }
    let maximum_day = temporal_bigint_days_in_month(&overflow_year, month)
        .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    let day = if day > BigInt::from(maximum_day) {
        match overflow {
            TemporalOverflow::Constrain => maximum_day,
            TemporalOverflow::Reject => {
                return Err(Error::range("Temporal day is out of range"));
            }
        }
    } else {
        day.to_i128()
            .ok_or_else(|| Error::range("Temporal day is out of range"))?
    };

    let resolved =
        temporal_plain_date_fields([BigInt::from(1972), BigInt::from(month), BigInt::from(day)])
            .ok_or_else(|| Error::range("Temporal.PlainMonthDay fields are out of range"))?;
    Ok((
        TemporalPlainMonthDayFields {
            reference_iso_year: resolved.year,
            month: resolved.month,
            day: resolved.day,
        },
        fields.calendar_identifier,
    ))
}

struct TemporalPlainMonthDayPartialFields {
    day: Option<BigInt>,
    month: Option<BigInt>,
    month_code: Option<(u8, bool)>,
    year: Option<BigInt>,
}

fn temporal_plain_month_day_partial_fields(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainMonthDayPartialFields> {
    let positive = |vm: &mut Vm, name: &str| -> error::Result<Option<BigInt>> {
        match vm.get_property(item, name)? {
            Value::Undefined => Ok(None),
            value => temporal_positive_integer_with_truncation(vm, value).map(Some),
        }
    };
    let day = positive(vm, "day")?;
    let month = positive(vm, "month")?;
    let month_code = match vm.get_property(item, "monthCode")? {
        Value::Undefined => None,
        value => Some(temporal_month_code(vm, value)?),
    };
    let year = match vm.get_property(item, "year")? {
        Value::Undefined => None,
        value => Some(temporal_integer_with_truncation(vm, value)?),
    };
    if day.is_none() && month.is_none() && month_code.is_none() && year.is_none() {
        return Err(Error::type_err(
            "Temporal.PlainMonthDay.prototype.with requires at least one field",
        ));
    }
    Ok(TemporalPlainMonthDayPartialFields {
        day,
        month,
        month_code,
        year,
    })
}

fn temporal_plain_month_day_with(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (receiver, calendar_identifier) = temporal_plain_month_day_slots(vm, this)?;
    let item = args.first().cloned().unwrap_or(Value::Undefined);
    let options = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !matches!(item, Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.PlainMonthDay.prototype.with requires an object",
        ));
    }

    vm.try_reserve_value_roots(&[item.clone(), options.clone()])?;
    let pins = vm.pin_many(&[item.clone(), options.clone()]);
    let result = (|| {
        temporal_reject_partial_calendar_or_time_zone(vm, &item, "PlainMonthDay")?;
        let partial = temporal_plain_month_day_partial_fields(vm, &item)?;
        let overflow = temporal_from_overflow(vm, Some(&options))?;

        if let Some((month_code, leap)) = partial.month_code {
            if leap || !(1..=12).contains(&month_code) {
                return Err(Error::range("Invalid monthCode for ISO 8601 calendar"));
            }
            if partial
                .month
                .as_ref()
                .is_some_and(|month| month != &BigInt::from(month_code))
            {
                return Err(Error::range("month and monthCode do not agree"));
            }
        }

        let mut month = partial.month.unwrap_or_else(|| {
            partial
                .month_code
                .map_or_else(|| BigInt::from(receiver.month), |code| BigInt::from(code.0))
        });
        if month > BigInt::from(12) {
            match overflow {
                TemporalOverflow::Constrain if partial.month_code.is_none() => {
                    month = BigInt::from(12)
                }
                TemporalOverflow::Constrain | TemporalOverflow::Reject => {
                    return Err(Error::range("Temporal month is out of range"));
                }
            }
        }
        let month = month
            .to_i128()
            .ok_or_else(|| Error::range("Temporal month is out of range"))?;
        let year = partial.year.unwrap_or_else(|| BigInt::from(1972));
        let day = partial.day.unwrap_or_else(|| BigInt::from(receiver.day));
        let maximum_day = temporal_bigint_days_in_month(&year, month)
            .ok_or_else(|| Error::range("Temporal month is out of range"))?;
        let day = if day > BigInt::from(maximum_day) {
            match overflow {
                TemporalOverflow::Constrain => maximum_day,
                TemporalOverflow::Reject => {
                    return Err(Error::range("Temporal day is out of range"));
                }
            }
        } else {
            day.to_i128()
                .ok_or_else(|| Error::range("Temporal day is out of range"))?
        };
        let resolved = temporal_plain_date_fields([
            BigInt::from(1972),
            BigInt::from(month),
            BigInt::from(day),
        ])
        .ok_or_else(|| Error::range("Temporal.PlainMonthDay fields are out of range"))?;
        Ok(TemporalPlainMonthDayFields {
            reference_iso_year: resolved.year,
            month: resolved.month,
            day: resolved.day,
        })
    })();
    vm.unpin_many(pins);
    let fields = result?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_month_day_in_realm(vm, fields, calendar_identifier, realm)
}

fn to_temporal_plain_month_day(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(TemporalPlainMonthDayFields, Arc<str>)> {
    if let Some(slots) = temporal_plain_month_day_slots_if_present(vm, item) {
        temporal_from_overflow(vm, options)?;
        Ok(slots)
    } else if matches!(item, Value::Object(_)) {
        temporal_plain_month_day_from_property_bag(vm, item, options)
    } else {
        let Value::String(source) = item else {
            return Err(Error::type_err(
                "Temporal.PlainMonthDay input must be a String or object",
            ));
        };
        vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
        let parsed = temporal::parse_plain_month_day_string(source)
            .ok_or_else(|| Error::range("Invalid Temporal.PlainMonthDay string"))?;
        temporal_from_overflow(vm, options)?;
        let fields = TemporalPlainMonthDayFields {
            reference_iso_year: i32::try_from(parsed.reference_iso_year)
                .map_err(|_| Error::range("Temporal.PlainMonthDay fields are out of range"))?,
            month: u8::try_from(parsed.month)
                .map_err(|_| Error::range("Temporal.PlainMonthDay fields are out of range"))?,
            day: u8::try_from(parsed.day)
                .map_err(|_| Error::range("Temporal.PlainMonthDay fields are out of range"))?,
        };
        Ok((fields, parsed.calendar_identifier))
    }
}

fn temporal_plain_month_day_from(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) =
        to_temporal_plain_month_day(vm, args.first().unwrap_or(&Value::Undefined), args.get(1))?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_month_day_in_realm(vm, fields, calendar_identifier, realm)
}

fn temporal_plain_month_day_calendar_id(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_month_day_slots(vm, this).map(|(_, calendar)| Value::String(calendar))
}

fn temporal_plain_month_day_month_code(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_month_day_slots(vm, this)
        .map(|(fields, _)| Value::String(Arc::from(format!("M{:02}", fields.month))))
}

fn temporal_plain_month_day_day(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_month_day_slots(vm, this).map(|(fields, _)| Value::Number(f64::from(fields.day)))
}

fn temporal_plain_month_day_value_of(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_month_day_slots(vm, this)?;
    Err(Error::type_err(
        "Temporal.PlainMonthDay.prototype.valueOf always throws",
    ))
}

fn temporal_plain_month_day_to_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) = temporal_plain_month_day_slots(vm, this)?;
    let calendar_name = temporal_calendar_name_to_string_option(vm, args, "PlainMonthDay")?;
    temporal::format_plain_month_day(
        fields.reference_iso_year,
        fields.month,
        fields.day,
        &calendar_identifier,
        calendar_name,
    )
    .map(Arc::<str>::from)
    .map(Value::String)
    .ok_or_else(|| Error::range("Temporal.PlainMonthDay string formatting failed"))
}

fn temporal_plain_year_month_slots(
    vm: &Vm,
    this: Option<Value>,
) -> error::Result<(TemporalPlainYearMonthFields, Arc<str>)> {
    let Value::Object(index) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(
            "Temporal.PlainYearMonth method called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::PlainYearMonth {
                    fields,
                    calendar_identifier,
                },
            ..
        }) => Ok((*fields, calendar_identifier.clone())),
        _ => Err(Error::type_err(
            "Temporal.PlainYearMonth method called on incompatible receiver",
        )),
    })
}

fn temporal_plain_year_month_slots_if_present(
    vm: &Vm,
    value: &Value,
) -> Option<(TemporalPlainYearMonthFields, Arc<str>)> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind:
                TemporalKind::PlainYearMonth {
                    fields,
                    calendar_identifier,
                },
            ..
        }) => Some((*fields, calendar_identifier.clone())),
        _ => None,
    })
}

fn temporal_plain_year_month_fields(
    year: BigInt,
    month: BigInt,
    reference_iso_day: BigInt,
) -> Option<TemporalPlainYearMonthFields> {
    let year = year.to_i128()?;
    let month = month.to_i128()?;
    let reference_iso_day = reference_iso_day.to_i128()?;
    if !(1..=temporal::days_in_month(year, month)?).contains(&reference_iso_day)
        || (year, month) < (-271_821, 4)
        || (year, month) > (275_760, 9)
    {
        return None;
    }
    Some(TemporalPlainYearMonthFields {
        year: i32::try_from(year).ok()?,
        month: u8::try_from(month).ok()?,
        reference_iso_day: u8::try_from(reference_iso_day).ok()?,
    })
}

fn create_temporal_plain_year_month(
    vm: &mut Vm,
    fields: TemporalPlainYearMonthFields,
    calendar_identifier: Arc<str>,
    prototype: Value,
) -> error::Result<Value> {
    if temporal_plain_year_month_fields(
        BigInt::from(fields.year),
        BigInt::from(fields.month),
        BigInt::from(fields.reference_iso_day),
    ) != Some(fields)
    {
        return Err(Error::range("Invalid Temporal.PlainYearMonth fields"));
    }
    vm.try_reserve_gc_pins(1)?;
    let pin_count = vm.pin(&prototype);
    let result = vm.alloc(HeapObj::Temporal(TemporalData {
        kind: TemporalKind::PlainYearMonth {
            fields,
            calendar_identifier,
        },
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    result.map(Value::Object)
}

fn create_temporal_plain_year_month_in_realm(
    vm: &mut Vm,
    fields: TemporalPlainYearMonthFields,
    calendar_identifier: Arc<str>,
    realm: GcIdx,
) -> error::Result<Value> {
    let prototype = vm
        .realm_temporal_plain_year_month_prototypes
        .get(&env::global_env_root(&vm.heap, realm).0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainYearMonth prototype is not installed"))?;
    create_temporal_plain_year_month(vm, fields, calendar_identifier, prototype)
}

fn temporal_plain_year_month_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Temporal.PlainYearMonth requires 'new'"));
    }
    let year =
        temporal_integer_with_truncation(vm, args.first().cloned().unwrap_or(Value::Undefined))?;
    let month =
        temporal_integer_with_truncation(vm, args.get(1).cloned().unwrap_or(Value::Undefined))?;
    let calendar_identifier = temporal_constructor_calendar(
        vm,
        args.get(2).cloned().unwrap_or(Value::Undefined),
        "PlainYearMonth",
    )?;
    let reference_iso_day = match args.get(3) {
        None | Some(Value::Undefined) => BigInt::from(1),
        Some(value) => temporal_integer_with_truncation(vm, value.clone())?,
    };
    let fields = temporal_plain_year_month_fields(year, month, reference_iso_day)
        .ok_or_else(|| Error::range("Invalid Temporal.PlainYearMonth fields"))?;
    let realm = env::global_env_root(&vm.heap, vm.native_callee_closure().unwrap_or(vm.global));
    let fallback = vm
        .realm_temporal_plain_year_month_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainYearMonth prototype is not installed"))?;
    let prototype =
        native_constructor_prototype_with_default(vm, "Temporal.PlainYearMonth", fallback)?;
    create_temporal_plain_year_month(vm, fields, calendar_identifier, prototype)
}

struct TemporalPlainYearMonthPropertyFields {
    year: Option<BigInt>,
    month: Option<BigInt>,
    month_code: Option<(u8, bool)>,
    calendar_identifier: Arc<str>,
}

fn temporal_plain_year_month_property_fields(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainYearMonthPropertyFields> {
    vm.try_reserve_value_roots(std::slice::from_ref(item))?;
    let item_pins = vm.pin(item);
    let result = (|| {
        let calendar_identifier =
            if let Some(calendar) = temporal_calendar_slot_if_present(vm, item) {
                calendar
            } else {
                let calendar = vm.get_property(item, "calendar")?;
                temporal_calendar_from_value(vm, calendar)?
            };
        let month = match vm.get_property(item, "month")? {
            Value::Undefined => None,
            value => Some(temporal_positive_integer_with_truncation(vm, value)?),
        };
        let month_code = match vm.get_property(item, "monthCode")? {
            Value::Undefined => None,
            value => Some(temporal_month_code(vm, value)?),
        };
        let year = match vm.get_property(item, "year")? {
            Value::Undefined => None,
            value => Some(temporal_integer_with_truncation(vm, value)?),
        };
        Ok(TemporalPlainYearMonthPropertyFields {
            year,
            month,
            month_code,
            calendar_identifier,
        })
    })();
    vm.unpin_many(item_pins);
    result
}

fn temporal_plain_year_month_from_property_bag(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(TemporalPlainYearMonthFields, Arc<str>)> {
    let fields = temporal_plain_year_month_property_fields(vm, item)?;
    let overflow = temporal_from_overflow(vm, options)?;

    let year = fields
        .year
        .ok_or_else(|| Error::type_err("Temporal property bag requires year"))?;
    if fields.month.is_none() && fields.month_code.is_none() {
        return Err(Error::type_err(
            "Temporal property bag requires month or monthCode",
        ));
    }
    if let Some((month_code, leap)) = fields.month_code {
        if leap || !(1..=12).contains(&month_code) {
            return Err(Error::range("Invalid monthCode for ISO 8601 calendar"));
        }
        if fields
            .month
            .as_ref()
            .is_some_and(|month| month != &BigInt::from(month_code))
        {
            return Err(Error::range("month and monthCode do not agree"));
        }
    }

    let mut month = fields
        .month
        .unwrap_or_else(|| BigInt::from(fields.month_code.unwrap().0));
    if month <= BigInt::zero() {
        return Err(Error::range("Temporal month is out of range"));
    }
    if month > BigInt::from(12) {
        match overflow {
            TemporalOverflow::Constrain if fields.month_code.is_none() => month = BigInt::from(12),
            TemporalOverflow::Constrain | TemporalOverflow::Reject => {
                return Err(Error::range("Temporal month is out of range"));
            }
        }
    }
    let resolved = temporal_plain_year_month_fields(year, month, BigInt::from(1))
        .ok_or_else(|| Error::range("Temporal.PlainYearMonth fields are out of range"))?;
    Ok((resolved, fields.calendar_identifier))
}

struct TemporalPlainYearMonthPartialFields {
    month: Option<BigInt>,
    month_code: Option<(u8, bool)>,
    year: Option<BigInt>,
}

fn temporal_plain_year_month_partial_fields(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainYearMonthPartialFields> {
    let month = match vm.get_property(item, "month")? {
        Value::Undefined => None,
        value => Some(temporal_positive_integer_with_truncation(vm, value)?),
    };
    let month_code = match vm.get_property(item, "monthCode")? {
        Value::Undefined => None,
        value => Some(temporal_month_code(vm, value)?),
    };
    let year = match vm.get_property(item, "year")? {
        Value::Undefined => None,
        value => Some(temporal_integer_with_truncation(vm, value)?),
    };
    if month.is_none() && month_code.is_none() && year.is_none() {
        return Err(Error::type_err(
            "Temporal.PlainYearMonth.prototype.with requires at least one field",
        ));
    }
    Ok(TemporalPlainYearMonthPartialFields {
        month,
        month_code,
        year,
    })
}

fn temporal_plain_year_month_with(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (receiver, calendar_identifier) = temporal_plain_year_month_slots(vm, this)?;
    let item = args.first().cloned().unwrap_or(Value::Undefined);
    let options = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !matches!(item, Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.PlainYearMonth.prototype.with requires an object",
        ));
    }

    vm.try_reserve_value_roots(&[item.clone(), options.clone()])?;
    let pins = vm.pin_many(&[item.clone(), options.clone()]);
    let result = (|| {
        temporal_reject_partial_calendar_or_time_zone(vm, &item, "PlainYearMonth")?;
        let partial = temporal_plain_year_month_partial_fields(vm, &item)?;
        let overflow = temporal_from_overflow(vm, Some(&options))?;

        if let Some((month_code, leap)) = partial.month_code {
            if leap || !(1..=12).contains(&month_code) {
                return Err(Error::range("Invalid monthCode for ISO 8601 calendar"));
            }
            if partial
                .month
                .as_ref()
                .is_some_and(|month| month != &BigInt::from(month_code))
            {
                return Err(Error::range("month and monthCode do not agree"));
            }
        }

        let mut month = partial.month.unwrap_or_else(|| {
            partial
                .month_code
                .map_or_else(|| BigInt::from(receiver.month), |code| BigInt::from(code.0))
        });
        if month > BigInt::from(12) {
            match overflow {
                TemporalOverflow::Constrain if partial.month_code.is_none() => {
                    month = BigInt::from(12)
                }
                TemporalOverflow::Constrain | TemporalOverflow::Reject => {
                    return Err(Error::range("Temporal month is out of range"));
                }
            }
        }
        temporal_plain_year_month_fields(
            partial.year.unwrap_or_else(|| BigInt::from(receiver.year)),
            month,
            BigInt::from(1),
        )
        .ok_or_else(|| Error::range("Temporal.PlainYearMonth fields are out of range"))
    })();
    vm.unpin_many(pins);
    let fields = result?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_year_month_in_realm(vm, fields, calendar_identifier, realm)
}

fn temporal_plain_year_month_add_or_subtract(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    subtract: bool,
) -> error::Result<Value> {
    let (receiver, calendar_identifier) = temporal_plain_year_month_slots(vm, this)?;
    let duration_like = args.first().cloned().unwrap_or(Value::Undefined);
    let options = args.get(1).cloned().unwrap_or(Value::Undefined);
    vm.try_reserve_value_roots(&[duration_like.clone(), options.clone()])?;
    let pins = vm.pin_many(&[duration_like.clone(), options.clone()]);
    let result = (|| {
        let duration = to_temporal_duration_fields(vm, &duration_like)?;
        let mut values = temporal_duration_integer_values(duration)?;
        if subtract {
            for value in &mut values {
                *value = value
                    .checked_neg()
                    .ok_or_else(|| Error::range("Temporal.Duration fields are out of range"))?;
            }
        }
        let _overflow = temporal_from_overflow(vm, Some(&options))?;
        if values[2..].iter().any(|value| *value != 0) {
            return Err(Error::range(
                "Temporal.PlainYearMonth arithmetic requires year and month units",
            ));
        }
        if calendar_identifier.as_ref() != "iso8601" {
            return Err(Error::range(
                "Temporal.PlainYearMonth arithmetic requires the ISO 8601 calendar",
            ));
        }

        let date = temporal_plain_date_fields([
            BigInt::from(receiver.year),
            BigInt::from(receiver.month),
            BigInt::from(1),
        ])
        .ok_or_else(|| Error::range("Temporal.PlainYearMonth date is out of range"))?;
        let epoch_day = temporal_duration_iso_date_add(date, values[0], values[1], 0, 0)?;
        let (year, month, _) = temporal::civil_from_days(epoch_day)
            .ok_or_else(|| Error::range("Temporal.PlainYearMonth result is out of range"))?;
        temporal_plain_year_month_fields(BigInt::from(year), BigInt::from(month), BigInt::from(1))
            .ok_or_else(|| Error::range("Temporal.PlainYearMonth result is out of range"))
    })();
    vm.unpin_many(pins);
    let fields = result?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_year_month_in_realm(vm, fields, calendar_identifier, realm)
}

fn temporal_plain_year_month_add(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_add_or_subtract(vm, args, this, false)
}

fn temporal_plain_year_month_subtract(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_add_or_subtract(vm, args, this, true)
}

fn temporal_plain_year_month_equals(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) = temporal_plain_year_month_slots(vm, this)?;
    let (other_fields, other_calendar) =
        to_temporal_plain_year_month(vm, args.first().unwrap_or(&Value::Undefined), None)?;
    Ok(Value::Bool(
        fields == other_fields && temporal_calendar_equals(&calendar_identifier, &other_calendar),
    ))
}

fn temporal_plain_year_month_compare(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let (one, _) =
        to_temporal_plain_year_month(vm, args.first().unwrap_or(&Value::Undefined), None)?;
    let (two, _) =
        to_temporal_plain_year_month(vm, args.get(1).unwrap_or(&Value::Undefined), None)?;
    let result = match (one.year, one.month, one.reference_iso_day).cmp(&(
        two.year,
        two.month,
        two.reference_iso_day,
    )) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    };
    Ok(Value::Number(result))
}

fn to_temporal_plain_year_month(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(TemporalPlainYearMonthFields, Arc<str>)> {
    if let Some(slots) = temporal_plain_year_month_slots_if_present(vm, item) {
        temporal_from_overflow(vm, options)?;
        Ok(slots)
    } else if matches!(item, Value::Object(_)) {
        temporal_plain_year_month_from_property_bag(vm, item, options)
    } else {
        let Value::String(source) = item else {
            return Err(Error::type_err(
                "Temporal.PlainYearMonth input must be a String or object",
            ));
        };
        vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
        let parsed = temporal::parse_plain_year_month_string(source)
            .ok_or_else(|| Error::range("Invalid Temporal.PlainYearMonth string"))?;
        temporal_from_overflow(vm, options)?;
        let fields = temporal_plain_year_month_fields(
            BigInt::from(parsed.year),
            BigInt::from(parsed.month),
            BigInt::from(parsed.reference_iso_day),
        )
        .ok_or_else(|| Error::range("Temporal.PlainYearMonth fields are out of range"))?;
        Ok((fields, parsed.calendar_identifier))
    }
}

fn temporal_plain_year_month_from(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) =
        to_temporal_plain_year_month(vm, args.first().unwrap_or(&Value::Undefined), args.get(1))?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_year_month_in_realm(vm, fields, calendar_identifier, realm)
}

fn temporal_plain_year_month_calendar_id(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_slots(vm, this).map(|(_, calendar)| Value::String(calendar))
}

fn temporal_plain_year_month_era(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_slots(vm, this).map(|_| Value::Undefined)
}

fn temporal_plain_year_month_era_year(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_slots(vm, this).map(|_| Value::Undefined)
}

fn temporal_plain_year_month_year(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_slots(vm, this)
        .map(|(fields, _)| Value::Number(f64::from(fields.year)))
}

fn temporal_plain_year_month_month(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_slots(vm, this)
        .map(|(fields, _)| Value::Number(f64::from(fields.month)))
}

fn temporal_plain_year_month_month_code(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_slots(vm, this)
        .map(|(fields, _)| Value::String(Arc::from(format!("M{:02}", fields.month))))
}

fn temporal_plain_year_month_days_in_month(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, _) = temporal_plain_year_month_slots(vm, this)?;
    let days = temporal::days_in_month(i128::from(fields.year), i128::from(fields.month))
        .ok_or_else(|| Error::internal("Invalid Temporal.PlainYearMonth slots"))?;
    Ok(Value::Number(days as f64))
}

fn temporal_plain_year_month_days_in_year(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, _) = temporal_plain_year_month_slots(vm, this)?;
    Ok(Value::Number(
        if temporal::leap_year(i128::from(fields.year)) {
            366.0
        } else {
            365.0
        },
    ))
}

fn temporal_plain_year_month_months_in_year(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_slots(vm, this).map(|_| Value::Number(12.0))
}

fn temporal_plain_year_month_in_leap_year(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_slots(vm, this)
        .map(|(fields, _)| Value::Bool(temporal::leap_year(i128::from(fields.year))))
}

fn temporal_plain_year_month_value_of(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_year_month_slots(vm, this)?;
    Err(Error::type_err(
        "Temporal.PlainYearMonth.prototype.valueOf always throws",
    ))
}

fn temporal_plain_year_month_to_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) = temporal_plain_year_month_slots(vm, this)?;
    let calendar_name = temporal_calendar_name_to_string_option(vm, args, "PlainYearMonth")?;
    temporal::format_plain_year_month(
        fields.year,
        fields.month,
        fields.reference_iso_day,
        &calendar_identifier,
        calendar_name,
    )
    .map(Arc::<str>::from)
    .map(Value::String)
    .ok_or_else(|| Error::range("Temporal.PlainYearMonth string formatting failed"))
}

fn temporal_plain_year_month_to_json(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) = temporal_plain_year_month_slots(vm, this)?;
    temporal::format_plain_year_month(
        fields.year,
        fields.month,
        fields.reference_iso_day,
        &calendar_identifier,
        temporal::AnnotationDisplay::Auto,
    )
    .map(Arc::<str>::from)
    .map(Value::String)
    .ok_or_else(|| Error::range("Temporal.PlainYearMonth JSON formatting failed"))
}

fn temporal_plain_year_month_to_plain_date(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (receiver, calendar_identifier) = temporal_plain_year_month_slots(vm, this)?;
    let item = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(item, Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.PlainYearMonth.prototype.toPlainDate requires an object",
        ));
    }

    vm.try_reserve_value_roots(std::slice::from_ref(&item))?;
    let item_pins = vm.pin(&item);
    let result = (|| {
        if calendar_identifier.as_ref() != "iso8601" {
            return Err(Error::range(
                "Temporal.PlainYearMonth.prototype.toPlainDate requires a supported calendar",
            ));
        }
        let day = match vm.get_property(&item, "day")? {
            Value::Undefined => {
                return Err(Error::type_err(
                    "Temporal.PlainYearMonth.prototype.toPlainDate requires day",
                ));
            }
            value => temporal_integer_with_truncation(vm, value)?,
        };
        if day <= BigInt::zero() {
            return Err(Error::range("Temporal day is out of range"));
        }
        let maximum_day =
            temporal::days_in_month(i128::from(receiver.year), i128::from(receiver.month))
                .ok_or_else(|| Error::range("Temporal month is out of range"))?;
        let day = if day > BigInt::from(maximum_day) {
            maximum_day
        } else {
            day.to_i128()
                .ok_or_else(|| Error::range("Temporal day is out of range"))?
        };
        let fields = temporal_plain_date_fields([
            BigInt::from(receiver.year),
            BigInt::from(receiver.month),
            BigInt::from(day),
        ])
        .ok_or_else(|| Error::range("Temporal.PlainDate fields are out of range"))?;
        let realm = vm.native_callee_closure().unwrap_or(vm.global);
        create_temporal_plain_date_in_realm(vm, fields, calendar_identifier.clone(), realm)
    })();
    vm.unpin_many(item_pins);
    result
}

fn temporal_plain_time_slots(
    vm: &Vm,
    this: Option<Value>,
) -> error::Result<TemporalPlainTimeFields> {
    let Value::Object(index) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(
            "Temporal.PlainTime method called on incompatible receiver",
        ));
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind: TemporalKind::PlainTime { fields },
            ..
        }) => Ok(*fields),
        _ => Err(Error::type_err(
            "Temporal.PlainTime method called on incompatible receiver",
        )),
    })
}

fn temporal_plain_time_slots_if_present(vm: &Vm, value: &Value) -> Option<TemporalPlainTimeFields> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Temporal(TemporalData {
            kind: TemporalKind::PlainTime { fields },
            ..
        }) => Some(*fields),
        _ => None,
    })
}

fn temporal_plain_time_fields(
    values: [BigInt; 6],
    overflow: TemporalOverflow,
) -> error::Result<TemporalPlainTimeFields> {
    let [hour, minute, second, millisecond, microsecond, nanosecond] = values;
    Ok(TemporalPlainTimeFields {
        hour: u8::try_from(temporal_regulate_field(hour, 23, overflow)?)
            .map_err(|_| Error::range("Invalid Temporal.PlainTime fields"))?,
        minute: u8::try_from(temporal_regulate_field(minute, 59, overflow)?)
            .map_err(|_| Error::range("Invalid Temporal.PlainTime fields"))?,
        second: u8::try_from(temporal_regulate_field(second, 59, overflow)?)
            .map_err(|_| Error::range("Invalid Temporal.PlainTime fields"))?,
        millisecond: u16::try_from(temporal_regulate_field(millisecond, 999, overflow)?)
            .map_err(|_| Error::range("Invalid Temporal.PlainTime fields"))?,
        microsecond: u16::try_from(temporal_regulate_field(microsecond, 999, overflow)?)
            .map_err(|_| Error::range("Invalid Temporal.PlainTime fields"))?,
        nanosecond: u16::try_from(temporal_regulate_field(nanosecond, 999, overflow)?)
            .map_err(|_| Error::range("Invalid Temporal.PlainTime fields"))?,
    })
}

fn temporal_plain_time_is_valid(fields: TemporalPlainTimeFields) -> bool {
    temporal_plain_time_fields(
        [
            BigInt::from(fields.hour),
            BigInt::from(fields.minute),
            BigInt::from(fields.second),
            BigInt::from(fields.millisecond),
            BigInt::from(fields.microsecond),
            BigInt::from(fields.nanosecond),
        ],
        TemporalOverflow::Reject,
    )
    .is_ok()
}

fn create_temporal_plain_time(
    vm: &mut Vm,
    fields: TemporalPlainTimeFields,
    prototype: Value,
) -> error::Result<Value> {
    if !temporal_plain_time_is_valid(fields) {
        return Err(Error::range("Invalid Temporal.PlainTime fields"));
    }
    vm.try_reserve_gc_pins(1)?;
    let pin_count = vm.pin(&prototype);
    let result = vm.alloc(HeapObj::Temporal(TemporalData {
        kind: TemporalKind::PlainTime { fields },
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    result.map(Value::Object)
}

fn create_temporal_plain_time_in_realm(
    vm: &mut Vm,
    fields: TemporalPlainTimeFields,
    realm: GcIdx,
) -> error::Result<Value> {
    let prototype = vm
        .realm_temporal_plain_time_prototypes
        .get(&env::global_env_root(&vm.heap, realm).0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainTime prototype is not installed"))?;
    create_temporal_plain_time(vm, fields, prototype)
}

fn temporal_plain_time_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Temporal.PlainTime requires 'new'"));
    }
    let numeric = |vm: &mut Vm, index: usize| -> error::Result<BigInt> {
        match args.get(index) {
            None | Some(Value::Undefined) => Ok(BigInt::zero()),
            Some(value) => temporal_integer_with_truncation(vm, value.clone()),
        }
    };
    let fields = temporal_plain_time_fields(
        [
            numeric(vm, 0)?,
            numeric(vm, 1)?,
            numeric(vm, 2)?,
            numeric(vm, 3)?,
            numeric(vm, 4)?,
            numeric(vm, 5)?,
        ],
        TemporalOverflow::Reject,
    )?;
    let realm = env::global_env_root(&vm.heap, vm.native_callee_closure().unwrap_or(vm.global));
    let fallback = vm
        .realm_temporal_plain_time_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("Temporal.PlainTime prototype is not installed"))?;
    let prototype = native_constructor_prototype_with_default(vm, "Temporal.PlainTime", fallback)?;
    create_temporal_plain_time(vm, fields, prototype)
}

macro_rules! temporal_plain_time_number_getter {
    ($name:ident, $field:ident) => {
        fn $name(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
            temporal_plain_time_slots(vm, this)
                .map(|fields| Value::Number(f64::from(fields.$field)))
        }
    };
}

temporal_plain_time_number_getter!(temporal_plain_time_hour, hour);
temporal_plain_time_number_getter!(temporal_plain_time_minute, minute);
temporal_plain_time_number_getter!(temporal_plain_time_second, second);
temporal_plain_time_number_getter!(temporal_plain_time_millisecond, millisecond);
temporal_plain_time_number_getter!(temporal_plain_time_microsecond, microsecond);
temporal_plain_time_number_getter!(temporal_plain_time_nanosecond, nanosecond);

#[derive(Clone, Copy)]
struct TemporalPlainTimeRecord {
    hour: i128,
    minute: i128,
    second: i128,
    millisecond: i128,
    microsecond: i128,
    nanosecond: i128,
}

impl TemporalPlainTimeRecord {
    const MIDNIGHT: Self = Self {
        hour: 0,
        minute: 0,
        second: 0,
        millisecond: 0,
        microsecond: 0,
        nanosecond: 0,
    };

    fn from_plain_time(fields: TemporalPlainTimeFields) -> Self {
        Self {
            hour: i128::from(fields.hour),
            minute: i128::from(fields.minute),
            second: i128::from(fields.second),
            millisecond: i128::from(fields.millisecond),
            microsecond: i128::from(fields.microsecond),
            nanosecond: i128::from(fields.nanosecond),
        }
    }
}

struct TemporalPlainTimePropertyFields {
    hour: Option<BigInt>,
    minute: Option<BigInt>,
    second: Option<BigInt>,
    millisecond: Option<BigInt>,
    microsecond: Option<BigInt>,
    nanosecond: Option<BigInt>,
}

fn temporal_plain_time_property_fields_rooted(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainTimePropertyFields> {
    let numeric = |vm: &mut Vm, name: &str| -> error::Result<Option<BigInt>> {
        match vm.get_property(item, name)? {
            Value::Undefined => Ok(None),
            value => temporal_integer_with_truncation(vm, value).map(Some),
        }
    };
    let hour = numeric(vm, "hour")?;
    let microsecond = numeric(vm, "microsecond")?;
    let millisecond = numeric(vm, "millisecond")?;
    let minute = numeric(vm, "minute")?;
    let nanosecond = numeric(vm, "nanosecond")?;
    let second = numeric(vm, "second")?;
    if [
        &hour,
        &microsecond,
        &millisecond,
        &minute,
        &nanosecond,
        &second,
    ]
    .into_iter()
    .all(Option::is_none)
    {
        return Err(Error::type_err(
            "Temporal.PlainTime property bag requires a time field",
        ));
    }
    Ok(TemporalPlainTimePropertyFields {
        hour,
        minute,
        second,
        millisecond,
        microsecond,
        nanosecond,
    })
}

fn temporal_plain_time_property_fields(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainTimePropertyFields> {
    vm.try_reserve_value_roots(std::slice::from_ref(item))?;
    let item_pins = vm.pin(item);
    let result = temporal_plain_time_property_fields_rooted(vm, item);
    vm.unpin_many(item_pins);
    result
}

fn to_temporal_plain_time_fields(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainTimeFields> {
    to_temporal_plain_time_fields_with_options(vm, item, None)
}

fn to_temporal_plain_time_fields_with_options(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<TemporalPlainTimeFields> {
    if let Some(fields) = temporal_plain_time_slots_if_present(vm, item) {
        temporal_from_overflow(vm, options)?;
        return Ok(fields);
    }
    if let Some((fields, _)) = temporal_plain_date_time_slots_if_present(vm, item) {
        temporal_from_overflow(vm, options)?;
        return Ok(TemporalPlainTimeFields {
            hour: fields.hour,
            minute: fields.minute,
            second: fields.second,
            millisecond: fields.millisecond,
            microsecond: fields.microsecond,
            nanosecond: fields.nanosecond,
        });
    }
    if let Some((epoch_nanoseconds, time_zone, _)) =
        temporal_zoned_date_time_slots_if_present(vm, item)
    {
        let fields = temporal_zoned_date_time_plain_fields(&epoch_nanoseconds, &time_zone)?;
        temporal_from_overflow(vm, options)?;
        return Ok(TemporalPlainTimeFields {
            hour: fields.hour,
            minute: fields.minute,
            second: fields.second,
            millisecond: fields.millisecond,
            microsecond: fields.microsecond,
            nanosecond: fields.nanosecond,
        });
    }
    if matches!(item, Value::Object(_)) {
        let fields = temporal_plain_time_property_fields(vm, item)?;
        let overflow = temporal_from_overflow(vm, options)?;
        return temporal_plain_time_fields(
            [
                fields.hour.unwrap_or_else(BigInt::zero),
                fields.minute.unwrap_or_else(BigInt::zero),
                fields.second.unwrap_or_else(BigInt::zero),
                fields.millisecond.unwrap_or_else(BigInt::zero),
                fields.microsecond.unwrap_or_else(BigInt::zero),
                fields.nanosecond.unwrap_or_else(BigInt::zero),
            ],
            overflow,
        );
    }
    let Value::String(source) = item else {
        return Err(Error::type_err(
            "Temporal.PlainTime input must be a String or object",
        ));
    };
    vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
    let parsed = temporal::parse_plain_time_string(source)
        .ok_or_else(|| Error::range("Invalid Temporal.PlainTime string"))?;
    temporal_from_overflow(vm, options)?;
    temporal_plain_time_fields(
        [
            BigInt::from(parsed.hour),
            BigInt::from(parsed.minute),
            BigInt::from(parsed.second),
            BigInt::from(parsed.millisecond),
            BigInt::from(parsed.microsecond),
            BigInt::from(parsed.nanosecond),
        ],
        TemporalOverflow::Reject,
    )
}

fn temporal_plain_time_from(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let fields = to_temporal_plain_time_fields_with_options(
        vm,
        args.first().unwrap_or(&Value::Undefined),
        args.get(1),
    )?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_time_in_realm(vm, fields, realm)
}

fn temporal_plain_time_value_of(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_time_slots(vm, this)?;
    Err(Error::type_err(
        "Temporal.PlainTime cannot be converted to a primitive value",
    ))
}

fn temporal_plain_time_equals(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let one = temporal_plain_time_slots(vm, this)?;
    let two = to_temporal_plain_time_fields(vm, args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(one == two))
}

fn temporal_plain_time_compare(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let one = to_temporal_plain_time_fields(vm, args.first().unwrap_or(&Value::Undefined))?;
    let two = to_temporal_plain_time_fields(vm, args.get(1).unwrap_or(&Value::Undefined))?;
    let result = match (
        one.hour,
        one.minute,
        one.second,
        one.millisecond,
        one.microsecond,
        one.nanosecond,
    )
        .cmp(&(
            two.hour,
            two.minute,
            two.second,
            two.millisecond,
            two.microsecond,
            two.nanosecond,
        )) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    };
    Ok(Value::Number(result))
}

fn temporal_plain_time_to_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let fields = temporal_plain_time_slots(vm, this)?;
    let options = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(options, Value::Undefined | Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.PlainTime.prototype.toString options must be an object",
        ));
    }

    vm.try_reserve_value_roots(std::slice::from_ref(&options))?;
    let options_pin = vm.pin(&options);
    let result = (|| {
        let get_option = |vm: &mut Vm, name: &str| {
            if options.is_undefined() {
                Ok(Value::Undefined)
            } else {
                vm.get_property(&options, name)
            }
        };

        let fractional_second_digits = match get_option(vm, "fractionalSecondDigits")? {
            Value::Undefined => InstantFractionalSecondDigits::Auto,
            Value::Number(number) => InstantFractionalSecondDigits::Number(number),
            value => InstantFractionalSecondDigits::String(temporal_option_to_string(vm, &value)?),
        };
        let fractional_second_digits =
            temporal_instant_fractional_second_digits(fractional_second_digits)?;
        let rounding_mode_value = get_option(vm, "roundingMode")?;
        let rounding_mode = if rounding_mode_value.is_undefined() {
            None
        } else {
            Some(temporal_option_to_string(vm, &rounding_mode_value)?)
        };
        let rounding_mode = temporal_instant_rounding_mode(rounding_mode.as_deref())?;
        let smallest_unit_value = get_option(vm, "smallestUnit")?;
        let smallest_unit = if smallest_unit_value.is_undefined() {
            None
        } else {
            Some(temporal_option_to_string(vm, &smallest_unit_value)?)
        };
        let smallest_unit = temporal_instant_smallest_unit(smallest_unit.as_deref())?;
        let precision = temporal_instant_precision(fractional_second_digits, smallest_unit)?;

        temporal_plain_time_format(fields, precision, rounding_mode)
    })();
    vm.unpin_many(options_pin);
    result
}

fn temporal_plain_time_to_json(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let fields = temporal_plain_time_slots(vm, this)?;
    temporal_plain_time_format(
        fields,
        temporal::InstantPrecision::Auto,
        temporal::InstantRoundingMode::Trunc,
    )
}

fn temporal_plain_time_round(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let fields = temporal_plain_time_slots(vm, this)?;
    let round_to = args.first().cloned().unwrap_or(Value::Undefined);
    if round_to.is_undefined() {
        return Err(Error::type_err(
            "Temporal.PlainTime.prototype.round requires an argument",
        ));
    }

    let (rounding_increment, rounding_mode, smallest_unit) = match round_to {
        Value::String(unit) => (
            1,
            temporal::InstantRoundingMode::HalfExpand,
            temporal_plain_time_round_unit(&unit)?,
        ),
        Value::Object(_) => {
            vm.try_reserve_value_roots(std::slice::from_ref(&round_to))?;
            let options_pin = vm.pin(&round_to);
            let result = (|| {
                let rounding_increment = match vm.get_property(&round_to, "roundingIncrement")? {
                    Value::Undefined => 1,
                    value => {
                        let increment = temporal_integer_with_truncation(vm, value)?;
                        if increment < BigInt::from(1_u8)
                            || increment > BigInt::from(1_000_000_000_u32)
                        {
                            return Err(Error::range("Invalid Temporal roundingIncrement option"));
                        }
                        increment.to_u32().ok_or_else(|| {
                            Error::range("Invalid Temporal roundingIncrement option")
                        })?
                    }
                };
                let rounding_mode = match vm.get_property(&round_to, "roundingMode")? {
                    Value::Undefined => temporal::InstantRoundingMode::HalfExpand,
                    value => {
                        let value = temporal_option_to_string(vm, &value)?;
                        temporal_instant_rounding_mode(Some(&value))?
                    }
                };
                let smallest_unit = match vm.get_property(&round_to, "smallestUnit")? {
                    Value::Undefined => {
                        return Err(Error::range(
                            "Temporal.PlainTime.prototype.round requires smallestUnit",
                        ));
                    }
                    value => {
                        let value = temporal_option_to_string(vm, &value)?;
                        temporal_plain_time_round_unit(&value)?
                    }
                };
                Ok((rounding_increment, rounding_mode, smallest_unit))
            })();
            vm.unpin_many(options_pin);
            result?
        }
        _ => {
            return Err(Error::type_err(
                "Temporal.PlainTime.prototype.round argument must be a String or object",
            ));
        }
    };

    let maximum = smallest_unit.maximum_rounding_increment();
    if rounding_increment >= maximum || maximum % rounding_increment != 0 {
        return Err(Error::range(
            "Invalid Temporal roundingIncrement for smallestUnit",
        ));
    }
    let (hour, minute, second, millisecond, microsecond, nanosecond) = temporal::round_plain_time(
        fields.hour,
        fields.minute,
        fields.second,
        fields.millisecond,
        fields.microsecond,
        fields.nanosecond,
        rounding_increment,
        smallest_unit,
        rounding_mode,
    )
    .ok_or_else(|| Error::range("Temporal.PlainTime rounding failed"))?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_time_in_realm(
        vm,
        TemporalPlainTimeFields {
            hour,
            minute,
            second,
            millisecond,
            microsecond,
            nanosecond,
        },
        realm,
    )
}

fn temporal_plain_time_round_unit(value: &str) -> error::Result<temporal::TimeUnit> {
    Ok(match value {
        "hour" | "hours" => temporal::TimeUnit::Hour,
        "minute" | "minutes" => temporal::TimeUnit::Minute,
        "second" | "seconds" => temporal::TimeUnit::Second,
        "millisecond" | "milliseconds" => temporal::TimeUnit::Millisecond,
        "microsecond" | "microseconds" => temporal::TimeUnit::Microsecond,
        "nanosecond" | "nanoseconds" => temporal::TimeUnit::Nanosecond,
        _ => return Err(Error::range("Invalid Temporal smallestUnit option")),
    })
}

fn temporal_plain_time_with(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = temporal_plain_time_slots(vm, this)?;
    let item = args.first().cloned().unwrap_or(Value::Undefined);
    let options = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !matches!(item, Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.PlainTime.prototype.with requires an object",
        ));
    }

    vm.try_reserve_value_roots(&[item.clone(), options.clone()])?;
    let pins = vm.pin_many(&[item.clone(), options.clone()]);
    let result = (|| {
        let is_temporal_date_or_time = if let Value::Object(index) = &item {
            vm.heap.with_obj(index.0, |object| {
                matches!(
                    object,
                    HeapObj::Temporal(TemporalData {
                        kind: TemporalKind::PlainDate { .. }
                            | TemporalKind::PlainMonthDay { .. }
                            | TemporalKind::PlainTime { .. }
                            | TemporalKind::PlainDateTime { .. }
                            | TemporalKind::PlainYearMonth { .. }
                            | TemporalKind::ZonedDateTime { .. },
                        ..
                    })
                )
            })
        } else {
            false
        };
        if is_temporal_date_or_time {
            return Err(Error::type_err(
                "Temporal.PlainTime.prototype.with requires a partial Temporal object",
            ));
        }
        if !vm.get_property(&item, "calendar")?.is_undefined() {
            return Err(Error::type_err(
                "Temporal.PlainTime.prototype.with rejects calendar",
            ));
        }
        if !vm.get_property(&item, "timeZone")?.is_undefined() {
            return Err(Error::type_err(
                "Temporal.PlainTime.prototype.with rejects timeZone",
            ));
        }

        let partial = temporal_plain_time_property_fields_rooted(vm, &item)?;
        let overflow = temporal_from_overflow(vm, Some(&options))?;
        temporal_plain_time_fields(
            [
                partial.hour.unwrap_or_else(|| BigInt::from(receiver.hour)),
                partial
                    .minute
                    .unwrap_or_else(|| BigInt::from(receiver.minute)),
                partial
                    .second
                    .unwrap_or_else(|| BigInt::from(receiver.second)),
                partial
                    .millisecond
                    .unwrap_or_else(|| BigInt::from(receiver.millisecond)),
                partial
                    .microsecond
                    .unwrap_or_else(|| BigInt::from(receiver.microsecond)),
                partial
                    .nanosecond
                    .unwrap_or_else(|| BigInt::from(receiver.nanosecond)),
            ],
            overflow,
        )
    })();
    vm.unpin_many(pins);
    let fields = result?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_time_in_realm(vm, fields, realm)
}

fn temporal_plain_time_add_or_subtract(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    subtract: bool,
) -> error::Result<Value> {
    let receiver = temporal_plain_time_slots(vm, this)?;
    let duration = to_temporal_duration_fields(vm, args.first().unwrap_or(&Value::Undefined))?;
    let duration_nanoseconds = temporal_duration_time_nanoseconds(duration)?;
    let duration_nanoseconds = if subtract {
        duration_nanoseconds
            .checked_neg()
            .ok_or_else(|| Error::range("Temporal.Duration time fields are out of range"))?
    } else {
        duration_nanoseconds
    };
    let (hour, minute, second, millisecond, microsecond, nanosecond) = temporal::add_plain_time(
        receiver.hour,
        receiver.minute,
        receiver.second,
        receiver.millisecond,
        receiver.microsecond,
        receiver.nanosecond,
        duration_nanoseconds,
    )
    .ok_or_else(|| Error::range("Temporal.PlainTime arithmetic failed"))?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_time_in_realm(
        vm,
        TemporalPlainTimeFields {
            hour,
            minute,
            second,
            millisecond,
            microsecond,
            nanosecond,
        },
        realm,
    )
}

fn temporal_plain_time_add(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_time_add_or_subtract(vm, args, this, false)
}

fn temporal_plain_time_subtract(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_plain_time_add_or_subtract(vm, args, this, true)
}

fn temporal_plain_time_format(
    fields: TemporalPlainTimeFields,
    precision: temporal::InstantPrecision,
    rounding_mode: temporal::InstantRoundingMode,
) -> error::Result<Value> {
    temporal::format_plain_time(
        fields.hour,
        fields.minute,
        fields.second,
        fields.millisecond,
        fields.microsecond,
        fields.nanosecond,
        precision,
        rounding_mode,
    )
    .map(Arc::<str>::from)
    .map(Value::String)
    .ok_or_else(|| Error::range("Temporal.PlainTime string formatting failed"))
}

fn temporal_time_record_or_midnight(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalPlainTimeRecord> {
    if item.is_undefined() {
        return Ok(TemporalPlainTimeRecord::MIDNIGHT);
    }
    to_temporal_plain_time_fields(vm, item).map(TemporalPlainTimeRecord::from_plain_time)
}

fn temporal_plain_date_to_plain_date_time(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (date, calendar_identifier) = temporal_plain_date_slots(vm, this)?;
    let time = temporal_time_record_or_midnight(vm, args.first().unwrap_or(&Value::Undefined))?;
    let fields = temporal_plain_date_time_fields_from_iso(temporal::IsoDateTimeFields {
        year: i128::from(date.year),
        month: i128::from(date.month),
        day: i128::from(date.day),
        hour: time.hour,
        minute: time.minute,
        second: time.second,
        millisecond: time.millisecond,
        microsecond: time.microsecond,
        nanosecond: time.nanosecond,
    })?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_date_time_in_realm(vm, fields, calendar_identifier, realm)
}

fn temporal_plain_date_format(
    fields: TemporalPlainDateFields,
    calendar_identifier: &str,
    calendar_name: temporal::AnnotationDisplay,
) -> error::Result<Value> {
    temporal::format_plain_date(
        fields.year,
        fields.month,
        fields.day,
        calendar_identifier,
        calendar_name,
    )
    .map(Arc::<str>::from)
    .map(Value::String)
    .ok_or_else(|| Error::range("Temporal.PlainDate string formatting failed"))
}

fn to_temporal_plain_date_time(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(TemporalPlainDateTimeFields, Arc<str>)> {
    if let Some(slots) = temporal_plain_date_time_slots_if_present(vm, item) {
        temporal_from_overflow(vm, options)?;
        Ok(slots)
    } else if let Some((epoch_nanoseconds, time_zone, calendar_identifier)) =
        temporal_zoned_date_time_slots_if_present(vm, item)
    {
        let fields = temporal_zoned_date_time_plain_fields(&epoch_nanoseconds, &time_zone)?;
        temporal_from_overflow(vm, options)?;
        Ok((fields, calendar_identifier))
    } else if let Some((date, calendar_identifier)) = temporal_plain_date_slots_if_present(vm, item)
    {
        temporal_from_overflow(vm, options)?;
        Ok((
            temporal_plain_date_time_fields_from_iso(temporal::IsoDateTimeFields {
                year: i128::from(date.year),
                month: i128::from(date.month),
                day: i128::from(date.day),
                hour: 0,
                minute: 0,
                second: 0,
                millisecond: 0,
                microsecond: 0,
                nanosecond: 0,
            })?,
            calendar_identifier,
        ))
    } else if matches!(item, Value::Object(_)) {
        temporal_plain_date_time_from_property_bag(vm, item, options)
    } else {
        let Value::String(source) = item else {
            return Err(Error::type_err(
                "Temporal.PlainDateTime input must be a String or object",
            ));
        };
        vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
        let parsed = temporal::parse_plain_date_time_string(source)
            .ok_or_else(|| Error::range("Invalid Temporal.PlainDateTime string"))?;
        temporal_from_overflow(vm, options)?;
        Ok((
            temporal_plain_date_time_fields_from_iso(parsed.fields)?,
            parsed.calendar_identifier,
        ))
    }
}

fn temporal_plain_date_time_from(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) =
        to_temporal_plain_date_time(vm, args.first().unwrap_or(&Value::Undefined), args.get(1))?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_date_time_in_realm(vm, fields, calendar_identifier, realm)
}

fn temporal_plain_date_time_compare(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let (one, _) =
        to_temporal_plain_date_time(vm, args.first().unwrap_or(&Value::Undefined), None)?;
    let (two, _) = to_temporal_plain_date_time(vm, args.get(1).unwrap_or(&Value::Undefined), None)?;
    let one = (
        one.year,
        one.month,
        one.day,
        one.hour,
        one.minute,
        one.second,
        one.millisecond,
        one.microsecond,
        one.nanosecond,
    );
    let two = (
        two.year,
        two.month,
        two.day,
        two.hour,
        two.minute,
        two.second,
        two.millisecond,
        two.microsecond,
        two.nanosecond,
    );
    let result = match one.cmp(&two) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    };
    Ok(Value::Number(result))
}

fn temporal_plain_date_time_equals(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (fields, calendar_identifier) = temporal_plain_date_time_slots(vm, this)?;
    let (other_fields, other_calendar) =
        to_temporal_plain_date_time(vm, args.first().unwrap_or(&Value::Undefined), None)?;
    Ok(Value::Bool(
        fields == other_fields && temporal_calendar_equals(&calendar_identifier, &other_calendar),
    ))
}

fn temporal_time_zone_from_value(vm: &mut Vm, value: Value) -> error::Result<TemporalTimeZone> {
    if let Some((_, time_zone, _)) = temporal_zoned_date_time_slots_if_present(vm, &value) {
        return Ok(time_zone);
    }
    let Value::String(source) = value else {
        return Err(Error::type_err(
            "Temporal timeZone must be a String or ZonedDateTime",
        ));
    };
    vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
    let (identifier, offset_minutes) = temporal::parse_time_zone_identifier_like(&source)
        .ok_or_else(|| Error::range("Invalid Temporal time zone identifier"))?;
    Ok(temporal_time_zone_from_identifier(
        identifier,
        offset_minutes,
    ))
}

fn temporal_time_zone_from_identifier(
    identifier: Arc<str>,
    offset_minutes: i16,
) -> TemporalTimeZone {
    TemporalTimeZone {
        kind: if identifier.as_ref() == "UTC" {
            TemporalTimeZoneKind::Utc
        } else {
            TemporalTimeZoneKind::FixedOffset(offset_minutes)
        },
        identifier,
    }
}

fn temporal_zoned_date_time_property_fields(
    vm: &mut Vm,
    item: &Value,
) -> error::Result<TemporalZonedDateTimePropertyFields> {
    vm.try_reserve_value_roots(std::slice::from_ref(item))?;
    let item_pins = vm.pin(item);
    let result = (|| {
        let calendar_value = vm.get_property(item, "calendar")?;
        let calendar_identifier = temporal_calendar_from_value(vm, calendar_value)?;
        let day = match vm.get_property(item, "day")? {
            Value::Undefined => None,
            value => Some(temporal_positive_integer_with_truncation(vm, value)?),
        };
        let numeric_or_zero = |vm: &mut Vm, name: &str| -> error::Result<BigInt> {
            match vm.get_property(item, name)? {
                Value::Undefined => Ok(BigInt::zero()),
                value => temporal_integer_with_truncation(vm, value),
            }
        };
        let hour = numeric_or_zero(vm, "hour")?;
        let microsecond = numeric_or_zero(vm, "microsecond")?;
        let millisecond = numeric_or_zero(vm, "millisecond")?;
        let minute = numeric_or_zero(vm, "minute")?;
        let month = match vm.get_property(item, "month")? {
            Value::Undefined => None,
            value => Some(temporal_positive_integer_with_truncation(vm, value)?),
        };
        let month_code = match vm.get_property(item, "monthCode")? {
            Value::Undefined => None,
            value => Some(temporal_month_code(vm, value)?),
        };
        let nanosecond = numeric_or_zero(vm, "nanosecond")?;
        let offset_nanoseconds = match vm.get_property(item, "offset")? {
            Value::Undefined => None,
            value => {
                let source = temporal_string_primitive(vm, value, "offset")?;
                Some(
                    temporal::parse_offset_string(&source)
                        .ok_or_else(|| Error::range("Invalid Temporal offset string"))?,
                )
            }
        };
        let second = numeric_or_zero(vm, "second")?;
        let time_zone_value = vm.get_property(item, "timeZone")?;
        let time_zone = if time_zone_value.is_undefined() {
            None
        } else {
            Some(temporal_time_zone_from_value(vm, time_zone_value)?)
        };
        let year = match vm.get_property(item, "year")? {
            Value::Undefined => None,
            value => Some(temporal_integer_with_truncation(vm, value)?),
        };
        Ok(TemporalZonedDateTimePropertyFields {
            year,
            month,
            month_code,
            day,
            hour,
            minute,
            second,
            millisecond,
            microsecond,
            nanosecond,
            offset_nanoseconds,
            time_zone,
            calendar_identifier,
        })
    })();
    vm.unpin_many(item_pins);
    result
}

fn temporal_regulate_field(
    value: BigInt,
    maximum: i128,
    overflow: TemporalOverflow,
) -> error::Result<i128> {
    let maximum = BigInt::from(maximum);
    if value >= BigInt::zero() && value <= maximum {
        return value
            .to_i128()
            .ok_or_else(|| Error::range("Temporal field is out of range"));
    }
    match overflow {
        TemporalOverflow::Constrain if value < BigInt::zero() => Ok(0),
        TemporalOverflow::Constrain => maximum
            .to_i128()
            .ok_or_else(|| Error::range("Temporal field is out of range")),
        TemporalOverflow::Reject => Err(Error::range("Temporal field is out of range")),
    }
}

fn temporal_zoned_date_time_from_property_bag(
    vm: &mut Vm,
    item: &Value,
    options: Option<&Value>,
) -> error::Result<(Arc<BigInt>, TemporalTimeZone, Arc<str>)> {
    let fields = temporal_zoned_date_time_property_fields(vm, item)?;
    let options = temporal_zoned_date_time_from_options(vm, options)?;
    temporal_zoned_date_time_from_property_fields(fields, options)
}

fn temporal_zoned_date_time_from_property_fields(
    fields: TemporalZonedDateTimePropertyFields,
    options: TemporalZonedDateTimeFromOptions,
) -> error::Result<(Arc<BigInt>, TemporalTimeZone, Arc<str>)> {
    let year = fields
        .year
        .as_ref()
        .ok_or_else(|| Error::type_err("Temporal property bag requires year"))?;
    if fields.month.is_none() && fields.month_code.is_none() {
        return Err(Error::type_err(
            "Temporal property bag requires month or monthCode",
        ));
    }
    let mut month = match (&fields.month, fields.month_code) {
        (Some(month), _) => month.clone(),
        (None, Some((month_code, _))) => BigInt::from(month_code),
        (None, None) => unreachable!("month or monthCode was checked"),
    };
    let day = fields
        .day
        .as_ref()
        .ok_or_else(|| Error::type_err("Temporal property bag requires day"))?;
    let year = year
        .to_i128()
        .ok_or_else(|| Error::range("Temporal year is out of range"))?;
    if let Some((month_code, leap)) = fields.month_code {
        if leap || !(1..=12).contains(&month_code) {
            return Err(Error::range("Invalid monthCode for ISO 8601 calendar"));
        }
        if fields.month.is_some() && month != BigInt::from(month_code) {
            return Err(Error::range("month and monthCode do not agree"));
        }
        month = BigInt::from(month_code);
    }
    let month = match options.overflow {
        TemporalOverflow::Constrain if month > BigInt::from(12) => BigInt::from(12),
        TemporalOverflow::Constrain => month,
        TemporalOverflow::Reject if month > BigInt::from(12) => {
            return Err(Error::range("Temporal month is out of range"))
        }
        TemporalOverflow::Reject => month,
    }
    .to_i128()
    .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    let maximum_day = temporal::days_in_month(year, month)
        .ok_or_else(|| Error::range("Temporal month is out of range"))?;
    let day = match options.overflow {
        TemporalOverflow::Constrain if day > &BigInt::from(maximum_day) => maximum_day,
        TemporalOverflow::Constrain => day
            .to_i128()
            .ok_or_else(|| Error::range("Temporal day is out of range"))?,
        TemporalOverflow::Reject if day > &BigInt::from(maximum_day) => {
            return Err(Error::range("Temporal day is out of range"))
        }
        TemporalOverflow::Reject => day
            .to_i128()
            .ok_or_else(|| Error::range("Temporal day is out of range"))?,
    };
    let hour = temporal_regulate_field(fields.hour, 23, options.overflow)?;
    let minute = temporal_regulate_field(fields.minute, 59, options.overflow)?;
    let second = temporal_regulate_field(fields.second, 59, options.overflow)?;
    let millisecond = temporal_regulate_field(fields.millisecond, 999, options.overflow)?;
    let microsecond = temporal_regulate_field(fields.microsecond, 999, options.overflow)?;
    let nanosecond = temporal_regulate_field(fields.nanosecond, 999, options.overflow)?;
    let local_nanoseconds =
        temporal::iso_date_time_to_local_nanoseconds(temporal::IsoDateTimeFields {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
            microsecond,
            nanosecond,
        })
        .ok_or_else(|| Error::range("Temporal date-time is out of range"))?;
    let time_zone = fields
        .time_zone
        .clone()
        .ok_or_else(|| Error::type_err("Temporal property bag requires timeZone"))?;
    let time_zone_offset_minutes = match &time_zone.kind {
        TemporalTimeZoneKind::Utc => 0,
        TemporalTimeZoneKind::FixedOffset(minutes) => *minutes,
        TemporalTimeZoneKind::Named(_) => {
            return Err(Error::range("Named Temporal time zones are not available"))
        }
    };
    let epoch_nanoseconds = temporal::resolve_fixed_offset_epoch(
        local_nanoseconds,
        fields.offset_nanoseconds,
        false,
        time_zone_offset_minutes,
        options.offset,
    )
    .ok_or_else(|| Error::range("Temporal.ZonedDateTime offset does not match"))?;
    let epoch_nanoseconds = BigInt::from(epoch_nanoseconds);
    if epoch_nanoseconds.abs() > temporal_instant_limit_nanoseconds() {
        return Err(Error::range(
            "Temporal.ZonedDateTime epoch nanoseconds out of range",
        ));
    }
    Ok((
        Arc::new(epoch_nanoseconds),
        time_zone,
        fields.calendar_identifier,
    ))
}

fn temporal_zoned_date_time_from_options(
    vm: &mut Vm,
    options: Option<&Value>,
) -> error::Result<TemporalZonedDateTimeFromOptions> {
    let options = options.cloned().unwrap_or(Value::Undefined);
    if !matches!(options, Value::Undefined | Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.ZonedDateTime.from options must be an object",
        ));
    }
    vm.try_reserve_value_roots(std::slice::from_ref(&options))?;
    let pin_count = vm.pin(&options);
    let result = (|| {
        let get_option = |vm: &mut Vm, name: &str| {
            if options.is_undefined() {
                Ok(None)
            } else {
                let value = vm.get_property(&options, name)?;
                if value.is_undefined() {
                    Ok(None)
                } else {
                    temporal_option_to_string(vm, &value).map(Some)
                }
            }
        };

        match get_option(vm, "disambiguation")?
            .as_deref()
            .unwrap_or("compatible")
        {
            "compatible" | "earlier" | "later" | "reject" => {}
            _ => return Err(Error::range("Invalid Temporal disambiguation option")),
        }
        let offset = match get_option(vm, "offset")?.as_deref().unwrap_or("reject") {
            "ignore" => temporal::ZonedDateTimeOffsetOption::Ignore,
            "prefer" => temporal::ZonedDateTimeOffsetOption::Prefer,
            "reject" => temporal::ZonedDateTimeOffsetOption::Reject,
            "use" => temporal::ZonedDateTimeOffsetOption::Use,
            _ => return Err(Error::range("Invalid Temporal offset option")),
        };
        let overflow = match get_option(vm, "overflow")?
            .as_deref()
            .unwrap_or("constrain")
        {
            "constrain" => TemporalOverflow::Constrain,
            "reject" => TemporalOverflow::Reject,
            _ => return Err(Error::range("Invalid Temporal overflow option")),
        };
        Ok(TemporalZonedDateTimeFromOptions { offset, overflow })
    })();
    vm.unpin_many(pin_count);
    result
}

#[derive(Clone, Copy)]
enum TemporalZonedDateTimeField {
    Era,
    EraYear,
    Year,
    Month,
    MonthCode,
    Day,
    Hour,
    Minute,
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
    DayOfWeek,
    DayOfYear,
    WeekOfYear,
    YearOfWeek,
    HoursInDay,
    DaysInWeek,
    DaysInMonth,
    DaysInYear,
    MonthsInYear,
    InLeapYear,
    OffsetNanoseconds,
    Offset,
}

fn temporal_zoned_date_time_field(
    vm: &mut Vm,
    this: Option<Value>,
    field: TemporalZonedDateTimeField,
) -> error::Result<Value> {
    let (epoch_nanoseconds, time_zone, _) = temporal_zoned_date_time_slots(vm, this)?;
    let offset_nanoseconds = temporal_time_zone_offset_nanoseconds(&time_zone, &epoch_nanoseconds)?;
    if matches!(field, TemporalZonedDateTimeField::Offset) {
        let identifier = if offset_nanoseconds == 0 {
            Arc::from("+00:00")
        } else {
            time_zone.identifier
        };
        return Ok(Value::String(identifier));
    }
    if matches!(field, TemporalZonedDateTimeField::OffsetNanoseconds) {
        return Ok(Value::Number(offset_nanoseconds as f64));
    }
    if matches!(
        field,
        TemporalZonedDateTimeField::Era | TemporalZonedDateTimeField::EraYear
    ) {
        return Ok(Value::Undefined);
    }

    let date_time = temporal::iso_date_time(&epoch_nanoseconds, offset_nanoseconds)
        .ok_or_else(|| Error::range("Temporal.ZonedDateTime local date is out of range"))?;
    let number = match field {
        TemporalZonedDateTimeField::Year => date_time.year,
        TemporalZonedDateTimeField::Month => date_time.month,
        TemporalZonedDateTimeField::Day => date_time.day,
        TemporalZonedDateTimeField::Hour => date_time.hour,
        TemporalZonedDateTimeField::Minute => date_time.minute,
        TemporalZonedDateTimeField::Second => date_time.second,
        TemporalZonedDateTimeField::Millisecond => date_time.millisecond,
        TemporalZonedDateTimeField::Microsecond => date_time.microsecond,
        TemporalZonedDateTimeField::Nanosecond => date_time.nanosecond,
        TemporalZonedDateTimeField::DayOfWeek => temporal::iso_day_of_week(date_time.epoch_days),
        TemporalZonedDateTimeField::DayOfYear => temporal::iso_day_of_year(date_time)
            .ok_or_else(|| Error::range("Temporal dayOfYear is out of range"))?,
        TemporalZonedDateTimeField::WeekOfYear => {
            temporal::iso_week_of_year(date_time)
                .ok_or_else(|| Error::range("Temporal weekOfYear is out of range"))?
                .0
        }
        TemporalZonedDateTimeField::YearOfWeek => {
            temporal::iso_week_of_year(date_time)
                .ok_or_else(|| Error::range("Temporal yearOfWeek is out of range"))?
                .1
        }
        TemporalZonedDateTimeField::DaysInWeek => 7,
        TemporalZonedDateTimeField::DaysInMonth => {
            temporal::days_in_month(date_time.year, date_time.month)
                .ok_or_else(|| Error::range("Temporal daysInMonth is out of range"))?
        }
        TemporalZonedDateTimeField::DaysInYear => {
            if temporal::leap_year(date_time.year) {
                366
            } else {
                365
            }
        }
        TemporalZonedDateTimeField::MonthsInYear => 12,
        TemporalZonedDateTimeField::HoursInDay => {
            let nanoseconds_per_day = BigInt::from(86_400_000_000_000_i64);
            let offset = BigInt::from(offset_nanoseconds);
            let today = BigInt::from(date_time.epoch_days) * &nanoseconds_per_day - &offset;
            let tomorrow = &today + &nanoseconds_per_day;
            let limit = temporal_instant_limit_nanoseconds();
            if today.abs() > limit || tomorrow.abs() > limit {
                return Err(Error::range("Temporal hoursInDay boundary is out of range"));
            }
            24
        }
        TemporalZonedDateTimeField::InLeapYear => {
            return Ok(Value::Bool(temporal::leap_year(date_time.year)))
        }
        TemporalZonedDateTimeField::MonthCode => {
            return Ok(Value::String(Arc::from(format!("M{:02}", date_time.month))))
        }
        TemporalZonedDateTimeField::Era
        | TemporalZonedDateTimeField::EraYear
        | TemporalZonedDateTimeField::OffsetNanoseconds
        | TemporalZonedDateTimeField::Offset => unreachable!(),
    };
    Ok(Value::Number(number as f64))
}

macro_rules! temporal_zoned_date_time_getter {
    ($name:ident, $field:ident) => {
        fn $name(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
            temporal_zoned_date_time_field(vm, this, TemporalZonedDateTimeField::$field)
        }
    };
}

temporal_zoned_date_time_getter!(temporal_zoned_date_time_era, Era);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_era_year, EraYear);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_year, Year);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_month, Month);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_month_code, MonthCode);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_day, Day);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_hour, Hour);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_minute, Minute);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_second, Second);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_millisecond, Millisecond);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_microsecond, Microsecond);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_nanosecond, Nanosecond);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_day_of_week, DayOfWeek);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_day_of_year, DayOfYear);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_week_of_year, WeekOfYear);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_year_of_week, YearOfWeek);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_hours_in_day, HoursInDay);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_days_in_week, DaysInWeek);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_days_in_month, DaysInMonth);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_days_in_year, DaysInYear);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_months_in_year, MonthsInYear);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_in_leap_year, InLeapYear);
temporal_zoned_date_time_getter!(
    temporal_zoned_date_time_offset_nanoseconds,
    OffsetNanoseconds
);
temporal_zoned_date_time_getter!(temporal_zoned_date_time_offset, Offset);

fn temporal_zoned_date_time_to_instant(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (epoch_nanoseconds, _, _) = temporal_zoned_date_time_slots(vm, this)?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_instant_in_realm(vm, epoch_nanoseconds, realm)
}

fn temporal_zoned_date_time_to_plain_date_time(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (epoch_nanoseconds, time_zone, calendar_identifier) =
        temporal_zoned_date_time_slots(vm, this)?;
    let fields = temporal_zoned_date_time_plain_fields(&epoch_nanoseconds, &time_zone)?;
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_plain_date_time_in_realm(vm, fields, calendar_identifier, realm)
}

fn temporal_annotation_display(
    value: Option<&str>,
    option: &str,
    allow_always: bool,
) -> error::Result<temporal::AnnotationDisplay> {
    Ok(match value.unwrap_or("auto") {
        "auto" => temporal::AnnotationDisplay::Auto,
        "always" if allow_always => temporal::AnnotationDisplay::Always,
        "critical" => temporal::AnnotationDisplay::Critical,
        "never" => temporal::AnnotationDisplay::Never,
        _ => return Err(Error::range(format!("Invalid Temporal {option} option"))),
    })
}

fn temporal_zoned_date_time_format(
    vm: &mut Vm,
    epoch_nanoseconds: &BigInt,
    time_zone: &TemporalTimeZone,
    calendar_identifier: &str,
    options: temporal::ZonedDateTimeFormatOptions,
) -> error::Result<Value> {
    let offset_nanoseconds = temporal_time_zone_offset_nanoseconds(time_zone, epoch_nanoseconds)?;
    temporal::format_zoned_date_time(
        epoch_nanoseconds,
        offset_nanoseconds,
        &time_zone.identifier,
        calendar_identifier,
        options,
    )
    .map(Arc::<str>::from)
    .map(Value::String)
    .ok_or_else(|| Error::range("Temporal.ZonedDateTime string formatting failed"))
}

fn temporal_zoned_date_time_to_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (epoch_nanoseconds, time_zone, calendar_identifier) =
        temporal_zoned_date_time_slots(vm, this)?;
    let options = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(options, Value::Undefined | Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.ZonedDateTime.prototype.toString options must be an object",
        ));
    }
    vm.try_reserve_value_roots(std::slice::from_ref(&options))?;
    let options_pin = vm.pin(&options);
    let result = (|| {
        let get_option = |vm: &mut Vm, name: &str| {
            if options.is_undefined() {
                Ok(Value::Undefined)
            } else {
                vm.get_property(&options, name)
            }
        };
        let option_string = |vm: &mut Vm, value: Value| {
            if value.is_undefined() {
                Ok(None)
            } else {
                temporal_option_to_string(vm, &value).map(Some)
            }
        };

        let calendar_name = get_option(vm, "calendarName")?;
        let calendar_name = option_string(vm, calendar_name)?;
        let calendar_name =
            temporal_annotation_display(calendar_name.as_deref(), "calendarName", true)?;
        let fractional_second_digits = match get_option(vm, "fractionalSecondDigits")? {
            Value::Undefined => InstantFractionalSecondDigits::Auto,
            Value::Number(number) => InstantFractionalSecondDigits::Number(number),
            value => InstantFractionalSecondDigits::String(temporal_option_to_string(vm, &value)?),
        };
        let fractional_second_digits =
            temporal_instant_fractional_second_digits(fractional_second_digits)?;
        let offset = get_option(vm, "offset")?;
        let offset = option_string(vm, offset)?;
        let show_offset = match offset.as_deref().unwrap_or("auto") {
            "auto" => true,
            "never" => false,
            _ => return Err(Error::range("Invalid Temporal offset option")),
        };
        let rounding_mode = get_option(vm, "roundingMode")?;
        let rounding_mode = option_string(vm, rounding_mode)?;
        let rounding_mode = temporal_instant_rounding_mode(rounding_mode.as_deref())?;
        let smallest_unit = get_option(vm, "smallestUnit")?;
        let smallest_unit = option_string(vm, smallest_unit)?;
        let smallest_unit = temporal_instant_smallest_unit(smallest_unit.as_deref())?;
        let time_zone_name = get_option(vm, "timeZoneName")?;
        let time_zone_name = option_string(vm, time_zone_name)?;
        let time_zone_name =
            temporal_annotation_display(time_zone_name.as_deref(), "timeZoneName", false)?;
        let precision = temporal_instant_precision(fractional_second_digits, smallest_unit)?;
        temporal_zoned_date_time_format(
            vm,
            &epoch_nanoseconds,
            &time_zone,
            &calendar_identifier,
            temporal::ZonedDateTimeFormatOptions {
                precision,
                rounding_mode,
                show_offset,
                time_zone_name,
                calendar_name,
            },
        )
    })();
    vm.unpin_many(options_pin);
    result
}

fn temporal_zoned_date_time_to_json(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (epoch_nanoseconds, time_zone, calendar_identifier) =
        temporal_zoned_date_time_slots(vm, this)?;
    temporal_zoned_date_time_format(
        vm,
        &epoch_nanoseconds,
        &time_zone,
        &calendar_identifier,
        temporal::ZonedDateTimeFormatOptions {
            precision: temporal::InstantPrecision::Auto,
            rounding_mode: temporal::InstantRoundingMode::Trunc,
            show_offset: true,
            time_zone_name: temporal::AnnotationDisplay::Auto,
            calendar_name: temporal::AnnotationDisplay::Auto,
        },
    )
}

fn temporal_zoned_date_time_value_of(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Err(Error::type_err(
        "Temporal.ZonedDateTime.prototype.valueOf always throws",
    ))
}

fn temporal_instant_factory_result(
    vm: &mut Vm,
    epoch_nanoseconds: Arc<BigInt>,
) -> error::Result<Value> {
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    create_temporal_instant_in_realm(vm, epoch_nanoseconds, realm)
}

fn temporal_instant_from_epoch_milliseconds(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let milliseconds = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    let milliseconds = Vm::number_to_bigint_exact(milliseconds).ok_or_else(|| {
        Error::range("Temporal.Instant epoch milliseconds must be an integral Number")
    })?;
    temporal_instant_factory_result(vm, Arc::new(milliseconds * BigInt::from(1_000_000_i64)))
}

fn temporal_instant_from_epoch_nanoseconds(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let epoch_nanoseconds = vm.coerce_bigint_shared(args.first().unwrap_or(&Value::Undefined))?;
    temporal_instant_factory_result(vm, epoch_nanoseconds)
}

fn to_temporal_instant_epoch(vm: &mut Vm, value: &Value) -> error::Result<Arc<BigInt>> {
    if let Value::Object(index) = value {
        if let Some(epoch_nanoseconds) = vm.heap.with_obj(index.0, |object| match object {
            HeapObj::Temporal(TemporalData { kind, .. }) => match kind {
                TemporalKind::Instant { epoch_nanoseconds }
                | TemporalKind::ZonedDateTime {
                    epoch_nanoseconds, ..
                } => Some(epoch_nanoseconds.clone()),
                TemporalKind::Duration { .. }
                | TemporalKind::PlainDate { .. }
                | TemporalKind::PlainMonthDay { .. }
                | TemporalKind::PlainTime { .. }
                | TemporalKind::PlainDateTime { .. }
                | TemporalKind::PlainYearMonth { .. } => None,
            },
            _ => None,
        }) {
            return Ok(epoch_nanoseconds);
        }
    }
    let primitive = if matches!(value, Value::Object(_)) {
        vm.to_primitive_hint(value, true)?
    } else {
        value.clone()
    };
    let Value::String(source) = primitive else {
        return Err(Error::type_err("Temporal.Instant input must be a String"));
    };
    vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
    let epoch_nanoseconds = temporal::parse_instant_string(&source)
        .ok_or_else(|| Error::range("Invalid Temporal.Instant string"))?;
    if epoch_nanoseconds.abs() > temporal_instant_limit_nanoseconds() {
        return Err(Error::range(
            "Temporal.Instant epoch nanoseconds out of range",
        ));
    }
    Ok(Arc::new(epoch_nanoseconds))
}

fn temporal_instant_from(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let epoch_nanoseconds =
        to_temporal_instant_epoch(vm, args.first().unwrap_or(&Value::Undefined))?;
    temporal_instant_factory_result(vm, epoch_nanoseconds)
}

fn temporal_instant_compare(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let one = to_temporal_instant_epoch(vm, args.first().unwrap_or(&Value::Undefined))?;
    let two = to_temporal_instant_epoch(vm, args.get(1).unwrap_or(&Value::Undefined))?;
    let result = match one.cmp(&two) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    };
    Ok(Value::Number(result))
}

fn temporal_instant_epoch_nanoseconds(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    temporal_instant_epoch(vm, this).map(Value::BigInt)
}

fn temporal_instant_equals(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let epoch_nanoseconds = temporal_instant_epoch(vm, this)?;
    let other_epoch_nanoseconds =
        to_temporal_instant_epoch(vm, args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(epoch_nanoseconds == other_epoch_nanoseconds))
}

fn temporal_instant_value_of(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Err(Error::type_err(
        "Temporal.Instant.prototype.valueOf always throws",
    ))
}

enum InstantFractionalSecondDigits {
    Auto,
    Number(f64),
    String(Arc<str>),
}

#[derive(Clone, Copy)]
enum InstantSmallestUnit {
    DateOrHour,
    Minute,
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
}

fn temporal_option_to_string(vm: &mut Vm, value: &Value) -> error::Result<Arc<str>> {
    vm.try_reserve_value_roots(std::slice::from_ref(value))?;
    let pin_count = vm.pin(value);
    let result = vm.to_string(value).and_then(|source| {
        vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
        Ok(source)
    });
    vm.unpin_many(pin_count);
    result
}

fn temporal_instant_rounding_mode(
    value: Option<&str>,
) -> error::Result<temporal::InstantRoundingMode> {
    Ok(match value.unwrap_or("trunc") {
        "ceil" => temporal::InstantRoundingMode::Ceil,
        "expand" => temporal::InstantRoundingMode::Expand,
        "floor" => temporal::InstantRoundingMode::Floor,
        "halfCeil" => temporal::InstantRoundingMode::HalfCeil,
        "halfEven" => temporal::InstantRoundingMode::HalfEven,
        "halfExpand" => temporal::InstantRoundingMode::HalfExpand,
        "halfFloor" => temporal::InstantRoundingMode::HalfFloor,
        "halfTrunc" => temporal::InstantRoundingMode::HalfTrunc,
        "trunc" => temporal::InstantRoundingMode::Trunc,
        _ => return Err(Error::range("Invalid Temporal roundingMode option")),
    })
}

fn temporal_instant_fractional_second_digits(
    fractional_second_digits: InstantFractionalSecondDigits,
) -> error::Result<Option<u8>> {
    Ok(match fractional_second_digits {
        InstantFractionalSecondDigits::Auto => None,
        InstantFractionalSecondDigits::Number(number) => {
            let number = number.floor();
            if !number.is_finite() || !(0.0..=9.0).contains(&number) {
                return Err(Error::range(
                    "Invalid Temporal fractionalSecondDigits option",
                ));
            }
            Some(number as u8)
        }
        InstantFractionalSecondDigits::String(value) if value.as_ref() == "auto" => None,
        InstantFractionalSecondDigits::String(_) => {
            return Err(Error::range(
                "Invalid Temporal fractionalSecondDigits option",
            ));
        }
    })
}

fn temporal_instant_smallest_unit(
    value: Option<&str>,
) -> error::Result<Option<InstantSmallestUnit>> {
    let Some(unit) = value else {
        return Ok(None);
    };
    Ok(Some(match unit {
        "auto" | "year" | "years" | "month" | "months" | "week" | "weeks" | "day" | "days"
        | "hour" | "hours" => InstantSmallestUnit::DateOrHour,
        "minute" | "minutes" => InstantSmallestUnit::Minute,
        "second" | "seconds" => InstantSmallestUnit::Second,
        "millisecond" | "milliseconds" => InstantSmallestUnit::Millisecond,
        "microsecond" | "microseconds" => InstantSmallestUnit::Microsecond,
        "nanosecond" | "nanoseconds" => InstantSmallestUnit::Nanosecond,
        _ => return Err(Error::range("Invalid Temporal smallestUnit option")),
    }))
}

fn temporal_instant_precision(
    digits: Option<u8>,
    smallest_unit: Option<InstantSmallestUnit>,
) -> error::Result<temporal::InstantPrecision> {
    let Some(unit) = smallest_unit else {
        return Ok(digits.map_or(
            temporal::InstantPrecision::Auto,
            temporal::InstantPrecision::Digits,
        ));
    };
    Ok(match unit {
        InstantSmallestUnit::DateOrHour => {
            return Err(Error::range("Invalid Temporal smallestUnit option"));
        }
        InstantSmallestUnit::Minute => temporal::InstantPrecision::Minute,
        InstantSmallestUnit::Second => temporal::InstantPrecision::Digits(0),
        InstantSmallestUnit::Millisecond => temporal::InstantPrecision::Digits(3),
        InstantSmallestUnit::Microsecond => temporal::InstantPrecision::Digits(6),
        InstantSmallestUnit::Nanosecond => temporal::InstantPrecision::Digits(9),
    })
}

fn temporal_instant_to_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let epoch_nanoseconds = temporal_instant_epoch(vm, this)?;
    let options = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(options, Value::Undefined | Value::Object(_)) {
        return Err(Error::type_err(
            "Temporal.Instant.prototype.toString options must be an object",
        ));
    }

    vm.try_reserve_value_roots(std::slice::from_ref(&options))?;
    let options_pin = vm.pin(&options);
    let result = (|| {
        let get_option = |vm: &mut Vm, name: &str| {
            if options.is_undefined() {
                Ok(Value::Undefined)
            } else {
                vm.get_property(&options, name)
            }
        };

        let fractional_second_digits = match get_option(vm, "fractionalSecondDigits")? {
            Value::Undefined => InstantFractionalSecondDigits::Auto,
            Value::Number(number) => InstantFractionalSecondDigits::Number(number),
            value => InstantFractionalSecondDigits::String(temporal_option_to_string(vm, &value)?),
        };
        let fractional_second_digits =
            temporal_instant_fractional_second_digits(fractional_second_digits)?;
        let rounding_mode_value = get_option(vm, "roundingMode")?;
        let rounding_mode = if rounding_mode_value.is_undefined() {
            None
        } else {
            Some(temporal_option_to_string(vm, &rounding_mode_value)?)
        };
        let rounding_mode = temporal_instant_rounding_mode(rounding_mode.as_deref())?;
        let smallest_unit_value = get_option(vm, "smallestUnit")?;
        let smallest_unit = if smallest_unit_value.is_undefined() {
            None
        } else {
            Some(temporal_option_to_string(vm, &smallest_unit_value)?)
        };
        let smallest_unit = temporal_instant_smallest_unit(smallest_unit.as_deref())?;
        let time_zone = get_option(vm, "timeZone")?;

        let precision = temporal_instant_precision(fractional_second_digits, smallest_unit)?;
        let display_offset = match time_zone {
            Value::Undefined => None,
            Value::String(source) => {
                vm.consume_fuel_units(source.len().min(i64::MAX as usize) as i64)?;
                Some(
                    temporal::parse_time_zone_offset(&source)
                        .ok_or_else(|| Error::range("Invalid Temporal timeZone option"))?,
                )
            }
            _ => {
                return Err(Error::type_err(
                    "Temporal timeZone option must be a String or undefined",
                ));
            }
        };
        temporal::format_instant(&epoch_nanoseconds, display_offset, precision, rounding_mode)
            .map(Arc::<str>::from)
            .map(Value::String)
            .ok_or_else(|| Error::range("Temporal.Instant string formatting failed"))
    })();
    vm.unpin_many(options_pin);
    result
}

fn temporal_instant_epoch_milliseconds(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let epoch_nanoseconds = temporal_instant_epoch(vm, this)?;
    let milliseconds = epoch_nanoseconds
        .as_ref()
        .div_floor(&BigInt::from(1_000_000_i64));
    let number = milliseconds
        .to_f64()
        .ok_or_else(|| Error::range("Temporal.Instant milliseconds out of range"))?;
    Ok(Value::Number(number))
}

fn populate_secondary_realm(vm: &mut Vm, realm_env: GcIdx) -> error::Result<Value> {
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
    define_realm_global_const(vm, realm_env, &global, "undefined", Value::Undefined);
    define_realm_global_const(vm, realm_env, &global, "NaN", Value::Number(f64::NAN));
    define_realm_global_const(
        vm,
        realm_env,
        &global,
        "Infinity",
        Value::Number(f64::INFINITY),
    );

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
    let parse_float_idx =
        vm.new_native_function_in_env("parseFloat", global_parse_float, 1, realm_env)?;
    define_realm_global(
        vm,
        realm_env,
        &global,
        "parseFloat",
        Value::Object(parse_float_idx),
    );
    let is_nan_idx = vm.new_native_function_in_env("isNaN", global_is_nan, 1, realm_env)?;
    define_realm_global(vm, realm_env, &global, "isNaN", Value::Object(is_nan_idx));
    let is_finite_idx =
        vm.new_native_function_in_env("isFinite", global_is_finite, 1, realm_env)?;
    define_realm_global(
        vm,
        realm_env,
        &global,
        "isFinite",
        Value::Object(is_finite_idx),
    );
    let mut uri_functions = Vec::new();
    for (name, function) in [
        ("escape", global_escape as NativeFn),
        ("unescape", global_unescape as NativeFn),
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
        let index = vm.new_native_function_in_env(name, function, 1, realm_env)?;
        define_realm_global(vm, realm_env, &global, name, Value::Object(index));
        uri_functions.push(index);
    }
    let realm_function_proto_idx =
        vm.new_native_function_in_env("", function_proto_noop, 0, realm_env)?;
    let realm_function_proto = Value::Object(realm_function_proto_idx);
    vm.heap.with_obj(realm_function_proto_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            *f.proto.lock() = None;
        }
    });
    vm.realm_function_prototypes
        .insert(realm_env.0, realm_function_proto.clone());
    for function in [
        eval_idx,
        parse_int_idx,
        parse_float_idx,
        is_nan_idx,
        is_finite_idx,
    ]
    .into_iter()
    .chain(uri_functions)
    {
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
            PropertyKey::symbol(vm.well_known_symbols.has_instance),
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

    let realm_json = build_json_in_env(vm, realm_env, realm_object_prototype.clone())?;
    define_realm_global(vm, realm_env, &global, "JSON", realm_json);

    let realm_intl = intl::build_intl_in_env(vm, realm_env, realm_object_prototype.clone())?;
    define_realm_global(vm, realm_env, &global, "Intl", realm_intl);

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
        "SuppressedError",
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
    setup_map_iterator_proto_in_env(vm, realm_env, realm_iterator_proto.clone())?;
    setup_set_iterator_proto_in_env(vm, realm_env, realm_iterator_proto.clone())?;
    install_map_intrinsic_in_env(vm, realm_env, Some(&global))?;
    install_set_intrinsic_in_env(vm, realm_env, Some(&global))?;
    install_weakmap_intrinsic_in_env(vm, realm_env, Some(&global))?;
    install_weakset_intrinsic_in_env(vm, realm_env, Some(&global))?;
    let (str_ctor, str_proto) = make_builtin_constructor_with_in_env(
        vm,
        "String",
        1,
        string_constructor,
        NativeConstructMode::InternalDeferredPrototype,
        &[
            ("valueOf", string_value_of, 0),
            ("toString", string_proto_to_string, 0),
            ("localeCompare", str_locale_compare, 1),
            ("substr", str_substr, 2),
            ("trimStart", str_trim_start, 0),
            ("trimEnd", str_trim_end, 0),
        ],
        realm_env,
    )?;
    install_annex_b_string_methods_in_env(vm, realm_env, str_proto)?;
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
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
        PropertyKey::symbol(vm.well_known_symbols.to_primitive),
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
        PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
        proto: Mutex::new(Some(object_proto.clone())),
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

    install_temporal_namespace_in_env(vm, realm_env, Some(&global), object_proto.clone())?;

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
    install_shadow_realm_intrinsic_in_env(vm, realm_env, Some(&global))?;
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
            ("getYear", date_get_component, 0),
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
            ("setYear", date_set_component, 1),
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

        let to_gmt_string = vm.heap.with_obj(prototype.0, |object| {
            object
                .props()
                .lock()
                .get(&PropertyKey::from("toUTCString"))
                .map(|descriptor| descriptor.value.clone())
        });

        vm.heap.with_obj(prototype.0, |object| {
            let mut props = object.props().lock();
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.to_primitive),
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
            if let Some(to_gmt_string) = to_gmt_string {
                props.insert(PropertyKey::from("toGMTString"), data_prop(to_gmt_string));
            }
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

fn shadow_realm_slot_key() -> crate::value::PrivateSlotKey {
    crate::value::PrivateSlotKey::Internal(Arc::from("[[ShadowRealm]]"))
}

fn shadow_realm_environment(vm: &Vm, receiver: &Value) -> error::Result<GcIdx> {
    let Value::Object(index) = receiver else {
        return Err(Error::type_err(
            "ShadowRealm method called on incompatible receiver",
        ));
    };
    match vm
        .heap
        .get_private_element(index.0, &shadow_realm_slot_key())
    {
        Some(crate::value::PrivateSlot::Value(Value::Object(environment))) => Ok(environment),
        _ => Err(Error::type_err(
            "ShadowRealm method called on incompatible receiver",
        )),
    }
}

fn shadow_realm_constructor(
    vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("ShadowRealm constructor requires 'new'"));
    }
    let constructor_realm = crate::environment::global_env_root(
        &vm.heap,
        vm.native_callee_closure().unwrap_or(vm.global),
    );
    let fallback = vm
        .realm_shadow_realm_prototypes
        .get(&constructor_realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing ShadowRealm prototype"))?;
    let prototype = native_constructor_prototype_with_default(vm, "ShadowRealm", fallback)?;
    let instance = new_object_with_prototype(vm, prototype)?;
    let instance_pin = vm.pin(&instance);
    let module_referrer = vm
        .frames
        .iter()
        .rev()
        .find_map(|frame| frame.chunk.source_path.clone());
    make_realm_transaction(
        vm,
        module_referrer,
        None,
        |_| {},
        |vm, realm_env, _| {
            let Value::Object(index) = &instance else {
                return Err(Error::internal("ShadowRealm receiver is not an object"));
            };
            vm.heap.with_private_elements(index.0, |slots| {
                slots.insert(
                    shadow_realm_slot_key(),
                    crate::value::PrivateSlot::Value(Value::Object(realm_env)),
                );
            });
            Ok(instance.clone())
        },
    )
    .inspect(|_| vm.unpin_many(instance_pin))
    .inspect_err(|_| vm.unpin_many(instance_pin))
}

fn shadow_realm_evaluate(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let eval_realm = shadow_realm_environment(vm, &receiver)?;
    let source = match args.first().cloned().unwrap_or(Value::Undefined) {
        Value::String(source) => source,
        _ => {
            return Err(Error::type_err(
                "ShadowRealm evaluate source must be a String",
            ))
        }
    };
    let caller_realm = crate::environment::global_env_root(
        &vm.heap,
        vm.native_callee_closure().unwrap_or(vm.global),
    );
    // Parsing belongs to the caller Realm. Once execution starts, every
    // catchable abrupt completion is replaced at the membrane boundary.
    let program = crate::parser::Parser::parse_internal(&source)?;
    let global = vm.realm_global_for_env(eval_realm);
    let result = vm.eval_shadowrealm_program_in(eval_realm, global, program);
    let value = match result {
        Ok(value) => value,
        Err(error) if Vm::preserve_shadowrealm_host_error(&error) => return Err(error),
        Err(_) => return Err(Error::type_err("ShadowRealm evaluation failed")),
    };
    match vm.shadowrealm_wrap_value(caller_realm, value) {
        Ok(value) => Ok(value),
        Err(error) if Vm::preserve_shadowrealm_host_error(&error) => Err(error),
        Err(_) => Err(Error::type_err(
            "ShadowRealm result cannot cross the boundary",
        )),
    }
}

fn shadow_realm_import_value(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let eval_realm = shadow_realm_environment(vm, &receiver)?;
    let specifier = vm
        .to_string(args.first().unwrap_or(&Value::Undefined))?
        .to_string();
    let Some(Value::String(export_name)) = args.get(1) else {
        return Err(Error::type_err(
            "ShadowRealm importValue export name must be a String",
        ));
    };

    let caller_realm = crate::environment::global_env_root(
        &vm.heap,
        vm.native_callee_closure().unwrap_or(vm.global),
    );
    let referrer = vm.with_realm_record(eval_realm, |record| record.module_referrer.lock().clone());

    // RealmImportValue first gives HostImportModuleDynamically an eval-Realm
    // capability. A separate internal reaction later crosses only the selected
    // export into the caller-Realm capability.
    let inner_constructor = vm.promise_constructor_for_env(eval_realm);
    let inner = new_promise_capability_in_env(vm, inner_constructor, eval_realm)?;
    let inner_promise = match inner.promise.clone() {
        Value::Object(promise) => promise,
        _ => {
            return Err(Error::internal(
                "Promise capability did not create an object",
            ))
        }
    };
    let inner_pins = vm.pin_many(&[
        Value::Object(inner_promise),
        inner.resolve.clone(),
        inner.reject.clone(),
    ]);
    let host_load = if let Some(referrer) = referrer {
        vm.microtask_queue
            .push_back(crate::vm::Microtask::DynamicImport {
                promise: inner_promise,
                resolve: inner.resolve.clone(),
                reject: inner.reject.clone(),
                realm: eval_realm,
                referrer,
                specifier: specifier.into(),
                import_type: None,
            });
        Ok(())
    } else {
        (|| -> error::Result<()> {
            let error = Error::type_err("ShadowRealm importValue requires a source-file referrer");
            let reason = vm.make_error_value_in_realm(&error, eval_realm)?;
            let reason_pin = vm.pin(&reason);
            let result = vm
                .call_function(
                    &inner.reject,
                    std::slice::from_ref(&reason),
                    Some(Value::Undefined),
                )
                .map(|_| ());
            vm.unpin_many(reason_pin);
            result
        })()
    };
    if let Err(error) = host_load {
        vm.unpin_many(inner_pins);
        return Err(error);
    }

    let outer_constructor = vm.promise_constructor_for_env(caller_realm);
    let outer = match new_promise_capability_in_env(vm, outer_constructor, caller_realm) {
        Ok(capability) => capability,
        Err(error) => {
            vm.unpin_many(inner_pins);
            return Err(error);
        }
    };
    let outer_promise = match outer.promise.clone() {
        Value::Object(promise) => promise,
        _ => {
            vm.unpin_many(inner_pins);
            return Err(Error::internal(
                "Promise capability did not create an object",
            ));
        }
    };
    let outer_pins = vm.pin_many(&[
        Value::Object(outer_promise),
        outer.resolve.clone(),
        outer.reject.clone(),
    ]);
    let continuation = Some(crate::value::PromiseContinuation::ShadowRealmImportValue {
        export_name: export_name.clone(),
        capability: crate::value::PromiseReactionCapability {
            promise: Value::Object(outer_promise),
            resolve: outer.resolve,
            reject: outer.reject,
        },
        caller_realm,
    });
    let state = vm.heap.with_obj(inner_promise.0, |object| {
        if let HeapObj::Promise(data) = object {
            *data.state.lock()
        } else {
            crate::value::PromiseStatus::Rejected
        }
    });
    if state == crate::value::PromiseStatus::Pending {
        vm.heap.with_obj(inner_promise.0, |object| {
            if let HeapObj::Promise(data) = object {
                data.handlers.lock().push(crate::value::PromiseHandler {
                    on_fulfilled: Value::Undefined,
                    on_rejected: Value::Undefined,
                    derived: None,
                    continuation,
                });
            }
        });
    } else {
        vm.microtask_queue.push_back(crate::vm::Microtask::Then {
            promise: inner_promise,
            on_fulfilled: Value::Undefined,
            on_rejected: Value::Undefined,
            derived: None,
            continuation,
            realm: None,
        });
    }
    vm.unpin_many(outer_pins);
    vm.unpin_many(inner_pins);
    Ok(Value::Object(outer_promise))
}

fn install_shadow_realm_intrinsic_in_env(
    vm: &mut Vm,
    realm_env: GcIdx,
    realm_global: Option<&Value>,
) -> error::Result<()> {
    let realm = crate::environment::global_env_root(&vm.heap, realm_env);
    let object_prototype = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing ShadowRealm Object prototype"))?;
    let evaluate = vm.new_native_function_in_env("evaluate", shadow_realm_evaluate, 1, realm)?;
    let mut pin_count = vm.pin(&Value::Object(evaluate));
    let import_value =
        vm.new_native_function_in_env("importValue", shadow_realm_import_value, 2, realm)?;
    pin_count += vm.pin(&Value::Object(import_value));
    let prototype = Value::Object(vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(object_prototype)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("ShadowRealm")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?);
    pin_count += vm.pin(&prototype);
    let constructor = vm.new_native_constructor_in_env(
        "ShadowRealm",
        shadow_realm_constructor,
        0,
        realm,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
    pin_count += vm.pin(&Value::Object(constructor));
    vm.heap.with_obj(constructor.0, |object| {
        if let HeapObj::Function(function) = object {
            *function.prototype.lock() = Some(prototype.clone());
        }
        object.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(prototype.clone()),
        );
    });
    let Value::Object(prototype_index) = prototype else {
        unreachable!("ShadowRealm prototype must be an object");
    };
    vm.heap.with_obj(prototype_index.0, |object| {
        let mut props = object.props().lock();
        props.insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(constructor)),
        );
        props.insert(
            PropertyKey::from("evaluate"),
            data_prop(Value::Object(evaluate)),
        );
        props.insert(
            PropertyKey::from("importValue"),
            data_prop(Value::Object(import_value)),
        );
        let mut tag = data_prop(Value::String(Arc::from("ShadowRealm")));
        tag.writable = false;
        props.insert(
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
    });
    vm.realm_shadow_realm_prototypes
        .insert(realm.0, Value::Object(prototype_index));
    if let Some(global) = realm_global {
        define_realm_global(vm, realm, global, "ShadowRealm", Value::Object(constructor));
    } else {
        define_global(vm, "ShadowRealm", Value::Object(constructor));
    }
    vm.unpin_many(pin_count);
    Ok(())
}

fn make_test262_realm(vm: &mut Vm) -> error::Result<Value> {
    let module_cache = vm.module_cache_for_env(vm.native_callee_closure().unwrap_or(vm.global));
    make_realm_transaction(
        vm,
        None,
        Some(module_cache),
        |_| {},
        |vm, _, global| {
            let realm = vm.new_object()?;
            vm.heap.with_obj(realm.0, |obj| {
                obj.props()
                    .lock()
                    .insert(PropertyKey::from("global"), data_prop(global));
            });
            Ok(Value::Object(realm))
        },
    )
}

fn make_realm_transaction<T>(
    vm: &mut Vm,
    module_referrer: Option<Arc<std::path::PathBuf>>,
    module_cache: Option<crate::value::ModuleCache>,
    before_population: impl FnOnce(&mut Vm),
    finish: impl FnOnce(&mut Vm, GcIdx, Value) -> error::Result<T>,
) -> error::Result<T> {
    let pin_base = vm.gc_pins.len();
    let realm_env = crate::environment::new_env(&vm.heap, None, true)?;
    vm.attach_realm_record(realm_env, module_referrer, module_cache);
    // Realm installers use fallible, stack-disciplined temporary pins. The
    // transaction owns their entire suffix so an early return cannot retain a
    // partially initialized Realm. Pin the environment itself until published
    // functions make it reachable through the provisional registry graph.
    vm.gc_pins.push(realm_env.0);
    before_population(vm);
    let result = (|| {
        let global = populate_secondary_realm(vm, realm_env)?;
        vm.publish_realm_record(realm_env);
        finish(vm, realm_env, global)
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
    let module_cache = vm.module_cache_for_env(vm.global);
    make_realm_transaction(
        vm,
        None,
        Some(module_cache),
        |vm| vm.gc(),
        |vm, _, global| {
            let realm = vm.new_object()?;
            vm.heap.with_obj(realm.0, |obj| {
                obj.props()
                    .lock()
                    .insert(PropertyKey::from("global"), data_prop(global));
            });
            Ok(Value::Object(realm))
        },
    )
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
                .run_internal_source(&source)
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
                        .and_then(|name| namespace.exports.lock().get(name.as_ref()).cloned());
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
                        .is_some_and(|name| namespace.exports.lock().contains_key(name.as_ref()))
                    {
                        return true;
                    }
                }
                if heap_obj.props().lock().contains_key(key) {
                    return true;
                }
                if let HeapObj::Array(a) = heap_obj {
                    if key.as_str().is_some_and(|name| name == "length") {
                        return !a.is_arguments.load(Ordering::Relaxed);
                    }
                    if let Some(name) = key.as_str() {
                        if let Some(i) = crate::value::parse_array_index(&name) {
                            return a.is_dense_present(i);
                        }
                    }
                }
                if let HeapObj::Object(od) = heap_obj {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        if key.as_str().is_some_and(|name| name == "length") {
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
            if key.as_str().is_some_and(|name| name == "length") {
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
        Value::Symbol(id) => Ok(PropertyKey::symbol(id)),
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    }
}

pub(crate) fn property_key_to_value(key: &PropertyKey) -> Value {
    if let Some(id) = key.symbol_id() {
        Value::Symbol(id)
    } else {
        Value::String(
            key.string_arc()
                .expect("non-Symbol property keys have string values"),
        )
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
    uri_decode(vm, &input, URI_RESERVED_SET)
}

fn global_decode_uri_component(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let input = vm.to_string(args.first().unwrap_or(&Value::Undefined))?;
    uri_decode(vm, &input, "")
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

fn uri_decode(vm: &mut Vm, input: &str, reserved_set: &str) -> error::Result<Value> {
    // Input bytes bound both scan work and output size: each decoded scalar is
    // no larger than its percent-encoded source, including RuJa's UTF-16
    // sentinel representation for supplementary code points.
    vm.consume_fuel_units(input.len().min(i64::MAX as usize) as i64)?;
    let mut out = String::new();
    out.try_reserve_exact(input.len())
        .map_err(|_| Error::range("decoded URI result is too large"))?;
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            let ch = input[index..]
                .chars()
                .next()
                .expect("index remains on a UTF-8 boundary");
            out.push(ch);
            index += ch.len_utf8();
            continue;
        }

        let first = parse_uri_hex_byte(bytes, index)?;
        let utf8_len = uri_utf8_sequence_len(first)?;
        let mut decoded_bytes = [0u8; 4];
        for offset in 0..utf8_len {
            let triplet_index = index + offset * 3;
            decoded_bytes[offset] = parse_uri_hex_byte(bytes, triplet_index)?;
        }

        let decoded = std::str::from_utf8(&decoded_bytes[..utf8_len])
            .map_err(|_| Error::uri("malformed URI sequence"))?;
        let decoded_char = decoded
            .chars()
            .next()
            .expect("validated URI sequence contains one scalar");
        if reserved_set.contains(decoded_char) {
            out.push_str(&input[index..index + utf8_len * 3]);
        } else {
            push_decoded_uri_char(&mut out, decoded_char);
        }
        index += utf8_len * 3;
    }
    Ok(Value::String(Arc::from(out.as_str())))
}

fn push_decoded_uri_char(out: &mut String, ch: char) {
    crate::value::push_utf16_scalar(out, ch);
}

fn parse_uri_hex_byte(bytes: &[u8], index: usize) -> error::Result<u8> {
    if bytes.get(index) != Some(&b'%') {
        return Err(Error::uri("malformed URI sequence"));
    }
    let high = bytes
        .get(index + 1)
        .and_then(|byte| uri_hex_value(*byte))
        .ok_or_else(|| Error::uri("malformed URI sequence"))?;
    let low = bytes
        .get(index + 2)
        .and_then(|byte| uri_hex_value(*byte))
        .ok_or_else(|| Error::uri("malformed URI sequence"))?;
    Ok((high << 4) | low)
}

fn uri_hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
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

#[cfg(test)]
fn take_ordinary_own_keys_reservation_failure(
    failure: &mut Option<(crate::vm::OrdinaryOwnKeysReservationSite, usize)>,
    site: crate::vm::OrdinaryOwnKeysReservationSite,
) -> bool {
    let Some((configured_site, remaining)) = *failure else {
        return false;
    };
    if configured_site != site {
        return false;
    }
    if remaining != 0 {
        *failure = Some((configured_site, remaining - 1));
        return false;
    }
    *failure = None;
    true
}

pub(crate) fn reserve_ordinary_own_keys_vec<T>(
    keys: &mut Vec<T>,
    #[cfg(test)] failure: &mut Option<(crate::vm::OrdinaryOwnKeysReservationSite, usize)>,
    #[cfg(test)] site: crate::vm::OrdinaryOwnKeysReservationSite,
    message: &'static str,
) -> error::Result<()> {
    if keys.len() < keys.capacity() {
        return Ok(());
    }
    #[cfg(test)]
    if take_ordinary_own_keys_reservation_failure(failure, site) {
        return Err(Error::range(message));
    }
    keys.try_reserve(1).map_err(|_| Error::range(message))
}

pub(crate) fn reserve_ordinary_own_keys_seen(
    seen: &mut IndexSet<PropertyKey>,
    #[cfg(test)] failure: &mut Option<(crate::vm::OrdinaryOwnKeysReservationSite, usize)>,
) -> error::Result<()> {
    if seen.len() < seen.capacity() {
        return Ok(());
    }
    #[cfg(test)]
    if take_ordinary_own_keys_reservation_failure(
        failure,
        crate::vm::OrdinaryOwnKeysReservationSite::Seen,
    ) {
        return Err(Error::range("ordinary own-key duplicate set is too large"));
    }
    seen.try_reserve(1)
        .map_err(|_| Error::range("ordinary own-key duplicate set is too large"))
}

pub(crate) fn push_unique_key(
    keys: &mut Vec<PropertyKey>,
    seen: &mut IndexSet<PropertyKey>,
    key: PropertyKey,
    #[cfg(test)] failure: &mut Option<(crate::vm::OrdinaryOwnKeysReservationSite, usize)>,
) -> error::Result<()> {
    if seen.contains(&key) {
        return Ok(());
    }
    reserve_ordinary_own_keys_seen(
        seen,
        #[cfg(test)]
        failure,
    )?;
    reserve_ordinary_own_keys_vec(
        keys,
        #[cfg(test)]
        failure,
        #[cfg(test)]
        crate::vm::OrdinaryOwnKeysReservationSite::Result,
        "ordinary own-key result is too large",
    )?;
    seen.insert(key.clone());
    keys.push(key);
    Ok(())
}

// Keep allocation-free integrity predicates on the same conservative scan
// budget as ordinary own-key materialization.
fn ordinary_own_property_scan_work(
    vm: &Vm,
    obj: &Value,
    include_strings: bool,
    typed_array_index_count: Option<usize>,
) -> usize {
    let mut work = typed_array_index_count.unwrap_or(0);
    match obj {
        Value::Object(idx) => {
            work = work.saturating_add(vm.heap.with_obj(idx.0, |object| {
                let mut object_work = object.props().lock().len();
                if include_strings {
                    if let HeapObj::Array(array) = object {
                        object_work = object_work.saturating_add(array.present.lock().len());
                    }
                    if let HeapObj::Object(data) = object {
                        if let Some(Value::String(string)) = data.primitive.lock().as_ref() {
                            object_work = object_work.saturating_add(string.len());
                        }
                    }
                    if let HeapObj::ModuleNamespace(namespace) = object {
                        object_work = object_work.saturating_add(namespace.exports.lock().len());
                    }
                }
                object_work
            }));
        }
        Value::String(string) if include_strings => {
            work = work.saturating_add(string.len());
        }
        _ => {}
    }
    work
}

fn ordinary_own_property_keys(
    vm: &mut Vm,
    obj: &Value,
    enumerable_only: bool,
    include_strings: bool,
    include_symbols: bool,
    charge_fuel: bool,
) -> error::Result<Vec<PropertyKey>> {
    let mut keys = Vec::new();
    let mut seen = IndexSet::new();
    let typed_array_index_count = include_strings
        .then(|| vm.typed_array_integer_index_own_property_key_count(obj))
        .flatten();

    // Charge before materializing native key collections. String byte length
    // is a conservative O(1) upper bound for its UTF-16 key count.
    let scan_work =
        ordinary_own_property_scan_work(vm, obj, include_strings, typed_array_index_count);
    if charge_fuel && vm.fuel_remaining().is_some() {
        for _ in 0..scan_work {
            vm.consume_fuel()?;
        }
    }

    #[cfg(test)]
    let (heap, reservation_failure) = (&vm.heap, &mut vm.fail_ordinary_own_keys_reservation);
    #[cfg(not(test))]
    let heap = &vm.heap;

    match obj {
        Value::Object(idx) => heap.with_obj(idx.0, |o| -> error::Result<()> {
            let mut index_keys: Vec<u32> = Vec::new();
            let mut string_keys: Vec<PropertyKey> = Vec::new();
            let mut symbol_keys: Vec<PropertyKey> = Vec::new();

            if let Some(count) = typed_array_index_count {
                for i in 0..count {
                    if let Ok(index) = u32::try_from(i) {
                        reserve_ordinary_own_keys_vec(
                            &mut index_keys,
                            #[cfg(test)]
                            reservation_failure,
                            #[cfg(test)]
                            crate::vm::OrdinaryOwnKeysReservationSite::Index,
                            "ordinary own-key index list is too large",
                        )?;
                        index_keys.push(index);
                    }
                }
            }

            if let HeapObj::Array(a) = o {
                if include_strings {
                    for (i, present) in a.present.lock().iter().copied().enumerate() {
                        if present {
                            reserve_ordinary_own_keys_vec(
                                &mut index_keys,
                                #[cfg(test)]
                                reservation_failure,
                                #[cfg(test)]
                                crate::vm::OrdinaryOwnKeysReservationSite::Index,
                                "ordinary own-key index list is too large",
                            )?;
                            index_keys.push(i as u32);
                        }
                    }
                    if !enumerable_only && !a.is_arguments.load(Ordering::Relaxed) {
                        reserve_ordinary_own_keys_vec(
                            &mut string_keys,
                            #[cfg(test)]
                            reservation_failure,
                            #[cfg(test)]
                            crate::vm::OrdinaryOwnKeysReservationSite::String,
                            "ordinary own-key string list is too large",
                        )?;
                        string_keys.push(PropertyKey::from("length"));
                    }
                }
            }

            if let HeapObj::Object(od) = o {
                if include_strings {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        for i in 0..crate::value::utf16_len(&s) {
                            reserve_ordinary_own_keys_vec(
                                &mut index_keys,
                                #[cfg(test)]
                                reservation_failure,
                                #[cfg(test)]
                                crate::vm::OrdinaryOwnKeysReservationSite::Index,
                                "ordinary own-key index list is too large",
                            )?;
                            index_keys.push(i as u32);
                        }
                        if !enumerable_only {
                            reserve_ordinary_own_keys_vec(
                                &mut string_keys,
                                #[cfg(test)]
                                reservation_failure,
                                #[cfg(test)]
                                crate::vm::OrdinaryOwnKeysReservationSite::String,
                                "ordinary own-key string list is too large",
                            )?;
                            string_keys.push(PropertyKey::from("length"));
                        }
                    }
                }
            }

            if let HeapObj::ModuleNamespace(namespace) = o {
                if include_strings {
                    for name in namespace.exports.lock().keys() {
                        reserve_ordinary_own_keys_vec(
                            &mut string_keys,
                            #[cfg(test)]
                            reservation_failure,
                            #[cfg(test)]
                            crate::vm::OrdinaryOwnKeysReservationSite::String,
                            "ordinary own-key string list is too large",
                        )?;
                        string_keys.push(PropertyKey::from(name.clone()));
                    }
                }
            }

            for (k, desc) in o.props().lock().iter() {
                if enumerable_only && !desc.enumerable {
                    continue;
                }
                if let Some(id) = k.symbol_id() {
                    if !include_symbols {
                        continue;
                    }
                    reserve_ordinary_own_keys_vec(
                        &mut symbol_keys,
                        #[cfg(test)]
                        reservation_failure,
                        #[cfg(test)]
                        crate::vm::OrdinaryOwnKeysReservationSite::Symbol,
                        "ordinary own-key Symbol list is too large",
                    )?;
                    symbol_keys.push(PropertyKey::symbol(id));
                } else if include_strings {
                    if let Some(index) = k.array_index() {
                        reserve_ordinary_own_keys_vec(
                            &mut index_keys,
                            #[cfg(test)]
                            reservation_failure,
                            #[cfg(test)]
                            crate::vm::OrdinaryOwnKeysReservationSite::Index,
                            "ordinary own-key index list is too large",
                        )?;
                        index_keys.push(index);
                    } else {
                        reserve_ordinary_own_keys_vec(
                            &mut string_keys,
                            #[cfg(test)]
                            reservation_failure,
                            #[cfg(test)]
                            crate::vm::OrdinaryOwnKeysReservationSite::String,
                            "ordinary own-key string list is too large",
                        )?;
                        string_keys.push(k.clone());
                    }
                }
            }

            index_keys.sort_unstable();
            index_keys.dedup();
            for n in index_keys {
                push_unique_key(
                    &mut keys,
                    &mut seen,
                    PropertyKey::from_array_index(n),
                    #[cfg(test)]
                    reservation_failure,
                )?;
            }
            for key in string_keys {
                push_unique_key(
                    &mut keys,
                    &mut seen,
                    key,
                    #[cfg(test)]
                    reservation_failure,
                )?;
            }
            for key in symbol_keys {
                push_unique_key(
                    &mut keys,
                    &mut seen,
                    key,
                    #[cfg(test)]
                    reservation_failure,
                )?;
            }
            Ok(())
        })?,
        Value::String(s) if include_strings => {
            for i in 0..crate::value::utf16_len(s) {
                push_unique_key(
                    &mut keys,
                    &mut seen,
                    PropertyKey::from_integer_index(i as u64),
                    #[cfg(test)]
                    reservation_failure,
                )?;
            }
            if !enumerable_only {
                push_unique_key(
                    &mut keys,
                    &mut seen,
                    PropertyKey::from("length"),
                    #[cfg(test)]
                    reservation_failure,
                )?;
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
    let result = (|| {
        #[cfg(test)]
        if !items.is_empty()
            && take_own_key_consumer_reservation_failure(
                vm,
                crate::vm::OwnKeyConsumerReservationSite::ArrayPresence,
            )
        {
            return Err(Error::range("Array presence bitmap is too large"));
        }
        let array = ArrayData::try_new(items, Some(prototype))
            .map_err(|_| Error::range("Array presence bitmap is too large"))?;
        vm.alloc(HeapObj::Array(array)).map(Value::Object)
    })();
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
    let mut items = Vec::new();
    items
        .try_reserve(strs.len())
        .map_err(|_| Error::range("String array result is too large"))?;
    items.extend(strs.into_iter().map(Value::String));
    make_value_array(vm, items)
}

#[cfg(test)]
fn take_own_key_consumer_reservation_failure(
    vm: &mut Vm,
    site: crate::vm::OwnKeyConsumerReservationSite,
) -> bool {
    let Some((configured_site, remaining)) = vm.fail_own_key_consumer_reservation else {
        return false;
    };
    if configured_site != site {
        return false;
    }
    if remaining != 0 {
        vm.fail_own_key_consumer_reservation = Some((configured_site, remaining - 1));
        return false;
    }
    vm.fail_own_key_consumer_reservation = None;
    true
}

pub(crate) fn reserve_own_key_consumer_values<T>(
    vm: &mut Vm,
    values: &mut Vec<T>,
    additional: usize,
    #[cfg(test)] site: crate::vm::OwnKeyConsumerReservationSite,
) -> error::Result<()> {
    if additional <= values.capacity() - values.len() {
        return Ok(());
    }
    #[cfg(test)]
    if take_own_key_consumer_reservation_failure(vm, site) {
        return Err(Error::range("own-key consumer result is too large"));
    }
    values
        .try_reserve(additional)
        .map_err(|_| Error::range("own-key consumer result is too large"))
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
    let mut values = Vec::new();
    for key in keys {
        if own_property_descriptor_for_key_or_throw(vm, &obj, &key)?
            .is_some_and(|desc| desc.enumerable)
        {
            if let Some(name) = key.into_string_arc() {
                reserve_own_key_consumer_values(
                    vm,
                    &mut values,
                    1,
                    #[cfg(test)]
                    crate::vm::OwnKeyConsumerReservationSite::Result,
                )?;
                values.push(Value::String(name));
            }
        }
    }
    let realm = vm.current_realm_global_env();
    create_array_from_values_in_realm(vm, values, realm)
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
    let mut vals = Vec::new();
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
            let value = vm.get_property(&obj, &k)?;
            reserve_own_key_consumer_values(
                vm,
                &mut vals,
                1,
                #[cfg(test)]
                crate::vm::OwnKeyConsumerReservationSite::Result,
            )?;
            vm.try_reserve_value_roots(std::slice::from_ref(&value))?;
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
            let Some(name) = k.into_string_arc() else {
                continue;
            };
            let value = vm.get_property(&obj, name.as_ref())?;
            let mut pair_values = Vec::new();
            reserve_own_key_consumer_values(
                vm,
                &mut pair_values,
                2,
                #[cfg(test)]
                crate::vm::OwnKeyConsumerReservationSite::EntryElements,
            )?;
            pair_values.push(Value::String(name));
            pair_values.push(value);
            let pair = create_array_from_values_in_realm(vm, pair_values, realm)?;
            reserve_own_key_consumer_values(
                vm,
                &mut pairs,
                1,
                #[cfg(test)]
                crate::vm::OwnKeyConsumerReservationSite::Result,
            )?;
            vm.try_reserve_value_roots(std::slice::from_ref(&pair))?;
            pair_pins += vm.pin(&pair);
            pairs.push(pair);
        }
        create_array_from_values_in_realm(vm, pairs, realm)
    })();
    vm.unpin_many(pair_pins);
    result
}

#[cfg(test)]
fn take_group_by_reservation_failure(vm: &mut Vm, site: crate::vm::GroupByReservationSite) -> bool {
    let Some((configured_site, remaining)) = vm.fail_group_by_reservation else {
        return false;
    };
    if configured_site != site {
        return false;
    }
    if remaining != 0 {
        vm.fail_group_by_reservation = Some((configured_site, remaining - 1));
        return false;
    }
    vm.fail_group_by_reservation = None;
    true
}

fn reserve_group_by_groups(
    vm: &mut Vm,
    groups: &mut IndexMap<PropertyKey, Vec<Value>>,
) -> error::Result<()> {
    let _ = &vm;
    #[cfg(test)]
    if take_group_by_reservation_failure(vm, crate::vm::GroupByReservationSite::Groups) {
        return Err(Error::range("Object.groupBy group list is too large"));
    }
    if groups.len() < groups.capacity() {
        return Ok(());
    }
    groups
        .try_reserve(1)
        .map_err(|_| Error::range("Object.groupBy group list is too large"))
}

fn reserve_group_by_elements(vm: &mut Vm, values: &mut Vec<Value>) -> error::Result<()> {
    let _ = &vm;
    // Test injection is per logical append so the existing-group path does
    // not depend on the allocator's unspecified spare Vec capacity.
    #[cfg(test)]
    if take_group_by_reservation_failure(vm, crate::vm::GroupByReservationSite::Elements) {
        return Err(Error::range("Object.groupBy element list is too large"));
    }
    if values.len() < values.capacity() {
        return Ok(());
    }
    values
        .try_reserve(1)
        .map_err(|_| Error::range("Object.groupBy element list is too large"))
}

fn reserve_group_by_value_roots(
    vm: &mut Vm,
    values: &[Value],
    #[cfg(test)] site: crate::vm::GroupByReservationSite,
) -> error::Result<()> {
    #[cfg(test)]
    if take_group_by_reservation_failure(vm, site) {
        return Err(Error::range("GroupBy temporary root set is too large"));
    }
    vm.try_reserve_value_roots(values)
}

fn reserve_group_by_root_slots(
    vm: &mut Vm,
    additional: usize,
    #[cfg(test)] site: crate::vm::GroupByReservationSite,
) -> error::Result<()> {
    #[cfg(test)]
    if take_group_by_reservation_failure(vm, site) {
        return Err(Error::range("GroupBy temporary root set is too large"));
    }
    vm.try_reserve_gc_pins(additional)
}

fn close_iterator_after_error_in_realm<T>(
    vm: &mut Vm,
    iterator: &Value,
    error: Arc<Error>,
    realm: GcIdx,
) -> error::Result<T> {
    // Native errors exist before IteratorClose. Materialize them now so user
    // return code cannot alter their Realm or collect their thrown value.
    let error = vm.materialize_error_in_realm(error, realm);
    close_iterator_after_error(vm, iterator, error)
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
    let realm = vm.current_realm_global_env();

    reserve_group_by_value_roots(
        vm,
        &[items.clone(), callback.clone()],
        #[cfg(test)]
        crate::vm::GroupByReservationSite::InputRoots,
    )?;
    let input_pins = vm.pin_many(&[items.clone(), callback.clone()]);
    let result = (|| -> error::Result<Value> {
        // Iterator record, current value, and callback-produced key can all be
        // live at once across observable calls.
        reserve_group_by_root_slots(
            vm,
            4,
            #[cfg(test)]
            crate::vm::GroupByReservationSite::IteratorRoots,
        )?;
        let iterator = get_sync_iterator(vm, items)?;
        let iterator_pins = vm.pin_many(&[iterator.iterator.clone(), iterator.next_method.clone()]);
        let mut groups: IndexMap<PropertyKey, Vec<Value>> = IndexMap::new();
        let mut group_pins = 0;
        let grouping = (|| -> error::Result<()> {
            #[cfg(test)]
            let mut k = vm.group_by_index_override.take().unwrap_or(0);
            #[cfg(not(test))]
            let mut k = 0u64;
            loop {
                if k >= 9_007_199_254_740_991 {
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        Error::type_err("Object.groupBy index exceeds the safe integer limit"),
                        realm,
                    );
                }
                // IteratorStepValue failures propagate without IteratorClose.
                #[cfg(test)]
                if std::mem::take(&mut vm.group_by_zero_fuel_before_step) {
                    vm.set_fuel(Some(0));
                }
                let Some(value) =
                    iterator_helper_step(vm, &iterator.iterator, &iterator.next_method, true)?
                else {
                    return Ok(());
                };

                let mut value_pin = 0;
                let mut key_pin = 0;
                let process = (|| -> error::Result<PropertyKey> {
                    reserve_group_by_value_roots(
                        vm,
                        std::slice::from_ref(&value),
                        #[cfg(test)]
                        crate::vm::GroupByReservationSite::ValueRoots,
                    )?;
                    value_pin = vm.pin(&value);
                    let key_value = vm.call_function(
                        &callback,
                        &[value.clone(), Value::Number(k as f64)],
                        Some(Value::Undefined),
                    )?;
                    reserve_group_by_value_roots(
                        vm,
                        std::slice::from_ref(&key_value),
                        #[cfg(test)]
                        crate::vm::GroupByReservationSite::KeyRoots,
                    )?;
                    key_pin = vm.pin(&key_value);
                    to_property_key_descriptor(vm, &key_value)
                })();
                let key = match process {
                    Ok(key) => key,
                    Err(error) => {
                        vm.unpin_many(value_pin + key_pin);
                        if !error.catchable() {
                            return Err(error);
                        }
                        return close_iterator_after_error_in_realm(
                            vm,
                            &iterator.iterator,
                            error,
                            realm,
                        );
                    }
                };
                vm.unpin_many(key_pin);

                let storage = if let Some(values) = groups.get_mut(&key) {
                    reserve_group_by_elements(vm, values).map(|()| values.push(value))
                } else {
                    reserve_group_by_groups(vm, &mut groups).and_then(|()| {
                        let mut values = Vec::new();
                        reserve_group_by_elements(vm, &mut values)?;
                        values.push(value);
                        groups.insert(key, values);
                        Ok(())
                    })
                };
                if let Err(error) = storage {
                    vm.unpin_many(value_pin);
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        error,
                        realm,
                    );
                }
                group_pins += value_pin;
                k += 1;
            }
        })();
        if let Err(error) = grouping {
            vm.unpin_many(group_pins);
            vm.unpin_many(iterator_pins);
            return Err(error);
        }

        let output = (|| -> error::Result<Value> {
            vm.try_reserve_gc_pins(2)?;
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
            let completion = (|| -> error::Result<Value> {
                for (key, values) in groups {
                    vm.consume_fuel()?;
                    let array = create_array_from_values_in_realm(vm, values, realm)?;
                    let array_pin = vm.pin(&array);
                    let publication = vm.define_own_property_or_throw(
                        &result,
                        key,
                        PropertyDescriptor::data(array),
                    );
                    vm.unpin_many(array_pin);
                    publication?;
                }
                Ok(result)
            })();
            vm.unpin_many(result_pin);
            completion
        })();
        vm.unpin_many(group_pins);
        vm.unpin_many(iterator_pins);
        output
    })();
    vm.unpin_many(input_pins);
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
    vm.try_reserve_value_roots(&[entries.clone(), prototype.clone()])?;
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
    let result = (|| -> error::Result<Value> {
        // The five reserved roots cover the iterator record plus one current
        // entry, key, and value across every observable operation.
        vm.try_reserve_gc_pins(5)?;
        let iterator = get_sync_iterator(vm, entries)?;
        let iterator_pins = vm.pin_many(&[iterator.iterator.clone(), iterator.next_method.clone()]);
        let iteration = (|| -> error::Result<Value> {
            loop {
                // IteratorStepValue failures do not perform IteratorClose.
                let Some(entry) =
                    iterator_helper_step(vm, &iterator.iterator, &iterator.next_method, true)?
                else {
                    return Ok(object.clone());
                };
                if !matches!(entry, Value::Object(_)) {
                    return close_iterator_after_error(
                        vm,
                        &iterator.iterator,
                        Error::type_err("Iterator value is not an entry object"),
                    );
                }

                let mut entry_pins = vm.pin(&entry);
                let entry_result = (|| -> error::Result<()> {
                    let key = vm.get_property_by_key(&entry, &PropertyKey::from("0"))?;
                    entry_pins += vm.pin(&key);
                    let value = vm.get_property_by_key(&entry, &PropertyKey::from("1"))?;
                    entry_pins += vm.pin(&value);
                    let key = to_property_key_descriptor(vm, &key)?;
                    vm.define_own_property_or_throw(&object, key, PropertyDescriptor::data(value))
                })();
                vm.unpin_many(entry_pins);
                if let Err(error) = entry_result {
                    if !error.catchable() {
                        return Err(error);
                    }
                    return close_iterator_after_error(vm, &iterator.iterator, error);
                }
            }
        })();
        vm.unpin_many(iterator_pins);
        iteration
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
    let keys = own_property_keys_or_throw(vm, &obj, false, true, false)?;
    let mut values = Vec::new();
    for key in keys {
        let Some(name) = key.into_string_arc() else {
            continue;
        };
        reserve_own_key_consumer_values(
            vm,
            &mut values,
            1,
            #[cfg(test)]
            crate::vm::OwnKeyConsumerReservationSite::Result,
        )?;
        values.push(Value::String(name));
    }
    let realm = vm.current_realm_global_env();
    create_array_from_values_in_realm(vm, values, realm)
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
    let keys = own_property_keys_or_throw(vm, &obj, false, false, true)?;
    let mut symbols = Vec::new();
    for key in keys {
        let Some(id) = key.symbol_id() else {
            continue;
        };
        reserve_own_key_consumer_values(
            vm,
            &mut symbols,
            1,
            #[cfg(test)]
            crate::vm::OwnKeyConsumerReservationSite::Result,
        )?;
        symbols.push(Value::Symbol(id));
    }
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

fn try_reserve_proxy_own_keys_roots(
    vm: &mut Vm,
    values: &[Value],
    #[cfg(test)] site: crate::vm::ProxyOwnKeysReservationSite,
) -> error::Result<()> {
    if !values.iter().any(|value| Vm::value_root_count(value) != 0) {
        return Ok(());
    }
    #[cfg(test)]
    if take_proxy_own_keys_reservation_failure(vm, site) {
        return Err(Error::range("temporary root set is too large"));
    }
    vm.try_reserve_value_roots(values)
}

pub(crate) fn reserve_proxy_own_keys_trap_result_key(
    _vm: &mut Vm,
    keys: &mut Vec<PropertyKey>,
) -> error::Result<()> {
    if keys.len() < keys.capacity() {
        return Ok(());
    }
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

pub(crate) fn reserve_proxy_own_keys_seen_key(
    _vm: &mut Vm,
    seen: &mut IndexSet<PropertyKey>,
) -> error::Result<()> {
    if seen.len() < seen.capacity() {
        return Ok(());
    }
    #[cfg(test)]
    if take_proxy_own_keys_reservation_failure(_vm, crate::vm::ProxyOwnKeysReservationSite::SeenKey)
    {
        return Err(Error::range("Proxy ownKeys duplicate set is too large"));
    }
    seen.try_reserve(1)
        .map_err(|_| Error::range("Proxy ownKeys duplicate set is too large"))
}

fn reserve_proxy_own_keys_target_key_set(
    _vm: &mut Vm,
    keys: &mut IndexSet<PropertyKey>,
    additional: usize,
) -> error::Result<()> {
    if additional == 0 {
        return Ok(());
    }
    #[cfg(test)]
    if take_proxy_own_keys_reservation_failure(
        _vm,
        crate::vm::ProxyOwnKeysReservationSite::TargetKeySet,
    ) {
        return Err(Error::range("Proxy ownKeys target-key set is too large"));
    }
    keys.try_reserve(additional)
        .map_err(|_| Error::range("Proxy ownKeys target-key set is too large"))
}

fn reserve_proxy_own_keys_filtered_key(
    _vm: &mut Vm,
    keys: &mut Vec<PropertyKey>,
) -> error::Result<()> {
    if keys.len() < keys.capacity() {
        return Ok(());
    }
    #[cfg(test)]
    if take_proxy_own_keys_reservation_failure(
        _vm,
        crate::vm::ProxyOwnKeysReservationSite::FilteredKey,
    ) {
        return Err(Error::range("Proxy ownKeys filtered result is too large"));
    }
    keys.try_reserve(1)
        .map_err(|_| Error::range("Proxy ownKeys filtered result is too large"))
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
    try_reserve_proxy_own_keys_roots(
        vm,
        std::slice::from_ref(key_list),
        #[cfg(test)]
        crate::vm::ProxyOwnKeysReservationSite::TrapResultRoot,
    )?;
    let list_pin = vm.pin(key_list);
    let result = (|| -> error::Result<Vec<PropertyKey>> {
        let length_value = vm.get_property(key_list, "length")?;
        try_reserve_proxy_own_keys_roots(
            vm,
            std::slice::from_ref(&length_value),
            #[cfg(test)]
            crate::vm::ProxyOwnKeysReservationSite::LengthValueRoot,
        )?;
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
            let key = PropertyKey::from_integer_index(index as u64);
            let item = vm.get_property_by_key(key_list, &key)?;
            let key = match item {
                Value::String(value) => PropertyKey::from_rc(value),
                Value::Symbol(id) => PropertyKey::symbol(id),
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

    try_reserve_proxy_own_keys_roots(
        vm,
        std::slice::from_ref(obj),
        #[cfg(test)]
        crate::vm::ProxyOwnKeysReservationSite::OperationRoot,
    )?;
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
                    true,
                )?;
            };

            let (target, handler) = proxy_result?;
            vm.consume_fuel()?;
            let layer_roots = [target.clone(), handler.clone()];
            try_reserve_proxy_own_keys_roots(
                vm,
                &layer_roots,
                #[cfg(test)]
                crate::vm::ProxyOwnKeysReservationSite::LayerRoots,
            )?;
            let proxy_pins = vm.pin_many(&layer_roots);
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
                    #[cfg(test)]
                    if take_proxy_own_keys_reservation_failure(
                        vm,
                        crate::vm::ProxyOwnKeysReservationSite::PendingFrame,
                    ) {
                        return Err(Error::range("Proxy ownKeys validation chain is too large"));
                    }
                    pending
                        .try_reserve(1)
                        .map_err(|_| Error::range("Proxy ownKeys validation chain is too large"))?;
                    let frame_roots = [current.clone(), target.clone()];
                    try_reserve_proxy_own_keys_roots(
                        vm,
                        &frame_roots,
                        #[cfg(test)]
                        crate::vm::ProxyOwnKeysReservationSite::FrameRoots,
                    )?;
                    pending_pins += vm.pin_many(&frame_roots);
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
                let mut target_key_set = IndexSet::new();
                reserve_proxy_own_keys_target_key_set(vm, &mut target_key_set, keys.len())?;
                target_key_set.extend(keys.iter().cloned());
                if target_key_set != frame.seen {
                    return Err(Error::type_err(
                        "Proxy ownKeys trap does not match a non-extensible target",
                    ));
                }
            }

            let mut filtered = Vec::new();
            for key in frame.trap_keys {
                vm.consume_fuel()?;
                let included = (!key.is_symbol() && frame.include_strings)
                    || (key.is_symbol() && frame.include_symbols);
                if !included {
                    continue;
                }
                if frame.enumerable_only
                    && !own_property_descriptor_for_key_or_throw(vm, &frame.object, &key)?
                        .is_some_and(|desc| desc.enumerable)
                {
                    continue;
                }
                reserve_proxy_own_keys_filtered_key(vm, &mut filtered)?;
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

pub(crate) fn observe_namespace_binding_initialized(
    vm: &mut Vm,
    environment: GcIdx,
    name: Arc<str>,
) -> error::Result<()> {
    let mut current_environment = environment;
    let mut current_name = name;
    let mut checkpoint_environment = current_environment;
    let mut checkpoint_name = current_name.clone();
    let mut checkpoint_power = 1usize;
    let mut checkpoint_span = 0usize;

    loop {
        let state = vm.heap.with_obj(current_environment.0, |object| {
            let HeapObj::Environment(environment) = object else {
                return None;
            };
            let bindings = environment.vars.lock();
            let binding = bindings.get(current_name.as_ref())?;
            Some((
                binding.initialized.load(Ordering::Relaxed),
                binding.indirect.clone(),
            ))
        });
        let Some((initialized, indirect)) = state else {
            return Ok(());
        };
        if !initialized {
            return Err(Error::reference(format!(
                "Cannot access '{}' before initialization",
                current_name
            )));
        }
        let Some((next_environment, next_name)) = indirect else {
            return Ok(());
        };
        vm.consume_fuel()?;
        current_environment = next_environment;
        current_name = next_name;
        checkpoint_span = checkpoint_span.saturating_add(1);
        if current_environment == checkpoint_environment && current_name == checkpoint_name {
            return Err(Error::reference(format!(
                "Cannot access '{}' before initialization",
                current_name
            )));
        }
        if checkpoint_span == checkpoint_power {
            checkpoint_environment = current_environment;
            checkpoint_name = current_name.clone();
            checkpoint_power = checkpoint_power.saturating_mul(2).max(1);
            checkpoint_span = 0;
        }
    }
}

fn direct_test_integrity_level(
    vm: &mut Vm,
    obj: &Value,
    frozen: bool,
) -> Option<error::Result<bool>> {
    let Value::Object(idx) = obj else {
        return Some(Ok(true));
    };
    let typed_array_index_count = vm.typed_array_integer_index_own_property_key_count(obj);
    let observable = vm.heap.with_obj(idx.0, |object| {
        matches!(object, HeapObj::Proxy(_) | HeapObj::ModuleNamespace(_))
    });
    if observable {
        return None;
    }

    let scan_work = ordinary_own_property_scan_work(vm, obj, true, typed_array_index_count);
    if vm.fuel_remaining().is_some() {
        for _ in 0..scan_work {
            if let Err(error) = vm.consume_fuel() {
                return Some(Err(error));
            }
        }
    }

    let mixed_dense_array = vm.heap.with_obj(idx.0, |object| {
        matches!(object, HeapObj::Array(array) if array.present.lock().iter().any(|present| *present))
    });
    if mixed_dense_array {
        return Some((|| {
            let keys = ordinary_own_property_keys(vm, obj, false, true, true, false)?;
            for key in keys {
                let Some(descriptor) = integrity_descriptor_for_key(vm, obj, &key)? else {
                    continue;
                };
                if descriptor.configurable || (frozen && descriptor.writable.unwrap_or(false)) {
                    return Ok(false);
                }
            }
            Ok(true)
        })());
    }

    let valid = vm.heap.with_obj(idx.0, |object| {
        let properties = object.props().lock();
        let mut valid = properties.values().all(|descriptor| {
            !descriptor.configurable && (!frozen || descriptor.is_accessor || !descriptor.writable)
        });
        drop(properties);

        if let HeapObj::TypedArray(_) = object {
            let count = typed_array_index_count.unwrap_or(0);
            if count != 0 {
                valid = false;
            }
        }
        if let HeapObj::Array(array) = object {
            let length_key = &vm.array_length_key;
            if !array.is_arguments.load(Ordering::Relaxed)
                && !array.props.lock().contains_key(length_key)
            {
                if frozen {
                    valid = false;
                }
            }
        }
        valid
    });
    Some(Ok(valid))
}

fn integrity_descriptor_for_key(
    vm: &mut Vm,
    obj: &Value,
    key: &PropertyKey,
) -> error::Result<Option<IntegrityDescriptor>> {
    let Value::Object(idx) = obj else {
        return Ok(None);
    };
    let is_proxy = vm
        .heap
        .with_obj(idx.0, |object| matches!(object, HeapObj::Proxy(_)));
    if is_proxy {
        return Ok(
            own_property_descriptor_for_key_or_throw(vm, obj, key)?.map(|desc| {
                IntegrityDescriptor {
                    configurable: desc.configurable,
                    writable: if desc.is_accessor {
                        None
                    } else {
                        Some(desc.writable)
                    },
                }
            }),
        );
    }

    if let Some(present) = vm.typed_array_integer_index_has_property(obj, key) {
        return Ok(present.then_some(IntegrityDescriptor {
            configurable: true,
            writable: Some(true),
        }));
    }

    let namespace_binding = vm.heap.with_obj(idx.0, |object| {
        let HeapObj::ModuleNamespace(namespace) = object else {
            return None;
        };
        key.as_str()
            .and_then(|name| namespace.exports.lock().get(name.as_ref()).cloned())
    });
    if let Some((environment, name)) = namespace_binding {
        observe_namespace_binding_initialized(vm, environment, name)?;
        return Ok(Some(IntegrityDescriptor {
            configurable: false,
            writable: Some(true),
        }));
    }

    Ok(vm.heap.with_obj(idx.0, |object| {
        if let HeapObj::Array(array) = object {
            if key.as_str().is_some_and(|name| name == "length") {
                if let Some(descriptor) = array.props.lock().get(key) {
                    return Some(IntegrityDescriptor {
                        configurable: descriptor.configurable,
                        writable: (!descriptor.is_accessor).then_some(descriptor.writable),
                    });
                }
                return (!array.is_arguments.load(Ordering::Relaxed)).then_some(
                    IntegrityDescriptor {
                        configurable: false,
                        writable: Some(true),
                    },
                );
            }
            if let Some(index) = key.array_index().map(|index| index as usize) {
                if let Some(descriptor) = array.props.lock().get(key) {
                    return Some(IntegrityDescriptor {
                        configurable: descriptor.configurable,
                        writable: (!descriptor.is_accessor).then_some(descriptor.writable),
                    });
                }
                if index < array.items.lock().len() && array.is_dense_present(index) {
                    return Some(IntegrityDescriptor {
                        configurable: true,
                        writable: Some(true),
                    });
                }
                return None;
            }
        }
        if let Some(descriptor) = object.props().lock().get(key) {
            return Some(IntegrityDescriptor {
                configurable: descriptor.configurable,
                writable: (!descriptor.is_accessor).then_some(descriptor.writable),
            });
        }
        if let HeapObj::Object(data) = object {
            if let Some(Value::String(value)) = data.primitive.lock().as_ref() {
                if key.as_str().is_some_and(|name| name == "length") {
                    return Some(IntegrityDescriptor {
                        configurable: false,
                        writable: Some(false),
                    });
                }
                if key
                    .array_index()
                    .is_some_and(|index| (index as usize) < crate::value::utf16_len(value))
                {
                    return Some(IntegrityDescriptor {
                        configurable: false,
                        writable: Some(false),
                    });
                }
            }
        }
        None
    }))
}

fn integrity_define_descriptor(writable: Option<bool>) -> crate::vm::ProxyDefinePropertyDescriptor {
    crate::vm::ProxyDefinePropertyDescriptor {
        descriptor: PropertyDescriptor {
            value: Value::Undefined,
            writable: writable.unwrap_or(false),
            enumerable: false,
            configurable: false,
            get: None,
            set: None,
            is_accessor: false,
        },
        has_value: false,
        has_writable: writable.is_some(),
        has_enumerable: false,
        has_configurable: true,
        has_get: false,
        has_set: false,
    }
}

pub(crate) fn set_integrity_level(vm: &mut Vm, obj: &Value, frozen: bool) -> error::Result<bool> {
    reserve_descriptor_materialization_roots(
        vm,
        std::slice::from_ref(obj),
        #[cfg(test)]
        crate::vm::DescriptorMaterializationReservationSite::IntegrityOperationRoot,
    )?;
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
            let descriptor = integrity_define_descriptor(writable);
            object_define_property_record_result(vm, obj, &key, &descriptor, true, true)?;
        }
        Ok(true)
    })();
    vm.unpin_many(operation_pin);
    result
}

pub(crate) fn test_integrity_level(vm: &mut Vm, obj: &Value, frozen: bool) -> error::Result<bool> {
    reserve_descriptor_materialization_roots(
        vm,
        std::slice::from_ref(obj),
        #[cfg(test)]
        crate::vm::DescriptorMaterializationReservationSite::IntegrityOperationRoot,
    )?;
    let operation_pin = vm.pin(obj);
    let result = (|| {
        if vm.is_extensible(obj)? {
            return Ok(false);
        }
        if let Some(result) = direct_test_integrity_level(vm, obj, frozen) {
            return result;
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
    if matches!(obj, Value::Object(_)) {
        if !set_integrity_level(vm, &obj, false)? {
            return Err(Error::type_err("Object.seal failed to seal object"));
        }
    }
    Ok(obj)
}

fn object_is_sealed(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if matches!(obj, Value::Object(_)) {
        return test_integrity_level(vm, &obj, false).map(Value::Bool);
    }
    Ok(Value::Bool(true))
}

fn object_is_frozen(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if matches!(obj, Value::Object(_)) {
        return test_integrity_level(vm, &obj, true).map(Value::Bool);
    }
    Ok(Value::Bool(true))
}

#[cfg(test)]
fn take_descriptor_materialization_reservation_failure(
    vm: &mut Vm,
    site: crate::vm::DescriptorMaterializationReservationSite,
) -> bool {
    let Some((configured_site, remaining)) = vm.fail_descriptor_materialization_reservation else {
        return false;
    };
    if configured_site != site {
        return false;
    }
    if remaining != 0 {
        vm.fail_descriptor_materialization_reservation = Some((configured_site, remaining - 1));
        return false;
    }
    vm.fail_descriptor_materialization_reservation = None;
    true
}

pub(crate) fn reserve_descriptor_materialization_roots(
    vm: &mut Vm,
    values: &[Value],
    #[cfg(test)] site: crate::vm::DescriptorMaterializationReservationSite,
) -> error::Result<()> {
    let required = values.iter().try_fold(0usize, |total, value| {
        total
            .checked_add(Vm::value_root_count(value))
            .ok_or_else(|| Error::range("descriptor temporary root set is too large"))
    })?;
    if vm.gc_pins.capacity().saturating_sub(vm.gc_pins.len()) >= required {
        return Ok(());
    }
    #[cfg(test)]
    if take_descriptor_materialization_reservation_failure(vm, site) {
        return Err(Error::range("descriptor temporary root set is too large"));
    }
    vm.try_reserve_value_roots(values)
}

pub(crate) fn reserve_descriptor_materialization_root_slots(
    vm: &mut Vm,
    additional: usize,
    #[cfg(test)] site: crate::vm::DescriptorMaterializationReservationSite,
) -> error::Result<()> {
    if vm.gc_pins.capacity().saturating_sub(vm.gc_pins.len()) >= additional {
        return Ok(());
    }
    #[cfg(test)]
    if take_descriptor_materialization_reservation_failure(vm, site) {
        return Err(Error::range("descriptor temporary root set is too large"));
    }
    vm.try_reserve_gc_pins(additional)
}

pub(crate) fn reserve_descriptor_property_storage(
    vm: &mut Vm,
    properties: &mut IndexMap<PropertyKey, PropertyDescriptor>,
    additional: usize,
    #[cfg(test)] site: crate::vm::DescriptorMaterializationReservationSite,
) -> error::Result<()> {
    if properties.capacity().saturating_sub(properties.len()) >= additional {
        return Ok(());
    }
    #[cfg(test)]
    if take_descriptor_materialization_reservation_failure(vm, site) {
        return Err(Error::range("descriptor property storage is too large"));
    }
    properties
        .try_reserve(additional)
        .map_err(|_| Error::range("descriptor property storage is too large"))
}

pub(crate) fn reserve_descriptor_record_storage(
    vm: &mut Vm,
    records: &mut Vec<(PropertyKey, crate::vm::ProxyDefinePropertyDescriptor)>,
) -> error::Result<()> {
    if records.len() < records.capacity() {
        return Ok(());
    }
    #[cfg(test)]
    if take_descriptor_materialization_reservation_failure(
        vm,
        crate::vm::DescriptorMaterializationReservationSite::DefinePropertiesRecord,
    ) {
        return Err(Error::range("property descriptor list is too large"));
    }
    records
        .try_reserve(1)
        .map_err(|_| Error::range("property descriptor list is too large"))
}

fn descriptor_record_roots(descriptor: &crate::vm::ProxyDefinePropertyDescriptor) -> [Value; 3] {
    [
        descriptor.descriptor.value.clone(),
        descriptor
            .descriptor
            .get
            .clone()
            .unwrap_or(Value::Undefined),
        descriptor
            .descriptor
            .set
            .clone()
            .unwrap_or(Value::Undefined),
    ]
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
    let keys = own_property_keys_or_throw(vm, &obj, false, true, true)?;
    let realm = vm.current_realm_global_env();
    let prototype = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Object prototype intrinsic"))?;
    reserve_descriptor_materialization_roots(
        vm,
        &[obj.clone(), prototype.clone()],
        #[cfg(test)]
        crate::vm::DescriptorMaterializationReservationSite::GetOwnDescriptorsOperationRoots,
    )?;
    let allocation_pins = vm.pin_many(&[obj.clone(), prototype.clone()]);
    if let Err(error) = reserve_descriptor_materialization_root_slots(
        vm,
        1,
        #[cfg(test)]
        crate::vm::DescriptorMaterializationReservationSite::GetOwnDescriptorsResultRoot,
    ) {
        vm.unpin_many(allocation_pins);
        return Err(error);
    }
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
        let mut props = IndexMap::new();
        for key in keys {
            if let Some(desc) = own_property_descriptor_for_key_or_throw(vm, &obj, &key)? {
                let descriptor = from_property_descriptor(vm, desc)?;
                reserve_descriptor_property_storage(
                    vm,
                    &mut props,
                    1,
                    #[cfg(test)]
                    crate::vm::DescriptorMaterializationReservationSite::GetOwnDescriptorsResultProperty,
                )?;
                reserve_descriptor_materialization_roots(
                    vm,
                    std::slice::from_ref(&descriptor),
                    #[cfg(test)]
                    crate::vm::DescriptorMaterializationReservationSite::GetOwnDescriptorsDescriptorRoot,
                )?;
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

fn to_property_descriptor_record(
    vm: &mut Vm,
    desc: &Value,
) -> error::Result<crate::vm::ProxyDefinePropertyDescriptor> {
    if !matches!(desc, Value::Object(_)) {
        return Err(Error::type_err("Property description must be an object"));
    }
    reserve_descriptor_materialization_roots(
        vm,
        std::slice::from_ref(desc),
        #[cfg(test)]
        crate::vm::DescriptorMaterializationReservationSite::ToDescriptorObjectRoot,
    )?;
    let mut pin_count = vm.pin(desc);
    let result = (|| {
        let mut has_data = false;
        let mut has_accessor = false;
        let mut descriptor_value = Value::Undefined;
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
            let observed = vm.get_property_by_key(desc, &key)?;
            match name {
                "enumerable" => {
                    enumerable = vm.to_boolean(&observed);
                    has_enumerable = true;
                }
                "configurable" => {
                    configurable = vm.to_boolean(&observed);
                    has_configurable = true;
                }
                "value" => {
                    reserve_descriptor_materialization_roots(
                        vm,
                        std::slice::from_ref(&observed),
                        #[cfg(test)]
                        crate::vm::DescriptorMaterializationReservationSite::ToDescriptorValueRoot,
                    )?;
                    pin_count += vm.pin(&observed);
                    descriptor_value = observed;
                    has_data = true;
                    has_value = true;
                }
                "writable" => {
                    writable = vm.to_boolean(&observed);
                    has_data = true;
                    has_writable = true;
                }
                "get" | "set" => {
                    has_accessor = true;
                    if !observed.is_undefined() && !is_callable(&observed, &vm.heap) {
                        return Err(Error::type_err(if name == "get" {
                            "Getter must be a function"
                        } else {
                            "Setter must be a function"
                        }));
                    }
                    if name == "get" {
                        reserve_descriptor_materialization_roots(
                            vm,
                            std::slice::from_ref(&observed),
                            #[cfg(test)]
                            crate::vm::DescriptorMaterializationReservationSite::ToDescriptorGetterRoot,
                        )?;
                        pin_count += vm.pin(&observed);
                        get = (!observed.is_undefined()).then_some(observed);
                        has_get = true;
                    } else {
                        reserve_descriptor_materialization_roots(
                            vm,
                            std::slice::from_ref(&observed),
                            #[cfg(test)]
                            crate::vm::DescriptorMaterializationReservationSite::ToDescriptorSetterRoot,
                        )?;
                        pin_count += vm.pin(&observed);
                        set = (!observed.is_undefined()).then_some(observed);
                        has_set = true;
                    }
                }
                _ => unreachable!(),
            }
        }
        if has_data && has_accessor {
            return Err(Error::type_err(
                "Invalid property descriptor. Cannot both specify accessors and a value or writable attribute",
            ));
        }
        Ok(crate::vm::ProxyDefinePropertyDescriptor {
            descriptor: PropertyDescriptor {
                value: descriptor_value,
                writable,
                enumerable,
                configurable,
                get,
                set,
                is_accessor: has_accessor,
            },
            has_value,
            has_writable,
            has_enumerable,
            has_configurable,
            has_get,
            has_set,
        })
    })();
    vm.unpin_many(pin_count);
    result
}

fn descriptor_same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            (left.is_nan() && right.is_nan()) || left.to_bits() == right.to_bits()
        }
        _ => left == right,
    }
}

pub(crate) fn object_define_properties(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Object.defineProperties target must be an object",
        ));
    }
    let properties = vm.to_object(&args.get(1).cloned().unwrap_or(Value::Undefined))?;
    reserve_descriptor_materialization_roots(
        vm,
        &[target.clone(), properties.clone()],
        #[cfg(test)]
        crate::vm::DescriptorMaterializationReservationSite::DefinePropertiesOperationRoots,
    )?;
    let base_pins = vm.pin_many(&[target.clone(), properties.clone()]);
    let mut record_pins = 0;
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
            let descriptor = to_property_descriptor_record(vm, &descriptor_object)?;
            reserve_descriptor_record_storage(vm, &mut descriptors)?;
            let roots = descriptor_record_roots(&descriptor);
            reserve_descriptor_materialization_roots(
                vm,
                &roots,
                #[cfg(test)]
                crate::vm::DescriptorMaterializationReservationSite::DefinePropertiesRecordRoots,
            )?;
            record_pins += vm.pin_many(&roots);
            descriptors.push((key, descriptor));
        }
        for (key, descriptor) in descriptors {
            object_define_property_record_result(vm, &target, &key, &descriptor, true, false)?;
        }
        Ok(target.clone())
    })();
    vm.unpin_many(record_pins);
    vm.unpin_many(base_pins);
    result
}

pub(crate) fn canonical_string_index(key: &PropertyKey) -> Option<usize> {
    let name = key.as_str()?;
    canonical_string_index_name(&name)
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
    if key.as_str().is_some_and(|name| name == "length") {
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
                if key.as_str().is_some_and(|name| name == "length") {
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
                    .and_then(|name| namespace.exports.lock().get(name.as_ref()).cloned());
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

#[cfg(test)]
fn take_proxy_descriptor_reservation_failure(
    vm: &mut Vm,
    site: crate::vm::ProxyDescriptorReservationSite,
) -> bool {
    let Some((configured_site, remaining)) = vm.fail_proxy_descriptor_reservation else {
        return false;
    };
    if configured_site != site {
        return false;
    }
    if remaining != 0 {
        vm.fail_proxy_descriptor_reservation = Some((configured_site, remaining - 1));
        return false;
    }
    vm.fail_proxy_descriptor_reservation = None;
    true
}

pub(crate) fn reserve_proxy_descriptor_roots(
    vm: &mut Vm,
    values: &[Value],
    #[cfg(test)] site: crate::vm::ProxyDescriptorReservationSite,
) -> error::Result<()> {
    if !values.iter().any(|value| Vm::value_root_count(value) != 0) {
        return Ok(());
    }
    #[cfg(test)]
    if take_proxy_descriptor_reservation_failure(vm, site) {
        return Err(Error::range("Proxy descriptor root set is too large"));
    }
    vm.try_reserve_value_roots(values)
}

pub(crate) fn reserve_proxy_descriptor_pending_frame(
    vm: &mut Vm,
    pending: &mut Vec<(Value, Value)>,
) -> error::Result<()> {
    if pending.len() < pending.capacity() {
        return Ok(());
    }
    #[cfg(test)]
    if take_proxy_descriptor_reservation_failure(
        vm,
        crate::vm::ProxyDescriptorReservationSite::PendingFrame,
    ) {
        return Err(Error::range(
            "Proxy descriptor validation chain is too large",
        ));
    }
    pending
        .try_reserve(1)
        .map_err(|_| Error::range("Proxy descriptor validation chain is too large"))
}

fn property_descriptor_from_object(vm: &mut Vm, desc: &Value) -> error::Result<PropertyDescriptor> {
    if !matches!(desc, Value::Object(_)) {
        return Err(Error::type_err(
            "Proxy getOwnPropertyDescriptor trap must return an object or undefined",
        ));
    }

    reserve_proxy_descriptor_roots(
        vm,
        std::slice::from_ref(desc),
        #[cfg(test)]
        crate::vm::ProxyDescriptorReservationSite::DescriptorObjectRoot,
    )?;
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
            reserve_proxy_descriptor_roots(
                vm,
                std::slice::from_ref(&value),
                #[cfg(test)]
                crate::vm::ProxyDescriptorReservationSite::DescriptorValueRoot,
            )?;
            pin_count += vm.pin(&value);
            has_value = true;
        }
        if vm.has_property_with_free_ordinary_edge(desc, "writable")? {
            writable = vm.get_property(desc, "writable")?.is_truthy();
            has_writable = true;
        }
        if vm.has_property_with_free_ordinary_edge(desc, "get")? {
            let getter = vm.get_property(desc, "get")?;
            if !getter.is_undefined() && !is_callable(&getter, &vm.heap) {
                return Err(Error::type_err("Getter must be a function"));
            }
            reserve_proxy_descriptor_roots(
                vm,
                std::slice::from_ref(&getter),
                #[cfg(test)]
                crate::vm::ProxyDescriptorReservationSite::DescriptorGetterRoot,
            )?;
            pin_count += vm.pin(&getter);
            get = if getter.is_undefined() {
                None
            } else {
                Some(getter)
            };
            has_get = true;
        }
        if vm.has_property_with_free_ordinary_edge(desc, "set")? {
            let setter = vm.get_property(desc, "set")?;
            if !setter.is_undefined() && !is_callable(&setter, &vm.heap) {
                return Err(Error::type_err("Setter must be a function"));
            }
            reserve_proxy_descriptor_roots(
                vm,
                std::slice::from_ref(&setter),
                #[cfg(test)]
                crate::vm::ProxyDescriptorReservationSite::DescriptorSetterRoot,
            )?;
            pin_count += vm.pin(&setter);
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
    reserve_proxy_descriptor_roots(
        vm,
        std::slice::from_ref(obj),
        #[cfg(test)]
        crate::vm::ProxyDescriptorReservationSite::OperationRoot,
    )?;
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
                        .and_then(|name| namespace.exports.lock().get(name.as_ref()).cloned());
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
            reserve_proxy_descriptor_roots(
                vm,
                &[target.clone(), handler.clone()],
                #[cfg(test)]
                crate::vm::ProxyDescriptorReservationSite::LayerRoots,
            )?;
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
            if !is_callable(&trap, &vm.heap) {
                vm.unpin_many(proxy_pins);
                return Err(Error::type_err(
                    "Proxy getOwnPropertyDescriptor trap is not callable",
                ));
            }
            let key_value = property_key_to_value(key);
            if let Err(error) = reserve_proxy_descriptor_roots(
                vm,
                std::slice::from_ref(&trap),
                #[cfg(test)]
                crate::vm::ProxyDescriptorReservationSite::TrapRoot,
            ) {
                vm.unpin_many(proxy_pins);
                return Err(error);
            }
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
            if let Err(error) = reserve_proxy_descriptor_pending_frame(vm, &mut pending) {
                vm.unpin_many(proxy_pins);
                return Err(error);
            }
            if let Err(error) = reserve_proxy_descriptor_roots(
                vm,
                &[target.clone(), trap_result.clone()],
                #[cfg(test)]
                crate::vm::ProxyDescriptorReservationSite::PendingRoots,
            ) {
                vm.unpin_many(proxy_pins);
                return Err(error);
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
    if result.is_undefined() {
        let Some(target_desc) = target_desc.as_ref() else {
            return Ok(None);
        };
        if !target_desc.configurable {
            return Err(Error::type_err(
                "Proxy getOwnPropertyDescriptor trap cannot hide the target property",
            ));
        }
        let descriptor_roots = [
            target_desc.value.clone(),
            target_desc.get.clone().unwrap_or(Value::Undefined),
            target_desc.set.clone().unwrap_or(Value::Undefined),
        ];
        reserve_proxy_descriptor_roots(
            vm,
            &descriptor_roots,
            #[cfg(test)]
            crate::vm::ProxyDescriptorReservationSite::ValidationDescriptorRoots,
        )?;
        let descriptor_pins = vm.pin_many(&descriptor_roots);
        let extensible = vm.is_extensible(target);
        vm.unpin_many(descriptor_pins);
        if !extensible? {
            return Err(Error::type_err(
                "Proxy getOwnPropertyDescriptor trap cannot hide the target property",
            ));
        }
        return Ok(None);
    }
    let descriptor_roots = target_desc.as_ref().map_or_else(
        || [Value::Undefined, Value::Undefined, Value::Undefined],
        |descriptor| {
            [
                descriptor.value.clone(),
                descriptor.get.clone().unwrap_or(Value::Undefined),
                descriptor.set.clone().unwrap_or(Value::Undefined),
            ]
        },
    );
    reserve_proxy_descriptor_roots(
        vm,
        &descriptor_roots,
        #[cfg(test)]
        crate::vm::ProxyDescriptorReservationSite::ValidationDescriptorRoots,
    )?;
    let descriptor_pins = vm.pin_many(&descriptor_roots);
    let validation = (|| {
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

pub(crate) fn from_property_descriptor(
    vm: &mut Vm,
    desc: PropertyDescriptor,
) -> error::Result<Value> {
    let realm = vm.current_realm_global_env();
    let prototype = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing Object prototype intrinsic"))?;
    let mut props = IndexMap::new();
    reserve_descriptor_property_storage(
        vm,
        &mut props,
        4,
        #[cfg(test)]
        crate::vm::DescriptorMaterializationReservationSite::FromDescriptorProperties,
    )?;
    let roots = [
        prototype.clone(),
        desc.value.clone(),
        desc.get.clone().unwrap_or(Value::Undefined),
        desc.set.clone().unwrap_or(Value::Undefined),
    ];
    reserve_descriptor_materialization_roots(
        vm,
        &roots,
        #[cfg(test)]
        crate::vm::DescriptorMaterializationReservationSite::FromDescriptorRoots,
    )?;
    let pin_count = vm.pin_many(&roots);
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
    if matches!(target, Value::Object(_)) {
        if !set_integrity_level(vm, &target, true)? {
            return Err(Error::type_err("Object.freeze failed to freeze object"));
        }
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
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Object.defineProperty target must be an object",
        ));
    }
    let key_input = args.get(1).cloned().unwrap_or(Value::Undefined);
    let desc_input = args.get(2).cloned().unwrap_or(Value::Undefined);
    reserve_descriptor_materialization_roots(
        vm,
        &[target.clone(), key_input.clone(), desc_input.clone()],
        #[cfg(test)]
        crate::vm::DescriptorMaterializationReservationSite::DefineOperationRoots,
    )?;
    let argument_pins = vm.pin_many(&[target.clone(), key_input.clone(), desc_input.clone()]);
    let result = (|| -> error::Result<bool> {
        let key = to_property_key_descriptor(vm, &key_input)?;
        let desc = to_property_descriptor_record(vm, &desc_input)?;
        object_define_property_record_result(vm, &target, &key, &desc, throw_on_failure, false)
    })();
    vm.unpin_many(argument_pins);
    result
}

fn object_define_property_record_result(
    vm: &mut Vm,
    target: &Value,
    key: &PropertyKey,
    desc: &crate::vm::ProxyDefinePropertyDescriptor,
    throw_on_failure: bool,
    integrity_mode: bool,
) -> error::Result<bool> {
    let target = target.clone();
    let key = key.clone();
    let value = desc.descriptor.value.clone();
    let writable = desc.descriptor.writable;
    let enumerable = desc.descriptor.enumerable;
    let configurable = desc.descriptor.configurable;
    let get = desc.descriptor.get.clone();
    let set = desc.descriptor.set.clone();
    let has_value = desc.has_value;
    let has_writable = desc.has_writable;
    let has_enumerable = desc.has_enumerable;
    let has_configurable = desc.has_configurable;
    let has_get = desc.has_get;
    let has_set = desc.has_set;
    let is_accessor = has_get || has_set;
    let is_data = has_value || has_writable;
    if let Value::Object(idx) = target {
        let ordinary_target = match vm.proxy_define_own_property(&target, &key, desc)? {
            crate::vm::ProxyDefinePropertyOutcome::Ordinary(target) => target,
            crate::vm::ProxyDefinePropertyOutcome::Complete(result) => {
                if !result && throw_on_failure {
                    return Err(Error::type_err("Proxy defineProperty trap returned false"));
                }
                return Ok(result);
            }
        };
        let idx = match ordinary_target.clone() {
            Value::Object(idx) => idx,
            _ => unreachable!("DefineOwnProperty target remains an object"),
        };
        let target = ordinary_target;
        let is_array_length = key.as_str().is_some_and(|name| name == "length")
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
        let is_namespace_string = key.as_str().is_some()
            && vm.heap.with_obj(idx.0, |object| {
                matches!(object, HeapObj::ModuleNamespace(_))
            });
        if is_namespace_string {
            let is_integrity_descriptor = !has_value
                && !has_enumerable
                && !has_get
                && !has_set
                && has_configurable
                && !configurable
                && (!has_writable || !writable);
            if integrity_mode && is_integrity_descriptor {
                let current = integrity_descriptor_for_key(vm, &target, &key)?;
                let success = current.is_some_and(|current| {
                    !current.configurable && (!has_writable || current.writable == Some(writable))
                });
                if !success && throw_on_failure {
                    return Err(Error::type_err("Cannot redefine module namespace property"));
                }
                return Ok(success);
            }
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
        let is_integrity_descriptor = !has_value
            && !has_enumerable
            && !has_get
            && !has_set
            && has_configurable
            && !configurable
            && (!has_writable || !writable);
        if integrity_mode && is_integrity_descriptor {
            let success =
                vm.define_integrity_attributes(idx, &key, has_writable.then_some(writable))?;
            if !success && throw_on_failure {
                return Err(Error::type_err("Cannot define missing integrity property"));
            }
            return Ok(success);
        }
        if key.array_index().is_some_and(|index| {
            vm.array_index_blocked_by_non_writable_length(idx.0, index as usize)
        }) {
            if throw_on_failure {
                return Err(Error::type_err(
                    "Cannot define Array index with non-writable length",
                ));
            }
            return Ok(false);
        }
        let current = own_property_descriptor_for_key(vm, &target, &key);
        if current.is_none() {
            let extensible = vm.heap.with_obj(idx.0, |obj| obj.is_extensible());
            if !extensible {
                if throw_on_failure {
                    return Err(Error::type_err(format!(
                        "Cannot define property '{}', object is not extensible",
                        key.as_str()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| "Symbol".to_string())
                    )));
                }
                return Ok(false);
            }
        }
        let descriptor = if let Some(mut current) = current {
            if !current.configurable {
                if has_configurable && configurable {
                    if throw_on_failure {
                        return Err(Error::type_err("Cannot redefine non-configurable property"));
                    }
                    return Ok(false);
                }
                if has_enumerable && enumerable != current.enumerable {
                    if throw_on_failure {
                        return Err(Error::type_err("Cannot redefine non-configurable property"));
                    }
                    return Ok(false);
                }
                if is_accessor != current.is_accessor && (is_accessor || is_data) {
                    if throw_on_failure {
                        return Err(Error::type_err("Cannot redefine non-configurable property"));
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
        vm.publish_ordinary_property_storage(idx, &key, descriptor, has_value, has_writable)?;
    }
    Ok(true)
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

pub(crate) fn suppressed_error_constructor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let error = args.first().cloned().unwrap_or(Value::Undefined);
    let suppressed = args.get(1).cloned().unwrap_or(Value::Undefined);
    let message = args.get(2).cloned().unwrap_or(Value::Undefined);
    let values = [error, suppressed, message];
    let required_roots = values.iter().try_fold(1usize, |count, value| {
        count
            .checked_add(Vm::value_root_count(value))
            .ok_or_else(|| Error::range("temporary root set is too large"))
    })?;
    vm.try_reserve_gc_pins(required_roots)?;
    let mut pin_count = vm.pin_many(&values);
    let idx = match error_object_for_constructor(vm, this) {
        Ok(idx) => idx,
        Err(error) => {
            vm.unpin_many(pin_count);
            return Err(error);
        }
    };
    pin_count += vm.pin(&Value::Object(idx));

    let result = (|| {
        if !matches!(values[2], Value::Undefined) {
            let message = vm.to_string(&values[2])?;
            vm.define_own_property_or_throw(
                &Value::Object(idx),
                PropertyKey::from("message"),
                data_prop(Value::String(message)),
            )?;
        }
        vm.define_own_property_or_throw(
            &Value::Object(idx),
            PropertyKey::from("error"),
            data_prop(values[0].clone()),
        )?;
        vm.define_own_property_or_throw(
            &Value::Object(idx),
            PropertyKey::from("suppressed"),
            data_prop(values[1].clone()),
        )?;
        Ok(Value::Object(idx))
    })();
    vm.unpin_many(pin_count);
    result
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
        "SuppressedError",
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
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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

    let iterator_key = PropertyKey::symbol(vm.well_known_symbols.iterator);
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
                set_physical_index: Mutex::new(0),
                set_compaction_epoch: Mutex::new(u64::MAX),
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(Some(proto)),
                extensible: AtomicBool::new(true),
            },
        ))?);
        vm.keep_during_job(&wrapper)?;
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
        vm.keep_during_job(&result)?;
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
    make_value_array_in_env(vm, values, realm)
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
        let iterator_key = PropertyKey::symbol(vm.well_known_symbols.iterator);
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
    let iterator_key = PropertyKey::symbol(vm.well_known_symbols.iterator);
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
    let iterator_key = PropertyKey::symbol(vm.well_known_symbols.iterator);
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
        PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                accessor_prop(Value::Object(tag_get), Value::Object(tag_set)),
            );
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.iterator),
                data_prop(Value::Object(iterator_fn)),
            );
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.dispose),
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
    let function_proto_idx = vm.new_native_function("", function_proto_noop, 0)?;
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
    let intl = intl::build_intl_in_env(vm, vm.global, vm.object_proto.clone())?;
    define_global(vm, "Intl", intl);
    // console
    let console = build_console(vm)?;
    define_global(vm, "console", console);
    // JSON
    let json = build_json(vm)?;
    define_global(vm, "JSON", json);
    // Reflect
    let reflect = build_reflect(vm)?;
    define_global(vm, "Reflect", reflect);
    install_temporal_namespace_in_env(vm, vm.global, None, vm.object_proto.clone())?;

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
    install_annex_b_string_methods_in_env(vm, vm.global, str_proto)?;
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
        ("escape", global_escape as NativeFn),
        ("unescape", global_unescape as NativeFn),
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
                        PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
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
            PropertyKey::symbol(vm.well_known_symbols.has_instance),
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
    install_shadow_realm_intrinsic_in_env(vm, vm.global, None)?;
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
