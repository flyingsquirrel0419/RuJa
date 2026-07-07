//! `eval` (indirect + direct).

mod common;
use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

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
