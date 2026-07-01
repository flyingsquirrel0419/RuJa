# Known limitations

- No `eval`/`with` process-level security sandbox (local-trust execution model)
- Execution fuel is **cooperative, not preemptive**: `Vm::set_fuel(Some(n))`
  bounds execution to ~n opcodes (exhaustion throws a `RangeError` that is
  *not* catchable by user `try/catch`, so untrusted code cannot swallow it).
  But a single long native call (e.g. a pathological regex, or a native
  function that loops in Rust) is not subdivided, and there is no true
  async interrupt / `vm.Interrupt()` like goja. To hard-bound untrusted
  code, also run RuJa in a separately killable process.
- Map/Set are backed by a `Vec`, so `get`/`has`/`set` are O(n) linear scans
  (keyed by SameValueZero). Correct but slow for large collections; no
  hash table yet. `WeakMap`/`WeakSet` use the same structure (entries are
  held strongly here, see below).
- Async generator scheduling uses a synchronous microtask-drain model (no
  real event-loop preemption)
- test262 conformance is partial: the full suite is run in CI (excluding
  `intl402`/`staging`), with a baseline pass rate of ~33%. A curated
  `language/` subset (~61%) is run on every push for fast regression
  detection. Full ES conformance is not claimed. See
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
  boundaries), so very long-running tight loops can accumulate memory before a
  collection; there is no incremental/generational collector
- Private methods are stored per-instance as private fields (each instance
  gets its own closure copy); behavior is spec-correct, but this is more
  memory-heavy than a shared per-class method table would be
- Static class field declarations (`static x = 1`) are not yet supported;
  static initialization blocks (`static { }`) are
- BigInt: arbitrary precision via `num-bigint`, but `toString(radix)`,
  `asIntN`/`asUintN`, and `DataView` interop are not yet implemented
- Wrapper objects (`new String(x)`, `new Number(x)`, `new Boolean(x)`,
  `Object(x)`) now store the wrapped primitive, so `.valueOf()` and
  `ToPrimitive` resolve to it (`new Number(5) + 1 === 6`). Boxed-string
  `.toString()` still falls back to the default object form.

---

**Next:** [Architecture](architecture.md) · [Features](features.md) · [Back to README](../README.md)
