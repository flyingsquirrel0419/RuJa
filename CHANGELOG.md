# Changelog

## [Unreleased]

### Added (v2.1 spec-completeness pass)
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

### Added
- Temporal Dead Zone (TDZ) for `let`/`const`: lexical bindings are hoisted as
  uninitialized at scope entry; reading or assigning them before the declaration
  throws `ReferenceError: Cannot access '<name>' before initialization`. Also
  fixes `const` reassignment detection in block/function scopes.
- Generators are now lazy (pull-based): `function*` bodies run incrementally
  across `next()` calls and suspend at each `yield`, so infinite generators
  (`while(true) yield`) no longer hang. `next(v)` resumes with a value, and an
  explicit `return` ends the generator. (Previously generators were eagerly
  evaluated, which could not terminate on infinite generators.)
- Destructuring assignment to existing bindings: `[a, b] = expr`,
  `{a, b} = expr` (including swaps, holes, rest, rename, and nested patterns)
- Object literal shorthand properties: `{x, y}` (equivalent to `{x: x, y: y}`)
- `yield` operand now spans the full assignment-expression, so `yield 1 + 1`
  means `yield (1 + 1)` per spec (was parsed as `(yield 1) + 1`)
- `Promise.resolve(v)` and `Promise.reject(r)` static methods
- Generators (function*/yield) with eager evaluation: next(), for...of, and spread over generators
- async/await: async functions return a Promise; await extracts a Promise result synchronously (drains microtasks)
- try/catch now catches runtime errors (TypeError, ReferenceError, ...) not just JS throw; native errors are surfaced as Error objects with name/message
- null/undefined property access now throws TypeError instead of returning undefined
- Object property insertion order is now preserved (`for...in`, `Object.keys`,
  `Object.entries`, and `JSON.stringify` yield keys in insertion order) using `IndexMap`
- JSON.stringify now throws TypeError on circular references (was: stack overflow)
- `Number.toString` now uses ECMAScript exponential notation for values >= 1e21
  and < 1e-6 (e.g. `1e21` prints as `1e+21`, was full digits)
- Regex literals `/pattern/flags` with a `RegExp` object, `test`/`exec`, `String.replace` with regex (including the `g` flag), and reserved-word property names after `.`
- Promise constructor with `then`/`catch` chaining, derived-promise resolution, microtask draining, and executor `resolve`/`reject` binding
- Optional chaining operator `?.` for property access (`a?.b`), computed access
  (`a?.[b]`), and calls (`a?.b()`, `f?.()`) with null/undefined short-circuit
  semantics (new `QuestionDot` token + `optional` flag on Member/Call AST nodes)
- Nullish coalescing operator `a ?? b` (JumpIfNotNullish opcode) with correct
  short-circuit semantics (`0 ?? 2` returns `0`, not `2`)
- Logical assignment operators `&&=`, `||=`, `??=` with short-circuit semantics
  on identifier, member, and element targets
- Compound assignment (`+=`, `-=`, ...) on member and element targets
  (previously only identifiers worked)
- `for...of` / `for...in` iteration with iterator protocol (HeapObj::Iterator, GetIterator/IteratorNext/GetForInKeys opcodes)
- ES2015 `class extends` with prototype-chain linking and static inheritance
- `super.method()` calls (CallSuper opcode, `#super` binding)
- Template literal interpolation `${...}` (lexer template state machine + Expr::TemplateInterp)
- Default parameters (`function f(a, b = 10)`) and rest parameters (`function f(...args)`)
- Array/object destructuring in declarations and `for...of` (nested, rename, default, rest)
- Array methods: `find`, `findIndex`, `findLast`, `fill`, `some`, `every`
- `Symbol.prototype.toString` + `symbol_proto`
- Array spread in literals `[...iterable]` (NewArray + ArrayPush/SpreadPush)
- `Array.sort`, `Array.reverse`, `Object.keys/values/entries/assign`
- `String.split` limit argument; `Array.includes` with NaN (SameValueZero)
- Error subclasses: TypeError/RangeError/ReferenceError/SyntaxError/EvalError/URIError
- Function declaration hoisting (top-level and within blocks)

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

## [0.2.0] - 2026-06-27

### Added
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

## [0.1.0-alpha] - 2026-06-26

Initial alpha: tree-walking interpreter, ES5.1 subset, 56 tests.
