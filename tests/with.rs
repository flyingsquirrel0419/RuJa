//! `with` statement: dynamic object environment records.

mod common;
use common::{run, run_err};
use ruja::{Value, Vm};
use std::sync::Arc;

#[test]
fn with_reads_object_property() {
    let src = r#"
        let o = { x: 1, y: 2 };
        let z = 100;
        let result;
        with (o) {
            result = x + y + z;
        }
        result;
    "#;
    assert_eq!(run(src), Value::Number(103.0));
}

#[test]
fn active_with_environment_keeps_binding_object_alive_across_gc() {
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
            var result;
            with ({ value: 41 }) {
              forceGc();
              result = value;
            }
            result;
            "#,
        )
        .expect("active with object should survive collection"),
        Value::Number(41.0)
    );
}

#[test]
fn with_shadows_outer_var() {
    let src = r#"
        let p = { name: "inner" };
        let name = "outer";
        let r;
        with (p) { r = name; }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("inner")));
}

#[test]
fn with_assignment_writes_to_object() {
    let src = r#"
        let o = { count: 0 };
        with (o) {
            count = count + 5;
        }
        o.count;
    "#;
    assert_eq!(run(src), Value::Number(5.0));
}

#[test]
fn with_reads_inherited_property() {
    let src = r#"
        let proto = { x: 1 };
        let o = Object.create(proto);
        let r;
        with (o) {
            r = x;
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(1.0));
}

#[test]
fn with_assignment_to_inherited_property_creates_own_property() {
    let src = r#"
        let proto = { x: 1 };
        let o = Object.create(proto);
        with (o) {
            x = 2;
        }
        proto.x + ":" + o.x + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1:2:true")));
}

#[test]
fn with_compound_assignment_uses_inherited_property_reference() {
    let src = r#"
        let proto = { x: 1 };
        let o = Object.create(proto);
        with (o) {
            x += 4;
        }
        proto.x + ":" + o.x + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1:5:true")));
}

#[test]
fn with_proxy_has_trap_controls_identifier_resolution() {
    let src = r#"
        var outer = "outer";
        var calls = [];
        var target = {};
        var proxy = new Proxy(target, {
          has: function(t, k) {
            calls.push("has:" + k);
            return k === "outer";
          },
          get: function(t, k) {
            calls.push(typeof k === "symbol" ? "get:symbol" : "get:" + k);
            if (k === "outer") return "proxy";
            return t[k];
          }
        });
        var result = (function() {
          with (proxy) {
            return outer;
          }
        })();
        result + ":" + calls.join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("proxy:has:outer|get:symbol|has:outer|get:outer"))
    );
}

#[test]
fn with_proxy_compound_assignment_preserves_reference() {
    let src = r#"
        var x = 10;
        var calls = [];
        var target = { x: 1 };
        var proxy = new Proxy(target, {
          has: function(t, k) {
            calls.push("has:" + k);
            return k === "x";
          },
          get: function(t, k) {
            calls.push(typeof k === "symbol" ? "get:symbol" : "get:" + k);
            return t[k];
          },
          set: function(t, k, v, r) {
            calls.push("set:" + k + ":" + v + ":" + (r === proxy));
            t[k] = v;
            return true;
          }
        });
        with (proxy) {
          x += 2;
        }
        x + ":" + target.x + ":" + calls.join("|");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "10:3:has:x|get:symbol|has:x|get:x|has:x|set:x:3:true"
        ))
    );
}

