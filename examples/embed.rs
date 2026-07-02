//! Embedding example: run untrusted JS with fuel + heap limits,
//! expose Rust functions, and exchange data via serde.
//!
//! Run with: cargo run --example embed --features serde

#[cfg(feature = "serde")]
fn main() {
    use ruja::{Value, Vm};

    let mut vm = Vm::new();

    // Sandbox: limit execution and memory.
    vm.set_fuel(Some(100_000));
    vm.set_max_heap_objects(Some(10_000));

    // Expose a Rust function to JS.
    vm.register_fn(
        "add",
        |vm, args, _| {
            let a = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
            let b = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
            Ok(Value::Number(a + b))
        },
        2,
    );

    // Run untrusted script.
    let result = vm.run("add(3, 4) * 2;").expect("evaluation failed");
    println!("Result: {:?}", result); // Number(14.0)

    // serde interop: convert a JS object to JSON.
    let json_result = vm.run("({ name: 'world', count: 42 });").expect("failed");
    let json_val = ruja::interop::to_json_value(&mut vm, &json_result);
    println!("As JSON: {}", json_val);
}

#[cfg(not(feature = "serde"))]
fn main() {
    println!("Run with: cargo run --example embed --features serde");
}
