# Changelog

## [Unreleased]

### Test tooling

- `tools/test262_runner.py` and `tools/test262_analyze.py` now honor the
  test262 `raw` flag by running those files without harness/include prelude
  injection, so directive-prologue parse-negative tests are evaluated from
  their real source start.
- `tools/test262_analyze.py` now mirrors the runner's handling of
  `onlyStrict` tests and indented `negative:` metadata, so strict-mode and
  parse-negative test262 files are no longer reported as false failure
  buckets during focused analysis.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  Symbol-key function-name inference tests without unskipping broader Symbol
  iterator coverage, keeping the unsupported-feature boundary narrow.
- `tools/test262_runner.py` now admits the Symbol-backed object-spread
  generated tests in array/call/new expression contexts without unskipping
  broader Symbol iterator coverage.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit
  `Symbol.iterator`-tagged `language/statements/for-of/` tests without
  unskipping broader Symbol iterator coverage.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `built-ins/TypedArrayConstructors/` coverage by lifting only the
  TypedArray, concrete TypedArray constructor, ArrayBuffer, DataView, Reflect,
  Proxy, Symbol, well-known Symbol, and generator feature gates needed by that
  path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `built-ins/DataView/` coverage by lifting only the DataView,
  ArrayBuffer, Float16Array, Reflect, and typed-array helper feature gates
  needed by that path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `built-ins/Error/prototype/stack/` coverage by lifting only the
  Error stack accessor, Proxy, Reflect, and `Reflect.construct` feature gates
  needed by that path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `language/statements/with/` Proxy/Reflect coverage by lifting only
  those feature gates on the `with` statement path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the five
  implemented `error-cause` files for `Error`, NativeError, and `AggregateError`
  without unskipping broader AggregateError coverage.
- `tools/test262_analyze.py` now mirrors the runner's unsupported-feature
  boundary exactly, and `tools/analyze_failures.py` passes test paths into
  `should_skip()`, so path-scoped feature exceptions are not hidden during
  focused diagnostics.

### Runtime hardening

- The VM call-stack guard now trips at a more conservative depth before
  recursive interpreted calls can exhaust smaller debug/CI thread stacks,
  preserving the catchable `RangeError` behavior for runaway recursion.

### test262 conformance improvements

Supported-subset pass rate: **100.0%** (up from 88.6%).
Current supported subset count: **5106 pass / 0 fail / 15332 skip / 0 timeout**.

- **`with` Proxy/Reflect runner admission**: the
  `language/statements/with/` path exception now admits already-implemented
  Proxy/Reflect object-environment binding coverage without unskipping broader
  Proxy/Reflect tests. The `with` path reports **176 pass / 0 fail / 5 skip**,
  and the broader Reference-adjacent cluster reports **842 pass / 0 fail / 356
  skip**.
- **Error `cause` semantics**: `Error` and NativeError constructors now perform
  observable `HasProperty(options, "cause")` followed by `Get`, after installing
  the message property. `AggregateError` now uses its `(errors, message,
  options)` signature, reports `length === 2`, creates a non-enumerable `errors`
  array, and shares the same `InstallErrorCause` path. The five `error-cause`
  test262 files now pass; `built-ins/Error` reports **86 pass / 0 fail / 7
  skip**, `built-ins/NativeErrors` reports **81 pass / 0 fail / 13 skip**, and
  `built-ins/AggregateError` admits its cause-property file at **1 pass / 0 fail
  / 24 skip**.
- **Error stack accessor runner admission**: the
  `built-ins/Error/prototype/stack/` path exception now admits the
  already-implemented stack accessor coverage, including Proxy receivers and
  `Reflect.construct` constructor checks. The normal `built-ins/Error` runner
  now reports **83 pass / 0 fail / 10 skip**.
- **ArrayBuffer `@@toStringTag` descriptor**: `%ArrayBuffer.prototype%` now
  exposes a non-writable, non-enumerable, configurable
  `Symbol.toStringTag` data property with value `"ArrayBuffer"`. The focused
  `built-ins/ArrayBuffer` run closes at **92 pass / 0 fail / 129 skip**.
- **TypedArray generator object-argument admission**: the
  `built-ins/TypedArrayConstructors/` path exception now admits generator
  metadata for the already-supported iterable constructor path, covering
  generator abrupt completion during `IterableToArrayLike`. The normal runner
  now reports **674 pass / 0 fail / 64 skip** on that path.
- **Mapped arguments object index writes**: property-Reference writes to sloppy
  mapped arguments objects now update the linked parameter binding, including
  writes after `Object.defineProperty(arguments, "0", ...)`. Dense arguments
  indices are also treated as own data properties during `[[Set]]`, so
  prototype numeric setters no longer intercept writes to `arguments[0]`. The
  focused `language/arguments-object` test262 run now reports **126 pass / 0
  fail / 137 skip**.
- **Object destructuring `RequireObjectCoercible`**: empty object assignment
  patterns such as `({} = null)` and rest-only object assignment patterns now
  throw `TypeError` for nullish sources instead of completing silently. With
  only the `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run improves to **249 pass / 11 fail
  / 108 skip** while the default supported subset remains green.
- **Array rest assignment pattern early errors**: assignment destructuring now
  rejects array rest elements followed by another element, elision, another
  rest element, a trailing comma, or an initializer. With only the
  `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run improves to **254 pass / 6 fail /
  108 skip** while the default supported subset remains green.
- **Object shorthand destructuring default function names**: object assignment
  shorthand defaults such as `{ fn = function() {} } = source` now apply
  `SetFunctionName` when the default initializer is an anonymous function,
  arrow function, class, or parenthesized anonymous function. With only the
  `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run improves to **258 pass / 2 fail /
  108 skip** while the default supported subset remains green.
- **Array cover grammar for nested object assignment defaults**: array literals
  that may become assignment patterns now defer nested object-literal shorthand
  initializer early errors until the outer assignment decision is known. This
  lets sloppy nested object defaults such as `[{ x = yield }] = vals` and
  `[...{ x = yield }] = vals` treat `yield` as an identifier in assignment
  patterns while ordinary array literals still reject `{x = ...}`. With only
  the `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run now closes at **260 pass / 0 fail
  / 108 skip**.
- **Binding destructuring default function names**: declaration and `for`
  binding patterns now apply `SetFunctionName` when a direct binding
  identifier's default initializer is an anonymous function, arrow function,
  class, or parenthesized anonymous function. With only the
  `destructuring-binding` skip lifted for diagnostics,
  `language/statements/{variable,let,const,for}/dstr` now closes at **412 pass
  / 0 fail / 156 skip** while the default supported subset remains green.
- **For-in/of array rest assignment-pattern early errors**: non-declaration
  `for` heads now reject array assignment patterns where a rest element is
  followed by a comma or elision before `in`/`of`, matching the ordinary
  assignment-pattern early error. With only the `destructuring-binding` skip
  lifted for diagnostics, `language/statements/for-in/dstr` now closes at
  **27 pass / 0 fail / 6 skip**, and `language/statements/for-of/dstr`
  improves to **417 pass / 32 fail / 120 skip** while the default supported
  subset remains green.
- **For-of assignment-pattern cover defaults**: non-declaration `for-of`
  heads now keep object shorthand defaults and nested object defaults in cover
  grammar until the `of` decision is known, so assignment patterns such as
  `for ({ x = 1 } of values)` and `for ([{ x = yield }] of values)` parse and
  execute through the destructuring assignment path. With only the
  `destructuring-binding` skip lifted for diagnostics,
  `language/statements/for-of/dstr` improves to **433 pass / 16 fail / 120
  skip** while the default supported subset remains green.
- **For-of assignment-pattern `in` initializers**: non-declaration `for-of`
  heads now distinguish a top-level `of` delimiter before parsing the left
  side, allowing `in` expressions inside array, shorthand object, and renamed
  object default initializers such as `for ([x = "x" in obj] of values)`.
  With only the `destructuring-binding` skip lifted for diagnostics,
  `language/statements/for-of/dstr` improves to **436 pass / 13 fail / 120
  skip** while the default supported subset remains green.
- **For-of destructuring IteratorClose semantics**: array binding patterns now
  close non-exhausted inner iterators on normal and non-iterator abrupt
  completion, array binding initialization now observes a missing
  `Array.prototype[Symbol.iterator]`, and non-declaration `for-of`
  assignment heads no longer close an inner iterator when that iterator's own
  step throws. With only the `destructuring-binding` skip lifted for
  diagnostics, `language/statements/for-of/dstr` now closes at **449 pass / 0
  fail / 120 skip** while the default supported subset remains green.
- **Property Reference records for simple member assignment**: ordinary member
  assignment now lowers final writes through an explicit property Reference
  while preserving simple-assignment ordering. `obj[x] = rhs` still evaluates
  the RHS before nullish-base failure and delays `ToPropertyKey` until after
  the RHS, while `PutValue` now handles the final write with Symbol keys, Proxy
  receiver identity, strict failed-write behavior, and primitive sloppy no-op
  semantics intact. The focused assignment/member-expression test262 cluster
  reports **204 pass / 0 fail / 282 skip**.
- **Property Reference records for destructuring member targets**:
  destructuring assignment targets such as `({ a: obj[key] } = rhs)` now finish
  member writes through `MakePropertyRefForSet` and `PutValue` instead of the
  legacy `SetElem`/`SetProp` opcodes. This preserves Symbol keys produced by
  `@@toPrimitive`, Proxy receiver identity, strict failed-write behavior, and
  primitive sloppy no-op semantics for member targets nested inside object and
  array assignment patterns. The focused `language/expressions/assignment/dstr`
  run remains closed at **90 pass / 0 fail / 278 skip**.
- **Property Reference records for `for-in`/`for-of` member heads**:
  non-declaration loop heads such as `for (obj[key] in source)` and
  `for (obj[key] of iterable)` now store through the property Reference
  `PutValue` path instead of direct `SetElem`/`SetProp` opcodes. This preserves
  Symbol keys and Proxy `set` receiver identity for loop-head assignment. The
  focused `language/statements/{for-in,for-of}` run reports **191 pass / 0 fail
  / 675 skip**.
- **Object rest destructuring assignment targets**: object rest assignment
  patterns such as `({ a, ...holder.rest } = rhs)` now validate the actual rest
  target and compile it through the destructuring assignment target path. Rest
  targets can now be identifiers or member expressions, including Proxy-backed
  member targets that rely on `PutValue` receiver identity. The focused
  `language/expressions/assignment/dstr` run remains closed at **90 pass / 0
  fail / 278 skip**.
- **Object rest computed-key exclusion**: object rest destructuring now excludes
  computed property names from the rest object after `ToPropertyKey`, including
  Symbol keys, and copies remaining enumerable Symbol properties into the rest
  result. This aligns `({ [key]: v, ...rest } = rhs)` and declaration patterns
  with ordinary object rest semantics. The focused
  `language/expressions/{assignment,destructuring}` run remains closed at
  **203 pass / 0 fail / 282 skip**.
- **Class field anonymous function names**: public and private class field
  initializers now apply `SetFunctionName` to anonymous function, arrow, and
  class values before defining the field. This gives static and instance field
  initializers names such as `"field"` and `"#field"` without changing
  non-anonymous initializer values. With class field skips temporarily lifted,
  `static-field-anonymous-function-name.js` now reports **2 pass / 0 fail / 0
  skip** across declaration and expression forms; the broader class-elements
  diagnostic now reports **1559 pass / 105 fail / 1298 skip** after the
  subsequent field early-error fixes.
- **Class field `ContainsArguments` early errors**: public and private class
  field initializers now reject `arguments` during parsing, including lexical
  occurrences inside arrows while preserving ordinary function-expression
  boundaries. With public/private class field and arrow gates temporarily
  lifted, the generated `*init-err-contains-arguments.js` class-elements
  cluster reports **60 pass / 0 fail / 0 skip**.
- **Class field `constructor` PropName early errors**: public instance and
  static fields whose non-computed literal name is `constructor` now fail during
  parsing, while computed `["constructor"]` fields remain valid and define data
  properties. With public/static class field and computed-name gates temporarily
  lifted, the focused constructor-PropName class-elements cluster reports
  **11 pass / 0 fail / 0 skip**.
- **Public class field computed-name records**: public instance and static
  field computed names now evaluate once during class definition and are stored
  as field keys for later `DefineField` execution. Instance field keys are no
  longer re-evaluated for each `new C()`, and static/instance public field keys
  now evaluate in field declaration order while initializers keep their existing
  static-vs-instance timing. With public/static class field and computed-name
  gates temporarily lifted, the focused incremental/intercalated/error
  computed-name class-elements cluster reports **12 pass / 0 fail / 0 skip**,
  and the broader class-elements diagnostic reports **1582 pass / 82 fail /
  1298 skip**. Remaining failures include full ordered class-element evaluation
  across computed methods and static blocks.
- **Ordered class element evaluation**: class parsing now preserves source
  order across methods, fields, and static blocks, and class compilation uses
  that order for public computed method names, public field computed names, and
  static element initialization. This fixes field-before-method computed-name
  ordering, allows computed method names to read the initialized inner class
  binding, and runs static block bodies and static field initializers in source
  order after all element names have been evaluated. With public/static class
  field and computed-name gates temporarily lifted, the generated
  `cpn-class-*-fields-methods-*` cluster reports **60 pass / 0 fail / 2
  skip**, and the broader class-elements diagnostic reports **1583 pass / 81
  fail / 1298 skip**.
- **TypedArray `[[HasProperty]]` prototype delegation**: ordinary property keys
  missing from a TypedArray now continue through the prototype's real
  `[[HasProperty]]` operation instead of raw own-property lookup. This preserves
  integer-indexed exotic handling for canonical numeric keys while propagating
  Proxy `has` traps on the prototype chain. With the Proxy gate temporarily
  lifted, `built-ins/TypedArrayConstructors/internals/HasProperty` reports
  **26 pass / 0 fail / 6 skip**.
- **Property Reference records for member logical assignment**: ordinary member
  logical assignments now preserve an explicit property Reference from
  `GetValue` through the conditional short-circuit and `PutValue` paths. This
  aligns `obj.x ||= rhs`, `obj[x] &&= rhs`, and `obj[x] ??= rhs` with the
  identifier Reference path while keeping short-circuit result values, computed
  key coercion count, Symbol keys, Proxy receiver identity, and strict
  failed-write behavior stable.
- **Property Reference records for member update expressions**: ordinary
  member update expressions now create an explicit property Reference and
  preserve it through `GetValue`, numeric increment/decrement, and `PutValue`.
  This aligns `obj.x++`, `++obj[x]`, and Symbol-keyed update targets with the
  identifier Reference path while preserving prefix/postfix result values,
  Proxy receiver identity, and strict failed-write behavior. The focused
  update-expression test262 cluster reports **138 pass / 0 fail / 4 skip**.
- **Property Reference records for member compound assignment**: ordinary
  member compound assignments now lower through the same spec
  `Reference -> GetValue -> PutValue` path as identifier compound
  assignments. The new property Reference path preserves Symbol keys, keeps
  computed keys single-evaluated, passes Proxy `set` traps the original
  receiver, and applies the Reference's strict flag when the final write
  fails. `language/expressions/compound-assignment` remains closed at **406
  pass / 0 fail / 48 skip**, with new Rust regression coverage for Proxy
  receiver and read-only-property strict/sloppy behavior.
- **Static class field initializer `this` binding**: static public and private
  field initializers now run with `this` bound to the class constructor, so
  `static g = this.f` and arrows created inside static initializers see the
  same receiver that `DefineField` passes to the initializer. With static
  public/private field skips temporarily lifted, the focused
  `static-field-init-with-this.js` and
  `static-field-init-this-inside-arrow-function.js` statement/expression
  tests now report **4 pass / 0 fail / 0 skip**. The
  `language/statements/class/elements` diagnostic improves from **466 pass /
  90 fail / 978 skip** to **469 pass / 87 fail / 978 skip**. The default
  supported subset remains **5099 pass / 0 fail / 0 timeout**.
- **Public class field `[[DefineOwnProperty]]` semantics**: public field
  initialization now routes through `CreateDataPropertyOrThrow` instead of
  raw property-map insertion. Fields therefore fail with `TypeError` when a
  previous initializer freezes the receiver and invoke Proxy `defineProperty`
  traps when a derived constructor returns a Proxy receiver. With public class
  field and Proxy skips temporarily lifted, the focused
  `class-field-on-frozen-objects.js` and
  `public-class-field-initialization-is-visible-to-proxy.js` tests now run at
  **2 pass / 0 fail / 0 skip**. The broader
  `language/{statements,expressions}/class/elements` diagnostic with public,
  private, and Proxy gates temporarily lifted reports **1460 pass / 184 fail /
  1318 skip**. The default supported subset remains **5099 pass / 0 fail / 0
  timeout**.
- **Private element duplicate initialization**: private field, private method,
  and private accessor initialization now rejects attempts to add the same
  class private name to the same receiver twice instead of overwriting the
  existing private slot. This matches derived-constructor cases where a base
  constructor returns an object that is reused across multiple subclass
  constructions. With private class feature skips temporarily lifted, the
  focused
  `language/statements/class/elements/private-method-double-initialisation*.js`
  and `privatefieldadd-typeerror.js` cluster now runs at **5 pass / 0 fail /
  0 skip**. The default supported subset remains **5099 pass / 0 fail / 0
  timeout**.
- **Public class fields baseline**: class bodies now parse public instance and
  static field declarations, including computed names and `static`/`get`/`set`/
  `async` names that are fields rather than method prefixes. Field
  initialization uses `DefineDataProperty`, so inherited setters are not
  invoked and uninitialized fields become own `undefined` data properties. With
  public class field skips temporarily lifted, the focused
  `language/{statements,expressions}/class/elements` diagnostic now reports
  **309 pass / 113 fail / 2540 skip**. The default supported subset remains
  **5099 pass / 0 fail / 0 timeout** because full public-field coverage is
  still gated behind direct-eval, computed-name ordering, and static
  initializer follow-ups.
- **DataView prototype and constructor ordering**: `DataView.prototype` now
  exposes spec-shaped `@@toStringTag`, `Reflect.construct(DataView, ...)`
  validates invalid byte offsets before reading `newTarget.prototype`, and
  rechecks detached ArrayBuffers after the observable prototype lookup. The
  runner now executes implemented `built-ins/DataView/` coverage under a
  path-scoped feature exception at **415 pass / 0 fail / 146 skip**; remaining
  skips stay behind broad Symbol, SharedArrayBuffer, and resizable ArrayBuffer
  coverage.
- **TypedArray constructors coverage lift**: the runner now executes the
  implemented `built-ins/TypedArrayConstructors/` tests under a path-scoped
  feature exception instead of requiring ad hoc temporary skip removal. The
  focused run reports **599 pass / 0 fail / 139 skip**; the remaining skips
  stay concentrated in SharedArrayBuffer, resizable ArrayBuffer, Proxy, broad
  Symbol iterator, and generator coverage.
- **For-of iterator protocol edges**: `for...of` now caches the iterator
  `next` method at `GetIterator` time, rejects non-object iterator results,
  applies `ToBoolean` to `done`, validates `return()` results during
  `IteratorClose`, preserves original throws when close also throws, and keeps
  hidden iterator-close state alive across labeled `continue`. The focused
  `language/statements/for-of` run now reports **113 pass / 0 fail / 638
  skip**, and the supported subset increases to **5099 pass / 0 fail / 0
  timeout**.
- **Object spread Symbol keys**: `{...source}` now copies enumerable own Symbol
  properties and follows `[[OwnPropertyKeys]]` order for integer index keys,
  string keys, then Symbol keys. It also re-checks each property descriptor at
  copy time and propagates Proxy `ownKeys` failures instead of falling back to
  the target. The focused
  `language/expressions/{array,call,new}/spread-obj-{spread-order,symbol-property,with-overrides}.js`
  cluster now runs at **9 pass / 0 fail**, and the supported subset increases
  to **5079 pass / 0 fail / 0 timeout**.
- **Symbol-key `SetFunctionName`**: object literal methods, anonymous
  function/arrow/class property values, and public class methods/accessors now
  infer function `name` properties from runtime Symbol property keys, using
  `[description]` formatting and `get ` / `set ` prefixes while preserving the
  cover-expression exception for `(0, function() {})`. The focused Symbol
  function-name cluster now runs at **10 pass / 0 fail / 5 skip**, and the
  supported subset increases to **5070 pass / 0 fail / 0 timeout**.
- **Disposal well-known Symbols**: `Symbol.dispose` and
  `Symbol.asyncDispose` are now exposed as shared well-known Symbols with
  spec-shaped static property descriptors and no global-registry keys.
  `tools/test262_runner.py` and `tools/test262_analyze.py` keep broader
  `explicit-resource-management` syntax coverage skipped while allowing the
  focused `built-ins/Symbol/{dispose,asyncDispose}` intrinsic tests to run at
  **6 pass / 0 fail / 0 skip**.
