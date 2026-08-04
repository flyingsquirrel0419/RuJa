# Features

## Language

- Arithmetic, comparison, logical, bitwise, and assignment operators
- `var`/`let`/`const` with environment-based scoping
- Control flow: `if`/`else`, `while`, `do...while`, `for`, `for...in`,
  `for...of`, `switch`, `break`/`continue`
- Functions, recursion, arrow functions, and closures (capture + mutation)
- `throw`/`try`/`catch`/`finally` with the `Error` type hierarchy
- Labeled statements (`label:`), `break label`, `continue label`; Annex B
  permits only ordinary sloppy labelled function declarations, while
  generator and async declarations are early errors. In a Module source goal,
  raw or escaped class binding names whose decoded value is `await` are early
  errors at every nesting depth
- Temporal Dead Zone (TDZ) for `let`/`const`; `const` reassignment is rejected
- Logical operators with correct short-circuit: `&&`, `||`, nullish `??`
- Logical assignment `&&=`/`||=`/`??=` and compound assignment on identifier,
  member, and element targets
- Optional chaining `?.` for property access, computed access, and calls
- Template literals with `${}` and tagged templates (`tag`...``); numeric separators (`1_000`, `0xff_ff`)
- `eval` (indirect runs globally; direct `eval(...)` is detected after runtime
  name resolution and runs in the caller's scope)
- `with` statement (dynamic object environment records)
- `new.target` meta-property (constructor-aware)
- `for(;;)` with any combination of empty init/condition/update
- `globalThis` routes property get/set to the global environment record
- `__proto__` accessor (get/set the object's [[Prototype]])

## ES2015+

- Realm-local `Temporal`, `Temporal.Now`, and `Temporal.Instant` expose their
  standard identities. Instant supports construction, exact epoch accessors,
  epoch factories, static `compare`, branded/string `equals`, basic and
  extended ISO `from`, the audited RFC 9557 annotation and nanosecond-offset
  subset, an always-throwing
  `valueOf`, exact `toString` options/rounding with UTC, fixed offsets, and
  ambiguity-checked annotated date/time forms, and
  `Date.prototype.toTemporalInstant`. Realm-local `Temporal.ZonedDateTime`
  supports hidden-slot construction for UTC/fixed offsets, exact epoch and
  identifier accessors, all ISO civil/calendar/offset accessors, subclassing,
  branded, fixed-offset-string, and ISO property-bag `from`, direct Instant
  conversion, option-aware `toString`, `toJSON`, and `valueOf`. Property-bag
  conversion currently accepts ISO calendars with UTC or minute-precision
  fixed offsets. Calendar arithmetic, duration, named IANA timezone
  transitions, the remaining RFC 9557 grammar, and `Temporal.Now` methods
  remain outside the supported boundary.
- `class`/`extends`/`super`
- Default & rest parameters
- Array/object destructuring (swaps, holes, rest, rename, nested) and object
  shorthand `{x, y}`
- `for...of` / `for...in`
- Computed property keys `[expr]` in object literals
- Object spread `{...a, y:2}` (copies enumerable own properties)
- Object rest destructuring `{a, ...r} = obj` (collects remaining own props)
- Getters/setters in object literals (`get x() {}` / `set x(v) {}`) and
  class methods (static and instance); inherited accessors bind `this` to
  the receiver, not the prototype that defines them
- Private class fields (`#field = init`): isolated storage per instance,
  not enumerable or accessible via `[]`/`for...in`/`Object.keys`
- Private class methods (`#method() {}`): called via `this.#method(...)`,
  may call other private methods and mutate private fields (`#f++` works)
- Static initialization blocks (`static { }`): run with `this` = the
  constructor in source order; can reference the class by name and hold
  local `let`/`const` bindings that do not leak
- Public/private, instance/static auto-accessors (`accessor name = init`) use
  hidden private backing storage with prototype or constructor getter/setter
  pairs. The audited decorator boundary supports class, public
  method/getter/setter/field/auto-accessor, and private
  field/method/getter/setter decorators; source-order expression evaluation;
  grouped application phases;
  constructable class/callable element replacement validation; auto-accessor
  `get`/`set`/`init` replacement records; public-property and private-brand
  `context.access`; `addInitializer` queues; field initializer transforms;
  computed Symbol names; and both class/export placements.
- BigInt literals (`123n`, `0xffn`, `0o17n`, `0b101n`): exact-integer
  arithmetic (`+ - * / % **`, comparisons, `===`/`==` with `Number`);
  mixing BigInt with `Number` throws `TypeError`; `BigInt()` constructor
  `valueOf()`, `toString(radix)`, `BigInt.asIntN`, and `BigInt.asUintN`
  supported. Runtime values share immutable limb storage across clones;
  arithmetic allocates fresh results and preserves value-based equality/hash
- `try/finally` non-local transfers: `return`/`throw`/`break`/`continue`
  in `try`/`catch` divert through **all** enclosing `finally` blocks,
  innermost-first, including nested `try/finally`; a `return`/`throw`
  inside a `finally` overrides the pending completion

## Async & generators

- `Promise` with `then`/`catch` chaining and microtask draining
- `Promise.resolve`/`Promise.reject`, static combinators, and `withResolvers`
- `async`/`await` (async functions and async arrows return a Promise; await
  resolves it, and returned Promises/thenables are assimilated)
- Lazy generators (`function*`/`yield`): pull-based `next()`/`for...of`/spread
  that suspend at each `yield`; supports infinite generators; `next(v)` resumes
  with a value; `return` ends the generator
- `yield*` delegation to generators, arrays, and strings (nestable)
- `async function*`: `next()` returns a Promise; `await` works in the body
- `Array.fromAsync` collects async iterables, sync iterables, and array-like
  inputs, optionally maps each value, awaits values and mapper results, closes
  iterators on the specified abrupt completions, and uses Realm-local Promise
  and result allocation
- Per-frame generator isolation: a generator body may call `next()` on another
  generator without corrupting either's run-state
- Realm-specific global `Iterator`: direct construction is rejected while
  subclass construction is supported. `%Iterator.prototype%` supplies
  `Symbol.iterator`, `Symbol.dispose`, `constructor`, and
  `Symbol.toStringTag`; generator, Array, Map, Set, and RegExp String iterator
  prototypes inherit from it while retaining their concrete tags, including
  the Generator prototype's own `"Generator"` tag. `Iterator.from` accepts
  iterables and direct iterators through a Realm-specific valid-iterator
  wrapper, `Iterator.prototype.toArray` eagerly collects an iterator, and the
  eager `Iterator.prototype.reduce` accumulates iterator values,
  `Iterator.prototype.forEach` visits each value for side effects, and
  `Iterator.prototype.some` and `every` short-circuit predicate results, while
  `Iterator.prototype.find` returns the first matching value. Static
  `Iterator.concat` validates and caches iterable methods eagerly, then opens
  and drains each iterator lazily in argument order. `Iterator.zip` eagerly
  opens its input records and supports shortest, longest with padding, and
  strict equal-length iteration with reverse close-all semantics.
  `Iterator.zipKeyed` applies the same modes to own enumerable string and
  Symbol keys and yields fresh null-prototype records. The
  Realm-specific Iterator Helper machinery powers lazy `map`, `filter`,
  `flatMap`, `take`, and `drop` pipelines with dynamic close and reentrancy
  semantics.

## Modules

- File-backed relative ES Module graphs support side-effect, default, named,
  namespace, star, and namespace re-exports; live bindings; cyclic graph
  instantiation; top-level await; dynamic import; and `import.meta`.
- Static import and re-export declarations accept Import Attributes. The host
  supports `type: "json"` and `type: "text"`; typed modules expose exactly one
  `default` export and share canonical namespace/value identity with dynamic
  imports of the same path and type.

## Property model

- Ordinary `[[Get]]`, `[[HasProperty]]`, and `[[Set]]` traverse prototype
  chains iteratively, retain reached objects across observable GC, and charge
  execution fuel per ordinary edge without imposing a fixed depth cutoff on
  acyclic chains. String and Symbol keys share one `ToPropertyKey` path,
  inherited accessors preserve the original receiver, and Module Namespace
  `[[Set]]` rejects every receiver consistently
- `Object.defineProperty` with data and accessor descriptors (`value`/
  `writable`, `get`/`set`); ordinary `[[Set]]` enforces `writable: false`
  (TypeError in strict mode, silent in sloppy) and invokes inherited setters
  through the prototype chain
- Proxy `[[DefineOwnProperty]]` preserves partial descriptor presence,
  revocation, `GetMethod`, trap-call, false-result, target-descriptor,
  extensibility, compatibility, configurable, and writable-tightening order.
  Transparent forwarding and nested invariant walks share one iterative,
  GC-rooted, fuel-metered VM state machine across internal and public
  `Object`/`Reflect` entry points
- Proxy `[[Set]]` preserves revocation, `GetMethod`, trap-call, false-result,
  and target descriptor invariant order while iterating transparent targets
  without a Proxy depth cap. Ordinary prototype traversal can hand control to
  the same state machine without Rust recursion. Receiver-side property
  creation and value-only updates retain their distinct descriptor fields and
  delegate through the shared iterative Proxy `[[DefineOwnProperty]]` path;
  descriptor objects use the current execution Realm
- `delete` respects `configurable` (false in sloppy, TypeError in strict).
  Proxy `deleteProperty` lookup, calls, and target invariants preserve Symbol
  keys and observable order; transparent chains are iterative, GC-rooted, and
  charged to cooperative execution fuel together with nested Proxy handler and
  invariant walks
- Symbol-keyed properties: `[Symbol.iterator]` and arbitrary Symbol keys are
  stored/read on objects and skipped by `for...in`/`JSON.stringify`
- Custom `Symbol.iterator`: objects with `[Symbol.iterator]()` are iterable via
  `for...of`/spread; lazy iterators call the JS `next()` per pull

## Standard library

- **Array**: `push`, `pop`, `shift`, `unshift`, `splice`, `map`, `filter`,
  `reduce`, `forEach`, `find`, `findIndex`, `findLast`, `fill`, `some`,
  `every`, `includes`, `indexOf`, `lastIndexOf`, `slice`, `concat`, `join`,
  `toLocaleString`, `flat`, `flatMap`, `at`, `sort`, `reverse`, `copyWithin`,
  `reduceRight`, `toReversed`, `toSorted`, `toSpliced`, `with`; `Array.from`/
  `fromAsync`/`of`/`isArray`. `%Array.prototype%` is a real Array exotic. Push,
  Pop, Shift, Unshift, Splice, Slice, Concat, CopyWithin, Fill, Filter, Flat,
  FlatMap, ForEach, Join, ToLocaleString, Reduce, ReduceRight, Reverse,
  ToReversed, ToSpliced, and With use generic indexed operations and logical
  lengths.
  Slice, Splice, Concat, and Filter
  honor species;
  Concat also applies `Symbol.isConcatSpreadable` to each input, preserves
  holes through `HasProperty`, and performs strict result definitions and a
  final length update. CopyWithin performs live `HasProperty` plus
  `Get`/strict `Set` or deletion without consulting species. Fill performs a
  live strict `Set` for each selected index after one length snapshot. Filter
  performs live `HasProperty`/`Get`, callback calls, and dense result property
  creation. Map creates its species result at the captured length, preserves
  holes, and performs live `HasProperty`/`Get`, callback, and strict result
  definitions. ForEach snapshots only length, then performs live
  `HasProperty`/`Get` and callback calls for each present index. Join snapshots
  length before separator coercion and performs live `Get`/`ToString` work for
  every index. ToLocaleString snapshots length, then performs live `Get` and
  invokes each non-nullish element's `toLocaleString` with no arguments. It
  uses `,` as RuJa's implementation-defined list separator. Reduce and
  ReduceRight discover an omitted initial accumulator
  and visit remaining values through live `HasProperty`/`Get` operations in
  ascending and descending order respectively. Reverse performs ordered live
  lower/upper existence checks, reads, strict writes, and deletions for each
  in-place pair. ToReversed creates a fresh intrinsic Array without consulting
  species, reads every source index live in descending order, and materializes
  holes as own `undefined` properties in the result. ToSpliced uses argument
  count to distinguish no arguments, one `start` argument, and an explicit
  `undefined` `skipCount`; it creates a fresh intrinsic Array without
  consulting species and reads the retained prefix and suffix live while
  materializing holes as own `undefined` properties. Flat and FlatMap share an
  iterative `FlattenIntoArray` path with
  live nested array access, species-created targets, mapper ordering, GC roots,
  and per-index fuel; cyclic infinite-depth inputs exhaust configured fuel or
  reach the bounded cycle-replay guard without native recursion. Fill and With deliberately ignore species. The Array constructor,
  Slice, and Concat can create sparse results above the dense cap. With reads
  through holes and retains a 1,048,576-element sandbox cap. `entries`, `keys`, and `values`
  are generic lazy iterators: they box primitive receivers once, re-read the
  live array-like length for every `next`, preserve inherited and Proxy index
  access, and use the iterator method Realm for result and entry arrays.
  Mapped and unmapped arguments objects install the Realm's immutable original
  `%Array.prototype.values%` as their own writable, non-enumerable
  `Symbol.iterator`, while iteration still observes later deletion or
  replacement of that own property
- **TypedArray**: `%TypedArray%.prototype.join` and `toLocaleString` validate the
  receiver, snapshot the internal view length, and read each current integer
  index in order. Join coerces its observed separator after the length snapshot,
  performs a live indexed `Get` and ordinary `ToString` for every non-nullish
  element, and retains the source across observable coercion. Locale conversion
  resolves `toLocaleString` through primitive `GetV` in the method Realm,
  receives no locale arguments in the non-ECMA-402 runtime, and converts the
  returned value with `ToString`. Both paths meter every captured index and use
  fallible intermediate output growth. `includes`, `indexOf`, and
  `lastIndexOf` preserve their internal-length and `fromIndex` semantics while
  consuming one fuel unit for every logical index they actually visit.
- **String**: `charAt`, `charCodeAt`, `slice`, `split`, `replace` (regex
  supported), `replaceAll`, `includes`, `startsWith`, `endsWith`, `repeat`,
  `padStart`/`padEnd`, `at`, `trim`/`trimStart`/`trimEnd`, `substring`, case
  conversions, `codePointAt`, `concat`, `search`; `replace` with function
  callback, `split` with regex
- **Object**: `defineProperty`, `defineProperties`, `keys`, `values`,
  `entries`, `assign`, `groupBy`, `hasOwn`, `create`,
  `getPrototypeOf`/`setPrototypeOf`, `preventExtensions`/`isExtensible`,
  `seal`/`isSealed`, `freeze`/`isFrozen`,
  `getOwnPropertyDescriptor`/`getOwnPropertyDescriptors`,
  `getOwnPropertyNames`; static methods, their Array/Object results, Proxy
  descriptor objects, primitive wrappers, and generated errors use the method
  Realm and survive callback/coercion GC. `groupBy` uses a cached direct
  iterator record with zero-argument metered steps,
  original-completion-preserving close semantics, fallible group storage, and
  Realm-local error/result objects. Native errors materialize before close and
  output is metered per group; step errors and host Fuel do not invoke user
  cleanup. `preventExtensions`, integrity-level
  tests, sealing, and freezing cover every observable exotic object; Proxy
  forwarding is iterative, fuel-metered, GC-rooted, and validates truthy traps
  through the target's complete nested `[[IsExtensible]]`. Prototype get/set
  forwarding is also iterative, rooted, and fuel-metered; ordinary prototype
  cycle detection has no fixed depth cap. Every Realm's original
  `%Object.prototype%` implements immutable-prototype semantics while remaining
  extensible. Shared `ToObject` coercion rejects `null` and `undefined`; the
  ordinary `Object(nullish)` and `new Object(nullish)` calls still create a
  fresh ordinary object in the active function Realm, construction with a
  distinct `NewTarget` keeps its constructor-derived prototype path, and
  algorithms such as `Object.assign` keep their own specified nullish
  exceptions
- **Number**: `parseInt`/`parseFloat`, `isNaN`, `isFinite`; `Number` statics
  (`isInteger`, `isFinite`, `isNaN`, `isSafeInteger`, constants) and
  `toString(radix)`/`toFixed`/`toPrecision`/`toExponential`
- **String** statics: `String.raw`, `String.fromCodePoint`, `String.fromCharCode`
- **Math**: full set of methods and constants (incl. `imul`, `clz32`,
  `fround`); each Realm owns an independent Math object with the standard
  `Symbol.toStringTag` and Realm-local method identities
- **Proxy**: callability and constructability are fixed from the target when a
  Proxy is created and remain stable after revocation. Transparent `[[Call]]`
  forwarding and Proxy-valued `apply` traps use an iterative, GC-rooted,
  fuel-metered dispatcher. Non-callable traps fail before argument-array
  allocation, and observable argument arrays use the current execution Realm.
  `[[Set]]` forwarding and receiver-side `[[DefineOwnProperty]]` delegation are
  likewise iterative, rooted, and fuel-metered while preserving complete
  versus value-only descriptor presence. Proxy `[[OwnPropertyKeys]]` is also
  iterative and invariant-complete, and `for...in` lazily invokes
  `[[OwnPropertyKeys]]`, `[[GetOwnProperty]]`, and `[[GetPrototypeOf]]` while
  suppressing Symbols and duplicate prototype names
- **Reflect**: all 13 standard methods: `apply`, `construct`, `defineProperty`,
  `deleteProperty`, `get`, `getOwnPropertyDescriptor`, `getPrototypeOf`, `has`,
  `isExtensible`, `ownKeys`, `preventExtensions`, `set`, and `setPrototypeOf`.
  Each Realm owns a distinct namespace and method set with the standard
  `Symbol.toStringTag`; omitted property-key arguments are converted from
  `undefined` after target validation, including Proxy trap, receiver, and
  abrupt-completion behavior. The complete direct Test262 Reflect directory is
  admitted at **153/153**; deeper internal-method limitations remain documented
  separately.
- **WeakMap**/`WeakSet`: object and weakly-holdable Symbol keyed collections.
  Constructors use cached zero-argument iterator records with standard close,
  Realm, rooting, Fuel, and fallible-storage behavior. Methods enforce internal
  brands; WeakMap provides `get`, `set`, `has`, `delete`, `getOrInsert`, and
  `getOrInsertComputed`, while WeakSet provides `add`, `has`, and `delete`.
  Hash-backed storage and registered-Symbol classification give average O(1)
  access. GC resolves transitive WeakMap ephemerons through key-indexed pending
  values and retraces incremental mutations before sweep. The complete pinned WeakMap/WeakSet
  directories are admitted at **226/226**
- **Date**: `now()`, constructor with timestamp, `getTime()`
- **JSON**: `parse` (with reviver) and `stringify` (with replacer/space)
- **RegExp**: literals `/pattern/flags` with `test`, `exec`, `match`, `source`,
  `flags`, `d`-flag match indices, forward lookahead, backward lookbehind,
  legacy quantified lookahead, Unicode named captures/backreferences,
  duplicate names across structurally disjoint alternatives with
  participating-capture selection, and the String-symbol
  match/search/split/replace/matchAll operations
- **Map/Set**: full key/value collections with iteration. Set includes
  `union`, `intersection`, `difference`, `symmetricDifference`, `isSubsetOf`,
  `isSupersetOf`, and `isDisjointFrom`. These methods cache Set-like `size`,
  `has`, `keys`, and iterator `next` observations, preserve live receiver or
  copied-result traversal as required, close active iterators on catchable
  post-step failures, and create results in the calling Realm. Set's ordered
  generation slots, tombstones, stable bounded compaction, and hash index
  preserve deletion/reinsertion order with average O(1) membership and removal
  while Fuel bounds traversal, compaction, and clear work. `forEach` uses the
  same rooted live cursor. Map and Set
  constructors plus `Map.groupBy` use cached zero-argument iterator records with no
  `HasProperty` probe, observe built-in iterator overrides, meter each step,
  preserve original close completions, keep observable state rooted, and use
  fallible native constructor storage. Constructor errors, prototypes, and
  collection iterator prototypes are Realm-local. Duplicate native Set
  insertion does not reserve unused capacity.
  `Map.groupBy` additionally preserves SameValueZero keys without
  `ToPropertyKey`, creates Realm-local group Arrays and Map iterators, and
  bypasses mutable global `Map`, species, and overridden `set` during result
  publication
- **Error**: `Error`/`TypeError`/`RangeError`/`ReferenceError`/`SyntaxError`
- **Intl foundation**: each Realm has its own `%Intl%` ordinary namespace with
  the standard `@@toStringTag`, `getCanonicalLocales`, and base `Intl.Locale`
  constructor/prototype. Locale lists preserve observable
  `length`/`HasProperty`/`Get`/`ToString` order, return Realm-local Arrays, and
  canonicalize Unicode locale identifiers with pinned ICU4X/CLDR alias data.
  Locale objects expose canonical component and Unicode-keyword accessors plus
  branded `toString`, `maximize`, and `minimize`; `firstDayOfWeek` and all seven
  Locale-info methods use generated CLDR calendar, hour-cycle, week,
  text-direction, and canonical IANA region-time-zone data. Region overrides,
  subdivisions, likely regions, and `001` inheritance follow ECMA-402 priority.
  `Intl.supportedValuesOf` exposes fresh Realm-local sorted Arrays for calendar,
  collation, currency, numbering-system, primary time-zone, and sanctioned
  simple-unit keys. Realm-local `Intl.Collator` is callable and constructable,
  negotiates `co`/`kn`/`kf`, caches its bound UTF-16 compare function, and
  returns exact Realm-local resolved options. `String.prototype.localeCompare`
  uses the immutable method-Realm Collator intrinsic. The collation capability
  set contains only values confirmed in ICU4X baked data; currency stays empty
  until NumberFormat or DisplayNames exists.
  Constructor/prototype fallback and fresh result Arrays/objects are
  Realm-correct. Locale, option, likely-subtag, and index scans consume VM fuel

## Type coercion

- `ToPrimitive` honors `valueOf`/`toString` (number hint) and `toString`/`valueOf`
  (string hint); arrays join with `,`. Numeric coercion (`+x`, `1 + obj`,
  arithmetic/bitwise ops) now runs `ToPrimitive` so `+{valueOf(){return 7}}`
  yields `7` instead of `NaN`
- `String()`/`Number()`/`Boolean()` as functions return primitives; `new` constructs
  a wrapper object with the correct prototype

## Embedding

- The optional `serde` Cargo feature converts between RuJa values and
  `serde_json::Value`; arrays and enumerable string-keyed object properties are
  traversed recursively. Undefined, symbols, unsupported heap objects, and
  internal-only values become `null`; BigInts become decimal strings. Valid
  UTF-16 pairs round-trip through host Unicode, while lone surrogates export as
  U+FFFD because `serde_json::String` cannot represent them. Host strings
  should enter through `Value::from_string`; constructing the public
  `Value::String` variant directly bypasses canonicalization. `Vm::to_string`
  and `Vm::to_property_key` expose canonical internal text for engine-facing
  integrations; host display/export code must use `Vm::to_string_pub`. Native
  callbacks returning host-written errors use `Error::host`,
  `Error::syntax_host`, or `Error::type_err_host`; ordinary error constructors
  accept canonical internal text. Both forms display as ordinary host Unicode.
- `cargo run --example embed --features serde` demonstrates sandbox limits,
  native Rust function registration, script execution, and JSON conversion.
- CI builds, tests, and runs clippy across all Cargo targets and features.

## CLI

- `ruja script.js` — run a file
- `ruja -e "code"` — evaluate an expression
- `ruja` — start the REPL
- `--version`, `--help`

---

**Next:** [Known limitations](limitations.md) · [Architecture](architecture.md) · [Back to README](../README.md)
