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

`is_constructor_value` and `construct_with_new_target` iteratively follow
arbitrarily deep BoundFunction and transparent-Proxy chains. Bound arguments
are prepended in wrapper order, BoundFunction `newTarget` substitution occurs
only when required, and a Proxy construct trap receives the still-active
target, argument array, and original forwarded new target. Normal and spread
`super()` execute this same `[[Construct]]` dispatcher rather than the call
path, so bound `this` and Proxy `apply` traps cannot leak into superclass
construction. Construction inputs and temporary trap values remain on
`gc_pins` across every observable or collecting boundary.

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
CreateDynamicFunction owns every observable step, leaving the registration
inventory at **14 eager / 31 deferred** native constructors. Parameter
arguments are converted left-to-right before the body, after which RuJa's local-trust
`HostEnsureCanCompileStrings` policy permits compilation. Parameters and body
are parsed separately with line terminators at both synthetic boundaries; a
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

```text
[Decision Log]
- 목적과 의도: Implement one specification-shaped CreateDynamicFunction path for all four dynamic constructors without weakening Realm identity or the exact heap cap.
- 기존 구현 및 제약 조건: The old wrapper parse allowed synthetic-boundary ambiguity, prototype selection happened on incomplete rules, generated functions used main-Realm parents, raw allocation bypassed the sandbox retry, and compilation-table entries could leak after failure.
- 검토한 주요 대안: Keep four mostly separate constructors, broaden generic native preallocation, parse only one combined wrapper, or centralize conversion, parsing, Realm fallback, publication, and allocation in the existing dynamic body.
- 선택한 방식: Use deferred native construction, three grammar checks with newline boundaries, immutable constructor-Realm registries, post-lookup table publication, and rooted one- or two-cell allocation with suffix rollback.
- 다른 대안 대신 이 방식을 선택한 이유: Generic preallocation observes `NewTarget.prototype` too early, a combined-only parse does not model separate parameter/body grammar, and per-kind copies would drift on ordering and GC cleanup. One kind-parameterized path keeps the shared abstract operation explicit.
- 장점, 단점 및 영향: Call/construct order, all four Realm fallbacks, Bound/Proxy new targets, parser early errors, forced GC, and exact-cap failures now share tested invariants. String compilation remains synchronous and is governed by the local-trust host policy rather than opcode fuel; source-text preservation is a separate limitation.
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
environment's entries from all 29 per-Realm registry families. Native error
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
- 기존 구현 및 제약 조건: Intrinsic installers publish 29 families of Realm roots incrementally and use fallible LIFO temporary pins; wrapper allocation remains fallible after every registry has been populated.
- 검토한 주요 대안: Publish nothing until setup completes; clean only the last inserted map; make every installer independently error-safe; or own all provisional roots and pins in one outer transaction.
- 선택한 방식: Keep provisional registry publication, pin the fresh environment, capture the incoming pin depth, include wrapper attachment in the transaction, truncate the owned pin suffix on every result, and remove every Realm registry entry on error.
- 다른 대안 대신 이 방식을 선택한 이유: Later installers require earlier intrinsic identities, map-specific cleanup misses other roots, and duplicating rollback in every installer creates drift. One lexical owner matches the actual observability boundary.
- 장점, 단점 및 영향: Every hard-cap failure point is reusable and collectible before caller-Realm error materialization. The registry inventory remains manually synchronized across the VM fields, root tracer, rollback helper, and regression counter, so new Realm registries must update all four sites.
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
native contexts additionally own `NewTarget` and any already-observed
`NewTarget.prototype` value.

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

---

**Next:** [Features](features.md) · [Known limitations](limitations.md) · [Back to README](../README.md)
