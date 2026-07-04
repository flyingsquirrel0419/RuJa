# Changelog

## [Unreleased]

### test262 conformance improvements

Supported-subset pass rate: **93.1%** (up from 88.6%).
Current supported subset count: **3889 pass / 289 fail / 2 timeout**.

- **Object.prototype.propertyIsEnumerable**: implemented the missing
  prototype method, including Symbol keys, array index/length behavior, string
  index enumerability, and nullish receiver errors. This unblocks test262
  `propertyHelper.js` descriptor checks for object literal accessor and method
  definitions.
- **Live array-like `for...of` iteration**: array and arguments-object
  iterators now read `length` and indexed properties lazily on each pull
  instead of snapshotting values at iterator creation. Array growth,
  contraction, accessor-index exceptions, strict arguments mutation, and
  sloppy mapped-arguments aliasing now match the covered test262 semantics.
  `Object.defineProperty(array, index, descriptor)` also advances array
  length for indexed descriptors, and deleting mapped arguments elements
  breaks the parameter alias as required.
- **`for...of` head parsing and early errors**: the parser now requires the
  `of` delimiter to be the raw keyword, allows `async` as a contextual
  identifier on assignment left-hand sides, rejects `const let` heads, and
  validates array/object assignment patterns before accepting them as
  `for...of` targets.
- **`for...of` lexical head environments**: `let`/`const` loop heads now use
  a temporary TDZ environment while evaluating the right-hand iterable and a
  fresh per-iteration lexical environment before binding each iterator value.
  Destructuring defaults and statement-body closures now capture the same
  initialized iteration binding, and `typeof` on TDZ bindings throws
  `ReferenceError` while unbound names still report `"undefined"`.
- **`for...in` lexical head environments**: `let`/`const` loop heads now mirror
  `for...of` by evaluating the right-hand object under a temporary TDZ
  environment and binding each enumerated property key inside a fresh
  per-iteration lexical environment. Destructuring defaults and loop-body
  closures now capture the initialized iteration binding, and array/object
  assignment-pattern left-hand sides are validated before accepting them as
  `for...in` targets.
- **Compile-only loop scope unwinding**: `break`/`continue` unwinding now pops
  only scopes that have a runtime environment record, so `for`/`for...in`/
  `for...of` compiler-only loop scopes no longer over-pop direct-eval
  environments when a labelled `continue` exits an inner loop. This preserves
  `UpdateEmpty` completion values for `for...in` and `for...of` labelled
  continue paths and reduces the `for...in` subset to **69 pass / 5 fail** and
  the `for...of` subset to **81 pass / 1 fail**.
- **`for...in` enumeration and descriptor preservation**: `for...in`
  enumeration now treats non-enumerable own string properties as visited so
  they shadow prototype properties, and rechecks a key's current
  enumerability before yielding so deleted not-yet-visited properties are
  skipped. `Object.defineProperty` now preserves existing descriptor fields
  that are absent from a redefinition descriptor and rejects invalid
  non-configurable/non-extensible redefinitions, keeping property order and
  enumerability intact for cases like `{ a, b }` followed by redefining `a`.
  The `for...in` subset improves to **72 pass / 2 fail**.
- **Array-index assignment through prototype setters**: writing to a missing
  array index now observes inherited accessor setters before extending the
  array, so member-expression `for...in` heads like `for ([let][1] in obj)`
  route through the same [[Set]] semantics as ordinary element assignment.
  The `for...in` subset improves to **73 pass / 1 fail**.
- **Direct eval `var` leakage from lexical `for...in` heads**: sloppy direct
  eval now copies `var`/function declarations back to the caller's variable
  environment rather than the temporary TDZ lexical environment used while
  evaluating a lexical `for...in` right-hand side. This preserves closures
  created before, during, and after the loop head expression and brings the
  `for...in` subset to **74 pass / 0 fail**.
- **Ordinary object `[[Set]]` own-property precedence**: assignment now handles
  an object's own accessor/data descriptor before consulting inherited
  setters or non-writable data properties. This lets ordinary function
  `.prototype` and `prototype.constructor` own data properties remain writable
  even when `Function.prototype`/`Object.prototype` define same-named accessors,
  reducing the `function` statement subset to **8 failures**.
- **Call frame operand-stack isolation**: each `CallFrame` now records its
  stack base, and `Pop`/`Return`/`Halt` cannot consume operands below the
  current frame. This prevents nested calls with loop-body cleanup (for
  example `out.push(f())` where `f` contains `for...in`/`for...of`) from
  corrupting the caller's method-call receiver stack.
- **BigInt exact comparison semantics**: BigInt equality and relational
  comparisons now avoid lossy `f64` conversion. BigInt-vs-Number comparison
  handles integer, fractional, `NaN`, and infinity cases separately, while
  BigInt-vs-String now implements `StringToBigInt` for empty strings and
  `0x`/`0o`/`0b` prefixes. This fixes the BigInt equality and comparison
  test262 clusters, including large literals beyond IEEE-754 precision.
- **Arbitrary-precision BigInt prefixed literals**: hex/octal/binary BigInt
  literals are parsed with `num_bigint` instead of overflowing through `i64`,
  so literals such as `0x10000000000000000n` preserve their exact value.
- **UTF-16 string comparison and iteration**: string relational comparison now
  uses UTF-16 code-unit order, and string iteration (`for...of`, destructuring,
  spread) yields Unicode code points by combining valid surrogate pairs. Lone
  surrogate escapes are accepted and preserved internally with private
  sentinels because Rust `String` cannot store surrogate code points directly.

- **for-of/for-in member expression LHS**: `for (x.y of [23])` now correctly
  evaluates the member expression as the assignment target using Swap-based
  stack reordering. Previously threw "Cannot set property of primitive".
- **for-in/for-of duplicate var names**: `for (var [x, x] in obj)` is no
  longer a SyntaxError. Duplicate names are only an error for `let`/`const`
  declarations, not `var`.
