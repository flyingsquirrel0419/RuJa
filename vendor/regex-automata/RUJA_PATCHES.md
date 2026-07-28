# RuJa regex-automata patches

This directory vendors `regex-automata` 0.4.14 under its upstream licenses.

RuJa carries two hidden features used only by Fancy's ECMAScript mode:

1. Dedicated positive/negative ECMAScript Unicode-ignore-case look states use
   ASCII alphanumeric or underscore plus long s and Kelvin sign. Syntax
   configuration forwards the custom HIR variants. Determinization may use the
   ASCII-equivalent transition on ASCII units; non-ASCII cases stay on PikeVM.
2. Thompson compilation can prepend `CaptureClear` states for every descendant
   capture before a repeated iteration. PikeVM updates slots transactionally
   and restores them while exploring alternatives. Fancy disables DFA, hybrid,
   one-pass, and bounded-backtracker strategies under this mode so PikeVM owns
   the capture result.

Post-match cleanup cannot recover capture state observed by a later
backreference or distinguish the final participating alternative. The NFA
state keeps regular non-nullable repeated captures linear; nullable repeats
remain in Fancy's backtracking `RepeatMatcher` because Pike priority alone does
not implement that ECMAScript choice.

When updating, reapply custom looks, syntax forwarding, `CaptureClear`, and the
meta configuration hook. Run `cargo test --manifest-path
vendor/regex-automata/Cargo.toml --lib --all-features`; the registry package
omits auxiliary integration-test sources referenced by its test harness.
