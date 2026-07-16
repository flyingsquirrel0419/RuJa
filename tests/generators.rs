//! Lazy generator tests: `function*`/`yield`, `next()`, `for...of`, spread,
//! resume values, return values, and infinite generators (the core reason the
//! VM uses a pull-based generator model rather than eager collection).

mod common;
use common::run;
use ruja::{Value, Vm};
use std::sync::Arc;

#[test]
fn generators_inherit_the_iterator_intrinsic() {
    assert_eq!(
        run(r#"
            function* values() { yield 1; }
            let iterator = values();
            let GeneratorPrototype = Object.getPrototypeOf(values.prototype);
            let IteratorPrototype = Object.getPrototypeOf(GeneratorPrototype);
            [
              typeof Iterator,
              iterator instanceof Iterator,
              IteratorPrototype === Iterator.prototype,
              iterator[Symbol.iterator]() === iterator,
              Object.getPrototypeOf(IteratorPrototype) === Object.prototype,
              Object.prototype.toString.call(IteratorPrototype)
            ].join("|");
        "#),
        Value::String(Arc::from("function|true|true|true|true|[object Iterator]"))
    );
}

#[test]
fn generator_prototype_has_own_to_string_tag() {
    assert_eq!(
        run(r#"
            function* values() {}
            let GeneratorPrototype = Object.getPrototypeOf(values.prototype);
            let desc = Object.getOwnPropertyDescriptor(
                GeneratorPrototype,
                Symbol.toStringTag
            );
            [
              desc.value,
              desc.writable,
              desc.enumerable,
              desc.configurable,
              Object.prototype.toString.call(values())
            ].join("|");
        "#),
        Value::String(Arc::from("Generator|false|false|true|[object Generator]"))
    );
}

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
fn generator_intrinsics_are_isolated_per_realm() {
    assert_eq!(
        run(r#"
            var a = $262.createRealm().global;
            var b = $262.createRealm().global;
            var aFunction = a.eval("(function* (value) { yield value; })");
            var bFunction = b.eval("(function* () {})");
            var AGeneratorFunction = Object.getPrototypeOf(aFunction).constructor;
            var BGeneratorFunction = Object.getPrototypeOf(bFunction).constructor;
            var aGeneratorPrototype = AGeneratorFunction.prototype.prototype;
            var constructorDescriptor = Object.getOwnPropertyDescriptor(
                AGeneratorFunction.prototype,
                "constructor"
            );
            var prototypeDescriptor = Object.getOwnPropertyDescriptor(
                AGeneratorFunction.prototype,
                "prototype"
            );
            var instance = aFunction(7);
            var result = instance.next();
            [
                AGeneratorFunction !== BGeneratorFunction,
                AGeneratorFunction.prototype !== BGeneratorFunction.prototype,
                aGeneratorPrototype !== BGeneratorFunction.prototype.prototype,
                Object.getPrototypeOf(AGeneratorFunction) === a.Function,
                Object.getPrototypeOf(aFunction) === AGeneratorFunction.prototype,
                Object.getPrototypeOf(aFunction.prototype) === aGeneratorPrototype,
                Object.getPrototypeOf(instance) === aFunction.prototype,
                Object.getPrototypeOf(result) === a.Object.prototype,
                AGeneratorFunction.prototype.constructor === AGeneratorFunction,
                aGeneratorPrototype.constructor === AGeneratorFunction.prototype,
                aGeneratorPrototype.next !==
                    BGeneratorFunction.prototype.prototype.next,
                aGeneratorPrototype.next.length,
                constructorDescriptor.writable,
                constructorDescriptor.enumerable,
                constructorDescriptor.configurable,
                prototypeDescriptor.writable,
                prototypeDescriptor.enumerable,
                prototypeDescriptor.configurable,
                result.value,
                result.done
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|1|false|false|true|false|false|true|7|false"
        ))
    );
}

#[test]
fn generator_realm_fallbacks_ignore_mutable_globals_and_survive_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    vm.run(
        r#"
        var mainGeneratorFunction = Object.getPrototypeOf(function* () {}).constructor;
        var other = $262.createRealm().global;
        var otherEval = other.eval;
        var foreignFunction = otherEval("(function* () {})");
        var foreignGeneratorFunction = Object.getPrototypeOf(foreignFunction).constructor;
        var foreignGeneratorFunctionPrototype = foreignGeneratorFunction.prototype;
        var foreignGeneratorPrototype = foreignGeneratorFunctionPrototype.prototype;
        var constructorRef = new WeakRef(foreignGeneratorFunction);
        var functionPrototypeRef = new WeakRef(foreignGeneratorFunctionPrototype);
        var generatorPrototypeRef = new WeakRef(foreignGeneratorPrototype);
        var target = new other.Function();
        var newTarget = new Proxy(target, {
            get: function (target, key, receiver) {
                if (key === "prototype") {
                    forceGc();
                    return null;
                }
                return Reflect.get(target, key, receiver);
            }
        });
        delete foreignGeneratorFunctionPrototype.constructor;
        delete foreignGeneratorFunctionPrototype.prototype;
        delete foreignGeneratorPrototype.constructor;
        foreignFunction = null;
        foreignGeneratorFunction = null;
        foreignGeneratorFunctionPrototype = null;
        foreignGeneratorPrototype = null;
        other.Function = null;
        other.Object = null;
    "#,
    )
    .expect("failed to prepare foreign generator Realm");

    vm.gc();
    assert_eq!(
        vm.run(
            r#"
            var dynamic = Reflect.construct(mainGeneratorFunction, [], newTarget);
            var rootedConstructor = constructorRef.deref();
            var rootedFunctionPrototype = functionPrototypeRef.deref();
            var rootedGeneratorPrototype = generatorPrototypeRef.deref();
            var fresh = otherEval("(function* () {})");
            fresh.prototype = null;
            var fallbackInstance = fresh();
            [
                rootedConstructor !== undefined,
                rootedFunctionPrototype !== undefined,
                rootedGeneratorPrototype !== undefined,
                Object.getPrototypeOf(dynamic) === rootedFunctionPrototype,
                Object.getPrototypeOf(dynamic.prototype) ===
                    mainGeneratorFunction.prototype.prototype,
                Object.getPrototypeOf(fallbackInstance) === rootedGeneratorPrototype,
                Object.getPrototypeOf(fresh) === rootedFunctionPrototype,
                !Object.prototype.hasOwnProperty.call(
                    rootedFunctionPrototype,
                    "constructor"
                ),
                !Object.prototype.hasOwnProperty.call(
                    rootedFunctionPrototype,
                    "prototype"
                ),
                !Object.prototype.hasOwnProperty.call(
                    rootedGeneratorPrototype,
                    "constructor"
                )
            ].join("|");
        "#
        )
        .expect("failed to inspect rooted generator intrinsics"),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true"
        ))
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
fn async_generator_awaits_yielded_and_returned_promises() {
    let src = r#"
        async function* gen() {
            yield Promise.resolve(4);
            return Promise.resolve(5);
        }
        let iter = gen();
        let first = await iter.next();
        let second = await iter.next();
        [first.value, first.done, second.value, second.done].join("|");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("4|false|5|true")));
}

