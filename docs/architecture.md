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
- **VM** (`src/vm.rs`) — the dispatch loop: call frames, operand stack,
  property access, type coercion, and non-local control flow.
- **GC** (`src/gc.rs`) — mark-and-sweep collector that traces from VM roots.
- **Values** (`src/value.rs`) — the `HeapObj` enum
  (Object/Array/Function/Environment/Map/Set/Promise/Generator) referenced by
  `GcIdx` handles.
- **Builtins** (`src/builtins.rs`) — the standard library: Object, Array,
  String, Number, Boolean, Function, Math, JSON, console, RegExp, Map, Set,
  Symbol, Promise, and the Error hierarchy.

## Garbage collection

A non-incremental, non-generational mark-and-sweep collector reclaims
reference cycles. Collection runs at safe points only (after a run settles,
and throttled at frame boundaries), so very long-running tight loops can
accumulate memory before a collection. A `gc_pins` stack lets call paths pin
heap values held in Rust locals across allocations that could trigger a GC.

---

**Next:** [Features](features.md) · [Known limitations](limitations.md) · [Back to README](../README.md)
