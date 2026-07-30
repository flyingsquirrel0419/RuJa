# RuJa

<p align="center"><img src="assets/logo.png" alt="RuJa" width="400"></p>

[![CI](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml/badge.svg)](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/ruja.svg)](https://crates.io/crates/ruja)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

[English](README.md) · [한국어](readme/README.ko.md) · [Español](readme/README.es.md) · [日本語](readme/README.ja.md) · [中文](readme/README.zh.md)

A **sandboxed, embeddable JavaScript runtime** for running untrusted scripts
inside Rust applications — **zero `unsafe`**, **fuel-metered execution**,
**panic-audited**, with **heap and call-stack limits**.

Designed for plugin systems, game scripting, and sandboxed evaluation where
the host process must not crash, hang, or OOM regardless of script input.
JavaScript is compiled to a stack-based bytecode and executed on a custom
VM with mark-and-sweep GC. The VM is `Send` (movable between threads) with
no `unsafe` code anywhere in the engine.

### Sandbox guarantees

- **Execution fuel**: `vm.set_fuel(Some(100_000))` bounds opcode count;
  exhaustion throws a non-catchable `RangeError`
- **Heap limit**: `vm.set_max_heap_objects(Some(10_000))` caps live objects;
  exceeding throws a catchable `RangeError`
- **Call-stack cap**: 512 VM frames max; deep recursion throws `RangeError`,
  not a native crash
- **ReDoS-safe regex**: RE2-style linear-time matching (no backtracking)
- **Panic-free**: 0 `unwrap()` in VM hot path; cargo-fuzz verified (96k+
  iterations without panics)

### Supported language subset

ES5.1 + classes, async/await, generators, Promises with static combinators
and `withResolvers`, Realm-correct `Function`, `AsyncFunction`,
`GeneratorFunction`, and `AsyncGeneratorFunction` constructors,
destructuring,
getters/setters, auto-accessors, audited public and private
field/method/getter/setter/auto-accessor decorator context/access and
replacement, initializer semantics, and restricted
decorator-expression early errors, tagged
templates, Symbols, Map/Set,
WeakMap/WeakSet,
WeakRef/FinalizationRegistry, Reflect, Proxy, resizable ArrayBuffer and growable
SharedArrayBuffer cores, Atomics including worker `wait`/`notify`, `waitAsync`,
and `pause`, Realm-local `%Intl%`, `Intl.getCanonicalLocales`, and the complete
`Intl.Locale` constructor/accessor/likely-subtag/Locale-info surface plus
`Intl.supportedValuesOf`, ICU4X-backed `Intl.Collator`, and intrinsic
`String.prototype.localeCompare`, file-backed
ES Module graphs with named imports/exports and live
bindings, relative dynamic imports, and static/dynamic JSON/text import
attributes,
the realm-specific global `Iterator`, common synchronous iterator prototype
hierarchy, branded Realm-specific String iterators, `Iterator.from`, and
`Iterator.prototype.toArray`, `reduce`, `forEach`, `some`, `every`, and `find`,
static `Iterator.concat`, `Iterator.zip`, and `Iterator.zipKeyed`, plus lazy
`Iterator.prototype.map`, `filter`, `flatMap`, `take`, and `drop`,
length-tracking TypedArray/DataView views, TypedArray constructors,
indexing, `at`/`copyWithin`/`entries`/`fill`/`filter`/`find`/`findIndex`/
`findLast`/`findLastIndex`/`forEach`/`includes`/`indexOf`/`join`/`keys`/
`lastIndexOf`/`map`/`reverse`/`set`/`slice`/
`sort`/`subarray`/`toLocaleString`/`toReversed`/`toSorted`/`reduce`/
`reduceRight`/`some`/`every`/`values`/`with`, BigInt, Date, RegExp `d` match
indices, directional lookahead/lookbehind, Unicode named groups, duplicate
names across disjoint alternatives, mode-aware quantifier/class early errors,
and more.
RuJa does
not claim
full ES conformance — conformance is scoped to this subset. See
[test262 conformance](docs/test262.md#supported-subset) for the exact
feature list and current pass rates, and [limitations](docs/limitations.md)
for intentionally-unsupported features.

**Supported-subset pass rate: 100.0%** (12,765 tests in `language/statements`
+ `language/expressions` on current Test262, unsupported-feature tests
skipped). The latest full matrix is 66.8% of all files and 86.6% of executed
files (32,376 pass / 5,028 fail / 11,062 skip / 3 timeout) — see
[test262 conformance](docs/test262.md)
for why these numbers differ.

```sh
$ cargo run --release -- examples/fib.js
0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
```

## Quick start

Requires Rust 1.88 or newer.

```sh
git clone https://github.com/flyingsquirrel0419/RuJa.git
cd RuJa
cargo build --release

./target/release/ruja script.js   # run a file
./target/release/ruja -e "1+2*3"  # evaluate an expression
./target/release/ruja             # start the REPL
```

## Examples

```javascript
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
console.log([0,1,2,3,4,5,6,7,8,9,10].map(fib).join(", "));
```

More in the [`examples/`](examples/) directory — generators, async/await,
class hierarchies, Promise chaining, and a
[plugin runner demo](examples/plugin_runner.rs) showing how to safely
execute untrusted JS with fuel limits, heap caps, and a curated host API.

## Library API

```rust
use ruja::{Vm, Value};

fn main() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

Register native functions and expose them to JS:

```rust
use ruja::{error::Result, NativeFn, Value, Vm};

fn add(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> Result<Value> {
    let a = vm.to_number(&args[0])?;
    let b = vm.to_number(&args[1])?;
    Ok(Value::Number(a + b))
}

fn main() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn("add", add as NativeFn, 2).unwrap();
    assert_eq!(vm.run("add(3, 4)").unwrap(), Value::Number(7.0));
}
```

> The `Vm` is `Send` (but not `Sync`): it can move between threads, but
> concurrent shared access needs external sync (e.g. `Mutex<Vm>`).
> See [Limitations](docs/limitations.md).

Bound untrusted code with an execution-fuel budget:

```rust
let mut vm = Vm::new().expect("failed to initialize VM");
vm.set_fuel(Some(1_000_000));      // ~1M opcodes before a RangeError
let _ = vm.run("while(true){}");    // returns Err("fuel exhausted")
vm.set_fuel(None);                  // unbounded again
```
The check is cooperative (before each opcode), not preemption - a single
long native call is not subdivided. See [Limitations](docs/limitations.md).

## Documentation

- [Architecture](docs/architecture.md) — pipeline, GC, and module layout
- [Features](docs/features.md) — full language and stdlib reference
- [Limitations](docs/limitations.md) — known gaps and edge cases
- [test262](docs/test262.md) — conformance suite runner and pass rate
- [Benchmarks](docs/benchmarks.md) — honest performance comparison vs QuickJS/Node.js
- [Changelog](CHANGELOG.md) — release history
- [Contributing](CONTRIBUTING.md) — how to propose changes

## License

Apache-2.0

---

⭐ If you find RuJa useful, please consider giving it a star on GitHub — it helps others discover the project.
