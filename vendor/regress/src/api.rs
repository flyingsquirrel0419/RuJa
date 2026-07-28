use crate::classicalbacktrack;
use crate::emit;
use crate::exec;
use crate::indexing;
use crate::insn::CompiledRegex;
use crate::optimizer;
use crate::parse;
use crate::types::MAX_CAPTURE_GROUPS;

#[cfg(feature = "utf16")]
use crate::{
    classicalbacktrack::MatchAttempter,
    indexing::{InputIndexer, Ucs2Input, Utf16Input},
};

#[cfg(feature = "backend-pikevm")]
use crate::pikevm;
use crate::util::to_char_sat;

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use core::{fmt, iter::FusedIterator, str::FromStr};

pub use parse::Error;

/// Error raised when bounded regex execution cannot finish within its work budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeError {
    /// The caller-provided instruction budget was exhausted.
    WorkLimitExceeded,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkLimitExceeded => f.write_str("regular expression work limit exceeded"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RuntimeError {}

/// Flags used to control regex parsing.
/// The default flags are case-sensitive, not-multiline, and optimizing.
#[derive(Debug, Copy, Clone, Default)]
pub struct Flags {
    /// If set, make the regex case-insensitive.
    /// Equivalent to the 'i' flag in JavaScript.
    pub icase: bool,

    /// If set, ^ and $ match at line separators, not just the input boundaries.
    /// Equivalent to the 'm' flag in JavaScript.
    pub multiline: bool,

    /// If set, . matches at line separators as well as any other character.
    /// Equivalent to the 's' flag in JavaScript.
    pub dot_all: bool,

    /// If set, disable regex IR passes.
    pub no_opt: bool,

    /// If set, the regex is interpreted as a Unicode regex.
    /// Equivalent to the 'u' flag in JavaScript.
    pub unicode: bool,

    /// If set, the regex is interpreted as a UnicodeSets regex.
    /// Equivalent to the 'v' flag in JavaScript.
    pub unicode_sets: bool,

    /// If set, reject string-valued UnicodeSets operands while retaining
    /// character-only set algebra.
    pub disable_string_sets: bool,
}

impl Flags {
    /// Construct a Flags from a Unicode codepoints iterator, using JavaScript field names.
    /// 'i' means to ignore case, 'm' means multiline, 'u' means unicode.
    /// Note the 'g' flag implies a stateful regex and is not supported.
    /// Other flags are not implemented and are ignored.
    #[inline]
    pub fn new<T: Iterator<Item = u32>>(chars: T) -> Self {
        let mut result = Self::default();
        for c in chars {
            match to_char_sat(c) {
                'm' => {
                    result.multiline = true;
                }
                'i' => {
                    result.icase = true;
                }
                's' => {
                    result.dot_all = true;
                }
                'u' => {
                    result.unicode = true;
                }
                'v' => {
                    result.unicode_sets = true;
                    result.unicode = true;
                }
                _ => {
                    // Silently skip unsupported flags.
                }
            }
        }
        result
    }
}

impl From<&str> for Flags {
    /// Construct a Flags from a string, using JavaScript field names.
    ///
    /// See also: [`Flags::new`].
    #[inline]
    fn from(s: &str) -> Self {
        Self::new(s.chars().map(u32::from))
    }
}

impl fmt::Display for Flags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.multiline {
            f.write_str("m")?;
        }
        if self.icase {
            f.write_str("i")?;
        }
        if self.dot_all {
            f.write_str("s")?;
        }
        if self.unicode_sets {
            f.write_str("v")?;
        } else if self.unicode {
            f.write_str("u")?;
        }
        Ok(())
    }
}

/// Range is used to express the extent of a match, as indexes into the input
/// string.
pub type Range = core::ops::Range<usize>;

/// An iterator type which yields `Match`es found in a string.
pub type Matches<'r, 't> = exec::Matches<backends::DefaultExecutor<'r, 't>>;

/// An iterator type which yields `Match`es found in a string, supporting ASCII
/// only.
pub type AsciiMatches<'r, 't> = exec::Matches<backends::DefaultAsciiExecutor<'r, 't>>;

/// A Match represents a portion of a string which was found to match a Regex.
#[derive(Debug, Clone)]
pub struct Match {
    /// The total range of the match. Note this may be empty, if the regex
    /// matched an empty string.
    pub range: Range,

    /// The list of captures. This has length equal to the number of capturing
    /// groups in the regex. For each capture, if the value is None, that group
    /// did not match (for example, it was in a not-taken branch of an
    /// alternation). If the value is Some, the group did match with the
    /// enclosed range.
    pub captures: Vec<Option<Range>>,

    // A list of capture group names. This is either:
    //   - Empty, if there were no named capture groups.
    //   - A list of names with length `captures.len()`, corresponding to the
    //     capture group names in order. Groups without names have an empty string.
    pub(crate) group_names: Box<[Box<str>]>,
}

impl Match {
    /// Access a group by index, using the convention of Python's group()
    /// function. Index 0 is the total match, index 1 is the first capture
    /// group.
    #[inline]
    pub fn group(&self, idx: usize) -> Option<Range> {
        if idx == 0 {
            Some(self.range.clone())
        } else if idx <= self.captures.len() {
            self.captures[idx - 1].clone()
        } else {
            None
        }
    }

    /// Access a named group by name.
    #[inline]
    pub fn named_group(&self, name: &str) -> Option<Range> {
        // Empty strings are used as sentinels to indicate unnamed group.
        if name.is_empty() {
            return None;
        }
        let pos = self.group_names.iter().position(|s| s.as_ref() == name)?;
        self.captures[pos].clone()
    }

