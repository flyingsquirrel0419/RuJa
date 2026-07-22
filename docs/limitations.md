# Known limitations

## Sandbox guarantees

RuJa is designed for running untrusted JavaScript safely inside a host process.
The following resource limits are enforced:

- **Execution fuel**: `Vm::set_fuel(Some(n))` bounds dispatched opcodes and
  explicitly metered native-loop steps. Proxy `[[Call]]`, `[[Delete]]`,
  `[[Get]]`, `[[Set]]`, `[[GetOwnProperty]]`, `[[DefineOwnProperty]]`,
  `[[IsExtensible]]`, `[[PreventExtensions]]`, `[[GetPrototypeOf]]`, and
  `[[SetPrototypeOf]]`, and `[[OwnPropertyKeys]]` consume one unit per
  traversed Proxy layer, including nested handler and invariant walks.
  Ordinary `[[Get]]`, `[[HasProperty]]`, and `[[Set]]` consume one unit per
  ordinary-to-ordinary prototype edge. An ordinary-to-Proxy edge is not
  charged separately because the Proxy dispatch consumes its own unit. Proxy
  `GetMethod` lookup and trapless Set retain one initial ordinary-edge credit
  so the established exact per-Proxy budgets remain stable; deeper inherited
  handler traversal is still metered. `for...in` precharges ordinary own-key
  snapshots before native collection growth, then charges each string
  candidate and ordinary prototype edge separately.
  `GetFunctionRealm` and actual Bound/Proxy `[[Construct]]` traversal consume
  one unit per followed wrapper edge, with Proxy revocation checked before the
  charge. `IsConstructor` is a constant-time immutable capability read and
  neither follows targets nor consumes fuel.
  Intrinsic Promise resolving functions preserve completed handler, thenable,
  and `then`-access stages across such an abort. Arbitrary species-provided
  capability functions are not automatically replayed after Fuel because the
  engine cannot repeat unknown user effects safely; a user-created output may
  remain pending after that host abort.
  `Array.prototype.concat` charges each outer input, including an empty
  spreadable value, and every scanned logical source index, including holes.
  TypedArray `includes`, `indexOf`, and `lastIndexOf` charge every visited
  logical index after `fromIndex` coercion; an empty search range consumes no
  loop fuel.
  Ordinary `[[SetPrototypeOf]]` cycle detection also consumes one unit per
  visited candidate object. Exhaustion throws a `RangeError("fuel exhausted")`
  that is *not catchable* by user `try/catch` (a host-level abort). `None` =
  unbounded (default).
- **Heap object limit**: `Vm::set_max_heap_objects(Some(n))` caps the number
  of live GC-managed heap objects. When exceeded, allocation throws a
  catchable `RangeError("heap limit exceeded")`. A GC cycle is attempted
  before the error is raised. `None` = unlimited (default).
  At an exact hard limit, Error materialization performs one rooted GC retry
  and then uses an immutable, preallocated `RangeError` from the operation
  Realm. That reserve already counts toward the cap, so an existing Promise or
  dynamic-import capability can reject without allocating over the limit.
  Repeated saturated failures in one Realm share that frozen object's identity
  (`e1 === e2` can be true), while reclaimable capacity still produces fresh
  Error objects. A failure that occurs before a Promise or capability itself
  exists remains a synchronous host resource error because there is no
  JavaScript object available to settle. Test262 Realm construction is
  transactional: a failed intrinsic or wrapper allocation restores temporary
  pin depth and removes every provisional per-Realm registry root before the
  caller Realm's error is materialized. Realm-local Reflect construction now
  uses rooted GC-retrying allocations for its complete 14-object batch. The
  general native-function bootstrap helper is still non-collecting because
  older batch installers do not consistently pin provisional functions;
  runtime Realm installers that still use that helper, including Math, can
  reject at an intermediate raw allocation even when a collection could
  reclaim enough garbage. They require the same per-builder rooting audit
  before the generic path can safely change.
- **Call-stack depth**: JavaScript execution is capped at 512 VM frames.
  Exceeding this throws a catchable `RangeError("Maximum call stack size
  exceeded")`, not a native stack overflow (SIGSEGV/abort).
