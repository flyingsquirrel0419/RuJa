//! `with` statement: dynamic object environment records.

mod common;
use common::run;
use ruja::Value;
use std::sync::Arc;

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
    assert_eq!(run(src), Value::String(Arc::from("inner")));
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
    assert_eq!(run(src), Value::String(Arc::from("outer")));
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
    assert_eq!(run(src), Value::String(Arc::from("true|5")));
}

// ---- `with` rebinding of `this` for unqualified calls (#6) ----

#[test]
fn with_unqualified_call_binds_this_to_object() {
    // `with(o){ getThis() }` binds `this` to `o` when `getThis` is found on `o`.
    let src = r#"
        let o = { x: 42, getThis: function() { return this.x; } };
        let r;
        with (o) {
            r = getThis();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(42.0));
}

#[test]
fn with_unqualified_call_this_is_object() {
    // Inside the with-block call, `this` is the with object itself.
    let src = r#"
        let o = { whoami: function() { return this; } };
        let r;
        with (o) {
            r = whoami() === o;
        }
        r;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn with_this_does_not_leak_to_outer_call() {
    // A plain unqualified call outside the with-block keeps `this` as the
    // global object in sloppy mode; the with-rebinding must not leak past
    // the block. Inside the block the call rebinds to `o`, outside it is
    // the global object.
    let src = r#"
        let o = { tag: "with-obj", f: function() { return this.tag; } };
        function g() { return (this === undefined) ? "none" : "leaked"; }
        let inside;
        with (o) { inside = f(); }
        let outside = g();
        inside + "|" + outside;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("with-obj|leaked")));
}

#[test]
fn with_this_not_set_when_name_not_on_object() {
    // If the called name is NOT a property of the with object, `this` is the
    // global object in sloppy mode (the function is resolved lexically, not
    // via the with object).
    let src = r#"
        function g() { return this === globalThis; }
        let o = { x: 1 };
        let r;
        with (o) {
            r = g();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn with_this_nested_inner_object_wins() {
    // Nested with-blocks: the innermost object that has the property provides
    // `this`.
    let src = r#"
        let outer = { tag: "outer", f: function() { return this.tag; } };
        let inner = { tag: "inner", f: function() { return this.tag; } };
        let r;
        with (outer) {
            with (inner) {
                r = f();
            }
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("inner")));
}

#[test]
fn with_this_uses_outer_when_inner_lacks_property() {
    // If the inner with object lacks the property, the outer one supplies both
    // the function and the `this` binding.
    let src = r#"
        let outer = { tag: "outer", f: function() { return this.tag; } };
        let inner = { other: 1 };
        let r;
        with (outer) {
            with (inner) {
                r = f();
            }
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("outer")));
}

#[test]
fn with_this_method_call_still_binds_receiver() {
    // `obj.method()` (qualified) must keep binding `this` to `obj`, unaffected
    // by an enclosing with-block.
    let src = r#"
        let o = { x: 7 };
        let receiver = { x: 99, m: function() { return this.x; } };
        let r;
        with (o) {
            r = receiver.m();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(99.0));
}

#[test]
fn with_this_function_reads_property_via_this() {
    // The function resolved via `with` can read other properties through `this`.
    let src = r#"
        let o = { a: 3, b: 4, sum: function() { return this.a + this.b; } };
        let r;
        with (o) {
            r = sum();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(7.0));
}