#[test]
fn async_generator_yield_rejection_throws_into_body_or_closes_generator() {
    let src = r#"
        let error = {};
        async function* uncaught() {
            yield Promise.reject(error);
            yield "unreachable";
        }
        async function* caught() {
            try {
                yield Promise.reject("bad");
            } catch (reason) {
                yield "caught:" + reason;
            }
        }

        let uncaughtIter = uncaught();
        let rejectedWithOriginal;
        try {
            await uncaughtIter.next();
            rejectedWithOriginal = false;
        } catch (reason) {
            rejectedWithOriginal = reason === error;
        }
        let closed = await uncaughtIter.next();
        let recovered = await caught().next();
        [
            rejectedWithOriginal,
            closed.value === undefined,
            closed.done,
            recovered.value,
            recovered.done
        ].join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("true|true|true|caught:bad|false"))
    );
}

#[test]
fn await_thenable_rejection_and_getter_error_propagate() {
    let src = r#"
        async function rejectedThenable() {
            try {
                await { then(resolve, reject) { reject("rejected"); } };
            } catch (error) {
                return error;
            }
        }
        async function throwingGetter() {
            try {
                await { get then() { throw "getter"; } };
            } catch (error) {
                return error;
            }
        }
        (await rejectedThenable()) + "|" + (await throwingGetter());
    "#;
    assert_eq!(run(src), Value::String(Arc::from("rejected|getter")));
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
fn async_generator_intrinsics_are_isolated_from_object_prototype() {
    let src = r#"
        let AsyncGeneratorFunction = (async function*() {}).constructor;
        let AsyncGeneratorPrototype = AsyncGeneratorFunction.prototype.prototype;
        let AsyncIteratorPrototype = Object.getPrototypeOf(AsyncGeneratorPrototype);
        let functionConstructorDesc = Object.getOwnPropertyDescriptor(
            AsyncGeneratorFunction.prototype,
            "constructor"
        );
        let generatorConstructorDesc = Object.getOwnPropertyDescriptor(
            AsyncGeneratorPrototype,
            "constructor"
        );
        Object.defineProperty(AsyncIteratorPrototype, Symbol.iterator, {
            get() { throw new Error("@@iterator accessed"); }
        });
        Object.defineProperty(AsyncIteratorPrototype, Symbol.asyncIterator, {
            get() { throw new Error("@@asyncIterator accessed"); }
        });
        async function* gen() { yield* []; }
        let result = await gen().next();
        let dynamic = new AsyncGeneratorFunction("value", "yield value;");
        let dynamicResult = await dynamic(7).next();
        [
            AsyncGeneratorFunction.name,
            AsyncIteratorPrototype !== Object.prototype,
            AsyncGeneratorFunction.prototype[Symbol.toStringTag],
            AsyncGeneratorPrototype[Symbol.toStringTag],
            AsyncGeneratorPrototype.constructor === AsyncGeneratorFunction.prototype,
            AsyncGeneratorPrototype.next.length,
            !functionConstructorDesc.writable && functionConstructorDesc.configurable,
            !generatorConstructorDesc.writable && generatorConstructorDesc.configurable,
            result.done,
            result.value === undefined,
            Object.getPrototypeOf(dynamic) === AsyncGeneratorFunction.prototype,
            dynamicResult.value,
            dynamicResult.done
        ].join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "AsyncGeneratorFunction|true|AsyncGeneratorFunction|AsyncGenerator|true|1|true|true|true|true|true|7|false"
        ))
    );
}

