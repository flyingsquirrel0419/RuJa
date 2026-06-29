# Known limitations

- No `eval`/`with` process-level security sandbox (local-trust execution model)
- Async generator scheduling uses a synchronous microtask-drain model (no
  real event-loop preemption)
- test262 conformance is a future target, not achieved
- `yield*` throw/return propagation into a delegated generator is not yet
  forwarded (direct `g.throw`/`g.return` work)
- Some strict-mode edge cases are not fully enforced: `this` defaults to
  `undefined` in all modes by design (strict mode does not rebind it to the
  global object), and a top-level strict `eval` `var` still routes through the
  global slot path (the in-function strict-eval case is handled)
- GC runs at safe points only (after a run settles, and throttled at frame
  boundaries), so very long-running tight loops can accumulate memory before a
  collection; there is no incremental/generational collector
 in `try`/`catch` divert through a single `finally`; nested `try/finally`
 only runs the innermost finally for break/continue (outer finally is skipped)
 (the `this` binding in the class-definition context is not yet wired)
- Wrapper objects (`new String(5)`) do not store the inner primitive; the
  prototype is correct and `typeof` is `"object"`, but `.valueOf()` is not
  implemented on wrapper objects
- BigInt is backed by `i128` (range roughly ±170 quintillion); integer
  literals beyond that range parse but saturate. `BigInt` arithmetic with
  `Number` throws `TypeError` per spec. `toString(radix)` / `asIntN` /
  `DataView` interop are not yet implemented
- Private methods are stored per-instance as private fields (each instance
  gets its own closure copy); behavior is spec-correct, but this is more
  memory-heavy than a shared per-class method table would be
- Static class field declarations (`static x = 1`) are not yet supported;
  static initialization blocks (`static { }`) are

---

**Next:** [Architecture](architecture.md) · [Features](features.md) · [Back to README](../README.md)
