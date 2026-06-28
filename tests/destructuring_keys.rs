//! Computed and numeric property keys in *declaration* destructuring
//! (`let {[k]: a} = o`, `let {1: b} = o`). Assignment-form destructuring is
//! covered in `destructuring.rs`; these tests target the binding form only.

mod common;
use common::run;
use ruja::Value;

#[test]
fn computed_key_object_destructure() {
    assert_eq!(
        run(r#"let k = "x"; let {[k]: a} = {x: 42}; a;"#),
        Value::Number(42.0)
    );
}

#[test]
fn computed_key_object_destructure_rename() {
    // Computed key with explicit binding name.
    assert_eq!(
        run(r#"let k = "v"; let {[k]: p} = {v: 7}; p;"#),
        Value::Number(7.0)
    );
}

#[test]
fn numeric_key_object_destructure() {
    assert_eq!(run(r#"let {1: b} = {"1": 99}; b;"#), Value::Number(99.0));
}

#[test]
fn numeric_key_array_like_destructure() {
    assert_eq!(
        run(r#"let {0: a, 2: c} = [10, 20, 30]; a + c;"#),
        Value::Number(40.0)
    );
}

#[test]
fn computed_key_default() {
    // Computed key whose property is missing uses the default value.
    assert_eq!(
        run(r#"let k = "missing"; let {[k]: a = 5} = {}; a;"#),
        Value::Number(5.0)
    );
}

#[test]
fn numeric_key_default() {
    assert_eq!(run(r#"let {0: a = 7} = {}; a;"#), Value::Number(7.0));
}

#[test]
fn string_key_object_destructure() {
    // Quoted string key with rename.
    assert_eq!(run(r#"let {"foo": p} = {foo: 3}; p;"#), Value::Number(3.0));
}

#[test]
fn computed_key_nested_object() {
    assert_eq!(
        run(r#"let k = "inner"; let {[k]: {x}} = {inner: {x: 8}}; x;"#),
        Value::Number(8.0)
    );
}

#[test]
fn computed_key_in_for_of() {
    let src = r#"
        let s = 0;
        let k = "v";
        for (let {[k]: a} of [{v: 1}, {v: 2}]) { s += a; }
        s;
    "#;
    assert_eq!(run(src), Value::Number(3.0));
}

#[test]
fn computed_key_array_pattern_still_works() {
    // Computed keys only apply to object patterns; array patterns still work.
    assert_eq!(
        run(r#"let k = 1; let [a, b] = [10, 20]; a + b;"#),
        Value::Number(30.0)
    );
}

#[test]
fn numeric_key_zero_index() {
    assert_eq!(
        run(r#"let {0: first} = ["a", "b"]; first;"#),
        Value::String("a".into())
    );
}

#[test]
fn computed_key_with_expression() {
    // Computed key may be an arbitrary expression, not just an identifier.
    assert_eq!(
        run(r#"let {[1 + 1]: b} = {2: 50}; b;"#),
        Value::Number(50.0)
    );
}
