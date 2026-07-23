# Architecture

RuJa is a self-contained JavaScript engine with no external runtime
dependencies. Source flows through four stages before execution:

```
source ─► Lexer ─► Parser ─► Compiler ─► Bytecode ─► VM
                              │              │
                              └─ AST         └─ Op stream
```

## Pipeline

- **Lexer** (`src/lexer.rs`) — tokenization with automatic semicolon insertion
  (ASI) and template literal support.
- **Parser** (`src/parser.rs`) — Pratt-style recursive descent producing an AST.
  Expression nesting is depth-capped to prevent stack overflow on untrusted input.
- **Compiler** (`src/compiler.rs`) — single-pass AST → bytecode compilation
  with lexical scope resolution, hoisting, and TDZ tracking.
- **Bytecode** (`src/bytecode.rs`) — a stack-machine instruction set (`Op`).
- **VM** (`src/vm/mod.rs`, `src/vm/ops.rs`) — the dispatch loop: call frames,
  operand stack, property access, type coercion, and non-local control flow.
  `ops.rs` holds the opcode dispatch and immediate helpers; `mod.rs` holds
  the `Vm` struct, public API, and runtime helpers. Identifier, property,
  private-name, and `super` expression evaluation is represented by rooted
  `ReferenceRecord` values and resolved through shared `GetValue`, `PutValue`,
  call, and delete operations; compiler-internal switch completion reads use
  the same path.
- **GC** (`src/gc.rs`) — mark-and-sweep collector that traces from VM roots.
- **Values** (`src/value.rs`) — the `HeapObj` enum
  (Object/Array/Function/Environment/Map/Set/Promise/Generator) referenced by
  `GcIdx` handles.
- **Builtins** (`src/builtins/mod.rs` + submodules) — the standard library:
  Object, Array, String, Number, Boolean, Function, Math, JSON, console, RegExp,
  Map, Set, Symbol, Promise, Proxy, TypedArray, and the Error hierarchy.

## Garbage collection

A mark-and-sweep collector with optional incremental marking reclaims
reference cycles. Collection runs at safe points only (after a run settles,
and throttled at frame boundaries). Incremental marking via
`collect_incremental(roots, budget)` allows limiting the number of cells
marked per GC step, avoiding long pauses. There is no generational collector.
accumulate memory before a collection. A `gc_pins` stack lets call paths pin
heap values held in Rust locals across allocations that could trigger a GC.

Native functions carry `Option<NativeConstructMode>` metadata instead of
deriving constructibility from an observable `.prototype` slot. `None` means
the function has no `[[Construct]]` internal method.
`Some(InternalEagerPrototype)` lets an internal allocator run after dispatch
has observed `NewTarget.prototype`, while
`Some(InternalDeferredPrototype)` gives the native body ownership of whether
and when that lookup occurs. The obsolete generic native-receiver
preallocation mode has no remaining user and was removed. Registration tests
inventory constructors and ordinary native functions in both the main and
created Realms so callability, constructibility, and allocation policy cannot
silently collapse together.

BigInt and Symbol intentionally have `[[Construct]]`: they can participate in
class heritage and serve as a `newTarget`, but their bodies throw before
coercing arguments. Proxy and the abstract `%TypedArray%` constructor also use
the body-controlled mode because dispatch must not observe
`NewTarget.prototype` before their own validation. Each created Realm installs
its own Proxy constructor and `revocable`; the result pair, revoker, and
construct-trap argument array are allocated with that operation Realm's
intrinsics. Exact-cap construction pins every provisional Proxy value before a
collecting allocation.

Bound Functions and Proxies store immutable `constructable` metadata when
their exotic object is created. `is_constructor_value` therefore implements
`IsConstructor` as a constant-time `[[Construct]]` capability read: it neither
walks a target chain, checks Proxy revocation, nor consumes fuel.

`constructor_realm` and `construct_with_new_target` share one constructor-step
classifier for the operations that do follow targets. A Bound or live Proxy
edge consumes one fuel unit immediately before traversal. Proxy revocation is
validated before that charge and before observable `construct` lookup. Each
followed wrapper, Proxy target, handler, trap, and argument array belongs to
the outer construction pin scope, so normal, thrown, allocation, and host-abort
exits restore the incoming pin depth even if a trap getter revokes its Proxy.

Construction records Bound wrapper IDs outer-to-inner and materializes the
argument list once in reverse wrapper order, followed by the original call
arguments. This preserves `innerArgs, outerArgs, callArgs` and per-wrapper
`newTarget` substitution while making copying linear in wrappers plus values.
The combined argument list shares the 1,048,576-entry call-argument sandbox
cap. Normal and spread `super()`, Reflect, species constructors, and native
constructor dispatch all use this path.

Eager native construction records either the already-observed object
prototype or the already-resolved fallback Realm in `NewTargetPrototype`.
Native bodies reuse that state, so non-object fallback does not repeat
`GetFunctionRealm`; both variants are GC roots in pending and active execution
contexts. Ordinary interpreted receiver allocation now uses the collecting VM
allocator while pinning its resolved prototype across a heap-cap retry.

```text
[Decision Log]
- 목적과 의도: Bound and Proxy constructor operations must preserve ECMAScript capability, Realm, argument, trap, and newTarget semantics while remaining stack-safe, fuel-bounded, linear, and GC-safe.
- 기존 구현 및 제약 조건: Bound IsConstructor walked targets, Realm and Construct edges were unmetered, every Bound layer recopied the accumulated argument list, eager native fallback repeated Realm traversal, and Proxy traversal roots depended on retained revoked slots.
- 검토한 주요 대안: Meter the existing IsConstructor walk, impose a wrapper-depth cap, keep repeated argument prepending, flatten each time a Proxy is reached, or cache immutable capability and carry one argument/prototype plan through the shared dispatcher.
- 선택한 방식: Cache Bound constructability at creation, keep IsConstructor constant-time, meter only GetFunctionRealm and actual Construct edges after revocation validation, collect wrapper IDs and flatten once, and pass observed-prototype or fallback-Realm state into native execution.
- 다른 대안 대신 이 방식을 선택한 이유: IsConstructor does not recursively inspect targets in ECMAScript; fixed depth caps reject legal programs; repeated or intermediate flattening remains quadratic; and recomputing fallback Realm changes exact fuel and can reorder errors relative to native constructor validation.
- 장점, 단점 및 영향: Deep legal chains remain stack-safe and now have exact host work bounds, 4,096 one-argument layers preserve order with linear materialization, intrinsic Promise settlement retains staged work after Fuel without replaying completed handlers or then access, and all construction callers share one policy. Arbitrary species capability functions are not replayed after a host abort, unbounded hosts can still spend linear time, and temporary wrapper/root vectors remain native memory subject to the broader process-memory policy.
```

Deep Proxy property forwarding uses the same stack-safety rule. Transparent
`get` chains are iterative and do not consume ordinary prototype depth.
`getOwnPropertyDescriptor` stores rooted target/trap-result pairs while
descending and validates them from the ordinary target outward;
`isExtensible` similarly collects trap booleans and checks invariants in
reverse. Short-lived roots are removed before pending roots are installed so
the LIFO pin stack cannot discard a fresh descriptor result. This trades
`O(depth)` temporary host memory for bounded Rust call-stack use and exact GC
liveness at depths exercised up to 100,000 wrappers.

```text
[Decision Log]
- 목적과 의도: Separate native `[[Construct]]` presence from receiver allocation while making wrapper forwarding stack-safe and GC-safe at adversarial depth.
- 기존 구현 및 제약 조건: Native constructibility depended on a mutable prototype slot, Proxy was shared across created Realms, `super()` used `[[Call]]`, and recursive Bound/Proxy/property forwarding could overflow the Rust stack or lose temporary values during collection.
- 검토한 주요 대안: Keep prototype-presence inference, add constructor-name exceptions, reject BigInt and Symbol at IsConstructor time, cap wrapper depth, or model constructibility explicitly and flatten the abstract-operation traversals.
- 선택한 방식: Store `Option<NativeConstructMode>`, let body-controlled constructors reject or validate in spec order, route all construction including `super()` through one iterative dispatcher, and reverse-validate rooted Proxy trap results.
- 다른 대안 대신 이 방식을 선택한 이유: Observable properties cannot represent internal methods, a depth cap changes valid JavaScript behavior, and per-builtin forwarding would duplicate new-target and GC rules. Explicit metadata plus iterative traversal preserves the abstract operation without using host recursion.
- 장점, 단점 및 영향: BigInt, Symbol, Proxy, BoundFunction, Realm, and exact-cap behavior now share one testable contract; 100,000-layer Proxy operations no longer abort the process. Iterative descriptor validation retains `O(depth)` pending state, while family-specific fallback and coercion order is handled by the constructor units below.
```

String, Number, and Boolean use `InternalDeferredPrototype` because their
constructor algorithms must finish primitive conversion before any observable
`NewTarget.prototype` read. Their native bodies distinguish calls from
construction through the active execution context's `NewTarget`, never through
the supplied `this`. A call therefore returns a primitive even when invoked
with `Function.prototype.call` and an object receiver. Construction selects the
wrapper-specific default from the existing GC-rooted
`realm_primitive_prototypes` registry after following BoundFunction and Proxy
new targets to their function Realm. It then pins the selected prototype while
the common sandbox allocator creates one ordinary object and initializes its
wrapped primitive slot. String adds its immutable UTF-16 `length` property
after allocation.

```text
[Decision Log]
- 목적과 의도: Preserve the specification's primitive-conversion, prototype-selection, and allocation order for String, Number, and Boolean in every Realm.
- 기존 구현 및 제약 조건: Generic preallocation read `NewTarget.prototype` too early, hardcoded `%Object.prototype%` as the fallback, and treated an object `this` as proof of construction. Wrapper allocation must still honor GC rooting and the exact heap cap.
- 검토한 주요 대안: Add wrapper-name exceptions to generic dispatch, add three more eager allocation modes, or let this family own conversion, prototype lookup, and allocation in its existing native bodies.
- 선택한 방식: Use `InternalDeferredPrototype`, select immutable Realm primitive prototypes from the VM registry, and share one rooted wrapper-allocation helper across the three bodies.
- 다른 대안 대신 이 방식을 선택한 이유: The three algorithms all convert before `GetPrototypeFromConstructor`, while String also has call-only Symbol behavior. Dispatcher exceptions would duplicate builtin semantics and still conflate calls with construction.
- 장점, 단점 및 영향: Observable order, foreign-Realm fallback, Bound/Proxy forwarding, direct calls, and cap failures share one tested path. The helper is deliberately limited to primitive wrappers; Date uses the separate body-controlled path below.
```

Date also uses `InternalDeferredPrototype`, but it has a distinct call branch.
Calls return a current date String without coercing supplied arguments or
branding an object `this`. Construction copies a Date input or converts the
single/component arguments, applies `TimeClip`, and only then observes
`NewTarget.prototype`; abrupt conversion therefore prevents the lookup. A
non-object prototype selects the immutable Date prototype from the new
target's function Realm through the GC-rooted `realm_date_prototypes` map.

Each Realm installs its own Date constructor, prototype, prototype methods,
and static functions. `%Date.prototype%` is an ordinary unbranded object, and
constructed instances store `[[DateValue]]` in an internal private slot rather
than an observable property. The selected prototype is pinned while the
common sandbox allocator creates exactly one object, so a cap-triggered
collection cannot reclaim it and a saturated heap still uses the existing
Realm-local emergency `RangeError` path.

```text
[Decision Log]
- 목적과 의도: Preserve Date's call-versus-construct split, conversion/prototype order, hidden DateValue, Realm fallback, and exact sandbox allocation contract.
- 기존 구현 및 제약 조건: Generic receiver preallocation read `NewTarget.prototype` before Date conversion, treated an object `this` as construction, exposed Date state through `__time__`, and reused main-Realm Date intrinsics in created Realms.
- 검토한 주요 대안: Keep generic preallocation with Date-specific repair steps, reuse the primitive-wrapper allocator, or let the Date body own conversion, prototype selection, slot initialization, and allocation.
- 선택한 방식: Use `InternalDeferredPrototype`, install a complete Date intrinsic graph per Realm, resolve immutable Date fallbacks through a traced registry, and allocate one internally branded object after all observable conversion.
- 다른 대안 대신 이 방식을 선택한 이유: Date calls do not wrap a primitive and construction has Date-copy, parsing, component, and TimeClip branches. Reusing preallocation or the primitive helper would preserve the wrong order or misrepresent the internal slot.
- 장점, 단점 및 영향: Direct/call/apply/bound use, subclasses, foreign new targets, abrupt order, forced GC, and exact-cap failure now share one tested path. The Realm registry inventory grows from 28 to 29 families and remains manually synchronized across storage, tracing, rollback, and tests.
```

The four Dynamic Function constructors use `InternalDeferredPrototype` so
CreateDynamicFunction owns every observable step. At that milestone the
registration inventory was **14 eager / 31 deferred** native constructors.
Parameter arguments are converted left-to-right before the body, after which
RuJa's local-trust `HostEnsureCanCompileStrings` policy permits compilation.
Parameters and body are parsed separately with line terminators at both synthetic boundaries; a
third combined parse enforces cross-part early errors such as a strict body
with non-simple parameters. This prevents comments or delimiter text in one
part from consuming the other part while preserving the specification's
conversion-before-parse order.

A call treats the active native constructor as the effective new target, while
`Reflect.construct` and `new` retain the supplied `NewTarget`. The active
constructor's closure selects the generated function Realm. If
`NewTarget.prototype` is not an object, the fallback comes from the immutable
Realm function-prototype registries for `%Function%`, `%AsyncFunction%`,
`%GeneratorFunction%`, or `%AsyncGeneratorFunction%`; mutable global bindings
are not consulted. Ordinary generated functions create a prototype whose
parent is that Realm's `%Object.prototype%`, while generator families use the
corresponding generator prototype.

Compilation remains side-effect free until the observable prototype lookup
has completed. Nested compiled definitions are appended afterward, and a
failed allocation truncates only that outer suffix so compilation performed
re-entrantly by a prototype getter remains valid. Async functions allocate one
function cell; the other three kinds allocate a function plus their own
prototype through the GC-aware sandbox allocator. Every intermediate and
getter-produced prototype is pinned across a collecting allocation.

BoundFunctionCreate now obtains the target's real `[[Prototype]]`, including a
Proxy `getPrototypeOf` trap, instead of hardcoding the main Realm
`%Function.prototype%`. Proxy trap results are pinned before re-entrant
non-extensible-target invariant checks, and the selected result stays pinned
through bound-function allocation.

The allocated Bound Function is then rooted before its observable metadata
steps. `length` uses `HasOwnProperty` semantics through the target's
`[[GetOwnProperty]]`; only an own property triggers `Get(target, "length")`,
and only a Number value participates in truncation and bound-argument
subtraction. `name` is always read with `Get`, accepts only a String value, and
is prefixed with `"bound "`. The resulting own properties are configurable,
non-writable, non-enumerable data properties inserted in `length`, `name`
order. Because these are real properties, deleting bound `name` resumes
ordinary prototype lookup; the internal FunctionData name is not exposed as a
replacement exotic property.

```text
[Decision Log]
- 목적과 의도: Make BoundFunctionCreate expose specification-shaped name and length metadata without weakening observable order, Proxy semantics, Realm behavior, or sandbox GC limits.
- 기존 구현 및 제약 조건: Bound functions exposed a synthetic internal name and had no real own metadata descriptors; adding the required target getters also required the newly allocated wrapper and captured state to survive re-entrant collection.
- 검토한 주요 대안: Synthesize metadata in generic property lookup, eagerly copy target fields without abstract operations, coerce every target length, or install properties before allocating the bound object.
- 선택한 방식: Allocate and pin the Bound Function first, run exact HasOwnProperty/Get steps against the live target, compute length only for Number values, install ordered configurable data descriptors, and suppress the internal-name fallback only for Bound functions after deletion.
- 다른 대안 대신 이 방식을 선택한 이유: Synthetic lookup cannot model deletion or descriptors; direct field reads bypass Proxies and accessors; ToNumber is forbidden here; and observable metadata work before allocation would not match BoundFunctionCreate and would leave no wrapper root for captured state.
- 장점, 단점 및 영향: Exact metadata, abrupt completion identity, inherited-name behavior, forced GC, and exact-cap allocation now share one path. Ordinary and internal native functions retain their existing fallback behavior. Bound call-chain iteration, fuel, and argument materialization remain a separate dispatch concern.
```

Ordinary Bound Function `[[Call]]` and Proxy apply forwarding now use one
iterative call traversal. Each Bound or Proxy edge consumes fuel before it is
followed. Bound wrapper IDs are retained outer-to-inner, then their arguments
are materialized once in reverse wrapper order before the original call
arguments. This preserves `innerArgs, outerArgs, callArgs` and the innermost
bound `this` without repeated vector prepending.

A Proxy apply boundary materializes pending Bound arguments exactly once,
creates the trap argument Array with the current operation Realm's intrinsic,
and resets traversal so a Bound apply trap can itself be dispatched normally.
The cumulative 1,048,576-entry argument limit is checked before the apply
getter runs. Entry arguments, current targets, Bound wrappers, handlers,
traps, and arrays are rooted for every observable operation. Root-vector and
trap-call reservations are fallible, and every normal, thrown, allocation, or
host-abort exit restores the incoming pin depth.

The shared Realm-aware value-Array allocator computes the exact roots for its
items and prototype and reserves that capacity before publishing either. This
last reservation was added in follow-up commit `c64076f` after documentation
review found that the original call dispatcher still reached an infallible
`pin_many` inside trap-array construction. A test-only one-shot reservation
failure exercises the real helper and proves that it returns a catchable
`RangeError` without changing pin depth.

```text
[Decision Log]
- 목적과 의도: Make legal deep Bound calls stack-safe and linear while preserving exact Call, Proxy, Realm, fuel, argument-limit, and GC behavior.
- 기존 구현 및 제약 조건: Ordinary Bound calls recursed once per wrapper, prepended the complete argument vector at every layer, charged no traversal fuel, checked only the original call arguments, and depended on infallible native vector growth around observable Proxy work.
- 검토한 주요 대안: Raise the call-stack cap, impose a Bound-depth limit, flatten every time a Proxy is reached, retain recursive dispatch with a guard, or carry one rooted traversal and materialization plan through Bound and Proxy edges.
- 선택한 방식: Traverse Bound and Proxy call edges iteratively, meter each edge, retain wrapper IDs, materialize once at a Proxy trap or final target, enforce the cumulative cap before trap lookup, and reserve every native root or trap-call vector fallibly before mutation.
- 다른 대안 대신 이 방식을 선택한 이유: Fixed depth limits reject valid ECMAScript; recursive guards still consume the Rust stack; intermediate flattening remains quadratic; and infallible growth can abort the host instead of producing the sandbox's catchable RangeError.
- 장점, 단점 및 영향: A 20,000-layer ordinary call is stack-safe, argument work is linear, Bound apply traps and foreign Realms preserve identity, exact fuel bounds wrapper traversal, and all tested abrupt paths restore roots. The shared value-Array path now turns root-capacity failure into RangeError before publishing roots. Unbounded hosts can still request linear work across arbitrarily deep legal chains. Bound forwarding in OrdinaryHasInstance was intentionally left as a separate state machine at this boundary and is completed by the iterative instanceof unit below.
```

