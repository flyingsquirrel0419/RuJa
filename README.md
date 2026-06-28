# RuJa

A JavaScript engine written in Rust — **bytecode VM** + **mark-and-sweep GC**,
with **zero external dependencies**.

Runs a pragmatic ES5.1 subset plus selected ES2015+ features: classes,
async/await, generators, Promises, destructuring, Symbols, Map/Set, regex,
and more. JavaScript is compiled to a stack-based bytecode and executed on a
custom VM with automatic memory management.

```sh
$ cargo run --release -- examples/fib.js
0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
```

## Quick start

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
class hierarchies, and Promise chaining.

## Library API

```rust
use ruja::{Vm, Value};

fn main() {
    let mut vm = Vm::new();
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

## Documentation

- [Architecture](docs/architecture.md) — pipeline, GC, and module layout
- [Features](docs/features.md) — full language and stdlib reference
- [Limitations](docs/limitations.md) — known gaps and edge cases
- [Changelog](CHANGELOG.md) — release history
- [Roadmap](ROADMAP.md) — future work

## License

MIT
