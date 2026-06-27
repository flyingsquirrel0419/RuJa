# Changelog

## [Unreleased]

### Added
- `for...of` / `for...in` iteration with iterator protocol (HeapObj::Iterator, GetIterator/IteratorNext/GetForInKeys opcodes)
- ES2015 `class extends` with prototype-chain linking and static inheritance
- `super.method()` calls (CallSuper opcode, `#super` binding)
- Template literal interpolation `${...}` (lexer template state machine + Expr::TemplateInterp)
- Default parameters (`function f(a, b = 10)`) and rest parameters (`function f(...args)`)
- Array/object destructuring in declarations and `for...of` (nested, rename, default, rest)
- Array methods: `find`, `findIndex`, `findLast`, `fill`, `some`, `every`
- `Symbol.prototype.toString` + `symbol_proto`

### Fixed
- Silent bug: `for...of` produced wrong values (0/empty) — was not compiled
- `extends` inheritance: subclass methods now resolve through the prototype chain
- `super.f() + 5` now returns 15 (was 2)
- Static methods now return their value (e.g. `C.s()` returns 42)
- `for...in` no longer leaks non-enumerable builtin prototype methods

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
