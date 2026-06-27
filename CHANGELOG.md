# Changelog

## [Unreleased]

### Added
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