#[test]
fn async_generator_intrinsics_and_completion_values_are_isolated_per_realm() {
    assert_eq!(
        run(r#"
            var a = $262.createRealm().global;
            var b = $262.createRealm().global;
            var aFunction = a.eval(
                "(async function* (value) { await 0; yield value; return value + 1; })"
            );
            var bFunction = b.eval("(async function* () {})");
            var AAsyncGeneratorFunction = Object.getPrototypeOf(aFunction).constructor;
            var BAsyncGeneratorFunction = Object.getPrototypeOf(bFunction).constructor;
            var aGeneratorPrototype = AAsyncGeneratorFunction.prototype.prototype;
            var bGeneratorPrototype = BAsyncGeneratorFunction.prototype.prototype;
            var aIteratorPrototype = Object.getPrototypeOf(aGeneratorPrototype);
            var bIteratorPrototype = Object.getPrototypeOf(bGeneratorPrototype);
            var mainGeneratorPrototype = (async function* () {}).constructor.prototype.prototype;

            var aGenerator = aFunction(7);
            var aPromise = aGenerator.next();
            var aResult = await aPromise;
            var borrowedMainPromise = mainGeneratorPrototype.next.call(aFunction(8));
            var borrowedMainResult = await borrowedMainPromise;
            var mainGenerator = (async function* () { await 0; yield 9; })();
            var borrowedForeignPromise = aGeneratorPrototype.next.call(mainGenerator);
            var borrowedForeignResult = await borrowedForeignPromise;
            var borrowedReturnPromise = mainGeneratorPrototype.return.call(
                aFunction(10),
                11
            );
            var borrowedReturnResult = await borrowedReturnPromise;
            var borrowedThrowPromise = mainGeneratorPrototype.throw.call(
                aFunction(12),
                "stop"
            );
            var borrowedThrowReason = await borrowedThrowPromise.then(
                function () { return false; },
                function (reason) { return reason === "stop"; }
            );
            var incompatibleError = await aGeneratorPrototype.next.call({}).then(
                function () { return false; },
                function (error) { return error instanceof a.TypeError; }
            );
            var delayedError = await a.eval(
                "(async function* () { await 0; null.missing; })"
            )().next().then(
                function () { return false; },
                function (error) { return error instanceof a.TypeError; }
            );
            var constructorDescriptor = Object.getOwnPropertyDescriptor(
                AAsyncGeneratorFunction.prototype,
                "constructor"
            );
            var prototypeDescriptor = Object.getOwnPropertyDescriptor(
                AAsyncGeneratorFunction.prototype,
                "prototype"
            );
            [
                AAsyncGeneratorFunction !== BAsyncGeneratorFunction,
                AAsyncGeneratorFunction.prototype !== BAsyncGeneratorFunction.prototype,
                aGeneratorPrototype !== bGeneratorPrototype,
                aIteratorPrototype !== bIteratorPrototype,
                Object.getPrototypeOf(AAsyncGeneratorFunction) === a.Function,
                Object.getPrototypeOf(aFunction) === AAsyncGeneratorFunction.prototype,
                Object.getPrototypeOf(aFunction.prototype) === aGeneratorPrototype,
                Object.getPrototypeOf(aGenerator) === aFunction.prototype,
                aGeneratorPrototype.next !== bGeneratorPrototype.next,
                aIteratorPrototype[Symbol.asyncIterator] !==
                    bIteratorPrototype[Symbol.asyncIterator],
                Object.getPrototypeOf(aIteratorPrototype) === a.Object.prototype,
                aPromise instanceof a.Promise,
                Object.getPrototypeOf(aResult) === a.Object.prototype,
                borrowedMainPromise instanceof Promise,
                !(borrowedMainPromise instanceof a.Promise),
                Object.getPrototypeOf(borrowedMainResult) === a.Object.prototype,
                borrowedForeignPromise instanceof a.Promise,
                !(borrowedForeignPromise instanceof Promise),
                Object.getPrototypeOf(borrowedForeignResult) === Object.prototype,
                borrowedReturnPromise instanceof Promise,
                !(borrowedReturnPromise instanceof a.Promise),
                Object.getPrototypeOf(borrowedReturnResult) === a.Object.prototype,
                borrowedThrowPromise instanceof Promise,
                !(borrowedThrowPromise instanceof a.Promise),
                borrowedThrowReason,
                incompatibleError,
                delayedError,
                constructorDescriptor.writable,
                constructorDescriptor.enumerable,
                constructorDescriptor.configurable,
                prototypeDescriptor.writable,
                prototypeDescriptor.enumerable,
                prototypeDescriptor.configurable,
                aResult.value,
                borrowedMainResult.value,
                borrowedForeignResult.value,
                borrowedReturnResult.value,
                borrowedReturnResult.done
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|false|false|true|false|false|true|7|8|9|11|true"
        ))
    );
}

