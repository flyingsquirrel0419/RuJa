# RuJa regex-syntax patches

This directory vendors `regex-syntax` 0.8.11 under its upstream licenses.

RuJa adds two hidden, opt-in HIR look assertions for ECMAScript Unicode
ignore-case word boundary and its negation. `ParserBuilder` forwards the mode
to the translator, which maps `\b` and `\B` to those variants. Look sets,
reversal, serialization, debug printing, and exhaustive conversions preserve
the variants. Default parsing remains upstream-compatible.

The HIR printer renders the custom variants as ordinary `\b`/`\B`; reparsing
must use the same opt-in translator setting to recover their meaning. No
equivalent assertion exists in ordinary Rust regex concrete syntax, and RuJa
does not expose these internal HIR values as a source round-trip API.

The dedicated HIR form is required because Rust Unicode word characters are a
strict superset of ECMA-262 WordCharacters. Source rewriting cannot preserve a
zero-width assertion efficiently across every backend.

When updating, reapply the two look variants and translator option, then run
the crate's library tests and doctests plus RuJa's complete gates.
