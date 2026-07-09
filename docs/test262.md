# test262 conformance

RuJa runs the [test262](https://github.com/tc39/test262) conformance suite
via `tools/test262_runner.py`. The runner uses the **real test262 harness**
(`sta.js`, `assert.js`, and per-test `includes:` such as `propertyHelper.js`
and `compareArray.js`) rather than a hand-rolled stub, so tests relying on
`verifyProperty`, `compareArray`, etc. are exercised correctly. It also
parses `negative:` metadata so a test that expects a `SyntaxError`/
`TypeError` (parse or runtime phase) passes when RuJa raises the matching
error, honors `flags: [raw]` by running those files without any harness
prelude, and keeps narrow path-scoped exceptions for feature-tagged coverage
that RuJa already supports.

RuJa does **not** claim full ES conformance. Instead, it targets a
deliberately scoped subset of ES5.1 + selected ES2015+ features (see
[Supported subset](#supported-subset) below). Tests requiring unsupported
features (modules, TypedArrays, Atomics, Intl, etc.) are skipped via the
runner's `SKIP_FEATURES` set. The `explicit-resource-management` feature is
still skipped for syntax/runtime coverage, with a narrow exception for the
already-supported `Symbol.dispose` and `Symbol.asyncDispose` intrinsics.

## Three pass-rate scopes

There are three distinct pass-rate numbers. Each measures a different
scope, so they are not comparable to each other:

| Scope | What it measures | Current rate | Where to verify |
|-------|-----------------|-------------|-----------------|
| **Full suite** | `test262-full` workflow matrix — includes thousands of tests for features RuJa does not support | 35.9% of all matrix files; 70.9% of executed files in the latest confirmed full run | `test262-full` CI workflow job summary |
| **Supported subset** | `language/statements` + `language/expressions` — the areas RuJa actively targets, with unsupported-feature tests skipped | 100.0% (5947 pass / 0 fail) | Run locally: `TEST262=… python3 tools/test262_runner.py language/statements language/expressions` |
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
Symbol.hasInstance, Symbol.unscopables, Symbol.dispose,
Symbol.asyncDispose, Map/Set/WeakMap/WeakSet, BigInt, Proxy, Reflect,
Promise, async/await, generators, for-of, optional chaining, nullish
coalescing, logical assignment.

**Intentionally unsupported**: ES Modules (import/export), Intl, Atomics,
SharedArrayBuffer, full TypedArray prototype method coverage beyond the
constructor/index basics and ArrayBuffer/DataView support,
WeakRef/FinalizationRegistry, Tail-call optimization.
Explicit resource management syntax (`using` / `await using`) is not yet
supported beyond the two well-known Symbol intrinsics.

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

The `test262-full` CI workflow runs the sharded test262 matrix in parallel,
excluding `harness`, `intl402`, and `staging`, and expanding `language/*` so the
top-level `language` directory is not re-run. Counts can vary slightly because
a small number of tests can cross the timeout boundary. Baseline confirmation
runs: `test262-full` 28907207537 on `2385b02`, 28907533009 on `2c6c08f`,
28908649991 on `40d9102`, 28909686737 on `605ed5e`, and 28910870657 on
`6d0f28a`; latest improvement confirmation: `test262-full` 28913345658 on
`4cfd15e`, 28913946764 on `2be895e`, 28914775928 on `cb3ea6f`,
28915947538 on `3b32bdf`, 28917214580 on `f6659da`, and 28918240994 on
`2c63328`; latest baseline refresh: `test262-full` 28918450090 on `e27b922`;
latest improvement confirmation: `test262-full` 28919444373 on `031e3c1`
and 28920345686 on `cde35de`, 28921293972 on `e3d11d3`, 28922341028 on
`28601c4`, 28922677138 on `a2425ab`, 28923518987 on `016a395`, and
28924515143 on `3e939c8`, 28926073317 on `12b94b9`, and 28927197662 on
`22973c3`, 28928286572 on `3cfdc6f`, 28930003448 on `6291e54`, and
28931360498 on `33f4d61`; latest baseline refresh: `test262-full`
28928721861 on `142d979` and 28931773376 on `bed25e4`.
Latest full baseline documentation check: `test262-full` 28932188774 on
`d9c9a1c`; latest improvement confirmation: `test262-full` 28932874097 on
`397d164`, 28935851288 on `5768889`, and 28937035685 on `9d7fe40`;
latest full baseline documentation check: `test262-full` 28937391393 on
`2e3bf0a`; latest improvement confirmation: `test262-full` 28947656670 on
`70aede1`, 28954804300 on `9ecf2e2`, 28956369778 on `220b6de`,
28960087423 on `500fd9a`, and 28961595128 on `2625301`.
Latest improvement confirmation: `test262-full` 28962802017 on `6d6328e`.
Latest improvement confirmation: `test262-full` 28964634961 on `05173b6`.
Latest improvement confirmation: `test262-full` 29010097608 on `7ac8ba5`.

Focused generator assignment destructuring local check:
generator assignment destructuring now treats `yield]` as a bare
`YieldExpression` before the closing array pattern bracket, so default
initializers and rest-target computed keys can suspend and resume in generator
assignment patterns. Generator shorthand `{ yield }` assignment targets are
now rejected as syntax errors. Array assignment destructuring now preserves
iterator-close errors when a suspended generator is resumed through `return()`,
while still preserving original throw completions, and lazy custom iterator
`next` validation is delayed until `IteratorNext` so target reference
evaluation can suspend before a missing `next` is observed. The runner now
admits generator coverage on the whole `language/expressions/assignment/`
path. The focused assignment path reports **485 pass / 0 fail / 0 skip**, the
broader Reference-adjacent cluster reports **1198 pass / 0 fail / 0 skip**,
and the supported subset rises to **5947 pass / 0 fail / 14491 skip**.

Focused public class fields local check:
`language/{statements,expressions}/class/elements` now parses public instance
and static fields, including computed names and field names such as `static`,
`get`, `set`, and `async` that are not method prefixes. Public field
initializers define own data properties instead of invoking inherited setters,
and fields without initializers are created with `undefined`. With
`class-fields-public` and `class-static-fields-public` temporarily lifted, the
focused diagnostic reports **309 pass / 113 fail / 2540 skip**. The default
supported subset remains **5099 pass / 0 fail / 15339 skip** because full
public-field coverage still needs direct-eval, computed-name ordering, and
static-initializer follow-ups before the runner can lift those features.

Focused public class field `[[DefineOwnProperty]]` local check:
public field initialization now uses `CreateDataPropertyOrThrow` semantics
instead of raw property-map insertion. A later field fails with `TypeError`
when an earlier initializer freezes the receiver, and derived-class field
initializers invoke Proxy `defineProperty` traps when `super()` returns a Proxy
receiver. With public class field and Proxy skips temporarily lifted,
`class-field-on-frozen-objects.js` and
`public-class-field-initialization-is-visible-to-proxy.js` now report **2 pass
/ 0 fail / 0 skip**. The broader
`language/{statements,expressions}/class/elements` diagnostic with public,
private, and Proxy gates temporarily lifted reports **1460 pass / 184 fail /
1318 skip**. The default supported subset remains **5099 pass / 0 fail /
15339 skip**.

Focused static class field initializer `this` local check:
static public and private field initializers now run with `this` bound to the
class constructor, matching `DefineField(receiver, fieldRecord)` for direct
`this` reads and arrows created inside the initializer. With static
public/private class field skips temporarily lifted,
`static-field-init-with-this.js` and
`static-field-init-this-inside-arrow-function.js` in both statement and
expression class-element directories report **4 pass / 0 fail / 0 skip**. The
`language/statements/class/elements` diagnostic improves from **466 pass / 90
fail / 978 skip** to **469 pass / 87 fail / 978 skip**. The default supported
subset remains **5099 pass / 0 fail / 15339 skip**.

Focused property Reference member-compound local check:
ordinary member compound assignments now create an explicit property Reference
before `GetValue` and preserve that Reference through `PutValue`, matching the
identifier compound-assignment path. This keeps Symbol property keys intact,
does not re-coerce computed keys, passes Proxy `set` traps the original
receiver, and uses the Reference's strict flag when writes fail. The focused
`language/expressions/compound-assignment` run now closes at **454 pass / 0
fail / 0 skip** after the runner admits implemented private-field Reference
coverage on that path.

Focused property Reference member-update local check:
ordinary member update expressions now use the same explicit property
Reference path before applying numeric increment/decrement and `PutValue`.
This keeps prefix/postfix result values stable while preserving Symbol keys,
single computed-key coercion, Proxy receiver identity, and strict failed-write
behavior. The focused update-expression cluster
`language/expressions/{update,prefix-increment,postfix-increment,prefix-decrement,postfix-decrement}`
reports **138 pass / 0 fail / 4 skip**.

Focused property Reference member-logical-assignment local check:
ordinary member logical assignments now carry a property Reference across
`GetValue`, short-circuit testing, and `PutValue`. This keeps skipped-write
expression results stable while preserving Symbol keys, single computed-key
coercion, Proxy receiver identity, strict failed-write behavior, and nullish
base ordering. The focused `language/expressions/logical-assignment` run
now closes at **78 pass / 0 fail / 0 skip** after the runner admits
implemented private-field Reference coverage on that path.

Focused property Reference simple member-assignment local check:
ordinary simple member assignments now create a property Reference after the
RHS has been evaluated, then complete the write through `PutValue`. This keeps
simple-assignment ordering stable: computed key expressions run before the
RHS, nullish-base failure happens after the RHS, and `ToPropertyKey` is still
delayed until after the RHS. The same path preserves Symbol keys, Proxy
receiver identity, strict failed-write behavior, and primitive sloppy no-op
semantics. The focused
`language/expressions/{assignment,member-expression}` run reports **204 pass /
0 fail / 282 skip**.

Focused property Reference destructuring member-target local check:
member targets nested inside destructuring assignment patterns now store
through `MakePropertyRefForSet` and `PutValue` instead of the legacy
`SetElem`/`SetProp` opcodes. This keeps `({ a: obj[key] } = rhs)` and
`[obj[key]] = rhs` aligned with ordinary member assignment for Symbol keys
returned from `@@toPrimitive`, Proxy receiver identity, strict failed-write
behavior, and primitive sloppy no-op semantics. The focused
`language/expressions/assignment/dstr` run remains closed at **90 pass / 0
fail / 278 skip**; `language/expressions/{assignment,destructuring}` reports
**203 pass / 0 fail / 282 skip**.

Focused property Reference `for-in`/`for-of` member-head local check:
non-declaration loop heads now store member targets through
`MakePropertyRefForSet` and `PutValue`. This keeps `for (obj[key] in source)`
and `for (obj[key] of iterable)` aligned with ordinary member assignment for
Symbol keys and Proxy `set` receiver identity. The focused
`language/statements/{for-in,for-of}` run reports **191 pass / 0 fail / 675
skip**; adding `language/expressions/member-expression` reports **192 pass / 0
fail / 675 skip**.

Focused object rest destructuring assignment-target local check:
object rest assignment patterns now validate the `PropertyKey::Spread` target
itself and compile rest targets through the destructuring assignment target
path. This accepts `({ a, ...holder.rest } = rhs)` and keeps member rest
targets aligned with `PutValue`/Proxy receiver semantics. The focused
`language/expressions/assignment/dstr` run remains closed at **90 pass / 0
fail / 278 skip**; `language/expressions/{assignment,destructuring}` reports
**203 pass / 0 fail / 282 skip**.

Focused object rest computed-key exclusion local check:
object rest destructuring now stores computed property keys after
`ToPropertyKey`, excludes those keys from the rest object, and copies remaining
enumerable Symbol properties. This keeps string and Symbol forms of
`({ [key]: v, ...rest } = rhs)` aligned with object rest semantics. The focused
`language/expressions/assignment/dstr` run remains closed at **90 pass / 0
fail / 278 skip**; `language/expressions/{assignment,destructuring}` reports
**203 pass / 0 fail / 282 skip**.

Focused class field anonymous function-name local check:
public and private class field initializers now apply `SetFunctionName` to
anonymous function, arrow, and class initializer values before defining the
field. This gives field initializer functions names derived from public field
keys and private names such as `#field`. With static public/private class field
skips temporarily lifted, the
`language/{statements,expressions}/class/elements/static-field-anonymous-function-name.js`
cluster reports **2 pass / 0 fail / 0 skip**. With class field, private method,
Proxy, Reflect, and Symbol skips temporarily lifted, the broader
`language/{statements,expressions}/class/elements` diagnostic now reports
**1559 pass / 105 fail / 1298 skip** after the subsequent field early-error
fixes.

Focused class field `ContainsArguments` early-error local check:
public and private class field initializers now raise a parse-time
`SyntaxError` when their initializer contains `arguments`. The check follows
lexical arrow boundaries, so `() => arguments` is rejected, while ordinary
function expressions keep their own `arguments` binding. With public/private
class field, static field, computed-name, and arrow gates temporarily lifted,
the generated
`language/{statements,expressions}/class/elements/*init-err-contains-arguments.js`
cluster reports **60 pass / 0 fail / 0 skip**.

Focused class field `constructor` PropName early-error local check:
public instance and static fields whose non-computed literal PropName is
`constructor` now raise a parse-time `SyntaxError`. Computed fields such as
`["constructor"]` still evaluate and define ordinary data properties, matching
the spec's empty PropName for computed names. With public/static class field and
computed-name gates temporarily lifted, the focused constructor-PropName
class-elements cluster reports **11 pass / 0 fail / 0 skip**.

Focused public class field computed-name local check:
public instance and static field computed names now evaluate once during class
definition and are stored as field keys for later `DefineField` execution.
Instance field keys are no longer re-evaluated for each construction, while
static and instance public field keys are evaluated in field declaration order.
With public/static class field and computed-name gates temporarily lifted, the
focused incremental/intercalated/error computed-name class-elements cluster
reports **12 pass / 0 fail / 0 skip**. With public/private class field,
private method, static block, Proxy, Reflect, Symbol, Symbol.iterator, computed
name, and arrow-function gates temporarily lifted, the broader
`language/{statements,expressions}/class/elements` diagnostic now reports
**1582 pass / 82 fail / 1298 skip**. Remaining failures still include full
ordered class-element evaluation across computed methods and static blocks.

Focused ordered class element local check:
class parsing now records source order across methods, fields, and static
blocks, while retaining the existing per-kind vectors as indexed storage. Class
compilation uses that ordered list for public computed method names, public
field computed names, and static element initialization. This fixes
field-before-method computed-name ordering, computed method names that read the
inner class binding, and static block/static field initializer ordering after
all element names have been evaluated. With public/static class field,
computed-name, logical-assignment, and exponentiation gates temporarily lifted,
the generated `cpn-class-*-fields-methods-*` cluster reports **60 pass / 0
fail / 2 skip**. With the broader class element gates temporarily lifted, the
`language/{statements,expressions}/class/elements` diagnostic now reports
**1583 pass / 81 fail / 1298 skip**.

Focused TypedArray `[[HasProperty]]` prototype-delegation local check:
ordinary property keys missing from a TypedArray now delegate to the
prototype's actual `[[HasProperty]]` operation instead of walking the chain with
raw own-property checks. Canonical numeric keys still use Integer-Indexed
exotic semantics, while ordinary keys now propagate Proxy `has` traps from
TypedArray prototype chains. With the Proxy gate temporarily lifted,
`built-ins/TypedArrayConstructors/internals/HasProperty` reports **26 pass / 0
fail / 6 skip**.

Focused private element duplicate-initialization local check:
private fields, methods, and accessors now throw `TypeError` instead of
overwriting an existing same-class private slot when a derived constructor
returns the same object across multiple constructions. With private class
feature skips temporarily lifted, the
`language/statements/class/elements/private-method-double-initialisation*.js`
and `privatefieldadd-typeerror.js` cluster reports **5 pass / 0 fail / 0
skip**. The default supported subset remains **5099 pass / 0 fail / 15339
skip**.

Focused ArrayBuffer transfer/immutable local check: `built-ins/ArrayBuffer`
improves from **57 pass / 34 fail / 130 skip** to **91 pass / 0 fail / 130
skip** after adding fixed-buffer `transfer`, `transferToFixedLength`,
`transferToImmutable`, `sliceToImmutable`, and the `immutable` accessor, then
pinning interpreted return values across VM GC safe points so
`sliceToImmutable` argument coercion keeps freshly returned buffers alive.

Focused ArrayBuffer `@@toStringTag` local check: `%ArrayBuffer.prototype%` now
exposes `Symbol.toStringTag` as a non-writable, non-enumerable, configurable
data property with value `"ArrayBuffer"`. The focused
`built-ins/ArrayBuffer/prototype` run now reports **72 pass / 0 fail / 100
skip**, and the broader `built-ins/ArrayBuffer` run closes at **92 pass / 0
fail / 129 skip**.

Latest improvement confirmation: `test262-full` 28965977305 on `576ba07`.
Latest improvement confirmation: `test262-full` 28966918564 on `2256c6a`.
Latest full baseline documentation check: `test262-full` 28967365155 on
`a6964b7`.
Latest improvement confirmation: `test262-full` 28968585053 on `135b01b`.
Latest improvement confirmation: `test262-full` 28969770053 on `67c9f2b`.
Latest improvement confirmation: `test262-full` 28970908797 on `0b0528f`.
Latest improvement confirmation: `test262-full` 28972311361 on `a1e44db`.
Latest improvement confirmation: `test262-full` 28973435387 on `1394ad3`.
Latest improvement confirmation: `test262-full` 28975444046 on `dc133c1`.
Latest improvement confirmation: `test262-full` 28977306579 on `2c6f617`.

Focused destructuring assignment IteratorClose local check:
`language/expressions/assignment/dstr` closes at **90 pass / 0 fail / 278
skip** after array assignment patterns close unfinished iterators on normal
partial completion, evaluate rest assignment target references before draining
rest values, and close on abrupt rest-target or rest-iterator completion. The
broader Reference-adjacent cluster
`language/expressions/assignment language/expressions/compound-assignment
language/expressions/logical-assignment language/expressions/update
language/statements/with` remained **835 pass / 0 fail / 363 skip** before the
subsequent `with` Proxy/Reflect admission.

Focused `with` runner admission local check:
`language/statements/with` now admits all implemented object-environment
binding coverage without opening Proxy, Reflect, TypedArray, generator, async
function, or async-iteration coverage outside that path. This includes
TypedArray prototype-chain binding deletion and async/generator declaration
parse-negative files. The path reports **181 pass / 0 fail / 0 skip**. The
broader Reference-adjacent cluster
`language/expressions/assignment language/expressions/compound-assignment
language/expressions/logical-assignment language/expressions/update
language/statements/with` now reports **847 pass / 0 fail / 351 skip**.

Focused assignment destructuring runner admission local check:
`language/expressions/assignment` now admits the implemented destructuring
assignment and object rest coverage by lifting only the
`destructuring-binding`, `object-rest`, `optional-chaining`, Symbol,
`Symbol.iterator`, and Proxy
feature gates on that path. Object rest assignment now rejects rest elements
followed by another assignment property, and rest-copy boxes primitive sources
so string indices are copied into the rest object. Optional chains now reject
direct and chained assignment/update/destructuring targets while preserving
parenthesized member targets such as `(a?.b).c = 1`. Array assignment patterns
now avoid `IteratorClose` when iterator stepping itself throws while still
closing for target/default abrupt completions. Generator coverage remains
behind its broader feature gate because that lift still exposes separate parser
work. The assignment path reports **453 pass / 0 fail / 32 skip**.

Focused private-field Reference runner admission local check:
`language/expressions/compound-assignment` and
`language/expressions/logical-assignment` now admit implemented
private-field Reference coverage by lifting only `class-fields-private` on
those paths. The combined run reports **532 pass / 0 fail / 0 skip**. The
broader Reference-adjacent cluster
`language/expressions/assignment language/expressions/compound-assignment
language/expressions/logical-assignment language/expressions/update
language/statements/with` now reports **1166 pass / 0 fail / 32 skip**.

Focused for-of iterator protocol local check:
`language/statements/for-of` now reports **598 pass / 0 fail / 153 skip**
after for-of caches iterator `next` at `GetIterator` time, rejects non-object
iterator results, applies `ToBoolean` to `done`, validates `return()` results
during `IteratorClose`, preserves the original throw when close also throws,
keeps hidden iterator-close state alive across labeled `continue`, and runs
generator `finally` blocks when generator-backed loops exit through `break`,
`continue`, `return`, or `throw`. The runner now admits implemented
destructuring-binding, object-rest, optional-chaining, Proxy,
`Symbol.iterator`, and the four generator-close files on this path while
leaving broader generator-tagged cases behind their broader gate.

Focused TypedArray static `from`/`of` local check:
`built-ins/TypedArrayConstructors/{from,from/BigInt,of,of/BigInt}` closes at
**126 pass / 0 fail / 0 skip** after concrete TypedArray constructors inherit
the `%TypedArray%` static methods, array-like sources construct results before
element reads, iterable sources cache their `next` method, mapper calls receive
the expected arguments/receiver, and immutable ArrayBuffer-backed results are
rejected before value conversion. With TypedArray-related skips temporarily
lifted, the broader `built-ins/TypedArrayConstructors` diagnostic now reports
**473 pass / 54 fail / 211 skip**.

Focused TypedArray integer-indexed `[[Delete]]` local check:
canonical numeric index deletes now follow Integer-Indexed exotic semantics for
numeric and BigInt TypedArrays. Valid in-bounds indexes return `false`, strict
delete of those indexes throws through the delete operator, detached buffers and
invalid canonical numeric keys such as `"-0"`, fractional, negative, infinite,
and out-of-bounds indexes return `true`, and non-canonical keys continue through
ordinary delete. With TypedArray-related skips temporarily lifted,
`built-ins/TypedArrayConstructors/internals/Delete` now reports **29 pass / 2
fail / 8 skip**; the remaining failures are detached-buffer realm constructor
coverage. The current runner's broader `built-ins/TypedArrayConstructors`
diagnostic moves from **419 pass / 54 fail / 265 skip** to **431 pass / 42 fail
/ 265 skip**.

Focused TypedArray integer-indexed `[[GetOwnProperty]]` local check:
valid canonical numeric index strings now synthesize descriptors from
TypedArray element storage with writable, enumerable, and configurable all set
to `true`. Detached buffers and invalid canonical numeric keys such as `"-0"`,
fractional, negative, infinite, and out-of-bounds indexes report no descriptor
without falling through to ordinary properties, while non-canonical keys such
as `"+1"` remain ordinary. The same descriptor lookup now feeds Proxy `has` and
`deleteProperty` invariants for non-extensible TypedArray targets. With
TypedArray-related skips temporarily lifted,
`built-ins/TypedArrayConstructors/internals/GetOwnProperty` now reports **18
pass / 2 fail / 4 skip**; the remaining failures are detached-buffer realm
constructor coverage. The current runner's broader
`built-ins/TypedArrayConstructors` diagnostic reports **429 pass / 44 fail /
265 skip**.

Focused TypedArray integer-indexed `[[Get]]` local check:
canonical numeric index property reads now use Integer-Indexed exotic element
access instead of Rust integer parsing or ordinary prototype lookup. Valid
indexes read numeric and BigInt elements from owned or ArrayBuffer-backed
storage, detached buffers and invalid canonical numeric keys such as `"-0"`,
fractional, negative, infinite, and out-of-bounds indexes return `undefined`
without touching inherited accessors, and non-canonical numeric-looking keys
such as `"+1"` continue through ordinary own/prototype lookup. With
TypedArray-related skips temporarily lifted,
`built-ins/TypedArrayConstructors/internals/Get` now reports **20 pass / 2 fail
/ 6 skip**; the remaining failures are detached-buffer realm constructor
coverage. The current runner's broader `built-ins/TypedArrayConstructors`
diagnostic improves to **437 pass / 36 fail / 265 skip**.

Focused TypedArray integer-indexed `[[DefineOwnProperty]]` local check:
canonical numeric index property definitions now follow Integer-Indexed exotic
validation. Invalid or detached indexes reject, accessor descriptors and
descriptors requesting non-configurable, non-enumerable, or non-writable
attributes reject, valid value descriptors write through element conversion for
numeric and BigInt arrays, and non-canonical numeric-looking keys remain
ordinary properties. With TypedArray-related skips temporarily lifted,
`built-ins/TypedArrayConstructors/internals/DefineOwnProperty` now reports
**16 pass / 2 fail / 36 skip**; the remaining failures are detached-buffer
realm constructor coverage. The current runner's broader
`built-ins/TypedArrayConstructors` diagnostic improves to **453 pass / 20 fail
/ 265 skip**.

Focused TypedArray integer-indexed `[[Set]]` local check:
numeric index assignments now run `ToNumber`/`ToBigInt` element conversion
before detached-buffer, out-of-bounds, invalid-index, or immutable-buffer
validation, so observable conversion side effects and abrupt completions are
preserved even when the write has no effect. With TypedArray-related skips
temporarily lifted, `built-ins/TypedArrayConstructors/internals/Set` improves
from **15 pass / 8 fail / 30 skip** to **21 pass / 2 fail / 30 skip**; the
remaining failures are detached-buffer realm constructor coverage. The current
runner's broader `built-ins/TypedArrayConstructors` diagnostic improves to
**459 pass / 14 fail / 265 skip**.

Focused TypedArray ArrayBuffer constructor-ordering local check:
ArrayBuffer-backed TypedArray construction now performs `byteOffset` ToIndex,
byte-offset alignment, and explicit `length` ToIndex before rechecking whether
the backing buffer was detached and reading its current byte length. This
rejects buffers detached during offset/length conversion instead of creating a
view from stale length data. With TypedArray-related skips temporarily lifted,
`built-ins/TypedArrayConstructors/ctors/buffer-arg
built-ins/TypedArrayConstructors/ctors-bigint/buffer-arg` now reports **44 pass
/ 0 fail / 62 skip**. The current runner's broader
`built-ins/TypedArrayConstructors` diagnostic improves to **463 pass / 10 fail
/ 265 skip**; the remaining failures are detached-buffer realm constructor
coverage.

Focused TypedArray cross-realm constructor local check:
`$262.createRealm()` now installs realm-local `ArrayBuffer`, `DataView`, a
hidden `%TypedArray%` intrinsic constructor/prototype pair, and all concrete
TypedArray constructors. This makes `other[TA.name]` constructable in test262
realm tests while keeping concrete constructors linked to that realm's
`%TypedArray%` and concrete prototypes linked to that realm's
`%TypedArray%.prototype`. With TypedArray-related skips temporarily lifted, the
broader `built-ins/TypedArrayConstructors` diagnostic now reports **473 pass /
0 fail / 265 skip**.

Focused constructor-realm prototype fallback local check:
`GetPrototypeFromConstructor` fallback now derives the default intrinsic
prototype from the active `newTarget` function realm when `.prototype` is not
an object. This closes cross-realm `Reflect.construct()` fallback coverage for
TypedArrays, ArrayBuffers, DataViews, and RegExps without changing the
observable `.prototype` lookup ordering. With the relevant TypedArray,
ArrayBuffer, DataView, Reflect, and `Reflect.construct` skips temporarily
lifted for the targeted files, `proto-from-ctor-realm` checks now report **13
pass / 0 fail / 0 skip**.

Focused Error-family constructor-realm fallback local check:
Error, NativeError, and `AggregateError` construction now applies
`GetPrototypeFromConstructor(newTarget, "%<ErrorName>.prototype%")` when
`newTarget.prototype` is not an object, instead of reusing the preallocated
ordinary-object fallback prototype. This keeps primitive `newTarget.prototype`
fallbacks on `%Error.prototype%`, `%TypeError.prototype%`, or
`%AggregateError.prototype%` from the `newTarget` realm. The focused
`built-ins/{Error,NativeErrors,AggregateError}` run reports **199 pass / 0 fail
/ 13 skip**, and `built-ins/AggregateError` now closes at **25 pass / 0 fail /
0 skip**.

Focused TypedArray integer-indexed `[[HasProperty]]` local check:
canonical numeric index property checks now return `true` only for valid
in-bounds TypedArray indexes and return `false` for detached buffers,
out-of-bounds indexes, `"-0"`, fractional, negative, and infinite canonical
numeric strings without consulting ordinary prototype properties. Non-canonical
keys still use ordinary lookup. With TypedArray, ArrayBuffer, DataView,
Reflect, and `Reflect.construct` skips temporarily lifted,
`built-ins/TypedArrayConstructors/internals/HasProperty` improves from **14
pass / 10 fail / 8 skip** to **22 pass / 2 fail / 8 skip**; the remaining
failures are the missing `%TypedArray%.prototype.subarray` method. Under the
same expanded diagnostic, broader `built-ins/TypedArrayConstructors` reports
**584 pass / 9 fail / 145 skip**.

Focused TypedArray `subarray()` local check:
`%TypedArray%.prototype.subarray` now lives on the shared intrinsic
`%TypedArray%.prototype`, creates offset views over the same ArrayBuffer,
normalizes begin/end bounds, rejects detached buffers, and uses `@@species`
while preserving Number-vs-BigInt content type. Concrete typed-array
prototypes still do not gain own `subarray` properties. With BigInt and
TypedArray skips lifted for the focused prototype tests,
`built-ins/TypedArrayConstructors/prototype/subarray` now reports **2 pass / 0
fail / 0 skip**. With TypedArray, ArrayBuffer, DataView, Reflect, and
`Reflect.construct` skips temporarily lifted,
`built-ins/TypedArrayConstructors/internals/HasProperty` improves from **22
pass / 2 fail / 8 skip** to **24 pass / 0 fail / 8 skip**. Under the same
expanded diagnostic, broader `built-ins/TypedArrayConstructors` reports
**586 pass / 7 fail / 145 skip**; the remaining failures are concentrated in
Integer-Indexed `[[OwnPropertyKeys]]` ordering, `Reflect.set` receiver writes,
and one typed-array-argument validation ordering case.

Focused TypedArray integer-indexed `[[OwnPropertyKeys]]` local check:
own-key enumeration now synthesizes attached TypedArray integer index keys
before ordinary string and symbol keys, including offset `subarray()` views,
while detached buffers expose no integer-indexed own keys. With TypedArray,
ArrayBuffer, DataView, Reflect, and `Reflect.construct` skips temporarily
lifted, `built-ins/TypedArrayConstructors/internals/OwnPropertyKeys` improves
from **0 pass / 4 fail / 6 skip** to **4 pass / 0 fail / 6 skip**. With
Symbol also lifted, the same focused path reports **8 pass / 0 fail / 2
skip**. Under the same expanded diagnostic, broader
`built-ins/TypedArrayConstructors` reports **590 pass / 3 fail / 145 skip**;
the remaining failures are now `Reflect.set` receiver writes and one
typed-array-argument validation ordering case.

Focused TypedArray receiver-aware integer-indexed `[[Set]]` local check:
`Reflect.set(target, index, value, receiver)` now writes valid integer-indexed
assignments through the receiver. Plain-object receivers receive ordinary data
properties without value coercion; TypedArray receivers perform their own
integer-index validation and element conversion, and invalid receiver indexes
fail before coercion. With TypedArray, ArrayBuffer, DataView, Reflect, and
`Reflect.construct` skips temporarily lifted,
`built-ins/TypedArrayConstructors/internals/Set` improves from **41 pass / 2
fail / 10 skip** to **43 pass / 0 fail / 10 skip**. Under the same expanded
diagnostic, broader `built-ins/TypedArrayConstructors` reports **592 pass / 1
fail / 145 skip**; the remaining failure is the typed-array-argument
validation ordering case.

Focused TypedArray typed-array-argument ordering local check:
TypedArray constructors now defer observable `newTarget.prototype` lookup
until allocation, so primitive argument validation and conversion can throw
first. In particular, `Reflect.construct(TA, [Symbol()], newTarget)` no longer
touches a throwing custom `newTarget.prototype` getter before reporting the
required `ToIndex` `TypeError`. With TypedArray, ArrayBuffer, DataView,
Reflect, and `Reflect.construct` skips temporarily lifted,
`built-ins/TypedArrayConstructors/ctors/typedarray-arg` improves from **12
pass / 1 fail / 1 skip** to **13 pass / 0 fail / 1 skip**. Under the same
expanded diagnostic, broader `built-ins/TypedArrayConstructors` now closes at
**593 pass / 0 fail / 145 skip**.

Focused TypedArrayConstructors runner coverage check:
`tools/test262_runner.py` and `tools/test262_analyze.py` now admit implemented
`built-ins/TypedArrayConstructors/` coverage with a path-scoped exception for
TypedArray, concrete TypedArray constructors, ArrayBuffer, DataView, Reflect,
`Reflect.construct`, Proxy, Symbol, `Symbol.iterator`, `Symbol.toPrimitive`,
`Symbol.toStringTag`, and generator metadata needed by the implemented
object-argument iterable constructor path. `Reflect.set()` also routes Symbol
property keys through the receiver-aware ordinary `[[Set]]` path, so
Symbol-named non-writable own data properties on TypedArrays return `false`;
generator object arguments now propagate abrupt completion during iteration.
The normal runner now reports **674 pass / 0 fail / 64 skip** on that path.
Remaining skips stay behind unsupported SharedArrayBuffer, resizable
ArrayBuffer, iterator-helper, and Atomics feature gates.

Focused DataView constructor length local check:
`built-ins/DataView/length.js` passes after `DataView.length` was corrected to
the spec value `1` with the standard non-writable, non-enumerable,
configurable descriptor. With DataView-related skips temporarily lifted, the
broader `built-ins/DataView` diagnostic now reports **310 pass / 11 fail /
240 skip**; the remaining failures are concentrated in immutable-buffer
DataView setters and `setFloat16`.

Focused DataView immutable-buffer setter local check:
the implemented numeric and BigInt DataView setters now reject immutable
ArrayBuffer backing stores with `TypeError` before reading `byteOffset` or
`value` arguments. With DataView-related skips temporarily lifted, the broader
`built-ins/DataView` diagnostic improves to **320 pass / 1 fail / 240 skip**;
the remaining failure is the unsupported `setFloat16` immutable-buffer case.

Focused DataView Float16 local check:
`built-ins/DataView/prototype/getFloat16` and
`built-ins/DataView/prototype/setFloat16` run at **32 pass / 0 fail / 13 skip**
with DataView/ArrayBuffer/Float16Array feature skips temporarily lifted. The
new accessors implement binary16 endian reads/writes, ties-to-even rounding,
signed zero, infinities, NaN, immutable-buffer validation before argument
coercion, and the same detached/range ordering as the other DataView numeric
methods. With DataView-related skips temporarily lifted, the broader
`built-ins/DataView` diagnostic now closes at **321 pass / 0 fail / 240 skip**;
also lifting `Float16Array` for that DataView diagnostic reports **352 pass / 0
fail / 209 skip**.

Focused DataView runner coverage check:
`DataView.prototype[Symbol.toStringTag]` is now exposed with the spec
non-writable, non-enumerable, configurable descriptor. `Reflect.construct`
DataView construction now delays `newTarget.prototype` lookup until after
invalid byte-offset validation, then rechecks whether the backing ArrayBuffer
was detached by that observable lookup before returning the new view.
`tools/test262_runner.py` and `tools/test262_analyze.py` now admit implemented
`built-ins/DataView/` coverage with a path-scoped exception for DataView,
ArrayBuffer, Float16Array, Reflect, `Reflect.construct`, Int8Array, Uint8Array,
Symbol, `Symbol.toPrimitive`, and `Symbol.toStringTag`. The normal runner now
reports **492 pass / 0 fail / 69 skip** on that path. Remaining skips stay
behind unsupported SharedArrayBuffer and resizable ArrayBuffer feature gates.

| Metric | Latest confirmed count |
|--------|------------------------|
| Total matrix files | 47,717 |
| Actually run | 24,133 reported run / 24,144 including timeouts |
| Pass | 17,120 |
| Fail | 7,013 |
| Timeout | 11 |
| Skip | 23,573 |
| **Pass rate (of run)** | **70.9%** |
| **Pass rate (of total)** | **35.9%** |

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

- **String exotic objects and coercion** —
  `String(object)` now performs observable `ToPrimitive` with string hint
  instead of bypassing overridden `toString` methods on arrays, while
  `OrdinaryToPrimitive` now skips non-callable `toString`/`valueOf`
  candidates. Boxed String numeric indices remain read-only/enumerable exotic
  own properties for assignment and `propertyIsEnumerable`, and
  `String.prototype.localeCompare` treats canonically equivalent Unicode
  strings as equal. The focused
  `built-ins/String` run improves from **1093 pass / 3 fail / 127 skip** to
  **1096 pass / 0 fail / 127 skip**.
- **Global `undefined` Reference semantics** —
  Source `undefined` now parses as an IdentifierReference instead of a literal
  expression, so assignment and delete use the same Reference/`PutValue` paths
  as other global names. The non-writable, non-configurable global property is
  preserved: sloppy assignment is ignored while returning the RHS, strict
  assignment throws `TypeError`, and `delete undefined` returns `false`. The
  focused `built-ins/global built-ins/undefined` run improves from **33 pass /
  4 fail / 0 skip** to **37 pass / 0 fail / 0 skip**.
- **Array destructuring assignment IteratorClose ordering** —
  Array assignment patterns now close unfinished iterators on normal partial
  completion, evaluate rest assignment target references before draining rest
  values, and close iterators when rest-target or rest-iterator evaluation
  completes abruptly. The focused `language/expressions/assignment/dstr` run
  closes at **90 pass / 0 fail / 278 skip**, and the broader
  Reference-adjacent cluster remains **835 pass / 0 fail / 363 skip**.
- **Proxy `[[Construct]]` semantics** —
  Constructable Proxy objects now inherit constructability from their target,
  dispatch `construct` traps with a current-Realm argument array, validate trap
  callability and object return values, and delegate to the target when no trap
  is present. The focused `built-ins/Proxy` run improves from **3 pass / 1
  fail / 307 skip** to **4 pass / 0 fail / 307 skip**.
- **Map/Set iterator prototype shape** —
  Map and Set iterators now inherit from shared `%MapIteratorPrototype%` and
  `%SetIteratorPrototype%` objects instead of carrying own `next` methods,
  expose spec-shaped `next` and `@@toStringTag` properties, and reject `next`
  calls on receivers without collection-iterator internal slots. The focused
  `built-ins/MapIteratorPrototype built-ins/SetIteratorPrototype` run improves
  from **9 pass / 5 fail / 8 skip** to **14 pass / 0 fail / 8 skip**.
- **DataView constructor ordering** —
  `DataView` now rejects calls without `new` before coercing constructor
  arguments, while detached ArrayBuffer validation runs after observable
  `byteOffset` coercion. The focused `built-ins/DataView` run improves from
  **266 pass / 2 fail / 293 skip** to **268 pass / 0 fail / 293 skip**.
- **Date component getter receiver validation** —
  Date component getters now use a `thisTimeValue`-style receiver check, so
  ordinary objects, arrays, arguments objects, primitives, and objects spoofing
  RuJa's internal `__time__` property throw `TypeError` instead of being read
  as Invalid Date. `%Date.prototype%` is no longer Date-branded, while
  constructed Date and Date subclass instances still expose the Date brand.
  The focused Date component getter run improves from **80 pass / 16 fail / 32
  skip** to **96 pass / 0 fail / 32 skip**; the broader `built-ins/Date`
  diagnostic now reports **309 pass / 173 fail / 112 skip**.
- **Date.UTC and TimeClip semantics** —
  `Date.UTC` now performs left-to-right numeric coercion for all supplied
  components, applies default month/date/time fields, normalizes 0-99 years,
  and returns the clipped MakeDate result. `TimeClip` now truncates fractional
  milliseconds and normalizes negative zero, so `Date` construction,
  `getTime`/`valueOf`, and `setTime` expose integer clipped time values. The
  focused `built-ins/Date/UTC built-ins/Date/prototype/{getTime,valueOf,setTime}`
  run improves from **20 pass / 16 fail / 6 skip** to **36 pass / 0 fail / 6
  skip**; the broader `built-ins/Date` diagnostic now reports **326 pass / 156
  fail / 112 skip**.
- **Date time-component setters** —
  `setMilliseconds`, `setSeconds`, `setMinutes`, `setHours`, and their UTC
  variants now read the receiver's DateValue before argument coercion, coerce
  optional arguments left to right, preserve omitted lower-order components,
  apply `TimeClip`, and expose spec-shaped `length` values. Invalid Date
  receivers still coerce supplied arguments but return `NaN` without
  overwriting side effects from coercion. The focused time-setter run improves
  from **28 pass / 68 fail / 12 skip** to **96 pass / 0 fail / 12 skip**; the
  broader `built-ins/Date` diagnostic now reports **394 pass / 88 fail / 112
  skip**.
- **Date date-component setters** —
  `setDate`, `setMonth`, `setFullYear`, and their UTC variants now preserve
  the existing time within day, coerce optional arguments left to right, avoid
  the constructor-only 1900 offset for `setFullYear(0..99)`, and apply the
  distinct Invalid Date semantics for date/month setters versus full-year
  setters. The focused date-setter run improves from **23 pass / 41 fail / 9
  skip** to **64 pass / 0 fail / 9 skip**; the broader `built-ins/Date`
  diagnostic now reports **435 pass / 47 fail / 112 skip**.
- **Date stringification, JSON, and ISO parsing** —
  Date prototype string methods now validate Date receivers, render UTC-backed
  date/time strings, return `Invalid Date` for invalid time values, and expose
  proper `toISOString` RangeError behavior. `Date.prototype.toJSON` now follows
  the generic `ToObject`/`ToPrimitive(number)`/`Invoke(toISOString)` path,
  while `Date.parse` recognizes the ISO and Date string forms emitted by RuJa.
  Single-argument Date construction now copies Date receivers without calling
  user hooks and parses Date strings. The focused string/parse/JSON run
  improves from **26 pass / 37 fail / 13 skip** to **63 pass / 0 fail / 13
  skip**; the broader `built-ins/Date` diagnostic now reports **476 pass / 6
  fail / 112 skip**, with the remaining failures isolated to Temporal
  `toTemporalInstant` coverage.
- **Date toTemporalInstant bridge** —
  `Date.prototype.toTemporalInstant` now validates Date-branded receivers,
  throws `RangeError` for invalid dates, and returns a minimal Temporal
  Instant-shaped object exposing `epochNanoseconds` as a BigInt
  millisecond-to-nanosecond conversion. The focused
  `built-ins/Date/prototype/toTemporalInstant` run improves from **0 pass / 6
  fail / 2 skip** to **6 pass / 0 fail / 2 skip**, closing the broader
  `built-ins/Date` diagnostic at **482 pass / 0 fail / 112 skip**.
- **BigInt TypedArray constructor surface** —
  BigInt typed array constructors and prototypes now expose
  `BYTES_PER_ELEMENT` as own non-writable, non-enumerable, non-configurable
  data properties, and typed array prototype accessors reject receivers
  without typed array internal slots. The focused
  `built-ins/TypedArrayConstructors` run improves from **10 pass / 6 fail /
  722 skip** to **16 pass / 0 fail / 722 skip**.
- **ArrayBuffer static surface** —
  `ArrayBuffer` now rejects function calls before coercing `length`, exposes
  spec-shaped `ArrayBuffer.isView()` for typed-array and DataView receivers,
  provides the `ArrayBuffer[Symbol.species]` getter, and uses the intrinsic
  `%ArrayBuffer.prototype%` fallback for `Reflect.construct` new targets with
  non-object prototypes. The focused `built-ins/ArrayBuffer` run improves from
  **41 pass / 50 fail / 130 skip** to **52 pass / 39 fail / 130 skip**.
- **ArrayBuffer slice species construction** —
  `ArrayBuffer.prototype.slice` now uses `SpeciesConstructor`, treats nullish
  `@@species` as the default `ArrayBuffer` constructor, calls custom species
  constructors with the slice length, rejects invalid species results, and
  preserves larger result buffer lengths while copying sliced bytes. The
  focused `built-ins/ArrayBuffer` run improves from **52 pass / 39 fail / 130
  skip** to **57 pass / 34 fail / 130 skip**.
- **`%ThrowTypeError%` intrinsic** —
  Restricted function and arguments accessors now share an anonymous, frozen,
  non-extensible `%ThrowTypeError%` function within each Realm. Strict
  arguments and non-simple-parameter unmapped arguments reuse the same
  Realm-local thrower for `callee`, while `$262.createRealm()` receives a
  distinct intrinsic. The focused `built-ins/ThrowTypeError` run improves from
  **8 pass / 6 fail / 0 skip** to **14 pass / 0 fail / 0 skip**.
- **`%ThrowTypeError%` identity through dynamic functions** —
  Dynamic `Function` bodies now register nested function expressions into the
  VM function table before execution, so IIFEs inside `new Function(...)`
  close over and call the intended function instead of an unrelated existing
  function slot. This also canonicalizes strict arguments'
  `%ThrowTypeError%` accessor to the Realm global environment and gives
  `$262.createRealm()` a Realm-local `Function.prototype` for dynamic
  functions' `[[Prototype]]`, bringing
  `built-ins/Function/prototype/caller built-ins/Function/prototype/arguments`
  from **0 pass / 2 fail / 0 skip** to **2 pass / 0 fail / 0 skip**.
- **`Error.isError` static method** —
  `Error.isError(value)` is now installed as a non-constructable unary builtin.
  It accepts real Error/NativeError objects, Error subclasses, and
  `$262.createRealm()` Error objects, while rejecting primitives, constructors,
  ordinary objects, and fake objects that only inherit from `Error.prototype`.
  `$262.createRealm()` now also exposes `Array` and the native error
  constructor surface required by the cross-realm Error tests. The focused
  `built-ins/Error/isError` run improves from **0 pass / 10 fail / 2 skip** to
  **10 pass / 0 fail / 2 skip**; the broader `built-ins/Error` diagnostic now
  reports **46 pass / 28 fail / 19 skip**.
- **`Error.prototype.toString` edge cases** —
  `Error.prototype.toString` now rejects primitive receivers with `TypeError`
  and follows the spec's empty-string formatting rules, returning only the
  message when `name` is empty and only the name when `message` is empty. The
  focused `built-ins/Error/prototype/toString` run closes at **15 pass / 0
  fail / 2 skip**, and the broader `built-ins/Error` diagnostic improves to
  **48 pass / 26 fail / 19 skip**. The remaining failures are concentrated in
  `Error.prototype.stack`.
- **`Error.prototype.stack` accessor** —
  `%Error.prototype%` now exposes a Realm-local `stack` accessor. The getter
  returns an implementation-defined string for real Error objects and
  `undefined` for ordinary objects, while the setter defines receiver-local
  enumerable/writable/configurable data properties and rejects writes to the
  home `%Error.prototype%` with that receiver Realm's `TypeError`.
  Native error synthesis now preserves the throwing native callee's Realm and
  uses that Realm's original intrinsic Error prototypes rather than mutable
  global `TypeError`/`Error` bindings. `$262.createRealm()` now builds
  Realm-local Error and NativeError constructor/prototype chains for those
  cross-Realm stack checks. `tools/test262_runner.py` and
  `tools/test262_analyze.py` now admit this implemented stack coverage with a
  path-scoped exception for the Error stack accessor, Proxy, Reflect, and
  `Reflect.construct` gates. The focused
  `built-ins/Error/prototype/stack` run closes at **35 pass / 0 fail / 0
  skip**, and the broader `built-ins/Error` runner reports **83 pass / 0 fail
  / 10 skip**.
- **Error `cause` and `AggregateError` constructor shape** —
  `Error` and NativeError constructors now run `InstallErrorCause` through
  observable `HasProperty`/`Get` after installing `message`, so Proxy `has`
  traps, cause getters, and message-before-cause ordering match test262.
  `AggregateError` now uses `(errors, message, options)`, reports
  `length === 2`, creates a non-enumerable `errors` array from the supplied
  iterable, and shares that cause path. The five `error-cause` files now pass:
  `built-ins/Error` reports **87 pass / 0 fail / 6 skip**,
  `built-ins/NativeErrors` reports **87 pass / 0 fail / 7 skip**, and
  `built-ins/AggregateError` now closes at **25 pass / 0 fail / 0 skip**.
- **Class private-name identity** —
  Class evaluation now allocates a fresh opaque private-name key for each
  private field, method, and accessor name and stores the key in the class
  lexical environment captured by constructors and methods. Private slots use
  those opaque keys instead of textual `#name` strings, while RegExp and Proxy
  internal slots use a separate internal-key namespace. Same-spelling private
  names across separate class evaluations or superclass/subclass bodies now have
  distinct brands. With private class feature skips temporarily lifted, the
  focused `language/{statements,expressions}/class/elements` diagnostic improves
  from **1085 pass / 547 fail / 1330 skip** to **1096 pass / 536 fail / 1330
  skip**.
- **Private names before division** —
  The lexer now treats private names as value-ending tokens for slash
  disambiguation, so `this.#x / y` and `this.#x /= y` parse as division and
  divide-assignment instead of starting a RegExp literal. With private class
  feature skips temporarily lifted, the focused
  `language/expressions/compound-assignment` diagnostic now reports **454 pass
  / 0 fail / 0 skip**.
- **String.prototype.matchAll and RegExp `@@matchAll`** —
  `String.prototype.matchAll` now performs the observable RegExp global-flag
  validation before custom `@@matchAll` delegation, preserves the uncoerced
  receiver argument for custom matchers, and creates a forced-global intrinsic
  RegExp for fallback matching. `RegExp.prototype[Symbol.matchAll]` is
  installed as a lazy RegExp String Iterator that calls `RegExpExec` per
  `next()`, so species construction, cached `lastIndex`, custom `exec`, match
  arrays, and empty-match advancement remain observable. The focused
  `built-ins/String/prototype/matchAll
  built-ins/RegExp/prototype/Symbol.matchAll` run now closes at **48 pass / 0
  fail / 3 skip**.
- **String.prototype.replaceAll and RegExp `@@replace`** —
  `String.prototype.replaceAll` now observes the spec ordering for RegExp
  detection, global-flag checks, custom `@@replace` delegation, receiver/search
  coercion, callable replacers, empty search strings, and `$` substitution
  tokens. `RegExp.prototype[Symbol.replace]` is installed for global/sticky,
  capture, named-capture, and functional replacement paths, and
  `RegExp.prototype.toString` reads observable `source`/`flags`. The focused
  `built-ins/String/prototype/replaceAll` run now closes at **35 pass / 0 fail /
  10 skip**. The same slice fixed `super[Symbol.*]` calls and nested array
  binding temporaries exposed by the replaceAll subclass tests.
- **RegExp `@@match` prototype builtin** —
  `RegExp.prototype[Symbol.match]` is now installed and delegates through
  `RegExpExec`, preserving observable `flags`, custom `exec`, `lastIndex`
  updates, global empty-match advancement, and thrown completions. The
  `String.prototype.match` fallback now creates an intrinsic RegExp clone so
  RegExp source/flags are preserved when an own `@@match` property is
  `undefined`. The focused
  `built-ins/String/prototype/match built-ins/RegExp/prototype/Symbol.match`
  run now reports **99 pass / 0 fail / 5 skip** after non-Unicode surrogate
  escapes are lowered for the regex backend using the stored RegExp flags.
- **URI encode/decode globals** —
  `encodeURI`, `encodeURIComponent`, `decodeURI`, and `decodeURIComponent` now
  implement ECMAScript percent encoding/decoding over UTF-16 code units,
  preserve `decodeURI` reserved escapes, reject malformed UTF-8 and lone
  surrogates with `URIError`, and keep `String.fromCharCode` pairs in RuJa's
  surrogate-sentinel range distinguishable from lone surrogates. The focused
  `built-ins/{decodeURI,decodeURIComponent,encodeURI,encodeURIComponent}` run
  improves from **74 pass / 93 fail / 2 timeout / 4 skip** to **167 pass / 0
  fail / 2 timeout / 4 skip**. The remaining two timeouts are exhaustive
  4-byte UTF-8 loop tests that pass under a longer local timeout.
- **Array `some`/`every` generic iteration** —
  `Array.prototype.some` and `Array.prototype.every` now use
  `LengthOfArrayLike`, skip absent indexes via `HasProperty`, read values only
  after presence checks, and pass `(value, index, object)` with the supplied
  callback `thisArg`. This makes array-like receivers, boxed primitives,
  inherited sparse indexes, length snapshots, and abrupt completions observable.
  The focused `built-ins/Array/prototype/{some,every}` run improves from
  **225 pass / 202 fail / 10 skip** to **427 pass / 0 fail / 10 skip**.
- **RegExp boolean flag accessors** —
  `global`, `ignoreCase`, `multiline`, `dotAll`, `sticky`, `unicode`,
  `unicodeSets`, and `hasIndices` now use RegExp internal-slot receiver
  validation: RegExp instances return stored flag bits, the current realm
  `%RegExp.prototype%` returns `undefined`, and ordinary/cross-realm prototype
  receivers throw. The focused
  `built-ins/RegExp/prototype/{flags,global,ignoreCase,multiline,dotAll,sticky,unicode,unicodeSets,hasIndices}`
  run now closes at **62 pass / 0 fail / 54 skip**.
- **Object integrity for arrays, arguments, functions, and Proxy traps** —
  `Object.seal` and `Object.freeze` now use the Proxy-aware
  `[[PreventExtensions]]` path so false Proxy traps throw for the `Object.*`
  forms. Dense Array and arguments indexes are materialized as own descriptors
  during seal/freeze, Array `length` is made non-writable by freeze and honored
  by later length assignment, and `Object.isSealed`/`Object.isFrozen` now
  require non-extensible ordinary objects/functions while checking
  Array/arguments descriptors. The
  focused `built-ins/Object/{seal,freeze,isSealed,isFrozen}` run now closes at
  **218 pass / 0 fail / 21 skip**.
- **TypedArray constructor surface** —
  The existing byte-backed TypedArray exotic now exposes `Int8Array`,
  `Uint8ClampedArray`, `Int16Array`, `Uint16Array`, `Int32Array`,
  `Uint32Array`, `Float32Array`, `Float64Array`, `BigInt64Array`, and
  `BigUint64Array` in addition to `Uint8Array`. The shared implementation now
  reports element-size-aware `length`/`byteLength`, reads and writes indexed
  numeric and BigInt elements, and tracks `[[Extensible]]` so Object integrity
  operations can seal typed-array instances. This removes the final
  TypedArray-constructor failures from the Object integrity focused run.
- **TypedArray constructor inputs** —
  Typed-array constructors now require `new`, use the active typed-array
  prototype as the default constructor prototype, coerce primitive lengths with
  `ToIndex`-style `NaN`/`undefined` handling, initialize from observable
  array-like `length` and indexed reads, and consume iterable inputs through an
  `IteratorToList`-style path before element conversion. `Array.prototype` now
  exposes `[Symbol.iterator]` as `values`, so array-backed iterable constructor
  inputs use the iterator protocol. With TypedArray-related skips temporarily
  lifted, the focused
  `built-ins/TypedArrayConstructors/ctors/{no-args,length-arg,object-arg}`
  diagnostic moved from **8 pass / 20 fail / 19 skip** to **25 pass / 3 fail /
  19 skip**.
- **ArrayBuffer-backed TypedArray views** —
  Typed-array instances now carry `[[ViewedArrayBuffer]]`, `[[ByteOffset]]`,
  and `[[ByteLength]]` slots. Constructors accept `ArrayBuffer` inputs with
  range/alignment checks, expose the original buffer through `.buffer`, report
  view-relative `length`, `byteLength`, and `byteOffset`, and route indexed
  reads/writes through the shared backing buffer. With TypedArray-related skips
  temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/ctors/{no-args,length-arg,object-arg}`
  diagnostic now reports **26 pass / 2 fail / 19 skip**. Remaining failures in
  that probe require iterable zero-fill coverage and shared
  ArrayIteratorPrototype mutation semantics.
- **TypedArray prototype numeric `[[Set]]`** —
  Assignment now recognizes full `CanonicalNumericIndexString` keys, including
  `"NaN"` and `"-0"`, when a TypedArray appears on an ordinary object's
  prototype chain. Invalid numeric indexes are treated as successful no-ops
  instead of creating receiver properties, while valid inherited numeric
  indexes still create receiver data properties. With TypedArray-related skips
  temporarily lifted, the focused `language/statements/with` diagnostic now
  reports **171 pass / 0 fail / 10 skip**.
- **TypedArray backing buffers and ArrayIteratorPrototype `next`** —
  Typed-array views now trace their `[[ViewedArrayBuffer]]` during GC, so
  length allocations keep their zero-filled backing storage alive after
  harness allocation pressure. Array iterator objects now inherit `next` and
  `@@iterator` from a shared prototype instead of defining own methods, so
  typed-array construction observes
  `Object.getPrototypeOf([].values()).next` overrides through the iterator
  protocol. With TypedArray-related skips temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/ctors/{no-args,length-arg,object-arg}`
  diagnostic now reports **28 pass / 0 fail / 19 skip**, closing that
  constructor probe.
- **TypedArray intrinsic prototype shape** —
  Concrete TypedArray constructors now expose the spec `length` of `3` and
  share a `%TypedArray%` intrinsic constructor/prototype pair. The concrete
  prototypes inherit `buffer`, `byteLength`, `byteOffset`, and `length`
  accessors from `%TypedArray%.prototype` instead of defining those accessors
  as own properties. With TypedArray-related skips temporarily lifted, the
  focused constructor/prototype-shape diagnostic now reports **120 pass / 0
  fail / 11 skip**.
- **TypedArray static `from`/`of`** —
  Concrete TypedArray constructors now inherit `%TypedArray%.from` and
  `%TypedArray%.of`, construct the result before reading array-like elements,
  call mapper functions with the expected arguments and receiver, cache
  iterable `next` methods, and reject immutable ArrayBuffer-backed results
  before value conversion. With TypedArray-related skips temporarily lifted,
  the focused
  `built-ins/TypedArrayConstructors/{from,from/BigInt,of,of/BigInt}` run
  closes at **126 pass / 0 fail / 0 skip**, and the broader
  `built-ins/TypedArrayConstructors` diagnostic now reports **473 pass / 54
  fail / 211 skip**.
- **TypedArray integer-indexed `[[Delete]]`** —
  TypedArray canonical numeric index deletes now return `false` for valid
  in-bounds elements and `true` for detached buffers or invalid canonical
  numeric indexes, including `"-0"`, fractional, negative, infinite, and
  out-of-bounds keys. Non-canonical keys continue through ordinary delete. With
  TypedArray-related skips temporarily lifted,
  `built-ins/TypedArrayConstructors/internals/Delete` now reports **29 pass / 2
  fail / 8 skip**, while the current runner's broader
  `built-ins/TypedArrayConstructors` diagnostic moves from **419 pass / 54 fail
  / 265 skip** to **431 pass / 42 fail / 265 skip**.
- **Nullish computed property write/delete ordering** —
  Simple computed property assignment and computed `delete` now check
  `null`/`undefined` bases before observable `ToPropertyKey` coercion. This
  keeps simple assignment's spec order where the RHS runs before the
  `PutValue` `TypeError`, while preventing the computed key's `toString`/
  `@@toPrimitive` from running after the nullish-base failure is known. The
  focused `language/expressions/assignment language/expressions/delete
  language/expressions/member-expression` diagnostic remains **273 pass / 0
  fail / 282 skip**.
- **Proxy SetIntegrityLevel/TestIntegrityLevel** —
  Transparent Proxy receivers for `Object.seal`/`Object.freeze` now define the
  integrity descriptors through the Proxy-aware `[[DefineOwnProperty]]` path,
  so target descriptors are actually sealed/frozen. `Object.isSealed` and
  `Object.isFrozen` now use Proxy `ownKeys` and `getOwnPropertyDescriptor`
  semantics instead of treating Proxy objects as ordinary empty exotics. With
  `Proxy`/`Reflect`/`Symbol` skips temporarily lifted, the focused Object
  integrity proxy diagnostic runs at **6 pass / 0 fail**.
- **Proxy prototype internal methods** —
  `Object.getPrototypeOf`, `Reflect.getPrototypeOf`, `Object.setPrototypeOf`,
  `Reflect.setPrototypeOf`, the `__proto__` accessor, and `instanceof` now
  route through Proxy `getPrototypeOf`/`setPrototypeOf` traps, including
  nullish trap delegation, revoked-proxy errors, and non-extensible target
  invariants. With `Proxy`/`Reflect` skips temporarily lifted, the focused
  `built-ins/Proxy/{getPrototypeOf,setPrototypeOf}
  built-ins/Reflect/{getPrototypeOf,setPrototypeOf}` diagnostic runs at **29
  pass / 0 fail / 31 skip**, and the broader Proxy descriptor/prototype
  diagnostic improves from **21 pass / 46 fail / 63 skip** to **46 pass / 21
  fail / 63 skip**. Remaining failures in that broader probe are descriptor
  conversion and define/getOwnPropertyDescriptor invariants.
- **Proxy.revocable revoke function shape** —
  `Proxy.revocable()` now creates its revoke closure through the native
  function helper, giving the closure spec-shaped own `length` and `name`
  properties in insertion order while keeping the associated proxy in a
  non-observable private slot. With `Proxy`/`Reflect`/`Symbol` skips
  temporarily lifted, `built-ins/Proxy/revocable` runs at **17 pass / 0 fail /
  1 skip** after callable Proxy support for function targets.
- **Callable Proxy `[[Call]]` support** —
  Proxy objects whose target is callable now report as callable for `typeof`
  and `IsCallable` checks. Ordinary calls to callable proxies reject revoked
  proxies, forward through nested callable proxy targets when `handler.apply`
  is nullish, and call callable `apply` traps with `(target, thisArgument,
  argumentsList)`. With Proxy-related skips temporarily lifted,
  `built-ins/Proxy/apply` now runs at **13 pass / 0 fail / 1 skip**.
- **Object descriptor helpers observe Proxy descriptors** —
  `Object.values`, `Object.entries`, and
  `Object.getOwnPropertyDescriptors` now revalidate each snapshotted key
  through the Proxy-aware `[[GetOwnProperty]]` path, so
  `getOwnPropertyDescriptor` traps are observable before enumerable filtering,
  value reads, and descriptor materialization. With `Proxy`/`Reflect`/`Symbol`
  skips temporarily lifted, focused
  `built-ins/Object/{values,entries,getOwnPropertyDescriptors}` runs at **59
  pass / 0 fail / 0 skip** after the separate RegExp internal-slot exposure
  fix.
- **RegExp internal slots hidden from own keys** —
  RegExp instances now store source, flags, and derived flag bits in
  non-observable internal storage instead of ordinary own properties, so
  `lastIndex` remains the only default own string key and user-defined keys
  keep spec insertion order in descriptor helpers. The individual flag
  accessors read those internal slots, while `RegExp.prototype.flags` still
  observes public `global`/`sticky`/other property overrides through normal
  `Get` semantics. Internal slot keys use non-identifier names so subclass
  `#private` fields cannot collide with them.
- **`RegExp.escape` static builtin** —
  `RegExp.escape` is now installed on every realm-local `RegExp` constructor
  with spec-shaped `name`, `length`, descriptor, and non-constructor behavior.
  It rejects non-string inputs without `ToString`, escapes initial ASCII
  letters/digits with `\xNN`, handles syntax characters, `/`, control escapes,
  other punctuators, whitespace/line terminators, and preserves RuJa's UTF-16
  string model by escaping lone surrogates as `\uNNNN`. The focused
  `built-ins/RegExp/escape` run now closes at **19 pass / 0 fail / 1 skip**.
- **Reference-preserving identifier calls through `with`** —
  direct IdentifierReference calls now carry their Reference record into the
  VM call opcode. `with (o) { f() }` still binds `this` to `o` when `f` is
  resolved from the object environment, while value-producing callee
  expressions such as `(0, f)()`, `(cond ? f : f)()`, and `(f && f)()` lose
  that binding. Spread and optional identifier calls use the same
  Reference-preserving path, and unqualified `eval(...)` keeps direct eval
  behavior only for the intrinsic eval function. The focused
  `language/statements/with` run closes at **169 pass / 0 fail / 12 skip**.
- **Optional method-call argument order** —
  Optional member calls now fetch the method and short-circuit nullish method
  values before evaluating ordinary or spread arguments. This keeps
  `o.m?.(sideEffect())` from running side effects when `o.m` is
  `null`/`undefined`, while present methods still receive the original object
  as `this`. The focused `language/expressions/optional-chaining` directory is
  still feature-skipped by the runner (**38 skip**), so the edge is pinned by
  local `operators` regression tests.
- **Symbol computed keys for member assignments** —
  computed member update, numeric/bitwise compound assignment, and logical
  assignment now coerce computed keys with `ToPropertyKey` instead of
  `ToString`, so Symbol property keys survive the read/write pair while base,
  key, and right-hand-side evaluation order stays unchanged. The focused
  `language/expressions/{compound-assignment,logical-assignment,
  prefix-increment,postfix-increment}` run closes at **532 pass / 0 fail / 71
  skip**.
- **Map/Set zero-key canonicalization** —
  Internal `MapKey` creation now canonicalizes numeric `-0` to `+0`, and its
  hash implementation now agrees with SameValueZero equality for both zero
  signs. Map replacement, Set de-duplication, and key iteration now share the
  same keyed-collection semantics. The focused zero-key test262 probe
  `built-ins/Map/prototype/set/replaces-a-value-normalizes-zero-key.js
  built-ins/Set/prototype/add/will-not-add-duplicate-entry-normalizes-zero.js`
  now runs at **2 pass / 0 fail**.
- **Map prototype receiver brand checks** —
  `Map.prototype` methods now require a receiver with a `[[MapData]]` internal
  slot before doing any keyed-collection work. Non-Map receivers now throw
  `TypeError` consistently across `get`, `set`, `has`, `delete`, `clear`,
  `entries`, `keys`, `values`, and `forEach` instead of silently returning
  empty or falsey fallback values. The focused
  `built-ins/Map/prototype/{get,set,has,delete,clear,entries,keys,values,
  forEach,size}` cluster now runs at **60 pass / 11 fail / 47 skip**, with
  remaining failures isolated to true MapIterator and live iteration semantics.
- **Set prototype size accessor and receiver brand checks** —
  `Set.prototype.size` is now an accessor property with a spec-shaped
  `"get size"` getter, and Set instance `size` reads now use ordinary
  prototype lookup instead of a VM fast path so overrides and deletion are
  observable. `Set.prototype` methods now require a receiver with a
  `[[SetData]]` internal slot, and `Set.prototype.clear` is exposed with the
  same validation. The focused
  `built-ins/Set/prototype/{size,add,has,delete,clear,entries,keys,values,
  forEach}` cluster now runs at **130 pass / 9 fail / 26 skip**, with
  remaining failures isolated to true SetIterator and live iteration semantics.
- **Map/Set collection iterators and live forEach** —
  `Map.prototype.entries`/`keys`/`values` and
  `Set.prototype.entries`/`keys`/`values` now return iterator objects with
  `next()` result objects instead of snapshot arrays. Built-in Map/Set
  iteration uses the same lazy collection iterator path, `Map.prototype`
  exposes `@@iterator` as the original `entries` method, and `Set.prototype`
  exposes both `keys` and `@@iterator` as the original `values` method.
  Map/Set `forEach` now observes values added during iteration, skips deleted
  unvisited values, and revisits delete-then-readded values in insertion order.
  The focused `built-ins/{Map,Set}/prototype` iterator/forEach/Symbol.iterator
  cluster now runs at **104 pass / 0 fail / 38 skip**.
- **Set composition methods and constructor iterable compliance** —
  `Set.prototype.union`, `intersection`, `difference`,
  `symmetricDifference`, `isSubsetOf`, `isSupersetOf`, and `isDisjointFrom`
  now implement the ES Set composition algorithms against Set-like operands,
  bypass user-overridden result `add`/`Symbol.species`, preserve live Set
  traversal where the algorithms require it, and close operand iterators on
  early exits. Set-like size handling now rejects negative sizes and truncates
  fractional sizes for algorithmic size comparisons. `Array.prototype.values()`
  now returns an iterator object instead of an array snapshot, so array
  iterators work as Set-like `keys()` results without accepting arbitrary
  iterables where an iterator is required. `new Set(iterable)` now requires
  construction with `new`, observes the instance `add` method before iterator
  creation, calls it for each iterated value, and closes the iterator if that
  call throws. The focused
  Set composition cluster now runs at **179 pass / 0 fail / 7 skip**,
  `built-ins/Set` now closes at **340 pass / 0 fail / 43 skip**, and
  `built-ins/Map built-ins/Set` improves to **432 pass / 17 fail / 138 skip**.
- **Map constructor iterable compliance and upsert methods** —
  `new Map(iterable)` now requires construction with `new`, observes the
  instance `set` method once before iterator creation, calls it for each entry
  pair, accepts array-like pair objects through ordinary property access, and
  closes the source iterator when pair access or `set` fails while preserving
  the original abrupt completion if iterator closing also throws.
  `Map.prototype.getOrInsert` and `Map.prototype.getOrInsertComputed` are now
  exposed with SameValueZero key canonicalization and computed-callback
  overwrite semantics. The focused `built-ins/Map built-ins/Set` run now
  closes at **449 pass / 0 fail / 138 skip**.
- **`Map.groupBy` static grouping** —
  `Map.groupBy` is now exposed as a static built-in, iterates arbitrary sync
  iterables, calls the grouping callback with `(value, index)`, stores group
  keys with SameValueZero Map-key semantics rather than `ToPropertyKey`,
  returns a real Map instance, and closes custom iterators when the callback
  abruptly completes. This closes the focused `built-ins/Map/groupBy` run at
  **14 pass / 0 fail / 0 skip**.
- **Map/Set feature lift** —
  `Map` and `Set` are removed from the test262 unsupported-feature skip list
  after the expanded `built-ins/Map built-ins/Set` diagnostic verifies at
  **473 pass / 0 fail / 114 skip**. The supported subset remains green while
  increasing to **5017 pass / 0 fail / 0 timeout**.
- **String well-formed Unicode methods** —
  `String.prototype.isWellFormed` and `String.prototype.toWellFormed` now
  apply UTF-16 surrogate-pair rules through RuJa's internal lone-surrogate
  sentinel representation, reject nullish receivers, and preserve valid pair
  representations while `toWellFormed` replaces unpaired surrogates with
  U+FFFD. The focused `built-ins/String/prototype/isWellFormed
  built-ins/String/prototype/toWellFormed` run now closes at **14 pass / 0
  fail / 2 skip**.
- **`String.prototype.normalize` Unicode forms** —
  `String.prototype.normalize` is now exposed with spec-shaped function
  metadata and descriptor attributes, defaults missing/undefined forms to NFC,
  observes `ToString(form)`, throws `RangeError` for invalid forms, and returns
  NFC/NFD/NFKC/NFKD-normalized strings. The focused
  `built-ins/String/prototype/normalize` run now closes at **11 pass / 0 fail /
  3 skip**.
- **`Array.of` constructor and property semantics** —
  `Array.of` now follows the constructable-`this` path, passes the argument
  count into custom constructors, creates element data properties without
  invoking inherited setters, observes strict `length` setters, and propagates
  constructor/property abrupt completions. Test262-created realms now expose a
  constructable `Function` constructor so cross-realm constructor fallback
  diagnostics execute. The focused `built-ins/Array/of` run now closes at
  **14 pass / 0 fail / 2 skip**.
- **Reflect.construct `newTarget` semantics** —
  `Reflect.construct` now validates constructor-ness in spec order, creates
  its argument list through ordinary array-like `length` and indexed property
  access, forwards the optional `newTarget` to allocation, and falls back to
  `%Object.prototype%` when the observed `newTarget.prototype` is not an
  object. Bound constructor forwarding now preserves the caller's `newTarget`
  rather than substituting the bound target. With `Reflect`/
  `Reflect.construct` skips temporarily lifted, the focused
  `built-ins/Reflect/construct` diagnostic runs at **10 pass / 0 fail**.
- **Reflect.apply array-like arguments** —
  `Reflect.apply` now performs the spec `IsCallable(target)` check before
  observing `argumentsList`, then creates the call argument list through
  ordinary array-like `length` and indexed property access. This rejects
  primitive or missing `argumentsList` values with `TypeError`, propagates
  abrupt `length`/index gets, and accepts ordinary array-like objects and
  functions instead of treating every non-Array object as an empty argument
  list. With `Reflect`/`Symbol` skips temporarily lifted, the focused
  `built-ins/Reflect/apply` diagnostic runs at **8 pass / 0 fail / 1 skip**.
- **Number static method descriptors** —
  `Number.isFinite`, `Number.isInteger`, `Number.isNaN`,
  `Number.isSafeInteger`, `Number.parseInt`, and `Number.parseFloat` are now
  installed as writable, non-enumerable, configurable constructor properties,
  while numeric constants remain non-writable and non-configurable. The
  focused `built-ins/Number/{isFinite,isInteger,isNaN,isSafeInteger}` run now
  closes at **26 pass / 0 fail / 8 skip**.
- **Boolean prototype receiver checks** —
  `Boolean.prototype` now carries the wrapped `false` primitive value,
  `Boolean.prototype.valueOf` returns Boolean primitives from primitive/boxed
  Boolean receivers, and `valueOf`/`toString` reject non-Boolean receivers
  with `TypeError`. The focused `built-ins/Boolean` run now closes at **46
  pass / 0 fail / 5 skip**.
- **Number/String prototype receiver checks** —
  `Number.prototype` now carries the wrapped `+0` primitive value and
  `String.prototype` carries the wrapped empty string, so their `valueOf`
  methods accept only primitive or matching boxed receivers and throw
  `TypeError` for borrowed incompatible receivers. `$262.createRealm()` now
  exposes realm-local primitive wrapper constructors so cross-realm String
  receiver errors are produced from the callee Realm. The focused
  `built-ins/Number/prototype/valueOf built-ins/String/prototype/valueOf` run
  now closes at **16 pass / 0 fail / 2 skip**.
- **Number prototype toString radix order** —
  `Number.prototype.toString` now follows the spec order of extracting the
  Number receiver before inspecting `radix`, treats absent or explicit
  `undefined` radix as decimal, and propagates abrupt completions from radix
  `ToNumber` instead of replacing them with base 10. The focused
  `built-ins/Number/prototype/toString built-ins/String/prototype/toString`
  run now closes at **95 pass / 0 fail / 2 skip**.
- **Number prototype toFixed integer conversion** —
  `Number.prototype.toFixed` now uses `ThisNumberValue`, truncates
  `fractionDigits` with `ToIntegerOrInfinity` semantics, checks the
  `0..=100` range before returning `"NaN"`, delegates `|x| >= 1e21` to
  ordinary Number stringification, and preserves exact fixed-point output for
  large integer-valued doubles with spec tie-up rounding. Ordinary Number
  stringification now uses the shortest decimal for integer-valued doubles, so
  `toString` and `toFixed(0)` diverge where the spec requires. The focused
  `built-ins/Number/prototype/toFixed` run now closes at **14 pass / 0 fail /
  2 skip**.
- **Number exponential/precision formatting** —
  `Number.prototype.toExponential` and `Number.prototype.toPrecision` now use
  `ThisNumberValue`, truncate their digit arguments with
  `ToIntegerOrInfinity`, return `"NaN"`/`"Infinity"` after argument coercion
  but before range checks, normalize exponent signs, format `-0` as `+0`, and
  use exact-rational half-up decimal rounding for exponential notation. The
  broader `built-ins/Number` run now closes at **312 pass / 0 fail / 28
  skip**.
- **Math.pow NaN and infinite exponent edges** —
  `Math.pow` now handles exponent `NaN` and `abs(base) === 1` with infinite
  exponents before delegating to Rust's `powf`, while keeping the required
  `x ** ±0 === 1` behavior. The focused `built-ins/Math/pow` run now closes
  at **27 pass / 0 fail / 1 skip**.
- **Math.sumPrecise** —
  `Math.sumPrecise` is now exposed as a unary Math builtin, consumes iterable
  Number values without coercing non-numbers, closes the iterator on
  non-number failures, preserves `NaN`, infinity, and signed-zero semantics,
  and accumulates finite values through exact-rational summation before final
  IEEE-754 rounding. The focused `built-ins/Math/sumPrecise` run now closes at
  **8 pass / 0 fail / 2 skip**, and broader `built-ins/Math` closes at **284
  pass / 0 fail / 43 skip**.
- **Number parse function identity** —
  `Number.parseInt` and `Number.parseFloat` now reference the same built-in
  function objects as the global `parseInt` and `parseFloat` properties,
  rather than separate native wrappers. The broader `built-ins/Number` run now
  improves to **301 pass / 11 fail / 28 skip**.
- **PrivateName lexical grammar** —
  Private class names now follow the same `IdentifierName` scanner rules as
  ordinary identifiers, including Unicode escapes, raw Unicode source text,
  `Other_ID_Start`, ZWNJ, and ZWJ. Private fields, methods, and accessors now
  accept names spelled with `\uXXXX`/`\u{...}` escapes and non-ASCII source
  text. The focused private-name diagnostic now closes at **50 pass / 0
  fail**.
- **Private method function identity** —
  Instance private methods are now created once during class evaluation and
  copied into each instance private slot from a shared class-environment
  binding, instead of allocating a fresh function object during every
  constructor call. This preserves private method names, `super` HomeObject
  capture, and `this.#m` identity across instances. With private method skips
  temporarily lifted, the focused
  `language/{statements,expressions}/class/elements/private-methods`
  diagnostic now closes at **2 pass / 0 fail / 8 skip**.
- **Private slot brand checks** —
  Private field, accessor, and method access now throws `TypeError` when the
  receiver is primitive or does not carry the relevant private slot. Class
  element initialization is split from ordinary private writes through
  `InitPrivate` opcodes, private methods are stored as non-writable method
  slots. With private class feature skips temporarily lifted, the
  focused `language/{statements,expressions}/class/elements` probe improves
  from **1045 pass / 587 fail / 1330 skip** to **1085 pass / 547 fail / 1330
  skip**. Remaining same-spelling cross-class brand failures still need
  per-evaluation private-name identity rather than textual `#name` slot keys.
- **Object/Reflect preventExtensions semantics** —
  Array and arguments objects now carry their own `[[Extensible]]` state,
  assignment and receiver-set paths reject new indexed or named properties on
  non-extensible arrays, arguments, and functions, and
  `Object.preventExtensions`/`Reflect.preventExtensions` now route through
  Proxy `preventExtensions` traps with the correct throw-vs-boolean behavior.
  The focused `built-ins/Object/preventExtensions` run now closes at **36 pass
  / 0 fail / 4 skip**, and the adjacent
  `built-ins/{Reflect,Proxy}/preventExtensions` probe closes at **19 pass / 0
  fail / 3 skip** with skips temporarily lifted.
- **Object/Reflect isExtensible Proxy semantics** —
  `Object.isExtensible` now routes object receivers through the Proxy-aware
  `[[IsExtensible]]` helper, returns `false` for primitive receivers, and
  enforces Proxy trap result invariants against the target's actual
  extensibility. `Reflect.isExtensible` now rejects primitive targets with
  `TypeError` and shares the same Proxy trap path. With `Proxy`/`Reflect`
  skips temporarily lifted, the focused
  `built-ins/{Object,Reflect,Proxy}/isExtensible` probe now closes at **55
  pass / 0 fail / 3 skip**.
- **parseInt radix and large-prefix conformance** —
  Global `parseInt` now converts its radix argument through `ToNumber` and
  `ToInt32`, so string, boxed primitive, ordinary object, infinite, and
  modulo-2^32 radix values select the same parse base as the spec. Digit
  accumulation no longer routes through fixed-width Rust integer parsing, so
  large valid prefixes produce their nearest IEEE-754 Number value instead of
  `NaN`. The focused `built-ins/parseInt` run now closes at **53 pass / 0 fail
  / 2 skip**.
- **Math inverse hyperbolic methods** —
  `Math.acosh`, `Math.asinh`, and `Math.atanh` are now exposed as unary native
  functions with the expected `name`, `length`, and own-property descriptors.
  They share the ordinary unary Math `ToNumber` coercion path and preserve
  NaN, infinity, and signed-zero behavior through the host libm operations.
  The focused `built-ins/Math/acosh built-ins/Math/asinh
  built-ins/Math/atanh` run now closes at **14 pass / 0 fail / 3 skip**.
- **Math integer conversion and signed-zero edges** —
  `Math.clz32` and `Math.imul` now use RuJa's spec-shaped
  `ToUint32`/`ToInt32` helpers instead of Rust casts, so infinities, `NaN`,
  modulo-2^32 inputs, and signed multiplication results match ECMAScript.
  `Math.sign` now preserves `NaN` and `-0`. The focused
  `built-ins/Math/{cbrt,clz32,cosh,expm1,fround,imul,log10,log1p,log2,sign,sinh,tanh,trunc}`
  run now closes at **68 pass / 0 fail / 13 skip**.
- **Math max/min/round edge semantics** —
  `Math.max` and `Math.min` now coerce every argument before returning `NaN`,
  propagate `NaN` after observable coercions, and apply the spec signed-zero
  ordering where `+0` is greater than `-0`. `Math.round` now preserves `-0`
  for `[-0.5, -0]`, returns `+0` for positive values below `0.5`, and keeps
  already-integral large Number values unchanged. The focused
  `built-ins/Math/{max,min,round}` run now closes at **28 pass / 0 fail / 3
  skip**.
- **String literal escape conformance** —
  String literal scanning now decodes UTF-8 `NonEscapeCharacter` escapes as
  source code points, permits literal U+2028/U+2029 in strings per
  JSON-superset source text, decodes sloppy legacy octal escapes, and rejects
  legacy octal/non-octal decimal escapes in strict-mode string literals. The
  focused `language/literals/string` run now closes at **71 pass / 0 fail / 2
  skip**, and broader `language/literals` improves to **434 pass / 40 fail /
  60 skip**.
- **RegExp quantifier early errors** —
  RegExp literal validation now rejects quantifiers that appear before any
  atom, including `/?/`, `/{2}/`, `/{2,}/`, and `/{2,3}/`, while preserving
  escaped quantifier characters, character classes, and ordinary atom
  quantifiers such as `/a?/` and `/a{2}/`. The `RegExp` constructor uses the
  same validation, so `new RegExp('?')` and braced-quantifier-only patterns
  also throw `SyntaxError`. The focused `language/literals/regexp` diagnostic
  now runs at **144 pass / 36 fail / 58 skip**, and broader
  `language/literals` improves to **438 pass / 36 fail / 60 skip**.
- **RegExp assertion quantifier early errors** —
  RegExp literal validation now rejects quantifiers applied to lookbehind
  assertions in all modes and to lookahead assertions in Unicode mode. Annex B
  non-Unicode lookahead quantifiers remain accepted by the lexical validator.
  The `RegExp` constructor and statement-list regex fallback path use the same
  validation. The focused `language/literals/regexp` diagnostic now runs at
  **156 pass / 24 fail / 58 skip**, and broader `language/literals` improves
  to **450 pass / 24 fail / 60 skip**.
- **RegExp Unicode-mode syntax early errors** —
  RegExp literal and constructor validation now reject malformed or
  out-of-range `\u{...}` escapes, invalid Unicode-mode identity, control, and
  decimal escapes, bare `{` pattern characters, and character-class ranges
  whose endpoints are multi-character class escapes such as `\d` or `\s`.
  This closes the remaining RegExp literal parse-negative bucket. The focused
  `language/literals/regexp` diagnostic now runs at **168 pass / 12 fail / 58
  skip**, and broader `language/literals` improves to **462 pass / 12 fail /
  60 skip**.
- **RegExp Unicode property escape validation** —
  Unicode-mode RegExp literal and constructor validation now parse
  `\p{...}`/`\P{...}` bodies semantically instead of accepting any non-empty
  ASCII name. Property-less escapes are limited to binary properties or
  `General_Category` values, so bare script names such as `\p{Greek}` and
  loose-cased names such as `\p{Ascii}` now report early syntax errors.
  Explicit `Script=...`, `Script_Extensions=...`, `gc=...`, and
  binary-property aliases remain accepted when the regex backend supports
  them.
- **RegExp null escapes and UTF-8 literal source** —
  RegExp literal scanning now preserves non-ASCII pattern text as Unicode code
  points instead of UTF-8 byte fragments, including escaped non-ASCII pattern
  characters. The internal regex compiler now maps ES `\0` null-character
  escapes to the backend-supported `\x00` form while preserving the public
  `source`, and `String.prototype.search` now accepts RegExp arguments for
  these focused probes while returning UTF-16 indices and preserving
  `lastIndex`. The focused `language/literals/regexp` diagnostic now runs at
  **173 pass / 7 fail / 58 skip**, and broader `language/literals` improves to
  **467 pass / 7 fail / 60 skip**.
- **RegExp sticky start assertions** —
  `RegExp.prototype.exec` now applies global and sticky matches to the full
  input at the UTF-16 `lastIndex` position instead of slicing the input before
  matching. This keeps `^` anchored to the real input start, or to real
  multiline line starts, even when the `y` flag is present. Global
  `lastIndex` updates now use the actual match end after skipped input. The
  focused `language/literals/regexp` diagnostic now runs at **174 pass / 6
  fail / 58 skip**, and broader `language/literals` improves to **468 pass /
  6 fail / 60 skip**.
- **RegExp non-Unicode case folding** —
  The internal regex backend now prevents Rust's Unicode case folding for
  non-ASCII literal atoms and `\uXXXX`/`\xNN` escapes when a pattern has `i`
  without `u`, while still allowing Unicode case folding for `iu`. This
  matches ES canonicalization for probes such as Kelvin sign `\u212a`. The
  focused `language/literals/regexp` diagnostic now runs at **175 pass / 5
  fail / 58 skip**, and broader `language/literals` improves to **469 pass /
  5 fail / 60 skip**.
- **RegExp Unicode surrogate-pair escapes** —
  The internal regex backend now combines adjacent Unicode-mode
  surrogate-pair escapes, such as `\ud800\udc00`, into scalar `\u{...}`
  escapes before compiling with Rust regex while preserving the public
  `source` text. This makes character classes and normal atoms observe the
  pair as one Unicode scalar instead of two surrogate code units. The focused
  `language/literals/regexp` diagnostic now runs at **177 pass / 3 fail / 58
  skip**, and broader `language/literals` improves to **471 pass / 3 fail /
  60 skip**, with the remaining RegExp literal failures isolated to
  backreference support.
- **RegExp exec result shape and `lastIndex` coercion** —
  `RegExp.prototype.exec` now returns match arrays with enumerable `index`,
  `input`, and `groups` properties, treats a missing argument as
  `"undefined"`, reads `lastIndex` through ordinary `Get`/`ToLength` on every
  call, and reports `TypeError` when global/sticky `lastIndex` write-back
  fails. Lone surrogate escapes now lower to RuJa's internal surrogate
  sentinel in Unicode mode and to code-unit-aware backend atoms in
  non-Unicode mode, so `/\udf06/u` keeps scalar semantics while `/\udf06/`
  can match the low half of a surrogate pair.
- **RegExp repeated capture clearing** —
  `RegExp.prototype.exec` now clears descendant captures left over from
  earlier iterations of quantified capturing and non-capturing groups when
  those descendants did not participate in the final iteration. This matches
  ES repeated-capture semantics for cases like
  `/(z)((a+)?(b+)?(c))*/`, where the final optional `(b+)` capture must be
  `undefined` instead of the previous iteration's `"bbb"`, and
  `/(?:(a)|(b))*/`, where `(a)` must be cleared after the final `(b)`
  iteration. The same clearing now feeds `String.prototype.match` and function
  replacement callbacks. The focused `built-ins/RegExp/prototype/exec`
  diagnostic is **75 pass / 0 fail / 4 skip**; the broader
  `built-ins/String/prototype/{match,replace}` diagnostic now closes at
  **100 pass / 0 fail / 6 skip**.
- **String match RegExp creation and `@@match` dispatch** —
  `String.prototype.match` now follows the `@@match` dispatch path before
  ordinary matching, so custom `searchValue[Symbol.match]` getters and methods
  are observable. Values without a custom matcher are converted through a
  `RegExpCreate`-style intrinsic RegExp instead of returning `null`, and that
  internally-created RegExp observes an overridden
  `RegExp.prototype[Symbol.match]` before falling back to RuJa's internal match
  algorithm. The focused
  `built-ins/String/prototype/match` diagnostic now closes at **47 pass / 0
  fail / 4 skip**.
- **String search `@@search` dispatch and RegExp search semantics** —
  `String.prototype.search` now observes custom object
  `searchValue[Symbol.search]` getters and methods, creates an intrinsic
  RegExp for ordinary search values, and lets internally-created RegExp
  objects dispatch through an overridden `RegExp.prototype[Symbol.search]`.
  `RegExp.prototype[Symbol.search]` is now exposed with custom `exec`
  dispatch, object-or-null result validation, strict `lastIndex` writes, and
  restoration of the previous `lastIndex` after the search. The focused
  `built-ins/String/prototype/search
  built-ins/RegExp/prototype/Symbol.search` diagnostic now closes at **61
  pass / 0 fail / 5 skip**.
- **String split `@@split` dispatch and RegExp separator semantics** —
  `String.prototype.split` now has the spec `length` of 2, rejects nullish
  receivers before coercion, observes custom `separator[Symbol.split]`
  getters and methods, and propagates abrupt completions from limit coercion.
  Ordinary separators now use `ToString` in order, while `undefined`
  separators and zero limits follow the expected array result shape. RegExp
  separators now include captured substrings, honor split limits, ignore
  boundary zero-length matches, and normalize additional ES RegExp escapes for
  the backend (`[]`, `[^]`, control escapes, class backspace, and incomplete
  `\x`). The focused `built-ins/String/prototype/split` diagnostic now closes
  at **117 pass / 0 fail / 3 skip**, and the combined
  `built-ins/String/prototype/search built-ins/String/prototype/split
  built-ins/RegExp/prototype/Symbol.search` run closes at **178 pass / 0 fail
  / 8 skip**.
- **String replace substitution tokens and `@@replace` dispatch** —
  `String.prototype.replace` string replacements now expand ECMAScript
  replacement tokens (`$$`, `$&`, ``$` ``, `$'`, `$n`, `$nn`) for both RegExp
  and plain string search values instead of delegating to backend replacement
  syntax or raw `String::replacen`. Unmatched captures substitute as the empty
  string, numeric fallback cases such as `$11` and `$01` match JS behavior,
  and the replacement-string path now uses the same repeated-capture clearing
  as `RegExp.prototype.exec`. Plain replacement now also coerces `searchValue`
  before non-callable `replaceValue`, and custom `searchValue[Symbol.replace]`
  methods are observed before ordinary replacement. The focused
  `built-ins/String/prototype/replace` diagnostic now closes at **53 pass / 0
  fail / 2 skip**.
- **String replace callback offsets** —
  Function replacements for both RegExp and string search values now receive
  the match offset as a UTF-16 code-unit index instead of a Rust UTF-8 byte
  offset, so matches after supplementary characters report the same offset
  that JS exposes through string indexing. The focused
  `built-ins/String/prototype/replace` diagnostic now closes at **53 pass / 0
  fail / 2 skip**.
- **RegExp named capture groups** —
  Named captures now feed the shared match result surface:
  `RegExp.prototype.exec` and non-global `String.prototype.match` expose a
  null-prototype `groups` object, RegExp function replacements receive that
  groups object as their final argument, and replacement strings expand
  `$<name>` using the same capture metadata. The focused
  `built-ins/RegExp/prototype/exec built-ins/String/prototype/{match,replace}`
  diagnostic now closes at **175 pass / 0 fail / 10 skip**.
- **RegExp backreferences and identity escapes** —
  RegExp compilation now keeps ordinary patterns on the existing Rust regex
  fast path, uses a backtracking-capable backend only for true numeric
  backreferences, and lowers non-Unicode legacy decimal escapes plus backend-
  unsupported identity escapes to equivalent literal backend patterns while
  preserving public `source`. The focused `language/literals/regexp`
  diagnostic now closes at **180 pass / 0 fail / 58 skip**, and broader
  `language/literals` closes at **474 pass / 0 fail / 60 skip**.
- **Map prototype size accessor** —
  `Map.prototype.size` is now an accessor property with a spec-shaped
  `"get size"` getter. The getter rejects non-Map receivers with `TypeError`,
  and Map instance `size` reads now use ordinary prototype lookup rather than
  a VM fast path, so prototype overrides and deletion are observable. The
  focused `built-ins/Map/prototype/size` cluster now runs at **6 pass / 0
  fail / 5 skip**.
- **RegExp literal line-terminator early errors** —
  Regular-expression literal scanning now rejects CR, LF, LS, and PS
  immediately after a backslash. This matches the grammar rule that
  `RegularExpressionBackslashSequence` cannot contain a line terminator,
  so parse-negative sources such as `/\\\n/` throw `SyntaxError` before
  executing `$DONOTEVALUATE()`. The focused `language/literals/regexp`
  diagnostic now runs at **55 pass / 125 fail / 58 skip**, and broader
  `language/literals` improves to **324 pass / 150 fail / 60 skip**.
- **RegExp flags and modifiers early errors** —
  RegExp literals, statement-list regex recovery, and `new RegExp(pattern,
  flags)` now share validation for duplicate/invalid flags and RegExp
  modifiers groups. Modifier groups only accept source-text `i`, `m`, and
  `s`, reject duplicates and add/remove intersections, require a colon, and
  reject Unicode escapes or case-folded flag spellings. The focused
  `language/literals/regexp` diagnostic now runs at **140 pass / 40 fail / 58
  skip**, broader `language/literals` improves to **409 pass / 65 fail / 60
  skip**, and `built-ins/RegExp/regexp-modifiers` runs at **37 pass / 33 fail
  / 0 skip** with remaining failures in runtime modifier semantics.
- **RegExp modifiers backend normalization** —
  The internal regex compiler now rewrites ES modifier groups with an empty
  remove-list, such as `(?s-:...)`, to the Rust regex backend's equivalent
  `(?s:...)` form while preserving the public `source` string. Constructor
  validation now uses the same normalized compile path as execution. The
  focused `built-ins/RegExp/regexp-modifiers` run improves to **57 pass / 13
  fail / 0 skip**.
- **RegExp modifier runtime semantics** —
  RegExp backend normalization now tracks modifier-local `s` and `i` state
  while lowering dot, word-boundary, word-character, and Unicode property
  escapes. Non-Unicode `.` observes ES UTF-16 code-unit behavior instead of
  Rust scalar behavior, local `(?-i:...)` word escapes use the ES ASCII word
  set inside and outside character classes, and modifier-local `\p{Lu}`/`\P{Lu}`
  Unicode property probes plus their `Uppercase_Letter` aliases compile inside
  and outside character classes with the expected ignore-case behavior. The focused
  `built-ins/RegExp/regexp-modifiers` run now closes at **70 pass / 0 fail /
  0 skip**.
- **RegExp prototype accessors** —
  `RegExp.prototype.source` and `RegExp.prototype.flags` are now accessor
  properties with spec-shaped getter functions. RegExp instances keep their
  raw source and flags in internal storage, while the public `source` getter
  escapes empty patterns, slashes, and line terminators for literal
  reconstruction and the `flags` getter reads boolean flag accessors in
  `dgimsuvy` order. `$262.createRealm()` now exposes a realm-local `RegExp`
  intrinsic with accessor getters bound to that realm, so
  `%RegExp.prototype.source%` accepts only its own realm prototype. The
  focused `built-ins/RegExp/prototype/flags
  built-ins/RegExp/prototype/source` run now closes at **18 pass / 0 fail / 10
  skip**.
- **Proxy-aware `[[HasProperty]]` for `with`/Reflect** —
  RuJa's internal `[[HasProperty]]` helper now invokes Proxy `has` traps,
  including revoked-proxy failures and basic invariant checks for
  non-configurable or non-extensible target properties. `with`
  object-environment HasBinding, the `in` operator, iterable detection, async
  iterable detection, and `Reflect.has` now share that observable existence
  path. Symbol-key Proxy `get` is also used for `Symbol.unscopables`, and
  `Reflect.get`/`set`/`has` preserve Symbol keys. `Reflect.set` now passes
  through its receiver argument for ordinary data-property writes, so Proxy
  receivers observe `getOwnPropertyDescriptor` and `defineProperty`, returns
  `false` for non-writable receiver/target failures, and propagates Proxy
  `set` trap abrupt completions; the corresponding Reflect wrappers are
  exposed. `Reflect.defineProperty` now returns `false` for failed ordinary
  definitions instead of throwing, propagates abrupt completions while reading
  descriptor fields, and `Reflect.getOwnPropertyDescriptor` observes Proxy
  `getOwnPropertyDescriptor` trap completions. With the `Proxy`/`Reflect`
  skips temporarily lifted, focused
  `language/statements/with built-ins/Reflect/has` runs at **183 pass / 0
  fail / 8 skip**. The focused
  `built-ins/Reflect/get built-ins/Reflect/set built-ins/Reflect/has
  built-ins/Reflect/defineProperty
  built-ins/Reflect/getOwnPropertyDescriptor` diagnostic now runs at **49
  pass / 0 fail / 15 skip**.
- **Proxy `set` trap failure propagation for `with` References** —
  Proxy `[[Set]]` now observes the boolean result of a `set` trap instead of
  treating every non-throwing trap call as success. Strict `PutValue` through a
  Proxy-backed `with` object now throws `TypeError` when that trap returns a
  falsy value, while sloppy assignment continues to fail silently. Simple,
  compound, update, and logical assignment forms all use the preserved
  object-environment Reference for this check. The focused
  `language/statements/with` run remains at **169 pass / 0 fail / 12 skip**.
- **Proxy-aware `[[Delete]]` for delete/Reflect** —
  RuJa's property deletion path now invokes Proxy `deleteProperty` traps with
  the handler as `this`, preserves string and Symbol property keys, falls
  through to nested proxy targets when the trap is null or missing, and
  enforces the non-configurable/non-extensible target invariants for truthy
  trap results. `Reflect.deleteProperty` now rejects primitive targets and
  returns the actual internal `[[Delete]]` boolean instead of always returning
  `true`. `Proxy.revocable()` now revokes through the native callee rather
  than the call receiver, so revoked proxy deletes throw. The test262
  `$262.createRealm()` host now also exposes the constructable `Proxy`
  constructor on the created global. With `Proxy`, `Reflect`, and
  `proxy-missing-checks` skips temporarily lifted, focused
  `built-ins/Reflect/deleteProperty built-ins/Proxy/deleteProperty` runs at
  **25 pass / 0 fail / 3 skip**.
- **Array search array-like access** —
  `Array.prototype.indexOf`, `lastIndexOf`, and `includes` now read
  `length` through `LengthOfArrayLike` and visit indices through
  `HasProperty`/`Get` rather than cloning only dense Array storage. Generic
  calls on ordinary array-like objects, Boolean/Number primitives, boxed
  strings, sparse arrays, and holes now match the spec-observable search
  behavior. Array `length` shrinkage now preserves non-configurable indexed
  own properties, so accessor side effects that try to shorten the receiver do
  not hide those elements from `indexOf`/`lastIndexOf`. The focused
  `built-ins/Array/prototype/includes
  built-ins/Array/prototype/indexOf
  built-ins/Array/prototype/lastIndexOf` cluster runs at **409 pass / 0 fail
  / 20 skip**.
- **Array find array-like access** —
  `Array.prototype.find`, `findIndex`, `findLast`, and `findLastIndex` now
  use `ToObject(this)`, `LengthOfArrayLike`, predicate callability checks, and
  per-index `Get` in spec order instead of cloning dense Array storage before
  iteration. Array-like receivers, nullish receiver errors, throwing
  `length`/index accessors, callback `thisArg`, holes as `undefined`, and
  mutations during traversal are now observable. The focused
  `built-ins/Array/prototype/{find,findIndex,findLast,findLastIndex}`
  diagnostic improves from **38 pass / 24 fail / 32 skip** to **62 pass / 0
  fail / 32 skip**, and the combined Array search/find run closes at
  **471 pass / 0 fail / 52 skip**.
- **Array `at` array-like access** —
  `Array.prototype.at` now applies `ToObject(this)` with nullish receiver
  errors, reads `length` through `LengthOfArrayLike`, uses indexed property
  access for generic array-like receivers, and normalizes `-0` to property key
  `"0"`. The focused `built-ins/Array/prototype/at` run now closes at **11
  pass / 0 fail / 2 skip**.
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
- **String slice/substring argument coercion** —
  `String.prototype.slice` and `substring` now coerce start/end arguments
  through the shared integer coercion path in spec order. `slice` observes
  object `valueOf`/`toString`, propagates abrupt completions from `start`
  before `end`, and treats explicit `undefined` end as the string length.
  `substring` now truncates fractional positions and treats missing or
  explicit `undefined` end as the string length before clamping and swapping.
  The Math intrinsic object is now extensible like an ordinary ECMAScript
  object, so borrowed string methods assigned onto `Math` are callable.
  The focused
  `built-ins/String/prototype/slice
  built-ins/String/prototype/substring` cluster runs at **80 pass / 0 fail /
  4 skip**.
- **String trim whitespace set** —
  `String.prototype.trim`, `trimStart`, and `trimEnd` now trim exactly the
  ECMAScript `WhiteSpace` plus `LineTerminator` set instead of Rust's host
  whitespace predicate. Boundary BOM (`\uFEFF`) is removed, while
  non-ECMAScript whitespace such as `\u180E` and `\u0085` is preserved.
  RegExp objects constructed from RegExp inputs now retain the wrapped pattern
  source/flags, and arguments objects now stringify through their object
  brand instead of RuJa's internal array storage. The
  focused
  `built-ins/String/prototype/trim
  built-ins/String/prototype/trimStart
  built-ins/String/prototype/trimEnd` cluster runs at **145 pass / 0 fail /
  30 skip**.
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
- **Disposal well-known Symbols** —
  `Symbol.dispose` and `Symbol.asyncDispose` are exposed as non-writable,
  non-enumerable, non-configurable static properties, share identity across
  `$262.createRealm()` realms, and remain outside the global Symbol registry.
  The runner keeps `explicit-resource-management` syntax tests skipped, but
  admits the focused `built-ins/Symbol/{dispose,asyncDispose}` intrinsic
  coverage, which runs at **6 pass / 0 fail / 0 skip**. The default
  `built-ins/Symbol` run now reports **47 pass / 0 fail / 51 skip**.
- **Symbol-key function-name inference** —
  Runtime `SetFunctionName` now formats Symbol property keys as
  `[description]`, applies accessor prefixes for `get`/`set`, and leaves
  non-anonymous cover expressions such as `(0, function() {})` unnamed. The
  runner admits only the Symbol-key `fn-name-*` coverage in object literal and
  class-definition directories, so the focused cluster runs at **10 pass / 0
  fail / 5 skip** and the supported subset rises to **5070 pass / 0 fail**.
- **Object spread Symbol keys** —
  Object spread now copies enumerable own Symbol properties and obtains keys
  through the same `[[OwnPropertyKeys]]` ordering used by Object/Reflect
  built-ins: array-index keys first, then string keys, then Symbol keys. It
  re-checks each property descriptor at copy time, so getter side effects can
  affect later keys, and Proxy `ownKeys` failures propagate. The
  runner admits only the generated Symbol object-spread coverage under
  `language/expressions/{array,call,new}/spread-obj-*`; the focused
  `spread-order`, `symbol-property`, and `with-overrides` cases run at **9
  pass / 0 fail**, and the supported subset rises to **5079 pass / 0 fail**.
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
- **`Reflect.ownKeys` Symbol key and Proxy abrupt-completion coverage** —
  `Reflect.ownKeys` now uses RuJa's full own-property-key helper instead of
  the string-only enumerable-key path, so it returns array-index strings,
  ordinary strings, and Symbols in `[[OwnPropertyKeys]]` order and includes
  non-enumerable keys. Primitive targets now throw `TypeError`, and Proxy
  `ownKeys` trap result conversion errors now propagate instead of falling
  back to the target's keys.
  `Symbol.for` also reuses a VM-level global symbol registry for repeat keys.
  With `Proxy`/`Reflect`/`Symbol` skips temporarily lifted, focused
  `built-ins/Reflect/ownKeys` runs at **13 pass / 0 fail / 0 skip**.
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
- **Object assignment shorthand defaults** —
  Object-literal cover grammar now keeps shorthand default forms such as
  `{ x = 1 }` available for simple destructuring assignment
  (`{ x = 1 } = rhs`), while ordinary object literals and compound assignments
  still reject that form as `SyntaxError`. With diagnostic feature skips
  temporarily lifted, `language/expressions/assignment
  language/statements/for-in language/statements/for-of` improves from
  **1009 pass / 220 fail / 122 skip** to
  **1022 pass / 207 fail / 122 skip**.
- **Object prototype receiver coercion** —
  `Object.prototype.valueOf` now applies `ToObject(this)` and rejects nullish
  receivers, while `Object.prototype.toLocaleString` invokes the receiver's
  observable `toString` property instead of sharing
  `Object.prototype.toString` directly. This preserves primitive receivers for
  strict user-defined `toString` methods, propagates getter/call failures, and
  keeps nullish receivers on the `TypeError` path. The focused
  `built-ins/Object/prototype/valueOf
  built-ins/Object/prototype/toLocaleString` cluster now runs at
  **30 pass / 0 fail / 2 skip**, closing 10 full-suite failures.
- **Object legacy accessor methods** —
  `Object.prototype.__defineGetter__`, `__defineSetter__`,
  `__lookupGetter__`, and `__lookupSetter__` are now installed with the
  expected builtin `name`/`length` descriptors. The define methods use the
  spec `ToObject`/callable/key-coercion order and ordinary
  `DefinePropertyOrThrow`, while lookup walks ordinary prototype chains and
  returns accessor functions or `undefined` for data/missing properties. The
  focused legacy accessor cluster now runs at
  **42 pass / 0 fail / 12 skip**.
- **Object prototype `__proto__` accessor and prototype mutation** —
  `Object.prototype` now has a null `[[Prototype]]` and exposes the Annex B
  `__proto__` accessor with `get __proto__` / `set __proto__` function
  descriptors. Ordinary property lookup and assignment now handle
  `__proto__`, so null-prototype objects and own data properties shadow the
  accessor correctly. `Object.setPrototypeOf`, `Reflect.setPrototypeOf`, and
  the legacy setter share the same prototype-mutation status path for
  immutable `Object.prototype`, non-extensible targets, ordinary cycles, and
  the Proxy-shadowed cycle exception. `Object.prototype.isPrototypeOf` also
  follows the specified nullish receiver order. The focused
  `built-ins/Object/prototype` run now closes at
  **191 pass / 0 fail / 57 skip**.
- **`Object.assign` target/source semantics** —
  `Object.assign` now applies `ToObject` to primitive targets, skips nullish
  sources, copies enumerable string and Symbol keys in own-key order, and
  throws `TypeError` when the required `Set(..., Throw=true)` operation
  fails. The shared PropertyKey `[[Get]]`/`[[Set]]` paths now observe array
  dense elements, string exotic indices/`length`, and array receiver
  index/length writes, while Proxy `ownKeys` trap order is preserved for
  normal array key-list results. This closes the focused
  `built-ins/Object/assign` run at **25 pass / 0 fail / 13 skip**.
- **`Object.fromEntries` entry coercion** —
  `Object.fromEntries` now rejects nullish iterables, requires each entry
  value to be an object, reads `entry[0]`/`entry[1]` through ordinary property
  access instead of only unpacking array storage, and preserves Symbol keys via
  `ToPropertyKey`. Boxed string entries such as `Object("ab")` now create
  `{ a: "b" }`, while primitive string entries throw `TypeError`. This closes
  the focused `built-ins/Object/fromEntries` run at
  **11 pass / 0 fail / 14 skip**.
- **`Object.groupBy` static grouping** —
  `Object.groupBy` is now exposed as a static built-in, iterates arbitrary
  sync iterables, calls the grouping callback with `(value, index)`, converts
  callback results with `ToPropertyKey`, preserves Symbol group keys, returns
  a null-prototype result object, and closes custom iterators when
  callback/key coercion abruptly completes. This closes the focused
  `built-ins/Object/groupBy` run at **13 pass / 0 fail / 1 skip**.
- **Native Error constructor shape** —
  Native Error constructors now inherit from `%Error%` instead of directly
  from `%Function.prototype%`, expose own non-enumerable `name`/`length`
  properties, and keep their prototype objects as ordinary objects rather than
  Error-branded instances. This closes the focused
  `built-ins/Object/getPrototypeOf built-ins/NativeErrors` run at
  **118 pass / 0 fail / 15 skip**.
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
- **Private-name delete early errors** —
  Strict/class code now rejects `delete obj.#x` and covered forms such as
  `delete (g().#x)` at parse time instead of compiling them and reaching
  `$DONOTEVALUATE()`. With private class feature skips temporarily lifted, the
  focused `language/statements/class/elements/syntax/early-errors/delete
  language/expressions/class/elements/syntax/early-errors/delete` diagnostic
  improves from **0 pass / 48 fail / 144 skip** to **48 pass / 0 fail / 144
  skip**, and the broader private class early-error diagnostic is now
  **136 pass / 60 fail / 248 skip**. The default supported-subset count is
  unchanged because those private-feature tests remain skipped.
- **Private-bound-name early errors** —
  Class parsing now rejects private names named `#constructor` and duplicate
  private bound names across static and instance elements, while still allowing
  the spec's one private getter plus one private setter exception. With private
  class feature skips temporarily lifted, the broader private class early-error
  diagnostic improves from **136 pass / 60 fail / 248 skip** to **162 pass / 34
  fail / 248 skip**. The default supported-subset count is unchanged because
  those private-feature tests remain skipped.
- **Private-name reference early errors** —
  The parser now applies `AllPrivateNamesValid` after building the AST, so
  class methods, nested functions, nested classes, static blocks, computed
  names, and initializers reject undeclared private-name references while
  preserving lexical access to outer class private names. `super.#x` is
  rejected as a syntax error. With private class feature skips temporarily
  lifted, the broader private class early-error diagnostic improves from
  **162 pass / 34 fail / 248 skip** to **196 pass / 0 fail / 248 skip**. The
  default supported-subset count is unchanged because those private-feature
  tests remain skipped.
- **Private method function names** —
  Private methods now compile their function `name` property with the spec
  `#name` display form instead of the bare identifier while keeping the
  internal private slot key unchanged. With private class feature skips
  temporarily lifted over `language/statements/class language/expressions/class`,
  the diagnostic improves from **934 pass / 170 fail / 7322 skip** to **936 pass
  / 168 fail / 7322 skip**. The default supported-subset count is unchanged
  because those private-feature tests remain skipped.
- **Private async/generator method heads** —
  Class bodies now parse private `async #name()`, `* #name()`, and
  `async * #name()` method heads, including static forms, so they preserve their
  async/generator flags while using the private method lowering path. With
  private class feature skips temporarily lifted over
  `language/statements/class language/expressions/class`, the diagnostic
  improves from **936 pass / 168 fail / 7322 skip** to **948 pass / 156 fail /
  7322 skip**. The default supported-subset count is unchanged because those
  private-feature tests remain skipped.
- **Strict destructuring assignment targets** —
  Strict-mode destructuring assignment patterns now reject `eval` and
  `arguments` targets recursively, including non-declaration `for-in`/`for-of`
  heads. With the `destructuring-binding` diagnostic temporarily lifted over
  `language/expressions/assignment language/statements/for-in
  language/statements/for-of`, the result improves from **1003 pass / 226 fail /
  122 skip** to **1009 pass / 220 fail / 122 skip**. The default
  supported-subset count is unchanged because destructuring-binding tests remain
  skipped.
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
- **Mapped arguments object index writes** — property Reference writes to
  sloppy mapped arguments objects now update the linked parameter binding,
  including writes after `Object.defineProperty(arguments, "0", ...)`. Dense
  arguments indices are treated as own data properties during `[[Set]]`, so
  prototype numeric setters no longer intercept writes to `arguments[0]`. The
  focused `language/arguments-object` run now closes at **126 pass / 0 fail /
  137 skip**.
- **Object destructuring `RequireObjectCoercible`** — empty object assignment
  patterns such as `({} = null)` and rest-only object assignment patterns now
  throw `TypeError` for nullish sources instead of completing silently. With
  only the `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run improves to **249 pass / 11 fail
  / 108 skip** while the default supported subset remains green.
- **Array rest assignment pattern early errors** — assignment destructuring now
  rejects array rest elements followed by another element, elision, another
  rest element, a trailing comma, or an initializer. With only the
  `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run improves to **254 pass / 6 fail /
  108 skip** while the default supported subset remains green.
- **Object shorthand destructuring default function names** — object assignment
  shorthand defaults such as `{ fn = function() {} } = source` now apply
  `SetFunctionName` when the default initializer is an anonymous function,
  arrow function, class, or parenthesized anonymous function. With only the
  `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run improves to **258 pass / 2 fail /
  108 skip** while the default supported subset remains green.
- **Array cover grammar for nested object assignment defaults** — array
  literals that may become assignment patterns now defer nested object-literal
  shorthand initializer early errors until the outer assignment decision is
  known. This lets sloppy nested object defaults such as `[{ x = yield }] =
  vals` and `[...{ x = yield }] = vals` treat `yield` as an identifier in
  assignment patterns while ordinary array literals still reject `{x = ...}`.
  With only the `destructuring-binding` skip lifted for diagnostics, the
  focused `language/expressions/assignment/dstr` run now closes at **260 pass /
  0 fail / 108 skip**.
- **Binding destructuring default function names** — declaration and `for`
  binding patterns now apply `SetFunctionName` when a direct binding
  identifier's default initializer is an anonymous function, arrow function,
  class, or parenthesized anonymous function. With only the
  `destructuring-binding` skip lifted for diagnostics,
  `language/statements/{variable,let,const,for}/dstr` now closes at **412 pass
  / 0 fail / 156 skip** while the default supported subset remains green.
- **For-in/of array rest assignment-pattern early errors** — non-declaration
  `for` heads now reject array assignment patterns where a rest element is
  followed by a comma or elision before `in`/`of`, matching the ordinary
  assignment-pattern early error. With only the `destructuring-binding` skip
  lifted for diagnostics, `language/statements/for-in/dstr` now closes at
  **27 pass / 0 fail / 6 skip**, and `language/statements/for-of/dstr`
  improves to **417 pass / 32 fail / 120 skip** while the default supported
  subset remains green.
- **For-of assignment-pattern cover defaults** — non-declaration `for-of`
  heads now keep object shorthand defaults and nested object defaults in cover
  grammar until the `of` decision is known, so assignment patterns such as
  `for ({ x = 1 } of values)` and `for ([{ x = yield }] of values)` parse and
  execute through the destructuring assignment path. With only the
  `destructuring-binding` skip lifted for diagnostics,
  `language/statements/for-of/dstr` improves to **433 pass / 16 fail / 120
  skip** while the default supported subset remains green.
- **For-of assignment-pattern `in` initializers** — non-declaration `for-of`
  heads now distinguish a top-level `of` delimiter before parsing the left
  side, allowing `in` expressions inside array, shorthand object, and renamed
  object default initializers such as `for ([x = "x" in obj] of values)`.
  With only the `destructuring-binding` skip lifted for diagnostics,
  `language/statements/for-of/dstr` improves to **436 pass / 13 fail / 120
  skip** while the default supported subset remains green.
- **For-of destructuring IteratorClose semantics** — array binding patterns
  now close non-exhausted inner iterators on normal completion and abrupt
  binding/default evaluation, while still skipping close when the iterator
  step itself throws. Array binding initialization also observes deletion of
  `Array.prototype[Symbol.iterator]` instead of falling back to an index
  iterator. With only the `destructuring-binding` skip lifted for diagnostics,
  `language/statements/for-of/dstr` now closes at **449 pass / 0 fail / 120
  skip** while the default supported subset remains green.

## Why the full-suite rate is not higher

The supported subset currently has no known failures. The full-suite rate is
still much lower because the full matrix includes unsupported features such as
ES Modules, Intl, Atomics, full TypedArray prototype method coverage, WeakRef,
and FinalizationRegistry. Those larger feature areas are tracked in
`HANDOFF.md` and will be pulled into support in later milestones.