```text
[Decision Log]
- 목적과 의도: Implement one specification-shaped CreateDynamicFunction path for all four dynamic constructors without weakening Realm identity or the exact heap cap.
- 기존 구현 및 제약 조건: The old wrapper parse allowed synthetic-boundary ambiguity, prototype selection happened on incomplete rules, generated functions used main-Realm parents, raw allocation bypassed the sandbox retry, and compilation-table entries could leak after failure.
- 검토한 주요 대안: Keep four mostly separate constructors, broaden generic native preallocation, parse only one combined wrapper, or centralize conversion, parsing, Realm fallback, publication, and allocation in the existing dynamic body.
- 선택한 방식: Use deferred native construction, three grammar checks with newline boundaries, immutable constructor-Realm registries, post-lookup table publication, and rooted one- or two-cell allocation with suffix rollback.
- 다른 대안 대신 이 방식을 선택한 이유: Generic preallocation observes `NewTarget.prototype` too early, a combined-only parse does not model separate parameter/body grammar, and per-kind copies would drift on ordering and GC cleanup. One kind-parameterized path keeps the shared abstract operation explicit.
- 장점, 단점 및 영향: Call/construct order, all four Realm fallbacks, Bound/Proxy new targets, parser early errors, forced GC, and exact-cap failures now share tested invariants. String compilation remains synchronous and is governed by the local-trust host policy rather than opcode fuel; source-text preservation is a separate limitation.
```

RegExp construction also uses `InternalDeferredPrototype`, bringing the
current native-constructor inventory to **13 eager / 32 deferred**. The
constructor first classifies its pattern through the shared specification
`IsRegExp` operation. An explicit internal `[[RegExpMatcher]]` marker provides
the fallback brand only when an observable `Symbol.match` value is absent.
Calls take the identity shortcut only when the pattern is RegExp, flags are
absent, and the pattern's constructor is the active constructor. Otherwise the
algorithm selects copied internal source/flags or ordered regexp-like property
gets, resolves the actual new target's prototype and Realm fallback, allocates
one matcher object, and only then performs source/flags string conversion and
initialization.

Each Realm now retains immutable `%RegExp%`, `%RegExp.prototype%`, and
`%RegExpStringIteratorPrototype%` identities. These add two registry families
because the RegExp prototype map already existed for literals; the complete
transactional inventory is now **31**. Literal creation, `RegExpCreate`,
species fallback, and `@@matchAll` use those maps rather than mutable global
bindings. The RegExp String Iterator and each match result use the method's
Realm, while species values, flags, lastIndex values, matcher state, and
iterator state are pinned across every re-entrant conversion, trap, call, and
allocation.

Native `RegExpBuiltinExec` now treats backend byte positions as an internal
transport detail. `RegExpBackendInput` either borrows the original internal
string or owns a normalized matcher view plus a sorted backend-byte to
original-UTF-16 boundary table. The owned path is required when a JavaScript
string contains adjacent sentinel-backed high and low surrogates under `u` or
`v`: the backend must see one scalar, while `lastIndex`, match `index`, capture
strings, and `d`-flag ranges must remain measured in the original two code
units. A Unicode `lastIndex` inside that pair maps to the code point's starting
boundary, matching `GetStringIndex` behavior.

After matching, all capture endpoints are converted in one ordered pass and
reused for result strings, named `groups`, `lastIndex`, and match indices.
With the internal `[[RegExpHasIndices]]` flag set, exec allocates one
method-Realm Array per participating capture, explicit `undefined` entries for
nonparticipating captures, and a null-prototype `indices.groups` object whose
named properties alias the same pair objects. Pair arrays and groups are
pinned through nested allocation, materialization consumes fuel per capture,
and exact heap-cap failure restores the original pin depth.

The same work made the write pipeline receiver-aware end to end. OrdinarySet
stops at the nearest data descriptor, delegates through Proxy prototypes, and
preserves the original receiver. Proxy `set`, `getOwnPropertyDescriptor`,
`defineProperty`, `has`, and `isExtensible` invariant checks support nested
Proxies and root fresh trap results. CreateDataProperty and value-only
DefineProperty retain Array, integer-indexed TypedArray, and mapped-arguments
exotic behavior. ArraySetLength owns its two observable conversions, sparse
length calculation, non-writable guards, descending deletion and rollback,
descriptor synchronization, and inline-cache invalidation. An unmaterialized
Array `length` is synthesized as an own non-configurable data descriptor before
prototype traversal.

```text
[Decision Log]
- 목적과 의도: Make RegExp construction, RegExp String iteration, and their observable property writes follow ECMAScript ordering, Realm identity, Proxy invariants, and the exact sandbox heap contract.
- 기존 구현 및 제약 조건: RegExp identity depended on an observable class-name approximation, generic preallocation selected prototypes too early, mutable globals supplied Realm fallbacks, matchAll intermediates were not uniformly rooted, and partial property helpers bypassed Proxy and Array/TypedArray/arguments exotic methods.
- 검토한 주요 대안: Patch individual Test262 failures, retain eager allocation with RegExp exceptions, broaden feature admission, or centralize the abstract operations and exotic dispatch before admitting exact files.
- 선택한 방식: Use deferred RegExp allocation with an internal matcher marker and immutable Realm registries; route matchAll through ordered species and strict Set operations; and share receiver-aware Set/DefineProperty dispatchers with a complete ArraySetLength implementation.
- 다른 대안 대신 이 방식을 선택한 이유: Local exceptions preserve the same incorrect observable order and GC hazards, while broad admission would mix unsupported RegExp syntax and matching semantics into a constructor/iterator/property unit. Shared abstract operations keep Proxy invariants and exotic receivers consistent across callers.
- 장점, 단점 및 영향: Eight skips become passes, the built-ins failure count drops by a net 100, all supported language tests remain green, and forced-GC/exact-cap cases use one rooted path. The cost is two more manually synchronized Realm registries and a larger property core; 137 broader RegExp failures remain explicit follow-up scope.
```

### RegExp duplicate named captures

RegExp source validation and runtime compilation share
`scan_regex_named_captures`. Each nested disjunction frame unions names from
completed alternatives but rejects a name when two concatenated terms can
participate together. Capture occurrences remain ordered and retain separate
numeric indices; a second index map groups all occurrences by decoded
ECMAScript name. Result construction walks occurrence order, replaces an
earlier `undefined` with the sole participating capture, and leaves the
`IndexMap` slot in place so property enumeration follows the first occurrence.
`indices.groups` reuses the numeric pair object rather than allocating a copy.

```text
pattern
  -> structural named-capture scan and MightBothParticipate early errors
  -> ordered (name, capture index) occurrences
  -> name -> [capture indices] table
  -> numbered backend pattern plus short (?@set_id) references
  -> RegExp VM selects zero or one populated capture in each set
  -> groups / indices.groups participating value with first-name order
```

RuJa vendors `fancy-regex` 0.18.0 because duplicate-name backreferences and
ECMAScript repeated-capture clearing require matcher-state operations that
cannot be repaired after a match. The fork stores each capture set once and
keeps only its ID in parser, AST, compiler, and VM instructions. Quantified
capture entry clears descendant slots through the backend's copy-on-write
state; current-delta membership is a bitset, and every cleared slot consumes
the existing backend work budget. Case-insensitive backreferences consume the
same number of scalar values as the capture and use Unicode simple folding for
`u`/`v`, otherwise the legacy ECMAScript uppercase relation.

Patterns with backreferences use the bounded backend directly. Ordinary
patterns with repeated captures use `CaptureCorrected`: the linear Rust
matcher establishes whether and where a match exists, then the ECMAScript
backend reconstructs captures only at those successful boundaries. This keeps
no-match probes on the linear path while removing the old post-match heuristic
that could mistake trailing text for the final quantified iteration. All fork
behavior is gated by `ecmascript_mode`; mode-off analysis retains upstream
delegation. Because Cargo substitutes a registry dependency when packaging a
path dependency, crates.io publication remains disabled until the fork is
upstreamed or published separately.

```text
[Decision Log]
- 목적과 의도: Support ECMAScript duplicate named captures, participating backreferences, and repeated-capture state without introducing quadratic compilation or an unbounded no-match path.
- 기존 구현 및 제약 조건: One name owned one index, duplicate declarations were always rejected, unmatched aliases needed empty backreference semantics, Rust captures retain stale values across iterations, post-match reconstruction could not observe matcher state, and path forks cannot transparently survive crates.io packaging.
- 검토한 주요 대안: Expand every alias into nested conditionals, guess the last iteration after matching, route every repeated-capture pattern entirely through a backtracking VM, hand-roll a new RegExp engine, or add isolated matcher-state primitives to a vendored backend.
- 선택한 방식: Share a structural early-error scanner, retain ordered occurrences plus one index table per name, lower references to ID-based BackrefSet instructions, clear captures transactionally in the VM, prefilter ordinary matches linearly, and gate the fork behind explicit ECMAScript options and a finite work budget.
- 다른 대안 대신 이 방식을 선택한 이유: Conditional expansion and repeated name scans are quadratic, post-processing is observably wrong, broad backtracking routing regresses hostile no-match patterns, and a replacement engine is too broad for this conformance unit. Backend state is the only point that can clear captures with correct backtracking restoration.
- 장점, 단점 및 영향: Exact duplicate-name syntax, groups, indices, replacement, backreference, Unicode case-fold, and quantified-state semantics are directly tested with linear source/table growth and bounded runtime work. The tradeoffs are a maintained backend fork and disabled crates.io publication until that fork has a registry path. The once-remaining hard variable-lookbehind case is closed by the directional matcher below.
```

### RegExp directional lookaround

`compile_regex_with_input_mode` detects assertions outside escaped text and
character classes and routes those patterns through the vendored ECMAScript
backend. Normalization still owns JavaScript-specific UTF-16 and case-fold
semantics. In particular, non-Unicode ignore-case literals and classes are
materialized from the legacy `Canonicalize` equivalence relation under a
scoped backend case disable; this admits `U+00E9` case pairs without also
admitting Unicode-only long-s or Kelvin folds.

The backend compiler carries an explicit forward or backward direction.
Lookahead compiles its subpattern forward; lookbehind compiles backward.
Backward concatenation visits terms in reverse execution order while leaving
alternatives and greediness unchanged. Capture groups save their end before
their start, and dedicated backward instructions consume literals, scalar
wildcards, general newlines, delegates, ordinary backreferences, and duplicate
name capture sets from the cursor toward the start of the input.

```text
RegExp source + flags
  -> source validation and JavaScript normalization
  -> lookaround detection
  -> ECMAScript fancy-regex parser
  -> forward lookahead / backward lookbehind compiler
  -> atomic assertion VM with one shared work budget
  -> capture byte ranges
  -> RuJa UTF-16 result materialization
```

Positive assertions save the outer cursor, enter an atomic region, run the
subpattern, leave the region, and restore only the cursor. Captures produced by
the successful assertion therefore remain observable, but later matching
cannot backtrack into the assertion. Negative assertions use a transactional
branch whose successful negative path discards subpattern state.

Annex B permits quantified lookahead only in legacy non-`u`/`v` patterns. The
parser exposes that exception only when both ECMAScript and legacy modes are
active. Finite and unbounded nullable repeats carry an explicit upper bound and
an ECMAScript empty-iteration failure mode, allowing child alternatives to
backtrack while preserving the required capture from completed iterations.

Resource accounting is also mode-specific. Every ECMAScript branch push,
repeat dispatch, and repeated-capture clear consumes the same finite work
budget, including paths that succeed without failed backtracking. The
ECMAScript branch stack is capped at 100,000 entries. Mode-off callers retain
upstream's one-million-entry stack and failed-backtrack counter, so the fork
does not silently tighten the public crate's ordinary semantics.

```text
[Decision Log]
- 목적과 의도: Implement ECMAScript lookahead, lookbehind, backward captures and backreferences, and Annex B quantified-lookahead semantics without an unbounded backtracking path.
- 기존 구현 및 제약 조건: Rust regex cannot express variable-length lookbehind or assertion capture semantics, upstream fancy-regex searched lookbehind prefixes forward, successful zero-width repetitions could bypass the failed-backtrack counter, and broad backend routing would weaken RuJa's resource boundary.
- 검토한 주요 대안: Continue translating assertions to Rust regex, enumerate candidate lookbehind starts, post-process captures, replace the complete matcher, or add directional instructions and explicit ECMAScript accounting to the maintained backend.
- 선택한 방식: Detect assertions at the RuJa boundary, normalize JavaScript case and UTF-16 semantics first, compile lookbehind backward with atomic cursor restoration, implement the legacy RepeatMatcher exception in the parser/VM, and charge every ECMAScript branch, repeat, and capture-clear operation to one bounded budget with a 100,000-entry stack cap.
- 다른 대안 대신 이 방식을 선택한 이유: Translation cannot preserve assertion capture/backreference order, prefix enumeration changes greediness and scales with input length, post-processing cannot reconstruct transactional matcher state, and a new engine is too broad for this unit. Directional compilation follows the specification directly while reusing the audited VM state model.
- 장점, 단점 및 영향: The complete Test262 lookbehind subtree passes, hard duplicate-name lookbehind works, positive assertions remain atomic, and hostile successful zero-width or branch-growth patterns terminate under explicit limits. The cost is a larger maintained backend fork; unrelated RegExp grammar, empty-class, sentinel, nested-v, and linear-boundary work remains separate.
```

### RegExp grammar validation

`validate_regex_literal` validates ECMAScript source grammar before any
backend-specific normalization or compilation. Quantifier validation uses an
explicit `NoAtom` / `Atom` / `Prefix` state machine: an atom admits one
quantifier prefix, `Prefix` admits only one lazy `?`, and assertions reset the
state. Escape scanning consumes a complete atom only when its syntax is valid,
so malformed legacy `\x`, `\c`, or identity `\k` cannot hide a repeated
quantifier. Named-backreference skipping is enabled only after the shared
named-capture scan proves that the pattern has named captures.

Class range validation has two representations. Legacy mode tokenizes class
contents into UTF-16 code units because raw supplementary characters and
identity escapes may contribute two range atoms. It decodes Annex B octal and
control forms and preserves the incomplete-`\c` fallback as separate `\` and
`c` atoms. Unicode modes keep scalar endpoints, combine a fixed lead/trail
surrogate escape pair, reject character-set range endpoints, and distinguish a
single range `-` from the `v` subtraction operator. The Unicode syntax pass
owns nested `v` class depth and restricted brackets.

```text
RegExp source + flags
  -> flags and named-group validation
  -> Unicode escape/bracket/class-depth validation
  -> legacy UTF-16 or Unicode-scalar class-range validation
  -> atom/prefix/lazy quantifier state validation
  -> assertion/modifier validation
  -> JavaScript normalization and bounded backend compilation
