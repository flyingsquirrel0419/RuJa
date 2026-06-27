//! Lazy generator tests: `function*`/`yield`, `next()`, `for...of`, spread,
//! resume values, return values, and infinite generators (the core reason the
//! VM uses a pull-based generator model rather than eager collection).

mod common;
use common::run;
use ruja::Value;

#[test]
fn gen_next_returns_value_done() {
    // The first next() yields the first value and is not done.
    let r = run("function* g(){ yield 7; } var it=g(); it.next().value;");
    assert_eq!(r, Value::Number(7.0));
    let done = run("function* g(){ yield 7; } var it=g(); it.next(); it.next().done;");
    assert_eq!(done, Value::Bool(true));
}

#[test]
fn gen_for_of_consumes_all() {
    assert_eq!(
        run("function* g(){ yield 1; yield 2; yield 3; } var s=0; for (var v of g()) s+=v; s;"),
        Value::Number(6.0)
    );
}

#[test]
fn gen_spread_into_array() {
    assert_eq!(
        run("function* g(){ yield 1; yield 2; yield 3; } [...g()].join(',');"),
        Value::String(std::rc::Rc::from("1,2,3"))
    );
}

#[test]
fn gen_infinite_counter_via_next() {
    // An infinite generator must not hang when pulled with next() manually.
    assert_eq!(
        run("function* counter(){ let i=0; while(true){ yield i; i++; } } var g=counter(); g.next().value + g.next().value + g.next().value;"),
        Value::Number(3.0) // 0 + 1 + 2
    );
}

#[test]
fn gen_next_resume_value_sent_to_yield() {
    // The value passed to next(v) becomes the result of the suspended yield.
    assert_eq!(
        run(
            "function* g(){ var x = yield 1; return x; } var it=g(); it.next(); it.next(42).value;"
        ),
        Value::Number(42.0)
    );
}

#[test]
fn gen_return_value() {
    // An explicit `return` ends the generator; its value surfaces via next().
    assert_eq!(
        run("function* g(){ yield 1; return 99; } var it=g(); it.next().value; it.next().value;"),
        Value::Number(99.0)
    );
}

#[test]
fn gen_done_after_return() {
    assert_eq!(
        run("function* g(){ yield 1; return 99; } var it=g(); it.next(); it.next(); it.next().done;"),
        Value::Bool(true)
    );
}

#[test]
fn gen_bounded_loop_body() {
    // Classic finite generator in a for-loop body.
    assert_eq!(
        run("function* r(a,b){ for(var i=a;i<b;i++) yield i; } var s=0; for (var v of r(1,4)) s+=v; s;"),
        Value::Number(6.0)
    );
}

#[test]
fn gen_first_next_value_is_first_yield() {
    assert_eq!(
        run("function* g(){ yield 10; yield 20; } g().next().value;"),
        Value::Number(10.0)
    );
}

#[test]
fn gen_empty_generator_is_done_immediately() {
    assert_eq!(run("function* g(){} g().next().done;"), Value::Bool(true));
}

#[test]
fn gen_yield_undefined() {
    assert_eq!(
        run("function* g(){ yield; yield 1; } var it=g(); it.next().value;"),
        Value::Undefined
    );
}

#[test]
fn gen_closure_capture() {
    assert_eq!(
        run("function* gen(n){ for(let i=0;i<n;i++) yield i*i; } var g=gen(3); g.next().value + g.next().value + g.next().value;"),
        Value::Number(5.0) // 0 + 1 + 4
    );
}

#[test]
fn gen_state_persists_across_next_calls() {
    // Mutation of closed-over let variables must survive suspension.
    assert_eq!(
        run(
            "function* g(){ var a=0; while(true){ yield a; a+=5; } } var it=g(); it.next().value + it.next().value + it.next().value;"
        ),
       Value::Number(15.0) // 0 + 5 + 10
    );
}
