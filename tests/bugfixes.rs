//! Regression tests for the five verified engine bugs that were fixed in
//! this pass:
//!   #1 num_to_string floating-point / exponential formatting
//!   #2 writable:false ignored by ordinary [[Set]]
//!   #3 missing call-stack depth limit (process-killing stack overflow)
//!   #4 accessor (get/set) descriptors ignored by defineProperty / get/set
//!   #5 Array.length assignment with an invalid (non-uint32) value
//!
//! These tests are deliberately self-contained so that a regression in any
//! one of the fixes is caught on its own.

mod common;

use common::{run, run_err};
use ruja::Value;

/// Run `src` on a worker thread with a large stack and return the result as
/// an f64 (panics if the result is not a number). Deep recursion up to the
/// engine's call-depth limit needs more than the default test-thread stack,
/// and `Vm`/`Value` are not `Send`, so the value is reduced to an f64 on the
/// worker thread.
fn run_num_big_stack(src: &str) -> f64 {
    use std::thread;
    let src = src.to_string();
    let worker = thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut vm = ruja::Vm::new();
            match vm.run(&src) {
                Ok(ruja::Value::Number(n)) => n,
                Ok(v) => panic!("expected number, got {:?}", v),
                Err(e) => panic!("evaluation errored: {}", e),
            }
        })
        .expect("failed to spawn worker");
    worker.join().expect("worker panicked")
}

/// Run `src` expected to error at runtime, on a large-stack worker thread.
/// Returns the error message as a `String`.
fn run_err_big_stack(src: &str) -> String {
    use std::thread;
    // Copy the source into an owned `String` so the closure is `'static`.
    let src = src.to_string();
    let worker = thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut vm = ruja::Vm::new();
            match vm.run(&src) {
                Err(e) => e.to_string(),
                Ok(v) => panic!("expected error, got value: {:?}", v),
            }
        })
        .expect("failed to spawn worker");
    worker.join().expect("worker panicked")
}

// ---------------------------------------------------------------------------
// #1 num_to_string exponential formatting (must match ECMAScript String(n))
// ---------------------------------------------------------------------------

fn num_str(src: &str) -> String {
    // Force a string coercion via `"" + value`, which routes through
    // to_string -> num_to_string (the path console.log also uses).
    match run(&format!("'' + ({})", src)) {
        Value::String(s) => s.to_string(),
        other => panic!("expected string, got {:?}", other),
    }
}

#[test]
fn num_to_string_small_exponential_is_exact() {
    // Previously produced "4.999999999999999e-17" due to n / 10f64.powi(exp).
    assert_eq!(num_str("5e-17"), "5e-17");
    assert_eq!(num_str("9e-17"), "9e-17");
}

#[test]
fn num_to_string_no_zero_padding_in_exponent() {
    // ECMAScript uses "e-7", never "e-07".
    assert_eq!(num_str("5e-7"), "5e-7");
    assert_eq!(num_str("9.99e-7"), "9.99e-7");
}

#[test]
fn num_to_string_large_exponential_has_explicit_sign() {
    assert_eq!(num_str("1e21"), "1e+21");
    assert_eq!(num_str("6.022e23"), "6.022e+23");
}

#[test]
fn num_to_string_fixed_notation_boundary() {
    // 1e-6 is rendered in fixed notation; 5e-7 in exponential.
    assert_eq!(num_str("1e-6"), "0.000001");
    assert_eq!(num_str("0.0000025"), "0.0000025");
}

// ---------------------------------------------------------------------------
// #2 writable:false honored by ordinary assignment [[Set]]
// ---------------------------------------------------------------------------

#[test]
fn writable_false_rejects_strict_assignment() {
    // In strict mode, writing to a non-writable own data property throws.
    let msg = run_err(
        r#"'use strict';
           var o = {};
           Object.defineProperty(o, 'x', { value: 1, writable: false });
           o.x = 99;"#,
    );
    assert!(
        msg.contains("read only") || msg.contains("Cannot assign"),
        "expected read-only error, got: {}",
        msg
    );
}

