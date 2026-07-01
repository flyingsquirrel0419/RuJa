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

- `args[0]` / `args[1]` direct indexing in a handful of builtins. Most are guarded
  by `args.is_empty()` checks or by the calling convention (`this` is always
  supplied), but each should be reviewed individually.
- `Mutex::lock().unwrap()` project-wide. Current policy: acceptable as an
  invariant, with the understanding that no user input should be able to cause
  a panic that could poison a lock.
- `frames.last().unwrap()` and similar VM invariants in `src/vm.rs`. These rely on
  correct bytecode/opcode sequences. They should become defensive `ok_or_else`
  over time, but are not the top panic risk from JavaScript.

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
3. Run `cargo-fuzz` on the public API (`Vm::run`) to discover remaining panic
   paths empirically.
