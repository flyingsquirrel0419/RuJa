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
