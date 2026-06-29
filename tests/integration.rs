//! Core language tests: arithmetic, variables, control flow, functions,
//! closures, objects/arrays/strings, prototypes, and `this` binding.

mod common;
use common::run;
use ruja::Value;
use std::sync::Arc;

#[test]
fn arithmetic() {
    assert_eq!(run("1 + 2 * 3;"), Value::Number(7.0));
    assert_eq!(run("(1 + 2) * 3;"), Value::Number(9.0));
    assert_eq!(run("10 % 3;"), Value::Number(1.0));
}

#[test]
fn variables() {
    assert_eq!(run("let x = 5; let y = 10; x + y;"), Value::Number(15.0));
    assert_eq!(run("var a = 1; a = 2; a;"), Value::Number(2.0));
}

#[test]
fn control_flow() {
    assert_eq!(
        run("let s = 0; let i = 0; while (i < 3) { s += i; i++; } s;"),
        Value::Number(3.0)
    );
}

#[test]
fn functions() {
    assert_eq!(
        run("function add(a, b) { return a + b; } add(3, 4);"),
        Value::Number(7.0)
    );
    assert_eq!(
        run("function fact(n) { return n <= 1 ? 1 : n * fact(n-1); } fact(5);"),
        Value::Number(120.0)
    );
}

#[test]
fn recursion() {
    assert_eq!(
        run("function fib(n){ if(n<=1) return n; return fib(n-1)+fib(n-2); } fib(10);"),
        Value::Number(55.0)
    );
}

#[test]
fn objects() {
    assert_eq!(
        run("let p = { x: 1, y: 2 }; p.x + p.y;"),
        Value::Number(3.0)
    );
}

#[test]
fn arrays() {
    assert_eq!(run("let a = [1, 2, 3]; a[0] + a[2];"), Value::Number(4.0));
    assert_eq!(run("[1,2,3].length;"), Value::Number(3.0));
}

#[test]
fn typeof_values() {
    assert_eq!(run("typeof 42;"), Value::String(Arc::from("number")));
    assert_eq!(run("typeof 's';"), Value::String(Arc::from("string")));
    assert_eq!(
        run("typeof undefined;"),
        Value::String(Arc::from("undefined"))
    );
    assert_eq!(run("typeof null;"), Value::String(Arc::from("object")));
}

#[test]
fn globals() {
    assert_eq!(run("parseInt('42');"), Value::Number(42.0));
    assert_eq!(run("isNaN(NaN);"), Value::Bool(true));
}

#[test]
fn prototype_inheritance() {
    let src = r#"function Shape() {} Shape.prototype.describe = function() { return 'shape'; }; let s = new Shape(); s.describe();"#;
    assert_eq!(run(src), Value::String(Arc::from("shape")));
}

#[test]
fn nested_functions() {
    let src = r#"function outer() { let x = 10; function inner() { return x; } return inner(); } outer();"#;
    assert_eq!(run(src), Value::Number(10.0));
}

#[test]
fn closures() {
    let src = r#"
        function mk() { let c = 0; return function() { c = c + 1; return c; }; }
        let f = mk(); f(); f();
    "#;
    assert_eq!(run(src), Value::Number(2.0));
}

#[test]
fn closure_capture_read() {
    assert_eq!(
        run("function mk(){ let c = 42; return function(){ return c; }; } mk()();"),
        Value::Number(42.0)
    );
}

#[test]
fn this_in_method() {
    let src = r#"
        let calc = {
            value: 10,
            add: function(n) { return this.value + n; }
        };
        calc.add(5);
    "#;
    assert_eq!(run(src), Value::Number(15.0));
}

#[test]
fn prototype_method() {
    let src = r#"
        function Animal(name) { this.name = name; }
        Animal.prototype.speak = function() { return this.name + " speaks"; };
        new Animal("Rex").speak();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("Rex speaks")));
}

#[test]
fn object_method_assignment() {
    assert_eq!(
        run("let o = {}; o.f = function(){ return 5; }; o.f();"),
        Value::Number(5.0)
    );
}

#[test]
fn try_catch() {
    assert_eq!(
        run("let r=0; try { throw 42; } catch(e){ r=e; } r;"),
        Value::Number(42.0)
    );
}

#[test]
fn function_hoisting() {
    assert_eq!(
        run("let r = hoisted(); function hoisted(){return 42;}; r;"),
        Value::Number(42.0)
    );
}