#[test]
fn writable_false_silently_ignored_in_non_strict() {
    // In non-strict mode the assignment must fail silently and keep the value.
    let v = run(r#"var o = {};
           Object.defineProperty(o, 'x', { value: 1, writable: false });
           o.x = 99;
           o.x"#);
    assert_eq!(v, Value::Number(1.0));
}

// ---------------------------------------------------------------------------
// #3 call-stack depth limit throws a catchable RangeError
// ---------------------------------------------------------------------------

#[test]
fn deep_recursion_throws_range_error_not_crash() {
    // Unbounded recursion previously overflowed the Rust stack and aborted
    // the process. It must now be catchable.
    let msg = run_err_big_stack(
        r#"function f() { f(); }
           try { f(); } catch (e) { throw e; }"#,
    );
    assert!(
        msg.contains("call stack") || msg.contains("Maximum call"),
        "expected maximum-call-stack error, got: {}",
        msg
    );
}

#[test]
fn deep_but_bounded_recursion_succeeds() {
    // A modest depth (well under the limit) must still work.
    let v = run_num_big_stack("function sum(n) { return n <= 1 ? 1 : n + sum(n - 1); } sum(800);");
    assert_eq!(v, 320400.0);
}

// ---------------------------------------------------------------------------
// #4 accessor (get/set) descriptors
// ---------------------------------------------------------------------------

#[test]
fn define_property_setter_is_invoked() {
    let v = run(r#"var log = [];
           var o = {};
           Object.defineProperty(o, 'x', { set: function (v) { log.push(v); } });
           o.x = 1;
           o.x = 2;
           log.join(',');"#);
    assert_eq!(v, Value::String(std::rc::Rc::from("1,2")));
}

#[test]
fn define_property_getter_is_invoked() {
    let v = run(r#"var o = {};
           Object.defineProperty(o, 'x', { get: function () { return 42; } });
           o.x"#);
    assert_eq!(v, Value::Number(42.0));
}

#[test]
fn inherited_setter_is_invoked_through_prototype_chain() {
    let v = run(r#"var log = [];
           var proto = {};
           Object.defineProperty(proto, 'x', { set: function (v) { log.push(v); } });
           var o = Object.create(proto);
           o.x = 7;
           log.join(',');"#);
    assert_eq!(v, Value::String(std::rc::Rc::from("7")));
}

#[test]
fn define_property_rejects_accessor_plus_value_mix() {
    let msg = run_err(
        r#"var o = {};
           Object.defineProperty(o, 'x', { get: function () {}, value: 1 });"#,
    );
    assert!(
        msg.contains("accessors") || msg.contains("Invalid property descriptor"),
        "expected accessor/value mix error, got: {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// #5 Array.length validation
// ---------------------------------------------------------------------------

#[test]
fn array_length_fractional_throws_range_error() {
    let msg = run_err("var a = [1,2,3]; a.length = 2.5;");
    assert!(msg.contains("Invalid array length"), "got: {}", msg);
}

#[test]
fn array_length_negative_throws_range_error() {
    let msg = run_err("var a = [1,2,3]; a.length = -1;");
    assert!(msg.contains("Invalid array length"), "got: {}", msg);
}

#[test]
fn array_length_non_numeric_throws_range_error() {
    let msg = run_err("var a = [1,2,3]; a.length = 'abc';");
    assert!(msg.contains("Invalid array length"), "got: {}", msg);
}

#[test]
fn array_length_too_large_throws_range_error() {
    // 2^32 is out of the uint32 array-length range.
    let msg = run_err("var a = [1,2,3]; a.length = 4294967296;");
    assert!(msg.contains("Invalid array length"), "got: {}", msg);
}

#[test]
fn array_length_valid_truncates_and_extends() {
    let v = run(r#"var a = [1,2,3];
           a.length = 2;
           a.length + ':' + a[0] + ',' + a[1];"#);
    assert_eq!(v, Value::String(std::rc::Rc::from("2:1,2")));

    let v = run(r#"var a = [1];
           a.length = 3;
           a.length + ':' + (a[2] === undefined ? 'hole' : 'val');"#);
    assert_eq!(v, Value::String(std::rc::Rc::from("3:hole")));
}
