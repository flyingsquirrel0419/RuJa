//! Built-in objects and globals for the RuJa VM.
//!
//! All built-in constructors, prototypes, and global functions are registered
//! here. Native functions follow the `NativeFn` signature used by the VM.

pub(crate) mod global;
pub(crate) mod json;
pub(crate) mod math;

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
    async_generator_function_constructor, bigint_as_int_n, bigint_as_uint_n, bigint_to_string,
    bigint_value_of, function_constructor, generator_function_constructor, global_bigint,
    global_eval, global_is_finite, global_is_nan, global_parse_float, global_parse_int,
};
pub(crate) use json::{
    build_json, build_reflect, date_constructor, date_get_component, date_get_time,
    date_get_timezone_offset, date_now, date_parse, date_set_component, date_to_iso_string,
    date_to_json, date_to_string, date_to_temporal_instant, date_utc,
};
pub(crate) use math::{build_console, build_math};
pub(crate) use proxy::*;
pub(crate) use typed_array::*;

use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{
    ArrayData, BindingKind, CollectionIteratorData, CollectionIteratorKind, FunctionData,
    FunctionKind, GcIdx, HeapObj, MapData, MapKey, ObjectData, PropertyDescriptor, PropertyKey,
    RegExpStringIteratorData, SetData, Value,
};
use crate::vm::{NativeFn, Vm};
use indexmap::{IndexMap, IndexSet};
use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_rational::Ratio;
use num_traits::{Signed, ToPrimitive, Zero};
use regex::{Regex as RustRegex, RegexBuilder as RustRegexBuilder};
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

    fn apply_ecmascript_capture_clearing(&mut self, source: &str, flags: &str, input: &str) {
        let rules = regex_repeated_capture_clear_rules(source);
        if rules.quantified_groups.is_empty() {
            return;
        }
        let Some(full_match) = self.get(0) else {
            return;
        };
        let mut quantified_spans = Vec::new();
        for group in &rules.quantified_groups {
            let span = match group.capture_index {
                Some(capture_index) => self
                    .groups
                    .get(capture_index)
                    .copied()
                    .flatten()
                    .map(|m| (m.start(), m.end())),
                None => regex_final_iteration_span(&group.body, flags, input, full_match),
            };
            quantified_spans.push((group.group_id, span));
        }
        for capture_index in 1..self.groups.len() {
            let Some(capture) = self.groups[capture_index] else {
                continue;
            };
            let should_clear = rules
                .ancestors
                .get(capture_index)
                .into_iter()
                .flatten()
                .any(|ancestor| {
                    quantified_spans
                        .iter()
                        .find(|(group_id, _)| *group_id == ancestor.group_id)
                        .and_then(|(_, span)| *span)
                        .is_some_and(|(start, end)| capture.start < start || capture.end > end)
                });
            if should_clear {
                self.groups[capture_index] = None;
            }
        }
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
    let capture_count = regex_capture_count(source);
    let backend_source = normalize_regex_for_backend(source, flags, capture_count);
    if regex_uses_backreference(source, capture_count) {
        let mut b = fancy_regex::RegexBuilder::new(&backend_source);
        b.case_insensitive(flags.contains('i'));
        b.multi_line(flags.contains('m'));
        b.dot_matches_new_line(flags.contains('s'));
        return b
            .build()
            .map(CompiledRegex::Fancy)
            .map_err(|e| e.to_string());
    }

    let mut b = RustRegexBuilder::new(&backend_source);
    b.case_insensitive(flags.contains('i'));
    b.multi_line(flags.contains('m'));
    b.dot_matches_new_line(flags.contains('s'));
    b.build()
        .map(CompiledRegex::Rust)
        .map_err(|e| e.to_string())
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
            CompiledRegex::Rust(re) => Ok(re.find_at(input, start).map(CompiledMatch::from)),
            CompiledRegex::Fancy(re) => re
                .find_from_pos(input, start)
                .map(|m| m.map(CompiledMatch::from))
                .map_err(regex_runtime_error),
        }
    }

    fn find_iter<'t>(&self, input: &'t str) -> error::Result<Vec<CompiledMatch<'t>>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.find_iter(input).map(CompiledMatch::from).collect()),
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

    fn captures_ecma<'t>(
        &self,
        input: &'t str,
        source: &str,
        flags: &str,
    ) -> error::Result<Option<CompiledCaptures<'t>>> {
        self.captures_at_ecma(input, 0, source, flags)
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
        }
    }

    fn captures_at_ecma<'t>(
        &self,
        input: &'t str,
        start: usize,
        source: &str,
        flags: &str,
    ) -> error::Result<Option<CompiledCaptures<'t>>> {
        let mut captures = self.captures_at(input, start)?;
        if let Some(caps) = captures.as_mut() {
            caps.apply_ecmascript_capture_clearing(source, flags, input);
        }
        Ok(captures)
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
        }
    }

    fn captures_iter_ecma<'t>(
        &self,
        input: &'t str,
        source: &str,
        flags: &str,
    ) -> error::Result<Vec<CompiledCaptures<'t>>> {
        let mut captures = self.captures_iter(input)?;
        for caps in &mut captures {
            caps.apply_ecmascript_capture_clearing(source, flags, input);
        }
        Ok(captures)
    }

    fn replace<'t>(&self, input: &'t str, replacement: &str) -> error::Result<Cow<'t, str>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.replace(input, replacement)),
            CompiledRegex::Fancy(_) => self.replace_fancy(input, replacement, false),
        }
    }

    fn replace_all<'t>(&self, input: &'t str, replacement: &str) -> error::Result<Cow<'t, str>> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.replace_all(input, replacement)),
            CompiledRegex::Fancy(_) => self.replace_fancy(input, replacement, true),
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