- **`%ThrowTypeError%` Realm identity**: restricted
  `Function.prototype.caller`/`arguments` accessors and strict-mode unmapped
  arguments objects now reuse the same canonical Realm `%ThrowTypeError%`
  intrinsic even when the arguments object is created by a function nested
  inside `new Function(...)`. `$262.createRealm()` now receives a Realm-local
  `Function.prototype` for dynamic functions, so cross-Realm restricted
  accessors compare against that Realm's thrower instead of the main Realm's.
  The focused
  `built-ins/Function/prototype/caller built-ins/Function/prototype/arguments`
  run improves from **0 pass / 2 fail / 0 skip** to **2 pass / 0 fail / 0
  skip**.
- **`Function.prototype[@@hasInstance]`**: `instanceof` now performs
  `GetMethod(C, @@hasInstance)` before falling back to `OrdinaryHasInstance`,
  and `Function.prototype[Symbol.hasInstance]` exposes the default hook with
  spec-shaped `name`, `length`, and property attributes. The
  `Symbol.hasInstance` test262 feature skip is removed; the supported subset
  remains green while increasing to **5060 pass / 0 fail / 0 timeout**.
- **Symbol prototype well-known properties**: `Symbol.prototype[@@toPrimitive]`
  and `Symbol.prototype[@@toStringTag]` now expose spec-shaped descriptors,
  Symbol primitives can resolve symbol-keyed prototype properties, and unary
  minus now uses `ToNumeric` so BigInt object wrappers negate as BigInt. The
  `Symbol.toPrimitive` and `Symbol.toStringTag` test262 feature skips are
  removed; the supported subset remains green while increasing to **5057 pass
  / 0 fail / 0 timeout**.
- **String exotic objects and coercion**: `String(object)` now performs
  observable `ToPrimitive` with string hint instead of bypassing overridden
  `toString` on arrays, while `OrdinaryToPrimitive` now skips non-callable
  `toString`/`valueOf` candidates. Boxed String numeric index properties now
  stay read-only/enumerable exotic own properties for assignment and
  `propertyIsEnumerable`, and `String.prototype.localeCompare` treats
  canonically equivalent Unicode strings as equal. The focused
  `built-ins/String` run improves from **1093 pass / 3 fail / 127 skip** to
  **1096 pass / 0 fail / 127 skip**.
- **Global `undefined` Reference semantics**: source `undefined` now parses as
  an IdentifierReference, so assignment uses `PutValue` against the
  non-writable global property and `delete undefined` uses the identifier
  delete path. Sloppy assignment remains ignored while returning the RHS,
  strict assignment throws `TypeError`, and delete returns `false`. The focused
  `built-ins/global built-ins/undefined` run improves from **33 pass / 4 fail
  / 0 skip** to **37 pass / 0 fail / 0 skip**.
- **Proxy `[[Construct]]` semantics**: constructable Proxy objects now follow
  their target's constructability, dispatch `construct` traps with a current
  Realm argument array, validate trap callability and object return values, and
  delegate through the target when no trap is present. The focused
  `built-ins/Proxy` run improves from **3 pass / 1 fail / 307 skip** to **4
  pass / 0 fail / 307 skip**.
- **Map/Set iterator prototype shape**: Map and Set iterators now inherit from
  shared `%MapIteratorPrototype%`/`%SetIteratorPrototype%` objects instead of
  carrying own `next` methods, expose spec-shaped `next` and
  `@@toStringTag` properties, and reject `next` calls on receivers without
  collection-iterator internal slots. The focused
  `built-ins/MapIteratorPrototype built-ins/SetIteratorPrototype` run improves
  from **9 pass / 5 fail / 8 skip** to **14 pass / 0 fail / 8 skip**.
- **DataView constructor ordering**: `DataView` now rejects function calls
  before coercing constructor arguments, and detached ArrayBuffers are checked
  only after the observable `byteOffset` coercion. The focused
  `built-ins/DataView` run improves from **266 pass / 2 fail / 293 skip** to
  **268 pass / 0 fail / 293 skip**.
- **DataView constructor length**: `DataView.length` now has the spec value
  `1` with the standard non-writable, non-enumerable, configurable descriptor.
  With DataView-related skips temporarily lifted, `built-ins/DataView/length.js`
  now passes, and the broader `built-ins/DataView` diagnostic reports
  **310 pass / 11 fail / 240 skip**.
- **DataView immutable-buffer setters**: implemented DataView setter
  validation for immutable ArrayBuffer backing stores. The implemented
  numeric and BigInt setters now throw `TypeError` before reading
  `byteOffset` or `value` arguments when the viewed buffer is immutable. With
  DataView-related skips temporarily lifted, `built-ins/DataView` improves to
  **320 pass / 1 fail / 240 skip**, leaving only the unsupported
  `setFloat16` immutable-buffer case in that diagnostic.
- **DataView Float16 accessors**: `DataView.prototype.getFloat16` and
  `setFloat16` now read and write IEEE-754 binary16 values with spec-shaped
  endian handling, ties-to-even rounding, signed zero, infinities, NaN, and the
  same validation ordering as the other DataView numeric methods. With
  DataView-related skips temporarily lifted, `built-ins/DataView` now closes at
  **321 pass / 0 fail / 240 skip**; additionally lifting `Float16Array` for the
  DataView diagnostic reports **352 pass / 0 fail / 209 skip**.
- **Date component getter receiver validation**: Date component getters now
  use a `thisTimeValue`-style receiver check, so ordinary objects, arrays,
  arguments objects, primitives, and objects spoofing RuJa's internal
  `__time__` property throw `TypeError` instead of reading as Invalid Date.
  `%Date.prototype%` is no longer Date-branded, while constructed Date and
  Date subclass instances still expose the Date brand. The focused Date
  component getter run improves from **80 pass / 16 fail / 32 skip** to
  **96 pass / 0 fail / 32 skip**; the broader `built-ins/Date` diagnostic now
  reports **309 pass / 173 fail / 112 skip**.
- **Date.UTC and TimeClip semantics**: `Date.UTC` now performs left-to-right
  numeric coercion for all supplied components, applies default month/date/time
  fields, normalizes 0-99 years, and returns the clipped MakeDate result.
  `TimeClip` now truncates fractional milliseconds and normalizes negative
  zero, so `Date` construction, `getTime`/`valueOf`, and `setTime` expose
  integer clipped time values. The focused
  `built-ins/Date/UTC built-ins/Date/prototype/{getTime,valueOf,setTime}` run
  improves from **20 pass / 16 fail / 6 skip** to **36 pass / 0 fail / 6
  skip**; the broader `built-ins/Date` diagnostic now reports **326 pass / 156
  fail / 112 skip**.
- **Date time-component setters**: `setMilliseconds`, `setSeconds`,
  `setMinutes`, `setHours`, and their UTC variants now read the receiver's
  DateValue before argument coercion, coerce optional arguments left to right,
  preserve omitted lower-order components, apply `TimeClip`, and expose
  spec-shaped `length` values. Invalid Date receivers still coerce supplied
  arguments but return `NaN` without overwriting side effects from coercion.
  The focused time-setter run improves from **28 pass / 68 fail / 12 skip** to
  **96 pass / 0 fail / 12 skip**; the broader `built-ins/Date` diagnostic now
  reports **394 pass / 88 fail / 112 skip**.
- **Date date-component setters**: `setDate`, `setMonth`, `setFullYear`, and
  their UTC variants now preserve the existing time within day, coerce optional
  arguments left to right, avoid the constructor-only 1900 offset for
  `setFullYear(0..99)`, and apply the distinct Invalid Date semantics for
  date/month setters versus full-year setters. The focused date-setter run
  improves from **23 pass / 41 fail / 9 skip** to **64 pass / 0 fail / 9
  skip**; the broader `built-ins/Date` diagnostic now reports **435 pass / 47
  fail / 112 skip**.
- **Date stringification, JSON, and ISO parsing**: Date prototype string
  methods now validate Date receivers, render UTC-backed date/time strings,
  return `Invalid Date` for invalid time values, and expose proper
  `toISOString` RangeError behavior. `Date.prototype.toJSON` now follows the
  generic `ToObject`/`ToPrimitive(number)`/`Invoke(toISOString)` path, while
  `Date.parse` recognizes the ISO and Date string forms emitted by RuJa.
  Single-argument Date construction now copies Date receivers without calling
  user hooks and parses Date strings. The focused string/parse/JSON run
  improves from **26 pass / 37 fail / 13 skip** to **63 pass / 0 fail / 13
  skip**; the broader `built-ins/Date` diagnostic now reports **476 pass / 6
  fail / 112 skip**, with the remaining failures isolated to Temporal
  `toTemporalInstant` coverage.
- **Date toTemporalInstant bridge**: `Date.prototype.toTemporalInstant` now
  validates Date-branded receivers, throws `RangeError` for invalid dates, and
  returns a minimal Temporal Instant-shaped object exposing
  `epochNanoseconds` as a BigInt millisecond-to-nanosecond conversion. The
  focused `built-ins/Date/prototype/toTemporalInstant` run improves from **0
  pass / 6 fail / 2 skip** to **6 pass / 0 fail / 2 skip**, closing the
  broader `built-ins/Date` diagnostic at **482 pass / 0 fail / 112 skip**.
- **BigInt TypedArray constructor surface**: BigInt typed array constructors
  and prototypes now expose non-writable, non-enumerable, non-configurable
  `BYTES_PER_ELEMENT` own properties, and typed array prototype accessors
  reject non-typed-array receivers. The focused
  `built-ins/TypedArrayConstructors` run improves from **10 pass / 6 fail /
  722 skip** to **16 pass / 0 fail / 722 skip**.
- **TypedArray integer-indexed `[[Set]]` ordering**: TypedArray numeric index
  assignments now run element value conversion before detached-buffer,
  out-of-bounds, invalid-index, or immutable-buffer validation. This preserves
  observable `ToNumber`/`ToBigInt` side effects and abrupt completions even
  when the write ultimately has no effect. With TypedArray-related skips
  temporarily lifted, `built-ins/TypedArrayConstructors/internals/Set`
  improves from **15 pass / 8 fail / 30 skip** to **21 pass / 2 fail / 30
  skip**; the remaining failures are detached-buffer realm constructor
  coverage.
- **TypedArray ArrayBuffer constructor ordering**: TypedArray constructors
  taking an ArrayBuffer now coerce `byteOffset` and explicit `length` before
  rechecking whether the backing buffer was detached, while still applying
  byte-offset alignment before length coercion. This prevents views from being
  created over buffers detached during argument conversion. With
  TypedArray-related skips temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/{ctors,ctors-bigint}/buffer-arg` run
  reports **44 pass / 0 fail / 62 skip**.
- **Cross-realm TypedArray constructors**: `$262.createRealm()` now exposes
  realm-local `ArrayBuffer`, `DataView`, `%TypedArray%`, and concrete
  TypedArray constructors instead of leaving `other[TA.name]` absent. Concrete
  constructors inherit from that realm's hidden `%TypedArray%`, their
  prototypes inherit from that realm's `%TypedArray%.prototype`, and detached
  buffers created through cross-realm constructors keep the expected
  integer-indexed behavior. With TypedArray-related skips temporarily lifted,
  the broader `built-ins/TypedArrayConstructors` diagnostic improves from
  **463 pass / 10 fail / 265 skip** to **473 pass / 0 fail / 265 skip**.
- **Constructor realm prototype fallback**: `GetPrototypeFromConstructor`
  fallback now uses the active `newTarget` function realm's intrinsic
  prototype instead of always falling back to the current VM realm. This lets
  `Reflect.construct()` with a cross-realm `newTarget` whose `.prototype` is
  non-object allocate TypedArrays, ArrayBuffers, DataViews, and RegExps with
  the other realm's intrinsic prototype. Focused `proto-from-ctor-realm`
  checks now pass at **13 pass / 0 fail / 0 skip** across
  `TypedArrayConstructors`, `ArrayBuffer`, `DataView`, and `RegExp`.
- **TypedArray integer-indexed `[[HasProperty]]`**: canonical numeric index
  property checks on TypedArrays now follow Integer-Indexed exotic semantics:
  valid in-bounds indexes report present, while detached, out-of-bounds,
  `"-0"`, fractional, negative, and infinite canonical numeric strings return
  `false` without falling through to ordinary prototype lookup. Non-canonical
  keys such as `"+1"` still use ordinary lookup. With TypedArray, ArrayBuffer,
  DataView, Reflect, and `Reflect.construct` skips temporarily lifted, focused
  `built-ins/TypedArrayConstructors/internals/HasProperty` improves from
  **14 pass / 10 fail / 8 skip** to **22 pass / 2 fail / 8 skip**; the
  remaining failures are the missing `%TypedArray%.prototype.subarray` method.
  Under the same expanded diagnostic, broader `built-ins/TypedArrayConstructors`
  improves to **584 pass / 9 fail / 145 skip**.
- **TypedArray `subarray()` inherited method**:
  `%TypedArray%.prototype.subarray` now creates offset views over the same
  ArrayBuffer, normalizes begin/end like slice bounds, rejects detached
  buffers, and routes allocation through `@@species` while preserving
  Number-vs-BigInt content type. Concrete typed-array prototypes inherit the
  method without gaining own `subarray` properties. Focused
  `built-ins/TypedArrayConstructors/prototype/subarray` now passes at
  **2 pass / 0 fail / 0 skip**. With TypedArray, ArrayBuffer, DataView,
  Reflect, and `Reflect.construct` skips temporarily lifted, focused
  `built-ins/TypedArrayConstructors/internals/HasProperty` improves from
  **22 pass / 2 fail / 8 skip** to **24 pass / 0 fail / 8 skip**, and broader
  `built-ins/TypedArrayConstructors` improves to **586 pass / 7 fail / 145
  skip**. The remaining broader failures are concentrated in
  Integer-Indexed `[[OwnPropertyKeys]]` ordering, `Reflect.set` receiver
  writes, and one typed-array-argument validation ordering case.
- **TypedArray integer-indexed `[[OwnPropertyKeys]]`**:
  `Reflect.ownKeys()` and shared own-key enumeration now synthesize attached
  TypedArray integer index keys before ordinary string and symbol keys,
  including offset `subarray()` views, while detached buffers expose no
  integer-indexed own keys. Focused
  `built-ins/TypedArrayConstructors/internals/OwnPropertyKeys` improves from
  **0 pass / 4 fail / 6 skip** to **4 pass / 0 fail / 6 skip** with
  TypedArray, ArrayBuffer, DataView, Reflect, and `Reflect.construct` skips
  temporarily lifted; with Symbol also lifted it reports **8 pass / 0 fail /
  2 skip**. Under the same expanded diagnostic, broader
  `built-ins/TypedArrayConstructors` improves to **590 pass / 3 fail / 145
  skip**. The remaining failures are now `Reflect.set` receiver writes and
  one typed-array-argument validation ordering case.
- **TypedArray receiver-aware integer-indexed `[[Set]]`**:
  `Reflect.set(target, index, value, receiver)` now routes valid
  integer-indexed writes through the receiver instead of always mutating the
  target. Plain-object receivers get ordinary data properties without
  coercing the value, while TypedArray receivers apply their own integer-index
  validation and element conversion; invalid receiver indexes fail before
  value coercion. Focused `built-ins/TypedArrayConstructors/internals/Set`
  improves from **41 pass / 2 fail / 10 skip** to **43 pass / 0 fail / 10
  skip** with TypedArray, ArrayBuffer, DataView, Reflect, and
  `Reflect.construct` skips temporarily lifted. Under the same expanded
  diagnostic, broader `built-ins/TypedArrayConstructors` improves to **592
  pass / 1 fail / 145 skip**; the remaining failure is the
  typed-array-argument validation ordering case.
- **TypedArray constructor `newTarget.prototype` ordering**:
  TypedArray construction now defers observable `newTarget.prototype` lookup
  until allocation, after argument validation and conversion have completed.
  `Reflect.construct(TA, [Symbol()], newTarget)` now reports the required
  `ToIndex` `TypeError` without touching a throwing custom prototype getter.
  Focused `built-ins/TypedArrayConstructors/ctors/typedarray-arg` improves
  from **12 pass / 1 fail / 1 skip** to **13 pass / 0 fail / 1 skip** with
  TypedArray, ArrayBuffer, DataView, Reflect, and `Reflect.construct` skips
  temporarily lifted. Under the same expanded diagnostic, broader
  `built-ins/TypedArrayConstructors` closes at **593 pass / 0 fail / 145
  skip**.
- **TypedArray Symbol-key `Reflect.set`**:
  `Reflect.set()` now routes Symbol property keys through the same
  receiver-aware ordinary `[[Set]]` path as string keys, so Symbol-named
  non-writable own data properties on TypedArrays return `false` instead of
  silently reporting success. With concrete TypedArray, Symbol, Proxy,
  ArrayBuffer, DataView, Reflect, and `Reflect.construct` gates admitted for
  the path, `built-ins/TypedArrayConstructors` now reports **673 pass / 0 fail
  / 65 skip** through the normal runner.
- **`Error.isError` static method**: `Error.isError(value)` is now exposed as
  a non-constructable unary builtin and recognizes real Error/NativeError
  objects, Error subclasses, and `$262.createRealm()` Error objects while
  rejecting primitives, constructors, ordinary objects, and objects that only
  spoof `Error.prototype`. `$262.createRealm()` also exposes `Array` and the
  native error constructor surface needed by cross-realm Error checks. The
  focused `built-ins/Error/isError` run improves from **0 pass / 10 fail / 2
  skip** to **10 pass / 0 fail / 2 skip**; the broader `built-ins/Error`
  diagnostic now reports **46 pass / 28 fail / 19 skip**.
- **`Error.prototype.toString` edge cases**:
  `Error.prototype.toString` now throws `TypeError` when called with a
  primitive receiver and omits the separating colon when either the resolved
  `name` or `message` string is empty. The focused
  `built-ins/Error/prototype/toString` run now closes at **15 pass / 0 fail /
  2 skip**, and the broader `built-ins/Error` diagnostic improves to **48 pass
  / 26 fail / 19 skip** with the remaining failures isolated to
  `Error.prototype.stack`.
- **`Error.prototype.stack` accessor**:
  `%Error.prototype%` now exposes a Realm-local `stack` accessor that accepts
  real Error objects, leaves newly constructed Error objects without an own
  `stack` data property, defines receiver-local stack data properties through
  the setter, and throws the receiver Realm's `TypeError` for forbidden
  prototype writes. Native error synthesis now preserves the throwing native
  callee's Realm and uses the Realm's original intrinsic Error prototypes
  instead of mutable global `TypeError`/`Error` bindings. `$262.createRealm()`
  now builds Realm-local Error and NativeError constructor/prototype chains for
  those cross-Realm checks. The
  focused `built-ins/Error/prototype/stack` run closes at **35 pass / 0 fail /
  0 skip**, and the broader `built-ins/Error` runner now reports **83 pass /
  0 fail / 10 skip**.
- **ArrayBuffer static surface**: `ArrayBuffer` now rejects calls without
  `new` before length coercion, exposes `ArrayBuffer.isView()` for typed-array
  and DataView receivers, provides the `ArrayBuffer[Symbol.species]` getter,
  and uses the intrinsic `%ArrayBuffer.prototype%` fallback for
  `Reflect.construct` new targets with non-object prototypes. The focused
  `built-ins/ArrayBuffer` run improves from **41 pass / 50 fail / 130 skip**
  to **52 pass / 39 fail / 130 skip**.
- **ArrayBuffer slice species construction**: `ArrayBuffer.prototype.slice`
  now uses `SpeciesConstructor`, accepts nullish `@@species` as the default
  `ArrayBuffer` constructor, calls custom species constructors with the slice
  length, rejects invalid species results, and preserves larger result buffer
  lengths while copying sliced bytes. The focused `built-ins/ArrayBuffer` run
  improves from **52 pass / 39 fail / 130 skip** to **57 pass / 34 fail / 130
  skip**.
- **ArrayBuffer transfer and immutable surface**: fixed-length ArrayBuffers now
  expose `transfer`, `transferToFixedLength`, `transferToImmutable`,
  `sliceToImmutable`, and the `immutable` accessor with descriptor-compatible
  names/lengths. Transfer operations copy, resize, zero-pad/truncate, detach
  the source, reject detached/immutable sources in spec order, and
  `ArrayBuffer.prototype.slice` now rejects detached sources and immutable
  species results. Related coercion fixes let `Array.from` read TypedArray
  array-like lengths, trim ES whitespace including `\uFEFF` in string numeric
  conversion, and coerce `Array.prototype.slice` bounds such as `null`. The
  focused `built-ins/ArrayBuffer` run improves from **57 pass / 34 fail / 130
  skip** to **90 pass / 1 fail / 130 skip**.
- **VM GC return-value rooting**: frame-boundary and top-level GC safe points
  now pin interpreted function return values and thrown values until the caller
  can observe them, and native calls pin their receiver while dispatching. This
  prevents a freshly returned `ArrayBuffer` from being swept during the long
  `sliceToImmutable` argument-coercion test and closes the focused
  `built-ins/ArrayBuffer` run at **91 pass / 0 fail / 130 skip**.
- **TypedArray intrinsic prototype shape**: concrete TypedArray constructors
  now report the spec `length` of `3`, inherit from a shared `%TypedArray%`
  intrinsic constructor, and their prototypes inherit `buffer`, `byteLength`,
  `byteOffset`, and `length` accessors from a shared `%TypedArray%.prototype`
  instead of defining them as own properties. With TypedArray skips temporarily
  lifted, the focused constructor/prototype-shape probe now reports
  **120 pass / 0 fail / 11 skip**.
- **TypedArray static `from`/`of`**: concrete TypedArray constructors now
  inherit `%TypedArray%.from` and `%TypedArray%.of`, construct the result before
  reading array-like elements, call mapper functions with the expected
  arguments and receiver, cache iterable `next` methods, and reject immutable
  ArrayBuffer-backed results before value conversion. With TypedArray skips
  temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/{from,from/BigInt,of,of/BigInt}` run
  closes at **126 pass / 0 fail / 0 skip**, and the broader
  `built-ins/TypedArrayConstructors` diagnostic now reports **473 pass / 54
  fail / 211 skip**.
