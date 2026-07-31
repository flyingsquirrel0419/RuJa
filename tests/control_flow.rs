//! Control flow, operators, and recently-fixed correctness bugs:
//! break/continue, switch, finally, hoisting, increment/decrement, typeof,
//! unary +, in/instanceof/delete, comparisons, loose equality.

mod common;
use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

// --- break / continue ---

#[test]
fn for_break() {
    assert_eq!(
        run("let s=0; for(let i=0;i<10;i++){ if(i==3) break; s+=i; } s;"),
        Value::Number(3.0)
    );
}

#[test]
fn for_continue() {
    assert_eq!(
        run("let s=0; for(let i=0;i<5;i++){ if(i==2) continue; s+=i; } s;"),
        Value::Number(8.0)
    );
}

#[test]
fn while_break() {
    assert_eq!(
        run("let i=0,s=0; while(i<10){ i++; if(i==4) break; s+=i; } s;"),
        Value::Number(6.0)
    );
}

#[test]
fn break_in_for_var() {
    assert_eq!(
        run("var s=0;for(var i=0;i<10;i++){if(i>=3)break;s+=i}s;"),
        Value::Number(3.0)
    );
}

#[test]
fn continue_in_for_var() {
    assert_eq!(
        run("var s=0;for(var i=0;i<5;i++){if(i==2)continue;s+=i}s;"),
        Value::Number(8.0)
    );
}

#[test]
fn nested_break() {
    assert_eq!(
        run("var s=0;for(var i=0;i<3;i++){for(var j=0;j<3;j++){if(j==1)break;s++}}s;"),
        Value::Number(3.0)
    );
}

#[test]
fn nested_call_argument_for_in_does_not_corrupt_caller_stack() {
    assert_eq!(
        run("var out=[]; function f(obj){ for (var x in obj) {} return 7; } out.push(f({a:1})); out[0];"),
        Value::Number(7.0)
    );
}

#[test]
fn nested_call_argument_for_of_does_not_corrupt_caller_stack() {
    assert_eq!(
        run("var out=[]; function f(){ for (var x of [1]) {} return 9; } out.push(f()); out[0];"),
        Value::Number(9.0)
    );
}

// --- switch ---

#[test]
fn switch_fallthrough() {
    assert_eq!(
        run("let r=''; switch(2){case 1: r+='a'; case 2: r+='b'; case 3: r+='c'; break; default: r+='d';} r;"),
        Value::String(Arc::from("bc"))
    );
}

#[test]
fn switch_default() {
    assert_eq!(
        run("let r=''; switch(99){case 1: r+='a'; default: r+='d'; case 3: r+='c';} r;"),
        Value::String(Arc::from("dc"))
    );
}

#[test]
fn switch_break() {
    assert_eq!(
        run("let r=''; switch(1){case 1: r+='a'; break; case 2: r+='b';} r;"),
        Value::String(Arc::from("a"))
    );
}

