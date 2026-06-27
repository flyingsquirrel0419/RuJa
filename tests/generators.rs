//! Lazy generator tests: `function*`/`yield`, `next()`, `for...of`, spread,
//! resume values, return values, and infinite generators (the core reason the
//! VM uses a pull-based generator model rather than eager collection).

mod common;
use common::run;
use ruja::Value;
use std::rc::Rc;

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

// ---- nested generator isolation (per-frame gen-state) ----

#[test]
fn nested_generator_next_is_isolated() {
    // A generator body that calls next() on *another* generator while it is
    // itself running must not corrupt either generator's run-state.
    let src = r#"
        function* inner() { yield 1; yield 2; yield 3; }
        function* outer() {
            let g = inner();
            yield g.next().value;
            yield g.next().value;
            yield 99;
            yield g.next().value;
        }
        let o = outer();
        let r = [];
        for (let v of o) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Rc::from("1,2,99,3")));
}

#[test]
fn nested_generator_interleaved() {
    let src = r#"
        function* a() { yield "a1"; yield "a2"; yield "a3"; }
        function* b() {
            yield "b1";
            let ga = a();
            yield ga.next().value;
            yield "b2";
            yield ga.next().value;
            yield "b3";
            yield ga.next().value;
        }
        let out = [];
        for (let v of b()) out.push(v);
        out.join(",");
    "#;
    assert_eq!(run(src), Value::String(Rc::from("b1,a1,b2,a2,b3,a3")));
}

#[test]
fn two_generators_pulled_independently() {
    let src = r#"
        function* gen() { let i = 0; while (i < 3) { yield i; i++; } }
        let g1 = gen();
        let g2 = gen();
        // Pull g1 twice, then g2 once, then g1 again: states stay independent.
        [g1.next().value, g1.next().value, g2.next().value, g1.next().value, g2.next().value].join(",");
    "#;
    assert_eq!(run(src), Value::String(Rc::from("0,1,0,2,1")));
}

// ---- yield* delegation ----

#[test]
fn yield_star_delegates_to_generator() {
    let src = r#"
        function* inner() { yield 1; yield 2; yield 3; }
        function* outer() { yield 0; yield* inner(); yield 4; }
        let r = [];
        for (let v of outer()) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Rc::from("0,1,2,3,4")));
}

#[test]
fn yield_star_delegates_to_array() {
    let src = r#"
        function* g() { yield* [10, 20, 30]; }
        let r = [];
        for (let v of g()) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Rc::from("10,20,30")));
}

#[test]
fn yield_star_delegates_to_string() {
    let src = r#"
        function* g() { yield* "ab"; }
        let r = [];
        for (let v of g()) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Rc::from("a,b")));
}

#[test]
fn yield_star_nested_delegation() {
    let src = r#"
        function* a() { yield 1; yield 2; }
        function* b() { yield* a(); yield 3; }
        function* c() { yield* b(); yield 4; }
        let r = [];
        for (let v of c()) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Rc::from("1,2,3,4")));
}

#[test]
fn yield_star_interleaved_with_own_yields() {
    let src = r#"
        function* inner() { yield "i1"; yield "i2"; }
        function* outer() {
            yield "o1";
            yield* inner();
            yield "o2";
            yield* inner();
        }
        let r = [];
        for (let v of outer()) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Rc::from("o1,i1,i2,o2,i1,i2")));
}

// ---- async function* ----

#[test]
fn async_generator_next_returns_promise() {
    let src = r#"
        async function* gen() { yield 1; yield 2; }
        let g = gen();
        let r = await g.next();
        r.value;
    "#;
    assert_eq!(run(src), Value::Number(1.0));
}

#[test]
fn async_generator_consumes_all() {
    let src = r#"
        async function* gen() { yield 1; yield 2; yield 3; }
        let g = gen();
        let out = [];
        let r;
        r = await g.next(); if (!r.done) out.push(r.value);
        r = await g.next(); if (!r.done) out.push(r.value);
        r = await g.next(); if (!r.done) out.push(r.value);
        r = await g.next();
        out.join(",") + "|" + r.done;
    "#;
    assert_eq!(run(src), Value::String(Rc::from("1,2,3|true")));
}

#[test]
fn async_generator_await_inside_body() {
    let src = r#"
        async function* gen() {
            let x = await Promise.resolve(10);
            yield x;
            let y = await Promise.resolve(20);
            yield x + y;
        }
        let g = gen();
        let a = await g.next();
        let b = await g.next();
        a.value + "," + b.value;
    "#;
    assert_eq!(run(src), Value::String(Rc::from("10,30")));
}

#[test]
fn async_generator_done_signal() {
    let src = r#"
        async function* gen() { yield "x"; }
        let g = gen();
        await g.next();
        let last = await g.next();
        last.done;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}
