//! Lazy generator tests: `function*`/`yield`, `next()`, `for...of`, spread,
//! resume values, return values, and infinite generators (the core reason the
//! VM uses a pull-based generator model rather than eager collection).

mod common;
use common::run;
use ruja::Value;
use std::sync::Arc;

#[test]
fn generator_function_constructor_is_distinct_and_subclassable() {
    assert_eq!(
        run(r#"
            var GeneratorFunction = Object.getPrototypeOf(function*() {}).constructor;
            class Gfn extends GeneratorFunction {}
            var gfn = new Gfn("a", "yield a; yield a * 2;");
            var iter = gfn(42);
            [
              GeneratorFunction === Function,
              Object.getPrototypeOf(function*() {}) === GeneratorFunction.prototype,
              gfn instanceof Gfn,
              gfn instanceof GeneratorFunction,
              iter.next().value,
              iter.next().value
            ].join(",");
            "#),
        Value::String(Arc::from("false,true,true,true,42,84"))
    );
}

#[test]
fn generator_function_instances_have_empty_prototype_descriptor() {
    assert_eq!(
        run(r#"
            var GeneratorFunction = Object.getPrototypeOf(function*() {}).constructor;
            class Gfn extends GeneratorFunction {}
            var gfn = new Gfn(";");
            var desc = Object.getOwnPropertyDescriptor(gfn, "prototype");
            [
              Object.keys(gfn.prototype).length,
              gfn.prototype.hasOwnProperty("constructor"),
              typeof gfn.prototype.next,
              desc.writable,
              desc.enumerable,
              desc.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("0,false,function,true,false,false"))
    );
}

#[test]
fn generator_intrinsic_prototype_is_the_non_object_fallback() {
    assert_eq!(
        run(r#"
            var GeneratorFunctionPrototype = Object.getPrototypeOf(function*() {});
            var GeneratorPrototype = GeneratorFunctionPrototype.prototype;
            var desc = Object.getOwnPropertyDescriptor(
                GeneratorFunctionPrototype,
                "prototype"
            );
            function* g() {}
            g.prototype = null;
            [
              Object.getPrototypeOf(g()) === GeneratorPrototype,
              desc.writable,
              desc.enumerable,
              desc.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("true,false,false,true"))
    );
}

#[test]
fn generator_and_async_functions_inherit_restricted_properties() {
    assert_eq!(
        run(r#"
            function* generator() {}
            var asyncFunction = async function() {};
            function throws(getter) {
              try { getter(); return false; }
              catch (error) { return error instanceof TypeError; }
            }
            [
              generator.hasOwnProperty("caller"),
              generator.hasOwnProperty("arguments"),
              throws(function() { return generator.caller; }),
              throws(function() { generator.arguments = {}; }),
              throws(function() { return asyncFunction.caller; }),
              throws(function() { asyncFunction.arguments = {}; })
            ].join(",");
            "#),
        Value::String(Arc::from("false,false,true,true,true,true"))
    );
}

#[test]
fn function_expression_names_use_their_own_yield_context() {
    assert_eq!(
        run(r#"
            function* outer() {
              return (function yield() { return yield.name; })();
            }
            var rejected = false;
            try { eval("var bad = function* yield() {};"); }
            catch (error) { rejected = error instanceof SyntaxError; }
            outer().next().value + "," + rejected;
            "#),
        Value::String(Arc::from("yield,true"))
    );
}

#[test]
fn generator_parameter_eval_rejects_var_parameter_conflicts() {
    assert_eq!(
        run(r#"
            var callCount = 0;
            function* declaration(a = eval("var a = 42")) { callCount++; }
            var expression = function*(a = eval("var a = 42")) { callCount++; };
            function throwsSyntax(generator) {
              try { generator(); return false; }
              catch (error) { return error instanceof SyntaxError; }
            }
            function* valid(a = eval("var b = 42")) { return b; }
            [
              throwsSyntax(declaration),
              throwsSyntax(expression),
              callCount,
              valid().next().value
            ].join(",");
            "#),
        Value::String(Arc::from("true,true,0,42"))
    );
}

#[test]
fn generator_parameter_nested_arrow_uses_yield_as_identifier() {
    assert_eq!(
        run(r#"
            function* g(callback = () => yield) {
              try { return callback(); }
              catch (error) { return error instanceof ReferenceError; }
            }
            g().next().value;
            "#),
        Value::Bool(true)
    );
}

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
        Value::String(std::sync::Arc::from("1,2,3"))
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
fn generator_yield_respects_line_terminators_and_omitted_operands() {
    assert_eq!(
        run(r#"
            class C {
                *newline() { yield
                    1; }
                *omitted() { return (yield) ? yield : yield; }
                *template() { return `a${yield}b`; }
            }
            let newline = new C().newline();
            let omitted = new C().omitted();
            let template = new C().template();
            [
                newline.next().value,
                newline.next().done,
                omitted.next().value,
                omitted.next(false).value,
                omitted.next(7).done,
                template.next().value,
                template.next(3).value
            ].join(",");
            "#,),
        Value::String(Arc::from(",true,,,true,,a3b"))
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
    assert_eq!(run(src), Value::String(Arc::from("1,2,99,3")));
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
    assert_eq!(run(src), Value::String(Arc::from("b1,a1,b2,a2,b3,a3")));
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
    assert_eq!(run(src), Value::String(Arc::from("0,1,0,2,1")));
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
    assert_eq!(run(src), Value::String(Arc::from("0,1,2,3,4")));
}

#[test]
fn yield_star_delegates_to_array() {
    let src = r#"
        function* g() { yield* [10, 20, 30]; }
        let r = [];
        for (let v of g()) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("10,20,30")));
}

#[test]
fn yield_star_delegates_to_string() {
    let src = r#"
        function* g() { yield* "ab"; }
        let r = [];
        for (let v of g()) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("a,b")));
}

#[test]
fn yield_star_observes_primitive_symbol_iterator() {
    let src = r#"
        Boolean.prototype[Symbol.iterator] = function* () {
            yield this.valueOf();
        };
        function* g() { yield* true; yield* false; }
        let values = [];
        for (let value of g()) values.push(value);
        values.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true,false")));
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
    assert_eq!(run(src), Value::String(Arc::from("1,2,3,4")));
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
    assert_eq!(run(src), Value::String(Arc::from("o1,i1,i2,o2,i1,i2")));
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
    assert_eq!(run(src), Value::String(Arc::from("1,2,3|true")));
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
    assert_eq!(run(src), Value::String(Arc::from("10,30")));
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

#[test]
fn async_generator_yield_star_selects_async_then_sync_iterator() {
    let src = r#"
        let log = [];
        let asyncIterable = {
            [Symbol.asyncIterator]() {
                log.push("async");
                let done = false;
                return {
                    next() {
                        if (done) return Promise.resolve({ value: 9, done: true });
                        done = true;
                        return Promise.resolve({ value: 1, done: false });
                    }
                };
            },
            [Symbol.iterator]() { throw new Error("sync must not be read"); }
        };
        let syncIterable = {
            [Symbol.asyncIterator]: null,
            [Symbol.iterator]() {
                log.push("sync");
                let done = false;
                return {
                    next() {
                        if (done) return { value: 8, done: true };
                        done = true;
                        return { value: 2, done: false };
                    }
                };
            }
        };
        async function* delegate(value) { return yield* value; }
        let a = delegate(asyncIterable);
        let b = delegate(syncIterable);
        let a1 = await a.next();
        let a2 = await a.next();
        let b1 = await b.next();
        let b2 = await b.next();
        [a1.value, a2.value, b1.value, b2.value, log.join(",")].join("|");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1|9|2|8|async,sync")));
}

#[test]
fn async_generator_protocol_errors_reject_next_promise() {
    let src = r#"
        let bad = { [Symbol.asyncIterator]() { return 1; } };
        async function* delegate() { yield* bad; }
        let rejected = false;
        try { await delegate().next(); }
        catch (error) { rejected = error instanceof TypeError; }
        rejected;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

// ---- yield* resume forwarding + return value (#8 fix) ----

#[test]
fn yield_star_preserves_inner_return_value() {
    let src = r#"
        function* inner() { yield 1; yield 2; return 99; }
        function* outer() {
            let r = yield* inner();
            return r;
        }
        let g = outer();
        let a = g.next();
        let b = g.next();
        let c = g.next();
        let d = g.next();
        a.value + "," + b.value + "," + c.value + "," + c.done + "," + d.done;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1,2,99,true,true")));
}

#[test]
fn yield_star_forwards_resume_value() {
    let src = r#"
        function* inner() {
            let x = yield 1;
            return x + 100;
        }
        function* outer() {
            let r = yield* inner();
            return r;
        }
        let g = outer();
        let a = g.next();
        let b = g.next(5);
        let c = g.next();
        a.value + "," + b.value + "," + b.done + "," + c.done;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1,105,true,true")));
}

#[test]
fn yield_star_forwards_iterator_result_objects_and_completion_values() {
    let src = r#"
        let first = { value: 1 };
        let final = { value: 42, done: true };
        let calls = [];
        let iterator = {
            next(value) {
                calls.push(arguments.length + ":" + value);
                return calls.length === 1 ? first : final;
            }
        };
        iterator[Symbol.iterator] = function() { return this; };
        function* outer() { return yield* iterator; }
        let generator = outer();
        let yielded = generator.next(99);
        let completed = generator.next(7);
        [
            yielded === first,
            "done" in yielded,
            completed.value,
            completed.done,
            calls.join(",")
        ].join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("true|false|42|true|1:undefined,1:7"))
    );
}

#[test]
fn yield_star_forwards_return_until_the_delegate_finishes() {
    let src = r#"
        let returned = { value: 2, done: false };
        let calls = [];
        let iterator = {
            next(value) {
                calls.push("next:" + arguments.length + ":" + value);
                return calls.length === 1
                    ? { value: 1, done: false }
                    : { value: 9, done: true };
            },
            return(value) {
                calls.push("return:" + arguments.length + ":" + value);
                return returned;
            }
        };
        iterator[Symbol.iterator] = function() { return this; };
        function* outer() { return yield* iterator; }
        let generator = outer();
        generator.next();
        let yielded = generator.return(7);
        let completed = generator.next(8);
        [
            yielded === returned,
            completed.value,
            completed.done,
            calls.join(",")
        ].join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "true|9|true|next:1:undefined,return:1:7,next:1:8"
        ))
    );
}

#[test]
fn yield_star_forwards_throw_and_closes_on_missing_throw() {
    let src = r#"
        let throwResult = { value: 3, done: false };
        let calls = [];
        let iterator = {
            next() { calls.push("next"); return { value: 1, done: false }; },
            throw(value) {
                calls.push("throw:" + value);
                return throwResult;
            }
        };
        iterator[Symbol.iterator] = function() { return this; };
        function* delegated() { yield* iterator; }
        let generator = delegated();
        generator.next();
        let yielded = generator.throw(7);

        let closed = 0;
        let missing = {
            next() { return { value: 1, done: false }; },
            return() { closed += 1; return {}; }
        };
        missing[Symbol.iterator] = function() { return this; };
        function* guarded() {
            try { yield* missing; }
            catch (error) { return error instanceof TypeError; }
        }
        let guardedGenerator = guarded();
        guardedGenerator.next();
        let caught = guardedGenerator.throw(8);
        [
            yielded === throwResult,
            calls.join(","),
            closed,
            caught.value,
            caught.done
        ].join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("true|next,throw:7|1|true|true"))
    );
}

