# Known limitations

- No `eval`/`with` process-level security sandbox (local-trust execution model)
- Async generator scheduling uses a synchronous microtask-drain model (no
  real event-loop preemption)
- test262 conformance is a future target, not achieved
- `Vm` is `Send` (but not `Sync`): the engine uses `Arc`/`Mutex`/atomics
  for shared ownership and interior mutability, so a `Vm` can be moved
  between threads. Concurrent *shared* access still requires external
  synchronization (e.g. wrapping it in a `Mutex<Vm>`), since the internal
  mutexes protect individual fields, not the whole-VM invariant. The GC
  trace loop is worklist-based to avoid re-entrant locking of the cells
  mutex.
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