```

```text
[Decision Log]
- 목적과 의도: Enforce ECMAScript RegExp early errors independently of backend parser quirks while preserving legacy UTF-16 and Annex B behavior.
- 기존 구현 및 제약 조건: The validator tracked only whether any atom had appeared, skipped malformed escapes too broadly, compared only Unicode character-set endpoints, and delegated range order and restricted brackets to backends whose grammars differ from ECMAScript.
- 검토한 주요 대안: Continue relying on backend errors, patch the 12 Test262 files by spelling, parse every RegExp into a new full AST, or add bounded source validators for the finite grammar surfaces.
- 선택한 방식: Use a small quantifier state machine, syntax-aware escape boundaries, a UTF-16 legacy class tokenizer, scalar Unicode endpoints with surrogate-pair composition, and explicit nested-v/subtraction checks before compilation.
- 다른 대안 대신 이 방식을 선택한 이유: Backend acceptance is observably wrong even for unexecuted literals, path-specific patches hide equivalent constructors, and a replacement parser is too broad for this unit. Mode-specific validators map directly to the relevant grammar invariants and can be differential-tested against Node.
- 장점, 단점 및 영향: Twelve failures become passes with no matrix movement outside built-ins; malformed quantifiers and ranges fail consistently for literals and constructors, and 1,219 class differentials show no regression. Full v set algebra, Annex B backend lowering, empty-class execution, large-count policy, and hybrid nullable matching remain explicit separate units.
```

`MakeClosure` follows the same rule for an ordinary function's fresh
`.prototype`: the prototype is pinned before a named-function environment or
the function object can allocate, then released only after the function owns
it. Allocation failures restore both the temporary environment pin and the
prototype pin, so a forced collection cannot reuse the prototype's heap slot.

Native Error materialization is also a collecting boundary. It first pins the
selected Realm intrinsic prototype and performs the ordinary rooted GC retry.
If every capped cell is still live, the VM returns an immutable, preallocated
`RangeError("heap limit exceeded")` owned by that Realm. The reserve is created
during Realm intrinsic setup, counts as an ordinary live heap object, and is a
permanent GC root together with the Realm's intrinsic Error prototypes. It
therefore settles an already-created Promise without allocating past the
sandbox limit or borrowing another Realm's Error identity.

The reserve is non-extensible and its `name`, `message`, and `stack` properties
are non-writable and non-configurable. Repeated fully saturated failures in the
same Realm intentionally share object identity; a fresh Error is still used
whenever GC reclaims a normal cell. Explicit JavaScript thrown values bypass
materialization, and non-catchable Fuel exhaustion never uses the reserve.
Because Error materialization can now collect, every caller must pin any heap
value held only in a Rust local that remains needed after the call.

```text
[Decision Log]
- 목적과 의도: Keep existing Promise and dynamic-import capabilities settleable at an exact object cap without weakening the cap.
- 기존 구현 및 제약 조건: A catchable heap failure needed one more GC cell for its JavaScript Error object; allocation failure propagated to the host after the Promise resolver or job had already been consumed.
- 검토한 주요 대안: Allow a bounded number of over-cap cells; defer settlement until a later GC; use one VM-wide fallback; preallocate one fallback per Realm.
- 선택한 방식: Try one rooted GC allocation, then return an immutable preallocated RangeError from the operation Realm.
- 다른 대안 대신 이 방식을 선택한 이유: Over-cap cells violate the host contract, deferred retries cannot guarantee a free cell and change job ordering, and a VM-wide object violates Realm identity.
- 장점, 단점 및 영향: The live-object ceiling remains exact and repeated failures always settle existing Promises; one cell is reserved per Realm and saturated failures expose shared identity as a documented host-limit deviation.
```

## Transactional test262 Realm construction

The test262 host builds a Realm by publishing intrinsic objects into per-Realm
VM registries as setup progresses. Those provisional entries are intentional:
later installers consult earlier intrinsic identities, and the same maps keep
the growing object graph alive across collecting allocations. They do not,
however, make a partially initialized Realm observable to JavaScript.

Realm creation therefore uses one transaction from fresh environment
allocation through final wrapper attachment. It records the incoming
`gc_pins` depth, pins the environment before any object can reach it, populates
the intrinsic graph, allocates the host wrapper, and attaches the Realm global.
A successful commit releases only the transaction's pins. Any error first
truncates the complete transaction-owned pin suffix and then removes that
environment's entries from all 33 rooted per-Realm registry families and the
non-rooting `%Object.prototype%` reverse identity index. Native error
materialization runs afterward in the calling Realm, so its collecting retry
can reclaim the abandoned graph.

Rollback does not rewind the heap allocator, inline cache, GC counters, fuel,
or finalization queue. A cap-triggered collection may legitimately clear the
cache or enqueue cleanup for pre-existing registries; restoring either would
reintroduce stale heap indices or lose required jobs. Realm setup itself does
not publish module records, template objects, generated symbols, or Promise
jobs, so registry roots and the transaction pin suffix are the complete
logical rollback surface.

```text
[Decision Log]
- 목적과 의도: Make failed test262 Realm construction leave no inaccessible GC roots while preserving exact heap-cap and error-Realm behavior.
- 기존 구현 및 제약 조건: Intrinsic installers publish 33 families of Realm roots incrementally and use fallible LIFO temporary pins; wrapper allocation remains fallible after every registry has been populated.
- 검토한 주요 대안: Publish nothing until setup completes; clean only the last inserted map; make every installer independently error-safe; or own all provisional roots and pins in one outer transaction.
- 선택한 방식: Keep provisional registry publication, pin the fresh environment, capture the incoming pin depth, include wrapper attachment in the transaction, truncate the owned pin suffix on every result, and remove every Realm registry entry on error.
- 다른 대안 대신 이 방식을 선택한 이유: Later installers require earlier intrinsic identities, map-specific cleanup misses other roots, and duplicating rollback in every installer creates drift. One lexical owner matches the actual observability boundary.
- 장점, 단점 및 영향: Every hard-cap failure point is reusable and collectible before caller-Realm error materialization. The registry inventory remains manually synchronized across the VM fields, root tracer, rollback helper, identity indexes, and regression counter, so new Realm registries must update every applicable site.
```

## Realm Object prototype identity

Every Realm's original `%Object.prototype%` uses the Immutable Prototype
Exotic Object `[[SetPrototypeOf]]` behavior. A request for its current `null`
prototype succeeds, while a different prototype returns `false` without
changing the object. Public `Object.setPrototypeOf` and the legacy
`__proto__` setter convert that status into a TypeError from the invoked
method's Realm; `Reflect.setPrototypeOf` returns the boolean. Proxy dispatch
still occurs before target handling, so transparent Proxies delegate and a
truthy trap over an extensible target may report success without mutating the
target. The ordinary same-prototype check remains before the immutable check.

`realm_object_prototypes` is the authoritative environment-to-intrinsic map
and GC root. `realm_object_prototype_ids` is a non-rooting reverse `HashSet`
used only for expected O(1) identity checks. Main and created Realms publish
both entries through one registration helper. Failed Realm construction
removes the owning map entry and then unconditionally removes the reverse
identity before the heap slot can be reused. The removal result is stored
before `debug_assert!`; placing the mutation inside the assertion would erase
it from release builds. The registry rollback counter includes both
collections, and CI executes the full heap-boundary rollback sweep in release
mode to preserve that guarantee.

```text
[Decision Log]
- 목적과 의도: Apply immutable-prototype semantics to the original Object prototype of every Realm without slowing unrelated prototype mutations or retaining stale heap identities.
- 기존 구현 및 제약 조건: SetPrototypeOf recognized only the main VM Object prototype. The environment-keyed Realm map already owned every intrinsic as a GC root, but scanning it would make each ordinary mutation O(number of Realms), and GcIdx slots can be reused after failed Realm construction.
- 검토한 주요 대안: Keep the main-only special case; scan all Realm map values; add a flag to every generic object allocation; or maintain a non-rooting reverse identity set beside the authoritative map.
- 선택한 방식: Register each intrinsic in the rooted map and an O(1) reverse HashSet through one helper, consult that set after same-prototype equality, and remove its entry unconditionally during transactional Realm rollback.
- 다른 대안 대신 이 방식을 선택한 이유: Main-only behavior violates Realm semantics, a linear scan creates attacker-controlled work, and an object-layout flag would widen dozens of unrelated allocation paths. The reverse index reuses the existing lifecycle boundary with a small, auditable surface.
- 장점, 단점 및 영향: All Realms now preserve the required null prototype through direct, borrowed, Proxy, and post-GC calls with constant expected lookup cost. The map and reverse index must remain synchronized; release rollback CI and the 32-collection counter guard against stale identities after slot reuse.
```

Observable materializers follow the same ownership rule. When an abstract
operation reads heap values into a Rust collection and a later getter, proxy
trap, coercion, call, or construction can re-enter JavaScript, every value is
pinned immediately after it is read. The caller owns those pins through the
last re-entrant operation and releases them on both normal and abrupt
completion. A Rust `Vec<Value>` is storage, not a GC root.

`src/builtins/call_arguments.rs` centralizes that contract for
`CreateListFromArrayLike`. `Reflect.apply`, `Reflect.construct`, and
`Function.prototype.apply` therefore share the same observable `length`,
`ToLength`, indexed `Get`, resource-cap, and pin-cleanup behavior instead of
maintaining array-specific shortcuts. `Function.prototype.apply` handles its
specified omitted, `null`, and `undefined` no-argument cases before entering
the shared object-only operation. The materialized list and its pin count move
together into the final call so a later getter or target re-entry cannot make
an earlier argument collectible.

## Native Array callback result ownership

`Array.prototype.map` and `flatMap` use live generic indexed traversal. Each
current source value is pinned across its callback, and each fresh mapped
result remains pinned until the destination owns it. `flatMap` additionally
retains mapped container roots while copying their elements, so tracing the
container also preserves nested values.

`Array.of` follows the same rule across a different observable boundary. Its
arguments and constructor remain rooted through construction, and the returned
object is pinned before the first `defineProperty` trap. The result stays live
through every element definition and the final `length` set. All three methods
run their fallible body inside a completion scope and release one LIFO pin
suffix afterward, so callback throws, Proxy trap errors, and result-allocation
failures restore the incoming pin depth.

```text
[Decision Log]
- 목적과 의도: Prevent native Array methods from exposing collected-and-reused heap slots when JavaScript re-enters the VM during callback or property-definition work.
- 기존 구현 및 제약 조건: GcIdx is a generation-free cell index, the collector sees only VM roots, and map/flatMap callback results plus the Array.of result lived only in Rust locals across observable calls. Rust locals do not participate in tracing.
- 검토한 주요 대안: Make all Rust Value locals implicit roots, add generations to every GcIdx, disable collection during native built-ins, pin only the final value, or explicitly own temporary roots at each observable boundary.
- 선택한 방식: Pin source/argument values before re-entry, pin each fresh callback or constructed result immediately, retain those roots until the destination owns the values, and release them from one success/error cleanup scope.
- 다른 대안 대신 이 방식을 선택한 이유: Global implicit rooting or generation handles require a VM-wide representation change, disabling GC weakens the sandbox, and late pinning occurs after a slot may already be reused. Explicit lexical ownership matches the existing gc_pins contract and keeps this correction bounded.
- 장점, 단점 및 영향: Forced GC can no longer turn prior map/flatMap results or a custom Array.of result into another HeapObj; abrupt and exact-cap failures leave no pin leak. Root storage grows with live object-valued inputs/results for the duration of the operation, while snapshot-based methods with broader observable-semantics issues remain separate follow-up work.
```

## Native Array SortIndexedProperties and writeback

`Array.prototype.sort` and `toSorted` use a shared stable merge sort whose
comparison closure returns a VM completion. This is necessary because both
custom comparator calls and default `ToString` conversion can execute
JavaScript. Materialized values, the receiver, and the comparator remain
explicit temporary roots for the complete operation; a custom comparator
result is additionally pinned across `ToNumber`. The first abrupt completion
stops merging immediately and the outer completion scope releases the entire
LIFO root suffix.

Both methods validate the comparator, apply `ToObject`, and cache
`LengthOfArrayLike` once. A shared `SortIndexedProperties` collector then
performs ascending, live VM property operations. `sort` uses the skip-holes
mode, issuing `HasProperty` and then `Get` only for present own or inherited
indices. `toSorted` uses the read-through-holes mode and issues `Get` for every
captured index, so holes and missing properties become explicit `undefined`
entries. Collection finishes before comparison starts, and getter, Proxy,
conversion, comparator, setter, or deletion errors stop the algorithm at the
first observable failure.

The common comparison path orders `undefined` after every defined value
without invoking a custom comparator. Default comparison converts each value
and compares RuJa's decoded UTF-16 code units rather than Rust scalar or UTF-8
order, preserving lone-surrogate sentinels. After sorting, `sort` writes the
materialized list through ascending strict indexed `[[Set]]` and deletes the
unused captured range in ascending order. Missing Array elements use the
generic receiver-aware setter path, so inherited ordinary descriptors and
Proxy `set` traps run before receiver length or extensibility is considered.
Existing own dense elements retain their metadata-synchronizing fast path.

`toSorted` has different ownership and hole behavior. It creates and pins its
fresh Array before the first indexed read, as required by `ArrayCreate`.
After successful sorting it installs every value as an own, present result
index without consulting inherited setters. Allocation failure therefore
occurs before source access or comparator side effects, and comparator failure
leaves the unreachable destination available for later collection without
leaking a pin.

Collection, merge comparisons, writeback, and deletion consume execution fuel.
The sandbox rejects captured lengths above `MAX_DENSE_ARRAY_LEN` before any
indexed scan. For `toSorted`, this policy check occurs after `ArrayCreate` so
the specified invalid-Array-length error order remains observable.

```text
[Decision Log]
- 목적과 의도: Implement the complete observable SortIndexedProperties boundary for Array sort and toSorted while preserving GC ownership, stable comparison, Array metadata, and sandbox resource limits.
- 기존 구현 및 제약 조건: The first hardening pass was correct only for direct dense Arrays. It bypassed ToObject and LengthOfArrayLike, inherited indices, accessors, Proxy Has/Get, and generic receiver writeback. Array's missing-index fast path also skipped Proxy prototypes, and Rust-owned Values remain invisible to the collector unless explicitly pinned.
- 검토한 주요 대안: Keep separate dense and generic algorithms, snapshot own keys before sorting, route every Array index write through the generic setter, remove the hard length cap and rely only on optional fuel, or share one mode-driven collector plus targeted fast paths.
- 선택한 방식: Cache the receiver and length once, collect with live ascending VM property operations under explicit skip-holes/read-through-holes modes, root every retained value immediately, preserve the shared completion-returning merge sort, use generic strict Set/Delete for sort, and keep direct destination installation for fresh toSorted Arrays.
- 다른 대안 대신 이 방식을 선택한 이유: Separate algorithms would drift in comparison and cleanup order, own-key snapshots miss live HasProperty/Get effects, routing existing dense writes through the generic path would discard their metadata optimization, and optional fuel alone does not bound an unmetered host. One collector exposes the specification distinction while retaining narrow exotic-object ownership points.
- 장점, 단점 및 영향: Generic objects, boxed primitives, inherited accessors, Proxy traps, getter mutation, partial writeback, forced GC, and fuel aborts now follow one tested order. The focused sort/toSorted Test262 set has no failures. The explicit 1,048,576 scan cap is intentionally stricter than ECMAScript for very large sparse receivers, temporary root and merge storage scale with the collected list, and default comparison still allocates UTF-16 vectors.
```

## Iterative Proxy deletion and nested traversal fuel

`Vm::delete_property_key` owns one root for its original receiver and advances
an owned `current` value through transparent Proxy targets. Each iteration
checks revocation, consumes one fuel unit, pins that layer's target and handler,
and performs observable `GetMethod(handler, "deleteProperty")`. A nullish trap
releases the per-layer pins and continues; a present trap is pinned through its
call, then the target remains rooted through descriptor and extensibility
invariants. One outer completion scope releases the original root on every
normal, thrown, allocation, or host-fuel completion. Ordinary exotic deletion
runs once after the loop reaches the final non-Proxy target.

Valid finite Proxy target chains have no specification depth bound. Proxy
targets are fixed at construction, so forwarding does not need a visited set or
an arbitrary host limit. Hosts that need a work bound opt into fuel. That bound
must include nested abstract operations: a shallow delete can read its trap
through a deep Proxy handler and can validate a truthy result through deep
`[[GetOwnProperty]]` and `[[IsExtensible]]` target chains. The shared iterative
`[[Get]]`, descriptor, and extensibility loops therefore charge each Proxy
layer as well. With no fuel budget they remain stack-safe and unbounded by
design.

```text
[Decision Log]
- 목적과 의도: Remove native-stack dependence from Proxy deletion while preserving exact trap order, GC lifetime, invariant validation, and an optional host work bound.
- 기존 구현 및 제약 조건: Trapless delete forwarding recursively called delete_property_key; Rust Value locals are not GC roots; and nested Proxy handler Get, target descriptor, and extensibility loops could bypass a fuel charge placed only on the outer delete wrapper.
- 검토한 주요 대안: Keep recursion, impose a fixed depth cap, track visited targets, accumulate every transparent layer, or advance one rooted current target iteratively while metering every nested Proxy operation.
- 선택한 방식: Pin the original receiver, iterate one Proxy layer at a time with constant per-hop target/handler/trap pins, preserve the first real trap's descriptor and extensibility checks, and consume fuel in Delete plus shared Get, GetOwnProperty, and IsExtensible traversal.
- 다른 대안 대신 이 방식을 선택한 이유: Recursion can abort the host, a cap rejects valid programs, target links are immutable and do not require cycle detection, and accumulated layers add unnecessary memory. A rooted current value directly models transparent forwarding while shared fuel closes nested-operation bypasses.
- 장점, 단점 및 영향: A 100,000-layer delete is stack-safe, forced GC and every abrupt path restore pin depth, and hosts can stop deep handlers or invariant targets with fuel. Unbounded hosts can still spend linear time on arbitrarily deep legal chains. Set and receiver-side DefineOwnProperty use the later iterative state machine, and the coordinated ordinary Get, HasProperty, and Set audit below removes their former caps.
```

## Reflect omitted property-key normalization

Every property-key-taking Reflect entry point validates its target before key
conversion. `Reflect.get`, `Reflect.set`, `Reflect.has`, and
`Reflect.deleteProperty` then call one shared helper that reads argument slot
one with `undefined` as its default and performs `ToPropertyKey`. An absent
slot is therefore semantically identical to an explicitly supplied
`undefined`; it is not a signal to return early. Receiver and value defaults
remain method-specific and are applied only after key conversion.

The native call boundary pins its callee, argument list, and receiver for the
complete call. Once conversion reaches an internal property operation, the
Proxy get, set, and has paths own their target, handler, trap, receiver, and
value roots across observable re-entry. The common key helper adds no new GC
or fuel ownership: it removes three early exits and routes omitted arguments
through the already-audited explicit-`undefined` paths. Forced collection in
normal and throwing traps verifies that those existing ownership boundaries
also hold for omitted keys.

```text
[Decision Log]
- 목적과 의도: Make omitted Reflect property keys obey the same ECMAScript conversion, receiver, Proxy, and abrupt-completion semantics as explicit undefined keys.
- 기존 구현 및 제약 조건: Reflect.get, Reflect.set, and Reflect.has each had a private missing-slot early return, while deleteProperty already performed ToPropertyKey(undefined); upstream Test262 does not distinguish these cases.
- 검토한 주요 대안: Keep per-method branches and patch three return values, duplicate the correct deleteProperty expression, or centralize only the argument-slot default and conversion while retaining each method's internal operation.
- 선택한 방식: Validate each target first, call one shared slot-one ToPropertyKey helper, and then apply each method's existing receiver, value, and internal-method behavior.
- 다른 대안 대신 이 방식을 선택한 이유: No return value can emulate an accessor, property creation, Proxy trap, revocation, or thrown completion. Sharing only conversion prevents future omission drift without merging semantically different get, set, has, and delete operations.
- 장점, 단점 및 영향: Omitted and explicit undefined keys now agree through ordinary and Proxy paths, local GC and abrupt regressions provide coverage missing from Test262, and the change adds no allocation or fuel policy. The later coordinated property-traversal state machine removes the deep Get, Has, and Set caps while preserving this coercion order.
```

## Realm-local Reflect intrinsic allocation

`build_reflect_in_env` receives an explicit global environment and
`%Object.prototype%`. The main-Realm wrapper supplies the VM globals, while
Test262 Realm population calls it only after that Realm's `%Function.prototype%`,
`%Object.prototype%`, global object, and registry roots exist. The resulting
namespace and all 13 methods are therefore distinct per Realm, and native calls
derive their function prototype and error Realm from the supplied environment.
The namespace owns the observable `Symbol.toStringTag` descriptor instead of
depending on its internal diagnostic class name.

Realm creation may run under a hard heap-object cap. The ordinary native
function registration helper intentionally remains a non-collecting bootstrap
allocator because older batch installers do not all root provisional values.
Reflect uses a separate GC-retrying entry point: after each method allocation,
the method is pushed onto `gc_pins` before another allocation can collect. The
final namespace object is allocated through the sandbox allocator while all
methods remain pinned, and one cleanup path removes the complete pin suffix on
success or failure. Once allocation succeeds, the namespace owns the methods
before those temporary pins are released and the caller publishes it globally.

The allocation regression gives the batch exactly 14 slots and varies
unreachable garbage so collection occurs before method 1, method 8, or the
final namespace allocation. It then collects again and verifies all methods
remain distinct callable functions with their exact names. A 13-slot failure
case verifies that no provisional method or pin survives rollback.

```text
[Decision Log]
- 목적과 의도: Install a specification-shaped Reflect namespace in every Realm without allowing heap-cap collection to reclaim provisional methods or reject while reclaimable capacity exists.
- 기존 구현 및 제약 조건: Reflect existed only in the main Realm, lacked @@toStringTag, and accumulated method handles in an untraced Rust map through raw non-retrying allocations. Globally changing native registration to collect would expose unpinned provisional values in older intrinsic builders.
- 검토한 주요 대안: Share the main Reflect object, install only the missing tag, force one unconditional GC before the batch, change every native-function allocation globally, or add one rooted GC-retrying runtime-installer path.
- 선택한 방식: Parameterize Reflect by Realm, allocate each method through the dedicated retrying path, pin it before the next allocation, allocate the namespace through Vm::alloc, and release all temporary pins through one result boundary.
- 다른 대안 대신 이 방식을 선택한 이유: Shared or main-only objects violate Realm identity; a pre-batch collection cannot protect future collecting allocations; and changing the global bootstrap helper without auditing every caller can publish stale GcIdx values. The narrow helper makes the new runtime path correct without silently widening GC behavior elsewhere.
- 장점, 단점 및 영향: Main and created Realms now have correct object/function/error provenance and exact-cap behavior, and direct Reflect Test262 closes at 153/153. The VM retains two native-function allocation paths until older intrinsic batches receive the same rooting audit, and separate Reflect internal-method defects remain explicit follow-up work.
```

Promise keyed combinators use a separate two-stage observable protocol. They
first snapshot raw `[[OwnPropertyKeys]]`, including non-enumerable keys, and
then perform Proxy-aware `[[GetOwnProperty]]` inside the per-key loop. An
undefined or non-enumerable descriptor skips that key before `Get`,
`C.resolve`, state allocation, or index advancement. Accepted keys therefore
form a compact result while descriptor traps remain interleaved with that
key's `Get`, resolve call, `then` lookup, and `then` invocation. Pre-filtering
all descriptors during key enumeration is forbidden because it changes
observable Proxy order and bypasses a delegating Proxy's descriptor trap.

Every accepted keyed entry pins its property value through `C.resolve`, then
keeps the resulting promise, shared state, element callbacks, and observable
`then` value rooted until invocation completes. Those roots are released in
LIFO order on both success and rejection, while skipped keys allocate no entry
state. This keeps the specification's operation order and the collector's
manual `gc_pins` ownership discipline aligned at each re-entry boundary.

## Promise and async jobs

Deferred jobs own the Realm selected when the job is created. Dynamic import
records the initiating Realm, thenable jobs record the callable `then` Realm,
and Promise reaction jobs record the selected handler Realm before later Proxy
revocation or unrelated re-entry can change how an error is constructed. Job
payloads and Promise continuations trace these Realm environments together
with their capabilities, handlers, promises, and generator references.

Handler Realm selection treats catchable `GetFunctionRealm` failures, such as
a revoked callable Proxy, as the specification's current-Realm fallback while
propagating non-catchable host aborts. Pending Promise settlement precomputes
every selected handler Realm before changing state or draining handlers. If
Fuel ends that preflight, the Promise, handler list, resolving one-shot flag,
and FIFO queue remain retryable. Intrinsic resolving functions claim their
one-shot state and enqueue only the unfinished direct Resolve/Reject stage, so
external jobs, reactions, async state machines, and thenables that already ran
are not replayed. Staged settlement runs before later external jobs, while a
direct Resolve/Reject stage that aborts again is pushed back to the front.
Direct staged settlement roots the resolver operation Realm for later handler
fallback. Promise resolution that completed observable `Get(resolution,
"then")` retains that Realm together with the resolution, observed `then`, and
claimed one-shot state until `GetFunctionRealm(then)` selects the thenable-job
Realm. Nested resolving functions and allocation-error materialization then use
that selected job Realm. Retry therefore resumes after the Get without invoking
the getter again or confusing the resolver and job Realms.
An arbitrary species-provided capability function is invoked once and never
automatically replayed after Fuel, because replay could duplicate unknown user
effects.

`promise_rejection_reason_in_realm` is the common completion boundary. It
preserves an explicit thrown JavaScript value, materializes a catchable native
error in the operation Realm, and returns non-catchable Fuel exhaustion to the
host unchanged. Promise construction, resolution, reactions, await,
`Array.fromAsync`, Async-from-Sync iteration, async iterator disposal, and
async generators use that classification before converting a completion into
a rejection. Values that exist only in Rust locals are pinned before error or
replacement-capability allocation.

Host aborts also unwind the owning state machine. Initial and resumed async
functions remove suspended frames and restore the operand stack; a module
continuation marks only its own cached record errored; unrelated pending module
jobs remain resumable. An async generator marks the active request aborted,
releases queue ownership, and schedules a rooted drain for queued siblings.
Only a terminal `next()` result is retryable after a catchable allocation
failure. Other errors may occur after bytecode state advanced, so replaying the
original request is forbidden.

## Execution contexts and Realms

`Vm::execution_contexts` is the authoritative LIFO record of active JavaScript
calls and resumptions. It is intentionally separate from the bytecode frame
stack: a native builtin can call interpreted code before a frame exists, and a
suspended generator or async function can later resume beneath an unrelated
native caller. Every context owns its callee Realm environment and callee;
native contexts additionally own `NewTarget` and either an already-observed
`NewTarget.prototype` value or its already-resolved fallback Realm.

Interpreted dispatch pushes a setup context before class validation, sloppy
`this` conversion, and arguments/rest allocation so those pre-frame operations
use the callee Realm. The interpreter then pushes a frame context while
bytecode runs. Keeping both entries is deliberate: the setup context covers
the interval before frame creation, while the frame context makes later
generator and async resumptions independent of whichever native method resumed
them.

General Realm lookup uses only the top execution context, then falls back to
the active frame and VM global. Native callee and construction accessors also
read only a top native context; searching downward would leak an outer native
call into an active interpreted call. Interpreted error lookup accepts a top
interpreted context and otherwise falls back to the active frame, preserving a
suspended function's Realm when a borrowed native `next`, `throw`, or `return`
method resumes it. Catchable errors are materialized before the owning context
is popped.

The GC traces every execution context's Realm environment, callee,
`NewTarget`, and cached prototype. Rust scope cleanup restores the previous
context depth on all normal and `Result`-based abrupt paths; unwinding and then
reusing a VM after a caught Rust panic remains outside the engine's supported
recovery contract.

## Exotic extensibility and Proxy preventExtensions

Every observable stateful `HeapObj` variant owns an atomic extensibility bit.
This includes collection objects, collection and RegExp String iterators,
WeakRef and FinalizationRegistry objects, Promises, sync and async generators,
TypedArrays, ArrayBuffers (shared or ordinary), and DataViews. Module namespace
objects remain intrinsically non-extensible. `HeapObj::is_extensible` and
`HeapObj::prevent_extensions` are the exhaustive storage boundary, so adding a
new public object kind requires an explicit extensibility decision instead of
silently inheriting `true`.

`Vm::prevent_extensions` is an iterative Proxy internal-method state machine.
It pins the original receiver for the complete operation, then checks
revocation, consumes one fuel unit, and pins each target, handler, and present
trap across observable lookup and call. A missing trap advances to the target
without recursion. A false trap result returns false. A truthy result invokes
the target's complete `[[IsExtensible]]`, including nested Proxy traps and
their errors, before enforcing the invariant. Reaching a non-Proxy target
updates that variant's atomic state. One cleanup boundary restores the incoming
pin depth after normal, thrown, allocation, or host-fuel completion.

Changing exotic extensibility exposed a pre-existing integrity-level shortcut:
`Object.seal`, `freeze`, `isSealed`, and `isFrozen` had treated unknown exotic
variants as if non-extensibility alone proved their descriptors immutable.
Non-specialized exotics now use the shared `SetIntegrityLevel` and
`TestIntegrityLevel` paths. Operation targets stay pinned, temporary descriptor
objects allocate through GC-retrying `Vm::alloc`, and exact-cap tests prove two
temporary cells can be reclaimed and reused across every own key. Map
collection entries are internal collection data, not object own keys; removing
them from ordinary own-key enumeration is required for sealing a non-empty Map.
The existing specialized ordinary-object, Array, Function, and Iterator Helper
paths remain because they materialize or mutate their descriptor storage
directly.

```text
[Decision Log]
- 목적과 의도: Make every observable object obey persistent extensibility, preserve Proxy preventExtensions ordering and invariants at arbitrary legal depth, and prevent seal/freeze from reporting integrity that descriptors do not have.
- 기존 구현 및 제약 조건: Several exotic variants had no extensibility state; transparent Proxy forwarding recursed without fuel or explicit roots; truthy traps inspected nested Proxy storage directly; and integrity predicates treated unrecognized non-extensible exotics as sealed or frozen.
- 검토한 주요 대안: Store one extensibility flag on every GC cell, keep a side table keyed by GcIdx, add flags only to the initially reported variants, raise a Proxy depth limit, or give each public variant explicit state and reuse the complete integrity/internal-method paths.
- 선택한 방식: Keep per-variant atomic state behind exhaustive HeapObj helpers, walk Proxy targets iteratively with constant per-layer roots and fuel, validate truthy results through full IsExtensible, and route non-specialized exotics through rooted GC-retrying SetIntegrityLevel and TestIntegrityLevel.
- 다른 대안 대신 이 방식을 선택한 이유: Cell-wide metadata would also describe internal Environment and Iterator records and complicate slot reuse; a side table risks stale GcIdx identity; a partial variant list recreates the bug when new exotics appear; and fixed depth limits reject valid programs. Existing complete internal-method helpers preserve observable order and typed-array or module-namespace behavior.
- 장점, 단점 및 영향: Deep transparent chains are stack-safe and host-bounded, nested traps and Realm errors remain observable in order, every current exotic blocks new properties after prevention, and integrity operations process real descriptors under heap caps. The cost is one atomic field per stateful variant plus duplicated constructor initialization. Receiver-side DefineOwnProperty and the remaining ordinary property traversals were completed by the later Set and coordinated traversal state machines.
```

## Iterative prototype internal methods

`Vm::get_prototype_of` implements the Proxy `[[GetPrototypeOf]]` algorithm as
an iterative state machine. The original receiver remains pinned for the whole
operation. Each Proxy layer is checked for revocation before consuming one fuel
unit, and its target and handler are pinned across observable trap lookup and
invocation. A missing trap advances to the target without native recursion. A
present trap must return an object or `null`; that result remains rooted while
the target's complete nested `[[IsExtensible]]` operation runs.

When a trapped target is non-extensible, the state machine records and pins
the expected prototype, then continues into the target. Once an ordinary
prototype or an extensible trapped result is reached, deferred expectations
are checked from the innermost Proxy outward. This is the iterative equivalent
of returning through nested internal-method calls: an inner mismatch or abrupt
completion prevents any outer validation from completing. A single cleanup
boundary releases every deferred root and restores the incoming pin depth on
normal, TypeError, user-throw, GC, and host-fuel exits.

`Vm::set_prototype_of` uses the same forwarding discipline. Both the original
receiver and proposed prototype are pinned before any handler lookup can run
GC. False trap results return immediately; truthy results inspect the target's
full nested `[[IsExtensible]]` and, when required, `[[GetPrototypeOf]]` before
enforcing the invariant. Missing traps advance iteratively until the ordinary
target or another observable trap decides the result. This preserves the
specified revocation, `GetMethod`, call, boolean conversion, and invariant
order from the ECMAScript Proxy algorithms.

Ordinary `[[SetPrototypeOf]]` no longer stops cycle detection after 4096
objects. `prototype_chain_blocks_set` follows raw ordinary prototype slots,
charges one fuel unit per visited candidate, and stops when it reaches `null`
or an object such as Proxy whose `[[GetPrototypeOf]]` method is non-ordinary.
That stop is required by `OrdinarySetPrototypeOf`; invoking a Proxy trap here
would incorrectly reject the specified Proxy-shadowed cycle exception. Brent
checkpoints detect an impossible pre-existing all-ordinary cycle with constant
native memory, avoiding both an infinite default-fuel walk and a growing set of
reusable `GcIdx` identities.

Transparent chains are O(n) time and constant state beyond active roots.
Fully trapped non-extensible nesting performs the specification-required
nested extensibility and prototype checks, so its worst case is O(n^2) work
with O(n) deferred expected prototypes; fuel bounds that work. Regressions use
100,000 transparent layers, exact N-1/N fuel boundaries, a 5000-link ordinary
cycle, nested invariant fuel, forced GC, abrupt completion, Realm-sensitive
public methods, and a WeakRef mutation test proving deferred roots are real.

```text
[Decision Log]
- 목적과 의도: Remove the native-recursion and 4096-link correctness limits from prototype internal methods while preserving every observable Proxy step, invariant, Realm error, GC root, and host resource bound.
- 기존 구현 및 제약 조건: Transparent Proxy get/setPrototypeOf forwarding recursively called the same Rust method without fuel; trapped getPrototypeOf invariants recursively re-entered the target; proposed prototypes and several observable intermediates were not owned by one cleanup scope; and ordinary cycle detection silently returned false after scanning only 4096 candidates, allowing a longer cycle.
- 검토한 주요 대안: Raise the depth cap, retain recursion behind a separate recursion guard, track ordinary candidates in a HashSet, invoke full GetPrototypeOf while checking ordinary cycles, or use iterative Proxy state machines plus constant-memory Brent checkpoints for the ordinary-only scan.
- 선택한 방식: Pin operation inputs once, consume one fuel unit and root observable values per Proxy layer, defer non-extensible getPrototypeOf expectations for reverse validation, iterate missing setPrototypeOf traps, and scan ordinary prototype slots with fuel and Brent cycle detection until null or a non-ordinary GetPrototypeOf method.
- 다른 대안 대신 이 방식을 선택한 이유: A larger cap still rejects valid programs and accepts cycles beyond its boundary; Rust recursion remains stack-dependent; a HashSet adds infallible native growth and stores reusable heap identities; and calling Proxy GetPrototypeOf during OrdinarySetPrototypeOf violates the specification's non-ordinary-method stop rule. The chosen state machines preserve exact call order and make every unbounded walk host-metered.
- 장점, 단점 및 영향: Legal transparent chains are stack-safe at arbitrary depth, ordinary cycles are rejected without a fixed limit, trap results and proposed prototypes survive observable GC, and fuel aborts leave the VM reusable with its pin depth restored. Fully trapped non-extensible chains retain specification-driven quadratic work and linear deferred storage; the VM-wide fallible native-temporary policy remains a separate architecture task.
```

## Iterative Proxy defineProperty and call dispatch

Proxy `[[DefineOwnProperty]]` now enters one shared VM state machine from both
the internal complete-descriptor path and the public `Object.defineProperty`,
`Object.defineProperties`, and `Reflect.defineProperty` paths. A dedicated
descriptor record retains every `has_*` bit, so a partial public descriptor is
not accidentally completed before Proxy compatibility checks. Ordinary
targets still use the existing partial-descriptor application path.

The state machine pins the original receiver and descriptor values for the
whole operation. At each Proxy layer it checks revocation, consumes one fuel
unit, roots the target, handler, trap, and materialized descriptor object, and
advances through a missing trap without Rust recursion. A present non-callable
trap fails before descriptor-object allocation. A false result short-circuits;
a truthy result then performs the target's complete `[[GetOwnProperty]]` and
`[[IsExtensible]]` operations before compatibility, configurable, and
writable-tightening invariants are checked. Target descriptor fields remain
rooted across every observable nested operation.

Making deep callable `defineProperty` traps stack-safe required fixing Proxy
`[[Call]]` at the same boundary. Proxy creation now stores immutable callable
and constructable metadata instead of discovering those internal methods by
recursively walking targets. The call dispatcher consumes one fuel unit per
Proxy layer and tail-transforms its current function, `this`, and arguments
for transparent targets or Proxy-valued `apply` traps. Added roots belong to
the outer call cleanup scope, so normal, thrown, allocation, and host-fuel
returns restore the incoming pin depth. `apply` argument arrays allocate in
the current execution Realm, and non-callable traps are rejected before that
allocation.

Transparent paths are linear and stack-safe at arbitrary legal depth when
fuel is unbounded; configured fuel gives hosts an exact per-layer work bound.
Specification-required nested invariant checks can still perform superlinear
work, and Proxy-valued `apply` traps can retain per-layer argument arrays until
the call resolves. Receiver-side property definition reached through ordinary
`[[Set]]` is completed by the state machine below. Revoked Proxy heap cells
still retain their target and handler strongly even though observable
operations reject them; that storage issue remains explicit follow-up work.

```text
[Decision Log]
- 목적과 의도: Make direct Proxy DefineOwnProperty and the callable traps it invokes stack-safe, fuel-bounded, GC-safe, and Realm-correct without changing observable ECMAScript order.
- 기존 구현 및 제약 조건: Transparent defineProperty forwarding and callable Proxy traps recursively re-entered Rust; public descriptors could lose partial-field presence at the wrong boundary; deep work bypassed fuel; and temporary descriptor values, trap arguments, and foreign-Realm arrays were not owned by one cleanup protocol.
- 검토한 주요 대안: Raise recursion guards, admit only shallow Test262 cases, duplicate public and internal algorithms, complete every descriptor eagerly, or use shared iterative state machines plus immutable Proxy call/construct metadata.
- 선택한 방식: Preserve partial descriptors in an explicit record, route both entry points through one rooted per-layer DefineOwnProperty state machine, store callability and constructability at Proxy creation, and tail-transform Proxy Call state while charging fuel and allocating argument arrays in the current Realm.
- 다른 대안 대신 이 방식을 선택한 이유: Larger guards still reject valid programs and remain stack-dependent; shallow admission would hide the sandbox failure; duplicated algorithms drift in trap and invariant order; eager completion changes compatibility semantics; and immutable metadata directly represents whether ProxyCreate installed the Call and Construct internal methods.
- 장점, 단점 및 영향: One implementation now covers 100,000 transparent defineProperty layers and 25,000 callable trap layers with exact fuel and pin cleanup, partial descriptors and Realm errors remain observable in order, and mutation tests prove the critical roots and allocation ordering. Receiver-side Set delegation is completed by the following state machine; revoked-slot retention and VM-wide fallible native temporary storage remain separate bounded units.
```

## Iterative Proxy Set and receiver definition

`Vm::try_set_property_key_with_receiver_tracked` owns one iterative driver for
Proxy `[[Set]]` and ordinary prototype traversal that reaches a Proxy. The
original base, value, and receiver remain pinned for the whole operation. Each
Proxy layer checks revocation, consumes one fuel unit, records the object for
cycle detection, and roots its target and handler across observable `set` trap
lookup. A missing trap advances directly to the target. A present trap is
validated before invocation, receives the original receiver, and
short-circuits on a false result before the target descriptor invariant walk.

Ordinary `[[Set]]` owns its specialized TypedArray, Array, mapped
arguments, accessor, and data-property behavior. When its prototype is a
Proxy, it returns a forwarding outcome to the outer driver instead of
recursively calling back into Proxy Set. This removes the native recursion and
the separate 128-layer Proxy guard. The coordinated traversal state machine
below later removes the former 1024-object ordinary guard.

`OrdinarySetWithOwnDescriptor` distinguishes two receiver definitions. A
missing receiver property uses the complete CreateDataProperty descriptor
`{value, writable: true, enumerable: true, configurable: true}`. An existing
writable data property uses only `{value}` so its attributes are preserved.
Both forms now retain descriptor presence bits and delegate through the shared
iterative Proxy `[[DefineOwnProperty]]` state machine. The trap descriptor
object is created in the current execution Realm and materializes only present
fields in FromPropertyDescriptor order. Reaching an ordinary target returns to
the existing TypedArray, Array length/index, mapped-arguments, namespace, and
extensibility paths.

Deep regressions cover 100,000 transparent Set/receiver layers, exact 3N and
2N fuel boundaries, nested target descriptor and callable-Proxy fuel, forced
collection, unique abrupt values, revoked inner targets, false-result
short-circuiting, descriptor mutation between lookup and trap call, Realm
identity, exact heap caps, cycle rejection, and pin-depth restoration.

```text
[Decision Log]
- 목적과 의도: Remove the 128-layer Proxy Set and receiver DefineOwnProperty correctness limits without losing ECMAScript trap order, descriptor presence, Realm identity, GC lifetime, cycle rejection, or host work bounds.
- 기존 구현 및 제약 조건: Missing Set and receiver defineProperty traps recursively re-entered Rust, both paths stopped at a fixed Proxy depth, complete and value-only receiver descriptors used duplicated validators and materializers, and ordinary prototype traversal recursively handed control to Proxy Set.
- 검토한 주요 대안: Raise the depth constant, retain separate recursive receiver helpers, build independent iterative Set and receiver-definition stacks, or let one rooted Set driver tail-forward while reusing the existing presence-aware iterative DefineOwnProperty state machine.
- 선택한 방식: Pin operation inputs once, iterate Proxy Set targets with one fuel charge per layer, return an explicit forwarding outcome at ordinary-to-Proxy boundaries, represent receiver descriptors with presence bits, and route both complete and value-only definitions through the shared Proxy DefineOwnProperty driver before specialized ordinary fallback.
- 다른 대안 대신 이 방식을 선택한 이유: A larger cap remains non-conforming and stack-dependent; duplicated iterative algorithms would drift in GetMethod, invariant, and cleanup order; an explicit continuation stack is unnecessary for transparent tail forwarding; and the existing DefineOwnProperty driver already owns the required roots, Realm allocation, and compatibility rules when descriptor presence is preserved.
- 장점, 단점 및 영향: Deep legal Proxy Set and receiver-definition chains are stack-safe and exactly fuel-bounded, one cleanup scope restores roots on every Result exit, and complete versus value-only descriptors remain observable. The coordinated property traversal below replaces the shared node-visited policy and removes the former ordinary Get, HasProperty, Set, and handler-prototype limits.
```

## Iterative ordinary property traversal

Ordinary `[[Get]]`, `[[HasProperty]]`, and `[[Set]]` now use iterative drivers
without the former 4096/1024/1024 depth cutoffs. `PropertyTraversal` records
directed `(from, to)` edges, pins each newly reached object until operation
cleanup, and owns ordinary-edge fuel credit. Directed edges are necessary
because a Proxy trap getter can mutate a previously visited target before
returning `undefined`; rejecting a repeated object before its own lookup would
skip that observable change. Keeping every reached object rooted also prevents
a collected heap cell from being reused under an identity retained by the
edge set.

Get extracts own-property handling into an explicit value/accessor/absent
result while retaining direct compatibility paths for TypedArray,
ArrayBuffer, and DataView fields. Inherited getters receive the original
receiver. Has checks TypedArray canonical numeric indices before ordinary own
properties. Set keeps its TypedArray, Array, String, mapped-arguments, and
receiver-definition paths, and Module Namespace `[[Set]]` now returns false
for every key and receiver. Public Value-key Get/Set first perform one
`ToPropertyKey` conversion into `PropertyKey`, preserving Symbols returned by
`@@toPrimitive`.

Fuel charges one unit per ordinary-to-ordinary edge and one per Proxy internal
method layer; an ordinary-to-Proxy edge relies on the Proxy charge. Proxy
`GetMethod` lookup and a transparently forwarded Set receive one initial
ordinary-edge credit to preserve the established exact per-Proxy budgets, but
deeper inherited handlers are metered. Revocation is validated before fuel.
Nested handler, invariant, and receiver operations create independent
traversal state.

Pure ordinary repeated edges fail immediately. A Proxy cycle is different:
each pass can run an observable trap lookup, so repeated edges are replayed.
An inert cycle is stopped after 512 replays with a catchable RangeError rather
than a native stack overflow; configured fuel can stop it earlier. This guard
does not limit acyclic chain depth. Traversal memory is O(depth) because both
the edge set and persistent GC roots grow with reached objects. Construction
and new-edge growth reserve HashSet and GC-root capacity before committing the
edge or pin, so allocation failure is catchable and leaves traversal state
retryable.

The same intrinsic audit installs the required own `Array.prototype.length`
descriptor in every Realm: value 0, writable, non-enumerable, and
non-configurable. This restores transparent Proxy Has forwarding for that
property and avoids a traversal-specific special case.

Treating an own data value of `undefined` as present also exposed two older
Array copy shortcuts that had passed by relying on the former incorrect Get
sentinel. `Array.prototype.slice` was changed to perform HasProperty before
Get, while `Array.prototype.with` was changed to Get every non-replaced index.
The interim result allocator intentionally used dense, hole-only `ArrayData`
and rejected copies above `MAX_DENSE_ARRAY_LEN` because the mutators of that
unit did not yet understand sparse logical length. The later Array-exotic unit
below supersedes that allocation restriction and completes the coupled
generic and sparse method paths. This paragraph records why the temporary
boundary existed; it is not the current Array architecture.

```text
[Decision Log]
- 목적과 의도: Remove non-conforming ordinary Get, HasProperty, Set, and inherited Proxy GetMethod depth cutoffs without losing receiver semantics, observable mutation order, GC identity, host work bounds, or correct Array hole copying once own undefined values stop acting as absence sentinels.
- 기존 구현 및 제약 조건: Get recursively returned undefined after 4096 hops, Has recursively returned false after 1024 hops, Set rejected after 1024 hops, Symbol Set bypassed the shared internal method, and node-based cycle rejection could suppress a later Proxy trap lookup. Rust-local object identities were not roots and could be reused after observable GC. Slice and With copied dense slots directly, so their inherited-value behavior depended on the old incorrect own-undefined fallback. General ArrayCreate installs an own length descriptor, but legacy push, pop, and splice still mutate dense backing storage directly and can leave that descriptor stale; sparse copy results would expose the same gap.
- 검토한 주요 대안: Raise the traversal constants, remove guards without fuel, keep separate string and Symbol walkers, reject every repeated object, restore the undefined sentinel to hide Array copy defects, retain general ArrayCreate and repair every legacy mutator in this unit, allow sparse copy results, or use rooted directed-edge traversal plus specification-shaped Array copy loops with a bounded hole-only allocator.
- 선택한 방식: For that bounded unit, preserve separate Get, Has, and Set exotic ordering while sharing PropertyTraversal for directed edges, persistent roots, fuel credit, and Proxy-cycle replay. Coerce Value keys once into PropertyKey, return false from Module Namespace Set, install the missing Array prototype length descriptor at Realm construction, and repair Slice/With at their own Has/Get boundaries. The interim fresh-copy path used the current Realm prototype without a stored length descriptor and rejected lengths above the dense cap before allocation.
- 다른 대안 대신 이 방식을 선택한 이유: Larger constants remain incorrect and stack-dependent; unmetered loops are unsafe for a sandbox; duplicate key paths drift; node rejection is observably wrong after trap mutation; native recursion cannot safely represent legal deep chains; and restoring sentinel behavior would make a present undefined property observably inherit through its prototype. The Array loops must implement their own distinct hole policies. Expanding this unit into every generic mutator would obscure the property-traversal correction, while returning a sparse copy before those mutators honor sparse_max creates immediately observable length and index errors.
- 장점, 단점 및 영향: Acyclic chains and inherited traps are stack-safe at arbitrary legal depth, exact fuel and LIFO pin cleanup are testable, Symbol and receiver behavior use one path, Proxy Has gains complete direct admission, and Slice/With no longer regress when Get is corrected. At that stage, dense copy results remained compatible with existing mutators and copying more than 1,048,576 elements raised a sandbox RangeError. Memory grows linearly with traversal depth, and inert Proxy cycles retain a deliberate 512-replay host guard that can differ from another engine's implementation-specific stack limit. The Array-specific copy cap, generic receiver gap, sparse mutators, and prototype facade described by this historical decision are superseded by the Array exotic unit below.
```

## Lazy Proxy-aware for-in enumeration

`ForInIteratorState` mirrors the state described by `CreateForInIterator`: it
retains the current object, whether that object has been snapshotted, the
visited string keys, and the current own-key snapshot plus cursor. It also
retains directed prototype edges, rooted node identities, Proxy presence, and
the cycle-replay count across pulls. The state lives in `IteratorData`, and GC
tracing follows both its current object and every traversal root. Creating an
iterator first boxes a non-nullish primitive and pins that wrapper across the
GC-retrying iterator allocation; `null` and `undefined` produce an already
empty iteration without traversal nodes.

Each `iterator_next_resume` advances only far enough to yield one key or reach
completion. It obtains a current object's keys through `[[OwnPropertyKeys]]`,
discards Symbols without invoking `[[GetOwnProperty]]` for them, then queries
each remaining string descriptor at the point it is processed. A name enters
the visited set only when the descriptor still exists, and is yielded only
when that descriptor is enumerable. After the snapshot is exhausted, the
iterator calls `[[GetPrototypeOf]]` and repeats on the returned object. This
preserves deleted-key, non-enumerable-shadowing, absent-descriptor, abrupt
completion, revocation, and early `break` behavior. State locks are never held
across an observable trap.

`own_property_keys_or_throw` now uses an explicit stack of pending Proxy
frames instead of recursive Rust calls. Every Proxy layer validates revocation,
consumes fuel, and roots its target and handler across `GetMethod` and trap
execution. A present trap validates its array-like result and duplicates,
then performs `IsExtensible(target)` before the target's
`[[OwnPropertyKeys]]`. On unwind, every target key's descriptor is queried
before an omitted non-configurable key is reported; non-extensible targets
then require an exact key set. The trap's original order is retained for the
caller. Missing traps tail-forward without imposing a legal chain-depth cap.

Ordinary own-key snapshots precharge fuel for every native key source before
materializing vectors or sets: typed-array indices, dense Array presence,
boxed or primitive String indices, module namespace exports, and stored
properties. String byte length is a conservative upper bound for UTF-16 key
count. Candidate processing and ordinary prototype edges are separately
metered. Newly reached prototypes are reserved and installed transactionally
in the iterator's traced state. Ordinary cycles fail when an edge repeats;
observable Proxy cycles may replay and are bounded by configured fuel or the
shared 512-replay host guard even when a Proxy yields one fresh key per pull.
Terminal completion replaces the traversal collections so a reachable
completed iterator does not retain capacity proportional to its former depth.

`Object.hasOwn` and `Object.prototype.hasOwnProperty` now use the same complete
Proxy `[[GetOwnProperty]]` path as `propertyIsEnumerable`, so virtual
descriptors, transparent nested targets, revocation, and abrupt values are no
longer bypassed. Map entries remain internal collection data and therefore do
not appear in object enumeration.

```text
[Decision Log]
- 목적과 의도: Make for-in enumerate ordinary and Proxy objects through the required internal methods while remaining lazy, stack-safe, GC-safe, and bounded by host fuel.
- 기존 구현 및 제약 조건: make_for_in_keys eagerly walked raw heap properties and prototype slots, treated Map entries as object keys, bypassed Proxy traps, retained per-key source objects, and could materialize an unmetered ordinary snapshot before a host could stop it.
- 검토한 주요 대안: Patch only transparent Proxy forwarding, eagerly collect all Proxy keys before loop execution, recurse through Proxy targets and prototypes, expose the internal iterator to JavaScript, or keep one lazy state machine backed by complete internal methods.
- 선택한 방식: Store CreateForInIterator-shaped state in IteratorData, advance one observable phase per pull, use iterative pending frames for Proxy OwnPropertyKeys invariants, trace and pin all live objects, and precharge ordinary snapshot work before native collection growth.
- 다른 대안 대신 이 방식을 선택한 이유: A forwarding-only patch would still miss descriptor and prototype traps; eager collection changes break and mutation order; Rust recursion makes legal depth host-stack-dependent; and an exposed iterator adds non-standard surface. Reusing the internal-method helpers keeps Object and for-in behavior aligned.
- 장점, 단점 및 영향: Proxy trap order, Symbol filtering, shadowing, abrupt completion, primitive boxing, and early break now follow the specification; deep chains are iterative and every unbounded walk is fuel- or replay-bounded. Iterator state and pending invariant frames use O(keys plus depth) native memory, non-ASCII String snapshot fuel is conservatively overcharged, and the 512 replay guard is an intentional sandbox policy for inert Proxy cycles.
```

## Array exotic prototype and generic indexed methods

Every Realm now installs `%Array.prototype%` as an actual `HeapObj::Array`
with empty `ArrayData`, not an ordinary object carrying an Array class tag.
Array index definitions therefore participate in `ArraySetLength`, so writing
prototype index `2` changes its length to `3`. The VM also records each
Realm's intrinsic Array constructor alongside its Array prototype. Both
registries are GC roots, Realm construction rolls them back transactionally,
and `ArraySpeciesCreate` can distinguish the current Realm's intrinsic
constructor from an observable foreign constructor.

`ArrayData` now has one representation rule. Default writable, enumerable,
configurable indexed data properties live in `items` and `present`; accessors,
non-default descriptors, and sparse entries live in `props`. A default
descriptor found in the legacy property table migrates into dense storage on
write, preventing duplicate states whose visible value depended on which
lookup path ran first. Mapped arguments retain their specialized aliasing
path. Array length shrink computes one removable-property plan, precharges
descriptor scanning and dense resize work after observable conversion but
before mutation, and applies the planned property and logical-length update
without rescanning mutable storage.

`push`, `pop`, `shift`, `unshift`, `splice`, `slice`, `copyWithin`, and `with`
now operate on generic receivers through ToObject, LengthOfArrayLike, Get,
HasProperty, Set, DeletePropertyOrThrow, and CreateDataProperty. Generic lengths
use the full ToLength range through `2^53 - 1`; creation of an actual Array
still enforces the ECMAScript uint32 length limit. Slice preserves holes and
uses `ArraySpeciesCreate`. Splice uses species for the deleted-elements result.
CopyWithin mutates through live property operations without a snapshot. With
intentionally ignores species and reads every non-replaced position, so source
holes become own `undefined` values. All observable values and fresh results
remain pinned until ownership transfers or an abrupt completion restores the
incoming pin depth.

The Array constructor and Slice can allocate sparse results beyond
`MAX_DENSE_ARRAY_LEN` without first reserving a dense vector. Dense allocation
uses the VM's collecting allocator, including retry at an exact heap-object
cap. With still rejects a result above 1,048,576 elements because its
read-through-holes contract must materialize every index and the sandbox does
not yet provide fallible native temporary storage for that many values.
`IsArray` follows Proxy targets iteratively and checks revocation before
charging one fuel unit per layer; that loop neither allocates nor re-enters
JavaScript between target reads. Large ArraySetLength scans and dense resizes
are similarly fuel-bounded before any irreversible mutation.

```text
[Decision Log]
- 목적과 의도: Model the intrinsic Array prototype and the coupled mutation and copy methods with real Array exotic semantics while preserving Realm, species, GC, fuel, and heap-cap invariants.
- 기존 구현 및 제약 조건: Array.prototype was an ordinary tagged facade, default indexed descriptors could exist in both dense storage and props, push/pop/splice derived positions from dense backing length, Slice and With accepted only represented Arrays, copy results above the dense cap were rejected, IsArray Proxy traversal was unmetered, and raw allocation could fail despite reclaimable garbage.
- 검토한 주요 대안: Special-case only Array.prototype length, patch each failing Test262 path, retain dense-only results, convert every Array method in one release, replace ArrayData entirely, or establish one exotic representation invariant and repair the tightly coupled constructor, species, mutation, and copy surface first.
- 선택한 방식: Allocate every intrinsic prototype as ArrayData, track Realm Array constructors, keep default dense descriptors in items/present and exceptional descriptors in props, implement seven methods through shared internal operations, permit sparse constructor and Slice results, and precharge Proxy, length-scan, and resize work before mutation.
- 다른 대안 대신 이 방식을 선택한 이유: A prototype-only special case would diverge from ordinary Arrays; individual test patches would miss generic receivers and observable order; dense-only allocation rejects legal sparse results; changing every callback and legacy method at once is too broad to validate atomically; and retaining duplicate indexed representations makes descriptor and mutator behavior order-dependent.
- 장점, 단점 및 영향: Array.prototype now has the same length/index invariants as every Array, generic receivers and species are observable in specification order, sparse construction no longer needs a giant vector, and exact fuel, GC retry, Realm rollback, and abrupt cleanup are regression-tested. The representation migration adds property-path complexity, With retains a deliberate 1,048,576-element sandbox cap, and at this decision boundary older methods such as reverse, fill, and several callback methods still needed their own generic and rooting audits; the later fill pipeline section records that follow-up's completion.
```

### Generic Array concat pipeline

`Array.prototype.concat` now treats its boxed receiver and each argument as an
ordered input stream rather than reading represented Array backing vectors.
It creates the result before querying spreadability, then performs one
`Symbol.isConcatSpreadable` Get per object. An undefined override falls back to
Proxy-aware `IsArray`; a defined value is converted with `ToBoolean`.
Spreadable inputs capture `LengthOfArrayLike`, reject `n + length` above
`2^53 - 1` before indexed work, and execute `HasProperty` followed by
conditional `Get` for every logical index. Missing properties advance the
target index without materialization, while present values use
`CreateDataPropertyOrThrow`. Non-spread inputs become one data property. The
algorithm always ends with strict `Set(result, "length", n)` so custom species
setters, false Proxy traps, and non-writable lengths remain observable.

The receiver and all arguments are pinned before any boxing or species work.
The boxed receiver and species result join that operation-wide root set, and a
fetched value receives a temporary root across its observable result property
definition. One cleanup boundary restores the incoming pin depth for semantic
throws, Proxy revocation, fuel aborts, and heap-limit errors. Default species
allocation uses the existing collecting Array allocator. Since the result
starts at length zero and only present properties are defined, a large sparse
source does not reserve its logical length as dense storage. One fuel unit is
charged for every outer input, including an empty spreadable object, plus one
for every scanned source index, including holes.

TypedArray concat coverage exposed a transitional property-model defect:
TypedArray, ArrayBuffer, and DataView instance-field compatibility reads ran
before ordinary own descriptors. Those fields conceptually stand in for
prototype accessors, so an own `length`, `byteLength`, `byteOffset`, or `buffer`
descriptor now shadows the compatibility value before that fallback executes.

```text
[Decision Log]
- 목적과 의도: Replace the dense-only concat shortcut with the complete generic, species-aware, sparse-safe algorithm while preserving Realm, GC, fuel, and heap-cap invariants.
- 기존 구현 및 제약 조건: Concat cloned ArrayData.items, spread only direct represented Arrays, converted holes to own undefined values, bypassed ToObject and observable property operations, allocated outside the collecting VM path, and ignored custom species and Symbol.isConcatSpreadable. TypedArray compatibility reads also hid valid ordinary own length properties.
- 검토한 주요 대안: Patch only the failing typed-array files, keep a fast represented-Array branch, preallocate from captured lengths, reuse Set for copied indices, broaden Test262 feature gates by prefix, or implement the abstract-operation sequence directly and freeze only audited feature-gated files.
- 선택한 방식: Reuse the shared ToObject, LengthOfArrayLike, ArraySpeciesCreate, Proxy-aware IsArray, property-definition, strict-Set, Realm registry, and collecting allocation paths; retain u64 logical indices; root every observable value; meter each outer item and index; and admit nine exact Test262 paths through one manifest shared by both tools.
- 다른 대안 대신 이 방식을 선택한 이유: Fast backing-storage paths cannot preserve holes, inherited indices, Proxy traps, or custom species; preallocation is unsafe for huge sparse lengths; Set invokes inherited setters instead of CreateDataProperty; prefix admission expands silently with upstream; and one generic path keeps represented and non-represented receivers behaviorally identical.
- 장점, 단점 및 영향: All 69 direct concat files pass, sparse output above the dense cap is bounded by fuel rather than allocation size, observable failures restore roots, and TypedArray own length shadowing is corrected for every direct compatibility field. The algorithm must still scan every spreadable logical index as ECMAScript requires, native key strings and root vectors remain infallible Rust allocations, and shared Proxy or Bound constructor traversal needs separate per-edge fuel and linear argument collection.
```

### Generic Array copyWithin pipeline

`Array.prototype.copyWithin` snapshots only `LengthOfArrayLike`, then coerces
target, start, and an optional non-undefined end in source order through
`ToIntegerOrInfinity`. Relative positions remain `u64` through the full
`2^53 - 1` range. Overlap selects backward iteration only when the target begins
inside the source interval; all other copies advance forward.

Each logical iteration consumes one fuel unit before observable work. It then
performs `HasProperty(source)`. A present source is read with `Get` and written
with strict `Set`; an absent source invokes `DeletePropertyOrThrow` on the
destination. The method therefore preserves inherited values and holes,
observes live mutations from earlier traps, retains partial writes before a
later abrupt completion, and performs same-range operations rather than
silently treating them as a no-op. It never snapshots or materializes the
source interval, so native temporary memory is constant apart from property
traversal state and index strings.

The receiver and all arguments are rooted before boxing or coercion. The boxed
object remains rooted for the operation, and every fetched value receives a
temporary root across an observable setter or Proxy trap. One cleanup boundary
restores the incoming pin depth after semantic throws, Proxy false results,
collection, heap-cap retry during primitive boxing, or fuel abort. Primitive
boxing and native errors use the method Realm.

```text
[Decision Log]
- 목적과 의도: Implement the complete generic copyWithin algorithm without dense-array shortcuts, source snapshots, or host work that configured fuel cannot bound.
- 기존 구현 및 제약 조건: The method accepted only represented Arrays, used dense backing length, coerced arguments incompletely, copied a temporary Vec of values, collapsed holes, bypassed prototypes and Proxy traps, returned undefined for primitives and generic objects, and had no loop fuel or operation-wide roots.
- 검토한 주요 대안: Patch the 18 failing Test262 files, retain a dense fast path, snapshot presence/value records before mutation, share one algorithm with reverse or fill, cap logical length at the dense limit, or execute the abstract property operations directly.
- 선택한 방식: Reuse ToObject, u64 LengthOfArrayLike and relative-index helpers, iterate in the specification-selected direction, call HasProperty plus Get/strict Set or DeletePropertyOrThrow for every position, root the receiver/arguments/object/value, and charge one fuel unit immediately before each indexed step.
- 다른 대안 대신 이 방식을 선택한 이유: Test-only patches and dense fast paths miss live traps, inherited values, holes, and generic receivers; snapshots change mutation and abrupt-completion order; reverse and fill have different property sequences; and a dense cap rejects legal sparse objects near the safe-integer limit.
- 장점, 단점 및 영향: The direct fixed Test262 directory is 39/39, TypedArray borrowing remains compatible, MAX_SAFE_INTEGER work uses O(1) source storage, and exact fuel, Realm, GC, heap-cap, partial-mutation, and pin cleanup behavior is regression-tested. Unconfigured hosts can still request a specification-required linear scan, and native property-key strings plus traversal work remain subject to the broader process-memory policy.
```

### Generic Array fill pipeline

`Array.prototype.fill` now boxes its receiver and snapshots
`LengthOfArrayLike` exactly once before coercing start and an optional,
non-undefined end through `ToIntegerOrInfinity`. Relative positions stay `u64`
through `2^53 - 1`; the fill value itself is never coerced. Each selected index
then consumes one fuel unit and performs a live strict `Set`, in ascending
order. This preserves inherited setters, Proxy traps, non-writable failures,
resizable TypedArray behavior, partial mutation before abrupt completion, and
safe-integer property keys without consulting species or allocating a result.

The receiver and every argument are rooted before boxing. The boxed object
stays rooted across the observable length read, index coercions, and every
setter or Proxy trap, while the original fill value remains rooted as part of
the argument suffix. One cleanup boundary restores the incoming pin depth on
normal return, semantic throws, strict-Set rejection, collection, primitive
boxing at an exact heap cap, or fuel abort. Boxing and native errors therefore
retain the active method Realm.

```text
[Decision Log]
- 목적과 의도: Replace the represented-Array fill shortcut with the complete generic live-Set algorithm while preserving safe-integer, fuel, GC, Realm, and abrupt-completion behavior.
- 기존 구현 및 제약 조건: Fill cloned and rewrote dense backing storage, ignored observable length and sparse indices, accepted only numeric bounds, returned undefined for primitive and generic receivers, bypassed descriptors, prototypes, Proxies, arguments mappings, and TypedArrays, and performed an unmetered host loop.
- 검토한 주요 대안: Patch only the failing direct tests, retain a dense fast path, reuse the specialized TypedArray bulk fill, precompute property writes, cap work at the dense-array limit, or execute the abstract-operation sequence directly.
- 선택한 방식: Reuse ToObject, u64 LengthOfArrayLike and relative-index helpers; coerce bounds once in specification order; call strict Set for each ascending index after one fuel charge; root the receiver, arguments, and boxed object for the entire operation; and return that object.
- 다른 대안 대신 이 방식을 선택한 이유: Backing and bulk paths cannot preserve setters, Proxy order, per-index TypedArray conversion, or partial writes; a precomputed write set loses live mutation; and a dense cap rejects legal sparse array-like lengths. One generic path gives represented Arrays and borrowed receivers identical observable semantics.
- 장점, 단점 및 영향: Direct Array fill is 22/22 and adjacent TypedArray fill remains 52/52; safe-integer tails need constant native state; every observable exit restores roots; and configured fuel bounds the logical scan. Unbounded hosts can still request a specification-required linear scan, and native index-string plus property-traversal allocations remain governed by broader process-memory policy.
```

### Generic Array filter pipeline

`Array.prototype.filter` now boxes its receiver, snapshots
`LengthOfArrayLike` once as a safe-integer `u64`, validates the callback, and
performs `ArraySpeciesCreate(source, 0)` before indexed traversal. Each logical
index consumes one fuel unit, then runs live `HasProperty`; present values are
read with `Get` and passed to the callback with the captured index and source.
Truthy selections are compacted into ascending result indices through
`CreateDataPropertyOrThrow`. The path therefore preserves holes, inherited
values, Proxy order, callback mutation, custom species, descriptor failures,
and partial result definitions without snapshotting source values.

The receiver and all arguments are rooted before boxing. The source and species
result remain roots for the entire operation, and each present source value is
temporarily rooted across callback execution and a selected result's observable
Proxy define trap. One cleanup boundary restores the incoming pin depth after
length, constructor, species, `HasProperty`, `Get`, callback, property-definition,
heap-cap, or fuel failures. Default arrays and native errors use the active
method Realm; custom species objects retain their constructor semantics.

```text
[Decision Log]
- 목적과 의도: Replace the represented-Array filter snapshot with the complete generic, species-aware, live-property algorithm while preserving Realm, GC, fuel, and abrupt-completion behavior.
- 기존 구현 및 제약 조건: Filter cloned dense ArrayData.items, called the predicate for holes, ignored observable length, prototypes, Proxies, primitives, generic receivers, species, and result descriptors, allocated through a raw heap path, and performed no loop fuel or operation-wide rooting.
- 검토한 주요 대안: Patch only direct failures, retain a dense fast path, collect selected values before constructing the result, reuse the specialized TypedArray filter, cap traversal at the dense-array limit, or implement the abstract-operation sequence directly.
- 선택한 방식: Reuse ToObject, u64 LengthOfArrayLike, callable validation, ArraySpeciesCreate, Proxy-aware HasProperty/Get, callback dispatch, and CreateDataPropertyOrThrow; root all observable state; and charge one fuel unit before each logical source index.
- 다른 대안 대신 이 방식을 선택한 이유: Dense and preselected snapshots change holes, mutation, species timing, and abrupt partial results; TypedArray filter has different validation and species timing; and a dense cap rejects legal sparse array-like lengths. One generic path keeps represented Arrays, arguments, primitives, Proxies, and borrowed receivers observably aligned.
- 장점, 단점 및 영향: All 242 direct fixed Test262 files pass, the prior million-index sparse timeout becomes a pass, TypedArray filter remains 85/85, and exact fuel, Realm, GC, heap-cap, descriptor, and cleanup behavior is regression-tested. Unconfigured hosts can still request a specification-required linear scan through a huge sparse length, while native index strings and property traversal remain governed by broader process-memory policy.
```

### Generic live Array iterator records

`entries`, `keys`, and `values` all create the same lazy
`CollectionIteratorData` shape after one method-Realm `ToObject`. The iterator
stores its source behind a mutex and its cursor as `u64`, so safe-integer
array-like positions are not truncated on 32-bit targets. Every `next` reads
the current `LengthOfArrayLike`; TypedArray sources instead use their current
buffer witness so resize, out-of-bounds, and detach state remain observable.
Keys return the cursor without an indexed Get. Values and entries advance the
cursor before indexed Get or any result allocation, preserving progress after
an abrupt getter or heap failure. Completion replaces the source with
`undefined` before allocating the done result, both releasing the collection
and making completion sticky if that allocation fails.

The source, fetched element, entry pair, and iterator result remain roots
across Proxy access and collecting allocation. Entry pairs and all iterator
results use the active `next` function Realm. Array, Map, and Set prototypes
have separate native `next` entry points that accept only their own iterator
kinds; String iteration retains its separate brand. The obsolete internal
`IteratorData.array_like` cursor and its `usize` resume path were removed, so
Array and arguments iteration always observes the actual `@@iterator`
protocol.

Arguments creation records each Realm's immutable original
`%Array.prototype.values%` in a traced registry and installs that identity as
an own writable, non-enumerable, configurable `Symbol.iterator`. The registry
is included in Realm rollback. Arguments allocation pins its values,
prototype, iterator function, restricted callee, and unpublished object while
using the GC-retrying VM allocator. Mapped and unmapped objects therefore
survive reclaimable heap caps without exposing a partially initialized object,
while later deletion or replacement of the own iterator remains observable.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshots and the duplicate internal array-like cursor with one specification-shaped, generic, live, Realm-correct, and GC-safe Array iterator path.
- 기존 구현 및 제약 조건: Entries and keys eagerly materialized arrays, values accepted only a narrow source shape, the shared iterator used a usize cursor and immutable source, arguments lacked the required own iterator identity, cross-brand next calls were accepted, and arguments allocation bypassed GC retry.
- 검토한 주요 대안: Patch only detached receiver tests, retain a dense fast path, keep the old IteratorData fallback for arguments, snapshot length or values, use one unbranded next function for every collection, or represent the specification iterator record directly.
- 선택한 방식: Perform ToObject at iterator creation, store a mutable rooted source and u64 cursor, read live length and indexed values per next, advance before abrupt indexed work, release the source at completion, allocate pairs/results in the method Realm, preserve the original Realm Array-values identity for arguments, and remove the unreachable fallback cursor.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshots lose live mutation and Proxy order, usize truncates legal safe-integer positions on wasm32, fallback paths drift in deletion and override behavior, and a shared unbranded native entry point violates internal-slot checks. One record keeps Array and TypedArray behavior aligned without claiming unrelated Array methods.
- 장점, 단점 및 영향: Generic, primitive, arguments, Proxy, inherited, resizable, detached, abrupt, cross-Realm, and exact-cap cases share one tested path; completion releases retained sources; and wasm32 cannot re-enter the obsolete cursor. Each next still performs specification-required property work. At this iterator decision boundary generic reverse, fill, and other snapshot-based Array methods remained separate follow-ups; the preceding fill pipeline section records fill's later completion.
```