- **TypedArray integer-indexed `[[Delete]]`**: deleting canonical numeric index
  strings on TypedArrays now follows Integer-Indexed exotic semantics: valid
  in-bounds indexes return `false`, while detached buffers, `"-0"`, fractional,
  negative, infinite, and out-of-bounds canonical numeric keys return `true`.
  Non-canonical keys continue through ordinary delete. With TypedArray skips
  temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/internals/Delete` diagnostic improves to
  **29 pass / 2 fail / 8 skip**, and the broader
  `built-ins/TypedArrayConstructors` diagnostic moves from **419 pass / 54 fail
  / 265 skip** to **431 pass / 42 fail / 265 skip**.
- **TypedArray integer-indexed `[[GetOwnProperty]]`**: valid canonical numeric
  index strings on TypedArrays now synthesize spec-shaped data descriptors
  with the element value and writable/enumerable/configurable all `true`.
  Detached buffers and invalid canonical numeric keys such as `"-0"`,
  fractional, negative, infinite, and out-of-bounds indexes stop ordinary
  fallback and report no descriptor, while non-canonical keys continue through
  ordinary properties. The same descriptor path feeds Proxy `has` and
  `deleteProperty` invariants for non-extensible TypedArray targets. With
  TypedArray skips temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/internals/GetOwnProperty` diagnostic now
  reports **18 pass / 2 fail / 4 skip**; the remaining failures are
  cross-realm detached-buffer constructor coverage. The broader
  `built-ins/TypedArrayConstructors` diagnostic reports **429 pass / 44 fail /
  265 skip**.
- **TypedArray integer-indexed `[[Get]]`**: canonical numeric index property
  reads now use Integer-Indexed exotic element access instead of Rust integer
  parsing or ordinary prototype lookup. Valid indexes read numeric and BigInt
  elements from owned or ArrayBuffer-backed storage, detached buffers and
  invalid canonical numeric keys such as `"-0"`, fractional, negative,
  infinite, and out-of-bounds indexes return `undefined` without touching
  inherited accessors, and non-canonical numeric-looking keys such as `"+1"`
  continue through ordinary own/prototype lookup. With TypedArray skips
  temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/internals/Get` diagnostic now reports
  **20 pass / 2 fail / 6 skip**; the remaining failures are cross-realm
  detached-buffer constructor coverage. The broader
  `built-ins/TypedArrayConstructors` diagnostic improves to **437 pass / 36
  fail / 265 skip**.
- **TypedArray integer-indexed `[[DefineOwnProperty]]`**: defining canonical
  numeric index properties now follows Integer-Indexed exotic validation.
  Invalid or detached indexes reject, accessor descriptors and descriptors
  requesting non-configurable, non-enumerable, or non-writable attributes
  reject, valid value descriptors write through element conversion for numeric
  and BigInt arrays, and non-canonical numeric-looking keys remain ordinary
  properties. With TypedArray skips temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/internals/DefineOwnProperty` diagnostic now
  reports **16 pass / 2 fail / 36 skip**; the remaining failures are
  cross-realm detached-buffer constructor coverage. The broader
  `built-ins/TypedArrayConstructors` diagnostic improves to **453 pass / 20
  fail / 265 skip**.
- **Nullish computed property write/delete ordering**: simple computed
  property assignment and `delete` now reject `null`/`undefined` bases before
  observable `ToPropertyKey` coercion. Assignment still evaluates the RHS
  before the `PutValue` `TypeError`, while delete evaluates only the computed
  key expression before the nullish-base failure. The focused
  `language/expressions/{assignment,delete,member-expression}` run remains
  **273 pass / 0 fail / 282 skip**.
- **Array destructuring assignment IteratorClose ordering**: array assignment
  patterns now close unfinished iterators on normal partial completion,
  evaluate rest assignment target references before draining rest values, and
  close iterators when rest-target or rest-iterator evaluation completes
  abruptly. The focused `language/expressions/assignment/dstr` run closes at
  **90 pass / 0 fail / 278 skip**, and the broader Reference-adjacent cluster
  `language/expressions/assignment language/expressions/compound-assignment
  language/expressions/logical-assignment language/expressions/update
  language/statements/with` remains **835 pass / 0 fail / 363 skip**.
- **`%ThrowTypeError%` intrinsic**: restricted function and arguments
  accessors now use an anonymous, frozen, non-extensible `%ThrowTypeError%`
  function per Realm. Strict and non-simple-parameter unmapped arguments reuse
  the same Realm-local thrower for `callee`, while `$262.createRealm()` gets a
  distinct intrinsic. The focused `built-ins/ThrowTypeError` run improves from
  **8 pass / 6 fail / 0 skip** to **14 pass / 0 fail / 0 skip**.
- **Private slot brand checks**: class private field, accessor, and method
  access now throws `TypeError` for primitive receivers and objects missing the
  private slot instead of returning `undefined` or creating a new slot.
  Class-element initialization now uses separate `InitPrivate` opcodes, private
  method slots are not writable. With private class feature skips temporarily
  lifted, the focused `language/{statements,expressions}/class/elements` probe
  improves from **1045 pass / 587 fail / 1330 skip** to **1085 pass / 547 fail
  / 1330 skip**. The remaining same-spelling cross-class failures require
  per-evaluation private-name identity instead of textual `#name` keys.
- **Per-evaluation private-name identity**: class evaluation now allocates a
  fresh opaque private-name key for each private field/method/accessor name and
  stores it in the class lexical environment captured by constructors and
  methods. Private slots use those opaque keys, while RegExp and Proxy internal
  slots use a separate internal-key namespace. Same-spelling private names from
  different class evaluations or from superclass/subclass bodies no longer
  share brands. With private class feature skips temporarily lifted, the
  focused `language/{statements,expressions}/class/elements` probe improves from
  **1085 pass / 547 fail / 1330 skip** to **1096 pass / 536 fail / 1330 skip**.
- **Private names before division**: the lexer now treats private names as
  value-ending tokens for slash disambiguation, so `this.#x / y` and
  `this.#x /= y` parse as division and divide-assignment instead of starting a
  RegExp literal. With private class feature skips temporarily lifted, the
  focused `language/expressions/compound-assignment` diagnostic now reports
  **454 pass / 0 fail / 0 skip**.
- **`String.prototype.matchAll` and RegExp `@@matchAll`**:
  `String.prototype.matchAll` is now exposed with spec-shaped builtin
  properties, validates non-global RegExp arguments before delegation, calls
  custom `@@matchAll` methods with the original receiver value, and falls back
  through a forced-global intrinsic RegExp. `RegExp.prototype[Symbol.matchAll]`
  now creates a lazy RegExp String Iterator through `RegExpExec`, preserving
  species construction, cached `lastIndex`, custom `exec`, match result
  arrays, and empty-match advancement. The focused
  `built-ins/String/prototype/matchAll
  built-ins/RegExp/prototype/Symbol.matchAll` run moves from **5 pass / 43 fail
  / 3 skip** to **48 pass / 0 fail / 3 skip**.
- **`String.prototype.normalize` Unicode forms**: `String.prototype.normalize`
  is now exposed as a non-constructor prototype builtin with spec-shaped
  `name`, `length`, and descriptor properties. It defaults to NFC, coerces the
  `form` argument observably, rejects invalid forms with `RangeError`, and
  returns NFC/NFD/NFKC/NFKD output through Unicode normalization. The focused
  `built-ins/String/prototype/normalize` run now closes at **11 pass / 0 fail /
  3 skip**.
- **RegExp `@@match` prototype builtin**:
  `RegExp.prototype[Symbol.match]` is now installed with the expected builtin
  shape and routes through `RegExpExec`, so direct `r[Symbol.match](value)`
  calls observe public `flags`, custom `exec`, `lastIndex` writes, global
  empty-match advancement, and abrupt completions. `String.prototype.match`
  now falls back through an intrinsic RegExp clone, preserving RegExp source
  and flags when an own `@@match` is `undefined`. The focused
  `built-ins/RegExp/prototype/Symbol.match` run moves from **8 pass / 44 fail /
  1 skip** to **52 pass / 0 fail / 1 skip** after non-Unicode surrogate escapes
  are lowered for the Rust regex backend.
- **URI encode/decode globals**: `encodeURI`, `encodeURIComponent`,
  `decodeURI`, and `decodeURIComponent` now implement ECMAScript percent
  encoding/decoding over UTF-16 code units, preserve `decodeURI` reserved
  escapes, reject malformed UTF-8 and lone surrogates with `URIError`, and keep
  `String.fromCharCode` pairs in RuJa's surrogate-sentinel range distinguishable
  from lone surrogates. The focused
  `built-ins/{decodeURI,decodeURIComponent,encodeURI,encodeURIComponent}` run
  improves from **74 pass / 93 fail / 2 timeout / 4 skip** to **167 pass / 0
  fail / 2 timeout / 4 skip**.
- **Array `some`/`every` generic iteration**: `Array.prototype.some` and
  `Array.prototype.every` now follow `LengthOfArrayLike`/`HasProperty` before
  `Get`, so array-like receivers, boxed primitives, inherited sparse indexes,
  callback `thisArg`, length snapshots, and abrupt completions are observed.
  The focused `built-ins/Array/prototype/{some,every}` run improves from
  **225 pass / 202 fail / 10 skip** to **427 pass / 0 fail / 10 skip**.
- **RegExp boolean flag accessors**: `global`, `ignoreCase`, `multiline`,
  `dotAll`, `sticky`, `unicode`, `unicodeSets`, and `hasIndices` now enforce
  RegExp internal-slot receiver validation. Real RegExp objects still expose
  their stored flag bits, the current realm `%RegExp.prototype%` returns
  `undefined`, and ordinary or cross-realm prototype receivers throw
  `TypeError`. The focused
  `built-ins/RegExp/prototype/{flags,global,ignoreCase,multiline,dotAll,sticky,unicode,unicodeSets,hasIndices}`
  run now closes at **62 pass / 0 fail / 54 skip**.
- **`String.prototype.replaceAll` and RegExp `@@replace`**:
  `String.prototype.replaceAll` now follows the spec's observable ordering for
  `IsRegExp`, global-flag validation, `@@replace` delegation, receiver/search
  coercion, callable replacers, empty search strings, and `$` substitution
  tokens. `RegExp.prototype[Symbol.replace]` is now installed for global,
  sticky, capture, named-capture, and functional replacement paths, while
  `RegExp.prototype.toString` observes the public `source`/`flags` getters.
  The same slice fixes `super[Symbol.*]` method lookup/calls and nested array
  binding temporaries uncovered by the focused test262 file. The focused
  `built-ins/String/prototype/replaceAll` run now closes at **35 pass / 0 fail /
  10 skip**.
- **`RegExp.escape` static builtin**: `RegExp.escape` is now installed on each
  realm-local `RegExp` constructor with the expected own property shape,
  rejects non-string inputs without coercion, and implements the ES
  `EncodeForRegExpEscape` rules for initial ASCII alphanumerics, syntax
  characters, `/`, control escapes, whitespace/line terminators, other
  punctuators, and lone UTF-16 surrogates. The focused
  `built-ins/RegExp/escape` run now closes at **19 pass / 0 fail / 1 skip**.
- **Object integrity for arrays, arguments, functions, and Proxy traps**:
  `Object.seal`/`Object.freeze` now route through the Proxy-aware
  `[[PreventExtensions]]` path so false Proxy traps throw for the `Object.*`
  forms, materialize dense Array and arguments indexes so sealed/frozen
  descriptors are observable, and freeze Array `length` by honoring its
  non-writable descriptor during length assignment. `Object.isSealed`/
  `Object.isFrozen` now require non-extensible ordinary objects/functions and
  report Array/arguments integrity from their materialized descriptors while
  preserving primitive receivers as already sealed/frozen. The focused
  `built-ins/Object/{seal,freeze,isSealed,isFrozen}` run now closes at **218
  pass / 0 fail / 21 skip**.
- **TypedArray constructor surface**: the existing byte-backed TypedArray
  exotic now exposes the full constructor family (`Int8Array`,
  `Uint8ClampedArray`, `Int16Array`, `Uint16Array`, `Int32Array`,
  `Uint32Array`, `Float32Array`, `Float64Array`, `BigInt64Array`, and
  `BigUint64Array`) alongside `Uint8Array`, with element-size-aware
  `length`/`byteLength`, indexed reads/writes, BigInt element conversion, and
  `[[Extensible]]` tracking for `Object.seal`/`Object.preventExtensions`.
  This removes the final TypedArray-constructor failures from the Object
  integrity focused run.
- **TypedArray constructor inputs**: typed-array constructors now reject
  function-call usage without `new`, use the active typed-array prototype as
  the fallback for `GetPrototypeFromConstructor`, coerce primitive lengths with
  `ToIndex`-style `NaN`/`undefined` handling, read ordinary array-like
  `length`/indexed properties observably, and consume iterable arguments via
  `IteratorToList` before element conversion. `Array.prototype[Symbol.iterator]`
  is now exposed as the `values` method so array-backed iterable constructor
  inputs use the normal iterator protocol. With TypedArray-related skips
  temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/ctors/{no-args,length-arg,object-arg}`
  diagnostic moved from **8 pass / 20 fail / 19 skip** to **25 pass / 3 fail /
  19 skip**.
- **ArrayBuffer-backed TypedArray views**: typed-array instances now carry
  `[[ViewedArrayBuffer]]`, `[[ByteOffset]]`, and `[[ByteLength]]` slots.
  Constructors accept `ArrayBuffer` inputs with range/alignment checks, expose
  the original buffer through `.buffer`, report view-relative `length`,
  `byteLength`, and `byteOffset`, and route indexed reads/writes through the
  shared backing buffer. With TypedArray-related skips temporarily lifted, the
  focused
  `built-ins/TypedArrayConstructors/ctors/{no-args,length-arg,object-arg}`
  diagnostic now reports **26 pass / 2 fail / 19 skip**; the remaining
  failures are isolated to iterable zero-fill coverage and shared
  ArrayIteratorPrototype mutation semantics.
- **TypedArray prototype numeric `[[Set]]`**: assignment now recognizes full
  `CanonicalNumericIndexString` keys, including `"NaN"` and `"-0"`, when a
  TypedArray appears on an ordinary object's prototype chain. Invalid numeric
  indexes are treated as successful no-ops instead of creating receiver
  properties, while valid inherited numeric indexes still create receiver data
  properties. With TypedArray-related skips temporarily lifted, the focused
  `language/statements/with` diagnostic now reports **171 pass / 0 fail / 10
  skip**.
- **TypedArray backing buffers and ArrayIteratorPrototype `next`**:
  typed-array views now trace their `[[ViewedArrayBuffer]]` during GC, keeping
  zero-filled length allocations alive across harness pressure. Array iterator
  objects now inherit `next` and `@@iterator` from a shared prototype instead
  of masking prototype writes with an own `next`, so typed-array construction
  observes `Object.getPrototypeOf([].values()).next` overrides through the
  iterator protocol. With TypedArray-related skips temporarily lifted, the
  focused `built-ins/TypedArrayConstructors/ctors/{no-args,length-arg,object-arg}`
  diagnostic now reports **28 pass / 0 fail / 19 skip**, closing that
  constructor probe.
- **Proxy SetIntegrityLevel/TestIntegrityLevel**: transparent Proxy receivers
  for `Object.seal`/`Object.freeze` now tighten the target's own descriptors
  through the Proxy-aware `[[DefineOwnProperty]]` path, and
  `Object.isSealed`/`Object.isFrozen` now use Proxy `ownKeys` and
  `getOwnPropertyDescriptor` semantics instead of treating Proxy objects as
  ordinary empty exotics. With `Proxy`/`Reflect`/`Symbol` skips temporarily
  lifted, the focused Object integrity proxy diagnostic runs at **6 pass / 0
  fail**.
- **Proxy prototype internal methods**: `Object.getPrototypeOf`,
  `Reflect.getPrototypeOf`, `Object.setPrototypeOf`, `Reflect.setPrototypeOf`,
  the `__proto__` accessor, and `instanceof` now route through Proxy
  `getPrototypeOf`/`setPrototypeOf` traps, including nullish trap delegation,
  revoked-proxy errors, and non-extensible target invariants. With
  `Proxy`/`Reflect` skips temporarily lifted, the focused
  `built-ins/Proxy/{getPrototypeOf,setPrototypeOf}
  built-ins/Reflect/{getPrototypeOf,setPrototypeOf}` diagnostic runs at
  **29 pass / 0 fail / 31 skip**, and the broader Proxy descriptor/prototype
  diagnostic improves from **21 pass / 46 fail / 63 skip** to **46 pass / 21
  fail / 63 skip**. The remaining failures are isolated to Proxy descriptor
  conversion and define/getOwnPropertyDescriptor invariants.
- **Proxy.revocable revoke function shape**: `Proxy.revocable()` now creates
  its revoke closure through the native function helper so the closure exposes
  spec-shaped own `length` and `name` properties in the expected order, while
  keeping the associated proxy in a non-observable private slot rather than an
  ordinary own property. With `Proxy`/`Reflect`/`Symbol` skips temporarily
  lifted, `built-ins/Proxy/revocable` now runs at **17 pass / 0 fail / 1
  skip** after callable Proxy support for function targets.
- **Callable Proxy `[[Call]]` support**: Proxy objects whose target is
  callable are now treated as callable by `typeof`, `IsCallable` checks, and
  ordinary function calls. Proxy calls throw on revoked proxies, forward to the
  target when `handler.apply` is nullish, and invoke callable `apply` traps
  with `(target, thisArgument, argumentsList)`. With Proxy-related skips
  temporarily lifted, `built-ins/Proxy/apply` now runs at **13 pass / 0 fail /
  1 skip**.
- **Object descriptor helpers observe Proxy descriptors**: `Object.values`,
  `Object.entries`, and `Object.getOwnPropertyDescriptors` now use the
  Proxy-aware `[[GetOwnProperty]]` path for each snapshotted key, so
  `getOwnPropertyDescriptor` traps run before enumerable filtering and
  descriptor materialization instead of being bypassed through target storage.
  With `Proxy`/`Reflect`/`Symbol` skips temporarily lifted, focused
  `built-ins/Object/{values,entries,getOwnPropertyDescriptors}` now runs at
  **59 pass / 0 fail / 0 skip** after the separate RegExp internal-slot
  exposure fix.
- **RegExp internal slots hidden from own keys**: RegExp instances now keep
  source, flags, and derived flag bits in non-observable internal storage
  rather than ordinary own properties, leaving `lastIndex` as the only default
  own string key. The public `RegExp.prototype.flags` getter still observes
  `global`/`sticky`/other property overrides through normal `Get` semantics,
  while the individual flag getters read the internal slots. This fixes
  `Object.getOwnPropertyNames`, `Reflect.ownKeys`, and
  `Object.getOwnPropertyDescriptors` order for RegExp instances without
  colliding with subclass `#private` fields.
- **Reference-preserving identifier calls through `with`**: direct
  IdentifierReference calls now retain their Reference record through the VM
  call opcode, so `with (o) { f() }` still binds `this` to `o`, while value
  expressions such as `(0, f)()`, `(cond ? f : f)()`, and `(f && f)()` lose
  the object-environment `this` binding as required. Spread and optional
  identifier calls share the same Reference-preserving path, and direct
  `eval(...)` keeps its existing intrinsic-eval behavior. The focused
  `language/statements/with` test262 run closes at **169 pass / 0 fail / 12
  skip**.
- **Optional method-call argument order**: optional member calls now resolve the
  method and short-circuit nullish method values before evaluating arguments,
  so `o.m?.(sideEffect())` and spread arguments skip side effects when `o.m` is
  `null`/`undefined` while preserving `this` for present methods. The focused
  `language/expressions/optional-chaining` test262 directory remains feature
  skipped by the runner (**38 skip**), so this edge is covered by local
  `operators` regressions.
- **Symbol computed keys for member assignments**: computed member update,
  numeric/bitwise compound assignment, and logical assignment now coerce keys
  with `ToPropertyKey` instead of `ToString`, preserving Symbol property keys
  while still evaluating the base, key, and right-hand side in spec order. The
  focused `language/expressions/{compound-assignment,logical-assignment,
  prefix-increment,postfix-increment}` test262 run closes at **532 pass / 0
  fail / 71 skip**.