#[test]
fn switch_var_uses_enclosing_variable_environment() {
    assert_eq!(
        run("var probeBefore = function(){ return x; };
             switch ((eval('var x = 1;'), null)) {
               case (eval('var x = 2;'), null):
                 var probeStmt = function(){ return x; };
                 var x = 3;
             }
             probeBefore() + probeStmt() + x;"),
        Value::Number(9.0)
    );
}

#[test]
fn switch_function_and_var_redeclaration_is_syntax_error() {
    for src in [
        "switch (0) { case 1: function f() {} default: var f }",
        "switch (0) { case 1: var f; default: function f() {} }",
    ] {
        let msg = run_err(src);
        assert!(
            msg.contains("already been declared"),
            "expected redeclaration error for {src}, got: {msg}"
        );
    }
}

#[test]
fn switch_function_declarations_do_not_leak() {
    for src in [
        "switch (0) { default: function * x() {} } x;",
        "switch (0) { default: async function x() {} } x;",
        "switch (0) { default: async function * x() {} } x;",
    ] {
        let msg = run_err(src);
        assert!(
            msg.contains("x is not defined"),
            "expected ReferenceError for {src}, got: {msg}"
        );
    }
}

#[test]
fn annex_b_duplicate_block_and_switch_functions_use_the_last_declaration() {
    assert_eq!(
        run("(function() { var result; { result = f(); function f() { return 1; } function f() { return 2; } } return result; }());"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("(function() { { function f() { return 1; } function f() { return 2; } } return f(); }());"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("var result; switch (1) { case 1: result = f(); function f() { return 1; } break; default: function f() { return 2; } } result;"),
        Value::Number(2.0)
    );
}

#[test]
fn annex_b_switch_mirrors_only_an_evaluated_function_declaration() {
    assert_eq!(
        run(r#"
            (function(value) {
              var before = typeof f;
              switch (value) {
                case 0: break; function f() { return 0; }
                case 1: function f() { return 1; }
              }
              return before + "," + typeof f + (typeof f === "function" ? "," + f() : "");
            }(0)) + ";" +
            (function(value) {
              switch (value) { case 1: function f() { return 1; } }
              return typeof f + "," + f();
            }(1));
        "#),
        Value::String(Arc::from("undefined,undefined;function,1"))
    );
}

#[test]
fn annex_b_if_functions_use_synthetic_block_semantics() {
    assert_eq!(
        run(r#"
            (function(value) {
              var before = typeof f;
              if (value) function f() { var lexical = f; f = 123; return lexical; }
              return before + "," + typeof f +
                (typeof f === "function" ? "," + f()() : "");
            }(false)) + ";" +
            (function(value) {
              if (value) function f() { return 1; }
              else function f() { return 2; }
              return f();
            }(false));
        "#),
        Value::String(Arc::from("undefined,undefined;2"))
    );

    assert_eq!(
        run(r#"
            (function() {
              if (true) function f() { var lexical = f; f = 123; return lexical; }
              return f() === f;
            }());
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            (function() {
              let f = 7;
              if (true) function f() { return 8; }
              return f;
            }());
        "#),
        Value::Number(7.0)
    );
}

#[test]
fn nested_labels_share_their_iteration_target() {
    assert_eq!(
        run(r#"
            var visited = 0;
            outer: inner: for (var i = 0; i < 3; i++) {
              if (i < 2) continue outer;
              visited++;
            }
            visited;
        "#),
        Value::Number(1.0)
    );
}

#[test]
fn unlabelled_control_skips_non_loop_label_frames() {
    assert_eq!(
        run(r#"
            var iterations = 0, tail = 0;
            while (iterations++ < 2) {
              marker: { break; }
              tail++;
            }
            iterations + "," + tail;
        "#),
        Value::String(Arc::from("1,0"))
    );

    assert_eq!(
        run(r#"
            var tail = 0;
            for (var i = 0; i < 3; i++) {
              marker: { continue; }
              tail++;
            }
            i + "," + tail;
        "#),
        Value::String(Arc::from("3,0"))
    );
}

#[test]
fn labelled_loop_keeps_labels_across_nested_function_compilation() {
    assert_eq!(
        run(r#"
            var count = 0;
            outer: for (
              var factory = function() { while (false) {} }, i = 0;
              i < 3;
              i++
            ) {
              count++;
              continue outer;
            }
            count;
        "#),
        Value::Number(3.0)
    );
}

#[test]
fn annex_b_for_in_initializer_runs_once_before_rhs() {
    assert_eq!(
        run(r#"
            var effects = 0, observed, iterations = 0;
            for (var key = (++effects, -1) in (observed = key, { a: 1, b: 2 })) {
              iterations++;
            }
            effects + "," + observed + "," + iterations + "," + key;
        "#),
        Value::String(Arc::from("1,-1,2,b"))
    );

    assert_eq!(
        run(r#"
            var rhsEffects = 0;
            try {
              for (var key = (function() { throw 7; }()) in (rhsEffects++, {})) {}
            } catch (error) {}
            rhsEffects;
        "#),
        Value::Number(0.0)
    );

    assert_eq!(
        run(r#"
            var initializerEffects = 0, rhsEffects = 0;
            var object = {};
            Object.defineProperty(object, "key", {
              set: function() { throw 7; }
            });
            try {
              with (object) {
                for (var key = (initializerEffects++, 1) in (rhsEffects++, {})) {}
              }
            } catch (error) {}
            initializerEffects + "," + rhsEffects;
        "#),
        Value::String(Arc::from("1,0"))
    );
}

#[test]
fn annex_b_for_in_initializer_uses_binding_reference_and_named_evaluation() {
    assert_eq!(
        run(r#"
            var key = 1;
            var object = { key: 2 };
            with (object) {
              for (
                var key = (object.observed = key, 3)
                in (object.before = object.key, { item: 1 })
              ) {}
            }
            key + "," + object.key + "," + object.observed + "," + object.before;
        "#),
        Value::String(Arc::from("1,item,2,3"))
    );

    assert_eq!(
        run(r#"
            var inferred;
            for (var key = function() {} in (inferred = key.name, {})) {}
            inferred;
        "#),
        Value::String(Arc::from("key"))
    );

    assert_eq!(
        run("for (var key = ('item' in { item: 1 }) in {}) {} key;"),
        Value::Bool(true)
    );
}

#[test]
fn continue_out_of_switch_preserves_completion_value() {
    assert_eq!(
        run("eval('5; do { switch (\"a\") { case \"a\": { 6; continue; } } } while (false)');"),
        Value::Number(6.0)
    );
}

#[test]
fn continue_out_of_nested_switch_preserves_inner_completion() {
    assert_eq!(
        run("eval('1; do { switch (0) { case 0: 2; switch (0) { case 0: 3; continue; } } } while (false)');"),
        Value::Number(3.0)
    );
}

#[test]
fn continue_out_of_switch_runs_finally_before_scope_unwind() {
    assert_eq!(
        run("eval('5; do { switch (0) { case 0: try { 6; continue; } finally {} } } while (false)');"),
        Value::Number(6.0)
    );
    assert_eq!(
        run("var log = ''; do { switch (0) { case 0: let x = 'x'; try { 7; continue; } finally { log += x; } } } while (false); log;"),
        Value::String(Arc::from("x"))
    );
}

#[test]
fn break_out_of_switch_runs_finally_before_scope_unwind() {
    assert_eq!(
        run("var log = ''; switch (0) { case 0: let x = 'x'; try { 8; break; } finally { log += x; } } log;"),
        Value::String(Arc::from("x"))
    );
}

// --- try / catch / finally ---

#[test]
fn finally_executes_after_try() {
    assert_eq!(run("let r=0;try{r=1;}finally{r=2;}r;"), Value::Number(2.0));
}

#[test]
fn finally_executes_after_catch() {
    assert_eq!(
        run("let r=0;try{throw 1;}catch(e){r=1;}finally{r=r+10;}r;"),
        Value::Number(11.0)
    );
}
#[test]
fn break_in_try_runs_outer_finally() {
    let src = "let r=[];for(let i=0;i<5;i++){try{if(i===2)break;r.push(i);}finally{r.push('f'+i);}}r.join(',');";
    assert_eq!(run(src), Value::String(Arc::from("0,f0,1,f1,f2")));
}
#[test]
fn continue_in_try_runs_finally() {
    let src = "let r=[];for(let i=0;i<4;i++){try{if(i===1)continue;r.push(i);}finally{r.push('f'+i);}}r.join(',');";
    assert_eq!(run(src), Value::String(Arc::from("0,f0,f1,2,f2,3,f3")));
}
#[test]
fn nested_finally_break_runs_both() {
    let src = "let r=[];for(let i=0;i<3;i++){try{try{if(i===1)break;r.push('i'+i);}finally{r.push('if'+i);}}finally{r.push('of'+i);}}r.join(',');";
    assert_eq!(run(src), Value::String(Arc::from("i0,if0,of0,if1,of1")));
}
#[test]
fn return_through_nested_finally() {
    let src = "let f=function(){try{try{return 42;}finally{}}finally{}};f();";
    assert_eq!(run(src), Value::Number(42.0));
}
#[test]
fn throw_runs_inner_finally_before_catch() {
    let src = "let r=[];try{try{throw 1;}finally{r.push('if');}}catch(e){r.push('c');}finally{r.push('of');}r.join(',');";
    assert_eq!(run(src), Value::String(Arc::from("if,c,of")));
}

#[test]
fn throw_inside_finally_replaces_pending_throw() {
    assert_eq!(
        run("var out; try { try { throw 'old'; } finally { throw 'new'; } } catch (e) { out = e; } out;"),
        Value::String(Arc::from("new"))
    );
}

#[test]
fn caught_throw_inside_finally_clears_stale_pending_completion() {
    assert_eq!(
        run("var out='', fin=0;\
             try { try { throw 'old'; } finally { throw 'new'; } } catch (e) { out += e; }\
             try { throw 'next'; } catch (e) { out += ':' + e; } finally { fin = 1; }\
             out + ':' + fin;"),
        Value::String(Arc::from("new:next:1"))
    );
}

#[test]
fn return_to_finally_disables_skipped_catch() {
    assert_eq!(
        run("function f() {\
               try { return 'try'; }\
               catch (e) { return 'catch'; }\
               finally { throw 'finally'; }\
             }\
             try { f(); } catch (e) { e; }"),
        Value::String(Arc::from("finally"))
    );
}

#[test]
fn break_to_finally_disables_skipped_catch() {
    assert_eq!(
        run("var out;\
             try {\
               while (true) {\
                 try { break; } catch (e) { out = 'catch'; } finally { throw 'finally'; }\
               }\
             } catch (e) { out = e; }\
             out;"),
        Value::String(Arc::from("finally"))
    );
}

#[test]
fn native_reference_error_runs_inner_finally_before_outer_catch() {
    let src =
        "let r=[];try{try{missing;}finally{r.push('fin');}}catch(e){r.push(e.name);}r.join(',');";
    assert_eq!(run(src), Value::String(Arc::from("fin,ReferenceError")));
}

#[test]
fn native_type_error_runs_inner_finally_before_outer_catch() {
    let src =
        "let r=[];try{try{null.x;}finally{r.push('fin');}}catch(e){r.push(e.name);}r.join(',');";
    assert_eq!(run(src), Value::String(Arc::from("fin,TypeError")));
}

#[test]
fn native_errors_preserve_kind_after_uncaught_finally() {
    let msg = run_err("try { null.x; } finally {}");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("try { missing; } finally {}");
    assert!(msg.contains("ReferenceError"), "got: {msg}");
}

#[test]
fn delete_catch_parameter_returns_false_and_preserves_binding() {
    assert_eq!(
        run("var out; try { throw 'catchme'; } catch (e) { out = (delete e) + ':' + e; } out;"),
        Value::String(Arc::from("false:catchme"))
    );
    assert_eq!(
        run("try { throw 1; } catch (e) {} typeof e;"),
        Value::String(Arc::from("undefined"))
    );
}

// --- operators ---

#[test]
fn typeof_undeclared() {
    assert_eq!(
        run("typeof noSuchVar;"),
        Value::String(Arc::from("undefined"))
    );
}

#[test]
fn unary_plus() {
    assert_eq!(run(r#"+"5";"#), Value::Number(5.0));
    assert_eq!(run("+true;"), Value::Number(1.0));
    assert_eq!(run("+(-5);"), Value::Number(-5.0));
    assert!(matches!(run(r#"+"INFINITY";"#), Value::Number(n) if n.is_nan()));
}

#[test]
fn void_operator() {
    assert_eq!(run("void 5;"), Value::Undefined);
    assert_eq!(run("typeof void 0;"), Value::String(Arc::from("undefined")));
}

#[test]
fn in_operator() {
    assert_eq!(run(r#""a" in {a:1};"#), Value::Bool(true));
    assert_eq!(run(r#""b" in {a:1};"#), Value::Bool(false));
    assert_eq!(run("0 in [1,2];"), Value::Bool(true));
    let err = run_err(r#""toString" in true;"#);
    assert!(err.contains("TypeError"), "got: {err}");
    let err = run_err(
        "var key = { toString: function() { throw new Error('key'); } };
         key in true;",
    );
    assert!(err.contains("TypeError"), "got: {err}");
}

#[test]
fn delete_operator() {
    assert_eq!(run("delete ({a:1}).a;"), Value::Bool(true));
}

#[test]
fn delete_lexical_binding_returns_false() {
    assert_eq!(
        run("let x = 1; (delete x) + ':' + x;"),
        Value::String(Arc::from("false:1"))
    );
}

#[test]
fn instanceof_basic() {
    assert_eq!(run("new Error() instanceof Error;"), Value::Bool(true));
    assert_eq!(
        run("Function.prototype.prototype = true; 0 instanceof Function.prototype;"),
        Value::Bool(false)
    );
    assert_eq!(
        run("Object.defineProperty(Function.prototype, 'prototype', {
               get: function() { throw new Error('getter'); }
             });
             0 instanceof Function.prototype;",),
        Value::Bool(false)
    );
}

#[test]
fn instanceof_uses_symbol_has_instance() {
    assert_eq!(
        run(r#"
            var F = {};
            var seenThis, seenArg, calls = 0;
            F[Symbol.hasInstance] = function(value) {
              seenThis = this;
              seenArg = value;
              calls += 1;
              return "truthy";
            };
            [0 instanceof F, seenThis === F, seenArg, calls].join(":");
            "#,),
        Value::String(Arc::from("true:true:0:1"))
    );
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(Function.prototype, Symbol.hasInstance);
            [
              typeof desc.value,
              desc.value.name,
              desc.value.length,
              desc.writable,
              desc.enumerable,
              desc.configurable,
              Function.prototype[Symbol.hasInstance].call({}, {})
            ].join(":");
            "#,),
        Value::String(Arc::from(
            "function:[Symbol.hasInstance]:1:false:false:false:false",
        ))
    );
}

#[test]
fn instanceof_gets_the_prototype_before_comparing_and_forwards_bound_handlers() {
    assert_eq!(
        run(r#"
            function F() {}
            var selfPrototype = F.prototype instanceof F;

            var sentinel = {};
            var calls = 0;
            var throwingPrototype = new Proxy({}, {
              getPrototypeOf: function () {
                calls += 1;
                throw sentinel;
              }
            });
            F.prototype = throwingPrototype;
            var sameSentinel = false;
            try { throwingPrototype instanceof F; }
            catch (error) { sameSentinel = error === sentinel; }

            var log = [];
            function Target() {}
            Object.defineProperty(Target, Symbol.hasInstance, {
              get: function () {
                log.push("get");
                return function (value) {
                  log.push(this === Target ? "this" : "bad-this");
                  log.push(value.marker);
                  return "truthy";
                };
              }
            });
            var Bound = Target.bind(null);
            var customResult = { marker: "value" } instanceof Bound;

            function WrappedDefault() {}
            var wrappedValue = Object.create(WrappedDefault.prototype);
            var defaultHandler = Function.prototype[Symbol.hasInstance];
            Object.defineProperty(WrappedDefault, Symbol.hasInstance, {
              value: defaultHandler.bind(WrappedDefault)
            });
            var boundDefaultResult = wrappedValue instanceof WrappedDefault;

            function ReboundDefault() {}
            var reboundValue = Object.create(ReboundDefault.prototype);
            function ReboundHost() {}
            Object.defineProperty(ReboundHost, Symbol.hasInstance, {
              value: defaultHandler.bind(ReboundDefault, reboundValue)
            });
            var reboundDefaultResult = ({} instanceof ReboundHost);

            function ProxiedDefault() {}
            var proxyCalls = 0;
            var proxiedValue = Object.create(ProxiedDefault.prototype);
            Object.defineProperty(ProxiedDefault, Symbol.hasInstance, {
              value: new Proxy(defaultHandler, {
                apply: function (target, thisArg, args) {
                  proxyCalls += 1;
                  return Reflect.apply(target, thisArg, args);
                }
              })
            });
            var proxiedDefaultResult = proxiedValue instanceof ProxiedDefault;

            [
              selfPrototype,
              sameSentinel,
              calls,
              customResult,
              log.join(","),
              boundDefaultResult,
              reboundDefaultResult,
              proxiedDefaultResult,
              proxyCalls
            ].join("|");
        "#),
        Value::String(Arc::from(
            "false|true|1|true|get,this,value|true|true|true|1"
        ))
    );
}

#[test]
fn string_gt_comparison() {
    assert_eq!(run(r#""b" > "a";"#), Value::Bool(true));
    assert_eq!(run(r#""ab" >= "a";"#), Value::Bool(true));
    assert_eq!(run(r#""a" > "b";"#), Value::Bool(false));
}

#[test]
fn loose_eq_array_bool() {
    assert_eq!(run("[] == false;"), Value::Bool(true));
}

// --- increment / decrement ---

#[test]
fn increment_postfix() {
    assert_eq!(run("var c=5; c++;"), Value::Number(5.0));
    assert_eq!(run("var c=5; c++; c;"), Value::Number(6.0));
}

#[test]
fn increment_prefix() {
    assert_eq!(run("var c=5; ++c;"), Value::Number(6.0));
    assert_eq!(run("var c=5; ++c; c;"), Value::Number(6.0));
}

#[test]
fn increment_in_expression() {
    assert_eq!(run("var c=0; c++; c++; ++c; c;"), Value::Number(3.0));
}

#[test]
fn decrement() {
    assert_eq!(run("var c=5; c--; c;"), Value::Number(4.0));
    assert_eq!(run("var c=5; --c;"), Value::Number(4.0));
}

#[test]
fn var_hoisting_toplevel() {
    assert_eq!(run("console.log(v); var v=5; v;"), Value::Number(5.0));
}

#[test]
fn var_hoisting_function() {
    // console.log prints "undefined" then returns 5; check the return value.
    assert_eq!(
        run("function f(){ var x=5; return x; } f();"),
        Value::Number(5.0)
    );
}

#[test]
fn var_function_scope() {
    assert_eq!(
        run("function f(){ if(true){ var y=10; } return y; } f();"),
        Value::Number(10.0)
    );
}

#[test]
fn let_block_scope() {
    // inner let shadows outer; outer retains its value.
    let r = run("let r; {let x=1;{let x=2;} r = x;} r;");
    assert_eq!(r, Value::Number(1.0));
}

#[test]
fn const_reassign_throws() {
    // const reassignment should throw a TypeError.
    let msg = run_err("const x=1; x=2; x;");
    assert!(msg.contains("constant"), "got: {}", msg);
}

#[test]
fn const_read_ok() {
    assert_eq!(run("const x=42; x;"), Value::Number(42.0));
}

// --- try/catch routing for runtime errors (not just JS throw) ---

#[test]
fn catch_type_error_null_property() {
    // `null.x` raises a TypeError that must be catchable.
    let r = run("var r; try { null.x; } catch(e) { r = e.message; } r;");
    assert!(matches!(r, Value::String(_)));
    let s = match r {
        Value::String(s) => s.to_string(),
        _ => String::new(),
    };
    assert!(s.contains("Cannot read properties"), "got: {}", s);
}

#[test]
fn catch_undefined_property() {
    let r = run("var r; try { (undefined).foo; } catch(e) { r = 'caught'; } r;");
    assert_eq!(r, Value::String(Arc::from("caught")));
}

#[test]
fn catch_reference_error() {
    let r = run("var r; try { missingVar; } catch(e) { r = 'ref'; } r;");
    assert_eq!(r, Value::String(Arc::from("ref")));
}

#[test]
fn catch_call_non_function() {
    let r = run("var r; try { (5)(); } catch(e) { r = 'call'; } r;");
    assert_eq!(r, Value::String(Arc::from("call")));
}

#[test]
fn catch_rethrow() {
    let r = run("var r; try { try { throw 'inner'; } catch(e) { throw 'rethrow'; } } catch(e) { r = e; } r;");
    assert_eq!(r, Value::String(Arc::from("rethrow")));
}

#[test]
fn catch_native_error_has_name_and_message() {
    let r =
        run("var r; try { null.x; } catch(e) { r = e.name + ':' + (e.message.length > 0); } r;");
    assert_eq!(r, Value::String(Arc::from("TypeError:true")));
}
