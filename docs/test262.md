# test262 conformance

RuJa runs a subset of the [test262](https://github.com/tc39/test262)
conformance suite via `tools/test262_runner.py`. This is **not** a full
conformance harness — it skips tests requiring unsupported features
(modules, TypedArrays, Atomics, Intl, etc.) and uses a minimal `assert`
stub rather than the full test262 harness.

## Running

```sh
# Clone test262 (shallow, sparse checkout keeps it small):
git clone --depth 1 --filter=blob:none --sparse https://github.com/tc39/test262.git
cd test262 && git sparse-checkout set harness test/language

# Run a subset:
TEST262=/path/to/test262 python3 tools/test262_runner.py language/expressions
```

## Current results

Measured on the `language/expressions` subset (the most exercisable for a
bytecode VM):

| Suite            | Total | Ran  | Pass | Fail | Pass rate |
|------------------|-------|------|------|------|----------|
| expressions      | 11101 | 8736 | 2476 | 6260 | 28.3%    |

The pass rate is dominated by spec features RuJa deliberately does not yet
implement (computed class names, optional chaining edge cases, tagged
template full semantics, etc.) rather than by correctness bugs in what it
does implement. Improving the rate is an ongoing goal; the runner makes
regressions visible.

## Why the rate is what it is

RuJa targets a pragmatic ES5.1 + selected ES2015+ subset, not full ES2024
conformance. The runner's `SKIP_FEATURES` set excludes whole feature
areas (modules, ArrayBuffer/TypedArray/DataView, Atomics, Intl, etc.),
but individual tests for *partially* supported features still run and may
fail on edge cases. As coverage expands, the pass rate rises and the gap
narrowing becomes measurable.
