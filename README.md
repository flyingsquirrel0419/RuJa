# RuJa

A JavaScript engine written in Rust.

RuJa v2.0 is a **bytecode VM** with a **mark-and-sweep garbage collector**,
replacing the v1.0 tree-walker. It runs a pragmatic ES5.1 subset plus
selected ES2015 features, with zero external runtime dependencies.

## Architecture

- **Lexer** (`lexer.rs`) - tokenization with ASI and template literal support
- **Parser** (`parser.rs`) - Pratt-style recursive descent producing an AST
- **Compiler** (`compiler.rs`) - single-pass AST to bytecode compilation with
  lexical scope resolution
- **Bytecode** (`bytecode.rs`) - stack-machine instruction set (`Op`)
- **VM** (`vm.rs`) - dispatch loop with call frames, operand stack, property
  access, and type coercion
- **GC** (`gc.rs`) - mark-and-sweep collector tracing from VM roots
- **Values** (`value.rs`) - `HeapObj` enum (Object/Array/Function/Environment/
  Map/Set/Promise/Generator) referenced by `GcIdx` handles
- **Builtins** (`builtins.rs`) - Object/Array/String/Number/Boolean/Function/
  Math/JSON/console/Error + globals

## Features

- Arithmetic, comparison, logical, bitwise, and assignment operators
- `var`/`let`/`const` with environment-based scoping
- Control flow: `if`/`else`, `while`, `do...while`, `for`, `for...in`,
  `for...of`, `switch`, `break`/`continue`
- Functions, recursion, and arrow functions
- `throw`/`try`/`catch`/`finally` with `Error` type hierarchy
- Objects, arrays, prototype chain, `new`, `instanceof`
- Built-in objects: `Object`, `Array`, `String`, `Number`, `Boolean`,
  `Function`, `Math`, `JSON`, `console`, `Error`/`TypeError`/`RangeError`/
  `ReferenceError`/`SyntaxError`
- Array methods: `push`, `pop`, `map`, `filter`, `reduce`, `forEach`, `find`,
  `includes`, `indexOf`, `slice`, `concat`, `join`, and more
- String methods: `charAt`, `charCodeAt`, `slice`, `split`, `replace`,
  `includes`, `startsWith`, `endsWith`, `repeat`, `trim`, case conversions
- `parseInt`, `parseFloat`, `isNaN`, `isFinite`
- JSON parse and stringify
- Mark-and-sweep GC reclaiming reference cycles
- CLI with file execution, `-e` eval, `--version`, `--help`

## Installation

```sh
git clone https://github.com/flyingsquirrel0419/RuJa.git
cd RuJa
cargo build --release
```

## Usage

```sh
ruja script.js        # run a file
ruja -e "1 + 2 * 3"   # evaluate an expression
ruja                  # start the REPL
```

## Example

```javascript
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
let nums = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
console.log(nums.map(fib).join(", "));
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

- Closure variable capture (in progress)
- `this` binding in constructors (in progress)
- ES2015 `class` syntax, `Map`/`Set`/`Promise`/`Symbol` (stubbed)
- `async`/`await`, generators (not yet implemented)
- test262 conformance (targeted for follow-up)

## License

MIT
