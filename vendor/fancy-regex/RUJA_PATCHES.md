# RuJa fancy-regex patches

This directory vendors `fancy-regex` 0.18.0 (upstream revision
`895301dadc30cbf466ba845c063a4158619a09b5`) under its MIT license.

RuJa carries three backend changes behind `RegexBuilder::ecmascript_mode(true)`:

1. A repeated expression clears all descendant capture start/end slots before
   each iteration. The VM writes `usize::MAX` through its copy-on-write
   `State::save` path, so an exit or backtracking branch restores the captures
   from the correct completed iteration. Current-delta membership uses a
   bitset instead of scanning prior saves, and each cleared slot is charged to
   the existing runtime work limit.
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

The mode is disabled by default. This preserves upstream Oniguruma behavior
and keeps the internal atom unavailable to ordinary `fancy-regex` callers.
RuJa additionally rejects the atom's spelling during ECMAScript source
validation; only named-backreference lowering can produce it.

Three upstream trailing/terminal whitespace instances are removed so the
vendored tree satisfies RuJa's repository-wide `git diff --check` gate; these
edits do not change source or test behavior.

Post-match capture cleanup cannot replace the first patch because a
backreference can observe stale capture state while a later quantified
iteration is still matching. Expanding a duplicate named reference into a
nested conditional per alias cannot replace the second patch without
quadratic generated source and the same stale-capture dependency.

When updating the vendored crate:

1. Copy the new upstream release and retain its license and provenance files.
2. Reapply the mode, ID-based `BackrefSet`, case mode, work accounting, and
   backtracking-aware repeat clearing.
3. Run `cargo test --manifest-path vendor/fancy-regex/Cargo.toml --all-features`.
4. Run RuJa's complete Rust, Test262, formatting, Clippy, release, and wasm32
   gates before changing the path dependency version.
