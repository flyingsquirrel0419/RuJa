# Features

## Language

- Arithmetic, comparison, logical, bitwise, and assignment operators
- `var`/`let`/`const` with environment-based scoping
- Control flow: `if`/`else`, `while`, `do...while`, `for`, `for...in`,
  `for...of`, `switch`, `break`/`continue`
- Functions, recursion, arrow functions, and closures (capture + mutation)
- `throw`/`try`/`catch`/`finally` with the `Error` type hierarchy
- Labeled statements (`label:`), `break label`, `continue label`
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
  supported
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

## Property model

- `Object.defineProperty` with data and accessor descriptors (`value`/
  `writable`, `get`/`set`); ordinary `[[Set]]` enforces `writable: false`
  (TypeError in strict mode, silent in sloppy) and invokes inherited setters
  through the prototype chain
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
  `flat`, `flatMap`, `at`, `sort`, `reverse`, `copyWithin`, `reduceRight`,
  `toReversed`, `toSorted`, `toSpliced`, `with`; `Array.from`/`fromAsync`/`of`/
  `isArray`
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
  Realm and survive callback/coercion GC
- **Number**: `parseInt`/`parseFloat`, `isNaN`, `isFinite`; `Number` statics
  (`isInteger`, `isFinite`, `isNaN`, `isSafeInteger`, constants) and
  `toString(radix)`/`toFixed`/`toPrecision`/`toExponential`
- **String** statics: `String.raw`, `String.fromCodePoint`, `String.fromCharCode`
- **Math**: full set of methods and constants (incl. `imul`, `clz32`,
  `fround`); each Realm owns an independent Math object with the standard
  `Symbol.toStringTag` and Realm-local method identities
- **Reflect**: `get`/`set`/`has`/`deleteProperty`/`ownKeys`/`getPrototypeOf`/
  `setPrototypeOf`/`isExtensible`/`preventExtensions`/`apply`/`construct`;
  omitted property-key arguments are converted from `undefined` after target
  validation, including Proxy trap, receiver, and abrupt-completion behavior
- **WeakMap**/`WeakSet`: object-keyed collections (get/set/has/delete)
- **Date**: `now()`, constructor with timestamp, `getTime()`
- **JSON**: `parse` (with reviver) and `stringify` (with replacer/space)
- **RegExp**: literals `/pattern/flags` with `test`, `exec`, `match`, `source`,
  `flags`, `d`-flag match indices, forward lookahead, backward lookbehind,
  legacy quantified lookahead, Unicode named captures/backreferences,
  duplicate names across structurally disjoint alternatives with
  participating-capture selection, and the String-symbol
  match/search/split/replace/matchAll operations
- **Map/Set**: full key/value collections with iteration
- **Error**: `Error`/`TypeError`/`RangeError`/`ReferenceError`/`SyntaxError`

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
  internal-only values become `null`; BigInts become decimal strings.
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