#[test]
fn async_generator_realm_fallbacks_ignore_mutable_globals_and_survive_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    vm.run(
        r#"
        var mainAsyncGeneratorFunction = Object.getPrototypeOf(async function* () {}).constructor;
        var mainAsyncGeneratorPrototype = mainAsyncGeneratorFunction.prototype.prototype;
        var other = $262.createRealm().global;
        var otherEval = other.eval;
        var foreignPromise = other.Promise;
        var foreignObjectPrototype = other.Object.prototype;
        var foreignFunction = otherEval("(async function* () {})");
        var foreignConstructor = Object.getPrototypeOf(foreignFunction).constructor;
        var foreignFunctionPrototype = foreignConstructor.prototype;
        var foreignGeneratorPrototype = foreignFunctionPrototype.prototype;
        var foreignIteratorPrototype = Object.getPrototypeOf(foreignGeneratorPrototype);
        var constructorRef = new WeakRef(foreignConstructor);
        var functionPrototypeRef = new WeakRef(foreignFunctionPrototype);
        var generatorPrototypeRef = new WeakRef(foreignGeneratorPrototype);
        var iteratorPrototypeRef = new WeakRef(foreignIteratorPrototype);
        var target = new other.Function();
        var newTarget = new Proxy(target, {
            get: function (target, key, receiver) {
                if (key === "prototype") {
                    forceGc();
                    return null;
                }
                return Reflect.get(target, key, receiver);
            }
        });
        delete foreignFunctionPrototype.constructor;
        delete foreignFunctionPrototype.prototype;
        delete foreignGeneratorPrototype.constructor;
        delete foreignIteratorPrototype[Symbol.asyncIterator];
        Object.setPrototypeOf(foreignGeneratorPrototype, null);
        foreignFunction = null;
        foreignConstructor = null;
        foreignFunctionPrototype = null;
        foreignGeneratorPrototype = null;
        foreignIteratorPrototype = null;
        other.Function = null;
        other.Object = null;
        other.Promise = null;
    "#,
    )
    .expect("failed to prepare foreign async generator Realm");

    vm.gc();
    assert_eq!(
        vm.run(
            r#"
            var dynamic = Reflect.construct(mainAsyncGeneratorFunction, [], newTarget);
            var rootedConstructor = constructorRef.deref();
            var rootedFunctionPrototype = functionPrototypeRef.deref();
            var rootedGeneratorPrototype = generatorPrototypeRef.deref();
            var rootedIteratorPrototype = iteratorPrototypeRef.deref();
            var fresh = otherEval("(async function* () { await 0; yield 1; })");
            fresh.prototype = null;
            var fallbackInstance = fresh();
            var promise = fallbackInstance.next();
            var result = await promise;
            [
                rootedConstructor !== undefined,
                rootedFunctionPrototype !== undefined,
                rootedGeneratorPrototype !== undefined,
                rootedIteratorPrototype !== undefined,
                Object.getPrototypeOf(dynamic) === rootedFunctionPrototype,
                Object.getPrototypeOf(dynamic.prototype) === mainAsyncGeneratorPrototype,
                Object.getPrototypeOf(fallbackInstance) === rootedGeneratorPrototype,
                Object.getPrototypeOf(fresh) === rootedFunctionPrototype,
                Object.getPrototypeOf(rootedGeneratorPrototype) === null,
                promise instanceof foreignPromise,
                Object.getPrototypeOf(result) === foreignObjectPrototype,
                !Object.prototype.hasOwnProperty.call(
                    rootedFunctionPrototype,
                    "constructor"
                ),
                !Object.prototype.hasOwnProperty.call(
                    rootedFunctionPrototype,
                    "prototype"
                ),
                !Object.prototype.hasOwnProperty.call(
                    rootedGeneratorPrototype,
                    "constructor"
                ),
                !Object.prototype.hasOwnProperty.call(
                    rootedIteratorPrototype,
                    Symbol.asyncIterator
                )
            ].join("|");
        "#,
        )
        .expect("failed to inspect rooted async generator intrinsics"),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn async_generator_yield_star_only_unwraps_async_from_sync_values() {
    let src = r#"
        let nativePromise = Promise.resolve("native");
        let nativeAsyncIterator = {
            [Symbol.asyncIterator]() { return this; },
            next() { return { value: nativePromise, done: false }; }
        };
        let syncIterable = {
            [Symbol.iterator]() {
                return {
                    next() {
                        return { value: Promise.resolve("sync"), done: false };
                    }
                };
            }
        };
        async function* delegate(value) { yield* value; }
        let nativeResult = await delegate(nativeAsyncIterator).next();
        let syncResult = await delegate(syncIterable).next();
        [nativeResult.value === nativePromise, syncResult.value].join("|");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true|sync")));
}

#[test]
fn async_generator_yield_star_closes_on_abrupt_value_resolution_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var closeCount = 0;
            var reason;
            var poisoned = Promise.resolve(1);
            Object.defineProperty(poisoned, "constructor", {
                get() { reason = {}; throw reason; }
            });
            var iterable = {
                [Symbol.iterator]() {
                    var done = false;
                    return {
                        next() {
                            if (done) return { done: true };
                            done = true;
                            return { value: poisoned, done: false };
                        },
                        return() {
                            closeCount += 1;
                            forceGc();
                            return { done: true };
                        }
                    };
                }
            };
            async function* delegate() { yield* iterable; }
            var preserved = await delegate().next().then(
                function () { return false; },
                function (error) { return error === reason; }
            );
            forceGc();
            [preserved, closeCount].join("|");
        "#
        )
        .expect("async-from-sync abrupt reason should survive close and GC"),
        Value::String(Arc::from("true|1"))
    );
}

