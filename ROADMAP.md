# RuJa Roadmap

RuJa is a JavaScript engine written in Rust.

## v2.0 (alpha) - Bytecode VM + GC

- [x] Stack-based bytecode VM
- [x] Mark-and-sweep garbage collector
- [x] HeapObj enum value model with GcIdx handles
- [x] AST-to-bytecode compiler
- [x] Function calls, recursion, this binding
- [x] try/catch via VM catch stack
- [x] Built-in: Math, JSON, console, Object, Array, String, Number, Boolean, Error
- [x] Map, Set, Symbol
- [x] CLI + REPL
- [ ] Closure capture of outer locals (in progress)
- [ ] class syntax
- [ ] test262 conformance harness

## v2.1 (planned)

- Promise + microtask queue
- async/await + generators
- class/extends
- Destructuring, default/rest params, template interpolation
- test262 ES2015 subset 85% pass gate

## v2.2+ (future)

- Hidden classes, inline caches
- Incremental/concurrent GC
- ES modules (import/export)
- Full test262 conformance

Last updated: 2026-06-27.
