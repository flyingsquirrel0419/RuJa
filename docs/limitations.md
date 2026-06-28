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
- `try/finally` partially suspends non-local transfers: `return` and `throw`
  in `try`/`catch` are correctly suspended across `finally` (and a `return`
  in `finally` overrides them), but `break`/`continue` inside `try`/`catch`
  do not yet route through `finally`
- `for(;;)` with an empty condition (infinite C-for) is not parsed; use
  `while(true)` instead
- Wrapper objects (`new String(5)`) do not store the inner primitive; the
  prototype is correct and `typeof` is `"object"`, but `.valueOf()` is not
  implemented on wrapper objects