// ---- generator.return / generator.throw ----

#[test]
fn generator_return_terminates() {
    let src = r#"
        function* g() { yield 1; yield 2; yield 3; }
        let it = g();
        it.next();
        let r = it.return(99);
        let after = it.next();
        r.value + "," + r.done + "," + after.done;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("99,true,true")));
}

#[test]
fn generator_throw_propagates() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let res = vm.run(
        r#"
            function* g() { yield 1; yield 2; }
            let it = g();
            it.next();
            it.throw("boom");
        "#,
    );
    assert!(res.is_err(), "expected throw to propagate");
}

#[test]
fn async_generator_return_promise() {
    let src = r#"
        async function* g() { yield 1; yield 2; }
        let it = g();
        await it.next();
        let r = await it.return(42);
        r.value + "," + r.done;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("42,true")));
}

// ---- generator throw/return injection into the body ----

#[test]
fn generator_throw_caught_by_body() {
    // throw(e) injects the exception at the yield point; the body's catch
    // handles it and the generator continues.
    let src = r#"
        function* g() {
            try { yield 1; yield 2; }
            catch(e) { yield "caught:" + e; }
            yield 3;
        }
        let it = g();
        let a = it.next();
        let b = it.throw("boom");
        let c = it.next();
        a.value + "," + b.value + "," + c.value + "," + c.done;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1,caught:boom,3,false")));
}