- **Property traversal cycle guard**: acyclic ordinary and transparent Proxy
  `[[Get]]`, `[[HasProperty]]`, and `[[Set]]` chains are iterative and have no
  fixed depth cutoff. Traversed nodes remain GC roots until the operation
  exits, and directed edges are retained for cycle detection, so temporary
  memory is linear in the traversed depth. Proxy-induced cycles must replay
  repeated edges because each trap lookup is observable and can mutate the
  target on a later pass. RuJa permits 512 such replays, then raises a
  catchable `RangeError("Maximum cyclic property traversal depth exceeded")`
  instead of overflowing the native stack; configured execution fuel can
  abort earlier. This guard applies only to cyclic topology, not to a legal
  acyclic chain. A malformed all-ordinary cycle, which normal ECMAScript APIs
  cannot create, is rejected as soon as a directed edge repeats.
- **Array generic-method coverage**: `%Array.prototype%` is a real
  `ArrayData` exotic, and `push`, `pop`, `shift`, `unshift`, `splice`, `slice`,
  `concat`, `copyWithin`, `fill`, `filter`, `flat`, `flatMap`, `forEach`,
  `join`, `toLocaleString`, `map`, `reduce`, `reduceRight`, `reverse`,
  `toReversed`, `toSpliced`, and `with` now use generic internal property
  operations and logical lengths.
  Slice, Splice, Concat, and Filter implement `ArraySpeciesCreate`; Concat also
  implements `Symbol.isConcatSpreadable`. Flat and FlatMap share an iterative,
  fuel-metered `FlattenIntoArray`, while CopyWithin, Fill, ToReversed,
  ToSpliced, and With intentionally do not consult species. The direct Test262
  `methods-called-as-functions.js` aggregate
  passes when forced through broad feature gates, but remains skipped by normal
  policy and outside exact admission because it spans otherwise independent
  Array method families.
  Infinite-depth flattening permits 512 repeated active-path source visits so
  observable getters can break a cycle, then raises `RangeError`; finite and
  acyclic nesting has no fixed depth cutoff.
- **Test262 result enforcement gap**: the Python runner reports
  fail, timeout, and error counts but still exits with status zero, so a matrix
  job can be green while semantic failures remain. Ordinary CI and all
  full-matrix jobs consume the same repository-pinned Test262 revision, and
  full-matrix setup validates exact TypedArray `toString`, `join`, and locale
  admissions against that checkout before scheduling shards. Aggregate totals
  can still hide pass-to-fail swaps, so until the runner exits nonzero under
  an explicit policy, release audits must download all 30 artifacts, compare
  each file to a known baseline, and investigate every changed shard.
- **Regex execution bounds**: ordinary matching uses the RE2-style,
  linear-time Rust `regex` backend. Backreferences use the vendored
  `fancy-regex` backend; that path has a finite work limit and reports an
  `Invalid regex match` error when exhausted. Repeated-capture patterns use a
  hybrid: the linear matcher prefilters match boundaries, and the bounded
  backend reconstructs captures only for successful matches. Capture clearing
  is charged per slot and uses bitset-backed copy-on-write state. Native
  matching is still cooperative rather than preemptible, so hosts that need a
  hard wall-clock deadline must use a separately killable process.
- **String/array caps**: `"x".repeat(n)` is capped at 256 MiB output.
  `Array.from(iterable)` is capped at 65k elements. Dense arrays are capped
  at 1M elements (`MAX_DENSE_ARRAY_LEN`); the Array constructor, Slice, and
  Concat can represent larger legal lengths sparsely without allocating a
  giant dense vector. `Array.prototype.with` still rejects a captured length
  above 1,048,576 before its indexed scan because it must materialize every
  result position. This sandbox bound is intentionally stricter than
  ECMAScript.
- **Call argument caps**: `Reflect.apply`, `Reflect.construct`, and
  `Function.prototype.apply` share an observable `CreateListFromArrayLike`
  implementation that materializes at most 1,048,576 arguments. Constructor
  dispatch applies the same cap to the combined inner-to-outer Bound arguments
  plus direct call arguments, accumulating the count on each Bound edge before
  a target Proxy's observable `construct` lookup. Both paths throw `RangeError`
  above the limit.
  The array-like cap is checked after `ToLength` truncates and clamps the
  observed `length`, but before any indexed `Get`. This sandbox resource policy
  is stricter than ECMAScript's full `ToLength` range and intentionally prevents
  enormous argument-list allocation.

