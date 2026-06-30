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
    assert_eq!(num_str("1e+21"), "1e+21");
    assert_eq!(num_str("6.022e+23"), "6.022e+23");
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
    assert_eq!(v, Value::String(std::sync::Arc::from("1,2")));
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
    assert_eq!(v, Value::String(std::sync::Arc::from("7")));
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
    assert_eq!(v, Value::String(std::sync::Arc::from("2:1,2")));

    let v = run(r#"var a = [1];
           a.length = 3;
           a.length + ':' + (a[2] === undefined ? 'hole' : 'val');"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("3:hole")));
}

// ---------------------------------------------------------------------------
// #6 String()/Number()/Boolean() as functions return primitives
// (previously returned [object Object] via the generic object_constructor)
// ---------------------------------------------------------------------------

#[test]
fn string_as_function_returns_primitive() {
    // String(x) is ToString(x); must be a primitive string, not an object.
    assert_eq!(
        run("typeof String(5) + ':' + String(5)"),
        Value::String(std::sync::Arc::from("string:5"))
    );
    // exponential value routes through num_to_string.
    assert_eq!(
        run("String(5e-17)"),
        Value::String(std::sync::Arc::from("5e-17"))
    );
    assert_eq!(
        run("String(true)"),
        Value::String(std::sync::Arc::from("true"))
    );
    assert_eq!(
        run("String(null)"),
        Value::String(std::sync::Arc::from("null"))
    );
    assert_eq!(
        run("String(undefined)"),
        Value::String(std::sync::Arc::from("undefined"))
    );
}

#[test]
fn string_with_no_argument_is_empty_string() {
    // `String()` is "", distinct from `String(undefined)` which is "undefined".
    assert_eq!(run("String()"), Value::String(std::sync::Arc::from("")));
}

#[test]
fn number_as_function_returns_primitive() {
    assert_eq!(run("Number('42')"), Value::Number(42.0));
    // Number(undefined) -> NaN
    match run("Number(undefined)") {
        Value::Number(n) => assert!(n.is_nan(), "expected NaN"),
        v => panic!("expected number, got {:?}", v),
    }
}

#[test]
fn number_with_no_argument_is_zero() {
    // `Number()` is 0, distinct from `Number(undefined)` which is NaN.
    assert_eq!(run("Number()"), Value::Number(0.0));
}

#[test]
fn boolean_as_function_returns_primitive() {
    assert_eq!(run("Boolean(0)"), Value::Bool(false));
    assert_eq!(run("Boolean('x')"), Value::Bool(true));
    assert_eq!(run("Boolean()"), Value::Bool(false));
    assert_eq!(run("Boolean(null)"), Value::Bool(false));
}

#[test]
fn new_string_number_boolean_return_objects() {
    // `new` must produce objects (typeof "object"), not primitives.
    assert_eq!(
        run("typeof new String(5)"),
        Value::String(std::sync::Arc::from("object"))
    );
    assert_eq!(
        run("typeof new Number(5)"),
        Value::String(std::sync::Arc::from("object"))
    );
    assert_eq!(
        run("typeof new Boolean(5)"),
        Value::String(std::sync::Arc::from("object"))
    );
}

// ---------------------------------------------------------------------------
// #2 deeply-nested expression depth limit (DoS prevention)
// ---------------------------------------------------------------------------