    /// Return an iterator over the named groups of a Match.
    #[inline]
    pub fn named_groups(&self) -> NamedGroups<'_> {
        NamedGroups::new(self)
    }

    /// Returns the range over the starting and ending byte offsets of the match in the haystack.
    ///
    /// This is a convenience function to work around
    /// the fact that Range does not support Copy.
    #[inline]
    pub fn range(&self) -> Range {
        self.range.clone()
    }

    /// Returns the starting byte offset of the match in the haystack.
    #[inline]
    pub fn start(&self) -> usize {
        self.range.start
    }

    /// Returns the ending byte offset of the match in the haystack.
    #[inline]
    pub fn end(&self) -> usize {
        self.range.end
    }

    /// Returns the matched text as a string slice.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use regress::Regex;
    ///
    /// let re = Regex::new(r"\d+").unwrap();
    /// let text = "Price: $123";
    /// let m = re.find(text).unwrap();
    /// assert_eq!(m.as_str(text), "123");
    /// ```
    #[inline]
    pub fn as_str<'t>(&self, text: &'t str) -> &'t str {
        &text[self.range()]
    }

    /// Return an iterator over a Match. The first returned value is the total
    /// match, and subsequent values represent the capture groups.
    #[inline]
    pub fn groups(&self) -> Groups<'_> {
        Groups::new(self)
    }
}

/// An iterator over the capture groups of a [`Match`]
///
/// This struct is created by the [`groups`] method on [`Match`].
///
/// [`Match`]: ../struct.Match.html
/// [`groups`]: ../struct.Match.html#method.groups
#[derive(Clone)]
pub struct Groups<'m> {
    mat: &'m Match,

    // The next group index to return, where 0 references the total match.
    next_group_idx: usize,

    // The maximum group index to return, with a +1 for the implicit total match.
    // For example, in a regex with 1 capture group, this will be 2.
    max: usize,
}

impl<'m> Groups<'m> {
    #[inline]
    fn new(mat: &'m Match) -> Self {
        Self {
            mat,
            next_group_idx: 0,
            max: mat.captures.len() + 1, // +1 for the total match
        }
    }
}

impl Iterator for Groups<'_> {
    type Item = Option<Range>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.next_group_idx;
        if i < self.max {
            self.next_group_idx += 1;
            Some(self.mat.group(i))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.max.saturating_sub(self.next_group_idx);
        (size, Some(size))
    }
}

impl<'m> ExactSizeIterator for Groups<'m> {}
impl<'m> FusedIterator for Groups<'m> {}

/// An iterator over the named capture groups of a [`Match`]
///
/// This struct is created by the [`named_groups`] method on [`Match`].
///
/// [`Match`]: ../struct.Match.html
/// [`named_groups`]: ../struct.Match.html#method.named_groups
#[derive(Clone)]
pub struct NamedGroups<'m> {
    mat: &'m Match,

    // The next group name index to return.
    // Note unlike `Groups` this does NOT include the implicit total match.
    // That is, group 0 is the first capture group, NOT the total match.
    next_group_idx: usize,
}

impl<'m> NamedGroups<'m> {
    #[inline]
    fn new(mat: &'m Match) -> Self {
        Self {
            mat,
            next_group_idx: 0,
        }
    }
}

impl<'m> Iterator for NamedGroups<'m> {
    type Item = (&'m str, Option<Range>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Increment next_group_idx until we find a non-empty name that we haven't seen yet.
        debug_assert!(self.next_group_idx <= self.mat.group_names.len());
        let end = self.mat.group_names.len();

        loop {
            let mut idx = self.next_group_idx;
            // Skip empty names
            while idx < end && self.mat.group_names[idx].is_empty() {
                idx += 1;
            }
            if idx == end {
                return None;
            }

            let name = self.mat.group_names[idx].as_ref();

            // Check if we've already returned this name (by looking backwards)
            let already_seen = self.mat.group_names[..idx]
                .iter()
                .any(|n| n.as_ref() == name);

            if already_seen {
                // Skip this duplicate and continue to next
                self.next_group_idx = idx + 1;
                continue;
            }

            // This is the first occurrence of this name. Find the best range value.
            // Prefer a Some value over None when there are duplicate names.
            let mut best_range = self.mat.captures[idx].clone();
            for check_idx in (idx + 1)..end {
                if self.mat.group_names[check_idx].as_ref() == name {
                    // Found a duplicate name. Prefer a Some value over None.
                    if best_range.is_none() && self.mat.captures[check_idx].is_some() {
                        best_range = self.mat.captures[check_idx].clone();
                        break; // Stop once we find a Some value
                    }
                }
            }

            self.next_group_idx = idx + 1;
            return Some((name, best_range));
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.mat.group_names[self.next_group_idx..]
            .iter()
            .filter(|s| !s.is_empty())
            .count();

        (size, Some(size))
    }
}

impl<'m> ExactSizeIterator for NamedGroups<'m> {}
impl<'m> FusedIterator for NamedGroups<'m> {}

/// A Regex is the compiled version of a pattern.
#[derive(Debug, Clone)]
pub struct Regex {
    cr: CompiledRegex,
}

impl From<CompiledRegex> for Regex {
    fn from(cr: CompiledRegex) -> Self {
        Self { cr }
    }
}

impl Regex {
    /// Validate a Unicode-code-point pattern without optimizing or emitting a
    /// matcher. This is useful for hosts that must report RegExp literal early
    /// errors before runtime construction.
    pub fn validate_syntax<I, F>(pattern: I, flags: F) -> Result<(), Error>
    where
        I: Iterator<Item = u32> + Clone,
        F: Into<Flags>,
    {
        parse::try_parse(pattern, flags.into()).map(|_| ())
    }

