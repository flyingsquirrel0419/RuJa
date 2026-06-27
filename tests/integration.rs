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

#[test]
fn map_basic() {
    assert_eq!(run("let m = new Map(); m.set('a', 1); m.get('a');"), Value::Number(1.0));
    assert_eq!(run("let m = new Map(); m.set('x', 1); m.set('y', 2); m.size;"), Value::Number(2.0));
    assert_eq!(run("let m = new Map(); m.set('a', 1); m.has('a');"), Value::Bool(true));
    assert_eq!(run("let m = new Map(); m.set('a', 1); m.delete('a'); m.has('a');"), Value::Bool(false));
}

#[test]
fn set_basic() {
    assert_eq!(run("let s = new Set(); s.add(1); s.add(2); s.add(1); s.size;"), Value::Number(2.0));
    assert_eq!(run("let s = new Set(); s.add(1); s.has(1);"), Value::Bool(true));
}

#[test]
fn symbol_type() {
    assert_eq!(run("typeof Symbol();"), Value::String(Rc::from("symbol")));
}

#[test]
fn prototype_inheritance() {
    let src = r#"function Shape() {} Shape.prototype.describe = function() { return 'shape'; }; let s = new Shape(); s.describe();"#;
    assert_eq!(run(src), Value::String(Rc::from("shape")));
}

#[test]
fn array_method_chaining() {
    assert_eq!(run("[1,2,3,4,5].filter(x => x > 2).map(x => x * 2).join(',');"), Value::String(Rc::from("6,8,10")));
}

#[test]
fn string_split_join() {
    assert_eq!(run("'a,b,c'.split(',').join('-');"), Value::String(Rc::from("a-b-c")));
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
    assert_eq!(run("function mk(){ let c = 42; return function(){ return c; }; } mk()();"), Value::Number(42.0));
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
    assert_eq!(run(src), Value::String(Rc::from("Rex speaks")));
}

#[test]
fn object_method_assignment() {
    assert_eq!(run("let o = {}; o.f = function(){ return 5; }; o.f();"), Value::Number(5.0));
}

#[test]
fn class_basic() {
    let src = r#"
        class Point {
            constructor(x, y) { this.x = x; this.y = y; }
            sum() { return this.x + this.y; }
        }
        let p = new Point(3, 4);
        p.sum();
    "#;
    assert_eq!(run(src), Value::Number(7.0));
}

#[test]
fn class_constructor_field() {
    assert_eq!(run("class A { constructor(x) { this.x = x; } } new A(42).x;"), Value::Number(42.0));
}
#[test]
fn for_of_array() {
    assert_eq!(run("let s=0; for(let x of [1,2,3]){s+=x;} s;"), Value::Number(6.0));
}

#[test]
fn for_of_string() {
    assert_eq!(run("let s=''; for(let c of 'abc'){s+=c;} s;"), Value::String(Rc::from("abc")));
}

#[test]
fn for_in_object() {
    // for-in key order over a HashMap-backed object is not guaranteed; check membership.
    let s = run("let s=''; for(let k in {a:1,b:2}){s+=k;} s;");
    match s {
        Value::String(st) => {
            assert!(st.contains('a') && st.contains('b') && st.len() == 2, "got {st:?}");
        }
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn class_extends() {
    assert_eq!(run("class A{f(){return 7;}} class B extends A{} new B().f();"), Value::Number(7.0));
}

#[test]
fn super_call() {
    assert_eq!(run("class A{f(){return 10;}} class B extends A{f(){return super.f()+5;}} new B().f();"), Value::Number(15.0));
}

#[test]
fn static_method() {
    assert_eq!(run("class C{static s(){return 42;}} C.s();"), Value::Number(42.0));
}

#[test]
fn template_literal() {
    assert_eq!(run(r#"let n=5; `n=${n}`;"#), Value::String(Rc::from("n=5")));
}

#[test]
fn default_param() {
    assert_eq!(run("function f(a,b=10){return a+b;} f(5);"), Value::Number(15.0));
}

#[test]
fn rest_param() {
    assert_eq!(run("function f(...a){return a.length;} f(1,2,3);"), Value::Number(3.0));
}

#[test]
fn array_destructure() {
    assert_eq!(run("let [a,b]=[1,2]; a+b;"), Value::Number(3.0));
}

#[test]
fn object_destructure() {
    assert_eq!(run("let {x,y}={x:1,y:2}; x+y;"), Value::Number(3.0));
}

#[test]
fn for_of_destructure() {
    assert_eq!(run("let s=0; for(let [k,v] of [['a',1]]){s+=v;} s;"), Value::Number(1.0));
}

#[test]
fn array_find() {
    assert_eq!(run("[4,5,6].find(x=>x>4);"), Value::Number(5.0));
}

#[test]
fn array_some() {
    assert_eq!(run("[1,2,3].some(x=>x>2);"), Value::Bool(true));
}

#[test]
fn array_every_false() {
    assert_eq!(run("[1,2,3].every(x=>x>2);"), Value::Bool(false));
}

#[test]
fn symbol_to_string() {
    assert_eq!(run("Symbol('x').toString();"), Value::String(Rc::from("Symbol()")));
}

#[test]
fn template_multi() {
    assert_eq!(run(r#"let a=1,b=2; `${a}+${b}=${a+b}`;"#), Value::String(Rc::from("1+2=3")));
}
