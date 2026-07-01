# Changelog

## [Unreleased]

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

## [0.2.0] - 2026-06-28

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
