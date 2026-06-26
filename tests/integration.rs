use ruja::{Vm, Value};
use std::rc::Rc;

fn run(src: &str) -> Value {
    let mut vm = Vm::new();
    vm.run(src).unwrap_or(Value::Undefined)
}

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
    assert_eq!(run("let s = 0; let i = 0; while (i < 3) { s += i; i++; } s;"), Value::Number(3.0));
}

#[test]
fn functions() {
    assert_eq!(run("function add(a, b) { return a + b; } add(3, 4);"), Value::Number(7.0));
    assert_eq!(run("function fact(n) { return n <= 1 ? 1 : n * fact(n-1); } fact(5);"), Value::Number(120.0));
}

#[test]
fn recursion() {
    assert_eq!(run("function fib(n){ if(n<=1) return n; return fib(n-1)+fib(n-2); } fib(10);"), Value::Number(55.0));
}

#[test]
fn objects() {
    assert_eq!(run("let p = { x: 1, y: 2 }; p.x + p.y;"), Value::Number(3.0));
}

#[test]
fn arrays() {
    assert_eq!(run("let a = [1, 2, 3]; a[0] + a[2];"), Value::Number(4.0));
    assert_eq!(run("[1,2,3].length;"), Value::Number(3.0));
}

#[test]
fn array_methods() {
    assert_eq!(run("[1,2,3].map(x => x*2).join(',');"), Value::String(Rc::from("2,4,6")));
    assert_eq!(run("[1,2,3].reduce((a,b)=>a+b, 0);"), Value::Number(6.0));
}

#[test]
fn strings() {
    assert_eq!(run("'hello'.toUpperCase();"), Value::String(Rc::from("HELLO")));
    assert_eq!(run("'hello'.charAt(1);"), Value::String(Rc::from("e")));
}

#[test]
fn math() {
    assert_eq!(run("Math.floor(3.7);"), Value::Number(3.0));
    assert_eq!(run("Math.max(1, 5, 3);"), Value::Number(5.0));
    assert_eq!(run("Math.sqrt(16);"), Value::Number(4.0));
}

#[test]
fn json() {
    assert_eq!(run("JSON.parse('[1,2,3]')[1];"), Value::Number(2.0));
    assert_eq!(run("JSON.stringify({a:1});"), Value::String(Rc::from("{\"a\":1}")));
}

#[test]
fn try_catch() {
    assert_eq!(run("let r=0; try { throw 42; } catch(e){ r=e; } r;"), Value::Number(42.0));
}

#[test]
fn typeof_values() {
    assert_eq!(run("typeof 42;"), Value::String(Rc::from("number")));
    assert_eq!(run("typeof 's';"), Value::String(Rc::from("string")));
    assert_eq!(run("typeof undefined;"), Value::String(Rc::from("undefined")));
    assert_eq!(run("typeof null;"), Value::String(Rc::from("object")));
}

#[test]
fn globals() {
    assert_eq!(run("parseInt('42');"), Value::Number(42.0));
    assert_eq!(run("isNaN(NaN);"), Value::Bool(true));
}
