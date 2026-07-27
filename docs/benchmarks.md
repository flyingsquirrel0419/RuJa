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
- **Boa**: `boa` CLI v0.21.1 (pure-Rust JS engine, closest peer)
- **QuickJS**: `qjs` (system package, version 2021-03-27)
- **Node.js**: V8 JIT (for reference; not a fair comparison for an
  embeddable engine, but shows the ceiling)

All benchmarks run on the same machine (x86_64 Linux, release builds).
Each test runs the workload 10 times and reports the average.

## Results

| Benchmark | RuJa | Boa | QuickJS | Node.js (V8) |
|-----------|------|-----|---------|-------------|
| fib(25) | 719 ms | 53 ms | 5.2 ms | 0.6 ms |
| loop 100k | 248 ms | 59 ms | 3.4 ms | 0.3 ms |
| array push 10k | 31 ms | 6.1 ms | 0.6 ms | 0.2 ms |

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

RuJa is roughly **14x slower** than Boa (the closest pure-Rust peer),
**100-140x slower** than QuickJS, and **250-1200x slower**
than V8 on these benchmarks. This is expected for a pure-Rust bytecode
interpreter with `Mutex`-guarded `Send` semantics and no JIT. The overhead
comes from:

- **`Arc`/`Mutex` everywhere**: every heap access, property lookup, and
  variable read goes through a `Mutex` lock (required for `Send` without
  `unsafe`). This is the single largest overhead source and the main reason
  RuJa is ~14x slower than Boa, which uses `Rc`/`RefCell` (no synchronization).
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

## BigInt shared-storage check

The immutable BigInt representation has two retained Criterion workloads:

| Benchmark | Current result |
|-----------|----------------|
| `bigint_value_clone_16k_digits` | 24.6-27.6 ns |
| `bigint_small_arithmetic_10k` | 45.3-53.0 ms |

A release A/B against the preceding downloaded CI binary reads one 64K-digit
BigInt property 100,000 times. Wall time improves from **1.05 s** to **0.74 s**
(about **29.5%**); maximum RSS remains similar at 8.8-9.2 MiB. Five-run small
BigInt arithmetic stays at **0.07 s** on both binaries, within timer noise.
These numbers prove clone-cost removal for this workload, not general BigInt
throughput or sandboxed allocation safety.

Reproduce the retained Criterion measurements and release A/B with:

```sh
cargo bench --bench basic -- bigint_ \
  --sample-size 20 --warm-up-time 1 --measurement-time 2

# Point PREVIOUS_RUJA at the preceding downloaded release artifact.
cargo build --release --all-features
for bin in "$PREVIOUS_RUJA" target/release/ruja; do
  echo "$bin"
  /usr/bin/time -f '%e sec %M KiB' "$bin" -e '
    var value = BigInt("9".repeat(65536));
    var object = { value: value };
    var result;
    for (var i = 0; i < 100000; i++) result = object.value;
    result === value;
  ' >/dev/null
done
```

## Compact PropertyKey checks

`property_key_numeric_lookup_10k` and `property_key_string_lookup_10k` isolate
prebuilt `IndexMap<PropertyKey, _>` lookup from VM creation and compilation.
They catch numeric stack-format/hash cost and ordinary Arc-backed string-key
regressions separately. The representation unit test also requires
`size_of::<PropertyKey>() == size_of::<Arc<str>>()`; the rejected decimal-byte
enum measured 24 bytes on x86_64, while the retained nested `u32` representation
is 16 bytes. The inline form is limited to 64-bit targets: wasm32 keeps the
previous Arc-backed numeric key and 8-byte `PropertyKey` rather than accepting
the safe nested representation's 12-byte layout.

One `--quick` run on a heavily shared two-CPU host measured 10,000 numeric
lookups at 273 microseconds and string lookups at 151-155 microseconds. The
rough 12 ns per-lookup difference is the bounded cost of formatting index bytes
for string-compatible hashing. End-to-end wall-time samples were too noisy at
host load 9-14 to support a regression claim, so they are not treated as
release evidence.

```sh
cargo bench --bench basic 'property_key_|array_index|ordinary_set_receiver' \
  -- --sample-size 20 --warm-up-time 1 --measurement-time 3
```

## Computed Reference checks

`computed_reference_numeric_30k` and `computed_reference_string_30k` reuse one
VM and call precompiled functions, excluding VM construction and parsing. Each
function performs 10,000 compound, logical, and update operations against
computed properties. The string workload is a control for general Reference
dispatch; on x86_64, the numeric workload catches accidental key
materialization. On wasm32 it covers only the redundant opcode/value handoff
because the final numeric key remains Arc-backed.

One sequential `--quick` A/B measured the preceding numeric path at **100.89
ms** and the direct structured-key path at **100.06 ms**. String control moved
from **99.78 ms** to **100.86 ms**. These roughly 1% shifts are shared-host
timer noise, not a throughput claim. The deterministic evidence is compiler
coverage proving the three forms emit `MakePropertyRef` without an earlier
`ToPropertyKey`.

After defensive Reference-record clones were removed, two short sequential
samples placed the current numeric workload at roughly **82.6-83.0 ms** and
the string control at **83.7-84.4 ms**. The preceding source measured roughly
**83.9-84.7 ms** and **85.1-88.3 ms**, respectively. The shared host and small
sample size make these no-regression checks, not a throughput claim. The
deterministic evidence is the removed clone sites plus direct root-identity
tests; retained `Dup` still clones one boxed Reference per operation.

After all 24 retained reads moved from `Dup; GetValue` to
`GetValueKeepReference`, two more short sequential samples placed numeric at
roughly **82.96-83.41 ms**, versus **84.60-84.87 ms** on the preceding source.
String control was roughly **84.9-86.8 ms** current versus **84.9-86.3 ms**
preceding, an overlapping result. Treat the numeric shift as about a 2% local
workload improvement rather than a general throughput claim. Compiler
inspection proving 24 eliminated boxed-Reference clones is deterministic.

```sh
cargo bench --bench basic -- computed_reference --quick
```

## Non-index Number PropertyKey checks

`non_index_numeric_property_key_30k` performs 30,000 `in` conversions using
`-1`, `1.5`, and `1e21`; the string-key twin controls operator and map lookup
cost. Setup requires both fixtures to return 30,000 before timing. The fixture
is applied unchanged to both source revisions.

One sequential forced-rebuild A/B measured numeric conversion at **65.796 ms**
with the stack formatter versus **70.611 ms** on the preceding source. String
control measured **65.932 ms** versus **66.754 ms**. Criterion reported no
significant change; the deterministic evidence is removal of the intermediate
`String`, exact differential formatting over 20,000 bit patterns, and pinned
Test262 identity rather than a throughput claim.

```sh
cargo bench --bench basic -- non_index_ --quick
```

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