#[test]
fn with_proxy_reference_survives_gc_across_get_set_delete_and_call() {
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
            var target = {
                x: 1,
                method: function() { "use strict"; return this === proxy; }
            };
            var counts = { has: 0, get: 0, set: 0, delete: 0 };
            var proxy = new Proxy(target, {
                has: function(t, key) {
                    forceGc();
                    counts.has++;
                    return Reflect.has(t, key);
                },
                get: function(t, key, receiver) {
                    forceGc();
                    counts.get++;
                    return Reflect.get(t, key, receiver);
                },
                set: function(t, key, value, receiver) {
                    forceGc();
                    counts.set++;
                    return Reflect.set(t, key, value, receiver);
                },
                deleteProperty: function(t, key) {
                    forceGc();
                    counts.delete++;
                    return Reflect.deleteProperty(t, key);
                }
            });
            var read, receiver, deleted;
            with (proxy) {
                read = x;
                x += 2;
                receiver = method();
                deleted = delete x;
            }
            read === 1 && receiver && deleted && target.x === undefined &&
                counts.has > 0 && counts.get > 0 && counts.set === 1 &&
                counts.delete === 1;
            "#,
        )
        .expect("object-environment Proxy operations should survive GC"),
        Value::Bool(true)
    );
}

#[test]
fn strict_with_proxy_set_false_throws_for_assignment_forms() {
    let src = r#"
        var target = { x: 1 };
        var proxy = new Proxy(target, {
          has: function(t, k) {
            return k === "x";
          },
          get: function(t, k, r) {
            return k === Symbol.unscopables ? undefined : Reflect.get(t, k, r);
          },
          set: function(t, k, v, r) {
            return false;
          }
        });
        var out = [];
        with (proxy) {
          try { (function() { "use strict"; x = 2; })(); }
          catch (e) { out.push(e.name); }
          try { (function() { "use strict"; x += 2; })(); }
          catch (e) { out.push(e.name); }
          try { (function() { "use strict"; ++x; })(); }
          catch (e) { out.push(e.name); }
          try { (function() { "use strict"; x &&= 2; })(); }
          catch (e) { out.push(e.name); }
        }
        out.join("|") + ":" + target.x;
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("TypeError|TypeError|TypeError|TypeError:1"))
    );
}

#[test]
fn sloppy_with_proxy_set_false_is_silent() {
    let src = r#"
        var target = { x: 1 };
        var proxy = new Proxy(target, {
          has: function(t, k) {
            return k === "x";
          },
          get: function(t, k, r) {
            return k === Symbol.unscopables ? undefined : Reflect.get(t, k, r);
          },
          set: function(t, k, v, r) {
            return false;
          }
        });
        with (proxy) {
          x = 2;
        }
        target.x;
    "#;
    assert_eq!(run(src), Value::Number(1.0));
}

#[test]
fn with_inherited_method_call_binds_this_to_with_object() {
    let src = r#"
        let proto = { f: function() { return this.tag; } };
        let o = Object.create(proto);
        o.tag = "with-object";
        let r;
        with (o) {
            r = f();
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("with-object")));
}

#[test]
fn with_boxes_primitive_binding_object() {
    let src = r#"
        let r;
        with ("abc") {
            r = length;
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(3.0));
}

#[test]
fn with_boxes_primitive_in_the_execution_realm() {
    let src = r#"
        var other = $262.createRealm().global;
        other.eval(`
            String.prototype.realmMarker = "other";
            (function() {
                var result;
                with ("abc") { result = realmMarker + ":" + length; }
                return result;
            })();
        `);
    "#;
    assert_eq!(run(src), Value::String(Arc::from("other:3")));
}

#[test]
fn with_unscopables_hides_object_binding() {
    let src = r#"
        let x = "outer";
        let o = { x: "inner" };
        o[Symbol.unscopables] = { x: true };
        let r;
        with (o) {
            r = x;
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("outer")));
}

#[test]
fn with_honors_intrinsic_array_unscopables() {
    let src = r#"
        let at = "at", copyWithin = "copyWithin", entries = "entries";
        let fill = "fill", find = "find", findIndex = "findIndex";
        let findLast = "findLast", findLastIndex = "findLastIndex";
        let flat = "flat", flatMap = "flatMap", includes = "includes";
        let keys = "keys", toReversed = "toReversed", toSorted = "toSorted";
        let toSpliced = "toSpliced", values = "values";
        let result;
        with ([]) {
            result = [
              at, copyWithin, entries, fill, find, findIndex, findLast,
              findLastIndex, flat, flatMap, includes, keys, toReversed,
              toSorted, toSpliced, values
            ].join(",");
        }
        let foreignArray = $262.createRealm().global.Array.of(1);
        let foreign;
        with (foreignArray) { foreign = at; }
        Array.prototype[Symbol.unscopables].values = false;
        let live;
        with ([]) { live = values; }
        result + ":" + foreign + ":" + (live === Array.prototype.values);
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "at,copyWithin,entries,fill,find,findIndex,findLast,findLastIndex,flat,flatMap,includes,keys,toReversed,toSorted,toSpliced,values:at:true"
        ))
    );
}