fn normalize_regex_for_backend(source: &str, flags: &str, capture_count: usize) -> String {
    if source == "[]" {
        return r"[^\s\S]".to_string();
    }
    if source == "[^]" {
        return if flags.contains('u') {
            "(?s:.)".to_string()
        } else {
            r"[\x00-\u{ffff}\u{f0000}-\u{f07ff}]".to_string()
        };
    }
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_class = false;
    let mut escaped = false;
    let unicode_mode = flags.contains('u') || flags.contains('v');
    let protect_non_unicode_case = flags.contains('i') && !flags.contains('u');
    let mut modifier_stack = vec![RegexModifierState {
        dot_all: flags.contains('s'),
        ignore_case: flags.contains('i'),
    }];

    while let Some(ch) = chars.next() {
        if escaped {
            if ch.is_ascii_digit() && ch != '0' {
                let mut digits = String::from(ch);
                while matches!(chars.peek(), Some(next) if next.is_ascii_digit()) {
                    digits.push(chars.next().unwrap());
                }
                let value = digits.parse::<usize>().unwrap_or(usize::MAX);
                if flags.contains('u') || (!in_class && value > 0 && value <= capture_count) {
                    out.push_str(&digits);
                } else {
                    out.pop();
                    push_legacy_decimal_escape_for_backend(&mut out, &digits);
                }
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
                    'w' => out.push_str(r"(?-iu:\w)"),
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
                        let scalar = 0x10000 + ((lead - 0xd800) << 10) + (trail - 0xdc00);
                        out.pop();
                        out.push_str("\\u{");
                        out.push_str(&format!("{scalar:x}"));
                        out.push('}');
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
                } else if protect_non_unicode_case && !in_class && lead > 0x7f {
                    out.pop();
                    out.push_str("(?-i:\\u");
                    out.push_str(&lead_hex);
                    out.push(')');
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
            } else if ch == 'x' && !has_exact_hex_escape(&chars, 2) {
                out.pop();
                push_regex_literal_for_backend(&mut out, ch);
            } else if protect_non_unicode_case && !in_class && ch == 'u' {
                let mut hex = String::new();
                for _ in 0..4 {
                    match chars.peek().copied() {
                        Some(next) if next.is_ascii_hexdigit() => {
                            hex.push(chars.next().unwrap());
                        }
                        _ => break,
                    }
                }
                if hex.len() == 4 {
                    let code = u32::from_str_radix(&hex, 16).unwrap_or(0);
                    if code > 0x7f {
                        out.pop();
                        out.push_str("(?-i:\\u");
                        out.push_str(&hex);
                        out.push(')');
                    } else {
                        out.push('u');
                        out.push_str(&hex);
                    }
                } else {
                    out.push(ch);
                    out.push_str(&hex);
                }
            } else if protect_non_unicode_case && !in_class && ch == 'x' {
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
                    if code > 0x7f {
                        out.pop();
                        out.push_str("(?-i:\\x");
                        out.push_str(&hex);
                        out.push(')');
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
                push_regex_literal_for_backend(&mut out, ch);
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
            in_class = true;
            out.push(ch);
            continue;
        }
        if ch == ']' && in_class {
            in_class = false;
            out.push(ch);
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
            modifier_stack.push(*modifier_stack.last().unwrap());
            out.push(ch);
            continue;
        }

        if !in_class && ch == ')' {
            if modifier_stack.len() > 1 {
                modifier_stack.pop();
            }
            out.push(ch);
            continue;
        }

        if protect_non_unicode_case && !in_class && !ch.is_ascii() {
            out.push_str("(?-i:");
            out.push(ch);
            out.push(')');
        } else {
            out.push(ch);
        }
    }

    out
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

fn regex_capture_names(source: &str) -> Vec<RegexCaptureName> {
    let mut names = Vec::new();
    let mut capture_index = 0;
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
                    capture_index += 1;
                    continue;
                }
                let mut lookahead = chars.clone();
                lookahead.next();
                if lookahead.next() == Some('<') && !matches!(lookahead.peek(), Some('=' | '!')) {
                    capture_index += 1;
                    let mut name = String::new();
                    for next in lookahead.by_ref() {
                        if next == '>' {
                            break;
                        }
                        name.push(next);
                    }
                    if !name.is_empty() {
                        names.push(RegexCaptureName {
                            name: Arc::from(name.as_str()),
                            index: capture_index,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    names
}

fn named_capture_index(names: &[RegexCaptureName], name: &str) -> Option<usize> {
    names
        .iter()
        .find(|capture| capture.name.as_ref() == name)
        .map(|capture| capture.index)
}

fn make_regexp_groups_object(
    vm: &mut Vm,
    caps: &CompiledCaptures<'_>,
    names: &[RegexCaptureName],
) -> error::Result<Value> {
    if names.is_empty() {
        return Ok(Value::Undefined);
    }
    let obj_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(None),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Object")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.heap.with_obj(obj_idx, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        for capture in names {
            let value = caps
                .get(capture.index)
                .map(|m| Value::String(Arc::from(m.as_str())))
                .unwrap_or(Value::Undefined);
            props.insert(
                PropertyKey::from(capture.name.clone()),
                PropertyDescriptor::data(value),
            );
        }
    });
    Ok(Value::Object(GcIdx(obj_idx)))
}

struct RegexCaptureClearRules {
    ancestors: Vec<Vec<RegexGroupAncestor>>,
    quantified_groups: Vec<RegexQuantifiedGroup>,
}

#[derive(Clone, Copy)]
struct RegexGroupAncestor {
    group_id: usize,
}

struct RegexQuantifiedGroup {
    group_id: usize,
    capture_index: Option<usize>,
    body: String,
}

struct RegexGroupFrame {
    group_id: usize,
    capture_index: Option<usize>,
    body_start: usize,
}

fn regex_repeated_capture_clear_rules(source: &str) -> RegexCaptureClearRules {
    let chars: Vec<char> = source.chars().collect();
    let mut ancestors = vec![Vec::new()];
    let mut quantified_groups = Vec::new();
    let mut stack: Vec<RegexGroupFrame> = Vec::new();
    let mut group_count = 0;
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
                group_count += 1;
                let group_id = group_count;
                let capture_index = if regex_group_is_capturing_chars(&chars, i) {
                    capture_count += 1;
                    let group_ancestors = stack
                        .iter()
                        .map(|frame| RegexGroupAncestor {
                            group_id: frame.group_id,
                        })
                        .collect();
                    ancestors.push(group_ancestors);
                    Some(capture_count)
                } else {
                    None
                };
                stack.push(RegexGroupFrame {
                    group_id,
                    capture_index,
                    body_start: regex_group_body_start_chars(&chars, i),
                });
            }
            ')' if !in_class => {
                if let Some(frame) = stack.pop() {
                    if regex_quantifier_starts_at_chars(&chars, i + 1) {
                        quantified_groups.push(RegexQuantifiedGroup {
                            group_id: frame.group_id,
                            capture_index: frame.capture_index,
                            body: chars[frame.body_start..i].iter().collect(),
                        });
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    RegexCaptureClearRules {
        ancestors,
        quantified_groups,
    }
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

fn regex_group_body_start_chars(chars: &[char], idx: usize) -> usize {
    if chars.get(idx) != Some(&'(') {
        return idx;
    }
    if chars.get(idx + 1) != Some(&'?') {
        return idx + 1;
    }
    match chars.get(idx + 2) {
        Some(':') | Some('=') | Some('!') => idx + 3,
        Some('<') if matches!(chars.get(idx + 3), Some('=' | '!')) => idx + 4,
        Some('<') => {
            let mut cursor = idx + 3;
            while cursor < chars.len() {
                if chars[cursor] == '>' {
                    return cursor + 1;
                }
                cursor += 1;
            }
            idx + 2
        }
        _ => {
            let mut cursor = idx + 2;
            while cursor < chars.len() {
                match chars[cursor] {
                    ':' => return cursor + 1,
                    ')' => break,
                    _ => cursor += 1,
                }
            }
            idx + 2
        }
    }
}

fn regex_final_iteration_span(
    body: &str,
    flags: &str,
    input: &str,
    full_match: CompiledMatch<'_>,
) -> Option<(usize, usize)> {
    if body.is_empty() {
        return None;
    }
    let re = compile_regex(body, flags).ok()?;
    let mut last = None;
    for m in re.find_iter(input).ok()? {
        if m.start() >= full_match.start()
            && m.end() <= full_match.end()
            && (m.start() != m.end() || last.is_none())
        {
            last = Some((m.start(), m.end()));
        }
    }
    last
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

fn regex_uses_backreference(source: &str, capture_count: usize) -> bool {
    if capture_count == 0 {
        return false;
    }
    let mut chars = source.chars().peekable();
    let mut in_class = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if escaped {
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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use std::sync::Arc;

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
    if let Some(proto) = vm.current_native_new_target_prototype.clone() {
        if matches!(proto, Value::Object(_)) {
            return Ok(proto);
        }
    } else if let Some(new_target) = vm.current_native_new_target.clone() {
        let proto = vm.get_property_by_key(&new_target, &PropertyKey::from("prototype"))?;
        if matches!(proto, Value::Object(_)) {
            return Ok(proto);
        }
    }
    if let Some(new_target) = vm.current_native_new_target.clone() {
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

pub(crate) fn is_callable(value: &Value, heap: &Heap) -> bool {
    match value {
        Value::Object(idx) => heap.with_obj(idx.0, |obj| match obj {
            HeapObj::Function(_) => true,
            HeapObj::Proxy(proxy) => is_callable(&proxy.target, heap),
            _ => false,
        }),
        _ => false,
    }
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
    if let Value::String(_) = &this {
        return Ok(Value::String(Arc::from("[object String]")));
    }
    if let Value::Number(_) = &this {
        return Ok(Value::String(Arc::from("[object Number]")));
    }
    if let Value::Bool(_) = &this {
        return Ok(Value::String(Arc::from("[object Boolean]")));
    }
    if let Value::Symbol(_) = &this {
        return Ok(Value::String(Arc::from("[object Symbol]")));
    }
    if let Value::BigInt(_) = &this {
        return Ok(Value::String(Arc::from("[object BigInt]")));
    }
    if let Value::Object(idx) = &this {
        let class = if let Some(hint) = class_hint {
            hint.to_string()
        } else {
            vm.heap.with_obj(idx.0, |obj| obj.class_name().to_string())
        };
        let result = format!("[object {}]", class);
        return Ok(Value::String(Arc::from(result.as_str())));
    }
    Ok(Value::String(Arc::from("[object Object]")))
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
        },
        closure: vm.global,
        lexical_new_target: Value::Undefined,
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
    let ctor_idx = vm.new_native_function_in_env(name, constructor, 3, env)?;
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
    let typed_array_ctor = Value::Object(vm.new_native_function_in_env(
        "TypedArray",
        typed_array_intrinsic_constructor,
        0,
        env,
    )?);
    let typed_array_proto =
        Value::Object(GcIdx(vm.heap.allocate(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("TypedArray")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?));
    if let Value::Object(idx) = &typed_array_ctor {
        let typed_array_from_fn =
            vm.new_native_function_in_env("from", typed_array_from, 1, env)?;
        let typed_array_of_fn = vm.new_native_function_in_env("of", typed_array_of, 0, env)?;
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
    let typed_array_subarray_fn =
        vm.new_native_function_in_env("subarray", typed_array_subarray, 2, env)?;
    let typed_array_fill_fn = vm.new_native_function_in_env("fill", typed_array_fill, 1, env)?;
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
                PropertyKey::from("subarray"),
                data_prop(Value::Object(typed_array_subarray_fn)),
            );
            props.insert(
                PropertyKey::from("fill"),
                data_prop(Value::Object(typed_array_fill_fn)),
            );
        });
    }

    Ok((typed_array_ctor, typed_array_proto))
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
        &[
            ("slice", array_buffer_slice, 2),
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
    if update_vm_slot {
        vm.array_buffer_proto = array_buffer_proto.clone();
    }
    let array_buffer_byte_length_getter =
        vm.new_native_function_in_env("get byteLength", array_buffer_byte_length_get, 0, env)?;
    let array_buffer_immutable_getter =
        vm.new_native_function_in_env("get immutable", array_buffer_immutable_get, 0, env)?;
    let array_buffer_detached_getter =
        vm.new_native_function_in_env("get detached", array_buffer_detached_get, 0, env)?;
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
        &[("slice", shared_array_buffer_slice, 2)],
        env,
    )?;
    let byte_length_getter = vm.new_native_function_in_env(
        "get byteLength",
        shared_array_buffer_byte_length_get,
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
    for function in [constructor, byte_length_getter, species_getter] {
        set_function_object_proto(vm, function, &function_proto);
    }
    let slice = vm.heap.with_obj(prototype.0, |obj| {
        obj.props()
            .lock()
            .get(&PropertyKey::from("slice"))
            .map(|descriptor| descriptor.value.clone())
    });
    if let Some(Value::Object(slice)) = slice {
        set_function_object_proto(vm, slice, &function_proto);
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
    let proto_parent = if name == "Error" {
        vm.object_proto.clone()
    } else if matches!(vm.error_proto, Value::Object(_)) {
        vm.error_proto.clone()
    } else {
        vm.object_proto.clone()
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
        },
        closure: env,
        lexical_new_target: Value::Undefined,
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(Some(Value::Object(proto_idx))),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
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
    let (regex_ctor, regex_proto) = make_builtin_constructor_with_in_env(
        vm,
        "RegExp",
        2,
        regexp_constructor,
        &[
            ("test", regexp_test, 1),
            ("exec", regexp_exec, 1),
            ("toString", regexp_to_string, 0),
        ],
        env,
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

fn make_test262_realm(vm: &mut Vm) -> error::Result<Value> {
    let realm_env = crate::environment::new_env(&vm.heap, None, true)?;
    let global_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("realm-global")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let global = Value::Object(GcIdx(global_idx));

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
    if let Some(object) = crate::environment::get(&vm.heap, vm.global, "Object") {
        define_realm_global(vm, realm_env, &global, "Object", object);
    }
    if let Some(array) = crate::environment::get(&vm.heap, vm.global, "Array") {
        define_realm_global(vm, realm_env, &global, "Array", array);
    }
    if let Some(bigint) = crate::environment::get(&vm.heap, vm.global, "BigInt") {
        define_realm_global(vm, realm_env, &global, "BigInt", bigint);
    }
    if let Some(proxy) = crate::environment::get(&vm.heap, vm.global, "Proxy") {
        define_realm_global(vm, realm_env, &global, "Proxy", proxy);
    }
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
    let realm_function_proto_idx =
        vm.new_native_function_in_env("Function.prototype", function_proto_noop, 0, realm_env)?;
    let realm_function_proto = Value::Object(realm_function_proto_idx);
    vm.heap.with_obj(realm_function_proto_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            *f.proto.lock() = Some(vm.object_proto.clone());
        }
    });
    vm.realm_function_prototypes
        .insert(realm_env.0, realm_function_proto.clone());

    let function_ctor_idx =
        vm.new_native_function_in_env("Function", function_constructor, 1, realm_env)?;
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
    let (str_ctor, str_proto) = make_builtin_constructor_with_in_env(
        vm,
        "String",
        1,
        string_constructor,
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
    define_realm_global(vm, realm_env, &global, "String", Value::Object(str_ctor));

    let (num_ctor, num_proto) = make_builtin_constructor_with_in_env(
        vm,
        "Number",
        1,
        number_constructor,
        &[
            ("toString", num_proto_to_string, 1),
            ("toLocaleString", num_proto_to_string, 0),
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
        &[
            ("valueOf", boolean_value_of, 0),
            ("toString", boolean_to_string, 0),
        ],
        realm_env,
    )?;
    vm.set_primitive(&Value::Object(bool_proto), Value::Bool(false));
    define_realm_global(vm, realm_env, &global, "Boolean", Value::Object(bool_ctor));

    let (regexp_ctor, _) = make_regexp_constructor_in_env(vm, realm_env)?;
    define_realm_global(vm, realm_env, &global, "RegExp", Value::Object(regexp_ctor));
    let symbol_idx = vm.new_native_function_in_env("Symbol", symbol_constructor, 0, realm_env)?;
    let symbol_for_idx = vm.new_native_function_in_env("for", symbol_for, 1, realm_env)?;
    let symbol_key_for_idx =
        vm.new_native_function_in_env("keyFor", symbol_key_for, 1, realm_env)?;
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
            const_prop(vm.symbol_proto.clone()),
        );
        drop(props);
        if let HeapObj::Function(function) = obj {
            *function.prototype.lock() = Some(vm.symbol_proto.clone());
        }
    });
    define_realm_global(vm, realm_env, &global, "Symbol", Value::Object(symbol_idx));

    install_array_buffer_constructor_in_env(vm, realm_env, Some(&global), false)?;
    install_shared_array_buffer_constructor_in_env(vm, realm_env, Some(&global))?;
    install_data_view_constructor_in_env(vm, realm_env, Some(&global))?;
    let (typed_array_ctor, typed_array_proto) = make_typed_array_intrinsic_in_env(vm, realm_env)?;
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
    install_atomics_in_env(vm, realm_env, Some(&global))?;
    install_weak_ref_constructor_in_env(vm, realm_env, Some(&global))?;
    install_finalization_registry_constructor_in_env(vm, realm_env, Some(&global))?;

    Ok(global)
}

fn test262_create_realm(vm: &mut Vm, _args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let global = make_test262_realm(vm)?;
    let realm = vm.new_object()?;
    vm.heap.with_obj(realm.0, |obj| {
        obj.props()
            .lock()
            .insert(PropertyKey::from("global"), data_prop(global));
    });
    Ok(Value::Object(realm))
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
            if let Err(error) = worker.run(&source) {
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

fn object_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        if args.is_empty() {
            return Ok(Value::Object(idx));
        }
        let first = args.first().unwrap_or(&Value::Undefined);
        match first {
            Value::Undefined | Value::Null => {}
            Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Symbol(_)
            | Value::BigInt(_) => {
                return vm.to_object(first);
            }
            Value::Object(_) => return Ok(first.clone()),
            Value::PrivateName(_) => {
                return Err(Error::type_err("Private name is not an object".to_string()))
            }
            Value::Reference(_) => {
                return Err(Error::type_err("Reference is not an object".to_string()))
            }
        }
        let new_idx = vm.new_object()?;
        return Ok(Value::Object(new_idx));
    }
    // Called as function
    if args.is_empty() {
        let new_idx = vm.new_object()?;
        return Ok(Value::Object(new_idx));
    }
    let first = args.first().unwrap_or(&Value::Undefined);
    match first {
        Value::Undefined | Value::Null => {
            let new_idx = vm.new_object()?;
            Ok(Value::Object(new_idx))
        }
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

fn object_has_own_key(vm: &Vm, obj: &Value, key: &PropertyKey) -> bool {
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
            return desc.is_some();
        }
    }

    match obj {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |heap_obj| {
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
                    return key
                        .as_str()
                        .and_then(|name| name.parse::<usize>().ok())
                        .is_some_and(|i| i < crate::value::utf16_len(&s));
                }
            }
            false
        }),
        Value::String(s) => {
            if key.as_str() == Some("length") {
                return true;
            }
            key.as_str()
                .and_then(|name| name.parse::<usize>().ok())
                .is_some_and(|i| i < crate::value::utf16_len(s))
        }
        _ => false,
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
    Ok(Value::Bool(object_has_own_key(vm, &this, &key)))
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
    Ok(Value::Bool(object_has_own_key(vm, &obj, &key)))
}

fn object_property_is_enumerable(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let key = match args.first() {
        Some(Value::Symbol(id)) => PropertyKey::Symbol(*id),
        Some(v) => PropertyKey::from(vm.to_property_key(v)?),
        None => PropertyKey::from(""),
    };
    match &this {
        Value::Object(idx) => {
            let enumerable = vm.heap.with_obj(idx.0, |obj| {
                if let HeapObj::Array(a) = obj {
                    if key.as_str() == Some("length") {
                        return false;
                    }
                    if let Some(name) = key.as_str() {
                        if let Some(i) = crate::value::parse_array_index(name) {
                            return a.is_dense_present(i);
                        }
                    }
                }
                if let HeapObj::Object(od) = obj {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        if let Some(name) = key.as_str() {
                            if let Ok(i) = name.parse::<usize>() {
                                return i.to_string() == name && i < crate::value::utf16_len(&s);
                            }
                        }
                    }
                }
                obj.props()
                    .lock()
                    .get(&key)
                    .is_some_and(|desc| desc.enumerable)
            });
            Ok(Value::Bool(enumerable))
        }
        Value::String(s) => {
            let enumerable = key
                .as_str()
                .and_then(|name| name.parse::<usize>().ok().filter(|i| i.to_string() == name))
                .is_some_and(|i| i < crate::value::utf16_len(s));
            Ok(Value::Bool(enumerable))
        }
        _ => Ok(Value::Bool(false)),
    }
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
    let idx = vm.new_object()?;
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
    Ok(Value::Object(idx))
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
    let key = get_arg(args, 0);
    let desc = legacy_accessor_descriptor(vm, slot, accessor)?;
    let define_args = [object, key, desc];
    object_define_property_result(vm, &define_args, true)?;
    Ok(Value::Undefined)
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
    for _ in 0..1024 {
        if let Some(desc) = own_property_descriptor_for_key(vm, &object, &key) {
            if desc.is_accessor {
                return Ok(match slot {
                    "get" => desc.get.unwrap_or(Value::Undefined),
                    "set" => desc.set.unwrap_or(Value::Undefined),
                    _ => Value::Undefined,
                });
            }
            return Ok(Value::Undefined);
        }
        let next = match object {
            Value::Object(idx) => vm.heap.with_obj(idx.0, |obj| obj.proto().lock().clone()),
            _ => None,
        };
        match next {
            Some(next @ Value::Object(_)) => object = next,
            _ => return Ok(Value::Undefined),
        }
    }
    Ok(Value::Undefined)
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

pub(crate) fn own_property_keys(
    vm: &mut Vm,
    obj: &Value,
    enumerable_only: bool,
    include_strings: bool,
    include_symbols: bool,
) -> Vec<PropertyKey> {
    if let Value::Object(idx) = obj {
        if let Some((target, handler)) = vm.heap.with_obj(idx.0, |heap_obj| {
            if let HeapObj::Proxy(proxy) = heap_obj {
                if *proxy.revoked.lock() {
                    None
                } else {
                    Some((proxy.target.clone(), proxy.handler.clone()))
                }
            } else {
                None
            }
        }) {
            if let Ok(trap) = vm.get_property(&handler, "ownKeys") {
                if !trap.is_undefined() {
                    if let Ok(key_list) =
                        vm.call_function(&trap, std::slice::from_ref(&target), Some(handler))
                    {
                        let items = if let Value::Object(list_idx) = &key_list {
                            vm.heap.with_obj(list_idx.0, |o| {
                                if let HeapObj::Array(a) = o {
                                    return Some(a.items.lock().clone());
                                }
                                None
                            })
                        } else {
                            None
                        };
                        if let Some(items) = items {
                            let mut keys = Vec::new();
                            let mut seen = IndexSet::new();
                            for item in items {
                                let Ok(key) = to_property_key_descriptor(vm, &item) else {
                                    continue;
                                };
                                if enumerable_only
                                    && !own_property_descriptor_for_key(vm, &target, &key)
                                        .is_some_and(|desc| desc.enumerable)
                                {
                                    continue;
                                }
                                match key {
                                    PropertyKey::Str(_) if include_strings => {
                                        push_unique_key(&mut keys, &mut seen, key);
                                    }
                                    PropertyKey::Symbol(_) if include_symbols => {
                                        push_unique_key(&mut keys, &mut seen, key);
                                    }
                                    _ => {}
                                }
                            }
                            return keys;
                        }
                    }
                }
            }
            return own_property_keys(
                vm,
                &target,
                enumerable_only,
                include_strings,
                include_symbols,
            );
        }
    }

    let mut keys = Vec::new();
    let mut seen = IndexSet::new();
    let typed_array_index_count = include_strings
        .then(|| vm.typed_array_integer_index_own_property_key_count(obj))
        .flatten();
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

            if let HeapObj::Map(m) = o {
                if include_strings {
                    for (k, _) in m.entries.lock().iter().map(|(k, v)| (&k.0, v)) {
                        if let Value::String(s) = k {
                            string_keys.push(PropertyKey::from(s.clone()));
                        }
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
    keys
}

/// Collect an object's own enumerable string keys in array-index-first then property order.
pub(crate) fn own_string_keys(vm: &mut Vm, obj: &Value) -> Vec<Arc<str>> {
    own_property_keys(vm, obj, true, true, false)
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::Str(s) => Some(s),
            PropertyKey::Symbol(_) => None,
        })
        .collect()
}

pub(crate) fn make_value_array(vm: &mut Vm, items: Vec<Value>) -> error::Result<Value> {
    let arr = HeapObj::Array(ArrayData::new(items, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr)?)))
}
pub(crate) fn norm_idx(n: f64, len: f64) -> f64 {
    if n < 0.0 {
        (len + n).max(0.0)
    } else {
        n.min(len)
    }
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
    let keys = own_string_keys(vm, &obj);
    make_str_array(vm, keys)
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
    for key in &keys {
        let Some(k) = key.as_str() else {
            continue;
        };
        if !own_property_descriptor_for_key_or_throw(vm, &obj, key)?
            .is_some_and(|desc| desc.enumerable)
        {
            continue;
        }
        vals.push(vm.get_property(&obj, k)?);
    }
    let arr = HeapObj::Array(ArrayData::new(vals, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr)?)))
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
    let mut pairs = Vec::new();
    for k in keys {
        if !own_property_descriptor_for_key_or_throw(vm, &obj, &k)?
            .is_some_and(|desc| desc.enumerable)
        {
            continue;
        }
        let Some(name) = k.as_str() else {
            continue;
        };
        let v = vm.get_property(&obj, name)?;
        let pair = HeapObj::Array(ArrayData::new(
            vec![Value::String(Arc::from(name)), v],
            Some(vm.array_proto.clone()),
        ));
        pairs.push(Value::Object(GcIdx(vm.heap.allocate(pair)?)));
    }
    let arr = HeapObj::Array(ArrayData::new(pairs, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr)?)))
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
    let mut groups: IndexMap<PropertyKey, Vec<Value>> = IndexMap::new();
    let mut k = 0usize;
    loop {
        let (value, done) = vm.iterator_next(&iterator)?;
        if done {
            break;
        }
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

    let obj_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(None),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Object")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    for (key, values) in groups {
        let array = make_value_array(vm, values)?;
        vm.heap.with_obj(obj_idx, |o| {
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
    Ok(Value::Object(GcIdx(obj_idx)))
}

fn object_assign(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if target.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let to = vm.to_object(&target)?;
    for src in &args[1..] {
        if src.is_nullish() {
            continue;
        }
        let from = vm.to_object(src)?;
        let keys = own_property_keys(vm, &from, false, true, true);
        for k in keys {
            if !own_property_descriptor_for_key(vm, &from, &k).is_some_and(|desc| desc.enumerable) {
                continue;
            }
            let v = vm.get_property_by_key(&from, &k)?;
            if !vm.try_set_property_key_with_receiver(&to, &k, v, &to)? {
                return Err(Error::type_err("Cannot assign to read only property"));
            }
        }
    }
    Ok(to)
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
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
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
            let key = vm.get_property_by_key(pair, &PropertyKey::from("0"))?;
            let value = vm.get_property_by_key(pair, &PropertyKey::from("1"))?;
            let key = to_property_key_descriptor(vm, &key)?;
            vm.heap.with_obj(obj_idx, |o| {
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
        }
    }
    Ok(Value::Object(GcIdx(obj_idx)))
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
    let keys: Vec<Arc<str>> = own_property_keys(vm, &obj, false, true, false)
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::Str(s) => Some(s),
            PropertyKey::Symbol(_) => None,
        })
        .collect();
    make_str_array(vm, keys)
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
    let symbols: Vec<Value> = own_property_keys(vm, &obj, false, false, true)
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::Symbol(id) => Some(Value::Symbol(id)),
            PropertyKey::Str(_) => None,
        })
        .collect();
    make_value_array(vm, symbols)
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

pub(crate) fn own_property_keys_or_throw(
    vm: &mut Vm,
    obj: &Value,
    enumerable_only: bool,
    include_strings: bool,
    include_symbols: bool,
) -> error::Result<Vec<PropertyKey>> {
    if let Value::Object(idx) = obj {
        if let Some(proxy_result) = vm.heap.with_obj(idx.0, |heap_obj| {
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
        }) {
            let (target, handler) = proxy_result?;
            let trap = vm.get_property(&handler, "ownKeys")?;
            if trap.is_undefined() {
                return own_property_keys_or_throw(
                    vm,
                    &target,
                    enumerable_only,
                    include_strings,
                    include_symbols,
                );
            }
            let key_list = vm.call_function(&trap, std::slice::from_ref(&target), Some(handler))?;
            let items = if let Value::Object(list_idx) = &key_list {
                vm.heap.with_obj(list_idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        return Some(a.items.lock().clone());
                    }
                    None
                })
            } else {
                None
            }
            .ok_or_else(|| Error::type_err("Proxy ownKeys trap must return an array"))?;
            let mut keys = Vec::new();
            let mut seen = IndexSet::new();
            for item in items {
                let key = to_property_key_descriptor(vm, &item)?;
                if enumerable_only
                    && !own_property_descriptor_for_key(vm, &target, &key)
                        .is_some_and(|desc| desc.enumerable)
                {
                    continue;
                }
                match key {
                    PropertyKey::Str(_) if include_strings => {
                        push_unique_key(&mut keys, &mut seen, key);
                    }
                    PropertyKey::Symbol(_) if include_symbols => {
                        push_unique_key(&mut keys, &mut seen, key);
                    }
                    _ => {}
                }
            }
            return Ok(keys);
        }
    }
    Ok(own_property_keys(
        vm,
        obj,
        enumerable_only,
        include_strings,
        include_symbols,
    ))
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
    let desc_obj = vm.heap.allocate(HeapObj::Object(ObjectData {
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
    vm.heap.with_obj(desc_obj, |o| {
        if let HeapObj::Object(od) = o {
            *od.props.lock() = props;
        }
    });
    Ok(Value::Object(GcIdx(desc_obj)))
}

fn set_integrity_level(vm: &mut Vm, obj: &Value, frozen: bool) -> error::Result<bool> {
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
}

fn test_integrity_level(vm: &mut Vm, obj: &Value, frozen: bool) -> error::Result<bool> {
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
}

fn object_is_extensible(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    vm.is_extensible(&obj).map(Value::Bool)
}

fn object_seal(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if is_proxy_value(vm, &obj) {
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
            _ => {}
        });
    }
    Ok(obj)
}

