# Known limitations

- No `eval`/`with` process-level security sandbox (local-trust execution model)
- Async generator scheduling uses a synchronous microtask-drain model (no
  real event-loop preemption)
- test262 conformance is a future target, not achieved
- `Vm` is `!Send`/`!Sync`: the engine uses `Rc`/`RefCell`/`Cell` for
  interior mutability and shared ownership (zero-cost single-threaded
  ergonomics). An embedding that needs to share a VM across threads must
  keep it on one thread (e.g. behind a channel or a single-worker model).
  Migrating to `Arc`/`Mutex`/atomics is a planned but invasive change
  (it touches the GC, every heap object, and the trace loop's nested
  borrows), so for now single-threaded embedding is the supported model.
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
- Wrapper objects (`new String(5)`) do not store the inner primitive; the
  prototype is correct and `typeof` is `"object"`, but `.valueOf()` is not
  implemented on wrapper objects

---

**Next:** [Architecture](architecture.md) · [Features](features.md) · [Back to README](../README.md)
