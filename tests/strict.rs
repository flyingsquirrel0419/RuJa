//! Strict mode: directive prologue parsing and enforcement.

mod common;
use common::run;
use ruja::Value;

// ---- directive prologue detection ----

#[test]
fn use_strict_at_top_enables_strict() {
    // A leading "use strict" makes the program strict: `with` is rejected.
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let r = vm.run("\"use strict\"; var o = {x:1}; with(o){ x; }");
    assert!(r.is_err(), "expected with to be rejected in strict mode");
}

#[test]
fn non_directive_use_strict_does_not_enable() {
    // "use strict" after a non-directive statement is just a string expr.
    let r = run("1; \"use strict\"; var o = {x:2}; var r; with(o){ r = x; } r;");
    assert_eq!(r, Value::Number(2.0));
}

#[test]
fn strict_in_function_via_directive() {
    // A function with a "use strict" directive is strict: `with` inside it is
    // a compile-time SyntaxError, while `with` in the (non-strict) outer
    // program is allowed.
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let r = vm.run(
        r#"var o = {x: 5};
           with(o){ }
           function f() {
               "use strict";
               var p = {y: 6};
               with(p){ y; }
           }
           f();"#,
    );
    assert!(r.is_err(), "expected strict with rejection inside function");
}

#[test]
fn strictness_inherits_into_nested_functions() {
    // An outer "use strict" makes nested functions strict too.
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let r = vm.run(
        r#"\"use strict\"; function outer(){ function inner(){ var o={x:1}; with(o){x;} } inner(); } outer();"#,
    );
    assert!(r.is_err(), "nested function should be strict");
}

// ---- duplicate parameter rejection ----

#[test]
fn strict_rejects_duplicate_params() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let r = vm.run("\"use strict\"; function f(a, a){ return a; }");
    assert!(r.is_err(), "expected duplicate param error in strict");
}

#[test]
fn non_strict_allows_duplicate_params_last_wins() {
    // Non-strict: duplicate params allowed, last value wins.
    assert_eq!(
        run("function f(a, a){ return a; } f(1, 2);"),
        Value::Number(2.0)
    );
}

#[test]
fn non_strict_duplicate_param_omitted_last_wins_with_undefined() {
    assert_eq!(
        run("function f(x, a, b, x){ return x; } f(1, 2);"),
        Value::Undefined
    );
}

#[test]
fn function_declarations_overwrite_parameter_and_arguments_bindings() {
    assert_eq!(
        run("function f(x){ return typeof x === 'function'; function x(){ return 7; } } f();"),
        Value::Bool(true)
    );
    assert_eq!(
        run("function f(){ return typeof arguments === 'function'; function arguments(){ return 7; } } f();"),
        Value::Bool(true)
    );
}

#[test]
fn var_declarations_reuse_parameter_bindings() {
    assert_eq!(
        run("function f(x){ var x; return x; } f(1);"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("function f(x){ var x; return x; } f();"),
        Value::Undefined
    );
}

#[test]
fn strict_function_directive_rejects_duplicate_params() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let r = vm.run("function f(a, a){ \"use strict\"; return a; }");
    assert!(r.is_err(), "expected duplicate param error");
}

// ---- classes are always strict ----

#[test]
fn class_methods_are_strict_reject_with() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let r = vm.run(r#"class C { m(){ var o={x:1}; with(o){ x; } } } new C().m();"#);
    assert!(r.is_err(), "class methods are always strict");
}

#[test]
fn class_heritage_is_strict() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let with_err = vm
        .run("class C extends (function B() { with ({}) {} return B; }()) {}")
        .unwrap_err();
    assert!(
        with_err.to_string().contains("strict"),
        "expected strict-mode with rejection, got: {}",
        with_err
    );

    let args_err = vm
        .run(
            "var D = class extends function() { arguments.callee; } {};
             Object.getPrototypeOf(D).arguments;",
        )
        .unwrap_err();
    assert!(
        args_err.to_string().contains("TypeError"),
        "expected restricted function arguments access, got: {}",
        args_err
    );

    let new_err = vm
        .run(
            "var D = class extends function() { arguments.callee; } {};
             new D;",
        )
        .unwrap_err();
    assert!(
        new_err.to_string().contains("TypeError"),
        "expected strict arguments.callee rejection, got: {}",
        new_err
    );
}

#[test]
fn strict_arguments_callee_is_restricted() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let err = vm
        .run("function f() { 'use strict'; return arguments.callee; } f();")
        .unwrap_err();
    assert!(
        err.to_string().contains("TypeError"),
        "expected strict arguments.callee rejection, got: {}",
        err
    );

    assert_eq!(
        run("function f() { return arguments.callee === f; } f();"),
        Value::Bool(true)
    );
}

// ---- with rejection variants ----

#[test]
fn strict_rejects_with_with_clear_message() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let e = vm.run("\"use strict\"; with({}){}").unwrap_err();
    assert!(
        e.to_string().contains("strict"),
        "expected strict-mode message, got: {}",
        e
    );
}

#[test]
fn strict_with_inside_block_scope_also_rejected() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    let r = vm.run("\"use strict\"; { with({}){} }");
    assert!(r.is_err());
}

#[test]
fn strict_block_function_declaration_stays_block_scoped() {
    assert_eq!(
        run(r#""use strict";
               var before, after;
               (function() {
                 try { f; } catch (e) { before = e.constructor === ReferenceError; }
                 { function f() {} }
                 try { f; } catch (e) { after = e.constructor === ReferenceError; }
               }());
               before && after;"#),
        Value::Bool(true)
    );
}

#[test]
fn use_strict_directive_rejects_non_simple_params() {
    for src in [
        r#"function f(a = 0) { "use strict"; }"#,
        r#"var f = function(a = 0) { "use strict"; }"#,
        r#"var f = ({ m(a = 0) { "use strict"; } });"#,
        r#"var f = (a = 0) => { "use strict"; };"#,
        r#"function f([a]) { "use strict"; }"#,
        r#"var f = ([a]) => { "use strict"; };"#,
    ] {
        let err = common::run_err(src);
        assert!(
            err.contains("SyntaxError"),
            "expected SyntaxError for {src:?}, got {err}"
        );
    }
}

#[test]
fn strict_object_literal_early_errors() {
    for src in [
        r#"function f() { "use strict"; ({ let }); }"#,
        r#"function f() { "use strict"; ({ yield }); }"#,
        r#"void { set x(eval) { "use strict"; } };"#,
        r#"void { set x(arguments) { "use strict"; } };"#,
        r#"void { m(eval) { "use strict"; } };"#,
    ] {
        let err = common::run_err(src);
        assert!(
            err.contains("SyntaxError"),
            "expected SyntaxError for {src:?}, got {err}"
        );
    }
}
