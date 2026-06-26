# RuJa Roadmap

RuJa is a JavaScript engine written in Rust. The goal of v1.0 is a working
tree-walking interpreter that runs a pragmatic ES5.1 subset with built-in
objects, a REPL, a CLI, and a test suite. A future v2.0 will introduce a
bytecode VM and a tracing garbage collector.

Legend: `[ ]` pending, `[~]` in progress, `[x]` done.

## v1.0 - Tree-walking interpreter

1. [x] Project scaffold + design (Cargo.toml, module tree, value/AST/error model)
2. [x] Lexer (tokenization, ASI, template literals)
3. [x] Parser (Pratt expressions, statements, functions, program)
4. [x] Value model + environment scope chain
5. [x] Interpreter core (arithmetic/comparison/logic/assignment/control flow)
6. [x] Functions + closures
7. [x] Objects / prototype / `this` / `new`
8. [x] Built-in objects (Array/String/Number/Boolean/Object/Function/Math/JSON/console/Error)
9. [x] Error handling (`throw` / `try` / `catch` / `finally`)
10. [x] REPL + CLI
11. [x] Test suite (fixtures + regression)
12. [x] Release prep (README/docs/CHANGELOG/CI)
13. [x] Release verification (tests pass + CLI runs + metadata sync)

## v2.0 - Performance tier (post-v1.0)

- Bytecode compiler + stack VM
- Tracing garbage collector
- ES2015+ features (`class`, `import`/`export`, `async`/`await`, generators)

## Tracking

This file is updated as each step is completed. Last updated: 2026-06-26.