fn object_is_sealed(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if is_proxy_value(vm, &obj) {
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
            _ => !o.is_extensible(),
        });
        return Ok(Value::Bool(sealed));
    }
    Ok(Value::Bool(true))
}

fn object_is_frozen(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if is_proxy_value(vm, &obj) {
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
    let result_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let keys = own_property_keys_or_throw(vm, &obj, false, true, true)?;
    let mut props = IndexMap::new();
    for key in keys {
        if let Some(desc) = own_property_descriptor_for_key_or_throw(vm, &obj, &key)? {
            props.insert(
                key,
                PropertyDescriptor::data(from_property_descriptor(vm, desc)?),
            );
        }
    }
    vm.heap.with_obj(result_idx, |o| {
        if let HeapObj::Object(od) = o {
            *od.props.lock() = props;
        }
    });
    Ok(Value::Object(GcIdx(result_idx)))
}

fn object_define_properties(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let props = args.get(1).cloned().unwrap_or(Value::Undefined);
    // Collect (key, descriptor) pairs first to avoid borrowing vm during iteration.
    let pairs: Vec<(String, Value)> = if let Value::Object(_) = &props {
        let keys = own_string_keys(vm, &props);
        keys.into_iter()
            .filter_map(|k| {
                let desc = vm.get_property(&props, &k).ok()?;
                if desc.is_undefined() {
                    None
                } else {
                    Some((k.to_string(), desc))
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    for (key, desc) in pairs {
        let dp = vec![obj.clone(), Value::String(Arc::from(key.as_str())), desc];
        object_define_property(vm, &dp, None)?;
    }
    Ok(obj)
}

fn canonical_string_index(key: &PropertyKey) -> Option<usize> {
    let name = key.as_str()?;
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
                    let mut desc =
                        PropertyDescriptor::data(Value::Number(a.items.lock().len() as f64));
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

    if vm.has_own(desc, "enumerable") {
        enumerable = vm.get_property(desc, "enumerable")?.is_truthy();
    }
    if vm.has_own(desc, "configurable") {
        configurable = vm.get_property(desc, "configurable")?.is_truthy();
    }
    if vm.has_own(desc, "value") {
        value = vm.get_property(desc, "value")?;
        has_value = true;
    }
    if vm.has_own(desc, "writable") {
        writable = vm.get_property(desc, "writable")?.is_truthy();
        has_writable = true;
    }
    if vm.has_own(desc, "get") {
        let getter = vm.get_property(desc, "get")?;
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
    if vm.has_own(desc, "set") {
        let setter = vm.get_property(desc, "set")?;
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
}

pub(crate) fn own_property_descriptor_for_key_or_throw(
    vm: &mut Vm,
    obj: &Value,
    key: &PropertyKey,
) -> error::Result<Option<PropertyDescriptor>> {
    if let Value::Object(idx) = obj {
        if let Some(proxy_result) = vm.heap.with_obj(idx.0, |o| {
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
        }) {
            let (target, handler) = proxy_result?;
            let key_value = property_key_to_value(key);
            let trap = vm.get_property(&handler, "getOwnPropertyDescriptor")?;
            if trap.is_undefined() {
                return own_property_descriptor_for_key_or_throw(vm, &target, key);
            }
            let result = vm.call_function(&trap, &[target, key_value], Some(handler))?;
            if result.is_undefined() {
                return Ok(None);
            }
            return property_descriptor_from_object(vm, &result).map(Some);
        }
    }
    Ok(own_property_descriptor_for_key(vm, obj, key))
}

fn from_property_descriptor(vm: &mut Vm, desc: PropertyDescriptor) -> error::Result<Value> {
    let desc_obj = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
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
    vm.heap.with_obj(desc_obj, |o| {
        if let HeapObj::Object(od) = o {
            *od.props.lock() = props;
        }
    });
    Ok(Value::Object(GcIdx(desc_obj)))
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
    if is_proxy_value(vm, &target) {
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
    let key = args
        .get(1)
        .map(|v| to_property_key_descriptor(vm, v))
        .transpose()?
        .unwrap_or_else(|| PropertyKey::from(""));
    let desc = args.get(2).cloned().unwrap_or(Value::Undefined);
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
        // ToPropertyDescriptor: the descriptor must be an Object, else a

        // TypeError. Without this, Object.defineProperty(o, "x", true)

        // silently succeeded instead of throwing (diverging from V8/Node).

        if !matches!(desc, Value::Object(_)) {
            return Err(Error::type_err(format!(
                "Property description must be an object: {}",
                crate::value::value_to_debug_string(&desc)
            )));
        }

        if let Some(proxy_result) = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Proxy(proxy) = obj {
                if *proxy.revoked.lock() {
                    return Some(Err(Error::type_err(
                        "Cannot perform 'defineProperty' on a proxy that has been revoked",
                    )));
                }
                Some(Ok((proxy.target.clone(), proxy.handler.clone())))
            } else {
                None
            }
        }) {
            let (proxy_target, proxy_handler) = proxy_result?;
            let trap = vm.get_property(&proxy_handler, "defineProperty")?;
            let key_value = property_key_to_value(&key);
            if trap.is_undefined() {
                return object_define_property_result(
                    vm,
                    &[proxy_target, key_value, desc.clone()],
                    throw_on_failure,
                );
            }

            let trap_result = vm.call_function(
                &trap,
                &[proxy_target, key_value, desc.clone()],
                Some(proxy_handler),
            )?;
            if !trap_result.is_truthy() {
                if throw_on_failure {
                    return Err(Error::type_err("Proxy defineProperty trap returned false"));
                }
                return Ok(false);
            }
            return Ok(true);
        }

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
                    if has_value && value != current.value {
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
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                if let Some(i) = key.as_str().and_then(crate::value::parse_array_index) {
                    if i >= a.items.lock().len() {
                        let new_len = i + 1;
                        if new_len <= crate::value::MAX_DENSE_ARRAY_LEN {
                            let mut items = a.items.lock();
                            let mut present = a.present.lock();
                            while items.len() < new_len {
                                items.push(Value::Undefined);
                                present.push(false);
                            }
                            *a.sparse_max.lock() = None;
                        } else {
                            *a.sparse_max.lock() = Some(new_len);
                        }
                    }
                }
            }
            obj.props().lock().insert(key.clone(), descriptor);
        });
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
}

// Minimal stubs to keep the crate compiling while parser/lexer work is in progress.

fn active_error_constructor_prototype(vm: &mut Vm) -> error::Result<Value> {
    if let Some(callee) = vm.current_native_callee.clone() {
        let proto = vm.get_property_by_key(&callee, &PropertyKey::from("prototype"))?;
        if matches!(proto, Value::Object(_)) {
            return Ok(proto);
        }
    }
    Ok(vm.error_proto.clone())
}

fn active_error_constructor_name(vm: &mut Vm) -> Arc<str> {
    let Some(Value::Object(idx)) = vm.current_native_callee.as_ref() else {
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
            } else if vm.current_native_new_target.is_some() {
                let proto = new_target_error_constructor_prototype(vm)?;
                Ok(new_error_object(vm, proto)?)
            } else {
                Ok(i)
            }
        }
        _ => {
            // Called as Error(msg) or TypeError(msg) without new: create a
            // fresh object from the active constructor's prototype.
            let proto = if vm.current_native_new_target.is_some() {
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

pub fn setup(vm: &mut Vm) -> error::Result<()> {
    let (object_ctor, object_proto) = make_builtin_constructor(
        vm,
        "Object",
        &[
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
        ],
    )?;
    // Object static methods
    for (n, f, len) in [
        ("keys", object_keys as NativeFn, 1),
        ("values", object_values as NativeFn, 1),
        ("entries", object_entries as NativeFn, 1),
        ("assign", object_assign as NativeFn, 2),
        ("is", object_is as NativeFn, 2),
        ("hasOwn", object_has_own as NativeFn, 2),
        ("fromEntries", object_from_entries as NativeFn, 1),
        ("groupBy", object_group_by as NativeFn, 2),
        ("create", object_create as NativeFn, 2),
        ("freeze", object_freeze as NativeFn, 1),
        (
            "getOwnPropertyNames",
            object_get_own_property_names as NativeFn,
            1,
        ),
        (
            "getOwnPropertySymbols",
            object_get_own_property_symbols as NativeFn,
            1,
        ),
        (
            "getOwnPropertyDescriptor",
            object_get_own_property_descriptor as NativeFn,
            2,
        ),
        ("defineProperty", object_define_property as NativeFn, 3),
        ("defineProperties", object_define_properties as NativeFn, 2),
        ("getPrototypeOf", object_get_prototype_of as NativeFn, 1),
        ("setPrototypeOf", object_set_prototype_of as NativeFn, 2),
        (
            "preventExtensions",
            object_prevent_extensions as NativeFn,
            1,
        ),
        ("isExtensible", object_is_extensible as NativeFn, 1),
        ("seal", object_seal as NativeFn, 1),
        ("isSealed", object_is_sealed as NativeFn, 1),
        ("isFrozen", object_is_frozen as NativeFn, 1),
        (
            "getOwnPropertyDescriptors",
            object_get_own_property_descriptors as NativeFn,
            1,
        ),
    ] {
        let m = vm.new_native_function(n, f, len)?;
        vm.heap.with_obj(object_ctor.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(n), data_prop(Value::Object(m)));
        });
    }
    define_global(vm, "Object", Value::Object(object_ctor));
    vm.object_proto = Value::Object(object_proto);
    let proto_get = vm.new_native_function("get __proto__", object_proto_get, 0)?;
    let proto_set = vm.new_native_function("set __proto__", object_proto_set, 1)?;
    vm.heap.with_obj(object_proto.0, |obj| {
        *obj.proto().lock() = None;
        obj.props().lock().insert(
            PropertyKey::from("__proto__"),
            accessor_prop(Value::Object(proto_get), Value::Object(proto_set)),
        );
    });

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
    Ok(())
}

// =========================================================================
// Extended setup
// =========================================================================
pub fn setup_full(vm: &mut Vm) -> error::Result<()> {
    // Allocate Function.prototype first so that every function created during
    // the rest of bootstrap inherits call/apply/bind via its [[Prototype]].
    let function_proto_idx =
        vm.new_native_function("Function.prototype", function_proto_noop, 0)?;
    vm.function_proto = Value::Object(function_proto_idx);
    setup(vm)?;
    // Per spec, Function.prototype's [[Prototype]] is Object.prototype.
    // (Function.prototype is itself a function, but it inherits Object.prototype
    // methods like isPrototypeOf, hasOwnProperty, toString, etc.)
    vm.heap.with_obj(function_proto_idx.0, |obj| {
        *obj.proto().lock() = Some(vm.object_proto.clone());
    });
    init_global_this(vm)?;
    // Math
    let math = build_math(vm)?;
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

    // Proxy constructor + revocable.
    let proxy_ctor_idx = vm.new_native_function("Proxy", proxy_constructor, 2)?;
    vm.heap.with_obj(proxy_ctor_idx.0, |o| {
        if let HeapObj::Function(f) = o {
            f.prototype.lock().replace(Value::Undefined);
        }
    });
    let proxy_rev_idx = vm.new_native_function("revocable", proxy_revocable, 2)?;
    vm.heap.with_obj(proxy_ctor_idx.0, |o| {
        if let HeapObj::Function(f) = o {
            f.props.lock().insert(
                PropertyKey::from("revocable"),
                data_prop(Value::Object(proxy_rev_idx)),
            );
        }
    });
    define_global(vm, "Proxy", Value::Object(proxy_ctor_idx));

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
    // Date (minimal: now() and constructor returning a timestamp wrapper)
    let (date_ctor, date_proto) = make_builtin_constructor_with_proto_class(
        vm,
        "Date",
        7,
        date_constructor,
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
        None,
    )?;
    vm.date_proto = Value::Object(date_proto);
    define_global(vm, "Date", Value::Object(date_ctor));
    let now_fn = vm.new_native_function("now", date_now, 0)?;
    let parse_fn = vm.new_native_function("parse", date_parse, 1)?;
    let utc_fn = vm.new_native_function("UTC", date_utc, 7)?;
    if let Value::Object(dc) = Value::Object(date_ctor) {
        vm.heap.with_obj(dc.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from("now"), data_prop(Value::Object(now_fn)));
            obj.props().lock().insert(
                PropertyKey::from("parse"),
                data_prop(Value::Object(parse_fn)),
            );
            obj.props()
                .lock()
                .insert(PropertyKey::from("UTC"), data_prop(Value::Object(utc_fn)));
        });
    }
    setup_array_iterator_proto(vm)?;
    // Array
    let (array_ctor, array_proto) = make_builtin_constructor_with(
        vm,
        "Array",
        1,
        array_constructor,
        &[
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
            ("toLocaleString", array_to_string, 0),
        ],
    )?;
    // override the constructor function to use array_constructor
    vm.array_proto = Value::Object(array_proto);
    let array_values_fn = vm.get_property(&vm.array_proto.clone(), "values")?;
    vm.heap.with_obj(array_proto.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.iterator),
            data_prop(array_values_fn),
        );
    });
    define_global(vm, "Array", Value::Object(array_ctor));
    // Array statics
    for (n, f, len) in [
        ("isArray", array_is_array as NativeFn, 1),
        ("from", array_from as NativeFn, 1),
        ("of", array_of as NativeFn, 0),
    ] {
        let m = vm.new_native_function(n, f, len)?;
        vm.heap.with_obj(array_ctor.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(n), data_prop(Value::Object(m)));
        });
    }
    let array_species_getter =
        vm.new_native_function("get [Symbol.species]", promise_species_get, 0)?;
    vm.heap.with_obj(array_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(array_species_getter)),
        );
    });
    // String
    let (str_ctor, str_proto) = make_builtin_constructor_with(
        vm,
        "String",
        1,
        string_constructor,
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
        &[
            ("toFixed", num_to_fixed, 1),
            ("toPrecision", num_to_precision, 1),
            ("toExponential", num_to_exponential, 1),
            ("toString", num_proto_to_string, 1),
            ("toLocaleString", num_proto_to_string, 0),
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
    // BigInt constructor (function form only; no prototype methods yet).
    let bigint_idx = vm.new_native_function("BigInt", global_bigint, 1)?;
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
                        crate::value::PropertyKey::from("valueOf"),
                        data_prop(Value::Object(value_of)),
                    );
                });
            }
        }
    }
    // Promise
    let (promise_ctor, promise_proto) = make_builtin_constructor_with(
        vm,
        "Promise",
        1,
        promise_constructor,
        &[
            ("then", promise_then, 2),
            ("catch", promise_catch, 1),
            ("finally", promise_finally, 1),
        ],
    )?;
    vm.promise_ctor = Value::Object(promise_ctor);
    vm.promise_proto = Value::Object(promise_proto);
    // Static methods on the Promise constructor.
    let resolve_static = vm.new_native_function("resolve", promise_static_resolve, 1)?;
    let reject_static = vm.new_native_function("reject", promise_static_reject, 1)?;
    let all_static = vm.new_native_function("all", promise_static_all, 1)?;
    let all_keyed_static = vm.new_native_function("allKeyed", promise_static_all_keyed, 1)?;
    let race_static = vm.new_native_function("race", promise_static_race, 1)?;
    let all_settled_static = vm.new_native_function("allSettled", promise_static_all_settled, 1)?;
    let all_settled_keyed_static =
        vm.new_native_function("allSettledKeyed", promise_static_all_settled_keyed, 1)?;
    let any_static = vm.new_native_function("any", promise_static_any, 1)?;
    let try_static = vm.new_native_function("try", promise_static_try, 1)?;
    let with_resolvers_static =
        vm.new_native_function("withResolvers", promise_with_resolvers, 0)?;
    let species_getter = vm.new_native_function("get [Symbol.species]", promise_species_get, 0)?;
    vm.heap.with_obj(promise_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("resolve"),
            data_prop(Value::Object(resolve_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("reject"),
            data_prop(Value::Object(reject_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("all"),
            data_prop(Value::Object(all_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("allKeyed"),
            data_prop(Value::Object(all_keyed_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("race"),
            data_prop(Value::Object(race_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("allSettled"),
            data_prop(Value::Object(all_settled_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("allSettledKeyed"),
            data_prop(Value::Object(all_settled_keyed_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("any"),
            data_prop(Value::Object(any_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("try"),
            data_prop(Value::Object(try_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("withResolvers"),
            data_prop(Value::Object(with_resolvers_static)),
        );
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(species_getter)),
        );
    });
    define_global(vm, "Promise", Value::Object(promise_ctor));
    // RegExp
    let (regex_ctor, regex_proto) = make_regexp_constructor_in_env(vm, vm.global)?;
    vm.regexp_proto = Value::Object(regex_proto);
    define_global(vm, "RegExp", Value::Object(regex_ctor));
    // Generator prototype with next(). Generator instances inherit this proto.
    let generator_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Generator")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    {
        let next_fn = vm.new_native_function("next", generator_next, 0)?;
        let return_fn = vm.new_native_function("return", generator_return, 1)?;
        let throw_fn = vm.new_native_function("throw", generator_throw, 1)?;
        vm.heap.with_obj(generator_proto_idx, |o| {
            o.props()
                .lock()
                .insert(PropertyKey::from("next"), data_prop(Value::Object(next_fn)));
            o.props().lock().insert(
                PropertyKey::from("return"),
                data_prop(Value::Object(return_fn)),
            );
            o.props().lock().insert(
                PropertyKey::from("throw"),
                data_prop(Value::Object(throw_fn)),
            );
        });
    }
    vm.generator_proto = Value::Object(GcIdx(generator_proto_idx));
    // Function constructor: new Function(p0, ..., body)
    let function_ctor_idx = vm.new_native_function("Function", function_constructor, 1)?;
    vm.heap.with_obj(function_ctor_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            f.prototype
                .lock()
                .replace(Value::Object(function_proto_idx));
        }
    });
    define_global(vm, "Function", Value::Object(function_ctor_idx));
    // %GeneratorFunction% is not exposed as a global binding, but generator
    // functions inherit from %GeneratorFunction.prototype%, whose constructor
    // property exposes it.
    let generator_function_ctor_idx =
        vm.new_native_function("GeneratorFunction", generator_function_constructor, 1)?;
    let generator_function_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.function_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("GeneratorFunction")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.generator_function_proto = Value::Object(GcIdx(generator_function_proto_idx));
    vm.heap.with_obj(generator_function_proto_idx, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(generator_function_ctor_idx)),
        );
        let mut prototype_desc = data_prop(vm.generator_proto.clone());
        prototype_desc.writable = false;
        props.insert(PropertyKey::from("prototype"), prototype_desc);
    });
    vm.heap.with_obj(generator_function_ctor_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            f.prototype
                .lock()
                .replace(Value::Object(GcIdx(generator_function_proto_idx)));
        }
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(Value::Object(GcIdx(generator_function_proto_idx))),
        );
    });
    // Async generator intrinsics are distinct from their synchronous
    // counterparts. In particular, %AsyncIteratorPrototype% must not alias
    // Object.prototype because user changes to it cannot affect arrays or
    // other ordinary objects.
    let async_iterator_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("AsyncIterator")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let async_iterator_fn = vm.new_native_function(
        "[Symbol.asyncIterator]",
        collections::collection_iterator_this,
        0,
    )?;
    vm.heap.with_obj(async_iterator_proto_idx, |obj| {
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.async_iterator),
            data_prop(Value::Object(async_iterator_fn)),
        );
    });
    vm.async_iterator_proto = Value::Object(GcIdx(async_iterator_proto_idx));

    let async_generator_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.async_iterator_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("AsyncGenerator")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let async_next_fn = vm.new_native_function("next", async_generator_next, 1)?;
    let async_return_fn = vm.new_native_function("return", async_generator_return, 1)?;
    let async_throw_fn = vm.new_native_function("throw", async_generator_throw, 1)?;
    vm.heap.with_obj(async_generator_proto_idx, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("next"),
            data_prop(Value::Object(async_next_fn)),
        );
        props.insert(
            PropertyKey::from("return"),
            data_prop(Value::Object(async_return_fn)),
        );
        props.insert(
            PropertyKey::from("throw"),
            data_prop(Value::Object(async_throw_fn)),
        );
        let mut tag_desc = data_prop(Value::String(Arc::from("AsyncGenerator")));
        tag_desc.writable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag_desc,
        );
    });
    vm.async_generator_proto = Value::Object(GcIdx(async_generator_proto_idx));

    let async_generator_function_ctor_idx = vm.new_native_function(
        "AsyncGeneratorFunction",
        async_generator_function_constructor,
        1,
    )?;
    let async_generator_function_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.function_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("AsyncGeneratorFunction")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.async_generator_function_proto = Value::Object(GcIdx(async_generator_function_proto_idx));
    vm.heap.with_obj(async_generator_function_proto_idx, |obj| {
        let mut props = obj.props().lock();
        let mut constructor_desc = data_prop(Value::Object(async_generator_function_ctor_idx));
        constructor_desc.writable = false;
        props.insert(PropertyKey::from("constructor"), constructor_desc);
        let mut prototype_desc = data_prop(vm.async_generator_proto.clone());
        prototype_desc.writable = false;
        props.insert(PropertyKey::from("prototype"), prototype_desc);
        let mut tag_desc = data_prop(Value::String(Arc::from("AsyncGeneratorFunction")));
        tag_desc.writable = false;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag_desc,
        );
    });
    vm.heap.with_obj(async_generator_proto_idx, |obj| {
        let mut constructor_desc = data_prop(vm.async_generator_function_proto.clone());
        constructor_desc.writable = false;
        obj.props()
            .lock()
            .insert(PropertyKey::from("constructor"), constructor_desc);
    });
    vm.heap
        .with_obj(async_generator_function_ctor_idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                f.prototype
                    .lock()
                    .replace(Value::Object(GcIdx(async_generator_function_proto_idx)));
            }
            obj.props().lock().insert(
                PropertyKey::from("prototype"),
                const_prop(Value::Object(GcIdx(async_generator_function_proto_idx))),
            );
        });
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
    let Value::Object(this_idx) = this_obj else {
        return Ok(Value::Bool(false));
    };
    let Value::Object(arg_idx) = arg else {
        return Ok(Value::Bool(false));
    };
    let mut cur = vm
        .heap
        .with_obj(arg_idx.0, |o| o.proto().lock().clone())
        .unwrap_or(Value::Null);
    let mut depth = 0;
    while let Value::Object(idx) = &cur {
        if depth > 1024 {
            break;
        }
        depth += 1;
        if *idx == this_idx {
            return Ok(Value::Bool(true));
        }
        let proto = vm.heap.with_obj(idx.0, |o| o.proto().lock().clone());
        cur = proto.unwrap_or(Value::Null);
        if cur.is_null() {
            break;
        }
    }
    Ok(Value::Bool(false))
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