## Generic Array FlattenIntoArray

`Array.prototype.flat` and `flatMap` share one specification-shaped
`flatten_into_array` path. The entry points perform `ToObject`, one source
length snapshot, depth or mapper validation, and `ArraySpeciesCreate` in their
specified order. The shared loop then observes every source index through
`HasProperty` and `Get`, applies the mapper only in the initial flatMap frame,
checks `IsArray`, reads each nested length at descent time, and defines dense
target properties from the supplied `start` index without setting an ordinary
custom species result's length.

The algorithm uses a Rust `Vec<FlattenFrame>` rather than native recursion.
Each child frame owns the exact GC pin suffix for its source and any original
flatMap input retained below a mapped result. Frame completion pops that suffix;
the outer cleanup pops all still-live suffixes after abrupt completion. One
fuel unit is charged before each logical source index, which bounds huge sparse
lengths when an embedder configures a budget. Infinite-depth traversal tracks
only repeated sources on the active path: it permits 512 observable replays so
getters can break a cycle, then raises `RangeError` instead of growing host
memory forever. Active identities are normalized through transparent Proxy
wrappers and stored in a ref-count map, so fresh wrappers cannot bypass the
guard and cycle checks do not turn deep acyclic nesting into quadratic native
work. Acyclic nesting has no fixed depth cutoff.

