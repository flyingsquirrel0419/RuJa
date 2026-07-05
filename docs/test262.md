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
| **Supported subset** | `language/statements` + `language/expressions` — the areas RuJa actively targets, with unsupported-feature tests skipped | 94.4% | Run locally: `TEST262=… python3 tools/test262_runner.py language/statements language/expressions` |
| **CI subset** | 9 narrow directories the `ci.yml` job runs on every push (identifiers, keywords, types, comments, white-space, punctuators, arrow-function, function, object) | 83.1% | `CI` workflow job summary |

**The number to cite in README and public-facing material is the
supported-subset rate (94.4%).** It reflects the portion of the spec
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
~56% to 94.4%:

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
- **Delete reference semantics** — non-reference operands are still evaluated
  before `delete` returns `true`; function parameters are non-deletable;
  configurable global object properties such as `JSON` delete correctly; and
  `delete super.x`/`delete super[x]` throw `ReferenceError` in spec order.
- **Update-expression Reference semantics** — prefix/postfix
  increment/decrement evaluate targets once, preserve the original Reference
  through `GetValue`/`PutValue`, and keep BigInt update results as BigInt.
- **Object literal computed property keys** — computed data/accessor names run
  `ToPropertyKey` before value/function evaluation and preserve Symbol keys.
- **Object literal method semantics** — concise methods/accessors are
  non-constructors where required, ordinary concise methods lack an own
  `prototype`, and `super` property assignment uses the original receiver.
- **Object literal `__proto__` semantics** — duplicate prototype-mutation
  entries are early errors, while computed and shorthand `__proto__` entries
  remain ordinary data properties.
- **Live array-like `for...of` iteration** — arrays and arguments objects now
  use lazy indexed iteration so mutations during traversal are observed,
  accessor-index errors propagate, sloppy mapped arguments stay aliased to
  parameters until deleted, and `Object.defineProperty` on array indices
  updates array length.
- **`for...of` head parsing and early errors** — escaped `of` is no longer
  accepted as the loop delimiter, `async` is accepted as a contextual
  assignment target, invalid `const let` heads are rejected, and array/object
  left-hand sides must be valid assignment patterns.
- **`for...of` lexical head environments** — lexical loop heads now put their
  bound names in TDZ while evaluating the iterable, then create a fresh
  per-iteration environment before binding each value so destructuring
  defaults and body closures observe the correct binding.
- **`for...in` lexical head environments** — lexical loop heads now put their
  bound names in TDZ while evaluating the enumerated object, then create a
  fresh per-iteration environment before binding each property key so
  destructuring defaults and body closures observe the correct binding.
- **Compile-only loop scope unwinding** — labelled `continue` from an inner
  `for`/`for...in`/`for...of` now unwinds only real runtime environment
  records, preserving direct-eval completion values instead of over-popping the
  eval environment.
- **`for...in` enumeration and descriptor preservation** — non-enumerable own
  properties now shadow prototype keys during enumeration, deleted
  not-yet-visited keys are skipped, and `Object.defineProperty` preserves
  unspecified fields when redefining an existing descriptor while rejecting
  invalid non-configurable/non-extensible redefinitions.
- **Array-index assignment through prototype setters** — missing array-index
  writes now honor inherited setters before extending the array, fixing
  member-expression `for...in` assignment heads such as `[let][1]`.
- **Direct eval `var` leakage from lexical `for...in` heads** — sloppy direct
  eval now leaks `var`/function declarations to the caller's variable
  environment, not the temporary TDZ environment used while evaluating the
  right-hand object expression.
- **Ordinary object `[[Set]]` own-property precedence** — own accessor/data
  descriptors now handle assignment before inherited setters or inherited
  non-writable data properties, fixing writable checks for ordinary function
  `.prototype` and `prototype.constructor` descriptors.
- **Catch parameter early errors** — catch parameters now reject duplicate
  bound names and direct catch-block lexical/function redeclarations of the
  same name while preserving allowed `var` and nested block shadowing.
- **Native runtime errors through `finally`** — catchable VM-raised
  `ReferenceError`/`TypeError` completions now run active `finally` blocks
  before reaching an outer `catch`, matching explicit `throw` control flow,
  while re-thrown Error objects preserve their specific error kind.
- **Native Error constructor call branding** — plain calls to native Error
  subclasses now allocate through the active callee's `prototype`, so
  `EvalError(1)`, `TypeError(1)`, and aliased constructor calls preserve
  their specific `name`, `toString()`, and `instanceof` behavior.
- **Declarative binding deletion semantics** — `delete` now returns `false`
  for lexical/catch declarative bindings instead of removing them, preserving
  catch parameter values and matching `DeleteBinding` behavior for covered
  sloppy-mode `delete identifier` cases.
- **`try`/`finally` completion replacement semantics** — pending return,
  break, continue, and throw completions are preserved across normal
  `finally` evaluation, but correctly replaced by abrupt completions from the
  `finally` body. Non-throw abrupt completions also disable skipped catch
  guards before entering `finally`, bringing `language/statements/try` to
  **98 pass / 0 fail**.
- **Function.prototype restricted properties** — bound functions now inherit
  the `%Function.prototype%` `caller`/`arguments` accessors that throw
  `TypeError`, while still reporting no own `caller` or `arguments`
  properties.
- **Function body `"use strict"` with non-simple parameters** — function
  declarations, function expressions, object/class methods, and arrow block
  bodies now reject directive-prologue `"use strict"` when parameters contain
  defaults, rest, or destructuring.
- **Object/class method formal-parameter early errors** — concise and async
  object methods plus class/private methods reject duplicate formal parameter
  bound names, including destructuring duplicates, and object async methods
  enforce no line terminator between `async` and the property name.
- **`yield` contextual identifier parsing** — sloppy non-generator code now
  treats `yield` as an identifier for bindings, expressions, destructuring
  patterns, object method parameters/defaults, and computed property names,
  while generator contexts keep `yield` as the generator keyword.
- **`let` declaration ASI/lookahead parsing** — `let` followed by a binding
  name is parsed as a LexicalDeclaration across line terminators where
  StatementListItem permits declarations, escaped `let` remains an identifier,
  and single-statement bodies still use ExpressionStatement lookahead rules.
- **Parenthesized assignment-pattern targets** — parenthesized object/array
  literals are rejected as targets for an outer assignment while valid inner
  destructuring assignments remain accepted.
- **`await` contextual identifier parsing** — sloppy non-async code now treats
  `await` as a contextual identifier for bindings, references, parameters,
  destructuring patterns, object method parameters, and computed property
  names, while async function/method/arrow contexts keep `await` as the async
  keyword.
- **`import.meta` assignment target early errors** — direct and parenthesized
  `import.meta` assignments now fail in the parser, closing the
  `language/expressions/assignmenttargettype` subset.
- **Object destructuring assignment target order** — member-expression targets
  inside object assignment patterns are evaluated before source property
  `GetV`, including computed source keys, reducing
  `language/expressions/assignment/destructuring` to **1 pass / 5 fail**.

## Why the rate is not higher

The remaining 235 failures plus 2 timeouts in the supported subset cluster
around object literal/method-definition semantics, function/class behavior,
super semantics, and destructuring assignment. These are tracked in
`HANDOFF.md` and will be addressed in subsequent rounds.
