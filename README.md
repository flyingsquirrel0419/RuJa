# RuJa

A JavaScript engine written in Rust.

RuJa is a tree-walking interpreter that runs a pragmatic ES5.1 subset of
JavaScript, plus a selection of ES2015+ conveniences (arrow functions, `let`/
`const`, template strings, `for...of`, spread, `nullish` coalescing). It ships
with a library API, a CLI, and an interactive REPL, all with zero external
runtime dependencies.

## Features

- Arithmetic, comparison, logical, bitwise, and assignment operators
- `var`, `let`, `const` with block and function scoping
- Control flow: `if`/`else`, `while`, `do...while`, `for`, `for...in`, `for...of`,
  `switch`, labeled `break`/`continue`
- Functions, closures, and arrow functions with lexical `this`
- Objects, arrays, and a real prototype chain with `new`, `instanceof`, `this`
- Error handling: `throw`, `try`/`catch`/`finally`, `Error`/`TypeError`/`RangeError`/
  `ReferenceError`/`SyntaxError`
- Built-in objects: `Object`, `Array`, `String`, `Number`, `Boolean`, `Function`,
  `Math`, `JSON`, `console`, and the `Error` family
- Array methods: `push`, `pop`, `shift`, `unshift`, `join`, `slice`, `concat`,
  `reverse`, `forEach`, `map`, `filter`, `reduce`, `find`, `includes`, `indexOf`
- String methods: `charAt`, `charCodeAt`, `slice`, `substring`, `split`,
  `replace`, `includes`, `startsWith`, `endsWith`, `repeat`, `trim`, case
  conversions, and more
- `parseInt`, `parseFloat`, `isNaN`, `isFinite`
- Spread in array literals and call arguments
- `nullish` coalescing (`??`) and optional logical assignment

## Installation

### From source

```sh
git clone https://github.com/flyingsquirrel0419/RuJa.git
cd RuJa
cargo build --release
```

The binary will be at `target/release/ruja`.

### As a library

Add to your `Cargo.toml`:

```toml
[dependencies]
ruja = "0.1.0"
```

## Usage

### Run a file

```sh
ruja script.js
```

### Evaluate an expression

```sh
ruja -e "1 + 2 * 3"
```

### Interactive REPL

```sh
ruja
```

## Library API

```rust
use ruja::{Interpreter, Value};

fn main() {
    let mut interp = Interpreter::new();
    let result = interp.run("let xs = [1, 2, 3]; xs.reduce((a, b) => a + b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

## Example

```javascript
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

let nums = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
console.log(nums.map(fib).join(", "));
// 0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
```

## Limitations

RuJa targets ES5.1 with selected ES2015+ additions. It does not yet support
`class` syntax, modules (`import`/`export`), `async`/`await`, generators, or
proxies. A future v2.0 will introduce a bytecode VM and a tracing garbage
collector for better performance.

## License

MIT
