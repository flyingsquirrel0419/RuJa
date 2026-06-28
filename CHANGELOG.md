# Changelog

## [Unreleased]

## [0.2.0] - 2026-06-28

### Added
- **Symbol-keyed properties**: a `PropertyKey` model (string/Symbol) backs all
  object `props` maps, so `[Symbol.iterator]` and arbitrary Symbol keys store
  and read correctly and are skipped by `for...in`/`JSON.stringify`.
- **Per-frame generator run-state**: `gen_mode`/`gen_yield`/`gen_suspended`/
  `gen_resume_value` moved from VM-global fields into `CallFrame`, so a
  generator body that calls `next()` on another generator is fully isolated.
- **`yield*` delegation**: `yield* expr` forwards each value of a delegated
  iterable/generator to the outer generator (supports arrays, strings, nesting).
- **Custom `Symbol.iterator`**: `make_iterator` honors a user-defined
  `[Symbol.iterator]()` method, wrapping the returned iterator in a lazy
  `IteratorData` that calls the JS `next()` per pull (infinite iterables work).
- **Computed property keys** `[expr]` in object literals now accept any
  expression (was restricted to identifiers/strings).
- **`async function*`**: `next()` returns a Promise resolved with `{value, done}`;
  `await` works inside the body (synchronous microtask-drain model).
- **TDZ for default-parameter self-reference**: `function f(a = a)` throws
  `ReferenceError` when the default is used (parameter is in the TDZ during
  default evaluation).
- **`with` statement**: dynamic object environment records; name lookups and
  assignments check the `with` object's properties first (precedence over the
  lexical chain), then fall back to lexical/global.
- **`eval`**: global `eval(x)` returns non-strings unchanged and parses/compiles/
  runs strings at runtime. Indirect eval runs globally (var leaks to global);
  direct `eval(...)` is detected at compile time and runs in the caller's scope.

- **Strict mode**: `"use strict"` directive prologues are parsed and propagated
  through the AST/compiler scope chain. `with` is a SyntaxError in strict mode;
  duplicate formal parameters are rejected (non-strict still allows them, last
  wins via a per-parameter slot map). Classes are always strict.
- **Generator `throw`/`return` injection**: `g.throw(e)` injects the exception
  at the suspended `yield` point (the body's try/catch can handle it; otherwise
  it propagates out). `g.return(v)` force-completes the generator with `v`.
  Driven by a new `ResumeKind` (Next/Throw/Return) and a frame-level
  `force_throw`.
- **`for await...of`**: async iteration via `Symbol.asyncIterator` (falling
  back to the sync `Symbol.iterator` protocol), awaiting each `next()` result.
  `Symbol.asyncIterator` is now exposed on the global `Symbol` object.
- **Direct eval lexical isolation**: `let`/`const`/`class` declared in direct
  `eval` no longer leak to the caller; `var`/function declarations still leak to
  the caller's function scope (and not over existing lexical bindings).
- **Iterator protocol for array destructuring**: `let [a, b] = iterable` now
  uses the iterator protocol, so generators, custom iterables, and strings
  destructure correctly (not just arrays). Rest uses a new `IteratorCollectRest`.
- **`Function` constructor**: `new Function(p0, ..., body)` dynamically compiles
  a function from parameter and body strings; a body `"use strict"` directive
  is honored (strict body rejects duplicate parameters).
- **Strict eval sandbox (minimal)**: under strict mode, direct eval no longer
  leaks `var` to the caller (in-function). `Chunk.is_strict` threads caller
  strictness to the eval.

- Bytecode compiler: AST -> stack-machine Op codes (single-pass, lexical scopes)
- Stack-based VM with call frames, operand stack, and return/call dispatch
- Mark-and-sweep garbage collector (gc.rs) tracing from VM roots
- New value model: HeapObj enum with GcIdx heap handles
- Environment-based variable storage (environment.rs)
- Try/catch/finally with Throw jumping to catch handlers
- Built-in objects: Object/Array/String/Number/Boolean/Function/Math/JSON/console/Error
- Array methods: push, pop, map, filter, reduce, forEach, find, includes, slice, concat, join
- String methods: charAt, charCodeAt, slice, split, replace, includes, startsWith, endsWith, repeat, trim, toUpperCase, toLowerCase
- Math: floor, ceil, round, abs, sqrt, pow, max, min, sin, cos, tan, log, exp, random, and constants
- JSON parse and stringify
- parseInt, parseFloat, isNaN, isFinite globals
- 17 passing integration tests + 13 unit tests

### Changed
- Replaced v1.0 tree-walking interpreter with bytecode VM
- Replaced Rc<RefCell> value model with GC-managed HeapObj
- Variables stored in environment chain instead of local slots

### Fixed
- Silent bug: `for...of` produced wrong values (0/empty) — was not compiled
- `extends` inheritance: subclass methods now resolve through the prototype chain
- `super.f() + 5` now returns 15 (was 2)
- Static methods now return their value (e.g. `C.s()` returns 42)
- `for...in` no longer leaks non-enumerable builtin prototype methods
- `break`/`continue` were no-ops (caused infinite loops) — now functional via loop jump stack
- `++`/`--` threw or returned wrong values — correct prefix/postfix semantics + store back
- Unary `+` was negation — now coerces to number (`+"5" === 5`)
- `>`/`>=` on strings always returned false — now correct
- `in` operator returned the key — now returns a boolean
- `void` returned its operand — now returns undefined
- `delete` returns boolean and removes the property
- `instanceof` returns a boolean (walks the prototype chain)
- `typeof undeclaredVar` threw — now returns "undefined"
- `switch` fallthrough and `default` were broken — now correct
- `finally` blocks never executed — now run on both try-normal and catch paths
- `Math.round` rounds half toward +Infinity per ES (`round(-0.5) === 0`)
- Default-param prologue left a stale stack value corrupting subsequent calls
- Builtin prototype methods and `constructor` are now non-enumerable
- Error constructor now links instances to `<Error>.prototype` (instanceof works)

## [0.1.0-alpha] - 2026-06-26

Initial alpha: tree-walking interpreter, ES5.1 subset, 56 tests.
