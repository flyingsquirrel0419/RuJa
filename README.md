# RuJa

A JavaScript engine written in Rust.

RuJa is a **bytecode VM** with a **mark-and-sweep garbage collector**,
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
 - Functions, recursion, arrow functions, and **closures** (variable capture + mutation)
- `throw`/`try`/`catch`/`finally` with `Error` type hierarchy
- Built-in objects: `Object`, `Array`, `String`, `Number`, `Boolean`,
  `Function`, `Math`, `JSON`, `console`, `Promise`, `RegExp`, `Map`, `Set`,
  `Symbol`, `Error`/`TypeError`/`RangeError`/`ReferenceError`/`SyntaxError`
- ES2015: `class`/`extends`/`super`, template literals with `${}`,
  default & rest parameters, array/object destructuring, `for...of`/`for...in`
- ES2015 destructuring assignment to existing bindings: `[a, b] = expr`,
  `{a, b} = expr` (swaps, holes, rest, rename, nested) and object shorthand `{x, y}`
- Temporal Dead Zone (TDZ) for `let`/`const`: accessing a lexical binding
  before its declaration throws `ReferenceError`; `const` reassignment is rejected
- Logical operators with correct short-circuit semantics: `&&`, `||`, and
  nullish coalescing `??` (keeps falsy-but-non-null left operands such as `0`)
- Logical assignment `&&=`, `||=`, `??=` and compound assignment (`+=`, `-=`,
  ...) on identifier, member, and element targets
- Optional chaining `?.` for property access (`a?.b`), computed access
  (`a?.[b]`), and calls (`a?.b()`, `f?.()`)
- Regex literals `/pattern/flags` with `RegExp` (`test`, `exec`, `match`, `source`,
  `flags`) and `String.replace` with regex
- `Promise` with `then`/`catch` chaining and microtask draining
- `Promise.resolve`/`Promise.reject` static methods
- `async`/`await` (async functions return a Promise; await resolves it synchronously)
- Lazy generators (`function*`/`yield`): pull-based `next()`/`for...of`/spread
  that suspend at each `yield`, supporting infinite generators; `next(v)` resumes
  with a value; `return` ends the generator
- `try`/`catch` catches runtime errors (TypeError/ReferenceError) and native
  errors surface as `Error` objects
- Array methods: `push`, `pop`, `shift`, `unshift`, `splice`, `map`, `filter`,
  `reduce`, `forEach`, `find`, `findIndex`, `findLast`, `fill`, `some`, `every`,
  `includes`, `indexOf`, `lastIndexOf`, `slice`, `concat`, `join`, `flat`,
  `flatMap`, `at`, `sort`, `reverse`, `copyWithin`; `Array.from`/`of`/`isArray`
- String methods: `charAt`, `charCodeAt`, `slice`, `split`, `replace` (regex
  supported), `replaceAll`, `includes`, `startsWith`, `endsWith`, `repeat`,
  `padStart`/`padEnd`, `at`, `trim`/`trimStart`/`trimEnd`, `substring`, case
  conversions
- `parseInt`/`parseFloat` (prefix parsing), `isNaN`, `isFinite`; `Number`
  statics (`isInteger`, `isFinite`, `isNaN`, constants) and `toString(radix)`
- JSON parse and stringify
- Mark-and-sweep GC reclaiming reference cycles
- CLI with file execution, `-e` eval, `--version`, `--help`
- **Symbol-keyed properties**: `[Symbol.iterator]` and arbitrary Symbol keys are
  stored/read on objects and skipped by `for...in`/`JSON.stringify`
- **Per-frame generator isolation**: a generator body may call `next()` on
  another generator without corrupting either's run-state
- **`yield*` delegation** to generators, arrays, and strings (nestable)
- **Custom `Symbol.iterator`**: objects with `[Symbol.iterator]()` are iterable
  via `for...of`/spread; lazy iterators call the JS `next()` per pull
- **Computed property keys** `[expr]` in object literals (any expression)
- **`async function*`**: `next()` returns a Promise; `await` works in the body
- **TDZ for default-parameter self-reference** (`function f(a = a)` throws)
- **`with` statement** (dynamic object environment records)
- **`eval`** (indirect runs globally; direct `eval(...)` runs in the caller's
  scope), with runtime parse/compile

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

- No `eval`/`with` process-level security sandbox (local-trust execution model)
- Async generator scheduling uses a synchronous microtask-drain model (no real event-loop preemption)
- test262 conformance is a future target, not achieved
- `yield*` throw/return propagation into a delegated generator is not yet forwarded (direct `g.throw`/`g.return` work)
- Some strict-mode edge cases are not fully enforced (e.g. strict `this` defaulting to the global object is not done - RuJa's `this` defaults to `undefined` in all modes by design)

## License

MIT
