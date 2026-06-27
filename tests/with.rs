//! `with` statement: dynamic object environment records.

mod common;
use common::run;
use ruja::Value;
use std::rc::Rc;

#[test]
fn with_reads_object_property() {
    let src = r#"
        let o = { x: 1, y: 2 };
        let z = 100;
        let result;
        with (o) {
            result = x + y + z;
        }
        result;
    "#;
    assert_eq!(run(src), Value::Number(103.0));
}

#[test]
fn with_shadows_outer_var() {
    let src = r#"
        let p = { name: "inner" };
        let name = "outer";
        let r;
        with (p) { r = name; }
        r;
    "#;
    assert_eq!(run(src), Value::String(Rc::from("inner")));
}

#[test]
fn with_assignment_writes_to_object() {
    let src = r#"
        let o = { count: 0 };
        with (o) {
            count = count + 5;
        }
        o.count;
    "#;
    assert_eq!(run(src), Value::Number(5.0));
}

#[test]
fn with_outer_var_unchanged_after_block() {
    let src = r#"
        let p = { name: "inner" };
        let name = "outer";
        with (p) { name; }
        name;
    "#;
    assert_eq!(run(src), Value::String(Rc::from("outer")));
}

#[test]
fn with_falls_back_to_outer_scope() {
    // Property only on outer object, not on the `with` object.
    let src = r#"
        let o = { a: 1 };
        let b = 99;
        let r;
        with (o) { r = a + b; }
        r;
    "#;
    assert_eq!(run(src), Value::Number(100.0));
}

#[test]
fn with_reads_function_value() {
    // The `with` object exposes a function-typed property that can be read
    // and called directly (no `this` dependence).
    let src = r#"
        let o = {
            getAnswer: function() { return 42; }
        };
        let r;
        with (o) {
            r = getAnswer();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(42.0));
}

#[test]
fn with_sees_undefined_valued_property() {
    // A property whose value is `undefined` must still be found by `with`
    // (regression for the old undefined-sentinel has_property check).
    let src = r#"
        let o = { x: undefined, real: 5 };
        let r;
        with (o) {
            // x exists (own property) even though its value is undefined.
            r = (typeof x === "undefined") + "|" + real;
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Rc::from("true|5")));
}