    /// Validate bounded named-capture preprocessing without compiling regex IR.
    pub fn validate_unicode_resource_limits<I, F>(pattern: I, flags: F) -> Result<(), Error>
    where
        I: Iterator<Item = u32> + Clone,
        F: Into<Flags>,
    {
        parse::validate_resource_limits(pattern, flags.into())
    }

    /// Construct a regex by parsing `pattern` using the default flags.
    /// An Error may be returned if the syntax is invalid.
    /// Note that this is rather expensive; prefer to cache a Regex which is
    /// intended to be used more than once.
    #[inline]
    pub fn new(pattern: &str) -> Result<Regex, Error> {
        Self::with_flags(pattern, Flags::default())
    }

    /// Construct a regex by parsing `pattern` with `flags`.
    /// An Error may be returned if the syntax is invalid.
    //
    /// Note it is preferable to cache a Regex which is intended to be used more
    /// than once, as the parse may be expensive. For example:
    #[inline]
    pub fn with_flags<F>(pattern: &str, flags: F) -> Result<Regex, Error>
    where
        F: Into<Flags>,
    {
        Self::from_unicode(pattern.chars().map(u32::from), flags)
    }

    /// Construct a regex by parsing `pattern` with `flags`, where
    /// `pattern` is an iterator of `u32` Unicode codepoints.
    /// An Error may be returned if the syntax is invalid.
    /// This allows parsing regular expressions from exotic strings in
    /// other encodings, such as UTF-16 or UTF-32.
    pub fn from_unicode<I, F>(pattern: I, flags: F) -> Result<Regex, Error>
    where
        I: Iterator<Item = u32> + Clone,
        F: Into<Flags>,
    {
        let flags = flags.into();
        let mut ire = parse::try_parse(pattern, flags)?;
        if !flags.no_opt {
            optimizer::optimize(&mut ire);
        }
        let cr = emit::emit(&ire);
        Ok(Regex { cr })
    }

    /// Searches `text` to find the first match.
    #[inline]
    pub fn find(&self, text: &str) -> Option<Match> {
        self.find_iter(text).next()
    }

    /// Searches `text`, returning an iterator over non-overlapping matches.
    /// Note that the resulting Iterator borrows both the regex `'r` and the
    /// input string as `'t`.
    #[inline]
    pub fn find_iter<'r, 't>(&'r self, text: &'t str) -> Matches<'r, 't> {
        self.find_from(text, 0)
    }

    /// Returns an iterator for matches found in 'text' starting at byte index
    /// `start`. Note this may be different from passing a sliced `text` in
    /// the case of lookbehind assertions.
    /// Example:
    ///
    ///  ```rust
    ///   use regress::Regex;
    ///   let text = "xyxy";
    ///   let re = Regex::new(r"(?<=x)y").unwrap();
    ///   let t1 = re.find(&text[1..]).unwrap().range();
    ///   assert!(t1 == (2..3));
    ///   let t2 = re.find_from(text, 1).next().unwrap().range();
    ///   assert!(t2 == (1..2));
    ///   ```
    ///
    /// # Panics
    ///
    /// Panics if `start` is not on a UTF-8 character boundary of `text`. An
    /// out-of-range `start` (`start > text.len()`) is not an error and yields an
    /// empty iterator, mirroring JavaScript's `lastIndex` semantics.
    #[inline]
    pub fn find_from<'r, 't>(&'r self, text: &'t str, start: usize) -> Matches<'r, 't> {
        assert!(
            start >= text.len() || text.is_char_boundary(start),
            "start index is not on a char boundary"
        );
        backends::find(self, text, start)
    }

    /// Searches `text` to find the first match.
    /// The input text is expected to be ascii-only: only ASCII case-folding is
    /// supported.
    #[inline]
    pub fn find_ascii(&self, text: &str) -> Option<Match> {
        self.find_iter_ascii(text).next()
    }

    /// Searches `text`, returning an iterator over non-overlapping matches.
    /// The input text is expected to be ascii-only: only ASCII case-folding is
    /// supported.
    #[inline]
    pub fn find_iter_ascii<'r, 't>(&'r self, text: &'t str) -> AsciiMatches<'r, 't> {
        self.find_from_ascii(text, 0)
    }

    /// Returns an iterator for matches found in 'text' starting at byte index
    /// `start`. An out-of-range `start` yields an empty iterator.
    #[inline]
    pub fn find_from_ascii<'r, 't>(&'r self, text: &'t str, start: usize) -> AsciiMatches<'r, 't> {
        backends::find(self, text, start)
    }

    /// Returns an iterator for matches found in 'text' starting at index `start`.
    #[cfg(feature = "utf16")]
    pub fn find_from_utf16<'r, 't>(
        &'r self,
        text: &'t [u16],
        start: usize,
    ) -> exec::Matches<super::classicalbacktrack::BacktrackExecutor<'r, indexing::Utf16Input<'t>>>
    {
        let input = Utf16Input::new(text, self.cr.flags.unicode);
        exec::Matches::new(
            super::classicalbacktrack::BacktrackExecutor::new(
                input,
                MatchAttempter::new(&self.cr, input.left_end()),
            ),
            start,
        )
    }

    /// Returns the first UTF-16 match at or after `start`, sharing a bounded
    /// instruction budget with the caller.
    #[cfg(all(feature = "utf16", feature = "backend-pikevm"))]
    pub fn find_from_utf16_bounded(
        &self,
        text: &[u16],
        start: usize,
        work_remaining: &mut usize,
    ) -> Result<Option<Match>, RuntimeError> {
        pikevm::find_from_utf16_bounded(&self.cr, text, start, work_remaining)
            .map_err(|_| RuntimeError::WorkLimitExceeded)
    }

