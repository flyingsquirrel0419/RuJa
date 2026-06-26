# Changelog

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
