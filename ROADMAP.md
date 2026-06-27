# RuJa Roadmap

Legend: `[ ]` pending, `[~]` in progress, `[x]` done.

## v2.0 - Bytecode VM + GC (current)

1. [x] GC heap + value model (gc.rs, value.rs)
2. [x] Bytecode Op set + compiler (bytecode.rs, compiler.rs)
3. [x] Stack VM dispatch (vm.rs)
4. [x] Lexer/parser ES2015 extensions (class, destructuring, template literals, default/rest params)
5. [x] Built-in objects (Math/JSON/String/Array/console/Map/Set/Symbol; Promise stubbed)
6. [x] Closure variable capture + this binding
7. [x] ES2015: class/extends + super, Map/Set, Symbol, iterator protocol (for-of/for-in)
8. [ ] async/await + generator state machine
9. [ ] Built-in spec conformance + TDZ
10. [~] regression tests (154 passing, split across 6 files)
11. [x] Release prep (README/CHANGELOG/CI)
12. [~] Release verification (tests + CLI + metadata)

## v1.0 - Tree-walking interpreter (archived)

Completed and tagged as v0.1.0-alpha. See v1-archive branch.

## Remaining known limitations (post v2.0 ES2015 work)

- Object property insertion order not preserved (HashMap); affects `for...in`/`Object.keys` order
- Number-to-string never uses exponential notation (e.g. 1e21 prints in full)
- Regex literals not supported
- `var` hoisting only covers direct declarations (not nested inside if/for blocks)
