//! Destructuring assignment (non-declaration): `[a, b] = expr`,
//! `{a, b} = expr`, swaps, rest, and nested patterns. Declaration-form
//! destructuring (`let [a] = ...`) is covered elsewhere; these tests
//! target assignment to *existing* bindings.

mod common;
use common::run;
use ruja::Value;

#[test]
fn array_swap() {
    assert_eq!(
        run("var a=1, b=2; [a, b] = [b, a]; a + b;"),
        Value::Number(3.0)
    );
}

#[test]
fn array_assign_to_existing() {
    assert_eq!(
        run("var a=0, b=0; [a, b] = [10, 20]; a + b;"),
        Value::Number(30.0)
    );
}

#[test]
fn array_assign_partial() {
    // Fewer targets than sources: extra sources are dropped.
    assert_eq!(run("var a=0; [a] = [1, 2, 3]; a;"), Value::Number(1.0));
}

#[test]
fn array_assign_with_holes() {
    assert_eq!(
        run("var a=0, c=0; [a, , c] = [1, 2, 3]; a + c;"),
        Value::Number(4.0) // 1 + 3
    );
}

#[test]
fn array_assign_rest() {
    assert_eq!(
        run("var head=0, rest=0; [head, ...rest] = [1, 2, 3]; head + rest.length;"),
        Value::Number(3.0) // 1 + 2
    );
}

#[test]
fn object_assign_shorthand() {
    assert_eq!(
        run("var x=0, y=0; ({x, y} = {x: 5, y: 7}); x + y;"),
        Value::Number(12.0)
    );
}

#[test]
fn object_assign_rename() {
    assert_eq!(
        run("var p=0, q=0; ({a: p, b: q} = {a: 1, b: 2}); p + q;"),
        Value::Number(3.0)
    );
}

#[test]
fn fib_via_destructure_assignment() {
    // The classic infinite fibonacci generator using destructuring swap.
    assert_eq!(
        run(
            "function* fib(){ let [a,b]=[0,1]; while(true){ yield a; [a,b]=[b,a+b]; } } var it=fib(); var s=0; for(var i=0;i<6;i++) s+=it.next().value; s;"
        ),
        Value::Number(12.0) // 0+1+1+2+3+5
    );
}

#[test]
fn nested_array_assign() {
    assert_eq!(
        run("var a=0, b=0; [[a, b]] = [[1, 2]]; a + b;"),
        Value::Number(3.0)
    );
}

#[test]
fn object_shorthand_literal() {
    // `{x, y}` object literal shorthand (not assignment).
    assert_eq!(
        run("var x=1, y=2; var o = {x, y}; o.x + o.y;"),
        Value::Number(3.0)
    );
}

// ---- array destructuring via iterator protocol (#5) ----

#[test]
fn destructure_custom_iterable() {
    let src = r#"
        let custom = { [Symbol.iterator]: function*(){ yield 1; yield 2; yield 3; } };
        let [a, b, c] = custom;
        a + b + c;
    "#;
    assert_eq!(run(src), Value::Number(6.0));
}

#[test]
fn destructure_generator() {
    let src = r#"
        function* gen() { yield 10; yield 20; yield 30; }
        let [a, b, c] = gen();
        a + b + c;
    "#;
    assert_eq!(run(src), Value::Number(60.0));
}

#[test]
fn destructure_generator_rest() {
    let src = r#"
        function* gen() { yield 10; yield 20; yield 30; }
        let [first, ...rest] = gen();
        first + "," + rest.length + "," + rest[0] + "," + rest[1];
    "#;
    assert_eq!(run(src), Value::String(std::sync::Arc::from("10,2,20,30")));
}

#[test]
fn destructure_string_iterable() {
    // Strings are iterable (code points).
    let src = r#"
        let [a, b] = "hi";
        a + b;
    "#;
    assert_eq!(run(src), Value::String(std::sync::Arc::from("hi")));
}

#[test]
fn destructure_short_iterable_pads_undefined() {
    // Fewer values than targets: missing elements bind undefined.
    let src = r#"
        let custom = { [Symbol.iterator]: function*(){ yield 1; } };
        let [a, b, c] = custom;
        a + "|" + (b === undefined) + "|" + (c === undefined);
    "#;
    assert_eq!(run(src), Value::String(std::sync::Arc::from("1|true|true")));
}

#[test]
fn plain_array_destructure_still_works() {
    // Regression: arrays must still destructure by index-equivalent iteration.
    assert_eq!(run("let [a, b] = [5, 6]; a + b;"), Value::Number(11.0));
}