```text
[Decision Log]
- 목적과 의도: Replace Array-only snapshot flattening with one generic, observable, GC-safe, and sandbox-bounded implementation shared by flat and flatMap.
- 기존 구현 및 제약 조건: flat coerced depth before receiver length, accepted only represented Arrays, copied dense backing vectors, lost holes and inherited values, ignored species and Proxy operations, and recursively consumed the native stack; flatMap separately snapshotted and mapped the dense backing vector.
- 검토한 주요 대안: Patch only detached calls, retain an Array fast path, recursively translate the specification, materialize nested lists before output, cap depth at a fixed number, or use an explicit traversal stack with lexical pins.
- 선택한 방식: Keep the entry-point ordering separate, share an iterative frame stack for FlattenIntoArray, perform live property operations, transfer temporary roots into child frames, use CreateDataPropertyOrThrow semantics, enforce the safe-integer target bound, meter every visited source index, and bound only repeated active-path sources after 512 observable replays.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable mutation and Proxy order, native recursion makes Infinity cycles a host crash risk, a fixed depth cap rejects finite valid programs, and late rooting permits heap-cell reuse during getters or callbacks. Frames preserve depth-first order while making both roots and resource work explicit.
- 장점, 단점 및 영향: Direct flat and flatMap Test262 are 43/43, cyclic Infinity inputs terminate by fuel or RangeError, and all abrupt paths restore pin depth. Frame storage and retained roots grow linearly with active acyclic nesting depth; a cycle that would mutate only after more than 512 replays is intentionally terminated by the sandbox guard. Copy-by-value and other independent Array methods remain separate conformance units.
```

