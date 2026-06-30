//! Regression tests for the execution-fuel mechanism:
//!  - a tight loop is bounded by the fuel budget
//!  - fuel exhaustion is NOT catchable by a JS try/catch (a host-level abort)

use ruja::Vm;

#[test]
fn fuel_bounds_infinite_loop() {
    let mut vm = Vm::new();
    vm.set_fuel(Some(10_000));
    let err = vm.run("var i=0; while(true){i++;}").unwrap_err();
    assert!(
        err.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        err
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn fuel_unbounded_by_default() {
    let mut vm = Vm::new();
    assert_eq!(vm.fuel_remaining(), None);
    // a bounded loop completes fine without a fuel limit
    let v = vm
        .run("var s=0; for(let i=0;i<1000;i++){s+=i;} s;")
        .unwrap();
    assert_eq!(v, ruja::Value::Number(499500.0));
}

#[test]
fn fuel_can_be_refilled_between_runs() {
    let mut vm = Vm::new();
    vm.set_fuel(Some(100));
    let _ = vm.run("while(true){}");
    // exhausted; refill and a fresh run succeeds
    vm.set_fuel(Some(1_000_000));
    let v = vm.run("1+1").unwrap();
    assert_eq!(v, ruja::Value::Number(2.0));
}

#[test]
fn fuel_exhaustion_is_uncatchable() {
    // Untrusted code must not be able to swallow a fuel exhaustion in a
    // try/catch and keep looping. The catch must not fire.
    let mut vm = Vm::new();
    vm.set_fuel(Some(5_000));
    let src = "var n=0; for(;;){ try { while(true){} } catch(e){ n++; if(n>2){throw 'done';} } }";
    let err = vm.run(src).unwrap_err();
    // The script never reaches its own `throw 'done'`: fuel exhaustion aborts.
    assert!(
        err.to_string().contains("fuel exhausted"),
        "expected uncatchable fuel exhaustion, got: {}",
        err
    );
}

#[test]
fn normal_errors_remain_catchable() {
    // Fuel change must not break ordinary try/catch of catchable errors.
    let mut vm = Vm::new();
    vm.set_fuel(Some(1_000_000));
    let v = vm
        .run("var r; try { null.x; } catch(e) { r = 'caught ' + (e instanceof Error); } r;")
        .unwrap();
    assert_eq!(v, ruja::Value::String(std::sync::Arc::from("caught true")));
}
