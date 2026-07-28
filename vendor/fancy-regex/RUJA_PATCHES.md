# RuJa fancy-regex patches

This directory vendors `fancy-regex` 0.18.0 (upstream revision
`895301dadc30cbf466ba845c063a4158619a09b5`) under its MIT license.

RuJa carries eight backend changes behind `RegexBuilder::ecmascript_mode(true)`:

1. A nullable repeated expression clears all descendant capture start/end
   slots before each hard-VM iteration. Non-nullable repeated captures stay on
   the regular delegated route, where RuJa's `regex-automata` fork emits
   transactional PikeVM `CaptureClear` states.
2. The internal `(?@N)` atom resolves `N` through a builder-supplied capture
   set and compiles to `BackrefSet`. It consumes the sole populated capture
   from a statically mutually-exclusive set, or succeeds without consuming
   input when every member is unset. The table stores each set once so repeated
   named references cannot expand the normalized pattern quadratically; AST
   and VM instructions retain only the set ID.
3. Case-insensitive backreferences compare the same number of Unicode scalar
   values rather than the same number of UTF-8 bytes. RuJa selects Unicode
   simple folding for `u`/`v` and the ECMAScript legacy uppercase relation
   otherwise.
4. Assertions use directional compilation. Lookahead runs forward and
   lookbehind runs backward, including reversed concatenation, end-before-start
   capture saves, and backward literal, wildcard, newline, delegate,
   `Backref`, and `BackrefSet` instructions. Positive assertions are atomic,
   restore the outer cursor, and retain captures; negative assertions roll
   transactional state back. Unmatched backreferences match empty only in
   ECMAScript mode.
5. The parser accepts quantified lookahead only for ECMAScript legacy
   non-Unicode patterns. Nullable repeat instructions retain finite upper
   bounds and can reject an empty iteration so child alternatives backtrack as
   required by Annex B `RepeatMatcher`. ECMAScript mode also retains trailing
   positive lookahead instead of applying an optimizer that can discard local
   flag normalization.
6. ECMAScript execution charges speculative branch pushes, attempted repeat
   iterations, and hard-VM capture clears to one work budget, including
   successful zero-width paths. Deterministic unanchored scanning is not
   charged. A terminal bound or no-progress check does not consume another
   iteration. ECMAScript hard execution uses a 100,000-entry branch stack cap;
   mode-off retains the upstream one-million-entry cap.
7. Braced repeat bounds use an exact host-independent finite representation,
   with infinity kept separate. Static size arithmetic saturates, and the
   hidden `ecmascript_non_delegated_repeats` option marks every repeat subtree
   for one-body counter compilation. Values unreachable by the host counter
   remain finite in the AST and consume the existing bounded work budget at
   runtime. ECMAScript mode also accepts quantified empty groups, rejects
   recognized `min > max` ranges, and keeps lower-bound-less legacy braces as
   literals; mode-off parser behavior remains unchanged.
8. ECMAScript boundary escape spellings lower to native ASCII and Unicode-i
   assertion variants. Regular delegates receive the same ECMAScript syntax
   options, and `find_at_pos`/`captures_at_pos` set an exact-position VM option
   while assertions still observe the full haystack. No second program is
   compiled or retained.

The mode is disabled by default. This preserves upstream Oniguruma behavior,
lookaround optimization, unmatched-backreference behavior, work accounting,
and stack limits, and keeps the internal atom unavailable to ordinary
`fancy-regex` callers. Mode-off repeat parsing retains its existing `usize`
acceptance boundary; exact arbitrary-width parsing, range checks, empty-group
handling, and legacy-brace rules are ECMAScript-gated. RuJa additionally
rejects the internal atom's spelling during ECMAScript source validation;
only named-backreference lowering can produce it. A separate internal parser
flag records whether ECMAScript Unicode mode is active so the Annex B
quantifier exception cannot leak into `u`/`v`.

Three upstream trailing/terminal whitespace instances are removed so the
vendored tree satisfies RuJa's repository-wide `git diff --check` gate; these
edits do not change source or test behavior.

Post-match capture cleanup cannot replace the first patch because a
backreference can observe stale capture state while a later quantified
iteration is still matching. Expanding a duplicate named reference into a
nested conditional per alias cannot replace the second patch without
quadratic generated source and the same stale-capture dependency. Enumerating
candidate lookbehind prefixes cannot replace directional compilation because
it changes backward greediness and makes work scale with the candidate input
range. Failed-backtrack counting alone cannot bound successful zero-width
repeats or branch-stack growth.

When updating the vendored crate:

1. Copy the new upstream release and retain its license and provenance files.
2. Reapply the mode, ID-based `BackrefSet`, case mode, directional assertions,
   legacy quantified-lookahead parsing, exact repeat bounds, non-delegated
   counter routing, work accounting, native ECMAScript boundaries,
   exact-position APIs, and both hard-VM and PikeVM repeat clearing.
3. Run `cargo test --manifest-path vendor/fancy-regex/Cargo.toml --all-features`.
4. Run RuJa's complete Rust, Test262, formatting, Clippy, release, and wasm32
   gates before changing the path dependency version.