    /// Attempts one UTF-16 match exactly at `start`, sharing a bounded
    /// instruction and state-copy budget with the caller.
    #[cfg(all(feature = "utf16", feature = "backend-pikevm"))]
    pub fn find_at_utf16_bounded(
        &self,
        text: &[u16],
        start: usize,
        work_remaining: &mut usize,
    ) -> Result<Option<Match>, RuntimeError> {
        pikevm::find_at_utf16_bounded(&self.cr, text, start, work_remaining)
            .map_err(|_| RuntimeError::WorkLimitExceeded)
    }

    /// Conservative size charged before each bounded PikeVM state allocation.
    pub fn bounded_execution_state_cost(&self) -> usize {
        let state = 64usize
            .saturating_add(self.cr.insns.len())
            .saturating_add((self.cr.groups as usize).saturating_mul(32))
            .saturating_add((self.cr.loops as usize).saturating_mul(32));
        let brackets = self.cr.brackets.iter().fold(0usize, |cost, bracket| {
            cost.saturating_add(bracket.cps.intervals().len().saturating_mul(16))
        });
        let names = self.cr.group_names.iter().fold(0usize, |cost, name| {
            cost.saturating_add(name.len()).saturating_add(16)
        });
        let conditional_backrefs = self.cr.insns.iter().fold(0usize, |cost, insn| {
            if let crate::insn::Insn::AllBackRefsUnmatched(groups) = insn {
                cost.saturating_add(groups.len().saturating_mul(core::mem::size_of::<u32>()))
            } else {
                cost
            }
        });
        state
            .saturating_add(brackets)
            .saturating_add(names)
            .saturating_add(conditional_backrefs)
    }

    /// Returns an iterator for matches found in 'text' starting at index `start`.
    #[cfg(feature = "utf16")]
    pub fn find_from_ucs2<'r, 't>(
        &'r self,
        text: &'t [u16],
        start: usize,
    ) -> exec::Matches<super::classicalbacktrack::BacktrackExecutor<'r, indexing::Ucs2Input<'t>>>
    {
        let input = Ucs2Input::new(text, self.cr.flags.unicode);
        exec::Matches::new(
            super::classicalbacktrack::BacktrackExecutor::new(
                input,
                MatchAttempter::new(&self.cr, input.left_end()),
            ),
            start,
        )
    }

    /// Replaces the first match of the regex in `text` with the replacement string.
    ///
    /// The replacement string may contain capture group references in the form `$1`, `$2`, etc.,
    /// where `$1` refers to the first capture group, `$2` to the second, and so on.
    /// `$0` refers to the entire match. Use `$$` to insert a literal `$`.
    ///
    /// If no match is found, the original text is returned unchanged.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use regress::Regex;
    ///
    /// let re = Regex::new(r"(\w+)\s+(\w+)").unwrap();
    /// let result = re.replace("hello world", "$2 $1");
    /// assert_eq!(result, "world hello");
    ///
    /// let re = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();
    /// let result = re.replace("2023-12-25", "$2/$3/$1");
    /// assert_eq!(result, "12/25/2023");
    /// ```
    pub fn replace(&self, text: &str, replacement: &str) -> String {
        match self.find(text) {
            Some(m) => {
                let mut result = String::with_capacity(text.len());
                result.push_str(&text[..m.start()]);
                self.expand_replacement(&m, text, replacement, &mut result);
                result.push_str(&text[m.end()..]);
                result
            }
            None => text.to_string(),
        }
    }

    /// Replaces all matches of the regex in `text` with the replacement string.
    ///
    /// The replacement string may contain capture group references in the form `$1`, `$2`, etc.,
    /// where `$1` refers to the first capture group, `$2` to the second, and so on.
    /// `$0` refers to the entire match. Use `$$` to insert a literal `$`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use regress::Regex;
    ///
    /// let re = Regex::new(r"(\w+)\s+(\w+)").unwrap();
    /// let result = re.replace_all("hello world foo bar", "$2-$1");
    /// assert_eq!(result, "world-hello bar-foo");
    ///
    /// let re = Regex::new(r"\b(\w)(\w+)").unwrap();
    /// let result = re.replace_all("hello world", "$1.$2");
    /// assert_eq!(result, "h.ello w.orld");
    /// ```
    pub fn replace_all(&self, text: &str, replacement: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut last_end = 0;

        for m in self.find_iter(text) {
            result.push_str(&text[last_end..m.start()]);
            self.expand_replacement(&m, text, replacement, &mut result);
            last_end = m.end();
        }

        result.push_str(&text[last_end..]);
        result
    }

    /// Replaces the first match of the regex in `text` using a closure.
    ///
    /// The closure receives a `&Match` and should return the replacement string.
    /// This is useful for dynamic replacements that depend on the match details.
    ///
    /// If no match is found, the original text is returned unchanged.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use regress::Regex;
    ///
    /// let re = Regex::new(r"\d+").unwrap();
    /// let text = "Price: $123";
    /// let result = re.replace_with(text, |m| {
    ///     let num: i32 = m.as_str(text).parse().unwrap();
    ///     format!("{}", num * 2)
    /// });
    /// assert_eq!(result, "Price: $246");
    /// ```
    pub fn replace_with<F>(&self, text: &str, replacement: F) -> String
    where
        F: FnOnce(&Match) -> String,
    {
        match self.find(text) {
            Some(m) => {
                let mut result = String::with_capacity(text.len());
                result.push_str(&text[..m.start()]);
                result.push_str(&replacement(&m));
                result.push_str(&text[m.end()..]);
                result
            }
            None => text.to_string(),
        }
    }