#[test]
fn async_generator_yield_star_async_from_sync_uses_generator_realm_intrinsics() {
    let source = r#"
        var other = $262.createRealm().global;
        var foreignPromise = other.Promise;
        var foreignObjectPrototype = other.Object.prototype;
        var foreignDelegate = other.eval(
            "(async function* (iterable) { yield* iterable; })"
        );
        other.Promise = function PoisonedPromise() { throw new Error("poisoned"); };
        var iterable = {
            [Symbol.iterator]() {
                return {
                    next() { return { value: 17, done: false }; }
                };
            }
        };
        var iterator = foreignDelegate(iterable);
        var promise = iterator.next();
        var result = await promise;
        [
            promise instanceof foreignPromise,
            Object.getPrototypeOf(result) === foreignObjectPrototype,
            result.value,
            result.done
        ].join("|");
    "#;
    assert_eq!(run(source), Value::String(Arc::from("true|true|17|false")));
}

#[test]
fn async_generator_yield_star_return_does_not_close_twice_on_value_rejection() {
    let source = r#"
        var returnCount = 0;
        var reason = {};
        var iterable = {
            [Symbol.iterator]() {
                return {
                    next() { return { value: 1, done: false }; },
                    return(value) {
                        returnCount += 1;
                        return { value: Promise.reject(reason), done: true };
                    }
                };
            }
        };
        async function* delegate() { yield* iterable; }
        var iterator = delegate();
        await iterator.next();
        var preserved = await iterator.return(2).then(
            function () { return false; },
            function (error) { return error === reason; }
        );
        [preserved, returnCount].join("|");
    "#;
    assert_eq!(run(source), Value::String(Arc::from("true|1")));
}

