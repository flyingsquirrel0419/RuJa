# RuJa

A JavaScript engine written in Rust.

RuJa v2.0 is a complete rewrite featuring a stack-based bytecode VM and a
self-contained mark-and-sweep garbage collector. It runs a pragmatic ES5.1
subset with ES2015 additions (arrow functions, `let`/`const`, `Map`/`Set`,
`Symbol`, spread, nullish coalescing), and ships with a CLI and REPL.

## v2.0 Architecture

- **Bytecode VM**: AST is compiled to a flat `Op` instruction stream and
  executed by a stack machine (`vm.rs`). This replaces the v1.0 tree-walker.
- **Garbage collector**: `gc.rs` implements mark-and-sweep tracing from VM
  roots (operand stack, call frames, globals, prototypes). `HeapObj` lives in
  a `Heap` cell array and is referenced via `GcIdx` handles. Reference cycles
  are collected automatically.
- **Compiler**: `compiler.rs` is a single-pass AST-to-bytecode compiler with
  compile-time lexical scope resolution.
- **Value model**: `Value` is a tagged union; heap objects are an enum
  (`Object`, `Array`, `Function`, `Environment`, `Map`, `Set`, `Promise`,
  `Generator`) traced by the GC.

## Features

- Arithmetic, comparison, logical, bitwise, and assignment operators
- `var`, `let`, `const` with block and function scoping
- Control flow: `if`/`else`, `while`, `do...while`, `for`, `for...of`,
  `switch`, `break`/`continue`
- Functions, closures, and arrow functions
- Objects, arrays, prototype chain, `new`, `instanceof`, `this`
- `throw` / `try` / `catch` / `finally`
- Built-in objects: `Object`, `Array`, `String`, `Number`, `Boolean`,
  `Function`, `Math`, `JSON`, `console`, `Error`, `Map`, `Set`, `Symbol`
- Array methods: `push`, `pop`, `map`, `filter`, `reduce`, `forEach`, `find`,
  `includes`, `indexOf`, `slice`, `concat`, `join`
- String methods: `charAt`, `slice`, `split`, `replace`, `includes`,
  `startsWith`, `endsWith`, `repeat`, `trim`, case conversions
- `parseInt`, `parseFloat`, `isNaN`, `isFinite`, JSON parse/stringify
- CLI with file execution, `-e` eval, `--version`, REPL

## Installation

```sh
git clone https://github.com/flyingsquirrel0419/RuJa.git
cd RuJa
cargo build --release
```

## Usage

```sh
ruja script.js              # run a file
ruja -e "1 + 2 * 3"         # eval
ruja                         # REPL
```

## Library API

```rust
use ruja::{Vm, Value};
fn main() {
    let mut vm = Vm::new();
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

## Known limitations

- Closures capturing outer locals: in progress (captured variables need
  environment-based storage rather than VM locals)
- `class` syntax, `async`/`await`, generators, `Promise` execution, modules:
  planned for v2.1
- test262 conformance harness: planned

## License

MIT