## Generic Array forEach

`Array.prototype.forEach` now boxes its receiver, snapshots
`LengthOfArrayLike` once, validates the callback after that observable length
read, and scans the captured index range with live `HasProperty` and `Get`
operations. A callback therefore observes inherited values and mutations to
unvisited indices, skips deleted indices, and never visits indices added beyond
the captured length. The callback receives `(value, index, object)` with the
supplied `thisArg`; its result is discarded.

The receiver and argument slice are pinned before boxing or length access, and
the boxed object plus each fetched value remain pinned across Proxy traps,
getters, and callback execution. A single outer cleanup restores all persistent
pins on normal or abrupt completion, while each temporary value pin is released
immediately after its callback. One fuel unit is charged per logical index,
including holes, so huge sparse array-like inputs remain bounded by the
embedder's cooperative budget.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot iteration with specification-shaped generic, live, GC-safe, and fuel-bounded forEach traversal.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, cloned dense storage, invoked callbacks for holes, ignored inheritance and Proxy operations, observed no later mutation, accepted invalid detached receivers, and had no explicit fuel or pin discipline.
- 검토한 주요 대안: Patch only detached calls, retain a dense Array fast path, share filter through a discarded result, precompute present values, or implement the direct indexed traversal.
- 선택한 방식: Perform ToObject, one LengthOfArrayLike snapshot, callback validation, then live HasProperty/Get/callback work while pinning all native-frame roots and charging every logical index.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable holes, inheritance, mutation, and Proxy order; reusing filter would incorrectly allocate and consult species. The direct loop mirrors the specification and existing generic Array runtime contracts.
- 장점, 단점 및 영향: Direct Array forEach is 190/190 and adjacent TypedArray forEach remains 42/42 on the fixed corpus; callback and fuel failures restore pin depth. Sparse scans remain linear in captured length as required, but configured fuel bounds their sandbox cost. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array join

`Array.prototype.join` now boxes its receiver, snapshots
`LengthOfArrayLike` once, coerces the separator, and then performs a live `Get`
for every captured index. Missing and explicitly nullish elements both append
only the separator, while every other fetched value is converted immediately.
Separator coercion can therefore mutate values before the first indexed read,
and an element conversion can mutate later values without extending the
captured range.

The receiver and argument slice are pinned before any observable operation.
The boxed object remains pinned for the whole scan, and each fetched element is
pinned across `ToString`; one outer cleanup restores persistent roots after
normal, property, conversion, allocation, or fuel completion. One fuel unit is
charged per logical index. The Rust string builder uses `try_reserve` before
each separator and element append so capacity overflow or allocation refusal
becomes a catchable `RangeError` rather than a host panic. Active receiver
identities bound cyclic element `toString`/`join` re-entry only after each
call's separator coercion. This preserves valid finite re-entry from a
separator while direct or indirect element cycles contribute an empty field.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot joining with a specification-shaped generic, live, GC-safe, fuel-bounded, and allocation-aware traversal.
- 기존 구현 및 제약 조건: The old method coerced the separator before the receiver and length, accepted only represented Arrays, cloned dense storage, ignored inheritance and Proxy reads, swallowed element conversion errors, observed no later mutation, and had no explicit fuel or pin discipline.
- 검토한 주요 대안: Patch only detached calls, retain a dense fast path, collect all element strings before joining, reuse TypedArray join, impose a fixed source-length cap, or build the result incrementally from generic indexed reads.
- 선택한 방식: Perform ToObject, one LengthOfArrayLike snapshot, separator ToString, then live Get and immediate element ToString while pinning native-frame roots, charging every index, reserving each string append fallibly, and tracking active receiver identities to suppress cyclic native re-entry.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable mutation, inheritance, Proxy, and abrupt order; TypedArray join has distinct receiver validation; and a fixed index cap rejects valid sparse programs that cooperative fuel can bound. Incremental construction follows the specification without retaining a second element snapshot.
- 장점, 단점 및 영향: Direct Array join is 23/23 and adjacent TypedArray join remains 32/32 on the fixed corpus; abrupt and fuel exits restore pin depth, finite separator re-entry remains observable, and direct or indirect element cycles cannot overflow the native stack. Runtime and output work remain linear in captured length and produced bytes, while configured fuel and checked reservation prevent unbounded native traversal or String capacity panic. Final conversion of the completed Rust String into Arc<str> still follows the runtime-wide infallible allocation model. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array map

`Array.prototype.map` boxes its receiver, snapshots `LengthOfArrayLike`,
validates the callback, and creates the species result at that captured length
before indexed work. It then performs live `HasProperty` and `Get` operations,
calls the mapper with `(value, index, object)` and the supplied `thisArg`, and
defines the mapped value at the same index. Holes remain holes, inherited
values are visited, and callback mutation affects only unvisited indices
inside the captured range.

The receiver, arguments, boxed object, and species result remain roots for the
operation. Each fetched value is pinned across callback execution, and each
mapped result is pinned across the potentially observable species-target
definition. One fuel unit is charged for every captured source index. Creating
an intrinsic dense result also uses the existing Array length materialization
meter, so a three-element ordinary map consumes three creation units plus
three scan units; custom sparse species results pay their own constructor work.
All normal and abrupt exits restore the incoming pin depth.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot mapping with specification-shaped generic, live, species-aware, GC-safe, and fuel-bounded traversal.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, copied dense storage, invoked callbacks for holes, ignored inheritance, mutation, Proxy operations, species, detached receiver errors, and method-Realm result allocation, then allocated the result only after every callback.
- 검토한 주요 대안: Patch only detached calls, retain a dense fast path, collect mapped values before allocation, share filter with an index-preserving mode, or implement the direct indexed algorithm.
- 선택한 방식: Perform ToObject, one length snapshot, callback validation, ArraySpeciesCreate(length), then live HasProperty/Get/callback/CreateDataPropertyOrThrow while explicitly rooting every native-frame value and metering result materialization plus each logical index.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable holes, mutation, inheritance, Proxy, allocation, and abrupt order; adapting filter would hide map's fixed-index and captured-length result contract. The direct loop mirrors the specification and reuses the established species helper without a second materialized list.
- 장점, 단점 및 영향: Direct Array map is 216/216 and adjacent TypedArray map remains 85/85 on the fixed corpus; forced GC, Proxy definitions, callback errors, species allocation failures, and fuel aborts preserve roots and cleanup. Runtime remains linear in captured length as required, with cooperative fuel bounding sparse scans. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array reduce

