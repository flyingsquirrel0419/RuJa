// Copyright 2016 The Fancy Regex Authors.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Backtracking VM for implementing fancy regexes.
//!
//! Read <https://swtch.com/~rsc/regexp/regexp2.html> for a good introduction for how this works.
//!
//! The VM executes a sequence of instructions (a program) against an input string. It keeps track
//! of a program counter (PC) and an index into the string (IX). Execution can have one or more
//! threads.
//!
//! One of the basic instructions is `Lit`, which matches a string against the input. If it matches,
//! the PC advances to the next instruction and the IX to the position after the matched string.
//! If not, the current thread is stopped because it failed.
//!
//! If execution reaches an `End` instruction, the program is successful because a match was found.
//! If there are no more threads to execute, the program has failed to match.
//!
//! A very simple program for the regex `a`:
//!
//! ```text
//! 0: Lit("a")
//! 1: End
//! ```
//!
//! The `Split` instruction causes execution to split into two threads. The first thread is executed
//! with the current string index. If it fails, we reset the string index and resume execution with
//! the second thread. That is what "backtracking" refers to. In order to do that, we keep a stack
//! of threads (PC and IX) to try.
//!
//! Example program for the regex `ab|ac`:
//!
//! ```text
//! 0: Split(1, 4)
//! 1: Lit("a")
//! 2: Lit("b")
//! 3: Jmp(6)
//! 4: Lit("a")
//! 5: Lit("c")
//! 6: End
//! ```
//!
//! The `Jmp` instruction causes execution to jump to the specified instruction. In the example it
//! is needed to separate the two threads.
//!
//! Let's step through execution with that program for the input `ac`:
//!
//! 1. We're at PC 0 and IX 0
//! 2. `Split(1, 4)` means we save a thread with PC 4 and IX 0 for trying later
//! 3. Continue at `Lit("a")` which matches, so we advance IX to 1
//! 4. `Lit("b")` doesn't match at IX 1 (`"b" != "c"`), so the thread fails
//! 5. We continue with the previously saved thread at PC 4 and IX 0 (backtracking)
//! 6. Both `Lit("a")` and `Lit("c")` match and we reach `End` -> successful match (index 0 to 2)

use alloc::collections::BTreeSet;
use alloc::string::String;
#[cfg(feature = "variable-lookbehinds")]
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use bit_set::BitSet;
use regex_automata::meta::Regex;
use regex_automata::util::look::LookMatcher;
use regex_automata::util::primitives::NonMaxUsize;
use regex_automata::Anchored;
use regex_automata::Input;

#[cfg(feature = "variable-lookbehinds")]
use regex_automata::util::pool::Pool;

#[cfg(feature = "variable-lookbehinds")]
pub(crate) type CachePoolFn = alloc::boxed::Box<
    dyn Fn() -> regex_automata::hybrid::dfa::Cache
        + Send
        + Sync
        + core::panic::UnwindSafe
        + core::panic::RefUnwindSafe,
>;

use crate::error::RuntimeError;
use crate::prev_codepoint_ix;
use crate::Assertion;
use crate::Error;
use crate::Formatter;
use crate::Result;
use crate::{codepoint_len, HardRegexRuntimeOptions, RepeatBound, RepeatCount};

/// Enable tracing of VM execution. Only for debugging/investigating.
const OPTION_TRACE: u32 = 1 << 0;
/// When iterating over all matches within a text (e.g. with `find_iter`), empty matches need to be
/// handled specially. If we kept matching at the same position, we'd never stop. So what we do
/// after we've had an empty match, is to advance the position where matching is attempted.
/// If `\G` is used in the pattern, that means it no longer matches. If we didn't tell the VM about
/// the fact that we skipped because of an empty match, it would still treat `\G` as matching. So
/// this option is for communicating that to the VM. Phew.
pub(crate) const OPTION_SKIPPED_EMPTY_MATCH: u32 = 1 << 1;
/// When this option is set, the VM will reject any match where the engine consumed no characters.
/// \K is ignored as part of this check - so empty matches can still be reported if the engine
/// consumed characters and then \K was used afterwards.
pub(crate) const OPTION_FIND_NOT_EMPTY: u32 = 1 << 2;
/// Require the first match attempt to start at the caller's position.
pub(crate) const OPTION_EXACT_POSITION: u32 = 1 << 3;

const MAX_STACK: usize = 1_000_000;
const ECMASCRIPT_MAX_STACK: usize = 100_000;

/// Represents a range of capture groups by storing the first and last group numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureGroupRange(pub usize, pub usize);

impl CaptureGroupRange {
    /// Returns the start (first) group number.
    pub fn start(&self) -> usize {
        self.0
    }

    /// Returns the end (last) group number.
    pub fn end(&self) -> usize {
        self.1
    }

    /// Converts this range to an Option, returning None if start equals end (no capture groups).
    pub fn to_option_if_non_empty(self) -> Option<Self> {
        if self.start() == self.end() {
            None
        } else {
            Some(self)
        }
    }
}

#[derive(Clone)]
/// Delegate matching to the regex crate
pub struct Delegate {
    /// The regex
    pub inner: Regex,
    /// The regex pattern as a string
    pub pattern: String,
    /// The range of capture groups. None if there are no capture groups.
    pub capture_groups: Option<CaptureGroupRange>,
}

impl core::fmt::Debug for Delegate {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        // Ensures it fails to compile if the struct changes
        let Self {
            inner: _,
            pattern,
            capture_groups,
        } = self;

        f.debug_struct("Delegate")
            .field("pattern", pattern)
            .field("capture_groups", capture_groups)
            .finish()
    }
}

#[cfg(feature = "variable-lookbehinds")]
/// Delegate matching in reverse to regex-automata
pub struct ReverseBackwardsDelegate {
    /// The regex pattern as a string which will be matched in reverse, in a backwards direction
    pub pattern: String,
    /// The delegate regex to match backwards (wrapped in Arc for efficient cloning)
    pub(crate) dfa: Arc<regex_automata::hybrid::dfa::DFA>,
    /// Cache pool for DFA searches
    pub(crate) cache_pool: Pool<regex_automata::hybrid::dfa::Cache, CachePoolFn>,
    /// The forward regex for capture group extraction
    pub(crate) capture_group_extraction_inner: Option<Regex>,
    /// The range of capture groups. None if there are no capture groups.
    pub capture_groups: Option<CaptureGroupRange>,
}

#[cfg(feature = "variable-lookbehinds")]
impl Clone for ReverseBackwardsDelegate {
    fn clone(&self) -> Self {
        let dfa_for_closure = Arc::clone(&self.dfa);
        let create: CachePoolFn = alloc::boxed::Box::new(move || dfa_for_closure.create_cache());
        Self {
            pattern: self.pattern.clone(),
            cache_pool: Pool::new(create),
            dfa: Arc::clone(&self.dfa),
            capture_group_extraction_inner: self.capture_group_extraction_inner.clone(),
            capture_groups: self.capture_groups,
        }
    }
}