#[test]
fn generator_throw_uncaught_marks_done() {
    // If throw(e) is not caught, the generator is done and the error propagates.
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let res = vm.run(
        r#"
            function* g() { yield 1; yield 2; }
            let it = g();
            it.next();
            it.throw("err");
            it.next();
        "#,
    );
    assert!(res.is_err());
}

#[test]
fn generator_throw_uncaught_propagates_through_call() {
    // The thrown error propagates out of throw() and can be caught by the
    // caller's try/catch.
    let src = r#"
        function* g() { yield 1; yield 2; }
        let it = g();
        it.next();
        let result;
        try { it.throw("err"); result = "no-throw"; }
        catch(e) { result = "caught:" + e; }
        result;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("caught:err")));
}

#[test]
fn generator_return_value_surfaces() {
    let src = r#"
        function* g() { yield 1; yield 2; }
        let it = g();
        it.next();
        let r = it.return(77);
        r.value + "," + r.done;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("77,true")));
}

#[test]
fn generator_return_runs_finally() {
    // Per spec, return(v) runs any finally block before completing.
    let src = r#"
        let closed = 0;
        function* g() {
            try { yield 1; }
            finally { closed++; }
        }
        let it = g();
        it.next();
        let r = it.return(42);
        r.value + "," + r.done + "," + closed + "," + it.next().done;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("42,true,1,true")));
}