#[test]
fn with_unscopables_assignment_uses_outer_binding() {
    let src = r#"
        let x = 1;
        let o = { x: 10 };
        o[Symbol.unscopables] = { x: true };
        with (o) {
            x = 2;
        }
        o.x + ":" + x;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("10:2")));
}

#[test]
fn with_unscopables_destructuring_assignment_uses_outer_binding() {
    let src = r#"
        var x = "outer";
        var o = { x: "inner" };
        o[Symbol.unscopables] = { x: true };
        with (o) {
            ({ a: x } = { a: "assigned" });
        }
        o.x + ":" + x + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("inner:assigned:true")));
}

#[test]
fn with_destructuring_assignment_uses_outer_binding_when_property_absent() {
    let src = r#"
        var x = "outer";
        var o = {};
        with (o) {
            ({ a: x } = { a: "assigned" });
        }
        o.hasOwnProperty("x") + ":" + o.x + ":" + x;
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("false:undefined:assigned"))
    );
}

#[test]
fn with_destructuring_assignment_uses_inherited_object_binding() {
    let src = r#"
        var x = "outer";
        var proto = { x: "inherited" };
        var o = Object.create(proto);
        with (o) {
            ({ a: x } = { a: "assigned" });
        }
        proto.x + ":" + o.x + ":" + x + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("inherited:assigned:outer:true"))
    );
}

#[test]
fn with_destructuring_identifier_target_preserves_reference_before_source_get() {
    let src = r#"
        var x = "outer";
        var o = { x: "inner" };
        var source = {
            get a() {
                delete o.x;
                return "assigned";
            }
        };
        with (o) {
            ({ a: x } = source);
        }
        o.x + ":" + x + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("assigned:outer:true")));
}

#[test]
fn with_array_destructuring_identifier_target_preserves_reference_before_iterator_step() {
    let src = r#"
        var x = "outer";
        var o = { x: "inner" };
        var iterable = {
            [Symbol.iterator]: function() {
                return {
                    next: function() {
                        delete o.x;
                        return { value: "assigned", done: false };
                    }
                };
            }
        };
        with (o) {
            [x] = iterable;
        }
        o.x + ":" + x + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("assigned:outer:true")));
}

#[test]
fn with_destructuring_identifier_target_preserves_reference_before_default() {
    let src = r#"
        var x = "outer";
        var o = { x: "inner" };
        function fallback() {
            delete o.x;
            return "assigned";
        }
        with (o) {
            ({ a: x = fallback() } = { a: undefined });
        }
        o.x + ":" + x + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("assigned:outer:true")));
}

#[test]
fn with_unscopables_for_in_identifier_head_uses_outer_binding() {
    let src = r#"
        var x = "outer";
        var o = { x: "inner" };
        o[Symbol.unscopables] = { x: true };
        with (o) {
            for (x in { assigned: 1 }) {}
        }
        o.x + ":" + x;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("inner:assigned")));
}

#[test]
fn with_unscopables_for_of_identifier_head_uses_outer_binding() {
    let src = r#"
        var x = "outer";
        var o = { x: "inner" };
        o[Symbol.unscopables] = { x: true };
        with (o) {
            for (x of ["assigned"]) {}
        }
        o.x + ":" + x;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("inner:assigned")));
}

#[test]
fn with_unscopables_getter_not_referenced_when_property_absent() {
    let src = r#"
        let calls = 0;
        let x = 7;
        let o = {};
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() {
                calls += 1;
                return { x: true };
            }
        });
        let r;
        with (o) {
            r = x;
        }
        r + ":" + calls;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("7:0")));
}

