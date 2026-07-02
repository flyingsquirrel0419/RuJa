# test262 conformance

RuJa runs the [test262](https://github.com/tc39/test262) conformance suite
via `tools/test262_runner.py`. The runner uses the **real test262 harness**
(`sta.js`, `assert.js`, and per-test `includes:` such as `propertyHelper.js`
and `compareArray.js`) rather than a hand-rolled stub, so tests relying on
`verifyProperty`, `compareArray`, etc. are exercised correctly. It also
parses `negative:` metadata so a test that expects a `SyntaxError`/
`TypeError` (parse or runtime phase) passes when RuJa raises the matching
error. The full suite is run in CI via a parallel matrix workflow
(`.github/workflows/test262-full.yml`); `intl402` and `staging` are
excluded.

RuJa does **not** claim full ES conformance. Instead, it targets a
deliberately scoped subset of ES5.1 + selected ES2015+ features (see
[Supported Subset](#supported-subset) below). Tests requiring unsupported
features (modules, TypedArrays, Atomics, Intl, etc.) are skipped via the
runner's `SKIP_FEATURES` set.

## Supported subset

RuJa targets conformance within a declared feature subset rather than
chasing the full test262 pass rate. The supported subset is:

**ES5.1 core**: var/let/const, all operators, control flow (if/else,
while, do-while, for, for-in, for-of, switch, break/continue, labeled
statements), functions, closures, try/catch/finally, throw, strict mode,
the full standard library (Array, String, Number, Math, Object, JSON,
Date, RegExp, Error hierarchy).

**Selected ES2015+**: arrow functions, classes/extends/super, default &
rest parameters, destructuring (array/object/nested), template literals,
tagged templates, computed property keys, object spread/rest, getters/
setters, Symbol.iterator, Map/Set/WeakMap/WeakSet, BigInt, Proxy,
Reflect, Promise, async/await, generators, for-of, optional chaining,
nullish coalescing, logical assignment.

**Intentionally unsupported**: ES Modules (import/export), Intl, Atomics,
SharedArrayBuffer, TypedArray (Uint8Array is partially supported),
WeakRef/FinalizationRegistry, Tail-call optimization.

Conformance within this subset is the goal — not the raw test262
percentage. The full-suite number includes thousands of tests for
features outside the supported subset, which naturally pulls the overall
rate down.

## Running

```sh
# Clone test262 (shallow, sparse checkout keeps it small):
git clone --depth 1 --filter=blob:none --sparse https://github.com/tc39/test262.git
cd test262 && git sparse-checkout set harness test/language

# Build a release binary (the runner expects target/release/ruja):
cargo build --release

# Run one or more subtrees:
TEST262=/path/to/test262 python3 tools/test262_runner.py language/identifiers language/keywords
```

For failure-bucket analysis with error samples, use the sibling analyzer:

```sh
python3 tools/test262_analyze.py language/expressions/arrow-function
```

## Subset results (0.4.0-alpha)

Measured locally against `test/language/` with `SKIP_FEATURES` applied.
These are the areas RuJa actively targets:

| Suite | Ran | Pass | Fail | Pass rate |
|-------|-----|------|------|-----------|
| identifiers + keywords + types + comments + whitespace + punctuators | 436 | 335 | 101 | 76.8% |
| expressions/arrow-function + function + object | 428 | 238 | 190 | 55.6% |
| expressions (all) | 2,745 | 1,639 | 1,106 | 59.7% |
| statements (all) | 1,415 | 608 | 807 | 43.0% |

**Subset aggregate**: 5,024 tests ran, 2,820 passed (~56%). The
identifiers/keywords/types/comments/whitespace/punctuators group is
near-spec-complete (76.8%). Expressions are at ~60%. Statements lag at
43% due to class-related and generator-related test paths that are
partially supported.

(Numbers move as bugs are fixed; the CI job summary is the source of
truth for the current commit.)

## Full-suite results (CI)

The `test262-full` CI workflow runs the entire test262 tree (excluding
`intl402`/`staging`) in parallel. Baseline from the first full run:

| Metric | Count |
|--------|-------|
| Total tests | 76,397 |
| Actually run | 60,178 |
| Pass | 19,987 |
| Fail | 40,191 |
| Skip | 15,481 |
| **Pass rate (of run)** | **33.2%** |

Longest jobs: `language/expressions` (~22 min), `language/statements`
(~25 min). Numbers move as bugs are fixed; the CI job summary is the
source of truth for the current commit. The full-suite number includes
tests for features outside the supported subset; see [Supported Subset](#supported-subset).

## What was fixed to get here

A round of test262-driven bug fixes raised the subset pass rate
substantially from the prior ~20% baseline:

- **Lexer: Unicode identifiers.** `IdentifierStart`/`IdentifierContinue`
  now accept Unicode letters (not just ASCII) and the `\uXXXX` /
  `\u{XXXX}` escape forms inside identifiers, so `\u{63}ase` parses as the
  keyword `case` and `café`/`π`/CJK names lex correctly. Stray non-id
  Unicode bytes and invalid escapes advance the cursor instead of
  looping forever. NEL/LS/PS are recognized as line terminators.
- **Parser: destructuring parameters.** Arrow functions and ordinary
  functions now accept destructuring parameters (`([a, b]) =>`, `function
  f({x, y})`), including nested patterns and defaults
  (`[[x, y, z] = [4, 5, 6]]) =>`). Each destructuring param is bound from
  a synthesized positional temp via a `let <pattern> = __argN;` prelude.
- **Parser: object-literal methods.** Generator methods (`*foo() {}`)
  and async methods (`async foo() {}`, `async *foo() {}`) now parse, and
  reserved words (`return`, `class`, `default`, ...) are accepted as
  property keys.
- **Harness: negative tests.** `negative: { phase, type }` metadata is
  honored, and the runner executes via a temp file instead of `-e` argv
  so long sources and non-ASCII survive intact.

## Why the rate is not higher

RuJa targets a pragmatic ES5.1 + selected ES2015+ subset, not full ES2024
conformance. The remaining failures cluster around a few areas: iterator
protocol edge cases in destructuring, `$DONOTEVALUATE`-style negative
parse tests for reserved words (`enum`/`export`/`import` as identifiers),
WeakRef/TypedArray/Intl features that are skipped entirely, and a long
tail of property-descriptor checks (`verifyProperty`) for builtin
attributes. Improving the rate is an ongoing goal; the runner makes
regressions visible on every push.

## Running

```sh
# Clone test262 (shallow, sparse checkout keeps it small):
git clone --depth 1 --filter=blob:none --sparse https://github.com/tc39/test262.git
cd test262 && git sparse-checkout set harness test/language

# Build a release binary (the runner expects target/release/ruja):
cargo build --release

# Run one or more subtrees:
TEST262=/path/to/test262 python3 tools/test262_runner.py language/identifiers language/keywords
```

For failure-bucket analysis with error samples, use the sibling analyzer:

```sh
python3 tools/test262_analyze.py language/expressions/arrow-function
```

## CI subset results

Measured on a representative subset of `language/` (arrow-function,
function, object, identifiers, keywords, types, comments, white-space,
punctuators). The subset is what the `ci.yml` job runs, so the number in
the job summary matches what is below.

| Suite            | Ran  | Pass | Fail | Pass rate |
|------------------|------|------|------|----------|
| identifiers      | 266  | 159  | 107  | 59.8%    |
| punctuators       | 11   | 11   | 0    | 100.0%   |
| white-space       | 67   | 49   | 18   | 73.1%    |
| keywords          | 25   | 24   | 1    | 96.0%    |
| types             | 113  | 80   | 33   | 70.8%    |
| comments          | 23   | 17   | 6    | 73.9%    |
| expressions/arrow-function | 343 | 245 | 98  | 71.4%    |
| expressions/function        | 264 | 159 | 105 | 60.2%    |
| expressions/object          | 946 | 506 | 440 | 53.5%    |
| **subset total**  | 2058 | 1250 | 808 | ~60.8%    |

(Numbers move as bugs are fixed; the CI job summary is the source of truth
for the current commit.)

## Full-suite results (CI)

The `test262-full` CI workflow runs the entire test262 tree (excluding
`intl402`/`staging`) in parallel. Baseline from the first full run:

| Metric | Count |
|--------|-------|
| Total tests | 76,397 |
| Actually run | 60,178 |
| Pass | 19,987 |
| Fail | 40,191 |
| Skip | 15,481 |
| **Pass rate (of run)** | **33.2%** |

Longest jobs: `language/expressions` (~22 min), `language/statements`
(~25 min). Numbers move as bugs are fixed; the CI job summary is the
source of truth for the current commit. RuJa does not assert ES
conformance; this only tracks regressions.

## What was fixed to get here

A round of test262-driven bug fixes raised the subset pass rate
substantially from the prior ~20% baseline:

- **Lexer: Unicode identifiers.** `IdentifierStart`/`IdentifierContinue`
  now accept Unicode letters (not just ASCII) and the `\uXXXX` /
  `\u{XXXX}` escape forms inside identifiers, so `\u{63}ase` parses as the
  keyword `case` and `café`/`π`/CJK names lex correctly. Stray non-id
  Unicode bytes and invalid escapes advance the cursor instead of
  looping forever. NEL/LS/PS are recognized as line terminators.
- **Parser: destructuring parameters.** Arrow functions and ordinary
  functions now accept destructuring parameters (`([a, b]) =>`, `function
  f({x, y})`), including nested patterns and defaults
  (`[[x, y, z] = [4, 5, 6]]) =>`). Each destructuring param is bound from
  a synthesized positional temp via a `let <pattern> = __argN;` prelude.
- **Parser: object-literal methods.** Generator methods (`*foo() {}`)
  and async methods (`async foo() {}`, `async *foo() {}`) now parse, and
  reserved words (`return`, `class`, `default`, ...) are accepted as
  property keys.
- **Harness: negative tests.** `negative: { phase, type }` metadata is
  honored, and the runner executes via a temp file instead of `-e` argv
  so long sources and non-ASCII survive intact.

## Why the rate is not higher

RuJa targets a pragmatic ES5.1 + selected ES2015+ subset, not full ES2024
conformance. The remaining failures cluster around a few areas: iterator
protocol edge cases in destructuring, `$DONOTEVALUATE`-style negative
parse tests for reserved words (`enum`/`export`/`import` as identifiers),
WeakRef/TypedArray/Intl features that are skipped entirely, and a long
tail of property-descriptor checks (`verifyProperty`) for builtin
attributes. Improving the rate is an ongoing goal; the runner makes
regressions visible on every push.
