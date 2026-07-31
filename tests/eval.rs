//! `eval` (indirect + direct).

mod common;
use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

#[test]
fn for_in_var_key_remains_a_var_binding_for_direct_eval() {
    assert_eq!(
        run(r#"
            (function() {
              for (var key = "initial" in { iterated: 1 }) {}
              eval("var key = 'eval';");
              return key;
            }());
        "#),
        Value::String(Arc::from("eval"))
    );
}

#[test]
fn eval_arithmetic() {
    assert_eq!(run(r#"eval("1 + 2 * 3")"#), Value::Number(7.0));
}

#[test]
fn eval_returns_non_string_unchanged() {
    assert_eq!(run(r#"eval(42)"#), Value::Number(42.0));
    assert_eq!(run(r#"eval(null)"#), Value::Null);
}

#[test]
fn eval_reads_global_var() {
    let src = r#"
        let x = 10;
        eval("x + 5");
    "#;
    assert_eq!(run(src), Value::Number(15.0));
}

#[test]
fn eval_var_leaks_to_global() {
    let src = r#"
        eval("var leaked = 99");
        leaked;
    "#;
    assert_eq!(run(src), Value::Number(99.0));
}

#[test]
fn indirect_eval_global_bindings_are_configurable() {
    assert_eq!(
        run(r#"
            (0, eval)("var rujaIndirectVar = 9; function rujaIndirectFn() { return 7; }");
            var vd = Object.getOwnPropertyDescriptor(this, "rujaIndirectVar");
            var fd = Object.getOwnPropertyDescriptor(this, "rujaIndirectFn");
            [
              rujaIndirectVar,
              rujaIndirectFn(),
              vd.writable, vd.enumerable, vd.configurable,
              fd.writable, fd.enumerable, fd.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("9,7,true,true,true,true,true,true"))
    );
}

#[test]
fn indirect_eval_strict_and_lexical_bindings_do_not_leak_to_global() {
    assert_eq!(
        run(r#"
            let outside = "outer";
            (0, eval)("let outside = 'inner'; const hidden = 1;");
            (0, eval)("'use strict'; var strictHidden = 2; function strictFn() {}");
            [
              outside,
              typeof hidden,
              typeof strictHidden,
              typeof strictFn
            ].join(",");
            "#),
        Value::String(Arc::from("outer,undefined,undefined,undefined"))
    );
}

#[test]
fn direct_eval_global_instantiation_checks_and_configurable_functions() {
    assert_eq!(
        run(r#"
            Object.defineProperty(this, "rujaEvalFnDescriptor", {
              value: 0,
              writable: false,
              enumerable: false,
              configurable: true
            });
            eval("function rujaEvalFnDescriptor() { return 345; }");
            var d = Object.getOwnPropertyDescriptor(this, "rujaEvalFnDescriptor");
            [rujaEvalFnDescriptor(), d.writable, d.enumerable, d.configurable].join(",");
            "#),
        Value::String(Arc::from("345,true,true,true"))
    );

    assert_eq!(
        run(r#"
            Object.preventExtensions(this);
            var ok = [];
            try { eval("var rujaNoEvalVar;"); ok.push(false); }
            catch (e) { ok.push(e.constructor === TypeError); }
            try { eval("function NaN() {}"); ok.push(false); }
            catch (e) { ok.push(e.constructor === TypeError); }
            ok.join(",");
            "#),
        Value::String(Arc::from("true,true"))
    );
}

#[test]
fn direct_eval_local_var_bindings_preserve_existing_and_new_are_deletable() {
    assert_eq!(
        run(r#"
            var initial;
            (function() {
              var x = 44443;
              eval("initial = x; var x;");
            }());
            initial;
            "#),
        Value::Number(44443.0)
    );

    assert_eq!(
        run(r#"
            var initial = null;
            var postDeletion;
            (function() {
              eval("initial = x; delete x; postDeletion = function() { x; }; var x;");
            }());
            var ok = [];
            ok.push(initial === undefined);
            try { postDeletion(); ok.push(false); }
            catch (e) { ok.push(e.constructor === ReferenceError); }
            ok.join(",");
            "#),
        Value::String(Arc::from("true,true"))
    );
}

#[test]
fn direct_eval_local_function_bindings_are_deletable_when_new() {
    assert_eq!(
        run(r#"
            var initial, postDeletion;
            (function() {
              eval("initial = f; delete f; postDeletion = function() { f; }; function f() { return 33; }");
            }());
            var ok = [];
            ok.push(typeof initial === "function");
            ok.push(initial());
            try { postDeletion(); ok.push(false); }
            catch (e) { ok.push(e.constructor === ReferenceError); }
            ok.join(",");
            "#),
        Value::String(Arc::from("true,33,true"))
    );
}

#[test]
fn direct_eval_parameter_initializer_rejects_arguments_redeclaration() {
    assert!(run_err(
        r#"
            function f(p = eval("var arguments")) {}
            f();
        "#
    )
    .contains("SyntaxError"));

    assert!(run_err(
        r#"
            var f = (p = eval("var arguments"), arguments) => {};
            f();
        "#
    )
    .contains("SyntaxError"));

    assert_eq!(
        run(r#"
            var f = (p = eval("var arguments = 'param'")) => arguments;
            f();
            "#),
        Value::String(Arc::from("param"))
    );
}

#[test]
fn direct_eval_parameter_conflicts_cover_method_forms() {
    assert_eq!(
        run(r#"
            var bodyCount = 0;
            var ordinary = { m(a = eval("var a")) { bodyCount++; } };
            var generator = { *m(a = eval("var a")) { bodyCount++; } };
            function throwsSyntax(call) {
              try { call(); return false; }
              catch (error) { return error instanceof SyntaxError; }
            }
            class C {
              m(a = eval("var a = 1; var b = 2")) {
                return typeof b + "," + a;
              }
            }
            [
              throwsSyntax(function() { ordinary.m(); }),
              throwsSyntax(function() { generator.m(); }),
              bodyCount,
              new C().m()
            ].join("|");
            "#),
        Value::String(Arc::from("true|true|0|undefined,undefined"))
    );
}

#[test]
fn generator_parameter_initializers_run_at_call_time() {
    assert!(run_err(
        r#"
            function* g(p = eval("var arguments")) {}
            g();
        "#
    )
    .contains("SyntaxError"));

    assert_eq!(
        run(r#"
            var seen = 0;
            function* g(p = (seen = 1)) { yield seen; }
            var iter = g();
            seen;
            "#),
        Value::Number(1.0)
    );

    assert_eq!(
        run(r#"
            function* g(p = 1) { yield p; }
            var iter = g();
            iter.next().value;
            "#),
        Value::Number(1.0)
    );
}

#[test]
fn indirect_eval_runs_in_global_scope() {
    let src = r#"
        function f() {
            let local = 42;
            let e = eval;       // indirect eval reference
            return e("typeof local");
        }
        f();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("undefined")));
}

#[test]
fn direct_eval_with_object_environment_shadow_calls_fake_eval() {
    let src = r#"
        function f() {
            var local = "caller-local";
            var o = {
                eval: function(src) {
                    return this === o ? "fake:" + src : "wrong-this";
                }
            };
            with (o) {
                return eval("local");
            }
        }
        f();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("fake:local")));
}

#[test]
fn direct_eval_with_object_environment_getter_error_propagates() {
    let err = run_err(
        r#"
        var o = {};
        Object.defineProperty(o, "eval", {
            get: function() { throw new Error("eval getter boom"); }
        });
        with (o) {
            eval("1");
        }
        "#,
    );
    assert!(err.contains("eval getter boom"));
}

#[test]
fn direct_eval_with_object_environment_intrinsic_eval_stays_direct() {
    let src = r#"
        function f() {
            var local = "caller-local";
            var o = { eval: eval };
            with (o) {
                return eval("local");
            }
        }
        f();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("caller-local")));
}

#[test]
fn test262_create_realm_eval_runs_in_its_own_global_scope() {
    let src = r#"
        var x = "outside";
        var otherX;
        (function() {
            var other = $262.createRealm().global;
            var eval = other.eval;
            eval('var x = "inside";');
            otherX = other.x;
        }());
        x + "," + otherX;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("outside,inside")));
}

#[test]
fn test262_create_realm_direct_eval_uses_that_realms_intrinsic_eval() {
    let src = r#"
        var other = $262.createRealm().global;
        other.eval('function f() { var x = "other-local"; return eval("x"); }');
        other.eval('f();');
    "#;
    assert_eq!(run(src), Value::String(Arc::from("other-local")));
}

#[test]
fn test262_create_realm_has_distinct_template_registry() {
    let src = r#"
        var other = $262.createRealm().global;
        var strings1 = (function(strings) { return strings; })`1234`;
        var strings2 = other.eval('(function(strings) { return strings; })`1234`');
        strings1 !== strings2;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn test262_create_realm_exposes_constructable_proxy() {
    let src = r#"
        var other = $262.createRealm().global;
        var target = {};
        var proxy = new other.Proxy(target, {
            deleteProperty: function() { return true; }
        });
        delete proxy.x;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn test262_create_realm_exposes_primitive_wrapper_constructors() {
    let src = r#"
        var other = $262.createRealm().global;
        var ok = [];
        try { other.String.prototype.valueOf.call(1); ok.push(false); }
        catch (e) { ok.push(e.constructor === other.TypeError); }
        ok.push(other.Number.prototype.valueOf() === 0);
        ok.push(other.Boolean.prototype.valueOf() === false);
        ok.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true,true,true")));
}

#[test]
fn primitive_references_use_the_current_execution_realms_prototypes() {
    let src = r#"
        var other = $262.createRealm().global;
        var values = [1, "", true, Symbol()];
        var constructors = [other.Number, other.String, other.Boolean, other.Symbol];
        var names = ["number", "string", "boolean", "symbol"];
        var result = [];
        for (var i = 0; i < values.length; i++) {
            constructors[i].prototype.test262 = names[i];
            other.value = values[i];
            result.push(other.eval("value.test262"));
            var count = 0;
            var spy = new Proxy({}, { set: function() { count++; return true; } });
            Object.setPrototypeOf(constructors[i].prototype, spy);
            other.eval(i === 0 ? "0..written = 1" :
                       i === 1 ? "''.written = 1" :
                       i === 2 ? "true.written = 1" : "Symbol().written = 1");
            result.push(count);
        }
        result.push(other.Symbol.prototype !== Symbol.prototype);
        result.push(other.BigInt.prototype !== BigInt.prototype);
        result.push(other.eval("Object.prototype.toString.call(0n)"));
        other.Symbol.prototype.realmOnly = "symbol";
        other.BigInt.prototype.realmOnly = "bigint";
        result.push(Symbol().realmOnly === undefined);
        result.push(0n.realmOnly === undefined);
        result.join(",");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "number,1,string,1,boolean,1,symbol,1,true,true,[object BigInt],true,true"
        ))
    );
}

#[test]
fn proxy_prototype_cycles_do_not_overflow_the_rust_stack_on_set() {
    assert_eq!(
        run(r#"
            var seen = 0;
            var root = { set value(v) { seen = v; } };
            var leaf = root;
            for (var i = 0; i < 129; i++) leaf = Object.create(leaf);
            leaf.value = 7;
            seen;
        "#),
        Value::Number(7.0)
    );

    let error = run_err(
        r#"
            var target = {};
            var proxy = new Proxy(target, {});
            Object.setPrototypeOf(target, proxy);
            var object = Object.create(proxy);
            object.value = 1;
        "#,
    );
    assert!(error.contains("Maximum cyclic property traversal depth exceeded"));

    assert_eq!(
        run(r#"
            var target = {};
            for (var i = 0; i < 10000; i++) target = new Proxy(target, {});
            Reflect.set(target, "value", 1);
        "#),
        Value::Bool(true)
    );
}

#[test]
fn test262_create_realm_exposes_typed_array_constructors() {
    let src = r#"
        var other = $262.createRealm().global;
        var names = [
          "Int8Array",
          "Uint8Array",
          "Uint8ClampedArray",
          "Int16Array",
          "Uint16Array",
          "Int32Array",
          "Uint32Array",
          "Float32Array",
          "Float64Array",
          "BigInt64Array",
          "BigUint64Array"
        ];
        var ok = [];
        for (var i = 0; i < names.length; i++) {
          var name = names[i];
          var C = other[name];
          var sample = name.indexOf("Big") === 0 ? new C([1n]) : new C([1]);
          ok.push(typeof C === "function");
          ok.push(sample.constructor === C);
          ok.push(Object.getPrototypeOf(sample) === C.prototype);
          ok.push(Object.getPrototypeOf(C) === Object.getPrototypeOf(other.Int8Array));
          ok.push(Object.getPrototypeOf(C.prototype) === Object.getPrototypeOf(other.Int8Array.prototype));
          $262.detachArrayBuffer(sample.buffer);
          ok.push(sample[0] === undefined);
        }
        var buffer = new other.ArrayBuffer(8);
        var view = new other.DataView(buffer);
        ok.push(Object.getPrototypeOf(buffer) === other.ArrayBuffer.prototype);
        ok.push(Object.getPrototypeOf(view) === other.DataView.prototype);
        ok.push(other.ArrayBuffer.isView(view));
        ok.join(",");
    "#;
    let expected = vec!["true"; 69].join(",");
    assert_eq!(run(src), Value::String(Arc::from(expected.as_str())));
}

#[test]
fn test262_create_realm_constructor_fallback_uses_new_target_realm() {
    let src = r#"
        var other = $262.createRealm().global;
        var C = new other.Function();
        C.prototype = null;
        var names = [
          "Int8Array",
          "Uint8Array",
          "Uint8ClampedArray",
          "Int16Array",
          "Uint16Array",
          "Int32Array",
          "Uint32Array",
          "Float32Array",
          "Float64Array",
          "BigInt64Array",
          "BigUint64Array"
        ];
        var ok = true;
        for (var i = 0; i < names.length; i++) {
          var name = names[i];
          var args = name.indexOf("Big") === 0 ? [[1n]] : [[1]];
          var sample = Reflect.construct(globalThis[name], args, C);
          ok = ok && Object.getPrototypeOf(sample) === other[name].prototype;
        }
        var buffer = Reflect.construct(ArrayBuffer, [8], C);
        var view = Reflect.construct(DataView, [buffer], C);
        var re = Reflect.construct(RegExp, ["a"], C);
        ok = ok && Object.getPrototypeOf(buffer) === other.ArrayBuffer.prototype;
        ok = ok && Object.getPrototypeOf(view) === other.DataView.prototype;
        ok = ok && Object.getPrototypeOf(re) === other.RegExp.prototype;
        ok;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn cross_realm_non_constructor_native_throws_current_realm_type_error() {
    let src = r#"
        var otherParseInt = $262.createRealm().global.parseInt;
        var ok = [];
        try { new otherParseInt(0); ok.push(false); }
        catch (e) { ok.push(e.constructor === TypeError); }
        try { new otherParseInt; ok.push(false); }
        catch (e) { ok.push(e.constructor === TypeError); }
        try { new parseInt(0); ok.push(false); }
        catch (e) { ok.push(e.constructor === TypeError); }
        ok.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true,true,true")));
}

#[test]
fn direct_eval_reads_caller_local() {
    let src = r#"
        function f() {
            let local = 42;
            return eval("local");
        }
        f();
    "#;
    assert_eq!(run(src), Value::Number(42.0));
}

#[test]
fn direct_eval_assigns_caller_var() {
    let src = r#"
        function f() {
            let a = 1;
            let b = 2;
            eval("a = a + b");
            return a;
        }
        f();
    "#;
    assert_eq!(run(src), Value::Number(3.0));
}

#[test]
fn eval_can_define_and_call_function() {
    let src = r#"
        eval("function sq(n) { return n * n; }");
        sq(7);
    "#;
    assert_eq!(run(src), Value::Number(49.0));
}

#[test]
fn annex_b_eval_mirror_uses_eval_declaration_instantiation_rules() {
    assert_eq!(
        run(r#"
            (function(f) {
              eval('var before = f; { function f() { return 2; } } var after = f;');
              return before + "," + after();
            }(1));
        "#),
        Value::String(Arc::from("1,2"))
    );

    assert_eq!(
        run(r#"
            (function() {
              var f = "outer", inside;
              { let f = "lexical"; eval('{ function f() {} }'); inside = f; }
              return inside + "," + f;
            }());
        "#),
        Value::String(Arc::from("lexical,outer"))
    );

    assert_eq!(
        run(r#"
            (function() {
              var f = "outer", caught;
              try { throw 1; } catch (f) {
                eval('{ function f() { return 3; } }');
                caught = f;
              }
              return caught + "," + f();
            }());
        "#),
        Value::String(Arc::from("1,3"))
    );

    assert_eq!(
        run(r#"
            var f = "global";
            {
              let f = "lexical";
              eval('{ function f() {} }');
            }
            f;
        "#),
        Value::String(Arc::from("global"))
    );

    assert_eq!(
        run(r#"
            (function() {
              var f = "outer", caught;
              try { throw { f: 1 }; } catch ({ f }) {
                eval('{ function f() {} }');
                caught = f;
              }
              return caught + "," + f;
            }());
        "#),
        Value::String(Arc::from("1,outer"))
    );
}

#[test]
fn annex_b_if_function_eval_uses_synthetic_block_admission() {
    assert_eq!(
        run(r#"
            (function(f) {
              eval('var before = f; if (true) function f() { return 4; } var after = f;');
              return before + "," + after();
            }(3));
        "#),
        Value::String(Arc::from("3,4"))
    );
}

#[test]
fn eval_class_declaration_completion_is_empty() {
    assert_eq!(run(r#"eval("class C {}")"#), Value::Undefined);
    assert_eq!(run(r#"eval("1; class C {}")"#), Value::Number(1.0));
}

#[test]
fn direct_eval_inherits_lexical_super_property_context() {
    assert_eq!(
        run("var A={x:1}; var B={}; Object.setPrototypeOf(B,A); var obj={m(){return eval('super.x;');}}; Object.setPrototypeOf(obj,B); obj.m();"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var A={x:1}; var B={}; Object.setPrototypeOf(B,A); var obj={m(){return eval('super[\"x\"];');}}; Object.setPrototypeOf(obj,B); obj.m();"),
        Value::Number(1.0)
    );
    assert!(run_err("eval('super.x;');").contains("SyntaxError"));
}

#[test]
fn direct_eval_with_spread_args_still_direct() {
    // eval(src, ...rest) must remain a direct eval (first arg = source).
    let src = r#"
        function f() {
            let local = 99;
            return eval("local", ...[1, 2, 3]);
        }
        f();
    "#;
    assert_eq!(run(src), Value::Number(99.0));
}

#[test]
fn eval_new_target_contexts_follow_script_and_function_rules() {
    assert!(run_err("new.target;").contains("SyntaxError"));
    assert!(run_err("() => { new.target; };").contains("SyntaxError"));

    assert_eq!(
        run(r#"
            var caught;
            try { eval("new.target;"); } catch (err) { caught = err; }
            caught && caught.constructor === SyntaxError;
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var caught;
            var f = () => eval("new.target;");
            try { f(); } catch (err) { caught = err; }
            caught && caught.constructor === SyntaxError;
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var seen = null, paramSeen = null;
            function F(param = new.target) {
                paramSeen = param;
                seen = eval("new.target;");
            }
            var callResult = F();
            var callSeen = seen;
            var callParam = paramSeen;
            var constructResult = new F();
            callResult === undefined && callSeen === undefined &&
                callParam === undefined && seen === F && paramSeen === F &&
                constructResult instanceof F;
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var caughtGlobal, caughtFunction;
            try { (0, eval)("new.target;"); } catch (err) { caughtGlobal = err; }
            try { (function() { (0, eval)("new.target;"); }()); }
            catch (err) { caughtFunction = err; }
            caughtGlobal.constructor === SyntaxError &&
                caughtFunction.constructor === SyntaxError;
            "#),
        Value::Bool(true)
    );
}

// ---- direct eval lexical-environment isolation (#4) ----

#[test]
fn eval_let_does_not_leak_to_caller() {
    // `let` declared in direct eval must not be visible in the caller.
    let src = r#"
        (function() {
            eval("let x = 5;");
            try { return x; } catch(e) { return "ref-err"; }
        })();
    "#;
    assert_eq!(
        run(src),
        ruja::Value::String(std::sync::Arc::from("ref-err"))
    );
}

#[test]
fn eval_const_does_not_leak_to_caller() {
    let src = r#"
        (function() {
            eval("const c = 9;");
            try { return c; } catch(e) { return "ref-err"; }
        })();
    "#;
    assert_eq!(
        run(src),
        ruja::Value::String(std::sync::Arc::from("ref-err"))
    );
}

#[test]
fn eval_var_leaks_to_caller() {
    // `var` declared in direct eval leaks to the caller's function scope.
    let src = r#"
        (function() {
            eval("var y = 7;");
            return y;
        })();
    "#;
    assert_eq!(run(src), ruja::Value::Number(7.0));
}

#[test]
fn direct_eval_var_conflicts_with_caller_lexical_declarations() {
    let cases = [
        r#"
            (function() {
                let x;
                eval("var x;");
            })();
        "#,
        r#"
            ({
                m() {
                    let x;
                    eval("var x;");
                }
            }).m();
        "#,
        r#"
            ({
                get a() {
                    let x;
                    eval("var x;");
                }
            }).a;
        "#,
        r#"
            ({
                set a(_) {
                    let x;
                    eval("var x;");
                }
            }).a = null;
        "#,
    ];
    for src in cases {
        let err = run_err(src);
        assert!(
            err.contains("SyntaxError"),
            "expected SyntaxError, got {err}"
        );
    }
}

#[test]
fn method_parameter_defaults_use_separate_body_var_environment() {
    let src = r#"
        var x = 'outside';
        var probeParams, probeBody;
        ({
            m(_ = probeParams = function() { return x; }) {
                var x = 'inside';
                probeBody = function() { return x; };
            }
        }.m());
        probeParams() + ':' + probeBody();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("outside:inside")));
}

#[test]
fn eval_var_in_method_parameters_is_visible_to_parameter_and_body_closures() {
    let src = r#"
        var x = 'outside';
        var probe1, probe2, probeBody;
        ({
            m(
                _ = (eval('var x = "inside";'), probe1 = function() { return x; }),
                __ = probe2 = function() { return x; }
            ) {
                probeBody = function() { return x; };
            }
        }.m());
        probe1() + ':' + probe2() + ':' + probeBody();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("inside:inside:inside")));
}

#[test]
fn eval_let_visible_inside_eval() {
    let src = r#"
        eval("let z = 9; z + 1;");
    "#;
    assert_eq!(run(src), ruja::Value::Number(10.0));
}

#[test]
fn eval_let_does_not_leak_at_top_level() {
    // Top-level eval `let` must not create a global binding.
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let _ = vm.run(r#"eval("let w = 3");"#);
    let r = match vm.run("typeof w;") {
        Ok(v) => v,
        Err(_) => ruja::Value::String(std::sync::Arc::from("undefined")),
    };
    assert_eq!(r, ruja::Value::String(std::sync::Arc::from("undefined")));
}

// ---- strict eval: no var leak (#7) ----

#[test]
fn sloppy_eval_still_leaks_var() {
    // Non-strict eval still leaks var (regression for the strict split).
    let src = r#"
        (function() {
            eval("var leaked = 7;");
            return leaked;
        })();
    "#;
    assert_eq!(run(src), ruja::Value::Number(7.0));
}

#[test]
fn direct_eval_inherits_caller_strictness_for_with() {
    let src = r#"
        (function() {
            'use strict';
            try {
                eval("var o = {}; with (o) {}");
            } catch (e) {
                return e.name;
            }
            return "no error";
        })();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("SyntaxError")));
}

#[test]
fn direct_eval_non_strict_allows_contextual_static_binding() {
    assert_eq!(
        run("var count = 0;\
             eval('var static; count += 1;');\
             eval('with ({}) {} count += 1;');\
             eval('unresolvable = null; count += 1;');\
             count;"),
        Value::Number(3.0)
    );
}

#[test]
fn eval_function_indices_are_offset_from_existing_functions() {
    assert_eq!(
        run(r#"
            var existing = function() { return 0; };
            eval("var fresh = function() { return 1; }; fresh();");
        "#),
        Value::Number(1.0)
    );

    assert_eq!(
        run(r#"
            var obj = { p: function() { return 0; } };
            eval("with (obj) { p = function() { return 1; }; }");
            obj.p();
        "#),
        Value::Number(1.0)
    );
}
