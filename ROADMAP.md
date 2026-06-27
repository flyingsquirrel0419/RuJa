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

## v2.1 - Spec completeness pass (current)

1. [x] PropertyKey model (Symbol-keyed properties via `[Symbol.iterator]`)
2. [x] Per-frame generator run-state (nested generator `next()` isolation)
3. [x] `yield*` delegation to iterables/generators
4. [x] Custom `Symbol.iterator` + lazy iterator protocol + computed keys
5. [x] `async function*` (next() returns Promise) + `await` in generators
6. [x] TDZ for default-parameter self-reference (`function f(a = a)`)
7. [x] `with` statement (dynamic object environment records)
8. [x] `eval` (indirect + direct) with runtime compilation

## v1.0 - Tree-walking interpreter (archived)

Completed and tagged as v0.1.0-alpha. See v1-archive branch.

## Remaining known limitations (post v2.1)

- **`for await...of`** is not supported; it now emits a clear `SyntaxError`
  ("for await...of is not supported") instead of a confusing parse error.
- **`yield*` delegation** forwards resume values and the delegated return
  value correctly, but `throw(v)` / `return(v)` sent *into* a delegated
  generator (via `yield*`) are not yet propagated to the inner generator.
- **Async generator scheduling** uses the synchronous microtask-drain model
  (a pending Promise is awaited by draining microtasks until it settles);
  there is no real event-loop preemption.
- **Direct eval lexical-environment isolation**: direct `eval` runs in the
  caller environment directly, so `let`/`const` declared inside `eval` leak
  to the caller (the spec's separate eval lexical environment, with an
  isolated operand stack, is a follow-up). `var`/function declarations
  leak to the caller's function scope as expected.
- **`with` statement `this` rebinding**: in sloppy mode, an unqualified
  function call inside `with` should bind `this` to the `with` object when
  the callee resolves through the object environment record; this is not
  implemented (requires name-resolution source tracking).
- **`with` in strict mode** is not rejected (strict mode is not implemented).
- **Computed/numeric keys in *declaration* destructuring** (`let {[expr]: x}
  = o`, `let {1: x} = o`): the `Pattern::Object` AST uses `Rc<str>` keys, so
  only string/identifier keys work in declaration destructuring. Assignment
  destructuring (`{[expr]: x} = o`) and object literals (`{[expr]: v}`)
  support computed keys.
- **Default-parameter reverse-order TDZ**: `function f(a = b, b = 2)` does
  not throw for `b` (the parameter binding is initialized from args before
  defaults evaluate); `function f(a = a)` does throw. Fixing the reverse
  case requires the `call_function` arg-binding to use uninitialized
  bindings.
- **Lexical duplicate-declaration checking** (`let a; let a;`, or a parameter
  shadowed by a same-name `let` in the body) is not enforced at compile time;
  later declarations silently overwrite.
- **Array destructuring of custom iterables** still uses index access rather
  than the iterator protocol (only `for...of`/spread use `Symbol.iterator`).
- **`Function` constructor** dynamic compilation is not exposed.
- **`eval`/`with` security sandbox** is absent (local execution assumed).