- **Map/Set zero-key canonicalization**: keyed collections now normalize
  numeric `-0` to `+0` when creating internal `MapKey`s, and the `MapKey`
  hash implementation now matches SameValueZero equality for both zero signs.
  This preserves O(1) lookups while making Map replacement, Set de-duplication,
  and key iteration agree with `CanonicalizeKeyedCollectionKey`. The focused
  zero-key test262 probe now runs at **2 pass / 0 fail**.
- **Map prototype receiver brand checks**: `Map.prototype` methods now reject
  receivers without a `[[MapData]]` internal slot with `TypeError` instead of
  silently returning `undefined`, `false`, empty arrays, or the original
  receiver. The focused `built-ins/Map/prototype/{get,set,has,delete,clear,
  entries,keys,values,forEach,size}` cluster now runs at **60 pass / 11 fail /
  47 skip**, with remaining failures isolated to true MapIterator and live
  iteration semantics.
- **Set prototype size accessor and receiver brand checks**:
  `Set.prototype.size` is now a spec-shaped `"get size"` accessor instead of
  a data method, Set instance `size` reads now use ordinary prototype lookup,
  and Set prototype methods now reject receivers without `[[SetData]]` with
  `TypeError`. `Set.prototype.clear` is exposed with the same receiver
  validation. The focused `built-ins/Set/prototype/{size,add,has,delete,clear,
  entries,keys,values,forEach}` cluster now runs at **130 pass / 9 fail / 26
  skip**, with remaining failures isolated to true SetIterator and live
  iteration semantics.
- **Map/Set collection iterators and live forEach**: `Map` and `Set`
  `entries`/`keys`/`values` now return iterator objects with `next()` result
  objects instead of snapshot arrays, built-in iteration uses the same lazy
  collection iterator path, `Map.prototype[Symbol.iterator]` and
  `Set.prototype[Symbol.iterator]` reuse the spec method objects, and
  `Set.prototype.keys === Set.prototype.values`. Map/Set `forEach` now observes
  values added during iteration, skips deleted unvisited values, and revisits
  delete-then-readded values in insertion order. The focused
  `built-ins/{Map,Set}/prototype` iterator/forEach/Symbol.iterator cluster now
  runs at **104 pass / 0 fail / 38 skip**.
- **Set composition methods and constructor iterable compliance**:
  `Set.prototype.union`, `intersection`, `difference`,
  `symmetricDifference`, `isSubsetOf`, `isSupersetOf`, and `isDisjointFrom`
  are now exposed with Set-like operand handling, direct Set result
  allocation, live receiver traversal where required, iterator closing for
  early exits, Set-like size validation, and SameValueZero key semantics.
  `Array.prototype.values()` now returns an iterator object instead of an array
  snapshot, which lets Set-like `keys()` methods return array iterators without
  relaxing `GetIteratorFromMethod` validation. `new Set(iterable)` now checks
  `new.target`, observes the instance `add` method once before iteration, calls
  it for each iterated value, and closes the iterator when `add` throws. The
  focused Set composition cluster runs at **179 pass / 0 fail / 7 skip**,
  `built-ins/Set` now closes at **340 pass / 0 fail / 43 skip**, and
  `built-ins/Map built-ins/Set` improves to **432 pass / 17 fail / 138 skip**.
- **Map constructor iterable compliance and upsert methods**: `new
  Map(iterable)` now requires construction with `new`, observes the instance
  `set` method once before iterator creation, calls it for each entry pair,
  accepts array-like pair objects through ordinary property access, and closes
  the source iterator when pair access or `set` fails while preserving the
  original abrupt completion if iterator closing also throws. `Map.prototype`
  additionally exposes `getOrInsert` and `getOrInsertComputed` with
  SameValueZero key canonicalization and computed-callback overwrite
  semantics. The focused `built-ins/Map built-ins/Set` run now closes at
  **449 pass / 0 fail / 138 skip**.
- **`Map.groupBy` static grouping**: `Map.groupBy` is now exposed as a static
  built-in, iterates arbitrary sync iterables, calls the grouping callback with
  `(value, index)`, stores group keys with SameValueZero Map-key semantics
  instead of `ToPropertyKey`, returns a real Map instance, and closes custom
  iterators when the callback abruptly completes. The focused
  `built-ins/Map/groupBy` run now closes at **14 pass / 0 fail / 0 skip**.
- **Map/Set feature lift**: `Map` and `Set` are removed from the test262
  unsupported-feature skip list after the expanded `built-ins/Map
  built-ins/Set` diagnostic verifies at **473 pass / 0 fail / 114 skip**.
  The supported subset remains green while increasing to **5017 pass / 0 fail
  / 0 timeout**.
- **String well-formed Unicode methods**: `String.prototype.isWellFormed`
  and `String.prototype.toWellFormed` now follow UTF-16 surrogate-pair
  semantics, reject nullish receivers, preserve valid internal surrogate-pair
  representations, and replace unpaired surrogates with U+FFFD for
  `toWellFormed`. The focused
  `built-ins/String/prototype/isWellFormed
  built-ins/String/prototype/toWellFormed` run now closes at **14 pass / 0
  fail / 2 skip**.
- **`Array.of` constructor and property semantics**: `Array.of` now uses a
  constructable `this` value with the argument count, creates element data
  properties without invoking prototype setters, routes final `length` through
  strict `Set`, and propagates constructor/property abrupt completions.
  Test262 realms now expose a constructable `Function` constructor so
  cross-realm constructor fallback cases can run. The focused
  `built-ins/Array/of` run now closes at **14 pass / 0 fail / 2 skip**.
- **Reflect.construct `newTarget` semantics**: `Reflect.construct` now
  validates constructor-ness in spec order, builds its argument list through
  ordinary array-like property access, forwards the optional `newTarget` into
  allocation, and uses `newTarget.prototype` with `%Object.prototype%`
  fallback when it is not an object. Bound constructors preserve the caller's
  `newTarget` instead of resetting it to the bound target. With
  `Reflect`/`Reflect.construct` skips temporarily lifted, the focused
  `built-ins/Reflect/construct` diagnostic now runs at **10 pass / 0 fail**.
- **Reflect.apply array-like arguments**: `Reflect.apply` now validates
  callability before observing `argumentsList`, then builds the call argument
  list through ordinary array-like `length` and indexed property access instead
  of cloning only dense Array storage. This makes primitive or missing
  `argumentsList` values throw `TypeError`, propagates abrupt `length`/index
  gets, and accepts ordinary array-like objects and functions. With
  `Reflect`/`Symbol` skips temporarily lifted, the focused
  `built-ins/Reflect/apply` diagnostic now runs at **8 pass / 0 fail / 1
  skip**.
- **Number static method descriptors**: `Number.isFinite`,
  `Number.isInteger`, `Number.isNaN`, `Number.isSafeInteger`,
  `Number.parseInt`, and `Number.parseFloat` are now installed as writable,
  non-enumerable, configurable constructor properties, while numeric constants
  remain non-writable and non-configurable. The focused
  `built-ins/Number/{isFinite,isInteger,isNaN,isSafeInteger}` run now closes
  at **26 pass / 0 fail / 8 skip**.
- **Boolean prototype receiver checks**: `Boolean.prototype` now carries the
  wrapped `false` primitive value, `Boolean.prototype.valueOf` returns Boolean
  primitives from primitive/boxed Boolean receivers, and `valueOf`/`toString`
  reject non-Boolean receivers with `TypeError`. The focused
  `built-ins/Boolean` run now closes at **46 pass / 0 fail / 5 skip**.
- **Number/String prototype receiver checks**: `Number.prototype` and
  `String.prototype` now carry their required wrapped primitive values,
  `Number.prototype.valueOf` and `String.prototype.valueOf` reject receivers
  without matching wrapper data, and `$262.createRealm()` exposes realm-local
  primitive wrapper constructors for cross-realm TypeError checks. The focused
  `built-ins/Number/prototype/valueOf built-ins/String/prototype/valueOf` run
  now closes at **16 pass / 0 fail / 2 skip**.
- **Number prototype toString radix order**: `Number.prototype.toString` now
  validates its Number receiver before radix coercion, treats an omitted or
  explicit `undefined` radix as decimal, and propagates abrupt completions from
  radix `ToNumber` instead of silently falling back to base 10. The focused
  `built-ins/Number/prototype/toString built-ins/String/prototype/toString`
  run now closes at **95 pass / 0 fail / 2 skip**.
- **Number prototype toFixed integer conversion**: `Number.prototype.toFixed`
  now uses `ThisNumberValue`, applies `ToIntegerOrInfinity` semantics to
  `fractionDigits`, validates the range before the NaN return path, delegates
  `|x| >= 1e21` to ordinary Number stringification, and preserves the spec's
  exact fixed-point output and tie-up rounding. Number stringification now uses
  the shortest decimal for integer-valued doubles, so `toString` and
  `toFixed(0)` differ where the spec requires. The focused
  `built-ins/Number/prototype/toFixed` run now closes at **14 pass / 0 fail /
  2 skip**.
- **Number exponential/precision formatting**: `Number.prototype.toExponential`
  and `Number.prototype.toPrecision` now use `ThisNumberValue`, truncate their
  digit arguments with `ToIntegerOrInfinity`, apply the special-value return
  path after argument coercion but before range checks, normalize exponent signs,
  format `-0` as `+0`, and use exact-rational half-up decimal rounding for
  exponential notation. The broader `built-ins/Number` run now closes at **312
  pass / 0 fail / 28 skip**.
- **Math.pow NaN and infinite exponent edges**: `Math.pow` now handles
  exponent `NaN` and `abs(base) === 1` with infinite exponents before
  delegating to Rust's `powf`, while preserving the required `x ** ±0 === 1`
  behavior. The focused `built-ins/Math/pow` run now closes at **27 pass / 0
  fail / 1 skip**.
- **Math.sumPrecise**: `Math.sumPrecise` is now exposed as a unary Math
  builtin, consumes iterable Number values without coercing non-numbers, closes
  the iterator on non-number failures, preserves `NaN`, infinity, and signed
  zero semantics, and accumulates finite values through exact-rational
  summation before final IEEE-754 rounding. The focused
  `built-ins/Math/sumPrecise` run now closes at **8 pass / 0 fail / 2 skip**,
  and broader `built-ins/Math` closes at **284 pass / 0 fail / 43 skip**.
- **Number parse function identity**: `Number.parseInt` and
  `Number.parseFloat` now reference the same built-in function objects as the
  global `parseInt` and `parseFloat` properties instead of separate native
  wrappers. The broader `built-ins/Number` run now improves to **301 pass /
  11 fail / 28 skip**.
- **PrivateName lexical grammar**: private class names now use the same
  `IdentifierName` Unicode escape and raw Unicode scanning rules as ordinary
  identifiers, including `Other_ID_Start`, ZWNJ, and ZWJ handling. This fixes
  private fields, methods, and accessors whose names are spelled with
  `\uXXXX`/`\u{...}` escapes or non-ASCII source text. The focused
  private-name diagnostic now closes at **50 pass / 0 fail**.
- **Private method function identity**: instance private methods are now
  created once during class evaluation and copied into each instance private
  slot from a shared class-environment binding, instead of allocating a fresh
  function object in every constructor call. This preserves private method
  names, `super` HomeObject capture, and `this.#m` identity across instances.
  With private method skips temporarily lifted, the focused
  `language/{statements,expressions}/class/elements/private-methods`
  diagnostic now closes at **2 pass / 0 fail / 8 skip**.
- **Object/Reflect preventExtensions semantics**: Array and arguments objects
  now store their own `[[Extensible]]` state, assignment/receiver-set paths
  reject new indexed or named properties on non-extensible arrays, arguments,
  and functions, and `Object.preventExtensions`/`Reflect.preventExtensions`
  now route through Proxy `preventExtensions` traps with the correct
  throw-vs-boolean behavior. The focused `built-ins/Object/preventExtensions`
  run now closes at **36 pass / 0 fail / 4 skip**, and the adjacent
  `built-ins/{Reflect,Proxy}/preventExtensions` probe closes at **19 pass / 0
  fail / 3 skip** with skips temporarily lifted.
- **Object/Reflect isExtensible Proxy semantics**: `Object.isExtensible` now
  routes object receivers through the Proxy-aware `[[IsExtensible]]` helper,
  returns `false` for primitive receivers, and enforces Proxy trap result
  invariants against the target's actual extensibility. `Reflect.isExtensible`
  now rejects primitive targets with `TypeError` and shares the same Proxy
  trap path. With `Proxy`/`Reflect` skips temporarily lifted, the focused
  `built-ins/{Object,Reflect,Proxy}/isExtensible` probe now closes at **55
  pass / 0 fail / 3 skip**.
- **parseInt radix and large-prefix conformance**: global `parseInt` now
  applies `ToNumber`/`ToInt32` to its radix argument, so string, boxed, object,
  infinite, and modulo-2^32 radix values follow the spec. Digit accumulation no
  longer overflows through Rust integer parsing, so large valid prefixes return
  their nearest IEEE-754 Number value instead of `NaN`. The focused
  `built-ins/parseInt` run now closes at **53 pass / 0 fail / 2 skip**.
- **Math inverse hyperbolic methods**: `Math.acosh`, `Math.asinh`, and
  `Math.atanh` are now exposed as unary native functions with spec-shaped
  `name`, `length`, and own-property descriptors. They reuse the normal
  `ToNumber` unary Math path and preserve NaN, infinity, and signed-zero
  behavior through the host libm operations. The focused
  `built-ins/Math/acosh built-ins/Math/asinh built-ins/Math/atanh` run now
  closes at **14 pass / 0 fail / 3 skip**.
- **Math integer conversion and signed-zero edges**: `Math.clz32` and
  `Math.imul` now use the engine's spec-shaped `ToUint32`/`ToInt32` helpers
  instead of Rust casts, so infinities, `NaN`, modulo-2^32 values, and signed
  multiplication results match ECMAScript. `Math.sign` now preserves `NaN` and
  `-0`. The focused
  `built-ins/Math/{cbrt,clz32,cosh,expm1,fround,imul,log10,log1p,log2,sign,sinh,tanh,trunc}`
  run now closes at **68 pass / 0 fail / 13 skip**.
- **Math max/min/round edge semantics**: `Math.max` and `Math.min` now coerce
  every argument before returning `NaN`, propagate `NaN` after observable
  coercions, and apply the spec signed-zero ordering where `+0` is greater
  than `-0`. `Math.round` now preserves `-0` for `[-0.5, -0]`, returns `+0`
  for positive values below `0.5`, and keeps already-integral large Number
  values unchanged. The focused
  `built-ins/Math/{max,min,round}` run now closes at **28 pass / 0 fail / 3
  skip**.
- **String literal escape conformance**: string literals now decode UTF-8
  `NonEscapeCharacter` escapes such as `\А` as source code points instead of
  corrupting the UTF-8 tail byte, allow literal U+2028/U+2029 in strings per
  JSON-superset source text, decode sloppy legacy octal escapes, and reject
  legacy octal/non-octal decimal escapes in strict-mode strings. The focused
  `language/literals/string` run now closes at **71 pass / 0 fail / 2 skip**,
  and broader `language/literals` improves to **434 pass / 40 fail / 60 skip**.
- **RegExp quantifier early errors**: RegExp literal validation now rejects
  quantifiers that appear before any atom, including `/?/`, `/{2}/`,
  `/{2,}/`, and `/{2,3}/`, and the same validation is shared by
  `new RegExp(pattern)`. Escaped quantifier characters, character classes, and
  normal atom quantifiers such as `/a?/` and `/a{2}/` remain accepted. The
  focused `language/literals/regexp` diagnostic now runs at **144 pass / 36
  fail / 58 skip**, and broader `language/literals` improves to **438 pass /
  36 fail / 60 skip**.
- **RegExp assertion quantifier early errors**: RegExp literal validation now
  rejects quantifiers applied to lookbehind assertions in all modes and to
  lookahead assertions in Unicode mode, while preserving Annex B non-Unicode
  lookahead quantifiers at the lexical validation layer. The `RegExp`
  constructor and parser fallback path share the same validation. The focused
  `language/literals/regexp` diagnostic now runs at **156 pass / 24 fail / 58
  skip**, and broader `language/literals` improves to **450 pass / 24 fail /
  60 skip**.
- **RegExp Unicode-mode syntax early errors**: RegExp literal and constructor
  validation now reject malformed/out-of-range `\u{...}` escapes, invalid
  Unicode-mode identity/control/decimal escapes, bare `{` pattern characters,
  and character-class ranges whose endpoints are multi-character class escapes
  such as `\d` or `\s`. This closes the remaining RegExp literal
  parse-negative bucket. The focused `language/literals/regexp` diagnostic now
  runs at **168 pass / 12 fail / 58 skip**, and broader `language/literals`
  improves to **462 pass / 12 fail / 60 skip**.
- **RegExp Unicode property escape validation**: Unicode-mode RegExp literal
  and constructor validation now parse `\p{...}`/`\P{...}` bodies instead of
  accepting any non-empty ASCII name. Property-less escapes are limited to
  binary properties or `General_Category` values, so bare script names such as
  `\p{Greek}` and loose-cased names such as `\p{Ascii}` now report early
  syntax errors. Explicit `Script=...`, `Script_Extensions=...`, `gc=...`,
  binary-property aliases, and existing modifier/property escape cases remain
  accepted when the backend supports them.
- **RegExp null escapes and UTF-8 literal source**: RegExp literals now keep
  non-ASCII pattern source as Unicode code points instead of UTF-8 byte
  fragments, and the internal regex backend lowers ES `\0` null-character
  escapes to the backend-supported `\x00` form without changing the public
  `source`. `String.prototype.search` now accepts RegExp arguments for these
  probes and returns UTF-16 indices while preserving `lastIndex`. The focused
  `language/literals/regexp` diagnostic now runs at **173 pass / 7 fail / 58
  skip**, and broader `language/literals` improves to **467 pass / 7 fail /
  60 skip**.
- **RegExp sticky start assertions**: `RegExp.prototype.exec` now runs
  global/sticky matches against the full input at the UTF-16 `lastIndex`
  position instead of slicing the input first, so `^` still observes the real
  beginning of input and multiline line starts. Global `lastIndex` updates now
  use the actual match end even when the search skips ahead. The focused
  `language/literals/regexp` diagnostic now runs at **174 pass / 6 fail / 58
  skip**, and broader `language/literals` improves to **468 pass / 6 fail /
  60 skip**.
- **RegExp non-Unicode case folding**: the internal regex backend now
  protects non-ASCII literal atoms and `\uXXXX`/`\xNN` escapes from Rust's
  Unicode case folding when a pattern has `i` without `u`, while preserving
  Unicode case folding for `iu`. This matches ES canonicalization for cases
  such as Kelvin sign `\u212a`. The focused `language/literals/regexp`
  diagnostic now runs at **175 pass / 5 fail / 58 skip**, and broader
  `language/literals` improves to **469 pass / 5 fail / 60 skip**.
- **RegExp Unicode surrogate-pair escapes**: the internal regex backend now
  lowers adjacent Unicode-mode surrogate-pair escapes such as
  `\ud800\udc00` to scalar `\u{...}` backend escapes while preserving the
  public `source` text. Character classes now treat those pairs as one
  Unicode scalar instead of two independent surrogate atoms. The focused
  `language/literals/regexp` diagnostic now runs at **177 pass / 3 fail / 58
  skip**, and broader `language/literals` improves to **471 pass / 3 fail /
  60 skip**, with the remaining RegExp literal failures isolated to
  backreference support.
- **RegExp exec result shape and `lastIndex` coercion**:
  `RegExp.prototype.exec` now returns match arrays with enumerable `index`,
  `input`, and `groups` properties, treats a missing argument as
  `"undefined"`, reads `lastIndex` through ordinary `Get`/`ToLength` on every
  call, and reports `TypeError` when global/sticky `lastIndex` write-back
  fails. Lone surrogate escapes now lower to RuJa's internal surrogate sentinel
  in Unicode mode and to code-unit-aware backend atoms in non-Unicode mode, so
  `/\udf06/u` keeps scalar semantics while `/\udf06/` can match the low half of
  a surrogate pair.
- **RegExp repeated capture clearing**: `RegExp.prototype.exec` now clears
  descendant captures left over from earlier iterations of quantified
  capturing and non-capturing groups when those descendants did not participate
  in the final iteration. This matches ES repeated-capture semantics for cases
  like `/(z)((a+)?(b+)?(c))*/`, where the final optional `(b+)` capture must be
  `undefined` instead of the previous iteration's `"bbb"`, and
  `/(?:(a)|(b))*/`, where `(a)` must be cleared after the final `(b)`
  iteration. The same clearing now feeds `String.prototype.match` and function
  replacement callbacks. The focused `built-ins/RegExp/prototype/exec`
  diagnostic is **75 pass / 0 fail / 4 skip**; the broader
  `built-ins/String/prototype/{match,replace}` diagnostic now closes at
  **100 pass / 0 fail / 6 skip**.
