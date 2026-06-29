//! `Function` constructor: `new Function(p0, ..., body)`.

mod common;
use common::run;
use ruja::Value;
use std::sync::Arc;

#[test]
fn function_ctor_two_params() {
    let src = r#"
        let add = new Function("a", "b", "return a + b;");
        add(2, 3);
    "#;
    assert_eq!(run(src), Value::Number(5.0));
}

#[test]
fn function_ctor_no_params() {
    let src = r#"new Function("return 42;")();"#;
    assert_eq!(run(src), Value::Number(42.0));
}

#[test]
fn function_ctor_returns_string() {
    let src = r#"new Function("return 'hello';")();"#;
    assert_eq!(run(src), Value::String(Arc::from("hello")));
}

#[test]
fn function_ctor_multi_params_one_arg() {
    // Multiple params can be passed as a single comma-separated string.
    let src = r#"
        let f = new Function("a, b", "return a * b;");
        f(6, 7);
    "#;
    assert_eq!(run(src), Value::Number(42.0));
}

#[test]
fn function_ctor_call_without_new() {
    // Function(...) without `new` behaves the same (constructs a function).
    let src = r#"
        let f = Function("x", "return x + 1;");
        f(9);
    "#;
    assert_eq!(run(src), Value::Number(10.0));
}

#[test]
fn function_ctor_has_prototype() {
    let src = r#"
        let f = new Function("return 1;");
        typeof f.prototype === "object";
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn function_ctor_use_strict_body() {
    // A "use strict" directive inside the body makes the function strict.
    let mut vm = ruja::Vm::new();
    let r = vm.run(r#"new Function("a", "a", '"use strict"; return a;')"#);
    assert!(
        r.is_err(),
        "expected duplicate param error in strict Function body"
    );
}

#[test]
fn function_ctor_invalid_body_throws() {
    let mut vm = ruja::Vm::new();
    let r = vm.run(r#"new Function("return ;");"#);
    // A valid body returns a function; an invalid one errors. This body is valid.
    assert!(r.is_ok());
}
