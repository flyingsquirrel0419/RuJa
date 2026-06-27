# RuJa Roadmap

Legend: `[ ]` pending, `[~]` in progress, `[x]` done.

## v2.0 - Bytecode VM + GC (current)

1. [x] GC heap + value model (gc.rs, value.rs)
2. [x] Bytecode Op set + compiler (bytecode.rs, compiler.rs)
3. [x] Stack VM dispatch (vm.rs)
4. [x] Lexer/parser ES2015 extensions (class, destructuring, template literals, default/rest params)
5. [x] Built-in objects (Math/JSON/String/Array/console/Map/Set/Symbol; Promise stubbed)
6. [x] Closure variable capture + this binding
7. [x] ES2015: class/extends + super, Map/Set, Symbol, iterator protocol (for-of/for-in)
8. [x] async/await + generators (lazy generator with next/for-of/spread, infinite support)
9. [x] Built-in spec conformance + TDZ (catch routing + full TDZ hoisting for let/const)
10. [x] regression tests (243 passing, split across 8 files)
11. [x] Release prep (README/CHANGELOG/CI)
12. [x] Release verification (tests + CLI + metadata)

## v1.0 - Tree-walking interpreter (archived)

Completed and tagged as v0.1.0-alpha. See v1-archive branch.

## Remaining known limitations (post v2.0 ES2015 work)

- **Nested generator `next()` inside a generator body**: a generator that calls
  `next()` on *another* generator while it is itself running is not fully
  isolated (generator run-state is VM-global). Single-generator use — including
  infinite generators pulled with `next()`, `for...of`, and spread — works.
- **`yield*` delegation** (`yield* gen()`) is not supported (syntax error).
- **`async function*` / `await` inside generators** is not supported.
- **Computed property keys** in object literals are limited to identifiers/strings.
- **Default-parameter self-reference** (`function f(a = a)`) does not hit the
  TDZ (the parameter is initialized before defaults evaluate); minor spec gap.
- **`Symbol.iterator` customization**: built-in iterables (Array/String/Map/Set/
  Generator) iterate, but user-defined `Symbol.iterator` is not honored yet.
- **`eval`/`with`** are not implemented (intentionally).
