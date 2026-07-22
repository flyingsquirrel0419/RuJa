# Panic Path Audit

This document tracks `unwrap()` / `expect()` usage in `src/vm/` and `src/builtins/`
that may be reachable from untrusted JavaScript input, and the policy for the rest.

## Current state

- Total `.unwrap()` in `src/` (excluding tests/fuzz): 22
- In `src/vm/ops.rs` + `src/vm/mod.rs`: 0
- `.expect()` in those files: 0
- Real `unsafe` blocks: 0
- `Mutex::lock().unwrap()` count: 0 (parking_lot, panic-free)

## Reachability policy

### Not user-reachable (invariant-based)

Most `Mutex::lock().unwrap()` calls guard engine-internal state (property maps,
array storage, generator saved state). Under normal execution they never panic.
They become dangerous only if another thread panics while holding the same lock
(poisoned lock), which today can happen only if the engine itself panics first.
Therefore the highest-value hardening is to remove **all** user-triggered panic
paths from the engine; once that is done, lock poisoning becomes impossible in
practice.

### User-reachable and converted

| File | Line | Original | Why reachable | Converted to |
|------|------|----------|---------------|--------------|
| `src/vm/mod.rs` | ~644 (in `resume_generator`) | `self.frames.pop().expect("generator frame present")` | Generator resume path; invariant-only but reads as unconditional unwrap | `pop().ok_or_else(\|\| Error::internal(...))?` — propagates internal error |
| `src/builtins/json.rs` | `biguint_to_radix` | `String::from_utf8(out).unwrap()` | ASCII-only digit buffer, but the unwrap is unnecessary | `String::from_utf8(out).unwrap_or_default()` |

## Remaining known categories

- ~~`args[0]` / `args[1]` direct indexing in a handful of builtins.~~ Converted to
  `args.first().unwrap_or(&Value::Undefined)` / `args.get(1).cloned().unwrap_or(Value::Undefined)`.
- ~~`Mutex::lock().unwrap()` project-wide.~~ Switched from `std::sync::Mutex` to
  `parking_lot::Mutex`; `parking_lot::lock()` is panic-free, so the ~200 lock
  `.unwrap()` calls are gone. The remaining `.unwrap()` in the engine are
  `Option`/`Result`/`Vec` operations on data that should be guarded by script-level
  checks (e.g. `String::from_utf8`, `parse`, `stack.pop()` with defaults).
- ~~`frames.last().unwrap()` and similar VM invariants~~ All 50 instances
  converted to `current_frame()?` / `current_frame_mut()?` safe propagation.
  `vm/ops.rs` and `vm/mod.rs` now have zero `unwrap()` calls.
- **File size**: `src/builtins.rs` was split into 12 submodules
  (`{mod,math,json,global,array,string,collections,regexp,function,proxy,typed_array}.rs`).
  `src/vm.rs` was split into `src/vm/{mod,ops}.rs`.
  `vm/mod.rs` (3,175 lines) can be further split into runtime helpers and
  generator/promise logic; `ops.rs` can be split by opcode category.

## Verification

After the conversions:

```bash
cargo check
cargo test --test builtins --test bigint
cargo test --test generators
```

All green.

## Heap limit enforcement

`Heap::allocate` returns `Result<usize, HeapLimitExceeded>`. A
`From<HeapLimitExceeded> for Arc<Error>` impl converts the error into a
catchable `RangeError("heap limit exceeded")`.

All heap allocations go through `Vm::alloc`, which first attempts a GC pass
(with the correct root set) before calling `Heap::allocate`. If the limit is
still exceeded after GC, the `RangeError` propagates to the caller.

Previously, 59+ call sites used `heap.allocate().unwrap_or(0)` or raw
`heap.allocate()` without checking the result, which meant builtin code paths
(Array.prototype.map, JSON.parse, RegExp.exec, Proxy, Map/Set constructors,
Promise allocation, etc.) silently bypassed the heap limit. These have all been
converted to propagate the error via `?`.

## Future work

1. ✅ Replace remaining `args[idx]` direct indexing — done.
2. ✅ Project-wide mutex policy — `parking_lot::Mutex` adopted.
3. ✅ `cargo-fuzz` target — added and verified (96k+ iterations, no panics).
4. ✅ `frames.last().unwrap()` — all converted to safe propagation.
5. Run longer fuzzing sessions (hours) to discover edge-case panics.
6. Add IC for `GetElem` and `SetElem` opcodes.
7. ✅ Proxy internal-method traps — `has`, `deleteProperty`, `ownKeys`, and the
   remaining baseline traps are implemented and covered by conformance tests.
8. ✅ TypedArray expansion — all standard numeric and BigInt element kinds plus
   `set` and `subarray` are implemented; remaining semantic gaps are tracked in
   [limitations](limitations.md).
9. ✅ Heap limit enforcement — all allocation sites now return `Result` and
   propagate `HeapLimitExceeded` via `?`. Verified with tests for `JSON.parse`,
   `Array.map`, and `RegExp.exec` under heap limit.
