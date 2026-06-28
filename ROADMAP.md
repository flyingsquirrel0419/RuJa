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
9. [x] Lexical duplicate-declaration checking at compile time (`let a; let a;`)
10. [x] Computed/numeric keys in declaration destructuring (`let {[k]: a} = o`)
11. [x] Default-parameter reverse-order TDZ (`function f(a = b, b = 2)` throws)
12. [x] `with` statement `this` rebinding for unqualified calls
13. [x] Strict mode directive prologue (`"use strict"`) + `with` rejection + duplicate-param rejection
14. [x] Generator `throw`/`return` injection at yield points
15. [x] `for await...of` async iteration (`Symbol.asyncIterator`)
16. [x] Direct eval lexical-environment isolation (`let`/`const` don't leak)
17. [x] Iterator protocol for array destructuring patterns
18. [x] `Function` constructor (`new Function(p..., body)`)
19. [x] Strict eval minimal sandbox (no `var` leak under strict mode)

## v1.0 - Tree-walking interpreter (archived)

Completed and tagged as v0.1.0-alpha. See v1-archive branch.

## Remaining known limitations (post v2.1)

- **`yield*` throw/return propagation**: a `throw(v)`/`return(v)` sent into a
  delegated generator (via `yield*`) is not yet forwarded to the inner
  generator's `throw`/`return` (the direct `g.throw`/`g.return` work).
- **Async generator scheduling** uses the synchronous microtask-drain model
  (a pending Promise is awaited by draining microtasks until it settles);
  there is no real event-loop preemption.
- **Strict-mode edge cases**: `with` is rejected in strict mode and duplicate
  params are rejected, but some strict-mode behaviors (e.g. strict `this`
  defaulting, `arguments`/parameter mapping restrictions, `eval`/`arguments`
  as binding names) are not fully enforced. RuJa's `this` defaults to
  `undefined` in all modes by design.
- **Top-level eval `var` under strict mode**: in-function strict eval does not
  leak `var`, but a top-level strict eval still routes `var` through the global
  slot path (the eval source compiles as a top-level program).
- **Nested array-of-generator destructuring**: `[a, [b, c]] = gen()` where the
  generator yields arrays has a residual edge case; the common destructuring
  cases (arrays, generators, custom iterables, strings, rest) work.
- **`eval`/`with` security sandbox**: there is no process-level isolation;
  execution is local-trust by design.
