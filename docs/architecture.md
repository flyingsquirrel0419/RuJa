# Architecture

RuJa is a self-contained JavaScript engine with no external runtime
dependencies. Source flows through four stages before execution:

```
source ─► Lexer ─► Parser ─► Compiler ─► Bytecode ─► VM
                              │              │
                              └─ AST         └─ Op stream
```

## Pipeline

- **Lexer** (`src/lexer.rs`) — tokenization with automatic semicolon insertion
  (ASI) and template literal support.
- **Parser** (`src/parser.rs`) — Pratt-style recursive descent producing an AST.
  Expression nesting is depth-capped to prevent stack overflow on untrusted input.
- **Compiler** (`src/compiler.rs`) — single-pass AST → bytecode compilation
  with lexical scope resolution, hoisting, and TDZ tracking.
- **Bytecode** (`src/bytecode.rs`) — a stack-machine instruction set (`Op`).
- **VM** (`src/vm/mod.rs`, `src/vm/ops.rs`) — the dispatch loop: call frames,
  operand stack, property access, type coercion, and non-local control flow.
  `ops.rs` holds the opcode dispatch and immediate helpers; `mod.rs` holds
  the `Vm` struct, public API, and runtime helpers. Identifier, property,
  private-name, and `super` expression evaluation is represented by rooted
  `ReferenceRecord` values and resolved through shared `GetValue`, `PutValue`,
  call, and delete operations; compiler-internal switch completion reads use
  the same path.
- **GC** (`src/gc.rs`) — mark-and-sweep collector that traces from VM roots.
- **Values** (`src/value.rs`) — the `HeapObj` enum
  (Object/Array/Function/Environment/Map/Set/Promise/Generator) referenced by
  `GcIdx` handles.
- **Builtins** (`src/builtins/mod.rs` + submodules) — the standard library:
  Object, Array, String, Number, Boolean, Function, Math, JSON, console, RegExp,
  Map, Set, Symbol, Promise, Proxy, TypedArray, and the Error hierarchy.

## Garbage collection

A mark-and-sweep collector with optional incremental marking reclaims
reference cycles. Collection runs at safe points only (after a run settles,
and throttled at frame boundaries). Incremental marking via
`collect_incremental(roots, budget)` allows limiting the number of cells
marked per GC step, avoiding long pauses. There is no generational collector.
accumulate memory before a collection. A `gc_pins` stack lets call paths pin
heap values held in Rust locals across allocations that could trigger a GC.

Native constructors that allocate specialized heap objects use an internal
allocation dispatch path. This lets each constructor perform specification
validation before the observable `NewTarget.prototype` lookup instead of
preallocating an ordinary object in generic dispatch. Values returned by that
lookup, and fresh specialized objects not yet linked from another heap object,
must remain on `gc_pins` across every collecting `Vm::alloc` call.

Observable materializers follow the same ownership rule. When an abstract
operation reads heap values into a Rust collection and a later getter, proxy
trap, coercion, call, or construction can re-enter JavaScript, every value is
pinned immediately after it is read. The caller owns those pins through the
last re-entrant operation and releases them on both normal and abrupt
completion. A Rust `Vec<Value>` is storage, not a GC root.

`src/builtins/call_arguments.rs` centralizes that contract for
`CreateListFromArrayLike`. `Reflect.apply`, `Reflect.construct`, and
`Function.prototype.apply` therefore share the same observable `length`,
`ToLength`, indexed `Get`, resource-cap, and pin-cleanup behavior instead of
maintaining array-specific shortcuts. `Function.prototype.apply` handles its
specified omitted, `null`, and `undefined` no-argument cases before entering
the shared object-only operation. The materialized list and its pin count move
together into the final call so a later getter or target re-entry cannot make
an earlier argument collectible.

---

**Next:** [Features](features.md) · [Known limitations](limitations.md) · [Back to README](../README.md)
