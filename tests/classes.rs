//! Class features: static initialization blocks and private methods/fields.

mod common;
use common::run;
use ruja::Value;
use std::rc::Rc;

// --- static initialization blocks ---

#[test]
fn static_block_sets_this() {
    assert_eq!(run("class A{static{this.x=42;}}A.x;"), Value::Number(42.0));
}

#[test]
fn static_block_multiple_in_order() {
    assert_eq!(
        run("class A{static{this.c=10;}static{this.c=this.c+5;}}A.c;"),
        Value::Number(15.0)
    );
}

#[test]
fn static_block_references_class_name() {
    assert_eq!(
        run("class A{static{A.tagged=true;}}A.tagged;"),
        Value::Bool(true)
    );
}

#[test]
fn static_block_local_bindings() {
    assert_eq!(
        run("class A{static{let x=100,y=200;this.sum=x+y;}}A.sum;"),
        Value::Number(300.0)
    );
}

#[test]
fn static_block_does_not_leak_locals() {
    // locals declared in a static block must not be visible outside it
    assert_eq!(
        run("class A{static{let secret=7;this.pub=secret;}}typeof secret;"),
        Value::String(Rc::from("undefined"))
    );
}

// --- private methods ---

#[test]
fn private_method_called() {
    assert_eq!(
        run("class C{#inc(){return 1;}g(){return this.#inc();}}new C().g();"),
        Value::Number(1.0)
    );
}

#[test]
fn private_method_mutates_field() {
    assert_eq!(
        run("class C{#c=0;#inc(){this.#c++;}bump(){this.#inc();this.#inc();}get v(){return this.#c;}}let c=new C();c.bump();c.v;"),
        Value::Number(2.0)
    );
}

#[test]
fn private_method_with_args() {
    assert_eq!(
        run("class C{#add(a,b){return a+b;}sum(){return this.#add(3,4);}}new C().sum();"),
        Value::Number(7.0)
    );
}

#[test]
fn private_method_calls_another_private() {
    assert_eq!(
        run(
            "class C{#a(){return 1;}#b(){return this.#a()+1;}c(){return this.#b()+1;}}new C().c();"
        ),
        Value::Number(3.0)
    );
}

#[test]
fn private_field_increment() {
    assert_eq!(
        run("class C{#c=5;inc(){this.#c++;}get v(){return this.#c;}}let c=new C();c.inc();c.v;"),
        Value::Number(6.0)
    );
}

#[test]
fn private_field_set_in_method() {
    assert_eq!(
        run("class C{#c=0;set(v){this.#c=v;}get v(){return this.#c;}}let c=new C();c.set(99);c.v;"),
        Value::Number(99.0)
    );
}