These limits bound the major interpreter-controlled resources, but they are
not OS-level isolation. A long native call is not preempted by fuel, and the
heap-object cap does not replace a process memory limit. Run hostile code in a
separately killable, memory-limited process when hard wall-clock or OOM
guarantees are required.

- No `eval`/`with` process-level security sandbox (local-trust execution model)
- Crates.io publication is disabled with `package.publish = false` while RuJa
  depends on a vendored `fancy-regex` fork. Cargo rewrites path dependencies to
  their registry versions when packaging, which would silently remove RuJa's
  required backend API and semantics. Publication can be re-enabled only after
  these patches are upstream or the fork is published as a distinct dependency.
- Dynamic Function constructors (`Function`, `AsyncFunction`,
  `GeneratorFunction`, and `AsyncGeneratorFunction`) follow the observable
  CreateDynamicFunction conversion, grammar, Realm, and allocation protocol,
  but the host policy currently permits string compilation unconditionally.
  There is no embedder callback equivalent to a restrictive
  `HostEnsureCanCompileStrings`. Parameter-only, body-only, and combined
  parsing also run synchronously outside opcode fuel. Generated source text is
  not retained, so the four constructor-specific
  `Function.prototype.toString` source-preservation tests still fail. Hosts
  that need a hard wall around attacker-controlled compilation must isolate
  the VM in a separately killable process.
- RegExp construction, `IsRegExp`, Realm fallback, the String-symbol methods,
  character-class escapes, active-ignoreCase `\w`/`\W` lowering, and `d`-flag
  match indices are implemented and audited, but full RegExp conformance is
  not complete. The current `built-ins/RegExp` diagnostic is **1036 pass / 7
  fail / 836 skip / 0 timeout**. The remaining failures are **5** valid
  empty-class matcher files, **1** quantifier integer-limit file, and **1**
  nullable-quantifier hybrid-boundary mismatch. The complete lookbehind subtree
  is **17/17**.

  Source validation now models one quantifier plus an optional lazy marker,
  legacy UTF-16 class ranges and Annex B escapes, Unicode scalar ranges,
  adjacent surrogate endpoints, standalone restricted brackets, and basic
  `v` subtraction operand structure before backend compilation. This closes
  the finite Test262 grammar cluster without relying on backend-specific parse
  errors.

  Lookahead and lookbehind execute in the vendored directional VM: lookbehind
  reverses concatenation and consumes atoms, captures, delegates, and
  backreferences backward while retaining source-order alternatives and
  greediness. Positive assertions are atomic and preserve their captures while
  restoring the outer cursor; negative assertions restore transactional state.
  Legacy quantified lookahead is enabled only outside `u`/`v`. ECMAScript mode
  charges branch creation, repeat dispatch, and capture clearing to one finite
  work budget and caps the branch stack at **100,000** entries. Ordinary
  mode-off `fancy-regex` callers retain upstream failed-backtrack accounting.

  Named groups support Unicode identifiers, escaped names, structurally
  disjoint duplicate names, participating-capture selection, and
  Unicode/legacy ignore-case backreferences, including hard variable-length
  lookbehind. Unicode `iu`/`iv` `\b`/`\B` on linear-backend patterns still
  inherits Rust's broader Unicode word boundary instead of the ECMAScript
  WordCharacters set. Valid scalars in `U+F0000..U+F07FF` collide with the
  internal UTF-16 sentinel representation, and nested `v` set
  subtraction/intersection and string properties remain incomplete. The
  backend also rejects some grammar-valid legacy forms, including empty
  classes, invalid-brace Annex B literals, and non-BMP code-unit ranges such as
  `[💩-\uFFFF]`; statement-list RegExp fallback still mishandles nearby lazy
  quantifiers. Moving every boundary onto the backtracking backend remains
  rejected; the general boundary fix needs a linear operation.
- Execution fuel is **cooperative, not preemptive**: `Vm::set_fuel(Some(n))`
  bounds execution to ~n opcodes (exhaustion throws a `RangeError` that is
  *not* catchable by user `try/catch`, so untrusted code cannot swallow it).
  But a single long native call (e.g. a pathological regex, or a native
  function that loops in Rust) is not subdivided, and there is no true
  async interrupt / `vm.Interrupt()` like goja. To hard-bound untrusted
  code, also run RuJa in a separately killable process.