- **`delete x` (implicit global)**: `delete x` where `x` was created by an
  implicit global assignment (`x = 1` without `var`) now returns `true` and
  removes the binding, matching spec non-strict-mode behavior.

- **`with`-statement scope semantics**: `var x = expr` inside a `with`-block
  now correctly resolves the assignment target through the environment chain.
  When the `with`-object has a matching property, the assignment targets the
  with-object (not the function-scope var binding). When a `var` binding
  already exists in a closer scope (e.g. inside a function defined within
  the `with`-block), the var binding takes precedence over the with-object
  property. This fixes ~45 `with`-statement test262 failures.
- **Identifier resolution order**: `LoadEnvName`, `get_value`, and
  `put_value` now walk the environment chain in spec order — at each
  environment record, var/let/const bindings are checked before
  with-object properties. This ensures a var binding in a child scope
  shadows a with-object property on a parent scope, while a with-object
  property shadows an outer var binding.
- **Var hoisting vs initializer separation**: a new `HoistVar` opcode
  creates the hoisted `var` binding as `undefined` in the function-scope
  root without touching `with`-object properties. The `DeclareVar` opcode
  then sets the initializer value via `set_checked`, which respects the
  environment chain precedence. This prevents var hoisting from
  clobbering existing with-object properties.
- **Try/catch environment unwinding**: when a `throw` inside a `try` body
  diverts to a `catch` handler, the frame environment is now restored to
  the try-entry point, unwinding any scopes or `with`-environments opened
  inside the try body but not popped because the throw bypassed their
  `Pop*` opcodes. `catch_stack` entries now store the saved environment
  alongside the handler IP.
- **`PutValue` env-chain traversal**: `put_value` for Reference-based
  assignments now walks the environment chain from the reference's base
  env, matching spec `SetMutableBinding` semantics. A deleted with-object
  property is recreated on the closest with-object (non-strict) rather
  than falling through to an outer with-object.

- **Template-literal raw/cooked escapes**: template segments now correctly
  handle line continuations (cooked empty, raw preserves `\\` + line
  terminator), legacy octal/hex/unicode invalid escapes (SyntaxError for
  untagged templates, `undefined` cooked for tagged templates), and raw
  values for invalid escapes per spec. Nested template literals inside
  interpolations are lexed correctly via a template-context stack.
- **Switch completion values with `continue`**: when `continue` exits a
  `switch` (e.g. inside `do-while`), the current switch completion value is
  now propagated to the enclosing completion slot. This makes
  `do { switch { case: { 6; continue; } } } while (false)` evaluate to `6`
  per spec.
- **Reference type for compound assignments**: identifier compound
  assignments (`x += y`) now use spec-conforming LoadRef → GetValue →
  operate → PutValue, preserving the original binding even if deleted
  between get and put (the `with` + getter-delete pattern). Unresolved
  references always throw ReferenceError per spec.
- **Resolved Reference bases across compound-assignment RHS evaluation**:
  `LoadRef` now records the resolved declarative environment or object
  environment base before evaluating the right-hand side. Direct eval can no
  longer introduce an inner `var` binding that steals the final `PutValue`;
  the compound-assignment test262 directory now passes 406/406 locally.
  Strict object-environment references also throw ReferenceError if the
  property disappears between `GetValue` and `PutValue`.
- **Reference GC rooting and global object writes**: GC root collection now
  follows object bases stored inside `Value::Reference`, so compound
  assignment RHS evaluation cannot collect a captured object/environment
  reference. Object-environment `PutValue` also bypasses the legacy
  global-env write shortcut, preserving sloppy global object property writes
  when a getter deletes the property before the final put.
- **`with` statement completion values and early errors**: `with` now resets
  its own completion value before evaluating the body, so empty normal and
  empty abrupt completions are updated to `undefined` while expression bodies
  preserve their value. Direct eval now preserves inherited strictness through
  to the compiled chunk, making `with` inside strict direct eval throw
  SyntaxError. Expression statements beginning with `let [` are rejected even
  across a line terminator, matching the grammar lookahead restriction.
- **Compiled function table index rebasing**: code compiled after previous
  functions already exist in the VM now rebases `MakeClosure`/`MakeClass`
  indices before appending function definitions. This prevents direct eval and
  repeated `Vm::run` calls from accidentally constructing an older function
  body when the new source creates function expressions.
- **Reference type for simple identifier assignment**: `x = rhs` now creates
  the identifier Reference before evaluating `rhs`, then stores through
  `PutValue`. This preserves the originally resolved with-object property even
  if `rhs` deletes it, and prevents direct eval in `rhs` from introducing a
  nearer `var x` that steals the final store. Unresolvable identifier
  References are now represented explicitly, so sloppy `with` assignments to
  names that were absent before `rhs` still create implicit globals rather
  than new with-object properties.
- **Inherited read-only data properties in `[[Set]]`**: ordinary assignment
  now rejects writes when the prototype chain contains a non-writable data
  property. Sloppy assignment fails silently and strict assignment throws
  `TypeError`, while writable inherited data properties still allow creating
  an own property on the receiver. Object literal data properties and object
  spread now use own data-property definition instead of ordinary `[[Set]]`,
  so inherited read-only `Object.prototype` properties do not block object
  initialization; non-computed `__proto__` colon properties still update the
  literal object's prototype.
- **Keyword IdentifierNames after member `.`**: dot property access now accepts
  every keyword token covered by the shared `as_keyword_str()` helper, so
  escaped keyword property names such as `obj.st\u0061tic = 42` parse as
  `obj.static` instead of throwing a SyntaxError.
- **Assignment function-name inference**: anonymous function assignment now
  performs SetFunctionName only for a bare identifier left-hand side. Member
  targets such as `obj.attr = function() {}` and parenthesized identifier
  targets such as `(fn) = function() {}` no longer infer a name, while
  `cover = (function() {})` still infers `cover` from the bare identifier
  assignment target.