- **String match RegExp creation and `@@match` dispatch**:
  `String.prototype.match` now follows the `@@match` dispatch path before
  ordinary matching, so custom `searchValue[Symbol.match]` getters and methods
  are observable. Values without a custom matcher are converted through a
  `RegExpCreate`-style intrinsic RegExp instead of returning `null`, and that
  internally-created RegExp observes an overridden
  `RegExp.prototype[Symbol.match]` before falling back to RuJa's internal match
  algorithm. The focused
  `built-ins/String/prototype/match` diagnostic now closes at **47 pass / 0
  fail / 4 skip**.
- **String search `@@search` dispatch and RegExp search semantics**:
  `String.prototype.search` now observes custom object
  `searchValue[Symbol.search]` getters and methods, creates an intrinsic
  RegExp for ordinary search values, and lets internally-created RegExp
  objects dispatch through an overridden `RegExp.prototype[Symbol.search]`.
  `RegExp.prototype[Symbol.search]` is now exposed with custom `exec`
  dispatch, object-or-null result validation, strict `lastIndex` writes, and
  restoration of the previous `lastIndex` after the search. The focused
  `built-ins/String/prototype/search
  built-ins/RegExp/prototype/Symbol.search` diagnostic now closes at **61
  pass / 0 fail / 5 skip**.
- **String split `@@split` dispatch and RegExp separator semantics**:
  `String.prototype.split` now has the spec `length` of 2, rejects nullish
  receivers before coercion, observes custom `separator[Symbol.split]`
  getters and methods, and propagates abrupt completions from limit coercion.
  Ordinary separators now use `ToString` in order, while `undefined`
  separators and zero limits follow the expected array result shape. RegExp
  separators now include captured substrings, honor split limits, ignore
  boundary zero-length matches, and normalize additional ES RegExp escapes for
  the backend (`[]`, `[^]`, control escapes, class backspace, and incomplete
  `\x`). The focused `built-ins/String/prototype/split` diagnostic now closes
  at **117 pass / 0 fail / 3 skip**, and the combined
  `built-ins/String/prototype/search built-ins/String/prototype/split
  built-ins/RegExp/prototype/Symbol.search` run closes at **178 pass / 0 fail
  / 8 skip**.