#[test]
fn generator_return_can_yield_from_finally_then_complete() {
    let src = r#"
        let log = [];
        function* g() {
            try { yield 1; }
            finally {
                log.push("finally");
                yield 2;
                log.push("after");
            }
        }
        let it = g();
        let a = it.next();
        let b = it.return(9);
        let c = it.next();
        let d = it.next();
        [
            a.value, a.done,
            b.value, b.done,
            c.value, c.done,
            d.value, d.done,
            log.join("|")
        ].join(",");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("1,false,2,false,9,true,,true,finally|after"))
    );
}

#[test]
fn generator_return_on_done_preserves_argument() {
    let src = r#"
        function* g() { yield 1; }
        let it = g();
        it.next();
        it.next();
        let r = it.return(7);
        r.value + "," + r.done;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("7,true")));
}

#[test]
fn for_of_break_closes_generator_with_finally() {
    let src = r#"
        let closed = 0;
        function* g() {
            try { yield 1; yield 2; }
            finally { closed++; }
        }
        for (let v of g()) { break; }
        closed;
    "#;
    assert_eq!(run(src), Value::Number(1.0));
}

#[test]
fn for_of_continue_closes_generator_with_finally() {
    let src = r#"
        let closed = 0;
        function* g() {
            try { yield 1; yield 2; }
            finally { closed++; }
        }
        outer: for (let i = 0; i < 1; i++) {
            for (let v of g()) { continue outer; }
        }
        closed;
    "#;
    assert_eq!(run(src), Value::Number(1.0));
}

#[test]
fn for_of_return_closes_generator_with_finally() {
    let src = r#"
        let closed = 0;
        function* g() {
            try { yield 1; yield 2; }
            finally { closed++; }
        }
        function f() {
            for (let v of g()) { return "done"; }
        }
        f();
        closed;
    "#;
    assert_eq!(run(src), Value::Number(1.0));
}

#[test]
fn for_of_throw_closes_generator_with_finally() {
    let src = r#"
        let closed = 0;
        function* g() {
            try { yield 1; yield 2; }
            finally { closed++; }
        }
        try {
            for (let v of g()) { throw "stop"; }
        } catch (e) {}
        closed;
    "#;
    assert_eq!(run(src), Value::Number(1.0));
}

#[test]
fn generator_throw_on_done_rethrows() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let res = vm.run(
        r#"
            function* g() { yield 1; }
            let it = g();
            it.next();
            it.next();
            it.throw("late");
        "#,
    );
    assert!(
        res.is_err(),
        "throw on a finished generator should re-throw"
    );
}