    /// Replaces all matches of the regex in `text` using a closure.
    ///
    /// The closure receives a `&Match` and should return the replacement string.
    /// This is useful for dynamic replacements that depend on the match details.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use regress::Regex;
    ///
    /// let re = Regex::new(r"\d+").unwrap();
    /// let text = "Items: 5, 10, 15";
    /// let result = re.replace_all_with(text, |m| {
    ///     let num: i32 = m.as_str(text).parse().unwrap();
    ///     format!("[{}]", num * 10)
    /// });
    /// assert_eq!(result, "Items: [50], [100], [150]");
    /// ```
    pub fn replace_all_with<F>(&self, text: &str, replacement: F) -> String
    where
        F: Fn(&Match) -> String,
    {
        let mut result = String::with_capacity(text.len());
        let mut last_end = 0;

        for m in self.find_iter(text) {
            result.push_str(&text[last_end..m.start()]);
            result.push_str(&replacement(&m));
            last_end = m.end();
        }

        result.push_str(&text[last_end..]);
        result
    }

    /// Helper method to expand replacement strings with capture group substitutions.
    fn expand_replacement(&self, m: &Match, text: &str, replacement: &str, output: &mut String) {
        let mut chars = replacement.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '$' {
                match chars.peek() {
                    Some('$') => {
                        // $$ -> literal $
                        chars.next();
                        output.push('$');
                    }
                    Some(&digit) if digit.is_ascii_digit() => {
                        // Parse the group number
                        let mut group_num = 0;
                        while let Some(&digit) = chars.peek() {
                            if digit.is_ascii_digit() {
                                chars.next();
                                group_num = group_num * 10 + (digit as u32 - '0' as u32) as usize;
                                // Limit to reasonable group numbers to avoid overflow
                                if group_num > MAX_CAPTURE_GROUPS {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }

                        // Get the matched text for this group
                        if let Some(range) = m.group(group_num) {
                            output.push_str(&text[range]);
                        }
                        // If group doesn't exist or didn't match, add nothing
                    }
                    Some('{') => {
                        // Handle ${name} syntax for named groups
                        chars.next(); // consume '{'
                        let mut name = String::new();
                        let mut found_closing_brace = false;

                        for ch in chars.by_ref() {
                            if ch == '}' {
                                found_closing_brace = true;
                                break;
                            }
                            name.push(ch);
                        }

                        if found_closing_brace {
                            if let Some(range) = m.named_group(&name) {
                                output.push_str(&text[range]);
                            }
                        } else {
                            // Malformed ${...}, treat as literal
                            output.push_str("${");
                            output.push_str(&name);
                        }
                    }
                    _ => {
                        // Just a $ at end or followed by non-digit
                        output.push('$');
                    }
                }
            } else {
                output.push(ch);
            }
        }
    }
}

#[cfg(all(test, feature = "utf16", feature = "backend-pikevm"))]
mod bounded_utf16_tests {
    use super::{Flags, Regex, RuntimeError};

