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
that RuJa already supports. Async tests remain skipped unless their exact path
is explicitly admitted or `TEST262_RUN_ASYNC=1` enables a diagnostic run. Such
tests inject a host `print` shim and test262's `doneprintHandle.js`, then require
exactly one `Test262:AsyncTestComplete` marker while preserving failure,
missing-marker, process-error, and timeout outcomes.

RuJa does **not** claim full ES conformance. Instead, it targets a
deliberately scoped subset of ES5.1 + selected ES2015+ features (see
[Supported subset](#supported-subset) below). Tests requiring unsupported
features (bare module host resolution, source/defer imports, Intl, etc.) are skipped via the
runner's `SKIP_FEATURES` set. The `explicit-resource-management` feature is
still skipped for syntax/runtime coverage, with a narrow exception for the
already-supported `Symbol.dispose` and `Symbol.asyncDispose` intrinsics.

## Three pass-rate scopes

There are three distinct pass-rate numbers. Each measures a different
scope, so they are not comparable to each other:

| Scope | What it measures | Current rate | Where to verify |
|-------|-----------------|-------------|-----------------|
| **Full suite** | `test262-full` workflow matrix — includes thousands of tests for features RuJa does not support | 62.9% of all matrix files; 83.4% of executed files in the latest confirmed full run | `test262-full` CI workflow job summary |
| **Supported subset** | `language/statements` + `language/expressions` — the areas RuJa actively targets, with unsupported-feature tests skipped | 100.0% (12751 pass / 0 fail on current Test262; 12752 / 0 on the pinned checkout) | Run locally: `TEST262=… python3 tools/test262_runner.py language/statements language/expressions` |
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
WeakRef, FinalizationRegistry, resizable ArrayBuffer and growable
SharedArrayBuffer cores, Promise, Atomics operations including worker
`wait`/`notify`, `waitAsync`, and `pause`, length-tracking TypedArray/DataView
views, `TypedArray.prototype.at`, the four Dynamic Function constructors,
async/await, generators, for-of, optional chaining, nullish coalescing,
logical assignment. TypedArray `fill`, `values`,
`join`, `set`, `subarray`, and default iteration are also included.

**Intentionally unsupported**: bare-specifier ES Module host resolution,
source-phase and deferred imports, Intl, a public multi-agent embedder API,
and tail-call optimization. File-backed relative module graphs, dynamic
imports, namespace objects, and JSON/text import attributes are supported in
the frozen module slice.
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

# Run the admitted async object-method path:
TEST262=/path/to/test262 \
  python3 tools/test262_runner.py language/expressions/object/method-definition

# Diagnose async tests outside admitted paths without changing their boundary:
TEST262=/path/to/test262 TEST262_RUN_ASYNC=1 \
  python3 tools/test262_runner.py language/expressions/async-arrow
```

For failure-bucket analysis with error samples, use the sibling analyzer:

```sh
python3 tools/test262_analyze.py
```

The focused analyzer mirrors the runner's `raw`, `onlyStrict` directive
prologue, and `negative:` metadata handling, so strict-mode and parse-negative
tests are not reported as false failure buckets.

## Private class boundary admission

`tools/test262_class_private_admission.txt` freezes **37** class declaration
and expression files whose remaining skip reason was an exact private class
feature gate. The runner and analyzer remove only the feature tags recorded for
each exact path; the broad private field and method gates remain active for all
other files.

Four files in this set require a parser early error: a private getter and setter
may share a name only when both are static or both are non-static. RuJa records
that staticness with each private bound name, rejects all four mismatched
orders, and continues to accept complementary accessor pairs in either order.
The other 33 files cover repeated private method, accessor, and static field
class evaluations plus private static-block scope.

Local verification reports **37 pass / 0 fail** for the frozen manifest. On
test262 `d1d583db95a521218f3eb8341a887fd63eda8ff1`, the supported subset is
**12400 pass / 0 fail / 8039 skip / 20439 total**. The current upstream
checkout `020cb74075849d1e404bbcdb62feb7a02e6966db` reports **12399 pass / 0
fail / 8039 skip / 20438 total**.

At commit `cf398c0`, CI `29316245920` and full matrix `29316245922`
succeeded. Expressions moved by **+32 pass / -32 skip** and statements by
**+5 pass / -5 skip**; the other 28 result artifacts are byte-for-byte
identical to matrix `29312667278`. The aggregate is **28704 pass / 6614 fail
/ 12987 skip / 12 timeout / 0 error / 48317 total / 35318 pass-or-fail
executed**.

## Class default-parameter admission

`tools/test262_class_default_parameter_admission.txt` freezes the exact **56**
class declaration and expression files whose only remaining skip reason was
`default-parameters`. The runner and analyzer remove that feature gate only
for manifest members; unrelated default-parameter tests remain behind the
broad gate.

Two parse-negative getter files exposed a shared accessor grammar defect.
Object, public class, and private class accessors previously reused the general
function parameter parser without enforcing accessor arity. All three paths
now require zero getter parameters and exactly one non-rest setter parameter,
while valid setter defaults and destructuring remain supported.

Local verification reports **56 pass / 0 fail** for the frozen manifest. On
test262 `d1d583db95a521218f3eb8341a887fd63eda8ff1`, the supported subset is
**12456 pass / 0 fail / 7983 skip / 20439 total**. The current upstream
checkout `020cb74075849d1e404bbcdb62feb7a02e6966db` reports **12455 pass / 0
fail / 7983 skip / 20438 total**.

At commit `99db3dc`, CI `29322169799` and full matrix `29322169773`
succeeded. Expressions and statements each moved by **+28 pass / -28 skip**;
the other 28 result artifacts are byte-for-byte identical to matrix
`29317703061`. The aggregate is **28760 pass / 6614 fail / 12931 skip / 12
timeout / 0 error / 48317 total / 35374 pass-or-fail executed**.

## Class destructuring admission

`tools/test262_class_destructuring_admission.txt` freezes the exact **272**
generated class declaration and expression files whose only remaining skip
reason was `destructuring-binding`. The set is the cross product of 136
ordinary/static method parameter-pattern cases with the two class forms. The
runner and analyzer remove only that feature gate for exact manifest members;
private methods, generators, defaults, iterator overrides, object rest, and
unrelated destructuring tests retain their independent gates.

Local verification reports **272 pass / 0 fail** for the frozen manifest. On
test262 `d1d583db95a521218f3eb8341a887fd63eda8ff1`, the supported subset is
**12728 pass / 0 fail / 7711 skip / 20439 total**. The current upstream
checkout `020cb74075849d1e404bbcdb62feb7a02e6966db` reports **12727 pass / 0
fail / 7711 skip / 20438 total**.

At commit `3fe3343`, CI `29326685124` and full matrix `29326685148`
succeeded. Expressions and statements each moved by **+136 pass / -136
skip**; the other 28 result artifacts are byte-for-byte identical to matrix
`29323953926`. The aggregate is **29032 pass / 6614 fail / 12659 skip / 12
timeout / 0 error / 48317 total / 35646 pass-or-fail executed**.

## JSON.parse reviver admission

`tools/test262_json_parse_admission.txt` freezes all **77** files under
`built-ins/JSON/parse`. The runner and analyzer lift otherwise broad Proxy,
Reflect, Symbol, and `json-parse-with-source` feature gates only for those exact
paths. The corresponding implementation uses the root holder and ordinary
property operations for `InternalizeJSONProperty`, preserves reviver mutations
and abrupt completions, and supplies `context.source` only when a primitive
still has the parsed value associated with its original source record.

At commit `caa689c`, local verification on test262
`d1d583db95a521218f3eb8341a887fd63eda8ff1` reports **77 pass / 0 fail / 0
skip** for the frozen JSON.parse set. The supported subset remains **12232 pass
/ 0 fail / 8207 skip / 20439 total**, and the Python tooling suite is **61/61**.
CI `29211878288` and `test262-full` `29211878312` both pass. The 30 downloaded
artifacts aggregate to **28424 pass / 6675 fail / 13206 skip / 12 timeout / 0
error / 48317 total / 35099 pass-or-fail executed**, or **58.8%** of all matrix
files and **81.0%** of executed files. Relative to the preceding confirmed
matrix this is **+26 pass / -14 fail / -12 skip**; the extra pass beyond the 25
newly admitted JSON.parse files comes from the shared property-invariant fixes.

## JSON.stringify serialization admission

`tools/test262_json_stringify_admission.txt` freezes all **66** files under
`built-ins/JSON/stringify`. Exact-path admission lifts Proxy, Reflect, Symbol,
and cross-Realm metadata only for this audited directory. The implementation
uses the specification's holder-based `SerializeJSONProperty` order, ordinary
Get and own-key operations, post-transformation active-stack cycle checks, and
fallible user callbacks. Replacer arrays are deduplicated in observed order;
boxed values and indentation use normal coercion; JSON string quoting operates
on UTF-16 code units and escapes lone surrogates.

At commit `ac708fd`, local verification on test262
`d1d583db95a521218f3eb8341a887fd63eda8ff1` reports **66 pass / 0 fail / 0
skip** for the frozen stringify set. The supported subset remains **12232 pass
/ 0 fail / 8207 skip / 20439 total**, and the Python tooling suite is **62/62**.
CI `29213833296` and `test262-full` `29213833314` both pass. The 30 downloaded
artifacts aggregate to **28468 pass / 6646 fail / 13191 skip / 12 timeout / 0
error / 48317 total / 35114 pass-or-fail executed**, or **58.9%** of all matrix
files and **81.1%** of executed files. The exact stringify slice contributes
**+45 pass / -30 fail / -15 skip**; one unrelated built-ins result varied from
the preceding run, so the observed aggregate delta is **+44 / -29 / -15**.

## Raw JSON admission

`tools/test262_json_raw_admission.txt` freezes **17** files covering
`JSON.rawJSON`, `JSON.isRawJSON`, and `JSON[Symbol.toStringTag]`. Exact-path
admission lifts `json-parse-with-source`, Reflect construction, and Symbol tag
metadata only for those audited files. Raw values use an internal brand that
cannot be forged with a `rawJSON` property, expose a frozen enumerable data
property on a null-prototype object, and preserve the validated primitive JSON
text verbatim through stringify.

At commit `196e8fd`, local verification on test262
`d1d583db95a521218f3eb8341a887fd63eda8ff1` reports **17 pass / 0 fail / 0
skip**. Validation handles large numeric spellings and escaped lone UTF-16
surrogates without normalizing the stored source. The supported subset remains
**12232 pass / 0 fail / 8207 skip / 20439 total**, and the Python tooling suite
is **63/63**.
The complete `built-ins/JSON` directory is now **165/165**. CI `29215737017`
and `test262-full` `29215737023` both pass. The 30 downloaded artifacts
aggregate to **28487 pass / 6629 fail / 13189 skip / 12 timeout / 0 error /
48317 total / 35116 pass-or-fail executed**, or **59.0%** of all matrix files
and **81.1%** of executed files. This is **+19 pass / -17 fail / -2 skip** from
the preceding confirmed matrix; the JSON tag also fixes shared behavior outside
the 17-file frozen admission.

## Date Symbol.toPrimitive admission

`tools/test262_date_to_primitive_admission.txt` freezes all **18** tests under
`built-ins/Date/prototype/Symbol.toPrimitive`. The intrinsic is installed with
the specified name, length, and descriptor, validates the three exact hint
strings, and performs generic ordinary conversion in string-first order for
`"default"` and `"string"` and number-first order for `"number"`. Property
access, callable Proxy methods, calls, primitive-result selection, and abrupt
completion remain observable in specification order. Deleting the configurable
intrinsic now correctly falls back to ordinary number-first default conversion
instead of retaining a Date-specific VM shortcut.

At commit `8939b5e`, local verification on test262
`d1d583db95a521218f3eb8341a887fd63eda8ff1` reports **18 pass / 0 fail / 0
skip**. The supported subset remains **12232 pass / 0 fail / 8207 skip / 20439
total**, and the Python tooling suite is **64/64**. CI `29218430070` and
`test262-full` `29218430199` both pass. The 30 downloaded artifacts aggregate
to **28502 pass / 6615 fail / 13188 skip / 12 timeout / 0 error / 48317 total /
35117 pass-or-fail executed**, or **59.0%** of all matrix files and **81.2%** of
executed files. This is **+15 pass / -14 fail / -1 skip** from the preceding
confirmed matrix.

## Identifier Reference-record routing

Ordinary `Expr::Ident` reads now compile to `LoadRef` plus `GetValue` instead
of the legacy value-producing `LoadEnvName` opcode. Identifier lookup therefore
shares one Reference-record implementation across ordinary reads, assignments,
compound and logical assignments, updates, TDZ checks, imports, globals, and
`with` object environments. Calls and direct `eval` continue to use their
dedicated Reference-preserving call opcodes so object-environment `this` and
direct-eval classification are not discarded.

At commit `f63145d`, the combined diagnostic over
`language/types/reference`, `language/statements/with`, and
`language/expressions/compound-assignment` reports **660 pass / 0 fail / 4
skip / 664 total** under the normal feature policy. With skips lifted only in
memory, `with` is **181/181** and compound assignment is **454/454**; the three
remaining failures are primitive-base and cross-Realm Reference cases outside
this routing change. The supported subset remains **12232 pass / 0 fail / 8207
skip / 20439 total**, and tooling remains **64/64**. CI `29220379603` and
`test262-full` `29220379613` both pass. The 30 downloaded artifacts reproduce
**28502 pass / 6615 fail / 13188 skip / 12 timeout / 0 error / 48317 total /
35117 pass-or-fail executed**, or **59.0%** of all matrix files and **81.2%** of
executed files.

This was the first consolidation step, not the end of the Reference migration
at commit `f63145d`. At that point compiler-internal completion slots still
used `LoadEnvName`, and direct member get/set and delete retained separate VM
paths that were migrated in later bounded units.

## Reference-record routing completion

The final value-producing `LoadEnvName` opcode and its independent environment
resolver are now removed. A `continue` that exits a switch copies the switch's
saved completion through `LoadRef` and `GetValue`, preserving `UpdateEmpty`
semantics without a parallel identifier lookup implementation. Each nested
switch receives a unique completion binding, and the compiler pops the
`StoreEnvName` result to keep repeated continues stack-balanced. Break and
continue scope unwinding now runs from a post-finally trampoline, so a finally
body can still observe the switch and block environments it guards. The
IteratorClose path resolves that trampoline to the semantic loop target, so a
same-loop `for...of` continue still does not close its iterator. The focused
switch directory is **69 pass / 0 fail / 42 skip / 111 total**.

A fresh source/opcode audit found no remaining high- or medium-risk bypass for
ordinary identifier, member, private, or `super` reads, calls, assignments,
compound/logical assignments, updates, deletes, destructuring targets, or
`for-in`/`for-of` targets. `typeof` on an unresolvable identifier, binding
initialization, `super()` construction, and decorator `context.access`
closures retain dedicated operations because they are distinct specification
operations rather than expression Reference evaluation. `StoreEnvName`
remains as a compiler write helper for internal `#` bookkeeping and a small
number of generated ordinary-name stores such as function declarations; its
ordinary-name branch constructs a Reference before `PutValue` and does not
reintroduce a value-producing identifier resolver.

Current Test262 `020cb740` reports **663 pass / 0 fail / 1 skip / 664 total**
for `language/types/reference`, `language/statements/with`, and
`language/expressions/compound-assignment`. The supported subset remains
**12751 pass / 0 fail / 7687 skip / 20438 total**. Combined with the switch
directory, the focused gate is **732 pass / 0 fail / 43 skip / 775 total**.
Rust all-targets/all-features, control flow **59/59**, Clippy with warnings
denied, rustfmt, and Python tooling **84/84** also pass.

Commits `00994c7` and `bbfa6f2` passed CI `29370269695` and full matrix
`29370269812`. All 30 Test262 result artifacts are byte-for-byte identical to
the Iterator documentation baseline, retaining **29085 pass / 6495 fail /
12725 skip / 12 timeout / 0 error / 48317 total / 35580 pass-or-fail
executed**. Artifacts are retained at
`/tmp/ruja-artifacts-reference-routing-feature.Qw4b7N`.

## Primitive Reference Realm admission

`tools/test262_reference_primitive_admission.txt` freezes the three
`language/types/reference` tests for primitive-base property `GetValue` and
`PutValue`, including their cross-Realm forms. The VM records each Realm's
global object and intrinsic primitive prototypes as GC roots. Property reads,
boxing, and writes select the current execution or native callee Realm;
primitive writes run ordinary `[[Set]]` with the original primitive receiver,
so inherited setters and Proxy `set` traps remain observable.

Child test262 Realms now expose realm-bound Object constructors and independent
BigInt and Symbol constructors/prototypes. BigInt and Symbol mutations no longer
leak into the main Realm, and the BigInt `Symbol.toStringTag` descriptor is
preserved. String and `PropertyKey` setter paths share one traversal state:
cycles are detected across Proxy targets and ordinary prototypes, Proxy
recursion is bounded independently, and ordinary chains retain the prior
1024-hop budget.

At follow-up commit `5f78f18`, `language/types/reference` is **28 pass / 0 fail
/ 1 skip / 29 total**, and the combined Reference/with/compound diagnostic is
**663 pass / 0 fail / 1 skip / 664 total**. The supported subset remains
**12232 pass / 0 fail / 8207 skip / 20439 total**, and tooling is **65/65**.
CI `29224760629` and `test262-full` `29224760619` both pass. The 30 downloaded
artifacts aggregate to **28506 pass / 6614 fail / 13185 skip / 12 timeout / 0
error / 48317 total / 35120 pass-or-fail executed**, or **59.0%** of all matrix
files and **81.2%** of executed files. The three admitted files account for
**+3 pass / -3 skip**; realm-bound Object boxing also converts one existing
built-ins failure to a pass, making the total movement **+4 pass / -1 fail / -3
skip**.

Independent review found and the implementation fixed unbounded Proxy setter
recursion, Symbol/BigInt prototype aliasing, missing BigInt tagging, the
parallel `Reflect.set` string-key recursion path, and an accidental reduction
of the ordinary prototype depth budget. The first feature full run also exposed
a cross-Realm JSON BigInt wrapper regression caused by a shared Object
constructor; commit `5f78f18` binds Object boxing to the callee Realm and
restores that test.

## Member-read Reference routing and Proxy Get admission

Ordinary computed and non-computed member reads now compile to
`MakePropertyRef` plus `GetValue`; optional-chain member reads use the same
property Reference path after their nullish short circuit. Reference creation
checks a nullish base before property-key coercion and pins the base, key, and
result across observable conversion. Receiver-sensitive calls and tagged
templates intentionally retain their dedicated path until property References
can provide their `this` value directly; `super` also remains separate because
its base and `thisValue` differ.

The shared `[[Get]]` implementation forwards string and Symbol keys with the
original receiver through nested Proxies and `Reflect.get`, treats null and
undefined traps as absent, and validates non-configurable data and accessor
invariants against Proxy-aware target descriptors. Proxy
`[[GetOwnProperty]]` compatibility, configurability, and extensibility checks
support that validation. String exotic descriptors expose their actual length
and UTF-16 code-unit values, and targets, handlers, receivers, traps, results,
and descriptor fields remain rooted across observable calls.

`tools/test262_proxy_get_admission.txt` freezes **30** exact files under
`built-ins/Proxy/get` and `built-ins/Reflect/get`; all are **30 pass / 0 fail**
against test262 `d1d583db95a521218f3eb8341a887fd63eda8ff1`. The first full run exposed
two unrelated mapped-arguments regressions: the new Reference read cache held
the initial `arguments[0]` value instead of consulting its live parameter map.
Follow-up commit `50b84f8` excludes arguments exotic objects from that cache
and restores `language/arguments-object` to **126 pass / 0 fail / 137 skip**.

Local gates pass: Rust all-targets, clippy with denied warnings, fmt/diff,
tooling **66/66**, the combined arguments/Proxy/Reflect gate **156 pass / 0
fail / 137 skip / 293 total**, and the supported subset **12232 pass / 0 fail /
8207 skip / 20439 total**. CI `29236604702` and `test262-full` `29236604723`
both pass for the follow-up. All 30 downloaded artifacts aggregate to **28536
pass / 6614 fail / 13155 skip / 12 timeout / 0 error / 48317 total / 35150
pass-or-fail executed**, or **59.1%** of all matrix files and **81.2%** of
executed files. Relative to the preceding confirmed matrix this is exactly
**+30 pass / -30 skip**.

## Non-optional member-call Reference routing

Ordinary direct and spread member calls now create one property Reference,
duplicate it for `GetValue`, and retain the original until
`CallRef`/`CallRefSpread` derives the call's `this` value. The VM's
`GetThisValue` equivalent returns the original `ReferenceBase::Value`, including
primitive bases, while preserving the existing object-environment behavior for
identifier calls inside `with`. Environment and unresolvable identifier
References continue to call with `undefined`.

This preserves base and computed-key evaluation order, performs exactly one
property read before argument evaluation, and delays callability checking until
after arguments have been evaluated. The Reference remains on the VM stack, so
its nested base participates in GC tracing across getters, Proxy traps, spread
iteration, and argument evaluation. A forced-GC regression uses a temporary
object expression with no other owner and verifies that the method still
receives the live object as `this` after an argument callback collects garbage.

The unit intentionally excludes optional calls, private calls, tagged
templates, and `super`. Optional calls still need their two distinct nullish
stack exits migrated together. A `super` Reference needs separate `[[Base]]`
and `[[ThisValue]]` fields, which the current Reference representation does not
yet encode.

At commit `9686f03`, the combined `language/expressions/call`, frozen
Proxy/Reflect `[[Get]]`, `language/types/reference`, and
`language/statements/with` gate is **307 pass / 0 fail / 25 skip / 332 total**.
Rust all-targets, clippy with denied warnings, fmt/diff, tooling **66/66**, and
the supported subset **12232 pass / 0 fail / 8207 skip / 20439 total** all
pass. Independent review found no correctness, stack-shape, receiver, rooting,
or scope-boundary issue. CI `29240428689` and `test262-full` `29240428617`
both succeed; all 30 downloaded artifacts exactly reproduce **28536 pass /
6614 fail / 13155 skip / 12 timeout / 0 error / 48317 total / 35150
pass-or-fail executed**, with no changed shard.

## Optional member-call Reference routing

Optional member calls now use the same retained property Reference as ordinary
member calls. The compiler emits `MakePropertyRef`, duplicates the Reference
for one `GetValue`, then selects `CallRef` or `CallRefSpread` after optional
nullish checks. This covers `o?.m()`, `o.m?.()`, `(o?.m)()`, and
`(o?.m)?.()` without changing private or `super` call paths.

The base-nullish exit consumes the single unevaluated base and skips computed
key and argument evaluation. Once a Reference and callee exist, the
callee-nullish exit consumes both stack values and skips arguments. A grouped
non-optional call such as `(null?.m)(argument())` intentionally continues,
evaluates its arguments, and then throws for the undefined callee; the optional
grouped form skips them. `compile_optional_chain_call_target` reports whether
its pair is `[reference, callee]` or `[explicitThis, callee]`, so member targets
use `CallRef` while private and non-Reference targets retain `CallThis`.

Regressions cover computed keys, Proxy getters, strict receivers, primitive
bases, direct and spread arguments, all four grouped/un-grouped forms, both
nullish exits, and forced GC while an optional spread argument is evaluated.
Independent review found no stack-shape, short-circuit, receiver, rooting, or
scope-boundary issue; grouped optional `super` calls also retain their existing
explicit receiver path.

At commit `81b50cf`, the combined optional-chaining, call, frozen Proxy/Reflect
`[[Get]]`, Reference, and `with` gate is **345 pass / 0 fail / 25 skip / 370
total**. Rust all-targets, operators **114/114**, clippy with denied warnings,
fmt/diff, tooling **66/66**, and the supported subset **12232 pass / 0 fail /
8207 skip / 20439 total** pass. CI `29244031712` and `test262-full`
`29244032070` both succeed; all 30 downloaded artifacts exactly reproduce
**28536 pass / 6614 fail / 13155 skip / 12 timeout / 0 error / 48317 total /
35150 pass-or-fail executed**, with no changed shard. `GetMethodForCall` now
remains only in the tagged-template compiler path.

## Tagged-template Reference routing

Tagged templates now preserve the specification Reference produced while
evaluating identifier, ordinary member, and private tags. Identifier tags emit
`LoadRef`, member/private tags construct the corresponding Reference, and each
path duplicates it for one `GetValue` before `CallRef` derives `this`. This
fixes strict identifier tags inside `with`, which previously received
`undefined` instead of the with object, while ordinary lexical identifier tags
continue to receive `undefined`.

`super` remains an explicit-receiver path because the current Reference record
cannot represent a `[[Base]]` distinct from `[[ThisValue]]`. The compiler keeps
the derived instance below the result of `GetSuperProp` and invokes the tag via
`CallThis`. This fixes `super` tagged templates with and without substitutions when
the property is a method or getter. Non-Reference expression tags remain on the
unbound `Call` path; parentheses preserve a member Reference while a comma
expression intentionally discards it.

Tag `GetValue` completes before `GetTemplateObject` and interpolation
evaluation. The retained Reference and callee remain VM-stack GC roots, so a
temporary member base and a getter-returned temporary function survive forced
GC from an interpolation. Regressions cover identifier/`with`, computed and
Symbol Proxy properties, primitive and private bases, `super` getters,
parenthesized/unbound distinctions, strict receivers, evaluation order, and
forced GC. Independent review found no functional issue; every identified
low-risk test gap was added before commit.

At commit `4f6975f`, the combined tagged-template, optional-chaining, call,
frozen Proxy/Reflect `[[Get]]`, Reference, and `with` gate is **370 pass / 0
fail / 27 skip / 397 total**. Rust all-targets, ES2015 **112/112**, clippy with
denied warnings, fmt/diff, tooling **66/66**, and the supported subset **12232
pass / 0 fail / 8207 skip / 20439 total** pass. CI `29247565071` and
`test262-full` `29247565062` both succeed; all 30 downloaded artifacts exactly
reproduce **28536 pass / 6614 fail / 13155 skip / 12 timeout / 0 error / 48317
total / 35150 pass-or-fail executed**, with no changed shard. No compiler or VM
reference to the obsolete `GetMethodForCall` opcode remains.

## Super Reference routing

Super property reads, direct and spread calls, optional calls, and tagged
templates now share a `MakeSuperPropertyRef` path. The Reference stores the
dynamic super base in `[[Base]]` and the current method receiver in a distinct
`[[ThisValue]]`; `GetValue` uses the latter as the `[[Get]]` receiver and
`CallRef` uses it as the call receiver. The dedicated `CallSuper` opcode is no
longer needed.

For computed properties, the compiler first obtains the current `this`, then
evaluates the key expression, obtains the HomeObject prototype, and finally
coerces the key while constructing the Reference. This preserves the required
ordering when the key expression or its coercion changes the HomeObject
prototype. Null super bases throw before property lookup for both string and
Symbol keys, while optional calls still skip their arguments only after a
successful lookup produces a nullish callee.

Interpreted concise methods and accessors now keep an immutable
`[[HomeObject]]` slot. Calls bind `#super` from that slot rather than from the
dynamic receiver, including borrowed methods with primitive `this` values and
object methods nested inside class methods. Copying an existing method into a
new ordinary property does not replace its HomeObject. Function HomeObjects
and Reference bases/this-values participate in heap tracing and temporary
pinning, so forced GC during key coercion, Proxy/getter work, arguments,
spread, or template interpolation cannot collect them.

At commit `aa83f7e`, Rust all-targets, clippy with denied warnings, fmt/diff,
ES2015 **118/118**, classes **60/60**, operators **114/114**, focused Test262
**214 pass / 0 fail / 45 skip / 259 total**, and the supported subset **12232
pass / 0 fail / 8207 skip / 20439 total** pass. CI `29252936209` succeeds. The
full matrix `29252935590` also succeeds after rerunning shards whose initial job
setup hit a GitHub Actions internal-server error. All 30 downloaded result files
are byte-for-byte identical to baseline run `29249047154`, retaining the
normalized **28536 pass / 6614 fail / 13155 skip / 12 timeout / 0 error / 48317
total / 35150 pass-or-fail executed** aggregate with no shard movement. The
remaining super stores, compound/logical assignments, updates, and delete stay
on their bounded legacy paths; they are the next Reference migration unit.

## Super write Reference routing

Simple and destructuring assignment, numeric compound assignment, logical
assignment, prefix/postfix update, and delete now all evaluate a super property
as a Reference with distinct base and actual-this components. `PutValue` uses
the latter as the `[[Set]]` receiver, so inherited data properties create or
update the borrowed/inherited receiver rather than the method's HomeObject.
Primitive actual-this values remain unboxed when invoking inherited setters.

Computed super References initially keep an `UncoercedProperty` referenced
name. This matches the current specification ordering: the key expression runs,
the super base is captured, and simple assignment evaluates its RHS before
`PutValue` performs `ToObject(base)` and then `ToPropertyKey(name)`. A null base
therefore evaluates the RHS but rejects before key coercion. Delete evaluates
the raw Reference and throws `ReferenceError` without coercion. Compound,
logical, and update forms insert `ResolvePropertyRef` before duplicating the
Reference, ensuring key coercion happens exactly once before `GetValue` and the
same resolved Reference reaches `PutValue`.

Deferred names participate in ordinary heap tracing, VM stack rooting, explicit
pinning, and environment tracing for destructuring temporaries. Regressions
cover captured-base/RHS/coercion order, null bases, computed and Symbol keys,
simple/compound/logical/prefix/postfix/destructuring forms, short circuiting,
primitive and inherited receivers, and forced GC during RHS and source getter
evaluation. `GetSuperProp` and `SetSuperProp` no longer exist in bytecode,
compiler, or VM code.

At commit `e0bd2a4`, Rust all-targets, clippy with denied warnings, fmt/diff,
ES2015 **124/124**, classes **60/60**, operators **114/114**, focused Test262
**1299 pass / 0 fail / 23 skip / 1322 total**, and the supported subset **12232
pass / 0 fail / 8207 skip / 20439 total** pass. Independent review found no
remaining super write/update/delete correctness issue; the next Reference
boundary is the remaining ordinary non-super member fallback paths. CI
`29257232329` and full matrix `29257232453` succeeded. All 30 full-matrix
artifacts are byte-for-byte identical to the preceding confirmed matrix, so the
normalized aggregate remains **28536 pass / 6614 fail / 13155 skip / 12 timeout
/ 0 error / 48317 total / 35150 pass-or-fail executed** with no shard movement.

## Ordinary member assignment References

Simple assignment, destructuring assignment targets, and non-declaration
`for-in`/`for-of` member targets now create a raw property Reference as soon as
their base and referenced name have been evaluated. That Reference remains live
through RHS evaluation, destructuring source access, or loop-value storage and
is passed directly to `PutValue`. This preserves the specification's delayed
`ToPropertyKey` for `a[b] = c` while rooting temporary base and key objects
through observable calls and forced collection.

Destructuring now stores one Reference temporary instead of separate base and
key temporaries. The assignment-only `MakePropertyRefForSet` bytecode and VM
handler are removed; ordinary read/call paths retain their resolved
`MakePropertyRef` behavior, keeping this migration bounded to assignment.
Regressions force GC while temporary base and key objects are retained only by
simple and destructuring assignment References.

At commit `345e3f3`, Rust all-targets, clippy with denied warnings, fmt/diff,
operators **115/115**, ES2015 **124/124**, destructuring **45/45**, and control
flow **55/55** pass. Latest Test262 assignment/destructuring/for-in/for-of paths
are **1169 pass / 0 fail / 190 skip / 1359 total**, and the pinned supported
subset remains **12232 pass / 0 fail / 8207 skip / 20439 total**. Independent
review found the next bounded correctness issue in ordinary delete: primitive
string index deletion bypasses non-configurable wrapper properties, and key
coercion can unroot a temporary base. CI `29260495441` and full matrix
`29260497188` succeeded. All 30 artifacts are byte-for-byte identical to the
preceding confirmed matrix, so the normalized aggregate remains **28536 pass /
6614 fail / 13155 skip / 12 timeout / 0 error / 48317 total / 35150
pass-or-fail executed** with no shard movement.

## Ordinary property delete References

Direct and optional-chain property delete now compile their evaluated base and
referenced name into `MakeRawPropertyRef`, followed by `DeleteValue`. The VM
performs `ToObject(base)` before `ToPropertyKey(name)`, calls the resulting
object's `[[Delete]]`, and converts a false result to `TypeError` from the
Reference's stored strict flag. Nullish optional-chain exits still skip the key
and produce `true`; ordinary nullish bases evaluate the key expression but
reject before key coercion.

The retained Reference and boxed/object base stay pinned through key coercion,
Proxy traps, and error paths. Primitive String indices and `length` now use the
String wrapper's non-configurable properties, so sloppy delete returns `false`
and strict delete throws. String exotic read, has, own-property, and delete
paths share canonical index recognition: `"01"`, `"00"`, `"+0"`, `"1e0"`, and
`"-0"` remain ordinary property names rather than aliases for character
indices. The legacy `DeleteProp` bytecode and VM handler are removed.

Proxy `deleteProperty` invariants now query a Proxy target through its actual
`[[GetOwnProperty]]` and `[[IsExtensible]]` operations. Nested target traps are
therefore observable, and an outer trap cannot report successful deletion of a
non-configurable property or a present property on a non-extensible target.
Regressions cover both invariants, strict optional delete trap failure,
temporary Proxy/key forced GC, primitive and boxed Strings, canonical and
non-canonical names, Symbols through the focused suite, and nullish ordering.

At commit `cba970d`, Rust all-targets, clippy with denied warnings, fmt/diff,
and operators **118/118** pass. Latest Test262 delete and optional-chaining paths
are **107 pass / 0 fail / 0 skip / 107 total**, and the pinned supported subset
remains **12232 pass / 0 fail / 8207 skip / 20439 total**. Independent review's
two functional findings, nested Proxy invariants and non-canonical String
indices, are fixed and covered before commit. CI `29263990433` and full matrix
`29263989422` succeeded. The first literals job attempt had one transient
timeout while local file-by-file and full literals reruns remained **474 pass /
60 skip / 0 timeout**; rerunning that job in the same workflow restored the
baseline. The final 30 artifacts are byte-for-byte identical to the preceding
confirmed matrix, so the normalized aggregate remains **28536 pass / 6614 fail
/ 13155 skip / 12 timeout / 0 error / 48317 total / 35150 pass-or-fail
executed** with no shard movement.

## Legacy property store removal

The final compiler fallback that could emit direct `SetProp` or `SetElem`
stores has been removed together with both bytecodes and VM handlers. Its stack
shape expected `[base, key, value]` while its only compiler construction would
have produced `[value, base, key]`; all valid member AST forms already bypassed
it through dedicated Reference paths. Identifier and private fallback handling
remain for their own valid bytecodes.

Source-level simple assignment, destructuring, non-declaration `for-in` and
`for-of`, compound/logical assignment, and prefix/postfix update therefore have
no legacy direct-property store path. A regression covers parenthesized
computed compound, logical, postfix, and prefix targets through Proxy get/set
traps, proving one key evaluation and one retained receiver per operation.

At commit `ea8e492`, Rust all-targets, clippy with denied warnings, fmt/diff,
operators **119/119**, and a source search with zero `SetProp`/`SetElem`
references pass. Latest Test262 assignment, compound-assignment, and
logical-assignment paths are **1017 pass / 0 fail / 0 skip / 1017 total**; the
pinned supported subset remains **12232 pass / 0 fail / 8207 skip / 20439
total**. CI `29266489197` and full matrix `29266489031` succeeded. All 30
artifacts are byte-for-byte identical to the preceding confirmed matrix, so the
normalized aggregate remains **28536 pass / 6614 fail / 13155 skip / 12 timeout
/ 0 error / 48317 total / 35150 pass-or-fail executed** with no shard movement.

## RegExp literal timeout boundary

Four legacy RegExp literal stress files take **3.8-6.3 seconds** in repeated
local file-by-file measurements and crossed the runner's 8-second timeout only
under concurrent CI load:

- `language/literals/regexp/S7.8.5_A1.1_T2.js`
- `language/literals/regexp/S7.8.5_A1.4_T2.js`
- `language/literals/regexp/S7.8.5_A2.1_T2.js`
- `language/literals/regexp/S7.8.5_A2.4_T2.js`

Runner and analyzer now select a 20-second timeout for exactly those files.
Ordinary tests remain at 8 seconds, while the existing 600-second TypedArray
copyWithin stress exception remains independent. Tooling regressions verify
inside, ordinary same-directory, and outside-path boundaries in both tools.

At commit `d6b142f`, tooling **67/67**, Rust all-targets, clippy with denied
warnings, and fmt/diff pass. The latest complete literals run is **474 pass / 0
fail / 60 skip / 0 timeout / 534 total**. CI `29269378405` and full matrix
`29269378378` succeeded without a literals rerun. All 30 artifacts are
byte-for-byte identical to the preceding confirmed matrix, so the normalized
aggregate remains **28536 pass / 6614 fail / 13155 skip / 12 timeout / 0 error
/ 48317 total / 35150 pass-or-fail executed** with no shard movement.

## Interpreted runtime Error Realms

Catchable Rust runtime errors raised by interpreted code are now converted to
JavaScript Error objects while the responsible frame is still active. The VM
uses that frame's global environment for the Error prototype, independently of
any enclosing native callee. Native functions continue to use their own Realm,
and errors that already carry an explicit thrown value are never recreated.

The same frame-boundary materialization is applied to ordinary functions,
generator parameter prologues and resumes, async functions before and after
`await`, async generators, and async module evaluation. Regressions cover a
foreign interpreted callback catching inside `Array.prototype.every`, borrowed
main-Realm generator methods, generator parameter initialization, async and
async-generator rejection after `await`, explicit throw identity, and forced
GC. Independent review also reproduced the module top-level-await path and
found no medium-or-higher correctness issue.

Runner and analyzer admit exactly six cross-Realm private method/getter/setter
brand-check files. At commit `c66fc1e`, tooling is **68/68**, classes are
**61/61**, the pinned class path is **1672 pass / 0 fail / 2387 skip / 4059
total**, and the pinned supported subset is **12238 pass / 0 fail / 8201 skip /
20439 total**. CI `29273287748` and full matrix `29273287842` succeeded. Of 30
downloaded result artifacts, only `language/expressions` changed, from
**7585/0/3517** to **7591/0/3511**. The normalized aggregate is therefore
**28542 pass / 6614 fail / 13149 skip / 12 timeout / 0 error / 48317 total /
35156 pass-or-fail executed**.

## Identifier delete References

Sloppy `delete identifier` now compiles as `LoadRef` followed by
`DeleteValue`. The resulting Reference distinguishes unresolvable names,
declarative environment bindings, global object bindings, and `with` object
environment properties. The standalone `DeleteVar` opcode and its second
environment-chain traversal are removed.

Global `var` and function deletion follows the selected Realm global object's
property descriptor, while global lexical bindings return `false` without
touching a configurable object property with the same name. Foreign Realm
dynamic properties and eval-created bindings are deleted from that Realm, not
the caller's global object. `with` Proxy false/throw outcomes propagate, and
the retained object-environment Reference is pinned while a delete trap forces
GC. Regressions also cover unresolvable names, strict parenthesized early
errors, local/global eval bindings, inherited and unscopables-hidden `with`
properties, parameters, and non-configurable script globals.

At commit `0c2f783`, operators are **121/121**, Rust all-targets, release
build, clippy with denied warnings, and fmt/diff pass. Pinned Test262 delete and
with paths are **250 pass / 0 fail / 0 skip / 250 total**, and the supported
subset remains **12238 pass / 0 fail / 8201 skip / 20439 total**. Independent
review found no correctness or rooting defect after the added boundary tests.
CI `29277397932` and full matrix `29277398192` succeeded; all 30 downloaded
artifacts are byte-for-byte identical to matrix `29274877594`, retaining the
normalized **28542 pass / 6614 fail / 13149 skip / 12 timeout / 0 error / 48317
total / 35156 pass-or-fail executed** aggregate.

## Private-call Reference routing

Ordinary and optional private calls now compile the private access into one
retained Reference, resolve its value before evaluating arguments, and invoke
it through `CallRef` or `CallRefSpread`. This gives direct, receiver-optional,
callee-optional, grouped, and spread forms the same receiver derivation and
callee snapshot rules. Private brand errors and accessor getters occur before
arguments, callable private accessors are accepted, and spread arguments no
longer leave the VM stack in the shape expected only by the removed
`CallPrivateMethod` opcode.

Regressions cover argument-side callee mutation, wrong-brand ordering,
accessor/getter ordering, nullish argument suppression, combined receiver and
callee optionality, grouped chain boundaries, non-callable and throwing
accessors, and ordinary/optional/grouped/spread calls whose temporary receiver
must survive forced argument GC. Independent review found no correctness
defect; its optional-chain and GC coverage gaps were added before the final
validation.

At commit `0dd6dfc`, Rust all-targets, release build, clippy with denied
warnings, and fmt/diff pass. Class Test262 is **1672 pass / 0 fail / 2387 skip /
4059 total**, and the pinned supported subset remains **12238 pass / 0 fail /
8201 skip / 20439 total**. CI `29281286232` and full matrix `29281286160`
succeeded. All 30 downloaded result artifacts are byte-for-byte identical to
matrix `29278945569`, retaining the normalized **28542 pass / 6614 fail / 13149
skip / 12 timeout / 0 error / 48317 total / 35156 pass-or-fail executed**
aggregate.

## Private read-modify-write Reference routing

Private prefix/postfix updates and compound and logical assignments now create
one private Reference, resolve its current value with `GetValue`, retain that
same Reference across numeric coercion or RHS evaluation, and store with
`PutValue`. The previous `GetPrivate` and `SetPrivate` compiler emissions,
bytecodes, and VM handlers are removed. Private base evaluation, name identity,
brand checks, accessors, assignment results, and rooting now use the same
Reference machinery as ordinary identifier, member, and `super` targets.

Local regressions cover all prefix/postfix increment/decrement combinations for
Number and BigInt; accessor getter, coercion, RHS, and setter ordering; coercion
and RHS mutation of the same field; throwing conversions; readonly accessors
and methods; mixed numeric and negative-BigInt-exponent errors; all logical
assignment and short-circuit branches; wrong brands before RHS; foreign-Realm
errors; and forced GC during update, compound, logical, getter, RHS, coercion,
and setter work. Independent review found no correctness defect, and each of
its prioritized test gaps was added before final validation.

At commit `17de0d9`, Rust all-targets, release build, clippy with denied
warnings, fmt/diff, and all **72** class regressions pass. Pinned compound and
logical assignment Test262 is **532/532**, class Test262 is **1672 pass / 0
fail / 2387 skip / 4059 total**, and the supported subset remains **12238 pass
/ 0 fail / 8201 skip / 20439 total**. CI `29285326599` and full matrix
`29285326527` succeeded. All 30 downloaded result artifacts are byte-for-byte
identical to matrix `29282907778`, retaining the normalized **28542 pass / 6614
fail / 13149 skip / 12 timeout / 0 error / 48317 total / 35156 pass-or-fail
executed** aggregate.

## Parenthesized optional-chain tag References

A parenthesized optional chain may be used as a tagged-template head even
though an unparenthesized optional-chain tag is an early error. When the chain
ends in an ordinary, computed, nested, or private member, the compiler now
retains its Reference and invokes the snapshotted tag through `CallRef`. When
the chain ends in a call result, it emits an explicit `undefined` receiver and
uses `CallThis`, preserving an unbound strict `this` without pretending the
result is a Reference.

Regressions cover member, computed, nested, private, and call-result tags;
nullish and non-callable errors after interpolation; getter errors before
interpolation; interpolation errors before invocation; interpolation-side
callee mutation; direct-versus-parenthesized parser boundaries; and forced GC
while computed, private, and call-result forms retain their inputs. Independent
review found no correctness defect after these boundary tests were added.

At commit `e9faed5`, Rust all-targets, release build, clippy with denied
warnings, fmt/diff, and tagged-template regressions pass. Pinned tagged-template
and optional-chaining Test262 is **63 pass / 0 fail / 2 skip / 65 total**, and
the supported subset remains **12238 pass / 0 fail / 8201 skip / 20439 total**.
CI `29289456680` and full matrix `29289456698` succeeded. All 30 downloaded
result artifacts are byte-for-byte identical to matrix `29286826876`, retaining
the normalized **28542 pass / 6614 fail / 13149 skip / 12 timeout / 0 error /
48317 total / 35156 pass-or-fail executed** aggregate.

## Super loop-target References

A direct member target in a `for-in` or `for-of` head is evaluated after each
iteration value is obtained. When that target is `super.name` or
`super[expression]`, the compiler now uses `MakeSuperPropertyRef` rather than
an ordinary raw property Reference. The retained record keeps the dynamic
super base separate from the method's actual `this`, so setters and Proxy
`set` traps observe an instance, constructor, or borrowed primitive receiver
instead of the home prototype.

Regressions cover direct and computed `for-in`/`for-of` targets, zero and
multiple iterations, key evaluation per iteration, key-side home-prototype
mutation before the super-base lookup, instance/static/primitive receivers,
setter failure with IteratorClose and close-error priority, and forced GC in
the key expression, `ToPropertyKey`, and Proxy trap. Independent review found
no correctness defect after these cases were added.

At commit `b395956`, Rust all-targets, release build, clippy with denied
warnings, and fmt/diff pass. Pinned class-super, for-in, and for-of Test262 is
**684 pass / 0 fail / 190 skip / 874 total**, and the supported subset remains
**12238 pass / 0 fail / 8201 skip / 20439 total**. CI `29292458497` and full
matrix `29292458431` succeeded. All 30 downloaded result artifacts are
byte-for-byte identical to matrix `29290696382`, retaining the normalized
**28542 pass / 6614 fail / 13149 skip / 12 timeout / 0 error / 48317 total /
35156 pass-or-fail executed** aggregate.

## RegExp literal Realm intrinsics

A RegExp literal is not equivalent to evaluating `new RegExp(pattern, flags)`:
it must construct through the current Realm's intrinsic while ignoring mutable
lexical and global `RegExp` bindings. The compiler now emits a dedicated
`NewRegExpLiteral` operation with pattern and flags constants. Each initialized
Realm retains its original `%RegExp.prototype%` in a traced VM table, and the
operation selects the executing interpreted frame's Realm. This distinction is
required when a main-Realm native builtin re-enters a foreign callback,
generator, async function, or async generator before the native call returns.
The obsolete, otherwise-unreferenced `LoadGlobal` operation is removed.

Regressions cover lexical, parameter, and global shadowing; fresh literal
identity; main and foreign prototype selection; foreign eval; GC retention;
and native re-entry through `Array.prototype.map`, generator `next`, async
callbacks, and async-generator `next`. Malformed bytecode constant indices or
types now produce an internal error instead of silently creating an empty
RegExp. Independent review reproduced the native re-entry defect, verified the
fix, and found no remaining literal-path defect. Existing `RegExp()` call
semantics and backend support for valid lookaround patterns remain separate
follow-up units rather than being hidden inside this literal change.

At commit `953a821`, Rust all-targets, release build, clippy with denied
warnings, fmt/diff, and **68/68** tooling tests pass. Pinned literal Test262 is
**474 pass / 0 fail / 60 skip / 534 total**; the full `built-ins/RegExp` run is
unchanged at **865 pass / 144 fail / 864 skip / 6 timeout / 1879 total**; and
the supported subset remains **12238 pass / 0 fail / 8201 skip / 20439 total**.
CI `29295535589` and full matrix `29295535579` succeeded. All 30 downloaded
result artifacts are byte-for-byte identical to matrix `29293614900`, retaining
the normalized **28542 pass / 6614 fail / 13149 skip / 12 timeout / 0 error /
48317 total / 35156 pass-or-fail executed** aggregate.

## Environment Reference completion

Environment Reference records now remain tied to the exact declarative record
selected during identifier resolution. `PutValue` no longer repeats name
resolution through parent environments after RHS evaluation; a deleted sloppy
binding is recreated on that exact record, while strict, TDZ, const, import,
and function-name behavior remains distinct. Environment GC tracing also keeps
an active `with` binding object alive before a Reference is created.

Global `var` bindings route reads and writes through the correct Realm global
object so accessors, non-writable descriptors, throwing setters, and foreign
Realms remain observable. Successful data-property writes synchronize the
declarative mirror only after `[[Set]]`, and deleting a configurable global
property removes its stale `var` mirror. Compiler-internal `StoreEnvName`
writes first resolve an identifier Reference instead of treating the current
frame environment as an already-resolved base.

Regressions cover forced GC in `with`, simple/compound/logical assignment after
direct-eval binding deletion, foreign global writes and readonly properties,
throwing and successful global setters, direct and setter-side global property
deletion, and sloppy block-function updates. A first full matrix exposed four
Annex B regressions and one offsetting improvement in the internal store path;
the follow-up resolution fix restores the prior **206 pass / 830 fail / 50
skip** Annex B result.

At commit `db0e5a9`, Rust all-targets, release build, clippy with denied
warnings, fmt/diff, and **68/68** tooling tests pass. Focused Reference Test262
is **1426 pass / 0 fail / 27 skip / 1453 total**; 19 skipped
`Symbol.iterator`/generator-gated files pass when run directly, leaving eight
tail-call tests outside the current engine scope. The pinned supported subset
remains **12238 pass / 0 fail / 8201 skip / 20439 total**. CI `29301189893` and
full matrix `29301189900` succeeded. All 30 downloaded result artifacts are
byte-for-byte identical to matrix `29296682790`, retaining the normalized
**28542 pass / 6614 fail / 13149 skip / 12 timeout / 0 error / 48317 total /
35156 pass-or-fail executed** aggregate.

## Computed public class-field admission

`tools/test262_class_computed_field_admission.txt` freezes exactly **120**
generated computed-name tests for public instance and static fields. The set is
the Cartesian family boundary of class declaration/expression forms and
field-only/field-plus-method forms, with 30 common expression suffixes in each
family. Runner and analyzer use the same exact-membership predicate and remove
only `class-fields-public` and `class-static-fields-public` for those paths;
the global feature gates remain unchanged.

Four corresponding `from-await-expression` files are intentionally excluded.
They carry async module and top-level-await requirements and remain skipped
until that broader host/runtime boundary is admitted. Tooling checks the 4 x 30
family shape, rejects an await sibling and an unrelated class file, and prevents
future upstream files from entering this unit implicitly. Two independent
audits reproduced **120/120** on both available Test262 revisions and found no
blocking issue.

At commit `31013cc`, Rust all-targets, release build, clippy with denied
warnings, fmt/diff, Python compilation, and **69/69** tooling tests pass. The
120-file manifest is **120 pass / 0 fail / 0 skip**; the current broad class
diagnostic is **3790 pass / 0 fail / 4636 skip / 8426 total**. On pinned
Test262 `d1d583db95a521218f3eb8341a887fd63eda8ff1`, the supported subset is
**12358 pass / 0 fail / 8081 skip / 20439 total**. CI `29303864482` and full
matrix `29303864484` succeeded. Only the expressions and statements artifacts
changed, each by **+60 pass / -60 skip**; the other 28 result artifacts exactly
match matrix `29302178860`. The normalized aggregate is **28662 pass / 6614
fail / 13029 skip / 12 timeout / 0 error / 48317 total / 35276 pass-or-fail
executed**.

## Residual public class-field admission

`tools/test262_class_public_field_admission.txt` freezes the remaining five
files for which `class-fields-public` or `class-static-fields-public` was the
only unsupported blocker. They cover three connected ClassDefinitionEvaluation
boundaries: binding derived-constructor `this` before instance fields run,
propagating abrupt completion from computed instance/static field names, and
interleaving static fields with static blocks while halting after an abrupt
completion.

Runner and analyzer share exact manifest membership and remove only the public
instance/static field feature gates on those paths. Both broad gates remain in
`SKIP_FEATURES`, preventing future upstream class-field files from being
admitted implicitly. A full before/after policy audit reports **0** remaining
files that would become runnable from removing only those broad gates. An
independent audit reproduced the exact five-file delta and **5/5** execution on
both available Test262 revisions.

At commit `9c5a2c2`, Rust all-targets, release build, clippy with denied
warnings, fmt/diff, Python compilation, and **70/70** tooling tests pass. The
five-file manifest is **5 pass / 0 fail / 0 skip**, the current broad class
diagnostic is **3795 pass / 0 fail / 4631 skip / 8426 total**, and pinned
Test262 `d1d583db95a521218f3eb8341a887fd63eda8ff1` reports **12363 pass / 0
fail / 8076 skip / 20439 total** for the supported subset. CI `29307597382`
and full matrix `29307597365` succeeded. Only expressions (**+1 pass / -1
skip**) and statements (**+4 pass / -4 skip**) changed; the other 28 artifacts
exactly match matrix `29305184262`. The normalized aggregate is **28667 pass /
6614 fail / 13024 skip / 12 timeout / 0 error / 48317 total / 35281
pass-or-fail executed**.

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
Latest improvement confirmation: `test262-full` 29058987345 on `5ba07b5`;
the aggregate reports **19897 pass / 6831 fail / 11 timeout / 0 error /
20979 skip / 47718 total / 26728 ran**, or **74.4%** of executed files and
**41.7%** of all matrix files.
Latest improvement confirmation: `test262-full` 29060028700 on `dfe9fc5`;
the aggregate reports **19899 pass / 6831 fail / 11 timeout / 0 error /
20977 skip / 47718 total / 26730 ran**, retaining **74.4%** of executed files
and **41.7%** of all matrix files after the two-file admission.
Latest improvement confirmation: `test262-full` 29061367736 on `c8cfa71`;
the aggregate reports **21547 pass / 6831 fail / 11 timeout / 0 error /
19329 skip / 47718 total / 28378 ran**, or **75.9%** of executed files and
**45.2%** of the matrix after broad private class-element admission.
Latest improvement confirmation: `test262-full` 29062576300 on `528d584`;
the aggregate reports **21551 pass / 6831 fail / 11 timeout / 0 error /
19325 skip / 47718 total / 28382 ran**, retaining **75.9%** of executed files
and **45.2%** of the matrix after Proxy class-element admission.
Latest improvement confirmation: `test262-full` 29063692814 on `e92bbea`;
the aggregate reports **21566 pass / 6830 fail / 11 timeout / 0 error /
19311 skip / 47718 total / 28396 ran**, retaining **75.9%** of executed files
and **45.2%** of the matrix after complete subclass admission and constructor
classification fixes.
Latest improvement confirmation: `test262-full` 29064651130 on `0b4a9e4`;
the aggregate reports **21600 pass / 6830 fail / 11 timeout / 0 error /
20026 skip / 48467 total / 28430 ran**, or **76.0%** of executed files and
**44.6%** of the matrix after built-in subclass admission and an upstream
test262 expansion of 749 unsupported files. Documentation confirmation run
`test262-full` 29064961455 on `f0f8fdd` produced the same aggregate.
Latest improvement confirmation: `test262-full` 29065804433 on `0b0a398`;
the aggregate reports **21625 pass / 6830 fail / 11 timeout / 0 error /
20001 skip / 48467 total / 28455 ran**, retaining **76.0%** of executed files
and **44.6%** of the current matrix after class-definition generator grammar
admission.
Latest improvement confirmation: `test262-full` 29066783354 on `37c7f5d`;
the aggregate reports **21778 pass / 6830 fail / 11 timeout / 0 error /
19848 skip / 48467 total / 28608 ran**, or **76.1%** of executed files and
**44.9%** of the matrix after generator-function intrinsic and binding
admission.
Latest improvement confirmation: `test262-full` 29067742201 on `4d28c86`;
the aggregate reports **22179 pass / 6830 fail / 11 timeout / 0 error /
19447 skip / 48467 total / 29009 ran**, or **76.5%** of executed files and
**45.8%** of the matrix after complete statement/expression generator-path
admission.
Latest improvement confirmation: `test262-full` 29068673850 on `2c1a04c`;
the aggregate reports **22587 pass / 6830 fail / 11 timeout / 0 error /
19039 skip / 48467 total / 29417 ran**, or **76.8%** of executed files and
**46.6%** of the matrix after complete ordinary function-form admission.
Parser-context confirmation run `test262-full` 29068958829 on `d84b9b5`
produced the same aggregate after the formal-parameter Yield/Await grammar
fixes.
Latest improvement confirmation: `test262-full` 29069739444 on `fd50d2b`;
the aggregate reports **22786 pass / 6830 fail / 11 timeout / 0 error /
18840 skip / 48467 total / 29616 ran**, or **76.9%** of executed files and
**47.0%** of the matrix after complete ordinary arrow-function admission and
admission-rule deduplication.
Escaped async-method parser confirmation: `test262-full` 29070658804 on
`9513056` produced the same aggregate. Latest improvement confirmation:
`test262-full` 29070781992 on `a4f5565`; the aggregate reports **22936 pass /
6830 fail / 11 timeout / 0 error / 18690 skip / 48467 total / 29766 ran**, or
**77.1%** of executed files and **47.3%** of the matrix after synchronous
object method-definition admission.
Latest improvement confirmation: `test262-full` 29072860541 on `643fd42`;
the aggregate reports **22999 pass / 6830 fail / 11 timeout / 0 error /
18627 skip / 48467 total / 29829 ran**, or **77.1%** of executed files and
**47.5%** of the matrix after complete synchronous yield-expression admission
and primitive String protocol-dispatch guards.
Documentation confirmation: `test262-full` 29073241655 on `5652b3b` retains
**22999 pass / 6830 fail / 11 timeout / 0 error / 29829 ran**. The current
upstream snapshot no longer contains the 749 unsupported files added in the
earlier snapshot, so the current aggregate is **17878 skip / 47718 total** and
the all-matrix rate is **48.2%**; this denominator-only change is not an engine
conformance gain.
Async generator completion confirmation: `test262-full` 29078884750 on
`3dfdaff` retains that aggregate after awaiting ordinary yielded values and
preserving native async rejection Error objects. Latest improvement
confirmation: `test262-full` 29079329840 on `45c2888`; the aggregate reports
**23100 pass / 6830 fail / 11 timeout / 0 error / 17777 skip / 47718 total /
29930 ran**, or **77.2%** of executed files and **48.4%** of the matrix after
the complete async object method-definition admission.
Latest improvement confirmation: `test262-full` 29080354981 on `05f3659`;
the aggregate reports **23142 pass / 6830 fail / 11 timeout / 0 error / 17735
skip / 47718 total / 29972 ran**, retaining **77.2%** of executed files and
raising the matrix rate to **48.5%** after complete async arrow-function
admission.
Latest improvement confirmation: `test262-full` 29081813828 on `3ebd528`;
the aggregate reports **23277 pass / 6830 fail / 11 timeout / 0 error / 17600
skip / 47718 total / 30107 ran**, or **77.3%** of executed files and **48.8%**
of the matrix after complete async function expression/declaration admission.
Async iterator-kind confirmation: `test262-full` 29094206133 on `a906242`
retains **23277 pass / 6830 fail / 11 timeout / 0 error / 30107 ran**. The
current upstream snapshot restores 749 unsupported, skip-only files, producing
**18349 skip / 48467 total** and a denominator-only all-matrix rate of
**48.0%**; the executed-file rate remains **77.3%**.
Async-generator receiver-brand confirmation: `test262-full` 29095756104 on
`43cc099` reports **23277 pass / 6830 fail / 11 timeout / 0 error / 18349 skip
/ 48467 total / 30107 ran**, retaining **77.3%** of executed files and
**48.0%** of the matrix. The six newly passing receiver checks are part of the
opt-in async diagnostic rather than the default full matrix.
The iterator-result descriptor change is confirmed by `test262-full`
29096575687 on `42ac4c4` with no engine-outcome regressions.
The complete async-generator language diagnostic against test262
`d1d583db95a521218f3eb8341a887fd63eda8ff1` reports **924 pass / 0 fail / 0
skip** across the statement and expression paths. Runner/analyzer admission is
limited to those exact paths; unrelated async-iteration tests remain gated.
The supported subset rises to **10761 pass / 0 fail / 9678 skip / 20439
total**.
Async-function return assimilation also keeps the async-enabled class-elements
diagnostic green at **2695 pass / 0 fail / 267 skip / 2962 total**, including
private instance/static async methods that return async closures or generic
thenables. Async execution is therefore admitted on the two exact
class-elements paths, raising the supported subset to **11249 pass / 0 fail /
9190 skip / 20439 total** while unrelated async paths remain gated.
The two async `super` method checks under
`language/statements/class/definition/` are also fully green and admitted by
default. The focused path now reports **65 pass / 0 fail / 0 skip**, raising
the supported subset to **11251 pass / 0 fail / 9188 skip / 20439 total**.
The admission is confirmed by CI `29102497188` and `test262-full`
`29102497174`.
Ordinary async functions now snapshot their execution frame at pending Await
boundaries and resume through Promise reaction jobs, including rejection into
active `catch`/`finally` state and GC-safe lexical environments. `for await`
iterator-result waits use the same bytecode continuation instead of draining
the microtask queue synchronously. The focused `language/expressions/await/`
path is fully admitted at **22 pass / 0 fail / 0 skip**, and the supported
subset reaches **11266 pass / 0 fail / 9173 skip / 20439 total**.
CI `29103907303` and `test262-full` `29103907305` confirm the implementation.
The full aggregate is **24705 pass / 6830 fail / 11 timeout / 0 error / 16921
skip / 48467 total / 31546 ran**, or **78.3%** of executed files and **51.0%**
of the matrix. Relative to the preceding confirmation, pass increases by the
15 newly admitted Await files while fail/error counts remain unchanged; the
two additional built-ins timeouts are run-to-run execution variance.
Async-from-sync iteration now applies Promise resolution before constructing
each iterator-result object, preserving Promise `constructor` lookup errors and
the specified reaction-job ordering. The `for await (async of iterable)`
grammar case is also accepted. The exact async-iteration slice under
`language/statements/for-await-of/` is admitted at **23 pass / 0 fail / 1211
skip / 1234 total**, raising the supported subset to **11289 pass / 0 fail /
9150 skip / 20439 total**. CI `29105422326` and `test262-full` `29105422278`
confirm the change. The full aggregate is **24728 pass / 6830 fail / 11
timeout / 0 error / 16898 skip / 48467 total / 31569 ran**, or **78.3%** of
executed files and **51.0%** of the matrix.
The class-elements paths now admit the `Symbol`, `Symbol.iterator`, and
`Symbol.asyncIterator` behavior exercised by computed names and private async
generator methods. The focused paths report **2951 pass / 0 fail / 11 skip /
2962 total**, raising the supported subset to **11545 pass / 0 fail / 8894
skip / 20439 total**. These feature exceptions remain scoped to
`language/{expressions,statements}/class/elements/`. CI `29106363624` and
`test262-full` `29106363581` confirm the admission. The full aggregate is
**24984 pass / 6830 fail / 11 timeout / 0 error / 16642 skip / 48467 total /
31825 ran**, or **78.5%** of executed files and **51.5%** of the matrix.
Optional-chain compilation now shares a single short-circuit target across
member, call, and private-field tails. Grouped member calls and super optional
calls preserve their Reference receiver, while optional-chain tagged templates
and private deletes are rejected during parsing. Optional delete short-circuits
to `true` or deletes the final property on a live chain. The full
`language/expressions/optional-chaining/` path reports **38 pass / 0 fail / 0
skip**, and class-elements reaches **2957 pass / 0 fail / 5 skip / 2962
total** after admitting its optional-chaining and destructuring cases. The
supported subset rises to **11589 pass / 0 fail / 8850 skip / 20439 total**.
CI `29108590113` and `test262-full` `29108590134` confirm the change. The full
aggregate is **25028 pass / 6830 fail / 11 timeout / 0 error / 16598 skip /
48467 total / 31869 ran**, or **78.5%** of executed files and **51.6%** of the
matrix.

The standard `dynamic-import/usage` subtree is complete at **108/108**. The 102
newly admitted generated contexts cover named and default live-binding updates,
script-origin host resolution to Module Records, computed `['then']` call
grammar, thenable use, and observable specifier coercion across arrows, async
functions, async generators, blocks, labels, branches, and loops. Exact
dynamic-import coverage is **282 pass / 0 fail / 723 skip / 1005 total**; the
supported subset is **11893 pass / 0 fail / 8546 skip / 20439 total**. CI
`29203471348` and `test262-full` `29203471334` pass for commit `1299636`.
Downloaded artifacts change only expressions by **+102 pass / -102 skip**,
producing **28028 pass / 6720 fail / 12 timeout / 0 error / 13557 skip / 48317
total / 34748 pass-or-fail executed**, or **80.7%** of executed files and
**58.0%** of the matrix.

The standard `dynamic-import/namespace` subtree is complete at **67/67**. The
65 newly admitted await and Promise-reaction cases cover string and Symbol
gets, `[[HasProperty]]`, descriptors, sorted own keys, strict/non-strict set and
delete, non-extensibility, null prototype, nested namespaces, and
`Symbol.toStringTag`. Exact dynamic-import paths now also lift supported
`Symbol`, `Symbol.iterator`, `Symbol.toStringTag`, and `Reflect` metadata in the
runner and analyzer, with a regression proving the descriptor case executes.
Exact dynamic-import coverage is **347 pass / 0 fail / 658 skip / 1005 total**;
the supported subset is **11958 pass / 0 fail / 8481 skip / 20439 total**. CI
`29205144135` and `test262-full` `29205144132` pass for commit `65f2b45`.
Downloaded artifacts change only expressions by **+65 pass / -65 skip**,
producing **28093 pass / 6720 fail / 12 timeout / 0 error / 13492 skip / 48317
total / 34813 pass-or-fail executed**, or **80.7%** of executed files and
**58.1%** of the matrix.

Standard dynamic-import syntax coverage is complete at **251/251**: 143 parse
negatives and 108 valid forms, including the current `import-attributes`
trailing-comma grammar. Direct `new import(...)` and property-access variants
now raise early SyntaxError in every generated context, while parenthesized
`new (import(...))` remains valid and fails only at runtime constructability as
required. Exact paths lift `import-attributes` metadata in both runner and
analyzer; source-phase and deferred-import proposal tests remain excluded.
Exact dynamic-import coverage is **598 pass / 0 fail / 407 skip / 1005 total**;
the supported subset is **12209 pass / 0 fail / 8230 skip / 20439 total**. CI
`29207058820` and `test262-full` `29207058806` pass for commit `954de2f`.
Downloaded artifacts change only expressions by **+251 pass / -251 skip**,
producing **28344 pass / 6720 fail / 12 timeout / 0 error / 13241 skip / 48317
total / 35064 pass-or-fail executed**, or **80.8%** of executed files and
**58.7%** of the matrix.

Dynamic import attributes now complete the remaining 23 standard files. The
options expression permits `in`, specifier coercion precedes observable
options access, and enumerable `with` keys follow Proxy descriptor and
`ownKeys` invariants. The relative-file host accepts only string-valued
`type: "json"` and `type: "text"` attributes, rejects unsupported keys/types,
parses JSON as data rather than executable source, and keeps typed module
cache identities distinct from real filesystem paths. Exact dynamic-import
coverage is **621 pass / 0 fail / 384 skip / 1005 total**; the remaining 384
files are the `import.source` and `import.defer` proposals. The supported
subset is **12232 pass / 0 fail / 8207 skip / 20439 total**.
Feature commit `22c49ec` is confirmed by CI `29209531923` and
`test262-full` `29209531948`. Downloaded artifacts aggregate to **28398 pass /
6689 fail / 13218 skip / 12 timeout / 0 error / 48317 total / 35087
pass-or-fail executed**, or **58.8%** of all files and **80.9%** of executed
files. Against the preceding matrix this is **+54 pass / -31 fail / -23
skip**: the 23 exact admissions move skip to pass, while strict JSON parsing
and Proxy `ownKeys` invariants convert 31 existing failures to passes.
Focused WeakRef coverage check:
`WeakRef` now uses a dedicated weak heap object whose object target is omitted
from normal marking and cleared during sweep. Construction and `deref()` add
live object targets to the current job's kept roots; registered Symbols and
non-weakly-holdable primitives are rejected while unregistered and well-known
Symbols are accepted. Realm-specific prototype fallback and the standard
constructor, prototype, method, and `@@toStringTag` descriptors are installed
in both the main and test262-created Realms. The exact
`built-ins/WeakRef/` path initially reported **28 pass / 0 fail / 1 skip / 29
total**. CI `29110157712` and `test262-full` `29110157754` confirmed that
admission, with downloaded full artifacts at **25056 pass / 6830 fail / 11
timeout / 0 error / 16570 skip / 48467 total / 31897 executed**.

Focused FinalizationRegistry coverage check:
`FinalizationRegistry` now uses registration cells with weak target and
unregister-token slots plus strongly traced held values. GC sweep clears dead
targets, schedules one cleanup job per registry, and invokes the captured
callback at a VM job checkpoint; `unregister()` removes all cells sharing the
same object or non-registered Symbol token. Constructor and method branding,
target/held-value validation, cross-Realm prototype fallback, descriptors, and
`@@toStringTag` are installed with the standard shape. Removing the global
feature gate and admitting its exact Reflect/Symbol metadata closes
`built-ins/FinalizationRegistry/` at **47 pass / 0 fail / 0 skip** and the
previous WeakRef brand skip at **29 pass / 0 fail / 0 skip**. The related
`built-ins/Object/seal/seal-finalizationregistry.js` file also passes, while
the supported language subset remains **11589 pass / 0 fail / 8850 skip /
20439 total**. CI `29111665821` and `test262-full` `29111666010` confirm the
change. Downloaded artifacts aggregate to **25105 pass / 6830 fail / 11
timeout / 0 error / 16521 skip / 48467 total / 31946 executed**, or **78.6%**
of executed files and **51.8%** of the matrix.

Focused SharedArrayBuffer coverage check:
Fixed-length `SharedArrayBuffer` now has its own constructor brand, Realm-aware
prototype fallback, `byteLength`, `@@species`, `@@toStringTag`, and a
species-aware `slice()` that copies shared bytes. TypedArray and DataView views
operate on the same backing bytes without detachment, while ordinary
ArrayBuffer-only operations reject the shared brand. The exact
`built-ins/SharedArrayBuffer/` path reports **60 pass / 0 fail / 44 skip / 104
total**; all 44 skipped files require growable/resizable shared buffers and
remain gated together with Atomics. The supported language subset remains
**11589 pass / 0 fail / 8850 skip / 20439 total**. CI `29113667245` and
`test262-full` `29113667267` confirm the change. Downloaded artifacts aggregate
to **25165 pass / 6830 fail / 11 timeout / 0 error / 16461 skip / 48467 total /
32006 executed**, or **78.6%** of executed files and **51.9%** of the matrix.

Focused Atomics coverage check:
`Atomics` now installs the ten synchronous Number/BigInt integer-TypedArray
operations with standard names, lengths, descriptors, prototype, and
`@@toStringTag`. Shared and ordinary mutable ArrayBuffers use one backing-store
mutex across each complete operation; immutable buffers permit `load` but
reject writes. Exact path admission closes these operations and the Atomics
object surface initially closed at **154 pass / 0 fail / 235 skip / 389
total**. CI `29115320336` and `test262-full` `29115320329` confirmed that
slice.
Downloaded artifacts aggregate to **25319 pass / 6769 fail / 11 timeout / 0
error / 16368 skip / 48467 total / 32099 executed**, or **78.9%** of executed
files and **52.2%** of the matrix.

SharedArrayBuffer backing stores now carry Arc-shared bytes and FIFO waiter
lists across independent test262 worker VMs. The host supports agent start,
broadcast, receive, report, sleep, and monotonic time, while runner metadata
selects the main agent's `CanBlock` mode. Condvar-backed `Atomics.wait` and
`notify` cover Number/BigInt waitable views, timeout and count conversion,
non-shared notification, and ordered wakeups. Exact admission raises the
focused path to **279 pass / 0 fail / 110 skip / 389 total**; the remaining 110
files are 101 `waitAsync`, 5 `pause`, and 4 resizable-buffer cases. The
supported language subset remains **11589 pass / 0 fail / 8850 skip / 20439
total**. CI `29117508967` and `test262-full` `29117508706` confirm the change.
Downloaded artifacts aggregate to **25444 pass / 6769 fail / 11 timeout / 0
error / 16243 skip / 48467 total / 32224 executed**, or **79.0%** of executed
files and **52.5%** of the matrix.

`Atomics.waitAsync` now returns the synchronous `not-equal` and zero-timeout
records directly, while pending waits retain their Promise resolver as a GC
root and settle through an external VM job after FIFO notification or timeout.
Test262 worker VMs drain those jobs before exit. `Atomics.pause` is exposed as
the implementation-defined no-op hint permitted by ECMAScript. The complete
fixed-length Atomics path reports **384 pass / 0 fail / 5 skip / 389 total**;
all five skipped files require growable or resizable buffers. The supported
language subset remains **11589 pass / 0 fail / 8850 skip / 20439 total**. CI
`29119574209` and `test262-full` `29119574146` confirm the change. Downloaded
artifacts aggregate to **25549 pass / 6769 fail / 11 timeout / 0 error / 16138
skip / 48467 total / 32329 executed**, or **79.0%** of executed files and
**52.7%** of the matrix.

Growable SharedArrayBuffer core coverage check:
`SharedArrayBuffer` now records the optional `maxByteLength` internal slot,
reports it through standard `growable` and `maxByteLength` accessors, and
extends the Arc-shared backing store monotonically through `grow()` while
preserving bytes and zero-initializing new storage. Constructor validation
observes the required option-coercion and `new.target.prototype` ordering, and
agent broadcasts preserve the growth limit. The exact
`built-ins/SharedArrayBuffer/` path now reports **104 pass / 0 fail / 0 skip /
104 total**. Resizable ArrayBuffer and dynamic length-tracking views remain a
separate gated unit. CI `29135330020` and `test262-full` `29135330077` confirm
the change. Downloaded artifacts aggregate to **25593 pass / 6769 fail / 11
timeout / 0 error / 16094 skip / 48467 total / 32373 executed**, or **79.1%**
of executed files and **52.8%** of the matrix.

Resizable ArrayBuffer core coverage check:
`ArrayBuffer` now records `maxByteLength`, exposes `resizable` and
`maxByteLength`, and resizes attached mutable backing stores in either
direction. Length coercion precedes detach revalidation, growth zero-fills new
bytes, and transfer distinguishes resizable-preserving and fixed-length modes.
Classifying ArrayBuffer as an internally allocating native constructor also
restores the required validation-before-prototype lookup order. The focused
ArrayBuffer path reports **194 pass / 0 fail / 27 skip / 221 total**, and the
complete Atomics path reports **389 pass / 0 fail / 0 skip / 389 total** after
admitting its five resize/grow coercion-order cases. Dynamic length-tracking
views remain the next gated unit. CI `29136048993` and `test262-full`
`29136049024` confirm the change. Downloaded artifacts aggregate to **25670
pass / 6769 fail / 11 timeout / 0 error / 16017 skip / 48467 total / 32450
executed**, or **79.1%** of executed files and **53.0%** of the matrix.

Length-tracking view coverage check:
TypedArray and DataView instances now retain a fixed-vs-tracking slot and
derive their effective byte length from the current resizable/growable backing
store. Integer-index exotics, getters, DataView operations, and Atomics consume
the same dynamic record, including out-of-bounds transitions and recovery.
TypedArray constructor coverage rises to **682 pass / 0 fail / 56 skip / 738
total**, DataView rises to **522 pass / 0 fail / 39 skip / 561 total**, and the
narrow resizable TypedArray exotic/getter slice adds **26 pass / 0 fail**.
Broader unimplemented TypedArray prototype methods remain gated separately. CI
`29136994077` and `test262-full` `29136994074` confirm the change. Downloaded
artifacts aggregate to **25734 pass / 6769 fail / 11 timeout / 0 error / 15953
skip / 48467 total / 32514 executed**, or **79.1%** of executed files and
**53.1%** of the matrix.

Focused TypedArray `at` coverage check:
`%TypedArray%.prototype.at` snapshots the validated view length before index
coercion, supports negative relative indices, and performs its final indexed
read against current resizable-buffer bounds. The exact path reports **15 pass
/ 0 fail / 0 skip / 15 total**. CI `29137525369` and `test262-full`
`29137525322` confirm the change. Downloaded artifacts aggregate to **25749
pass / 6766 fail / 11 timeout / 0 error / 15941 skip / 48467 total / 32526
executed**, or **79.2%** of executed files and **53.1%** of the matrix.

Focused TypedArray `fill` coverage check:
`%TypedArray%.prototype.fill` preserves its initial length snapshot across
value/start/end coercion, revalidates resized bounds before writing, and works
for Number, BigInt, immutable rejection, and resizable views. TypedArrays now
also expose lazy `values()`/default iteration. The exact fill path reports **52
pass / 0 fail / 0 skip / 52 total**. CI `29138124604` and `test262-full`
`29138124559` confirm the change. Downloaded artifacts aggregate to **25801
pass / 6766 fail / 0 timeout / 0 error / 15140 skip / 47718 total / 32567
executed**, or **79.2%** of executed files and **54.1%** of the matrix.

Focused TypedArray `subarray` coverage check:
`%TypedArray%.prototype.subarray` snapshots the initial effective length while
retaining the raw internal byte offset, performs begin/end coercion before
species construction even for detached or out-of-bounds sources, and creates
a length-tracking result when the source is length-tracking and `end` is
omitted. The exact path reports **67 pass / 0 fail / 0 skip / 67 total**. CI
`29138611001` and `test262-full` `29138610981` confirm the change. Downloaded
artifacts aggregate to **25868 pass / 6766 fail / 0 timeout / 0 error / 15073
skip / 47718 total / 32634 executed**, or **79.3%** of executed files and
**54.2%** of the matrix.

Focused TypedArray `set` coverage check:
`%TypedArray%.prototype.set` distinguishes TypedArray and array-like sources,
validates immutable and dynamic bounds in specification order, snapshots
overlapping source bytes, preserves same-type bit encodings, converts across
Number or BigInt element kinds, and continues ordered source access while a
resizable target changes. The exact path reports **110 pass / 0 fail / 0 skip
/ 110 total**. CI `29139260377` and `test262-full` `29139260415` confirm the
change. Downloaded artifacts aggregate to **25978 pass / 6766 fail / 0 timeout
/ 0 error / 14963 skip / 47718 total / 32744 executed**, or **79.3%** of
executed files and **54.4%** of the matrix.

Focused TypedArray `join` coverage check:
`%TypedArray%.prototype.join` validates and snapshots the receiver length
before separator coercion, then observes current element bounds while keeping
that iteration count across detach, shrink, and growth. The exact path reports
**32 pass / 0 fail / 0 skip / 32 total**. CI `29139734054` and `test262-full`
`29139734042` confirm the change. Downloaded artifacts aggregate to **26010
pass / 6766 fail / 0 timeout / 0 error / 14931 skip / 47718 total / 32776
executed**, or **79.4%** of executed files and **54.5%** of the matrix.

Focused TypedArray `values` and default-iterator coverage check:
`%TypedArray%.prototype.values` validates the receiver before iterator
creation, rechecks dynamic bounds on every pull, observes length-tracking
growth and shrink, throws when a fixed view becomes out of bounds, and stays
done after exhaustion. The exact values path reports **21 pass / 0 fail / 0
skip / 21 total** and the matching `Symbol.iterator` path reports **1 pass / 0
fail / 0 skip / 1 total** locally. CI `29140858679` and `test262-full`
`29140858676` confirm the change. Downloaded artifacts aggregate to **26031
pass / 6766 fail / 12 timeout / 0 error / 14909 skip / 47718 total / 32797
pass-or-fail executed**, or **79.4%** of pass-or-fail files and **54.6%** of
the matrix. The full built-ins shard had one additional timeout versus the
previous run, so that variance is not counted as a conformance gain.

Focused TypedArray `keys` and `entries` coverage check:
Both methods validate their receiver before creating a shared Array Iterator,
then recheck current dynamic bounds on every pull. `keys` yields numeric
indexes and `entries` yields fresh `[index, value]` arrays across fixed and
length-tracking resize transitions. The two exact paths report **38 pass / 0
fail / 0 skip / 38 total**. CI `29141404792` and `test262-full` `29141404775`
confirm the change. Downloaded artifacts aggregate to **26067 pass / 6768
fail / 12 timeout / 0 error / 15620 skip / 48467 total / 32835 pass-or-fail
executed**, or **79.4%** of pass-or-fail files and **53.8%** of the matrix. The
matrix contains 749 more files than the preceding run, including two additional
failures, so the aggregate delta is recorded as an upstream-suite snapshot and
is not attributed solely to these 38 focused tests.

Focused TypedArray `reverse` coverage check:
`%TypedArray%.prototype.reverse` validates a writable receiver and snapshots
its internal length without consulting a `length` property. It swaps Number and
BigInt elements through integer-indexed access, returns the original receiver,
preserves unrelated properties, and handles fixed and length-tracking resizable
views. The exact path reports **22 pass / 0 fail / 0 skip / 22 total**. CI
`29141851460` and `test262-full` `29141851451` confirm the change. Downloaded
artifacts aggregate to **26089 pass / 6768 fail / 12 timeout / 0 error / 15598
skip / 48467 total / 32857 pass-or-fail executed**, or **79.4%** of
pass-or-fail files and **53.8%** of the matrix. Against the preceding run's
identical matrix, 22 files moved from skip to pass with no fail, timeout, or
error change.

Focused TypedArray `toReversed` coverage check:
`%TypedArray%.prototype.toReversed` validates the source and snapshots its
internal length without reading a `length` property. It creates a distinct
same-kind TypedArray while ignoring the source's `constructor` and `@@species`,
then copies Number or BigInt elements in reverse order without changing the
source. The exact path reports **9 pass / 0 fail / 0 skip / 9 total**. CI
`29142341248` and `test262-full` `29142341265` confirm the change. Downloaded
artifacts aggregate to **26100 pass / 6766 fail / 12 timeout / 0 error / 15589
skip / 48467 total / 32866 pass-or-fail executed**, or **79.4%** of
pass-or-fail files and **53.9%** of the matrix. Against the preceding run's
identical matrix, the nine focused files moved from skip to pass. Two unrelated
built-ins failures also passed in this shard run, but no `toReversed` references
exist outside the focused path, so that variance is not attributed to this fix.

Focused TypedArray `copyWithin` coverage check:
`%TypedArray%.prototype.copyWithin` validates mutability before argument
coercion, computes indexes from the initial internal length, then revalidates
current bounds after coercion-driven resize or detach. The final overlap-safe
raw byte move respects byte offsets, truncates length-tracking views to their
current bounds, and preserves NaN payloads and other element bit patterns. The
exact path reports **65 pass / 0 fail / 0 skip / 65 total**. Its three 10,000
element detach stress files use a path-limited 600-second timeout after a
102-second local measurement and slower CI execution; ordinary files retain the
shared 8-second limit. CI `29143846038` and `test262-full` `29143846110`
confirm the change. Downloaded artifacts aggregate to **26164 pass / 6767
fail / 12 timeout / 0 error / 15524 skip / 48467 total / 32931 pass-or-fail
executed**, or **79.5%** of pass-or-fail files and **54.0%** of the matrix. The
CI artifact binary separately reproduces **65 pass / 0 fail / 0 timeout** on
the focused path. One unrelated built-ins file varied from pass to fail against
the preceding run, so the aggregate net gain is 64 while the attributable
focused gain remains 65.

Focused TypedArray `slice` coverage check:
`%TypedArray%.prototype.slice` computes start and end from the initial internal
length, constructs a writable `@@species` result, then revalidates source bounds
after observable species execution. Same-kind copies preserve raw element bits,
including same-buffer forward-copy semantics, while different kinds convert by
value. The shared `%TypedArray%[Symbol.species]` accessor and unaligned
length-tracking resizable-buffer construction are also covered. The exact path
reports **92 pass / 0 fail / 0 skip / 92 total**. CI `29144932312` and
`test262-full` `29144932309` confirm the change. Downloaded artifacts aggregate
to **26259 pass / 6764 fail / 12 timeout / 0 error / 15432 skip / 48467 total /
33023 pass-or-fail executed**, or **79.5%** of pass-or-fail files and **54.2%**
of the matrix. Against the preceding identical matrix, all 92 focused files
moved from skip to pass and three additional built-ins failures passed,
consistent with the shared `%TypedArray%[Symbol.species]` foundation added by
this unit; the focused gain is reported separately from those extra results.

Focused TypedArray `find` coverage check:
`%TypedArray%.prototype.find` validates the receiver and callback, snapshots the
initial internal length, then reads each current integer-indexed value before
calling the predicate with `(value, index, receiver)` and the supplied
`thisArg`. Callback-driven detach or shrink yields `undefined` for invalidated
future indexes, growth does not extend the visit count, and a truthy predicate
returns the value observed before that callback. The exact path reports **38
pass / 0 fail / 0 skip / 38 total**. CI `29145657670` and `test262-full`
`29145657675` confirm the change. Downloaded artifacts aggregate to **26297
pass / 6764 fail / 12 timeout / 0 error / 15394 skip / 48467 total / 33061
pass-or-fail executed**, or **79.5%** of pass-or-fail files and **54.3%** of the
matrix. Against the preceding identical matrix, exactly 38 files moved from
skip to pass with no fail, timeout, or error change.

Focused TypedArray `findIndex` coverage check:
`%TypedArray%.prototype.findIndex` uses the same receiver validation, callback
protocol, initial internal-length snapshot, and current integer-indexed reads as
`find`. Callback-driven detach or shrink yields `undefined` for invalidated
future indexes, growth does not extend the visit count, and a truthy predicate
returns its index while exhaustion returns `-1`. The exact path reports **38
pass / 0 fail / 0 skip / 38 total** locally. CI `29146424305` and
`test262-full` `29146424303` confirm the change. Downloaded artifacts aggregate
to **26332 pass / 6767 fail / 12 timeout / 0 error / 15356 skip / 47718 total /
33099 pass-or-fail executed**, or **79.6%** of pass-or-fail files and **55.2%**
of the matrix. Against the immediately preceding full run over the identical
matrix, all 38 focused files moved out of skip while the aggregate changed by
35 pass and 3 fail; the focused exact result is therefore reported separately
from the parallel full-run variance.

Focused TypedArray `findLast` coverage check:
`%TypedArray%.prototype.findLast` validates the receiver and callback, snapshots
the initial internal length, then reads each current integer-indexed value from
the final index toward zero before invoking the predicate. Callback-driven
detach or shrink yields `undefined` for invalidated future visits, growth does
not extend the visit count, and a truthy predicate returns the value observed
before that callback. The exact path reports **38 pass / 0 fail / 0 skip / 38
total**. CI `29147184493` and `test262-full` `29147184510` confirm the change.
Downloaded artifacts aggregate to **26371 pass / 6766 fail / 12 timeout / 0
error / 15318 skip / 47718 total / 33137 pass-or-fail executed**, or **79.6%**
of pass-or-fail files and **55.3%** of the matrix. Against the preceding
identical matrix, all 38 focused files moved out of skip while the aggregate
gained 39 pass and lost one fail, so the additional passing file is kept
separate from the focused gain.

Focused TypedArray `findLastIndex` coverage check:
`%TypedArray%.prototype.findLastIndex` uses the same receiver validation,
callback protocol, initial internal-length snapshot, and reverse current-value
reads as `findLast`. Callback-driven detach or shrink yields `undefined` for
invalidated future visits, growth does not extend the visit count, and a truthy
predicate returns its index while exhaustion returns `-1`. The exact path
reports **38 pass / 0 fail / 0 skip / 38 total**. CI `29147889854` and
`test262-full` `29147889860` confirm the change. Downloaded artifacts aggregate
to **26410 pass / 6765 fail / 12 timeout / 0 error / 15280 skip / 47718 total /
33175 pass-or-fail executed**, or **79.6%** of pass-or-fail files and **55.3%**
of the matrix. Against the preceding identical matrix, all 38 focused files
moved out of skip while the aggregate gained 39 pass and lost one fail, so the
additional passing file is kept separate from the focused gain.

Focused TypedArray `some` coverage check:
`%TypedArray%.prototype.some` validates the receiver and callback, snapshots the
initial internal length, reads each current integer-indexed value before
invoking the predicate with `(value, index, receiver)`, and returns immediately
on the first truthy result. Callback-driven detach or shrink yields `undefined`
for invalidated future indexes and growth does not extend the visit count. The
exact path reports **44 pass / 0 fail / 0 skip / 44 total**. Consolidating the
predicate loop leaves the four find-family paths at **152 pass / 0 fail / 0
skip / 152 total**. CI `29148631959` and `test262-full` `29148631970` confirm
the change. Downloaded artifacts aggregate to **26453 pass / 6766 fail / 12
timeout / 0 error / 15236 skip / 47718 total / 33219 pass-or-fail executed**,
or **79.6%** of pass-or-fail files and **55.4%** of the matrix. Against the
preceding identical matrix, all 44 focused files moved out of skip while the
aggregate gained 43 pass and one fail; the focused exact result is therefore
reported separately from the parallel full-run variance.

Focused TypedArray `every` coverage check:
`%TypedArray%.prototype.every` validates the receiver and callback, snapshots
the initial internal length, reads each current integer-indexed value before
invoking the predicate with `(value, index, receiver)`, and returns immediately
on the first falsy result. Callback-driven detach or shrink yields `undefined`
for invalidated future indexes and growth does not extend the visit count. The
exact path reports **44 pass / 0 fail / 0 skip / 44 total**. The shared `some`
and four find-family paths remain at **196 pass / 0 fail / 0 skip / 196 total**.
CI `29149369782` and `test262-full` `29149369800` confirm the change. Downloaded
artifacts aggregate to **26497 pass / 6766 fail / 12 timeout / 0 error / 15192
skip / 47718 total / 33263 pass-or-fail executed**, or **79.7%** of pass-or-fail
files and **55.5%** of the matrix. Against the preceding identical matrix,
exactly 44 files moved from skip to pass with no fail, timeout, or error change.

Focused TypedArray `forEach` coverage check:
`%TypedArray%.prototype.forEach` validates the receiver and callback, snapshots
the initial internal length, reads each current integer-indexed value before
invoking the callback with `(value, index, receiver)`, ignores callback return
values, and returns `undefined`. Callback-driven detach or shrink yields
`undefined` for invalidated future indexes and growth does not extend the visit
count. The exact path reports **42 pass / 0 fail / 0 skip / 42 total**. The
shared `every`, `some`, and four find-family paths remain at **240 pass / 0 fail
/ 0 skip / 240 total**. CI `29150129716` and `test262-full` `29150129689`
confirm the change. Downloaded artifacts aggregate to **26540 pass / 6765 fail
/ 12 timeout / 0 error / 15150 skip / 47718 total / 33305 pass-or-fail
executed**, or **79.7%** of pass-or-fail files and **55.6%** of the matrix.
Against the preceding identical matrix, all 42 focused files moved out of skip
while the aggregate gained 43 pass and lost one fail, so the additional passing
file is kept separate from the focused gain.

Focused TypedArray `includes` coverage check:
`%TypedArray%.prototype.includes` validates the receiver and snapshots its
internal length before observable `fromIndex` coercion, then reads current
integer-indexed values and compares them with SameValueZero. Resize or detach
during coercion therefore exposes `undefined` at invalidated snapshot indexes,
while NaN matches NaN and Number/BigInt content types remain distinct. The exact
path reports **45 pass / 0 fail / 0 skip / 45 total**. Its full resizable-buffer
matrix also exposed and now covers an interpreted call-environment GC root bug:
dynamic derived TypedArray constructors retain their `this` binding across
allocation pressure until `super()` initializes it. CI `29151186097` and
`test262-full` `29151186100` confirm the change. Downloaded artifacts aggregate
to **26586 pass / 6764 fail / 12 timeout / 0 error / 15105 skip / 48467 total /
33350 pass-or-fail executed**, or **79.7%** of pass-or-fail files and **54.9%**
of the matrix. Against the preceding identical matrix from `test262-full`
`29150690813`, the focused path moved 45 files from skip to pass while the GC
root fix moved one additional file from fail to pass.

Focused TypedArray `reduceRight` coverage check:
`%TypedArray%.prototype.reduceRight` validates its receiver and callback after
snapshotting the internal length. Without an explicit initial value it reads
the last current element as the accumulator and skips that index; otherwise it
visits every snapshot index in descending order. Values are read immediately
before each callback, so detach or shrink exposes `undefined` at invalidated
indexes while growth does not extend iteration. The exact Number/BigInt and
resizable-buffer path reports **50 pass / 0 fail / 0 skip / 50 total**. CI
`29152822658` and `test262-full` `29152822656` confirm the final change.
Downloaded artifacts aggregate to **26636 pass / 6764 fail / 12 timeout / 0
error / 15055 skip / 48467 total / 33400 pass-or-fail executed**, or **79.7%**
of pass-or-fail files and **55.0%** of the matrix. Against the preceding
identical matrix, exactly 50 focused files moved from skip to pass. The first
candidate full run exposed an allocation-order-sensitive Proxy failure; tracing
Proxy target/handler slots and rooting the transient `defineProperty`
descriptor restores that file without changing the final aggregate fail count.

Focused TypedArray `reduce` coverage check:
`%TypedArray%.prototype.reduce` shares the receiver validation, length snapshot,
callback checks, and accumulator rooting used by `reduceRight`, but selects the
first current element as the default accumulator and visits remaining snapshot
indexes in ascending order. Values are read immediately before each callback,
so detach and shrink expose `undefined` while growth does not extend iteration.
The exact Number/BigInt and resizable-buffer path reports **50 pass / 0 fail / 0
skip / 50 total**; the refactored reverse path remains **50 / 0 / 0 / 50**. CI
`29156205544` and `test262-full` `29156205566` confirm the change. Downloaded
artifacts aggregate to **26734 pass / 6764 fail / 12 timeout / 0 error / 14957
skip / 48467 total / 33498 pass-or-fail executed**, or **79.8%** of pass-or-fail
files and **55.2%** of the matrix. Against the preceding identical matrix,
exactly 50 focused files moved from skip to pass.

Focused TypedArray `map` coverage check:
`%TypedArray%.prototype.map` validates and snapshots the source, checks the
callback, and creates a writable species destination of at least the snapshot
length before visiting values. Species may select another TypedArray kind with
the same Number/BigInt content type. Each current value is read immediately
before callback invocation and the callback result is converted by the target
element kind, so constructor/callback resize and detach effects remain
observable without extending iteration. The exact path reports **85 pass / 0
fail / 0 skip / 85 total**. CI `29157121160` and `test262-full` `29157121173`
confirm the change. Downloaded artifacts aggregate to **26819 pass / 6764 fail /
12 timeout / 0 error / 14872 skip / 48467 total / 33583 pass-or-fail executed**,
or **79.9%** of pass-or-fail files and **55.3%** of the matrix.
Against the preceding identical matrix, exactly 85 focused files moved from
skip to pass.

Focused TypedArray `filter` coverage check:
`%TypedArray%.prototype.filter` validates and snapshots the source, then visits
all snapshot indexes before consulting `constructor` or `Symbol.species`.
Selected current values are preserved in visit order; only after predicates
complete does filter create a writable same-content-type species destination
sized to the selection count. Resize and detach expose `undefined` at
invalidated indexes without extending iteration. The exact path reports **85
pass / 0 fail / 0 skip / 85 total**. CI `29157976820` and `test262-full`
`29157976810` confirm the change. Downloaded artifacts aggregate to **26904
pass / 6764 fail / 12 timeout / 0 error / 14787 skip / 48467 total / 33668
pass-or-fail executed**, or **79.9%** of pass-or-fail files and **55.5%** of the
matrix. Against the preceding identical matrix, exactly 85 focused files moved
from skip to pass.

Focused TypedArray `indexOf` coverage check:
`%TypedArray%.prototype.indexOf` validates its receiver and snapshots the view
length before coercing `fromIndex`. It then checks each snapshot index for
current integer-index validity and compares present Number or BigInt values
with Strict Equality, so detach and shrink during coercion do not turn missing
elements into matches for `undefined`, while growth does not extend the search.
The exact path improves from **6 pass / 37 fail / 0 skip** to **43 pass / 0 fail
/ 0 skip / 43 total**. CI `29158983437` and `test262-full` `29158983402`
confirm the change. Downloaded artifacts aggregate to **26947 pass / 6764 fail
/ 12 timeout / 0 error / 14744 skip / 48467 total / 33711 pass-or-fail
executed**, or **79.9%** of pass-or-fail files and **55.6%** of the matrix.
Against the preceding identical matrix, exactly 43 focused files moved from
skip to pass.

Focused TypedArray `lastIndexOf` coverage check:
`%TypedArray%.prototype.lastIndexOf` validates its receiver and snapshots the
view length before coercing `fromIndex`. An omitted position starts at the last
snapshot index, while explicit `undefined` starts at index zero. The reverse
search checks current integer-index validity and uses Strict Equality, so
detach and shrink during coercion skip invalidated indexes and growth does not
extend the search. The exact path improves from **6 pass / 36 fail / 0 skip**
to **42 pass / 0 fail / 0 skip / 42 total**. CI `29159883869` and
`test262-full` `29159883857` confirm the change. Downloaded artifacts aggregate
to **26989 pass / 6764 fail / 12 timeout / 0 error / 14702 skip / 48467 total /
33753 pass-or-fail executed**, or **80.0%** of pass-or-fail files and **55.7%**
of the matrix. Against the preceding identical matrix, exactly 42 focused
files moved from skip to pass.

Focused TypedArray `toLocaleString` coverage check:
`%TypedArray%.prototype.toLocaleString` validates its receiver and snapshots
the view length, then reads each current element and invokes its
`toLocaleString` method with exactly two locale arguments, including explicit
`undefined` values when omitted. Primitive method lookup uses the TypedArray
method's Realm; Number locale conversion ignores radix semantics and BigInt
uses its own intrinsic locale method. Each returned value is converted with
`ToString`, preserving observable `toString`/`valueOf` hooks and abrupt
completions. Detach and shrink produce empty fields at invalidated snapshot
indexes, while growth does not extend the visit range. The exact path improves
from **6 pass / 33 fail / 0 skip** to **39 pass / 0 fail / 0 skip / 39 total**.
CI `29161632961` and `test262-full` `29161632974` confirm the independently
reviewed change. Downloaded artifacts aggregate to **27028 pass / 6764 fail /
12 timeout / 0 error / 14663 skip / 48467 total / 33792 pass-or-fail
executed**, or **80.0%** of pass-or-fail files and **55.8%** of the matrix.
Against the preceding identical matrix, exactly 39 focused files moved from
skip to pass.

Focused TypedArray `with` coverage check:
`%TypedArray%.prototype.with` validates and snapshots its source, coerces the
index before the replacement Number/BigInt value, then checks the computed
index against the source's current integer-index bounds. It creates a fresh
same-kind result from the method Realm's original intrinsic constructor and
copies the snapshot visit range without consulting `constructor` or
`Symbol.species`. Realm intrinsic constructors are held in a GC-rooted table,
so mutable global bindings do not affect `with`, `toReversed`, or `toSorted`.
The exact path improves from **1 pass / 21 fail / 0 skip** to **22 pass / 0 fail
/ 0 skip / 22 total**. CI `29162715549` and `test262-full` `29162715530`
confirm the independently reviewed change. Downloaded artifacts aggregate to
**27050 pass / 6764 fail / 12 timeout / 0 error / 14641 skip / 48467 total /
33814 pass-or-fail executed**, or **80.0%** of pass-or-fail files and **55.8%**
of the matrix. Against the preceding identical matrix, exactly 22 focused
files moved from skip to pass.

Focused TypedArray `Symbol.toStringTag` coverage check:
The configurable `%TypedArray%.prototype[Symbol.toStringTag]` getter reads the
receiver's internal TypedArray kind without consulting user properties or
buffer state. It returns the exact Number/BigInt TypedArray name even when the
view is detached, and returns `undefined` for primitives, DataView, and
ordinary objects instead of throwing. The exact path improves from **2 pass /
16 fail / 0 skip** to **18 pass / 0 fail / 0 skip / 18 total**. Independent
review also exposed that `Object.prototype.toString` ignored custom tags and
used heap class labels as fallback tags. It now performs observable
`Symbol.toStringTag` lookup, propagates getter failures, ignores non-string
values, and uses specification internal-slot fallback categories. The currently
admitted `built-ins/Object/prototype/toString/` coverage remains **26 pass / 0
fail / 15 skip / 41 total** after the structural fix. CI `29163993136` and
`test262-full` `29163993119` confirm the combined change. Downloaded artifacts
aggregate to **27070 pass / 6756 fail / 12 timeout / 0 error / 14629 skip /
48467 total / 33826 pass-or-fail executed**, or **80.0%** of pass-or-fail files
and **55.9%** of the matrix. Against the preceding identical matrix, 12 skipped
files and eight failing files moved to pass.

Focused TypedArray `buffer` coverage check:
The `%TypedArray%.prototype.buffer` accessor returns the receiver's original
backing ArrayBuffer by identity even after detach. It requires actual
TypedArray internal slots, so invoking it on primitives, DataView, ordinary
objects, or objects merely inheriting from a TypedArray throws TypeError. The
VM roots each Realm's original `%ArrayBuffer.prototype%`, so buffers allocated
by foreign-Realm TypedArray constructors, `from`, and `of` retain the correct
prototype even if globals are replaced. Native getter functions likewise use
their closure Realm's `%Function.prototype%`. The previously gated exact path
is now fully admitted at **12 pass / 0 fail / 0 skip / 12 total**. Against test262
`d1d583db95a521218f3eb8341a887fd63eda8ff1`, the supported subset remains
**11589 pass / 0 fail / 8850 skip / 20439 total**. CI `29165210243` and
`test262-full` `29165210230` succeeded. Downloaded artifacts aggregate to
**27082 pass / 6756 fail / 12 timeout / 0 error / 14617 skip / 48467 total /
33838 pass-or-fail executed**, or **80.0%** of pass-or-fail files and **55.9%**
of the matrix. Against the preceding identical matrix, exactly 12 skipped files
moved to pass.

Focused TypedArray `from` coverage check:
The exact frozen `built-ins/TypedArray/from/` file set is **21 pass / 0 fail /
0 skip / 21 total**. `%TypedArray%.from` no longer imposes a non-standard
65,536-element iterable cap. Constructor results are validated from raw
TypedArray slots so detached and out-of-bounds zero-length views cannot pass an
empty-source call. Iterator objects, cached `next`, each iterator result, and
previously collected object values remain rooted across `next`, `done`,
`value`, mapper, conversion, and forced-GC callbacks. Adversarial regressions
cover all three gaps found by independent review. The broader TypedArray
built-ins subtree is **1433 pass / 0 fail / 13 skip / 1446 total**, while the
supported subset remains **11589 pass / 0 fail / 8850 skip / 20439 total**. CI
`29169720681` and `test262-full` `29169720682` succeeded. Downloaded artifacts
aggregate to **27143 pass / 6755 fail / 12 timeout / 0 error / 14557 skip /
48467 total / 33898 pass-or-fail executed**, or **80.1%** of pass-or-fail files
and **56.0%** of the matrix. Against the preceding identical matrix, exactly
21 skipped files moved to pass.

Focused TypedArray built-ins completion check:
Against pinned test262 `d1d583db95a521218f3eb8341a887fd63eda8ff1`, the
complete `built-ins/TypedArray/` subtree now runs at **1446 pass / 0 fail / 0
skip / 1446 total**. The final 13 exact-path admissions cover the intrinsic
constructor surface, `Symbol.species`, and `%TypedArray%.of`; future files
remain gated until audited. Independent reviews found no admission-blocking
implementation defect. Rust regressions preserve rejection of detached and
out-of-bounds empty constructor results and root `of` arguments across forced
GC during construction and element conversion. The supported subset remains
**11589 pass / 0 fail / 8850 skip / 20439 total**. CI `29170927445` and
`test262-full` `29170927448` succeeded. Downloaded artifacts aggregate to
**27157 pass / 6755 fail / 11 timeout / 0 error / 14544 skip / 48467 total /
33912 pass-or-fail executed**, or **80.1%** of pass-or-fail files and **56.0%**
of the matrix. The code change moved exactly 13 skipped files to pass. One
additional built-ins timeout also passed in this run; that is timing variance,
not a conformance claim for this change.

Focused ES Module source-goal core check:
The first module milestone introduces an explicit Module source type through
the parser, compiler, VM, and CLI rather than treating module tests as strict
scripts. Module code is implicitly strict, has undefined top-level `this`, and
stores top-level var/function/lexical declarations in an isolated declarative
environment. Nested ordinary functions reset the Await grammar parameter,
while top-level module `await` remains contextual module syntax. Duplicate
labels are rejected as early errors. The runner invokes the real `--module`
CLI path and freezes admission to 35 import/export-free
`language/module-code/` files, which report **35 pass / 0 fail / 0 skip**.
The remaining 564 files in that subtree stay gated pending real import/export
parsing, resolution, linking, namespaces, live bindings, cycles, and module
evaluation. The supported subset remains **11589 pass / 0 fail / 8850 skip /
20439 total**. Independent parser review also exposed an accidental
`AsyncFunction` constructor pass that depended on permitting AwaitExpression
inside an ordinary function. RuJa now has a distinct `%AsyncFunction%`
intrinsic and dynamic async constructor; the focused built-ins path closes at
**15 pass / 0 fail / 3 skip / 18 total** without weakening module Await
grammar. A follow-up audit also isolated `%AsyncFunction%` and its prototype
per Realm, including dynamic-constructor fallback and GC roots. Feature
commits `6d0254f` and `9a22731` are confirmed by CI `29173247360` and
`test262-full` `29173247358`. Downloaded artifacts aggregate to **27196 pass /
6750 fail / 12 timeout / 0 error / 14509 skip / 48467 total / 33946
pass-or-fail executed**, or **80.1%** of pass-or-fail files and **56.1%** of
the matrix. Relative to the pre-module baseline, this is **+40 pass / -5 fail
/ -35 skip**; the module slice accounts for the 35 skip-to-pass moves and the
AsyncFunction intrinsic closes five previously failing files.

Focused ES Module graph check:
The next frozen slice adds 11 exact `language/module-code/` files for exported
var/let/const/function/generator/class declarations, side-effect imports,
named imports with aliases and trailing commas, live binding updates, shared
global state, and abrupt dependency completion. The subtree now reports
**46 pass / 0 fail / 553 skip / 599 total**. `Vm::run_module_file` resolves
relative canonical paths, links indirect environment bindings, evaluates each
dependency once per Realm, validates named and star re-exports, and preserves
cached module environments through GC. A subsequent cycle slice separates
declaration instantiation from evaluation, admits 22 additional binding and
cycle files, and brings the subtree to **68 pass / 0 fail / 531 skip / 599
total**. Cycles expose hoisted functions, preserve lexical TDZ, and cache one
abrupt evaluation result across each strongly connected component. The
Test262 runner stages module entries and relative fixtures in an isolated
temporary graph, leaving the pinned upstream checkout untouched. Negative
parse, resolution, and runtime metadata selects distinct CLI phases; parse
checks compile static semantics without evaluating the test. The supported
subset remains **11589 pass / 0 fail / 8850 skip / 20439 total**. Feature
commit `f0d5525` is confirmed by CI `29174854010` and `test262-full`
`29174854002`. Downloaded artifacts aggregate to **27207 pass / 6750 fail /
12 timeout / 0 error / 14498 skip / 48467 total / 33957 pass-or-fail
executed**, or **80.1%** of pass-or-fail files and **56.1%** of the matrix.
The admission moved exactly 11 files from skip to pass with fail, timeout, and
error outcomes unchanged.

The cyclic-module follow-up is feature commit `55f3b87`, confirmed by CI
`29176281928` and `test262-full` `29176281902`. Downloaded artifacts aggregate
to **27230 pass / 6750 fail / 11 timeout / 0 error / 14476 skip / 48467 total /
33980 pass-or-fail executed**, or **80.1%** of pass-or-fail files and **56.2%**
of the matrix. The admitted module slice accounts for exactly **+22 pass / -22
skip**; one pre-existing built-ins timeout also passed in this run and is
recorded as timing variance rather than part of the module conformance claim.

The next default-binding slice adds 47 exact files and brings
`language/module-code/` to **115 pass / 0 fail / 484 skip / 599 total**.
Default imports, mixed default-plus-named imports, default function/class
declarations (including generator and async forms), default expressions,
anonymous name inference, live bindings, default re-exports, abrupt completion,
and early errors all use the existing named-binding resolver with export name
`default`. Namespace imports and namespace objects remain gated as a separate
feature boundary. The authoritative supported subset remains **11589 pass / 0
fail / 8850 skip / 20439 total**.

Feature commit `f74d4bb` is confirmed by CI `29177056387` and `test262-full`
`29177056390`. Downloaded artifacts aggregate to **27276 pass / 6750 fail / 12
timeout / 0 error / 14429 skip / 48467 total / 34026 pass-or-fail executed**,
or **80.2%** of pass-or-fail files and **56.3%** of the matrix. The module job
moved exactly **+47 pass / -47 skip**. One unrelated built-ins test moved from
the preceding run's pass result back to timeout, so the aggregate delta is
**+46 pass / -47 skip / +1 timeout**.

The namespace slice adds 53 exact files and brings `language/module-code/` to
**168 pass / 0 fail / 431 skip / 599 total**. Namespace imports and
`export * as` share one cached namespace object per module record. The object
has a null prototype, is non-extensible, exposes sorted unambiguous export
names followed by `Symbol.toStringTag`, and reads live binding values through
its data descriptors. Set, delete, define-property, prototype mutation, TDZ,
cycle identity, default-through-star exclusion, and ambiguous-star omission
follow Module Namespace Exotic Object semantics. Imported bindings re-exported
with `export { name }` are normalized to indirect export entries so duplicate
paths to the same namespace resolve to the same module and binding. The
authoritative supported subset remains **11589 pass / 0 fail / 8850 skip /
20439 total**. Feature commit `c3683cb` is confirmed by CI `29178992161` and
`test262-full` `29178992137`. Downloaded artifacts aggregate to **27359 pass /
6720 fail / 12 timeout / 0 error / 14376 skip / 48467 total / 34079
pass-or-fail executed**, or **80.3%** of pass-or-fail files and **56.4%** of
the matrix. The module shard moved exactly **+53 pass / -53 skip**. The same
change made the `in` operator preserve Symbol PropertyKeys, independently
moving 30 built-ins files from fail to pass; the aggregate delta is therefore
**+83 pass / -30 fail / -53 skip** with timeout and error counts unchanged.

The static-semantics slice admits 125 additional exact module files, bringing
`language/module-code/` to **293 pass / 0 fail / 306 skip / 599 total**.
Arbitrary ModuleExportName string literals now work in named imports, direct
and indirect exports, and namespace re-exports. String import names require an
explicit local binding, local export specifiers cannot use a string as their
LocalName, and lone UTF-16 surrogate export names are rejected by the required
IsStringWellFormedUnicode early error. The slice also freezes already-supported
module declaration-position early errors, resolution failures, star/cycle
identity, and evaluation cases in a shared 125-file runner/analyzer manifest.
One upstream file that imports another non-fixture test and executes its
unprovided `assert` remains excluded. The authoritative supported subset stays
**11589 pass / 0 fail / 8850 skip / 20439 total**; full-matrix evidence is
confirmed by feature commit `1143291`, CI `29180581547`, and `test262-full`
`29180581587`. Downloaded artifacts aggregate to **27484 pass / 6720 fail / 12
timeout / 0 error / 14251 skip / 48467 total / 34204 pass-or-fail executed**,
or **80.4%** of pass-or-fail files and **56.7%** of the matrix. The module
shard accounts for exactly **+125 pass / -125 skip**; fail, timeout, error, and
every other shard are unchanged.

The first top-level-await slice freezes 209 exact syntax files and brings
`language/module-code/` to **502 pass / 0 fail / 97 skip / 599 total**. It
verifies AwaitExpression grammar across top-level statements, nested blocks,
exports, class heritage, `for await`, and other expression contexts while
preserving module early errors and ordinary-function Await grammar boundaries.
Two syntax files that require dynamic import remain gated. Runtime async-module
evaluation is deliberately not claimed by this slice: pending Promise
suspension, sibling dependency ordering, rejection propagation, and async SCC
completion require a persistent Promise-returning module evaluator rather than
the current synchronous DFS and microtask drain. The supported subset remains
**11589 pass / 0 fail / 8850 skip / 20439 total**; CI and full-matrix evidence
are confirmed by admission commit `28a3e58`, CI `29182081049`, and
`test262-full` `29182081069`. Downloaded artifacts aggregate to **27693 pass /
6720 fail / 12 timeout / 0 error / 14042 skip / 48467 total / 34413
pass-or-fail executed**, or **80.5%** of pass-or-fail files and **57.1%** of
the matrix. The module shard accounts for exactly **+209 pass / -209 skip**;
every other outcome and shard are unchanged.

The async-module evaluator adds 27 exact non-dynamic-import module-runtime TLA
files. Three parse-negative files and the script-goal `new await` test move to
the syntax manifest, which now contains 213 exact files. Together the syntax
and runtime admissions bring `language/module-code/` to **533 pass / 0 fail /
66 skip / 599 total**.
Module bodies execute with Promise capabilities and reuse the VM's suspended
async continuation state instead of synchronously draining `await_value`.
Evaluation starts ready siblings while async dependencies are pending, waits
for external dependency SCCs, runs cycle members in DFS postorder, delays
outside importers until the whole cycle settles, propagates rejection through
dependent SCCs, and persists every pending evaluation Promise in the canonical
module record as a GC root, including siblings left running after another
dependency rejects. The remaining TLA files depend on dynamic import or related
host semantics. The supported subset remains **11589 pass / 0 fail / 8850 skip
/ 20439 total**;
CI `29184350526` and `test262-full` `29184350527` confirm feature commit
`535fa28`. Downloaded artifacts aggregate to **27724 pass / 6720 fail / 12
timeout / 0 error / 14011 skip / 48467 total / 34444 pass-or-fail executed**,
or **80.5%** of pass-or-fail files and **57.2%** of the matrix. The module
shard accounts for exactly **+31 pass / -31 skip**; every other outcome and
shard are unchanged.

The first dynamic-import runtime slice freezes six exact
`language/expressions/dynamic-import/usage/top-level-import-then-*` script
files. A dedicated ImportCall opcode creates a fresh intrinsic Promise, uses
the canonical script-file referrer, loads and evaluates the relative module,
and fulfills with its cached live Module Namespace object. The focused subtree
is **6 pass / 0 fail / 999 skip / 1005 total** and tooling is **59/59**.
Module-origin imports inside an in-flight top-level-await graph, import
attributes, and rejection/catch matrices remain gated.
The supported subset rises to **11595 pass / 0 fail / 8844 skip / 20439
total**. CI `29186666321` and `test262-full` `29186666320` confirm feature
commit `66a0f4a`. Downloaded artifacts aggregate to **27730 pass / 6720 fail /
12 timeout / 0 error / 13855 skip / 48317 total / 34450 pass-or-fail
executed**, or **80.5%** of pass-or-fail files and **57.4%** of the matrix.

The second script-origin dynamic-import admission adds six exact files for a
fresh Promise per call, direct-eval referrer inheritance, missing-module
rejection, abrupt specifier coercion, and TypeError/URIError evaluation
rejections. Its exact path is now **12 pass / 0 fail / 993 skip / 1005 total**;
the supported subset is **11601 pass / 0 fail / 8838 skip / 20439 total**.
CI `29188197692` and `test262-full` `29188197664` pass for commit `6a81a07`.
Artifact-to-artifact comparison against the preceding run changes only the
expressions shard by **+6 pass / -6 skip**. The current 30 artifacts aggregate
to **27736 pass / 6720 fail / 12 timeout / 0 error / 13849 skip / 48317 total /
34456 pass-or-fail executed**, or **80.5%** of executed files and **57.4%** of
the matrix. This corrects the earlier prose transcription that overstated skip
and total by 150; the artifacts themselves are the authoritative denominator.

Module-origin dynamic imports now share a canonical `ModuleRuntime` across the
active graph and the VM registry. Static and dynamic imports reuse one namespace
object, self-imports evaluate once, and imports of an in-flight TLA module
settle through a reaction on its canonical evaluation Promise rather than a
recursive host drain. TLA continuation completion is separated from the host
API completion value, and all pending capability/runtime values participate in
GC tracing. The exact dynamic-import admission is **44 pass / 0 fail / 961 skip
/ 1005 total**; the supported subset is **11633 pass / 0 fail / 8806 skip /
20439 total**. CI `29190914767` and `test262-full` `29190914770` pass for commit
`8c2a5a4`. Artifact comparison changes only the expressions shard by **+32 pass
/ -32 skip**, producing **27768 pass / 6720 fail / 12 timeout / 0 error / 13817
skip / 48317 total / 34488 pass-or-fail executed**, or **80.5%** of executed
files and **57.5%** of the matrix.

`import.meta` now has a dedicated AST and bytecode operation, module-only
grammar and assignment-target checks, and a canonical null-prototype extensible
object per file-backed or inline module evaluation. Inline nested functions
retain their originating module's object and GC root rather than consulting a
VM-global slot. The frozen import-meta admission is **23/23** across runtime,
source-goal, constructor-goal, and assignment-target tests; dynamic import is
**45/45**, and the supported subset is **11656 pass / 0 fail / 8783 skip /
20439 total**. CI `29193198575` and `test262-full` `29193198576` pass for commit
`408ed9f`. Artifact comparison changes only expressions by **+23 pass / -23
skip**, producing **27791 pass / 6720 fail / 12 timeout / 0 error / 13794 skip
/ 48317 total / 34511 pass-or-fail executed**, or **80.5%** of executed files
and **57.5%** of the matrix.

Dynamic-import admission now also covers all 32 generated catch variants for
ambiguous indirect exports and circular indirect re-exports. These tests assert
that module declaration instantiation rejects the import Promise with a
`SyntaxError` across top-level, nested function, arrow, async-function,
async-generator, block, and loop contexts. The exact dynamic-import subtree is
**77 pass / 0 fail / 928 skip / 1005 total**; the supported subset is **11688
pass / 0 fail / 8751 skip / 20439 total**. CI `29195024392` and
`test262-full` `29195024404` pass for commit `26482ef`. Downloaded artifacts
change only expressions by **+32 pass / -32 skip**, producing **27823 pass /
6720 fail / 12 timeout / 0 error / 13762 skip / 48317 total / 34543
pass-or-fail executed**, or **80.5%** of executed files and **57.6%** of the
matrix.

The ordinary module-evaluation rejection family is now complete across its 32
generated TypeError and URIError cases. The 30 newly admitted nested variants
cover arrow functions, async functions, async generators, blocks, labels,
branches, and loops while preserving the fixture's original error kind through
the import Promise. Exact dynamic-import coverage is **107 pass / 0 fail / 898
skip / 1005 total**; the supported subset is **11718 pass / 0 fail / 8721 skip
/ 20439 total**. CI `29197086179` and `test262-full` `29197086220` pass for
commit `14341e0`. Downloaded artifacts change only expressions by **+30 pass /
-30 skip**, producing **27853 pass / 6720 fail / 12 timeout / 0 error / 13732
skip / 48317 total / 34573 pass-or-fail executed**, or **80.6%** of executed
files and **57.6%** of the matrix.

Missing-module and abrupt specifier-coercion rejection coverage now spans all
32 generated ordinary-import cases. The 30 newly admitted nested variants
verify asynchronous host-resolution rejection and raw thrown-value preservation
through arrow, async-function, async-generator, block, branch, and loop
contexts; source-phase and deferred imports remain separate proposal
boundaries. Exact dynamic-import coverage is **137 pass / 0 fail / 868 skip /
1005 total**; the supported subset is **11748 pass / 0 fail / 8691 skip /
20439 total**. CI `29198663249` and `test262-full` `29198663253` pass for commit
`eb27527`. Downloaded artifacts change only expressions by **+30 pass / -30
skip**, producing **27883 pass / 6720 fail / 12 timeout / 0 error / 13702 skip
/ 48317 total / 34603 pass-or-fail executed**, or **80.6%** of executed files
and **57.7%** of the matrix.

Dynamic import now admits the complete 28-file assignment-expression subtree.
The 27 newly admitted files verify full AssignmentExpression parsing and
evaluation for references, calls, members, assignments, short-circuiting,
observable object/function coercion, `await`, `yield`, `new.target`, and cover
grammar before host loading. Exact dynamic-import coverage is **164 pass / 0
fail / 841 skip / 1005 total**; the supported subset is **11775 pass / 0 fail /
8664 skip / 20439 total**. CI `29200204814` and `test262-full` `29200204823`
pass for commit `5a781d3`. Downloaded artifacts change only expressions by
**+27 pass / -27 skip**, producing **27910 pass / 6720 fail / 12 timeout / 0
error / 13675 skip / 48317 total / 34630 pass-or-fail executed**, or **80.6%**
of executed files and **57.8%** of the matrix.

The standard dynamic-import root runtime slice now admits 16 additional files
covering intrinsic and fresh Promise identity, observable specifier coercion,
once-only module evaluation, script/module namespace reuse, indirect
resolution, errored async cycles, and for-await rejection propagation. The
exact-path gate now also removes `top-level-await` for admitted dynamic-import
tests whose imported fixture, rather than the script itself, contains TLA; the
runner and analyzer share a regression for that boundary. Exact dynamic-import
coverage is **180 pass / 0 fail / 825 skip / 1005 total**; the supported subset
is **11791 pass / 0 fail / 8648 skip / 20439 total**. CI `29201893578` and
`test262-full` `29201893579` pass for commit `65b881f`. Downloaded artifacts
change only expressions by **+16 pass / -16 skip**, producing **27926 pass /
6720 fail / 12 timeout / 0 error / 13659 skip / 48317 total / 34646
pass-or-fail executed**, or **80.6%** of executed files and **57.8%** of the
matrix.
The expressions shard accounts for exactly **+6 pass / -6 skip**; every other
outcome and shard are unchanged.

Focused TypedArray prototype completion check:
Against pinned test262 `d1d583db95a521218f3eb8341a887fd63eda8ff1`, every
file admitted by RuJa's runner under `built-ins/TypedArray/prototype` now runs
green at **1404 pass / 0 fail / 0 skip / 1404 total**. The final two parent
files verify `%TypedArray%.prototype.constructor === %TypedArray%` and
`%TypedArray%.prototype[Symbol.iterator] === values` with writable,
non-enumerable, configurable descriptors. Focused regressions preserve these
identities through forced GC and independent main/created-Realm mutation. This
is complete admitted file coverage for the pinned subtree, not a claim of full
Test262 variant coverage or fully independent Realm primordials. The supported
subset remains **11589 pass / 0 fail / 8850 skip / 20439 total**. CI
`29168553907` and `test262-full` `29168553904` succeeded. Downloaded artifacts
aggregate to **27122 pass / 6755 fail / 12 timeout / 0 error / 14578 skip /
48467 total / 33877 pass-or-fail executed**, or **80.1%** of pass-or-fail files
and **56.0%** of the matrix. Against the preceding identical matrix, exactly
two skipped files moved to pass.

Focused TypedArray alias coverage check:
`%TypedArray%.prototype.toString` now aliases the exact original
`Array.prototype.toString` function with the standard data-property
descriptor. The Array method performs `ToObject`, observes `join`, calls a
callable join with the receiver, and otherwise falls back to
`Object.prototype.toString`. This makes detached Number and BigInt TypedArrays
throw through TypedArray join while preserving generic Array behavior. The VM
roots the original function so later Realm bootstrap does not observe mutable
`Array.prototype` state. The four frozen toString files and the
`Symbol.iterator === values` non-constructor file are **5 pass / 0 fail / 0
skip / 5 total**. The supported subset remains **11589 pass / 0 fail / 8850
skip / 20439 total**. CI `29167399296` and `test262-full` `29167399293`
succeeded. Downloaded artifacts aggregate to **27120 pass / 6755 fail / 12
timeout / 0 error / 14580 skip / 48467 total / 33875 pass-or-fail executed**,
or **80.1%** of pass-or-fail files and **56.0%** of the matrix. Against the
preceding identical matrix, four skipped files and one existing failing file
moved to pass.

Focused TypedArray size-accessor coverage check:
The `%TypedArray%.prototype.byteLength`, `byteOffset`, and `length` getters use
the receiver's internal slots and report zero for detached or out-of-bounds
views. Fixed and length-tracking resizable or growable shared-buffer views
preserve byte offsets and update aligned lengths as their backing storage
changes. Cross-Realm getter functions and TypeErrors use the getter Realm, and
the getters plus backing buffers survive forced GC. The audited file set is
frozen rather than prefix-admitted: `byteLength` is **18 pass / 0 fail**,
`byteOffset` is **16 pass / 0 fail**, and `length` is **18 pass / 0 fail**, for
**52 pass / 0 fail / 0 skip / 52 total**. The supported subset remains **11589
pass / 0 fail / 8850 skip / 20439 total** against test262
`d1d583db95a521218f3eb8341a887fd63eda8ff1`. CI `29166327417` and
`test262-full` `29166327397` succeeded. Downloaded artifacts aggregate to
**27115 pass / 6756 fail / 12 timeout / 0 error / 14584 skip / 48467 total /
33871 pass-or-fail executed**, or **80.1%** of pass-or-fail files and **55.9%**
of the matrix. Against the preceding identical matrix, exactly 33 skipped files
moved to pass.

Focused TypedArray `sort` coverage check:
`%TypedArray%.prototype.sort` validates write access and snapshots all values
before comparison. Its stable merge sort uses numeric Number/BigInt ordering by
default, places NaN after numeric values and `-0` before `+0`, and converts
custom comparator results with `ToNumber`. Comparator-driven resize or detach
does not alter the sorted snapshot; writes target only indexes still accepted
by the current view. Immutable buffers reject before comparator invocation. The
exact path reports **36 pass / 0 fail / 0 skip / 36 total**. CI `29154453789`
and `test262-full` `29154453779` confirm the final change. Downloaded artifacts
aggregate to **26672 pass / 6764 fail / 12 timeout / 0 error / 15019 skip /
48467 total / 33436 pass-or-fail executed**, or **79.8%** of pass-or-fail files
and **55.0%** of the matrix. Against the preceding identical matrix, exactly 36
focused files moved from skip to pass. The first candidate full run exposed an
allocation-order-sensitive stale inline-cache entry after heap-cell reuse;
invalidating property caches whenever GC collects restores that existing file
without changing the final aggregate fail count.

Focused TypedArray `toSorted` coverage check:
`%TypedArray%.prototype.toSorted` validates and snapshots its source, applies
the same stable Number/BigInt comparison semantics as `sort`, and writes the
ordered values into a fresh current-realm intrinsic of the source element kind.
The source remains unchanged and may use an immutable backing buffer. User
`constructor` and `Symbol.species` properties are never observed. The exact
path reports **12 pass / 0 fail / 0 skip / 12 total**. CI `29155327452` and
`test262-full` `29155327470` confirm the change. Downloaded artifacts aggregate
to **26684 pass / 6764 fail / 12 timeout / 0 error / 15007 skip / 48467 total /
33448 pass-or-fail executed**, or **79.8%** of pass-or-fail files and **55.1%**
of the matrix. Against the preceding identical matrix, exactly 12 focused files
moved from skip to pass.

Focused class-definition generator grammar check:
`yield` is now parsed as an AssignmentExpression alternative instead of a
PrimaryExpression. This preserves its weak binding, treats a line terminator
after `yield` as an omitted operand, rejects a newline before the `*` in
`yield*`, and recognizes omitted operands before conditional colons and
template substitution tails. The runner/analyzer lift `generators` and
`async-functions` only on `language/statements/class/definition/`. That path
moves from **38 pass / 0 fail / 27 skip** to **63 pass / 0 fail / 2 skip**,
while the supported subset moves to **8186 pass / 0 fail / 12252 skip**.

Focused generator function intrinsic and binding check:
`%GeneratorFunction.prototype%.prototype` now exposes the intrinsic
GeneratorPrototype and supplies the generator-object fallback when a function's
own `prototype` is not an object. Generator and async functions inherit the
restricted `caller`/`arguments` accessors, while ordinary FunctionExpression
names parse under their own Yield/Await parameters and GeneratorExpression
names still reject `yield`. Runner/analyzer admission removes only the
`generators` gate on `language/{statements,expressions}/generators/`. The two
paths report **155 pass / 0 fail / 401 skip**, and the supported subset moves
to **8339 pass / 0 fail / 12099 skip**.

Focused complete generator statement/expression check:
generator formal parameters now reject duplicate bound names in non-simple
lists and reject `YieldExpression`. Sloppy direct eval from a parameter
initializer raises `SyntaxError` when an introduced `var` conflicts with that
parameter binding before the generator body runs; non-conflicting eval
variables remain available to the body. Runner/analyzer admission additionally
lifts `default-parameters`, `destructuring-binding`, `object-rest`, `Symbol`,
and `Symbol.iterator` only on
`language/{statements,expressions}/generators/`. The two paths report **556
pass / 0 fail / 0 skip**, and the supported subset moves to **8740 pass / 0
fail / 11698 skip**.

Focused complete ordinary function declaration/expression check:
runner/analyzer admission lifts `default-parameters`,
`destructuring-binding`, `generators`, `object-rest`,
`class-fields-private`, and `Symbol.iterator` only on
`language/{statements,expressions}/function/`. This exercises the complete
ordinary function-form coverage while retaining those feature gates elsewhere.
Formal-parameter grammar also now rejects `AwaitExpression` in async forms and
outer `yield`/`await` expressions in arrow defaults, while nested ordinary
arrow bodies receive their own Yield/Await context.
The two paths move from **307 pass / 0 fail / 408 skip** to **715 pass / 0 fail
/ 0 skip**, and the supported subset moves to **9148 pass / 0 fail / 11290
skip**.

Focused complete ordinary arrow-function check:
runner/analyzer admission lifts `default-parameters`,
`destructuring-binding`, `generators`, `object-rest`, and `Symbol.iterator`
only on `language/expressions/arrow-function/`. The path moves from **144 pass
/ 0 fail / 199 skip** to **343 pass / 0 fail / 0 skip**, and the supported
subset moves to **9347 pass / 0 fail / 11091 skip**.

Focused escaped object async-method prefix local check:
object literal parsing now requires the contextual `async` method prefix to be
an unescaped token. An escaped identifier such as `\u0061sync` remains valid
as an ordinary method, property, or shorthand name, but cannot introduce a
second method name. With the relevant method-definition feature skips
temporarily lifted, `language/expressions/object/method-definition/` improves
from **158 pass / 1 fail / 144 skip** to **159 pass / 0 fail / 144 skip**.

Focused synchronous object method-definition check:
runner/analyzer admission lifts `async-functions`, `async-iteration`,
`class-fields-public`, `class-methods-private`, `default-parameters`,
`generators`, Symbol, `Symbol.asyncIterator`, and `Symbol.iterator` only on
`language/expressions/object/method-definition/`. Together with the escaped
`async` prefix early error above, the path moves from **52 pass / 0 fail / 251
skip** to **202 pass / 0 fail / 101 skip**. All 101 residual skips carry the
test262 `async` flag. This was the intermediate synchronous admission; those
files are admitted by the completed async check below. The supported subset at
this stage moved to **9497 pass / 0 fail / 10941 skip**.

Focused synchronous `yield*` delegation local check:
`yield*` now uses a dedicated VM state machine rather than a bytecode loop, so
the delegated iterator result object is forwarded unchanged and outer
`next`, `throw`, and `return` completions invoke the corresponding inner
methods with spec receiver and argument semantics. Final values, abrupt
getters/calls, non-object results, missing-`throw` cleanup, and primitive
well-known Symbol lookup are preserved. Runner/analyzer admission now lifts
`generators` and `Symbol.iterator` only on
`language/expressions/yield/`; the path moves from **34 pass / 29 fail** in
the prior relaxed diagnostic to **63 pass / 0 fail / 0 skip** under the
default runner, raising the supported subset to **9560 pass / 0 fail / 10878
skip**. Async-generator delegation also
prefers `Symbol.asyncIterator`, falls back to the sync iterator protocol,
awaits each delegated result, and rejects the returned Promise for iterator
protocol errors.

Focused primitive String protocol dispatch check:
`String.prototype.match`, `replace`, `replaceAll`, and `split` now query their
well-known Symbol hook only for object arguments. The 16 test262
`cstm-*-on-{string,number,boolean,bigint}-primitive.js` cases therefore avoid
prototype getter side effects, and a differential run over all **1130**
admitted `built-ins/String` files reports **0 status regressions** against the
previous green binary.

Focused async object method-definition check and admission:
`TEST262_RUN_ASYNC=1` runs `flags: [async]` files with completion-marker
classification instead of treating an empty process result as success. Across
`language/expressions/object/method-definition/`, the opt-in diagnostic moves
from **238 pass / 65 fail / 0 skip** before async-generator delegation fixes to
**286 pass / 17 fail / 0 skip**, then to **296 pass / 7 fail / 0 skip** after
`await` begins assimilating generic thenables and async `yield*` rewraps the
awaited delegated value instead of forwarding its raw iterator-result object.
Awaiting ordinary async-generator yields and preserving JavaScript Error
objects for native async-function rejections closes the diagnostic at **303
pass / 0 fail / 0 skip**. Synchronous `yield*` still forwards the original
result without observing its `value` getter. Runner and analyzer now admit the
async flag only on this exact path, so the default focused run is **303 pass /
0 fail / 0 skip** while unrelated async paths remain gated. The 101-file
admission raises the supported subset to **9661 pass / 0 fail / 10778 skip /
20439 total**.

Focused async arrow-function admission:
runner/analyzer admission removes `async-functions`, default-parameter, and
async-completion gates only on
`language/expressions/async-arrow-function/`. The exact path moves from **18
pass / 0 fail / 42 skip** to **60 pass / 0 fail / 0 skip** under the default
runner, and the analyzer reports no failure buckets. The supported subset
rises to **9703 pass / 0 fail / 10736 skip / 20439 total** while unrelated
async paths remain gated.

Focused async function-form admission:
runner/analyzer admission removes `async-functions`, default-parameter, and
async-completion gates only on `language/expressions/async-function/` and
`language/statements/async-function/`. Across the 93 expression files and 74
declaration files, the default run moves from **32 pass / 0 fail / 135 skip**
to **167 pass / 0 fail / 0 skip**, and the analyzer reports no failure buckets.
The supported subset rises to **9838 pass / 0 fail / 10601 skip / 20439
total** while unrelated async paths remain gated.

Focused async generator iterator-kind diagnostic:
with the gated features and async completion temporarily lifted only for
`language/{expressions,statements}/async-generator/`, the 924-file run improves
from **919 pass / 5 fail / 0 skip** to **921 pass / 3 fail / 0 skip**. Async
generator functions now expose distinct async intrinsic prototypes, so changes
to `%AsyncIteratorPrototype%` cannot leak onto Object.prototype. Internal
iterator records also distinguish Async-from-Sync adapters from native async
iterators: adapter values are awaited, while a Promise yielded by a manually
implemented async iterator preserves its identity. The remaining three cases
assert observable microtask ordering, so these paths are not admitted yet.

Focused async generator receiver-brand diagnostic:
`%AsyncGeneratorPrototype%` now installs dedicated async `next`, `return`, and
`throw` entry points. Each verifies the receiver has RuJa's async lazy-generator
internal brand before any resume, and otherwise returns a rejected Promise with
a native `TypeError`. The six test262 cases covering primitive, ordinary
object/function, async-generator function/prototype, and synchronous generator
receivers now pass. Across `built-ins/{AsyncGeneratorFunction,
AsyncGeneratorPrototype,AsyncIteratorPrototype}`, the diagnostic improves from
**57 pass / 27 fail / 0 skip** to **63 pass / 21 fail / 0 skip**. The focused
`language/statements/async-generator` result remains **298 pass / 3 fail / 0
skip**; request ordering, return-value awaiting, and the cross-realm prototype
case remain outside the supported admission boundary.

Focused iterator-result descriptor check:
the shared `CreateIterResultObject` path now defines `value` and `done` as
writable, enumerable, configurable data properties in insertion order. Local
regressions cover synchronous generators, async generators, Array iterators,
and RegExp String iterators, which also exercises the collection and regular
expression call sites that share the helper. The supported subset remains
**9838 pass / 0 fail / 10601 skip**, and the async-generator statement and
built-in diagnostics remain **298 pass / 3 fail** and **63 pass / 21 fail**,
respectively.

Focused async generator request-queue diagnostic:
async generators now retain a FIFO request queue and suspend their frames at
`await` without draining the Promise job queue inline. Explicit
`return expression` compilation emits the required Await, while implicit
completion and bare `return;` remain direct. The previously failing return and
`yield*` tick-order cases now pass, as do delegated iterator-value getter
errors and broken Promise `constructor` access during return. With async
generator gates temporarily lifted, the two async-generator language paths
are **924 pass / 0 fail / 0 skip**, and the statement-only path is **301 pass /
0 fail / 0 skip**. The related 84-file built-in diagnostic
improves from **63 pass / 21 fail** to **73 pass / 11 fail**. The remaining
failures are one general async-function pending-Await continuation case, one
cross-Realm constructor-prototype case, and nine `Symbol.asyncDispose` cases,
so the async-generator paths remain outside the default admission boundary.
The default supported subset remains **9838 pass / 0 fail**; the current local
test262 checkout reports **10600 skip / 20438 total**.

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

Focused class field initializer direct eval local check:
direct eval calls emitted from class field initializer values now carry
initializer context, including through functions defined by the initializer.
Eval source containing `arguments` is rejected as `SyntaxError`, `super()` is
disallowed while `super.prop` remains valid, and instance field initializer
`eval("new.target")` evaluates to `undefined` rather than the constructor. With
`class-fields-public` and `class-static-fields-public` temporarily lifted, the
direct-eval class-elements slice reports **82 pass / 0 fail / 94 skip**.

Focused class field initializer arrow `super` local check:
public class field initializers now parse with the class field initializer
`super` and `new.target` context, allowing `super.prop` while still rejecting
`super()` calls. Static public/private field initializer scopes now bind
`#super` to the constructor, so arrows created by static field initializers
resolve `super.staticProp` through the class constructor home object. The
runner now admits implemented public instance/static field coverage on
`language/{statements,expressions}/class/elements` by lifting only
`class-fields-public` and `class-static-fields-public` on those paths. The
focused class-elements run reports **422 pass / 0 fail / 2540 skip**, and the
supported subset rises to **6328 pass / 0 fail / 14110 skip**.

Focused class special-method constructor early-error local check:
class parsing now rejects instance `async constructor()`, `* constructor()`,
and `async * constructor()` methods as `SyntaxError`, while preserving static
async/generator methods named `constructor` as ordinary static methods. The
runner now admits implemented generator, async method, and async generator
method coverage on `language/{statements,expressions}/class/elements` without
lifting those features outside the class-elements paths. The focused
class-elements run reports **509 pass / 0 fail / 2453 skip**, and the
supported subset rises to **6415 pass / 0 fail / 14023 skip**.

Focused private-name direct eval visibility local check:
direct eval parsing now inherits private names visible through the caller's
class environment, so `eval("this.#m")` is accepted in class methods, instance
field initializers, private accessors, private methods, and static private
elements while preserving runtime private-name identity checks for unrelated
classes with the same spelling. The runner now admits only the 12 implemented
`language/statements/class/elements/private-*-visible-to-direct-eval*.js`
files by lifting private class-element feature gates on those exact paths. The
focused class-elements run reports **521 pass / 0 fail / 2441 skip**, and the
supported subset rises to **6427 pass / 0 fail / 14011 skip**.

Focused private async/generator contextual-keyword early-error local check:
escaped `await` identifiers are now rejected in binding, identifier-reference,
and label positions inside async method bodies. Generator method bodies also
reject a bare `yield` as the direct operand of a unary expression while still
allowing parenthesized YieldExpressions and identifiers inside nested ordinary
functions where the grammar permits them. The runner admits only the 32
matching private async/generator method parse-negative files. The focused
class-elements run reports **553 pass / 0 fail / 2409 skip**, the broader
private-class diagnostic improves from **2161 pass / 42 fail / 759 skip** to
**2193 pass / 10 fail / 759 skip**, and the supported subset rises to **6459
pass / 0 fail / 13979 skip**.

Focused instance private method initialization-order local check:
`InitializeInstanceElements` lowering now installs all own private methods and
accessors before any field initializer executes. Public and private fields
still run in source order, and derived constructors perform the two phases on
the receiver returned by `super()`. The runner admits only the declaration and
expression `prod-private-method-initialize-order.js` files. The focused
class-elements run reports **555 pass / 0 fail / 2407 skip**, the relaxed
private-class diagnostic improves from **2193 pass / 10 fail / 759 skip** to
**2195 pass / 8 fail / 759 skip**, and the supported subset rises to **6461
pass / 0 fail / 13977 skip**.

Focused private assignment-target Reference local check:
private names are now first-class `ReferenceRecord` referenced names. Private
reads and writes use `GetValue`/`PutValue`, while array/object destructuring,
rest targets, and `for-in`/`for-of` preserve the private Reference before
reading the source value. Brand checks occur when `PutValue` runs, so a source
getter can initialize the target's private slot after target evaluation, and
missing slots still throw `TypeError`. All 14 `privatefieldset-*` files pass.
The runner now admits all implemented private class-element feature tags on
`language/{statements,expressions}/class/elements`, which reports **2203 pass /
0 fail / 759 skip**; the supported subset rises to **8109 pass / 0 fail /
12329 skip**.

Focused Proxy/exotic private-element stamping local check:
ECMAScript private elements now belong to the common GC object cell, allowing
derived constructors to stamp fields, methods, and accessors onto Proxy,
revoked Proxy, Array, collection, Promise, ArrayBuffer/DataView/TypedArray,
iterator, and function receivers. The brand stays on the receiver itself;
private initialization and access do not forward to Proxy target objects or
invoke handler traps. GC traces values reachable only through private elements
and clears private brands before reclaimed cells are reused. The three private
Proxy tests pass, the class-elements paths report **2207 pass / 0 fail / 755
skip**, and the supported subset rises to **8113 pass / 0 fail / 12325 skip**.

Focused subclass constructor-classification local check:
class heritage now classifies interpreted async functions, generators, and
async generators as non-constructors before reading their `prototype`,
including through bound functions and Proxy wrappers. Async functions no
longer receive an own `prototype`; generator and async-generator functions
retain the prototype objects required by their call behavior. The Symbol
intrinsic remains valid as a class heritage constructor identity while direct
or derived construction rejects `new.target`. The complete
`language/statements/class/subclass/` path reports **109 pass / 0 fail / 0
skip**, and the supported subset rises to **8127 pass / 0 fail / 12311 skip**.

Focused built-in subclass admission local check:
the statement and expression `class/subclass-builtins/` paths now exercise
subclass construction for implemented AggregateError,
ArrayBuffer/DataView/TypedArray, Promise, and WeakMap/WeakSet constructors.
The path-scoped exception does not admit SharedArrayBuffer or WeakRef, whose
globals remain unimplemented. The combined paths report **68 pass / 0 fail / 4
skip**, and the supported subset rises to **8161 pass / 0 fail / 12277 skip**.

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

Focused ArrayBuffer detached accessor runner coverage check:
`ArrayBuffer.prototype.detached` now exposes a non-enumerable, configurable
getter named `get detached` with length 0. It returns `false` for live
ArrayBuffers, `true` after `$262.detachArrayBuffer()` or transfer detaches the
source, and throws `TypeError` for non-ArrayBuffer receivers. The normal runner
now admits implemented `built-ins/ArrayBuffer/` coverage with a path-scoped
exception for ArrayBuffer, `Reflect.construct`, and Symbol, reporting
**122 pass / 0 fail / 99 skip** on that path. Remaining skips stay behind
unsupported SharedArrayBuffer, resizable ArrayBuffer, DataView, and typed-array
helper feature gates.

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
  skip**. A subsequent round replaced those textual slot keys with
  per-evaluation private-name identities, closing the same-spelling
  cross-class brand gap described by this intermediate probe.
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

## Decorator syntax and auto-accessor core

`tools/test262_decorator_admission.txt` freezes the exact 24 generated class
files whose metadata is limited to `class` and `decorators`. The runner and
analyzer remove only the `decorators` gate for these paths; unknown future
files and broader decorator semantics remain skipped.

The lexer/parser retains restricted decorator member/call/parenthesized
expressions on classes and public class elements. Compilation evaluates those
expressions in source order with computed names, calls each list in reverse,
validates class/method/field replacement types, composes field initializer
functions in source order with the correct instance or constructor `this`, and
applies class decorators before static initialization. Auto-accessors use
unique hidden private backing slots and non-enumerable getter/setter pairs for
public/private and instance/static forms. Contextual `accessor` still parses as
an ordinary method or field when followed by `(`, `=`, `;`, `}`, `*`, or a
line terminator.

Local verification against Test262 `020cb740` is **24 pass / 0 fail / 0 skip**
for the exact manifest, **4184 pass / 0 fail / 4242 skip** for both class
subtrees, and **12751 pass / 0 fail / 7687 skip / 20438 total** for the current
supported subset. The pinned `d1d583d` subset is **12752 pass / 0 fail / 7687
skip / 20439 total**. Pending-PR private decorator families remain explicitly
outside this current-main admission.

The next runtime corpus is pending in Test262 PR #5048. Against pinned PR head
`58b825d0eb50f66bc171d7254d19d0e2928d2d3a`, the complete diagnostic is **657
pass / 0 fail / 0 skip / 657 total**. Its audited 509-file runtime slice covers
31 class, 116 method (40 public and 76 private), 64 getter (32 public and 32
private), 72 setter (36 public and 36 private), 136 field (68 public and 68
private), 88 public auto-accessor, and two late-`addInitializer` tests. The
complete diagnostic additionally includes all 88 private auto-accessor files,
both computed-member early-error files, and the PR's remaining parser and
expression coverage across class expressions and declarations.

That boundary verifies fresh context objects, receiver-argument
`access.has/get/set`, per-decorator `addInitializer` lifetime and callable
checks, grouped static-method/instance-method/static-field/instance-field/class
application, field and extra-initializer ordering, explicit initializer `this`,
constructable class replacements, replacement-aware inner class bindings, and
auto-accessor `{ get, set }` input plus optional `get`/`set`/`init` replacement
composition. It also verifies that auto-accessors apply with methods/accessors
but initialize backing storage with fields. Private fields additionally verify
identity-preserving brand checks and private `get`/`set`, `#`-prefixed context
names, wrong-brand behavior, and instance/static initializer ordering. Private
callables additionally verify mutable replacement bindings, ordinary/async/
generator/async-generator forms, accessor merging, `super` home objects,
instance/static installation, and static private slots on class decorator
replacements. Private auto-accessors additionally verify private-name identity,
branded access, `get`/`set`/`init` replacement composition, named-slot
installation before instance fields, source-order backing initialization,
static installation on the original and replacement classes, GC retention,
and cross-Realm errors. The class's inner binding exposes the original
constructor while class decorators run and the final replacement before static
initialization.
Because these files do not exist in current Test262 main (`020cb740`), the
runner keeps its broad `decorators` gate and does not claim an admission count
before upstream merge.

Commit `1981ee2` rejects the ambiguous parse where `@decorators[0]` became a
decorated computed field and the following method began after ASI. Decorated
computed methods and initialized fields remain valid, as do
`@(decorators[0])` and computed expressions inside decorator-call arguments.
CI `29374930032` and full matrix `29374930033` succeeded. All 30 downloaded
result artifacts are byte-for-byte identical to the Reference-routing
baseline, retaining **29085 pass / 6495 fail / 12725 skip / 12 timeout / 0
error / 48317 total / 35580 pass-or-fail executed**. Artifacts are retained at
`/tmp/ruja-artifacts-decorator-early-feature.jfQCql`.

Feature commit `e7cdaf2` completes private instance and static auto-accessor
decorators. CI `29378359326` and full matrix `29378359319` succeeded. All 30
downloaded result artifacts are byte-for-byte identical to the preceding
decorator baseline, retaining **29085 pass / 6495 fail / 12725 skip / 12
timeout / 0 error / 48317 total / 35580 pass-or-fail executed**. Artifacts are
retained at `/tmp/ruja-artifacts-private-auto-feature.W6Uh7X`.

Feature commit `d31bbb4` passed CI `29353983772` and full matrix
`29353983750`. All 30 downloaded result artifacts are byte-for-byte identical
to matrix `29351071014`, reproducing **29056 pass / 6614 fail / 12635 skip / 12
timeout / 0 error / 48317 total / 35670 pass-or-fail executed**. The feature
artifacts are retained at `/tmp/ruja-artifacts-private-field-feature.jCGjjh`.

Feature commit `7a7531a` passed CI `29349296562` and full matrix
`29349296827`. All 30 downloaded result artifacts are byte-for-byte identical
to matrix `29345693370`, reproducing **29056 pass / 6614 fail / 12635 skip / 12
timeout / 0 error / 48317 total / 35670 pass-or-fail executed**. The feature
artifacts are retained at `/tmp/ruja-artifacts-public-auto-feature.mI0pRB`.

Feature commit `4d7dbc9` passed CI `29343897291` and full matrix
`29343897349`. All 30 downloaded result artifacts are byte-for-byte identical
to matrix `29336676776`, reproducing **29056 pass / 6614 fail / 12635 skip / 12
timeout / 0 error / 48317 total / 35670 pass-or-fail executed**. The feature
artifacts are retained at
`/tmp/ruja-artifacts-public-decorator-feature.NYB1xi`.

Feature commit `139c6af` passed CI `29334768817` and full matrix
`29334768891`. Relative to matrix `29328245924`, expressions move by **+10
pass / -10 skip** and statements by **+14 pass / -14 skip**; the other 28
downloaded result artifacts are byte-for-byte identical. The aggregate is
**29056 pass / 6614 fail / 12635 skip / 12 timeout / 0 error / 48317 total /
35670 pass-or-fail executed**, or **81.5%** of executed tests.

Because decorator calls can throw while the original class or element remains
on the operand stack, catch guards now retain their frame-relative try-entry
stack depth. Native and explicit throws truncate to that depth before entering
the handler, preventing caught decorator failures from accumulating hidden GC
roots across loops or async/generator suspension.

Feature commit `72fe364` passed CI `29359759264` and full matrix
`29359759319`. All 30 downloaded result artifacts are byte-for-byte identical
to matrix `29355670154`, reproducing **29056 pass / 6614 fail / 12635 skip / 12
timeout / 0 error / 48317 total / 35670 pass-or-fail executed**. The feature
artifacts are retained at
`/tmp/ruja-artifacts-private-callable-feature.ny7KEc`.

## Iterator intrinsic and helper core

`tools/test262_iterator_admission.txt` freezes exactly 483 files: 23
`built-ins/Iterator` files covering the global constructor, subclass
construction, `%Iterator.prototype%[Symbol.iterator]`,
`%Iterator.prototype%[Symbol.dispose]`, and the `constructor` and
`Symbol.toStringTag` accessors; the Generator prototype's own
`Symbol.toStringTag`; all six `String.prototype[Symbol.iterator]` files; and
all seven `%StringIteratorPrototype%` files; all 19 `Iterator.from` files; and
all 18 `Iterator.prototype.toArray` files; plus all 36
`Iterator.prototype.map` and 37 `Iterator.prototype.filter` files; plus all 33
`Iterator.prototype.take` and 34 `Iterator.prototype.drop` files; plus all 44
`Iterator.prototype.flatMap` files; all 30 `Iterator.prototype.reduce` files;
all 27 `Iterator.prototype.forEach` files; all 33 `Iterator.prototype.some`
files; all 33 `Iterator.prototype.every` files; all 32
`Iterator.prototype.find` files; all 32 static `Iterator.concat` files; and all
38 static `Iterator.zip` files. The runner and analyzer admit only those exact
paths. `Iterator.zipKeyed` stays behind its joint-iteration feature gate.

The global constructor and prototype are Realm-specific. Direct calls and
construction reject, subclass construction allocates with the derived
prototype, and cross-Realm `NewTarget` fallback uses the active Realm's
intrinsic. Generator, Array, Map, Set, and RegExp String iterator prototypes
now inherit from `%Iterator.prototype%`; concrete prototypes retain their own
tags. RegExp String Iterator `next` also performs the required receiver brand
check. Primitive and boxed strings use the public iterator protocol and a
Realm-specific `%StringIteratorPrototype%`; replacement/deletion, normal
`ToString`, UTF-16 surrogate representation, branding, exhaustion,
extensibility, and GC retention are observable as specified.
`Iterator.from` implements GetIteratorFlattenable, intrinsic identity
preservation, cached `next`, and a branded Realm-specific wrapper whose
`return` lookup remains dynamic. `Iterator.prototype.toArray` caches `next`,
reads `done` before `value`, and allocates from the method Realm. Its
host-safety materialization cap closes the source iterator before throwing.
The shared `Array.from` path also follows iterable-first generic-constructor
and per-step mapping semantics with IteratorClose on abrupt completion.
`Iterator.prototype.map` and `filter` use Realm-specific branded lazy helpers
with cached direct `next`, deferred callback execution, exact mathematical
callback counters, ToBoolean filtering, dynamic IteratorClose, and distinct
suspended-start,
executing, suspended-yield, and completed states. Helper GC slots, integrity
operations, result/error Realms, `Iterator Helper` tagging, close-time fuel
aborts, and short-lock counter updates are covered by Rust regressions.
`Iterator.prototype.take` and `drop` reuse those branded helpers with exact
finite `BigUint` limits and an explicit positive-infinity state. Limit
conversion and close ordering, boundary close, skipped-value elision, native
loop fuel consumption, large radix-prefixed numeric strings, Realm behavior,
and GC retention are covered by Rust and Test262 regressions.
`Iterator.prototype.flatMap` retains a GC-traced inner iterator record with a
cached `next`, drains it before advancing the outer source, rejects primitive
mapper results, and preserves inner-before-outer close ordering and abrupt
completion priority. Reentrant running state, close-time roots, Realm behavior,
and empty-inner native-loop fuel are covered by dedicated Rust regressions.
`Iterator.prototype.reduce` is an eager direct-iterator consumer with cached
`next`, omitted initial-value seeding, specified callback indices, and a
GC-rooted accumulator. Step abrupt completions propagate without close;
reducer abrupt completions close the source while preserving the original
error. Realm behavior and native-loop fuel have dedicated regressions.
`Iterator.prototype.forEach` eagerly invokes `(value, index)` callbacks and
returns undefined. Validation ordering, cached `next`, callback-only close,
object-value GC roots, method-Realm errors, and native-loop fuel are covered by
Rust and Test262 regressions.
`Iterator.prototype.some` eagerly invokes `(value, index)` predicates and
returns a Boolean. Exhaustion does not close; predicate abrupt completion
closes while preserving the original error; a truthy result performs normal
close and propagates return lookup, call, and non-object-result failures.
Object-value roots, method-Realm errors, and native-loop fuel have dedicated
regressions. Callback helpers use exact `BigUint` counters and convert the
mathematical index to Number only at the callback boundary, with no non-spec
safe-integer exception.
`Iterator.prototype.every` uses the same validation, cached-next, exact-index,
rooting, and fuel model. Exhaustion returns true without closing; a falsey
predicate result performs normal close before returning false; predicate
abrupt completion closes while preserving the original error; and iterator
step failures propagate without close. Dedicated Rust regressions cover the
normal/abrupt close matrix, zero-close step failures, GC, and method-Realm
TypeErrors beyond the exact Test262 path.
`Iterator.prototype.find` returns the original first value whose predicate is
truthy and returns undefined on exhaustion. A found object remains rooted
through dynamic normal close; close failures override the value. Predicate
abrupt completion preserves the original error while closing, and iterator
step failures do not close. Local regressions additionally cover validation
before next lookup, cached next, callback `this`, done/value ordering,
no-return continuation, close precedence, GC, Realm errors, and fuel. This
completes every synchronous `%Iterator.prototype%` helper directory in the
current Test262 checkout.
`Iterator.concat` captures each object argument and its callable
`Symbol.iterator` method left-to-right without opening an iterator. Pulling
then lazily opens and drains one cached iterator record at a time. A yielded
`return()` closes only the active inner iterator; pre-start return, natural
exhaustion, and opener or step failures never close or open later records.
Captured records, active iterator methods, and the helper creation Realm are
GC-traced, and empty-source scans consume fuel. Shared Iterator Helper Realm
handling now distinguishes creation-Realm yielded results and resumed protocol
errors from borrowed-method-Realm terminal results and direct validation.
`Iterator.zip` validates options before eagerly exhausting the outer iterable,
opens and caches each direct input record, and eagerly snapshots longest-mode
padding. Its helper steps records in ascending order for shortest, longest,
and strict modes while tracking open records independently and applying
reverse `IteratorCloseAll` completion priority. Fresh tuple arrays and resumed
errors use the creation Realm; open records, cached methods, and padding values
are traced by GC. Native setup, inactive-slot, strict-check, extraction, and
close loops charge fuel, and extraction is failure-atomic so a fuel-aborted
`return()` leaves the helper completed. The supporting `Array.prototype.fill`
path now marks replaced sparse slots present, as required by the upstream zip
padding and input fixtures.

`Iterator.zipKeyed` selects own enumerable string and Symbol keys with
descriptor rechecks, omits missing, non-enumerable, and undefined-valued
entries, and uses keyed padding reads in longest mode. It shares zip's
ascending stepping, reverse close-all, Realm, GC, reentrancy, failure-atomic,
and fuel behavior while producing fresh null-prototype result records.
Proxy `ownKeys` support validates trap result types and duplicates before
target invariants and consumer filtering, and descriptor operations preserve
the required key, conversion, SameValue, Realm, and GC behavior.

Local verification is **513 pass / 0 fail / 1 skip / 514 total** for all of
`built-ins/Iterator`; the separately admitted Generator and String iterator
files also pass, for **527/527** exact manifest members.
`built-ins/Array/from` is **27 pass / 0 fail / 20 skip / 47 total**.
The related String iterator method,
`StringIteratorPrototype`, `ArrayIteratorPrototype`, `GeneratorPrototype`,
`MapIteratorPrototype`, `SetIteratorPrototype`, and
`RegExpStringIteratorPrototype` directories are **44 pass / 0 fail / 96 skip /
140 total**. Against pending Test262 PR #5048 at `58b825d0`, the complete
public-plus-private-callable decorator boundary is now **509/509**; the eight
private generator assertions previously blocked by the absent global
`Iterator` all pass. The current-main decorator manifest remains **24/24**,
and the supported subset remains **12751 pass / 0 fail / 7687 skip / 20438
total**.

Feature commits `3b6da8a` and `5a9ff6f` passed CI `29364732026` and full
matrix `29364732182`. Relative to the private-callable baseline, only the
built-ins result changes. The aggregate is **29085 pass / 6495 fail / 12725
skip / 12 timeout / 0 error / 48317 total / 35580 pass-or-fail executed**, or
**60.2%** of all files and **81.7%** of executed files. Downloaded artifacts
are retained at `/tmp/ruja-artifacts-iterator-feature.FIhF0c`.

Feature commit `4af8c31` passed CI `29381725859` and full matrix
`29381725849`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the private-auto-accessor docs baseline; built-ins moves exactly
**+13 pass / -13 skip**. The aggregate is **29098 pass / 6495 fail / 12712
skip / 12 timeout / 0 error / 48317 total / 35593 pass-or-fail executed**.
Artifacts are retained at
`/tmp/ruja-artifacts-string-iterator-feature.refyfB`.

Feature commit `a6d3949` passed CI `29386623291` and full matrix
`29386623314`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the String Iterator documentation baseline; built-ins moves
exactly **+46 pass / -9 fail / -37 skip**. The aggregate is **29144 pass /
6486 fail / 12675 skip / 12 timeout / 0 error / 48317 total / 35630
pass-or-fail executed**, or **60.3%** of all files and **81.8%** of executed
files. Artifacts are retained at
`/tmp/ruja-artifacts-iterator-from-feature.hvj9Nr`.

Feature commit `8c911a5` passed CI `29390731665` and full matrix
`29390731676`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the Iterator.from documentation baseline; built-ins moves exactly
**+73 pass / -73 skip**. The aggregate is **29217 pass / 6486 fail / 12602
skip / 12 timeout / 0 error / 48317 total / 35703 pass-or-fail executed**, or
**60.5%** of all files and **81.8%** of executed files. Artifacts are retained
at `/tmp/ruja-artifacts-iterator-map-filter-feature.ZSWPOC`.

Feature commit `d36456b` passed CI `29396353550` and full matrix
`29396353596`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the map/filter documentation baseline; built-ins moves exactly
**+67 pass / -67 skip**. The aggregate is **29284 pass / 6486 fail / 12535
skip / 12 timeout / 0 error / 48317 total / 35770 pass-or-fail executed**, or
**60.6%** of all files and **81.9%** of executed files. Artifacts are retained
at `/tmp/ruja-artifacts-iterator-take-drop-feature.1nIWLD`.

Feature commit `043852b` passed CI `29401085165` and full matrix
`29401085162`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the take/drop documentation baseline; built-ins moves exactly
**+44 pass / -44 skip**. The aggregate is **29328 pass / 6486 fail / 12491
skip / 12 timeout / 0 error / 48317 total / 35814 pass-or-fail executed**, or
**60.7%** of all files and **81.9%** of executed files. Artifacts are retained
at `/tmp/ruja-artifacts-iterator-flatmap-feature.wPZoin`.

Feature commit `92ce768` passed CI `29405803022` and full matrix
`29405803115`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the flatMap documentation baseline; built-ins moves exactly
**+30 pass / -30 skip**. The aggregate is **29358 pass / 6486 fail / 12461
skip / 12 timeout / 0 error / 48317 total / 35844 pass-or-fail executed**, or
**60.8%** of all files and **81.9%** of executed files. Artifacts are retained
at `/tmp/ruja-artifacts-iterator-reduce-feature.Sk5GRM`.

Feature commit `20dc605` passed CI `29410076808` and full matrix
`29410076834`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the reduce documentation baseline; built-ins moves exactly **+27
pass / -27 skip**. The aggregate is **29385 pass / 6486 fail / 12434 skip / 12
timeout / 0 error / 48317 total / 35871 pass-or-fail executed**, or **60.8%**
of all files and **81.9%** of executed files. Artifacts are retained at
`/tmp/ruja-artifacts-iterator-foreach-feature.7r4JfZ`.

Feature commit `74f540b` passed CI `29415121304` and full matrix
`29415121337`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the forEach documentation baseline; built-ins moves exactly **+33
pass / -33 skip**. The aggregate is **29418 pass / 6486 fail / 12401 skip / 12
timeout / 0 error / 48317 total / 35904 pass-or-fail executed**, or **60.9%**
of all files and **81.9%** of executed files. Artifacts are retained at
`/tmp/ruja-artifacts-iterator-some-feature.QN9R24`.

Feature commit `fdd6223` passed CI `29420614915` and full matrix
`29420614912`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the some documentation baseline; built-ins moves exactly **+33
pass / -33 skip**. The aggregate is **29451 pass / 6486 fail / 12368 skip / 12
timeout / 0 error / 48317 total / 35937 pass-or-fail executed**, or **61.0%**
of all files and **82.0%** of executed files. Artifacts are retained at
`/tmp/ruja-artifacts-iterator-every-feature.41ETUd`.

Feature commit `2bbe8e7` passed CI `29426108186` and full matrix
`29426108093`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the every documentation baseline; built-ins moves exactly **+32
pass / -32 skip**. The aggregate is **29483 pass / 6486 fail / 12336 skip / 12
timeout / 0 error / 48317 total / 35969 pass-or-fail executed**, or **61.0%**
of all files and **82.0%** of executed files. Artifacts are retained at
`/tmp/ruja-artifacts-iterator-find-feature.hwOlff`.

Feature commit `decf8d5` passed CI `29432285167` and full matrix
`29432285229`. Of the 30 downloaded result artifacts, 29 are byte-for-byte
identical to the find documentation baseline; built-ins moves exactly **+32
pass / -32 skip**. The aggregate is **29515 pass / 6486 fail / 12304 skip / 12
timeout / 0 error / 48317 total / 36001 pass-or-fail executed**, or **61.1%**
of all files and **82.0%** of executed files. Artifacts are retained at
`/tmp/ruja-artifacts-iterator-concat-feature.35pEbB`.

Feature commit `6f8c291` admits all 38 static `Iterator.zip` files. Local
verification is **38/38** for zip, **483/483** for the exact Iterator manifest,
**469 pass / 0 fail / 45 skip** for `built-ins/Iterator`, and **12751 pass / 0
fail / 7687 skip / 20438 total** for the supported subset. CI `29438450582`
and full matrix `29438450881` succeeded. Of the 30 result artifacts at
`/tmp/ruja-artifacts-iterator-zip-feature.9FadMn`, 29 are byte-for-byte
identical to the concat documentation baseline; built-ins moves exactly **+39
pass / -1 fail / -38 skip**, combining 38 newly admitted zip files with the
sparse `Array.prototype.fill` correction. The normalized aggregate is **29554
pass / 6485 fail / 12266 skip / 12 timeout / 0 error / 48317 total / 36039
pass-or-fail executed**, or **61.2%** of all files and **82.0%** of executed
files. The raw CI artifacts additionally contain 150 unsupported built-ins
skips and therefore total 48467 files.

Feature commit `93de368` admits all **44** static `Iterator.zipKeyed` files and
all **40** Proxy/Reflect `ownKeys` files. Local verification is **44/44** for
zipKeyed, **84/84** for the combined new boundary, **527/527** for the exact
Iterator manifest, **513 pass / 0 fail / 1 skip / 514 total** for
`built-ins/Iterator`, and **12751 pass / 0 fail / 7687 skip / 20438 total** for
the supported subset. Rust all-target/all-feature tests, Clippy with warnings
denied, formatting, release and wasm32 builds, and the **86/86** Python tooling
suite pass. Two independent final reviews reported no high- or medium-severity
finding.

The first feature CI `29446474795` exposed a tooling-only permission error
while probing an unavailable local Test262 checkout; full run `29446474751`
was canceled to avoid duplicating the corrected run. Commit `5339ba3` fixed
that probe, and commit `e59bc43` classified Test262's unsupported host-only
`IsHTMLDDA` feature as a skip instead of retaining accidental Object target
validation results. Final CI `29448974479` and full matrix `29448974428`
succeeded. Compared with the Iterator.zip documentation baseline, only annexB
and built-ins change: built-ins moves **+208 pass / -124 fail / -84 skip**,
while the corrected `IsHTMLDDA` policy moves annexB **-11 pass / -13 fail /
+24 skip**. The aggregate is **29751 pass / 6348 fail / 12206 skip / 12 timeout
/ 0 error / 48317 total / 36099 pass-or-fail executed**, or **61.6%** of all
files and **82.4%** of executed files. Artifacts are retained at
`/tmp/ruja-artifacts-iterator-zip-keyed-feature-final.CJEoTi`.

Feature commit `6dad4e3` makes every foreign Realm `Object` static method a
distinct native function with that Realm's `%Function.prototype%`, native
errors, Array results, ordinary-object results, descriptor normalization, and
primitive wrappers. Object-specific allocation paths retain original Realm
intrinsics even after mutable global bindings are replaced. Forced-GC Rust
regressions cover values/entries accumulation, fresh `groupBy` iterators and
coercible keys, accessor-produced `fromEntries` keys and values, Proxy
descriptor aggregation, and retained methods across explicit collection.

Local final gates pass: Rust all-target/all-feature tests including builtins
**429/429** and fuel **19/19**, Clippy with warnings denied, formatting,
release and wasm32 builds, Python tooling **86/86**, `built-ins/Object`
**3120 pass / 121 fail / 170 skip / 3411 total**, and the current full-clone
supported subset **12752 pass / 0 fail / 7687 skip / 20439 total**. Upstream
Test262 currently contains only one cross-Realm Object file, which covers
NewTarget prototype fallback rather than static method Realm, so no matrix
test changes are expected. CI `29454210263` and full matrix `29454210292`
succeeded; all 30 artifacts at
`/tmp/ruja-artifacts-object-statics-feature.kgM3Qv` are byte-for-byte identical
to the Iterator.zipKeyed documentation baseline and retain **29751 pass / 6348
fail / 12206 skip / 12 timeout / 0 error / 48317 total / 36099 pass-or-fail
executed** (**61.6%** of all files, **82.4%** of executed files).

Feature commit `436e7ea` completes the active `%Object%` constructor and
distinct-`NewTarget` Realm paths. Function calls ignore their receiver;
nullish inputs allocate with the active constructor Realm's rooted
`%Object.prototype%`; primitive boxing uses that Realm; and object arguments
are returned only when `NewTarget` is the active `%Object%`. Subclass and
`Reflect.construct` paths ignore the argument and allocate exactly once from
the observed `NewTarget.prototype`, falling back to the constructor Realm's
rooted intrinsic when the value is not an object.

`GetFunctionRealm`-style traversal now follows bound and Proxy targets without
an arbitrary depth cap, rejects revoked Proxies, and normalizes a function's
lexical closure environment to its global Realm. The Object fallback no longer
consults a replaceable global `Object` binding. Rust regressions cover active
and distinct NewTarget paths, foreign primitive boxing, subclassing, forced GC
during Proxy prototype lookup, 40 bound-function layers, revoked Proxy errors,
nested foreign closures, mutable global replacement, intrinsic survival, and
the zero-preallocation object-argument path under a saturated heap limit.

The frozen admission adds exactly
`built-ins/Object/is-a-constructor.js`,
`built-ins/Object/proto-from-ctor-realm.js`, and
`built-ins/Object/subclass-object-arg.js`, all passing at **3/3**. Final local
gates pass: all Rust targets/features including builtins **431/431**, Clippy
with warnings denied, formatting, release and wasm32 builds, tooling **87/87**,
`built-ins/Object` **3123 pass / 121 fail / 167 skip / 3411 total**, and the
supported subset **12751 pass / 0 fail / 7687 skip / 20438 total**. Two
independent final reviews reported no high- or medium-severity finding.

CI `29458827810` and full matrix `29458827839` succeeded. Of the 30 artifacts
at `/tmp/ruja-artifacts-object-constructor-feature.WPjmhF`, 29 are
byte-for-byte identical to the Object-static documentation baseline; built-ins
moves exactly **+3 pass / -3 skip**. The aggregate is **29754 pass / 6348 fail
/ 12203 skip / 12 timeout / 0 error / 48317 total / 36102 pass-or-fail
executed**, or **61.6%** of all files and **82.4%** of executed files.

## Object.prototype Realm isolation

Feature commit `44ff53f` gives every Realm fresh native identities for all ten
`%Object.prototype%` methods and both `__proto__` accessors. The foreign global
object, `%Function.prototype%`, Error hierarchy, primitive wrapper prototypes,
TypedArray intrinsic chain, and Atomics namespace now consistently inherit
from that Realm's rooted `%Object.prototype%`; mutable main-Realm Object state
cannot leak into a newly created Realm.

The legacy accessor methods preserve Symbol keys, abrupt property-descriptor
and prototype operations, current-Realm descriptor allocation, and forced-GC
lifetimes. `isPrototypeOf` and legacy prototype walks use Proxy-aware internal
operations and consume host fuel, so a Proxy that reports itself as its own
prototype cannot trap the VM in an unbounded native loop. The exact frozen
admission adds 40 files at **40/40**. The full `built-ins/Object/prototype`
subtree is **242 pass / 0 fail / 6 skip / 248 total**, broad
`built-ins/Object` is **3164 pass / 120 fail / 127 skip / 3411 total**, the
supported subset remains **12751 pass / 0 fail / 7687 skip / 20438 total**,
and Python tooling is **88/88**. Rust all-target/all-feature tests, Clippy with
warnings denied, formatting, release, and wasm32 checks all pass. Independent
reviews left no valid high- or medium-severity finding; one proposed
`propertyIsEnumerable` ordering change was rejected because ECMA-262 and
Test262 require `ToPropertyKey` before `ToObject(this)`.

CI `29463295702` and full matrix `29463295657` succeeded. Of the 30 downloaded
result artifacts at `/tmp/ruja-artifacts-object-prototype-feature.Oxopq0`, 29
are byte-for-byte identical to the Object-constructor documentation baseline;
built-ins moves exactly **+41 pass / -1 fail / -40 skip**. The normalized
aggregate is **29795 pass / 6347 fail / 12163 skip / 12 timeout / 0 error /
48317 total / 36142 pass-or-fail executed**, or **61.7%** of all files and
**82.4%** of executed files. Raw artifacts additionally contain the same 150
unsupported built-ins skips as the baseline and therefore total 48467 files.
The six remaining prototype-subtree skips require Proxy array/callable
classification and GeneratorFunction/Promise fallback tag work.

## Object.prototype.toString completion

Feature commit `0d64d5b` closes those final six skips. `Object.prototype`
`toString` now applies `ToObject`, then Proxy-aware iterative `IsArray`, then
callable and internal-slot fallback classification before the observable
`@@toStringTag` lookup. This preserves nested and revoked Proxy semantics,
including revocation during the tag getter, and gives strict tag getters the
boxed primitive receiver required by the specification. Promise and
GeneratorFunction prototypes expose configurable, non-writable standard tags;
non-string and deleted tags fall back without incorrectly branding Promise,
Symbol, or BigInt values.

The same audit found that `Proxy.revocable` stored its associated Proxy as an
untraced numeric heap index. The revoker now owns a traced object reference,
removes it on first call, remains idempotent, and permits collection after
revocation. Cross-job WeakRef/GC regressions prove both retention and release;
forced-GC tag getters prove the boxed receiver remains rooted.

Exact Object-prototype admission is now **46/46** and the complete
`built-ins/Object/prototype` subtree is **248 pass / 0 fail / 0 skip / 248
total**. Broad `built-ins/Object` is **3170 pass / 120 fail / 121 skip / 3411
total**, the supported subset remains **12751 pass / 0 fail / 7687 skip /
20438 total**, and tooling is **88/88**. Rust all-target/all-feature tests
including builtins **434/434**, Clippy with warnings denied, formatting,
release, and wasm32 checks pass. GPT and Umans reviews found no remaining valid
high- or medium-severity issue after their GC-test findings were strengthened.

CI `29466781968` and full matrix `29466782034` succeeded. Of the 30 artifacts
at `/tmp/ruja-artifacts-object-tostring-feature.g9JfLR`, 29 are byte-for-byte
identical to the preceding documentation baseline. Built-ins moves **+7 pass /
-1 fail / -6 skip**: the six newly admitted files plus the existing
`built-ins/Promise/prototype/Symbol.toStringTag.js` failure now pass. The
normalized aggregate is **29802 pass / 6346 fail / 12157 skip / 12 timeout /
0 error / 48317 total / 36148 pass-or-fail executed**, or **61.7%** of all
files and **82.4%** of executed files. Raw artifacts contain the same 150
unsupported built-ins skips and total 48467 files.

## Promise Realm intrinsic isolation

Feature commit `6916e03` installs a distinct `%Promise%` constructor,
`%Promise.prototype%`, prototype methods, static methods, and species getter
for every `$262.createRealm()` Realm. VM maps retain the original constructor
and prototype as GC roots, so internal operations do not depend on replaceable
global `Promise` properties.

Promise allocation now distinguishes the Realm of the active native method,
an interpreted async function's closure, an explicit module environment, and
the Realm of a distinct `NewTarget`. Resolving functions and capability
executors carry that Realm in their native closure and inherit its
`%Function.prototype%`. `Promise.all`, `allSettled`, `any`, and
`withResolvers` allocate their observable arrays, records, result objects,
and `AggregateError` values from the method Realm. This also keeps foreign
async functions and `for await` on the foreign Promise graph after globals are
replaced and after explicit collection.

The exact frozen admission opens only
`built-ins/Promise/proto-from-ctor-realm.js`. Local verification is **256 pass
/ 0 fail / 447 skip / 703 total** for `built-ins/Promise`, **12751 pass / 0
fail / 7687 skip / 20438 total** for the supported subset, and **89/89** for
the Python tooling suite. Rust all-target/all-feature tests including builtins
**437/437**, Clippy with warnings denied, formatting, release, and wasm32
checks pass. Two independent final reviews report no remaining high- or
medium-severity finding in this Promise unit.

[Decision Log]
- 목적과 의도: make created Realms own the complete Promise behavior
  needed by construction, async execution, and Promise combinators.
- 기존 구현 및 제약 조건: RuJa stored only one main-Realm
  Promise pair, while native closures, result containers, and error objects
  could expose their Realm through prototype identity and error provenance.
- 검토한 주요 대안: clone main-Realm function values; consult
  each Realm's mutable global `Promise`; or retain original per-Realm
  intrinsics and select them from execution or constructor Realm state.
- 선택한 방식: install fresh native graphs, store constructor/prototype
  maps as GC roots, pass an explicit Realm where async setup precedes frame
  creation, and use active native closures for method-created functions and
  containers.
- 다른 대안 대신 이 방식을 선택한 이유: cloning preserves the wrong native
  `[[Realm]]`, while global lookup is observably mutable and cannot implement
  intrinsic fallback semantics.
- 장점, 단점 및 영향: identity, error provenance, async allocation,
  and GC behavior now follow Realm boundaries; VM state and allocation helpers
  gain small Realm maps and explicit selection APIs. Generator and
  AsyncGenerator intrinsic graphs were kept as separate follow-up units at
  that stage because their method objects still had main-Realm identities.

CI `29471610323` and full matrix `29471610399` succeeded. Of the 30 result
artifacts at `/tmp/ruja-artifacts-promise-feature.qrjKLP`, 29 are byte-for-byte
identical to the Object-toString documentation baseline; built-ins moves
exactly **+1 pass / -1 skip**. The aggregate is **29803 pass / 6346 fail /
12156 skip / 12 timeout / 0 error / 48317 total / 36149 pass-or-fail
executed**, or **61.7%** of all files and **82.4%** of executed files.

## Synchronous Generator Realm isolation

Feature commit `c768189` installs independent `%GeneratorFunction%`,
`%GeneratorFunction.prototype%`, and `%Generator.prototype%` graphs for every
created Realm. The constructor inherits from that Realm's `%Function%`, the
function prototype inherits from `%Function.prototype%`, and the generator
prototype inherits from `%Iterator.prototype%`. Constructor, function
prototype, and generator prototype are all direct GC roots because their
configurable graph properties may be deleted independently.

Source `function*` creation selects the defining Realm for both the function
object and its fresh own `prototype` object. Dynamic `GeneratorFunction`
construction keeps the fresh own prototype and global scope in the active
constructor Realm while obtaining a non-object `NewTarget.prototype` fallback
from the `NewTarget` Realm. Calling a generator with a non-object own
`prototype` falls back through the generator function's Realm, and iterator
result objects use the borrowed native method's Realm `%Object.prototype%`.

The frozen admission opens only the three cross-Realm files in
`tools/test262_generator_function_admission.txt`. They pass **3/3**; all 23
`built-ins/GeneratorFunction` files also pass in a direct diagnostic, and
`language/expressions/generators` is **290/290**. The supported subset remains
**12751 pass / 0 fail / 7687 skip / 20438 total**, Python tooling is **90/90**,
and Rust generator tests are **74/74** with builtins **437/437**. Rust
all-target/all-feature tests, Clippy with warnings denied, formatting,
release, and wasm32 checks pass. Both final reviews report no high- or
medium-severity finding in the stated synchronous-only boundary.

The initial feature commit `c768189` passed all local gates but CI
`29475087099` found that the optional live-Test262 metadata probe could raise
`PermissionError` while checking an unavailable checkout; its corresponding
full run `29475087079` was cancelled. Follow-up `935df28` treats any filesystem
lookup error as "metadata unavailable", matching the existing optional probe
contract. Final CI `29475407227` and full matrix `29475407238` succeeded.

Against the Promise documentation baseline, 29 of 30 result artifacts at
`/tmp/ruja-artifacts-generator-feature.dT0FGu` are byte-for-byte identical.
Only built-ins changes, by exactly **+2 pass / -2 skip**. The admitted
`language/expressions/generators/eval-body-proto-realm.js` was already executed
and passing through the broader generator-prefix admission, so the three-file
manifest intentionally produces only two newly executed matrix files. The
aggregate is **29805 pass / 6346 fail / 12154 skip / 12 timeout / 0 error /
48317 total / 36151 pass-or-fail executed**, or **61.7%** of all files and
**82.4%** of executed files.

[Decision Log]
- 목적과 의도: give each Realm a complete synchronous generator intrinsic
  graph and make every syntax, dynamic-construction, call, and result path use
  the specification's defining, constructor, or method Realm.
- 기존 구현 및 제약 조건: all synchronous generator objects referenced two
  main-Realm VM fields; configurable constructor/prototype properties also
  meant graph reachability alone could not preserve every intrinsic through
  GC.
- 검토한 주요 대안: clone main-Realm graph objects, infer intrinsics from
  mutable global properties, combine synchronous and asynchronous generator
  support in one patch, or install and root an explicit synchronous graph per
  Realm.
- 선택한 방식: add a per-Realm installer and direct roots for all three
  intrinsic identities, select maps from lexical/native/NewTarget Realms, and
  admit only the three cross-Realm Test262 files covered by this unit.
- 다른 대안 대신 이 방식을 선택한 이유: cloning keeps the wrong native
  `[[Realm]]`; global lookup is mutable; and async generators have a separate
  AsyncIterator and Promise allocation graph whose risk and Test262 boundary
  should be verified independently.
- 장점, 단점 및 영향: synchronous generator identity, fallback, result
  allocation, and GC behavior now match Realm boundaries. Each created Realm
  retains three additional intrinsic roots. The asynchronous graph is handled
  independently in the following section because its queued Promise Realm
  semantics require a separate boundary.

## Asynchronous Generator Realm isolation

Every Realm now installs and directly roots `%AsyncIteratorPrototype%`,
`%AsyncGeneratorPrototype%`, `%AsyncGeneratorFunction%`, and
`%AsyncGeneratorFunction.prototype%`. The async iterator prototype inherits
from the Realm's `%Object.prototype%`; the async generator prototype and its
`next`/`return`/`throw` methods are fresh per Realm; and the hidden constructor
inherits from the Realm's `%Function%` constructor while its prototype inherits
from `%Function.prototype%`.

Source and dynamic async generator functions select the defining or active
constructor Realm for their function and fresh own `prototype` objects.
Distinct-`NewTarget` fallback uses the `NewTarget` Realm, while calling a
function whose own `prototype` is not an object falls back through the
function's Realm. All four intrinsic identities are independent GC roots so
deleting configurable graph links cannot make a live Realm lose an intrinsic.

Async generator queue processing preserves the specification's two Realm
roles. Borrowing `next`, `return`, or `throw` creates the request Promise in the
native method's Realm. Internal PromiseResolve/Await work, delayed native error
materialization, and `{ value, done }` completion records use the generator's
closure Realm. Tests cover both borrowing directions, all three methods,
await-before-yield, delayed TypeError creation, mutable `Function`/`Object`/
`Promise` globals, distinct `NewTarget` with GC inside a Proxy trap, deleted
graph links, and WeakRef retention of all four roots.

`tools/test262_async_generator_realm_admission.txt` freezes only the three
cross-Realm constructor/source fallback files. They pass **3/3**. With gates
lifted diagnostically, all 23 `built-ins/AsyncGeneratorFunction` files and all
48 `built-ins/AsyncGeneratorPrototype` files pass. The general
`AsyncIteratorPrototype[Symbol.asyncIterator]` surface passes separately;
`Symbol.asyncDispose` remains part of the unsupported explicit-resource-
management unit and is not admitted here. The supported subset remains
**12751 pass / 0 fail / 7687 skip / 20438 total**, Python tooling is **91/91**,
Rust generator tests are **76/76**, and builtins are **437/437**. Rust
all-target/all-feature tests, Clippy with warnings denied, formatting, release,
and wasm32 checks pass. Independent GPT and Umans reviews leave no valid high-
or medium-severity finding after code/spec triage.

Feature commit `7827093` passed CI `29480165633` and full matrix
`29480165138`. Against the synchronous Generator documentation baseline, 29 of
30 result artifacts at `/tmp/ruja-artifacts-async-generator-feature.mqSdSG`
are byte-for-byte identical. Only built-ins changes, by exactly **+2 pass / -2
skip**. The admitted language-expression file was already executed and passing
through the broad async-generator path. The aggregate is **29807 pass / 6346
fail / 12152 skip / 12 timeout / 0 error / 48317 total / 36153 pass-or-fail
executed**, or **61.7%** of all files and **82.4%** of executed files.

[Decision Log]
- 목적과 의도: install a complete asynchronous generator intrinsic graph in
  every Realm while preserving the distinct method Realm and generator Realm
  roles across queued Promise operations.
- 기존 구현 및 제약 조건: all async generator intrinsics were main-Realm
  singletons, dynamic/source/call fallbacks read those singletons, and delayed
  queue reactions allocated await Promises, native errors, and result objects
  from whichever execution context happened to be current.
- 검토한 주요 대안: clone main-Realm objects, switch only constructor
  fallbacks, store a new Realm field in every generator, or install four rooted
  intrinsics and derive the generator Realm from its already traced closure.
- 선택한 방식: install and root four identities per Realm, select source and
  dynamic prototypes through Realm maps, derive queue-internal Realm state from
  `LazyGeneratorData.closure`, and keep request capability creation in the
  borrowed native method Realm.
- 다른 대안 대신 이 방식을 선택한 이유: cloning preserves the wrong native
  `[[Realm]]`; constructor-only changes do not repair delayed queue allocation;
  and the traced closure already carries the authoritative Realm without
  duplicating mutable state on the generator object.
- 장점, 단점 및 영향: identity, prototype fallback, GC retention, Promise
  provenance, native errors, and completion records now follow Realm
  boundaries. Each Realm retains four additional direct roots. Async iterator
  helpers and async disposal were kept as explicit later units rather than
  being over-admitted with the Realm fix; disposal is handled immediately
  below.

## AsyncIterator async disposal

Every Realm's `%AsyncIteratorPrototype%` now owns a fresh
`[Symbol.asyncDispose]` native method. Calling or borrowing it creates the
result capability from the method Realm's rooted `%Promise%`, so replacing the
Realm's mutable global `Promise` does not affect the intrinsic operation. The
method has the standard `[Symbol.asyncDispose]` name, length zero, and
writable/non-enumerable/configurable property descriptor.

The implementation follows the observable algorithm order: create the
capability, dynamically get `return`, resolve immediately when it is nullish,
reject when it is non-callable or when the getter/call throws, otherwise call
it with an empty argument list. The returned value is assimilated
through intrinsic PromiseResolve. A Realm-local anonymous length-one unwrap
reaction converts fulfillment to `undefined`, while rejection passes through
unchanged to the result capability. Receiver, method, returned value,
capability, wrapper Promise, and reaction values are pinned or traced across
observable callbacks, thenable jobs, and forced GC.

`tools/test262_async_iterator_dispose_admission.txt` freezes the exact nine
`built-ins/AsyncIteratorPrototype/Symbol.asyncDispose` files. They pass
**9/9** in the normal runner; with all gates lifted, the complete 13-file
`built-ins/AsyncIteratorPrototype` diagnostic passes **13/13**. The supported
subset remains **12751 pass / 0 fail / 7687 skip / 20438 total**, Python
tooling is **92/92**, and Rust builtins are **438/438**. Rust
all-target/all-feature tests, Clippy with warnings denied, formatting, release,
and wasm32 checks pass. Umans reports no high- or medium-severity issue. GPT
identified an obsolete synthetic `undefined` argument and delayed rejection
for abrupt PromiseResolve; both are corrected to the current normative
algorithm. Its final re-review reports no remaining high- or medium-severity
issue.

Feature commit `d8c48fa` passed CI `29485564973` and full matrix
`29485565185`. Against the asynchronous Generator documentation baseline, 29
of 30 result artifacts at `/tmp/ruja-artifacts-async-dispose-feature.ds1DTt`
are byte-for-byte identical. Only built-ins changes, by exactly **+9 pass / -9
skip**. The aggregate is **29816 pass / 6346 fail / 12143 skip / 12 timeout / 0
error / 48317 total / 36162 pass-or-fail executed**, or **61.7%** of all files
and **82.5%** of executed files.

[Decision Log]
- 목적과 의도: implement the complete AsyncIterator disposal operation as a
  narrow, Realm-correct Promise workflow and close its nine known Test262
  failures without admitting unrelated async iterator helpers.
- 기존 구현 및 제약 조건: `%AsyncIteratorPrototype%` exposed only
  `@@asyncIterator`; synchronous iterator disposal could call `return` but did
  not provide the Promise capability, assimilation, and rejection semantics
  required by async disposal.
- 검토한 주요 대안: wrap the synchronous disposer in an async source
  function, call observable `Promise.resolve`/`then`, broadly enable explicit
  resource management, or implement the abstract operation with intrinsic
  capability and internal reaction records.
- 선택한 방식: install a per-Realm native method, use rooted Promise
  intrinsics and internal Promise handlers, materialize native errors in the
  method Realm, reject the original capability immediately when intrinsic
  PromiseResolve is abrupt, and freeze admission to the nine directly covered
  files.
- 다른 대안 대신 이 방식을 선택한 이유: source wrappers introduce parser
  and execution-context artifacts; observable global Promise methods are
  mutable; and broad feature admission would claim unrelated `using` syntax
  and disposal-stack semantics that this method does not implement.
- 장점, 단점 및 영향: method shape, Realm provenance, dynamic return lookup,
  thenable assimilation, abrupt rejection, and GC retention now match the
  specification. Each Realm gains one native method identity; async iterator
  helpers and broader explicit resource management remain separately bounded.

## Array.fromAsync

`Array.fromAsync` now selects an async iterator first, falls back through the
specification's Async-from-Sync wrapper, and otherwise reads an array-like
length. A traced continuation record carries the source, iterator record,
mapper, `thisArg`, result object, index, Promise capability, pending await
kind, and close completion across Promise jobs. The operation therefore
returns its result Promise before iterator, element, mapper, or thenable
failures settle it, while errors that precede capability creation retain their
specified synchronous behavior.

Each await boundary uses intrinsic Promise machinery from the native method
Realm. Async-from-Sync `next` is called with zero arguments, reads `value` even
when `done` is true, and creates iterator-result objects in the wrapper
method's Realm. A direct `next` throw rejects without closing; rejection while
awaiting a yielded sync-iterator value closes synchronously and preserves the
original throw completion. Mapping and result-property failures use
`AsyncIteratorClose` with the required original-completion precedence. The
constructor is invoked before `next` callability is validated, and ordinary
Array creation uses sparse length representation instead of imposing a
non-standard semantic element cap.

Observable values are pinned at native entry points and represented in GC
traces while suspended. This covers primitive array-like wrappers, iterator
and mapper callbacks, thenables, close reasons, PromiseResolve abrupt reasons,
and queued continuations. Native `[[Set]]` and `CreateDataProperty` paths now
invalidate matching inline-cache entries, preventing a cached array length or
element from surviving continuation writes. The same audit exposed and fixed
`Array.prototype.splice(start)`, whose omitted `deleteCount` must delete
through the array tail.

`tools/test262_array_from_async_admission.txt` freezes the complete current
95-file `built-ins/Array/fromAsync` corpus. It passes **95/95** with no skip.
Combined diagnostics for `Array.fromAsync`, `for-await-of`, and
`AsyncIteratorPrototype` are **127 pass / 0 fail / 1215 skip / 1342 total**.
The supported subset remains **12751 pass / 0 fail / 7687 skip / 20438
total**, Python tooling is **92/92**, Rust builtins are **446/446**, and Rust
all-target/all-feature tests, Clippy with warnings denied, formatting, release,
and wasm32 checks pass. Independent GPT and Umans final reviews found no
remaining high- or medium-severity issue after the Promise timing, close
provenance, Realm allocation, cache invalidation, and GC-rooting findings were
corrected.

Feature commit `1a48969` passed CI `29493228430` and full matrix
`29493228431`. Against the AsyncIterator-disposal documentation baseline, 29
of 30 result artifacts at
`/tmp/ruja-artifacts-array-from-async-feature.Zw21Ge` are byte-for-byte
identical. Only built-ins changes. Within the 95-file cohort, 91 skipped files
and four previously executing failures all move to pass: **+95 pass / -4 fail
/ -91 skip**. `splice/called_with_one_argument.js`,
`splice/target-array-with-non-writable-property.js`, and Function
`S15.3_A3_T5.js`/`S15.3_A3_T6.js` also move from fail to pass through the
shared splice and native-property cache fixes. The complete built-ins delta is
therefore **+99 pass / -8 fail / -91 skip**. The normalized aggregate is
**29915 pass / 6338 fail / 12052 skip / 12 timeout / 0 error / 48317 total /
36253 pass-or-fail executed**, or **61.9%** of all files and **82.5%** of
executed files. Raw artifacts retain the baseline's 150 extra unsupported
built-ins skips and report 48467 total files.

[Decision Log]
- 목적과 의도: implement the complete current `Array.fromAsync` surface as a
  specification-ordered async operation and use it to harden shared Promise,
  Async-from-Sync, iterator-close, Realm, cache, and GC machinery.
- 기존 구현 및 제약 조건: `Array.fromAsync` was absent, while the VM already
  had Promise jobs and `for await` support whose helper paths did not preserve
  every observable job boundary, completion provenance, Realm, or suspended
  root required by this algorithm.
- 검토한 주요 대안: translate the operation into an async source wrapper;
  collect synchronously and wrap only the final array; reuse `for await`
  bytecode directly; or model each specification await and close transition
  with a native continuation record.
- 선택한 방식: install a Realm-local native method, create the result
  capability before async-body work, represent every await/close stage in a
  traced continuation, and share corrected Realm-explicit Async-from-Sync
  primitives with the existing async runtime.
- 다른 대안 대신 이 방식을 선택한 이유: source wrappers add parser and
  execution-context artifacts, synchronous collection collapses observable
  jobs, and direct bytecode reuse cannot preserve the distinct direct-throw,
  yielded-rejection, mapping, property-creation, and close completion rules.
- 장점, 단점 및 영향: the complete current Test262 corpus is admitted and
  shared async iteration behavior is more specification-correct. The VM gains
  additional continuation variants and explicit Realm/root plumbing; async
  iterator helper methods remain a separate unsupported family rather than
  being claimed through this consumer.

## Math branding and Realm installation

Math now owns the standard `Symbol.toStringTag` data property with value
`"Math"` and attributes non-writable, non-enumerable, and configurable.
`Object.prototype.toString.call(Math)` and borrowed string operations therefore
observe `[object Math]`; deleting the configurable property deliberately falls
back to `[object Object]`. The implementation does not add Math to
`Object.prototype.toString`'s internal-slot classification because the
specification derives this brand through the observable well-known-symbol
property.

The Math builder now accepts an explicit global environment and
`%Object.prototype%`. Every native method is created in that environment and
inherits its Realm's `%Function.prototype%`; the Math object inherits the
matching `%Object.prototype%`. Test262-created Realms install their own Math
object after those intrinsic prototypes are established, rather than sharing
or omitting the main-Realm object.

The two direct branding files,
`built-ins/Math/Symbol.toStringTag.js` and
`built-ins/String/prototype/split/instance-is-math.js`, both pass. Complete
diagnostics are **285 pass / 0 fail / 42 skip / 327 total** for Math and
**1136 pass / 0 fail / 87 skip / 1223 total** for String. The supported subset
remains **12751 pass / 0 fail / 7687 skip / 20438 total**, Python tooling is
**92/92**, and Rust builtins are **447/447**. Rust all-target/all-feature tests,
Clippy with warnings denied, formatting, release, and wasm32 checks pass. GPT
and Umans independently found no remaining high- or medium-severity issue in
the descriptor, conversion, bootstrap, Realm, or GC boundary.

Feature commit `01a65a1` passed CI `29497747578` and full matrix
`29497747662`. Against the Array.fromAsync documentation baseline, 29 of 30
result artifacts at `/tmp/ruja-artifacts-math-feature.6cmV4u` are
byte-for-byte identical. Only built-ins changes: the two direct branding files
and existing
`built-ins/Array/prototype/every/15.4.4.16-1-10.js` and
`built-ins/Array/prototype/some/15.4.4.17-1-10.js` move from fail to pass, for
exactly **+4 pass / -4 fail**. The normalized aggregate is **29919 pass / 6334
fail / 12052 skip / 12 timeout / 0 error / 48317 total / 36253 pass-or-fail
executed**, or **61.9%** of all files and **82.5%** of executed files. Raw
artifacts retain the baseline's 150 extra unsupported built-ins skips and
report 48467 total files.

[Decision Log]
- 목적과 의도: close the final executing Math and String failures while
  making the Math intrinsic follow its observable brand and Realm ownership.
- 기존 구현 및 제약 조건: the main Math object had only an internal debug
  class name, no `@@toStringTag`, and Test262-created Realms did not install a
  Math object. `Object.prototype.toString` correctly ignored the debug class
  for ordinary-object builtin-tag classification.
- 검토한 주요 대안: classify `class_name == "Math"` as an internal Math
  builtin tag; add only the main-Realm symbol property; share the main Math
  object with child Realms; or parameterize the builder and install fresh
  Realm-local objects and functions.
- 선택한 방식: define the standard configurable own symbol property and
  construct Math with explicit Realm environment and object prototype in both
  main and created Realms.
- 다른 대안 대신 이 방식을 선택한 이유: a class-name special case would
  incorrectly preserve `[object Math]` after deleting the observable tag;
  main-only or shared installation would leave missing or foreign intrinsic
  identities in created Realms.
- 장점, 단점 및 영향: Math and String's current executing corpora have no
  failures, tag mutation follows specification fallback, and created Realms
  own their method identities. Realm bootstrap allocates one additional Math
  object and its native method set per created Realm.

## Async-from-Sync iterator completion

The internal Async-from-Sync adapter now implements `next`, `return`, and
`throw` through one Realm-explicit Promise capability path. It reads `done`
before `value`, applies intrinsic PromiseResolve to the value, and creates the
iterator-result object only from the resulting reaction. A rejected unfinished
`next` or `throw` value performs IteratorClose but preserves the original
rejection over catchable close failures; `return` deliberately disables that
second close. A missing sync `throw` instead closes with a normal completion,
so close failures retain their required precedence over the generated
TypeError.

Async-generator `yield*` no longer calls the embedding-only `await_value`
helper or drains the global microtask queue. `YieldDelegateAsync` stores a
small persisted resume phase, suspends the generator on the adapter method's
Promise, and resumes with a distinct fulfilled-result, rejected-result, or
missing-throw completion. Fulfilled delegated values are rewrapped in the
generator Realm and settle the active async-generator request without an
extra forwarded-result job. This preserves Promise FIFO ordering while
keeping adapter rejection, iterator, cached methods, request capabilities,
and Realm roots live across observable GC.

`tools/test262_async_from_sync_iterator_admission.txt` freezes all 38 current
`built-ins/AsyncFromSyncIteratorPrototype` files. The normal runner passes
**38/38** with no skips. The supported subset remains **12751 pass / 0 fail /
7687 skip / 20438 total**, Python tooling is **93/93**, and the focused async
iterator plus generator suites are **91/91**. Rust all-target/all-feature
tests, Clippy with warnings denied, formatting, release, and wasm32 checks
pass. Independent GPT and Umans reviews reproduced the original **31/7**
baseline; the final GPT review found no remaining high- or medium-severity
issue in suspension, close precedence, Realm selection, or GC tracing.

Feature commit `a257066` passed CI `29503902319` and full matrix
`29503902678`. Against the Math documentation baseline, 29 of 30 result
artifacts at `/tmp/ruja-artifacts-async-from-sync-feature.ykDk8Q` are
byte-for-byte identical. Only built-ins changes, by exactly **+38 pass / -38
skip**. The aggregate is **29957 pass / 6334 fail / 12014 skip / 12 timeout /
0 error / 48317 total / 36291 pass-or-fail executed**, or **62.0%** of all
files and **82.5%** of executed files.

[Decision Log]
- 목적과 의도: complete the shared Async-from-Sync adapter and make async
  generator delegation obey Promise job, close, Realm, and GC semantics.
- 기존 구현 및 제약 조건: `for await` and `Array.fromAsync` already used an
  internal continuation, but generator-backed wrappers were not recognized
  consistently and async `yield*` synchronously called `await_value`, which
  collapsed job ordering and bypassed PromiseResolve/close-on-rejection.
- 검토한 주요 대안: keep the synchronous helper and add close handling;
  duplicate value-unwrapping logic in `YieldDelegateAsync`; expose a synthetic
  JavaScript adapter object; or suspend on the existing internal adapter
  Promise and persist only the delegated resume phase.
- 선택한 방식: share a Realm-explicit `next`/`return`/`throw` adapter path,
  attach the existing AsyncFromSync continuation, and route its Promise through
  the async-generator queue with dedicated internal resume completions.
- 다른 대안 대신 이 방식을 선택한 이유: close handling around a synchronous
  drain still violates observable FIFO ordering; duplicated bytecode logic
  would diverge from `for await` and `Array.fromAsync`; and a synthetic public
  object would add observable identity and prototype behavior that the VM does
  not otherwise need.
- 장점, 단점 및 영향: all current prototype tests are admitted, original
  rejection identity and close precedence are shared across consumers, and
  generator Realm/GC behavior is explicit. The generator frame gains one
  compact internal phase byte and additional resume variants; async iterator
  helpers and absent-argument tracking for broader delegation remain separate
  follow-up surfaces.

## Promise combinator IteratorClose on abrupt completion

`Promise.all`, `Promise.allSettled`, `Promise.any`, and `Promise.race` now
perform `IteratorClose` after an input iterator has produced a value when the
receiver's `resolve` call, the resulting value's `then` lookup, or the `then`
call completes abruptly. Iterator-step and iterator-value abrupt completions
remain on the no-close path because those operations set the iterator record's
done state before returning the error.

The shared close-and-reject path retains the original rejection reason and
Promise capability while the iterator's observable `return` getter and method
run. A catchable close failure does not replace the original throw completion,
matching `IteratorClose`'s throw-completion precedence; a non-catchable host
abort still propagates. The iterator wrapper is rooted for the whole
combinator loop, and the iterator, capability functions, result Promise, and
original reason are additionally rooted across close callbacks and forced GC.

`tools/test262_promise_combinator_close_admission.txt` freezes exactly three
files for each of the four combinators, for **12/12** passing files with no
skip. A forced adjacent `*close*.js` diagnostic passes **24/24**, including
the iterator-step/value no-close cases. The supported subset remains **12751
pass / 0 fail / 7687 skip / 20438 total**, Python tooling is **94/94**, and
Rust builtins are **448/448**. Rust all-target/all-feature tests, Clippy with
warnings denied, formatting, release, and wasm32 checks pass. GPT found no
high- or medium-severity issue. Umans initially questioned close-error
precedence and the fetched return method's GC lifetime, then withdrew both
findings after rechecking current Test262, ECMA-262, and `Vm::call_function`'s
callee pinning.

Feature commit `d0352df` passed CI `29508890153` and full matrix
`29508890137`. Against the Async-from-Sync feature baseline, 29 of 30 result
artifacts at `/tmp/ruja-artifacts-promise-close-feature.gvh4gZ` are
byte-for-byte identical. Only built-ins changes, by exactly **+12 pass / -12
skip**. The aggregate is **29969 pass / 6334 fail / 12002 skip / 12 timeout /
0 error / 48317 total / 36303 pass-or-fail executed**, or **62.0%** of all
files and **82.6%** of executed files.

[Decision Log]
- 목적과 의도: close the bounded Promise-combinator IteratorClose defect
  without claiming the much broader remaining Promise iterable corpus.
- 기존 구현 및 제약 조건: all four combinators rejected directly when
  receiver `resolve`, `then` lookup, or `then` invocation threw, so the active
  iterator's `return` hook was skipped. Iterator-step/value failures already
  followed the required no-close behavior and had to remain unchanged.
- 검토한 주요 대안: close on every loop error; duplicate close/reject code in
  each combinator; replace the original rejection with a close error; or share
  one helper for only the three specification-defined abrupt points.
- 선택한 방식: root each iterator for the combinator lifetime and route the
  three post-step abrupt paths through a shared helper that performs close,
  preserves the original catchable throw completion, and rejects the existing
  capability.
- 다른 대안 대신 이 방식을 선택한 이유: broad closing would regress the
  iterator-record done rules, duplicated implementations would drift across
  four algorithms, and close-error replacement contradicts current
  `IteratorClose` precedence when the incoming completion is already a throw.
- 장점, 단점 및 영향: all four combinators now share one audited close path,
  observable cleanup runs exactly once, and rejection identity survives GC.
  The admission remains an exact 12-file boundary; broader Promise combinator
  conformance and host-abort cleanup remain separately scoped work.

## Promise combinator setup rejection objects

Promise setup failures now follow the same JavaScript-visible completion path
as ordinary thrown values. A catchable native `TypeError` without an existing
`thrown_value` is materialized as an Error object in the active Promise
method's Realm; an explicit object or Symbol throw retains identity, while a
non-catchable fuel abort propagates to the host instead of becoming a Promise
rejection. Capability functions, the result Promise, and each materialized
reason remain pinned across custom reject callbacks and forced GC.

A shared GetPromiseResolve helper now performs both the observable `resolve`
lookup and callability validation. After NewPromiseCapability succeeds,
`Promise.all`, `Promise.allSettled`, `Promise.any`, and `Promise.race` reject
that capability for either abrupt operation and return its Promise without
touching the iterable. The same helper keeps the keyed variants consistent.
`Promise.race` no longer lets those post-capability failures escape
synchronously, and Promise element callbacks plus `Promise.try` use the same
Error-object conversion instead of a message string.

`tools/test262_promise_combinator_rejection_admission.txt` freezes the exact 95
files that failed before this change. They pass **95/95** with no skip. The
normal runner across `all`, `allSettled`, `any`, and `race` is **229 pass / 0
fail / 161 skip / 390 total**; the complete `built-ins/Promise` diagnostic is
**363/0/340/703**. With all gates lifted, the four combinator directories move
from **294/96** to **389/1**; the sole remaining failure is the independently
scoped `allSettled/resolved-then-catch-finally.js` Promise-finally defect. The
supported subset remains **12751/0/7687/20438**, Python tooling is **95/95**,
and Rust builtins are **449/449**. All-target/all-feature Rust tests, Clippy
with warnings denied, formatting, release, and wasm32 checks pass. Final GPT
and Umans reviews found no remaining high- or medium-severity issue after
live metadata, Realm, Fuel, and pin-lifetime findings were checked.

Feature commit `fa21315` passed CI `29515343282` and full matrix
`29515343238`. Against the preceding Promise-close feature baseline, 29 of 30
result artifacts at
`/tmp/ruja-artifacts-promise-rejection-feature.ScWfwN` are byte-for-byte
identical. Only built-ins changes, by exactly **+95 pass / -95 skip**. The
aggregate is **30064 pass / 6334 fail / 11907 skip / 12 timeout / 0 error /
48317 total / 36398 pass-or-fail executed**, or **62.2%** of all files and
**82.6%** of executed files.

[Decision Log]
- 목적과 의도: make all audited Promise-combinator setup abrupt completions
  reject with specification-shaped Error values and close the largest shared
  failure cluster without admitting unrelated Promise-finally behavior.
- 기존 구현 및 제약 조건: `promise_rejection_value` converted native errors
  to message strings; three combinators manually rejected non-callable
  `resolve` values as strings; and `race` propagated resolve lookup and
  callability failures synchronously even though its capability already
  existed.
- 검토한 주요 대안: special-case each of the 95 tests; create Error objects in
  every combinator branch; materialize all errors globally across async and
  generator runtimes; or repair the shared synchronous Promise rejection and
  GetPromiseResolve boundaries first.
- 선택한 방식: preserve existing thrown values, materialize only catchable
  native errors in the current method Realm, propagate host aborts, share
  GetPromiseResolve across the Promise static methods, and freeze admission to
  the exact 95 files that exercise this behavior.
- 다른 대안 대신 이 방식을 선택한 이유: per-test branches would encode the
  corpus rather than the specification; duplicated Error creation would drift
  across combinators; and a repository-wide async error conversion changes
  captured-Realm and suspension contracts that require their own audit.
- 장점, 단점 및 영향: rejection reasons now have the correct object identity,
  prototype Realm, ordering, and GC lifetime, and all four combinators share
  one setup path. Promise-finally and pre-existing async-runtime string/Fuel
  fallback sites remain explicit follow-up units rather than hidden scope.

## Promise finally and reaction completion

`Promise.prototype.finally` now follows the complete wrapper algorithm. It
validates that the receiver is an object, obtains `C` through
`SpeciesConstructor`, and creates distinct anonymous, non-constructible
`ThenFinally` and `CatchFinally` built-ins with length 1 in the method Realm.
Each wrapper invokes `onFinally` with no arguments, performs the abstract
`PromiseResolve(C, result)` operation without reading `C.resolve`, and invokes
the resulting promise's observable `then` with one anonymous length-0 value
thunk or thrower. Ordinary cleanup results therefore preserve the original
settlement, while a thrown value or rejected cleanup promise replaces it.

The shared Promise reaction path now normalizes non-callable fulfillment and
rejection handlers to pass-through behavior. A successful reaction always
calls the derived capability's resolve function instead of directly adopting
a native Promise's internal state. This preserves the required thenable job
boundary, observes overridden `then` methods, and delegates self-resolution to
the resolving function. `Promise.resolve` validates that its receiver is a
constructor before checking whether an input Promise has the same constructor,
so a primitive receiver cannot escape through the fast path. Bound native
closure receivers are rooted before their target allocation, and the finally
state, constructor, callback, original completion, intermediate promise, and
all four anonymous functions remain rooted across observable getters, calls,
and forced GC.

`tools/test262_promise_finally_admission.txt` freezes 37 exact files: all 29
current `Promise.prototype.finally` files, six adjacent
`Promise.prototype.then` reaction files, the allSettled/finally integration
file, and the Promise.resolve receiver-ordering file. Runner and analyzer pass
**37/37**. Complete forced diagnostics are **29/29** for `finally`, **75/75**
for `then`, **30/30** for `resolve`, and **390/390** across `all`,
`allSettled`, `any`, and `race`. Broad `built-ins/Promise` is **387 pass / 0
fail / 316 skip / 703 total** under the normal boundary and **698/5** with all
gates lifted. The supported subset remains **12751/0/7687/20438**, Python
tooling is **96/96**, and Rust builtins are **452/452**. All-target/all-feature
tests, Clippy with warnings denied, formatting, release, and wasm32 checks
pass. GPT found no high- or medium-severity issue in the final diff; the
independent Umans reconnaissance identified the pre-fix wrapper, rooting, and
Realm requirements that the implementation and forced-GC regressions cover.

Feature commit `0581788` passed CI `29549608490` and full matrix
`29549608468`. Against the Promise setup-rejection feature baseline, 29 of 30
result artifacts at `/tmp/ruja-artifacts-promise-finally-feature.klMTHm` are
byte-for-byte identical. Only built-ins changes, by exactly **+24 pass / -24
skip**. The aggregate is **30088 pass / 6334 fail / 11883 skip / 12 timeout /
0 error / 48317 total / 36422 pass-or-fail executed**, or **62.3%** of all
files and **82.6%** of executed files.

[Decision Log]
- 목적과 의도: complete Promise finally without hiding its observable
  species, PromiseResolve, handler metadata, and job-ordering requirements
  behind an internal shortcut.
- 기존 구현 및 제약 조건: `finally` passed the same callback directly to both
  `then` branches; reactions treated non-callable values as functions and
  directly adopted native Promise state, skipping the capability resolver,
  overridden `then`, one job boundary, and one subclass construction.
- 검토한 주요 대안: patch only the 12 failing finally files; preserve direct
  adoption and special-case finally wrappers; synthesize interpreted
  JavaScript closures; add a new microtask type; or use Realm-explicit bound
  native closures plus the existing resolving functions and queue.
- 선택한 방식: factor abstract PromiseResolve by constructor, implement all
  four anonymous closures with bound native state, normalize handlers at
  PerformPromiseThen, and route every normal reaction result through the
  capability resolve function.
- 다른 대안 대신 이 방식을 선택한 이유: test-specific branches would encode
  corpus accidents, direct adoption cannot reproduce observable `then` and
  constructor counts, interpreted wrappers add parser/runtime dependencies,
  and the existing resolving functions already implement self-resolution and
  thenable jobs.
- 장점, 단점 및 영향: the full Promise corpus is now **698/703** with every
  gate lifted, all finally behavior is Realm- and GC-stable, and no new job
  representation is needed. Native Promise returns now take the required
  resolving-function path and allocate/queue accordingly; the five remaining
  Promise failures and the older async-runtime string/Fuel fallback are
  separate bounded follow-ups.

## Promise constructor ordering and allocation roots

Promise construction now follows the required observable order: require
construction, validate that the executor is callable, read
`NewTarget.prototype`, allocate the Promise, create resolving functions, and
finally invoke the executor. `Promise` is classified as an internally
allocating native constructor, so generic native dispatch no longer performs
an ordinary-object allocation and prototype read before `promise_constructor`
can reject an omitted or non-callable executor. A callable executor still
observes exactly one prototype read, preserves an abrupt getter value by
identity, and is not invoked when that read fails. TypeErrors come from the
target Promise Realm, while a non-object prototype still falls back to the
NewTarget Realm's `%Promise.prototype%`.

The observable prototype is pinned before the collecting Promise allocation,
and the fresh Promise is pinned before resolving-state allocation. A heap-cap
regression first reproduced the old slot reuse, where collection replaced the
new Promise with the resolving-state object, and now verifies that the returned
value remains a Promise. `tools/test262_promise_constructor_order_admission.txt`
freezes only
`built-ins/Promise/get-prototype-abrupt-executor-not-callable.js` with its
exact `Reflect` and `Reflect.construct` feature gates; runner and analyzer pass
**1/1** without admitting the adjacent callable-executor case.

Broad `built-ins/Promise` is **388 pass / 0 fail / 315 skip / 703 total** under
the normal boundary and **699/4/0/703** with every gate lifted. The remaining
four failures are the paired `allKeyed` and `allSettledKeyed` Proxy
`[[GetOwnProperty]]` cases. The supported subset remains
**12751/0/7687/20438**, Python tooling is **97/97**, and Rust builtins are
**452/452**. All-target/all-feature tests and builds, Clippy with warnings
denied, formatting, release, and wasm32 checks pass. GPT and Umans found no
high- or medium-severity issue introduced by the final diff.

Feature commit `568171c` and CI-environment follow-up `c224613` passed CI
`29552437436` and full matrix `29552437437`. Against the Promise-finally
baseline, 29 of 30 result artifacts at
`/tmp/ruja-artifacts-promise-constructor-order-feature.KVXx1G` are
byte-for-byte identical. Only built-ins changes, by exactly **+1 pass / -1
skip**. The aggregate is **30089 pass / 6334 fail / 11882 skip / 12 timeout /
0 error / 48317 total / 36423 pass-or-fail executed**, or **62.3%** of all
files and **82.6%** of executed files.

[Decision Log]
- 목적과 의도: enforce Promise constructor ordering without moving unrelated
  Promise or generic construction behavior into the same change, and prevent
  allocation-time collection from invalidating the new Promise.
- 기존 구현 및 제약 조건: generic native construction observed
  `NewTarget.prototype` before entering `promise_constructor`; the constructor
  then allocated a Promise outside `Vm::alloc` and held it only in a Rust local
  while allocating resolving state.
- 검토한 주요 대안: special-case the single Test262 file; reorder validation
  in generic construction; precompute and cache the prototype; classify Promise
  with the existing internal-allocation path; or redesign native constructor
  metadata before fixing the observable bug.
- 선택한 방식: route Promise through the established internally allocating
  native path, preserve its existing local algorithm order, use collecting
  `Vm::alloc`, and pin the prototype and Promise at their exact allocation
  boundaries.
- 다른 대안 대신 이 방식을 선택한 이유: generic reordering would break
  ordinary constructors, caching would hide observable access, a test-specific
  branch would encode the corpus, and constructor metadata redesign is broader
  than this one-file semantic repair.
- 장점, 단점 및 영향: the exact ordering case and allocation-time GC failure
  are fixed with no Promise-corpus regression. Name-based internal-constructor
  classification remains an architectural follow-up; pre-existing Reflect
  argument rooting and native-to-interpreted Realm tracking are recorded as
  separate units rather than hidden in this patch.

## Reflect call argument materialization and GC ownership

`Reflect.apply` and `Reflect.construct` now preserve the full
`CreateListFromArrayLike` lifetime contract. The observable `length` value is
pinned while `ToLength` can invoke conversion hooks. Each indexed `Get` result
is pinned immediately before the next getter can re-enter JavaScript, and the
caller retains all of those roots through the final target call or
construction. Indexed-get, length-conversion, target-call, and constructor
errors all release exactly the roots owned by this materialization.

Previously, earlier getter results lived only in a Rust `Vec<Value>`, which is
not scanned by the collector. A later getter that forced GC could reclaim and
reuse those handles before the call began. The deterministic regression
changed the first object into the second object and could turn an ephemeral
Promise executor into a non-callable value. Tests now force GC during length
coercion, later index getters, nested and Proxy calls, explicit
`NewTarget.prototype` lookup after materialization, returned and thrown value
transfer, target errors, and Promise construction. A unit-level pin-stack test
also covers successful materialization and every abrupt cleanup boundary.

`tools/test262_reflect_call_admission.txt` freezes all 19 current direct files:
nine under `built-ins/Reflect/apply` and ten under
`built-ins/Reflect/construct`. Runner and analyzer subtract only each file's
audited metadata features; unknown future Reflect files and files with added
feature gates remain skipped. The exact boundary and the same boundary with
all gates forcibly lifted are both **19/19**. Broad Promise remains **388 pass
/ 0 fail / 315 skip / 703 total** normally and **699/4/0/703** fully opened.
The supported subset remains **12751/0/7687/20438**, Python tooling is
**98/98**, and Rust builtins are **453/453**. All-target/all-feature tests and
builds, Clippy with warnings denied, formatting, release, and wasm32 checks
pass. Independent GPT and Umans reviews found no implementation defect or
admission leak; the requested `ToLength`, post-materialization NewTarget,
success-cleanup, return, and throw coverage was added before the final run.

Feature commit `be24904` passed CI `29555440736` and full matrix
`29555440756`. Against
`/tmp/ruja-artifacts-promise-constructor-order-feature.KVXx1G`, 29 of 30
result artifacts at `/tmp/ruja-artifacts-reflect-call-feature.dh7hH0` are
byte-for-byte identical. Only built-ins changes, by exactly **+19 pass / -19
skip**. The aggregate is **30108 pass / 6334 fail / 11863 skip / 12 timeout /
0 error / 48317 total / 36442 pass-or-fail executed**, or **62.3%** of all
files and **82.6%** of executed files.

The existing 1,048,576-argument materialization cap remains an explicit
sandbox policy and therefore differs from the specification's full `ToLength`
range for enormous values. The shared operation now checks that cap after
`ToLength` truncation, so a fractional value that truncates to the limit is
accepted while a resulting integer length above the limit fails before any
indexed `Get`.

[Decision Log]
- 목적과 의도: prevent re-entrant GC from invalidating values already produced
  by `CreateListFromArrayLike`, while admitting only the audited direct
  Reflect call surface.
- 기존 구현 및 제약 조건: a Rust `Vec<Value>` was not part of the GC root set;
  `call_function` and construction pinned arguments only after all observable
  getters had already run; and every abrupt path had to restore the stack-like
  `gc_pins` discipline.
- 검토한 주요 대안: pin the complete vector only after materialization; rely
  on call dispatch to pin arguments; copy values into a temporary JS Array;
  add a general Rust-container root scanner or RAII guard; or transfer each
  successful getter result into `gc_pins` immediately and return its ownership
  count to the caller.
- 선택한 방식: pin `length` across conversion, pin each indexed result at its
  first stable boundary, return `(Vec<Value>, pin_count)`, clean partial lists
  inside the helper, and have each caller capture its final result before
  releasing all list roots.
- 다른 대안 대신 이 방식을 선택한 이유: post-materialization and dispatch
  pinning are too late, a temporary JS Array adds observable allocation and
  prototype concerns, and a general root-container or RAII redesign is broader
  than the two shared Reflect call sites.
- 장점, 단점 및 영향: getter order and error precedence remain unchanged,
  every materialized value survives arbitrary re-entry through the target
  operation, and pin ownership is directly testable. The manual pin-count
  protocol remains a local discipline and the resource cap remains
  intentionally non-spec for huge lists.

## Function.prototype.apply observable argument materialization

`Function.prototype.apply` now uses the same shared
`CreateListFromArrayLike` operation as both Reflect call builtins. Callable
validation still happens first. An omitted, `null`, or `undefined` argument
list takes the specified empty-list path; every other value must be an object.
The shared operation performs observable `Get("length")`, `ToLength`, and
ascending indexed `Get` calls, so accessors, inherited values, Proxy traps,
array holes, and TypedArray elements are no longer bypassed by an Array clone
or own-data-property shortcut. There is deliberately no real-Array fast path:
Array index access can itself be observable, and splitting the algorithms was
the source of the semantic drift.

The observed length is rooted across coercion, every indexed value is rooted
before the next JavaScript re-entry, and the caller owns all resulting pins
through the target invocation. Helper, indexed-get, and target-call errors
restore the original pin depth while returned and thrown argument values retain
their identity. The 1,048,576-element sandbox cap is evaluated after
`ToLength`; this also fixes the shared Reflect boundary where `1048576.5`
previously threw instead of truncating to the accepted limit.

`tools/test262_function_apply_admission.txt` admits only the two previously
feature-gated direct files,
`built-ins/Function/prototype/apply/not-a-constructor.js` and
`built-ins/Function/prototype/apply/resizable-buffer.js`. The complete direct
directory is now **48/48**, up from **42 pass / 4 fail / 2 skip**. Direct
Reflect remains **19/19**, the supported subset remains
**12751/0/7687/20438**, Python tooling is **99/99**, and Rust builtins are
**454/454**. All-target/all-feature tests and builds, Clippy with warnings
denied, formatting, release, and wasm32 checks pass. Independent GPT and Umans
reviews found no implementation defect; explicit omitted/nullish and abrupt
`ToLength` regressions requested during review were added before the final run.

Feature commit `ffea75a` passed CI `29558468870` and full matrix
`29558468852`. Against `/tmp/ruja-artifacts-reflect-call-feature.dh7hH0`, 29
of 30 result artifacts at
`/tmp/ruja-artifacts-function-apply-feature.wpT8MO` are byte-for-byte
identical. Only built-ins changes, by exactly **+6 pass / -4 fail / -2 skip**.
The aggregate is **30114 pass / 6330 fail / 11861 skip / 12 timeout / 0 error /
48317 total / 36444 pass-or-fail executed**, or **62.3%** of all files and
**82.6%** of executed files.

[Decision Log]
- 목적과 의도: make `Function.prototype.apply` follow its observable
  array-like argument algorithm while preserving every argument across
  re-entrant garbage collection.
- 기존 구현 및 제약 조건: the legacy path cloned real Arrays or inspected own
  data properties directly, skipped inherited/accessor/Proxy behavior, and
  held intermediate heap values only in a Rust vector. Reflect had a corrected
  pin protocol, but keeping two materializers allowed ordering and cap behavior
  to diverge again.
- 검토한 주요 대안: repair only the four failing Test262 files; retain an
  Array fast path and patch accessors; convert inputs into a temporary JS
  Array; keep separate Apply and Reflect helpers; or move the complete abstract
  operation and pin contract into one builtin module.
- 선택한 방식: preserve Apply's callable and nullish front-end rules, then use
  one object-only shared materializer whose return value carries both the Rust
  argument vector and its owned pin count into the target call.
- 다른 대안 대신 이 방식을 선택한 이유: file-specific fixes would encode
  corpus accidents, even ordinary Arrays can expose indexed access, a
  temporary JS Array adds allocation and prototype semantics, and duplicated
  helpers had already drifted in both observability and cap ordering.
- 장점, 단점 및 영향: Apply and Reflect now share getter order, error
  precedence, cap semantics, and GC ownership; all 48 direct Apply files pass
  without widening unrelated gates. The manual LIFO pin-count contract and the
  non-spec sandbox cap remain explicit local constraints. Execution-context
  Realm tracking and the four keyed Promise failures stay separate follow-ups.

## Promise keyed descriptor ordering and admission

`Promise.allKeyed` and `Promise.allSettledKeyed` now follow the observable
ordering in the
[Await Dictionary draft](https://tc39.es/proposal-await-dictionary/). The
implementation snapshots raw `[[OwnPropertyKeys]]` once, then performs
Proxy-aware `[[GetOwnProperty]]` separately for each key. Only an enumerable
descriptor advances the accepted-entry index and proceeds through `Get`,
`C.resolve`, callback creation, `then` lookup, and `then` invocation. Missing
or non-enumerable descriptors are skipped without reading the value or
allocating result state, so accepted entries remain compact. This separation
also preserves the required trap order when a Proxy omits `ownKeys` and
delegates key enumeration to its target.

The property value is rooted through `C.resolve`; the returned promise,
per-entry record, callbacks, and observable `then` value remain rooted through
the final call. Key/state allocation and callback-creation failures after
object validation reject the existing capability, and the keyed entry paths
restore their original pin depth on normal and abrupt completion. Forced-GC
regressions cover `ownKeys`, descriptor lookup, property `Get`, `then` lookup,
and `then` invocation for both keyed methods. Ordering tests also cover
descriptor `undefined`, exact thrown-value identity, explicit and delegated
key enumeration, and compact middle-key skipping.

`tools/test262_promise_keyed_admission.txt` freezes all current 63 files under
`built-ins/Promise/allKeyed` and `built-ins/Promise/allSettledKeyed` at test262
`020cb74075849d1e404bbcdb62feb7a02e6966db`. Runner and analyzer remove only
the exact metadata features recorded for each manifest member; unknown future
files remain skipped even when they carry only the async flag. Both the normal
and all-gates-lifted exact runs are **63/63**. Broad Promise is **433 pass / 0
fail / 270 skip / 703 total** normally and **703/703** with every gate lifted.
The supported subset remains **12751/0/7687/20438**, Python tooling is
**100/100**, Rust lib/unit tests are **68/68**, and Rust builtins are
**457/457**. All-target/all-feature tests and builds, Clippy with warnings
denied, formatting, release, and wasm32 checks pass. Independent GPT and Umans
reviews found no semantic defect; an admission test was strengthened so future
paths stay skipped independently of the proposal feature gate.

Feature commit `3489f00` passed CI `29562059144` and full matrix
`29562059145`. Against
`/tmp/ruja-artifacts-function-apply-feature.wpT8MO`, 29 of 30 result artifacts
at `/tmp/ruja-artifacts-promise-keyed-feature.pqnizJ` are byte-for-byte
identical. Only built-ins changes, by exactly **+45 pass / -45 skip**. The
artifact and workflow aggregates agree at **30159 pass / 6330 fail / 11816
skip / 12 timeout / 0 error / 48317 total / 36489 pass-or-fail executed**, or
**62.4%** of all files and **82.7%** of executed files. Re-summing the retained
Promise-finally, Promise-constructor, Reflect-call, and Function.apply
artifacts also corrects their recently documented totals from an erroneous
**48467** to the actual **48317**; each mistaken skip count was high by 150.

[Decision Log]
- 목적과 의도: make keyed Promise combinators observe every Proxy descriptor
  at the specified per-key point while preserving accepted values across
  arbitrary re-entry and GC.
- 기존 구현 및 제약 조건: enumerable filtering was folded into the shared
  own-key helper. A Proxy without `ownKeys` delegated that filtering to its
  target, bypassing the original Proxy's descriptor trap, while explicit
  `ownKeys` still caused all descriptors to be read before any entry's
  resolve/then chain. Intermediate Rust locals were not GC roots.
- 검토한 주요 대안: special-case the four failing files; teach delegated
  `ownKeys` filtering to call back through the original Proxy; precompute
  descriptor/key pairs; reuse a temporary JavaScript object as state; or keep
  raw key enumeration separate from per-entry descriptor processing.
- 선택한 방식: snapshot unfiltered own keys, query the original input's
  descriptor inside the loop, append a key/value placeholder only after the
  descriptor is accepted, and use explicit LIFO pins through the observable
  resolve and `then` operations.
- 다른 대안 대신 이 방식을 선택한 이유: file-specific and delegation-only
  fixes would retain the wrong interleaving, precomputation is observably
  incorrect, and temporary user-visible storage adds unrelated allocation and
  prototype semantics. The two-stage algorithm directly matches the proposal
  and existing internal property operations.
- 장점, 단점 및 영향: the complete keyed surface and fully opened Promise
  corpus now pass without widening future admissions. The manual pin protocol
  remains local and review-sensitive; execution-context Realm tracking is the
  next separate architecture unit rather than being mixed into this semantic
  repair.

## Stack-ordered execution Realm tracking

Native-to-interpreted re-entry previously used three scalar VM fields for the
current native callee, `NewTarget`, and cached prototype. A native builtin
could overwrite or clear that state while invoking a callback from another
Realm, leaving pre-frame call setup and later generator or async resumption to
consult the outer caller, active frame, or VM global inconsistently. The
observable failures included wrong primitive prototypes, sloppy global writes
and boxed/nullish `this`, arguments/rest prototypes, and native TypeError
objects created after nested or abrupt callbacks.

The VM now records native and interpreted execution contexts in one LIFO
stack. Interpreted dispatch installs a context before class validation and
argument setup, and the interpreter installs a second frame-owned context
while bytecode executes or a suspended frame resumes. Native contexts carry
the callee, Realm environment, `NewTarget`, and any observable prototype value.
General Realm lookup and native-only metadata use only the top context, while
interpreted error lookup falls back from a top native method to the active
resumed frame. Error materialization happens before that frame context is
removed, and the collector traces every context-owned heap value.

Regressions exercise callbacks in both Realm directions, bound functions and
Proxies, primitive and global lookup, sloppy `this`, arguments/rest objects,
nested native errors, abrupt restoration, generator default-parameter setup,
borrowed generator `next`/`throw`/`return`, async functions and async
generators before and after `await`, and forced collection. An internal VM test
also requires an empty context stack after synchronous success, synchronous
failure, a later call, and async rejection queue draining.

This architecture-only unit changes no Test262 admission manifest or runner
gate. The supported subset remains **12751 pass / 0 fail / 7687 skip / 20438
total**, Python tooling is **100/100**, Rust lib/unit tests are **69/69**, and
Rust builtins are **457/457**. All-target/all-feature tests and builds, Clippy
with warnings denied, formatting, release, and wasm32 checks pass. Independent
GPT and Umans reviews found no high- or medium-severity defect after generator
prologue, borrowed abrupt resume, and async rejection cleanup coverage was
added. Feature commit `46fecef` passed CI `29567895773`.

Full matrix `29567895748` also passed. All 30 result artifacts at
`/tmp/ruja-artifacts-execution-context-feature.9m50B4` are byte-for-byte
identical to `/tmp/ruja-artifacts-promise-keyed-feature.pqnizJ`. The
artifact-derived aggregate is unchanged at **30159 pass / 6330 fail / 11816
skip / 12 timeout / 0 error / 48317 total / 36489 pass-or-fail executed**, or
**62.4%** of all files and **82.7%** of executed files.

[Decision Log]
- 목적과 의도: make the active ECMAScript execution context, rather than the
  outermost native caller or mutable VM global, determine callee Realm and
  construction metadata across every re-entry and resumption boundary.
- 기존 구현 및 제약 조건: three scalar native fields could represent only one
  active native call; bytecode frames did not exist during interpreted call
  setup and could be suspended and resumed under an unrelated native method;
  context-owned heap values also had to participate in manual GC rooting.
- 검토한 주요 대안: patch individual builtins with explicit Realm arguments;
  infer Realm only from the frame stack; maintain separate Realm, callee, and
  construction stacks; or represent native and interpreted calls in one typed
  execution-context stack.
- 선택한 방식: use one LIFO stack with typed native/interpreted entries, keep
  short setup and frame contexts for interpreted calls, restrict native
  metadata to a top native entry, materialize errors before scope cleanup, and
  trace every stored value from the VM root set.
- 다른 대안 대신 이 방식을 선택한 이유: call-site patches would leave new
  re-entry paths vulnerable, frames cannot cover pre-frame setup or all resume
  boundaries, and independent stacks can drift out of sync. One typed stack
  expresses observable nesting and gives cleanup and GC a single invariant.
- 장점, 단점 및 영향: cross-Realm callbacks and resumptions now preserve
  callee-owned intrinsics without widening Test262 support, and construction
  metadata cannot leak through interpreted calls. The scope helper restores
  all `Result` paths but is not designed to recover and reuse a VM after a
  caught Rust panic; module-instantiation error timing and name-based native
  constructor classification remain separate audit units.

## Promise job Realm and host-abort preservation

Deferred Promise work now carries the Realm selected by the operation that
created it. Dynamic import retains its initiating Realm, thenable jobs use the
callable `then` function Realm, and Promise reactions capture the selected
handler Realm when enqueued rather than when executed. The GC traces those
Realm environments together with every queued capability and continuation, so
forced collection cannot silently fall back to the main Realm.

A shared completion classifier preserves explicit thrown-value identity,
materializes catchable native errors in the operation Realm, and propagates
Fuel exhaustion as a non-catchable host abort. The same policy now covers
Promise executors and resolution, reaction handlers and capability settlement,
await setup, `Array.fromAsync`, Async-from-Sync iteration, async iterator
disposal, async generators, and dynamic import. Async host aborts restore pin,
frame, and stack ownership; module continuations cache only their own errored
record; async-generator drains are rooted, release queue ownership, preserve
siblings, and retry only terminal `next()` settlement so an already-advanced
generator body is never replayed. Function creation separately pins its fresh
`.prototype` through environment and closure allocation.

The final local release run is unchanged at **433 pass / 0 fail / 270 skip /
703 total** for `built-ins/Promise`, **620 pass / 0 fail / 384 skip / 1004
total** for dynamic import, and **12751 pass / 0 fail / 7687 skip / 20438
total** for the supported subset. Python tooling is **100/100**; Rust lib/unit,
builtins, modules, and Fuel tests are **77/77**, **458/458**, **31/31**, and
**24/24**. All-target/all-feature tests and builds, warnings-denied Clippy,
formatting, release, and wasm32 checks pass. Independent GPT and Umans reviews
both found the unsafe state-advanced async-generator replay and both returned
`CLEAN` after the terminal-only retry correction and hard-heap regressions.

Feature commit `d3698b3` passed CI `29577669220` and full matrix
`29577669208`. All 30 result artifacts at
`/tmp/ruja-artifacts-promise-jobs-feature.P2W5fL` are byte-for-byte identical
to `/tmp/ruja-artifacts-execution-context-feature.9m50B4`. The aggregate
therefore remains **30159 pass / 6330 fail / 11816 skip / 12 timeout / 0 error
/ 48317 total / 36489 pass-or-fail executed**, or **62.4%** of all files and
**82.7%** of executed files. No Test262 admission or runner gate changed.

[Decision Log]
- 목적과 의도: preserve the specification-owned Realm and completion class
  across deferred Promise and async boundaries without converting host resource
  aborts into observable JavaScript rejections.
- 기존 구현 및 제약 조건: jobs often selected a Realm only when they ran,
  several async paths stringified native errors or rejected Fuel, and abrupt
  await/module/generator paths could leave suspended frames, evaluating module
  records, or owned generator queues behind. Rust locals and `Arc<Error>` are
  not GC roots.
- 검토한 주요 대안: reject every error through the current Realm; use the VM
  global Realm for all generated errors; patch only dynamic import; retry every
  failed async-generator request; or carry Realm metadata and classify
  completions at each Promise boundary while explicitly unwinding state.
- 선택한 방식: store and trace operation Realms in jobs and continuations,
  use one catchable/thrown/host-abort classifier, pin values before collecting
  conversion, mark only the owning module or async frame aborted, and permit
  async-generator retry only for a terminal front `next()` request.
- 다른 대안 대신 이 방식을 선택한 이유: execution-time or VM-global Realm
  lookup is observably wrong after re-entry, blanket rejection makes Fuel
  catchable, path-specific fixes would drift again, and replaying a request
  after bytecode advancement can duplicate effects or change its result.
- 장점, 단점 및 영향: cross-Realm errors and thrown objects now retain their
  identity, host aborts leave reusable VM state, and queued generator/module
  siblings survive without changing Test262 counts. A hard heap limit can
  still leave the active async Promise pending when no capacity exists to
  allocate its rejection object, and file-backed module records are not yet
  independently owned per created Realm; both remain explicit follow-ups.

## Transactional test262 Realm construction

`$262.createRealm()` now treats intrinsic population and final host-wrapper
attachment as one transaction. The fresh environment is pinned before any
publication, nested installer pins are owned as one suffix, and an error
removes all 31 per-Realm registry families before native error materialization
runs in the caller Realm. The heap allocator, cache, fuel, and unrelated
finalization jobs are intentionally not rewound.

The allocation regression dynamically measures the complete Realm graph,
sweeps every insufficient capacity from zero through the final-wrapper
boundary, repeats that latest failure, and then creates and evaluates code in
a Realm at the exact successful capacity. Every failure restores all registry
counts and pin depth, materializes the main-Realm `RangeError`, and returns to
the original live-object count after GC. Separate tests collect immediately
after the fresh environment pin and while the fully populated provisional
graph is live; the former verifies that the registered heap cell remains an
`Environment` with the correct `globalThis` binding.

Against pinned Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, the 186
files containing `$262.createRealm` produce **109 pass / 8 fail / 69 skip**
with both feature commit `87741b1` and the preceding release artifact. The
eight existing failures and every status are identical. Broad release runs
remain Promise **433/0/270/703**, dynamic import **620/0/384/1004**, and the
supported subset **12751/0/7687/20438**. Python tooling is **100/100** and the
all-target/all-feature Rust, formatting, Clippy, release, and wasm32 gates
pass. CI `29587781649` and full matrix `29587781683` succeeded. All 30 result
files at `/tmp/ruja-artifacts-realm-rollback-feature.S47HSt` are byte-for-byte
identical to `/tmp/ruja-artifacts-hard-heap-feature.Im5lWX`; the workflow
aggregate remains **30159 pass / 6330 fail / 11816 skip / 12 timeout / 0 error
/ 48317 total / 36489 pass-or-fail executed**, or **62.4%** of all files and
**82.7%** of executed files.

## Explicit native constructor allocation modes

Native construction selected receiver allocation through immutable
`NativeConstructMode` metadata rather than a function-name allowlist. The
initial migration represented ordinary receiver preallocation, eager internal
allocation, and deferred internal allocation separately. Subsequent
primitive-wrapper and Date units moved the last preallocated users into their
native bodies and removed that enum variant; the current engine retains eager
and deferred modes. Bound and transparent Proxy construction forwards the
original new target, while the constructor, new target, argument list, cached
prototype, and fresh specialized object are rooted across every re-entrant or
collecting boundary.

At this migration checkpoint, internal registration tests required **19 eager
/ 19 deferred** constructors in both the main and a created Realm. Regressions
cover exact-cap Array
construction, getter and fallback-error order, bound/Proxy forwarding, forced
collection of direct-new arguments, pre-dispatch pending-state cleanup,
WeakMap/WeakSet call rejection and subclass prototypes, and revoked-Proxy
`super()` failures for normal and spread arguments. Two independent GPT 5.6
reviews returned `CLEAN` after eager fallback precedence, argument rooting,
foreign-Realm inventory, call-depth cleanup, and both `super()` paths were
covered.

Against the pinned Test262 revision, the eager affected cohort changes from
**3952 pass / 1223 fail / 1621 skip / 9 timeout / 6805 total** to **3954 /
1221 / 1621 / 9 / 6805**. The two status changes are the WeakMap and WeakSet
`undefined-newtarget` cases. The deferred cohort remains byte-identical at
**5693 pass / 120 fail / 514 skip / 1 timeout / 6328 total**. Promise remains
**433/0/270/703**, dynamic import **620/0/384/1004**, and the supported subset
**12751/0/7687/20438**.

Final local gates pass all targets and features, warnings-denied Clippy,
formatting, release, and wasm32, with Python tooling **100/100**, all-target
Rust lib/unit **95/95**, builtins **461/461**, classes **104/104**, modules
**31/31**, and Fuel **24/24**. Feature commit `6cc6dff` passed CI
`29596899916` and full matrix `29596899918`. Of the 30 downloaded result files
at `/tmp/ruja-artifacts-native-construct-feature.KyS5dn`, 29 are byte-identical
to `/tmp/ruja-artifacts-realm-rollback-feature.S47HSt`; built-ins changes only
by **+2 pass / -2 fail**. The aggregate is **30161 pass / 6328 fail / 11816
skip / 12 timeout / 0 error / 48317 total / 36489 pass-or-fail executed**, or
**62.4%** of all files and **82.7%** of executed files.

```text
[Decision Log]
- 목적과 의도: Replace implicit native-constructor allocation classification without broad, unmeasured changes to observable prototype timing.
- 기존 구현 및 제약 조건: A fixed function-name allowlist suppressed generic receivers, several specialized constructors depended on different validation and prototype orders, and Rust construction inputs were not automatically GC roots.
- 검토한 주요 대안: Expand the allowlist, move every constructor to one eager path, defer every lookup to builtin bodies, or encode the existing protocols explicitly and migrate ordering defects separately.
- 선택한 방식: Introduce three immutable migration modes, preserve each constructor's baseline eager or deferred timing, inventory every registration, and add scoped NewTarget cleanup plus complete input rooting; later family audits may remove modes with no remaining users.
- 다른 대안 대신 이 방식을 선택한 이유: One universal timing rule would introduce unrelated conformance regressions, while names cannot safely encode semantics. Explicit modes make the current contract testable and permit later constructor-specific migrations with focused Test262 evidence.
- 장점, 단점 및 영향: The allowlist and wasted exact-cap receiver are gone and two real Weak collection failures pass. At this checkpoint, native constructibility, super forwarding, wrapper coercion order, and constructors requiring no automatic prototype lookup remained visible follow-up units; the later sections record their independent audits.
```

## Explicit native constructibility and stack-safe Proxy forwarding

Native `[[Construct]]` presence is now represented by
`Option<NativeConstructMode>` rather than a function's observable prototype
slot. BigInt and Symbol remain constructors for heritage and `newTarget`
checks, but reject construction before argument coercion. Proxy and the
abstract `%TypedArray%` constructor use body-controlled dispatch without an
automatic prototype lookup. Created Realms own their Proxy constructor,
`revocable`, result objects, revokers, and construct-trap argument arrays.

BoundFunction and transparent-Proxy IsConstructor and construction traversal
are iterative, including argument prepending and `newTarget` substitution.
Normal and spread `super()` now invoke the same `[[Construct]]` path, so Proxy
superclasses use `construct` rather than `apply` and bound superclasses ignore
their bound `this`. Proxy `get`, `getOwnPropertyDescriptor`, and
`isExtensible` operations also flatten transparent and trap-bearing chains.
Descriptor targets and fresh trap results stay rooted until reverse invariant
validation completes. Regressions cover 20,000 constructor wrappers, 100,000
Proxy layers, fresh descriptor allocation under GC, exact-cap revocation, and
main/created-Realm identity.

Against pinned Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, the
frozen native-construct admission contains exactly five files:

- `built-ins/BigInt/is-a-constructor.js`
- `built-ins/Symbol/is-constructor.js`
- `built-ins/Proxy/constructor.js`
- `built-ins/Proxy/proxy-newtarget.js`
- `built-ins/Proxy/proxy-undefined-newtarget.js`

All five pass. With the new runner applied to both binaries, the combined
BigInt, Symbol, Proxy, `Reflect.construct`, and `Function.prototype.bind`
cohort improves from **250 pass / 8 fail / 338 skip / 596 total** to **251 /
7 / 338 / 596**; the implementation gain is Proxy call-without-`new`.
BigInt plus Symbol is **104/0/71/175**, Proxy is **53/0/258/311**, Promise
remains **433/0/270/703**, dynamic import remains **620/0/384/1004**, and the
supported subset remains **12751/0/7687/20438**. The 186 `$262.createRealm`
files remain **109/8/69** with the same eight failures.

Final local gates pass all targets and features, warnings-denied Clippy,
formatting, release, and wasm32, with Python tooling **101/101**, all-target
Rust lib/unit **100/100**, builtins **461/461**, classes **105/105**, modules
**31/31**, and Fuel **24/24**. Both GPT 5.6 final reviews returned `CLEAN` and
their sessions were closed. Feature commit `894e4bc` passed CI `29609644806`
and full matrix `29609644698`.

Of the 30 downloaded result files at
`/tmp/ruja-artifacts-native-constructibility-feature.UeJr8Q`, 29 are
byte-identical to
`/tmp/ruja-artifacts-native-construct-feature.KyS5dn`. Only built-ins changes,
from **14563 pass / 5511 fail / 3582 skip / 12 timeout** to **14568 / 5511 /
3577 / 12**. This artifact delta is **+5 pass / -5 skip** because the previous
workflow skipped the entire frozen admission. The aggregate is **30166 pass /
6328 fail / 11811 skip / 12 timeout / 0 error / 48317 total / 36494
pass-or-fail executed**, or **62.4%** of all files and **82.7%** of executed
files.

```text
[Decision Log]
- 목적과 의도: Admit only the native construction behavior proven by exact regressions while removing host-stack and GC-lifetime failure modes from wrapper forwarding.
- 기존 구현 및 제약 조건: Feature-level skips hid five passing-or-fixable files, and recursive Proxy/Bound/property traversal was invalid at legal depth even when ordinary shallow tests passed.
- 검토한 주요 대안: Remove broad feature gates, admit whole directories, keep a host depth cap, or freeze exact files and make the relevant abstract operations iterative.
- 선택한 방식: Share one five-file manifest between runner and analyzer, require exact parity tests, and retain pending Proxy results as explicit roots for reverse validation.
- 다른 대안 대신 이 방식을 선택한 이유: Directory or feature-wide admission would expose unrelated unsupported semantics, while a depth cap rejects valid programs. Exact admission ties every reported gain to covered behavior.
- 장점, 단점 및 영향: Five files move from skip to pass with no new full-matrix failure and deep wrappers cannot abort the host. The manifest remains intentionally narrow and must be extended only with a focused conformance unit.
```

## Primitive wrapper constructor order and Realm fallbacks

String, Number, and Boolean now use body-controlled native construction. The
value conversion happens before `NewTarget.prototype` is observed, direct
calls return primitives regardless of their `this` argument, and non-object
prototype values select the corresponding immutable primitive prototype from
the new target's Realm. This lookup follows BoundFunction and transparent
Proxy targets without consulting mutable Realm globals. String's Symbol
special case is limited to calls, so construction throws before reading the
new target prototype. Getter-produced prototypes remain rooted through the
one-cell wrapper allocation, and a saturated heap returns the preallocated
Realm `RangeError` without exceeding the cap.

The frozen native-construction manifest adds exactly ten audited files:

- String: `is-a-constructor.js`, `proto-from-ctor-realm.js`,
  `symbol-string-coercion.js`, and `symbol-wrapping.js`.
- Number: `is-a-constructor.js`, `proto-from-ctor-realm.js`, and
  `return-abrupt-tonumber-value-symbol.js`.
- Boolean: `is-a-constructor.js`, `proto-from-ctor-realm.js`, and
  `symbol-coercion.js`.

All ten pass against Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`.
The complete String/Number/Boolean subtree is **1504 pass / 0 fail / 110 skip
/ 1614 total**. Applying the same new runner to the preceding release binary
produces **1500 / 4 / 110 / 1614**; the four implementation fixes are the
three foreign-Realm prototype files and String Symbol wrapping. The other six
files were already runtime-green and move from skip to pass through exact
admission. All 13 wrapper subclass files remain green, and the supported
subset remains **12751 pass / 0 fail / 7687 skip / 20438 total**.

Final local gates pass all targets and features, warnings-denied Clippy,
formatting, release, and wasm32, with Python tooling **101/101**, Rust lib/unit
**104/104**, builtins **461/461**, classes **105/105**, modules **31/31**, and
Fuel **24/24**. GPT 5.6 reviewers Herschel and Russell both returned `CLEAN`
and were closed. Feature commit `ddf3d55` passed CI `29613370285` and full
matrix `29613370302`.

Of the 30 downloaded result files at
`/tmp/ruja-artifacts-primitive-wrappers-feature.ArVzjB`, 29 are byte-identical
to `/tmp/ruja-artifacts-native-constructibility-feature.UeJr8Q`. Only the
built-ins result changed, from **14568 pass / 5511 fail / 3577 skip / 12
timeout** to **14578 / 5511 / 3567 / 12**. The workflow delta is **+10 pass /
-10 skip**
because all ten paths were skipped by the preceding workflow's manifest. The
aggregate is **30176 pass / 6328 fail / 11801 skip / 12 timeout / 0 error /
48317 total / 36504 pass-or-fail executed**, or **62.5%** of all files and
**82.7%** of executed files.

```text
[Decision Log]
- 목적과 의도: Report only primitive-wrapper behavior whose conversion order, Realm fallback, allocation, and call-versus-construct semantics are directly verified.
- 기존 구현 및 제약 조건: Feature-level gates skipped ten relevant files; four failed when forced to run because generic preallocation observed the wrong thing at the wrong time. Broad feature admission would also expose unrelated unsupported semantics.
- 검토한 주요 대안: Remove Reflect/cross-realm/Symbol gates globally, admit complete constructor directories, or extend the existing exact native-construction manifest.
- 선택한 방식: Freeze ten paths with per-file feature exceptions shared by the runner and analyzer, and require local order/Realm/GC/cap regressions beyond the pinned files.
- 다른 대안 대신 이 방식을 선택한 이유: Exact admission ties every metric change to audited behavior while local regressions cover observable ordering that the current pinned files do not exercise.
- 장점, 단점 및 영향: Ten skips become passes with no new fail, timeout, or error; the manifest remains narrow and must be extended deliberately. These results did not imply Date support, which is handled by the separate unit below.
```

## Date constructor call/construct order and Realm fallbacks

Date now uses body-controlled native construction. Calls return a date String
regardless of the supplied `this` and do not coerce argument values.
Construction computes and clips its Date value before observing
`NewTarget.prototype`; abrupt conversion stops before that getter. Non-object
prototypes select the immutable `%Date.prototype%` from the new target's Realm,
including BoundFunction and transparent-Proxy targets. Each created Realm owns
its Date constructor, prototype, methods, and static functions. Constructed
instances keep `[[DateValue]]` in an internal slot, while `%Date.prototype%`
remains unbranded. Getter-produced prototypes remain rooted through the
single-cell sandbox allocation.

The frozen native-construction manifest adds exactly five audited files:

- `built-ins/Date/is-a-constructor.js`
- `built-ins/Date/subclassing.js`
- `built-ins/Date/proto-from-ctor-realm-zero.js`
- `built-ins/Date/proto-from-ctor-realm-one.js`
- `built-ins/Date/proto-from-ctor-realm-two.js`

All five pass against Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`.
The complete Date subtree is **516 pass / 0 fail / 78 skip / 594 total**.
Applying the same new runner to the preceding release binary produces **512 /
4 / 78 / 594**; the four implementation fixes are Date subclassing and the
three foreign-Realm fallback files. `is-a-constructor.js` was already
runtime-green and moves from skip to pass through exact admission. All four
Date subclass files pass, and the supported subset remains **12751 pass / 0
fail / 7687 skip / 20438 total**.

Final local gates pass all targets and features, warnings-denied Clippy,
formatting, release, and wasm32, with Python tooling **101/101**, Rust lib/unit
**108/108**, builtins **461/461**, classes **105/105**, modules **31/31**, and
Fuel **24/24**. GPT 5.6 reviewers Bacon and Mendel both returned `CLEAN` and
were closed. Feature commit `5bdc7bd` passed CI `29618073392` and full matrix
`29618073439`.

Of the 30 downloaded result files at
`/tmp/ruja-artifacts-date-feature.ezDrIL`, 29 are byte-identical to
`/tmp/ruja-artifacts-primitive-wrappers-feature.ArVzjB`. Only the built-ins
result changed, from **14578 pass / 5511 fail / 3567 skip / 12 timeout** to
**14583 / 5511 / 3562 / 12**. The workflow delta is **+5 pass / -5 skip**
because all five paths were skipped by the preceding workflow's manifest. The
aggregate is **30181 pass / 6328 fail / 11796 skip / 12 timeout / 0 error /
48317 total / 36509 pass-or-fail executed**, or **62.5%** of all files and
**82.7%** of executed files.

```text
[Decision Log]
- 목적과 의도: Admit only Date construction behavior whose call split, conversion order, Realm fallback, hidden state, and sandbox allocation are directly verified.
- 기존 구현 및 제약 조건: The runner skipped five relevant files; four failed when forced because generic preallocation observed the new-target prototype too early or selected the wrong Realm fallback. Broad Date admission would expose unrelated unsupported paths.
- 검토한 주요 대안: Remove cross-Realm and Reflect gates globally, admit the complete Date directory, reuse primitive-wrapper admission, or extend the exact native-construction manifest.
- 선택한 방식: Freeze five Date paths with per-file feature exceptions shared by runner and analyzer, and pair them with local call/apply/bound, abrupt-order, Realm, GC, and exact-cap regressions.
- 다른 대안 대신 이 방식을 선택한 이유: Exact admission ties every matrix gain to audited behavior while local regressions cover observable order and hidden-slot properties not asserted by the pinned files.
- 장점, 단점 및 영향: Five skips become passes with no new fail, timeout, or error, and four real runtime defects are removed. Dynamic Function-family and RegExp construction remain separate conformance units.
```

## Dynamic Function construction, Realm fallback, and allocation

`Function`, `AsyncFunction`, `GeneratorFunction`, and
`AsyncGeneratorFunction` now share one CreateDynamicFunction implementation.
It preserves parameter-then-body `ToString` order, validates the two grammar
parts independently before a combined early-error parse, and places newline
boundaries around the synthetic parameter and body delimiters. Calls use the
active constructor as the effective new target; construction uses the actual
`NewTarget`. Generated closures and their fresh ordinary/generator prototype
parents come from the active constructor's Realm. When
`NewTarget.prototype` is not an object, the default function prototype comes
from the actual new target's immutable Realm registry.

The frozen native-construction manifest adds exactly seven audited files:

- `built-ins/Function/is-a-constructor.js`
- `built-ins/Function/proto-from-ctor-realm-prototype.js`
- `built-ins/Function/proto-from-ctor-realm.js`
- `built-ins/AsyncFunction/is-a-constructor.js`
- `built-ins/AsyncFunction/proto-from-ctor-realm.js`
- `built-ins/GeneratorFunction/is-a-constructor.js`
- `built-ins/AsyncGeneratorFunction/is-a-constructor.js`

All seven pass against Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`.
The four complete constructor directories are **429 pass / 52 fail / 92 skip
/ 573 total**. Applying the same new runner to the preceding Date binary gives
**420 / 61 / 92 / 573**. The exact nine fail-to-pass transitions, with no
regressions, are Function `S15.3.2.1_A3_T1`, `T3`, `T4`, `T5`, and `T8`, the
two Function constructor-Realm files, Function prototype
`S15.3.5.2_A1_T2`, and the AsyncFunction constructor-Realm file. The 24-file
direct constructor/source/Realm audit and a separate 16-file forced
constructor/order cohort both pass completely. The supported subset remains
**12751 pass / 0 fail / 7687 skip / 20438 total**.

Local regressions additionally cover abrupt conversion and prototype-getter
order, comment and delimiter injection boundaries, contextual and
destructuring BindingIdentifiers, late strictness, class-static-block arrow
early errors, call/bind/construct behavior, main and foreign Realms, replaced
globals, BoundFunction and Proxy new targets, fresh Proxy trap results across
re-entrant GC, exact one- and two-cell allocation, saturated failure, pin and
pending-context cleanup, and compilation-table rollback that preserves a
successful re-entrant function. Final local gates pass all targets/features,
warnings-denied Clippy, formatting/diff, release, and wasm32, with Python
tooling **101/101**, Rust lib/unit **114/114**, builtins **461/461**, classes
**105/105**, modules **31/31**, and Fuel **24/24**. GPT 5.6 reviewers Ptolemy
and Copernicus returned `CLEAN` and were closed.

Feature commit `a320d15` passed CI `29624418616` and full matrix
`29624418655`. Of the 30 result files at
`/tmp/ruja-artifacts-dynamic-function-feature.upHgF8`, 29 are byte-identical
to `/tmp/ruja-artifacts-date-feature.ezDrIL`. Only built-ins changed, from
**14583 pass / 5511 fail / 3562 skip / 12 timeout** to **14596 / 5505 / 3555
/ 12**, exactly **+13 pass / -6 fail / -7 skip**. The aggregate is **30194
pass / 6322 fail / 11789 skip / 12 timeout / 0 error / 48317 total / 36516
pass-or-fail executed**, or **62.5%** of all files and **82.7%** of executed
files.

The constructor algorithm does not yet retain specification source text for
`Function.prototype.toString`; the four constructor-specific source tests
remain failures and are not admitted. RuJa's local-trust host policy also
permits string compilation unconditionally, and its three parser passes are a
synchronous host operation rather than fuel-metered bytecode.

```text
[Decision Log]
- 목적과 의도: Admit only Dynamic Function behavior whose conversion order, separate grammar boundaries, Realm fallback, prototype graph, GC rooting, and exact-cap allocation are directly verified.
- 기존 구현 및 제약 조건: Eager construction observed incomplete new-target rules, a combined-only wrapper parse could cross synthetic boundaries, generated functions reused main-Realm parents, and raw allocation plus early table publication could leave stale heap or compiler state.
- 검토한 주요 대안: Admit complete constructor directories, remove cross-Realm and Reflect feature gates globally, keep eager generic allocation, or freeze a narrow exact manifest around one shared body-controlled implementation.
- 선택한 방식: Move all four constructors to deferred native construction, parse parameter/body/combined sources with newline guards, use immutable constructor-Realm registries, publish nested definitions after observable lookup, and allocate through rooted sandbox paths with suffix rollback.
- 다른 대안 대신 이 방식을 선택한 이유: Broad admission exposes unrelated source-text and async gaps, generic preallocation has the wrong observable order, and separate per-kind implementations would duplicate the same abstract operation and GC cleanup.
- 장점, 단점 및 영향: Seven skips and six previously executed failures become passes with no new fail, timeout, or error. The exact manifest remains narrow; source-text preservation, a restrictive host compile hook, and native parse-time metering remain explicit follow-up work.
```

## RegExp construction, matchAll, and exotic property writes

RegExp now uses body-controlled native construction. The shared `IsRegExp`
operation observes `Symbol.match` before consulting the internal
`[[RegExpMatcher]]` marker. The call identity shortcut, actual-RegExp internal
source/flags copy, regexp-like property gets, new-target prototype selection,
allocation, and final string conversions follow their separate specification
phases. Non-object new-target prototypes use the immutable
`%RegExp.prototype%` from the actual new target's Realm. Created Realms retain
their own immutable `%RegExp%`, `%RegExp.prototype%`, and
`%RegExpStringIteratorPrototype%`; literals, RegExpCreate, species defaults,
iterators, and match result objects use those identities without consulting
replaced globals.

`RegExp.prototype[Symbol.matchAll]` now performs input `ToString` before species
lookup, reads flags and lastIndex in order, writes matcher lastIndex through
strict `Set`, and returns a Realm-correct branded iterator. Observable species,
getter, trap, matcher, iterator, and result values remain pinned through
re-entrant GC and exact-cap allocation.

The property support required by strict `Set` is shared rather than
matchAll-specific. OrdinarySet preserves the original receiver and stops at
the nearest data descriptor. Proxy prototypes and nested Proxy invariant
checks observe null/missing traps and current target state in order. Fresh
descriptor values are rooted across later traps. CreateDataProperty and
value-only DefineProperty preserve Array, integer-indexed TypedArray, and
mapped-arguments exotic semantics. ArraySetLength performs both observable
number conversions, rejects indices beyond a non-writable length, deletes in
descending order with rollback above a non-configurable element, tracks sparse
maximum indices, synchronizes materialized length descriptors, and invalidates
the length inline cache.

The frozen native-construction manifest adds exactly eight audited files:

- `built-ins/RegExp/is-a-constructor.js`
- `built-ins/RegExp/proto-from-ctor-realm.js`
- `built-ins/RegExp/from-regexp-like-flag-override.js`
- `built-ins/RegExp/from-regexp-like-get-ctor-err.js`
- `built-ins/RegExp/from-regexp-like-get-flags-err.js`
- `built-ins/RegExp/from-regexp-like-get-source-err.js`
- `built-ins/RegExp/from-regexp-like-short-circuit.js`
- `built-ins/RegExp/from-regexp-like.js`

All eight pass against Test262
`020cb74075849d1e404bbcdb62feb7a02e6966db`. The complete
`built-ins/RegExp` subtree improves from **865 pass / 144 fail / 864 skip / 6
timeout** to **880 / 137 / 856 / 6**. The focused
`built-ins/RegExp/prototype/Symbol.matchAll` result is **25 pass / 0 fail / 1
skip**, and the supported subset remains **12751 pass / 0 fail / 7687 skip /
20438 total**.

Final local gates pass all targets/features, warnings-denied Clippy,
formatting/diff, release, and wasm32. Python tooling is **101/101**, Rust
lib/unit **120/120**, bugfixes **67/67**, and builtins **463/463**. GPT 5.6
reviewers Dirac and Laplace returned `CLEAN` after the ArraySetLength,
sparse-length, and synthetic-own-length findings were fixed, and both were
closed. No Umans provider route or coder model was used.

Feature commit `ff492ff` passed CI `29633368519` and full matrix
`29633368501`. Of the 30 result files at
`/tmp/ruja-artifacts-regexp-feature.kNQTlF`, 29 are byte-identical to
`/tmp/ruja-artifacts-dynamic-function-feature.upHgF8`. Only built-ins changed,
from **14596 pass / 5505 fail / 3555 skip / 12 timeout** to **14704 / 5405 /
3547 / 12**, a net **+108 pass / -100 fail / -8 skip**. The aggregate is
**30302 pass / 6222 fail / 11781 skip / 12 timeout / 0 error / 48317 total /
36524 pass-or-fail executed**, or **62.7%** of all files and **83.0%** of
executed files. The remaining **137** RegExp failures are broader syntax,
matching, and method semantics and are not claimed complete by this unit.

```text
[Decision Log]
- 목적과 의도: Admit only RegExp construction and matchAll behavior whose observable ordering, Realm identity, Proxy/exotic property semantics, GC rooting, and exact-cap allocation are directly verified.
- 기존 구현 및 제약 조건: Class-name branding, eager preallocation, mutable global fallbacks, partial Set/DefineProperty helpers, and unrooted fresh trap results produced wrong order or unsafe lifetime behavior. Broad RegExp support still contains unrelated syntax and matching gaps.
- 검토한 주요 대안: Remove RegExp-related feature gates globally, admit the complete RegExp directory, patch matchAll locally, or extend the exact native-construction manifest after implementing shared abstract operations.
- 선택한 방식: Freeze eight constructor paths, use a deferred internal-slot-based constructor and immutable Realm registries, and route strict matcher writes through receiver-aware Proxy and exotic-object dispatch with full ArraySetLength behavior.
- 다른 대안 대신 이 방식을 선택한 이유: Exact admission ties each skip transition to audited behavior, while shared operations fix the same correctness defects for ordinary built-ins without claiming the remaining RegExp surface. Local exceptions would leave divergent Proxy, Array, and GC semantics.
- 장점, 단점 및 영향: The full matrix gains 108 passes and removes 100 failures with no language-subset regression. Two Realm registry families and the property core require synchronized maintenance; 137 RegExp failures remain explicit future work.
```

## RegExp Symbol.split and UTF-16 execution

`RegExp.prototype[Symbol.split]` now implements the generic ECMA-262 algorithm.
It converts the input before species lookup, reads and converts flags, appends
sticky mode when required, constructs the splitter, creates the result with the
method Realm's `%Array.prototype%`, converts the limit with `ToUint32`, and
performs strict `lastIndex` writes plus dynamic `RegExpExec` calls. Empty
matches advance with `AdvanceStringIndex`; separators, captures, and the tail
are sliced at UTF-16 indices and appended with CreateDataProperty semantics.

`String.prototype.split` no longer contains a class-name-based RegExp branch.
It delegates through `separator[Symbol.split]` only when that value is
non-nullish and callable, including callable Proxies. A nullish hook follows
the ordinary separator path, whose search and slicing operate on UTF-16 code
units rather than Rust scalar boundaries. This preserves lone-surrogate
separators, empty separators, and supplementary strings.

The Rust regex backends consume Unicode scalars, while non-Unicode ECMAScript
patterns consume UTF-16 code units. RuJa therefore uses a sentinel-backed input
view only for non-Unicode `RegExp.exec`, paired with a code-unit pattern
normalization mode. Raw supplementary pattern characters and escaped surrogate
pairs become two backend atoms in that mode. Scalar-backed direct replacement
keeps its existing compile mode, avoiding a regression for raw supplementary
patterns. The same mode is propagated into repeated-capture clearing so the
last iteration cannot retain a stale earlier capture.

Every observable `@@split` intermediate is pinned across conversion,
construction, property access, and custom `exec` calls. Search and capture
loops, plus code-unit input conversion, consume VM fuel. Native match arrays
use a private GC-retrying allocator because their elements are restricted to
strings and `undefined`; the helper uses the executing Realm's Array prototype.
Keeping this path private is intentional: changing the shared array helper to
collect would invalidate unrelated callers that still hold unpinned
object-valued Rust locals.

On Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, focused
`built-ins/RegExp/prototype/Symbol.split` is **43 pass / 0 fail / 1 skip** and
`built-ins/String/prototype/split` is **117 / 0 / 3**. The complete
`built-ins/RegExp` subtree moves from **880 pass / 137 fail / 856 skip / 6
timeout** to **922 / 95 / 856 / 6**, exactly **+42 pass / -42 fail**. The
supported subset remains **12751 pass / 0 fail / 7687 skip / 20438 total**.

Final local gates pass all targets/features, warnings-denied Clippy,
formatting/diff, release, and wasm32. Python tooling is **101/101**, Rust
lib/unit **122/122**, bugfixes **67/67**, builtins **467/467**, and Fuel
**25/25**. GPT 5.6 reviewers Fermat and Beauvoir returned `CLEAN` after fixes
for surrogate boundaries, callable Proxy hooks, nullish fallback, repeated
capture clearing, exact-cap allocation, and foreign-Realm match arrays. No
Umans provider route or coder model was used.

Feature SHA `0e08dc87c288789fe1c7f5b9e21809b053eff131` passed ordinary CI
`29638102394` and full matrix `29638102407`. Of the 30 result files at
`/tmp/ruja-artifacts-regexp-split-feature.0TQyVs`, 29 are byte-identical to
`/tmp/ruja-artifacts-regexp-feature.kNQTlF`. Built-ins changes from **14704
pass / 5405 fail / 3547 skip / 12 timeout** to **14746 / 5363 / 3547 / 12**,
exactly **+42 pass / -42 fail**. The aggregate is **30344 pass / 6180 fail /
11781 skip / 12 timeout / 0 error / 48317 total / 36524 pass-or-fail
executed**, or **62.8%** of all files and **83.1%** of executed files.

At this checkpoint, the remaining **95** RegExp failures grouped exactly into
**42** direct/root legacy syntax and matcher files, **29**
`prototype/Symbol.replace` files, **16** lookbehind files, **6** match-indices
files, and **2** CharacterClassEscapes files. The following unit closes the 29
`Symbol.replace` failures; the larger 42-file root bucket remains
heterogeneous and must not be treated as one patch.

```text
[Decision Log]
- 목적과 의도: Implement the complete generic RegExp Symbol.split contract while preserving sandbox GC, Realm, UTF-16, and fuel invariants.
- 기존 구현 및 제약 조건: String split recognized RegExp by observable class name, bypassed species and dynamic exec, searched Rust scalar strings directly, and allocated native match arrays without GC retry. The regex backends cannot directly address the middle of a supplementary scalar.
- 검토한 주요 대안: Keep a special String split regex branch, rewrite the regex engine, convert every regex consumer to sentinel-backed input, or isolate code-unit execution and implement the specification method directly.
- 선택한 방식: Register Realm-local Symbol.split, follow the specification algorithm with rooted observable state, use Realm-aware ArrayCreate, isolate sentinel-backed code-unit compilation to non-Unicode exec, retain scalar replacement mode, and give native match arrays a private GC-retrying allocator.
- 다른 대안 대신 이 방식을 선택한 이유: A String-only shortcut cannot satisfy generic species and exec observability; a global backend conversion regresses direct replacement and changes unrelated allocation contracts; replacing the regex engine is a separate architectural project.
- 장점, 단점 및 영향: All 42 executed Symbol.split failures become passes with no RegExp skip, timeout, language-subset, or matrix regression. Non-Unicode execution now preserves code units, but rebuilding the sentinel input on each native exec remains a bounded performance issue for future optimization.
```

## RegExp Symbol.replace and generic substitution

`RegExp.prototype[Symbol.replace]` now implements the generic
[ECMA-262 replacement algorithm](https://tc39.es/ecma262/multipage/text-processing.html#sec-regexp.prototype-@@replace).
The method converts the input and replacement in the required order,
recognizes callable Proxies, observes global and Unicode state, performs the
strict initial `lastIndex = 0` write for global matching, and repeatedly calls
the dynamic `RegExpExec` path. Empty global matches advance with
`AdvanceStringIndex` rather than a Rust scalar offset.

Matching and replacement are intentionally separate phases. The first phase
retains each raw result object. The second reads result length, matched text,
clamped index, captures, and named groups in order, then invokes the functional
replacer or GetSubstitution. This preserves getters, custom `exec` results,
side effects, and exceptions that the previous direct backend loop bypassed.

GetSubstitution operates on UTF-16 code units and supports `$$`, `$&`,
``$` ``, `$'`, `$n`, `$nn`, and `$<name>`. Numeric capture parsing now requires
the leading `$`, so ordinary text such as `a1` or `foo1` is not consumed as a
capture reference. Functional replacement materializes at most
`MAX_MATERIALIZED_CALL_ARGUMENTS - 3` captures before the string, position,
and input arguments; the final optional groups argument is checked against the
same sandbox call cap.

All retained results, getter values, captures, groups, replacer arguments, and
partially assembled output remain rooted across re-entrant GC. Named-groups
objects use the sandbox allocator, and match collection plus UTF-16 output
assembly consume VM fuel. Exact-cap and forced-GC regressions cover both
string and functional replacement. The Test262 host also installs `print` for
both synchronous and asynchronous harnesses; the shim is covered by the
tooling tests instead of changing engine semantics.

On Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, focused
`built-ins/RegExp/prototype/Symbol.replace` is **60 pass / 0 fail / 10 skip**.
The complete `built-ins/RegExp` subtree moves from **922 pass / 95 fail / 856
skip / 6 timeout** to **951 / 66 / 856 / 6**, exactly **+29 pass / -29 fail**.
The supported subset remains **12751 pass / 0 fail / 7687 skip / 20438 total**.

Final local gates pass all targets/features, warnings-denied Clippy,
formatting/diff, release, and wasm32. Python tooling is **102/102**, Rust
lib/unit **124/124**, bugfixes **67/67**, builtins **468/468**, and Fuel
**26/26**. GPT 5.6 reviewers Ampere and Nash returned `CLEAN` after fixes for
ordinary trailing digits, the capture-materialization boundary, GC-rooted
group allocation, and custom result ordering. Both were closed. No Umans
provider route or coder model was used.

Feature SHA `55c5943c8100a3dba12a429ea025a3d70dd3b30d` passed ordinary CI
`29640862796` and full matrix `29640862819`. Of the 30 result files at
`/tmp/ruja-artifacts-regexp-replace-feature.v6d1PN`, 29 are byte-identical to
`/tmp/ruja-artifacts-regexp-split-feature.0TQyVs`. Only built-ins changed, from
**14746 pass / 5363 fail / 3547 skip / 12 timeout** to **14775 / 5334 / 3547
/ 12**, exactly **+29 pass / -29 fail**. The aggregate is **30373 pass / 6151
fail / 11781 skip / 12 timeout / 0 error / 48317 total / 36524 pass-or-fail
executed**, or **62.9%** of all files and **83.2%** of executed files.

The remaining **66** RegExp failures group into **42** direct/root legacy
syntax and matcher files, **16** lookbehind files, **6** match-indices files,
and **2** CharacterClassEscapes files. The two CharacterClassEscapes files are
the smallest next diagnostic unit; they must be baselined and classified
before any feature admission. Match indices and lookbehind remain independent
contracts.

```text
[Decision Log]
- 목적과 의도: Implement the generic RegExp Symbol.replace and GetSubstitution contracts without weakening sandbox GC, argument, UTF-16, Realm, or fuel invariants.
- 기존 구현 및 제약 조건: The old method iterated the native regex backend directly, bypassed dynamic RegExpExec and observable result objects, mixed matching with replacement conversion, used scalar offsets, and left result/group lifetimes and large replacer argument lists unsafe at sandbox boundaries.
- 검토한 주요 대안: Patch the 29 tests individually, keep a fast native-only path for branded RegExp objects, delegate substitution to the Rust backend, or follow the two-phase specification algorithm for every receiver.
- 선택한 방식: Use dynamic RegExpExec to collect rooted result objects, process their properties in specification order, implement UTF-16 GetSubstitution locally, meter native work, and enforce the existing call-argument and allocation limits explicitly.
- 다른 대안 대신 이 방식을 선택한 이유: Native shortcuts cannot preserve custom exec, getters, Proxy callability, side-effect order, or ECMAScript replacement-token behavior. A single generic path keeps observable semantics consistent and leaves backend optimization as a later non-semantic unit.
- 장점, 단점 및 영향: All 29 executed Symbol.replace failures become passes with no skip, timeout, supported-subset, or non-built-ins matrix regression. The generic path retains all matches before replacement and therefore uses bounded heap proportional to global match count; fuel and the heap cap remain the controlling sandbox limits.
```

## RegExp CharacterClassEscape digit and whitespace sets

RuJa now lowers `\d`, `\D`, `\s`, and `\S` according to the
[ECMA-262 CharacterClassEscape semantics](https://tc39.es/ecma262/multipage/text-processing.html#sec-runtime-semantics-compiletocharset)
before selecting either Rust regex backend. ECMAScript digits are exactly
ASCII `0-9`, unlike Rust regex's default Unicode `Nd` set. ECMAScript `\s`
is the lexical
[WhiteSpace](https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#sec-white-space)
plus LineTerminator set: it includes `U+FEFF`, and it excludes `U+0085` and
the former whitespace character `U+180E`.

The same normalization is used for ordinary patterns and patterns that need
the backreference-capable backend. It handles positive and complemented sets
outside classes and in ordinary, non-nested character classes. The existing
non-Unicode code-unit mode still treats a supplementary character as two code
units.
Escaped-backslash parity remains intact, so a literal `\d` is not rewritten.

In non-Unicode mode,
[Annex B range compatibility](https://tc39.es/ecma262/multipage/additional-ecmascript-features-for-web-browsers.html#sec-regular-expressions-patterns)
requires a range with a multi-character set endpoint to become the union of
both endpoints and a literal hyphen. The normalizer therefore protects an
unescaped hyphen before a right-hand set endpoint. Both `[\d-a]` and
`[a-\d]`, chained forms, and already escaped hyphens have focused Rust
regressions. Unicode `u`/`v` syntax remains governed by the stricter main
grammar.

Six generated complement-set files construct strings spanning roughly 1.1
million code points and take 14-21 seconds after they reach the matcher. One
root file exhaustively checks all 65,536 UTF-16 code units and takes about 25
seconds. The runner and both analyzers therefore share exact path-scoped
timeouts: 30 seconds for only the six generated files and 60 seconds for only
the exhaustive root file. Every neighboring RegExp test retains the 8-second
default.

On Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, focused
`built-ins/RegExp/CharacterClassEscapes` moves from **4 pass / 2 fail / 0 skip
/ 6 timeout** to **12 / 0 / 0 / 0**. The separate exhaustive root file also
passes. The complete `built-ins/RegExp` subtree moves from **951 pass / 66
fail / 856 skip / 6 timeout** to **960 / 63 / 856 / 0**, exactly **+9 pass /
-3 fail / -6 timeout**. The supported subset remains **12751 pass / 0 fail /
7687 skip / 20438 total**.

Final local gates pass all targets/features, warnings-denied Clippy,
formatting/diff, release, and wasm32. Python tooling is **105/105**, Rust
lib/unit **124/124**, bugfixes **67/67**, builtins **469/469**, and Fuel
**26/26**. GPT 5.6 reviewers Godel and Sagan found the Annex B right-hand
range issue and the risk of partially changing active-ignoreCase word
semantics. The range issue is fixed, the partial word-set change was removed,
and both reviewers returned final `CLEAN`. Both are closed. No Umans provider
route or coder model was used.

Feature SHA `c9065fae2687fab9ac211e2b251cfde854f82d21` passed ordinary CI
`29644825071` and full matrix `29644825073`. Of the 30 result files at
`/tmp/ruja-artifacts-regexp-character-class-feature.k2utbC`, 28 are
byte-identical to `/tmp/ruja-artifacts-regexp-replace-feature.1Cy2uW`.
Built-ins changes from **14775 pass / 5334 fail / 3547 skip / 12 timeout**
to **14784 / 5331 / 3547 / 6**, exactly **+9 pass / -3 fail / -6
timeout**. Annex B changes from **195 / 817 / 74 / 0** to **196 / 816 /
74 / 0**, exactly **+1 pass / -1 fail**, because
`annexB/language/literals/regexp/non-empty-class-ranges-no-dash.js` now
passes. A baseline-binary/new-binary differential over all 1,086 Annex B
files confirms it is the only changed test there.

Counting the human-readable result bodies gives **30383 pass / 6147 fail /
11931 skip / 6 timeout / 0 error / 48467 total / 36530 pass-or-fail
executed**. The feature run's `RATE=` aggregate undercounted the 150 files in
three skipped-only directories; the separate accounting fix below makes that
true total machine-readable.

The remaining **63** RegExp failures group into **41** direct/root legacy
syntax and matcher files, **16** lookbehind files, and **6** match-indices
files. The next coherent character-set unit is active-ignoreCase
WordCharacters: `\w`/`\W` matching inside and outside classes, plus
outside-class `\b`/`\B` boundary checks, must derive from one set across plain
`i`, `iu`, `iv`, and local modifiers. Unicode-aware canonicalization adds
`U+017F` and `U+212A` without
admitting unrelated Unicode letters or digits. That contract remains separate
from this digit/whitespace patch.

```text
[Decision Log]
- 목적과 의도: Make RegExp digit and whitespace character-class escapes match ECMAScript exactly without weakening sandbox, UTF-16, or backend invariants.
- 기존 구현 및 제약 조건: Both Rust regex backends interpreted digit and whitespace escapes with Unicode-native sets, omitting FEFF and admitting characters that ECMAScript excludes. Generated complement tests and one exhaustive root test also exceed the generic 8-second process limit for legitimate work.
- 검토한 주요 대안: Disable Unicode globally in the backend, post-filter match results, replace the regex engine, broaden every RegExp timeout, or lower only the affected escapes and grant exact per-file timeouts.
- 선택한 방식: Normalize d/D/s/S to explicit ECMAScript sets before backend selection, preserve Annex B hyphen-union behavior in non-Unicode classes, and share narrowly enumerated timeout policies across the runner and analyzers.
- 다른 대안 대신 이 방식을 선택한 이유: A global backend mode changes unrelated literals, properties, and case folding; post-filtering cannot reconstruct captures, backreferences, complements, or class ranges; replacing the backend is a separate architecture project; broad timeouts would hide regressions.
- 장점, 단점 및 영향: Ten matrix outcomes become passes, all six prior timeouts disappear, the supported subset stays green, and 28 unrelated result files remain byte-identical. The explicit whitespace inventory must track future specification changes, while active-ignoreCase word and boundary semantics remain a separate complete unit.
```

## Test262 zero-execution aggregate accounting

`tools/test262_runner.py` emits a human-readable count block followed by one
machine-readable `RATE=` line consumed by CI. When `RAN=0`, meaning there is
no pass/fail outcome, the old branch hardcoded every `RATE=` count to zero
even if a directory contained skipped, timed-out, or errored files. The
human-readable block
remained correct, which is why direct artifact inspection exposed a
150-file discrepancy in `language/{destructuring,export,import}`.

The zero-execution branch now preserves `PASS`, `FAIL`, `SKIP`, and `TOTAL`
from the same counters as the ordinary branch while retaining `RATE=0.0` and
`RAN=0`. Key names and order are unchanged for workflow parsers. A focused
test drives `main()` through an all-skipped temporary directory and asserts
the complete output contract; a real `language/import` run reports its
nonzero skipped total. Python tooling is **106/106**.

Commit `8f705936f6a8a070fbc48ecb77102b065011aebe` passed ordinary CI
`29646302861` and full matrix `29646302891`. Of the 30 result files at
`/tmp/ruja-artifacts-zero-run-rate.yRcxW7`, 27 are byte-identical to feature
run `29644825073`. The other three differ only in their final `RATE=` line:
`language/export` preserves **3** skipped/total files, `language/import`
preserves **128**, and `language/destructuring` preserves **19**. The
machine and human aggregates now agree at **30383 pass / 6147 fail / 11931
skip / 6 timeout / 0 error / 48467 total / 36530 pass-or-fail executed**, or
**62.7%** of all files and **83.2%** of executed files. Pass, fail, timeout,
and executed counts are unchanged from the feature run.

```text
[Decision Log]
- 목적과 의도: Make the published full-matrix file count and skip count agree with the runner's own result summaries.
- 기존 구현 및 제약 조건: The RAN=0 branch emitted an all-zero RATE line, while CI aggregates only RATE fields and cannot recover skipped or total counts from the preceding human-readable block.
- 검토한 주요 대안: Leave the historical aggregate unchanged, teach the workflow to parse two output formats, count files independently in CI, or preserve counters in the existing RATE contract.
- 선택한 방식: Keep RATE=0.0 and RAN=0 but emit the actual PASS, FAIL, SKIP, and TOTAL counters in the established key order.
- 다른 대안 대신 이 방식을 선택한 이유: The runner already owns the authoritative counters, and preserving its stable single-line contract fixes every consumer without adding workflow-only parsing logic.
- 장점, 단점 및 영향: Skipped-only files are included in the published denominator and artifacts become internally consistent. The all-file percentage decreases when the previously omitted files are counted, but executed-test results and the supported-subset rate do not change.
```

## RegExp ignore-case word escapes

Active-ignoreCase `\w` and `\W` now derive from the ECMA-262
[WordCharacters](https://tc39.es/ecma262/multipage/text-processing.html#sec-runtime-semantics-wordcharacters)
set. Plain `i` mode uses exactly ASCII letters, digits, and underscore.
Unicode-aware `iu` and `iv` modes add only `U+017F` and `U+212A`, because
those are the non-ASCII characters whose simple case fold enters the ASCII
word set. Unrelated Unicode letters, CJK characters, and digits such as
`U+0660` remain outside the set. The same active modifier state applies to
top-level escapes, local `(?i:...)`/`(?-i:...)` scopes, and character classes.

Outside a class, each active escape becomes an exact, scoped case-sensitive
backend set. Inside an ordinary class, normalization first parses the class
with the `regex-syntax` HIR parser, materializes the applicable
[Canonicalize](https://tc39.es/ecma262/multipage/text-processing.html#sec-runtime-semantics-canonicalize-ch)
equivalence closure, applies outer negation after that closure, serializes the
result, and disables backend case folding only for the emitted set. This order
matches the specification's
[CharacterSetMatcher](https://tc39.es/ecma262/multipage/text-processing.html#sec-runtime-semantics-charactersetmatcher)
contract and avoids Rust regex's wider Unicode case behavior. Non-Unicode
mode uses the legacy uppercase relation, including its multi-character and
non-ASCII-to-ASCII restrictions; Unicode mode uses simple case folding.

Dynamic class compilation is bounded by a 128-entry cache. Only source keys
up to 512 bytes and normalized results up to 4096 bytes are cached. The legacy
closure path uses precomputed nontrivial equivalence groups, so a cache miss
does not scan all 65,536 BMP code units. A 100-pattern probe with distinct
600-byte classes completes in about 0.64 seconds including process startup.

Unicode Sets classes require a conservative split. Classes containing nested
sets, `&&`, `--`, `\p`, `\P`, or `\q` retain native fallback, while simple
`v` classes are materialized. Fallback is tracked per class and with explicit
nested depth: a complex class cannot disable normalization for a later simple
class, and a nested operand cannot escape fallback early. This preserves the
pre-existing backend behavior for complex set algebra without presenting it
as complete ECMAScript `v` semantics.

Plain-`i` `\b` and `\B` are lowered to the same ASCII inventory. Unicode
boundaries intentionally remain on Rust's native Unicode boundary. A first
implementation expressed the ECMAScript boundary with `fancy-regex`
lookarounds, but a nested quantified regression such as `^(a+)+\b$` could hit
the backend backtracking limit at runtime. Preserving the linear path is more
important than hiding the remaining mismatch: for example, `é` and CJK are
still treated as word characters by `\b` under `iu`/`iv`. A complete fix needs
a separate linear boundary representation.

Focused tests cover plain/Unicode/Unicode-Sets modes, complements and negated
classes, long s and Kelvin, unrelated Unicode exclusions, local modifiers,
Annex B ranges, escape parity, lone surrogates, supplementary input, nested
`v` fallback, class-order isolation, and the backtracking regression. A
curated differential matrix agrees with Node. On Test262
`020cb74075849d1e404bbcdb62feb7a02e6966db`, `regexp-modifiers` remains **70
pass / 0 fail**, complete `built-ins/RegExp` remains **960 pass / 63 fail /
856 skip / 0 timeout**, and the supported subset remains **12751 pass / 0
fail / 7687 skip / 20438 total**.

Final local gates pass all targets/features, warnings-denied Clippy,
formatting/diff, release, and wasm32. Python tooling is **106/106**, Rust
lib/unit **124/124**, bugfixes **67/67**, builtins **470/470**, and Fuel
**26/26**. GPT 5.6 reviewers Ohm and Boole found the unsafe lookaround path,
nested-`v` depth leaks, repeated BMP scans, and pattern-wide fallback. All
findings were fixed or explicitly deferred, Boole's final follow-up returned
`CLEAN`, and both agents were closed. No coder model or Umans provider route
was used. Feature SHA `844593bdb808c385200076f9f4242d8956f47080`
passed ordinary CI `29653243121` and full matrix `29653243102`. The 30 result
files at `/tmp/ruja-regexp-word-feature.WNQIpn` are byte-identical to corrected
baseline `29646302891` at
`/tmp/ruja-regexp-word-baseline.1D3kiS`. The aggregate therefore remains
**30383 pass / 6147 fail / 11931 skip / 6 timeout / 0 error / 48467 total /
36530 pass-or-fail executed**, or **62.7%** of all files and **83.2%** of
executed files. The unchanged matrix is expected because this unit corrects
word-set cases that are not newly admitted by the current Test262 manifest.

The remaining RegExp defects after this historical unit were intentionally
separate. Unicode `iu`/`iv` word boundaries, sentinel-backed UTF-16 scalar
collisions, nested `v` algebra, match indices, and ignore-case backreferences
were not changed here. Match indices and ignore-case backreferences are closed
by later units below; the other boundaries remain current work.

```text
[Decision Log]
- 목적과 의도: Implement active-ignoreCase word escapes as one coherent ECMAScript set while preserving UTF-16 behavior and bounded regex execution.
- 기존 구현 및 제약 조건: Rust regex applies a broader Unicode word inventory and case closure than ECMAScript, character-class negation must occur after canonicalization, local modifiers change the active relation, complex v classes need nested state, and the backreference backend can exhaust a finite backtracking limit.
- 검토한 주요 대안: Keep Rust native word semantics, special-case only U+017F/U+212A, enumerate every class manually, express Unicode boundaries with fancy-regex lookarounds, or replace the regex backend in this unit.
- 선택한 방식: Lower top-level escapes to exact scoped sets, parse ordinary classes through regex-syntax HIR, materialize the canonicalization closure before complement, bound and optimize class caching, retain depth-aware native fallback for complex v sets, and keep Unicode boundaries on the linear backend until a separate design exists.
- 다른 대안 대신 이 방식을 선택한 이유: Native case folding admits the wrong characters, point fixes break complements and mixed classes, ad hoc class parsing is unsafe, lookarounds introduced a runtime backtracking-limit regression, and replacing the backend is too broad for a verified compatibility unit.
- 장점, 단점 및 영향: Word escapes and simple classes now follow ECMAScript across i/iu/iv and local modifiers with bounded compile cost and no Test262 regression. Unicode boundaries, sentinel-range scalars, and full nested-v algebra remain explicit architectural work; ignore-case backreferences are completed by the later duplicate-name backend unit.
```

## RegExp match indices and named groups

Native RegExp exec now implements the ECMA-262
[`RegExpBuiltinExec`](https://tc39.es/ecma262/multipage/text-processing.html#sec-regexpbuiltinexec)
`d`-flag result contract. The match Array receives an own enumerable
`indices` property after `groups`; every participating capture receives a
method-Realm `[start, end]` Array; nonparticipating captures are explicit
`undefined`; and `indices.groups` is either `undefined` or a null-prototype
object. Named entries alias the exact pair objects already stored by numeric
capture index, including a data property named `__proto__`.

The implementation reads the immutable internal `[[RegExpHasIndices]]` bit,
not shadowable public `hasIndices` or `flags` properties. Backend byte
positions are converted to original JavaScript UTF-16 positions in one
left-to-right pass. Unicode matching over dynamically concatenated surrogate
pairs uses a normalized backend scalar view with an explicit byte-to-code-unit
boundary map, so match strings, `index`, `lastIndex`, named groups, and index
pairs all retain the original code-unit coordinates. A `u`/`v` `lastIndex`
inside a surrogate pair maps to the code point start, as it does for an
ordinary supplementary scalar.

Named capture declarations are decoded independently of backend syntax. Raw
and escaped Unicode names use the ECMAScript `ID_Start`/`ID_Continue` sets,
including `$`, `_`, ZWNJ, and ZWJ. Definitions lower to numbered captures and
named references lower through unambiguous noncapturing boundaries, preserving
the original public `source`. Unmatched, forward, and self references follow
ECMAScript's empty-match behavior; a following decimal digit cannot merge into
another capture number. Literal declarations and references are validated as
early errors. Legacy non-`u` `\k<name>` remains an identity escape only when
the pattern has no named capture, while malformed `\u` and `\u{n}` retain
their Annex B identity/quantifier meaning.

All pair Arrays and the null-prototype groups object are pinned across nested
allocation. Pair materialization consumes fuel per capture, and exact-cap
tests prove that the six retained objects for one matched and one unmatched
named capture succeed at the exact limit, fail one cell below it, and restore
pin depth. The shared capture-range conversion prevents an attacker-controlled
`capture_count * input_length` rescan.

The exact frozen admission in
`tools/test262_regexp_match_indices_admission.txt` lifts only the
`regexp-named-groups` dependency for seven audited match-indices files; future
and outside files retain their broad feature gate. On Test262
`020cb74075849d1e404bbcdb62feb7a02e6966db`, the complete
`built-ins/RegExp/match-indices` directory is **14 pass / 0 fail / 0 skip / 0
timeout**, `regexp-modifiers` remains **70/70**, the full RegExp diagnostic
improves from **960/63/856/0** to **978/52/849/0**, and the supported subset
remains **12751 pass / 0 fail / 7687 skip / 20438 total**. The RegExp delta is
exactly **+18 pass / -11 fail / -7 skip**; an isolated binary from the previous
HEAD passed none of the final 52 failing files, so no old RegExp pass regressed.

Final local gates pass all Rust targets/features, warnings-denied Clippy,
rustfmt/diff, release, wasm32, Python tooling **107/107**, Rust lib/unit
**126/126**, bugfixes **67/67**, builtins **473/473**, and Fuel **27/27**.
GPT 5.6 reviews found and closed digit-boundary merging, Fancy word-boundary
compatibility, legacy `\k` regression, incorrect XID-only names, dynamic
surrogate-pair indices, delayed literal errors, malformed non-`u` `\u`, and a
lookbehind compilation regression. All reviewer sessions were closed; no
coder model or Umans provider route was used. Feature commit `7ceb8e9` and
tooling portability fix `08f2fa4` are pushed. Ordinary CI `29657974089`
passes every build, Rust, formatting, Clippy, tooling, and supported-Test262
step.

Full matrix `29657718789` passes all 30 shards. Its downloaded artifacts at
`/tmp/ruja-regexp-indices-feature.29657718789` aggregate to **30404 pass /
6133 fail / 11924 skip / 6 timeout / 0 error / 48467 total / 36537
pass-or-fail executed**, or **62.7%** of all files and **83.2%** of executed
files. Twenty-eight result files are byte-identical to the prior RegExp
feature baseline. `built-ins` moves from **14784/5331/3547/6** to
**14802/5320/3540/6**, exactly the locally reproduced RegExp improvement.
`annexB` also moves **+3 pass / -3 fail**, but old and new binaries produce no
Annex B status changes against the same local Test262 checkout. That second
artifact difference is upstream checkout drift because each matrix job still
clones an unpinned Test262 HEAD independently; it is not attributed to this
engine change. The aggregate delta therefore combines the verified
`built-ins` improvement with unrelated Annex B corpus drift.

The remaining RegExp work is deliberately separate. The duplicate-name and
ignore-case backreference work described by this historical boundary is now
closed by the following unit. Unicode word boundaries on linear-backend
patterns, the reserved sentinel scalar range, nested `v` set algebra, and the
remaining lookbehind backend limitations retain their existing boundaries.

```text
[Decision Log]
- 목적과 의도: Implement complete d-flag match indices and the named-capture behavior required to expose them without weakening Realm, GC, UTF-16, fuel, or backend safety invariants.
- 기존 구현 및 제약 조건: Native exec already produced captures but discarded their ranges; backend offsets are UTF-8 bytes, JavaScript offsets are UTF-16 code units, split surrogate sentinels require a different Unicode matcher view, nested result allocation can collect unrooted pairs, and the linear backend cannot execute backreferences.
- 검토한 주요 대안: Re-run the pattern once per capture, derive indices from returned strings, expose backend byte offsets, move every pattern to fancy-regex, hand-roll a second matcher, or retain one match and convert all finalized capture boundaries together.
- 선택한 방식: Preserve finalized backend capture ranges, normalize only Unicode inputs containing split surrogate pairs with an explicit boundary map, convert all endpoints once, lower named groups to numbered backend captures, and materialize Realm-local rooted index Arrays under the internal d flag.
- 다른 대안 대신 이 방식을 선택한 이유: Re-matching changes observable capture state, string search is ambiguous and quadratic, byte offsets violate ECMAScript, broad fancy-regex routing weakens bounded linear execution, and a new matcher is too broad for this verified unit. One range pipeline reuses the existing successful match and keeps every coordinate conversion explicit.
- 장점, 단점 및 영향: Match indices, named aliases, descriptors, order, Realm identity, surrogate coordinates, exact heap caps, and fuel are directly tested; seven exact files are safely admitted and five pre-existing RegExp failures also close. Duplicate names and ignore-case backreferences are completed by the following unit; boundary, sentinel, nested-v, and lookbehind architecture remains explicit follow-up work.
```

## RegExp duplicate named captures

ECMAScript permits two capture groups to share a decoded name when
[`MightBothParticipate`](https://tc39.es/ecma262/multipage/text-processing.html#sec-patterns-static-semantics-mightbothparticipate)
is false. RuJa now applies that structural early error to literals and the
`RegExp` constructor: names in alternatives of one disjunction are compatible,
while a duplicate introduced by concatenated terms, a parent group, or the
same branch remains a syntax error. Raw and escaped spellings are compared
after decoding, so `x` and `\u0078` are the same name.

Every occurrence keeps its own numeric capture index while one name maps to an
ordered index set. `exec`, `match`, `matchAll`, `replace`, `replaceAll`,
`search`, `split`, and `test` select the sole participating index. `groups`
and `indices.groups` keep the property position established by the first
occurrence, replace an earlier `undefined` when a later alias participated,
and preserve identity with the selected numeric indices pair. Named
backreferences consume the participating capture or the empty string when no
alias participated.

The matcher changes are isolated in a vendored `fancy-regex` 0.18.0 fork.
Named references lower to `(?@set_id)` and the capture-index table is stored
once, so parser, AST, compiler, and VM growth is linear in source size.
Repeated capture slots are cleared on each iteration through backtracking-aware
copy-on-write state. A bitset removes the old quadratic save lookup and each
clear is charged to the backend work limit. Ordinary repeated-capture patterns
use a linear prefilter and invoke the capture backend only for successful
boundaries. Unicode ignore-case backreferences compare equal scalar counts
using simple folding; non-`u` mode uses the legacy uppercase relation.

`tools/test262_regexp_duplicate_named_groups_admission.txt` freezes exactly
**19** paths. Fifteen positive files lift only
`regexp-duplicate-named-groups`; four parse-negative literal files lift only
`regexp-named-groups`. The runner and analyzer share the same exact-path map,
and tooling checks the manifest against the live Test262 checkout when it is
available. On Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, the
admission is **19 pass / 0 fail / 0 skip**. Full `built-ins/RegExp` moves from
**978/52/849/0** to **991/52/836/0**, exactly **+13 pass / -13 skip** with no
failure or timeout increase. The supported subset remains **12751 pass / 0
fail / 7687 skip / 20438 total**.

Focused regressions cover nested alternatives, escaped equivalent names,
same-branch errors, forward/self/unmatched references, repeated aliases,
trailing backreference text, property order and identity, replacement and
split, Unicode `iu` folding, legacy `i`, backend mode isolation, invalid set
IDs, and backtracking restoration. Reviewer stress probes improve from about
**1.22 s / 271 MB** to **0.03 s / 14 MB** for 4,000 aliases and 4,000
references; 8,000 of each takes **0.06 s / 19 MB**. A repeated 6,400-capture
pattern drops from about **4.8 s** to a bounded work-limit error in **0.02 s**.

Final local gates pass Rust all targets/features, warnings-denied Clippy,
rustfmt/diff, release, wasm32, vendored tests, Python tooling **108/108**, the
frozen admission **19/19**, full RegExp **991/52/836/0**, and the supported
subset **12751/0/7687/20438**. Feature commit `48b8b78` passed ordinary CI
`29662790684` and every setup, build, shard, and summary job in full matrix
`29662790652`. Its 30 downloaded artifacts aggregate to **30423 pass / 6133
fail / 11905 skip / 6 timeout / 0 error / 48467 total / 36556 pass-or-fail
executed** (**62.8%** of all files, **83.2%** of executed files). Twenty-eight
artifacts are byte-identical to the match-indices baseline; `built-ins` changes
by exactly **+15 pass / -15 skip** and `language/literals` by **+4 / -4**,
matching the frozen 19-file admission with no failure or timeout change.

GPT 5.6 explorers Mendel
(`019f76d1-333c-79b1-8bb2-639107e9f971`) and Hilbert
(`019f76d1-33e3-7260-ad9f-544fde4a9ec5`) froze the specification/Test262
surface and identified in-matcher clearing as necessary. GPT 5.6 reviewers
Raman (`019f76fa-b61e-78a0-94eb-5f366d67a1b3`) and Sartre
(`019f76fa-b484-7c71-8df8-acd15ca18dae`) found the post-match, Unicode byte
length, mode isolation, quadratic resource, lookbehind, and packaging issues.
All correctness/resource findings are closed, every agent is closed, and no
coder or Umans route was used. The previous hard variable-length lookbehind
limitation for duplicate-name backreferences is closed by the directional
matching unit below. Crates.io publication is disabled until the fork is
upstreamed or published separately.

```text
[Decision Log]
- 목적과 의도: Admit the exact ECMAScript duplicate-named-capture cluster with correct syntax, result objects, backreferences, quantified state, and bounded resource behavior.
- 기존 구현 및 제약 조건: Named captures were represented one-to-one, all duplicates were rejected, Rust capture results retain stale iteration values, post-match cleanup cannot reconstruct matcher state, Unicode case-equivalent scalars may have different UTF-8 widths, and a path fork cannot be substituted by upstream during packaging.
- 검토한 주요 대안: Broaden the feature gate, expand every reference into conditional source, retain post-match cleanup, route all repeated captures through a backtracking matcher, build a new RegExp engine, or add narrowly gated state operations to the existing backend.
- 선택한 방식: Freeze 19 exact paths, share a structural MightBothParticipate scanner, store ordered occurrences plus ID-based capture sets, clear repeated captures inside the VM, use a linear match prefilter, implement ECMAScript case relations, and account all clearing work under the backend limit.
- 다른 대안 대신 이 방식을 선택한 이유: Broad admission hides unrelated failures, conditional/source and lookup expansion is quadratic, post-processing is incorrect, broad VM routing regresses hostile no-match input, and replacing the matcher is disproportionate to this finite conformance cluster.
- 장점, 단점 및 영향: Thirteen skips become passes with no failure increase; result identity/order and matcher state are observable and tested; large alias patterns scale linearly; mode-off upstream behavior is preserved. The maintained fork blocks crates.io publication until it has a registry path. Hard variable-lookbehind duplicate references are completed by the following directional unit.
```

## RegExp lookaround and directional matching

RuJa now implements the ECMA-262
[`CompileAssertion`](https://tc39.es/ecma262/multipage/text-processing.html#sec-compileassertion)
and backward `CompileSubpattern` model directly in the vendored matcher.
Lookahead compiles forward and lookbehind compiles backward. Backward
concatenation reverses term execution while preserving alternative order and
greediness; captures save end then start; literals, wildcards, general
newlines, delegates, ordinary backreferences, and duplicate-name capture sets
all have backward execution paths.

Positive assertions run inside an atomic region, restore the outer input
cursor, and retain captures from the successful assertion. Negative
assertions use a transactional failure branch, so failed probes do not leak
captures or repeat state. This closes variable-length lookbehind, backward
capture greediness, both source orders of lookbehind backreferences, nested
lookahead/backreference behavior, and the hard duplicate-name lookbehind case
left by the preceding unit. Unmatched references match the empty string only
under ECMAScript mode; ordinary `fancy-regex` semantics are unchanged.

Annex B quantified lookahead is accepted only when ECMAScript mode is active
without `u` or `v`. The repeat instructions carry finite upper bounds and an
empty-iteration failure mode that implements the legacy `RepeatMatcher`
behavior without losing required captures. The ECMAScript path also skips the
trailing positive-lookahead optimizer because removing the assertion can lose
locally normalized case flags.

The backend limit is a work limit rather than only a failed-backtrack count.
Every ECMAScript branch push, repeat dispatch, and capture clear shares that
budget, and the branch stack has a 100,000-entry hard cap. A 100-million
successful zero-width-repeat probe terminates at the work limit in about
**0.26 s / 14 MB** in reviewer verification. A path that formerly grew toward
one million branch entries terminates at the stack limit in about **0.18 s /
27 MB**. Catastrophic failed matching remains work-bounded, exact limit edges
were exercised, and mode-off low-limit behavior remains upstream-compatible.

On the pinned local Test262 checkout
`020cb74075849d1e404bbcdb62feb7a02e6966db`, `built-ins/RegExp/lookBehind` is
**17 pass / 0 fail / 0 skip / 0 timeout**. Full `built-ins/RegExp` moves from
**991/52/836/0** to **1024/19/836/0**, exactly **+33 pass / -33 fail** with no
skip or timeout movement. A fresh analyzer independently reproduces all 19
remaining failures: **11** legacy grammar early errors, **5** valid empty-class
matcher files rejected by the backend, **1** quantifier integer-limit file,
**1** nullable-quantifier capture-prefilter disagreement, and **1** Unicode
restricted-bracket early error.

Final local gates pass all Rust targets/features, warnings-denied Clippy,
rustfmt/diff, release, wasm32, vendored `fancy-regex` tests and doc-tests
**447/447**, builtins **476/476**, the complete lookbehind subtree, and full
RegExp. A 33-case Node/RuJa differential matrix is identical. GPT 5.6
reviewers Kant (`019f777e-5ea9-7ee0-9e68-fe0bfd004d6a`) and Carver
(`019f777e-8420-78d2-b752-492fdde16609`) independently returned `CLEAN` after
closing legacy non-ASCII case-folding, Unicode over-folding, nested assertion
backreferences, scoped trailing-lookahead flags, successful zero-width work
accounting, and branch-stack growth. Both sessions are closed; no duplicate
agent, coder model, or Umans provider route remains. Feature commit `f1e48f1`
passed ordinary CI `29666307842` and all **33/33** jobs in full matrix
`29666307826`.

The 30 downloaded matrix result files at
`/tmp/ruja-regexp-lookaround-feature.29666307826.EgEfvX` aggregate to **30458
pass / 6098 fail / 11905 skip / 6 timeout / 0 error / 48467 total / 36556
pass-or-fail executed**, or **62.8%** of all files and **83.3%** of executed
files. Twenty-eight files are byte-identical to duplicate-name baseline
`29662790652` at
`/tmp/ruja-regexp-duplicate-feature.29662790652.Uoyktr`. `built-ins` changes
by exactly **+33 pass / -33 fail**. Annex B changes by **+2 / -2**; an isolated
old/new binary comparison on the same Test262 checkout
`9e61c12835c5e4a3bdba93850427e6742c4f64c4` identifies exactly
`quantifiable-assertion-followed-by.js` and
`quantifiable-assertion-not-followed-by.js`. The extra matrix delta is
therefore the expected Annex B implementation, not unpinned-checkout drift.

```text
[Decision Log]
- 목적과 의도: Close ECMAScript lookaround and legacy quantified-lookahead semantics with specification-directed captures, backreferences, atomicity, and finite resources.
- 기존 구현 및 제약 조건: Rust regex lacks variable-length lookbehind and assertion capture semantics, upstream lookbehind searches prefixes forward, failed-backtrack counting misses successful zero-width work, and RuJa must preserve the linear backend for patterns that do not need assertions.
- 검토한 주요 대안: Enumerate lookbehind start positions, rewrite lookbehind into lookahead, repair captures after matching, route every RegExp through a backtracking engine, replace the matcher, or add explicit direction and accounting to the maintained fork.
- 선택한 방식: Route only assertion patterns to ECMAScript mode, compile lookbehind terms backward, make successful assertions atomic with cursor restoration, model Annex B RepeatMatcher in parser/compiler/VM state, and charge all ECMAScript branches, repeats, and capture clears under one work budget plus a 100,000-entry stack cap.
- 다른 대안 대신 이 방식을 선택한 이유: Start enumeration changes greediness and scales with input length, rewrites and post-processing cannot reproduce transactional capture state, broad routing weakens linear execution, and a replacement engine is disproportionate to this finite unit. Directional compilation maps directly onto the specification while retaining existing VM rollback machinery.
- 장점, 단점 및 영향: Thirty-three failures become passes, lookbehind reaches 17/17, hard duplicate-name lookbehind closes, and successful hostile paths are bounded. The maintained fork grows, while 19 unrelated grammar, class, quantifier, and hybrid-boundary failures remain explicit next units.
```

## RegExp grammar early errors

RuJa now validates the ECMA-262
[`Quantifier`](https://tc39.es/ecma262/multipage/text-processing.html#prod-Quantifier)
shape as one prefix plus one optional lazy marker. Repeated simple or braced
quantifiers, quantifiers after assertions, and malformed escapes that hide a
following prefix are rejected before backend compilation. The
[Annex B pattern extensions](https://tc39.es/ecma262/multipage/additional-ecmascript-features-for-web-browsers.html#sec-regular-expressions-patterns)
remain active only in legacy mode, including quantifiable lookahead and
character-set range endpoints.

Range validation follows the active pattern mode. Legacy classes are flattened
to UTF-16 code units and decode Annex B octal/control forms, incomplete `\c`,
raw supplementary characters, and non-ASCII identity escapes. Unicode classes
compare scalar endpoints and combine adjacent surrogate escapes; `v` classes
also require subtraction operands and balanced nested classes. Unicode modes
reject malformed `\xHH`, standalone unescaped `]`/`}`, and character-set range
endpoints.

On pinned Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, full
`built-ins/RegExp` moves from **1024 pass / 19 fail / 836 skip / 0 timeout** to
**1036 / 7 / 836 / 0**, exactly **+12 pass / -12 fail**. The remaining seven
are five empty-class backend failures, `quantifier-integer-limit.js`, and
`nullable-quantifier.js`. Local gates pass all Rust targets/features,
warnings-denied Clippy, rustfmt/diff, release, wasm32, builtins **477/477**, and
the full RegExp diagnostic. Differential checks cover **1,219** legacy class
combinations with **569** prior mismatches fixed and no remaining mismatch,
plus **858** quantifier/escape combinations with no regression. Two independent
GPT 5.6 reviews returned `CLEAN` after their UTF-16, octal/control,
malformed-escape, surrogate, and `v` subtraction findings were fixed; every
agent was closed and no coder model or Umans route was used.

Feature commit `8578ea2` passed ordinary CI `29669380090` and all **33/33**
jobs in full matrix `29669380082`. The 30 downloaded result artifacts at
`/tmp/ruja-regexp-grammar-feature.29669380082.uQLPyV` aggregate to **30470 pass
/ 6086 fail / 11905 skip / 6 timeout / 0 error / 48467 total / 36556
pass-or-fail executed** (**62.9%** of all files, **83.4%** of executed files).
Twenty-nine artifacts are byte-identical to lookaround baseline
`29666307826`; only `built-ins` changes, by exactly **+12 pass / -12 fail**.
The matrix therefore reproduces the pinned local delta without corpus drift or
movement in skip, timeout, total, or executed counts.

## Array callback GC rooting

Native `Array.prototype.map` and `flatMap` keep source values and callback
results in Rust collections while invoking later callbacks. Those collections
are not part of the VM root set. RuJa now pins the source snapshot before the
first callback, pins every fresh result before the next callback can collect,
and releases the complete root suffix only after the destination Array owns
the values. `Array.of` similarly roots its arguments, constructor, and returned
object across custom `defineProperty` traps and the final observable `length`
write.

Forced-GC regressions make heap-cell reuse deterministic. Before the fix,
`map` and `flatMap` both changed the expected `"1,2"` result to `"2,2"`, and a
custom `Array.of` Proxy lost the first property after the wrapper cell was
reused. The original 5,652-case accumulating RegExp differential instead
reused one stale result as an Environment and panicked with `env has no props`.
Callback-throw and final-allocation-failure tests now also assert exact
`gc_pins` restoration and successful VM reuse.

On pinned Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, the supported
subset remains **12751 pass / 0 fail / 7687 skip / 20438 total**. The 256 files
under `built-ins/Array/of`, `built-ins/Array/prototype/map`, and
`built-ins/Array/prototype/flatMap` remain **117 pass / 118 fail / 20 skip / 1
timeout**, with zero per-file status changes against the preceding grammar
binary. The unchanged failures are broader Array conformance work, not a
root-lifetime regression. Local gates pass all Rust targets/features,
builtins **480/480**, lib tests **129/129**, warnings-denied Clippy,
rustfmt/diff, release, wasm32, and Python tooling **108/108**. GPT 5.6
reviewers Curie (`019f783f-f39c-78f2-952b-f514cbbbcef7`) and Hooke
(`019f783f-f4da-7cd1-b3ee-f752f7aeff89`) returned `CLEAN` and were closed.
Feature commit `6f822ce` passed ordinary CI `29671480301` and all **33/33**
jobs in full matrix `29671480315`.

The 30 downloaded matrix artifacts at
`/tmp/ruja-array-gc-feature.29671480315.01KzGH` aggregate to **30470 pass /
6086 fail / 11905 skip / 6 timeout / 0 error / 48467 total / 36556
pass-or-fail executed** (**62.9%** of all files, **83.4%** of executed files).
Every artifact is byte-identical to RegExp grammar baseline `29669380082`, so
the root-lifetime fix changes no Test262 status in any shard.

## Why the full-suite rate is not higher

The supported subset currently has no known failures. The full-suite rate is
still much lower because the full matrix includes unsupported features such as
Intl, async iterator helpers, remaining RegExp semantics, and tail-call
optimization. Those larger feature areas are tracked
in `HANDOFF.md` and will be pulled into support in later milestones.
