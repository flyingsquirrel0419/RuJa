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

#[test]
fn member_read_uses_property_reference() {
    assert_eq!(
        run(r#"
            var log = [];
            var symbol = Symbol("member-read");
            var key = {
              toString: function() {
                log.push("key");
                return "x";
              }
            };
            var target = { x: 3 };
            target[symbol] = 7;
            var proxy = new Proxy(target, {
              get: function(t, k, r) {
                log.push("get:" + (k === symbol ? "symbol" : k) + ":" + (r === proxy));
                return Reflect.get(t, k, r);
              }
            });
            [proxy.x, proxy[key], proxy[symbol], proxy?.x, log.join("|")].join(";");
            "#),
        Value::String(Arc::from(
            "3;3;7;3;get:x:true|key|get:x:true|get:symbol:true|get:x:true"
        ))
    );
}

#[test]
fn member_read_null_base_precedes_property_key_coercion() {
    assert_eq!(
        run(r#"
            var log = [];
            var key = {
              toString: function() {
                log.push("coerce");
                return "x";
              }
            };
            try {
              null[(log.push("evaluate"), key)];
            } catch (e) {
              log.push(e.name);
            }
            null?.[(log.push("optional"), key)];
            log.join("|");
            "#),
        Value::String(Arc::from("evaluate|TypeError"))
    );
}

#[test]
fn member_read_keeps_mapped_arguments_live_after_failed_delete() {
    assert_eq!(
        run(r#"
            function check(a) {
              Object.defineProperty(arguments, "0", { configurable: false });
              var firstDelete = delete arguments[0];
              var strictDeleteThrew = false;
              var args = arguments;
              try {
                (function() { "use strict"; delete args[0]; })();
              } catch (error) {
                strictDeleteThrew = error instanceof TypeError;
              }
              a = 2;
              return [firstDelete, strictDeleteThrew, arguments[0]].join(":");
            }
            check(1);
            "#),
        Value::String(Arc::from("false:true:2"))
    );
}

#[test]
fn non_optional_member_calls_use_property_references() {
    assert_eq!(
        run(r#"
            var log = [];
            var symbol = Symbol("call");
            var method = function(value) {
              "use strict";
              log.push(this === proxy ? "this" : "bad-this");
              return value + 1;
            };
            var target = { run: method };
            target[symbol] = method;
            var proxy = new Proxy(target, {
              get: function(t, key, receiver) {
                log.push("get:" + (key === symbol ? "symbol" : key));
                return Reflect.get(t, key, receiver);
              }
            });
            var key = {
              toString: function() {
                log.push("key");
                return "run";
              }
            };
            var direct = proxy[key](2);
            var spread = proxy[symbol](...[3]);
            [direct, spread, log.join("|")].join(";");
            "#),
        Value::String(Arc::from("3;4;key|get:run|this|get:symbol|this"))
    );

    assert_eq!(
        run(r#"
            String.prototype.capture = function(value) {
              "use strict";
              return typeof this + ":" + this + ":" + value;
            };
            "base".capture(...[7]);
            "#),
        Value::String(Arc::from("string:base:7"))
    );
}

#[test]
fn member_call_reference_roots_temporary_base_during_arguments() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
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
            function makeBase() {
              return {
                marker: 9,
                method: function() { return this.marker; }
              };
            }
            makeBase().method(forceGc());
            "#
        )
        .expect("member call should keep its temporary base alive"),
        Value::Number(9.0)
    );
    assert_eq!(
        vm.run(
            r#"
            function makeOptionalBase() {
              return {
                marker: 11,
                method: function() { return this.marker; }
              };
            }
            makeOptionalBase().method?.(...(forceGc(), []));
            "#
        )
        .expect("optional member call should keep its temporary base alive"),
        Value::Number(11.0)
    );
}

#[test]
fn proxy_get_without_trap_preserves_receiver() {
    assert_eq!(
        run(r#"
            var symbol = Symbol("proxy-get");
            var target = {
              get value() { return this; },
              get [symbol]() { return this; }
            };
            var inner = new Proxy(target, {});
            var outer = new Proxy(inner, {});
            var receiver = {};
            [
              inner.value === inner,
              inner[symbol] === inner,
              outer.value === outer,
              outer[symbol] === outer,
              Reflect.get(outer, "value", receiver) === receiver,
              Reflect.get(outer, symbol, receiver) === receiver
            ].join(":");
            "#),
        Value::String(Arc::from("true:true:true:true:true:true"))
    );
}

#[test]
fn ordinary_property_walks_and_inherited_proxy_traps_have_no_depth_cutoff() {
    assert_eq!(
        run(r#"
            var symbol = Symbol("ordinary-deep-set");
            var readSymbol = Symbol("ordinary-deep-read");
            var ordinaryRoot = { marker: 17 };
            ordinaryRoot[readSymbol] = 13;
            Object.defineProperty(ordinaryRoot, "sink", {
              set: function(value) { this.received = value; }
            });
            Object.defineProperty(ordinaryRoot, symbol, {
              set: function(value) { this.symbolReceived = value; }
            });
            Object.defineProperty(ordinaryRoot, "receiverValue", {
              get: function() { return this.receiverMarker; }
            });
            var ordinaryLeaf = ordinaryRoot;
            for (var i = 0; i < 5000; i += 1) {
              ordinaryLeaf = Object.create(ordinaryLeaf);
            }
            ordinaryLeaf.receiverMarker = 31;
            ordinaryLeaf.sink = 23;
            ordinaryLeaf[symbol] = 29;

            var getCalls = 0;
            var hasCalls = 0;
            var setCalls = 0;
            var defineCalls = 0;
            var handlerRoot = {
              get: function(target, key, receiver) {
                getCalls += 1;
                return receiver === deepGetProxy ? 37 : -1;
              },
              has: function(target, key) {
                hasCalls += 1;
                return key === "present";
              },
              set: function(target, key, value, receiver) {
                setCalls += 1;
                target[key] = value;
                return receiver === deepSetProxy;
              },
              defineProperty: function(target, key, descriptor) {
                defineCalls += 1;
                return Reflect.defineProperty(target, key, descriptor);
              }
            };
            var deepHandler = handlerRoot;
            for (var j = 0; j < 5000; j += 1) {
              deepHandler = Object.create(deepHandler);
            }

            var deepGetProxy = new Proxy({}, deepHandler);
            var deepHasProxy = new Proxy({}, deepHandler);
            var deepSetTarget = {};
            var deepSetProxy = new Proxy(deepSetTarget, deepHandler);
            var deepDefineTarget = {};
            var deepDefineProxy = new Proxy(deepDefineTarget, deepHandler);
            deepSetProxy.value = 41;
            Object.defineProperty(deepDefineProxy, "defined", { value: 53 });

            var proxySymbol = Symbol("proxy-symbol-set");
            var symbolSetCalls = 0;
            var symbolTarget = {};
            var symbolProxy = new Proxy(symbolTarget, {
              set: function(target, key, value, receiver) {
                symbolSetCalls += 1;
                target[key] = value;
                return receiver === symbolProxy;
              }
            });
            symbolProxy[proxySymbol] = 61;

            var cycle = {};
            var cycleProxy = new Proxy(cycle, {});
            Reflect.setPrototypeOf(cycle, cycleProxy);
            var cycleGetThrew = false;
            var cycleHasThrew = false;
            var cycleSetThrew = false;
            try { cycle.missing; } catch (error) { cycleGetThrew = true; }
            try { "missing" in cycle; } catch (error) { cycleHasThrew = true; }
            try { Reflect.set(cycle, "missing", 1); } catch (error) { cycleSetThrew = true; }

            [
              ordinaryLeaf.marker,
              "marker" in ordinaryLeaf,
              ordinaryLeaf.received,
              ordinaryLeaf.symbolReceived,
              ordinaryLeaf[readSymbol],
              readSymbol in ordinaryLeaf,
              ordinaryLeaf.receiverValue,
              deepGetProxy.value,
              getCalls,
              "present" in deepHasProxy,
              hasCalls,
              deepSetTarget.value,
              setCalls,
              deepDefineTarget.defined,
              defineCalls,
              symbolTarget[proxySymbol],
              symbolSetCalls,
              cycleGetThrew,
              cycleHasThrew,
              cycleSetThrew
            ].join(":");
            "#),
        Value::String(Arc::from(
            "17:true:23:29:13:true:31:37:1:true:1:41:1:53:1:61:1:true:true:true"
        ))
    );
}

#[test]
fn transparent_proxy_cycles_recheck_targets_after_observable_trap_lookup() {
    assert_eq!(
        run(r#"
            function makeCycle(trapName, mutate) {
              var target = {};
              var handlerPrototype = {};
              Object.defineProperty(handlerPrototype, trapName, {
                get: function() {
                  mutate(target);
                  return undefined;
                }
              });
              var proxy = new Proxy(target, Object.create(handlerPrototype));
              Reflect.setPrototypeOf(target, proxy);
              return target;
            }

            var getTarget = makeCycle("get", function(target) {
              Object.defineProperty(target, "value", {
                value: 42,
                writable: true,
                configurable: true
              });
            });
            var getResult = getTarget.value;

            var hasTarget = makeCycle("has", function(target) {
              Object.defineProperty(target, "present", {
                value: true,
                configurable: true
              });
            });
            var hasResult = "present" in hasTarget;

            var setTarget = makeCycle("set", function(target) {
              Object.defineProperty(target, "written", {
                value: 0,
                writable: true,
                configurable: true
              });
            });
            var setResult = Reflect.set(setTarget, "written", 9);

            [getResult, hasResult, setResult, setTarget.written].join(":");
            "#),
        Value::String(Arc::from("42:true:true:9"))
    );
}

#[test]
fn transparent_proxy_cycles_preserve_repeated_observable_trap_lookups() {
    assert_eq!(
        run(r#"
            function makeDelayedCycle(trapName, key, initialValue) {
              var count = 0;
              var target = {};
              var handlerPrototype = {};
              Object.defineProperty(handlerPrototype, trapName, {
                get: function() {
                  count += 1;
                  if (count === 2) {
                    Object.defineProperty(target, key, {
                      value: initialValue,
                      writable: true,
                      configurable: true
                    });
                  }
                  return undefined;
                }
              });
              var proxy = new Proxy(target, Object.create(handlerPrototype));
              Reflect.setPrototypeOf(target, proxy);
              return { target: target, count: function() { return count; } };
            }

            var getCycle = makeDelayedCycle("get", "value", 42);
            var getResult = getCycle.target.value;
            var getCount = getCycle.count();

            var hasCycle = makeDelayedCycle("has", "present", 42);
            var hasResult = "present" in hasCycle.target;
            var hasCount = hasCycle.count();

            var setCycle = makeDelayedCycle("set", "written", 0);
            var setResult = Reflect.set(setCycle.target, "written", 9);
            var setCount = setCycle.count();

            [
              getResult, getCount, getCycle.target.value,
              hasResult, hasCount, hasCycle.target.present,
              setResult, setCount, setCycle.target.written
            ].join(":");
            "#),
        Value::String(Arc::from("42:2:42:true:2:42:true:2:9"))
    );
}

#[test]
fn proxy_get_validates_nested_and_string_exotic_invariants() {
    assert_eq!(
        run(r#"
            var fixed = {};
            Object.defineProperty(fixed, "x", {
              value: 1,
              writable: false,
              configurable: false
            });
            var nestedError;
            try {
              new Proxy(new Proxy(fixed, {}), { get: function() { return 2; } }).x;
            } catch (e) {
              nestedError = e.name;
            }
            var hiddenError;
            try {
              var hiding = new Proxy(fixed, {
                getOwnPropertyDescriptor: function() { return undefined; }
              });
              new Proxy(hiding, { get: function() { return 2; } }).x;
            } catch (e) {
              hiddenError = e.name;
            }

            var valid = new Proxy(new String("abc"), {
              get: function(target, key, receiver) {
                if (key === "0") return "a";
                return Reflect.get(target, key, receiver);
              }
            });
            var stringError;
            try {
              new Proxy(new String("abc"), {
                get: function() { return undefined; }
              })[0];
            } catch (e) {
              stringError = e.name;
            }
            var forwarded = new Proxy(new String("abc"), {});
            [nestedError, hiddenError, valid[0], stringError, forwarded["01"]].join(":");
            "#),
        Value::String(Arc::from("TypeError:TypeError:a:TypeError:"))
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
fn delete_identifier_uses_the_resolved_reference() {
    assert_eq!(run("delete missingDeleteTarget;"), Value::Bool(true));
    assert!(run_err(r#""use strict"; delete (((strictDeleteTarget)));"#).contains("SyntaxError"));

    assert_eq!(
        run(r#"
            Object.defineProperty(globalThis, "shadowedDelete", {
              value: 1,
              configurable: true
            });
            let shadowedDelete = 2;
            var deleted = delete shadowedDelete;
            [deleted, shadowedDelete, globalThis.shadowedDelete].join("|");
            "#),
        Value::String(Arc::from("false|2|1"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            globalThis.realmDeleteTarget = "main";
            other.realmDeleteTarget = "foreign";
            var foreignDelete = other.eval(
              "(function() { return delete realmDeleteTarget; })"
            );
            var deleted = foreignDelete();
            [
              deleted,
              "realmDeleteTarget" in other,
              globalThis.realmDeleteTarget
            ].join("|");
            "#),
        Value::String(Arc::from("true|false|main"))
    );

    assert_eq!(
        run(r#"
            eval("var evalDeleteVar = 1; function evalDeleteFunction() {}");
            [
              delete evalDeleteVar,
              typeof evalDeleteVar,
              delete evalDeleteFunction,
              typeof evalDeleteFunction
            ].join("|");
            "#),
        Value::String(Arc::from("true|undefined|true|undefined"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            other.eval(`
              var evalDeleteVar = 1;
              function evalDeleteFunction() {}
              Object.defineProperty(globalThis, "shadowedDelete", {
                value: 1,
                configurable: true
              });
              let shadowedDelete = 2;
            `);
            var foreignDelete = other.eval(`(function() {
              return [
                delete evalDeleteVar,
                typeof evalDeleteVar,
                delete evalDeleteFunction,
                typeof evalDeleteFunction,
                delete shadowedDelete,
                shadowedDelete,
                globalThis.shadowedDelete
              ].join("|");
            })`);
            foreignDelete();
            "#),
        Value::String(Arc::from("true|undefined|true|undefined|false|2|1"))
    );
}

#[test]
fn delete_identifier_object_environment_proxy_survives_gc() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
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
            var target = { blocked: 1, thrown: 2, collected: 3 };
            var log = [];
            var blockedResult, thrownResult, collectedResult;
            var scope = new Proxy(target, {
              has: function(target, key) {
                return Reflect.has(target, key);
              },
              deleteProperty: function(target, key) {
                log.push(key);
                if (key === "blocked") return false;
                if (key === "thrown") throw new Error("delete trap");
                forceGc();
                return Reflect.deleteProperty(target, key);
              }
            });
            with (scope) {
              blockedResult = delete blocked;
              try { delete thrown; }
              catch (error) { thrownResult = error.message; }
              collectedResult = delete collected;
            }
            [
              blockedResult,
              thrownResult,
              collectedResult,
              "collected" in target,
              log.join(",")
            ].join("|");
            "#,
        )
        .expect("object environment Reference should survive Proxy deletion GC"),
        Value::String(Arc::from(
            "false|delete trap|true|false|blocked,thrown,collected"
        ))
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
fn delete_primitive_string_properties_uses_wrapper_delete_semantics() {
    assert_eq!(
        run(r#"
            [
              delete "abc"[0],
              delete "abc".length,
              delete "abc"[3],
              delete "abc"["01"],
              delete "abc"["00"],
              delete "abc"["+0"],
              delete "abc"["1e0"],
              delete "abc"["-0"],
              delete (1).missing
            ].join(":");
            "#),
        Value::String(Arc::from("false:false:true:true:true:true:true:true:true"))
    );
    assert_eq!(
        run(r#"
            var boxed = new String("abc");
            Object.defineProperty(boxed, "01", {
              value: 1,
              configurable: true
            });
            var before = boxed["01"];
            var deleted = delete boxed["01"];
            [
              typeof "abc"["01"],
              before,
              deleted,
              "01" in boxed,
              typeof boxed["01"]
            ].join(":");
            "#),
        Value::String(Arc::from("undefined:1:true:false:undefined"))
    );
    assert!(
        run_err(r#"(function() { "use strict"; delete "abc"[0]; })();"#).contains("TypeError"),
        "strict deletion of a non-configurable string index must throw"
    );
}

#[test]
fn delete_references_root_temporary_base_and_key_during_coercion() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
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
            var log = [];
            function makeBase(label) {
              return new Proxy({ x: 1 }, {
                deleteProperty: function(target, key) {
                  log.push("delete:" + label + ":" + key);
                  return Reflect.deleteProperty(target, key);
                }
              });
            }
            function makeKey(label) {
              return {
                toString: function() {
                  log.push("key:" + label);
                  forceGc();
                  return "x";
                }
              };
            }

            var direct = delete makeBase("direct")[makeKey("direct")];
            var optional = delete makeBase("optional")?.[makeKey("optional")];
            [direct, optional, log.join("|")].join(";");
            "#,
        )
        .expect("delete References should root temporary bases and names"),
        Value::String(Arc::from(
            "true;true;key:direct|delete:direct:x|key:optional|delete:optional:x"
        ))
    );
}

#[test]
fn delete_proxy_enforces_nested_target_invariants() {
    assert_eq!(
        run(r#"
            var log = [];
            var target = {};
            Object.defineProperty(target, "fixed", {
              value: 1,
              configurable: false
            });
            var inner = new Proxy(target, {
              getOwnPropertyDescriptor: function(t, key) {
                log.push("gopd:" + key);
                return Reflect.getOwnPropertyDescriptor(t, key);
              }
            });
            var outer = new Proxy(inner, {
              deleteProperty: function() {
                log.push("delete");
                return true;
              }
            });
            try {
              delete outer.fixed;
            } catch (error) {
              log.push(error.name);
            }
            log.join("|");
            "#),
        Value::String(Arc::from("delete|gopd:fixed|TypeError"))
    );

    assert_eq!(
        run(r#"
            var log = [];
            var target = { present: 1 };
            Object.preventExtensions(target);
            var inner = new Proxy(target, {
              getOwnPropertyDescriptor: function(t, key) {
                log.push("gopd:" + key);
                return Reflect.getOwnPropertyDescriptor(t, key);
              },
              isExtensible: function(t) {
                log.push("extensible");
                return Reflect.isExtensible(t);
              }
            });
            var outer = new Proxy(inner, {
              deleteProperty: function() {
                log.push("delete");
                return true;
              }
            });
            try {
              delete outer.present;
            } catch (error) {
              log.push(error.name);
            }
            log.push("present" in target);
            log.join("|");
            "#),
        Value::String(Arc::from("delete|gopd:present|extensible|TypeError|true"))
    );

    assert!(
        run_err(
            r#"
            (function() {
              "use strict";
              var proxy = new Proxy({ x: 1 }, {
                deleteProperty: function() { return false; }
              });
              delete proxy?.x;
            })();
            "#
        )
        .contains("TypeError"),
        "strict optional delete must reject a false Proxy trap result"
    );
}

#[test]
fn delete_proxy_follows_deep_transparent_chains_iteratively() {
    assert_eq!(
        run(r#"
            var log = [];
            var target = { removable: 1 };
            Object.defineProperty(target, "fixed", {
              value: 2,
              configurable: false
            });
            var transparent = { deleteProperty: null };
            var proxy = target;
            for (var i = 0; i < 100000; i += 1) {
              proxy = new Proxy(proxy, transparent);
            }
            var outerHandler = {};
            Object.defineProperty(outerHandler, "deleteProperty", {
              get: function() {
                log.push("get");
                return undefined;
              }
            });
            proxy = new Proxy(proxy, outerHandler);

            var removed = Reflect.deleteProperty(proxy, "removable");
            var fixed = Reflect.deleteProperty(proxy, "fixed");
            [
              removed,
              !Object.prototype.hasOwnProperty.call(target, "removable"),
              fixed,
              Object.prototype.hasOwnProperty.call(target, "fixed"),
              log.join("|")
            ].join(",");
            "#),
        Value::String(Arc::from("true,true,false,true,get|get"))
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
fn parenthesized_member_read_modify_write_uses_one_reference() {
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
              get: function(t, k, receiver) {
                log.push("get:" + t[k]);
                return Reflect.get(t, k, receiver);
              },
              set: function(t, k, value, receiver) {
                log.push("set:" + value);
                return Reflect.set(t, k, value, receiver);
              }
            });

            var compound = (proxy[key]) += 2;
            var logical = (proxy[key]) &&= 5;
            var post = (proxy[key])++;
            var pre = ++(proxy[key]);
            [compound, logical, post, pre, target.x, log.join("|")].join(";");
            "#),
        Value::String(Arc::from(
            "3;5;5;7;7;key|get:1|set:3|key|get:3|set:5|key|get:5|set:6|key|get:6|set:7"
        ))
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
        r#""use strict"; 0, { ...eval } = {};"#,
        r#""use strict"; 0, [{ x: arguments }] = [{}];"#,
        r#""use strict"; for ([arguments] in [[]]) ;"#,
        r#""use strict"; for ({ eval } of [{}]) ;"#,
    ] {
        assert!(run_err(src).contains("SyntaxError"), "{src}");
    }
}

#[test]
fn object_assignment_rest_must_be_last() {
    assert!(run_err("var rest, b; 0, {...rest, b} = {};").contains("SyntaxError"));
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

#[test]
fn deferred_member_assignment_references_survive_gc() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
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
            var log = [];
            function makeBase(label) {
              var target = {};
              return new Proxy(target, {
                set: function(t, key, value, receiver) {
                  log.push("set:" + label + ":" + key + ":" + value);
                  t[key] = value;
                  return true;
                }
              });
            }
            function makeKey(label) {
              return {
                toString: function() {
                  log.push("key:" + label);
                  return label;
                }
              };
            }

            var simple = makeBase("simple")[makeKey("x")] = (forceGc(), 7);

            var source = {
              get value() {
                log.push("source");
                forceGc();
                return 9;
              }
            };
            ({ value: makeBase("destructure")[makeKey("y")] } = source);

            [simple, log.join("|")].join(";");
            "#,
        )
        .expect("raw member references should root their base and name"),
        Value::String(Arc::from(
            "7;key:x|set:simple:x:7|source|key:y|set:destructure:y:9"
        ))
    );
}

#[test]
fn deferred_proxy_object_key_observes_rhs_mutation_after_gc() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
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
            var log = [];
            var symbol = Symbol("deferred");
            var keyTarget = {
                [Symbol.toPrimitive]: function() {
                    log.push("old");
                    return "old";
                }
            };
            var key = new Proxy(keyTarget, {
                get: function(target, property, receiver) {
                    forceGc();
                    if (property === Symbol.toPrimitive) {
                        log.push("get:" + (receiver === key));
                    }
                    return Reflect.get(target, property, receiver);
                }
            });
            var target = {};
            var proxy = new Proxy(target, {
                set: function(target, property, value, receiver) {
                    log.push("set:" + (property === symbol) + ":" + (receiver === proxy));
                    target[property] = value;
                    return true;
                }
            });
            proxy[key] = (
                keyTarget[Symbol.toPrimitive] = function() {
                    forceGc();
                    log.push("new:" + (this === key));
                    return symbol;
                },
                forceGc(),
                9
            );
            target[symbol] + ":" + target.old + ":" + log.join("|");
            "#,
        )
        .expect("deferred Proxy object key should survive GC and RHS mutation"),
        Value::String(Arc::from("9:undefined:get:true|new:true|set:true:true"))
    );
}

#[test]
fn destructuring_member_assignment_uses_property_reference_for_set() {
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
            var target = {};
            var proxy = new Proxy(target, {
              set: function(t, k, v, r) {
                var label = k === toPrimitiveSym ? "toPrimitiveSym" : k;
                log.push("set:" + label + ":" + v + ":" + (r === proxy));
                t[k] = v;
                return true;
              }
            });
            ({ a: proxy[key], b: proxy[toPrimitiveKey] } = { a: 2, b: 9 });
            [target.x, target[toPrimitiveSym], log.join("|")].join(";");
            "#),
        Value::String(Arc::from(
            "2;9;key|set:x:2:true|toPrimitiveKey|set:toPrimitiveSym:9:true"
        ))
    );

    assert_eq!(
        run(r#"
            var o = {};
            Object.defineProperty(o, "x", { value: 1, writable: false });
            var sloppy = ({ a: o.x } = { a: 2 });
            var primitive = ({ a: "abc".x } = { a: 4 });
            var strict;
            try {
              (function() { "use strict"; ({ a: o.x } = { a: 3 }); })();
            } catch (e) {
              strict = e.name;
            }
            sloppy.a + ":" + primitive.a + ":" + o.x + ":" + strict;
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
fn numeric_computed_references_preserve_property_key_boundaries() {
    assert_eq!(
        run(r#"
            var object = {
              "0": 1,
              "4294967294": 2,
              "4294967295": 3,
              "-0": 4,
              "1.5": 5
            };
            object[-0] += 1;
            object[4294967294]++;
            object[4294967295] ||= 9;
            object[-0] &&= 7;
            ++object[1.5];
            [
              object[0], object["-0"], object[4294967294],
              object[4294967295], object[1.5],
              Object.keys(object).join("|")
            ].join(";");
        "#),
        Value::String(Arc::from("7;4;3;3;6;0|4294967294|4294967295|-0|1.5"))
    );
}

#[test]
fn non_index_numeric_keys_reach_proxies_as_exact_strings() {
    assert_eq!(
        run(r#"
            var log = [];
            var target = {};
            var proxy = new Proxy(target, {
              get: function(t, key, receiver) {
                log.push("get:" + typeof key + ":" + key);
                return Reflect.get(t, key, receiver);
              },
              set: function(t, key, value, receiver) {
                log.push("set:" + typeof key + ":" + key);
                return Reflect.set(t, key, value, receiver);
              },
              has: function(t, key) {
                log.push("has:" + typeof key + ":" + key);
                return Reflect.has(t, key);
              },
              deleteProperty: function(t, key) {
                log.push("delete:" + typeof key + ":" + key);
                return Reflect.deleteProperty(t, key);
              }
            });

            proxy[-1] = 1;
            proxy[1.5] = 2;
            proxy[4294967295] = 3;
            proxy[1e21] = 4;
            proxy[5e-17] = 5;
            proxy[NaN] = 6;
            proxy[Infinity] = 7;
            proxy[-Infinity] = 8;
            var read = proxy[5e-17];
            var present = 1e21 in proxy;
            var deleted = delete proxy[-Infinity];
            [read, present, deleted, log.join("|")].join(";");
        "#),
        Value::String(Arc::from(concat!(
            "5;true;true;set:string:-1|set:string:1.5|set:string:4294967295|",
            "set:string:1e+21|set:string:5e-17|set:string:NaN|",
            "set:string:Infinity|set:string:-Infinity|get:string:5e-17|",
            "has:string:1e+21|delete:string:-Infinity"
        )))
    );
}

#[test]
fn computed_read_modify_write_null_base_skips_key_coercion_and_rhs() {
    for source in ["base[prop] += rhs();", "base[prop]++;", "++base[prop];"] {
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
            {source}
            "#
        ));
        assert!(err.contains("TypeError"), "source {source} got {err}");
        assert!(
            !err.contains("property key evaluated"),
            "source {source} coerced property key before null-base check: {err}"
        );
        assert!(
            !err.contains("right-hand side evaluated"),
            "source {source} evaluated RHS before null-base check: {err}"
        );
    }
}

#[test]
fn computed_read_modify_write_roots_reference_across_gc() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
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
            var log = [];
            function makeKey(label) {
              return {
                toString: function() {
                  log.push("toString:" + label);
                  return {};
                },
                valueOf: function() {
                  log.push("valueOf:" + label);
                  forceGc();
                  return 0;
                }
              };
            }
            function makeBase(label, initial) {
              return new Proxy({ 0: initial }, {
                get: function(target, name, receiver) {
                  log.push("get:" + label + ":" + name);
                  forceGc();
                  return Reflect.get(target, name, receiver);
                },
                set: function(target, name, value, receiver) {
                  log.push("set:" + label + ":" + value);
                  forceGc();
                  return Reflect.set(target, name, value, receiver);
                }
              });
            }
            var compound = makeBase("compound", 1)[makeKey("compound")] +=
              (forceGc(), 2);
            var logical = makeBase("logical", 1)[makeKey("logical")] &&=
              (forceGc(), 4);
            var update = makeBase("update", 4)[makeKey("update")]++;
            [compound, logical, update, log.join("|")].join(";");
            "#,
        )
        .expect("computed read-modify-write References should survive GC"),
        Value::String(Arc::from(
            "3;4;4;toString:compound|valueOf:compound|get:compound:0|set:compound:3|toString:logical|valueOf:logical|get:logical:0|set:logical:4|toString:update|valueOf:update|get:update:0|set:update:5"
        ))
    );
}

#[test]
fn computed_read_modify_write_preserves_object_to_symbol_keys() {
    assert_eq!(
        run(r#"
            var symbol = Symbol("key");
            var coercions = 0;
            var key = {};
            key[Symbol.toPrimitive] = function() {
              coercions++;
              return symbol;
            };
            var target = {};
            target[symbol] = 1;
            var log = [];
            var proxy = new Proxy(target, {
              get: function(inner, name, receiver) {
                log.push("get:" + (name === symbol));
                return Reflect.get(inner, name, receiver);
              },
              set: function(inner, name, value, receiver) {
                log.push("set:" + (name === symbol) + ":" + value);
                return Reflect.set(inner, name, value, receiver);
              }
            });
            var compound = proxy[key] += 1;
            var logical = proxy[key] &&= 4;
            var update = proxy[key]++;
            [compound, logical, update, target[symbol], coercions, log.join("|")].join(";");
        "#),
        Value::String(Arc::from(
            "2;4;4;5;3;get:true|set:true:2|get:true|set:true:4|get:true|set:true:5"
        ))
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
fn assignment_uses_the_resolved_declarative_environment_after_delete() {
    assert_eq!(
        run(r#"
            var x = "outer";
            var y = "outer";
            var z = "outer";
            function f() {
              eval("var x; x = (delete x, 'inner');");
              eval("var y = 'local'; y += (delete y, '-updated');");
              eval("var z = 0; z ||= (delete z, 7);");
              return [x, y, z].join(":");
            }
            [f(), x, y, z].join("|");
            "#),
        Value::String(Arc::from("inner:local-updated:7|outer|outer|outer"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var writable = other.eval(
              "var x = 1; x = 2; [x, globalThis.x].join(':');"
            );
            var readonly = other.eval(`
              var fixed = 1;
              Object.defineProperty(globalThis, "fixed", { writable: false });
              var error = "";
              try { (function() { "use strict"; fixed = 2; })(); }
              catch (e) { error = e.name; }
              [error, fixed, globalThis.fixed].join(":");
            `);
            writable + "|" + readonly;
            "#),
        Value::String(Arc::from("2:2|TypeError:1:1"))
    );

    assert_eq!(
        run(r#"
            eval("var blocked = 1; var observed = 1;");
            Object.defineProperty(globalThis, "blocked", {
              get: function() { return 10; },
              set: function() { throw new Error("blocked"); },
              configurable: true
            });
            Object.defineProperty(globalThis, "observed", {
              get: function() { return this._observed; },
              set: function(value) { this._observed = value + 1; },
              configurable: true
            });
            var error = "";
            try { blocked = 2; } catch (e) { error = e.message; }
            observed = 2;
            [
              error, blocked, globalThis.blocked,
              observed, globalThis.observed
            ].join("|");
            "#),
        Value::String(Arc::from("blocked|10|10|3|3"))
    );

    assert_eq!(
        run(r#"
            eval("var removed = 1; var selfRemoving = 1;");
            var deleted = delete globalThis.removed;
            Object.defineProperty(globalThis, "selfRemoving", {
              get: function() { return 1; },
              set: function() { delete globalThis.selfRemoving; },
              configurable: true
            });
            selfRemoving = 2;
            [
              deleted, typeof removed, "removed" in globalThis,
              typeof selfRemoving, "selfRemoving" in globalThis
            ].join("|");
            "#),
        Value::String(Arc::from("true|undefined|false|undefined|false"))
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

#[test]
fn optional_chain_continues_through_non_optional_segments() {
    assert_eq!(
        run(r#"
            var calls = 0;
            function key() { calls++; return "x"; }
            var root = null;
            var member = root?.a.b;
            var computed = root?.a[key()].b;
            var call = root?.method(key()).result;
            [member, computed, call, calls].join("|");
        "#),
        Value::String(Arc::from("|||0"))
    );

    assert_eq!(
        run(r#"
            var root = { a: undefined };
            try { root?.a.b; } catch (error) { error.name; }
        "#),
        Value::String(Arc::from("TypeError"))
    );

    assert_eq!(
        run(r#"
            var root = null;
            try { (root?.a).b; } catch (error) { error.name; }
        "#),
        Value::String(Arc::from("TypeError"))
    );
}

#[test]
fn delete_optional_chain_deletes_only_after_a_live_base() {
    assert_eq!(run("delete null?.x;"), Value::Bool(true));
    assert_eq!(
        run("var object = { x: 1 }; var deleted = delete object?.x; deleted && !('x' in object);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var object = null; var calls = 0; delete object?.[calls++]; calls;"),
        Value::Number(0.0)
    );
    assert!(run_err("class C { #x; remove(value) { delete value?.#x; } }").contains("SyntaxError"));
}

#[test]
fn optional_chain_is_not_assignment_target() {
    for src in [
        "var a = {}; a?.b = 1;",
        "var a = { b: {} }; a?.b.c = 1;",
        "var a = { b: {} }; (a?.b.c) = 1;",
        "var a = {}; ++a?.b;",
        "var a = { b: {} }; ++a?.b.c;",
        "var a = {}; a?.b++;",
        "var a = { b: {} }; a?.b.c++;",
        "var a = {}; 0, [a?.b = 1] = [2];",
        "var a = { b: {} }; 0, [a?.b.c = 1] = [2];",
        "var a = {}; 0, { x: a?.b = 1 } = { x: 2 };",
        "var a = { b: {} }; 0, { x: a?.b.c = 1 } = { x: 2 };",
        "var a = { b: {} }; for (a?.b.c in {}) ;",
    ] {
        assert!(run_err(src).contains("SyntaxError"), "{src}");
    }

    assert_eq!(
        run("var a = { b: {} }; (a?.b).c = 1; a.b.c;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var a = { b: {} }; for ((a?.b).c in {x: 1}) {} a.b.c;"),
        Value::String(Arc::from("x"))
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
    assert_eq!(
        run("var o = {x: 3, m: function(a){ return this.x + a; }}; (o?.m)(4);"),
        Value::Number(7.0)
    );
    assert_eq!(
        run("var o = {x: 3, m: function(a){ return this.x + a; }}; (o?.m)?.(4);"),
        Value::Number(7.0)
    );
}

#[test]
fn optional_member_calls_use_property_references() {
    assert_eq!(
        run(r#"
            var log = [];
            var method = function(value) {
              "use strict";
              log.push(this === proxy ? "this" : "bad-this");
              return value + 1;
            };
            var proxy = new Proxy({ method: method }, {
              get: function(target, property, receiver) {
                log.push("get:" + property);
                return Reflect.get(target, property, receiver);
              }
            });
            var key = {
              toString: function() {
                log.push("key");
                return "method";
              }
            };
            var results = [
              proxy?.[key](1),
              proxy[key]?.(...[2]),
              (proxy?.[key])(3),
              (proxy?.[key])?.(...[4])
            ];
            results.join(":") + ";" + log.join("|");
            "#),
        Value::String(Arc::from(
            "2:3:4:5;key|get:method|this|key|get:method|this|key|get:method|this|key|get:method|this"
        ))
    );

    assert_eq!(
        run(r#"
            var log = [];
            var key = { toString: function() { log.push("key"); return "method"; } };
            function argument() { log.push("argument"); return 1; }
            var missing = null;
            missing?.[key](argument());
            ({})[key]?.(argument());
            try { (missing?.method)(argument()); } catch (error) { log.push(error.name); }
            (missing?.method)?.(argument());
            log.join("|");
            "#),
        Value::String(Arc::from("key|argument|TypeError"))
    );

    assert_eq!(
        run(r#"
            String.prototype.capture = function(value) {
              "use strict";
              return typeof this + ":" + this + ":" + value;
            };
            "base".capture?.(...[7]);
            "#),
        Value::String(Arc::from("string:base:7"))
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

#[test]
fn annex_b_html_comments_respect_literal_and_template_boundaries() {
    assert_eq!(
        run(r#"
            var regex = /<!--|-->/;
            var raw = `<!--
-->`;
            var open = `${<!-- open comment
1}`;
            var close = `${
--> close comment
2}`;
            var nested = `outer${`inner${
--> nested close comment
3}`}`;
            var slash;
            0;
            --> keep the following slash in expression-start context
            slash = /x/.test("x");
            [
              regex.test("<!--"),
              regex.test("-->"),
              raw,
              open,
              close,
              nested,
              slash
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|<!--\n-->|1|2|outerinner3|true"))
    );

    for source in [
        concat!("'a\\", "\n", "b'-->0"),
        "`a\nb`-->0",
        "`a${--> not a close comment\n1}`",
        "`outer${`inner${--> not a nested close comment\n1}`}`",
    ] {
        assert!(run_err(source).contains("SyntaxError"), "{source:?}");
    }
}

#[test]
fn annex_b_call_assignment_targets_throw_after_only_the_call() {
    assert_eq!(
        run(r#"
            var log = [];
            function f() {
              log.push("f");
              return { valueOf() { log.push("valueOf"); return 1; } };
            }
            function g() { log.push("g"); return 1; }
            function capture(action) {
              log.length = 0;
              try { action(); return "no-error:" + log.join(","); }
              catch (error) {
                return (error instanceof ReferenceError) + ":" + log.join(",");
              }
            }
            [
              capture(function() { f() = g(); }),
              capture(function() { (f()) += g(); }),
              capture(function() { f()++; }),
              capture(function() { ++f(); }),
              capture(function() { for (f() in { key: 1 }) {} }),
              capture(function() { for (f() of [1]) {} }),
              capture(function() { function async() {} async() = g(); })
            ].join("|");
        "#),
        Value::String(Arc::from("true:f|true:f|true:f|true:f|true:f|true:f|true:"))
    );

    assert_eq!(
        run(r#"
            var called = false;
            var closed = false;
            function f() { called = true; return 0; }
            var iterable = {
              [Symbol.iterator]: function() {
                return {
                  next: function() { return { value: 1, done: false }; },
                  return: function() { closed = true; return { done: true }; }
                };
              }
            };
            try { for (f() of iterable) {} } catch (error) {}
            called + ":" + closed;
        "#),
        Value::String(Arc::from("true:false"))
    );

    assert_eq!(
        run(r#"
            var called = false;
            function f() { called = true; }
            for (f() of []) {}
            called;
        "#),
        Value::Bool(false)
    );

    assert!(run_err("function f() {} f() &&= 1;").contains("SyntaxError"));
    assert!(run_err("'use strict'; function f() {} f() = 1;").contains("SyntaxError"));
}

#[test]
fn annex_b_call_assignment_preserves_abrupt_completion_precedence() {
    assert_eq!(
        run(r#"
            var sentinel = {};
            function f() { throw sentinel; }
            try { f() = 1; } catch (error) { error === sentinel; }
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var original = [];
            var closed = false;
            var sentinel = {};
            function f() { throw sentinel; }
            var iterable = {
              [Symbol.iterator]() {
                return {
                  next() { return { value: 1, done: false }; },
                  return() { closed = true; throw new TypeError("close"); }
                };
              }
            };
            try { for (f() of iterable) {} } catch (error) {
              original.push(error instanceof ReferenceError);
            }
            try { for (f() in { x: 1 }) {} } catch (error) {
              original.push(error instanceof ReferenceError);
            }
            original.join(",") + ":" + closed;
        "#),
        Value::String(Arc::from("true,true:false"))
    );
}

#[test]
fn annex_b_call_assignment_respects_eval_async_and_optional_boundaries() {
    assert_eq!(
        run(r#"
            var log = [];
            function sloppy() {
              var result = false;
              eval("function f(){ log.push('call'); } try { f() = log.push('rhs'); } catch (e) { result = e instanceof ReferenceError; }");
              return result;
            }
            function strict() {
              "use strict";
              try { eval("function f(){} f() = 1;"); return false; }
              catch (error) { return error instanceof SyntaxError; }
            }
            sloppy() + ":" + strict() + ":" + log.join(",");
        "#),
        Value::String(Arc::from("true:true:call"))
    );

    assert_eq!(
        run(r#"
            var log = [];
            async function f() { log.push("async-call"); }
            try { f() = log.push("rhs"); } catch (error) {
              log.push(error instanceof ReferenceError);
            }
            function outer() {
              log.push("outer");
              return function inner() { log.push("inner"); };
            }
            try { (outer?.())() = 1; } catch (error) {
              log.push(error instanceof ReferenceError);
            }
            log.join(",");
        "#),
        Value::String(Arc::from("async-call,true,outer,inner,true"))
    );

    assert_eq!(
        run(r#"
            var closed = false;
            var sentinel = {};
            var iterable = {
              [Symbol.asyncIterator]() {
                return {
                  next() { return Promise.resolve({ value: 1, done: false }); },
                  return() { closed = true; return Promise.resolve({}); }
                };
              }
            };
            async function check() {
              function f() { throw sentinel; }
              try { for await (f() of iterable) {} }
              catch (error) { return (error instanceof ReferenceError) + ":" + closed; }
            }
            await check();
        "#),
        Value::String(Arc::from("true:false"))
    );

    assert_eq!(
        run(r#"
            var result;
            try {
              try {
                outer: while (true) {
                  try {
                    try {
                      try { break outer; } catch (_) {}
                    } finally {}
                  } catch (_) {}
                }
                throw "after";
              } catch (error) { result = error; }
            } catch (_) { result = "escaped"; }
            result;
        "#),
        Value::String(Arc::from("after"))
    );

    assert_eq!(
        run(r#"
            var result;
            try {
              result = (function () {
                outer: while (true) {
                  try {
                    try {
                      try { break outer; } finally {}
                    } catch (_) { return "middle"; }
                  } finally { throw "outer"; }
                }
              })();
            } catch (error) { result = error; }
            result;
        "#),
        Value::String(Arc::from("outer"))
    );

    assert_eq!(
        run(r#"
            function check() {
              outer: while (true) {
                try {
                  try {
                    try { break outer; } finally {}
                  } catch (_) {}
                } finally {}
              }
              return "ok";
            }
            check();
        "#),
        Value::String(Arc::from("ok"))
    );
}

#[test]
fn finally_abrupt_completion_replaces_pending_call_target_error() {
    assert_eq!(
        run(r#"
            var iterations = 0;
            function f() {}
            outer: while (iterations < 2) {
              try {
                iterations++;
                for (f() of [1]) {}
              } finally {
                continue outer;
              }
            }
            var marker = 0;
            try { marker = 1; } finally { marker = 2; }
            marker;
        "#),
        Value::Number(2.0)
    );

    assert_eq!(
        run(r#"
            var sentinel = {};
            var caught = false;
            try {
              try { throw sentinel; }
              finally {
                label: {
                  try {} finally { break label; }
                }
              }
            } catch (error) { caught = error === sentinel; }
            caught;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var sentinel = {};
            var caught;
            var marker = false;
            try {
              try { throw sentinel; }
              finally {
                label: {
                  try { throw "inner"; } finally { break label; }
                }
                marker = true;
              }
            } catch (error) { caught = error; }
            (caught === sentinel) + ":" + marker;
        "#),
        Value::String(Arc::from("true:true"))
    );

    assert_eq!(
        run(r#"
            var log = [];
            try {
              while (true) { log.push("body"); break; }
              log.push("after");
            } finally { log.push("finally"); }
            log.join(",");
        "#),
        Value::String(Arc::from("body,after,finally"))
    );

    assert_eq!(
        run(r#"
            var log = [];
            outer: while (true) {
              try {
                try { throw "original"; }
                finally { break outer; }
              } finally { log.push("outer-finally"); }
            }
            log.join(",");
        "#),
        Value::String(Arc::from("outer-finally"))
    );

    assert_eq!(
        run(r#"
            var caught;
            try {
              try { throw "original"; }
              finally { throw "replacement"; }
            } catch (error) { caught = error; }
            var marker = 0;
            try { marker = 1; } finally { marker = 2; }
            caught + ":" + marker;
        "#),
        Value::String(Arc::from("replacement:2"))
    );
}

#[test]
fn nested_finally_completions_survive_generator_and_async_suspension() {
    assert_eq!(
        run(r#"
            var sentinel = {};
            function* generate() {
              try {
                try { throw sentinel; }
                finally { yield "pause"; }
              } catch (error) { yield error === sentinel; }
            }
            var iterator = generate();
            iterator.next().value + ":" + iterator.next().value;
        "#),
        Value::String(Arc::from("pause:true"))
    );

    assert_eq!(
        run(r#"
            var sentinel = {};
            async function check() {
              try {
                try { throw sentinel; }
                finally { await 0; }
              } catch (error) { return error === sentinel; }
            }
            await check();
        "#),
        Value::Bool(true)
    );
}

#[test]
fn finally_control_exit_discards_stale_catches_and_roots_pending_values() {
    assert_eq!(
        run(r#"
            var result = "none";
            try {
              outer: while (true) {
                try {} finally {
                  try { break outer; }
                  catch (_) { result = "stale"; }
                }
              }
              throw "after";
            } catch (error) { result = error; }
            result;
        "#),
        Value::String(Arc::from("after"))
    );

    assert_eq!(
        run(r#"
            function make() {
              try { return { marker: 1 }; }
              finally {
                for (var i = 0; i < 100000; i++) ({ i: i });
              }
            }
            make().marker;
        "#),
        Value::Number(1.0)
    );

    assert_eq!(
        run(r#"
            var result;
            try {
              result = (function nested() { return 7; })();
            } finally {}
            result;
        "#),
        Value::Number(7.0)
    );

    assert_eq!(
        run(r#"
            var caught = false;
            var escaped = false;
            try {
              outer: while (true) {
                try {
                  try { break outer; }
                  finally { throw "replacement"; }
                } catch (error) {
                  caught = error === "replacement";
                  break outer;
                }
              }
            } catch (_) { escaped = true; }
            caught + ":" + escaped;
        "#),
        Value::String(Arc::from("true:false"))
    );
}