- Map/Set are backed by `IndexMap`/`IndexSet` with SameValueZero keys
  (`MapKey` wrapper), so `get`/`has`/`set` are O(1). `WeakMap`/`WeakSet`
  still use `Vec` (entries are keyed by heap index for GC integration).
- `WeakRef` supports object, unregistered Symbol, and well-known Symbol
  targets. Object targets are cleared by GC once unreachable and are kept
  alive through the current job after construction or `deref()`.
  `FinalizationRegistry` stores targets and unregister tokens weakly, retains
  held values strongly, and schedules cleanup callbacks at VM job checkpoints
  after collection. As required by ECMAScript, callback timing is
  nondeterministic and embedders must not depend on cleanup running promptly.
- Revoked Proxy cells currently mark themselves revoked but retain strong
  target and handler references in their heap storage. Calls and internal
  methods reject the revoked Proxy correctly, but a target or handler that is
  otherwise unreachable can remain visible to `WeakRef` and delay
  `FinalizationRegistry` cleanup while the revoked Proxy itself is live.
  Clearing those slots without losing immutable call/construct internal-method
  metadata remains a separate storage audit.
- `SharedArrayBuffer` instances share backing bytes with TypedArray and DataView
  views, support species-aware `slice()`, and can opt into monotonic growth with
  `maxByteLength`. Atomics operations serialize through the backing-buffer
  mutex; worker agents use FIFO Condvar-backed `wait`/`notify` queues on the
  same Arc backing store.
  `waitAsync` settles through the VM's external Promise-job queue, which CLI and
  test262 hosts drain at job checkpoints, and `pause` is a no-op hint.
  Resizable ArrayBuffer provides `maxByteLength`, `resizable`, and `resize()`.
  TypedArray and DataView views distinguish fixed from length-tracking slots,
  recompute dynamic bounds after resize/grow, and recover when a fixed view
  returns in bounds. A public embedder-facing agent API is not yet implemented.
- Async generators serialize requests, and ordinary async functions preserve
  suspended frames across pending Await operations and GC. Both resume through
  the FIFO microtask queue. There is no event-loop preemption; `Vm::tick()`
  allows hosts to execute one microtask at a time cooperatively.
- `Vm::run_module`, CLI `--module`, and `.mjs` files use the Module source goal:
  code is implicitly strict, top-level `this` is `undefined`, and top-level
  declarations live in a module-local declarative environment. File-backed
  module graphs support relative side-effect and named imports, local/named/
  star/default exports, default and mixed default-plus-named imports, live
  imported bindings, dependency-first evaluation, and canonical-path caching.
  Cyclic graphs instantiate declarations before
  evaluation, preserve lexical TDZ, and cache abrupt evaluation per strongly
  connected component. Namespace imports and `export * as` expose canonical
  null-prototype Module Namespace Exotic Objects with live bindings, sorted
  export keys, and the specified non-extensible mutation semantics. Import and
  export specifiers support arbitrary well-formed string export names.
  Top-level `await` suspends module bodies through Promise continuations;
  sibling dependencies, async cycles, and rejection propagation are scheduled
  through SCC-aware graph evaluation. Script- and module-origin dynamic imports
  resolve relative file specifiers, share canonical module records, and reject
  loading, coercion, instantiation, and evaluation failures asynchronously.
  Dynamic import attributes enumerate observable `with` properties and support
  `type: "json"` and `type: "text"` for relative files; other keys and types
  reject. Bare-specifier resolution, `import.source`, and `import.defer` are
  not implemented yet. Dynamic-import jobs and generated errors retain the
  initiating Realm, but file-backed module environments and the canonical
  module cache are still VM-wide and rooted under the main global environment;
  independently Realm-owned module graphs are not implemented yet.