#[test]
fn with_unscopables_primitive_value_is_ignored() {
    let src = r#"
        let marker = {};
        let o = { x: marker };
        o[Symbol.unscopables] = "x";
        let r;
        with (o) {
            r = x === marker;
        }
        r;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn with_unscopables_getter_error_propagates_when_property_exists() {
    let err = run_err(
        r#"
        let o = { x: 1 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() { throw new Error("boom"); }
        });
        with (o) {
            x;
        }
        "#,
    );
    assert!(err.contains("boom"));
}

#[test]
fn with_unscopables_update_expression_checks_once() {
    let src = r#"
        let calls = 0;
        let x = 1;
        let o = { x: 10 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() {
                calls += 1;
                return { x: false };
            }
        });
        with (o) {
            x++;
        }
        o.x + ":" + x + ":" + calls;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("11:1:1")));
}

#[test]
fn with_unscopables_deleted_binding_then_strict_get_throws() {
    let err = run_err(
        r#"
        let calls = 0;
        let o = { x: 1 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() {
                calls += 1;
                delete o.x;
                return null;
            }
        });
        with (o) {
            (function() {
                "use strict";
                x;
            })();
        }
        "#,
    );
    assert!(err.contains("ReferenceError"));
}

#[test]
fn with_unscopables_deleted_binding_then_strict_set_throws() {
    let err = run_err(
        r#"
        let o = { x: 1 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() {
                delete o.x;
                return null;
            }
        });
        with (o) {
            (function() {
                "use strict";
                x = 2;
            })();
        }
        "#,
    );
    assert!(err.contains("ReferenceError"));
}

#[test]
fn with_delete_identifier_deletes_object_environment_binding() {
    let src = r#"
        var o = { x: 2 };
        var deleted;
        with (o) {
            deleted = delete x;
        }
        deleted + ":" + o.hasOwnProperty("x");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true:false")));
}

#[test]
fn with_delete_identifier_uses_inherited_object_environment_binding() {
    let src = r#"
        var x = 1;
        var proto = { x: 2 };
        var o = Object.create(proto);
        var deleted;
        with (o) {
            deleted = delete x;
        }
        deleted + ":" + x + ":" + o.hasOwnProperty("x") + ":" + proto.x;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true:1:false:2")));
}

#[test]
fn with_delete_identifier_honors_unscopables() {
    let src = r#"
        var x = 1;
        var o = { x: 2 };
        o[Symbol.unscopables] = { x: true };
        var deleted;
        with (o) {
            deleted = delete x;
        }
        deleted + ":" + x + ":" + o.hasOwnProperty("x") + ":" + o.x;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("false:1:true:2")));
}

#[test]
fn with_delete_identifier_propagates_unscopables_getter_error() {
    let err = run_err(
        r#"
        var o = { x: 2 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() { throw new Error("boom"); }
        });
        with (o) {
            delete x;
        }
        "#,
    );
    assert!(err.contains("boom"), "got: {err}");
}

#[test]
fn with_var_initializer_resolves_binding_before_rhs() {
    let src = r#"
        var obj = { test262id: 1 };
        with (obj) {
          var test262id = delete obj.test262id;
        }
        obj.test262id + ":" + test262id;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true:undefined")));
}

#[test]
fn with_outer_var_unchanged_after_block() {
    let src = r#"
        let p = { name: "inner" };
        let name = "outer";
        with (p) { name; }
        name;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("outer")));
}

#[test]
fn with_falls_back_to_outer_scope() {
    // Property only on outer object, not on the `with` object.
    let src = r#"
        let o = { a: 1 };
        let b = 99;
        let r;
        with (o) { r = a + b; }
        r;
    "#;
    assert_eq!(run(src), Value::Number(100.0));
}

#[test]
fn with_reads_function_value() {
    // The `with` object exposes a function-typed property that can be read
    // and called directly (no `this` dependence).
    let src = r#"
        let o = {
            getAnswer: function() { return 42; }
        };
        let r;
        with (o) {
            r = getAnswer();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(42.0));
}