`Array.prototype.reduce` boxes its receiver, snapshots `LengthOfArrayLike`, and
validates the callback before inspecting indexed properties. An explicitly
provided initial value is used even when it is `undefined`; otherwise the
method scans upward with live `HasProperty` and `Get` operations until it finds
the first accumulator. The remaining captured indices are visited in ascending
order, skipping absent properties and calling the reducer with
`(accumulator, value, index, object)` and `undefined` as the call this-value.

Receiver, arguments, and boxed object remain roots for the operation. The
current value is pinned across callback execution. A callback result is pinned,
then the temporary new root and previous accumulator root are removed together
in LIFO order before exactly one persistent root is installed for the new
accumulator. This keeps native root storage O(1) while preserving the live
accumulator across later Proxy traps, getters, forced GC, and abrupt exits. One
fuel unit is charged for every examined logical index, including holes during
omitted-initial discovery.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot reduction with specification-shaped generic, live, GC-safe, and fuel-bounded ascending traversal.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, copied dense storage, treated a leading hole as undefined, invoked callbacks for holes, ignored inheritance and mutation, accepted invalid detached receivers, used a non-standard third argument as callback this, and did not root a changing accumulator.
- 검토한 주요 대안: Patch only detached calls, retain a dense Array fast path, precompute present values, share one direction-parameterized core with the still-unrepaired reduceRight, or implement reduce's direct ascending algorithm first.
- 선택한 방식: Perform ToObject, one length snapshot, callback validation, live omitted-initial discovery, then one ascending HasProperty/Get/callback loop with explicit current-value and accumulator root ownership and one fuel charge per examined index.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable holes, inheritance, mutation, and Proxy order. Combining reduceRight before its independent baseline and descending-order audit would widen this unit. The direct loop mirrors the specification and keeps the next change reviewable.
- 장점, 단점 및 영향: Direct Array reduce is 260/260 and adjacent TypedArray reduce remains 50/50 on the fixed corpus; object-to-object accumulator replacement, forced GC, fuel aborts, property errors, callback throws, and empty-without-initial errors restore pin depth. Runtime remains linear in captured length with O(1) native temporary roots. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array reduceRight

`Array.prototype.reduceRight` boxes its receiver, snapshots
`LengthOfArrayLike`, and validates the callback before inspecting indexed
properties. An explicitly provided initial value is used even when it is
`undefined`; otherwise the method scans downward with live `HasProperty` and
`Get` operations until it finds the first accumulator. Remaining captured
indices are visited in descending order, skipping absent properties and
calling the reducer with `(accumulator, value, index, object)` and `undefined`
as the call this-value.

The receiver, arguments, boxed object, current value, and changing accumulator
follow the same explicit root ownership as ascending reduce. The descending
loops hold the next index as an exclusive upper bound and decrement before
property access, so index zero is examined exactly once without unsigned
underflow. A callback result temporarily becomes the newest root; that root
and the old accumulator root are popped together before exactly one persistent
new accumulator root is installed. One fuel unit is charged for every examined
logical index, including holes during omitted-initial discovery.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot reduction with specification-shaped generic, live, GC-safe, and fuel-bounded descending traversal.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, copied dense storage, selected physical storage rather than live properties, invoked callbacks for holes, ignored inheritance and mutation, accepted invalid detached receivers, used a non-standard third argument as callback this, and did not root a changing accumulator.
- 검토한 주요 대안: Patch only detached calls, reverse a collected value list, parameterize ascending reduce before auditing reverse boundaries, or implement the direct descending algorithm with an exclusive upper-bound index.
- 선택한 방식: Perform ToObject, one length snapshot, callback validation, live omitted-initial discovery, then one descending HasProperty/Get/callback loop with decrement-before-access indexing, explicit current-value and accumulator root ownership, and one fuel charge per examined index.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshots change observable holes, inheritance, mutation, and Proxy order. Premature direction parameterization can hide zero-boundary and omitted-initial position defects. An exclusive upper bound mirrors the specification while making underflow impossible.
- 장점, 단점 및 영향: Direct Array reduceRight is 260/260 and adjacent TypedArray reduceRight remains 50/50 on the fixed corpus; forced GC, fuel aborts, property errors, callback throws, and empty-without-initial errors restore pin depth. Runtime remains linear in captured length with O(1) native temporary roots. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array reverse

`Array.prototype.reverse` boxes its receiver, snapshots `LengthOfArrayLike`,
and processes `floor(length / 2)` lower/upper pairs in place. For each pair it
performs lower `HasProperty` and conditional `Get`, then upper `HasProperty` and
conditional `Get`. Only after those observations does it apply the specified
strict writes and deletion for the pair's four possible existence states.
Thus inherited values participate, holes move as holes, Proxy traps observe the
normative order, and an abrupt mutation retains all preceding partial effects.

