//! ES2015 features: class/extends/super, template literals, default/rest
//! params, destructuring, for-of/for-in, spread, Map/Set/Symbol.

mod common;
use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

#[test]
fn class_basic() {
    let src = r#"
        class Point {
            constructor(x, y) { this.x = x; this.y = y; }
            sum() { return this.x + this.y; }
        }
        let p = new Point(3, 4);
        p.sum();
    "#;
    assert_eq!(run(src), Value::Number(7.0));
}

#[test]
fn class_constructor_field() {
    assert_eq!(
        run("class A { constructor(x) { this.x = x; } } new A(42).x;"),
        Value::Number(42.0)
    );
}

#[test]
fn class_extends() {
    assert_eq!(
        run("class A{f(){return 7;}} class B extends A{} new B().f();"),
        Value::Number(7.0)
    );
}

#[test]
fn super_call() {
    assert_eq!(
        run("class A{f(){return 10;}} class B extends A{f(){return super.f()+5;}} new B().f();"),
        Value::Number(15.0)
    );
}

#[test]
fn static_method() {
    assert_eq!(
        run("class C{static s(){return 42;}} C.s();"),
        Value::Number(42.0)
    );
}

#[test]
fn template_literal() {
    assert_eq!(
        run(r#"let n=5; `n=${n}`;"#),
        Value::String(Arc::from("n=5"))
    );
}

#[test]
fn template_multi() {
    assert_eq!(
        run(r#"let a=1,b=2; `${a}+${b}=${a+b}`;"#),
        Value::String(Arc::from("1+2=3"))
    );
}

#[test]
fn default_param() {
    assert_eq!(
        run("function f(a,b=10){return a+b;} f(5);"),
        Value::Number(15.0)
    );
}

#[test]
fn default_param_override() {
    assert_eq!(
        run("function f(a,b=10){return a+b;} f(5,20);"),
        Value::Number(25.0)
    );
}

#[test]
fn rest_param() {
    assert_eq!(
        run("function f(...a){return a.length;} f(1,2,3);"),
        Value::Number(3.0)
    );
}

#[test]
fn rest_param_after_fixed() {
    assert_eq!(
        run("function f(a, ...r){return r[0]+r[1];} f(1,2,3);"),
        Value::Number(5.0)
    );
}

#[test]
fn arrow_default_param() {
    assert_eq!(run("((a,b=5)=>a+b)(3);"), Value::Number(8.0));
}

#[test]
fn array_destructure() {
    assert_eq!(run("let [a,b]=[1,2]; a+b;"), Value::Number(3.0));
}

#[test]
fn object_destructure() {
    assert_eq!(run("let {x,y}={x:1,y:2}; x+y;"), Value::Number(3.0));
}

#[test]
fn object_destructure_rename() {
    assert_eq!(run("let {a:p,b:q}={a:10,b:20}; p+q;"), Value::Number(30.0));
}

#[test]
fn destructure_default() {
    assert_eq!(run("let {x=5} = {}; x;"), Value::Number(5.0));
}

#[test]
fn destructure_rest() {
    assert_eq!(
        run("let [a, ...rest] = [1,2,3,4]; rest.length;"),
        Value::Number(3.0)
    );
}

#[test]
fn for_of_destructure() {
    assert_eq!(
        run("let s=0; for(let [k,v] of [['a',1]]){s+=v;} s;"),
        Value::Number(1.0)
    );
}

#[test]
fn for_of_array() {
    assert_eq!(
        run("let s=0; for(let x of [1,2,3]){s+=x;} s;"),
        Value::Number(6.0)
    );
}

#[test]
fn for_of_string() {
    assert_eq!(
        run("let s=''; for(let c of 'abc'){s+=c;} s;"),
        Value::String(Arc::from("abc"))
    );
    assert_eq!(
        run(r#"let s=''; for(let c of "\uD801\uDC28"){s+=c;} s.length;"#),
        Value::Number(2.0)
    );
    assert_eq!(
        run(r#"let count=0; for(let c of "\uD801\uDC28"){count++;} count;"#),
        Value::Number(1.0)
    );
}

#[test]
fn for_in_object() {
    // for-in key order over a HashMap-backed object is not guaranteed; check membership.
    let s = run("let s=''; for(let k in {a:1,b:2}){s+=k;} s;");
    match s {
        Value::String(st) => {
            assert!(
                st.contains('a') && st.contains('b') && st.len() == 2,
                "got {st:?}"
            );
        }
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn array_spread_literal() {
    assert_eq!(run("[1, ...[2,3], 4].length;"), Value::Number(4.0));
    assert_eq!(
        run(r#"[..."hi"].join("");"#),
        Value::String(Arc::from("hi"))
    );
}

#[test]
fn map_basic() {
    assert_eq!(
        run("let m = new Map(); m.set('a', 1); m.get('a');"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("let m = new Map(); m.set('x', 1); m.set('y', 2); m.size;"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("let m = new Map(); m.set('a', 1); m.has('a');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("let m = new Map(); m.set('a', 1); m.delete('a'); m.has('a');"),
        Value::Bool(false)
    );
}

#[test]
fn set_basic() {
    assert_eq!(
        run("let s = new Set(); s.add(1); s.add(2); s.add(1); s.size;"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("let s = new Set(); s.add(1); s.has(1);"),
        Value::Bool(true)
    );
}

#[test]
fn symbol_type() {
    assert_eq!(run("typeof Symbol();"), Value::String(Arc::from("symbol")));
}

#[test]
fn symbol_to_string() {
    assert_eq!(
        run("Symbol('x').toString();"),
        Value::String(Arc::from("Symbol()"))
    );
}

#[test]
fn call_spread() {
    assert_eq!(
        run("function f(a,b,c){return a+b+c;} f(...[1,2,3]);"),
        Value::Number(6.0)
    );
}

#[test]
fn call_spread_mixed() {
    assert_eq!(
        run("function f(a,b,c){return a+b+c;} f(1, ...[2,3]);"),
        Value::Number(6.0)
    );
}

#[test]
fn derived_class_auto_super() {
    assert_eq!(
        run("class A{constructor(x){this.x=x;}} class B extends A{} new B(5).x;"),
        Value::Number(5.0)
    );
}

#[test]
fn derived_class_super_method() {
    assert_eq!(
        run("class A{constructor(x){this.x=x;} get(){return this.x;}} class B extends A{get(){return super.get()+10;}} new B(5).get();"),
        Value::Number(15.0)
    );
}

#[test]
fn explicit_super_constructor() {
    assert_eq!(
        run("class A{constructor(x){this.x=x;}} class B extends A{constructor(x){super(x); this.y=x*2;}} new B(5).y;"),
        Value::Number(10.0)
    );
}

// ---- Symbol-keyed properties ----

#[test]
fn symbol_key_store_and_read() {
    let src = r#"
        let it = Symbol.iterator;
        let o = {};
        o[it] = 42;
        o[it];
    "#;
    assert_eq!(run(src), Value::Number(42.0));
}

#[test]
fn symbol_key_not_in_for_in() {
    // Symbol-keyed properties must be skipped by for...in (string keys only).
    let src = r#"
        let it = Symbol.iterator;
        let o = { a: 1, b: 2 };
        o[it] = 99;
        let sum = 0;
        for (let k in o) { sum += o[k]; }
        sum;
    "#;
    assert_eq!(run(src), Value::Number(3.0));
}

#[test]
fn symbol_key_not_in_json_stringify() {
    let src = r#"
        let it = Symbol.iterator;
        let o = { a: 1 };
        o[it] = 99;
        JSON.stringify(o);
    "#;
    assert_eq!(run(src), Value::String(Arc::from("{\"a\":1}")));
}

#[test]
fn symbol_key_survives_round_trip() {
    let src = r#"
        let s1 = Symbol();
        let o = {};
        o[s1] = "hi";
        let out = o[s1];
        out;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("hi")));
}

// ---- custom Symbol.iterator ----

#[test]
fn custom_symbol_iterator_for_of() {
    let src = r#"
        let range = {
            [Symbol.iterator]() {
                let n = 0;
                return {
                    next() {
                        n++;
                        if (n <= 3) return { value: n, done: false };
                        return { value: undefined, done: true };
                    }
                };
            }
        };
        let r = [];
        for (let v of range) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1,2,3")));
}

#[test]
fn custom_symbol_iterator_spread() {
    let src = r#"
        let range = {
            [Symbol.iterator]() {
                let n = 0;
                return {
                    next() {
                        n++;
                        if (n <= 5) return { value: n * 10, done: false };
                        return { value: undefined, done: true };
                    }
                };
            }
        };
        [...range].join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("10,20,30,40,50")));
}

#[test]
fn custom_symbol_iterator_infinite_truncated() {
    let src = r#"
        let counter = {
            [Symbol.iterator]() {
                let n = 0;
                return { next() { n++; return { value: n, done: false }; } };
            }
        };
        let r = [];
        for (let v of counter) {
            if (v > 4) break;
            r.push(v);
        }
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1,2,3,4")));
}

#[test]
fn builtin_array_still_iterable() {
    // Regression: built-in iterables must keep working after Symbol.iterator support.
    let src = r#"
        let r = [];
        for (let v of [10, 20, 30]) r.push(v);
        r.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("10,20,30")));
}

#[test]
fn array_for_of_observes_live_length_changes() {
    assert_eq!(
        run("var a=[0,1]; var out=''; for (var v of a) { out += v; a.pop(); } out;"),
        Value::String(Arc::from("0"))
    );
    assert_eq!(
        run("var a=[0]; var out=''; for (var v of a) { out += v; if (v === 0) a.push(1); } out;"),
        Value::String(Arc::from("01"))
    );
}

#[test]
fn array_for_of_reads_accessor_indices_lazily() {
    let err = common::run_err(
        "var a=[]; Object.defineProperty(a, '0', { get: function(){ throw new Error('hit'); }}); for (var v of a) {}",
    );
    assert!(err.contains("hit") || err.contains("Error"), "got {err}");
}

#[test]
fn arguments_for_of_observes_mutation_and_sloppy_parameter_mapping() {
    assert_eq!(
        run("(function(){ 'use strict'; var out=''; var i=0; for (var v of arguments) { out += v; i++; arguments[i] *= 2; } return out; })(1,2,3);"),
        Value::String(Arc::from("146"))
    );
    assert_eq!(
        run("(function(a,b,c){ var out=''; var i=0; for (var v of arguments) { a=b; b=c; c=i; out += v; i++; } return out; })(1,2,3);"),
        Value::String(Arc::from("131"))
    );
}

#[test]
fn for_of_allows_async_as_lhs_identifier_name() {
    assert_eq!(
        run("var async = { x: 0 }; for (async.x of [1]) {} async.x;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("let async; for ((async) of [7]) {} async;"),
        Value::Number(7.0)
    );
    assert_eq!(
        run("let async; for (\\u0061sync of [7]) {} async;"),
        Value::Number(7.0)
    );
}

#[test]
fn for_of_lexical_head_tdz_and_iteration_scope() {
    let msg = run_err("let x = 1; for (let x of [x]) {}");
    assert!(
        msg.contains("Cannot access 'x' before initialization"),
        "got: {}",
        msg
    );

    assert_eq!(
        run("var value; for (let [x] of [[34]]) { value = x; } typeof x + ':' + value;"),
        Value::String(Arc::from("undefined:34"))
    );

    assert_eq!(
        run("let x = 'outside'; var probeDecl, probeBody; for (let [x, _ = probeDecl = function(){ return x; }] of [['inside']]) probeBody = function(){ return x; }; probeDecl() + ':' + probeBody() + ':' + x;"),
        Value::String(Arc::from("inside:inside:outside"))
    );

    assert_eq!(
        run("let x = 'outside'; var probeExpr; for (let x of (probeExpr = function(){ try { typeof x; return 'no'; } catch (e) { return e.name; } }, [])) ; probeExpr();"),
        Value::String(Arc::from("ReferenceError"))
    );
}

#[test]
fn computed_key_in_object_literal() {
    let src = r#"
        let key = "dynamic";
        let o = { [key]: 42, normal: 1 };
        o["dynamic"] + o.normal;
    "#;
    assert_eq!(run(src), Value::Number(43.0));
}

#[test]
fn object_literal_computed_key_before_value() {
    let src = r#"
        let value = "bad";
        let key = { toString() { value = "ok"; return "p"; } };
        let obj = { [key]: value };
        obj.p;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("ok")));
}

#[test]
fn computed_accessor_key_to_property_key_errors() {
    let err = common::run_err("let badKey = Object.create(null); ({ get [badKey]() {} });");
    assert!(
        err.contains("Cannot convert object to primitive value") || err.contains("TypeError"),
        "expected ToPropertyKey TypeError, got {err}"
    );
}

#[test]
fn object_methods_are_not_constructors() {
    let err = common::run_err("let obj = { method() {} }; new obj.method();");
    assert!(
        err.contains("not a constructor") || err.contains("TypeError"),
        "expected method constructor TypeError, got {err}"
    );
}

#[test]
fn object_methods_do_not_have_own_prototype() {
    let src = "let method = { method() {} }.method; Object.prototype.hasOwnProperty.call(method, 'prototype');";
    assert_eq!(run(src), Value::Bool(false));
}

#[test]
fn ordinary_functions_and_generator_methods_keep_own_prototype() {
    assert_eq!(
        run("function ordinary() {} Object.prototype.hasOwnProperty.call(ordinary, 'prototype');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("let method = { *method() {} }.method; Object.prototype.hasOwnProperty.call(method, 'prototype');"),
        Value::Bool(true)
    );
}

#[test]
fn object_accessors_bind_super() {
    let src = r#"
        let proto = {
            get value() { return 40; },
            set value(v) { this.seen = v + 1; }
        };
        let obj = {
            __proto__: proto,
            get value() { return super.value + 2; },
            set value(v) { super.value = v; }
        };
        let got = obj.value;
        obj.value = 4;
        got + obj.seen;
    "#;
    assert_eq!(run(src), Value::Number(47.0));
}

#[test]
fn object_super_get_uses_receiver() {
    let src = r#"
        let proto = { get x() { return this._x; } };
        let object = {
            __proto__: proto,
            _x: 9,
            get x() { return super.x; }
        };
        object.x;
    "#;
    assert_eq!(run(src), Value::Number(9.0));
}

#[test]
fn object_methods_reject_super_call() {
    for src in [
        "({ method(){ super(); } });",
        "({ get x(){ super(); } });",
        "({ set x(v){ super(); } });",
    ] {
        let err = common::run_err(src);
        assert!(
            err.contains("super call") || err.contains("SyntaxError"),
            "{err}"
        );
    }
}

#[test]
fn object_proto_duplicate_colon_is_syntax_error() {
    let err = common::run_err("({ __proto__: null, other: null, '__proto__': null });");
    assert!(
        err.contains("Duplicate __proto__") || err.contains("SyntaxError"),
        "{err}"
    );
}

#[test]
fn computed_and_shorthand_proto_are_data_properties() {
    let computed = r#"
        let proto = {};
        let ownProp = {};
        let obj = { __proto__: proto, ['__proto__']: {}, ['__proto__']: ownProp };
        Object.getPrototypeOf(obj) === proto && obj.__proto__ === ownProp;
    "#;
    assert_eq!(run(computed), Value::Bool(true));

    let shorthand = r#"
        let __proto__ = 2;
        let obj = { __proto__, __proto__ };
        obj.hasOwnProperty("__proto__") && obj.__proto__ === 2;
    "#;
    assert_eq!(run(shorthand), Value::Bool(true));
}

#[test]
fn array_prototype_iterator_override_honored() {
    let src = r#"
        Array.prototype[Symbol.iterator] = function() {
            let i = 0; let self = this;
            return { next() {
                if (i < self.length) { let v = self[i]*10; i++; return {value: v, done: false}; }
                return {value: undefined, done: true};
            }};
        };
        let r = [];
        for (let v of [1,2,3]) r.push(v);
        delete Array.prototype[Symbol.iterator];
        let r2 = [];
        for (let v of [1,2,3]) r2.push(v);
        r.join(",") + "|" + r2.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("10,20,30|1,2,3")));
}
