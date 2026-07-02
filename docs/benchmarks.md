# Benchmarks

RuJa is not designed to be the fastest JS engine. It is designed to be a
**safe, embeddable runtime for untrusted scripts** with zero `unsafe`,
fuel-metered execution, and panic-free operation. The tradeoff is speed:
RuJa's bytecode VM with `Arc`/`Mutex`-based `Send` semantics and
worklist-based GC is slower than C-based engines like QuickJS or JIT-based
engines like V8.

These benchmarks are here to be honest about that tradeoff, not to hide it.

## Methodology

- **RuJa**: `cargo bench` (criterion, 100 samples) + standalone `Date.now()`
  timing for cross-engine comparison
- **QuickJS**: `qjs` (system package, version 2021-03-27)
- **Node.js**: V8 JIT (for reference; not a fair comparison for an
  embeddable engine, but shows the ceiling)

All benchmarks run on the same machine (x86_64 Linux, release builds).
Each test runs the workload 10 times and reports the average.

## Results

| Benchmark | RuJa | QuickJS | Node.js (V8) |
|-----------|------|---------|-------------|
| fib(25) | 719 ms | 5.2 ms | 0.6 ms |
| loop 100k | 248 ms | 3.4 ms | 0.3 ms |
| array push 10k | 31 ms | 0.6 ms | 0.2 ms |

### Criterion detail (RuJa only)

Measured via `cargo bench` (criterion, 100 samples):

| Benchmark | Mean |
|-----------|------|
| fib(25) | 758 ms |
| loop_100k | 178 ms |
| array_push_10k | 22 ms |

The standalone timing is slower because each iteration creates a new `Vm`
and runs the entire script (including parse + compile), while criterion
reuses a pre-built `Vm`.

## Interpretation

RuJa is roughly **100-140x slower** than QuickJS and **250-1200x slower**
than V8 on these benchmarks. This is expected for a pure-Rust bytecode
interpreter with `Mutex`-guarded `Send` semantics and no JIT. The overhead
comes from:

- **`Arc`/`Mutex` everywhere**: every heap access, property lookup, and
  variable read goes through a `Mutex` lock (required for `Send` without
  `unsafe`). This is the single largest overhead source.
- **No JIT**: all code is interpreted bytecode. QuickJS has a bytecode
  interpreter too, but it uses direct C struct access without synchronization.
- **Worklist-based GC**: mark-sweep with a worklist (to avoid re-entrant
  mutex locking) is slower than a pointer-following collector.

## When speed matters

For sandboxed plugin/script execution where the host needs fuel limits,
heap caps, and crash guarantees, RuJa's overhead is acceptable — the typical
use case runs short scripts with tight resource limits, not tight inner
loops. If you need high-throughput JS execution, use QuickJS or V8 with
process-level isolation instead.

## Reproducing

```sh
# RuJa
cargo bench

# Cross-engine comparison
cat > /tmp/bench_all.js << 'EOF'
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
var t0 = Date.now();
for (var i = 0; i < 10; i++) fib(25);
console.log("fib(25) avg: " + (Date.now() - t0) / 10 + " ms");

t0 = Date.now();
for (var i = 0; i < 10; i++) {
    var sum = 0;
    for (var j = 0; j < 100000; j++) sum += j;
}
console.log("loop_100k avg: " + (Date.now() - t0) / 10 + " ms");

t0 = Date.now();
for (var i = 0; i < 10; i++) {
    var arr = [];
    for (var j = 0; j < 10000; j++) arr.push(j);
}
console.log("array_push_10k avg: " + (Date.now() - t0) / 10 + " ms");
EOF

qjs /tmp/bench_all.js     # QuickJS
node /tmp/bench_all.js    # Node.js
./target/release/ruja /tmp/bench_all.js  # RuJa
```

---

**Next:** [Architecture](architecture.md) · [Features](features.md) · [Back to README](../README.md)