#[cfg(feature = "variable-lookbehinds")]
impl core::fmt::Debug for ReverseBackwardsDelegate {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        // Ensures it fails to compile if the struct changes
        let Self {
            pattern,
            dfa: _,
            cache_pool: _,
            capture_group_extraction_inner: _,
            capture_groups,
        } = self;

        f.debug_struct("ReverseBackwardsDelegate")
            .field("pattern", pattern)
            .field("capture_groups", capture_groups)
            .finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VmRepeatCount(pub(crate) Option<usize>);

impl VmRepeatCount {
    pub(crate) fn from_repeat_count(value: &RepeatCount) -> Self {
        Self(value.to_usize())
    }

    fn reached(self, repetitions: usize) -> bool {
        self.0 == Some(repetitions)
    }

    fn minimum_satisfied(self, repetitions: usize) -> bool {
        self.0.map_or(false, |minimum| repetitions >= minimum)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum VmRepeatBound {
    Finite(VmRepeatCount),
    Infinity,
}

impl VmRepeatBound {
    pub(crate) fn from_repeat_bound(value: &RepeatBound) -> Self {
        match value {
            RepeatBound::Finite(value) => Self::Finite(VmRepeatCount::from_repeat_count(value)),
            RepeatBound::Infinity => Self::Infinity,
        }
    }

    fn reached(self, repetitions: usize) -> bool {
        matches!(self, Self::Finite(value) if value.reached(repetitions))
    }
}

/// Instruction of the VM.
#[derive(Debug)]
pub enum Insn {
    /// Successful end of program
    End,
    /// Match any character (including newline)
    Any,
    /// Match the character immediately before the current index
    AnyBackwards,
    /// Match any character except for the line feed character (`\n`)
    AnyNoNL,
    /// Match the character immediately before the current index unless it is `\n`
    AnyNoNLBackwards,
    /// Match any character except for a carriage return or line feed character (`\r` or `\n`)
    AnyNoCRLF,
    /// Match the character immediately before the current index unless it is `\r` or `\n`
    AnyNoCRLFBackwards,
    /// Match an ECMAScript general newline immediately before the current index
    GeneralNewlineBackwards {
        /// Whether Unicode-only newline characters are recognized
        unicode: bool,
    },
    /// Assertions
    Assertion(Assertion),
    /// Match the literal string at the current index
    Lit(String), // should be cow?
    /// Match the literal string immediately before the current index
    LitBackwards(String),
    /// Match a case-insensitive literal immediately before the current index
    LitCaseiBackwards(String),
    /// Split execution into two threads. The two fields are positions of instructions. Execution
    /// first tries the first thread. If that fails, the second position is tried.
    Split(usize, usize),
    /// Like `Split`, but also updates `match_attempt_start` for `OPTION_FIND_NOT_EMPTY` tracking.
    /// Used exclusively for the unanchored search preamble.
    SplitUnanchored(usize, usize),
    /// Jump to instruction at position
    Jmp(usize),
    /// Save the current string index into the specified slot
    Save(usize),
    /// Save `0` into the specified slot
    Save0(usize),
    /// Save the current string index into the specified capture group start slot if the capture group is empty
    /// or has already completed.
    SaveCaptureGroupStart(usize),
    /// Clear the start/end slots for a half-open range of capture groups.
    /// Every write is recorded through State::save so backtracking restores the
    /// captures visible at the repetition exit branch.
    ClearCaptureRange {
        /// First capture group to clear.
        start_group: usize,
        /// One past the final capture group to clear.
        end_group: usize,
    },
    /// Set the string index to the value that was saved in the specified slot
    Restore(usize),
    /// Repeat greedily (match as much as possible)
    RepeatGr {
        /// Minimum number of matches
        lo: VmRepeatCount,
        /// Maximum number of matches
        hi: VmRepeatBound,
        /// The instruction after the repeat
        next: usize,
        /// The slot for keeping track of the number of repetitions
        repeat: usize,
    },
    /// Repeat non-greedily (prefer matching as little as possible)
    RepeatNg {
        /// Minimum number of matches
        lo: VmRepeatCount,
        /// Maximum number of matches
        hi: VmRepeatBound,
        /// The instruction after the repeat
        next: usize,
        /// The slot for keeping track of the number of repetitions
        repeat: usize,
    },
    /// Repeat greedily and prevent infinite loops from empty matches
    RepeatEpsilonGr {
        /// Minimum number of matches
        lo: VmRepeatCount,
        /// Maximum number of matches
        hi: VmRepeatBound,
        /// The instruction after the repeat
        next: usize,
        /// The slot for keeping track of the number of repetitions
        repeat: usize,
        /// The slot for saving the previous IX to check if we had an empty match
        check: usize,
        /// Whether an empty optional iteration fails instead of ending the repeat
        fail_on_empty: bool,
    },
    /// Repeat non-greedily and prevent infinite loops from empty matches
    RepeatEpsilonNg {
        /// Minimum number of matches
        lo: VmRepeatCount,
        /// Maximum number of matches
        hi: VmRepeatBound,
        /// The instruction after the repeat
        next: usize,
        /// The slot for keeping track of the number of repetitions
        repeat: usize,
        /// The slot for saving the previous IX to check if we had an empty match
        check: usize,
        /// Whether an empty optional iteration fails instead of ending the repeat
        fail_on_empty: bool,
    },
    /// Negative look-around failed
    FailNegativeLookAround,
    /// Set IX back by the specified number of characters
    GoBack(usize),
    /// Back reference to a group number to check
    Backref {
        /// The save slot representing the start of the capture group
        slot: usize,
        /// Whether the backref should be matched case insensitively
        casei: bool,
    },
    /// Back reference matched immediately before the current index
    BackrefBackwards {
        /// The save slot representing the start of the capture group
        slot: usize,
        /// Whether the backref should be matched case insensitively
        casei: bool,
    },
    /// Back reference to the sole populated capture in a registered set, or
    /// an empty match when every capture in that set is unset.
    BackrefSet {
        /// Index into the program's ECMAScript backreference-set table.
        set_id: usize,
        /// Whether the captured text is matched case-insensitively.
        casei: bool,
    },
    /// ECMAScript duplicate-name back reference matched before the current index
    BackrefSetBackwards {
        /// Index into the program's ECMAScript backreference-set table.
        set_id: usize,
        /// Whether the captured text is matched case-insensitively.
        casei: bool,
    },
    /// Begin of atomic group
    BeginAtomic,
    /// End of atomic group
    EndAtomic,
    /// Delegate matching to the regex crate
    Delegate(Delegate),
    /// Match one delegated character expression immediately before the current index
    DelegateBackwards(Delegate),
    /// Anchor to match at the position where the previous match ended
    ContinueFromPreviousMatchEnd {
        /// Whether this is at the start of the pattern (allowing early exit on failure)
        at_start: bool,
    },
    /// Continue only if the specified capture group has already been populated as part of the match
    BackrefExistsCondition(usize),
    /// Immediately fail the current match attempt and trigger backtracking.
    /// This is used for backtracking control verbs like `(*FAIL)`.
    Fail,
    #[cfg(feature = "variable-lookbehinds")]
    /// Reverse lookbehind using regex-automata for variable-sized patterns
    BackwardsDelegate(ReverseBackwardsDelegate),
    /// Absent repeater operator - matches if delegate does not match from current position
    AbsentRepeater(Delegate),
}

/// Sequence of instructions for the VM to execute.
#[derive(Debug)]
pub struct Prog {
    /// Instructions of the program
    pub body: Vec<Insn>,
    n_saves: usize,
    ecmascript_backref_sets: Vec<Vec<usize>>,
    unicode_casei: bool,
    ecmascript_mode: bool,
}

impl Prog {
    pub(crate) fn new(
        body: Vec<Insn>,
        n_saves: usize,
        ecmascript_backref_sets: Vec<Vec<usize>>,
        unicode_casei: bool,
        ecmascript_mode: bool,
    ) -> Prog {
        Prog {
            body,
            n_saves,
            ecmascript_backref_sets,
            unicode_casei,
            ecmascript_mode,
        }
    }

    #[doc(hidden)]
    pub(crate) fn debug_print(&self, writer: &mut Formatter<'_>) -> core::fmt::Result {
        for (i, insn) in self.body.iter().enumerate() {
            writeln!(writer, "{:3}: {:?}", i, insn)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Branch {
    pc: usize,
    ix: usize,
    nsave: usize,
}

#[derive(Debug)]
struct Save {
    slot: usize,
    value: usize,
}

struct State {
    /// Saved values indexed by slot. Mostly indices to s, but can be repeat values etc.
    /// Always contains the saves of the current state.
    saves: Vec<usize>,
    /// Stack of backtrack branches.
    stack: Vec<Branch>,
    /// Old saves (slot, value)
    oldsave: Vec<Save>,
    /// Number of saves at the end of `oldsave` that need to be restored to `saves` on pop
    nsave: usize,
    /// Slots already recorded in the current copy-on-write delta.
    current_save_slots: BitSet,
    explicit_sp: usize,
    /// Maximum size of the stack. If the size would be exceeded during execution, a `StackOverflow`
    /// error is raised.
    max_stack: usize,
    #[allow(dead_code)]
    options: u32,
}

// Each element in the stack conceptually represents the entire state
// of the machine: the pc (index into prog), the index into the
// string, and the entire vector of saves. However, copying the save
// vector on every push/pop would be inefficient, so instead we use a
// copy-on-write approach for each slot within the save vector. The
// top `nsave` elements in `oldsave` represent the delta from the
// current machine state to the top of stack.

impl State {
    fn new(n_saves: usize, max_stack: usize, options: u32) -> State {
        State {
            saves: vec![usize::MAX; n_saves],
            stack: Vec::new(),
            oldsave: Vec::new(),
            nsave: 0,
            current_save_slots: BitSet::new(),
            explicit_sp: n_saves,
            max_stack,
            options,
        }
    }

    // push a backtrack branch
    fn push(&mut self, pc: usize, ix: usize) -> Result<()> {
        if self.stack.len() < self.max_stack {
            let nsave = self.nsave;
            self.stack.push(Branch { pc, ix, nsave });
            self.nsave = 0;
            self.current_save_slots.clear();
            self.trace_stack("push");
            Ok(())
        } else {
            Err(Error::RuntimeError(RuntimeError::StackOverflow))
        }
    }

    // pop a backtrack branch
    fn pop(&mut self) -> (usize, usize) {
        for _ in 0..self.nsave {
            let Save { slot, value } = self.oldsave.pop().unwrap();
            self.saves[slot] = value;
        }
        let Branch { pc, ix, nsave } = self.stack.pop().unwrap();
        self.nsave = nsave;
        self.rebuild_current_save_slots();
        self.trace_stack("pop");
        (pc, ix)
    }

    fn save(&mut self, slot: usize, val: usize) {
        if self.current_save_slots.contains(slot) {
            self.saves[slot] = val;
            return;
        }
        self.oldsave.push(Save {
            slot,
            value: self.saves[slot],
        });
        self.nsave += 1;
        self.current_save_slots.insert(slot);
        self.saves[slot] = val;

        #[cfg(feature = "std")]
        if self.options & OPTION_TRACE != 0 {
            println!("saves: {:?}", self.saves);
        }
    }

    fn get(&self, slot: usize) -> usize {
        self.saves[slot]
    }

    fn rebuild_current_save_slots(&mut self) {
        self.current_save_slots.clear();
        let start = self.oldsave.len().saturating_sub(self.nsave);
        for save in &self.oldsave[start..] {
            self.current_save_slots.insert(save.slot);
        }
    }

    // push a value onto the explicit stack; note: the entire contents of
    // the explicit stack is saved and restored on backtrack.
    fn stack_push(&mut self, val: usize) {
        if self.saves.len() == self.explicit_sp {
            self.saves.push(self.explicit_sp + 1);
        }
        let explicit_sp = self.explicit_sp;
        let sp = self.get(explicit_sp);
        if self.saves.len() == sp {
            self.saves.push(val);
        } else {
            self.save(sp, val);
        }
        self.save(explicit_sp, sp + 1);
    }

    // pop a value from the explicit stack
    fn stack_pop(&mut self) -> usize {
        let explicit_sp = self.explicit_sp;
        let sp = self.get(explicit_sp) - 1;
        let result = self.get(sp);
        self.save(explicit_sp, sp);
        result
    }

    /// Get the current number of backtrack branches
    fn backtrack_count(&self) -> usize {
        self.stack.len()
    }

    /// Discard backtrack branches that were pushed since the call to `backtrack_count`.
    ///
    /// What we want:
    /// * Keep the current `saves` as they are
    /// * Only keep `count` backtrack branches on `stack`, discard the rest
    /// * Keep the first `oldsave` for each slot, discard the rest (multiple pushes might have
    ///   happened with saves to the same slot)
    fn backtrack_cut(&mut self, count: usize) {
        if self.stack.len() == count {
            // no backtrack branches to discard, all good
            return;
        }
        // start and end indexes of old saves for the branch we're cutting to
        let (oldsave_start, oldsave_end) = {
            let mut end = self.oldsave.len() - self.nsave;
            for &Branch { nsave, .. } in &self.stack[count + 1..] {
                end -= nsave;
            }
            let start = end - self.stack[count].nsave;
            (start, end)
        };
        let mut saved = BTreeSet::new();
        // keep all the old saves of our branch (they're all for different slots)
        for &Save { slot, .. } in &self.oldsave[oldsave_start..oldsave_end] {
            saved.insert(slot);
        }
        let mut oldsave_ix = oldsave_end;
        // for other old saves, keep them only if they're for a slot that we haven't saved yet
        for ix in oldsave_end..self.oldsave.len() {
            let Save { slot, .. } = self.oldsave[ix];
            let new_slot = saved.insert(slot);
            if new_slot {
                // put the save we want to keep (ix) after the ones we already have (oldsave_ix)
                // note that it's fine if the indexes are the same (then swapping is a no-op)
                self.oldsave.swap(oldsave_ix, ix);
                oldsave_ix += 1;
            }
        }
        self.stack.truncate(count);
        self.oldsave.truncate(oldsave_ix);
        self.nsave = oldsave_ix - oldsave_start;
        self.rebuild_current_save_slots();
    }

    #[inline]
    #[allow(unused_variables)]
    fn trace_stack(&self, operation: &str) {
        #[cfg(feature = "std")]
        if self.options & OPTION_TRACE != 0 {
            println!("stack after {}: {:?}", operation, self.stack);
        }
    }
}

fn charge_work(work_count: &mut usize, amount: usize, limit: usize) -> Result<()> {
    *work_count = work_count.saturating_add(amount);
    if *work_count > limit {
        Err(Error::RuntimeError(RuntimeError::BacktrackLimitExceeded))
    } else {
        Ok(())
    }
}

fn push_branch(
    state: &mut State,
    pc: usize,
    ix: usize,
    work_count: &mut usize,
    work_limit: usize,
    charge: bool,
) -> Result<()> {
    if charge {
        charge_work(work_count, 1, work_limit)?;
    }
    state.push(pc, ix)
}

fn codepoint_len_at(s: &str, ix: usize) -> usize {
    codepoint_len(s.as_bytes()[ix])
}

#[inline]
fn matches_literal(s: &str, ix: usize, end: usize, literal: &str) -> bool {
    // Compare as bytes because the literal might be a single byte char whereas ix
    // points to a multibyte char. Comparing with str would result in an error like
    // "byte index N is not a char boundary".
    end <= s.len() && &s.as_bytes()[ix..end] == literal.as_bytes()
}

fn legacy_canonicalize(ch: char) -> char {
    let mut uppercase = ch.to_uppercase();
    let Some(canonical) = uppercase.next() else {
        return ch;
    };
    if uppercase.next().is_some() || (!ch.is_ascii() && canonical.is_ascii()) {
        ch
    } else {
        canonical
    }
}

fn unicode_simple_case_eq(left: char, right: char) -> bool {
    if left == right {
        return true;
    }
    if left.is_ascii() && right.is_ascii() {
        return left.eq_ignore_ascii_case(&right);
    }
    use regex_syntax::hir::{ClassUnicode, ClassUnicodeRange};
    let mut class = ClassUnicode::new([ClassUnicodeRange::new(left, left)]);
    class.case_fold_simple();
    class
        .ranges()
        .iter()
        .any(|range| range.start() <= right && right <= range.end())
}

fn chars_equal_case_insensitive(left: char, right: char, unicode_casei: bool) -> bool {
    if unicode_casei {
        unicode_simple_case_eq(left, right)
    } else {
        legacy_canonicalize(left) == legacy_canonicalize(right)
    }
}

fn matches_literal_casei(s: &str, ix: usize, literal: &str, unicode_casei: bool) -> Option<usize> {
    let mut end = ix;
    let mut actual = s.get(ix..)?.chars();
    for expected in literal.chars() {
        let candidate = actual.next()?;
        if !chars_equal_case_insensitive(expected, candidate, unicode_casei) {
            return None;
        }
        end += candidate.len_utf8();
    }
    Some(end)
}

fn previous_char(s: &str, ix: usize) -> Option<(usize, char)> {
    if ix == 0 || ix > s.len() || !s.is_char_boundary(ix) {
        return None;
    }
    let start = prev_codepoint_ix(s, ix);
    Some((start, s.get(start..ix)?.chars().next()?))
}

fn is_ecmascript_word_character(ch: char, unicode_ignore_case: bool) -> bool {
    ch.is_ascii_alphanumeric()
        || ch == '_'
        || (unicode_ignore_case && matches!(ch, '\u{017f}' | '\u{212a}'))
}

fn is_ecmascript_word_boundary(s: &str, ix: usize, unicode_ignore_case: bool) -> bool {
    let previous = previous_char(s, ix)
        .map(|(_, ch)| is_ecmascript_word_character(ch, unicode_ignore_case))
        .unwrap_or(false);
    let next = s
        .get(ix..)
        .and_then(|suffix| suffix.chars().next())
        .map(|ch| is_ecmascript_word_character(ch, unicode_ignore_case))
        .unwrap_or(false);
    previous != next
}

fn matches_literal_backwards(s: &str, ix: usize, literal: &str) -> Option<usize> {
    let start = ix.checked_sub(literal.len())?;
    matches_literal(s, start, ix, literal).then_some(start)
}

fn matches_literal_casei_backwards(
    s: &str,
    ix: usize,
    literal: &str,
    unicode_casei: bool,
) -> Option<usize> {
    let mut start = ix;
    for expected in literal.chars().rev() {
        let (candidate_start, candidate) = previous_char(s, start)?;
        if !chars_equal_case_insensitive(expected, candidate, unicode_casei) {
            return None;
        }
        start = candidate_start;
    }
    Some(start)
}

fn matches_general_newline_backwards(s: &str, ix: usize, unicode: bool) -> Option<usize> {
    if ix >= 2 && s.as_bytes().get(ix - 2..ix) == Some(b"\r\n") {
        return Some(ix - 2);
    }
    let (start, ch) = previous_char(s, ix)?;
    let is_newline = matches!(ch, '\n' | '\u{000B}' | '\u{000C}' | '\r')
        || (unicode && matches!(ch, '\u{0085}' | '\u{2028}' | '\u{2029}'));
    is_newline.then_some(start)
}

/// Helper function to store capture group positions from inner_slots into state.
/// This is used by both Delegate and BackwardsDelegate instructions.
#[inline]
fn store_capture_groups(
    state: &mut State,
    inner_slots: &[Option<NonMaxUsize>],
    range: CaptureGroupRange,
    skip_earlier_captures: bool,
) {
    let start_group = range.start();
    let end_group = range.end();
    for i in 0..(end_group - start_group) {
        let slot = (start_group + i) * 2;
        if let Some(start) = inner_slots[(i + 1) * 2] {
            let end = inner_slots[(i + 1) * 2 + 1].unwrap();

            let mut save = !skip_earlier_captures;
            if skip_earlier_captures {
                let existing_start = state.get(slot);
                let existing_end = state.get(slot + 1);
                save = (start.get() >= existing_start || existing_start == usize::MAX)
                    && (end.get() >= existing_end || existing_end == usize::MAX);
            }
            if save {
                state.save(slot, start.get());
                state.save(slot + 1, end.get());
            }
        }
    }
}

/// Run the program with trace printing for debugging.
pub fn run_trace(prog: &Prog, s: &str, pos: usize) -> Result<Option<Vec<usize>>> {
    run(
        prog,
        s,
        pos,
        OPTION_TRACE,
        &HardRegexRuntimeOptions::default(),
    )
}

/// Run the program with default options.
pub fn run_default(prog: &Prog, s: &str, pos: usize) -> Result<Option<Vec<usize>>> {
    run(prog, s, pos, 0, &HardRegexRuntimeOptions::default())
}

/// Run the program with options.
#[allow(clippy::cognitive_complexity)]
pub(crate) fn run(
    prog: &Prog,
    s: &str,
    pos: usize,
    option_flags: u32,
    options: &HardRegexRuntimeOptions,
) -> Result<Option<Vec<usize>>> {
    let max_stack = if prog.ecmascript_mode {
        ECMASCRIPT_MAX_STACK.min(options.backtrack_limit.saturating_add(1))
    } else {
        MAX_STACK
    };
    let mut state = State::new(prog.n_saves, max_stack, option_flags);
    let mut inner_slots: Vec<Option<NonMaxUsize>> = Vec::new();
    let look_matcher = LookMatcher::new();
    #[cfg(feature = "std")]
    if option_flags & OPTION_TRACE != 0 {
        println!("pos\tinstruction");
    }
    let mut work_count = 0usize;
    let mut pc = 0;
    let mut ix = pos;
    let mut match_attempt_start = pos;
    loop {
        // break from this loop to fail, causes stack to pop
        'fail: loop {
            #[cfg(feature = "std")]
            if option_flags & OPTION_TRACE != 0 {
                println!("{}\t{} {:?}", ix, pc, prog.body[pc]);
            }
            match prog.body[pc] {
                Insn::End => {
                    // save of end position into slot 1 is now done
                    // with an explicit group; we might want to
                    // optimize that.
                    //state.saves[1] = ix;
                    #[cfg(feature = "std")]
                    if option_flags & OPTION_TRACE != 0 {
                        println!("saves: {:?}", state.saves);
                    }
                    // Reject the match if it is empty and the flag to do so is enabled.
                    // `match_attempt_start` is set by `SplitUnanchored` each time the unanchored
                    // preamble begins a new match attempt at a fresh position, so this correctly
                    // rejects empty matches regardless of where in the haystack the attempt starts.
                    if option_flags & OPTION_FIND_NOT_EMPTY != 0 && ix == match_attempt_start {
                        break 'fail;
                    }
                    if let Some(&slot1) = state.saves.get(1) {
                        // With some features like keep out (\K), the match start can be after
                        // the match end. Cap the start to <= end.
                        if state.get(0) > slot1 {
                            state.save(0, slot1);
                        }
                    }
                    return Ok(Some(state.saves));
                }
                Insn::Any => {
                    if ix < s.len() {
                        ix += codepoint_len_at(s, ix);
                    } else {
                        break 'fail;
                    }
                }
                Insn::AnyBackwards => {
                    let Some((start, _)) = previous_char(s, ix) else {
                        break 'fail;
                    };
                    ix = start;
                }
                Insn::AnyNoNL => {
                    if ix < s.len() && s.as_bytes()[ix] != b'\n' {
                        ix += codepoint_len_at(s, ix);
                    } else {
                        break 'fail;
                    }
                }
                Insn::AnyNoNLBackwards => {
                    let Some((start, ch)) = previous_char(s, ix) else {
                        break 'fail;
                    };
                    if ch == '\n' {
                        break 'fail;
                    }
                    ix = start;
                }
                Insn::AnyNoCRLF => {
                    if ix < s.len() && s.as_bytes()[ix] != b'\r' && s.as_bytes()[ix] != b'\n' {
                        ix += codepoint_len_at(s, ix);
                    } else {
                        break 'fail;
                    }
                }
                Insn::AnyNoCRLFBackwards => {
                    let Some((start, ch)) = previous_char(s, ix) else {
                        break 'fail;
                    };
                    if matches!(ch, '\r' | '\n') {
                        break 'fail;
                    }
                    ix = start;
                }
                Insn::GeneralNewlineBackwards { unicode } => {
                    let Some(start) = matches_general_newline_backwards(s, ix, unicode) else {
                        break 'fail;
                    };
                    ix = start;
                }
                Insn::Lit(ref val) => {
                    let ix_end = ix + val.len();
                    if !matches_literal(s, ix, ix_end, val) {
                        break 'fail;
                    }
                    ix = ix_end
                }
                Insn::LitBackwards(ref val) => {
                    let Some(start) = matches_literal_backwards(s, ix, val) else {
                        break 'fail;
                    };
                    ix = start;
                }
                Insn::LitCaseiBackwards(ref val) => {
                    let Some(start) =
                        matches_literal_casei_backwards(s, ix, val, prog.unicode_casei)
                    else {
                        break 'fail;
                    };
                    ix = start;
                }
                Insn::Assertion(assertion) => {
                    if !match assertion {
                        Assertion::StartText => look_matcher.is_start(s.as_bytes(), ix),
                        Assertion::EndText => look_matcher.is_end(s.as_bytes(), ix),
                        Assertion::EndTextIgnoreTrailingNewlines { crlf } => {
                            let bytes = s.as_bytes();
                            if ix == bytes.len() {
                                // At the end of string
                                true
                            } else if crlf {
                                // In CRLF mode, trailing \r\n pairs and bare \n are ignored
                                bytes[ix..].iter().all(|&b| b == b'\n' || b == b'\r')
                            } else {
                                // Check if all remaining bytes are newlines
                                bytes[ix..].iter().all(|&b| b == b'\n')
                            }
                        }
                        Assertion::StartLine { crlf: false } => {
                            look_matcher.is_start_lf(s.as_bytes(), ix)
                        }
                        Assertion::StartLine { crlf: true } => {
                            look_matcher.is_start_crlf(s.as_bytes(), ix)
                        }
                        Assertion::EndLine { crlf: false } => {
                            look_matcher.is_end_lf(s.as_bytes(), ix)
                        }
                        Assertion::EndLine { crlf: true } => {
                            look_matcher.is_end_crlf(s.as_bytes(), ix)
                        }
                        Assertion::LeftWordBoundary => look_matcher
                            .is_word_start_unicode(s.as_bytes(), ix)
                            .unwrap(),
                        Assertion::RightWordBoundary => {
                            look_matcher.is_word_end_unicode(s.as_bytes(), ix).unwrap()
                        }
                        Assertion::LeftWordHalfBoundary => look_matcher
                            .is_word_start_half_unicode(s.as_bytes(), ix)
                            .unwrap(),
                        Assertion::RightWordHalfBoundary => look_matcher
                            .is_word_end_half_unicode(s.as_bytes(), ix)
                            .unwrap(),
                        Assertion::WordBoundary => {
                            look_matcher.is_word_unicode(s.as_bytes(), ix).unwrap()
                        }
                        Assertion::NotWordBoundary => look_matcher
                            .is_word_unicode_negate(s.as_bytes(), ix)
                            .unwrap(),
                        Assertion::EcmaWordBoundary => is_ecmascript_word_boundary(s, ix, false),
                        Assertion::EcmaNotWordBoundary => {
                            !is_ecmascript_word_boundary(s, ix, false)
                        }
                        Assertion::EcmaUnicodeIgnoreCaseWordBoundary => {
                            is_ecmascript_word_boundary(s, ix, true)
                        }
                        Assertion::EcmaUnicodeIgnoreCaseNotWordBoundary => {
                            !is_ecmascript_word_boundary(s, ix, true)
                        }
                    } {
                        break 'fail;
                    }
                }
                Insn::Split(x, y) => {
                    push_branch(
                        &mut state,
                        y,
                        ix,
                        &mut work_count,
                        options.backtrack_limit,
                        prog.ecmascript_mode,
                    )?;
                    pc = x;
                    continue;
                }
                Insn::SplitUnanchored(x, y) => {
                    match_attempt_start = ix;
                    if option_flags & OPTION_EXACT_POSITION != 0 {
                        pc = x;
                        continue;
                    }
                    push_branch(
                        &mut state,
                        y,
                        ix,
                        &mut work_count,
                        options.backtrack_limit,
                        false,
                    )?;
                    pc = x;
                    continue;
                }
                Insn::Jmp(target) => {
                    pc = target;
                    continue;
                }
                Insn::Save(slot) => state.save(slot, ix),
                Insn::Save0(slot) => state.save(slot, 0),
                Insn::SaveCaptureGroupStart(group) => {
                    let start_slot = group * 2;
                    // if the capture group's start slot is empty
                    // i.e. execution is not currently inside this capture group
                    // or the end slot for that capture group is complete
                    // then we save the current position in the capture group start slot
                    if state.get(start_slot) == usize::MAX || state.get(start_slot + 1) <= ix {
                        state.save(start_slot, ix);
                    }
                }
                Insn::ClearCaptureRange {
                    start_group,
                    end_group,
                } => {
                    let clear_work = end_group.saturating_sub(start_group).saturating_mul(2);
                    charge_work(&mut work_count, clear_work, options.backtrack_limit)?;
                    for group in start_group..end_group {
                        state.save(group * 2, usize::MAX);
                        state.save(group * 2 + 1, usize::MAX);
                    }
                }
                Insn::Restore(slot) => ix = state.get(slot),
                Insn::RepeatGr {
                    lo,
                    hi,
                    next,
                    repeat,
                } => {
                    let repcount = state.get(repeat);
                    if hi.reached(repcount) {
                        pc = next;
                        continue;
                    }
                    if prog.ecmascript_mode {
                        charge_work(&mut work_count, 1, options.backtrack_limit)?;
                    }
                    let next_count = repcount
                        .checked_add(1)
                        .ok_or(Error::RuntimeError(RuntimeError::BacktrackLimitExceeded))?;
                    state.save(repeat, next_count);
                    if lo.minimum_satisfied(repcount) {
                        push_branch(
                            &mut state,
                            next,
                            ix,
                            &mut work_count,
                            options.backtrack_limit,
                            prog.ecmascript_mode,
                        )?;
                    }
                }
                Insn::RepeatNg {
                    lo,
                    hi,
                    next,
                    repeat,
                } => {
                    let repcount = state.get(repeat);
                    if hi.reached(repcount) {
                        pc = next;
                        continue;
                    }
                    if prog.ecmascript_mode {
                        charge_work(&mut work_count, 1, options.backtrack_limit)?;
                    }
                    let next_count = repcount
                        .checked_add(1)
                        .ok_or(Error::RuntimeError(RuntimeError::BacktrackLimitExceeded))?;
                    state.save(repeat, next_count);
                    if lo.minimum_satisfied(repcount) {
                        push_branch(
                            &mut state,
                            pc + 1,
                            ix,
                            &mut work_count,
                            options.backtrack_limit,
                            prog.ecmascript_mode,
                        )?;
                        pc = next;
                        continue;
                    }
                }
                Insn::RepeatEpsilonGr {
                    lo,
                    hi,
                    next,
                    repeat,
                    check,
                    fail_on_empty,
                } => {
                    let repcount = state.get(repeat);
                    if repcount > 0 && state.get(check) == ix {
                        if fail_on_empty {
                            break 'fail;
                        }
                        pc = next;
                        continue;
                    }
                    if hi.reached(repcount) {
                        pc = next;
                        continue;
                    }
                    if prog.ecmascript_mode {
                        charge_work(&mut work_count, 1, options.backtrack_limit)?;
                    }
                    let next_count = repcount
                        .checked_add(1)
                        .ok_or(Error::RuntimeError(RuntimeError::BacktrackLimitExceeded))?;
                    state.save(repeat, next_count);
                    if lo.minimum_satisfied(repcount) {
                        state.save(check, ix);
                        push_branch(
                            &mut state,
                            next,
                            ix,
                            &mut work_count,
                            options.backtrack_limit,
                            prog.ecmascript_mode,
                        )?;
                    }
                }
                Insn::RepeatEpsilonNg {
                    lo,
                    hi,
                    next,
                    repeat,
                    check,
                    fail_on_empty,
                } => {
                    let repcount = state.get(repeat);
                    if repcount > 0 && state.get(check) == ix {
                        if fail_on_empty {
                            break 'fail;
                        }
                        pc = next;
                        continue;
                    }
                    if hi.reached(repcount) {
                        pc = next;
                        continue;
                    }
                    if prog.ecmascript_mode {
                        charge_work(&mut work_count, 1, options.backtrack_limit)?;
                    }
                    let next_count = repcount
                        .checked_add(1)
                        .ok_or(Error::RuntimeError(RuntimeError::BacktrackLimitExceeded))?;
                    state.save(repeat, next_count);
                    if lo.minimum_satisfied(repcount) {
                        state.save(check, ix);
                        push_branch(
                            &mut state,
                            pc + 1,
                            ix,
                            &mut work_count,
                            options.backtrack_limit,
                            prog.ecmascript_mode,
                        )?;
                        pc = next;
                        continue;
                    }
                }
                Insn::GoBack(count) => {
                    for _ in 0..count {
                        if ix == 0 {
                            break 'fail;
                        }
                        ix = prev_codepoint_ix(s, ix);
                    }
                }
                Insn::FailNegativeLookAround => {
                    // Reaching this instruction means that the body of the
                    // look-around matched. Because it's a *negative* look-around,
                    // that means the look-around itself should fail (not match).
                    // But before, we need to discard all the states that have
                    // been pushed with the look-around, because we don't want to
                    // explore them.
                    loop {
                        let (popped_pc, _) = state.pop();
                        if popped_pc == pc + 1 {
                            // We've reached the state that would jump us to
                            // after the look-around (in case the look-around
                            // succeeded). That means we popped enough states.
                            break;
                        }
                    }
                    break 'fail;
                }
                Insn::Backref { slot, casei } => {
                    let lo = state.get(slot);
                    if lo == usize::MAX {
                        if prog.ecmascript_mode {
                            pc += 1;
                            continue;
                        }
                        break 'fail;
                    }
                    let hi = state.get(slot + 1);
                    if hi == usize::MAX {
                        if prog.ecmascript_mode {
                            pc += 1;
                            continue;
                        }
                        break 'fail;
                    }
                    let ref_text = &s[lo..hi];
                    if casei {
                        let Some(ix_end) =
                            matches_literal_casei(s, ix, ref_text, prog.unicode_casei)
                        else {
                            break 'fail;
                        };
                        ix = ix_end;
                    } else {
                        let ix_end = ix + ref_text.len();
                        if !matches_literal(s, ix, ix_end, ref_text) {
                            break 'fail;
                        }
                        ix = ix_end;
                    }
                }
                Insn::BackrefBackwards { slot, casei } => {
                    let lo = state.get(slot);
                    let hi = state.get(slot + 1);
                    if lo != usize::MAX && hi != usize::MAX {
                        let ref_text = &s[lo..hi];
                        let start = if casei {
                            matches_literal_casei_backwards(s, ix, ref_text, prog.unicode_casei)
                        } else {
                            matches_literal_backwards(s, ix, ref_text)
                        };
                        let Some(start) = start else {
                            break 'fail;
                        };
                        ix = start;
                    }
                }
                Insn::BackrefSet { set_id, casei } => {
                    let Some(groups) = prog.ecmascript_backref_sets.get(set_id) else {
                        break 'fail;
                    };
                    let mut selected = None;
                    for group in groups {
                        let lo = state.get(group * 2);
                        let hi = state.get(group * 2 + 1);
                        if lo != usize::MAX
                            && hi != usize::MAX
                            && selected.replace((lo, hi)).is_some()
                        {
                            // RuJa's structural early error guarantees mutual
                            // exclusion. Fail closed if an invalid internal
                            // pattern violates that invariant.
                            break 'fail;
                        }
                    }
                    if let Some((lo, hi)) = selected {
                        let ref_text = &s[lo..hi];
                        if casei {
                            let Some(ix_end) =
                                matches_literal_casei(s, ix, ref_text, prog.unicode_casei)
                            else {
                                break 'fail;
                            };
                            ix = ix_end;
                        } else {
                            let ix_end = ix + ref_text.len();
                            if !matches_literal(s, ix, ix_end, ref_text) {
                                break 'fail;
                            }
                            ix = ix_end;
                        }
                    }
                }
                Insn::BackrefSetBackwards { set_id, casei } => {
                    let Some(groups) = prog.ecmascript_backref_sets.get(set_id) else {
                        break 'fail;
                    };
                    let mut selected = None;
                    for group in groups {
                        let lo = state.get(group * 2);
                        let hi = state.get(group * 2 + 1);
                        if lo != usize::MAX
                            && hi != usize::MAX
                            && selected.replace((lo, hi)).is_some()
                        {
                            break 'fail;
                        }
                    }
                    if let Some((lo, hi)) = selected {
                        let ref_text = &s[lo..hi];
                        let start = if casei {
                            matches_literal_casei_backwards(s, ix, ref_text, prog.unicode_casei)
                        } else {
                            matches_literal_backwards(s, ix, ref_text)
                        };
                        let Some(start) = start else {
                            break 'fail;
                        };
                        ix = start;
                    }
                }
                Insn::BackrefExistsCondition(group) => {
                    let lo = state.get(group * 2);
                    if lo == usize::MAX {
                        // Referenced group hasn't matched, so the backref doesn't match either
                        break 'fail;
                    }
                }
                Insn::Fail => {
                    // Immediately fail and trigger backtracking
                    break 'fail;
                }
                #[cfg(feature = "variable-lookbehinds")]
                Insn::BackwardsDelegate(ReverseBackwardsDelegate {
                    ref dfa,
                    ref cache_pool,
                    pattern: _,
                    ref capture_group_extraction_inner,
                    capture_groups,
                }) => {
                    // Use regex-automata to search backwards from current position
                    let mut cache_guard = cache_pool.get();
                    let input = Input::new(s).anchored(Anchored::Yes).range(0..ix);

                    match dfa.try_search_rev(&mut cache_guard, &input) {
                        Ok(Some(match_result)) => {
                            // Update ix to the start position of the match
                            let match_start = match_result.offset();

                            if let Some(inner) = capture_group_extraction_inner {
                                if let Some(range) = capture_groups {
                                    // There are capture groups, need to search forward to populate them
                                    let forward_input =
                                        Input::new(s).span(match_start..ix).anchored(Anchored::Yes);
                                    inner_slots.resize((range.end() - range.start() + 1) * 2, None);

                                    if inner
                                        .search_slots(&forward_input, &mut inner_slots)
                                        .is_some()
                                    {
                                        // Store capture group positions, ignoring any whose range is earlier than what has been stored already
                                        store_capture_groups(&mut state, &inner_slots, range, true);
                                    } else {
                                        break 'fail;
                                    }
                                } else {
                                    // No groups, just update ix to the match start
                                    ix = match_start;
                                }
                            } else {
                                // No groups, just update ix to the match start
                                ix = match_start;
                            }
                        }
                        _ => break 'fail,
                    }
                }
                Insn::BeginAtomic => {
                    let count = state.backtrack_count();
                    state.stack_push(count);
                }
                Insn::EndAtomic => {
                    let count = state.stack_pop();
                    state.backtrack_cut(count);
                }
                Insn::Delegate(Delegate {
                    ref inner,
                    pattern: _,
                    capture_groups,
                }) => {
                    let input = Input::new(s).span(ix..s.len()).anchored(Anchored::Yes);
                    if let Some(range) = capture_groups {
                        // Has capture groups, need to extract them
                        inner_slots.resize((range.end() - range.start() + 1) * 2, None);
                        if inner.search_slots(&input, &mut inner_slots).is_some() {
                            // store the capture groups, no need to check current state to see if new values are further to the right
                            store_capture_groups(&mut state, &inner_slots, range, false);
                            ix = inner_slots[1].unwrap().get();
                        } else {
                            break 'fail;
                        }
                    } else {
                        // No groups, so we can use faster methods
                        match inner.search_half(&input) {
                            Some(m) => ix = m.offset(),
                            _ => break 'fail,
                        }
                    }
                }
                Insn::DelegateBackwards(Delegate {
                    ref inner,
                    pattern: _,
                    capture_groups,
                }) => {
                    let Some((start, _)) = previous_char(s, ix) else {
                        break 'fail;
                    };
                    let input = Input::new(s).span(start..ix).anchored(Anchored::Yes);
                    if let Some(range) = capture_groups {
                        inner_slots.resize((range.end() - range.start() + 1) * 2, None);
                        if inner.search_slots(&input, &mut inner_slots).is_none()
                            || inner_slots[1].map(NonMaxUsize::get) != Some(ix)
                        {
                            break 'fail;
                        }
                        store_capture_groups(&mut state, &inner_slots, range, false);
                    } else if inner.search_half(&input).map(|m| m.offset()) != Some(ix) {
                        break 'fail;
                    }
                    ix = start;
                }
                Insn::AbsentRepeater(ref delegate) => {
                    // The absent operator matches the shortest string not containing the delegate pattern
                    // We advance one character at a time, checking if delegate matches at each position
                    // If delegate matches, we've found the boundary and continue to next instruction
                    // If we reach end of string without delegate matching, we also continue

                    // Check if delegate matches at current position
                    let input = Input::new(s).span(ix..s.len()).anchored(Anchored::Yes);
                    // capture groups in the delegate are always ignored, so we can use the quicker search_half method
                    let delegate_matches_here = delegate.inner.search_half(&input).is_some();

                    if delegate_matches_here {
                        // Delegate matches at current position - we've reached the boundary
                        // Continue to next instruction without consuming any characters
                        // Fall through via pc += 1 below
                    } else if ix < s.len() {
                        // Try advancing one character and checking again
                        push_branch(
                            &mut state,
                            pc + 1,
                            ix,
                            &mut work_count,
                            options.backtrack_limit,
                            prog.ecmascript_mode,
                        )?;
                        ix += codepoint_len_at(s, ix);
                        // Stay at same pc to check delegate match at new position
                        continue;
                    } else {
                        // Reached end of string - delegate never matched, so we succeed
                        // Fall through via pc += 1 below
                    }
                }
                Insn::ContinueFromPreviousMatchEnd { at_start } => {
                    if ix > pos || option_flags & OPTION_SKIPPED_EMPTY_MATCH != 0 {
                        // If \G is at the start of the pattern, we can fail early
                        // instead of checking at each position in the haystack
                        // because \G will never match at any other position
                        if at_start && state.stack.len() == 1 {
                            // The only item on the stack is from the SplitUnanchored instruction for non-anchored search
                            // We can safely return None immediately
                            return Ok(None);
                        }
                        break 'fail;
                    }
                }
            }
            pc += 1;
        }
        #[cfg(feature = "std")]
        if option_flags & OPTION_TRACE != 0 {
            println!("fail");
        }
        // "break 'fail" goes here
        if state.stack.is_empty() {
            return Ok(None);
        }

        if !prog.ecmascript_mode {
            charge_work(&mut work_count, 1, options.backtrack_limit)?;
        }
        let (newpc, newix) = state.pop();
        pc = newpc;
        ix = newix;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::{quickcheck, Arbitrary, Gen};

    #[test]
    fn state_push_pop() {
        let mut state = State::new(1, MAX_STACK, 0);

        state.push(0, 0).unwrap();
        state.push(1, 1).unwrap();
        assert_eq!(state.pop(), (1, 1));
        assert_eq!(state.pop(), (0, 0));
        assert!(state.stack.is_empty());

        state.push(2, 2).unwrap();
        assert_eq!(state.pop(), (2, 2));
        assert!(state.stack.is_empty());
    }

    #[test]
    fn state_save_override() {
        let mut state = State::new(1, MAX_STACK, 0);
        state.save(0, 10);
        state.push(0, 0).unwrap();
        state.save(0, 20);
        assert_eq!(state.pop(), (0, 0));
        assert_eq!(state.get(0), 10);
    }

    #[test]
    fn state_save_override_twice() {
        let mut state = State::new(1, MAX_STACK, 0);
        state.save(0, 10);
        state.push(0, 0).unwrap();
        state.save(0, 20);
        state.push(1, 1).unwrap();
        state.save(0, 30);

        assert_eq!(state.get(0), 30);
        assert_eq!(state.pop(), (1, 1));
        assert_eq!(state.get(0), 20);
        assert_eq!(state.pop(), (0, 0));
        assert_eq!(state.get(0), 10);
    }

    #[test]
    fn state_explicit_stack() {
        let mut state = State::new(1, MAX_STACK, 0);
        state.stack_push(11);
        state.stack_push(12);

        state.push(100, 101).unwrap();
        state.stack_push(13);
        assert_eq!(state.stack_pop(), 13);
        state.stack_push(14);
        assert_eq!(state.pop(), (100, 101));

        // Note: 14 is not there because it was pushed as part of the backtrack branch
        assert_eq!(state.stack_pop(), 12);
        assert_eq!(state.stack_pop(), 11);
    }

    #[test]
    fn state_backtrack_cut_simple() {
        let mut state = State::new(2, MAX_STACK, 0);
        state.save(0, 1);
        state.save(1, 2);

        let count = state.backtrack_count();

        state.push(0, 0).unwrap();
        state.save(0, 3);
        assert_eq!(state.backtrack_count(), 1);

        state.backtrack_cut(count);
        assert_eq!(state.backtrack_count(), 0);
        assert_eq!(state.get(0), 3);
        assert_eq!(state.get(1), 2);
    }

    #[test]
    fn state_backtrack_cut_complex() {
        let mut state = State::new(2, MAX_STACK, 0);
        state.save(0, 1);
        state.save(1, 2);

        state.push(0, 0).unwrap();
        state.save(0, 3);

        let count = state.backtrack_count();

        state.push(1, 1).unwrap();
        state.save(0, 4);
        state.push(2, 2).unwrap();
        state.save(1, 5);
        assert_eq!(state.backtrack_count(), 3);

        state.backtrack_cut(count);
        assert_eq!(state.backtrack_count(), 1);
        assert_eq!(state.get(0), 4);
        assert_eq!(state.get(1), 5);

        state.pop();
        assert_eq!(state.backtrack_count(), 0);
        // Check that oldsave were set correctly
        assert_eq!(state.get(0), 1);
        assert_eq!(state.get(1), 2);
    }

    #[derive(Clone, Debug)]
    enum Operation {
        Push,
        Pop,
        Save(usize, usize),
    }

    impl Arbitrary for Operation {
        fn arbitrary(g: &mut Gen) -> Self {
            match g.choose(&[0, 1, 2]) {
                Some(0) => Operation::Push,
                Some(1) => Operation::Pop,
                _ => Operation::Save(
                    *g.choose(&[0usize, 1, 2, 3, 4]).unwrap(),
                    usize::arbitrary(g),
                ),
            }
        }
    }

    fn check_saves_for_operations(operations: Vec<Operation>) -> bool {
        let slots = operations
            .iter()
            .map(|o| match o {
                &Operation::Save(slot, _) => slot + 1,
                _ => 0,
            })
            .max()
            .unwrap_or(0);
        if slots == 0 {
            // No point checking if there's no save instructions
            return true;
        }

        // Stack with the complete VM state (including saves)
        let mut stack = Vec::new();
        let mut saves = vec![usize::MAX; slots];

        let mut state = State::new(slots, MAX_STACK, 0);

        let mut expected = Vec::new();
        let mut actual = Vec::new();

        for operation in operations {
            match operation {
                Operation::Push => {
                    // We're not checking pc and ix later, so don't bother
                    // putting in random values.
                    stack.push((0, 0, saves.clone()));
                    state.push(0, 0).unwrap();
                }
                Operation::Pop => {
                    // Note that because we generate the operations randomly
                    // there might be more pops than pushes. So ignore a pop
                    // if the stack was empty.
                    if let Some((_, _, previous_saves)) = stack.pop() {
                        saves = previous_saves;
                        state.pop();
                    }
                }
                Operation::Save(slot, value) => {
                    saves[slot] = value;
                    state.save(slot, value);
                }
            }

            // Remember state of saves for checking later
            expected.push(saves.clone());
            let mut actual_saves = vec![usize::MAX; slots];
            for (i, item) in actual_saves.iter_mut().enumerate().take(slots) {
                *item = state.get(i);
            }
            actual.push(actual_saves);
        }

        expected == actual
    }

    quickcheck! {
        fn state_save_quickcheck(operations: Vec<Operation>) -> bool {
            check_saves_for_operations(operations)
        }
    }
}