#[test]
fn with_sees_undefined_valued_property() {
    // A property whose value is `undefined` must still be found by `with`
    // (regression for the old undefined-sentinel has_property check).
    let src = r#"
        let o = { x: undefined, real: 5 };
        let r;
        with (o) {
            // x exists (own property) even though its value is undefined.
            r = (typeof x === "undefined") + "|" + real;
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true|5")));
}

#[test]
fn with_typeof_uses_object_environment_binding() {
    let src = r#"
        var x = 1;
        var o = { x: function(){} };
        var r;
        with (o) {
            r = typeof x;
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("function")));
}

#[test]
fn with_typeof_uses_inherited_object_environment_binding() {
    let src = r#"
        var proto = { x: 1 };
        var o = Object.create(proto);
        var r;
        with (o) {
            r = typeof x;
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("number")));
}

#[test]
fn with_typeof_honors_unscopables_and_observable_getter() {
    let src = r#"
        var calls = 0;
        var x = "outer";
        var o = { x: "inner" };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() {
                calls += 1;
                return { x: true };
            }
        });
        var r;
        with (o) {
            r = typeof x;
        }
        r + ":" + calls;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("string:1")));
}

#[test]
fn with_typeof_propagates_unscopables_getter_error() {
    let err = run_err(
        r#"
        var o = { x: 1 };
        Object.defineProperty(o, Symbol.unscopables, {
            get: function() { throw new Error("boom"); }
        });
        with (o) {
            typeof x;
        }
        "#,
    );
    assert!(err.contains("boom"), "got: {err}");
}

// ---- `with` rebinding of `this` for unqualified calls (#6) ----

#[test]
fn with_unqualified_call_binds_this_to_object() {
    // `with(o){ getThis() }` binds `this` to `o` when `getThis` is found on `o`.
    let src = r#"
        let o = { x: 42, getThis: function() { return this.x; } };
        let r;
        with (o) {
            r = getThis();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(42.0));
}

#[test]
fn with_unqualified_call_this_is_object() {
    // Inside the with-block call, `this` is the with object itself.
    let src = r#"
        let o = { whoami: function() { return this; } };
        let r;
        with (o) {
            r = whoami() === o;
        }
        r;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn with_this_does_not_leak_to_outer_call() {
    // A plain unqualified call outside the with-block keeps `this` as the
    // global object in sloppy mode; the with-rebinding must not leak past
    // the block. Inside the block the call rebinds to `o`, outside it is
    // the global object.
    let src = r#"
        let o = { tag: "with-obj", f: function() { return this.tag; } };
        function g() { return (this === undefined) ? "none" : "leaked"; }
        let inside;
        with (o) { inside = f(); }
        let outside = g();
        inside + "|" + outside;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("with-obj|leaked")));
}

#[test]
fn with_this_not_set_when_name_not_on_object() {
    // If the called name is NOT a property of the with object, `this` is the
    // global object in sloppy mode (the function is resolved lexically, not
    // via the with object).
    let src = r#"
        function g() { return this === globalThis; }
        let o = { x: 1 };
        let r;
        with (o) {
            r = g();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn with_global_fallback_keeps_strict_unqualified_call_this_undefined() {
    let src = r#"
        function globalStrictReceiver() { "use strict"; return this; }
        var o = {};
        var result;
        with (o) {
            result = globalStrictReceiver() === undefined;
        }
        result;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn with_this_nested_inner_object_wins() {
    // Nested with-blocks: the innermost object that has the property provides
    // `this`.
    let src = r#"
        let outer = { tag: "outer", f: function() { return this.tag; } };
        let inner = { tag: "inner", f: function() { return this.tag; } };
        let r;
        with (outer) {
            with (inner) {
                r = f();
            }
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("inner")));
}

#[test]
fn with_this_uses_outer_when_inner_lacks_property() {
    // If the inner with object lacks the property, the outer one supplies both
    // the function and the `this` binding.
    let src = r#"
        let outer = { tag: "outer", f: function() { return this.tag; } };
        let inner = { other: 1 };
        let r;
        with (outer) {
            with (inner) {
                r = f();
            }
        }
        r;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("outer")));
}

