# Panic Path Audit

This document tracks `unwrap()` / `expect()` usage in `src/vm.rs` and `src/builtins.rs`
that may be reachable from untrusted JavaScript input, and the policy for the rest.

## Current state

- Total `.unwrap()` in `src/`: 469
- In `src/vm.rs` + `src/builtins.rs`: 376
- `.expect()` in those files: 1 (`src/vm.rs:644`)
- Real `unsafe` blocks: 0

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
| `src/vm.rs` | 644 | `self.frames.pop().expect("generator frame present")` | Generator resume path; invariant-only but reads as unconditional unwrap | `pop().ok_or_else(\|\| Error::internal(...))?` — propagates internal error |
| `src/builtins.rs` | 5090 | `String::from_utf8(out).unwrap()` | `biguint_to_radix` digits are ASCII-only today, but the unwrap is unnecessary | `String::from_utf8(out).unwrap_or_default()` |

## Remaining known categories

- ~~`args[0]` / `args[1]` direct indexing in a handful of builtins.~~ Converted to
  `args.first().unwrap_or(&Value::Undefined)` / `args.get(1).cloned().unwrap_or(Value::Undefined)`.
- ~~`Mutex::lock().unwrap()` project-wide.~~ Switched from `std::sync::Mutex` to
  `parking_lot::Mutex`; `parking_lot::lock()` is panic-free, so the ~200 lock
  `.unwrap()` calls are gone. The remaining `.unwrap()` in the engine are
  `Option`/`Result`/`Vec` operations on data that should be guarded by script-level
  checks (e.g. `String::from_utf8`, `parse`, `stack.pop()` with defaults).
- `frames.last().unwrap()` and similar VM invariants in `src/vm.rs`. The most
  dangerous invariant failure (an empty frame stack at the top of the interpret
  loop) is now caught and returns `Error::internal`. The remaining `.unwrap()`
  calls rely on the loop invariant that a frame is always present while
  dispatching opcodes; they will be converted to defensive `ok_or_else` during
  the `vm.rs` module split.
- **File size**: `src/builtins.rs` was split into `src/builtins/{mod,math,json,global,array,string,collections,regexp,function}.rs`.
  The `vm.rs` split is still pending.

## Verification

After the conversions:

```bash
cargo check
cargo test --test builtins --test bigint
cargo test --test generators
```

All green.

## Future work

1. Replace remaining `args[idx]` direct indexing with `.get(idx).unwrap_or(...)`
   in builtins that may be called with fewer arguments.
2. Decide on a project-wide mutex policy (`parking_lot`, poison-recovery, or
   documented invariant).
3. ✅ Run `cargo-fuzz` on the public API (`Vm::run`) to discover remaining panic
   paths empirically. Fuzz target added at `fuzz/fuzz_targets/fuzz_target_1.rs`;
   initial 30-second run completed 50k+ iterations without panics.