#[test]
fn deeply_nested_expression_throws_syntax_error_not_crash() {
    // Deeply-nested parens previously overflowed the Rust parser stack and
    // aborted the process. It must now fail with a SyntaxError. Run on a
    // large-stack worker because the parser recurses before the depth check
    // trips on the default test-thread stack.
    use std::thread;
    let src = "(".repeat(500) + "1" + &")".repeat(500);
    let src_owned = src.to_string();
    let worker = thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut vm = ruja::Vm::new();
            match vm.run(&src_owned) {
                Ok(_) => String::new(),
                Err(e) => e.to_string(),
            }
        })
        .expect("failed to spawn worker");
    let msg = worker.join().expect("worker panicked");
    assert!(!msg.is_empty(), "expected an error for deeply-nested input");
    assert!(
        msg.contains("nesting depth") || msg.contains("Maximum"),
        "expected depth-limit error, got: {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// #3 Array() constructor returns real arrays
// ---------------------------------------------------------------------------

#[test]
fn array_constructor_with_length() {
    assert_eq!(run("Array(5).length"), Value::Number(5.0));
    assert_eq!(run("new Array(3).length"), Value::Number(3.0));
}

#[test]
fn array_constructor_with_elements() {
    assert_eq!(
        run("Array(1,2,3).join(',')"),
        Value::String(std::sync::Arc::from("1,2,3"))
    );
    assert_eq!(
        run("new Array('a','b').join('-')"),
        Value::String(std::sync::Arc::from("a-b"))
    );
}

#[test]
fn array_constructor_invalid_length_throws_range_error() {
    for src in [
        "Array(-1)",
        "Array(4294967296)",
        "Array(2.5)",
        "new Array(-1)",
    ] {
        let full = format!(
            "var __e = ''; try {{ {} }} catch(e) {{ __e = e.message }}; __e",
            src
        );
        let v = run(&full);
        match v {
            Value::String(s) => assert!(
                s.contains("Invalid array length"),
                "expected Invalid array length, got: {}",
                s
            ),
            other => panic!("expected error string for {}, got {:?}", src, other),
        }
    }
}

// ---------------------------------------------------------------------------
// #4 delete respects configurable
// ---------------------------------------------------------------------------

#[test]
fn delete_non_configurable_returns_false() {
    let v = run(r#"var o = {};
           Object.defineProperty(o, 'a', { value: 1, configurable: false });
           var r = delete o.a;
           r + ':' + o.a"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("false:1")));
}

#[test]
fn delete_non_configurable_strict_throws() {
    let msg = run_err(
        r#"'use strict';
           var o = {};
           Object.defineProperty(o, 'a', { value: 1, configurable: false });
           delete o.a;"#,
    );
    assert!(
        msg.contains("non-configurable"),
        "expected non-configurable error, got: {}",
        msg
    );
}

#[test]
fn delete_normal_property_works() {
    let v = run(r#"var o = { a: 1, b: 2 };
           delete o.a;
           Object.keys(o).join(',')"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("b")));
}

// ---------------------------------------------------------------------------
// #6 ToPrimitive calls valueOf / toString
// ---------------------------------------------------------------------------

#[test]
fn value_of_called_in_to_primitive() {
    let v = run("var o = { valueOf() { return 42 } }; o + 0");
    assert_eq!(v, Value::Number(42.0));
}

#[test]
fn to_string_called_in_to_primitive() {
    let v = run("var o = { toString() { return 'hi' } }; o + ''");
    assert_eq!(v, Value::String(std::sync::Arc::from("hi")));
}

#[test]
fn array_to_primitive_joins() {
    assert_eq!(
        run("[1,2] + [3,4]"),
        Value::String(std::sync::Arc::from("1,23,4"))
    );
    assert_eq!(run("[] + []"), Value::String(std::sync::Arc::from("")));
}

// ---------------------------------------------------------------------------
// #7 const reassignment throws TypeError
// ---------------------------------------------------------------------------

#[test]
fn const_reassignment_throws() {
    let msg = run_err("const x = 1; x = 2;");
    assert!(
        msg.contains("constant variable"),
        "expected constant-variable error, got: {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// #8 labeled statements (break/continue label)
// ---------------------------------------------------------------------------

#[test]
fn labeled_break_works() {
    let v = run(r#"var r = [];
           outer: for (var i = 0; i < 3; i++) {
             for (var j = 0; j < 3; j++) {
               if (j == 1) break outer;
               r.push(i + '-' + j);
             }
           }
           r.join(',')"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("0-0")));
}

#[test]
fn labeled_continue_works() {
    let v = run(r#"var r = [];
           outer: for (var i = 0; i < 3; i++) {
             for (var j = 0; j < 3; j++) {
               if (j == 1) continue outer;
               r.push(i + '-' + j);
             }
           }
           r.join(',')"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("0-0,1-0,2-0")));
}

#[test]
fn labeled_statement_runs_body() {
    // A labeled statement runs its body; here a side effect confirms it.
    let v = run("label1: { var ran = 'yes' }; ran");
    assert_eq!(v, Value::String(std::sync::Arc::from("yes")));
}

// ---------------------------------------------------------------------------
// #5 try/finally control flow (return/throw override)
// ---------------------------------------------------------------------------

#[test]
fn finally_return_overrides_try_return() {
    // finally's return must win over try's return.
    let v = run("function f() { try { return 1; } finally { return 2; } } f()");
    assert_eq!(v, Value::Number(2.0));
}

#[test]
fn finally_runs_after_try_return() {
    // finally must run even when try returns, and the try return still takes
    // effect if finally doesn't return.
    let v = run(r#"var log = [];
           function f() { try { return 1; } finally { log.push('fin'); } }
           var r = f();
           r + ':' + log.join(',')"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("1:fin")));
}

#[test]
fn finally_runs_after_catch() {
    let v = run(r#"var r = [];
           try { throw 1; } catch (e) { r.push('catch'); }
           finally { r.push('fin'); }
           r.join(',')"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("catch,fin")));
}

#[test]
fn finally_return_overrides_catch_return() {
    let v =
        run("function f() { try { throw 1; } catch (e) { return 3; } finally { return 4; } } f()");
    assert_eq!(v, Value::Number(4.0));
}

#[test]
fn finally_runs_on_normal_completion() {
    let v = run(r#"var r = [];
           try { r.push('try'); } finally { r.push('fin'); }
           r.join(',')"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("try,fin")));
}