#[test]
fn with_this_method_call_still_binds_receiver() {
    // `obj.method()` (qualified) must keep binding `this` to `obj`, unaffected
    // by an enclosing with-block.
    let src = r#"
        let o = { x: 7 };
        let receiver = { x: 99, m: function() { return this.x; } };
        let r;
        with (o) {
            r = receiver.m();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(99.0));
}

#[test]
fn with_this_function_reads_property_via_this() {
    // The function resolved via `with` can read other properties through `this`.
    let src = r#"
        let o = { a: 3, b: 4, sum: function() { return this.a + this.b; } };
        let r;
        with (o) {
            r = sum();
        }
        r;
    "#;
    assert_eq!(run(src), Value::Number(7.0));
}

#[test]
fn with_sequence_callee_loses_object_environment_this() {
    // The comma operator applies GetValue to its operands, so the final call is
    // not a direct IdentifierReference call and must not inherit the with-object
    // this binding.
    let src = r#"
        let o = { tag: "with", f: function() { return this === o ? "bad" : "ok"; } };
        with (o) {
            (0, f)();
        }
    "#;
    assert_eq!(run(src), Value::String(Arc::from("ok")));
}

#[test]
fn with_conditional_callee_loses_object_environment_this() {
    let src = r#"
        let o = { tag: "with", f: function() { return this === o ? "bad" : "ok"; } };
        with (o) {
            (true ? f : f)();
        }
    "#;
    assert_eq!(run(src), Value::String(Arc::from("ok")));
}

#[test]
fn with_logical_callee_loses_object_environment_this() {
    let src = r#"
        let o = { tag: "with", f: function() { return this === o ? "bad" : "ok"; } };
        with (o) {
            (f && f)();
        }
    "#;
    assert_eq!(run(src), Value::String(Arc::from("ok")));
}

#[test]
fn with_statement_normal_completion_value() {
    assert_eq!(run("1; with ({}) { }"), Value::Undefined);
    assert_eq!(run("2; with ({}) { 3; }"), Value::Number(3.0));
}

#[test]
fn with_statement_abrupt_empty_completion_value() {
    assert_eq!(
        run("1; do { 2; with ({}) { 3; break; } 4; } while (false);"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("5; do { 6; with ({}) { break; } 7; } while (false);"),
        Value::Undefined
    );
    assert_eq!(
        run("8; do { 9; with ({}) { 10; continue; } 11; } while (false);"),
        Value::Number(10.0)
    );
    assert_eq!(
        run("12; do { 13; with ({}) { continue; } 14; } while (false);"),
        Value::Undefined
    );
}

#[test]
fn with_single_statement_rejects_let_array_expression_start() {
    let err = run_err(
        r#"
        if (false) {
            with ({}) let
            [a] = 0;
        }
        "#,
    );
    assert!(err.contains("SyntaxError"));
}

#[test]
fn with_deleted_binding_on_typed_array_numeric_proto_is_noop() {
    assert_eq!(
        run(r#"
            var ta = new Int32Array(10);
            var env = Object.create(ta);
            Object.defineProperty(env, "NaN", { configurable: true, value: 100 });
            with (env) {
                NaN = (delete env.NaN, 0);
            }
            Object.getOwnPropertyDescriptor(env, "NaN") === undefined;
        "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            var ta = new Int32Array(10);
            var env = Object.create(ta);
            Object.defineProperty(env, "NaN", { configurable: true, value: 100 });
            with (env) {
                NaN += (delete env.NaN, 0);
            }
            Object.getOwnPropertyDescriptor(env, "NaN") === undefined;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn strict_with_deleted_binding_on_typed_array_numeric_proto_throws() {
    let err = run_err(
        r#"
        var ta = new Int32Array(10);
        var env = Object.create(ta);
        Object.defineProperty(env, "NaN", { configurable: true, value: 100 });
        with (env) {
            (function() {
                "use strict";
                NaN = (delete env.NaN, 0);
            })();
        }
        "#,
    );
    assert!(err.contains("ReferenceError"));
}
