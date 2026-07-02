//! Plugin runner demo: execute untrusted user JS plugins with fuel,
//! heap limits, and a sandboxed API surface.
//!
//! This demonstrates RuJa's core use case — running untrusted scripts
//! safely inside a host process. Each plugin gets:
//!   - 50k opcode fuel (prevents infinite loops)
//!   - 500-object heap cap (prevents OOM)
//!   - A curated API (log, fetch_data, compute_hash)
//!   - Catchable errors (plugins fail gracefully, host survives)
//!
//! Run with: cargo run --example plugin_runner --release

use ruja::{Value, Vm};

fn main() {
    println!("=== RuJa Plugin Runner Demo ===\n");

    // --- Plugin 1: a well-behaved plugin ---
    let plugin_ok = r#"
        // This plugin computes a hash of some data and logs it.
        var data = fetch_data("user_123");
        var hash = compute_hash(data + ":salt");
        log("Hash computed: " + hash);
        hash;
    "#;

    // --- Plugin 2: a malicious plugin (infinite loop) ---
    let plugin_loop = r#"
        while (true) { /* spin forever */ }

        // Fuel exhaustion will catch this before any heap allocation happens
    "#;

    // --- Plugin 3: a plugin that allocates excessively ---
    let plugin_oom = r#"
        var arr = [];
        for (var i = 0; i < 1000000; i++) { arr.push({ x: i }); }
    "#;

    // --- Plugin 4: a plugin that throws ---
    let plugin_throw = r#"
        throw new Error("plugin self-destruct");
    "#;

    // ok_plugin: normal workload, generous limits
    run_plugin("ok_plugin", plugin_ok, Some(500_000), Some(50_000));
    // loop_plugin: tight fuel to catch infinite loops
    run_plugin("loop_plugin", plugin_loop, Some(100_000), Some(50_000));
    // oom_plugin: generous fuel, tight heap to catch memory bombs
    run_plugin("oom_plugin", plugin_oom, Some(5_000_000), Some(1_000));
    // throw_plugin: normal limits
    run_plugin("throw_plugin", plugin_throw, Some(500_000), Some(50_000));

    println!("\n=== All plugins processed. Host process survived. ===");
}

fn run_plugin(name: &str, source: &str, fuel: Option<i64>, heap: Option<usize>) {
    println!("--- Running plugin: {} ---", name);

    let mut vm = match Vm::new() {
        Ok(vm) => vm,
        Err(e) => {
            println!("  VM init failed: {:?}\n", e);
            return;
        }
    };

    // Sandbox limits — each plugin gets its own resource budget.
    vm.set_fuel(fuel);
    vm.set_max_heap_objects(heap);

    // Curated host API: log to stdout
    vm.register_fn(
        "log",
        |_vm, args, _| {
            let msg = match args.first() {
                Some(Value::String(s)) => s.as_ref().to_string(),
                Some(v) => format!("{:?}", v),
                None => String::new(),
            };
            println!("  [plugin log] {}", msg);
            Ok(Value::Undefined)
        },
        1,
    )
    .ok();

    // Curated host API: fetch data (simulated)
    vm.register_fn(
        "fetch_data",
        |_vm, args, _| {
            let key = match args.first() {
                Some(Value::String(s)) => s.as_ref().to_string(),
                _ => "unknown".to_string(),
            };
            Ok(Value::String(format!("data_for_{}", key).into()))
        },
        1,
    )
    .ok();

    // Curated host API: compute hash (simulated)
    vm.register_fn(
        "compute_hash",
        |_vm, args, _| {
            let input = match args.first() {
                Some(Value::String(s)) => s.as_ref().to_string(),
                _ => String::new(),
            };
            let mut hash: u32 = 0;
            for c in input.chars() {
                hash = hash.wrapping_mul(31).wrapping_add(c as u32);
            }
            Ok(Value::String(format!("0x{:08x}", hash).into()))
        },
        1,
    )
    .ok();

    match vm.run(source) {
        Ok(result) => println!("  Result: {:?}\n", result),
        Err(e) => match e.kind {
            ruja::ErrorKind::Fuel => {
                println!("  BLOCKED: fuel exhausted (infinite loop detected)\n");
            }
            _ => {
                println!("  Plugin error: {} ({:?})\n", e.message, e.kind);
            }
        },
    }
}
