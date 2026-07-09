//! Logical operators (and/or/nullish) and logical/compound assignment,
//! including member and element targets.

mod common;
use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

// --- && short-circuit ---

#[test]
fn logical_and_truthy() {
    assert_eq!(run("1 && 2;"), Value::Number(2.0));
    assert_eq!(run("true && 'x';"), Value::String(Arc::from("x")));
}

#[test]
fn logical_and_falsy_keeps_left() {
    assert_eq!(run("0 && 2;"), Value::Number(0.0));
    assert_eq!(run("null && 2;"), Value::Null);
    assert_eq!(run("'' && 'x';"), Value::String(Arc::from("")));
    assert_eq!(run("false && true;"), Value::Bool(false));
    assert_eq!(run("undefined && 1;"), Value::Undefined);
}

#[test]
fn logical_and_chain() {
    assert_eq!(run("1 && 2 && 3;"), Value::Number(3.0));
    assert_eq!(run("1 && 0 && 3;"), Value::Number(0.0));
}

#[test]
fn nullish_member_assignment_throws_in_sloppy_mode() {
    assert!(
        run_err("null.x = 1;").contains("TypeError"),
        "null member assignment must throw"
    );
    assert!(
        run_err("undefined.x = 1;").contains("TypeError"),
        "undefined member assignment must throw"
    );
}

// --- || short-circuit ---

#[test]
fn logical_or_falsy() {
    assert_eq!(run("0 || 2;"), Value::Number(2.0));
    assert_eq!(run("null || 'd';"), Value::String(Arc::from("d")));
    assert_eq!(run("false || true;"), Value::Bool(true));
    assert_eq!(run("'' || 'x';"), Value::String(Arc::from("x")));
}

#[test]
fn logical_or_truthy_keeps_left() {
    assert_eq!(run("1 || 2;"), Value::Number(1.0));
    assert_eq!(run("'a' || 'b';"), Value::String(Arc::from("a")));
}

#[test]
fn logical_or_chain() {
    assert_eq!(run("0 || 0 || 3;"), Value::Number(3.0));
    assert_eq!(run("1 || 2 || 3;"), Value::Number(1.0));
}

// --- ?? nullish coalescing ---

#[test]
fn nullish_null() {
    assert_eq!(run("null ?? 1;"), Value::Number(1.0));
}

#[test]
fn nullish_undefined() {
    assert_eq!(run("undefined ?? 5;"), Value::Number(5.0));
}

#[test]
fn undefined_can_be_declared_as_var_name() {
    assert_eq!(run("var undefined;"), Value::Undefined);
    assert_eq!(run("var undefined = 1;"), Value::Undefined);
}

