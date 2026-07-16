# Known limitations

## Sandbox guarantees

RuJa is designed for running untrusted JavaScript safely inside a host process.
The following resource limits are enforced:

- **Execution fuel**: `Vm::set_fuel(Some(n))` bounds execution to ~n opcodes.
  Exhaustion throws a `RangeError("fuel exhausted")` that is *not catchable*
  by user `try/catch` (a host-level abort). `None` = unbounded (default).
- **Heap object limit**: `Vm::set_max_heap_objects(Some(n))` caps the number
  of live GC-managed heap objects. When exceeded, allocation throws a
  catchable `RangeError("heap limit exceeded")`. A GC cycle is attempted
  before the error is raised. `None` = unlimited (default).
- **Call-stack depth**: JavaScript recursion is capped at 1000 frames.
  Exceeding this throws a catchable `RangeError("Maximum call stack size
  exceeded")`, not a native stack overflow (SIGSEGV/abort).
- **ReDoS-safe regex**: The `regex` crate uses RE2-style linear-time matching
  with no backtracking, so catastrophic regex patterns cannot cause
  exponential-time hangs.
- **String/array caps**: `"x".repeat(n)` is capped at 256 MiB output.
  `Array.from(iterable)` is capped at 65k elements. Dense arrays are capped
  at 1M elements (`MAX_DENSE_ARRAY_LEN`); beyond that, indices are stored
  sparsely as named properties.

These limits make it safe to pass untrusted JS to `Vm::run` without risking
process crashes, infinite loops, or OOM kills. For truly hard real-time
guarantees, run RuJa in a separately killable process as well.

- No `eval`/`with` process-level security sandbox (local-trust execution model)
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
  not implemented yet.
- test262 conformance is scoped, not full: RuJa targets a deliberately
  scoped subset of ES5.1 + selected ES2015+ features (see
  [test262.md](test262.md#supported-subset) for the exact list). The full
  suite is run in CI (excluding `intl402`/`staging`) with a baseline pass
  rate of 62.2% of all matrix files and 82.6% of executed files; within the
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
  an independent Math object whose methods inherit that Realm's
  `%Function.prototype%`.
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
  Realm, resolve-before-iterator ordering, and host-abort propagation. The
  remaining forced failure in those four combinator directories depends on
  incomplete `Promise.prototype.finally`; broader Promise edge cases remain
  separately gated.
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
