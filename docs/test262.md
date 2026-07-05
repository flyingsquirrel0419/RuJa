# test262 conformance

RuJa runs the [test262](https://github.com/tc39/test262) conformance suite
via `tools/test262_runner.py`. The runner uses the **real test262 harness**
(`sta.js`, `assert.js`, and per-test `includes:` such as `propertyHelper.js`
and `compareArray.js`) rather than a hand-rolled stub, so tests relying on
`verifyProperty`, `compareArray`, etc. are exercised correctly. It also
parses `negative:` metadata so a test that expects a `SyntaxError`/
`TypeError` (parse or runtime phase) passes when RuJa raises the matching
error.

RuJa does **not** claim full ES conformance. Instead, it targets a
deliberately scoped subset of ES5.1 + selected ES2015+ features (see
[Supported subset](#supported-subset) below). Tests requiring unsupported
features (modules, TypedArrays, Atomics, Intl, etc.) are skipped via the
runner's `SKIP_FEATURES` set.

## Three pass-rate scopes

There are three distinct pass-rate numbers. Each measures a different
scope, so they are not comparable to each other:

| Scope | What it measures | Current rate | Where to verify |
|-------|-----------------|-------------|-----------------|
| **Full suite** | Entire test262 tree (excl. intl402/staging) — includes thousands of tests for features RuJa does not support | 23.9% of all matrix files; 49.9% of executed files | `test262-full` CI workflow job summary |
| **Supported subset** | `language/statements` + `language/expressions` — the areas RuJa actively targets, with unsupported-feature tests skipped | 99.1% | Run locally: `TEST262=… python3 tools/test262_runner.py language/statements language/expressions` |
| **CI subset** | 9 narrow directories the `ci.yml` job runs on every push (identifiers, keywords, types, comments, white-space, punctuators, arrow-function, function, object) | 92.0% | `CI` workflow job summary |

**The number to cite in README and public-facing material is the
supported-subset rate (99.1%).** It reflects the portion of the spec
RuJa actively targets. The full-suite number is published for
transparency but is dominated by unsupported features. The CI-subset
number is a narrow regression gate, not a conformance claim.

## Supported subset

RuJa targets conformance within a declared feature subset rather than
chasing the full test262 pass rate. The supported subset is:

**ES5.1 core**: var/let/const, all operators, control flow (if/else,
while, do-while, for, for-in, for-of, switch, break/continue, labeled
statements), functions, closures, try/catch/finally, throw, strict mode,
the full standard library (Array, String, Number, Math, Object, JSON,
Date, RegExp, Error hierarchy).

**Selected ES2015+**: arrow functions, classes/extends/super, default &
rest parameters, destructuring (array/object/nested), template literals,
tagged templates, computed property keys, object spread/rest, getters/
setters, Symbol.iterator, Map/Set/WeakMap/WeakSet, BigInt, Proxy,
Reflect, Promise, async/await, generators, for-of, optional chaining,
nullish coalescing, logical assignment.

**Intentionally unsupported**: ES Modules (import/export), Intl, Atomics,
SharedArrayBuffer, TypedArray (Uint8Array is partially supported),
WeakRef/FinalizationRegistry, Tail-call optimization.

Conformance within this subset is the goal — not the raw test262
percentage. The full-suite number includes thousands of tests for
features outside the supported subset, which naturally pulls the overall
rate down.

## Running

```sh
# Clone test262 (shallow, sparse checkout keeps it small):
git clone --depth 1 --filter=blob:none --sparse https://github.com/tc39/test262.git
cd test262 && git sparse-checkout set harness test/language

# Build a release binary (the runner expects target/release/ruja):
cargo build --release

# Run the supported subset (statements + expressions):
TEST262=/path/to/test262 python3 tools/test262_runner.py language/statements language/expressions

# Or run a narrower subtree:
TEST262=/path/to/test262 python3 tools/test262_runner.py language/identifiers language/keywords
```

For failure-bucket analysis with error samples, use the sibling analyzer:

```sh
python3 tools/analyze_failures.py
```

## Full-suite baseline

The `test262-full` CI workflow runs the entire test262 tree (excluding
`intl402`/`staging`) in parallel. Latest confirmed full run:

| Metric | Count |
|--------|-------|
| Total matrix files | 48,465 |
| Actually run | 23,164 |
| Pass | 11,560 |
| Fail | 11,604 |
| Timeout | 11 |
| Skip | 25,290 |
| **Pass rate (of run)** | **49.9%** |
| **Pass rate (of total)** | **23.9%** |

This number is dominated by tests for features RuJa does not support.
It is published for transparency and regression tracking, not as a
conformance claim. The CI job summary is the source of truth for the
current commit.

## CI subset detail

The `ci.yml` workflow runs a narrow 9-directory subset on every push.
This is a regression gate, not a conformance metric:

| Suite | Ran | Pass | Fail | Timeout | Pass rate |
|-------|-----|------|------|---------|-----------|
| identifiers | 206 | 165 | 41 | 2 | 80.1% |
| punctuators | 11 | 11 | 0 | 0 | 100.0% |
| white-space | 65 | 49 | 16 | 0 | 75.4% |
| keywords | 25 | 24 | 1 | 0 | 96.0% |
| types | 109 | 104 | 5 | 0 | 95.4% |
| comments | 20 | 14 | 6 | 0 | 70.0% |
| expressions/arrow-function | 90 | 90 | 0 | 0 | 100.0% |
| expressions/function | 53 | 53 | 0 | 0 | 100.0% |
| expressions/object | 285 | 285 | 0 | 0 | 100.0% |
| **Total** | 864 | 795 | 69 | 2 | 92.0% |

(Numbers move as bugs are fixed; the CI job summary is the source of truth
for the current commit.)

## What was fixed to get here

Key test262-driven bug fixes that raised the supported-subset rate from
~56% to 98.9%:

- **Lexer: Unicode identifiers** — `\uXXXX`/`\u{XXXX}` escape forms,
  Unicode letters, NEL/LS/PS line terminators.
- **Parser: destructuring parameters, object-literal methods, reserved
  words as property keys, `negative:` harness metadata.**
- **Switch/catch lexical scope** — `let`/`const` in case bodies and
  catch parameters are properly block-scoped.
- **Escaped get/set** — `\u0067et` is not treated as a getter keyword.
- **Class constructors** — `TypeError` when called without `new`;
  non-object return from derived constructor throws; double `super()`
  throws; `extends` validates the parent is a constructor.
- **Null-extending classes and bound subclass construction** —
  `class C extends null {}` uses `null` as `C.prototype`'s prototype and
  `%Function.prototype%` as `C`'s prototype, `super()` in null-extending
  classes throws `TypeError`, and `new (Sub.bind(...))` delegates construction
  to `Sub` with prepended bound arguments while ignoring bound `this`.
- **C-style `for` lexical head environments** — `for (let/const ...; ...; ...)`
  creates the loop-head lexical environment and per-iteration sibling
  environments required for initializer/test/body/update closures, rejects
  body `var` redeclarations of head lexical names, and parses `async of => {}`
  as an async-arrow initializer in ordinary `for` loops.
- **Label identifiers and strict labelled functions** — contextual `await`
  labels in non-module code and contextual `yield` labels in sloppy
  non-generator code parse correctly, including escaped spellings, while
  strict labelled function declarations are rejected during parsing.
- **Function statement-control parser boundaries and raw meta-property
  tokens** — nested function bodies reset loop/switch/label parse context,
  `async function` honors the no-LineTerminator rule, escaped `new.target`
  forms are rejected, and `debugger` is statement-only.
- **Named function-expression bindings** — named function expressions create
  an immutable inner name binding; sloppy assignments to that binding are
  ignored, while strict assignments throw `TypeError`.
- **Call-expression environment and argument ordering** — named function
  expression body `var` declarations can shadow the explicit function name
  binding, sloppy direct eval accepts `static` as a contextual `var` name, and
  member calls perform property lookup before argument evaluation.
- **BigInt increment/decrement** — `x++` on BigInt returns BigInt.
- **Object.prototype.toString** — returns `[object Object]` for plain
  objects; `String.prototype.toString` added.
- **Inline-cache invalidation** — `SetElem` and `Object.defineProperty`
  invalidate the monomorphic cache so writes are visible to subsequent
  reads.
- **BigInt divide/modulo by zero** — `1n / 0n` and `1n % 0n` throw
  `RangeError`.
- **BigInt exponent overflow** — huge exponents no longer clamp to zero
  and return a wrong value; they throw `RangeError`.
- **Addition primitive coercion** — binary `+` now runs `ToPrimitive` before
  BigInt mixing checks, performs string concatenation before mixed-BigInt
  errors, and gives Date default-hint coercion the Date string-order behavior.
- **Contextual `of` division lexing** — `/` after contextual `of` remains
  division in expression contexts while raw `of` delimiters in `for...of`
  heads still allow a following regex literal.
- **Arrow function early errors** — arrow functions reject duplicate
  parameter names in sloppy and strict mode, and reject `eval`/`arguments`
  parameter names when strict mode applies.
- **Tagged-template objects** — template-literal sites return a cached,
  frozen template object with a frozen `raw` property per
  `GetTemplateObject`.
- **Array exotic descriptors** — `Object.getOwnPropertyDescriptor` now
  returns descriptors for Array `length` and index properties.
- **Delete reference semantics** — non-reference operands are still evaluated
  before `delete` returns `true`; function parameters are non-deletable;
  configurable global object properties such as `JSON` delete correctly; and
  `delete super.x`/`delete super[x]` throw `ReferenceError` in spec order.
- **Update-expression Reference semantics** — prefix/postfix
  increment/decrement evaluate targets once, preserve the original Reference
  through `GetValue`/`PutValue`, and keep BigInt update results as BigInt.
- **Object literal computed property keys** — computed data/accessor names run
  `ToPropertyKey` before value/function evaluation and preserve Symbol keys.
- **String literal line continuations** — a backslash followed by a
  LineTerminatorSequence contributes no cooked characters, including when the
  string is used as a computed object accessor name.
- **Direct eval lexical declaration conflicts** — sloppy direct eval rejects
  `var`/function declarations that would hoist over a caller `let`/`const`
  binding, including object method/accessor bodies.
- **Object literal method semantics** — concise methods/accessors are
  non-constructors where required, ordinary concise methods lack an own
  `prototype`, and `super` property assignment uses the original receiver.
- **Object literal `__proto__` semantics** — duplicate prototype-mutation
  entries are early errors, while computed and shorthand `__proto__` entries
  remain ordinary data properties.
- **Live array-like `for...of` iteration** — arrays and arguments objects now
  use lazy indexed iteration so mutations during traversal are observed,
  accessor-index errors propagate, sloppy mapped arguments stay aliased to
  parameters until deleted, and `Object.defineProperty` on array indices
  updates array length.
- **`for...of` head parsing and early errors** — escaped `of` is no longer
  accepted as the loop delimiter, `async` is accepted as a contextual
  assignment target, invalid `const let` heads are rejected, and array/object
  left-hand sides must be valid assignment patterns.
- **`for...of` lexical head environments** — lexical loop heads now put their
  bound names in TDZ while evaluating the iterable, then create a fresh
  per-iteration environment before binding each value so destructuring
  defaults and body closures observe the correct binding.
- **`for...in` lexical head environments** — lexical loop heads now put their
  bound names in TDZ while evaluating the enumerated object, then create a
  fresh per-iteration environment before binding each property key so
  destructuring defaults and body closures observe the correct binding.
- **Compile-only loop scope unwinding** — labelled `continue` from an inner
  `for`/`for...in`/`for...of` now unwinds only real runtime environment
  records, preserving direct-eval completion values instead of over-popping the
  eval environment.
- **`for...in` enumeration and descriptor preservation** — non-enumerable own
  properties now shadow prototype keys during enumeration, deleted
  not-yet-visited keys are skipped, and `Object.defineProperty` preserves
  unspecified fields when redefining an existing descriptor while rejecting
  invalid non-configurable/non-extensible redefinitions.
- **Array-index assignment through prototype setters** — missing array-index
  writes now honor inherited setters before extending the array, fixing
  member-expression `for...in` assignment heads such as `[let][1]`.
- **Direct eval `var` leakage from lexical `for...in` heads** — sloppy direct
  eval now leaks `var`/function declarations to the caller's variable
  environment, not the temporary TDZ environment used while evaluating the
  right-hand object expression.
- **Ordinary object `[[Set]]` own-property precedence** — own accessor/data
  descriptors now handle assignment before inherited setters or inherited
  non-writable data properties, fixing writable checks for ordinary function
  `.prototype` and `prototype.constructor` descriptors.
- **Catch parameter early errors** — catch parameters now reject duplicate
  bound names and direct catch-block lexical/function redeclarations of the
  same name while preserving allowed `var` and nested block shadowing.
- **Native runtime errors through `finally`** — catchable VM-raised
  `ReferenceError`/`TypeError` completions now run active `finally` blocks
  before reaching an outer `catch`, matching explicit `throw` control flow,
  while re-thrown Error objects preserve their specific error kind.
- **Native Error constructor call branding** — plain calls to native Error
  subclasses now allocate through the active callee's `prototype`, so
  `EvalError(1)`, `TypeError(1)`, and aliased constructor calls preserve
  their specific `name`, `toString()`, and `instanceof` behavior.
- **Declarative binding deletion semantics** — `delete` now returns `false`
  for lexical/catch declarative bindings instead of removing them, preserving
  catch parameter values and matching `DeleteBinding` behavior for covered
  sloppy-mode `delete identifier` cases.
- **`try`/`finally` completion replacement semantics** — pending return,
  break, continue, and throw completions are preserved across normal
  `finally` evaluation, but correctly replaced by abrupt completions from the
  `finally` body. Non-throw abrupt completions also disable skipped catch
  guards before entering `finally`, bringing `language/statements/try` to
  **98 pass / 0 fail**.
- **Function.prototype restricted properties** — bound functions now inherit
  the `%Function.prototype%` `caller`/`arguments` accessors that throw
  `TypeError`, while still reporting no own `caller` or `arguments`
  properties.
- **Class heritage strictness and strict arguments objects** — class
  heritage expressions parse as strict code without making script-goal
  `await` class names illegal, and strict function calls create an unmapped
  `arguments` object with a restricted `callee` accessor.
- **Operator edge semantics** — BigInt exponentiation throws `RangeError` for
  negative exponents, BigInt relational comparison handles Boolean/nullish
  numeric operands through `ToNumeric`, `in` rejects primitive right-hand
  sides before property-key conversion, `instanceof` skips prototype lookup
  for primitive left-hand sides, and strict non-generator `yield` is a parse
  error.
- **Function body `"use strict"` with non-simple parameters** — function
  declarations, function expressions, object/class methods, and arrow block
  bodies now reject directive-prologue `"use strict"` when parameters contain
  defaults, rest, or destructuring.
- **Object/class method formal-parameter early errors** — concise and async
  object methods plus class/private methods reject duplicate formal parameter
  bound names, including destructuring duplicates, and object async methods
  enforce no line terminator between `async` and the property name.
- **`yield` contextual identifier parsing** — sloppy non-generator code now
  treats `yield` as an identifier for bindings, expressions, destructuring
  patterns, object method parameters/defaults, and computed property names,
  while generator contexts keep `yield` as the generator keyword.
- **`let` declaration ASI/lookahead parsing** — `let` followed by a binding
  name is parsed as a LexicalDeclaration across line terminators where
  StatementListItem permits declarations, escaped `let` remains an identifier,
  and single-statement bodies still use ExpressionStatement lookahead rules.
- **Parenthesized assignment-pattern targets** — parenthesized object/array
  literals are rejected as targets for an outer assignment while valid inner
  destructuring assignments remain accepted.
- **`await` contextual identifier parsing** — sloppy non-async code now treats
  `await` as a contextual identifier for bindings, references, parameters,
  destructuring patterns, object method parameters, and computed property
  names, while async function/method/arrow contexts keep `await` as the async
  keyword.
- **`import.meta` assignment target early errors** — direct and parenthesized
  `import.meta` assignments now fail in the parser, closing the
  `language/expressions/assignmenttargettype` subset.
- **Object destructuring assignment target order** — member-expression targets
  inside object assignment patterns are evaluated before source property
  `GetV`, including computed source keys, reducing
  `language/expressions/assignment/destructuring` to **1 pass / 5 fail**.
- **Array destructuring assignment iterator semantics** — array assignment
  patterns now use the iterator protocol, evaluate member targets before
  `IteratorStep`, apply defaults, close unfinished iterators on abrupt default
  evaluation or target assignment while preserving the original throw, and
  read lazy iterator `done` before `value`. This reduces
  `language/expressions/assignment/destructuring` to **5 pass / 1 fail**.
- **Duplicate `__proto__` object assignment properties** — object assignment
  patterns now allow duplicate static `__proto__` colon properties while
  object literals still reject duplicate prototype-mutation entries. Plain
  destructuring assignment expressions also leave the RHS value on the stack as
  their result. This brings `language/expressions/assignment/destructuring` to
  **6 pass / 0 fail**.
- **Computed property names in `for` heads** — computed property names and
  computed member keys now allow `in` inside their bracketed expressions even
  when the surrounding expression is parsed under `for (... in ...)`
  lookahead. This reduces `language/expressions/object` to
  **271 pass / 14 fail**.
- **Object literal strict early errors** — strict object literal shorthand
  properties reject reserved IdentifierReferences such as `let` and `yield`,
  and object accessors/methods apply body `"use strict"` directives to
  formal-parameter `eval`/`arguments` checks. This reduces
  `language/expressions/object` to **275 pass / 10 fail**.
- **String literal line continuations** — a backslash followed by a
  LineTerminatorSequence contributes no cooked characters inside string
  literals, including computed object accessor names. This reduces
  `language/expressions/object` to **276 pass / 9 fail**.
- **Direct eval lexical declaration conflicts** — sloppy direct eval rejects
  `var`/function declarations that conflict with caller `let`/`const`
  bindings before leaking hoisted declarations to the caller variable
  environment. This reduces `language/expressions/object` to
  **279 pass / 6 fail** and raises the supported subset to
  **3960 pass / 218 fail / 2 timeout**.
- **Object method parameter/body environments** — parameter defaults and
  destructuring preludes now run before the separate body variable environment
  is pushed, so parameter closures do not see body `var` declarations while
  direct eval `var`s created during parameter evaluation remain visible to
  parameter and body closures. Nested function/method parameter scratch state
  is isolated in the parser. This brings `language/expressions/object` to
  **285 pass / 0 fail** and raises the supported subset to
  **3979 pass / 199 fail / 2 timeout**.
- **Arrow formal-parameter early errors** — arrow functions reject duplicate
  bound names introduced by destructuring parameters and reject a line
  terminator before `=>` for parenthesized and parenless forms. Async-arrow
  lookahead also preserves the no-LineTerminator restrictions around `async`
  and `=>`. This brings
  `language/expressions/arrow-function/syntax/early-errors` to
  **25 pass / 0 fail** and raises the supported subset to
  **3990 pass / 188 fail / 2 timeout**.
- **Sloppy arrow contextual parameters** — non-strict arrow functions allow
  `eval`, `arguments`, and `yield` as formal parameter names where the grammar
  permits them, while strict enclosing code or a block-body `"use strict"`
  directive still rejects `eval`/`arguments`. This brings
  `language/expressions/arrow-function/syntax` to **45 pass / 0 fail**,
  raises `language/expressions/arrow-function` to **88 pass / 2 fail**, and
  raises the supported subset to **3996 pass / 182 fail / 2 timeout**.
- **Arrow lexical `arguments`** — arrow function calls no longer create their
  own `arguments` object binding, so `arguments` references inside an arrow
  resolve through the captured lexical environment unless shadowed by an
  explicit parameter. This raises `language/expressions/arrow-function` to
  **89 pass / 1 fail** and the supported subset to
  **3997 pass / 181 fail / 2 timeout**.
- **Lexical arrow `super()` binding order** — `super()` calls now perform the
  superclass constructor call before rebinding the derived constructor's
  lexical `this` environment and forward the active constructor's
  `new.target`. A repeated `super()` call, including one captured in an arrow
  and invoked after the constructor returns, now throws `ReferenceError` only
  after the superclass constructor has run. This closes
  `language/expressions/arrow-function` at **90 pass / 0 fail** and raises the
  supported subset to **3999 pass / 179 fail / 2 timeout**.
- **`super()` constructor mixed spread arguments** — `super(...)` now lowers
  mixed spread and non-spread arguments through the same iterator-backed
  argument-array path used by ordinary calls and `new`. This preserves
  left-to-right evaluation, handles empty spreads, and reports unresolvable
  spread operands as `ReferenceError`. This raises
  `language/expressions/super` to **30 pass / 6 fail** and the supported
  subset to **4003 pass / 175 fail / 2 timeout**.
- **Lexical arrow `super` property parsing** — block-bodied arrow functions
  now preserve the enclosing method's `super` parse context instead of
  resetting it like ordinary function bodies. This allows `super.x` and
  `super["x"]` inside arrows nested in object methods while still rejecting
  `super` in arrows without an enclosing super binding. This raises
  `language/expressions/super` to **32 pass / 4 fail** and the supported
  subset to **4005 pass / 173 fail / 2 timeout**.
- **Direct eval lexical `super` parsing** — direct eval now inherits the
  caller's `super` parse context when the caller environment has a `#super`
  binding. This allows `eval("super.x")` and computed `super` property access
  inside object methods while preserving SyntaxError for eval code without an
  enclosing super binding. This raises `language/expressions/super` to
  **34 pass / 2 fail** and the supported subset to
  **4007 pass / 171 fail / 2 timeout**.
- **Computed `super[...]` putvalue evaluation order** — compound assignment
  and update expressions now evaluate a `super` property target by checking
  the derived constructor `this` binding before evaluating a computed property
  expression, then reuse the same receiver/base/key reference for the get and
  set. This closes `language/expressions/super` at **36 pass / 0 fail** and
  raises the supported subset to **4009 pass / 169 fail / 2 timeout**.
- **Nullish/logical chain early errors** — unparenthesized `??` mixed directly
  with `&&` or `||` now throws a parse-time `SyntaxError`, while
  parenthesized combinations still parse and evaluate. This closes
  `language/expressions/coalesce` at **22 pass / 0 fail** and raises the
  supported subset to **4013 pass / 165 fail / 2 timeout**.
- **BigInt `ToNumeric` operator semantics** — unary plus and unsigned right
  shift reject BigInt operands with `TypeError`, while BigInt-aware
  arithmetic, bitwise, and signed shift operations preserve BigInt results
  after `ToNumeric`, including boxed BigInts. `ToNumber` no longer silently
  converts BigInt except through `Number()`, and string numeric conversion no
  longer accepts incorrectly-cased Infinity spellings. This closes the BigInt
  failures in `bitwise-and`, `bitwise-or`, `bitwise-xor`, and
  `unsigned-right-shift`, reduces `unary-plus` to **0 failures**, and raises
  the supported subset to **4034 pass / 144 fail / 2 timeout**.
- **Native Error subclass construction** — `Error.prototype` now inherits
  `Object.prototype` during bootstrap, NativeError subclass instances no
  longer receive own `message` properties when the message argument is
  omitted, and `name` is inherited through the prototype chain so
  `class Err extends EvalError {}` instances report `EvalError`. This closes
  `language/statements/class/subclass/builtin-objects/NativeError` at
  **18 pass / 0 fail** and raises the supported subset to
  **4047 pass / 131 fail / 2 timeout**.
- **Class element grammar and named class expression scope** — class bodies
  now accept empty `;` elements, computed accessor names, and generator
  methods, and named class expressions create an inner immutable class-name
  binding instead of leaking the name to the outer scope. Class names now
  reject `yield` even in sloppy surrounding scripts. This closes
  `language/expressions/class` at **48 pass / 0 fail**, improves
  `language/statements/class/syntax` to **9 pass / 4 fail**, and raises the
  supported subset to **4061 pass / 117 fail / 2 timeout**.
- **Class declaration early errors** — script and block statement lists now
  reject duplicate lexical class declarations and lexical/`var` name clashes
  during parsing, and escaped `static` is no longer accepted as the class
  `static` modifier. This improves `language/statements/class/syntax` to
  **12 pass / 1 fail** and raises the supported subset to
  **4064 pass / 114 fail / 2 timeout**.
- **Class `super` property HomeObject setup** — class constructors and
  instance methods now bind `Class.prototype` as their SuperProperty
  HomeObject, while static methods, static accessors, and static blocks bind
  `Class`. SuperProperty evaluation reads the HomeObject prototype
  dynamically. This closes `language/statements/class/super` and
  `language/statements/class/syntax` at **21 pass / 0 fail** and raises the
  supported subset to **4069 pass / 109 fail / 2 timeout**.
- **Class definition/name-binding semantics** — class declarations now hoist as
  immutable lexical bindings, anonymous class assignment infers constructor
  display names, class bodies parse nested functions in strict context, and
  method/accessor display names no longer create body bindings that shadow
  outer variables. `extends` evaluates the superclass `prototype` getter
  exactly once and derived constructors return the `this` object bound by
  `super()` when no object is explicitly returned. This closes
  `language/statements/class/definition` and
  `language/statements/class/name-binding` at **41 pass / 0 fail** and raises
  the supported subset to **4080 pass / 98 fail / 2 timeout**.
- **Dynamic class `super` references** — `super` property reads, calls, simple
  assignments, updates, and compound assignments now derive the super base
  from the method HomeObject at evaluation time instead of using a stale
  class-definition-time prototype value. This follows later
  `Object.setPrototypeOf` changes, and simple `super.x = rhs` /
  `super[expr] = rhs` evaluates `rhs` before throwing `TypeError` when the
  dynamic super base is `null`. This closes `language/expressions/assignment`
  at **110 pass / 0 fail** and raises the supported subset to
  **4082 pass / 96 fail / 2 timeout**.
- **Null-extending classes and bound subclass construction** —
  `extends null` now uses the spec prototype parents and `super()` failure
  behavior, while bound class construction ignores bound `this` and delegates
  to the target constructor with bound arguments. This raises
  `language/statements/class/subclass` to **75 pass / 19 fail** and the
  supported subset to **4089 pass / 89 fail / 2 timeout**.
- **C-style `for` lexical head environments** — ordinary `for` loops now use
  runtime loop-head and per-iteration environments for lexical declarations,
  including destructuring heads and the update-before-next-iteration boundary.
  Parser early errors now reject body `var` redeclarations of head lexical
  names, while `async of => {}` remains a normal async-arrow initializer. This
  closes `language/statements/for` at **93 pass / 0 fail** and raises the
  supported subset to **4103 pass / 77 fail / 0 timeout**.
- **Label identifiers and strict labelled functions** — labelled statements
  now accept contextual `await` labels in non-module code and contextual
  `yield` labels in sloppy non-generator code, including escaped spellings.
  Strict labelled function declarations now fail during parsing instead of
  executing. This closes `language/statements/labeled` at **17 pass / 0 fail**
  and raises the supported subset to **4108 pass / 72 fail / 0 timeout**.
- **Function statement-control parser boundaries and raw meta-property
  tokens** — parser state for loops, switches, and labels no longer leaks into
  nested function bodies, so inner `break`/`continue` cannot target an outer
  function's labels. `async function` declarations and expressions now require
  no line terminator between the keywords, `new.target` requires raw `new` and
  `target` tokens, and `debugger` is a statement-only keyword. This closes
  `language/statements/break`, `language/statements/continue`,
  `language/statements/debugger`, `language/statements/async-function`, and
  `language/expressions/new.target` in the supported subset, raising it to
  **4114 pass / 66 fail / 0 timeout**.
- **Named function-expression bindings** — named function expressions now
  create an immutable inner name binding. Sloppy assignment, including through
  direct eval or a nested lexical arrow, is ignored; strict assignment throws
  `TypeError`. This closes `language/expressions/function` at
  **53 pass / 0 fail** and raises the supported subset to
  **4118 pass / 62 fail / 0 timeout**.
- **Call-expression environment and argument ordering** — explicit named
  function-expression bindings now live in the function closure environment
  rather than the call body's variable environment, so body `var` declarations
  with the same name create the required separate binding. Sloppy direct eval
  accepts `static` as a contextual `var` binding name, and member calls perform
  property lookup before argument evaluation while keeping the callability
  check after argument evaluation. This improves `language/expressions/call`
  to **48 pass / 1 fail** and raises the supported subset to
  **4121 pass / 59 fail / 0 timeout**.
- **Tagged-template call context and conditional `in` grammar** — tagged
  templates used as member expressions now preserve their receiver as `this`,
  ``new tag`...` `` constructs the tag result rather than the tag function
  itself, and constructor arguments after a tagged template are applied to that
  result. Conditional-expression true branches now allow `in` even inside
  no-`in` contexts such as `for` heads. This reduces
  `language/expressions/tagged-template` to the remaining cross-realm
  `$262.createRealm()` case, closes
  `language/expressions/conditional/in-branch-1.js`, and raises the supported
  subset to **4124 pass / 56 fail / 0 timeout**.
- **Boxed String methods and Date method surface** — String prototype methods
  now read the wrapped primitive from `new String(...)` objects, so indexed
  operations like `charAt` agree with boxed string index properties. The
  bootstrap also installs `String.prototype.length`, `Date.parse`, `Date.UTC`,
  and the ES5 Date prototype method surface needed for property-access checks.
  This closes `language/expressions/property-accessors` at **15 pass / 0
  fail** and raises the supported subset to **4127 pass / 53 fail / 0
  timeout**.
- **Switch CaseBlock scoping and redeclarations** — switch `var`
  declarations bind in the enclosing variable environment, function
  declarations in case bodies stay scoped to the CaseBlock, and switch
  redeclaration early errors treat function declarations as lexical names.
  This closes `language/statements/switch` at **69 pass / 0 fail** and raises
  the supported subset to **4133 pass / 47 fail / 0 timeout**.
- **Class heritage strictness and strict arguments objects** — class
  heritage expressions now parse under strict mode while preserving
  script-goal `await` class names, and strict function calls now create an
  unmapped `arguments` object whose `callee` accessor throws `TypeError`.
  This closes `language/statements/class/strict-mode` at **2 pass / 0 fail**
  and raises the supported subset to **4135 pass / 45 fail / 0 timeout**.
- **Operator edge semantics** — BigInt exponentiation now throws `RangeError`
  for negative exponents, BigInt relational comparisons now coerce
  Boolean/nullish numeric operands through `ToNumeric`, `in` now rejects
  primitive right-hand sides before property-key conversion, `instanceof` now
  returns `false` for primitive left-hand sides before reading `prototype`,
  and strict non-generator `yield` is rejected during parsing. This closes
  `language/expressions/exponentiation`, `greater-than`, `less-than`, `in`,
  and `instanceof` at **188 pass / 0 fail** and raises the supported subset
  to **4142 pass / 38 fail / 0 timeout**.

## Why the rate is not higher

The remaining 38 failures in the supported subset cluster around class
behavior, super semantics, and a few expression edge cases. These are
tracked in `HANDOFF.md` and will be addressed in subsequent rounds.
