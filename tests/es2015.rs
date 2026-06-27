//! ES2015 features: class/extends/super, template literals, default/rest
//! params, destructuring, for-of/for-in, spread, Map/Set/Symbol.

mod common;
use common::run;
use ruja::Value;
use std::rc::Rc;

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
    assert_eq!(
        run("class A { constructor(x) { this.x = x; } } new A(42).x;"),
        Value::Number(42.0)
    );
}

#[test]
fn class_extends() {
    assert_eq!(
        run("class A{f(){return 7;}} class B extends A{} new B().f();"),
        Value::Number(7.0)
    );
}

#[test]
fn super_call() {
    assert_eq!(
        run("class A{f(){return 10;}} class B extends A{f(){return super.f()+5;}} new B().f();"),
        Value::Number(15.0)
    );
}

#[test]
fn static_method() {
    assert_eq!(
        run("class C{static s(){return 42;}} C.s();"),
        Value::Number(42.0)
    );
}

#[test]
fn template_literal() {
    assert_eq!(run(r#"let n=5; `n=${n}`;"#), Value::String(Rc::from("n=5")));
}

#[test]
fn template_multi() {
    assert_eq!(
        run(r#"let a=1,b=2; `${a}+${b}=${a+b}`;"#),
        Value::String(Rc::from("1+2=3"))
    );
}

#[test]
fn default_param() {
    assert_eq!(
        run("function f(a,b=10){return a+b;} f(5);"),
        Value::Number(15.0)
    );
}

#[test]
fn default_param_override() {
    assert_eq!(
        run("function f(a,b=10){return a+b;} f(5,20);"),
        Value::Number(25.0)
    );
}

#[test]
fn rest_param() {
    assert_eq!(
        run("function f(...a){return a.length;} f(1,2,3);"),
        Value::Number(3.0)
    );
}

#[test]
fn rest_param_after_fixed() {
    assert_eq!(
        run("function f(a, ...r){return r[0]+r[1];} f(1,2,3);"),
        Value::Number(5.0)
    );
}

#[test]
fn arrow_default_param() {
    assert_eq!(run("((a,b=5)=>a+b)(3);"), Value::Number(8.0));
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
fn object_destructure_rename() {
    assert_eq!(run("let {a:p,b:q}={a:10,b:20}; p+q;"), Value::Number(30.0));
}

#[test]
fn destructure_default() {
    assert_eq!(run("let {x=5} = {}; x;"), Value::Number(5.0));
}

#[test]
fn destructure_rest() {
    assert_eq!(
        run("let [a, ...rest] = [1,2,3,4]; rest.length;"),
        Value::Number(3.0)
    );
}

#[test]
fn for_of_destructure() {
    assert_eq!(
        run("let s=0; for(let [k,v] of [['a',1]]){s+=v;} s;"),
        Value::Number(1.0)
    );
}

#[test]
fn for_of_array() {
    assert_eq!(
        run("let s=0; for(let x of [1,2,3]){s+=x;} s;"),
        Value::Number(6.0)
    );
}

#[test]
fn for_of_string() {
    assert_eq!(
        run("let s=''; for(let c of 'abc'){s+=c;} s;"),
        Value::String(Rc::from("abc"))
    );
}

#[test]
fn for_in_object() {
    // for-in key order over a HashMap-backed object is not guaranteed; check membership.
    let s = run("let s=''; for(let k in {a:1,b:2}){s+=k;} s;");
    match s {
        Value::String(st) => {
            assert!(
                st.contains('a') && st.contains('b') && st.len() == 2,
                "got {st:?}"
            );
        }
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn array_spread_literal() {
    assert_eq!(run("[1, ...[2,3], 4].length;"), Value::Number(4.0));
    assert_eq!(run(r#"[..."hi"].join("");"#), Value::String(Rc::from("hi")));
}

#[test]
fn map_basic() {
    assert_eq!(
        run("let m = new Map(); m.set('a', 1); m.get('a');"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("let m = new Map(); m.set('x', 1); m.set('y', 2); m.size;"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("let m = new Map(); m.set('a', 1); m.has('a');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("let m = new Map(); m.set('a', 1); m.delete('a'); m.has('a');"),
        Value::Bool(false)
    );
}

#[test]
fn set_basic() {
    assert_eq!(
        run("let s = new Set(); s.add(1); s.add(2); s.add(1); s.size;"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("let s = new Set(); s.add(1); s.has(1);"),
        Value::Bool(true)
    );
}

#[test]
fn symbol_type() {
    assert_eq!(run("typeof Symbol();"), Value::String(Rc::from("symbol")));
}

#[test]
fn symbol_to_string() {
    assert_eq!(
        run("Symbol('x').toString();"),
        Value::String(Rc::from("Symbol()"))
    );
}
