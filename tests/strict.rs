//! Strict mode: directive prologue parsing and enforcement.

mod common;
use common::run;
use ruja::Value;

// ---- directive prologue detection ----

#[test]
fn use_strict_at_top_enables_strict() {
    // A leading "use strict" makes the program strict: `with` is rejected.
    let mut vm = ruja::Vm::new();
    let r = vm.run("\"use strict\"; var o = {x:1}; with(o){ x; }");
    assert!(r.is_err(), "expected with to be rejected in strict mode");
}

#[test]
fn non_directive_use_strict_does_not_enable() {
    // "use strict" after a non-directive statement is just a string expr.
    let r = run("1; \"use strict\"; var o = {x:2}; var r; with(o){ r = x; } r;");
    assert_eq!(r, Value::Number(2.0));
}

#[test]
fn strict_in_function_via_directive() {
    // A function with a "use strict" directive is strict: `with` inside it is
    // a compile-time SyntaxError, while `with` in the (non-strict) outer
    // program is allowed.
    let mut vm = ruja::Vm::new();
    let r = vm.run(
        r#"var o = {x: 5};
           with(o){ }
           function f() {
               "use strict";
               var p = {y: 6};
               with(p){ y; }
           }
           f();"#,
    );
    assert!(r.is_err(), "expected strict with rejection inside function");
}

#[test]
fn strictness_inherits_into_nested_functions() {
    // An outer "use strict" makes nested functions strict too.
    let mut vm = ruja::Vm::new();
    let r = vm.run(
        r#"\"use strict\"; function outer(){ function inner(){ var o={x:1}; with(o){x;} } inner(); } outer();"#,
    );
    assert!(r.is_err(), "nested function should be strict");
}

// ---- duplicate parameter rejection ----

#[test]
fn strict_rejects_duplicate_params() {
    let mut vm = ruja::Vm::new();
    let r = vm.run("\"use strict\"; function f(a, a){ return a; }");
    assert!(r.is_err(), "expected duplicate param error in strict");
}

#[test]
fn non_strict_allows_duplicate_params_last_wins() {
    // Non-strict: duplicate params allowed, last value wins.
    assert_eq!(
        run("function f(a, a){ return a; } f(1, 2);"),
        Value::Number(2.0)
    );
}

#[test]
fn strict_function_directive_rejects_duplicate_params() {
    let mut vm = ruja::Vm::new();
    let r = vm.run("function f(a, a){ \"use strict\"; return a; }");
    assert!(r.is_err(), "expected duplicate param error");
}

// ---- classes are always strict ----

#[test]
fn class_methods_are_strict_reject_with() {
    let mut vm = ruja::Vm::new();
    let r = vm.run(r#"class C { m(){ var o={x:1}; with(o){ x; } } } new C().m();"#);
    assert!(r.is_err(), "class methods are always strict");
}

// ---- with rejection variants ----

#[test]
fn strict_rejects_with_with_clear_message() {
    let mut vm = ruja::Vm::new();
    let e = vm.run("\"use strict\"; with({}){}").unwrap_err();
    assert!(
        e.to_string().contains("strict"),
        "expected strict-mode message, got: {}",
        e
    );
}

#[test]
fn strict_with_inside_block_scope_also_rejected() {
    let mut vm = ruja::Vm::new();
    let r = vm.run("\"use strict\"; { with({}){} }");
    assert!(r.is_err());
}
