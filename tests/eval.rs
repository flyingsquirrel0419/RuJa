//! `eval` (indirect + direct).

mod common;
use common::run;
use ruja::Value;
use std::rc::Rc;

#[test]
fn eval_arithmetic() {
    assert_eq!(run(r#"eval("1 + 2 * 3")"#), Value::Number(7.0));
}

#[test]
fn eval_returns_non_string_unchanged() {
    assert_eq!(run(r#"eval(42)"#), Value::Number(42.0));
    assert_eq!(run(r#"eval(null)"#), Value::Null);
}

#[test]
fn eval_reads_global_var() {
    let src = r#"
        let x = 10;
        eval("x + 5");
    "#;
    assert_eq!(run(src), Value::Number(15.0));
}

#[test]
fn eval_var_leaks_to_global() {
    let src = r#"
        eval("var leaked = 99");
        leaked;
    "#;
    assert_eq!(run(src), Value::Number(99.0));
}

#[test]
fn indirect_eval_runs_in_global_scope() {
    let src = r#"
        function f() {
            let local = 42;
            let e = eval;       // indirect eval reference
            return e("typeof local");
        }
        f();
    "#;
    assert_eq!(run(src), Value::String(Rc::from("undefined")));
}

#[test]
fn direct_eval_reads_caller_local() {
    let src = r#"
        function f() {
            let local = 42;
            return eval("local");
        }
        f();
    "#;
    assert_eq!(run(src), Value::Number(42.0));
}

#[test]
fn direct_eval_assigns_caller_var() {
    let src = r#"
        function f() {
            let a = 1;
            let b = 2;
            eval("a = a + b");
            return a;
        }
        f();
    "#;
    assert_eq!(run(src), Value::Number(3.0));
}

#[test]
fn eval_can_define_and_call_function() {
    let src = r#"
        eval("function sq(n) { return n * n; }");
        sq(7);
    "#;
    assert_eq!(run(src), Value::Number(49.0));
}

#[test]
fn direct_eval_with_spread_args_still_direct() {
    // eval(src, ...rest) must remain a direct eval (first arg = source).
    let src = r#"
        function f() {
            let local = 99;
            return eval("local", ...[1, 2, 3]);
        }
        f();
    "#;
    assert_eq!(run(src), Value::Number(99.0));
}