    #[test]
    fn distinguishes_surrogates_from_private_use_scalars() {
        let flags = Flags {
            unicode: true,
            ..Flags::default()
        };
        let surrogate = Regex::from_unicode([0xdb80].into_iter(), flags).unwrap();
        let scalar = Regex::from_unicode([0xf0000].into_iter(), flags).unwrap();
        let mut budget = 10_000;
        assert_eq!(
            surrogate
                .find_from_utf16_bounded(&[0xdb80], 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..1
        );
        assert!(
            surrogate
                .find_from_utf16_bounded(&[0xdb80, 0xdc00], 0, &mut budget)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            scalar
                .find_from_utf16_bounded(&[0xdb80, 0xdc00], 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..2
        );
    }

    #[test]
    fn reports_work_budget_exhaustion() {
        let regex = Regex::with_flags("(?:a|aa)*b", "u").unwrap();
        let mut budget = 1;
        assert!(matches!(
            regex.find_from_utf16_bounded(&[b'a' as u16; 32], 0, &mut budget),
            Err(RuntimeError::WorkLimitExceeded)
        ));
    }

    #[test]
    fn unicode_string_sets_preserve_spec_order_and_empty_strings() {
        let regex = Regex::with_flags(r"^([\q{|a}])a?$", "v").unwrap();
        let mut budget = 10_000;
        let matched = regex
            .find_at_utf16_bounded(&[b'a' as u16], 0, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(matched.range, 0..1);
        assert_eq!(matched.group(1), Some(0..1));

        let longest = Regex::with_flags(r"^[\q{a|ab|}]b$", "v").unwrap();
        let mut budget = 10_000;
        assert_eq!(
            longest
                .find_at_utf16_bounded(&[b'a' as u16, b'b' as u16, b'b' as u16], 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..3
        );
        let mut budget = 10_000;
        assert_eq!(
            longest
                .find_at_utf16_bounded(&[b'b' as u16], 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..1
        );
    }

    #[test]
    fn unicode_string_sets_fold_before_set_algebra() {
        let intersection = Regex::with_flags(r"^[\q{Kx}&&\q{Kx}]$", "iv").unwrap();
        let mut budget = 10_000;
        assert_eq!(
            intersection
                .find_at_utf16_bounded(&[b'k' as u16, b'X' as u16], 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..2
        );

        let subtraction = Regex::with_flags(r"^[\q{Kx}--\q{Kx}]$", "iv").unwrap();
        let mut budget = 10_000;
        assert!(
            subtraction
                .find_at_utf16_bounded(&[b'K' as u16, b'x' as u16], 0, &mut budget)
                .unwrap()
                .is_none()
        );

        let single = Regex::with_flags(r"^[\q{K}&&\q{K}]$", "iv").unwrap();
        let mut budget = 10_000;
        assert_eq!(
            single
                .find_at_utf16_bounded(&[b'k' as u16], 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..1
        );

        let mixed_subtraction = Regex::with_flags(r"^[k--\q{K}]$", "iv").unwrap();
        for input in [b'k' as u16, b'K' as u16, 0x212a] {
            let mut budget = 10_000;
            assert!(
                mixed_subtraction
                    .find_at_utf16_bounded(&[input], 0, &mut budget)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn unicode_string_sets_reverse_in_lookbehind() {
        let regex = Regex::with_flags(r"(?<=[\q{ab}])c", "v").unwrap();
        let mut budget = 10_000;
        assert_eq!(
            regex
                .find_from_utf16_bounded(&[b'a' as u16, b'b' as u16, b'c' as u16], 0, &mut budget,)
                .unwrap()
                .unwrap()
                .range,
            2..3
        );
        let mut budget = 10_000;
        assert!(
            regex
                .find_from_utf16_bounded(&[b'b' as u16, b'a' as u16, b'c' as u16], 0, &mut budget,)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn unicode_string_set_grammar_tracks_strings_and_punctuators() {
        let complement = Regex::with_flags(r"^[^\q{a}]$", "v").unwrap();
        let mut budget = 10_000;
        assert_eq!(
            complement
                .find_at_utf16_bounded(&[b'b' as u16], 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..1
        );
        assert!(Regex::with_flags(r"[^\q{ab}]", "v").is_err());
        let nested_complement = Regex::with_flags(r"^[[^\q{a}]]$", "v").unwrap();
        let mut budget = 10_000;
        assert!(
            nested_complement
                .find_at_utf16_bounded(&[b'a' as u16], 0, &mut budget)
                .unwrap()
                .is_none()
        );
        let mut budget = 10_000;
        assert!(
            nested_complement
                .find_at_utf16_bounded(&[b'b' as u16], 0, &mut budget)
                .unwrap()
                .is_some()
        );
        assert!(Regex::with_flags(r"[^\q{ab}--\q{ab}]", "v").is_err());
        assert!(Regex::with_flags(r"[^\q{ab}&&\q{a}]", "v").is_ok());
        assert!(Regex::with_flags(r"[^a--\p{RGI_Emoji}]", "v").is_ok());
        assert!(Regex::with_flags(r"[^\p{RGI_Emoji}&&a]", "v").is_ok());

        let backspace = Regex::with_flags(r"^[\q{\b}]$", "v").unwrap();
        let mut budget = 10_000;
        assert!(
            backspace
                .find_at_utf16_bounded(&[0x08], 0, &mut budget)
                .unwrap()
                .is_some()
        );
        assert!(Regex::with_flags(r"[\q{!#}]", "v").is_ok());
        assert!(Regex::with_flags(r"[\q{!!}]", "v").is_err());
        assert!(Regex::with_flags(r"[\q{[}]", "v").is_err());

        let ampersand = Regex::with_flags(r"^[a&b]$", "v").unwrap();
        for code_point in [b'a' as u16, b'&' as u16, b'b' as u16] {
            let mut budget = 10_000;
            assert!(
                ampersand
                    .find_at_utf16_bounded(&[code_point], 0, &mut budget)
                    .unwrap()
                    .is_some()
            );
        }
    }

    #[test]
    fn unicode_string_properties_are_bounded_and_reject_negation() {
        let keycap = Regex::with_flags(r"^\p{Emoji_Keycap_Sequence}$", "v").unwrap();
        let mut budget = 100_000;
        assert_eq!(
            keycap
                .find_at_utf16_bounded(&[b'9' as u16, 0xfe0f, 0x20e3], 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..3
        );
        let mut budget = 100_000;
        assert!(
            keycap
                .find_at_utf16_bounded(&[b'9' as u16], 0, &mut budget)
                .unwrap()
                .is_none()
        );

        assert!(Regex::with_flags(r"[^\q{ab}]", "v").is_err());
        assert!(Regex::with_flags(r"\P{RGI_Emoji}", "v").is_err());
        assert!(Regex::with_flags(&r"\p{RGI_Emoji}".repeat(64), "v").is_err());
        let hostile = r"\p{RGI_Emoji}".repeat(1_000);
        assert!(
            Regex::validate_syntax(hostile.chars().map(u32::from), "v")
                .unwrap_err()
                .text
                .contains("materialization is too large")
        );
        let duplicate_empty_alternatives = format!(r"[\q{{{}}}]", "|".repeat(70_000));
        assert!(
            Regex::validate_syntax(duplicate_empty_alternatives.chars().map(u32::from), "v")
                .unwrap_err()
                .text
                .contains("materialization is too large")
        );

        let mut alternatives = Vec::new();
        for index in 0..300u32 {
            let mut alternative = "a".repeat(255);
            alternative.push(char::from_u32(0x1000 + index).unwrap());
            alternatives.push(alternative);
        }
        let oversized_trie = format!(r"[\q{{{}}}]", alternatives.join("|"));
        assert!(
            Regex::validate_syntax(oversized_trie.chars().map(u32::from), "v")
                .unwrap_err()
                .text
                .contains("trie is too large")
        );
    }

    #[test]
    fn exact_match_does_not_scan_later_candidates() {
        let regex = Regex::with_flags("z", "u").unwrap();
        let mut budget = 1_000;
        assert!(
            regex
                .find_at_utf16_bounded(&[b'a' as u16; 1000], 0, &mut budget)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn duplicate_named_backreference_selects_the_participating_capture() {
        let regex = Regex::with_flags(r"(?:(?<q>a)|(?<q>b))\k<q>", "u").unwrap();
        let mut budget = 10_000;
        let matched = regex
            .find_from_utf16_bounded(&[b'b' as u16, b'b' as u16], 0, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(matched.range, 0..2);
        let mut budget = 10_000;
        assert!(
            regex
                .find_from_utf16_bounded(&[b'b' as u16, b'c' as u16], 0, &mut budget)
                .unwrap()
                .is_none()
        );

        let optional = Regex::with_flags(r"(?:(?<q>a)|(?<q>b))?\k<q>", "u").unwrap();
        let mut budget = 10_000;
        assert_eq!(
            optional
                .find_at_utf16_bounded(&[], 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..0
        );
    }

    #[test]
    fn deterministic_counted_capture_loop_charges_linear_work() {
        let regex = Regex::with_flags(r"^((a)){100000}\uD800$", "u").unwrap();
        let mut input = vec![b'a' as u16; 100_000];
        input.push(0xd800);
        let mut budget = 2_000_000;
        assert_eq!(
            regex
                .find_at_utf16_bounded(&input, 0, &mut budget)
                .unwrap()
                .unwrap()
                .range,
            0..100_001
        );
    }

    #[test]
    fn bounds_named_capture_prepass_and_duplicate_backref_expansion() {
        let unique = (0..1_025)
            .map(|index| format!("(?<q{index}>a)"))
            .collect::<String>();
        assert!(
            Regex::validate_unicode_resource_limits(unique.chars().map(u32::from), "u").is_err()
        );
        assert!(Regex::with_flags(&unique, "u").is_err());

        let duplicate_branches = (0..65).map(|_| "(?<q>a)").collect::<Vec<_>>().join("|");
        assert!(Regex::with_flags(&format!("(?:{duplicate_branches})"), "u").is_err());

        let bounded_branches = (0..64).map(|_| "(?<q>a)").collect::<Vec<_>>().join("|");
        let expanded = format!("(?:{bounded_branches}){}", r"\k<q>".repeat(257));
        assert!(
            Regex::validate_unicode_resource_limits(expanded.chars().map(u32::from), "u").is_err()
        );
        assert!(Regex::with_flags(&expanded, "u").is_err());
    }
}

#[cfg(test)]
mod flags_tests {
    use super::{Flags, Regex};

    #[test]
    fn unicode_sets_flags_round_trip_as_v() {
        let flags = Flags::from("v");
        assert!(flags.unicode);
        assert!(flags.unicode_sets);
        assert_eq!(flags.to_string(), "v");
    }

    #[test]
    fn string_sets_match_through_the_classical_api() {
        let keycap = "9\u{fe0f}\u{20e3}";
        let property = Regex::with_flags(r"^\p{Emoji_Keycap_Sequence}$", "v").unwrap();
        assert_eq!(property.find(keycap).unwrap().range, 0..keycap.len());

        let lookbehind = Regex::with_flags(r"(?<=[\q{ab}])c", "v").unwrap();
        assert_eq!(lookbehind.find("abc").unwrap().range, 2..3);
        assert!(lookbehind.find("bac").is_none());

        let subtraction = Regex::with_flags(r"^[k--\q{K}]$", "iv").unwrap();
        for input in ["k", "K", "K"] {
            assert!(subtraction.find(input).is_none());
        }
    }
}

impl FromStr for Regex {
    type Err = Error;

    /// Attempts to parse a string into a regular expression
    #[inline]
    fn from_str(s: &str) -> Result<Self, Error> {
        Self::new(s)
    }
}

// Pattern trait implementation for str::find, str::contains, etc.
#[cfg(feature = "pattern")]
mod pattern_impl {
    use super::*;
    use core::str::pattern::{Pattern, ReverseSearcher, SearchStep, Searcher};

    /// A searcher for a regex pattern.
    pub struct RegexSearcher<'r, 't> {
        haystack: &'t str,
        regex: &'r Regex,
        current_pos: usize,
        done: bool,
        // For reverse searching
        reverse_pos: usize,
        reverse_done: bool,
    }

    impl<'r, 't> RegexSearcher<'r, 't> {
        fn new(regex: &'r Regex, haystack: &'t str) -> Self {
            Self {
                haystack,
                regex,
                current_pos: 0,
                done: false,
                reverse_pos: haystack.len(),
                reverse_done: false,
            }
        }

        fn find_last_match_before(&self, pos: usize) -> Option<super::Match> {
            // Find all matches up to the given position and return the last one
            let mut last_match = None;
            for m in self.regex.find_from(self.haystack, 0) {
                if m.end() <= pos {
                    last_match = Some(m);
                } else {
                    break;
                }
            }
            last_match
        }
    }

    unsafe impl<'r, 't> Searcher<'t> for RegexSearcher<'r, 't> {
        fn haystack(&self) -> &'t str {
            self.haystack
        }

        fn next(&mut self) -> SearchStep {
            if self.done {
                return SearchStep::Done;
            }

            // Try to find the next match starting from current position
            if let Some(m) = self.regex.find_from(self.haystack, self.current_pos).next() {
                let match_start = m.start();
                let match_end = m.end();

                // Handle any gap between current position and match start
                if self.current_pos < match_start {
                    let reject_end = match_start;
                    let reject_start = self.current_pos;
                    self.current_pos = match_start;
                    return SearchStep::Reject(reject_start, reject_end);
                }

                // Return the match
                self.current_pos = match_end;

                // Handle zero-width matches to avoid infinite loops
                if match_start == match_end {
                    // For zero-width matches, we need to advance at least one byte
                    // to avoid infinite loops
                    if match_end < self.haystack.len() {
                        // Find the next character boundary
                        let mut next_pos = match_end + 1;
                        while next_pos < self.haystack.len()
                            && !self.haystack.is_char_boundary(next_pos)
                        {
                            next_pos += 1;
                        }
                        self.current_pos = next_pos;
                    } else {
                        // We're at the end of the string
                        self.done = true;
                    }
                }

                SearchStep::Match(match_start, match_end)
            } else {
                // No more matches, reject remaining text if any
                if self.current_pos < self.haystack.len() {
                    let reject_start = self.current_pos;
                    let reject_end = self.haystack.len();
                    self.current_pos = self.haystack.len();
                    self.done = true;
                    SearchStep::Reject(reject_start, reject_end)
                } else {
                    self.done = true;
                    SearchStep::Done
                }
            }
        }
    }

    unsafe impl<'r, 't> ReverseSearcher<'t> for RegexSearcher<'r, 't> {
        fn next_back(&mut self) -> SearchStep {
            if self.reverse_done {
                return SearchStep::Done;
            }

            // Try to find the last match before current reverse position
            if let Some(m) = self.find_last_match_before(self.reverse_pos) {
                let match_start = m.start();
                let match_end = m.end();

                // Handle any gap between match end and current reverse position
                if match_end < self.reverse_pos {
                    let reject_start = match_end;
                    let reject_end = self.reverse_pos;
                    self.reverse_pos = match_end;
                    return SearchStep::Reject(reject_start, reject_end);
                }

                // Return the match
                self.reverse_pos = match_start;

                // Handle zero-width matches
                if match_start == match_end {
                    // For zero-width matches, move back by one character
                    if match_start > 0 {
                        let mut prev_pos = match_start - 1;
                        while prev_pos > 0 && !self.haystack.is_char_boundary(prev_pos) {
                            prev_pos -= 1;
                        }
                        self.reverse_pos = prev_pos;
                    } else {
                        // We're at the beginning of the string
                        self.reverse_done = true;
                    }
                }

                SearchStep::Match(match_start, match_end)
            } else {
                // No more matches, reject remaining text if any
                if self.reverse_pos > 0 {
                    let reject_start = 0;
                    let reject_end = self.reverse_pos;
                    self.reverse_pos = 0;
                    self.reverse_done = true;
                    SearchStep::Reject(reject_start, reject_end)
                } else {
                    self.reverse_done = true;
                    SearchStep::Done
                }
            }
        }
    }

    impl<'r> Pattern for &'r Regex {
        type Searcher<'a> = RegexSearcher<'r, 'a>;

        fn into_searcher(self, haystack: &str) -> Self::Searcher<'_> {
            RegexSearcher::new(self, haystack)
        }
    }
}

#[cfg(feature = "pattern")]
pub use pattern_impl::*;

// Support for using regress with different regex backends.
// Currently there is only the classical backtracking, and PikeVM.
#[doc(hidden)]
pub mod backends {
    use super::Regex;
    use super::exec;
    use super::indexing;
    pub use crate::emit::emit;
    pub use crate::optimizer::optimize;
    pub use crate::parse::try_parse;

    /// An Executor using the classical backtracking algorithm.
    pub type BacktrackExecutor<'r, 't> =
        super::classicalbacktrack::BacktrackExecutor<'r, indexing::Utf8Input<'t>>;

    /// A Executor using the PikeVM executor.
    #[cfg(feature = "backend-pikevm")]
    pub type PikeVMExecutor<'r, 't> = super::pikevm::PikeVMExecutor<'r, indexing::Utf8Input<'t>>;

    /// An alias type to the default Executor.
    pub type DefaultExecutor<'r, 't> = BacktrackExecutor<'r, 't>;

    /// An alias type to the default executor's ASCII form.
    pub type DefaultAsciiExecutor<'r, 't> =
        <DefaultExecutor<'r, 't> as exec::Executor<'r, 't>>::AsAscii;

    /// Searches `text`, returning an iterator over non-overlapping matches.
    pub fn find<'r, 't, Executor: exec::Executor<'r, 't>>(
        re: &'r Regex,
        text: &'t str,
        start: usize,
    ) -> exec::Matches<Executor> {
        exec::Matches::new(Executor::new(&re.cr, text), start)
    }

    /// Searches `text`, returning an iterator over non-overlapping matches.
    /// This is a convenience method to avoid E0223.
    pub fn find_ascii<'r, 't, Executor: exec::Executor<'r, 't>>(
        re: &'r Regex,
        text: &'t str,
        start: usize,
    ) -> exec::Matches<Executor::AsAscii> {
        find::<Executor::AsAscii>(re, text, start)
    }
}

/// Escapes all special regex characters in a string to make it a literal match.
///
/// This function takes a string and returns a new string with all special
/// regex characters escaped with backslashes, so the resulting string can be
/// used as a literal pattern in a regular expression.
///
/// # Example
///
/// ```
/// use regress::escape;
///
/// let escaped = escape("Hello. How are you?");
/// assert_eq!(escaped, "Hello\\. How are you\\?");
///
/// let escaped = escape("$100 + tax (15%)");
/// assert_eq!(escaped, "\\$100 \\+ tax \\(15%\\)");
/// ```
pub fn escape(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for c in text.chars() {
        match c {
            // Characters that have special meaning in regex and need escaping
            '\\' | '^' | '$' | '.' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}' => {
                result.push('\\');
                result.push(c);
            }
            // All other characters are literal
            _ => result.push(c),
        }
    }

    result
}
