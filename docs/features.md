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
- Template literals with `${}`; numeric separators (`1_000`, `0xff_ff`)
- `eval` (indirect runs globally; direct `eval(...)` runs in the caller's scope)
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

## Async & generators

- `Promise` with `then`/`catch` chaining and microtask draining
- `Promise.resolve`/`Promise.reject`
- `async`/`await` (async functions return a Promise; await resolves it)
- Lazy generators (`function*`/`yield`): pull-based `next()`/`for...of`/spread
  that suspend at each `yield`; supports infinite generators; `next(v)` resumes
  with a value; `return` ends the generator
- `yield*` delegation to generators, arrays, and strings (nestable)
- `async function*`: `next()` returns a Promise; `await` works in the body
- Per-frame generator isolation: a generator body may call `next()` on another
  generator without corrupting either's run-state

## Property model

- `Object.defineProperty` with data and accessor descriptors (`value`/
  `writable`, `get`/`set`); ordinary `[[Set]]` enforces `writable: false`
  (TypeError in strict mode, silent in sloppy) and invokes inherited setters
  through the prototype chain
- `delete` respects `configurable` (false in sloppy, TypeError in strict)
- Symbol-keyed properties: `[Symbol.iterator]` and arbitrary Symbol keys are
  stored/read on objects and skipped by `for...in`/`JSON.stringify`
- Custom `Symbol.iterator`: objects with `[Symbol.iterator]()` are iterable via
  `for...of`/spread; lazy iterators call the JS `next()` per pull

## Standard library

- **Array**: `push`, `pop`, `shift`, `unshift`, `splice`, `map`, `filter`,
  `reduce`, `forEach`, `find`, `findIndex`, `findLast`, `fill`, `some`,
  `every`, `includes`, `indexOf`, `lastIndexOf`, `slice`, `concat`, `join`,
  `flat`, `flatMap`, `at`, `sort`, `reverse`, `copyWithin`, `reduceRight`,
  `toReversed`, `toSorted`, `toSpliced`, `with`; `Array.from`/`of`/`isArray`
- **String**: `charAt`, `charCodeAt`, `slice`, `split`, `replace` (regex
  supported), `replaceAll`, `includes`, `startsWith`, `endsWith`, `repeat`,
  `padStart`/`padEnd`, `at`, `trim`/`trimStart`/`trimEnd`, `substring`, case
  conversions, `codePointAt`, `concat`, `search`
- **Object**: `defineProperty`, `defineProperties`, `keys`, `values`,
  `entries`, `assign`, `create`, `getPrototypeOf`/`setPrototypeOf`,
  `preventExtensions`/`isExtensible`, `seal`/`isSealed`, `freeze`/`isFrozen`,
  `getOwnPropertyDescriptor`/`getOwnPropertyDescriptors`, `getOwnPropertyNames`
- **Number**: `parseInt`/`parseFloat`, `isNaN`, `isFinite`; `Number` statics
  (`isInteger`, `isFinite`, `isNaN`, `isSafeInteger`, constants) and
  `toString(radix)`/`toFixed`/`toPrecision`/`toExponential`
- **String** statics: `String.raw`, `String.fromCodePoint`, `String.fromCharCode`
- **Math**: full set of methods and constants (incl. `imul`, `clz32`, `fround`)
- **JSON**: `parse` and `stringify`
- **RegExp**: literals `/pattern/flags` with `test`, `exec`, `match`, `source`,
  `flags`; `String.replace` with regex
- **Map/Set**: full key/value collections with iteration
- **Error**: `Error`/`TypeError`/`RangeError`/`ReferenceError`/`SyntaxError`

## Type coercion

- `ToPrimitive` honors `valueOf`/`toString` (number hint) and `toString`/`valueOf`
  (string hint); arrays join with `,`. Numeric coercion (`+x`, `1 + obj`,
  arithmetic/bitwise ops) now runs `ToPrimitive` so `+{valueOf(){return 7}}`
  yields `7` instead of `NaN`
- `String()`/`Number()`/`Boolean()` as functions return primitives; `new` constructs
  a wrapper object with the correct prototype

## CLI

- `ruja script.js` — run a file
- `ruja -e "code"` — evaluate an expression
- `ruja` — start the REPL
- `--version`, `--help`

---

**Next:** [Known limitations](limitations.md) · [Architecture](architecture.md) · [Back to README](../README.md)
