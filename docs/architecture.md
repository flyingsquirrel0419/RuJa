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
environment's entries from all 31 rooted per-Realm registry families and the
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
- 기존 구현 및 제약 조건: Intrinsic installers publish 31 families of Realm roots incrementally and use fallible LIFO temporary pins; wrapper allocation remains fallible after every registry has been populated.
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

`Array.prototype.map` and `flatMap` keep source snapshots in Rust vectors and
invoke JavaScript once per element. The VM therefore pins every heap value in
the snapshot before the first callback and pins each callback result before the
next callback can collect. `flatMap` retains the mapped container roots while
copying their elements, so tracing the container also preserves nested values.
The roots remain owned by the native operation until the final Array has taken
ownership.

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
- 기존 구현 및 제약 조건: GcIdx is a generation-free cell index, the collector sees only VM roots, and map/flatMap callback results plus the Array.of result lived only in Rust locals across observable calls. A Rust Vec<Value> does not participate in tracing.
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
- 장점, 단점 및 영향: A 100,000-layer delete is stack-safe, forced GC and every abrupt path restore pin depth, and hosts can stop deep handlers or invariant targets with fuel. Unbounded hosts can still spend linear time on arbitrarily deep legal chains, and separately capped Set and DefineOwnProperty paths remain future iterative audits.
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
- 장점, 단점 및 영향: Omitted and explicit undefined keys now agree through ordinary and Proxy paths, local GC and abrupt regressions provide coverage missing from Test262, and the change adds no allocation or fuel policy. Existing deep get, has, set, and receiver-define traversal caps remain separate architecture work.
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