#[test]
fn async_generator_yield_star_awaits_thenable_and_rewraps_result() {
    let src = r#"
        let log = [];
        let delegated;
        let iterable = {
            [Symbol.asyncIterator]() {
                return {
                    next() {
                        return {
                            name: "thenable",
                            get then() {
                                log.push("get then");
                                return function(resolve) {
                                    log.push("call then:" + this.name + ":" + arguments.length);
                                    delegated = {
                                        get done() { log.push("get done"); return false; },
                                        get value() { log.push("get value"); return 7; }
                                    };
                                    resolve(delegated);
                                };
                            }
                        };
                    }
                };
            }
        };
        async function* outer() { yield* iterable; }
        let result = await outer().next();
        [
            result !== delegated,
            result.value,
            result.done,
            log.join(",")
        ].join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "true|7|false|get then,call then:thenable:2,get done,get value"
        ))
    );
}

#[test]
fn async_generator_yield_star_rewraps_sync_iterator_result() {
    let src = r#"
        let log = [];
        let delegated;
        let iterable = {
            [Symbol.iterator]() {
                return {
                    next() {
                        delegated = {
                            get done() { log.push("get done"); return false; },
                            get value() { log.push("get value"); return 8; }
                        };
                        return delegated;
                    }
                };
            }
        };
        async function* outer() { yield* iterable; }
        let result = await outer().next();
        [result !== delegated, result.value, result.done, log.join(",")].join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("true|8|false|get done,get value"))
    );
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

#[test]
fn async_generator_explicit_return_adds_await_boundary() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var actual = [];
        async function* implicit() {}
        async function* bare() { return; }
        async function* explicit() { return undefined; }
        async function* explicitVoid() { return void 0; }

        Promise.resolve()
            .then(() => actual.push("tick1"))
            .then(() => actual.push("tick2"));
        implicit().next().then(() => actual.push("implicit"));
        bare().next().then(() => actual.push("bare"));
        explicit().next().then(() => actual.push("explicit"));
        explicitVoid().next().then(() => actual.push("explicitVoid"));
        "#,
    )
    .expect("async generator scheduling failed");

    assert_eq!(
        vm.run("actual.join('|');")
            .expect("failed to read scheduling log"),
        Value::String(Arc::from("tick1|implicit|bare|tick2|explicit|explicitVoid"))
    );
}

#[test]
fn async_generator_return_then_getter_observes_job_order() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var actual = [];
        async function* gen() {
            actual.push("start");
            yield 123;
            actual.push("unreachable");
        }

        Promise.resolve()
            .then(() => actual.push("tick1"))
            .then(() => actual.push("tick2"));
        var iterator = gen();
        iterator.next();
        iterator.return({
            get then() { actual.push("get then"); }
        });
        "#,
    )
    .expect("async generator scheduling failed");

    assert_eq!(
        vm.run("actual.join('|');")
            .expect("failed to read scheduling log"),
        Value::String(Arc::from("start|tick1|get then|tick2"))
    );
}