The receiver and boxed object remain roots for the operation. A fetched lower
value is pinned before the upper existence check, and both fetched values stay
pinned across strict writes and deletes. Pair-local roots are removed after
every completion before an error propagates. One fuel unit is charged per pair;
the method materializes no index list, so huge sparse lengths remain bounded by
cooperative fuel rather than an implementation-only length cap.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array storage reversal with the specification-shaped generic, sparse, observable, GC-safe, and fuel-bounded in-place algorithm.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, reversed dense storage and presence bits without internal property operations, returned undefined for generic or primitive receivers, ignored inheritance and Proxy traps, could not report strict Set/Delete failures, and exposed no cooperative work bound.
- 검토한 주요 대안: Patch only detached calls, collect all indexed values before rewriting, retain a dense fast path, reuse the sorting collector, or implement the direct four-state pair algorithm.
- 선택한 방식: Perform ToObject and one length snapshot, then for each pair execute ordered HasProperty/Get observations and the exact Set/Delete branch while rooting fetched values and charging one pair fuel unit.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot, dense, and sorting paths reorder Proxy effects, erase holes or inheritance, and prevent specification-required partial mutations. The direct pair state machine is allocation-free and maps each observable step to the normative algorithm.
- 장점, 단점 및 영향: Direct Array reverse is 18/18 and adjacent TypedArray reverse remains 22/22 on the fixed corpus; forced GC and abrupt Has/Get/Set/Delete paths restore pin depth. Runtime is linear in half the captured length and can partially mutate before an error as required. ToReversed, toSpliced, and other copy-by-value methods remain independent units.
```

## Iterative OrdinaryHasInstance traversal

`InstanceofOperator` roots its left and right operands before any
`@@hasInstance` lookup can re-enter JavaScript. `OrdinaryHasInstance` then
uses one iterative state machine for Bound Function forwarding and prototype
walking. Bound targets are observed only after an edge fuel debit. Ordinary
prototype edges are debited before `[[GetPrototypeOf]]`; Proxy edges retain
the traversal helper's internal debit so one logical edge is never charged
twice. The walk performs `[[GetPrototypeOf]]`, tests for `null`, and only then
applies `SameValue` to the constructor prototype.

Calls that reach the exact default
`%Function.prototype%[@@hasInstance]` through transparent Bound or Proxy
wrappers trampoline back into the state machine with their transformed
`this`, first argument, and intrinsic Realm. Observable Proxy `apply` traps
remain ordinary calls. Interpreted calls retain the 512-frame VM limit;
every native dispatch participates in an independent 128-frame active-native
limit, so an apply trap that recursively calls `Reflect.apply` produces a
catchable `RangeError` instead of exhausting the Rust stack. This broader
guard can also reject otherwise valid builtin/callback native re-entry deeper
than 128 even when interpreted depth remains available. Both counters and all
operation roots are restored on normal and abrupt exits.

```text
[Decision Log]
- 목적과 의도: Make instanceof specification-ordered, stack-safe, fuel-bounded, Realm-correct, and GC-safe across deep Bound and Proxy wrapper graphs.
- 기존 구현 및 제약 조건: OrdinaryHasInstance recursively followed Bound targets, operands and the constructor prototype could become stale across observable work, ordinary prototype edges were unmetered, comparison before GetPrototypeOf accepted a constructor prototype as its own instance, and recursive native Proxy apply traps could still overflow the host stack.
- 검토한 주요 대안: Add only a recursion cap, flatten Bound targets at bind creation, bypass every Proxy wrapper, charge both the caller and shared property traversal, or reuse the interpreted frame counter for native calls.
- 선택한 방식: Root the operation state, iterate Bound/default-handler forwarding and prototype traversal, preserve observable Proxy apply behavior, debit each edge exactly once, order GetPrototypeOf before SameValue, and cap native re-entry independently from interpreted frames.
- 다른 대안 대신 이 방식을 선택한 이유: A depth cap rejects valid deep wrapper chains, eager flattening changes observable target identity and metadata, broad Proxy bypass skips apply traps, duplicate debits make fuel depend on implementation layering, and one shared depth counter would reduce valid interpreted recursion to protect a different host-stack boundary.
- 장점, 단점 및 영향: Fifty-thousand-layer Bound chains complete without native recursion, self-prototype and Proxy ordering match the specification, hostile native re-entry becomes a catchable error, and exact fuel, GC, Realm, abrupt identity, and cleanup are covered. The 128 limit applies to all active native dispatch, so sufficiently deep valid native builtin/callback re-entry now throws before the 512 interpreted-frame boundary. Nested property and GetPrototypeOf scratch allocation is still only partially fallible and remains a separate runtime-wide allocation unit.
```

## Fallible Proxy prototype validation state

Proxy `[[GetPrototypeOf]]` remains iterative across transparent and validating
targets. Each root it owns directly is now preceded by an exact
`try_reserve_gc_pins`: the input after object validation, target/handler after
the edge fuel debit, trap after `GetMethod`, returned object after type
validation, and deferred expected prototype after nested `IsExtensible`.
Deferred entries reserve scratch storage before publishing their root, then
validate in reverse after the terminal target prototype is known.

The nested Proxy `[[IsExtensible]]` walk applies the same fallible root rule.
It records the first Boolean trap result and whether any later result differs,
rather than retaining every Boolean in a vector. Validation still waits until
the terminal target result is observed, so a deeper revoked proxy or abrupt
trap completion outranks an already-known mismatch exactly as before.

```text
[Decision Log]
- 목적과 의도: Turn directly owned Proxy prototype-validation scratch and root growth into catchable sandbox errors without changing ECMAScript observation order.
- 기존 구현 및 제약 조건: GetPrototypeOf pinned five classes of values through infallible Vec pushes, appended deferred prototypes after pinning, and called an IsExtensible implementation that retained an unbounded Vec<bool> and used more infallible pins. Ordinary Result errors cleaned up correctly, but allocator failure could bypass that control flow.
- 검토한 주요 대안: Reserve all capacity at entry, use one broad depth cap, make pin globally fallible in the same change, retain Vec<bool> with try_reserve, or stage exact reserves at the operations that acquire each value.
- 선택한 방식: Reserve each root at its current semantic boundary, reserve deferred scratch before pin and push, use exact test-only site failpoints, and reduce IsExtensible trap-result storage to a delayed O(1) consistency summary.
- 다른 대안 대신 이 방식을 선택한 이유: Entry preallocation can pre-empt revocation, fuel, getter, call, type, or invariant errors; a depth cap rejects valid chains; a global pin API migration crosses every VM subsystem; and retaining every Boolean allocates despite only equality with one terminal result mattering.
- 장점, 단점 및 영향: Direct reserve failures are catchable, Realm-correct, ordered, and leak-free; later failures release earlier deferred roots; null continuations need no object root; and validating chains no longer allocate result Booleans per layer. This does not make the transitive path fully allocator-fallible: PropertyTraversal HashSets and pins, trap execution, PropertyKey and Error strings, GC root enumeration, and mark worklists remain separate units.
```

## Fallible shared property traversal state

`PropertyTraversal` construction reserves its initial object-identity set and
the caller-owned GC root suffix before callers publish pins. Advancing a new
edge preserves semantic priority: ordinary fuel or credit is consumed first,
duplicate ordinary or Proxy replay handling runs next, and only a genuinely
new edge reserves the edge set. A newly reached node additionally reserves the
node set and GC root storage before the edge, node, or pin is committed.
Reservation failure therefore cannot leave a half-visible edge or leak a root.

Lazy `for...in` cannot use operation-local traversal state because each public
`next()` returns after one key. Its iterator-owned edge set, rooted-node set,
traced root vector, Proxy marker, and replay count persist across calls. This
closes the case where a cyclic Proxy returned itself while producing a fresh
key on every pull and previously obtained a new 512-replay budget each time.
An abrupt `next()` remains a completed operation: a later call re-observes the
Proxy prototype trap rather than caching its prior result across the error.

```text
[Decision Log]
- 목적과 의도: Make shared property-chain state allocation catchable and atomic, and enforce one cycle-replay budget across the complete lifetime of a lazy for-in traversal.
- 기존 구현 및 제약 조건: PropertyTraversal collected initial nodes and inserted edges and roots through infallible HashSet/Vec growth. Lazy for-in recreated that traversal on every next call, so a self-returning Proxy with one fresh key per pull could bypass the documented replay guard. Persisting raw heap indices without traced Values would allow GC slot reuse to change their identity.
- 검토한 주요 대안: Reserve a large fixed capacity at entry, impose a prototype depth cap, make every VM pin globally fallible in one patch, keep operation-local for-in traversal, cache a successful Proxy prototype result across a thrown allocation error, or persist only numeric heap indices.
- 선택한 방식: Reserve exact local capacity immediately before each state publication, keep caller initial roots in the existing pin suffix, store lazy for-in traversal state and corresponding Values in IteratorData, trace those Values in both GC iterator paths, and release collection capacity at terminal completion.
- 다른 대안 대신 이 방식을 선택한 이유: Entry preallocation changes failure priority and can over-allocate; a depth cap rejects legal acyclic chains; a global pin migration crosses unrelated subsystems; operation-local state resets cycle protection; caching across an abrupt operation suppresses later Proxy observations; and untraced indices become stale after collection.
- 장점, 단점 및 영향: Get, HasProperty, Set, inherited Proxy GetMethod, and for-in edge growth now fail through ordinary Result cleanup with retryable state, exact fuel and cycle ordering, Realm-correct errors, and stable GC identities. Persistent for-in state uses O(depth) memory while active and releases that capacity when done. Key snapshots, visited-key growth, trap-call internals, PropertyKey/Error strings, GC root enumeration, and mark worklists remain explicitly outside this unit.
```

## Fallible lazy for-in key state

After `OwnPropertyKeys` completes, lazy `for...in` counts only string keys and
reserves the iterator-owned snapshot before replacing any prior state. The
snapshot is published by consuming the returned key vector directly, so there
is no second infallible temporary collection. A symbol-only result requires no
reservation. Snapshot failure leaves the iterator unvisited and causes the
next pull to re-observe `OwnPropertyKeys`.

For each consumed candidate, `[[GetOwnProperty]]` runs before visited-key
growth. An absent descriptor does not mark the name, while an existing
descriptor reserves the visited set before insertion. Reservation failure
leaves the mark uncommitted but retains the consumed candidate cursor, because
the specification removes the candidate before descriptor lookup and the
existing fuel and descriptor abrupt paths have the same progression. A retry
can therefore observe a same-name prototype property. Already visited names
skip both descriptor lookup and reservation. Terminal completion releases the
snapshot and visited-set capacities; prototype transitions retain capacity for
reuse during the active traversal.

```text
[Decision Log]
- 목적과 의도: Make the two key collections directly owned by a lazy for-in iterator allocator-fallible without changing observation, shadowing, fuel, or retry order.
- 기존 구현 및 제약 조건: OwnPropertyKeys results were filtered through an infallible temporary Vec and assigned to remaining_keys without reservation, while every existing descriptor inserted into visited_keys through infallible IndexSet growth. Iterator pulls deliberately preserve consumed-candidate progression across fuel and descriptor abrupt completions.
- 검토한 주요 대안: Reserve for all returned keys, build a second filtered vector, roll back the cursor on visited reservation failure, mark absent descriptors, reserve on duplicate prototype names, or combine Proxy own-key frames and GC worklists into the same patch.
- 선택한 방식: Count string keys without allocation, reserve the snapshot immediately before publication, consume the returned PropertyKey vector directly, and reserve a new visited entry only after an existing descriptor is observed. Keep the already consumed cursor on failure and release both capacities only at terminal completion.
- 다른 대안 대신 이 방식을 선택한 이유: Reserving symbols changes failure behavior for discarded data; a second vector retains an infallible allocation; cursor rollback repeats a key that the specification already removed and disagrees with existing abrupt progression; absent and duplicate keys need no new state; and broader allocator ownership would obscure this exact boundary.
- 장점, 단점 및 영향: Snapshot and visited growth now produce catchable, Realm-correct errors with atomic collection publication, exact no-op boundaries, stable Proxy and fuel ordering, and terminal capacity release. A failed child visited mark intentionally permits a same-name prototype key on retry. Proxy own-key trap-result vectors and duplicate sets, pending validation frames, filtered results, PropertyKey/Error strings, GC root enumeration, and mark worklists remain separate units.
```

## Fallible Proxy ownKeys entry collection

`CreateListFromArrayLike` for a Proxy `ownKeys` result remains incremental. Each
logical index consumes fuel, performs `Get`, and validates that the value is a
String or Symbol before the native key vector checks whether the next push
would exceed capacity. Only a full vector requests capacity and can fail;
spare-capacity pushes publish directly. The operation does not preallocate from
`length`, because doing so could fail before observable index access on a large
array-like result.

Duplicate validation remains a second pass after every index has been read.
For each key, membership is checked before growth; an existing key therefore
throws the required duplicate `TypeError` without reserving. A new key reserves
the `IndexSet` before insertion only when the set is full; spare capacity does
not create a failure boundary. String/Symbol consumer filtering and target
invariant checks happen later. Any reservation failure discards the
operation-local collections and unwinds all owned pins, so a retry starts from
the `ownKeys` trap and cannot observe a partial list.

```text
[Decision Log]
- 목적과 의도: Make Proxy ownKeys trap-result keys and duplicate-detection entries allocator-fallible while preserving CreateListFromArrayLike and Proxy invariant observation order.
- 기존 구현 및 제약 조건: The trap-result Vec pushed every validated String or Symbol through infallible growth, and the later IndexSet used infallible insert. The array-like length can reach MAX_SAFE_INTEGER, every index Get is observable and fuel-bounded, duplicate validation must wait until all entries are collected, and consumer filtering occurs only after Proxy invariants.
- 검토한 주요 대안: Reserve the complete reported length, combine collection with duplicate validation, reserve before Get or type validation, reserve before checking membership, impose a fixed key limit, or include pending frames and target-key sets in the same patch.
- 선택한 방식: After each successful Get and key-type validation, request Vec capacity only if the next push would exceed current capacity; keep the complete-list duplicate pass, test membership first, and request IndexSet capacity only for a new key when the set is full. Translate either actual growth failure into the current operation Realm's RangeError.
- 다른 대안 대신 이 방식을 선택한 이유: Length preallocation and early reservation change getter and error priority; fused duplicate detection can suppress later entry errors; reserving duplicates creates failures for state that will not grow; a fixed cap rejects valid programs; and broader ownership boundaries would make retry and cleanup evidence harder to isolate.
- 장점, 단점 및 영향: Both directly owned entry collections now fail through ordinary Result cleanup only at real growth boundaries, with exact fuel, getter, type, duplicate, Symbol, Realm, retry, nested-frame, and for-in snapshot behavior. Helper-level tests fill the allocator-reported capacity and prove spare slots cannot consume a failure. Reservation remains amortized, and contains plus insert hashes each unique key twice. Operation input, target/handler, trap-result list, and length-value roots, pending validation frames and roots, filtered vectors, non-extensible target sets, index strings, PropertyKey/Error strings, GC root enumeration, and mark worklists remain separate units.
```

## Fallible Proxy ownKeys validation frames

A trapped `ownKeys` layer cannot validate target invariants until the innermost
target key list is known. After that layer has collected and duplicate-checked
its trap result and completed `IsExtensible(target)`, the VM first requests
capacity for one additional pending frame. It then reserves the GC roots
required by the frame's `object` and `target`, pins the same two values, and
publishes the frame. No fallible operation remains between pinning and push.

Frame or root reservation failure occurs before `current` advances to the
target, so nested `[[OwnPropertyKeys]]` and later descriptor/invariant work do
not begin. Every already published outer frame remains covered by
`pending_pins` and is unwound with the operation root. Transparent forwarding
does not create a frame; a trapped empty result still does because omission and
non-extensible exact-set invariants remain applicable.

```text
[Decision Log]
- 목적과 의도: Make Proxy ownKeys pending validation-frame publication allocator-fallible and atomic without changing trap, invariant, nested traversal, or retry order.
- 기존 구현 및 제약 조건: Each validating Proxy layer pinned current and target through infallible gc_pins growth and then pushed a PendingProxyKeys frame through infallible Vec growth. Frames must survive later nested ownKeys calls, forced GC, and reverse invariant validation, while transparent layers require no frame.
- 검토한 주요 대안: Reserve roots before frame capacity, pin before either reserve, preallocate a fixed maximum chain, store only heap indices, merge frame state into recursive calls, or combine every remaining ownKeys root and result collection in this patch.
- 선택한 방식: After successful trap-list, duplicate, and IsExtensible work, request capacity for one additional frame, reserve roots from the exact current/target Value pair, pin that pair, push the frame, and only then advance current and filters.
- 다른 대안 대신 이 방식을 선택한 이유: Reserving roots first can leave unnecessary global capacity after local frame failure; pinning before reserve can leak on allocation error; fixed caps reject valid chains; untraced indices can become stale after GC; recursion risks host-stack failure; and broader ownership would obscure this publication boundary.
- 장점, 단점 및 영향: Frame and root failures are catchable, Realm-correct, retryable, and leak-free after all earlier observations but before nested target work. Nested countdown and 1,024-layer trapped-chain tests cover existing-frame cleanup and iterative growth. Operation input, target/handler, trap-result list, and length-value roots, filtered vectors, non-extensible target sets, index strings, PropertyKey/Error strings, GC root enumeration, and mark worklists remain separate units.
```

## Fallible Proxy ownKeys direct roots

The iterative `[[OwnPropertyKeys]]` operation now reserves each root set at the
boundary where it becomes owned. The input is reserved before its first pin.
After a Proxy is proven live, its target and handler are reserved after the
Proxy-edge fuel debit and before trap lookup. A trap result is first validated
as an object and then reserved before reading `length`; an object-valued length
is reserved after `Get` and before `ToNumber` can invoke user code.

One shared helper counts the roots contributed by each `Value` before touching
the GC-pin vector. Primitive inputs and lengths therefore perform no reserve,
and a missing or nullish trap forwards without creating trap-result state.
Operation-root failure precedes revocation and fuel because the operation must
own its input before dispatch, while layer-root failure follows revocation and
the edge fuel debit. Ordinary result cleanup unwinds every pin and previously
published validation frame when any later reservation fails.

```text
[Decision Log]
- 목적과 의도: Make every temporary GC root directly owned by Proxy ownKeys allocator-fallible at the exact point where the operation assumes ownership.
- 기존 구현 및 제약 조건: The operation input, Proxy target/handler, trap-result object, and object-valued length were pinned through infallible gc_pins growth. Their observation boundaries differ, primitive Values contribute no roots, and nested Proxy validation may already have published outer frames when an inner list or length fails.
- 검토한 주요 대안: Reserve a fixed root budget at entry, reserve every Value including primitives, move target/handler reservation before fuel, pin first and rely on cleanup, keep only injected site tests, or combine post-validation key collections and GC internals into this patch.
- 선택한 방식: Use one root-count-aware reservation helper immediately before each pin, retain the existing observation order around revocation, fuel, Call, list validation, length Get, and ToNumber, and verify both exact sites and the real GC-pin reserve path.
- 다른 대안 대신 이 방식을 선택한 이유: Entry preallocation over-reserves paths never taken and changes failure priority; reserving primitives creates spurious failures; moving layer reservation changes fuel order; pin-first can abort during vector growth; synthetic sites alone do not prove the production reserve path; and broader allocator ownership would obscure cleanup and retry evidence.
- 장점, 단점 및 영향: All directly owned ownKeys roots now fail through catchable, Realm-correct RangeError paths with primitive/nullish no-op behavior, nested cleanup, caller retry, forced-GC survival, and for-in snapshot atomicity. Root counting adds a bounded scan over at most two Values per site. Filtered output growth, non-extensible target-key sets, index and PropertyKey/Error strings, GC root enumeration, and mark worklists remain independent units.
```

## Fallible Proxy ownKeys post-validation collections

Reverse validation first observes every target descriptor and omission rule.
Only after that work succeeds does a non-extensible frame reserve its complete
target-key `IndexSet` and publish the keys used for exact-set comparison. This
keeps descriptor throws and missing non-configurable-key errors ahead of any
allocation failure while placing reservation before native set growth.

Consumer filtering remains a later pass. Each candidate consumes its existing
fuel, applies the String/Symbol filter, and optionally observes
`[[GetOwnProperty]]` and enumerability before requesting capacity. The helper
checks `len == capacity`, so an injected or real reserve failure is possible
only when the next accepted key would actually grow the result vector. A
failure discards the operation-local partial result; caller retry re-observes
the complete Proxy operation and never publishes a partial `for...in`
snapshot.

```text
[Decision Log]
- 목적과 의도: Make the two post-validation collections directly owned by Proxy ownKeys allocator-fallible without changing invariant validation, consumer filtering, or retry semantics.
- 기존 구현 및 제약 조건: The non-extensible target-key IndexSet collected a fully observed target list through infallible growth, and the filtered result Vec pushed accepted keys infallibly after per-key fuel and optional descriptor lookup. Reverse Proxy frames can re-enter descriptor traps, and for-in must not publish a partial key snapshot.
- 검토한 주요 대안: Reserve target keys before descriptor traversal, reserve the trap-result length for every frame, reserve before filtering or descriptor lookup, request capacity before every accepted push even when spare capacity exists, impose a fixed key cap, or combine shared string and GC-worklist allocation in the same patch.
- 선택한 방식: Reserve the target-key set once after all descriptor and omission validation but before insertion and exact-set comparison; reserve the filtered Vec only after a key passes every filter and only when its next push requires growth.
- 다른 대안 대신 이 방식을 선택한 이유: Earlier reservation changes abrupt-completion priority and can over-allocate discarded keys; unconditional failpoints model failures where the allocator is never called; fixed caps reject valid programs; and broader allocation ownership would obscure the reverse-frame, Realm, and atomic-retry boundary.
- 장점, 단점 및 영향: Both collections now report catchable, operation-Realm RangeError with exact no-growth exclusions, completed descriptor observations, reverse-frame cleanup, caller retry, and for-in snapshot atomicity. Target-set reservation is O(number of target keys), and filtered growth remains amortized. Shared index and PropertyKey/Error strings, ordinary own-key producers, GC root enumeration, and mark worklists remain independent units.
```

## Fallible ordinary own-key collections

`ordinary_own_property_keys` retains its up-front work charge: TypedArray
indices, stored properties, Array presence slots, Module Namespace exports,
and a String byte-length upper bound for UTF-16 keys contribute to the scan
estimate. The full precomputed work charge is consumed before native key
materialization starts. Each accepted candidate then checks the capacity of
its type-specific index, String, or Symbol staging vector and requests one
additional slot only when full.

Sorted index keys, insertion-ordered strings, and symbols are published into a
single final result. Membership is checked first; for a new key, both the
duplicate `IndexSet` and result `Vec` reserve before either is mutated. A
duplicate such as a materialized Array `length` alongside the synthetic length
therefore consumes no final reservation. Any failure discards all local state,
unwinds the operation root, and leaves a lazy `for...in` snapshot unpublished
so caller retry re-runs the ordinary key snapshot.

```text
[Decision Log]
- 목적과 의도: Make every native collection directly owned by ordinary [[OwnPropertyKeys]] allocator-fallible at its real growth boundary without changing fuel, key order, filtering, Proxy invariant, or retry semantics.
- 기존 구현 및 제약 조건: Index, String, and Symbol candidates were pushed into three infallible staging Vecs, while final deduplication inserted into an infallible IndexSet and Vec. Fuel is intentionally prepaid before materialization; Array and boxed String synthesize length, Module Namespace exports have specified ordering, and a pending Proxy validates descriptors only after the ordinary target snapshot succeeds.
- 검토한 주요 대안: Reserve from the precomputed work count, preallocate all five collections at entry, remove staging vectors in a broad rewrite, impose a key-count cap, combine numeric/Arc string allocation, or reserve seen and result after mutating one side.
- 선택한 방식: Check len against capacity immediately before each accepted staging push; after sorting and filtering, check final membership, reserve both seen and result when needed, then publish to both collections. Keep all string construction and caller-owned conversion containers outside this unit.
- 다른 대안 대신 이 방식을 선택한 이유: Work counts include holes, excluded keys, duplicates, and different key classes, so bulk reservation creates false failures and over-allocation; a broad rewrite obscures ordering; fixed caps reject valid programs; stable Rust has no fallible Arc<str> construction; and one-sided publication complicates atomic cleanup evidence.
- 장점, 단점 및 영향: Ordinary objects, Arrays, primitive and boxed Strings, TypedArrays, Symbols, and Module Namespace exports now share catchable, Realm-correct growth failure with exact fuel, duplicate, no-op, Proxy-order, retry, and for-in atomicity evidence. The three staging vectors plus final Vec/IndexSet remain O(number of candidates). Numeric formatting, PropertyKey/Error Arc strings, caller result containers, GC root enumeration, and mark worklists remain independent scopes.
```

## Fallible own-key consumer materialization

The six public key-array consumers own a second publication layer after
`[[OwnPropertyKeys]]`: `Object.keys`, `Object.values`, `Object.entries`,
`Object.getOwnPropertyNames`, `Object.getOwnPropertySymbols`, and
`Reflect.ownKeys`. Each accepted result now requests vector capacity only when
full. Keys reserve after their enumerable descriptor succeeds; values reserve
after `Get`, then reserve GC-pin capacity before `pin -> push`; entries reserve
their two pair elements after `Get`, create the pair in the method Realm, then
reserve the outer vector and pair root before publication.

Names, Symbols, and Reflect conversion perform no descriptor or value access.
Empty and filtered lists therefore create an empty Array without consuming a
result or presence reservation. Non-empty result Arrays use
`ArrayData::try_new`, which reserves the dense presence bitmap before resize.
`make_value_array_in_env` computes and reserves every object root before
pinning, and all Realm-explicit callers delegate to that path. Consequently a
foreign `Reflect.ownKeys` now receives its own Realm's `%Array.prototype%`.

```text
[Decision Log]
- 목적과 의도: Make the complete result-materialization layer of the six public own-key consumers catchable and Realm-correct without changing snapshot, descriptor, Get, ordering, or partial-observation semantics.
- 기존 구현 및 제약 조건: The producer snapshot was fallible, but consumer Vec growth, Object.entries pair storage, some pins, a temporary root Vec, and ArrayData's presence bitmap were infallible. Reflect.ownKeys used the main Realm Array, while keys/values/entries must observe descriptors and Gets in snapshot order.
- 검토한 주요 대안: Preallocate from snapshot length, reserve before descriptor/Get, share one bulk conversion for all consumers, leave Array presence as hard OOM, clone entry key strings, or rewrite every ArrayData constructor and own-key caller together.
- 선택한 방식: Reserve each accepted consumer publication at its actual growth boundary; reserve object roots before pinning; treat entries pair elements and outer pairs independently; route Realm-explicit arrays through make_value_array_in_env; and add a fallible dense-presence constructor used by that shared Value-array path.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot length overallocates filtered results and changes failure priority; early reservation precedes required observable work; values and entries need different root lifetimes; leaving the bitmap would preserve an end-to-end abort; key ownership removes an unnecessary Arc allocation; and a global Array/caller rewrite exceeds this reviewable boundary.
- 장점, 단점 및 영향: All six APIs now have exact growth, presence, Realm, retry, fuel, GC, and cleanup evidence, and shared Value-array callers gain reserve-before-pin/presence safety. Descriptor result materialization, JSON and descriptor-related own-key caller containers, unrelated direct ArrayData::new constructors, PropertyKey/Error strings, GC root enumeration, and mark worklists remain independent scopes.
```

## Fallible Proxy descriptor traversal state

`own_property_descriptor_for_key_or_throw` evaluates a Proxy
`getOwnPropertyDescriptor` chain iteratively. It now reserves its operation
input before the first pin, each target/handler layer after revocation and the
edge fuel debit, and a callable trap after `GetMethod` and callability
validation. A trapped layer reserves pending-frame capacity only when the
vector is full, then reserves every frame root before publishing the frame.
Transparent forwarding therefore creates neither trap nor pending-frame
state.

Descriptor conversion reserves the returned descriptor object and each
object-valued `value`, `get`, and `set` field at the point ownership begins.
Getter and setter callability errors remain ahead of their root reservations.
Target descriptor fields use a fixed three-value root set across observable
`IsExtensible` work. On the `undefined` trap-result path, an absent target
descriptor returns immediately, a hidden non-configurable descriptor throws
immediately, and only a configurable target descriptor retains fields across
extensibility observation. Reverse validation still processes outer trapped
layers only after the terminal descriptor is known, and every abrupt
completion unwinds operation-local roots and frames.

```text
[Decision Log]
- 목적과 의도: Make every native collection and temporary GC root directly owned by iterative Proxy getOwnPropertyDescriptor traversal allocator-fallible without changing trap, descriptor conversion, invariant, or caller observation order.
- 기존 구현 및 제약 조건: Operation, layer, trap, pending-frame, descriptor-conversion, and validation values were pinned or pushed through infallible Vec growth; nested frames validate in reverse; descriptor Get and IsExtensible can execute user code; and several primitive or terminal error paths need no retained root.
- 검토한 주요 대안: Reserve a maximum root budget at entry, preallocate pending frames from Proxy depth, root every primitive field, reserve before callability or invariant checks, replace all descriptor materialization and callers in one patch, or impose a fixed Proxy-depth limit.
- 선택한 방식: Reserve each root at its semantic ownership boundary, reserve pending storage only when full and before any frame root or publication, retain target descriptor fields in a fixed array across IsExtensible, skip trap/pending sites for transparent forwarding and field-root sites for primitives, and skip validation-descriptor roots for absent or immediately non-configurable targets on the undefined trap-result path.
- 다른 대안 대신 이 방식을 선택한 이유: Entry reservation and primitive rooting introduce false failures; Proxy depth is not known before observable traversal; early reservation changes required error priority; a fixed depth rejects valid programs; and final descriptor objects and caller containers have separate allocation and Realm boundaries.
- 장점, 단점 및 영향: All ten sites now have catchable Realm-correct failure, actual-growth, ordering, nested reverse-validation, GC, retry, and cleanup evidence while successful semantics and Test262 admission remain unchanged. Final FromPropertyDescriptor object construction, Object.getOwnPropertyDescriptors and defineProperties containers, Proxy defineProperty descriptor containers, shared strings, GC root enumeration, and mark worklists remain independent scopes.
```

## Fallible descriptor materialization and definition publication

`ToPropertyDescriptor` now produces a presence-aware internal descriptor
record directly. It retains the input object while observing inherited
`enumerable`, `configurable`, `value`, `writable`, `get`, and `set` fields in
specification order, and roots each newly observed object-valued data or
accessor field before a later callback can collect it. Getter and setter
callability checks remain ahead of publication. This removes the temporary
ordinary descriptor object that previously required a second property walk
and could change observable results between conversion and definition.

`Object.defineProperties` stores the converted records and their field roots
in a fallibly grown operation-local vector, completing every descriptor
conversion before the first target definition. `Object.defineProperty` and
`Reflect.defineProperty` validate the target before reserving their argument
roots, convert once, and pass the same record to ordinary, Array,
TypedArray, Module Namespace, mapped-arguments, or Proxy definition. A Proxy
creates its descriptor object only after revocation, fuel, trap lookup, and
callability succeed; the exact present-property count is reserved before the
object is allocated, and target descriptor fields remain rooted across
invariant validation.

`FromPropertyDescriptor` reserves its fixed four-property map and exact value,
getter, setter, and Realm-prototype roots before allocation.
`Object.getOwnPropertyDescriptors` obtains `[[OwnPropertyKeys]]` before
allocating the result object, then reserves each accepted result property and
materialized descriptor root before publication. Both paths require the
current Realm's registered `%Object.prototype%` rather than silently falling
back to the main Realm.

```text
[Decision Log]
- 목적과 의도: Make descriptor conversion, object materialization, public Object/Reflect callers, and Proxy defineProperty publication allocator-fallible while preserving ECMAScript field observation, two-pass definition, invariant, Realm, and retry semantics.
- 기존 구현 및 제약 조건: ToPropertyDescriptor first allocated a normalized JS object and later reread its own properties; Object.defineProperties retained those objects in an infallible Vec; FromPropertyDescriptor and getOwnPropertyDescriptors inserted through infallible maps; Proxy defineProperty eagerly reused or built descriptor objects through infallible property insertion. Every field Get, ownKeys trap, defineProperty trap, and invariant check can execute user code and trigger GC.
- 검토한 주요 대안: Preallocate from own-key counts, keep the normalized object as the internal record, reserve all roots and properties at public-call entry, create Proxy descriptor objects before trap lookup, impose fixed descriptor or key limits, or combine ordinary property storage, JSON, shared strings, and GC worklists in the same patch.
- 선택한 방식: Convert once into a presence-aware Rust record in specification field order; reserve and root observed object fields at ownership boundaries; retain records through the complete first defineProperties pass; reserve maps and vectors only at actual growth; materialize Realm-correct descriptor objects only when their caller requires them; and allocate getOwnPropertyDescriptors output only after ownKeys succeeds.
- 다른 대안 대신 이 방식을 선택한 이유: Rereading a normalized object duplicates observable semantics and allocation; count-based entry reservation over-allocates filtered or absent fields and changes abrupt-completion priority; eager Proxy materialization runs before required trap errors; fixed limits reject valid programs; and broader container ownership would obscure the conversion/publication boundary.
- 장점, 단점 및 영향: Descriptor fields now have one observable conversion, the defineProperties conversion pass completes before the first target mutation, public and Proxy output is Realm-correct, and every directly owned root or collection has exact growth, GC, cleanup, and retry evidence. Ordinary object property-map insertion, Array backing and length storage, ordinary Set/set_array_index, seal/freeze materialization, TypedArray byte conversion, JSON containers, unrelated ArrayData constructors, PropertyKey/Error and IC temporary strings, GC root enumeration, and mark worklists remain independent hard-host-OOM scopes.
```

---

**Next:** [Features](features.md) · [Known limitations](limitations.md) · [Back to README](../README.md)
