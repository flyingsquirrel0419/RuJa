//! Temporal Dead Zone (TDZ) tests for `let`/`const`.
//!
//! Per the ECMAScript spec, `let`/`const` bindings are hoisted to the top of
//! their enclosing block/function scope but remain *uninitialized* until the
//! declaration is evaluated. Accessing (reading or assigning) such a binding
//! before its declaration throws a `ReferenceError`.

mod common;
use common::{run, run_err};
use ruja::Value;

// --- reads before initialization ---

#[test]
fn let_self_reference_init_tdz() {
    let msg = run_err("let x = x;");
    assert!(
        msg.contains("Cannot access 'x' before initialization"),
        "got: {}",
        msg
    );
}

#[test]
fn const_self_reference_init_tdz() {
    let msg = run_err("const c = c;");
    assert!(
        msg.contains("Cannot access 'c' before initialization"),
        "got: {}",
        msg
    );
}

#[test]
fn let_access_before_decl_in_block() {
    let msg = run_err("{ console.log(y); let y = 5; }");
    assert!(
        msg.contains("Cannot access 'y' before initialization"),
        "got: {}",
        msg
    );
}

#[test]
fn let_access_before_decl_in_function() {
    let msg = run_err("function f(){ console.log(z); let z = 5; } f();");
    assert!(
        msg.contains("Cannot access 'z' before initialization"),
        "got: {}",
        msg
    );
}

#[test]
fn const_access_before_decl_in_function() {
    let msg = run_err("function f(){ return c; const c = 1; } console.log(f());");
    assert!(
        msg.contains("Cannot access 'c' before initialization"),
        "got: {}",
        msg
    );
}

#[test]
fn function_shadows_outer_in_tdz() {
    // The inner `g` is in TDZ; the outer `g` is *not* visible.
    let msg = run_err("let g = 1; function f(){ return g; let g = 2; } f();");
    assert!(
        msg.contains("Cannot access 'g' before initialization"),
        "got: {}",
        msg
    );
}

// --- writes before initialization are also TDZ errors ---

#[test]
fn assign_before_let_is_tdz() {
    let msg = run_err("{ x = 5; let x; }");
    assert!(
        msg.contains("Cannot access 'x' before initialization"),
        "got: {}",
        msg
    );
}

// --- const reassignment ---

#[test]
fn const_reassign_in_block_is_typeerror() {
    let msg = run_err("function f(){ const c = 1; c = 2; return c; } f();");
    assert!(
        msg.contains("Assignment to constant variable 'c'"),
        "got: {}",
        msg
    );
}

#[test]
fn const_destructure_reassign_is_typeerror() {
    let msg = run_err("const {a} = {a:1}; a = 2;");
    assert!(
        msg.contains("Assignment to constant variable 'a'"),
        "got: {}",
        msg
    );
}

// --- TDZ is catchable ---

#[test]
fn tdz_caught_by_try_catch() {
    assert_eq!(
        run("(function(){ try { let x = x; } catch (e) { return e.message; } })()"),
        Value::String(std::rc::Rc::from("Cannot access 'x' before initialization"))
    );
}

// --- valid uses still work after declaration ---

#[test]
fn let_shadowing_after_decl_ok() {
    assert_eq!(
        run("let a = 1; { let a = 2; a; } a;"),
        // outer block returns 2, top-level returns 1
        Value::Number(1.0)
    );
}

#[test]
fn let_then_use_same_scope() {
    assert_eq!(
        run("(function(){ let b = 3; return b * 2; })()"),
        Value::Number(6.0)
    );
}

#[test]
fn const_destructure_works() {
    assert_eq!(run("const {a, b} = {a:1, b:2}; a + b;"), Value::Number(3.0));
}

#[test]
fn let_destructure_array_works() {
    assert_eq!(run("let [x, y] = [10, 20]; x + y;"), Value::Number(30.0));
}

#[test]
fn let_in_for_of_works() {
    assert_eq!(
        run("let sum = 0; for (let x of [1,2,3]) sum += x; sum;"),
        Value::Number(6.0)
    );
}

#[test]
fn let_in_for_loop_works() {
    assert_eq!(
        run("let s = 0; for (let i = 0; i < 3; i++) s += i; s;"),
        Value::Number(3.0)
    );
}

#[test]
fn closure_over_let_works() {
    assert_eq!(
        run("function f(){ let x = 10; return function(){ return x; }; } f()();"),
        Value::Number(10.0)
    );
}

// ---- default-parameter self-reference TDZ ----

#[test]
fn default_param_self_reference_is_tdz() {
    // `function f(a = a)` must throw ReferenceError when the default is used,
    // because `a` is in the TDZ during default evaluation.
    use ruja::Value;
    let mut vm = ruja::Vm::new();
    match vm.run("function f(a = a) { return a; } f();") {
        Err(e) => assert!(
            e.to_string().contains("before initialization")
                || e.to_string().contains("ReferenceError"),
            "expected TDZ/ReferenceError, got: {}",
            e
        ),
        Ok(v) => panic!("expected error, got value: {:?}", v),
    }
    let _ = Value::Undefined;
}

#[test]
fn default_param_normal_default_still_works() {
    // Regression: a normal default must still apply when arg is omitted.
    let src = r#"
        function f(a = 5) { return a; }
        f();
    "#;
    assert_eq!(run(src), ruja::Value::Number(5.0));
}

#[test]
fn default_param_cross_reference_works() {
    // A default may reference an earlier parameter that is already initialized.
    let src = r#"
        function h(a = 10, b = a + 1) { return b; }
        h();
    "#;
    assert_eq!(run(src), ruja::Value::Number(11.0));
}
