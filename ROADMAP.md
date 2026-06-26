# RuJa Roadmap

Legend: `[ ]` pending, `[~]` in progress, `[x]` done.

## v2.0 - Bytecode VM + GC (current)

1. [x] GC heap + value model (gc.rs, value.rs)
2. [x] Bytecode Op set + compiler (bytecode.rs, compiler.rs)
3. [x] Stack VM dispatch (vm.rs)
4. [~] Lexer/parser ES2015 extensions (class, destructuring, generators)
5. [~] Built-in objects (Math/JSON/String/Array/console done; Map/Set/Promise stubbed)
6. [~] Closure variable capture + this binding
7. [ ] ES2015: class/extends, Map/Set, Promise, Symbol, iterator protocol
8. [ ] async/await + generator state machine
9. [ ] Built-in spec conformance + TDZ
10. [ ] test262 harness + regression tests (200+)
11. [x] Release prep (README/CHANGELOG/CI)
12. [ ] Release verification (tests + CLI + metadata)

## v1.0 - Tree-walking interpreter (archived)

Completed and tagged as v0.1.0-alpha. See v1-archive branch.
