# test262 conformance

RuJa runs the [test262](https://github.com/tc39/test262) conformance suite
via `tools/test262_runner.py`. The runner uses the **real test262 harness**
(`sta.js`, `assert.js`, and per-test `includes:` such as `propertyHelper.js`
and `compareArray.js`) rather than a hand-rolled stub, so tests relying on
`verifyProperty`, `compareArray`, etc. are exercised correctly. It also
parses `negative:` metadata so a test that expects a `SyntaxError`/
`TypeError` (parse or runtime phase) passes when RuJa raises the matching
error, and honors `flags: [raw]` by running those files without any harness
prelude.

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
| **Full suite** | Entire test262 tree (excl. intl402/staging) — includes thousands of tests for features RuJa does not support | 31.4% of all matrix files; 62.2% of executed files in the latest confirmed full run | `test262-full` CI workflow job summary |
| **Supported subset** | `language/statements` + `language/expressions` — the areas RuJa actively targets, with unsupported-feature tests skipped | 100.0% (5003 pass / 0 fail) | Run locally: `TEST262=… python3 tools/test262_runner.py language/statements language/expressions` |
| **CI subset** | 9 narrow directories the `ci.yml` job runs on every push (identifiers, keywords, types, comments, white-space, punctuators, arrow-function, function, object) | 100.0% | `CI` workflow job summary |

**The number to cite in README and public-facing material is the
supported-subset rate (100.0%).** It reflects the portion of the spec
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

**Selected ES2015+**: arrow functions, classes/extends/super, class static
blocks, default & rest parameters, destructuring (array/object/nested),
template literals,
tagged templates, computed property keys, object spread/rest, getters/
setters, `new.target`, optional catch binding, Symbol.iterator,
Symbol.unscopables, Map/Set/WeakMap/WeakSet, BigInt, Proxy, Reflect,
Promise, async/await, generators, for-of, optional chaining, nullish
coalescing, logical assignment.

**Intentionally unsupported**: ES Modules (import/export), Intl, Atomics,
SharedArrayBuffer, most TypedArray variants beyond partial Uint8Array/
ArrayBuffer/DataView support, WeakRef/FinalizationRegistry, Tail-call
optimization.

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
python3 tools/test262_analyze.py
```

The focused analyzer mirrors the runner's `raw`, `onlyStrict` directive
prologue, and `negative:` metadata handling, so strict-mode and parse-negative
tests are not reported as false failure buckets.

## Full-suite baseline

The `test262-full` CI workflow runs the entire test262 tree (excluding
`intl402`/`staging`) in parallel. Counts can vary slightly because a small
number of tests can cross the timeout boundary. Baseline confirmation run:
`test262-full` 28835294182 on `64a763a`.

| Metric | Recent count |
|--------|--------------|
| Total matrix files | 47,717 |
| Actually run | 24,131 |
| Pass | 15,001 |
| Fail | 9,130 |
| Timeout | 0 |
| Skip | 23,573 |
| **Pass rate (of run)** | **62.2%** |
| **Pass rate (of total)** | **31.4%** |

This number is dominated by tests for features RuJa does not support.
It is published for transparency and regression tracking, not as a
conformance claim. The CI job summary is the source of truth for the
current commit.

## CI subset detail

The `ci.yml` workflow runs a narrow 9-directory subset on every push.
This is a regression gate, not a conformance metric:

| Suite | Ran | Pass | Fail | Timeout | Pass rate |
|-------|-----|------|------|---------|-----------|
| identifiers | 208 | 208 | 0 | 0 | 100.0% |
| punctuators | 11 | 11 | 0 | 0 | 100.0% |
| white-space | 65 | 65 | 0 | 0 | 100.0% |
| keywords | 25 | 25 | 0 | 0 | 100.0% |
| types | 109 | 109 | 0 | 0 | 100.0% |
| comments | 20 | 20 | 0 | 0 | 100.0% |
| expressions/arrow-function | 92 | 92 | 0 | 0 | 100.0% |
| expressions/function | 53 | 53 | 0 | 0 | 100.0% |
| expressions/object | 285 | 285 | 0 | 0 | 100.0% |
| **Total** | 868 | 868 | 0 | 0 | 100.0% |

(Numbers move as bugs are fixed; the CI job summary is the source of truth
for the current commit.)

## What was fixed to get here

Key test262-driven bug fixes that raised the supported-subset rate from
~56% to 100.0%:

- **String search argument coercion** —
  `String.prototype.indexOf` and `lastIndexOf` now coerce `searchString`
  through `ToString` before reading the position argument. Missing arguments
  therefore search for `"undefined"` instead of `""`, object search values
  invoke observable `toString`, and abrupt completions happen in spec order.
  `indexOf` positions now clamp negative values to 0 rather than using
  Array-style from-index wrapping, and `lastIndexOf` clamps finite negative
  values to 0 while preserving the `NaN`/`+Infinity` search-from-end path.
  The focused
  `built-ins/String/prototype/indexOf
  built-ins/String/prototype/lastIndexOf` cluster runs at **62 pass / 0 fail
  / 10 skip**.
- **String repeat count coercion** —
  `String.prototype.repeat` now applies the shared integer coercion path to
  its count argument before range checking. `NaN`, `undefined`, `false`,
  `"0"`, and fractional counts below 1 now produce the empty string, while
  negative values and infinities still throw `RangeError`. The focused
  `built-ins/String/prototype/repeat` cluster runs at **13 pass / 0 fail / 3
  skip**.
- **String index position coercion** —
  `String.prototype.charAt`, `charCodeAt`, and `codePointAt` now route their
  position arguments through the shared integer-position coercion path before
  range checks. Explicit `undefined`, `NaN`, and other non-numeric values now
  select index 0, while negative, infinite, and out-of-range positions still
  return the method-specific empty string, `NaN`, or `undefined`. The focused
  `built-ins/String/prototype/charAt
  built-ins/String/prototype/charCodeAt
  built-ins/String/prototype/codePointAt` cluster runs at **66 pass / 0 fail
  / 5 skip**.
- **Symbol intrinsic surface completion** —
  `Symbol.length`, well-known Symbol constructor property descriptors,
  missing well-known Symbols (`isConcatSpreadable`, `matchAll`, `replace`,
  `search`, `split`), `Symbol.prototype.valueOf`, and primitive
  `Object.getPrototypeOf(Symbol())` now follow the spec surface. Array, Map,
  Promise, RegExp, and Set expose named `get [Symbol.species]` accessors that
  return the receiver, preserving subclass species lookup. With the `Symbol`
  skip temporarily lifted, the full `built-ins/Symbol` diagnostic runs at
  **67 pass / 0 fail / 31 skip**.
- **`new.target` eval-context early errors** —
  Script/global code, indirect eval code, and direct eval code contained in
  arrow-function code now reject `new.target` with `SyntaxError`, while direct
  eval contained in non-arrow function code inherits the caller's active
  `new.target`. Ordinary function parameter defaults parse under the same
  `new.target` context. The focused
  `language/global-code language/eval-code` cluster now runs at
  **331 pass / 0 fail / 58 skip**.
- **Symbol description/keyFor registry semantics** —
  Symbols now retain optional descriptions, `Symbol.prototype.description`
  and `Symbol.keyFor` are exposed with spec receiver checks, and
  `String(Symbol(...))` / `Symbol.prototype.toString` include descriptions.
  Test262-created realms install distinct `Symbol`, `Symbol.for`, and
  `Symbol.keyFor` function objects while sharing the VM-level global Symbol
  registry, closing the cross-realm registry tests. Sloppy writes to
  coercible Symbol primitives are ignored, strict writes throw, and nullish
  member assignment still throws in sloppy mode. With the `Symbol` skip
  temporarily lifted, the focused
  `built-ins/Symbol/for built-ins/Symbol/keyFor
  built-ins/Symbol/prototype/description built-ins/Symbol/prototype/toString`
  cluster runs at **28 pass / 0 fail / 4 skip**.
- **`Reflect.ownKeys` Symbol key coverage** —
  `Reflect.ownKeys` now uses RuJa's full own-property-key helper instead of
  the string-only enumerable-key path, so it returns array-index strings,
  ordinary strings, and Symbols in `[[OwnPropertyKeys]]` order and includes
  non-enumerable keys. Primitive targets now throw `TypeError`.
  `Symbol.for` also reuses a VM-level global symbol registry for repeat keys.
  With `Reflect` and `Symbol` skips temporarily lifted, focused
  `built-ins/Reflect/ownKeys` runs at **11 pass / 0 fail / 2 skip**.
- **Thrown custom object display** —
  Uncaught ordinary objects created by custom constructors now include their
  prototype constructor name in the host error message. This preserves
  test262's `Test262Error` marker while keeping the actual thrown value
  unchanged for `catch` blocks. The focused `language/line-terminators` shard
  now runs at **41 pass / 0 fail / 0 skip**.
- **Statement-list regex literal recovery** —
  Regular expression literals that start a new statement after a preceding
  block-like statement boundary are now recovered in parser primary-expression
  position when the eager lexer emitted `/` tokens. This closes the focused
  `language/statementList` regex-literal failures, raising that shard to
  **60 pass / 0 fail / 20 skip**.
- **Block-scope declaration early errors** —
  Block statement-list early-error checks now use block-specific declaration
  name semantics: block-level function declarations contribute lexical names,
  nested statement `var` declarations contribute to the enclosing block's
  `VarDeclaredNames`, and `for-in`/`for-of` declaration heads reject multiple
  declarators. The focused `language/block-scope` shard now runs at **94 pass
  / 0 fail / 51 skip**.
- **Escaped reserved-word early errors** —
  Identifiers containing Unicode escapes now remain identifier-name tokens
  instead of being promoted to keyword/literal tokens, and reserved words such
  as escaped `true`, `false`, `null`, or `var` are rejected in
  identifier-reference, binding, shorthand, and label positions. Escaped
  reserved words remain valid property names. The focused
  `language/literals/boolean language/literals/null language/reserved-words
  language/keywords language/future-reserved-words` cluster now runs at **113
  pass / 0 fail / 1 skip**, and `language/literals` improves to **315 pass /
  159 fail / 60 skip**.
- **`with` object-environment HasBinding** —
  `with` statements now box primitive binding objects with `ToObject` after
  rejecting null/undefined, and object-environment binding lookup uses
  `[[HasProperty]]` over the prototype chain instead of own-property checks.
  Inherited properties now resolve through `with` for reads, calls,
  assignments, and compound assignments, while primitive string binding
  objects expose `length`. The focused `language/statements/with`
  plus assignment/update cluster runs at **398 pass / 0 fail / 410 skip**.
- **Destructuring assignment Reference target preservation** —
  Object and array destructuring assignment targets now evaluate identifier
  References before reading source properties, stepping iterators, or running
  default initializers. This preserves the selected `with` object-environment
  binding even if those later operations delete the property before `PutValue`.
  The focused Reference cluster
  `language/statements/with language/expressions/assignment
  language/expressions/destructuring language/expressions/compound-assignment
  language/expressions/logical-assignment language/expressions/delete` runs at
  **904 pass / 0 fail / 363 skip**.
- **Promise built-in surface expansion** —
  `Symbol.species`, `Promise[@@species]`, `Promise.prototype.finally`, and the
  Promise static surface `all`/`race`/`allSettled`/`any`/`try`/`withResolvers`
  are now exposed, and `String(Symbol(...))` uses the special `String`
  constructor conversion. Promise constructor resolve/reject functions now have
  the expected anonymous `name`, unary `length`, property order, extensibility,
  non-constructor shape, and `Function.prototype` inheritance. Static
  `Promise.resolve` and `Promise.reject` now create and invoke a
  receiver-constructor capability instead of directly allocating a base
  Promise. `Promise.prototype.catch` and `Promise.prototype.finally` now invoke
  the receiver's observable `then` property, including getter and call abrupt
  completions. `Promise.prototype.then` now validates its receiver as a real
  Promise, uses `SpeciesConstructor` for the derived promise, and stores
  Promise reaction capabilities so custom species constructors and capability
  executor validation follow the spec path. The `Promise` constructor now
  rejects calls made without `new` and invokes its executor with `undefined` as
  the receiver, so sloppy executors see `globalThis` while strict executors keep
  `undefined`. `Promise.try` now creates its result through
  `NewPromiseCapability(this)`, so subclass/custom receivers, constructor
  abrupt completions, and non-constructor receiver validation follow the spec
  path. Class computed method and accessor names now use `ToPropertyKey`, and
  method definition preserves Symbol keys, so `static get [Symbol.species]`
  defines the well-known Symbol property instead of a string-named property.
  Promise reactions that return an already-settled Promise now schedule direct
  pass-through adoption instead of storing an undrainable handler, avoiding
  hangs while preserving pending-Promise adoption. `Promise.race` now constructs
  through the receiver capability, reads `C.resolve` once, and invokes each
  resolved entry's observable `then` with the capability resolve/reject
  functions. `Promise.all` now constructs through the receiver capability,
  reads `C.resolve` once, invokes each resolved entry's observable `then`, and
  resolves through the outer capability with an ordered result array.
  `Promise.allSettled` now follows the same constructor capability and
  `C.resolve` path, creates paired per-element resolve/reject functions sharing
  an `alreadyCalled` guard, records ordered fulfilled/rejected result objects,
  and rejects the outer capability if the final capability resolve abruptly
  completes. `Promise.any` now follows the receiver constructor capability and
  `C.resolve` path, invokes observable `then`, tracks per-element rejection
  functions with `alreadyCalled` guards, preserves rejection order, and rejects
  with a minimal `AggregateError` carrying a non-enumerable `errors` array.
  `Promise.all` also rejects its outer capability if the final capability
  resolve abruptly completes. `Promise.allKeyed` and
  `Promise.allSettledKeyed` now expose the `await-dictionary` proposal
  surface, construct through the receiver capability, read `C.resolve` once,
  enumerate own enumerable string and Symbol keys, preserve key order
  independently of settlement order, invoke each resolved entry's observable
  `then`, and resolve to a null-prototype keyed result object.
  A diagnostic `built-ins/Promise` run with only the `Promise` skip lifted is
  **255 pass / 0 fail / 0 timeout / 448 skip**. Focused
  `built-ins/Promise/allKeyed built-ins/Promise/allSettledKeyed` runs at
  **18 pass / 0 fail / 45 skip**, `built-ins/Promise/any` runs at
  **26 pass / 0 fail / 68 skip**, and the
  `all`/`race`/`allSettled`/`any`/`resolve` diagnostic cluster runs at
  **136 pass / 0 fail / 284 skip**. The Promise skip remains in the supported
  runner until the broader skipped async/proposal coverage is intentionally
  lifted.
- **Mapped arguments exotic descriptors** —
  Non-strict arguments objects now use `Object.prototype`, expose `length` as
  a configurable ordinary data property rather than Array exotic length,
  report `Array.isArray(arguments) === false`, and keep mapped parameter
  bindings synchronized with index data descriptors until an accessor
  descriptor or `writable: false` unmaps that index. Computed delete now uses
  the same configurability path as direct delete, and accessor indices no
  longer fall through to dense element storage when no getter exists. Sloppy
  function `caller` lookup now supports the Annex B call-stack path needed by
  `arguments.callee.caller`, while strict callers remain restricted. Member
  calls with spread arguments now preserve their receiver and spread arity.
  The focused `language/arguments-object` cluster now runs at **126 pass / 0
  fail / 137 skip**.
- **Logical-assignment Reference preservation** —
  Identifier logical assignments now carry the original spec Reference from
  `GetValue` through `PutValue`, preventing `with`/global object references
  from being re-resolved to an outer binding when the RHS deletes the original
  property. Member logical assignments discard their saved target pair on
  short-circuit paths so the expression yields the existing value, perform the
  nullish-base `ToObject` check before computed-key `ToPropertyKey`, and
  identifier logical assignments apply NamedEvaluation to anonymous function,
  arrow, and class RHS values. `logical-assignment-operators` is now removed
  from the skip filters; `language/expressions/logical-assignment` runs at
  **57 pass / 0 fail / 21 skip**. The focused
  `language/statements/with language/expressions/assignment
  language/expressions/logical-assignment language/expressions/update` cluster
  runs at **338 pass / 0 fail / 406 skip**, with local regression coverage for
  the previously untested Reference edges.
- **Private-field assignment targets** —
  Private-field update, compound, and logical assignments now keep the private
  reference base evaluated once and store back through the same object. This
  makes `obj.#x++`, `obj.#x += y`, and `obj.#x ||= y` update private fields and
  accessors correctly while preserving short-circuit expression results. The
  focused `language/expressions/compound-assignment
  language/expressions/logical-assignment language/expressions/update` run is
  **463 pass / 0 fail / 69 skip**. Private class feature tests are still
  skipped by the runner's unsupported-feature filters, so this edge is guarded
  by local class regression tests until those filters are lifted.
- **Class static block feature lift** —
  Static initialization blocks now have their own parse context instead of
  being parsed as function bodies: `return` is rejected, `super.prop` is
  accepted, and static-block early errors catch direct `await`, `yield`,
  `arguments`, and duplicate labels without crossing function/static-block
  boundaries. Async class method metadata is also preserved through
  compilation. `class-static-block` is now removed from the skip filters, and
  the supported subset runs at **4335 pass / 0 fail / 16103 skip**.
- **`delete` through `with` object environments** —
  Identifier deletion now routes `with` object environment records through
  the same `[[HasProperty]]` and `Symbol.unscopables` HasBinding logic as
  reads and writes before applying ordinary property deletion. Inherited
  `with` bindings, unscopables-hidden properties, and abrupt unscopables
  getters now follow the same Reference path as other identifier operations.
  The focused `language/statements/with language/expressions/delete` run
  stays at **235 pass / 0 fail / 15 skip**, and the broader
  Reference-focused delete cluster runs at **404 pass / 0 fail / 409 skip**.
- **Direct eval through `with` object environments** —
  Unqualified `eval(...)` now evaluates the `eval` identifier and then
  decides direct-vs-indirect eval from the resolved callee. This lets `with`
  object properties shadow eval with ordinary functions, propagates abrupt
  `eval` getters in the correct order, and still treats `with ({ eval }) {
  eval(src) }` as direct eval when the resolved function is the current
  Realm's intrinsic `%eval%`. The focused `language/statements/with` run
  remains **169 pass / 0 fail / 12 skip**, and the supported subset remains
  **4276 pass / 0 fail / 16162 skip** while closing this previously untracked
  Reference/eval edge.
- **Strict directive and future-reserved-word early errors** —
  Sloppy binding names now accept strict-only future reserved words such as
  `implements`, `interface`, `package`, `private`, `protected`, `public`,
  `static`, and `yield`, while `enum` remains reserved in all binding
  contexts. Strict binding and identifier-reference positions reject the full
  strict-only reserved set. String literal tokens now preserve whether their
  source contained an escape sequence or line continuation, preventing escaped
  `"use strict"` spellings from enabling strict mode; Function-constructor
  strict bodies also report `SyntaxError` when direct eval parses strict-only
  reserved identifier references. The focused
  `language/future-reserved-words language/directive-prologue` cluster now
  runs at **117 pass / 0 fail / 0 skip**.
- **Identifier Unicode tables and reserved binding names** —
  Identifier lexing now uses Unicode identifier property tables with the ES
  `$`, `_`, ZWNJ/ZWJ, and grandfathered `Other_ID_Start`/`Other_ID_Continue`
  additions. Invalid Pattern_Syntax characters such as U+2E2F now surface as
  `SyntaxError` rather than accidental binding names, and `import`/`export`
  are rejected as binding names in variable declarations, function names, and
  parameters. The focused `language/identifiers` cluster now runs at **208
  pass / 0 fail / 60 skip**, bringing the CI subset to **866 pass / 0 fail / 0
  timeout**.
- **Object.values/Object.entries enumerable snapshot semantics** —
  `Object.values` and `Object.entries` now reject nullish inputs before key
  conversion, use the ordinary own-string-key snapshot order, then re-check
  each key's current enumerable descriptor before reading the value. Keys that
  are deleted or made non-enumerable by an earlier getter are omitted.
  Empty-handler Proxies now forward own-key, own-descriptor, `hasOwnProperty`,
  and `Object.defineProperty` no-trap operations to their targets for this
  path. The focused
  `built-ins/Object/values built-ins/Object/entries built-ins/Object/hasOwn
  built-ins/Object/getOwnPropertyDescriptors` cluster now runs at **98 pass /
  0 fail / 23 skip**.
- **String static constructors** —
  `String.fromCodePoint` now throws `RangeError` for non-integral,
  non-finite, negative, and out-of-range code point inputs instead of silently
  truncating through integer casts. `String.raw` now appends the empty string
  when a substitution is missing, while explicit `undefined` raw segments
  still convert through `ToString`. The focused
  `built-ins/String/fromCodePoint built-ins/String/raw
  built-ins/String/fromCharCode` cluster now runs at **51 pass / 0 fail / 7
  skip**.
- **Function-code `this` binding and primitive receivers** —
  Non-strict interpreted functions now convert `this` per function-code entry
  rules: nullish receivers become the global object and primitive receivers are
  boxed, while strict functions keep the raw receiver. Primitive prototype
  accessor lookup also preserves the original primitive as the receiver through
  the prototype chain, so strict getters see the primitive and sloppy getters
  see the boxed object.
- **Function declaration instantiation edges** —
  Non-strict duplicate parameters now keep separate raw argument slots while
  sharing the same parameter binding, so an omitted later duplicate overwrites
  with `undefined`. Function-scope `var` declarations reuse parameter bindings,
  function declarations overwrite parameter and `arguments` bindings, and
  strict block-level function declarations are initialized as block-scoped
  lexical bindings instead of leaking through Annex B function-scope hoisting.
  The focused `language/function-code` cluster now runs at **217 pass / 0 fail
  / 0 skip**.
- **Global declaration instantiation edges** —
  Global scripts now validate lexical/global-var collisions, restricted global
  properties, and non-extensible global objects before executing script body
  side effects. Global function declarations use
  `CreateGlobalFunctionBinding` descriptor rules, global `var` declarations
  use `CreateGlobalVarBinding`, sloppy direct-eval global `var` properties
  remain configurable, and strict global block-level function declarations stay
  block-scoped. The focused `language/global-code` cluster now runs at **31
  pass / 0 fail / 11 skip**.
- **Eval global declaration bindings** —
  Non-strict direct and indirect eval now preflight global `var`/function
  declarations with eval-specific global declaration checks and create
  configurable global eval bindings. `$262.evalScript()` remains routed
  through script-global declaration semantics, so its global declarations stay
  non-configurable. Same-Realm indirect eval now runs in a fresh lexical
  environment, preventing eval-local lexical declarations and strict
  `var`/function declarations from leaking to the global object. Direct eval
  now creates non-strict `var`/function declarations in the caller's variable
  environment, keeps existing local var bindings intact, makes newly-created
  local eval bindings deletable, preserves `with` object lookup inside eval
  source, and reflects cross-Realm indirect eval declarations on the target
  Realm's global object. Direct eval during function, arrow, and generator
  parameter initialization now rejects `var arguments` declarations against an
  existing arguments binding, and generator calls now execute parameter and
  declaration-instantiation bytecode before returning a suspended generator.
  The focused `language/eval-code` cluster now runs at **225 pass / 0 fail /
  122 skip**.
- **Numeric literal early errors** —
  The lexer now rejects malformed radix prefixes, invalid numeric separator
  placement, BigInt suffixes on fractional/exponent/legacy-octal-like forms,
  and identifier-start characters immediately following numeric literals.
  Legacy octal and non-octal decimal literals remain accepted in sloppy mode
  but are rejected in strict mode. The focused
  `language/literals/bigint language/literals/numeric` cluster now runs at
  **216 pass / 0 fail / 0 skip**, and broader `language/literals` improves to
  **312 pass / 162 fail / 60 skip**; the remaining failures are isolated to
  regexp literal early errors/engine semantics, string legacy escapes, and
  keyword-unicode literal edges.
- **Unicode whitespace/comment lexing and `String.fromCharCode` coercion** —
  The lexer now skips ES Unicode space separators and BOM as whitespace,
  handles CR/LF/LS/PS as the only line terminators for single-line comments,
  preserves ASI newline tracking through multiline comments, and reports
  unterminated multiline comments and regular-expression literals. NEL
  (U+0085) remains ordinary comment text rather than a line terminator.
  `String.fromCharCode` now applies `ToNumber` and `ToUint16` to every
  argument, so hex string code units such as `"0x000A"` create the expected
  line terminator. The focused `language/comments language/white-space`
  cluster now runs at **85 pass / 0 fail / 34 skip**, and the CI subset now
  runs locally at **823 pass / 41 fail / 2 timeout**.
- **test262 `$262.createRealm()` and native constructability** — RuJa now
  exposes the test262 host `createRealm()` surface, runs indirect eval against
  the callee's Realm environment, keeps tagged-template caches per compiled
  Realm execution, and distinguishes constructable native constructors from
  callable-only native functions. This closes the remaining cross-realm
  supported-subset failures and the Proxy subclass edge case.
- **Small language conformance edges** — Sloppy functions with rest/default/
  destructuring parameters now use unmapped arguments objects, `__proto__`
  assignment cannot mutate a non-extensible object's prototype, and
  Symbol-keyed assignment follows ordinary property-set semantics for
  accessors, inherited setters, read-only descriptors, and non-extensible
  receivers. The parser/lexer now rejects `throw` followed by a line
  terminator, unterminated string literals, and reserved-word shorthand object
  properties such as `({ this })`, while accepting `undefined` as a `var`
  binding name. The focused
  `language/asi language/computed-property-names language/keywords
  language/rest-parameters language/types` cluster now runs at **290 pass / 0
  fail / 9 skip**.
- **String search methods and `Symbol.match`** —
  `String.prototype.includes`, `String.prototype.startsWith`, and
  `String.prototype.endsWith` now reject nullish receivers, consult
  `@@match` before converting the search argument to string, reject RegExp
  search arguments, propagate abrupt completions from receivers and
  `Symbol.match` accessors, and clamp position/end-position arguments using
  integer conversion. `Object.defineProperty` now preserves Symbol property
  keys for this path, and generated Symbols start after the well-known Symbol
  range. The focused
  `built-ins/String/prototype/startsWith built-ins/String/prototype/endsWith
  built-ins/String/prototype/includes` cluster now runs at **63 pass / 0 fail /
  12 skip**.
- **Sparse array holes and own-key enumeration** — Dense arrays now keep a
  separate present-bit vector so an explicit `undefined` element is distinct
  from an elision or deleted element. Array literal holes, `Array(length)`,
  dense index assignment/deletion, descriptors, `hasOwnProperty`,
  `propertyIsEnumerable`, `Object.keys`, `Object.getOwnPropertyNames`, and
  array `for...in` now agree on whether an index is present. Boxed String
  exotic indices are also included in `for...in`, matching
  `Object.keys(new String(...))`. The focused Object own-key cluster
  `built-ins/Object/getOwnPropertyNames built-ins/Object/keys` now runs at
  **90 pass / 0 fail / 14 skip**.
- **ArrayBuffer/DataView prototype accessors and detach host hook** —
  `ArrayBuffer.prototype.byteLength` and DataView `buffer`/`byteLength`/
  `byteOffset` are now real accessor properties with named getter functions,
  receiver validation, and detached-buffer semantics. `$262.detachArrayBuffer`
  is implemented for the test262 host surface, closing the focused
  ArrayBuffer/DataView accessor cluster at 29 pass / 0 fail / 19 skip.
- **DataView 8-bit element accessors** — `DataView.prototype.getUint8`,
  `getInt8`, `setUint8`, and `setInt8` now validate DataView receivers,
  convert byte offsets with `ToIndex`, preserve setter value-conversion order,
  reject detached buffers, validate byte ranges, write Uint8-wrapped bytes,
  and read Int8 values with signed interpretation. This closes the focused
  DataView 8-bit method cluster at 49 pass / 0 fail / 29 skip.
- **DataView 16-bit element accessors** — `DataView.prototype.getUint16`,
  `getInt16`, `setUint16`, and `setInt16` now handle big-endian defaults,
  `ToBoolean` little-endian arguments, Uint16 wrapping writes, signed Int16
  reads, and the required `ToIndex`/value/detached/range validation order.
  This closes the focused DataView 16-bit method cluster at 56 pass / 0 fail /
  28 skip.
- **DataView 32-bit element accessors** — `DataView.prototype.getUint32`,
  `getInt32`, `setUint32`, and `setInt32` now handle big-endian defaults,
  `ToBoolean` little-endian arguments, Uint32 wrapping writes, signed Int32
  reads, and the required `ToIndex`/value/detached/range validation order.
  This closes the focused DataView 32-bit method cluster at 56 pass / 0 fail /
  38 skip.
- **DataView floating-point element accessors** —
  `DataView.prototype.getFloat32`, `setFloat32`, `getFloat64`, and
  `setFloat64` now handle IEEE-754 byte encoding/decoding, big-endian
  defaults, `ToBoolean` little-endian arguments, `-0`/NaN/Infinity
  preservation, and the required `ToIndex`/value/detached/range validation
  order. This closes the focused DataView float method cluster at 62 pass /
  0 fail / 28 skip.
- **DataView BigInt element accessors** — `DataView.prototype.getBigInt64`,
  `getBigUint64`, `setBigInt64`, and `setBigUint64` now handle signed and
  unsigned 64-bit BigInt reads, big-endian defaults, `ToBoolean`
  little-endian arguments, `ToBigInt` setter conversion, modulo-`2^64`
  backing-store writes, and the required receiver/`ToIndex`/value/detached/
  range validation order. The official runner still skips this focused
  cluster while `ArrayBuffer` and `DataView` are marked unsupported; with only
  those feature skips lifted for diagnosis, the focused cluster runs at 40
  pass / 3 fail / 26 skip, with remaining failures requiring immutable
  ArrayBuffer and additional typed-array receiver support. The shared
  `BigInt()` constructor path now also converts primitive-producing objects
  and reports `TypeError` for missing/nullish input.
- **BigInt fixed-width statics** — `BigInt.asIntN` and `BigInt.asUintN` now
  coerce `bits` with `ToIndex` before coercing the value with `ToBigInt`,
  preserve the required error ordering, wrap signed and unsigned values modulo
  `2^bits`, and expose non-enumerable writable configurable static function
  properties with the required `name` and `length`. This closes the focused
  BigInt fixed-width static cluster at 14 pass / 0 fail / 14 skip and improves
  the broader `built-ins/BigInt` smoke run to 49 pass / 25 fail / 29 skip.
- **BigInt prototype conversion methods** — `BigInt.prototype.valueOf` now
  returns only primitive BigInt receiver data, and
  `BigInt.prototype.toString(radix)` now handles radices 2 through 36 with
  required `ToNumber`/`ToIntegerOrInfinity` validation. BigInt prototype
  descriptors, primitive-wrapper `Object(value)` prototype wiring, and ordinary
  `ToPrimitive` lookup for boxed primitives were tightened as part of the same
  cluster. This closes the focused BigInt prototype/valueOf/toString cluster
  at 16 pass / 0 fail / 5 skip and brings the broader `built-ins/BigInt` smoke
  run to 74 pass / 0 fail / 29 skip.
- **`Object.hasOwn`** — `Object.hasOwn` now exposes the ES2022 static
  own-property predicate with the required `ToObject` before `ToPropertyKey`
  ordering, symbol-key support, primitive string own `length`/index handling,
  and standard built-in function descriptors. This closes the focused
  `Object.hasOwn` cluster at 56 pass / 0 fail / 6 skip.
- **`Object.getOwnPropertyDescriptor`** —
  `Object.getOwnPropertyDescriptor` now performs `ToObject` before
  `ToPropertyKey`, preserves Symbol keys, synthesizes string exotic
  `length`/index descriptors, and creates `FromPropertyDescriptor` result
  objects with enumerable descriptor fields. Built-in constructor
  `length`/`name`/`prototype` descriptors plus descriptor-visible Object,
  Array, String, Number, Date, RegExp, and URI global members were tightened as
  part of the same cluster. This closes the focused
  `Object.getOwnPropertyDescriptor` cluster at 308 pass / 0 fail / 2 skip.
- **Object own-key enumeration** — `Object.keys`,
  `Object.getOwnPropertyNames`, `Object.getOwnPropertySymbols`, and
  `Object.getOwnPropertyDescriptors` now use shared array-index-first own-key
  ordering, reject nullish inputs with `TypeError`, run supported primitives
  through `ToObject`, include non-enumerable names where required, preserve
  Symbol keys, and synthesize string primitive index/`length` descriptor keys.
  This closes the focused `Object.getOwnPropertyDescriptors` and
  `Object.getOwnPropertySymbols` clusters at 13 pass / 0 fail / 17 skip; the
  broader own-key smoke run is 97 pass / 6 fail / 31 skip, with remaining
  failures requiring receiver-brand handling and sparse-array hole
  representation fixes.
- **`Object.prototype.toString` receiver brands** —
  `Object.prototype` no longer receives Error-prototype `name`/`message`/
  `toString` properties during bootstrap, and native calls no longer route
  every function named `toString` through Object's brand algorithm. The brand
  algorithm now distinguishes `undefined`, BigInt primitives, boxed primitive
  wrappers, functions, arrays, arguments objects, Date instances, and Error
  instances. This closes the focused `Object.prototype.toString` cluster; the
  combined Object toString/own-key smoke run is 105 pass / 3 fail / 37 skip,
  with the remaining failures isolated to sparse array holes and dense
  array-index deletion.
- **ArrayBuffer and DataView subclass internals** — minimal ArrayBuffer and
  DataView exotic heap objects now initialize internal slots during subclass
  construction; `ArrayBuffer.prototype.slice` returns the default subclass
  constructor result, and DataView exposes `buffer`, `byteOffset`, and
  `byteLength`.
- **Uint8Array subclass exotic construction** — `Uint8Array` now exposes a
  constructor `prototype`, subclass construction allocates typed-array exotic
  objects with `new.target.prototype`, and integer-index writes update the
  backing buffer with Uint8 wrapping semantics.
- **Promise subclass executor validation** — `Promise` constructors now throw
  `TypeError` for non-callable executors before allocating the promise object
  and create subclass promise objects with `new.target.prototype`.
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
- **Class declaration completion and binding mutability** — class declarations
  produce an empty statement completion; their outer lexical binding is
  mutable, while the inner class-name binding captured by methods and
  heritage remains immutable.
- **Class method descriptor validation** — computed static class methods and
  accessors now respect non-configurable constructor properties, so attempts
  to redefine a class constructor's `prototype` property throw `TypeError`.
- **Dynamic Function subclass construction** — native constructors preserve
  the active `new.target`, and dynamic `Function` instances get
  `new.target.prototype` as their internal prototype plus own `length` and
  `name` descriptors, so Function subclass `instanceof` and descriptor checks
  pass.
- **Array, RegExp, and String subclass exotic construction** — Array and
  RegExp constructors now use `new.target.prototype` when allocating their
  exotic objects, RegExp `lastIndex` stays non-configurable across
  `test`/`exec`, and boxed String instances expose their own
  non-configurable `length` descriptor.
- **GeneratorFunction constructor and prototype chain** — generator functions
  now inherit from `%GeneratorFunction.prototype%`, dynamic
  `GeneratorFunction` construction compiles `function*` bodies, generator
  function `prototype` objects inherit from the generator prototype without an
  own `constructor`, and generator calls allocate iterators from the callee's
  `prototype`.
- **For-of IteratorClose on abrupt completion** — `for...of` closes unfinished
  iterators when a loop body exits abruptly via `return`, `break`, or throw,
  while same-loop `continue` keeps the iterator open. Iterator `return()`
  errors override the pending for-of completion where required, closing the
  derived-constructor return-override for-of checks.
- **Private accessors and non-extensible private slots** — private
  getters/setters now install as accessor slots, static private elements
  initialize on constructor objects, functions track extensibility for
  `Object.preventExtensions`, and defining a new private slot on a
  non-extensible object throws `TypeError`.
- **Date subclass component semantics** — Date construction from
  year/month/day/time components now stores a clipped time value, and Date
  component getters derive calendar fields from the stored value, so Date
  subclasses initialize and read their inherited Date state correctly.
- **Null-extending classes and bound subclass construction** —
  `class C extends null {}` uses `null` as `C.prototype`'s prototype and
  `%Function.prototype%` as `C`'s prototype, `super()` in null-extending
  classes throws `TypeError`, and `new (Sub.bind(...))` delegates construction
  to `Sub` with prepended bound arguments while ignoring bound `this`.
- **C-style `for` lexical head environments** — `for (let/const ...; ...; ...)`
  creates the loop-head lexical environment and per-iteration sibling
  environments required for initializer/test/body/update closures, rejects
  body `var` redeclarations of head lexical names, and parses `async of => {}`
  as an async-arrow initializer in ordinary `for` loops.
- **Label identifiers and strict labelled functions** — contextual `await`
  labels in non-module code and contextual `yield` labels in sloppy
  non-generator code parse correctly, including escaped spellings, while
  strict labelled function declarations are rejected during parsing.
- **Function statement-control parser boundaries and raw meta-property
  tokens** — nested function bodies reset loop/switch/label parse context,
  `async function` honors the no-LineTerminator rule, escaped `new.target`
  forms are rejected, and `debugger` is statement-only.
- **Named function-expression bindings** — named function expressions create
  an immutable inner name binding; sloppy assignments to that binding are
  ignored, while strict assignments throw `TypeError`.
- **Call-expression environment and argument ordering** — named function
  expression body `var` declarations can shadow the explicit function name
  binding, sloppy direct eval accepts `static` as a contextual `var` name, and
  member calls perform property lookup before argument evaluation.
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
- **Addition primitive coercion** — binary `+` now runs `ToPrimitive` before
  BigInt mixing checks, performs string concatenation before mixed-BigInt
  errors, and gives Date default-hint coercion the Date string-order behavior.
- **Contextual `of` division lexing** — `/` after contextual `of` remains
  division in expression contexts while raw `of` delimiters in `for...of`
  heads still allow a following regex literal.
- **Var initializer Reference resolution** — `var x = init` resolves the
  binding Reference before evaluating `init`, so `with` object references
  survive same-property mutations in the initializer, and global `var`
  bindings keep their global object property descriptors synchronized.
- **Arrow function early errors** — arrow functions reject duplicate
  parameter names in sloppy and strict mode, and reject `eval`/`arguments`
  parameter names when strict mode applies.
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
- **String literal line continuations** — a backslash followed by a
  LineTerminatorSequence contributes no cooked characters, including when the
  string is used as a computed object accessor name.
- **Direct eval lexical declaration conflicts** — sloppy direct eval rejects
  `var`/function declarations that would hoist over a caller `let`/`const`
  binding, including object method/accessor bodies.
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
- **Class heritage strictness and strict arguments objects** — class
  heritage expressions parse as strict code without making script-goal
  `await` class names illegal, and strict function calls create an unmapped
  `arguments` object with a restricted `callee` accessor.
- **Operator edge semantics** — BigInt exponentiation throws `RangeError` for
  negative exponents, BigInt relational comparison handles Boolean/nullish
  numeric operands through `ToNumeric`, `in` rejects primitive right-hand
  sides before property-key conversion, `instanceof` skips prototype lookup
  for primitive left-hand sides, and strict non-generator `yield` is a parse
  error.
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
- **Array destructuring assignment iterator semantics** — array assignment
  patterns now use the iterator protocol, evaluate member targets before
  `IteratorStep`, apply defaults, close unfinished iterators on abrupt default
  evaluation or target assignment while preserving the original throw, and
  read lazy iterator `done` before `value`. This reduces
  `language/expressions/assignment/destructuring` to **5 pass / 1 fail**.
- **Duplicate `__proto__` object assignment properties** — object assignment
  patterns now allow duplicate static `__proto__` colon properties while
  object literals still reject duplicate prototype-mutation entries. Plain
  destructuring assignment expressions also leave the RHS value on the stack as
  their result. This brings `language/expressions/assignment/destructuring` to
  **6 pass / 0 fail**.
- **Computed property names in `for` heads** — computed property names and
  computed member keys now allow `in` inside their bracketed expressions even
  when the surrounding expression is parsed under `for (... in ...)`
  lookahead. This reduces `language/expressions/object` to
  **271 pass / 14 fail**.
- **Object literal strict early errors** — strict object literal shorthand
  properties reject reserved IdentifierReferences such as `let` and `yield`,
  and object accessors/methods apply body `"use strict"` directives to
  formal-parameter `eval`/`arguments` checks. This reduces
  `language/expressions/object` to **275 pass / 10 fail**.
- **String literal line continuations** — a backslash followed by a
  LineTerminatorSequence contributes no cooked characters inside string
  literals, including computed object accessor names. This reduces
  `language/expressions/object` to **276 pass / 9 fail**.
- **Direct eval lexical declaration conflicts** — sloppy direct eval rejects
  `var`/function declarations that conflict with caller `let`/`const`
  bindings before leaking hoisted declarations to the caller variable
  environment. This reduces `language/expressions/object` to
  **279 pass / 6 fail** and raises the supported subset to
  **3960 pass / 218 fail / 2 timeout**.
- **Object method parameter/body environments** — parameter defaults and
  destructuring preludes now run before the separate body variable environment
  is pushed, so parameter closures do not see body `var` declarations while
  direct eval `var`s created during parameter evaluation remain visible to
  parameter and body closures. Nested function/method parameter scratch state
  is isolated in the parser. This brings `language/expressions/object` to
  **285 pass / 0 fail** and raises the supported subset to
  **3979 pass / 199 fail / 2 timeout**.
- **Arrow formal-parameter early errors** — arrow functions reject duplicate
  bound names introduced by destructuring parameters and reject a line
  terminator before `=>` for parenthesized and parenless forms. Async-arrow
  lookahead also preserves the no-LineTerminator restrictions around `async`
  and `=>`. This brings
  `language/expressions/arrow-function/syntax/early-errors` to
  **25 pass / 0 fail** and raises the supported subset to
  **3990 pass / 188 fail / 2 timeout**.
- **Sloppy arrow contextual parameters** — non-strict arrow functions allow
  `eval`, `arguments`, and `yield` as formal parameter names where the grammar
  permits them, while strict enclosing code or a block-body `"use strict"`
  directive still rejects `eval`/`arguments`. This brings
  `language/expressions/arrow-function/syntax` to **45 pass / 0 fail**,
  raises `language/expressions/arrow-function` to **88 pass / 2 fail**, and
  raises the supported subset to **3996 pass / 182 fail / 2 timeout**.
- **Arrow lexical `arguments`** — arrow function calls no longer create their
  own `arguments` object binding, so `arguments` references inside an arrow
  resolve through the captured lexical environment unless shadowed by an
  explicit parameter. This raises `language/expressions/arrow-function` to
  **89 pass / 1 fail** and the supported subset to
  **3997 pass / 181 fail / 2 timeout**.
- **Lexical arrow `super()` binding order** — `super()` calls now perform the
  superclass constructor call before rebinding the derived constructor's
  lexical `this` environment and forward the active constructor's
  `new.target`. A repeated `super()` call, including one captured in an arrow
  and invoked after the constructor returns, now throws `ReferenceError` only
  after the superclass constructor has run. This closes
  `language/expressions/arrow-function` at **90 pass / 0 fail** and raises the
  supported subset to **3999 pass / 179 fail / 2 timeout**.
- **`super()` constructor mixed spread arguments** — `super(...)` now lowers
  mixed spread and non-spread arguments through the same iterator-backed
  argument-array path used by ordinary calls and `new`. This preserves
  left-to-right evaluation, handles empty spreads, and reports unresolvable
  spread operands as `ReferenceError`. This raises
  `language/expressions/super` to **30 pass / 6 fail** and the supported
  subset to **4003 pass / 175 fail / 2 timeout**.
- **Lexical arrow `super` property parsing** — block-bodied arrow functions
  now preserve the enclosing method's `super` parse context instead of
  resetting it like ordinary function bodies. This allows `super.x` and
  `super["x"]` inside arrows nested in object methods while still rejecting
  `super` in arrows without an enclosing super binding. This raises
  `language/expressions/super` to **32 pass / 4 fail** and the supported
  subset to **4005 pass / 173 fail / 2 timeout**.
- **Direct eval lexical `super` parsing** — direct eval now inherits the
  caller's `super` parse context when the caller environment has a `#super`
  binding. This allows `eval("super.x")` and computed `super` property access
  inside object methods while preserving SyntaxError for eval code without an
  enclosing super binding. This raises `language/expressions/super` to
  **34 pass / 2 fail** and the supported subset to
  **4007 pass / 171 fail / 2 timeout**.
- **Computed `super[...]` putvalue evaluation order** — compound assignment
  and update expressions now evaluate a `super` property target by checking
  the derived constructor `this` binding before evaluating a computed property
  expression, then reuse the same receiver/base/key reference for the get and
  set. This closes `language/expressions/super` at **36 pass / 0 fail** and
  raises the supported subset to **4009 pass / 169 fail / 2 timeout**.
- **Nullish/logical chain early errors** — unparenthesized `??` mixed directly
  with `&&` or `||` now throws a parse-time `SyntaxError`, while
  parenthesized combinations still parse and evaluate. This closes
  `language/expressions/coalesce` at **22 pass / 0 fail** and raises the
  supported subset to **4013 pass / 165 fail / 2 timeout**.
- **BigInt `ToNumeric` operator semantics** — unary plus and unsigned right
  shift reject BigInt operands with `TypeError`, while BigInt-aware
  arithmetic, bitwise, and signed shift operations preserve BigInt results
  after `ToNumeric`, including boxed BigInts. `ToNumber` no longer silently
  converts BigInt except through `Number()`, and string numeric conversion no
  longer accepts incorrectly-cased Infinity spellings. This closes the BigInt
  failures in `bitwise-and`, `bitwise-or`, `bitwise-xor`, and
  `unsigned-right-shift`, reduces `unary-plus` to **0 failures**, and raises
  the supported subset to **4034 pass / 144 fail / 2 timeout**.
- **Native Error subclass construction** — `Error.prototype` now inherits
  `Object.prototype` during bootstrap, NativeError subclass instances no
  longer receive own `message` properties when the message argument is
  omitted, and `name` is inherited through the prototype chain so
  `class Err extends EvalError {}` instances report `EvalError`. This closes
  `language/statements/class/subclass/builtin-objects/NativeError` at
  **18 pass / 0 fail** and raises the supported subset to
  **4047 pass / 131 fail / 2 timeout**.
- **Class element grammar and named class expression scope** — class bodies
  now accept empty `;` elements, computed accessor names, and generator
  methods, and named class expressions create an inner immutable class-name
  binding instead of leaking the name to the outer scope. Class names now
  reject `yield` even in sloppy surrounding scripts. This closes
  `language/expressions/class` at **48 pass / 0 fail**, improves
  `language/statements/class/syntax` to **9 pass / 4 fail**, and raises the
  supported subset to **4061 pass / 117 fail / 2 timeout**.
- **Class declaration early errors** — script and block statement lists now
  reject duplicate lexical class declarations and lexical/`var` name clashes
  during parsing, and escaped `static` is no longer accepted as the class
  `static` modifier. This improves `language/statements/class/syntax` to
  **12 pass / 1 fail** and raises the supported subset to
  **4064 pass / 114 fail / 2 timeout**.
- **Class `super` property HomeObject setup** — class constructors and
  instance methods now bind `Class.prototype` as their SuperProperty
  HomeObject, while static methods, static accessors, and static blocks bind
  `Class`. SuperProperty evaluation reads the HomeObject prototype
  dynamically. This closes `language/statements/class/super` and
  `language/statements/class/syntax` at **21 pass / 0 fail** and raises the
  supported subset to **4069 pass / 109 fail / 2 timeout**.
- **Class definition/name-binding semantics** — class declarations now hoist as
  immutable lexical bindings, anonymous class assignment infers constructor
  display names, class bodies parse nested functions in strict context, and
  method/accessor display names no longer create body bindings that shadow
  outer variables. `extends` evaluates the superclass `prototype` getter
  exactly once and derived constructors return the `this` object bound by
  `super()` when no object is explicitly returned. This closes
  `language/statements/class/definition` and
  `language/statements/class/name-binding` at **41 pass / 0 fail** and raises
  the supported subset to **4080 pass / 98 fail / 2 timeout**.
- **Dynamic class `super` references** — `super` property reads, calls, simple
  assignments, updates, and compound assignments now derive the super base
  from the method HomeObject at evaluation time instead of using a stale
  class-definition-time prototype value. This follows later
  `Object.setPrototypeOf` changes, and simple `super.x = rhs` /
  `super[expr] = rhs` evaluates `rhs` before throwing `TypeError` when the
  dynamic super base is `null`. This closes `language/expressions/assignment`
  at **110 pass / 0 fail** and raises the supported subset to
  **4082 pass / 96 fail / 2 timeout**.
- **Null-extending classes and bound subclass construction** —
  `extends null` now uses the spec prototype parents and `super()` failure
  behavior, while bound class construction ignores bound `this` and delegates
  to the target constructor with bound arguments. This raises
  `language/statements/class/subclass` to **75 pass / 19 fail** and the
  supported subset to **4089 pass / 89 fail / 2 timeout**.
- **C-style `for` lexical head environments** — ordinary `for` loops now use
  runtime loop-head and per-iteration environments for lexical declarations,
  including destructuring heads and the update-before-next-iteration boundary.
  Parser early errors now reject body `var` redeclarations of head lexical
  names, while `async of => {}` remains a normal async-arrow initializer. This
  closes `language/statements/for` at **93 pass / 0 fail** and raises the
  supported subset to **4103 pass / 77 fail / 0 timeout**.
- **Label identifiers and strict labelled functions** — labelled statements
  now accept contextual `await` labels in non-module code and contextual
  `yield` labels in sloppy non-generator code, including escaped spellings.
  Strict labelled function declarations now fail during parsing instead of
  executing. This closes `language/statements/labeled` at **17 pass / 0 fail**
  and raises the supported subset to **4108 pass / 72 fail / 0 timeout**.
- **Function statement-control parser boundaries and raw meta-property
  tokens** — parser state for loops, switches, and labels no longer leaks into
  nested function bodies, so inner `break`/`continue` cannot target an outer
  function's labels. `async function` declarations and expressions now require
  no line terminator between the keywords, `new.target` requires raw `new` and
  `target` tokens, and `debugger` is a statement-only keyword. This closes
  `language/statements/break`, `language/statements/continue`,
  `language/statements/debugger`, `language/statements/async-function`, and
  `language/expressions/new.target` in the supported subset, raising it to
  **4114 pass / 66 fail / 0 timeout**.
- **Named function-expression bindings** — named function expressions now
  create an immutable inner name binding. Sloppy assignment, including through
  direct eval or a nested lexical arrow, is ignored; strict assignment throws
  `TypeError`. This closes `language/expressions/function` at
  **53 pass / 0 fail** and raises the supported subset to
  **4118 pass / 62 fail / 0 timeout**.
- **Call-expression environment and argument ordering** — explicit named
  function-expression bindings now live in the function closure environment
  rather than the call body's variable environment, so body `var` declarations
  with the same name create the required separate binding. Sloppy direct eval
  accepts `static` as a contextual `var` binding name, and member calls perform
  property lookup before argument evaluation while keeping the callability
  check after argument evaluation. This improves `language/expressions/call`
  to **48 pass / 1 fail** and raises the supported subset to
  **4121 pass / 59 fail / 0 timeout**.
- **Tagged-template call context and conditional `in` grammar** — tagged
  templates used as member expressions now preserve their receiver as `this`,
  ``new tag`...` `` constructs the tag result rather than the tag function
  itself, and constructor arguments after a tagged template are applied to that
  result. Conditional-expression true branches now allow `in` even inside
  no-`in` contexts such as `for` heads. This reduces
  `language/expressions/tagged-template` to the remaining cross-realm
  `$262.createRealm()` case, closes
  `language/expressions/conditional/in-branch-1.js`, and raises the supported
  subset to **4124 pass / 56 fail / 0 timeout**.
- **Boxed String methods and Date method surface** — String prototype methods
  now read the wrapped primitive from `new String(...)` objects, so indexed
  operations like `charAt` agree with boxed string index properties. The
  bootstrap also installs `String.prototype.length`, `Date.parse`, `Date.UTC`,
  and the ES5 Date prototype method surface needed for property-access checks.
  This closes `language/expressions/property-accessors` at **15 pass / 0
  fail** and raises the supported subset to **4127 pass / 53 fail / 0
  timeout**.
- **Switch CaseBlock scoping and redeclarations** — switch `var`
  declarations bind in the enclosing variable environment, function
  declarations in case bodies stay scoped to the CaseBlock, and switch
  redeclaration early errors treat function declarations as lexical names.
  This closes `language/statements/switch` at **69 pass / 0 fail** and raises
  the supported subset to **4133 pass / 47 fail / 0 timeout**.
- **Class heritage strictness and strict arguments objects** — class
  heritage expressions now parse under strict mode while preserving
  script-goal `await` class names, and strict function calls now create an
  unmapped `arguments` object whose `callee` accessor throws `TypeError`.
  This closes `language/statements/class/strict-mode` at **2 pass / 0 fail**
  and raises the supported subset to **4135 pass / 45 fail / 0 timeout**.
- **Operator edge semantics** — BigInt exponentiation now throws `RangeError`
  for negative exponents, BigInt relational comparisons now coerce
  Boolean/nullish numeric operands through `ToNumeric`, `in` now rejects
  primitive right-hand sides before property-key conversion, `instanceof` now
  returns `false` for primitive left-hand sides before reading `prototype`,
  and strict non-generator `yield` is rejected during parsing. This closes
  `language/expressions/exponentiation`, `greater-than`, `less-than`, `in`,
  and `instanceof` at **188 pass / 0 fail** and raises the supported subset
  to **4142 pass / 38 fail / 0 timeout**.
- **`with` `@@unscopables` HasBinding** — `Symbol.unscopables` is now exposed,
  `with` object environment records consult it only after `[[HasProperty]]`
  succeeds, ignore non-object unscopables values, propagate abrupt getters,
  and re-check deleted bindings for strict `GetBindingValue`/
  `SetMutableBinding`. This closes `language/statements/with` at **169 pass /
  0 fail / 12 skip**; the Reference-focused cluster
  `language/statements/with language/expressions/assignment
  language/expressions/prefix-increment
  language/expressions/prefix-decrement
  language/expressions/postfix-increment
  language/expressions/postfix-decrement` now runs at **409 pass / 0 fail /
  399 skip**, raising the supported subset to **4191 pass / 0 fail / 0
  timeout**.
- **Arrow lexical `new.target` and optional catch binding** — arrow closures
  now capture enclosing `new.target`, and the runner no longer skips
  `optional-catch-binding` or `new.target` feature tests. This closes the
  focused `language/statements/try language/expressions/new.target
  language/expressions/arrow-function` cluster at **204 pass / 0 fail / 354
  skip**, raising the supported subset to **4215 pass / 0 fail / 0 timeout**.
- **`for-in-order` enumeration** — `for...in`, JSON object serialization, and
  JSON reviver traversal now use array-index-first property order, and
  `Object.create(proto, descriptors)` applies descriptor maps so
  non-enumerable own properties shadow inherited enumerable keys. This lifts
  `for-in-order` from the skip filters at **9 pass / 0 fail**, raising the
  supported subset to **4219 pass / 0 fail / 0 timeout**.
- **Logical-assignment feature lift** — member logical assignments now check
  nullish bases before computed-key coercion, identifier logical assignments
  apply NamedEvaluation for anonymous RHS functions/classes, and
  `logical-assignment-operators` is removed from the skip filters at **57
  pass / 0 fail / 21 skip**, raising the supported subset to **4276 pass / 0
  fail / 0 timeout**.
- **`delete` through `with` object environments** — `delete x` now uses the
  object environment HasBinding path for `with` objects, including inherited
  properties, `Symbol.unscopables`, and abrupt unscopables getters. The
  supported subset remains at **4276 pass / 0 fail / 0 timeout** while closing
  this untracked Reference edge.
- **Direct eval through `with` object environments** — unqualified `eval(...)`
  now performs runtime callee resolution before direct-eval classification, so
  `with` object properties can shadow eval and abrupt eval getters propagate
  before arguments run. The supported subset remains at **4276 pass / 0 fail
  / 0 timeout** while closing this untracked Reference/eval edge.
- **Destructuring assignment feature lift** — object/array destructuring
  assignment patterns now reject escaped reserved words when the target would
  bind an identifier, including shorthand object assignment properties and
  destructuring arrow/function parameters. Escaped reserved words remain valid
  property names in renamed patterns such as `{ bre\u0061k: x }`. This removes
  `destructuring-assignment` from the skip filters at **135 pass / 0 fail / 6
  skip**, raising the supported subset to **4470 pass / 0 fail / 0 timeout**.
- **Class feature lift** — class numeric method/accessor names now use
  JavaScript number-to-string canonicalization, `static constructor()` parses
  as an ordinary static method, class-element early errors reject duplicate
  constructors, constructor accessors, static `prototype` definitions, and
  unparenthesized arrow-function heritage, and `super()` dynamically reads the
  active constructor's current `[[Prototype]]`. The not-a-constructor check now
  runs after argument evaluation for direct and spread super constructor calls.
  This removes `class` from the skip filters at **522 pass / 0 fail / 7904
  skip**, raising the supported subset to **4741 pass / 0 fail / 0 timeout**.
- **ES2015 syntax/global feature lift** — `computed-property-names`,
  `rest-parameters`, `object-spread`, and `globalThis` are removed from the
  skip filters after verifying 0 failures in the supported subset. Focused
  verification covered computed/object tests at **370 pass / 0 fail / 848
  skip**, call/new/array/super spread tests at **217 pass / 0 fail / 80
  skip**, and class/function/arrow tests at **1117 pass / 0 fail / 8367
  skip**, raising the supported subset to **5000 pass / 0 fail / 0 timeout**.
- **`super`/`for-of` feature lift** — method parameter default initializers
  now preserve the enclosing method's `super` property parse context while
  still rejecting direct `super()` calls, and non-declaration
  `for ([x] of iterable)` / `for ({x} of iterable)` heads now route through
  destructuring assignment instead of discarding the iterator value. `super`
  and `for-of` are removed from the skip filters; focused verification over
  `language/statements/for-of` and object method definitions runs at
  **134 pass / 0 fail / 920 skip**, raising the supported subset to
  **5003 pass / 0 fail / 0 timeout**.
- **`typeof` through `with` object environments** — `typeof identifier` now
  reuses the VM's spec Reference-record resolution path before applying
  `GetValue`, except for the required unresolvable-reference `"undefined"`
  case. This means `with` object properties, inherited properties,
  `Symbol.unscopables`, abrupt unscopables getters, and TDZ bindings are
  observed consistently with ordinary identifier reads. The focused
  `language/statements/with` run stays at **169 pass / 0 fail / 12 skip**,
  and the broader Reference-focused cluster runs at **900 pass / 0 fail / 367
  skip** while the supported subset remains at **4470 pass / 0 fail / 0
  timeout**.
- **Identifier writes through destructuring and `for-in`/`for-of` heads** —
  destructuring-assignment identifier targets and non-declaration
  `for-in`/`for-of` identifier heads now store through the same spec
  Reference-record path as ordinary assignment. This fixes `with`
  object-environment writes where inherited properties should receive the
  assignment and `Symbol.unscopables` should fall through to an outer binding.
  The focused `language/statements/with` run stays at **169 pass / 0 fail / 12
  skip**, the broader Reference-focused cluster stays at **900 pass / 0 fail /
  367 skip**, and the supported subset remains at **4470 pass / 0 fail / 0
  timeout**.

## Why the full-suite rate is not higher

The supported subset currently has no known failures. The full-suite rate is
still much lower because the full matrix includes unsupported features such as
ES Modules, Intl, Atomics, most TypedArray variants, WeakRef, and
FinalizationRegistry. Those larger feature areas are tracked in `HANDOFF.md`
and will be pulled into support in later milestones.