- **Native function own descriptors**: native functions now install own
  non-writable, non-enumerable, configurable `length` and `name` data
  properties when allocated. Strict writes such as `Function.length = 42`
  now throw `TypeError`, and `Object.getOwnPropertyDescriptor(Function,
  "length")` reports the expected descriptor. The assignment test262 subset
  improves to **101 pass / 9 fail**.
- **Global object property descriptors**: global bindings are now mirrored
  onto `globalThis` as own properties, with `NaN`/`Infinity`/`undefined`
  installed as non-writable, non-configurable data properties. Sloppy
  implicit globals create configurable enumerable global object properties,
  strict script top-level `this` resolves to `globalThis`, and top-level
  `var` declarations create non-configurable enumerable global object
  properties without treating `var x;` as `x = undefined`. Initializers for
  existing read-only globals such as `var NaN = 42` now respect the global
  object's non-writable descriptor instead of mutating only the environment
  binding. This removes the remaining direct assignment failures, improves
  `language/expressions/delete` to **61 pass / 5 fail**, and raises the
  supported subset to **3799 pass / 379 fail / 2 timeout**.
- **Delete reference semantics**: `delete` now evaluates non-reference
  operands before returning `true`, treats function parameter bindings as
  non-deletable mutable bindings, deletes configurable global object
  properties such as `JSON` even when mirrored by a global env binding, and
  throws `ReferenceError` for `delete super.x` / `delete super[x]` in the
  correct evaluation order. The `language/expressions/delete` subset now
  passes **66/66** run tests, and the supported subset rises to
  **3804 pass / 374 fail / 2 timeout**.
- **Update-expression Reference semantics**: prefix/postfix
  increment/decrement now evaluate the target once, preserve the original
  Reference across `GetValue` and `PutValue`, use `ToNumeric` so BigInt update
  results stay BigInt, and call computed property-key coercion only once.
  The four increment/decrement test262 directories now pass **130/130** run
  tests.
- **Object literal computed property keys**: computed data and accessor
  property names now perform `ToPropertyKey` immediately after evaluating the
  key expression, before evaluating the property value or creating the
  accessor function, while preserving Symbol keys. The
  `language/expressions/object` subset improves to
  **250 pass / 35 fail / 285 ran**, and the supported subset rises to
  **3828 pass / 350 fail / 2 timeout**.
- **Object literal method semantics**: concise methods and accessors are now
  non-constructors, ordinary concise methods no longer get an own
  `prototype` property, and object accessor methods bind `super` through the
  literal home object. `super.x` reads and `super.x = v` writes now use the
  original receiver instead of the prototype object. The
  `language/expressions/object` subset improves to
  **254 pass / 31 fail / 285 ran**, and the supported subset rises to
  **3840 pass / 338 fail / 2 timeout**.
- **Object literal `__proto__` semantics**: duplicate non-computed
  `__proto__:` prototype-mutation properties now throw a parse-time
  `SyntaxError`, while computed `["__proto__"]` and shorthand `{__proto__}`
  remain ordinary data properties. Own data `__proto__` properties now take
  precedence over the legacy prototype getter. The
  `language/expressions/object` subset improves to
  **258 pass / 27 fail / 285 ran**, and the supported subset rises to
  **3850 pass / 328 fail / 2 timeout**.
- **test262 metadata parser indentation**: the local runner now accepts
  `negative:` metadata with arbitrary YAML indentation, matching current
  test262 files such as update-expression early-error tests.
- **Null/undefined base before ToPropertyKey**: `base[key]` compound
  assignments now check for null/undefined base (ToObject) after
  evaluating the key expression but before calling ToPropertyKey,
  matching spec evaluation order. `null[throwingToString] *= x` throws
  TypeError (not the toString error). ToPropertyKey is called exactly
  once per spec (T4 series).
- **With-object own-property checks**: `with`-object binding lookups in
  get_value/put_value now use `has_own_property` (not `has_property`),
  so inherited prototype properties are not mistakenly found on the
  binding object.
- **`with` + `var` initialization**: `var foo = x` inside a `with` block
  now also sets the with-object's property when it already has one, so
  `with(o){ var foo = "set in with" }` results in `o.foo === "set in with"`.
- **Const/TDZ enforcement in put_value**: `put_value` now uses
  `set_checked` so const reassignment throws TypeError and TDZ access
  throws ReferenceError.
