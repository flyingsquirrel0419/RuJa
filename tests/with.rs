//! `with` statement: dynamic object environment records.

mod common;
use common::{run, run_err};
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
fn with_reads_inherited_property() {
    let src = r#"
        let proto = { x: 1 };
        let o = Object.create(proto);
        let r;
        with (o) {
            r = x;
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(1.0));
}

#[test]
fn with_assignment_to_inherited_property_creates_own_property() {
    let src = r#"
        let proto = { x: 1 };
        let o = Object.create(proto);
        with (o) {
            x = 2;
        }
        proto.x + ":" + o.x + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1:2:true")));
}

#[test]
fn with_compound_assignment_uses_inherited_property_reference() {
    let src = r#"
        let proto = { x: 1 };
        let o = Object.create(proto);
        with (o) {
            x += 4;
        }
        proto.x + ":" + o.x + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1:5:true")));
}

#[test]
fn with_inherited_method_call_binds_this_to_with_object() {
    let src = r#"
        let proto = { f: function() { return this.tag; } };
        let o = Object.create(proto);
        o.tag = "with-object";
        let r;
        with (o) {
            r = f();
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("with-object")));
}

#[test]
fn with_boxes_primitive_binding_object() {
    let src = r#"
        let r;
        with ("abc") {
            r = length;
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(3.0));
}

#[test]
fn with_unscopables_hides_object_binding() {
    let src = r#"
        let x = "outer";
        let o = { x: "inner" };
        o[Symbol.unscopables] = { x: true };
        let r;
        with (o) {
            r = x;
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("outer")));
}

#[test]
fn with_unscopables_assignment_uses_outer_binding() {
    let src = r#"
        let x = 1;
        let o = { x: 10 };
        o[Symbol.unscopables] = { x: true };
        with (o) {
            x = 2;
        }
        o.x + ":" + x;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("10:2")));
}

#[test]
fn with_unscopables_getter_not_referenced_when_property_absent() {
    let src = r#"
        let calls = 0;
        let x = 7;
        let o = {};
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() {
                calls += 1;
                return { x: true };
            }
        });
        let r;
        with (o) {
            r = x;
        }
        r + ":" + calls;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("7:0")));
}

#[test]
fn with_unscopables_primitive_value_is_ignored() {
    let src = r#"
        let marker = {};
        let o = { x: marker };
        o[Symbol.unscopables] = "x";
        let r;
        with (o) {
            r = x === marker;
        }
        r;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn with_unscopables_getter_error_propagates_when_property_exists() {
    let err = run_err(
        r#"
        let o = { x: 1 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() { throw new Error("boom"); }
        });
        with (o) {
            x;
        }
        "#,
    );
    assert!(err.contains("boom"));
}

#[test]
fn with_unscopables_update_expression_checks_once() {
    let src = r#"
        let calls = 0;
        let x = 1;
        let o = { x: 10 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() {
                calls += 1;
                return { x: false };
            }
        });
        with (o) {
            x++;
        }
        o.x + ":" + x + ":" + calls;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("11:1:1")));
}

#[test]
fn with_unscopables_deleted_binding_then_strict_get_throws() {
    let err = run_err(
        r#"
        let calls = 0;
        let o = { x: 1 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() {
                calls += 1;
                delete o.x;
                return null;
            }
        });
        with (o) {
            (function() {
                "use strict";
                x;
            })();
        }
        "#,
    );
    assert!(err.contains("ReferenceError"));
}

#[test]
fn with_unscopables_deleted_binding_then_strict_set_throws() {
    let err = run_err(
        r#"
        let o = { x: 1 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() {
                delete o.x;
                return null;
            }
        });
        with (o) {
            (function() {
                "use strict";
                x = 2;
            })();
        }
        "#,
    );
    assert!(err.contains("ReferenceError"));
}

#[test]
fn with_delete_identifier_deletes_object_environment_binding() {
    let src = r#"
        var o = { x: 2 };
        var deleted;
        with (o) {
            deleted = delete x;
        }
        deleted + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true:false")));
}

#[test]
fn with_delete_identifier_uses_inherited_object_environment_binding() {
    let src = r#"
        var x = 1;
        var proto = { x: 2 };
        var o = Object.create(proto);
        var deleted;
        with (o) {
            deleted = delete x;
        }
        deleted + ":" + x + ":" + o.hasOwnProperty("x") + ":" + proto.x;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true:1:false:2")));
}

#[test]
fn with_delete_identifier_honors_unscopables() {
    let src = r#"
        var x = 1;
        var o = { x: 2 };
        o[Symbol.unscopables] = { x: true };
        var deleted;
        with (o) {
            deleted = delete x;
        }
        deleted + ":" + x + ":" + o.hasOwnProperty("x") + ":" + o.x;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("false:1:true:2")));
}

#[test]
fn with_delete_identifier_propagates_unscopables_getter_error() {
    let err = run_err(
        r#"
        var o = { x: 2 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() { throw new Error("boom"); }
        });
        with (o) {
            delete x;
        }
        "#,
    );
    assert!(err.contains("boom"), "got: {err}");
}

#[test]
fn with_var_initializer_resolves_binding_before_rhs() {
    let src = r#"
        var obj = { test262id: 1 };
        with (obj) {
          var test262id = delete obj.test262id;
        }
        obj.test262id + ":" + test262id;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true:undefined")));
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

#[test]
fn with_statement_normal_completion_value() {
    assert_eq!(run("1; with ({}) { }"), Value::Undefined);
    assert_eq!(run("2; with ({}) { 3; }"), Value::Number(3.0));
}

#[test]
fn with_statement_abrupt_empty_completion_value() {
    assert_eq!(
        run("1; do { 2; with ({}) { 3; break; } 4; } while (false);"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("5; do { 6; with ({}) { break; } 7; } while (false);"),
        Value::Undefined
    );
    assert_eq!(
        run("8; do { 9; with ({}) { 10; continue; } 11; } while (false);"),
        Value::Number(10.0)
    );
    assert_eq!(
        run("12; do { 13; with ({}) { continue; } 14; } while (false);"),
        Value::Undefined
    );
}

#[test]
fn with_single_statement_rejects_let_array_expression_start() {
    let err = run_err(
        r#"
        if (false) {
            with ({}) let
            [a] = 0;
        }
        "#,
    );
    assert!(err.contains("SyntaxError"));
}