#[test]
fn async_generator_yield_star_return_awaits_each_stage() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var actual = [];
        var asyncIterator = {
            [Symbol.asyncIterator]() { return this; },
            next() { return { done: false }; },
            get return() { actual.push("get return"); }
        };
        async function* gen() {
            actual.push("start");
            yield* asyncIterator;
            actual.push("unreachable");
        }

        Promise.resolve()
            .then(() => actual.push("tick1"))
            .then(() => actual.push("tick2"))
            .then(() => actual.push("tick3"));
        var iterator = gen();
        iterator.next();
        iterator.return({
            get then() { actual.push("get then"); }
        });
        "#,
    )
    .expect("async generator scheduling failed");

    assert_eq!(
        vm.run("actual.join('|');")
            .expect("failed to read scheduling log"),
        Value::String(Arc::from(
            "start|tick1|get then|tick2|get return|get then|tick3"
        ))
    );
}

#[test]
fn async_generator_await_keeps_block_environment_alive_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var release;
        var gate = new Promise(resolve => { release = resolve; });
        async function* gen() {
            {
                let held = { value: 42 };
                await gate;
                yield held.value;
            }
        }
        var iterator = gen();
        var request = iterator.next();
        "#,
    )
    .expect("failed to suspend async generator");

    vm.gc();
    vm.run("release();")
        .expect("failed to resume async generator");
    vm.run("var resumed; request.then(result => { resumed = result.value; });")
        .expect("failed to observe async generator result");

    assert_eq!(
        vm.run("resumed;").expect("failed to read resumed value"),
        Value::Number(42.0)
    );
}

#[test]
fn async_generator_yield_star_getter_errors_reach_the_body_catch() {
    let src = r#"
        var returnToken = {};
        var throwToken = {};
        function makeIterator(method, token) {
            return {
                [Symbol.asyncIterator]() { return this; },
                next() { return { done: false, value: undefined }; },
                [method]() {
                    return {
                        done: false,
                        get value() { throw token; }
                    };
                }
            };
        }
        async function* delegate(iterator) {
            try {
                yield* iterator;
            } catch (error) {
                return error;
            }
        }
        var returnGenerator = delegate(makeIterator("return", returnToken));
        var throwGenerator = delegate(makeIterator("throw", throwToken));
        var actual = [];
        returnGenerator.next()
            .then(() => returnGenerator.return())
            .then(result => actual.push(result.value === returnToken));
        throwGenerator.next()
            .then(() => throwGenerator.throw())
            .then(result => actual.push(result.value === throwToken));
    "#;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(src).expect("delegated getter error test failed");
    assert_eq!(
        vm.run("actual.join('|');")
            .expect("failed to read delegated getter results"),
        Value::String(Arc::from("true|true"))
    );
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
        let valueGets = 0;
        let first = { get value() { valueGets++; return 1; } };
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
            valueGets,
            completed.value,
            completed.done,
            calls.join(",")
        ].join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("true|false|0|42|true|1:undefined,1:7"))
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

#[test]
fn async_generator_methods_reject_incompatible_receivers() {
    let src = r#"
        async function* asyncGenerator() {}
        function* syncGenerator() {}
        let proto = Object.getPrototypeOf(asyncGenerator).prototype;
        let receivers = [
            undefined,
            1,
            "string",
            null,
            true,
            Symbol(),
            {},
            function() {},
            asyncGenerator,
            asyncGenerator.prototype,
            syncGenerator()
        ];
        let methods = [proto.next, proto.return, proto.throw];
        let rejected = 0;
        let synchronousThrows = 0;
        for (let method of methods) {
            for (let receiver of receivers) {
                let promise;
                try {
                    promise = method.call(receiver);
                } catch (error) {
                    synchronousThrows++;
                    continue;
                }
                try {
                    await promise;
                } catch (error) {
                    if (error instanceof TypeError) rejected++;
                }
            }
        }
        synchronousThrows + "|" + rejected;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("0|33")));
}

#[test]
fn iterator_result_properties_are_enumerable_data_properties() {
    let src = r#"
        function shape(result) {
            let value = Object.getOwnPropertyDescriptor(result, "value");
            let done = Object.getOwnPropertyDescriptor(result, "done");
            return [
                Object.keys(result).join(","),
                value.writable,
                value.enumerable,
                value.configurable,
                done.writable,
                done.enumerable,
                done.configurable
            ].join("|");
        }

        function* syncGenerator() { yield 1; }
        async function* asyncGenerator() { yield 1; }
        let expected = "value,done|true|true|true|true|true|true";
        [
            shape(syncGenerator().next()),
            shape(await asyncGenerator().next()),
            shape([1].values().next()),
            shape("a".matchAll(/a/g).next())
        ].every(result => result === expected);
    "#;
    assert_eq!(run(src), Value::Bool(true));
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