See [docs/test262.md](docs/test262.md#three-pass-rate-scopes) for the
three distinct pass-rate scopes and what each measures.

- **Switch lexical scope**: `switch` now creates a lexical environment
  (like a block). Function declarations, `var`, and `let`/`const` in case
  bodies are hoisted into the switch scope instead of leaking to the
  enclosing scope.
- **Catch parameter scope**: `catch (x)` now uses block scope
  (`push_scope(false)` + `PushScope`/`PopScope`) instead of function scope,
  so catch bindings are properly lexically scoped.
- **Escaped get/set**: `Token` gains `had_escape` field; escaped identifiers
  like `\u0067et` are treated as regular property names, not getter keywords.
- **For-of destructuring init errors**: `for (const [x] = 1 of [])` now
  throws `SyntaxError`.
- **For-of/for-in head-body name clash**: `for (let x of []) { var x; }`
  now throws `SyntaxError` per spec EarlyErrors.
- **Class constructor call check**: class constructors throw `TypeError`
  when called without `new`. `CallSuperCtor` sets `pending_new_target`
  so `super()` is treated as a construct call.
- **Derived constructor return override**: returning a non-object value
  from a derived constructor throws `TypeError` per spec.
- **Class accessor enumerability**: class getter/setter accessors now use
  `DefineClassAccessor` with `enumerable=false`.
- **Class prototype writability**: class constructor `.prototype` is now
  non-writable per spec.
- **Double super() check**: a second `super()` call throws `ReferenceError`.
- **Class extends validation**: `ValidateExtends` opcode checks superclass
  is a constructor with valid prototype.
- **BigInt increment/decrement**: `Inc`/`Dec` opcodes handle both Number and
  BigInt types.
- **Delete on null/undefined**: `delete null[0]` throws `TypeError`.
- **Object.prototype.toString**: returns `[object Object]` for plain objects.
- **String.prototype.toString/valueOf**: added missing methods to
  `String.prototype`.

- **Inline-cache invalidation**: `SetElem` (`o["x"] = v`) and
  `Object.defineProperty` now invalidate the monomorphic property cache
  so that subsequent `GetProp` reads the freshly written value instead of
  a stale cached value.

- **BigInt divide/modulo by zero**: `1n / 0n` and `1n % 0n` now throw a
  `RangeError` instead of returning `0n`.

- **BigInt exponent overflow**: `BigInt` exponentiation with an exponent
  that does not fit in a `u32` now throws a `RangeError` instead of
  silently clamping to zero and returning the wrong value.

- **Arrow function early errors**: arrow functions now reject
  `eval`/`arguments` parameter names and duplicate parameter names in
  both sloppy and strict mode, matching the spec's strict-mode parameter
  rules for arrows.

- **Tagged-template objects**: each template-literal site now returns a
  cached, frozen template object with a frozen `raw` property, matching
  `GetTemplateObject`. `Object.getOwnPropertyDescriptor` also returns
  descriptors for Array exotic `length` and index properties.

- **for-in/for-of non-declaration parsing**: `for (x in obj)` and
  `for ((x) in obj)` now parse and assign correctly (was: SyntaxError or
  undefined). Added `no_in` flag to prevent `in` being consumed as a binary
  operator in for-head expressions.
- **StoreGlobal stack imbalance**: function declaration hoisting left an
  `undefined` on the stack, corrupting `console.log(f())` when `f` contained
  a nested function declaration. Fixed by emitting `Pop` after each hoisted
  function declaration.
- **with-statement var semantics**: `var foo = "x"` inside `with(o)` where
  `o` has a `foo` property now sets `o.foo` per ES5 spec. Previously the
  assignment went to the function-scope root, bypassing the with-object.
- **Strict-mode eval/arguments enforcement**: `eval = 42`, `var eval`,
  `function eval()`, `function f(eval)`, and duplicate parameters in strict
  mode now throw `SyntaxError` at parse time.
- **try-finally continue/break**: `continue`/`break` inside a `finally`
  block caused an infinite loop because `finally_stack` was not popped
  before compiling the finally body, causing `DivertContinue` to loop back
  into the same finally.
- **Class declarations in single-statement position**: `if (x) class C {}`
  and similar now throw `SyntaxError` per ES6 spec.
- **for-in/for-of non-declaration left side assignment**: the iterator value
  was left on the stack and discarded instead of being assigned to the
  variable. Now uses `compile_assign_target` for proper assignment.
- **eval stack corruption**: eval ran on the shared VM stack, so `Halt`
  could pop caller values when the eval body ended via break/continue. Fixed
  by pushing a sentinel and truncating the stack after eval.
- **do-while continue target**: `continue` in `do-while` jumped to the loop
  body start instead of the condition test, causing infinite loops.
- **let/const in single-statement position**: `if (x) let y = 1;` now throws
  `SyntaxError` per ES6 spec (lexical declarations require a block).
- **Switch completion value tracking**: switch now returns the last non-empty
  expression value as its completion, matching ES spec `UpdateEmpty` semantics.
- **Assignment target validation**: invalid assignments like `x - y = 1` or
  `1 + 2 = 3` now throw `SyntaxError` at parse time. Valid targets: identifiers,
  member/element access, private field access, and destructuring patterns.
- **test262 runner**: handles `onlyStrict` flag by prepending `'use strict'`.

## [0.4.0-alpha] - 2026-07-02

### Heap Limit Enforcement Overhaul

- **`Heap::allocate` now returns `Result<usize, HeapLimitExceeded>`** with a
  `From<HeapLimitExceeded> for Arc<Error>` impl, so exceeding the limit
  produces a catchable `RangeError("heap limit exceeded")` at *every*
  allocation site — not just object literals.
- **Eliminated `allocate_unchecked` / raw `heap.allocate().unwrap_or(0)`**:
  all 59+ call sites that previously bypassed the heap limit (Array methods
  like `map`/`filter`/`slice`, `JSON.parse`, `RegExp.exec`, `Proxy`,
  `Map`/`Set` constructors, Promise allocation, generator creation, etc.)
  now propagate the error via `?`.
- **Removed sentinel `usize::MAX`** return from `Heap::allocate` — the
  previous sentinel pattern could cause index-out-of-bounds panics when
  callers forgot to check it.
- **Fixed GC-on-allocate with empty roots**: `Heap::allocate` no longer
  calls `self.collect(&[])` (which would sweep every live object). GC
  before allocation is now done by `Vm::alloc` with the correct root set
  via `self.collect_roots()`.
- **Signature changes**: `Vm::new()`, `register_fn()`, `setup()`,
  `setup_full()`, `make_builtin_constructor()`, `make_error_constructor()`,
  `make_builtin_constructor_with()`, `build_math()`, `build_json()`,
  `build_reflect()`, `build_console()`, `make_value_array()`,
  `make_str_array()`, `make_array()`, `map_entries_list()`,
  `clone_lexical_env()`, `clone_loop_vars()`, `new_env()`,
  `new_with_env()`, `new_iterator()`, `new_lazy_iterator()`,
  `new_generator_iterator()`, `make_error_value()` now return `Result`.
- **Verification tests**: added `heap_limit_enforced_in_json_parse`,
  `heap_limit_enforced_in_array_map`, `heap_limit_enforced_in_regexp`
  to `tests/fuel.rs`, confirming the limit is enforced through builtin
  code paths that were previously bypassable.

### Security / Hardening
- **Generator resume panic**: `resume_generator` used `frames.pop().expect(...)`, which
  would abort the process if a generator frame was missing. Converted to an
  internal `Error` so the VM reports a catchable runtime error instead of panicking.
- **Number radix formatting panic**: `biguint_to_radix` used `String::from_utf8(...).unwrap()`
  on an ASCII-only digit buffer. Replaced with `unwrap_or_default()` to remove the
  unconditional panic path.
- **Direct `args[idx]` indexing**: Replaced the remaining direct `args[0]`/`args[1]`
  accesses in `src/builtins.rs` with safe `get()`/`first()` fallbacks. All call sites
  were already guarded by length checks, but the new form removes any latent panic
  path if a builtin is invoked with fewer arguments through meta-programming.
- **VM invariant unwraps**: Added an empty-frame guard at the top of the
  `interpret_inner_raw` loop and converted the two `finally_stack.last().unwrap()`
  paths in throw/finally diversion to `ok_or_else` propagation. The remaining
  `frames.last().unwrap()` calls are loop-invariant and will be hardened during
  the `vm.rs` module split.
- **Lock poisoning panic**: Replaced `std::sync::Mutex` with `parking_lot::Mutex`
  throughout the engine. `parking_lot::lock()` is panic-free, removing ~200
  latent `lock().unwrap()` panic paths (the remaining unwraps are on `Option`/
  `Result`/`Vec` operations, not on mutex acquisition).
- **cargo-fuzz target**: Added `fuzz/fuzz_targets/fuzz_target_1.rs` exercising the
  public `Vm::run` API with fuel-capped execution. Initial 30-second run completed
  over 50,000 iterations without triggering a panic.
- **Module split**: `src/builtins.rs` (7,000+ lines) was split into
  `src/builtins/{mod,math,json,global,array,string,collections,regexp,function}.rs`.
  `src/vm.rs` was split into `src/vm/{mod,ops}.rs`, with the main opcode dispatch
  loop and helpers moved to `ops.rs`.

### Performance
- **Map/Set O(1) lookups**: Replaced `Vec`-backed linear scans with
  `IndexMap`/`IndexSet` using a `MapKey(Value)` wrapper that implements
  `Hash`/`Eq` via SameValueZero semantics (NaN == NaN, -0 == +0).
- **UTF-16 ASCII fast-path**: `utf16_len`, `utf16_get`, `utf16_slice` now
  check `is_ascii()` first, skipping `encode_utf16().count()` for the
  common case where byte length equals UTF-16 length.
- **Monomorphic inline cache**: `GetProp` caches `(heap_idx, key)` -> `Value`
  to skip the prototype-chain walk on repeated property reads. Cache is
  invalidated on `SetProp` to prevent stale reads. Capped at 4096 entries.

### Features
- **Proxy**: `new Proxy(target, handler)` with `get`/`set` traps and
  `Proxy.revocable()` returning `{ proxy, revoke }`. Revoked proxies throw
  `TypeError` on trap invocation.
- **TypedArray (Uint8Array)**: `new Uint8Array(length)` or
  `new Uint8Array(arrayLike)` with index access, `length`, `byteLength`,
  and `byteOffset` properties. `TypedArrayKind` enum defined for all 8
  typed array element kinds.
- **toJSON support**: `JSON.stringify` now calls `toJSON()` on objects
  that define it before serializing, matching ES spec behavior.
- **UTF-16 correctness**: `str_includes`, `str_split` (empty separator),
  and `str_replace_all` (empty pattern) now use UTF-16 code-unit iteration
  instead of Rust `chars()`.
- **RegExp lastIndex UTF-16**: `RegExp.exec` bounds check and match offset
  calculation now use UTF-16 code-unit indices, preventing panics on
  supplementary characters.
- **VM invariant unwrap removal**: All 50 `frames.last().unwrap()` calls
  in `vm/ops.rs` and `vm/mod.rs` converted to `current_frame()?` /
  `current_frame_mut()?` safe propagation. Zero `unwrap()` calls remain
  in those files.
- **Incremental GC**: `Heap::collect_incremental(roots, budget)` marks up
  to `budget` cells per call, allowing the VM to avoid long GC pauses.
- **Async tick API**: `Vm::tick()` executes a single microtask and returns,
  enabling cooperative event-loop scheduling by hosts.

### Documentation
- Added `docs/audit-panics.md` documenting the `unwrap()`/`expect()` inventory in
  `src/vm.rs` and `src/builtins.rs`, reachability policy, and remaining work.

## [0.3.0-alpha] - 2026-07-01

### Added
- **Execution fuel / interrupt**: `Vm::set_fuel(Some(n))` bounds execution
  to ~n opcodes; exhaustion throws `RangeError("fuel exhausted")` that is
  **not catchable** by user `try/catch` (a host-level abort), so untrusted
  code cannot swallow it and keep looping. `None` (default) is unbounded.
  Cooperative, checked before each opcode.
- `Map`/`Set`/`Array.includes` keys now compare by **SameValueZero**
  (`NaN === NaN`, `-0 === +0`), so `new Map().set(NaN,1).get(NaN)` returns 1.

- **Full test262 CI**: `.github/workflows/test262-full.yml` runs the entire
  test262 suite across directory-split parallel jobs and aggregates results
  into the GitHub Actions summary. `intl402`/`staging` are excluded;
  unsupported-feature tests are skipped via an expanded `SKIP_FEATURES` set.
  Baseline: 76,397 tests, 60,178 run, 19,987 pass (33.2%).

### Security
- **Array-index DoS (OOM)**: `a[0x80000000]` used to materialize ~2B dense
  slots and OOM-kill the host. Now only `0..2^32-1` are array indices (ES
  spec); valid indices beyond the dense cap are stored sparsely, so
  `a[0x80000000]` returns the value and advances `length` without holes.
- **`String.prototype.repeat` panic**: `repeat(Infinity)` panicked with
  a capacity overflow; `repeat(-1)` returned `""`. Now validates the count
  (non-negative integer, 256 MiB result cap) and throws `RangeError`.
- **`padStart`/`padEnd` hang**: `padStart(Infinity)` hung the engine in an
  unbounded fill loop. Now clamps negatives to 0 and throws `RangeError`
  on `Infinity`/absurd lengths.
- **`JSON.parse` / `JSON.stringify` stack overflow**: deeply nested input
  (e.g. `"[" * 100000`) aborted the process via native-stack overflow in
  `parse_json_value` / `stringify_value` / `has_json_cycle`. All three now
  take a depth parameter capped at 256 and throw/return instead of crashing.
- **`Array.from` DoS**: `Array.from({length: 2**26})` materialized 64M dense
  slots and hung. Now capped at 4M with a `RangeError`.
- **Prototype-cycle DoS**: `a.__proto__=b` (where b's chain contained a)
  created a cycle; a later property read overflowed the native stack and
  aborted the process. Cyclic `__proto__` assignments now throw `TypeError`
  in strict mode / no-op in sloppy mode, and `get_property_rx` carries a
  depth cap as a backstop.
- **`Array.prototype.sort` DoS (O(n^2))**: `a.sort(cmp)` used an inline
  O(n^2) insertion sort; sorting 10k random elements took ~30s and called
  the comparator ~250k times. Now uses a stable merge sort (O(n log n));
  comparator calls dropped to ~9k for 1k elements. NaN/non-number
  comparator results are treated as 0 (equal); thrown errors propagate.

### Fixed (conformance)
- **`Date` TimeValue range**: `new Date(1e20).getTime()` returned the raw
  number instead of `NaN`. ES TimeValue must be within +/-8.64e15 ms;
  out-of-range/NaN/Infinity are now Invalid Date, matching V8/Node.
- **`Number.prototype.toString(radix)` fractional**: `(1.5).toString(2)`
  returned `"1.5"` instead of `"1.1"`. Now converts both the integer and
  fractional parts in the requested radix (common cases match V8/Node;
  minimal shortest-round-trip representation is still longer).
- **`String.prototype.charAt` range**: `charAt(-1)` returned `"a"` instead
  of `""` (Rust `as usize` saturates negatives to 0). Now uses `ToInteger`
  with an explicit range check, matching V8/Node.
- **`ToInt32`/`ToUint32`**: bitwise ops coerced with Rust's `as i32`/`as
  u32`, which saturate large values to `INT32_MAX`/`UINT32_MAX`. Now uses
  modular reduction (`(2**31)|0` -> `-2147483648`, `(2**32)|0` -> `0`).
- **`charCodeAt`/`codePointAt`**: negative/out-of-range indices returned
  the index-0 value instead of `NaN`/`undefined` (Rust `as usize` saturates
  negatives to 0). Now uses `ToInteger` with explicit range checks.
- **`String.prototype.split` limit**: negatives returned `[]` instead of
  all parts; `NaN` returned all parts instead of `[]`. Now `NaN` -> 0,
  negative/infinite -> unbounded, otherwise trunc toward zero.
- **`Number.prototype.toFixed`**: `toFixed(-1)` returned `"1"`, `toFixed(200)`
  produced a 201-digit string. Now validates `0..=100` and throws
  `RangeError`, matching V8/Node.
- **`Number.prototype.toPrecision`**: `toPrecision(0/-1/101)` produced wrong
  output instead of `RangeError`. Now validates `1..=100`.
- **`Object.defineProperty`**: a non-object descriptor (e.g. `true`) was
  silently accepted. Now throws `TypeError` per `ToPropertyDescriptor`.


### Fixed
- `gc::live_count` now locks `free_list` before `cells` to match
  `allocate()`, removing a lock-order inversion deadlock.
- GC alloc counter uses `fetch_add` instead of a racy load+store.
- Removed the global `#![allow(unreachable_patterns)]`; a duplicate lexer
  arm and a shadowed bool/bigint loose-eq arm were real dead code and are
  gone, remaining intentional fallbacks carry a local `#[allow]`.

### Changed
- Documented that `pub` internal modules are not a semver-stable API
  (embed against the re-exports), Map/Set are O(n) `Vec`-backed, and
  `with_obj` is non-reentrant on the same index. test262 numbers clarified
  as a curated subset, not full conformance.
- **Unicode identifiers & escapes**: `IdentifierStart`/`IdentifierContinue`
  now accept Unicode letters and the `\uXXXX` / `\u{XXXX}` escape forms
  inside identifiers (`\u{63}ase` parses as `case`; `café`/`π`/CJK names
  lex correctly). NEL/LS/PS are recognized as line terminators. Invalid
  escapes and non-id Unicode bytes advance the cursor instead of looping.
- **Destructuring parameters**: arrow functions and ordinary functions
  accept destructuring params (`([a, b]) =>`, `function f({x, y})`),
  including nested patterns and defaults (`[[x, y, z] = [4, 5, 6]]) =>`).
  Each destructuring param binds from a synthesized positional temp.
- **Object-literal methods**: generator methods (`*foo() {}`) and async
  methods (`async foo() {}`, `async *foo() {}`) now parse; reserved words
  (`return`, `class`, `default`, ...) are accepted as property keys.
- **Sloppy-mode `this`**: top-level `this` in non-strict script binds to
  the global object.
- **test262 negative-test handling**: the runner parses `negative: { phase,
  type }` metadata so a test that expects a `SyntaxError`/`TypeError`
  passes when RuJa raises the matching error; tests run via a temp file
  instead of `-e` argv so long sources and non-ASCII survive intact.
- **test262 subset pass rate**: raised from ~20% to ~67% on a
  representative `language/` subset (arrow-function 35%→69%, function
  16%→57%, object 26%→69%, identifiers 28%→59%).
- **test262 harness**: the runner now loads the real test262 harness files
  (`assert.js`, `sta.js`, and per-test `includes:` like `propertyHelper.js`,
  `compareArray.js`) instead of a hand-rolled stub. This makes pass/fail
  accurate (the stub was too lenient, e.g. `-0` vs `+0`). Pass rate is now
  measured against the real conformance assertions: 20.1% (was 28.3% under
  the lenient stub — the drop is correctness, not regression).
- **`Function.prototype.toString`**: returns `function name() { [native code] }`
  for native functions and `function name() { ... }` for interpreted ones.
  This fixes function-to-primitive coercion (`fn + 1`) which previously threw
  because the function had no `toString`.
- **Boxed primitives store their value**: `new Number(5)`, `new Boolean(true)`,
  `new String("x")`, and `Object(x)` now keep the wrapped primitive on the
  object, so `.valueOf()` returns it and `ToPrimitive` resolves to it
  (`new Number(5) + 1 === 6`). Previously wrappers were empty objects.
- **`ToPrimitive` throws on unconvertible objects**: when both `valueOf` and
  `toString` return objects, OrdinaryToPrimitive now throws `TypeError` per
  spec (was: silently fell back to a string form).
- **`Object(1n) + 1` throws `TypeError`**: BigInt-wrapper arithmetic now
  applies the BigInt/Number mixing rule after ToPrimitive unwraps the box.
- **Vertical tab / form feed are whitespace**: the lexer now treats `\x0b`
  and `\x0c` as whitespace, fixing a class of test262 parse failures.
- test262 expressions pass rate: 28.3% -> 31.9% (2476 -> 2790 passing).
- **`Vm` is now `Send`**: the engine migrated from `Rc`/`RefCell`/`Cell`
  to `Arc`/`Mutex`/atomics for shared ownership and interior mutability.
  A `Vm` can be moved between threads; concurrent shared access still needs
  external synchronization (e.g. `Mutex<Vm>`). The GC trace loop is now
  worklist-based to avoid re-entrant locking of the cells mutex (which
  would deadlock under `Mutex`). `with_obj` takes the object out of its
  cell during the callback so the cells mutex is never held across a
  user/allocation callback.
now run the `finally` body before completing the transfer (single-level).
- **Private class fields** (`#field = init`): isolated per-instance storage
  via `GetPrivate`/`SetPrivate` opcodes; not enumerable or in `Object.keys`.
 a known limitation).
- **Sloppy-mode `this`**: plain function calls now bind `this` to `globalThis`
  in non-strict mode (strict mode stays `undefined`).
- **`new C(...spread)`**: constructor calls with spread arguments via a new
  `NewSpread` opcode.
- **Tagged template literals**: `tag`q0${e0}q1`` calls `tag(strings, e0)`
  with a `strings.raw` array.
- **Async arrow functions**: `async () => ...`, `async (a,b) => ...`,
  `async x => ...`.
- **JSON.stringify** replacer (array whitelist / function) and space
  (indentation); **JSON.parse** reviver (bottom-up transform).
- **String.replace** with a function callback (match, captures, offset,
  string); **String.split** with a RegExp separator.
- **Reflect** global: get/set/has/deleteProperty/ownKeys/getPrototypeOf/
  setPrototypeOf/isExtensible/preventExtensions/apply/construct.
- **WeakMap**/`WeakSet` globals (API-compatible; entries are strong-ref).
- **Date** global (minimal): `Date.now()`, constructor, `getTime()`.

### Added (round 2)
- **Static initialization blocks executed**: `static { }` now runs with
  `this` = the constructor in source order (was parsed-but-ignored). Fixed
  the `CallThis` stack ordering and a `StoreEnv` undefined leak that left
  the constructor off the top of the stack.
- **Private class methods** (`#method() {}`): called via `this.#method(...)`;
  private method calls use a new `CallPrivateMethod` opcode so `this` binds
  to the receiver. Private field `++`/`--` also works.
- **BigInt literals**: `123n`, `0xffn`, `0o17n`, `0b101n` with exact
  arithmetic (`+ - * / % **`), comparison, `===`/`==` (BigInt vs Number is
  `false` for `===`, numeric for `==`); mixing throws `TypeError`. `BigInt()`
  constructor and `BigInt.prototype.toString` supported.
- **Nested try/finally**: non-local transfers (`return`/`throw`/`break`/
  `continue`) now run **all** enclosing `finally` blocks innermost-first
  (was: only the innermost for break/continue). Guard ordering is tracked
  with push-sequence numbers so a throw runs a finally nested inside the
  nearest catch before reaching the catch; a `return`/`throw` inside a
  `finally` overrides the pending completion.


### Added
- **Object spread** `{...a, y:2}` copies enumerable own properties via a new
  `ObjSpread` opcode.
- **Object rest destructuring** `{a, ...r} = obj` collects remaining own
  enumerable properties via a new `ObjRest(n)` opcode; `Pattern::Object` now
  carries an optional rest field.
- **Getters/setters** in object literals (`get x() {}` / `set x(v) {}`) and
  class methods (static + instance) via a new `DefineAccessor` opcode.
  Inherited accessors bind `this` to the receiver (`get_property_rx`).
- **`new.target`** meta-property via a new `NewTarget` opcode; `Construct`
  sets `pending_new_target` on the pushed frame.
- **`for(;;)`** with any combination of empty init/condition/update.
- **Numeric separators** (`1_000`, `0xff_ff`, `0b1010_1010`, `3.14_15`).
- **`globalThis`** routes property get/set to the global environment record;
  rooted in `collect_roots` to survive GC.
- **`__proto__`** accessor: get returns `[[Prototype]]`, set updates it.
- **Object statics**: `getPrototypeOf`/`setPrototypeOf`,
  `preventExtensions`/`isExtensible`, `seal`/`isSealed`/`isFrozen`,
  `getOwnPropertyDescriptors`, `defineProperties`.
- **Array**: `reduceRight`, `toReversed`, `toSorted`, `toSpliced`, `with`.
- **String**: `codePointAt`, `concat`, `search`, `String.raw`,
  `String.fromCodePoint`.
- **Number**: `toPrecision`, `toExponential`.
- **Math**: `imul`.
- **`console.log`** now formats arrays as `[ 1, 2, 3 ]` and objects as
  `{ a: 1 }` (Node.js inspect-style) instead of bare `toString`.

### Fixed
- **Labeled block break**: `lab:{r=1; break lab; r=2;}` previously returned
  `2` because `StmtNode::Block` never received a labeled frame. Block now
  takes the non-loop labeled-statement branch that pushes a break-only frame.
- **`to_number` on objects** now runs `ToPrimitive` (valueOf then toString)
  instead of returning `NaN`, so `+{valueOf(){return 7}}` yields `7` and
  `1 + [1]` yields `11`.


## [0.2.1-alpha] - 2026-06-28

### Fixed
- **GC root safety**: `collect_roots` now roots the microtask queue (Promise
  handlers, resolve/reject values), `generator_proto`, and `global_constants`,
  all of which were previously missing. A new `gc_pins` stack lets call paths
  pin heap values held in Rust locals (Promise handler, call args, derived
  promise) across allocations. Per-instruction GC was unsafe (it could free
  values held in Rust locals); it now runs at safe points only (after `run()`
  settles all frames, and throttled at frame boundaries). Fixes use-after-free
  panics under heavy allocation + Promise chains.
- **Runtime error source lines**: errors now report their source line, e.g.
  `ReferenceError: undefinedVar is not defined (at line 3)`. Previously every
  error reported `(at line 0)` because the compiler emitted all ops with line
  0 and the AST carried no line info. `Stmt` now carries a `line` (set by the
  parser at statement start), the compiler tracks `current_line` and flows it
  into every `Op`, and `Chunk::line_for_ip` resolves it.
- **Unimplemented Op panic**: the dispatch fallthrough arm now panics with
  the offending op (Op derives Debug) instead of silently skipping, so
  compiler bugs surface immediately.
- **`run()` test helper**: the shared test helper now panics on runtime error
  instead of returning `Value::Undefined`, so a test can no longer silently
 pass on a thrown error. Tests that genuinely expect an error use `run_err`.
- **Call-stack depth limit**: unbounded JS recursion now throws a catchable
  `RangeError: Maximum call stack size exceeded` instead of overflowing the
  Rust thread stack and aborting the process with `SIGSEGV`. The engine caps
  the interpreted call depth, and the `ruja` binary runs execution on a
  64 MiB worker thread so the limit can be generous.
- **`writable: false` honored by ordinary assignment**: writing to a
  non-writable own data property now fails per ES `[[Set]]` — throwing a
  `TypeError` in strict mode and failing silently in non-strict mode —
  instead of always overwriting the value.
- **Accessor (getter/setter) descriptors**: `Object.defineProperty` now
  reads `get`/`set` from the descriptor (rejecting a get+value or set+value
  mix with a TypeError), and `get_property`/`set_property` invoke the
  accessor. Inherited setters up the prototype chain are honored on write.
- **`Array.length` validation**: assigning a fractional, negative,
  non-numeric, or out-of-`uint32`-range value to an array's `length` now
  throws `RangeError: Invalid array length` (matching V8) instead of silently
  truncating via `as usize` or attempting an enormous allocation.
- **`num_to_string` exponential precision**: `String(n)` for values rendered
  in exponential notation (e.g. `5e-17`, `9e-17`, `9.99e-7`) is now exact,
  using Rust's `{:e}` formatting. Previously `n / 10f64.powi(exp)` introduced
  floating-point error (`5e-17` -> `4.999999999999999e-17`) and the exponent
  could be padded (`e-07` instead of `e-7`). The mantissa is now
  normalized (trailing zeros and a dangling `.` stripped) and the exponent
  digits are stripped of leading zeros, so output stays correct regardless
  of how the formatter rounds a given value.
- **`String()`/`Number()`/`Boolean()` as functions return primitives**:
  previously these routed through the generic `Object` constructor and
  returned `[object Object]` for every input. They now use dedicated
  constructors: `String(x)` returns the ToString coercion (`String()` is `""`),
  `Number(x)` returns the ToNumber coercion (`Number()` is `0`,
  `Number(undefined)` is `NaN`), and `Boolean(x)` returns the ToBoolean
  coercion. `new String/Number/Boolean(x)` still constructs an object with the
  correct prototype (RuJa does not model wrapper-object internal slots, so the
  primitive is not stored, but `typeof new String(5)` is now `"object"`).
- **Deeply-nested expression DoS**: untrusted input with deeply-nested
  expressions (e.g. thousands of nested parens) previously overflowed the Rust
  parser stack and aborted the process. The parser now caps expression nesting
  depth and throws a SyntaxError instead.
- **`Array()` constructor**: `Array(n)` / `new Array(n)` (single numeric arg)
  and `Array(a, b, c)` now create real arrays. Previously the generic
  `object_constructor` was wired in, returning `[object Object]` for every
  input. Invalid lengths (negative, fractional, out of `uint32` range) throw
  `RangeError: Invalid array length`.
- **`delete` respects `configurable`**: `delete o.x` on a non-configurable
  own property now returns `false` (or throws a TypeError in strict mode)
  instead of forcibly removing it.
- **`ToPrimitive` honors `valueOf`/`toString`**: object-to-primitive coercion
  (used by `+`, comparison, etc.) now calls the object's `valueOf` then
  `toString` (or vice-versa for the string hint). Arrays join correctly
  (`[1,2] + [3,4]` is `"1,23,4"`); a custom `valueOf`/`toString` is honored.
- **Labeled statements**: `label: stmt`, `break label`, and `continue label`
  now parse and compile (for `while`/`for`/`do...while`). A `break label`
  exits the matching outer loop; `continue label` resumes it.
- **`try/finally` non-local transfers**: a `return` or `throw` in a
  `try` (or `catch`) is now suspended across the `finally` block and re-raised
  afterward, so a `return` inside `finally` correctly overrides an earlier
  completion. (`break`/`continue` in `try`/`catch` still bypass `finally`.)

### Changed
- **README `Known limitations`** rewritten to reflect the implemented state
  (for-await, strict mode, eval isolation, array-destructuring iterator
  protocol, Function constructor are done) and list only the genuine remaining
  limits.
- **`interpret_inner` refactor**: the largest call/closure-related Op
  handlers (`op_call`, `op_call_method`, `op_call_method_opt`,
  `op_call_spread`, `op_new`, `op_await`, `op_make_closure`) extracted into
  dedicated methods, shrinking the dispatch loop from 1366 to ~1216 lines.

## [0.2.0-alpha] - 2026-06-28

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