#[test]
fn global_undefined_is_read_only_reference() {
    assert_eq!(
        run("undefined = 5; typeof undefined;"),
        Value::String(Arc::from("undefined"))
    );
    assert_eq!(
        run("var result = undefined = 42; result;"),
        Value::Number(42.0)
    );
    assert_eq!(
        run("(delete undefined) + ':' + typeof undefined;"),
        Value::String(Arc::from("false:undefined"))
    );
    let err = run_err(r#""use strict"; undefined = 12;"#);
    assert!(err.contains("TypeError"), "got: {}", err);
}

#[test]
fn nullish_keeps_falsy_non_nullish() {
    // 0, '', false are NOT nullish -> kept as-is.
    assert_eq!(run("0 ?? 2;"), Value::Number(0.0));
    assert_eq!(run("'' ?? 'x';"), Value::String(Arc::from("")));
    assert_eq!(run("false ?? true;"), Value::Bool(false));
}

#[test]
fn nullish_non_nullish() {
    assert_eq!(run("1 ?? null;"), Value::Number(1.0));
    assert_eq!(run("'a' ?? 'b';"), Value::String(Arc::from("a")));
}

#[test]
fn nullish_chain() {
    assert_eq!(run("null ?? undefined ?? 3;"), Value::Number(3.0));
    assert_eq!(run("1 ?? 2 ?? 3;"), Value::Number(1.0));
}

// --- mixed precedence ---

#[test]
fn nullish_lower_than_or() {
    for src in [
        "1 || 2 ?? 3;",
        "1 && 2 ?? 3;",
        "1 ?? 2 || 3;",
        "1 ?? 2 && 3;",
    ] {
        assert!(run_err(src).contains("SyntaxError"), "{src}");
    }

    assert_eq!(run("(0 || 2) ?? 3;"), Value::Number(2.0));
    assert_eq!(run("0 ?? (2 || 3);"), Value::Number(0.0));
    assert_eq!(run("(1 && 2) ?? 3;"), Value::Number(2.0));
    assert_eq!(run("null ?? (0 && 3);"), Value::Number(0.0));
}

#[test]
fn and_or_mix() {
    assert_eq!(run("0 && 1 || 2;"), Value::Number(2.0));
    assert_eq!(run("1 && 1 || 0;"), Value::Number(1.0));
}

// --- simple assignment ---

#[test]
fn assign_ident() {
    assert_eq!(run("var a; a = 5; a;"), Value::Number(5.0));
}

#[test]
fn assign_ident_preserves_resolved_reference_across_rhs() {
    assert_eq!(
        run(r#"
            function f() {
              var x = 0;
              var scope = { x: 1 };
              with (scope) {
                x = (delete scope.x, 2);
              }
              return scope.x + ':' + x;
            }
            f();
            "#,),
        Value::String(Arc::from("2:0"))
    );

    assert_eq!(
        run(r#"
            function f() {
              var x = 0;
              var inner = (function() {
                x = (eval('var x;'), 1);
                return x;
              })();
              return String(inner) + ':' + x;
            }
            f();
            "#,),
        Value::String(Arc::from("undefined:1"))
    );

    assert_eq!(
        run(r#"
            function f() {
              var scope = {};
              with (scope) {
                missing = 2;
              }
              return scope.missing + ':' + missing;
            }
            f();
            "#,),
        Value::String(Arc::from("undefined:2"))
    );
}

#[test]
fn assign_member() {
    assert_eq!(run("var o = {n: 0}; o.n = 7; o.n;"), Value::Number(7.0));
}

#[test]
fn assign_member_allows_escaped_keyword_property_name() {
    assert_eq!(
        run(r#"var obj = {}; obj.st\u0061tic = 42; obj.static;"#),
        Value::Number(42.0)
    );
}

#[test]
fn assign_anonymous_function_name_only_for_bare_identifier_ref() {
    assert_eq!(
        run("var fn; fn = function() {}; fn.name;"),
        Value::String(Arc::from("fn"))
    );
    assert_eq!(
        run("var cover; cover = (function() {}); cover.name;"),
        Value::String(Arc::from("cover"))
    );
    assert_eq!(
        run("var fn; (fn) = function() {}; fn.name;"),
        Value::String(Arc::from(""))
    );
    assert_eq!(
        run("var obj = {}; obj.attr = function() {}; obj.attr.name;"),
        Value::String(Arc::from(""))
    );
}

#[test]
fn native_function_length_is_read_only_own_property() {
    let err = run_err(r#""use strict"; Function.length = 42;"#);
    assert!(err.contains("TypeError"), "got: {}", err);

    assert_eq!(
        run(r#"
            var d = Object.getOwnPropertyDescriptor(Function, "length");
            [Function.length, d.value, d.writable, d.enumerable, d.configurable].join(",");
            "#,),
        Value::String(Arc::from("1,1,false,false,true"))
    );
}

#[test]
fn implicit_global_assignment_defines_global_object_property() {
    assert_eq!(
        run(r#"
            function f() {
              implicitGlobalForDescriptor = 42;
            }
            f();
            var d = Object.getOwnPropertyDescriptor(this, "implicitGlobalForDescriptor");
            [
              implicitGlobalForDescriptor,
              d.value,
              d.writable,
              d.enumerable,
              d.configurable
            ].join(",");
            "#,),
        Value::String(Arc::from("42,42,true,true,true"))
    );

    assert_eq!(
        run(r#"
            temporaryImplicitGlobal = 1;
            var before = temporaryImplicitGlobal;
            var deleted = delete temporaryImplicitGlobal;
            before + ":" + deleted + ":" + typeof temporaryImplicitGlobal;
            "#,),
        Value::String(Arc::from("1:true:undefined"))
    );
}

#[test]
fn global_var_declaration_defines_non_configurable_global_property() {
    assert_eq!(
        run(r#"
            var declaredGlobalForDescriptor = 7;
            var d = Object.getOwnPropertyDescriptor(this, "declaredGlobalForDescriptor");
            [
              this.declaredGlobalForDescriptor,
              declaredGlobalForDescriptor,
              d.value,
              d.writable,
              d.enumerable,
              d.configurable,
              delete declaredGlobalForDescriptor,
              delete this.declaredGlobalForDescriptor
            ].join(",");
            "#,),
        Value::String(Arc::from("7,7,7,true,true,false,false,false"))
    );

    assert_eq!(
        run(r#"
            this.hoistedGlobalVar = "balloon";
            var hoistedGlobalVar;
            hoistedGlobalVar;
            "#,),
        Value::String(Arc::from("balloon"))
    );
}

#[test]
fn strict_script_this_is_global_and_read_only_globals_reject_assignment() {
    assert_eq!(
        run(r#""use strict"; this === globalThis;"#),
        Value::Bool(true)
    );

    let err = run_err(r#""use strict"; var global = this; global.Infinity = 42;"#);
    assert!(err.contains("TypeError"), "got: {}", err);

    let err = run_err(r#""use strict"; var global = this; global.undefined = 42;"#);
    assert!(err.contains("TypeError"), "got: {}", err);

    assert_eq!(
        run(r#"var NaN = 42; Number.isNaN(NaN);"#),
        Value::Bool(true)
    );

    let err = run_err(r#""use strict"; var NaN = 42;"#);
    assert!(err.contains("TypeError"), "got: {}", err);

    assert_eq!(
        run(r#"
            var d = Object.getOwnPropertyDescriptor(this, "undefined");
            [d.value === undefined, d.writable, d.enumerable, d.configurable].join(",");
            "#,),
        Value::String(Arc::from("true,false,false,false"))
    );
}

#[test]
fn delete_non_reference_evaluates_operand_and_returns_true() {
    assert_eq!(
        run(r#"
            var called = false;
            function f() { called = true; }
            var d = delete f();
            d + ":" + called;
            "#,),
        Value::String(Arc::from("true:true"))
    );
}

#[test]
fn delete_global_builtin_uses_global_property_configurable() {
    assert_eq!(
        run(r#"
            var d = delete JSON;
            d + ":" + typeof JSON;
            "#,),
        Value::String(Arc::from("true:undefined"))
    );
}

#[test]
fn delete_function_parameter_returns_false() {
    assert_eq!(
        run(r#"
            function f(a) {
              return (delete a) + ":" + a;
            }
            f(1);
            "#,),
        Value::String(Arc::from("false:1"))
    );
}

#[test]
fn delete_super_property_throws_reference_error_before_topropertykey() {
    let err = run_err(
        r#"
        var key = { toString: function() { throw new Error("key coerced"); } };
        var obj = {
          m() {
            delete super[key];
          }
        };
        obj.m();
        "#,
    );
    assert!(err.contains("ReferenceError"), "got: {}", err);
    assert!(!err.contains("key coerced"), "got: {}", err);
}

#[test]
fn delete_super_property_checks_this_before_key_expression() {
    let err = run_err(
        r#"
        class Base {
          constructor() { throw new Error("base constructor called"); }
        }
        class Derived extends Base {
          constructor() {
            delete super[(super(), 0)];
          }
        }
        new Derived();
        "#,
    );
    assert!(err.contains("ReferenceError"), "got: {}", err);
    assert!(!err.contains("base constructor called"), "got: {}", err);
}

#[test]
fn delete_nullish_computed_property_skips_key_coercion() {
    assert_eq!(
        run(r#"
            var log = [];
            var base = null;
            var prop = {
              toString: function() {
                log.push("key");
                return "x";
              }
            };
            try {
              delete base[(log.push("prop"), prop)];
            } catch (e) {
              log.push(e.name);
            }
            log.join("|");
            "#),
        Value::String(Arc::from("prop|TypeError"))
    );
}

#[test]
fn update_identifier_preserves_with_reference_after_getter_delete() {
    assert_eq!(
        run(r#"
            var x = 0;
            var scope = {
              get x() {
                delete this.x;
                return 2;
              }
            };
            with (scope) {
              ++x;
            }
            scope.x + ":" + x;
            "#,),
        Value::String(Arc::from("3:0"))
    );
}

#[test]
fn update_member_evaluates_computed_key_once() {
    assert_eq!(
        run(r#"
            var calls = 0;
            var base = {};
            var prop = {
              toString: function() {
                calls = calls + 1;
                return "k";
              }
            };
            ++base[prop];
            calls + ":" + base.k;
            "#,),
        Value::String(Arc::from("1:NaN"))
    );
}

#[test]
fn update_member_preserves_symbol_computed_key() {
    assert_eq!(
        run(r#"
            var s = Symbol("update");
            var base = {};
            base[s] = 1;
            var pre = ++base[s];
            var post = base[s]++;
            [pre, post, base[s], Object.getOwnPropertySymbols(base).length].join(":");
            "#),
        Value::String(Arc::from("2:2:3:1"))
    );
}

#[test]
fn update_member_uses_property_reference() {
    assert_eq!(
        run(r#"
            var log = [];
            var toPrimitiveSym = Symbol("toPrimitive");
            var toPrimitiveKey = {};
            Object.defineProperty(toPrimitiveKey, Symbol.toPrimitive, {
              value: function() {
                log.push("toPrimitiveKey");
                return toPrimitiveSym;
              }
            });
            var key = {
              toString: function() {
                log.push("key");
                return "x";
              }
            };
            var target = { x: 1 };
            var proxy = new Proxy(target, {
              get: function(t, k, r) {
                log.push("get:" + (r === proxy));
                return Reflect.get(t, k, r);
              },
              set: function(t, k, v, r) {
                log.push("set:" + v + ":" + (r === proxy));
                return Reflect.set(t, k, v, r);
              }
            });
            var post = proxy[key]++;
            var pre = ++proxy[key];
            post + ";" + pre + ";" + target.x + ";" + log.join("|");
            "#),
        Value::String(Arc::from(
            "1;3;3;key|get:true|set:2:true|key|get:true|set:3:true"
        ))
    );

    assert_eq!(
        run(r#"
            var o = {};
            Object.defineProperty(o, "x", { value: 1, writable: false });
            var sloppy = o.x++;
            var strict;
            try {
              (function() { "use strict"; o.x++; })();
            } catch (e) {
              strict = e.name;
            }
            sloppy + ":" + o.x + ":" + strict;
            "#),
        Value::String(Arc::from("1:1:TypeError"))
    );
}

#[test]
fn update_preserves_bigint_numeric_type() {
    assert_eq!(
        run(r#"
            var x = 0n;
            var post = x++;
            var pre = ++x;
            [typeof post, post, typeof pre, pre, typeof x, x].join(":");
            "#,),
        Value::String(Arc::from("bigint:0:bigint:2:bigint:2"))
    );
}

#[test]
fn strict_parenthesized_eval_arguments_assignment_is_syntax_error() {
    assert!(run_err(r#""use strict"; (eval) = 20;"#).contains("SyntaxError"));
    assert!(run_err(r#""use strict"; (arguments) = 20;"#).contains("SyntaxError"));
}

#[test]
fn strict_destructuring_eval_arguments_assignment_targets_are_syntax_errors() {
    for src in [
        r#""use strict"; 0, [arguments] = [];"#,
        r#""use strict"; 0, { eval } = {};"#,
        r#""use strict"; 0, [{ x: arguments }] = [{}];"#,
        r#""use strict"; for ([arguments] in [[]]) ;"#,
        r#""use strict"; for ({ eval } of [{}]) ;"#,
    ] {
        assert!(run_err(src).contains("SyntaxError"), "{src}");
    }
}

#[test]
fn assign_element() {
    assert_eq!(run("var a = [0,0,0]; a[1] = 9; a[1];"), Value::Number(9.0));
}

#[test]
fn assign_nullish_computed_property_evaluates_rhs_before_type_error_but_skips_key_coercion() {
    assert_eq!(
        run(r#"
            var log = [];
            var base = null;
            var prop = {
              toString: function() {
                log.push("key");
                return "x";
              }
            };
            try {
              base[(log.push("prop"), prop)] = (log.push("rhs"), 1);
            } catch (e) {
              log.push(e.name);
            }
            log.join("|");
            "#),
        Value::String(Arc::from("prop|rhs|TypeError"))
    );
}

#[test]
fn assign_member_uses_property_reference_for_set() {
    assert_eq!(
        run(r#"
            var log = [];
            var toPrimitiveSym = Symbol("toPrimitive");
            var toPrimitiveKey = {};
            Object.defineProperty(toPrimitiveKey, Symbol.toPrimitive, {
              value: function() {
                log.push("toPrimitiveKey");
                return toPrimitiveSym;
              }
            });
            var key = {
              toString: function() {
                log.push("key");
                return "x";
              }
            };
            var sym = Symbol("assign");
            var target = {};
            var proxy = new Proxy(target, {
              set: function(t, k, v, r) {
                var label = k === sym ? "sym" : (k === toPrimitiveSym ? "toPrimitiveSym" : k);
                log.push("set:" + label + ":" + v + ":" + (r === proxy));
                t[k] = v;
                return true;
              }
            });
            var result = proxy[(log.push("prop"), key)] = (log.push("rhs"), 2);
            var symResult = proxy[sym] = (log.push("symrhs"), 8);
            var primitiveSymResult = proxy[toPrimitiveKey] = (log.push("primitiveSymRhs"), 9);
            [result, symResult, primitiveSymResult, target.x, target[sym], target[toPrimitiveSym], log.join("|")].join(";");
            "#),
        Value::String(Arc::from(
            "2;8;9;2;8;9;prop|rhs|key|set:x:2:true|symrhs|set:sym:8:true|primitiveSymRhs|toPrimitiveKey|set:toPrimitiveSym:9:true"
        ))
    );

    assert_eq!(
        run(r#"
            var o = {};
            Object.defineProperty(o, "x", { value: 1, writable: false });
            var sloppy = (o.x = 2);
            var primitive = ("abc".x = 4);
            var strict;
            try {
              (function() { "use strict"; o.x = 3; })();
            } catch (e) {
              strict = e.name;
            }
            sloppy + ":" + primitive + ":" + o.x + ":" + strict;
            "#),
        Value::String(Arc::from("2:4:1:TypeError"))
    );
}

// --- compound assignment (numeric/bitwise) ---

#[test]
fn compound_ident() {
    assert_eq!(run("var a = 1; a += 5; a;"), Value::Number(6.0));
    assert_eq!(run("var a = 10; a -= 3; a;"), Value::Number(7.0));
    assert_eq!(run("var a = 4; a *= 3; a;"), Value::Number(12.0));
    assert_eq!(run("var a = 20; a /= 4; a;"), Value::Number(5.0));
    assert_eq!(run("var a = 17; a %= 5; a;"), Value::Number(2.0));
}

#[test]
fn compound_member() {
    assert_eq!(run("var o = {n: 3}; o.n += 5; o.n;"), Value::Number(8.0));
    assert_eq!(run("var o = {n: 10}; o.n -= 4; o.n;"), Value::Number(6.0));
    assert_eq!(run("var o = {n: 2}; o.n *= 5; o.n;"), Value::Number(10.0));
    assert_eq!(run("var o = {n: 20}; o.n /= 4; o.n;"), Value::Number(5.0));
}

#[test]
fn compound_element() {
    assert_eq!(
        run("var a = [10,20,30]; a[1] += 5; a[1];"),
        Value::Number(25.0)
    );
}

#[test]
fn compound_member_preserves_symbol_computed_key() {
    assert_eq!(
        run(r#"
            var s = Symbol("compound");
            var base = {};
            base[s] = 7;
            base[s] += 2;
            base[s] *= 3;
            [base[s], Object.getOwnPropertySymbols(base).length].join(":");
            "#),
        Value::String(Arc::from("27:1"))
    );
}

#[test]
fn compound_member_uses_property_reference() {
    assert_eq!(
        run(r#"
            var log = [];
            var key = {
              toString: function() {
                log.push("key");
                return "x";
              }
            };
            var target = { x: 1 };
            var proxy = new Proxy(target, {
              get: function(t, k, r) {
                log.push("get:" + (r === proxy));
                return Reflect.get(t, k, r);
              },
              set: function(t, k, v, r) {
                log.push("set:" + v + ":" + (r === proxy));
                return Reflect.set(t, k, v, r);
              }
            });
            var result = proxy[key] += (log.push("rhs"), 2);
            result + ";" + target.x + ";" + log.join("|");
            "#),
        Value::String(Arc::from("3;3;key|get:true|rhs|set:3:true"))
    );

    assert_eq!(
        run(r#"
            var o = {};
            Object.defineProperty(o, "x", { value: 1, writable: false });
            var sloppy = o.x += 1;
            var strict;
            try {
              (function() { "use strict"; o.x += 1; })();
            } catch (e) {
              strict = e.name;
            }
            sloppy + ":" + o.x + ":" + strict;
            "#),
        Value::String(Arc::from("2:1:TypeError"))
    );
}

#[test]
fn compound_ident_preserves_resolved_binding_across_eval() {
    assert_eq!(
        run("function f(){ var x=3; var inner=(function(){ x *= (eval('var x=2;'), 4); return x; })(); return inner + ':' + x; } f();"),
        Value::String(Arc::from("2:12"))
    );
    assert_eq!(
        run("function f(){ var x=5; var inner=(function(){ x <<= (eval('var x=2;'), 1); return x; })(); return inner + ':' + x; } f();"),
        Value::String(Arc::from("2:10"))
    );
}

#[test]
fn compound_ident_strict_object_environment_missing_after_get_throws() {
    let global_err = run_err(
        r#"
        Object.defineProperty(this, 'x', {
          configurable: true,
          get: function() { delete this.x; return 2; }
        });
        (function() { 'use strict'; x ^= 3; })();
        "#,
    );
    assert!(global_err.contains("ReferenceError"));

    let with_err = run_err(
        r#"
        var scope = {
          get x() { delete this.x; return 2; }
        };
        with (scope) {
          (function() { 'use strict'; x ^= 3; })();
        }
        "#,
    );
    assert!(with_err.contains("ReferenceError"));
}

#[test]
fn compound_ident_sloppy_object_environment_recreates_deleted_property() {
    assert_eq!(
        run(r#"
            var scope = {
              get x() { delete this.x; return 2; }
            };
            with (scope) {
              x += 3;
            }
            scope.hasOwnProperty('x') && scope.x === 5;
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            Object.defineProperty(this, 'x', {
              configurable: true,
              get: function() { delete this.x; return 2; }
            });
            x += 3;
            this.hasOwnProperty('x') && this.x === 5;
            "#),
        Value::Bool(true)
    );
}

// --- logical assignment ---

#[test]
fn nullish_assign_ident() {
    assert_eq!(run("var a = null; a ??= 5; a;"), Value::Number(5.0));
    assert_eq!(run("var a = 1; a ??= 99; a;"), Value::Number(1.0));
    assert_eq!(run("var a = 0; a ??= 9; a;"), Value::Number(0.0));
}

#[test]
fn logical_assign_ident_preserves_resolved_reference_across_rhs() {
    assert_eq!(
        run(r#"
            var x = 0;
            var result;
            var scope = { x: 0 };
            with (scope) {
              result = (x ||= (delete scope.x, 3));
            }
            result + ":" + scope.hasOwnProperty("x") + ":" + scope.x + ":" + x;
            "#),
        Value::String(Arc::from("3:true:3:0"))
    );

    assert_eq!(
        run(r#"
            var x = 1;
            var result;
            var scope = { x: 1 };
            with (scope) {
              result = (x &&= (delete scope.x, 4));
            }
            result + ":" + scope.hasOwnProperty("x") + ":" + scope.x + ":" + x;
            "#),
        Value::String(Arc::from("4:true:4:1"))
    );

    assert_eq!(
        run(r#"
            var x = 9;
            var result;
            var scope = { x: null };
            with (scope) {
              result = (x ??= (delete scope.x, 5));
            }
            result + ":" + scope.hasOwnProperty("x") + ":" + scope.x + ":" + x;
            "#),
        Value::String(Arc::from("5:true:5:9"))
    );

    assert_eq!(
        run(r#"
            var x = 0;
            var result;
            var scope = { x: 1 };
            with (scope) {
              result = (x ||= (delete scope.x, 3));
            }
            result + ":" + scope.hasOwnProperty("x") + ":" + scope.x + ":" + x;
            "#),
        Value::String(Arc::from("1:true:1:0"))
    );
}

#[test]
fn nullish_assign_member() {
    assert_eq!(
        run("var p = {n: null}; p.n ??= 10; p.n;"),
        Value::Number(10.0)
    );
    assert_eq!(run("var q = {n: 1}; q.n ??= 99; q.n;"), Value::Number(1.0));
}

#[test]
fn logical_assign_member_short_circuit_keeps_expression_result() {
    assert_eq!(
        run("var o = { a: 1 }; var y = (o.a ||= 2); y + ':' + o.a;"),
        Value::String(Arc::from("1:1"))
    );
    assert_eq!(
        run("var o = { a: 0 }; var y = (o.a &&= 2); y + ':' + o.a;"),
        Value::String(Arc::from("0:0"))
    );
    assert_eq!(
        run("var o = { a: 1 }; var y = (o.a ??= 2); y + ':' + o.a;"),
        Value::String(Arc::from("1:1"))
    );
}

#[test]
fn logical_assign_member_preserves_symbol_computed_key() {
    assert_eq!(
        run(r#"
            var a = Symbol("or");
            var b = Symbol("and");
            var c = Symbol("nullish");
            var base = {};
            base[a] ||= 5;
            base[b] = 1;
            base[b] &&= 6;
            base[c] ??= 7;
            [base[a], base[b], base[c], Object.getOwnPropertySymbols(base).length].join(":");
            "#),
        Value::String(Arc::from("5:6:7:3"))
    );
}

#[test]
fn logical_assign_member_uses_property_reference() {
    assert_eq!(
        run(r#"
            var log = [];
            function key(name) {
              return {
                toString: function() {
                  log.push("key:" + name);
                  return name;
                }
              };
            }
            var sym = Symbol("sym");
            var target = { or: 0, and: 1, nil: null, skip: 1 };
            target[sym] = 0;
            var proxy = new Proxy(target, {
              get: function(t, k, r) {
                log.push("get:" + (k === sym ? "sym" : k) + ":" + (r === proxy));
                return Reflect.get(t, k, r);
              },
              set: function(t, k, v, r) {
                log.push("set:" + (k === sym ? "sym" : k) + ":" + v + ":" + (r === proxy));
                t[k] = v;
                return true;
              }
            });
            var orValue = (proxy[key("or")] ||= 2);
            var andValue = (proxy[key("and")] &&= 4);
            var nullishValue = (proxy[key("nil")] ??= 5);
            var skipValue = (proxy[key("skip")] ||= 9);
            var symValue = (proxy[sym] ||= 8);
            [orValue, andValue, nullishValue, skipValue, symValue, target.or, target.and, target.nil, target.skip, target[sym], log.join("|")].join(";");
            "#),
        Value::String(Arc::from(
            "2;4;5;1;8;2;4;5;1;8;key:or|get:or:true|set:or:2:true|key:and|get:and:true|set:and:4:true|key:nil|get:nil:true|set:nil:5:true|key:skip|get:skip:true|get:sym:true|set:sym:8:true"
        ))
    );

    assert_eq!(
        run(r#"
            var o = {};
            Object.defineProperty(o, "x", { value: 0, writable: false });
            var sloppy = (o.x ||= 2);
            var strict;
            try {
              (function() { "use strict"; o.x ||= 3; })();
            } catch (e) {
              strict = e.name;
            }
            sloppy + ":" + o.x + ":" + strict;
            "#),
        Value::String(Arc::from("2:0:TypeError"))
    );
}

#[test]
fn logical_assign_member_null_base_precedes_property_key_coercion() {
    for op in ["&&=", "||=", "??="] {
        let err = run_err(&format!(
            r#"
            var base = null;
            var prop = {{
              toString: function() {{
                throw new Error("property key evaluated");
              }}
            }};
            var rhs = function() {{
              throw new Error("right-hand side evaluated");
            }};
            base[prop] {} rhs();
            "#,
            op
        ));
        assert!(err.contains("TypeError"), "operator {op} got {err}");
        assert!(
            !err.contains("property key evaluated"),
            "operator {op} coerced property key before null-base check: {err}"
        );
        assert!(
            !err.contains("right-hand side evaluated"),
            "operator {op} evaluated RHS before null-base check: {err}"
        );
    }
}

#[test]
fn logical_assign_identifier_infers_anonymous_function_names() {
    assert_eq!(
        run("var value = 1; value &&= function() {}; value.name;"),
        Value::String(Arc::from("value"))
    );
    assert_eq!(
        run("var value = 0; value ||= () => {}; value.name;"),
        Value::String(Arc::from("value"))
    );
    assert_eq!(
        run("var value; value ??= class {}; value.name;"),
        Value::String(Arc::from("value"))
    );
    assert_eq!(
        run("var value = 0; (value) ||= function() {}; value.name;"),
        Value::String(Arc::from(""))
    );
}

#[test]
fn nullish_assign_element() {
    assert_eq!(
        run("var a = [null, 1, 0]; a[0] ??= 5; a[2] ??= 9; a[0];"),
        Value::Number(5.0)
    );
}

#[test]
fn and_assign_ident() {
    assert_eq!(run("var a = 0; a &&= 2; a;"), Value::Number(0.0));
    assert_eq!(run("var a = 5; a &&= a + 1; a;"), Value::Number(6.0));
}

#[test]
fn and_assign_member() {
    assert_eq!(run("var r = {n: 0}; r.n &&= 2; r.n;"), Value::Number(0.0));
}

#[test]
fn or_assign_ident() {
    assert_eq!(run("var a = 0; a ||= 2; a;"), Value::Number(2.0));
    assert_eq!(run("var a = 1; a ||= 99; a;"), Value::Number(1.0));
}

#[test]
fn or_assign_member() {
    assert_eq!(run("var s = {n: 0}; s.n ||= 9; s.n;"), Value::Number(9.0));
}

#[test]
fn or_assign_element() {
    assert_eq!(
        run("var a = [0, 1]; a[0] ||= 99; a[1] ||= 99; a[0];"),
        Value::Number(99.0)
    );
}

// --- optional chaining (?.) ---

#[test]
fn optional_member_present() {
    assert_eq!(
        run("var o = {a:{b:{c:42}}}; o?.a?.b?.c;"),
        Value::Number(42.0)
    );
    assert_eq!(run("var o = {x: 7}; o?.x;"), Value::Number(7.0));
}

#[test]
fn optional_member_null() {
    assert_eq!(run("null?.foo;"), Value::Undefined);
    assert_eq!(run("undefined?.foo;"), Value::Undefined);
}

#[test]
fn optional_member_missing() {
    assert_eq!(run("var o = {a:1}; o?.b?.c;"), Value::Undefined);
    assert_eq!(run("var o = {a:{b:1}}; o?.a?.b?.c;"), Value::Undefined);
}

#[test]
fn optional_computed() {
    assert_eq!(
        run("var o = {a:{b:5}}; o?.[\"a\"]?.[\"b\"];"),
        Value::Number(5.0)
    );
    assert_eq!(run("var o = {a:1}; o?.[\"x\"]?.[\"y\"];"), Value::Undefined);
}

#[test]
fn optional_method_call() {
    assert_eq!(
        run("var o = {greet: function(){return 'hi';}}; o?.greet();"),
        Value::String(Arc::from("hi"))
    );
}

#[test]
fn optional_method_on_null() {
    // null?.greet() short-circuits the whole chain to undefined.
    assert_eq!(run("null?.greet();"), Value::Undefined);
}

#[test]
fn optional_call_null() {
    assert_eq!(run("var f = null; f?.();"), Value::Undefined);
}

#[test]
fn optional_call_present() {
    assert_eq!(
        run("var g = function(){return 99;}; g?.();"),
        Value::Number(99.0)
    );
}

#[test]
fn optional_chain_deep() {
    assert_eq!(
        run("var d = {a:{b:{c:{d:5}}}}; d?.a?.b?.c?.d;"),
        Value::Number(5.0)
    );
    assert_eq!(
        run("var d = {a:{b:{c:{d:5}}}}; d?.a?.x?.y?.z;"),
        Value::Undefined
    );
}

// --- Number toString (exponential notation) ---

#[test]
fn number_to_string_large() {
    assert_eq!(run("1e21 + '';"), Value::String(Arc::from("1e+21")));
    assert_eq!(run("1e22 + '';"), Value::String(Arc::from("1e+22")));
}

#[test]
fn number_to_string_small() {
    assert_eq!(run("1e-7 + '';"), Value::String(Arc::from("1e-7")));
    assert_eq!(run("0.0000001 + '';"), Value::String(Arc::from("1e-7")));
    assert_eq!(run("5e-8 + '';"), Value::String(Arc::from("5e-8")));
}

#[test]
fn number_to_string_normal() {
    assert_eq!(run("(1.5e3) + '';"), Value::String(Arc::from("1500")));
    assert_eq!(run("42 + '';"), Value::String(Arc::from("42")));
    assert_eq!(run("0 + '';"), Value::String(Arc::from("0")));
    assert_eq!(run("3.14 + '';"), Value::String(Arc::from("3.14")));
}

// --- deep optional method chains ---

#[test]
fn optional_method_chain_missing() {
    assert_eq!(
        run("var o = {g: function(){return 1;}}; o?.missing?.();"),
        Value::Undefined
    );
}

#[test]
fn optional_method_chain_null_root() {
    assert_eq!(run("null?.missing?.();"), Value::Undefined);
}

#[test]
fn optional_method_chain_present() {
    assert_eq!(
        run("var o = {greet: function(){return 'hi';}}; o?.greet?.();"),
        Value::String(Arc::from("hi"))
    );
}

#[test]
fn optional_method_call_skips_arguments_when_method_nullish() {
    assert_eq!(
        run("var called = false; var o = {m: null}; o.m?.(called = true); called;"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var called = false; var o = {}; o.m?.(called = true); called;"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var called = false; var o = {m: null}; o.m?.(...(called = true, [])); called;"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var called = false; var o = null; o?.m?.(called = true); called;"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var log = []; var o = { get m(){ log.push('get'); return null; } }; o.m?.(log.push('arg')); log.join(',');"),
        Value::String(Arc::from("get"))
    );
    assert_eq!(
        run("var called = false; var o = {m: 1}; var ok = false; try { o.m?.(called = true); } catch (e) { ok = e instanceof TypeError; } called && ok;"),
        Value::Bool(true)
    );
}

#[test]
fn optional_method_call_preserves_receiver_when_present() {
    assert_eq!(
        run("var o = {x: 3, m: function(a){ return this.x + a; }}; o.m?.(4);"),
        Value::Number(7.0)
    );
    assert_eq!(
        run("var o = {x: 3, m: function(a){ return this.x + a; }}; o.m?.(...[4]);"),
        Value::Number(7.0)
    );
}

// --- ToInt32 / ToUint32 conformance (Rust `as i32` saturates; spec needs modular reduction) ---

#[test]
fn toint32_large_values_wrap() {
    // 2**31 is exactly -2147483648 as int32, not saturated to INT32_MAX.
    assert_eq!(run("(2**31) | 0"), Value::Number(-2147483648.0));
    // 2**32 wraps to 0.
    assert_eq!(run("(2**32) | 0"), Value::Number(0.0));
    assert_eq!(run("(2**33) | 0"), Value::Number(0.0));
    // 2**32 - 1 wraps to -1.
    assert_eq!(run("4294967295 | 0"), Value::Number(-1.0));
}

#[test]
fn touint32_negatives() {
    assert_eq!(run("-1 >>> 0"), Value::Number(4294967295.0));
    assert_eq!(run("-5 >>> 0"), Value::Number(4294967291.0));
}

#[test]
fn bitwise_normal_unchanged() {
    assert_eq!(run("~5"), Value::Number(-6.0));
    assert_eq!(run("5 | 2"), Value::Number(7.0));
    assert_eq!(run("1 << 31"), Value::Number(-2147483648.0));
    assert_eq!(run("5 & 3"), Value::Number(1.0));
    assert_eq!(run("5 ^ 1"), Value::Number(4.0));
}

// --- prototype cycle DoS guard ---

#[test]
fn proto_cycle_strict_throws() {
    // Setting __proto__ to create a cycle must throw in strict mode
    // (was a stack-overflow crash before the fix).
    let res = common::run_err("\"use strict\"; var a={}; var b=Object.create(a); a.__proto__=b;");
    assert!(
        res.contains("Cannot mutate object prototype"),
        "expected TypeError, got: {}",
        res
    );
}

#[test]
fn proto_cycle_sloppy_throws_and_is_safe() {
    let res = common::run_err("var a={}; var b=Object.create(a); a.__proto__=b;");
    assert!(
        res.contains("Cannot mutate object prototype"),
        "expected TypeError, got: {}",
        res
    );
}

#[test]
fn normal_proto_set_still_works() {
    let v = run("var a={}; a.__proto__={x:1}; a.x");
    assert_eq!(v, Value::Number(1.0));
}

#[test]
fn non_extensible_object_proto_set_is_rejected() {
    assert_eq!(
        run("var a = Object.preventExtensions({}); var p = {}; try { a.__proto__ = p; } catch (e) {} Object.getPrototypeOf(a) === Object.prototype;"),
        Value::Bool(true)
    );
    assert!(common::run_err(
        "\"use strict\"; var a = Object.preventExtensions({}); a.__proto__ = {};"
    )
    .contains("Cannot mutate object prototype"));
}

// --- Object.defineProperty descriptor validation ---

#[test]
fn define_property_non_object_descriptor_throws() {
    let res = common::run_err("Object.defineProperty({}, 'x', true);");
    assert!(
        res.contains("must be an object"),
        "expected TypeError, got: {}",
        res
    );
    // Non-object primitives too.
    let res = common::run_err("Object.defineProperty({}, 'x', 42);");
    assert!(res.contains("must be an object"), "got: {}", res);
}

#[test]
fn define_property_object_descriptor_works() {
    let v = run("var o={}; Object.defineProperty(o,'x',{value:7,writable:true}); o.x");
    assert_eq!(v, Value::Number(7.0));
}

// --- Array.prototype.sort DoS guard (was O(n^2)) ---

/// Sorting with a comparator must be O(n log n), not O(n^2). Before the fix,
/// sorting ~1000 elements called the comparator ~250k times (quadratic).
/// Here we assert the comparison count stays well under the quadratic bound.
#[test]
fn sort_comparator_is_not_quadratic() {
    use std::thread;
    let src = r#"
        var a = [];
        for (var i = 0; i < 1000; i++) a.push(Math.random());
        var c = 0;
        a.sort(function (x, y) { c++; return x - y; });
        c
    "#;
    let src = src.to_string();
    let worker = thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let mut vm = ruja::Vm::new().expect("failed to initialize VM");
            match vm.run(&src) {
                Ok(ruja::Value::Number(n)) => n,
                Ok(v) => panic!("expected number, got {:?}", v),
                Err(e) => panic!("evaluation errored: {}", e),
            }
        })
        .expect("failed to spawn worker");
    let count = worker.join().expect("worker panicked");
    // O(n^2) would be ~500k; O(n log n) is ~10k. Allow generous slack.
    assert!(
        count < 30_000.0,
        "sort called comparator {} times (expected O(n log n), got O(n^2))",
        count
    );
}

#[test]
fn sort_comparator_correctness() {
    let v = run("var a=[3,1,4,1,5,9,2,6]; a.sort(function(x,y){return x-y}); a.join(',')");
    assert_eq!(v, Value::String(std::sync::Arc::from("1,1,2,3,4,5,6,9")));
}

#[test]
fn sort_throwing_comparator_propagates() {
    let res = common::run_err("[3,1,2].sort(function(){ throw new Error('boom'); });");
    assert!(res.contains("boom"), "got: {}", res);
}

#[test]
fn sort_nan_comparator_keeps_order() {
    // NaN comparator result is treated as 0 (equal): elements stay put.
    let v = run("var a=[3,1,2]; a.sort(function(){return NaN}); a.join(',')");
    assert_eq!(v, Value::String(std::sync::Arc::from("3,1,2")));
}

#[test]
fn sort_default_is_string_compare() {
    let v = run("var a=[10,2,1,30]; a.sort(); a.join(',')");
    assert_eq!(v, Value::String(std::sync::Arc::from("1,10,2,30")));
}

// --- Date TimeValue range (Invalid Date) ---

#[test]
fn date_out_of_range_is_invalid() {
    // ES TimeValue must be within +/-8.64e15 ms; beyond is an Invalid Date.
    let v = run("new Date(1e20).getTime()");
    assert!(matches!(v, Value::Number(n) if n.is_nan()), "got {:?}", v);
    let v = run("new Date(8.64e15 + 1).getTime()");
    assert!(matches!(v, Value::Number(n) if n.is_nan()), "got {:?}", v);
    // Infinity is also invalid.
    let v = run("new Date(Infinity).getTime()");
    assert!(matches!(v, Value::Number(n) if n.is_nan()), "got {:?}", v);
}

#[test]
fn date_in_range_works() {
    let v = run("new Date(0).getTime()");
    assert_eq!(v, Value::Number(0.0));
    let v = run("Number.isFinite(new Date().getTime())");
    assert_eq!(v, Value::Bool(true));
}

// --- Number.prototype.toString(radix) fractional conversion ---

#[test]
fn to_string_radix_fractional() {
    let v = run("(1.5).toString(2)");
    assert_eq!(v, Value::String(std::sync::Arc::from("1.1")));
    let v = run("(255.5).toString(16)");
    assert_eq!(v, Value::String(std::sync::Arc::from("ff.8")));
    let v = run("(-1.5).toString(2)");
    assert_eq!(v, Value::String(std::sync::Arc::from("-1.1")));
}

#[test]
fn to_string_radix_integer_unchanged() {
    let v = run("(255).toString(16)");
    assert_eq!(v, Value::String(std::sync::Arc::from("ff")));
    let v = run("(0).toString(2)");
    assert_eq!(v, Value::String(std::sync::Arc::from("0")));
}

#[test]
fn to_string_radix_invalid_throws() {
    let res = common::run_err("(5).toString(1)");
    assert!(res.contains("between 2 and 36"), "got: {}", res);
    let res = common::run_err("(5).toString(37)");
    assert!(res.contains("between 2 and 36"), "got: {}", res);
}

#[test]
fn to_string_radix_undefined_and_abrupt_completion() {
    let v = run("(5).toString(undefined)");
    assert_eq!(v, Value::String(std::sync::Arc::from("5")));
    let res = common::run_err("(5).toString({ valueOf(){ throw new Error('radix'); } })");
    assert!(res.contains("radix"), "got: {}", res);
    let res = common::run_err(
        "Number.prototype.toString.call({}, { valueOf(){ throw new Error('radix'); } })",
    );
    assert!(res.contains("TypeError"), "got: {}", res);
}

#[test]
fn unary_minus_uses_to_numeric_for_bigint_objects() {
    assert_eq!(
        run("(-Object(1n)).toString();"),
        Value::String(Arc::from("-1"))
    );
    assert_eq!(
        run(r#"(-{ [Symbol.toPrimitive]: function() { return 2n; } }).toString();"#),
        Value::String(Arc::from("-2"))
    );
}
