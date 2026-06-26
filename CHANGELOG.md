# Changelog

## [2.0.0-alpha] - 2026-06-27

Complete rewrite: bytecode VM + mark-and-sweep GC.

### Added
- Stack-based bytecode VM (`vm.rs`) replacing the tree-walking interpreter
- Self-contained mark-and-sweep garbage collector (`gc.rs`)
- `HeapObj` enum value model with `GcIdx` handles, GC-traced
- AST-to-bytecode compiler with lexical scope resolution
- Function calls, recursion, `this` binding via VM call frames
- Function `.prototype` object creation and constructor linking
- `try`/`catch` via VM catch stack
- Global variable storage via `StoreGlobal`/`LoadGlobal`
- Built-in: `Math` (full), `JSON` (parse/stringify), `console`, `Object`,
  `Array` (push/map/filter/reduce/forEach/slice/concat/join/includes/indexOf),
  `String` (charAt/slice/split/replace/trim/case conversions/repeat),
  `Number`, `Boolean`, `Error`, `Map`, `Set`, `Symbol`
- `parseInt`, `parseFloat`, `isNaN`, `isFinite`, `NaN`, `Infinity`, `undefined`
- CLI with file execution, `-e` eval, `--version`, `--help`, REPL

### Changed
- Value model: `Rc<RefCell<Obj>>` replaced with GC-managed `HeapObj`
- Execution: tree-walking `interpreter.rs` removed; bytecode VM is the engine

### Known issues
- Closure capture of outer locals (partial)
- `class`/`async`/`await`/generators/Promise runtime not yet implemented
- test262 conformance harness pending

## [0.1.0-alpha] - 2026-06-26

Initial tree-walking interpreter release.
