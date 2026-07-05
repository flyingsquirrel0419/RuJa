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
- Async generator scheduling uses a synchronous microtask-drain model (no
  real event-loop preemption), though `Vm::tick()` now allows hosts to
  execute a single microtask at a time for cooperative scheduling
- test262 conformance is scoped, not full: RuJa targets a deliberately
  scoped subset of ES5.1 + selected ES2015+ features (see
  [test262.md](test262.md#supported-subset) for the exact list). The full
  suite is run in CI (excluding `intl402`/`staging`) with a baseline pass
  rate of ~33%; within the supported subset, ~56% of `language/` tests
  pass. Full ES conformance is not claimed. See
  [test262.md](test262.md) for current numbers and the failure breakdown.
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
  `asIntN`/`asUintN` and DataView 64-bit interop are implemented, but
  `BigInt.prototype.toString(radix)` and some prototype descriptor edges are
  still incomplete
- Wrapper objects (`new String(x)`, `new Number(x)`, `new Boolean(x)`,
  `Object(x)`) now store the wrapped primitive, so `.valueOf()` and
  `ToPrimitive` resolve to it (`new Number(5) + 1 === 6`). Boxed-string
  `.toString()` still falls back to the default object form.

---

**Next:** [Architecture](architecture.md) · [Features](features.md) · [Back to README](../README.md)
