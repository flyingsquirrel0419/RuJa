# test262 conformance

RuJa runs the [test262](https://github.com/tc39/test262) conformance suite
via `tools/test262_runner.py`. The runner uses the **real test262 harness**
(`sta.js`, `assert.js`, and per-test `includes:` such as `propertyHelper.js`
and `compareArray.js`) rather than a hand-rolled stub, so tests relying on
`verifyProperty`, `compareArray`, etc. are exercised correctly. It also
parses `negative:` metadata so a test that expects a `SyntaxError`/
`TypeError` (parse or runtime phase) passes when RuJa raises the matching
error.

RuJa does **not** claim full ES conformance. Instead, it targets a
deliberately scoped subset of ES5.1 + selected ES2015+ features (see
[Supported subset](#supported-subset) below). Tests requiring unsupported
features (modules, TypedArrays, Atomics, Intl, etc.) are skipped via the
runner's `SKIP_FEATURES` set.

## Three pass-rate scopes

There are three distinct pass-rate numbers. Each measures a different
scope, so they are not comparable to each other:

| Scope | What it measures | Current rate | Where to verify |
|-------|-----------------|-------------|-----------------|
| **Full suite** | Entire test262 tree (excl. intl402/staging) — includes thousands of tests for features RuJa does not support | 33.2% | `test262-full` CI workflow job summary |
| **Supported subset** | `language/statements` + `language/expressions` — the areas RuJa actively targets, with unsupported-feature tests skipped | 85.0% | Run locally: `TEST262=… python3 tools/test262_runner.py language/statements language/expressions` |
| **CI subset** | 9 narrow directories the `ci.yml` job runs on every push (identifiers, keywords, types, comments, white-space, punctuators, arrow-function, function, object) | 83.1% | `CI` workflow job summary |

**The number to cite in README and public-facing material is the
supported-subset rate (84.8%).** It reflects the portion of the spec
RuJa actively targets. The full-suite number is published for
transparency but is dominated by unsupported features. The CI-subset
number is a narrow regression gate, not a conformance claim.

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

# Run the supported subset (statements + expressions):
TEST262=/path/to/test262 python3 tools/test262_runner.py language/statements language/expressions

# Or run a narrower subtree:
TEST262=/path/to/test262 python3 tools/test262_runner.py language/identifiers language/keywords
```

For failure-bucket analysis with error samples, use the sibling analyzer:

```sh
python3 tools/analyze_failures.py
```

## Full-suite baseline

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

This number is dominated by tests for features RuJa does not support.
It is published for transparency and regression tracking, not as a
conformance claim. The CI job summary is the source of truth for the
current commit.

## CI subset detail

The `ci.yml` workflow runs a narrow 9-directory subset on every push.
This is a regression gate, not a conformance metric:

| Suite | Ran | Pass | Fail | Pass rate |
|-------|-----|------|------|-----------|
| identifiers | 266 | 159 | 107 | 59.8% |
| punctuators | 11 | 11 | 0 | 100.0% |
| white-space | 67 | 49 | 18 | 73.1% |
| keywords | 25 | 24 | 1 | 96.0% |
| types | 113 | 80 | 33 | 70.8% |
| comments | 23 | 17 | 6 | 73.9% |
| expressions/arrow-function | 343 | 245 | 98 | 71.4% |
| expressions/function | 264 | 159 | 105 | 60.2% |
| expressions/object | 946 | 506 | 440 | 53.5% |
| **Total** | 2,058 | 1,250 | 808 | 60.8% |

(Numbers move as bugs are fixed; the CI job summary is the source of truth
for the current commit.)

## What was fixed to get here

Key test262-driven bug fixes that raised the supported-subset rate from
~56% to 84.8%:

- **Lexer: Unicode identifiers** — `\uXXXX`/`\u{XXXX}` escape forms,
  Unicode letters, NEL/LS/PS line terminators.
- **Parser: destructuring parameters, object-literal methods, reserved
  words as property keys, `negative:` harness metadata.**
- **Switch/catch lexical scope** — `let`/`const` in case bodies and
  catch parameters are properly block-scoped.
- **Escaped get/set** — `\u0067et` is not treated as a getter keyword.
- **Class constructors** — `TypeError` when called without `new`;
  non-object return from derived constructor throws; double `super()`
  throws; `extends` validates the parent is a constructor.
- **BigInt increment/decrement** — `x++` on BigInt returns BigInt.
- **Object.prototype.toString** — returns `[object Object]` for plain
  objects; `String.prototype.toString` added.
- **Inline-cache invalidation** — `SetElem` and `Object.defineProperty`
  invalidate the monomorphic cache so writes are visible to subsequent
  reads.
- **BigInt divide/modulo by zero** — `1n / 0n` and `1n % 0n` throw
  `RangeError`.
- **BigInt exponent overflow** — huge exponents no longer clamp to zero
  and return a wrong value; they throw `RangeError`.
- **Arrow function early errors** — arrow functions reject
  `eval`/`arguments` parameter names and duplicate parameter names in
  sloppy as well as strict mode.
- **Tagged-template objects** — template-literal sites return a cached,
  frozen template object with a frozen `raw` property per
  `GetTemplateObject`.
- **Array exotic descriptors** — `Object.getOwnPropertyDescriptor` now
  returns descriptors for Array `length` and index properties.

## Why the rate is not higher

The remaining ~633 failures in the supported subset cluster around:
`with`-statement + compound-assignment reference semantics (~130, needs
VM reference type), class builtin subclassing (~57), switch/try
completion-value edge cases (~38), arrow-function early errors (~16),
and tagged-template caching/`raw` property (~25). These are tracked in
`HANDOFF.md` and will be addressed in subsequent rounds.