- test262 conformance is scoped, not full: RuJa targets a deliberately
  scoped subset of ES5.1 + selected ES2015+ features (see
  [test262.md](test262.md#supported-subset) for the exact list). The full
  suite is run in CI (excluding `intl402`/`staging`) with a baseline pass
  rate of 63.5% of all matrix files and 83.6% of executed files (**30,754
  pass / 6,049 fail / 11,658 skip / 6 timeout / 0 error**); within the
  supported subset, tests currently run at 100%.
  Full ES conformance is not claimed. See
  [test262.md](test262.md) for current numbers and the failure breakdown.
- Decorator support covers audited class and public/private
  method/getter/setter/field/auto-accessor semantics, including private async,
  generator, and async-generator methods. The complete pending Test262 PR
  #5048 diagnostic passes **657/657**, including both computed-member
  early-error cases and all 88 private auto-accessor files. These files are
  still pending upstream, so they are verified against the PR's pinned head
  rather than admitted into the current-main runner early; the broad current-
  main gate remains closed until the files land upstream.
- The global `Iterator` and common synchronous iterator prototype hierarchy
  are implemented, including Realm-specific branded String iterators,
  `Iterator.from`, eager `Iterator.prototype.toArray`, and lazy `map`/`filter`/
  `flatMap`/`take`/`drop` helpers and eager `reduce`/`forEach`/`some`/`every`.
  Eager `find` completes the current synchronous prototype-helper surface, and
  static `Iterator.concat`, `Iterator.zip`, and `Iterator.zipKeyed` provide
  iterator sequencing and joint iteration. Async iterator helpers are a
  separate unsupported surface.
- Foreign Realm `Object` constructors, static methods, prototype methods, and
  `__proto__` accessors now own Realm-specific function identities, intrinsic
  prototypes, generated results, and native errors. The full 248-file
  `Object.prototype` subtree now executes without failures or skips. Created
  Test262 Realms also install rooted Realm-local Promise, synchronous
  GeneratorFunction/Generator, and asynchronous
  AsyncGeneratorFunction/AsyncGenerator/AsyncIterator intrinsic graphs, plus
  independent Math and Reflect namespace objects whose methods inherit that
  Realm's `%Function.prototype%`. Reflect also owns the standard observable
  `Symbol.toStringTag`. Every Realm's original `%Object.prototype%` keeps its
  null prototype through the immutable-prototype internal method while
  remaining extensible; the calling method still determines generated Error
  identity.
  Date constructors, prototypes, methods, and static functions are also
  Realm-local. A non-object Date new-target prototype falls back to the
  immutable Date prototype from that new target's Realm, while constructed
  instances keep their Date value in a non-observable internal slot.
  Dynamic Function-family calls and construction likewise select generated
  closures and fresh ordinary/generator prototype parents from the active
  constructor's Realm. A non-object new-target prototype falls back through
  the actual new target's immutable Realm registry without consulting replaced
  global bindings.
  `AsyncIterator.prototype[Symbol.asyncDispose]` is implemented with
  Realm-correct Promise and abrupt-completion behavior. `Array.fromAsync`
  consumes async, sync, and array-like sources through intrinsic Promise jobs,
  including Realm-correct `AsyncFromSyncIterator` and iterator-close behavior.
  The complete current `%AsyncFromSyncIteratorPrototype%` corpus is admitted;
  async-generator `yield*` suspends on adapter Promise reactions without
  synchronously draining host jobs.
  The 12 audited IteratorClose-on-abrupt paths across `Promise.all`,
  `Promise.allSettled`, `Promise.any`, and `Promise.race` are also admitted;
  another 95 exact setup-rejection files now preserve Error identity, method
  Realm, resolve-before-iterator ordering, and host-abort propagation.
  `Promise.prototype.finally` now implements the species, abstract
  PromiseResolve, observable reaction, original-completion, Realm, and GC
  semantics required by its complete 29-file corpus. The four standard
  combinator directories consequently pass all 390 files with their gates
  lifted. Promise construction validates executor callability before
  observing `NewTarget.prototype` and roots its instance across resolving-state
  allocation. `Promise.allKeyed` and `Promise.allSettledKeyed` preserve raw
  own-key order while observing each Proxy descriptor before reading or
  resolving that entry; the complete 703-file Promise corpus passes when all
  diagnostic gates are lifted.
  Async iterator helpers remain a separate unsupported surface.
- `Vm` is `Send` (but not `Sync`): the engine uses `Arc`/`Mutex`/atomics
  for shared ownership and interior mutability, so a `Vm` can be moved
  between threads. Concurrent *shared* access still requires external
  synchronization (e.g. wrapping it in a `Mutex<Vm>`), since the internal
  mutexes protect individual fields, not the whole-VM invariant. The GC
  trace loop is worklist-based to avoid re-entrant locking of the cells
  mutex.
- `CallFrame` per-frame state (`gen_yield`, `finally_completion_val`,
  `pending_with_this`, ...) is stored in `Mutex<T>` even though a frame is
  only ever touched by the single thread running the `Vm`. This keeps the
  whole `Vm` (and thus `CallFrame`) `Send` without `unsafe`; it is a minor
  runtime overhead and could become `RefCell` if `Send` is later asserted
  via a manual `unsafe impl`.
- `Heap::with_obj` takes the object out of its cell for the duration of the
  callback (so the cells mutex is not held re-entrantly). If the callback
  touches the *same* object index it will see `None` ("temporarily absent")
  rather than the live value; callers must not re-enter on the same idx.
- `yield*` throw/return propagation into a delegated generator is not yet
  forwarded (direct `g.throw`/`g.return` work)
- Dynamic `import()` supports relative imports from file-backed Script and
  Module chunks, including in-flight top-level-await graphs, cached rejection
  objects, namespace identity, and JSON/text import attributes. Bare host
  specifiers and the source/defer proposals remain gated.
- Some strict-mode edge cases are not fully enforced: `this` defaults to
  `undefined` in all modes by design (strict mode does not rebind it to the
  global object), and a top-level strict `eval` `var` still routes through the
  global slot path (the in-function strict-eval case is handled)
- GC runs at safe points only (after a run settles, and throttled at frame
  boundaries). Incremental marking is supported via `collect_incremental(roots, budget)`,
  but there is no generational collector yet
- Generic Array `join` and `toLocaleString`, plus TypedArray
  `toLocaleString`, grow their Rust `String` through fallible reservation, but
  publishing the completed value as `Arc<str>` still uses the runtime-wide
  infallible allocation path. Hard host OOM isolation therefore remains an
  embedder responsibility even though capacity overflow is reported as
  `RangeError`.
- Native operation rooting is complete for `Array.of`, `sort`, `toSorted`, and
  `toSpliced`; `map` and `flatMap` no longer use snapshots. The sorting methods
  implement generic
  `ToObject`/`LengthOfArrayLike`, inherited and accessor-backed indices, live
  Proxy-aware `HasProperty`/`Get`, strict `Set`/`Delete`, and the distinct
  skip-holes versus read-through-holes modes. Sorting rejects a captured
  length above 1,048,576 with `RangeError` before any indexed scan. For
  `toSorted`, `ArrayCreate` still precedes this sandbox check. This bound is
  intentionally stricter than ECMAScript for very large sparse receivers;
  native temporary-root, collected-list, and merge-buffer storage is bounded
  by the same limit but still uses infallible Rust vector allocation.
- Push, Pop, Shift, Unshift, Splice, Slice, Concat, Flat, FlatMap, ForEach,
  Join, ToLocaleString, Map, Reduce, ReduceRight, Reverse, ToReversed,
  ToSpliced, and With use live generic indexed operations with operation-wide
  roots. ToSpliced retains only the captured length and coerced splice bounds
  before copying; it does not retain source elements in a Rust snapshot.
  Remaining method-specific gaps are tracked above.
- Private methods are stored per-instance as private fields (each instance
  gets its own closure copy); behavior is spec-correct, but this is more
  memory-heavy than a shared per-class method table would be
- Static class field declarations (`static x = 1`) are not yet supported;
  static initialization blocks (`static { }`) are
- BigInt: arbitrary precision via `num-bigint`; fixed-width
  `asIntN`/`asUintN`, prototype conversion methods, and DataView 64-bit
  interop are implemented
- Wrapper objects (`new String(x)`, `new Number(x)`, `new Boolean(x)`,
  `Object(x)`) now store the wrapped primitive, so `.valueOf()` and
  `ToPrimitive` resolve to it (`new Number(5) + 1 === 6`). Boxed-string
  `.toString()` still falls back to the default object form.

---

**Next:** [Architecture](architecture.md) · [Features](features.md) · [Back to README](../README.md)