- **String replace substitution tokens and `@@replace` dispatch**:
  `String.prototype.replace` string replacements now expand ECMAScript
  replacement tokens (`$$`, `$&`, ``$` ``, `$'`, `$n`, `$nn`) for both RegExp
  and plain string search values instead of delegating to backend replacement
  syntax or raw `String::replacen`. Unmatched captures substitute as the empty
  string, numeric fallback cases such as `$11` and `$01` match JS behavior,
  and the replacement-string path now uses the same repeated-capture clearing
  as `RegExp.prototype.exec`. Plain replacement now also coerces `searchValue`
  before non-callable `replaceValue`, and custom `searchValue[Symbol.replace]`
  methods are observed before ordinary replacement. The focused
  `built-ins/String/prototype/replace` diagnostic now closes at **53 pass / 0
  fail / 2 skip**.
- **String replace callback offsets**: Function replacements for both
  RegExp and string search values now receive the match offset as a UTF-16
  code-unit index instead of a Rust UTF-8 byte offset, so matches after
  supplementary characters report the same offset that JS exposes through
  string indexing. The focused `built-ins/String/prototype/replace`
  diagnostic now closes at **53 pass / 0 fail / 2 skip**.
- **RegExp named capture groups**: Named captures now feed the shared match
  result surface: `RegExp.prototype.exec` and non-global
  `String.prototype.match` expose a null-prototype `groups` object,
  RegExp function replacements receive that groups object as their final
  argument, and replacement strings expand `$<name>` using the same capture
  metadata. The focused `built-ins/RegExp/prototype/exec
  built-ins/String/prototype/{match,replace}` diagnostic now closes at
  **175 pass / 0 fail / 10 skip**.
- **RegExp backreferences and identity escapes**: RegExp compilation now keeps
  the existing Rust regex fast path for ordinary patterns while routing true
  numeric backreferences through a backtracking-capable backend. Non-Unicode
  legacy decimal escapes and identity escapes that Rust regex does not accept
  are lowered to equivalent backend literals without changing public
  `source`. The focused `language/literals/regexp` diagnostic now closes at
  **180 pass / 0 fail / 58 skip**, and broader `language/literals` closes at
  **474 pass / 0 fail / 60 skip**.
- **Map prototype size accessor**: `Map.prototype.size` is now installed as
  the spec accessor property instead of a data method. The getter has the
  expected `"get size"` name/zero length, validates that the receiver is a
  real Map, and Map instance reads now go through the ordinary prototype
  lookup path so overriding or deleting `Map.prototype.size` is observable.
  The focused `built-ins/Map/prototype/size` cluster now runs at **6 pass / 0
  fail / 5 skip**.
- **RegExp literal line-terminator early errors**: regular-expression
  literals now reject CR, LF, LS, and PS immediately after a backslash instead
  of treating the line terminator as an escaped pattern character. This makes
  parse-negative literals such as `/\\\n/` stop before executing test code and
  also routes `eval()` of those literals through `SyntaxError`. The focused
  `language/literals/regexp` diagnostic now runs at
  **55 pass / 125 fail / 58 skip**, and broader `language/literals` improves
  to **324 pass / 150 fail / 60 skip**.
- **RegExp flags and modifiers early errors**: RegExp literals, parser
  recovery for statement-start regexes, and the `RegExp` constructor now share
  syntax validation for duplicate/invalid flags plus RegExp modifiers groups.
  Modifier groups only accept source-text `i`, `m`, and `s`, reject duplicate
  add/remove flags, reject add/remove intersections, require a colon, and do
  not accept Unicode escapes or case-folded flag spellings. This closes the
  focused modifiers parse-negative cluster while leaving the remaining
  `built-ins/RegExp/regexp-modifiers` failures isolated to runtime modifier
  semantics. The focused `language/literals/regexp` diagnostic now runs at
  **140 pass / 40 fail / 58 skip**, broader `language/literals` improves to
  **409 pass / 65 fail / 60 skip**, and
  `built-ins/RegExp/regexp-modifiers` runs at **37 pass / 33 fail / 0 skip**.
- **RegExp modifiers backend normalization**: the internal regex compiler now
  lowers ES modifier groups with an empty remove-list, such as `(?s-:...)`,
  to the Rust regex backend's equivalent `(?s:...)` form while preserving the
  public `source` string. Constructor validation now uses the same normalized
  compile path as execution. This closes the backend syntax failures for
  add-only modifier groups without changing modifier properties on the
  RegExp instance. The focused `built-ins/RegExp/regexp-modifiers` run now
  improves to **57 pass / 13 fail / 0 skip**.
- **RegExp modifier runtime semantics**: backend normalization now tracks
  modifier-local `s` and `i` state when lowering dot, word-boundary, word
  character, and Unicode property escapes. Non-Unicode `.` now follows ES
  UTF-16 code-unit semantics instead of Rust scalar matching, local
  `(?-i:...)` word escapes use the ES ASCII word set inside and outside
  character classes, and modifier-local `\p{Lu}`/`\P{Lu}` probes plus their
  `Uppercase_Letter` aliases compile both inside and outside character classes
  in Unicode mode. The focused
  `built-ins/RegExp/regexp-modifiers` run now closes at **70 pass / 0 fail /
  0 skip**.
- **RegExp prototype accessors**: `RegExp.prototype.source` and
  `RegExp.prototype.flags` are now accessor properties with spec-shaped
  getter functions. RegExp instances keep their raw pattern and flags in
  internal storage, while the public `source` getter escapes empty patterns,
  slashes, and line terminators for literal reconstruction and the `flags`
  getter reads boolean flag accessors in `dgimsuvy` order. `$262.createRealm()`
  now exposes a realm-local `RegExp` intrinsic with accessor getters bound to
  that realm, so `%RegExp.prototype.source%` accepts only its own realm
  prototype. The focused `built-ins/RegExp/prototype/flags
  built-ins/RegExp/prototype/source` run now closes at **18 pass / 0 fail / 10
  skip**.
- **Proxy-aware `[[HasProperty]]` for `with`/Reflect**:
  internal property-existence checks now route through Proxy `has` traps,
  including revoked-proxy errors and basic non-configurable/non-extensible
  target invariants. `with` object-environment binding lookup, the `in`
  operator, `Array.from` iterator detection, async iterator detection, and
  `Reflect.has` now share the same observable `[[HasProperty]]` path.
  Symbol-key Proxy `get` is used for `Symbol.unscopables`, and
  `Reflect.get`/`set`/`has` now preserve Symbol property keys. `Reflect.set`
  also honors its receiver argument enough for Proxy receivers to observe
  `getOwnPropertyDescriptor`/`defineProperty` during ordinary data-property
  writes, returns `false` instead of `true` for receiver/target
  non-writable failures, and propagates abrupt completions from Proxy `set`
  traps. `Reflect.defineProperty` and `Reflect.getOwnPropertyDescriptor` are
  exposed for this path; `Reflect.defineProperty` now returns `false` for
  failed ordinary definitions instead of throwing, propagates abrupt
  completions while reading descriptor fields, and
  `Reflect.getOwnPropertyDescriptor` now observes Proxy
  `getOwnPropertyDescriptor` trap completions. With the
  `Proxy`/`Reflect` skips temporarily lifted, focused
  `language/statements/with built-ins/Reflect/has` now runs at
  **183 pass / 0 fail / 8 skip**, and the focused
  `built-ins/Reflect/get built-ins/Reflect/set built-ins/Reflect/has
  built-ins/Reflect/defineProperty
  built-ins/Reflect/getOwnPropertyDescriptor` diagnostic now runs at
  **49 pass / 0 fail / 15 skip**.
- **Proxy `set` trap failure propagation for `with` References**: Proxy
  `[[Set]]` now checks the `set` trap's boolean result instead of discarding
  it. Strict `PutValue` through a Proxy-backed `with` object now throws
  `TypeError` when the trap returns a falsy value, while sloppy assignment
  remains a silent failed write. This covers simple, compound, update, and
  logical assignment forms that preserve the same object-environment
  Reference. The focused `language/statements/with` run remains at **169 pass
  / 0 fail / 12 skip**.
- **Proxy-aware `[[Delete]]` for delete/Reflect**:
  property deletion now routes through Proxy `deleteProperty` traps with the
  handler as `this`, preserves string and Symbol property keys, falls through
  to nested proxy targets when the trap is null or missing, and enforces the
  non-configurable/non-extensible target invariants for truthy trap results.
  `Reflect.deleteProperty` now rejects primitive targets and returns the
  actual internal `[[Delete]]` boolean instead of always returning `true`.
  `Proxy.revocable()` now revokes through the native callee rather than the
  call receiver, so revoked proxy deletes throw. The test262
  `$262.createRealm()` host now also exposes the constructable `Proxy`
  constructor on the created global. With `Proxy`, `Reflect`, and
  `proxy-missing-checks` skips temporarily lifted, focused
  `built-ins/Reflect/deleteProperty built-ins/Proxy/deleteProperty` now runs
  at **25 pass / 0 fail / 3 skip**.
- **Array search array-like access**: `Array.prototype.indexOf`,
  `lastIndexOf`, and `includes` now use `LengthOfArrayLike` plus per-index
  property access instead of scanning only RuJa's dense array storage.
  Generic calls on ordinary array-like objects, Boolean/Number primitives
  with prototype `length`/index properties, boxed strings, sparse arrays, and
  holes now follow the expected `HasProperty`/`Get` behavior. Array
  `length` shrinkage now preserves non-configurable indexed own properties,
  so searches still observe those elements after accessor side effects try to
  shorten the receiver. The focused
  `built-ins/Array/prototype/includes
  built-ins/Array/prototype/indexOf
  built-ins/Array/prototype/lastIndexOf` cluster now runs at
  **409 pass / 0 fail / 20 skip**.
- **Array find array-like access**: `Array.prototype.find`, `findIndex`,
  `findLast`, and `findLastIndex` now share the spec order for
  `ToObject(this)`, `LengthOfArrayLike`, predicate callability checks, and
  per-index `Get`. They no longer clone dense Array storage before iteration,
  so array-like receivers, nullish receiver errors, throwing `length`/index
  accessors, callback `thisArg`, holes as `undefined`, and mutations during
  traversal are observable. The focused
  `built-ins/Array/prototype/{find,findIndex,findLast,findLastIndex}`
  diagnostic improves from **38 pass / 24 fail / 32 skip** to **62 pass / 0
  fail / 32 skip**, and the combined Array search/find run closes at
  **471 pass / 0 fail / 52 skip**.
- **Array `at` array-like access**: `Array.prototype.at` now applies
  `ToObject(this)` with nullish receiver errors, reads `length` through
  `LengthOfArrayLike`, uses indexed property access for generic array-like
  receivers, and normalizes `-0` to property key `"0"`. The focused
  `built-ins/Array/prototype/at` run now closes at **11 pass / 0 fail / 2
  skip**.
- **String search argument coercion**: `String.prototype.indexOf` and
  `lastIndexOf` now coerce `searchString` through `ToString` before reading
  the position argument, so missing arguments search for `"undefined"`, object
  search values run observable `toString` first, and abrupt completions occur
  in spec order. `indexOf` positions now clamp negative values to 0 instead
  of using Array-style from-index wrapping, and `lastIndexOf` clamps finite
  negative values to 0 while preserving the `NaN`/`+Infinity` search-from-end
  path. The UTF-16 last-index helper also handles needles longer than the
  haystack without panicking. The focused
  `built-ins/String/prototype/indexOf
  built-ins/String/prototype/lastIndexOf` cluster now runs at
  **62 pass / 0 fail / 10 skip**.
- **String slice/substring argument coercion**:
  `String.prototype.slice` and `substring` now coerce start/end arguments
  through `ToIntegerOrInfinity` in spec order. `slice` now observes object
  `valueOf`/`toString`, propagates abrupt completions from `start` before
  `end`, and treats explicit `undefined` end as the string length.
  `substring` now truncates fractional positions and also treats missing or
  explicit `undefined` end as the string length before clamping/swapping. The
  Math intrinsic object is now extensible like an ordinary ECMAScript object,
  so borrowed string methods assigned onto `Math` are callable. The
  focused
  `built-ins/String/prototype/slice
  built-ins/String/prototype/substring` cluster now runs at
  **80 pass / 0 fail / 4 skip**.
- **String trim whitespace set**: `String.prototype.trim`, `trimStart`, and
  `trimEnd` now use the ECMAScript `WhiteSpace` plus `LineTerminator` set
  instead of Rust's host whitespace predicate, so BOM (`\uFEFF`) is trimmed
  at string boundaries while non-ECMAScript whitespace such as `\u180E` and
  `\u0085` is preserved. RegExp objects constructed from RegExp inputs now
  retain the wrapped pattern source/flags, and arguments objects now stringify
  through their object brand instead of RuJa's internal array storage. The
  focused
  `built-ins/String/prototype/trim
  built-ins/String/prototype/trimStart
  built-ins/String/prototype/trimEnd` cluster now runs at
  **145 pass / 0 fail / 30 skip**.
- **String repeat count coercion**: `String.prototype.repeat` now applies
  `ToIntegerOrInfinity`-style truncation to its count before range checking,
  so `NaN`, `undefined`, `false`, `"0"`, and `0.9` produce the empty string
  instead of throwing, while negative counts and infinities still throw
  `RangeError`. The focused `built-ins/String/prototype/repeat` cluster now
  runs at **13 pass / 0 fail / 3 skip**.
- **String index position coercion**: `String.prototype.charAt`,
  `charCodeAt`, and `codePointAt` now coerce explicit `undefined`, `NaN`,
  non-numeric strings, fractional values, and infinities through the shared
  integer-position path before range checking. This preserves index-0 access
  for `undefined`/`NaN` while keeping negative, infinite, and out-of-range
  positions on the empty/`NaN`/`undefined` result paths. The focused
  `built-ins/String/prototype/charAt
  built-ins/String/prototype/charCodeAt
  built-ins/String/prototype/codePointAt` cluster now runs at
  **66 pass / 0 fail / 5 skip**.
- **Symbol intrinsic surface completion**: `Symbol.length` now has the spec
  value/descriptor, `Symbol.prototype.valueOf` is exposed with Symbol wrapper
  validation, `Object.getPrototypeOf(Symbol())` returns `Symbol.prototype`,
  and nullish `Object.getPrototypeOf` inputs throw `TypeError`. The remaining
  well-known Symbol constructor properties
  (`isConcatSpreadable`, `matchAll`, `replace`, `search`, `split`, plus the
  existing well-known properties) are now installed as non-writable,
  non-enumerable, non-configurable data properties with stored descriptions.
  Array, Map, Promise, RegExp, and Set now expose named
  `get [Symbol.species]` accessors that return the receiver, so subclass
  species lookup follows the inherited accessor path. With the `Symbol`
  feature skip temporarily lifted, the whole `built-ins/Symbol` diagnostic now
  runs at **67 pass / 0 fail / 31 skip**.
- **`new.target` eval-context early errors**: `new.target` is now rejected in
  script/global code, indirect eval code, and direct eval code reached through
  arrow-function code, while direct eval inside non-arrow function code sees
  the caller's active `new.target`. Function parameter defaults also parse
  `new.target` in the same ordinary-function context. The focused
  `language/global-code language/eval-code` cluster now runs at
  **331 pass / 0 fail / 58 skip**.
- **Symbol description/keyFor registry semantics**: Symbols now retain
  optional descriptions, well-known Symbols expose spec-style descriptions,
  `Symbol.prototype.description` and `Symbol.keyFor` are implemented with
  the right primitive/wrapper receiver validation, and `String(Symbol(...))`
  / `Symbol.prototype.toString` include descriptions. Test262-created realms
  now receive distinct `Symbol`, `Symbol.for`, and `Symbol.keyFor` function
  objects while sharing the VM-level global Symbol registry, closing the
  cross-realm registry cases. Sloppy writes to coercible Symbol primitives are
  ignored while strict writes still throw, and nullish member assignments keep
  throwing in sloppy mode. Focused
  `built-ins/Symbol/for built-ins/Symbol/keyFor
  built-ins/Symbol/prototype/description built-ins/Symbol/prototype/toString`
  with the `Symbol` feature skip temporarily lifted runs at **28 pass / 0
  fail / 4 skip**.
- **`Reflect.ownKeys` Symbol key and Proxy abrupt-completion coverage**:
  `Reflect.ownKeys` now rejects primitive targets with `TypeError`, returns
  the full `[[OwnPropertyKeys]]` list by preserving non-enumerable string keys
  and Symbol keys in spec order, and propagates abrupt completions from Proxy
  `ownKeys` trap result conversion instead of falling back to target keys.
  `Symbol.for` now uses a VM-level global symbol registry for repeat-key
  identity, which closes the Symbol-backed `Reflect.ownKeys` ordering case.
  Focused `built-ins/Reflect/ownKeys` with `Proxy`/`Reflect`/`Symbol` feature
  skips temporarily lifted now runs at **13 pass / 0 fail / 0 skip**.
- **Destructuring assignment Reference target preservation**: identifier
  targets in object and array destructuring assignments now capture the
  spec Reference before reading the source property, stepping the iterator, or
  evaluating a default initializer. This keeps `with` object-environment
  targets stable even when a getter, iterator step, or default expression
  deletes the selected property before `PutValue`. The focused Reference
  cluster
  `language/statements/with language/expressions/assignment
  language/expressions/destructuring language/expressions/compound-assignment
  language/expressions/logical-assignment language/expressions/delete` runs at
  **904 pass / 0 fail / 363 skip**, and the supported subset remains
  **5003 pass / 0 fail / 0 timeout**.
- **Object assignment shorthand defaults**: object-literal cover grammar now
  accepts shorthand default forms such as `{ x = 1 }` long enough for simple
  destructuring assignment (`{ x = 1 } = rhs`) to consume them as assignment
  patterns, while ordinary object literals and compound assignments still
  reject the form as a `SyntaxError`. With diagnostic feature skips
  temporarily lifted, `language/expressions/assignment
  language/statements/for-in language/statements/for-of` improves from
  **1009 pass / 220 fail / 122 skip** to
  **1022 pass / 207 fail / 122 skip**.
- **Object prototype receiver coercion**: `Object.prototype.valueOf` now
  applies `ToObject(this)` and rejects nullish receivers, so primitive
  receivers produce wrapper objects while detached calls throw `TypeError`.
  `Object.prototype.toLocaleString` now performs the observable
  `Invoke(this, "toString")` path instead of aliasing
  `Object.prototype.toString`, preserving primitive receivers for strict
  user-defined `toString` methods and propagating accessor/call failures.
  The focused `built-ins/Object/prototype/valueOf
  built-ins/Object/prototype/toLocaleString` cluster now runs at
  **30 pass / 0 fail / 2 skip**, closing 10 full-suite failures.
- **Object legacy accessor methods**: `Object.prototype.__defineGetter__`,
  `__defineSetter__`, `__lookupGetter__`, and `__lookupSetter__` are now
  installed with spec-shaped `name`/`length` descriptors. The define methods
  apply the required `ToObject`/callable/key-coercion order, define enumerable
  configurable accessors through the ordinary `DefinePropertyOrThrow` path,
  and preserve an existing complementary getter or setter. The lookup methods
  walk ordinary prototype chains and return the first accessor getter/setter,
  or `undefined` for data properties and missing accessors. The focused
  legacy accessor cluster now runs at **42 pass / 0 fail / 12 skip**.
- **Object prototype `__proto__` accessor and prototype mutation**:
  `Object.prototype` now has a null `[[Prototype]]` and the Annex B
  `__proto__` accessor with named getter/setter functions. Ordinary
  `__proto__` access now flows through the inherited accessor instead of a
  VM-wide shortcut, so null-prototype objects and own data properties shadow
  it correctly. `Object.setPrototypeOf`, `Reflect.setPrototypeOf`, and the
  legacy setter share the same prototype mutation status path, rejecting
  immutable `Object.prototype`, non-extensible targets, and ordinary cycles
  while allowing the Proxy-shadowed cycle case required by test262.
  `Object.prototype.isPrototypeOf` now follows the specified nullish receiver
  order. The focused `built-ins/Object/prototype` run now closes at
  **191 pass / 0 fail / 57 skip**.
- **`Object.assign` target/source semantics**: `Object.assign` now applies
  `ToObject` to primitive targets, skips nullish sources, copies enumerable
  string and Symbol keys in own-key order, and throws `TypeError` when the
  required `Set(..., Throw=true)` operation fails. Property-key based
  `[[Get]]`/`[[Set]]` now observes array dense elements, string exotic
  indices/`length`, and array receiver index/length writes, while Proxy
  `ownKeys` trap order is preserved for normal array key-list results. The
  focused `built-ins/Object/assign` run now closes at
  **25 pass / 0 fail / 13 skip**.
- **`Object.fromEntries` entry coercion**: `Object.fromEntries` now rejects
  nullish iterables, requires each entry value to be an object, reads
  `entry[0]`/`entry[1]` through ordinary property access instead of only
  unpacking array storage, and preserves Symbol property keys via
  `ToPropertyKey`. Boxed string entries such as `Object("ab")` now create
  `{ a: "b" }`, while primitive string entries throw `TypeError`. The focused
  `built-ins/Object/fromEntries` run now closes at
  **11 pass / 0 fail / 14 skip**.
- **`Object.groupBy` static grouping**: `Object.groupBy` is now exposed as a
  static built-in, iterates arbitrary sync iterables, calls the grouping
  callback with `(value, index)`, converts callback results with
  `ToPropertyKey`, preserves Symbol group keys, returns a null-prototype
  result object, and closes custom iterators when callback/key coercion
  abruptly completes. The focused `built-ins/Object/groupBy` run now closes at
  **13 pass / 0 fail / 1 skip**.
- **Native Error constructor shape**: Native Error constructors now inherit
  from `%Error%` instead of directly from `%Function.prototype%`, expose own
  non-enumerable `name`/`length` properties, and keep their prototype objects
  as ordinary objects rather than Error-branded instances. The focused
  `built-ins/Object/getPrototypeOf built-ins/NativeErrors` run now closes at
  **118 pass / 0 fail / 15 skip**.
- **Promise built-in surface expansion**: `Symbol.species` is now exposed,
  `String(Symbol(...))` follows the special `String` constructor path instead
  of ordinary `ToString`, and `Promise` exposes `all`, `race`, `allSettled`,
  `any`, `try`, `withResolvers`, `prototype.finally`, and the
  `Promise[@@species]` accessor. Promise resolve/reject functions created by
  the constructor are now anonymous unary built-ins with the expected
  `length`/`name` descriptors and no own `prototype`. Static
  `Promise.resolve` and `Promise.reject` now create and invoke a
  `NewPromiseCapability` from their receiver constructor, so subclass/custom
  constructor capabilities and bad receivers follow the spec path.
  `Promise.prototype.catch` and `Promise.prototype.finally` now invoke the
  receiver's observable `then` property instead of bypassing it through RuJa's
  internal Promise path. `Promise.prototype.then` now validates its receiver
  as a real Promise, resolves the derived promise through
  `SpeciesConstructor`, and stores Promise reaction capabilities so custom
  species constructors and capability executor validation follow the spec path.
  The `Promise` constructor now rejects calls made without `new` and invokes
  its executor with `undefined` as the receiver, letting ordinary sloppy
  functions see `globalThis` while strict executors preserve `undefined`.
  `Promise.try` now creates its result through `NewPromiseCapability(this)`,
  so subclass/custom receivers, constructor abrupt completions, and
  non-constructor receiver validation follow the spec path. Class computed
  method and accessor names now use `ToPropertyKey`, and method definition
  preserves Symbol keys, so `static get [Symbol.species]` defines the
  well-known Symbol property instead of a string-named property. Promise
  reactions that return an already-settled Promise now schedule direct
  pass-through adoption instead of storing an undrainable handler, avoiding
  hangs while preserving pending-Promise adoption.
  `Promise.race` now constructs through the receiver capability, reads
  `C.resolve` once, and invokes each resolved entry's observable `then` with
  the capability resolve/reject functions. `Promise.all` now follows the same
  constructor capability and `C.resolve` path, creates per-element resolving
  functions, invokes each resolved entry's observable `then`, and resolves the
  outer capability with the ordered result array. `Promise.allSettled` now
  follows the same constructor capability and `C.resolve` path, creates paired
  per-element resolve/reject functions sharing an `alreadyCalled` guard,
  records ordered fulfilled/rejected result objects, and rejects the outer
  capability if the final capability resolve abruptly completes. `Promise.all`
  also now rejects its outer capability if the final capability resolve
  abruptly completes. `Promise.any` now follows the receiver constructor
  capability and `C.resolve` path, invokes observable `then`, tracks
  per-element rejection functions with `alreadyCalled` guards, preserves
  rejection order, and rejects with a minimal `AggregateError` carrying a
  non-enumerable `errors` array. `Promise.allKeyed` and
  `Promise.allSettledKeyed` are now exposed for the `await-dictionary`
  proposal surface: they construct through the receiver capability, read
  `C.resolve` once, enumerate own enumerable string and Symbol keys, preserve
  key order independently of settlement order, invoke each resolved entry's
  observable `then`, and resolve to a null-prototype keyed result object.
  The diagnostic `built-ins/Promise` run with only the `Promise` feature skip
  lifted is now **255 pass / 0 fail / 0 timeout / 448 skip**. Focused
  `built-ins/Promise/allKeyed built-ins/Promise/allSettledKeyed` runs at
  **18 pass / 0 fail / 45 skip**, `built-ins/Promise/any` runs at
  **26 pass / 0 fail / 68 skip**, and the
  `all`/`race`/`allSettled`/`any`/`resolve` diagnostic cluster runs at
  **136 pass / 0 fail / 284 skip**. The Promise skip remains in the supported
  runner until the broader skipped async/proposal coverage is intentionally
  lifted.
- **`super`/`for-of` feature lift**: method parameter default initializers now
  preserve the enclosing method's `super` property parse context while still
  rejecting direct `super()` calls, and non-declaration
  `for ([x] of iterable)` / `for ({x} of iterable)` heads now use the existing
  destructuring-assignment compiler path instead of discarding the iterator
  value. `super` and `for-of` are removed from the test262 skip filters after
  focused verification over `language/statements/for-of` and object method
  definitions ran at **134 pass / 0 fail / 920 skip**, raising the supported
  subset to **5003 pass / 0 fail / 0 timeout**.
- **ES2015 syntax/global feature lift**: `computed-property-names`,
  `rest-parameters`, `object-spread`, and `globalThis` are removed from the
  test262 skip filters after the supported subset verified at 0 failures. The
  focused computed/object cluster runs at **370 pass / 0 fail / 848 skip**,
  the call/new/array/super spread cluster at **217 pass / 0 fail / 80 skip**,
  and the class/function/arrow cluster at **1117 pass / 0 fail / 8367 skip**.
  This raises the supported subset to **5000 pass / 0 fail / 0 timeout**.
- **Class feature lift**: `class` is removed from the test262 skip filters
  after tightening class numeric method/accessor names, `static constructor`
  parsing, class-element early errors, and dynamic `super()` constructor
  lookup through the active class constructor's current `[[Prototype]]`.
  `super(...)` now evaluates arguments before the not-a-constructor check,
  including spread calls, so catchable TypeErrors match test262 ordering. The
  focused class directories run at **522 pass / 0 fail / 7904 skip**, raising
  the supported subset to **4741 pass / 0 fail / 0 timeout**.
- **Thrown custom object display**: uncaught ordinary objects created by custom
  constructors now include their prototype constructor name in the host error
  message, preserving test262's `Test262Error` signal without changing the
  caught thrown value. This closes the remaining `language/line-terminators`
  failures, raising that shard to **41 pass / 0 fail / 0 skip**.
- **Statement-list regex literal recovery**: parser primary-expression
  handling now recovers regular expression literals that the eager lexer can
  only tokenize as `/` after a preceding block-like statement boundary. This
  closes the `language/statementList` regex-literal failures, raising that
  shard to **60 pass / 0 fail / 20 skip**.
- **Block-scope declaration early errors**: block statement-list early-error
  checks now treat block-level function declarations as lexical declarations
  and include nested statement `var` names in a block's `VarDeclaredNames`.
  `for-in`/`for-of` declaration heads now also reject multiple declarators.
  This closes the focused `language/block-scope` failures, raising that shard
  to **94 pass / 0 fail / 51 skip**.
- **Escaped reserved-word early errors**: identifiers containing Unicode
  escapes now remain identifier-name tokens instead of being promoted to
  keyword/literal tokens, and reserved words such as escaped `true`, `false`,
  `null`, or `var` are rejected in identifier-reference, binding, shorthand,
  and label positions. Escaped reserved words still work as property names.
  The focused
  `language/literals/boolean language/literals/null language/reserved-words
  language/keywords language/future-reserved-words` cluster now runs at
  **113 pass / 0 fail / 1 skip**, and `language/literals` improves to
  **315 pass / 159 fail / 60 skip**.
- **Destructuring assignment feature lift**: object/array destructuring
  assignment patterns now reject escaped reserved words when they would become
  binding identifiers, including shorthand object assignment properties and
  arrow/function destructuring parameters, while escaped reserved words remain
  valid property names in renamed patterns. `destructuring-assignment` is
  removed from the test262 skip filters at **135 pass / 0 fail / 6 skip**,
  raising the supported subset to **4470 pass / 0 fail / 0 timeout**.
- **`with` object-environment HasBinding**: `with` statements now box
  primitive binding objects with `ToObject` after the nullish TypeError check,
  and object-environment binding lookup uses `[[HasProperty]]` over the
  prototype chain instead of own-property checks. Inherited `with` properties
  now resolve for reads, calls, assignments, and compound assignments, while
  primitive strings expose `length` inside `with`. The focused
  `language/statements/with language/expressions/assignment
  language/expressions/prefix-increment language/expressions/prefix-decrement
  language/expressions/postfix-increment
  language/expressions/postfix-decrement` cluster runs at **398 pass / 0 fail
  / 410 skip**.
- **`with` `@@unscopables` HasBinding**: `Symbol.unscopables` is now exposed
  on the `Symbol` constructor, and `with` object environment records consult
  it after a successful `[[HasProperty]]` check. Object-valued unscopables can
  hide bindings, primitive unscopables values are ignored, abrupt getters
  propagate, and strict reads/writes re-check properties deleted by the
  unscopables getter. This closes `language/statements/with` at **169 pass / 0
  fail / 12 skip** and moves the Reference-focused with/assignment/inc/dec
  cluster to **409 pass / 0 fail / 399 skip**.
- **`delete` through `with` object environments**: identifier deletion now
  routes `with` object environment records through the same `[[HasProperty]]`
  and `Symbol.unscopables` HasBinding logic used by reads and writes before
  applying ordinary property deletion. This preserves inherited `with`
  bindings, leaves unscopables-hidden properties untouched while falling
  through to outer bindings, and propagates abrupt unscopables getters. The
  focused `language/statements/with language/expressions/delete` run stays at
  **235 pass / 0 fail / 15 skip**, and the broader Reference-focused delete
  cluster runs at **404 pass / 0 fail / 409 skip**.
- **`typeof` through `with` object environments**: `typeof identifier` now
  creates the same spec Reference record as ordinary identifier evaluation
  before applying `GetValue`, so `with` object properties, inherited
  properties, `Symbol.unscopables`, abrupt unscopables getters, and TDZ
  bindings are all observed correctly. The focused
  `language/statements/with` run stays at **169 pass / 0 fail / 12 skip**,
  while the broader Reference-focused cluster now runs at **900 pass / 0 fail
  / 367 skip**.
- **Identifier writes through destructuring and `for-in`/`for-of` heads**:
  destructuring-assignment identifier targets and non-declaration
  `for-in`/`for-of` identifier heads now create a spec Reference record before
  `PutValue`, matching ordinary assignment. This preserves `with`
  object-environment `[[HasProperty]]`, inherited binding, and
  `Symbol.unscopables` behavior instead of writing through the current
  environment directly.
- **Direct eval through `with` object environments**: unqualified `eval(...)`
  calls now resolve the callee at runtime before deciding whether the call is
  direct eval. A `with` object can shadow `eval` with an ordinary function,
  abrupt `eval` getters propagate before argument evaluation, and
  `with ({ eval }) { eval(src) }` still stays direct when the resolved value
  is the current Realm's intrinsic `%eval%`. The focused
  `language/statements/with` run stays closed at **169 pass / 0 fail / 12
  skip**, and the supported subset remains at **4276 pass / 0 fail / 16162
  skip** while closing this untracked Reference/eval edge.
- **Private-field assignment targets**: private-field update, compound, and
  logical assignments now preserve the evaluated private reference base instead
  of re-evaluating the object expression or only returning the computed value.
  `obj.#x++`, `obj.#x += y`, and `obj.#x ||= y` now update the private slot or
  accessor through the same object, and logical short-circuit paths keep the
  existing value as the expression result. The focused
  `language/expressions/compound-assignment
  language/expressions/logical-assignment language/expressions/update` run is
  **463 pass / 0 fail / 69 skip**; private class feature tests remain skipped
  by the runner, so local class regression tests cover this edge.
- **Private-name delete early errors**: strict/class code now rejects
  `delete obj.#x` and covered forms such as `delete (g().#x)` at parse time
  instead of compiling them and reaching `$DONOTEVALUATE()`. With private class
  feature skips temporarily lifted, the focused
  `language/statements/class/elements/syntax/early-errors/delete
  language/expressions/class/elements/syntax/early-errors/delete` diagnostic
  improves from **0 pass / 48 fail / 144 skip** to **48 pass / 0 fail / 144
  skip**, and the broader private class early-error diagnostic is now
  **136 pass / 60 fail / 248 skip**. The default supported-subset count is
  unchanged because those private-feature tests remain skipped.
- **Private-bound-name early errors**: class parsing now rejects private names
  named `#constructor` and duplicate private bound names across static and
  instance elements, while still allowing the spec's one private getter plus
  one private setter exception. With private class feature skips temporarily
  lifted, the broader private class early-error diagnostic improves from
  **136 pass / 60 fail / 248 skip** to **162 pass / 34 fail / 248 skip**. The
  default supported-subset count is unchanged because those private-feature
  tests remain skipped.
- **Private-name reference early errors**: the parser now applies
  `AllPrivateNamesValid` after building the AST, so class methods, nested
  functions, nested classes, static blocks, computed names, and initializers
  reject undeclared private-name references while preserving lexical access to
  outer class private names. `super.#x` is rejected as a syntax error. With
  private class feature skips temporarily lifted, the broader private class
  early-error diagnostic improves from **162 pass / 34 fail / 248 skip** to
  **196 pass / 0 fail / 248 skip**. The default supported-subset count is
  unchanged because those private-feature tests remain skipped.
- **Private method function names**: private methods now compile their function
  `name` property with the spec `#name` display form instead of the bare
  identifier while keeping the internal private slot key unchanged. With
  private class feature skips temporarily lifted over
  `language/statements/class language/expressions/class`, the diagnostic
  improves from **934 pass / 170 fail / 7322 skip** to **936 pass / 168 fail /
  7322 skip**. The default supported-subset count is unchanged because those
  private-feature tests remain skipped.
- **Private async/generator method heads**: class bodies now parse private
  `async #name()`, `* #name()`, and `async * #name()` method heads, including
  static forms, so they preserve their async/generator flags while using the
  private method lowering path. With private class feature skips temporarily
  lifted over `language/statements/class language/expressions/class`, the
  diagnostic improves from **936 pass / 168 fail / 7322 skip** to **948 pass /
  156 fail / 7322 skip**. The default supported-subset count is unchanged
  because those private-feature tests remain skipped.
- **Strict destructuring assignment targets**: strict-mode destructuring
  assignment patterns now reject `eval` and `arguments` targets recursively,
  including non-declaration `for-in`/`for-of` heads. With the
  `destructuring-binding` diagnostic temporarily lifted over
  `language/expressions/assignment language/statements/for-in
  language/statements/for-of`, the result improves from **1003 pass / 226 fail /
  122 skip** to **1009 pass / 220 fail / 122 skip**. The default
  supported-subset count is unchanged because destructuring-binding tests
  remain skipped.
- **Class static block feature lift**: class static blocks now parse as
  dedicated static initialization blocks instead of ordinary function bodies,
  so `return` is rejected, `super.prop` is accepted, and static-block early
  errors reject direct `await`, `yield`, `arguments`, and duplicate labels
  without crossing function/static-block boundaries. Class methods now carry
  async-method metadata through compilation. The `class-static-block` feature
  is removed from the test262 skip filters; the supported subset moves to
  **4335 pass / 0 fail / 16103 skip**.
- **Arrow lexical `new.target`**: arrow closures now capture their enclosing
  frame's `new.target` at creation time and reuse it when executing later,
  including arrows returned from constructors. `optional-catch-binding` and
  `new.target` are now removed from the test262 skip filters. The focused
  `language/statements/try language/expressions/new.target
  language/expressions/arrow-function` cluster runs at **204 pass / 0 fail /
  354 skip**, and the supported subset moves to **4215 pass / 0 fail /
  16223 skip**.
- **`for-in-order` enumeration**: `for...in`, JSON object serialization, and
  JSON reviver traversal now use ES own-property ordering: array-index keys in
  ascending numeric order followed by string keys in insertion order.
  `Object.create(proto, descriptors)` now applies its descriptor map, so
  non-enumerable own properties correctly shadow inherited enumerable keys.
  The `for-in-order` feature is now removed from the test262 skip filters; its
  9 metadata tests run at **9 pass / 0 fail**, and the supported subset moves
  to **4219 pass / 0 fail / 16219 skip**.
- **Logical-assignment feature lift**: member logical assignments now perform
  the nullish-base `ToObject` check after evaluating the computed property
  expression but before `ToPropertyKey`, and identifier logical assignments
  now apply NamedEvaluation to anonymous function, arrow, and class RHS values.
  `logical-assignment-operators` is removed from the test262 skip filters; the
  focused `language/expressions/logical-assignment` directory runs at **57
  pass / 0 fail / 21 skip**, the Reference-focused with/assignment/logical
  assignment/update cluster runs at **338 pass / 0 fail / 406 skip**, and the
  supported subset moves to **4276 pass / 0 fail / 16162 skip**.
- **Mapped arguments exotic descriptors**: non-strict arguments objects now
  use `Object.prototype`, expose `length` as a configurable ordinary data
  property, report `Array.isArray(arguments) === false`, and keep mapped
  parameter bindings synchronized through descriptor redefinitions until an
  accessor descriptor or non-writable data descriptor unmaps the index.
  Computed deletion of arguments properties now shares the same
  configurability checks as direct deletion, and accessor indices no longer
  fall through to dense element storage when no getter is present. Sloppy
  function `caller` lookup now supports the Annex B call-stack path needed by
  `arguments.callee.caller`, while strict callers remain restricted. Member
  calls with spread arguments now preserve their receiver and spread arity.
  The focused `language/arguments-object` cluster now runs at **126 pass / 0
  fail / 137 skip**.
- **Logical-assignment Reference preservation**: identifier logical
  assignments (`&&=`, `||=`, `??=`) now keep the original spec Reference from
  `GetValue` through `PutValue`, so a `with` or global-object property deleted
  by the RHS is written back through the original reference instead of
  re-resolving to an outer binding. Member logical assignments now also clean
  up their saved target pair on short-circuit paths, preserving the existing
  value as the expression result. After the feature lift above, the focused
  `language/statements/with language/expressions/assignment
  language/expressions/logical-assignment language/expressions/update` cluster
  runs at **338 pass / 0 fail / 406 skip** with additional regression
  coverage for these Reference edges.
- **Strict directive and future-reserved-word early errors**: sloppy bindings
  may now use strict-only future reserved words such as `implements`,
  `interface`, `package`, `private`, `protected`, `public`, `static`, and
  `yield`, while `enum` remains always reserved and strict binding/
  identifier-reference positions reject the full strict-only set. String
  literal tokens now remember whether they contained an escape sequence or line
  continuation, so escaped `"use strict"` spellings no longer create strict
  mode and Function-constructor/direct-eval strict bodies report `SyntaxError`
  for reserved identifier references. The focused
  `language/future-reserved-words language/directive-prologue` cluster now
  runs at **117 pass / 0 fail / 0 skip**.
- **Identifier Unicode tables and reserved binding names**: identifier lexing
  now uses Unicode identifier property tables with the ES `$`, `_`, ZWNJ/ZWJ,
  and grandfathered `Other_ID_Start`/`Other_ID_Continue` additions, while
  invalid Pattern_Syntax characters such as U+2E2F surface as `SyntaxError`
  instead of accidental binding names. `import` and `export` are now rejected
  as binding names in variable declarations, function names, and parameters.
  The focused `language/identifiers` cluster now runs at **208 pass / 0 fail /
  60 skip**, and the CI subset now runs locally at **866 pass / 0 fail / 0
  timeout**.
- **Object.values/Object.entries enumerable snapshot semantics**:
  `Object.values` and `Object.entries` now perform the required nullish
  `ToObject` check, re-check each snapshotted own string key's current
  enumerable descriptor before reading its value, and omit keys deleted or
  made non-enumerable by earlier getters. Empty-handler Proxies now forward
  own-key, own-descriptor, `hasOwnProperty`, and `Object.defineProperty`
  no-trap operations to their targets for this path. The focused
  `built-ins/Object/values built-ins/Object/entries built-ins/Object/hasOwn
  built-ins/Object/getOwnPropertyDescriptors` cluster now runs at **98 pass /
  0 fail / 23 skip**.
- **String static constructors**: `String.fromCodePoint` now throws
  `RangeError` for non-integral, non-finite, negative, and out-of-range code
  point inputs instead of silently truncating through integer casts.
  `String.raw` now appends the empty string when a substitution is missing,
  rather than the string `"undefined"`, while still converting explicit
  `undefined` raw segments through `ToString`. The focused
  `built-ins/String/fromCodePoint built-ins/String/raw
  built-ins/String/fromCharCode` cluster now runs at **51 pass / 0 fail / 7
  skip**.
- **Function-code `this` binding and primitive receiver semantics**:
  non-strict interpreted functions now apply the required `this` binding
  conversion, mapping nullish receivers to the global object and boxing
  primitive receivers with `ToObject`, while strict functions preserve the raw
  receiver. Primitive prototype accessor lookup now keeps the original
  primitive receiver through the prototype chain so strict getters see the
  primitive and sloppy getters receive the boxed object.
- **Function declaration instantiation edges**: non-strict duplicate
  parameters now keep distinct raw argument slots so an omitted later duplicate
  initializes the shared binding to `undefined`; `var` declarations now reuse
  parameter bindings instead of reporting lexical redeclarations; function
  declarations overwrite parameter and `arguments` bindings; and strict
  block-level function declarations stay block-scoped instead of leaking
  through Annex B hoisting. The focused `language/function-code` cluster now
  runs at **217 pass / 0 fail / 0 skip**.
- **Global declaration instantiation edges**: global scripts now perform
  declaration-instantiation preflight for lexical/global-var collisions,
  restricted global properties, and non-extensible global objects before any
  script body side effect runs. Global function declarations now use
  `CreateGlobalFunctionBinding` descriptor rules, global `var` declarations
  use `CreateGlobalVarBinding`, sloppy direct-eval global `var` properties are
  configurable, and strict global block-level function declarations remain
  block-scoped. The focused `language/global-code` cluster now runs at **31
  pass / 0 fail / 11 skip**.
- **Eval global declaration bindings**: non-strict direct and indirect eval
  now preflight global `var`/function declarations with
  `EvalDeclarationInstantiation`-style checks and create configurable global
  eval bindings, while `$262.evalScript()` keeps script-global
  non-configurable binding semantics. Same-Realm indirect eval also gets a
  fresh lexical environment so eval-local lexical and strict `var`/function
  bindings do not leak to the global object. Direct eval now uses the caller's
  variable environment for non-strict `var`/function declarations, preserves
  existing local var bindings during eval declaration instantiation, makes
  newly-created local eval bindings deletable, respects `with` object lookup
  inside eval source, and reflects cross-Realm indirect eval declarations on
  that Realm's global object. Direct eval during function, arrow, and
  generator parameter initialization now rejects `var arguments` declarations
  against an existing arguments binding, and generator calls now run parameter
  and declaration-instantiation bytecode before returning the suspended
  generator object. The focused `language/eval-code` cluster now runs at
  **225 pass / 0 fail / 122 skip**.
- **Numeric literal early errors**: numeric and BigInt literal lexing now
  rejects malformed radix prefixes, invalid numeric separator placement,
  BigInt suffixes on fractional/exponent/legacy-octal-like forms, and
  identifier-start characters immediately after numeric literals. Legacy
  octal and non-octal decimal literals are preserved for sloppy mode but now
  surface as `SyntaxError` in strict mode. The focused
  `language/literals/bigint language/literals/numeric` cluster now runs at
  **216 pass / 0 fail / 0 skip**, and broader `language/literals` improves to
  **312 pass / 162 fail / 60 skip**.
- **Unicode whitespace/comment lexing and `String.fromCharCode` coercion**:
  lexer whitespace/comment handling now recognizes ES Unicode space separators,
  treats only CR/LF/LS/PS as line terminators, reports unterminated multiline
  comments and regular-expression literals, and preserves ASI newline tracking
  across multiline comments. `String.fromCharCode` now applies `ToNumber` and
  `ToUint16` to every argument instead of ignoring non-number values. The
  focused `language/comments language/white-space` cluster now runs at **85
  pass / 0 fail / 34 skip**, and the CI subset rises to **823 pass / 41 fail /
  2 timeout** locally.
- **test262 `$262.createRealm()` and native constructability**: added a
  test262 host object with `createRealm()`/`evalScript()`, realm-bound global
  `eval` and `parseInt` functions, and indirect eval execution against the
  callee's Realm environment. Native functions are now constructable only when
  marked with an internal constructor prototype, so `new parseInt` throws while
  `new Proxy(...)` remains constructable without exposing `Proxy.prototype`.
  This closes the remaining cross-realm eval/template/non-constructor checks
  plus the Proxy subclass edge case, raising the supported subset to **4180
  pass / 0 fail / 0 timeout**.
- **Small language conformance edges**: rest-parameter functions now create
  unmapped arguments objects in sloppy mode, non-extensible objects reject
  `__proto__` prototype mutation, and Symbol-keyed assignment respects
  accessors, inherited setters, non-writable descriptors, and extensibility.
  Parser/lexer handling now rejects a line terminator after `throw`, reports
  unterminated string literals, rejects reserved-word object-literal shorthand
  such as `({ this })`, and accepts `undefined` as a `var` binding name. The
  focused language cluster
  `language/asi language/computed-property-names language/keywords
  language/rest-parameters language/types` now runs at **290 pass / 0 fail / 9
  skip**.
- **String search methods and `Symbol.match`**: aligned
  `String.prototype.includes`, `startsWith`, and `endsWith` with
  `RequireObjectCoercible`, `IsRegExp`, `ToString`, and position/end-position
  ordering. `Symbol.match` is now exposed, `Object.defineProperty` preserves
  Symbol property keys, property lookup invokes accessor getters for
  Symbol-keyed `@@match`, and generated Symbols no longer collide with
  well-known Symbols. The focused String search cluster now runs at **63 pass /
  0 fail / 12 skip**.
- **Sparse array holes and own-key enumeration**: dense arrays now track
  whether each backing-store slot is actually present, so array literal
  elisions, `Array(length)` holes, `delete array[index]`, `hasOwnProperty`,
  `propertyIsEnumerable`, `Object.keys`, and
  `Object.getOwnPropertyNames` agree on absent elements. `for...in` now uses
  the same present-bit model for arrays and includes boxed String exotic
  indices. The focused Object own-key cluster now runs at **90 pass / 0 fail /
  14 skip**.
- **ArrayBuffer/DataView prototype accessors and detach host hook**:
  `ArrayBuffer.prototype.byteLength` and DataView `buffer`/`byteLength`/
  `byteOffset` are now installed as spec-visible accessor properties with
  named getter functions and receiver validation. The test262 host object now
  exposes `$262.detachArrayBuffer()`, ArrayBuffers track detached state, and
  detached ArrayBuffers/DataViews report the required byteLength/byteOffset
  behavior. The focused built-ins accessor cluster now runs at **29 pass / 0
  fail / 19 skip**.
- **DataView 8-bit element accessors**: implemented
  `DataView.prototype.getUint8`, `getInt8`, `setUint8`, and `setInt8` with
  DataView receiver validation, `ToIndex` byte-offset conversion, setter value
  conversion ordering, detached-buffer checks, byte-range validation, Uint8
  wrapping writes, and signed Int8 reads. The focused DataView 8-bit method
  cluster now runs at **49 pass / 0 fail / 29 skip**.
- **DataView 16-bit element accessors**: implemented
  `DataView.prototype.getUint16`, `getInt16`, `setUint16`, and `setInt16` with
  big-endian defaults, `ToBoolean` little-endian handling, Uint16 wrapping
  writes, signed Int16 reads, and the same `ToIndex`/value/detached/range
  validation ordering as the 8-bit methods. The focused DataView 16-bit
  method cluster now runs at **56 pass / 0 fail / 28 skip**.
- **DataView 32-bit element accessors**: implemented
  `DataView.prototype.getUint32`, `getInt32`, `setUint32`, and `setInt32` with
  big-endian defaults, `ToBoolean` little-endian handling, Uint32 wrapping
  writes, signed Int32 reads, and the same `ToIndex`/value/detached/range
  validation ordering as the smaller DataView integer methods. The focused
  DataView 32-bit method cluster now runs at **56 pass / 0 fail / 38 skip**.
- **DataView floating-point element accessors**: implemented
  `DataView.prototype.getFloat32`, `setFloat32`, `getFloat64`, and
  `setFloat64` with IEEE-754 byte encoding/decoding, big-endian defaults,
  `ToBoolean` little-endian handling, `-0`/NaN/Infinity preservation, and the
  same `ToIndex`/value/detached/range validation ordering as the integer
  DataView methods. The focused DataView float method cluster now runs at
  **62 pass / 0 fail / 28 skip**.
- **DataView BigInt element accessors**: implemented
  `DataView.prototype.getBigInt64`, `getBigUint64`, `setBigInt64`, and
  `setBigUint64` with signed/unsigned 64-bit BigInt reads, big-endian
  defaults, `ToBoolean` little-endian handling, `ToBigInt` setter conversion,
  modulo-`2^64` byte writes, and the same receiver, `ToIndex`, detached-buffer,
  and byte-range validation ordering as the numeric DataView methods. The
  official runner still skips this focused cluster while `ArrayBuffer` and
  `DataView` remain marked unsupported; with only those feature skips lifted
  for diagnosis, the BigInt DataView cluster runs at **40 pass / 3 fail / 26
  skip**, with remaining failures requiring immutable ArrayBuffer and
  additional typed-array receiver support. The shared `BigInt()` constructor
  conversion path now also handles primitive-producing objects and reports
  `TypeError` for missing/nullish input.
- **BigInt fixed-width statics**: implemented `BigInt.asIntN` and
  `BigInt.asUintN` with `ToIndex(bits)` before `ToBigInt(value)`, signed and
  unsigned modulo-`2^bits` wrapping, correct `name`/`length` descriptors, and
  non-constructable native functions. The focused BigInt fixed-width static
  cluster now runs at **14 pass / 0 fail / 14 skip**, and the broader
  `built-ins/BigInt` smoke run improves to **49 pass / 25 fail / 29 skip**.
- **BigInt prototype conversion methods**: implemented
  `BigInt.prototype.valueOf` and radix-aware
  `BigInt.prototype.toString(radix)` with `thisBigIntValue` receiver checks,
  `ToNumber`/`ToIntegerOrInfinity` radix validation, own
  `BigInt.prototype.constructor`, the non-writable `BigInt` constructor
  `prototype` property, primitive-wrapper `Object(value)` prototype wiring,
  and ordinary `ToPrimitive` lookup for boxed primitives. The focused BigInt
  prototype/valueOf/toString cluster now runs at **16 pass / 0 fail / 5 skip**,
  and the broader `built-ins/BigInt` smoke run is now **74 pass / 0 fail / 29
  skip**.
- **`Object.hasOwn`**: added the ES2022 static own-property predicate with
  `ToObject` before `ToPropertyKey`, symbol-key support, primitive string
  wrapper `length`/index handling, correct `name`/`length` descriptors, and no
  constructable `prototype` property. The focused `Object.hasOwn` test262
  cluster now runs at **56 pass / 0 fail / 6 skip**.
- **`Object.getOwnPropertyDescriptor` conformance**: aligned
  `Object.getOwnPropertyDescriptor` with `ToObject` before `ToPropertyKey`,
  symbol property keys, string exotic `length`/index descriptors, and
  `FromPropertyDescriptor` result-object attributes. Built-in constructor
  `length`/`name`/`prototype` descriptors and a small set of missing
  descriptor-visible built-in prototype members were also tightened. The
  focused `Object.getOwnPropertyDescriptor` test262 cluster now runs at **308
  pass / 0 fail / 2 skip**.
- **Object own-key enumeration**: `Object.keys`,
  `Object.getOwnPropertyNames`, `Object.getOwnPropertySymbols`, and
  `Object.getOwnPropertyDescriptors` now share array-index-first own-key
  ordering, apply `ToObject` with nullish `TypeError` checks, include
  non-enumerable string keys where required, preserve Symbol keys, and
  synthesize primitive string index/`length` keys for descriptor collection.
  `Object.getOwnPropertySymbols` is now exposed on `Object`, and the focused
  `Object.getOwnPropertyDescriptors` plus `Object.getOwnPropertySymbols`
  clusters now run at **13 pass / 0 fail / 17 skip**. The broader focused
  own-key smoke run is **97 pass / 6 fail / 31 skip**, with remaining failures
  tied to receiver-brand handling and sparse-array hole representation work.
- **`Object.prototype.toString` receiver brands**: removed an unsafe native
  `toString` dispatch workaround and stopped installing Error-prototype
  `name`/`message`/`toString` properties on `Object.prototype`. The builtin
  now distinguishes `null` from `undefined`, reports BigInt primitives,
  boxed primitive wrappers, functions, arrays, arguments objects, Date
  instances, and Error instances with the expected brands, and keeps
  `Error.prototype.toString` separate. The focused
  `built-ins/Object/prototype/toString` cluster now has **0 failures**, and
  the combined Object toString/own-key smoke run is **105 pass / 3 fail / 37
  skip**, with the remaining failures isolated to sparse array holes and dense
  array-index deletion.
- **ArrayBuffer and DataView subclass internals**: added minimal
  `ArrayBuffer` and `DataView` exotic heap objects, constructor/prototype
  bootstrap, `ArrayBuffer.prototype.slice`, and DataView `buffer`/
  `byteOffset`/`byteLength` accessors. Subclass construction now initializes
  the required internal slots and `ArrayBuffer.prototype.slice` returns the
  default subclass constructor result while clamping inverted slice ranges and
  rejecting oversized backing-store lengths, closing the ArrayBuffer/DataView
  subclass checks and raising the supported subset to **4177 pass / 3 fail / 0
  timeout**. Promise GC tracing now also marks downstream derived promises held
  in pending handlers, fixing a stress failure exposed by the additional
  bootstrap allocations.
- **Uint8Array subclass exotic construction**: `Uint8Array` now exposes a
  constructor `prototype`, subclass construction allocates typed-array exotic
  objects with `new.target.prototype`, and integer-index writes update the
  backing buffer with Uint8 wrapping semantics. This closes the remaining
  generic builtin subclassing check, raising the supported subset to **4173
  pass / 7 fail / 0 timeout**.
- **Promise subclass executor validation**: `Promise` construction now rejects
  non-callable executors before allocating the promise object and uses
  `new.target.prototype` when creating subclass promise instances. This closes
  the Promise subclass regular-construction check, raising the supported
  subset to **4172 pass / 8 fail / 0 timeout**.
- **Date subclass component semantics**: `Date` construction with multiple
  date/time components now stores a clipped time value instead of treating the
  year as a raw timestamp, and Date component getters now derive calendar
  fields from the stored time value. This closes the remaining Date subclass
  regular-construction check, raising the supported subset to **4171 pass / 9
  fail / 0 timeout**.
- **Private accessors and non-extensible private slots**: class private
  accessors now parse and install as private accessor slots, static private
  fields/methods/accessors initialize on the constructor object, function
  objects now track extensibility for `Object.preventExtensions`, and adding a
  new private slot to a non-extensible object now throws `TypeError`. This
  closes the private-field non-extensible class checks, raising the supported
  subset to **4170 pass / 10 fail / 0 timeout**.
- **For-of IteratorClose on abrupt completion**: `for...of` now closes
  unfinished iterators when loop bodies complete abruptly via `return`,
  `break`, or throw, while preserving same-loop `continue` without closing the
  iterator. Iterator `return()` errors now override the pending for-of
  completion where required, while destructuring assignment keeps preserving
  the original throw. This closes the derived-constructor return-override
  for-of checks, raising the supported subset to **4168 pass / 12 fail / 0
  timeout**.
- **GeneratorFunction constructor and prototype chain**: generator functions now
  inherit from `%GeneratorFunction.prototype%`, whose `constructor` exposes the
  non-global `GeneratorFunction` constructor. Dynamic generator functions parse
  and compile as `function*`, their own `prototype` objects inherit from the
  generator prototype without an own `constructor` reference, and generator
  calls now use the callee's `prototype` for created generator objects. This
  closes the remaining GeneratorFunction subclass `prototype` and regular
  subclassing checks, raising the supported subset to **4166 pass / 14 fail / 0
  timeout**.
- **Array, RegExp, and String subclass exotic construction**: native
  constructors now share `new.target.prototype` fallback handling for
  OrdinaryCreateFromConstructor-style allocation. Array subclass construction
  now returns Array exotic objects with the subclass prototype, RegExp
  subclass construction now uses the subclass prototype and preserves
  non-configurable `lastIndex` descriptors across `test`/`exec`, and boxed
  String objects now materialize their own non-writable, non-enumerable,
  non-configurable `length` descriptor. This closes Array/RegExp subclass
  checks plus String `length`, raising the supported subset to **4164 pass /
  16 fail / 0 timeout**.
- **Dynamic Function subclass construction**: native constructors now preserve
  the active `new.target` for native constructor bodies, and the dynamic
  `Function` constructor now creates function objects with the
  `new.target.prototype` internal prototype plus own `length`, `name`, and
  `prototype` descriptors. This closes Function subclass `length`/`name` and
  `instanceof` checks, improves GeneratorFunction subclass descriptor checks,
  and raises the supported subset to **4158 pass / 22 fail / 0 timeout**.
- **Class method descriptor validation**: static class methods and accessors now
  route through own-property descriptor validation when defining constructor
  properties, so computed static `prototype` methods/accessors throw instead of
  overwriting the constructor's non-configurable `prototype` property. This
  improves `language/statements/class` to **187 pass / 22 fail** and raises the
  supported subset to **4152 pass / 28 fail / 0 timeout**.
- **Class declaration completion and binding mutability**: class declarations
  now produce an empty statement completion, so direct eval returns
  `undefined` or the previous non-empty completion rather than the constructor.
  The outer class declaration binding is now initialized as a mutable lexical
  binding while the inner class-name binding captured by methods and heritage
  remains immutable. This improves `language/statements/class` to **186 pass /
  23 fail** and raises the supported subset to **4151 pass / 29 fail / 0
  timeout**.
- **Var initializer Reference resolution**: `var x = init` now resolves the
  binding Reference before evaluating `init`, matching the spec's
  `BindingInitialization` order for `VariableDeclaration`. This preserves
  `with` object references when the initializer mutates the same property and
  keeps global `var` bindings synchronized with their global object
  descriptors. This closes `language/statements/variable` at **77 pass / 0
  fail** and raises the supported subset to **4147 pass / 33 fail / 0
  timeout**.
- **Contextual `of` division lexing**: `/` after contextual `of` now remains a
  division operator in expression contexts such as `instance/of/g`, while
  raw `of` delimiters in `for...of` heads still allow a following regex
  literal. This closes `language/expressions/division` at **41 pass / 0
  fail** and raises the supported subset to **4146 pass / 34 fail / 0
  timeout**.
- **Addition primitive coercion**: binary `+` now performs `ToPrimitive`
  before BigInt mixing checks, concatenates when either primitive is a string
  so BigInt-to-string concatenation is allowed, and treats Date objects with
  the default hint as string-hinted ordinary primitives. This closes
  `language/expressions/addition` at **38 pass / 0 fail** and raises the
  supported subset to **4144 pass / 36 fail / 0 timeout**.
- **Operator edge semantics**: BigInt exponentiation now throws `RangeError`
  for negative exponents, BigInt relational comparisons now coerce
  Boolean/nullish numeric operands through `ToNumeric`, `in` now rejects
  primitive right-hand sides before property-key conversion, `instanceof`
  now returns `false` for primitive left-hand sides before reading
  `prototype`, and strict non-generator `yield` is rejected during parsing.
  This closes `language/expressions/exponentiation`, `greater-than`,
  `less-than`, `in`, and `instanceof` at **188 pass / 0 fail** and raises the
  supported subset to **4142 pass / 38 fail / 0 timeout**.
- **Class heritage strictness and strict arguments objects**: class heritage
  expressions now parse under strict mode while preserving script-goal
  `await` class names, and strict function calls now create an unmapped
  `arguments` object whose `callee` accessor throws `TypeError`. This closes
  `language/statements/class/strict-mode` at **2 pass / 0 fail** and raises
  the supported subset to **4135 pass / 45 fail / 0 timeout**.
- **Switch CaseBlock scoping and redeclarations**: switch `var`
  declarations now bind in the enclosing variable environment instead of the
  switch lexical environment, while function declarations in case bodies stay
  scoped to the CaseBlock. Switch redeclaration early errors now treat
  function declarations as lexical names. This closes
  `language/statements/switch` at **69 pass / 0 fail** and raises the
  supported subset to **4133 pass / 47 fail / 0 timeout**.
- **Boxed String methods and Date method surface**: String prototype methods
  now read the wrapped primitive from `new String(...)` objects, so indexed
  operations like `charAt` agree with boxed string index properties. The
  bootstrap also installs `String.prototype.length`, `Date.parse`, `Date.UTC`,
  and the ES5 Date prototype method surface needed for property-access checks.
  This closes `language/expressions/property-accessors` at **15 pass / 0
  fail** and raises the supported subset to **4127 pass / 53 fail / 0
  timeout**.
- **Tagged-template call context and conditional `in` grammar**: tagged
  templates used as member expressions now preserve their receiver as `this`,
  ``new tag`...` `` constructs the tag result rather than the tag function
  itself, and constructor arguments after a tagged template are applied to that
  result. Conditional-expression true branches now allow `in` even inside
  no-`in` contexts such as `for` heads. This reduces
  `language/expressions/tagged-template` to its remaining cross-realm
  `$262.createRealm()` failure, closes
  `language/expressions/conditional/in-branch-1.js`, and raises the supported
  subset to **4124 pass / 56 fail / 0 timeout**.
- **Call-expression environment and argument ordering**: explicit named
  function-expression bindings now live in the function closure environment
  rather than the call body's variable environment, so body `var` declarations
  with the same name create the required separate binding. Sloppy direct eval
  now accepts `static` as a contextual `var` binding name, and member calls now
  perform the property lookup before evaluating arguments while leaving the
  callability check after argument evaluation. This improves
  `language/expressions/call` to **48 pass / 1 fail** and raises the
  supported subset to **4121 pass / 59 fail / 0 timeout**.
- **Named function-expression bindings**: named function expressions now create
  an immutable inner name binding. Sloppy assignments to that binding are
  ignored, while strict assignments throw `TypeError`; direct eval and lexical
  arrows inside the function body resolve to the same protected binding. This
  closes `language/expressions/function` at **53 pass / 0 fail** and raises
  the supported subset to **4118 pass / 62 fail / 0 timeout**.
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
- **Catch parameter early errors**: `try` statements now reject duplicate
  catch-parameter bound names and direct catch-block lexical/function
  redeclarations of the catch parameter while still allowing `var` and nested
  block shadowing. This fixes the `early-catch-duplicates`,
  `early-catch-lex`, and `early-catch-function` test262 cases and reduces the
  `try` statement subset to **9 failures**.
- **Native runtime errors through `finally`**: catchable native VM errors such
  as `ReferenceError` and `TypeError` now divert through active `finally`
  guards before reaching an outer `catch`, matching the same path as explicit
  JS `throw`. Re-thrown Error objects also preserve their specific error kind
  in host error reporting. This fixes test262 `S12.14_A3` and `S12.14_A13_T2`
  and reduces the `try` statement subset to **7 failures**.
- **Native Error constructor call branding**: plain calls to native Error
  subclasses, such as `EvalError(1)` and `TypeError(1)`, now allocate through
  the active callee's `prototype` instead of always using `Error.prototype`.
  Native constructor dispatch also clears consumed `new.target` state so a
  native `new Error(...)` call cannot leak construct state into the next call.
  This fixes test262 `S12.14_A19_T1` and `S12.14_A19_T2` and reduces the
  `try` statement subset to **5 failures**.
- **Declarative binding deletion semantics**: `delete` now returns `false`
  for declarative environment bindings, including lexical bindings and
  catch parameters, instead of deleting `let`/`const`-classified bindings.
  This preserves catch parameter values through `delete e`, fixes test262
  `S12.14_A4`, and reduces the `try` statement subset to **4 failures**.
- **`try`/`finally` completion replacement semantics**: abrupt completions
  entering a `finally` block now keep the original completion isolated from
  the `finally` body's own completion. Normal expression values inside
  `finally` no longer overwrite a pending empty `break`, while a `throw`
  inside `finally` correctly replaces a pending `return`, `break`, or outer
  `throw`. Non-throw abrupt completions also disable skipped catch handlers
  before entering `finally`, so a `finally`-body `throw` cannot be caught by
  the catch clause of the same already-completed try statement. This brings
  `language/statements/try` to **98 pass / 0 fail**.
- **Function.prototype restricted properties**: `%Function.prototype%` now has
  inherited `caller` and `arguments` accessor properties whose getter and
  setter throw `TypeError`. Bound functions created by
  `Function.prototype.bind` therefore do not gain own `caller`/`arguments`
  properties but still inherit the required restricted accessors, reducing the
  `function` statement subset to **7 failures**.
- **Function body `"use strict"` with non-simple parameters**: function
  declarations, function expressions, object/class methods, and arrow block
  bodies now reject a directive prologue `"use strict"` when the formal
  parameter list contains defaults, rest, or destructuring. Directive
  detection now runs before synthesized destructuring-parameter prelude
  statements are prepended, reducing the `function` statement subset to
  **6 failures**.
- **Object/class method formal-parameter early errors**: concise object
  methods, async object methods, and class/private methods now reject duplicate
  formal parameter bound names, including duplicates introduced by
  destructuring patterns. Object async method parsing also enforces the
  required no-LineTerminator restriction between `async` and the property
  name, reducing the object method-definition subset to **3 failures**.
- **`yield` contextual identifier parsing**: sloppy non-generator contexts now
  parse `yield` as an identifier in bindings, expressions, destructuring
  patterns, object method parameters/defaults, and computed property names,
  while generator parameter/body contexts continue to parse `yield` as the
  generator keyword. This brings the object method-definition subset to
  **40 pass / 0 fail**.
- **`let` declaration ASI/lookahead parsing**: `let` followed by a binding
  name now remains a LexicalDeclaration across line terminators in
  StatementListItem positions, so cases like `let\nlet` and `let\nawait 0`
  fail during parse instead of executing. Escaped `l\u0065t` stays an
  identifier, and single-statement bodies still use ExpressionStatement
  lookahead rules, reducing `language/statements/let/syntax` to
  **26 pass / 4 fail**.
- **Parenthesized assignment-pattern targets**: parenthesized object/array
  literals are no longer accepted as assignment targets for an outer
  assignment. This preserves valid inner destructuring such as `({} = obj)`
  while rejecting `({}) = 1` and arrow-expression bodies like
  `() => ({}) = 1`, reducing `language/expressions/assignmenttargettype` to
  **313 pass / 3 fail**.
- **`await` contextual identifier parsing**: sloppy non-async script and
  function contexts now parse `await` as a contextual identifier in
  declarations, formal parameters, assignment/reference positions,
  destructuring patterns, object method parameters, computed property names,
  and nested non-async functions inside async bodies. Async function, async
  method, and async arrow parameter/body contexts still parse `await` as the
  async keyword. This brings `language/expressions/await` to
  **7 pass / 0 fail** and reduces `language/expressions/assignmenttargettype`
  to **314 pass / 2 fail**.
- **`import.meta` assignment target early errors**: direct and parenthesized
  assignments to `import.meta` are now rejected during parsing instead of
  reaching runtime. This closes the remaining
  `language/expressions/assignmenttargettype` failures, bringing that
  directory to **316 pass / 0 fail**.
- **Object destructuring assignment target order**: member-expression targets
  inside object assignment patterns are now evaluated before reading the
  source property value, including computed source keys. This fixes the
  test262 target-reference evaluation-order case and reduces
  `language/expressions/assignment/destructuring` to **1 pass / 5 fail**.
- **Array destructuring assignment iterator semantics**: array assignment
  patterns now use the iterator protocol, evaluate member targets before
  stepping the iterator, apply assignment defaults, and close unfinished
  iterators when default evaluation or target assignment throws while
  preserving the original throw completion. Lazy iterator `next()` results now
  read `done` before `value`, so done iterators do not invoke `value` getters.
  This reduces `language/expressions/assignment/destructuring` to
  **5 pass / 1 fail**.
- **Duplicate `__proto__` object assignment properties**: object assignment
  patterns now allow duplicate static `__proto__` colon properties while
  keeping the Annex B duplicate-`__proto__` early error for object literals.
  Destructuring assignment expressions also preserve the RHS value as their
  expression result, so nested assignments such as `result = ({ x } = obj)`
  store `obj`. This brings `language/expressions/assignment/destructuring` to
  **6 pass / 0 fail**.
- **Computed property names in `for` heads**: computed property names and
  computed member keys now parse their bracketed expressions with `in`
  allowed, even when the surrounding expression is being parsed under
  `for (... in ...)` lookahead. This fixes object accessor names such as
  `{ get ["x" in obj]() {} }` inside `for` initializers and reduces
  `language/expressions/object` to **271 pass / 14 fail**.
- **Object literal strict early errors**: strict object literal shorthand
  properties now reject reserved IdentifierReferences such as `let` and
  `yield`, and object accessors/methods apply body `"use strict"` directives
  to formal-parameter `eval`/`arguments` checks. This reduces
  `language/expressions/object` to **275 pass / 10 fail** and raises the
  supported subset to **3953 pass / 225 fail / 2 timeout**.
- **String literal line continuations**: string literals now treat a backslash
  followed by a LineTerminatorSequence as a LineContinuation that contributes
  no cooked characters. This fixes computed object accessor names and reduces
  `language/expressions/object` to **276 pass / 9 fail**, raising the
  supported subset to **3954 pass / 224 fail / 2 timeout**.
- **Direct eval lexical declaration conflicts**: sloppy direct eval now rejects
  `var`/function declarations that would hoist over an existing caller
  `let`/`const` binding. This fixes method/accessor body lexical environment
  conflict cases, reduces `language/expressions/object` to
  **279 pass / 6 fail**, and raises the supported subset to
  **3960 pass / 218 fail / 2 timeout**.
- **Object method parameter/body environments**: functions with parameter
  expressions now evaluate defaults and synthetic destructuring preludes before
  pushing a separate body variable environment. Parameter closures no longer
  see later body `var` declarations, while direct eval `var`s created during
  parameter evaluation remain visible to both parameter and body closures.
  Parser parameter scratch state is also scoped per nested function/method, so
  nested function expressions in defaults no longer steal outer defaults. This
  brings `language/expressions/object` to **285 pass / 0 fail** and raises the
  supported subset to **3979 pass / 199 fail / 2 timeout**.
- **Arrow formal-parameter early errors**: arrow functions now reject
  duplicate bound names introduced by destructuring parameters and reject a
  line terminator before `=>` for both parenthesized and parenless parameter
  forms. Async-arrow lookahead also preserves the required no-LineTerminator
  restrictions around `async` and `=>`. This brings
  `language/expressions/arrow-function/syntax/early-errors` to
  **25 pass / 0 fail** and raises the supported subset to
  **3990 pass / 188 fail / 2 timeout**.
- **Sloppy arrow contextual parameters**: non-strict arrow functions now allow
  `eval`, `arguments`, and `yield` as formal parameter names where the grammar
  permits them, while strict enclosing code or a block-body `"use strict"`
  directive still rejects `eval`/`arguments`. This brings
  `language/expressions/arrow-function/syntax` to **45 pass / 0 fail**,
  raises `language/expressions/arrow-function` to **88 pass / 2 fail**, and
  raises the supported subset to **3996 pass / 182 fail / 2 timeout**.
- **Arrow lexical `arguments`**: arrow function calls no longer create their
  own `arguments` object binding, so `arguments` references inside an arrow
  resolve through the captured lexical environment unless shadowed by an
  explicit parameter. This raises `language/expressions/arrow-function` to
  **89 pass / 1 fail** and the supported subset to
  **3997 pass / 181 fail / 2 timeout**.
- **Lexical arrow `super()` binding order**: `super()` calls now perform the
  superclass constructor call before rebinding the derived constructor's
  lexical `this` environment and forward the active constructor's
  `new.target`. A repeated `super()` call, including one captured in an arrow
  and invoked after the constructor returns, now throws `ReferenceError` only
  after the superclass constructor has run. This closes
  `language/expressions/arrow-function` at **90 pass / 0 fail** and raises the
  supported subset to **3999 pass / 179 fail / 2 timeout**.
- **`super()` constructor mixed spread arguments**: `super(...)` now lowers
  mixed spread and non-spread arguments through the same iterator-backed
  argument-array path used by ordinary calls and `new`. This preserves
  left-to-right evaluation, handles empty spreads, and reports unresolvable
  spread operands as `ReferenceError`. This raises
  `language/expressions/super` to **30 pass / 6 fail** and the supported
  subset to **4003 pass / 175 fail / 2 timeout**.
- **Lexical arrow `super` property parsing**: block-bodied arrow functions
  now preserve the enclosing method's `super` parse context instead of
  resetting it like ordinary function bodies. This allows `super.x` and
  `super["x"]` inside arrows nested in object methods while still rejecting
  `super` in arrows without an enclosing super binding. This raises
  `language/expressions/super` to **32 pass / 4 fail** and the supported
  subset to **4005 pass / 173 fail / 2 timeout**.
- **Direct eval lexical `super` parsing**: direct eval now inherits the
  caller's `super` parse context when the caller environment has a `#super`
  binding. This allows `eval("super.x")` and computed `super` property access
  inside object methods while preserving SyntaxError for eval code without an
  enclosing super binding. This raises `language/expressions/super` to
  **34 pass / 2 fail** and the supported subset to
  **4007 pass / 171 fail / 2 timeout**.
- **Computed `super[...]` putvalue evaluation order**: compound assignment and
  update expressions now evaluate a `super` property target by checking the
  derived constructor `this` binding before evaluating a computed property
  expression, then reuse the same receiver/base/key reference for the get and
  set. This closes `language/expressions/super` at **36 pass / 0 fail** and
  raises the supported subset to **4009 pass / 169 fail / 2 timeout**.
- **Nullish/logical chain early errors**: unparenthesized `??` mixed directly
  with `&&` or `||` now throws a parse-time `SyntaxError`, while
  parenthesized combinations still parse and evaluate. This closes
  `language/expressions/coalesce` at **22 pass / 0 fail** and raises the
  supported subset to **4013 pass / 165 fail / 2 timeout**.
- **BigInt `ToNumeric` operator semantics**: unary plus and unsigned right
  shift now reject BigInt operands with `TypeError`, while BigInt-aware
  arithmetic, bitwise, and signed shift operations preserve BigInt results
  after `ToNumeric`, including boxed BigInts. `ToNumber` no longer silently
  converts BigInt except through the `Number()` constructor, and string
  numeric conversion no longer accepts incorrectly-cased Infinity spellings.
  This closes the BigInt failures in `bitwise-and`, `bitwise-or`,
  `bitwise-xor`, and `unsigned-right-shift`, reduces `unary-plus` to
  **0 failures**, and raises the supported subset to
  **4034 pass / 144 fail / 2 timeout**.
- **Native Error subclass construction**: `Error.prototype` now inherits
  `Object.prototype` during bootstrap, NativeError subclass instances no
  longer receive own `message` properties when the message argument is
  omitted, and `name` is inherited through the prototype chain so
  `class Err extends EvalError {}` instances report `EvalError`. This closes
  `language/statements/class/subclass/builtin-objects/NativeError` at
  **18 pass / 0 fail** and raises the supported subset to
  **4047 pass / 131 fail / 2 timeout**.
- **Class element grammar and named class expression scope**: class bodies now
  accept empty `;` elements, computed accessor names, and generator methods,
  and named class expressions create an inner immutable class-name binding
  instead of leaking the name to the outer scope. Class names now reject
  `yield` even in sloppy surrounding scripts. This closes
  `language/expressions/class` at **48 pass / 0 fail**, improves
  `language/statements/class/syntax` to **9 pass / 4 fail**, and raises the
  supported subset to **4061 pass / 117 fail / 2 timeout**.
- **Class declaration early errors**: script and block statement lists now
  reject duplicate lexical class declarations and lexical/`var` name clashes
  during parsing, and escaped `static` is no longer accepted as the class
  `static` modifier. This improves `language/statements/class/syntax` to
  **12 pass / 1 fail** and raises the supported subset to
  **4064 pass / 114 fail / 2 timeout**.
- **Class `super` property HomeObject setup**: class evaluation now gives
  constructor and instance methods a per-class `#super` binding based on
  `Class.prototype`, while static methods, static accessors, and static blocks
  bind `Class` in their own closure environment. SuperProperty evaluation
  reads the HomeObject prototype dynamically. This allows base-class
  constructor and method `super.prop`, fixes static `super.x` lookup on
  subclasses, closes `language/statements/class/super` and
  `language/statements/class/syntax` at **21 pass / 0 fail**, and raises the
  supported subset to **4069 pass / 109 fail / 2 timeout**.
- **Class definition/name-binding semantics**: class declarations now hoist as
  immutable lexical bindings, anonymous class assignment infers constructor
  display names, class bodies parse nested functions in strict context, and
  method/accessor display names no longer create body bindings that shadow
  outer variables. Class `extends` now performs the superclass `prototype`
  getter exactly once and reuses that value for prototype wiring, while
  derived constructors return the `this` object bound by `super()` when no
  object is explicitly returned. This closes
  `language/statements/class/definition` and
  `language/statements/class/name-binding` at **41 pass / 0 fail** and raises
  the supported subset to **4080 pass / 98 fail / 2 timeout**.
- **Dynamic class `super` references**: `super` property reads, calls, simple
  assignments, updates, and compound assignments now derive the super base
  from the method HomeObject at evaluation time instead of using a stale
  class-definition-time prototype value. This follows later
  `Object.setPrototypeOf` changes, and simple `super.x = rhs` /
  `super[expr] = rhs` evaluates `rhs` before throwing `TypeError` when the
  dynamic super base is `null`. This closes `language/expressions/assignment`
  at **110 pass / 0 fail** and raises the supported subset to
  **4082 pass / 96 fail / 2 timeout**.
- **Null-extending classes and bound subclass construction**:
  `class C extends null {}` now wires `C.prototype.[[Prototype]]` to `null`
  while making the constructor inherit from `%Function.prototype%`, and
  `super()` in such a class throws `TypeError` because `%Function.prototype%`
  is not a constructor. Constructing a bound class now ignores the bound
  `this` value and delegates to the target constructor with prepended bound
  arguments. This raises `language/statements/class/subclass` to
  **75 pass / 19 fail** and the supported subset to
  **4089 pass / 89 fail / 2 timeout**.
- **C-style `for` lexical head environments**: `for (let/const ...; ...; ...)`
  now creates a runtime loop-head lexical environment, evaluates the first
  condition/body/update in a per-iteration child environment, and reclones a
  sibling environment before each update so body closures keep pre-update
  bindings while the update prepares the next iteration. The parser also
  applies the head/body `var` redeclaration early error to ordinary `for`
  loops and accepts `async of => {}` as a normal async-arrow initializer. This
  closes `language/statements/for` at **93 pass / 0 fail** and raises the
  supported subset to **4103 pass / 77 fail / 0 timeout**.
- **Label identifiers and strict labelled functions**: labelled statements now
  accept contextual `await` labels in non-module code and contextual `yield`
  labels in sloppy non-generator code, including escaped spellings, while
  strict labelled function declarations are rejected during parsing. This
  closes `language/statements/labeled` at **17 pass / 0 fail** and raises the
  supported subset to **4108 pass / 72 fail / 0 timeout**.
- **Function statement-control parser boundaries and raw meta-property
  tokens**: nested function bodies now reset loop/switch/label parsing context
  so inner `break`/`continue` cannot target an outer function's labels.
  `async function` declarations and expressions now require no line terminator
  between `async` and `function`, `new.target` requires raw `new` and `target`
  tokens, and `debugger` is parsed as a statement-only keyword. This closes
  the remaining supported-subset failures in `language/statements/break`,
  `language/statements/continue`, `language/statements/debugger`,
  `language/statements/async-function`, and `language/expressions/new.target`,
  raising the supported subset to **4114 pass / 66 fail / 0 timeout**.
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

- **Arrow function early errors**: arrow functions reject duplicate parameter
  names in sloppy and strict mode, and reject `eval`/`arguments` parameter
  names when strict mode applies.

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
  direct `eval(...)` is detected from runtime callee resolution and runs in the
  caller's scope.

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
