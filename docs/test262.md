# test262 conformance

RuJa runs a subset of the [test262](https://github.com/tc39/test262)
conformance suite via `tools/test262_runner.py`. The runner uses the **real
test262 harness** (`sta.js`, `assert.js`, and per-test `includes:` such as
`propertyHelper.js` and `compareArray.js`) rather than a hand-rolled stub,
so tests relying on `verifyProperty`, `compareArray`, etc. are exercised
correctly. It also parses `negative:` metadata so a test that expects a
`SyntaxError`/`TypeError` (parse or runtime phase) passes when RuJa raises
the matching error.

Tests requiring unsupported features (modules, TypedArrays, Atomics, Intl,
etc.) are skipped via the runner's `SKIP_FEATURES` set; async-function and
module tests are also skipped.

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

## Current results

Measured on a representative subset of `language/` (arrow-function,
function, object, identifiers, keywords, types, comments, white-space,
punctuators). The subset is what the CI job runs, so the number in the job
summary matches what is below.

| Suite            | Ran  | Pass | Fail | Pass rate |
|------------------|------|------|------|----------|
| identifiers      | 266  | 158  | 108  | 59.4%    |
| punctuators       | 11   | 10   | 1    | 90.9%    |
| white-space       | 67   | 49   | 18   | 73.1%    |
| keywords          | 25   | 24   | 1    | 96.0%    |
| types             | 113  | 80   | 33   | 70.8%    |
| comments          | 23   | 17   | 6    | 73.9%    |
| expressions/arrow-function | 343 | 183 | 160 | 53.4% |
| expressions/function        | 264 | 105 | 159 | 39.8% |
| expressions/object          | 722 | 415 | 307 | 57.5% |
| **subset total**  | 1834 | 1041 | 793 | ~56.7% |

(Numbers move as bugs are fixed; the CI job summary is the source of truth
for the current commit.)

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
