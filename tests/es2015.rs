//! ES2015 features: class/extends/super, template literals, default/rest
//! params, destructuring, for-of/for-in, spread, Map/Set/Symbol.

mod common;
use common::{run, run_err};
use ruja::{Value, Vm};
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
fn object_accessor_parameter_grammar_is_enforced() {
    for src in [
        "({ get value(param = 1) {} });",
        "({ set value() {} });",
        "({ set value(a, b) {} });",
        "({ set value(...items) {} });",
    ] {
        let err = run_err(src);
        assert!(
            err.contains("SyntaxError"),
            "expected SyntaxError for {src}, got {err}"
        );
    }

    assert_eq!(
        run("var o = { set value(v = 3) { this.result = v; } }; o.value = undefined; o.result;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("var o = { set value({ result }) { this.result = result; } }; o.value = { result: 4 }; o.result;"),
        Value::Number(4.0)
    );
}

#[test]
fn class_name_may_be_await_in_script_goal() {
    assert_eq!(run("class await {} 1;"), Value::Number(1.0));
    assert_eq!(
        run("var C = class await {}; C.name;"),
        Value::String(Arc::from("await"))
    );
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
fn class_extends_reads_superclass_prototype_once() {
    assert_eq!(
        run("var calls=0; var Base=function(){}.bind(); Object.defineProperty(Base,'prototype',{get:function(){calls++; return null;}, configurable:true}); class C extends Base{} calls;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var calls=0; var Base=function(){}.bind(); Object.defineProperty(Base,'prototype',{get:function(){calls++; return 42;}, configurable:true}); try{class C extends Base{}}catch(e){} calls;"),
        Value::Number(1.0)
    );
}

#[test]
fn class_extends_null_uses_function_prototype_parent() {
    assert_eq!(
        run("class C extends null{} Object.getPrototypeOf(C.prototype) === null && Object.getPrototypeOf(C) === Function.prototype && C.prototype.constructor === C;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class C extends null{constructor(){return Object.create(null);}} Object.getPrototypeOf(new C()) === null;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var before=0, after=0; class C extends null{constructor(){before++; super(); after++;}} try{new C();}catch(e){} before + ':' + after;"),
        Value::String(Arc::from("1:0"))
    );
    assert!(
        run_err("class C extends null{constructor(){super();}} new C();").contains("TypeError")
    );
}

#[test]
fn bound_class_construction_ignores_bound_this() {
    assert_eq!(
        run("class Base{constructor(x,y){this.x=x; this.y=y;}} class Sub extends Base{} var f=Sub.bind({bad:true},1); var s=new f(2); [s.x,s.y,Object.getPrototypeOf(s)===Sub.prototype].join(',');"),
        Value::String(Arc::from("1,2,true"))
    );
    assert!(
        run_err("class Base{} class Sub extends Base{} var f=Sub.bind({}); f();")
            .contains("TypeError")
    );
}

#[test]
fn derived_constructor_returns_bound_super_this() {
    assert_eq!(
        run("class Base{constructor(a,b){var o=new Object(); o.prp=a+b; return o;}} class Sub extends Base{constructor(){super(1,2);}} new Sub().prp;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("class Base{constructor(a,b){var o=new Object(); o.prp=a+b; return o;}} class Sub extends Base{constructor(){super(1,2); var called=false; function tmp(){called=true; return 3;} var exn=null; try{super(tmp(),4);}catch(e){exn=e;} this.ok=called && exn instanceof ReferenceError;}} var s=new Sub(); [s.prp,s.ok].join(',');"),
        Value::String(Arc::from("3,true"))
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
fn class_super_property_uses_home_object_prototype() {
    assert_eq!(
        run("class A{constructor(){this.s=super.toString();}} new A().s === Object.prototype.toString.call(new A());"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class A{m(){super.x = 1;}} var a = new A(); a.m(); a.x;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("class C{m(){return super.x;}} Object.setPrototypeOf(C.prototype,{x:2}); new C().m();"),
        Value::Number(2.0)
    );
}

#[test]
fn static_class_super_property_uses_super_constructor() {
    assert_eq!(
        run("class B{static get x(){return 2;} static method(){return 1;}} class C extends B{static method(){return super.x + super.method();}} C.method();"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("class B{static get x(){return 2;} static method(){return 1;}} class C extends B{static get x(){return super.x + super.method();}} C.x;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("class B{static get x(){return 2;} static method(){return 1;}} class C extends B{static set x(v){this.y = v + super.x + super.method();}} C.x = 3; C.y;"),
        Value::Number(6.0)
    );
    assert_eq!(
        run("class B{} B.x=7; class C{static m(){return super.x;}} Object.setPrototypeOf(C,B); C.m();"),
        Value::Number(7.0)
    );
    assert_eq!(
        run("var count=0; class C{static m(){try{super.x=count+=1;}catch(e){return (e instanceof TypeError)+':'+count;}}} Object.setPrototypeOf(C,null); C.m();"),
        Value::String(Arc::from("true:1"))
    );
    assert_eq!(
        run("var count=0; class C{static m(){try{super[0]=count+=1;}catch(e){return (e instanceof TypeError)+':'+count;}}} Object.setPrototypeOf(C,null); C.m();"),
        Value::String(Arc::from("true:1"))
    );
}

#[test]
fn super_reference_observes_key_then_base_then_property_key_coercion() {
    assert_eq!(
        run(r#"
            var initial = { value: "initial" };
            var duringKey = { value: "during-key" };
            var duringCoercion = { value: "during-coercion" };
            var key = {
              toString: function() {
                Object.setPrototypeOf(home, duringCoercion);
                return "value";
              }
            };
            var home = {
              read() {
                return super[(Object.setPrototypeOf(home, duringKey), key)];
              }
            };
            Object.setPrototypeOf(home, initial);
            home.read();
            "#),
        Value::String(Arc::from("during-key"))
    );
}

#[test]
fn super_reference_rejects_null_base_before_property_key_coercion() {
    assert_eq!(
        run(r#"
            var log = [];
            var key = {
              toString: function() { log.push("key"); return "missing"; }
            };
            var home = {
              readString() { return super[key]; },
              readSymbol() { return super[Symbol.iterator]; },
              optionalCall() { return super[Symbol.iterator]?.(log.push("argument")); }
            };
            Object.setPrototypeOf(home, null);
            for (var name of ["readString", "readSymbol", "optionalCall"]) {
              try { home[name](); } catch (error) { log.push(error.name); }
            }
            log.join("|");
            "#),
        Value::String(Arc::from("TypeError|TypeError|TypeError"))
    );
}

#[test]
fn super_reference_preserves_primitive_this_value() {
    assert_eq!(
        run(r#"
            var parent = {
              get value() { "use strict"; return typeof this + ":" + this; }
            };
            var home = {
              read() { "use strict"; return super.value; }
            };
            Object.setPrototypeOf(home, parent);
            home.read.call("base");
            "#),
        Value::String(Arc::from("string:base"))
    );
}

#[test]
fn nested_object_method_uses_its_own_home_object() {
    assert_eq!(
        run(r#"
            var outerParent = { value: "outer" };
            class Outer {
              make() {
                var nested = {
                  read() { return super.value; }
                };
                Object.setPrototypeOf(nested, { value: "nested" });
                return nested.read;
              }
            }
            Object.setPrototypeOf(Outer.prototype, outerParent);
            Outer.prototype.make()();
            "#),
        Value::String(Arc::from("nested"))
    );
}

#[test]
fn copying_method_does_not_replace_its_home_object() {
    assert_eq!(
        run(r#"
            var original = {
              read() { return super.value; }
            };
            Object.setPrototypeOf(original, { value: "original" });
            var copy = { read: original.read };
            Object.setPrototypeOf(copy, { value: "copy" });
            original.read() + ":" + copy.read();
            "#),
        Value::String(Arc::from("original:original"))
    );
}

#[test]
fn super_reference_roots_base_and_this_across_observable_calls() {
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
            var receiver;
            var receiverChecks = [];
            var target = {
              get value() {
                receiverChecks.push(this === receiver);
                forceGc();
                return this.marker;
              },
              method: function(ignored, value) {
                "use strict";
                forceGc();
                return this.marker + value;
              },
              tag: function() {
                "use strict";
                forceGc();
                return this.marker;
              }
            };
            var parent = new Proxy(target, {
              get: function(object, property, actualReceiver) {
                receiverChecks.push(actualReceiver === receiver);
                forceGc();
                return Reflect.get(object, property, actualReceiver);
              }
            });
            var home = {
              read() { return super.value; },
              call() { return super.method(forceGc(), 2); },
              spread() { return super.method(...[forceGc(), 3]); },
              optional() { return super.method?.(forceGc(), 4); },
              tagged() { return super.tag`value:${forceGc()}`; }
            };
            Object.setPrototypeOf(home, parent);
            receiver = Object.create(home);
            receiver.marker = 10;
            [
              receiver.read(),
              receiver.call(),
              receiver.spread(),
              receiver.optional(),
              receiver.tagged(),
              receiverChecks.every(function(value) { return value; })
            ].join(":");
            "#
        )
        .expect("super References should root base and thisValue"),
        Value::String(Arc::from("10:12:13:14:10:true"))
    );
}

#[test]
fn super_reference_writes_use_actual_this_across_assignment_forms() {
    assert_eq!(
        run(r#"
            var symbol = Symbol("value");
            var parent = { x: 1, y: 0, z: 0, w: 1 };
            parent[symbol] = 2;
            var home = {
              simple(value) { return super.x = value; },
              compound() { return super.x += 2; },
              logical() { return super.y ||= 4; },
              shortCircuit() { return super.x ||= 99; },
              postfix() { return super.w++; },
              prefix() { return ++super[symbol]; },
              destructure(source) {
                ({ value: super.z } = source);
                return this.z;
              }
            };
            Object.setPrototypeOf(home, parent);
            var receiver = Object.create(home);
            [
              receiver.simple(7), receiver.x,
              receiver.compound(), receiver.x,
              receiver.logical(), receiver.y,
              receiver.shortCircuit(), receiver.x,
              receiver.postfix(), receiver.w,
              receiver.prefix(), receiver[symbol],
              receiver.destructure({ value: 8 }),
              Object.prototype.hasOwnProperty.call(home, "x")
            ].join(":");
            "#),
        Value::String(Arc::from("7:7:3:3:4:4:1:3:1:2:3:3:8:false"))
    );
}

#[test]
fn super_simple_assignment_captures_base_before_rhs_and_key_coercion() {
    assert_eq!(
        run(r#"
            var log = [];
            var receiver;
            var duringKey = {
              set value(value) {
                log.push("setter:" + (this === receiver) + ":" + value);
              }
            };
            var duringRhs = {};
            var duringCoercion = {};
            var key = {
              toString: function() {
                log.push("coerce");
                Object.setPrototypeOf(home, duringCoercion);
                return "value";
              }
            };
            var home = {
              write() {
                return super[(log.push("key"), Object.setPrototypeOf(home, duringKey), key)] =
                  (log.push("rhs"), Object.setPrototypeOf(home, duringRhs), 7);
              }
            };
            receiver = Object.create(home);
            var result = receiver.write();
            log.push("result:" + result);
            log.join("|");
            "#),
        Value::String(Arc::from("key|rhs|coerce|setter:true:7|result:7"))
    );
}

#[test]
fn super_simple_assignment_checks_null_base_after_rhs_before_key_coercion() {
    assert_eq!(
        run(r#"
            var log = [];
            var key = {
              toString: function() { log.push("coerce"); return "value"; }
            };
            var home = {
              write() {
                try {
                  super[key] = (log.push("rhs"), 1);
                } catch (error) {
                  log.push(error.name);
                }
              }
            };
            Object.setPrototypeOf(home, null);
            home.write();
            log.join("|");
            "#),
        Value::String(Arc::from("rhs|TypeError"))
    );
}

#[test]
fn computed_super_read_modify_write_coerces_each_key_once() {
    assert_eq!(
        run(r#"
            var coercions = 0;
            var key = {
              toString: function() { coercions++; return "value"; }
            };
            var parent = { value: 1 };
            var home = {
              compound() { return super[key] += 2; },
              logical() { return super[key] ||= 9; },
              update() { return super[key]++; }
            };
            Object.setPrototypeOf(home, parent);
            var receiver = Object.create(home);
            [
              receiver.compound(), receiver.value, coercions,
              receiver.logical(), receiver.value, coercions,
              receiver.update(), receiver.value, coercions
            ].join(":");
            "#),
        Value::String(Arc::from("3:3:1:1:3:2:1:2:3"))
    );
}

#[test]
fn super_reference_write_preserves_primitive_receiver() {
    assert_eq!(
        run(r#"
            var observed;
            var parent = {
              set value(value) {
                "use strict";
                observed = typeof this + ":" + this + ":" + value;
              }
            };
            var home = {
              write() { "use strict"; super.value = 9; }
            };
            Object.setPrototypeOf(home, parent);
            home.write.call("base");
            observed;
            "#),
        Value::String(Arc::from("string:base:9"))
    );
}

#[test]
fn deferred_super_reference_names_survive_gc() {
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
            var receiver;
            var values = [];
            var parent = {
              set value(value) { values.push((this === receiver) + ":" + value); }
            };
            var home = {
              simple() {
                super[{ toString: function() { forceGc(); return "value"; } }] =
                  (forceGc(), 11);
              },
              destructure(source) {
                ({ item: super[{ toString: function() { forceGc(); return "value"; } }] } = source);
              }
            };
            Object.setPrototypeOf(home, parent);
            receiver = Object.create(home);
            receiver.simple();
            receiver.destructure({ get item() { forceGc(); return 12; } });
            values.join("|");
            "#,
        )
        .expect("super reference GC regression should run"),
        Value::String(Arc::from("true:11|true:12"))
    );
}

#[test]
fn super_object_key_to_symbol_survives_gc_and_coerces_once() {
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
            var coercions = 0;
            var symbol = Symbol("super-key");
            var keyTarget = {
                [Symbol.toPrimitive]: function() {
                    forceGc();
                    coercions++;
                    return this === key ? symbol : "wrong";
                }
            };
            var key = new Proxy(keyTarget, {
                get: function(target, property, receiver) {
                    forceGc();
                    return Reflect.get(target, property, receiver);
                }
            });
            var parent = {};
            Object.defineProperty(parent, symbol, {
                get: function() { return this.current; },
                set: function(value) { this.current = value; }
            });
            var home = {
                update() { return super[key] += (forceGc(), 2); }
            };
            Object.setPrototypeOf(home, parent);
            var receiver = Object.create(home);
            receiver.current = 1;
            receiver.update() + ":" + receiver.current + ":" + coercions;
            "#,
        )
        .expect("super object key should preserve Symbol and receiver across GC"),
        Value::String(Arc::from("3:3:1"))
    );
}

#[test]
fn class_super_capture_is_per_class() {
    assert_eq!(
        run("class B{static get x(){return 2;}} class C extends B{static m(){return super.x;}} class D{} C.m();"),
        Value::Number(2.0)
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
fn class_empty_elements_and_computed_generator_method() {
    assert_eq!(
        run("class A{method(){return 1;} static method(){return 2;} ;} A.method() + new A().method();"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("class A{*[1](){yield 42;}} new A()[1]().next().value;"),
        Value::Number(42.0)
    );
}

#[test]
fn class_computed_accessor_names() {
    assert_eq!(
        run("var k='x'; class A{get [k](){return 3;} set [k](v){this.y=v+1;}} var a=new A(); var before=a.x; a.x=4; before + a.y;"),
        Value::Number(8.0)
    );
    assert_eq!(
        run("var k='x'; class A{static get [k](){return 5;} static set [k](v){this.y=v+1;}} var before=A.x; A.x=6; before + A.y;"),
        Value::Number(12.0)
    );
    assert_eq!(
        run("var s = Symbol(); class A{[s](){return 9;} static [s](){return 4;}} new A()[s]() + A[s]();"),
        Value::Number(13.0)
    );
    assert_eq!(
        run("var s = Symbol(); class A{get [s](){return 5;} static get [s](){return 6;}} new A()[s] + A[s];"),
        Value::Number(11.0)
    );
}

#[test]
fn class_accessor_getter_results_are_not_cached() {
    assert_eq!(
        run("class C{static get x(){return this._x;} static set x(v){this._x=v;}} C._x=3; var a=C.x; C._x=4; a + C.x;"),
        Value::Number(7.0)
    );
}

#[test]
fn named_class_expression_uses_inner_immutable_binding() {
    assert_eq!(
        run("var C='outside'; var cls=class C{method(){return C;}}; [C, cls.prototype.method() === cls].join(',');"),
        Value::String(Arc::from("outside,true"))
    );
    assert!(run_err(
        "var C='outside'; var cls=class C{method(){C=null;}}; cls.prototype.method();"
    )
    .contains("Assignment to constant variable"));
    assert_eq!(
        run("var probe; var C='outside'; var cls=class C extends (probe=function(){return C;}, Object){}; [C, probe() === cls].join(',');"),
        Value::String(Arc::from("outside,true"))
    );
}

#[test]
fn class_declaration_name_is_immutable_inside_body() {
    assert!(run_err("class C{constructor(){C=42;}} new C();")
        .contains("Assignment to constant variable"));
    assert!(run_err("class C{m(){C=42;}} new C().m();").contains("Assignment to constant variable"));
}

#[test]
fn class_bodies_make_nested_functions_strict() {
    assert_eq!(
        run("var r; class C{constructor(){try{(function(){missing=1;})(); r='no';}catch(e){r=e.constructor.name;}}} new C(); r;"),
        Value::String(Arc::from("ReferenceError"))
    );
}

#[test]
fn anonymous_class_assignment_infers_constructor_name() {
    assert_eq!(
        run("var E = class {}; E.name;"),
        Value::String(Arc::from("E"))
    );
    assert_eq!(
        run("var F = class { constructor() {} }; F.name;"),
        Value::String(Arc::from("F"))
    );
}

#[test]
fn class_methods_named_eval_arguments_override_restricted_function_props() {
    assert_eq!(
        run("class C{eval(){return 1;} arguments(){return 2;} static eval(){return 3;} static arguments(){return 4;}} [new C().eval(), new C().arguments(), C.eval(), C.arguments()].join(',');"),
        Value::String(Arc::from("1,2,3,4"))
    );
}

#[test]
fn class_method_names_do_not_shadow_outer_bindings() {
    assert_eq!(
        run("var x; class C{set x(v){x=v;}} new C().x=1; x;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var x=1; class C{m(){x=2;}} new C().m(); x;"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("var f=function inner(n){return n ? inner(n-1)+1 : 0;}; f(2);"),
        Value::Number(2.0)
    );
}

#[test]
fn class_name_yield_is_rejected() {
    assert!(run_err("var C = class yield {};").contains("SyntaxError"));
    assert!(run_err(r#"var C = class yi\u0065ld {};"#).contains("SyntaxError"));
    assert!(run_err("class yield {}").contains("SyntaxError"));
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
fn template_unicode_scalars_preserve_utf16_identity() {
    assert_eq!(
        run(concat!(
            "function inspect(strings) {",
            " return [strings[0].length, strings.raw[0].length,",
            " strings[0] === String.fromCodePoint(0xF0000),",
            " strings.raw[0] === String.fromCodePoint(0xF0000)].join('|');",
            "}",
            "inspect`\u{F0000}`;"
        )),
        Value::String(Arc::from("2|2|true|true"))
    );
    assert_eq!(
        run("[`\\u{F0000}`.length, `\\u{F0000}` === String.fromCodePoint(0xF0000)].join('|');"),
        Value::String(Arc::from("2|true"))
    );
    assert_eq!(
        run("[`\\uDB80\\uDC00`.length, `\\uDB80\\uDC00` === String.fromCodePoint(0xF0000)].join('|');"),
        Value::String(Arc::from("2|true"))
    );
    assert_eq!(
        run(concat!(
            "function inspectEscape(strings) {",
            " return [strings[0].length, strings.raw[0].length,",
            " strings[0] === String.fromCodePoint(0xF0000),",
            " strings.raw[0].charCodeAt(0),",
            " strings.raw[0].slice(1) === String.fromCodePoint(0xF0000)]",
            " .join('|');",
            "}",
            "inspectEscape`\\",
            "\u{F0000}",
            "`;"
        )),
        Value::String(Arc::from("2|3|true|92|true"))
    );
}

#[test]
fn tagged_template_member_preserves_this() {
    assert_eq!(
        run("var context; var obj={fn:function(){context=this;}}; obj.fn`x`; context===obj;"),
        Value::Bool(true)
    );
}

#[test]
fn tagged_templates_use_reference_this_values() {
    assert_eq!(
        run(r#"
            var scope = {};
            scope.tag = function() { "use strict"; return this === scope; };
            var withResult;
            with (scope) { withResult = tag`with`; }

            var log = [];
            var symbol = Symbol("tag");
            var method = function(strings, value) {
              "use strict";
              return (this === proxy) + ":" + strings[0] + ":" + value;
            };
            var target = { tag: method };
            target[symbol] = method;
            var proxy = new Proxy(target, {
              get: function(object, property, receiver) {
                log.push("get:" + (property === symbol ? "symbol" : property));
                return Reflect.get(object, property, receiver);
              }
            });
            var key = {
              toString: function() {
                log.push("key");
                return "tag";
              }
            };
            var memberResult = proxy[key]`member:${2}`;
            var symbolResult = proxy[symbol]`symbol:${3}`;

            String.prototype.tag = function() {
              "use strict";
              return typeof this + ":" + this;
            };
            var primitiveResult = "base".tag`primitive`;

            var plainThis = "unset";
            function plainTag() { "use strict"; plainThis = this; }
            plainTag`plain`;
            var holder = {
              tag: function() { "use strict"; return this; }
            };
            var groupedResult = (holder.tag)`grouped` === holder;
            var unboundResult = (0, holder.tag)`unbound` === undefined;

            [
              withResult,
              memberResult,
              symbolResult,
              primitiveResult,
              plainThis === undefined,
              groupedResult,
              unboundResult,
              log.join("|")
            ].join(";");
            "#),
        Value::String(Arc::from(
            "true;true:member::2;true:symbol::3;string:base;true;true;true;key|get:tag|get:symbol"
        ))
    );

    assert_eq!(
        run(r#"
            var superReceiver;
            class Base {
              get tag() {
                superReceiver = this;
                return function(strings, value) { return this.marker + value; };
              }
            }
            class Derived extends Base {
              constructor() { super(); this.marker = 10; }
              callTag() {
                var result = super.tag`super:${2}`;
                return result + ":" + (superReceiver === this);
              }
            }
            class PrivateTag {
              constructor() { this.marker = 20; }
              #tag(strings, value) { return this.marker + value; }
              callTag() { return this.#tag`private:${3}`; }
            }
            new Derived().callTag() + ":" + new PrivateTag().callTag();
            "#),
        Value::String(Arc::from("12:true:23"))
    );
}

#[test]
fn tagged_template_reference_roots_temporary_base_during_interpolation() {
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
            var order = [];
            function makeBase() {
              return {
                marker: 17,
                make() {
                  return function() { "use strict"; return this === undefined; };
                },
                get tag() {
                  order.push("get");
                  return function() {
                    order.push("call");
                    return this.marker;
                  };
                }
              };
            }
            class PrivateBase {
              constructor() { this.marker = 23; }
              #tag() { return this.marker; }
              static call(make) {
                return (make()?.#tag)`private:${(order.push("private-expression"), forceGc())}`;
              }
            }
            var computed = (makeBase()?.["tag"])
              `value:${(order.push("computed-expression"), forceGc())}`;
            var privateResult = PrivateBase.call(function() { return new PrivateBase(); });
            var callResult = (makeBase()?.make())
              `call:${(order.push("call-expression"), forceGc())}`;
            [computed, privateResult, callResult, order.join("|")].join(":");
            "#
        )
        .expect("tagged template should keep its temporary base alive"),
        Value::String(Arc::from(
            "17:23:true:get|computed-expression|call|private-expression|call-expression"
        ))
    );
}

#[test]
fn parenthesized_optional_chain_tagged_templates_preserve_references() {
    assert_eq!(
        run(r#"
            var log = [];
            var holder = {
              tag: function() { "use strict"; return this === holder; },
              nested: {
                tag: function() { "use strict"; return this === holder.nested; }
              },
              make: function() {
                return function() { "use strict"; return this === undefined; };
              }
            };
            var key = "tag";
            class C {
              #tag() { return this; }
              call() { return (this?.#tag)`private` === this; }
            }
            var nullishError;
            try { (null?.tag)`nullish:${log.push("interpolation")}`; }
            catch (error) { nullishError = error.name; }
            var oldTag = holder.tag;
            var snapshot = (holder?.tag)
              `snapshot:${(holder.tag = function() { return false; }, 0)}`;
            holder.tag = oldTag;
            [
              (holder?.tag)`member`,
              (holder?.[key])`computed`,
              (holder?.nested.tag)`nested`,
              (holder?.make())`value`,
              new C().call(),
              snapshot,
              nullishError,
              log.join("|")
            ].join(";");
            "#),
        Value::String(Arc::from(
            "true;true;true;true;true;true;TypeError;interpolation"
        ))
    );

    assert_eq!(
        run(r#"
            var log = [];
            var throwing = {
              get tag() { log.push("getter"); throw "getter-error"; }
            };
            var nonCallable = { tag: 1 };
            var called = false;
            var holder = { tag() { called = true; } };
            function boom() { log.push("interpolation-throw"); throw "boom"; }
            var getterError;
            var callableError;
            var interpolationError;
            try { (throwing?.tag)`x:${log.push("getter-interpolation")}`; }
            catch (error) { getterError = error; }
            try { (nonCallable?.tag)`x:${log.push("noncallable-interpolation")}`; }
            catch (error) { callableError = error.name; }
            try { (holder?.tag)`x:${boom()}`; }
            catch (error) { interpolationError = error; }
            [
              getterError, callableError, interpolationError, called,
              log.join("|")
            ].join(";");
            "#),
        Value::String(Arc::from(
            "getter-error;TypeError;boom;false;getter|noncallable-interpolation|interpolation-throw"
        ))
    );
}

#[test]
fn new_tagged_template_constructs_tag_result() {
    assert_eq!(
        run("function C(x){arg=x;} var tag=function(x){templateObject=x; return C;}; var arg=null, templateObject; var instance = new tag`first`; instance instanceof C && templateObject[0] === 'first' && arg === undefined;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("function C(x){arg=x;} var tag=function(x){templateObject=x; return C;}; var arg=null, templateObject; var instance = new tag`second`('arg'); instance instanceof C && templateObject[0] === 'second' && arg === 'arg';"),
        Value::Bool(true)
    );
}

#[test]
fn conditional_then_branch_allows_in_inside_for_head() {
    assert_eq!(
        run("var c1=0,c2=0; function one(){c1+=1; return {};} function two(){c2+=1;} for (true ? '' in one() : two(); false;) ; c1 === 1 && c2 === 0;"),
        Value::Bool(true)
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
fn rest_parameter_functions_use_unmapped_arguments_object() {
    assert_eq!(
        run("function f(a, ...rest){ arguments[0] = 1; arguments[1] = 2; return a + ':' + rest.join(','); } f(3,4,5);"),
        Value::String(Arc::from("3:4,5"))
    );
}

#[test]
fn computed_symbol_accessor_setter_is_called() {
    assert_eq!(
        run("var calls=0; var s=Symbol(); var o={ set [s](v){ calls += v; } }; o[s]=3; calls;"),
        Value::Number(3.0)
    );
}

#[test]
fn arrow_default_param() {
    assert_eq!(run("((a,b=5)=>a+b)(3);"), Value::Number(8.0));
}

#[test]
fn arrow_uses_lexical_arguments_binding() {
    assert_eq!(
        run("function f(){ var args = arguments; var af = _ => arguments; return args === af(); } f(1,2);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("function f(){ var af = arguments => arguments; return af(7); } f(1,2);"),
        Value::Number(7.0)
    );
}

#[test]
fn arrow_uses_lexical_new_target_binding() {
    assert_eq!(
        run("function F(){ this.seen = (_ => new.target)() === F; } new F().seen;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("function F(){ return (_ => new.target)(); } F();"),
        Value::Undefined
    );
    assert_eq!(
        run("function F(){ this.af = _ => new.target === F; } var af = new F().af; af();"),
        Value::Bool(true)
    );
}

#[test]
fn arrow_functions_inherit_restricted_caller_arguments_accessors() {
    assert_eq!(
        run("var af = () => {}; af.hasOwnProperty('caller') + ':' + af.hasOwnProperty('arguments');"),
        Value::String(Arc::from("false:false"))
    );

    let msg = run_err("var af = () => {}; af.caller;");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("var af = () => {}; af.caller = {};");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("var af = () => {}; af.arguments;");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("var af = () => {}; af.arguments = {};");
    assert!(msg.contains("TypeError"), "got: {msg}");
}

#[test]
fn arrow_body_uses_lexical_super_property_binding() {
    assert_eq!(
        run("var A={x:1}; var B={}; Object.setPrototypeOf(B,A); var obj={m(){return (()=>{return super.x;})();}}; Object.setPrototypeOf(obj,B); obj.m();"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var A={x:1}; var B={}; Object.setPrototypeOf(B,A); var obj={m(){return (()=>{return super['x'];})();}}; Object.setPrototypeOf(obj,B); obj.m();"),
        Value::Number(1.0)
    );
    assert!(run_err("(()=>{ return super.x; })();").contains("SyntaxError"));
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
fn destructuring_binding_defaults_infer_function_names() {
    assert_eq!(
        run("var [fn = function(){}, arrow = () => {}, cls = class {}, cover = (function(){})] = []; [fn.name, arrow.name, cls.name, cover.name].join(':');"),
        Value::String(Arc::from("fn:arrow:cls:cover"))
    );
    assert_eq!(
        run("let {fn = function(){}, arrow = () => {}, cls = class {}, cover = (function(){})} = {}; [fn.name, arrow.name, cls.name, cover.name].join(':');"),
        Value::String(Arc::from("fn:arrow:cls:cover"))
    );
    assert_eq!(
        run("let k = 'missing'; let {a: renamed = function(){}, [k]: computed = function(){}, 0: numeric = function(){}} = {}; [renamed.name, computed.name, numeric.name].join(':');"),
        Value::String(Arc::from("renamed:computed:numeric"))
    );
    assert_eq!(
        run("var result; for (const [fn = function(){}] = []; ; ) { result = fn.name; break; } result;"),
        Value::String(Arc::from("fn"))
    );
}

#[test]
fn destructuring_binding_defaults_keep_existing_function_names() {
    assert_eq!(
        run("var [xFn = function x(){}, xCover = (0, function(){})] = []; [xFn.name, xCover.name].join(':');"),
        Value::String(Arc::from("x:"))
    );
    assert_eq!(
        run("const {xCls = class { static name() {} }} = {}; typeof xCls.name;"),
        Value::String(Arc::from("function"))
    );
    assert_eq!(
        run("let box = []; let {a: {name: extracted} = (box[0] = function(){})} = {}; box[0].name + ':' + extracted;"),
        Value::String(Arc::from(":"))
    );
}

#[test]
fn destructure_rest() {
    assert_eq!(
        run("let [a, ...rest] = [1,2,3,4]; rest.length;"),
        Value::Number(3.0)
    );
}

#[test]
fn object_rest_destructuring_assignment_targets() {
    assert_eq!(
        run("var holder = {}, a; ({ a, ...holder.rest } = { a: 1, b: 2 }); a + ':' + holder.rest.b;"),
        Value::String(Arc::from("1:2"))
    );

    assert_eq!(
        run(r#"
            var calls = [];
            var target = {};
            var proxy = new Proxy(target, {
              set: function(t, k, v, r) {
                calls.push(k + ":" + v.b + ":" + (r === proxy));
                t[k] = v;
                return true;
              }
            });
            var a;
            ({ a, ...proxy.rest } = { a: 1, b: 2 });
            a + ":" + target.rest.b + ":" + calls.join("|");
        "#),
        Value::String(Arc::from("1:2:rest:2:true"))
    );
}

#[test]
fn object_rest_assignment_boxes_string_sources() {
    assert_eq!(
        run(r#"
            var rest;
            var vals = "foo";
            var result = ({...rest} = vals);
            rest[0] + rest[1] + rest[2] + ":" + (rest instanceof Object) + ":" + result;
        "#),
        Value::String(Arc::from("foo:true:foo"))
    );
}

#[test]
fn object_rest_excludes_computed_keys_and_copies_symbols() {
    assert_eq!(
        run(r#"
            var key = "a";
            var v;
            var rest;
            ({ [key]: v, ...rest } = { a: 1, b: 2 });
            v + ":" + rest.a + ":" + rest.b;
        "#),
        Value::String(Arc::from("1:undefined:2"))
    );

    assert_eq!(
        run(r#"
            var s = Symbol("slot");
            var v;
            var rest;
            var source = { b: 2 };
            source[s] = 1;
            ({ [s]: v, ...rest } = source);
            v + ":" + rest[s] + ":" + rest.b + ":" + Object.getOwnPropertySymbols(rest).length;
        "#),
        Value::String(Arc::from("1:undefined:2:0"))
    );

    assert_eq!(
        run(r#"
            var s = Symbol("slot");
            var source = { a: 1 };
            source[s] = 2;
            var rest;
            ({ a: a, ...rest } = source);
            rest[s] + ":" + Object.getOwnPropertySymbols(rest).length;
        "#),
        Value::String(Arc::from("2:1"))
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
fn sloppy_var_destructuring_stays_in_the_function_variable_environment() {
    assert_eq!(
        run(r#"
            function read() {
                var {direct} = {direct: 2};
                for (var {loop} of [{loop: 3}]) {}
                return direct + loop;
            }
            read() + ":" + typeof direct + ":" + typeof loop;
        "#),
        Value::String("5:undefined:undefined".into())
    );
}

#[test]
fn for_of_assignment_destructure() {
    assert_eq!(
        run("var x; var c=0; for ([x] of [[0]]) { c += x + 1; } x + ':' + c;"),
        Value::String(Arc::from("0:1"))
    );
    assert_eq!(run("var x; for ({x} of [{x:2}]) {} x;"), Value::Number(2.0));
    assert_eq!(
        run("var x, c = 0; for ({x = 1} of [{}]) { c += x; } x + ':' + c;"),
        Value::String(Arc::from("1:1"))
    );
    assert_eq!(
        run("var fn; for ({fn = function(){}} of [{}]) {} fn.name;"),
        Value::String(Arc::from("fn"))
    );
    assert_eq!(
        run("var yield = 3, x; for ([{x = yield}] of [[{}]]) {} x;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("var x, prop, elem; for ([x = 'x' in {}] of [[]]) {} for ({ prop = 'x' in {}, key: elem = 'x' in {} } of [{key: undefined}]) {} [x, prop, elem].join(':');"),
        Value::String(Arc::from("false:false:false"))
    );
    assert_eq!(
        run(r#"
            var nextCount = 0, returnCount = 0, counter = 0, x;
            function Sentinel() {}
            var iterable = {};
            var iterator = {
              next: function() {
                nextCount += 1;
                throw new Sentinel();
              },
              return: function() {
                returnCount += 1;
                return {};
              }
            };
            iterable[Symbol.iterator] = function() { return iterator; };
            try {
              for ([x] of [iterable]) {
                counter += 1;
              }
              counter += 1;
            } catch (e) {}
            [counter, nextCount, returnCount].join(":");
        "#),
        Value::String(Arc::from("0:1:0"))
    );
    assert_eq!(
        run(r#"
            var nextCount = 0, returnCount = 0, counter = 0, x;
            function Sentinel() {}
            var iterable = {};
            var iterator = {
              next: function() {
                nextCount += 1;
                throw new Sentinel();
              },
              return: function() {
                returnCount += 1;
                return {};
              }
            };
            iterable[Symbol.iterator] = function() { return iterator; };
            try {
              for ([...x] of [iterable]) {
                counter += 1;
              }
              counter += 1;
            } catch (e) {}
            [counter, nextCount, returnCount].join(":");
        "#),
        Value::String(Arc::from("0:1:0"))
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
fn for_in_uses_spec_property_order() {
    assert_eq!(
        run("var o={p1:'p1',p2:'p2',p3:'p3'}; o.p4='p4'; o[2]='2'; o[0]='0'; o[1]='1'; delete o.p1; delete o.p3; o.p1='p1'; var keys=[]; for(var key in o){keys.push(key);} keys.join(',');"),
        Value::String(Arc::from("0,1,2,p2,p4,p1"))
    );
    assert_eq!(
        run("var proto={p2:'p2'}; var o=Object.create(proto,{p1:{value:'p1',enumerable:true},p2:{value:'own',enumerable:false}}); var keys=[]; for(var key in o){keys.push(key);} keys.join(',');"),
        Value::String(Arc::from("p1"))
    );
}

#[test]
fn for_in_enumeration_descriptor_edges() {
    assert_eq!(
        run("var proto = { prop: 1 }; function C(){} C.prototype = proto; var child = new C(); Object.defineProperty(child, 'prop', { value: 2, enumerable: false }); var seen = false; for (var k in child) { if (k === 'prop') seen = true; } seen;"),
        Value::Bool(false)
    );

    assert_eq!(
        run("var obj = Object.create(null); obj.aa = 1; obj.ba = 2; obj.ca = 3; var out = ''; for (var k in obj) { delete obj.ba; out += k + obj[k]; } out;"),
        Value::String(Arc::from("aa1ca3"))
    );

    assert_eq!(
        run("var obj = {}; obj.a = 1; obj.b = 2; Object.defineProperty(obj, 'a', { value: 11 }); var keys = []; for (var k in obj) keys.push(k); keys.join(',') + ':' + Object.prototype.propertyIsEnumerable.call(obj, 'a');"),
        Value::String(Arc::from("a,b:true"))
    );

    assert_eq!(
        run("var obj = {}; Object.defineProperty(obj, 'a', { get: function(){ return 1; }, enumerable: true, configurable: true }); obj.b = 2; Object.defineProperty(obj, 'a', { get: function(){ return 2; } }); var keys = []; for (var k in obj) keys.push(k); keys.join(',') + ':' + obj.a;"),
        Value::String(Arc::from("a,b:2"))
    );

    assert_eq!(
        run("var proto = { x: 1 }; var obj = Object.create(proto); obj.a = 1; obj.b = 2; var keys = []; for (var k in obj) { if (k === 'a') Object.defineProperty(obj, 'x', { value: 2, enumerable: false }); keys.push(k); } keys.join(',');"),
        Value::String(Arc::from("a,b,x"))
    );

    assert_eq!(
        run("var arr = [1, 2]; Object.defineProperty(arr, '0', { enumerable: false }); var keys = []; for (var k in arr) keys.push(k); keys.join(',');"),
        Value::String(Arc::from("1"))
    );
}

#[test]
fn for_in_observes_proxy_internal_methods_lazily() {
    assert_eq!(
        run(r#"
            var log = [];
            var proxy = new Proxy({ a: 1, b: 2 }, {
              ownKeys: function(target) {
                log.push("ownKeys");
                return Reflect.ownKeys(target);
              },
              getOwnPropertyDescriptor: function(target, key) {
                log.push("descriptor:" + key);
                return Reflect.getOwnPropertyDescriptor(target, key);
              },
              getPrototypeOf: function() {
                log.push("prototype");
                return null;
              }
            });
            for (var key in proxy) log.push("body:" + key);
            log.join(",");
        "#),
        Value::String(Arc::from(
            "ownKeys,descriptor:a,body:a,descriptor:b,body:b,prototype"
        ))
    );

    assert_eq!(
        run(r#"
            var log = [];
            var proxy = new Proxy({ a: 1, b: 2 }, {
              ownKeys: function(target) {
                log.push("ownKeys");
                return Reflect.ownKeys(target);
              },
              getOwnPropertyDescriptor: function(target, key) {
                log.push("descriptor:" + key);
                return Reflect.getOwnPropertyDescriptor(target, key);
              },
              getPrototypeOf: function() {
                log.push("prototype");
                return null;
              }
            });
            for (var key in proxy) {
              log.push("body:" + key);
              break;
            }
            log.join(",");
        "#),
        Value::String(Arc::from("ownKeys,descriptor:a,body:a"))
    );

    assert_eq!(
        run(r#"
            var forwarded = [];
            for (var key in new Proxy({ targetKey: 1 }, {})) forwarded.push(key);
            var fabricated = new Proxy({}, {
              ownKeys: function() { return ["second", "first"]; },
              getOwnPropertyDescriptor: function() {
                return { value: 1, enumerable: true, configurable: true };
              },
              getPrototypeOf: function() { return null; }
            });
            var custom = [];
            for (var customKey in fabricated) custom.push(customKey);
            forwarded.join(",") + "|" + custom.join(",");
        "#),
        Value::String(Arc::from("targetKey|second,first"))
    );

    assert_eq!(
        run(r#"
            var calls = 0;
            var cyclic;
            cyclic = new Proxy({}, {
              ownKeys: function() {
                calls += 1;
                return ["key" + calls];
              },
              getOwnPropertyDescriptor: function() {
                return { value: 1, enumerable: true, configurable: true };
              },
              getPrototypeOf: function() { return cyclic; }
            });
            var keys = [];
            for (var key in cyclic) {
              keys.push(key);
              if (keys.length === 3) break;
            }
            keys.join(",") + "|" + calls;
        "#),
        Value::String(Arc::from("key1,key2,key3|3"))
    );
}

#[test]
fn for_in_proxy_filters_and_shadowing_follow_descriptor_results() {
    assert_eq!(
        run(r#"
            var symbol = Symbol("ignored");
            var target = { a: 1 };
            target[symbol] = 2;
            var descriptorKeys = [];
            var proxy = new Proxy(target, {
              ownKeys: function() { return [symbol, "a"]; },
              getOwnPropertyDescriptor: function(target, key) {
                descriptorKeys.push(key === symbol ? "symbol" : key);
                return Reflect.getOwnPropertyDescriptor(target, key);
              },
              getPrototypeOf: function() { return null; }
            });
            var keys = [];
            for (var key in proxy) keys.push(key);
            keys.join(",") + "|" + descriptorKeys.join(",");
        "#),
        Value::String(Arc::from("a|a"))
    );

    assert_eq!(
        run(r#"
            var proto = Object.create(null);
            proto.x = 1;
            var hidden = new Proxy({}, {
              ownKeys: function() { return ["x"]; },
              getOwnPropertyDescriptor: function() {
                return { value: 2, enumerable: false, configurable: true };
              },
              getPrototypeOf: function() { return proto; }
            });
            var absent = new Proxy({}, {
              ownKeys: function() { return ["x"]; },
              getOwnPropertyDescriptor: function() { return undefined; },
              getPrototypeOf: function() { return proto; }
            });
            var hiddenKeys = [];
            var absentKeys = [];
            for (var hiddenKey in hidden) hiddenKeys.push(hiddenKey);
            for (var absentKey in absent) absentKeys.push(absentKey);
            hiddenKeys.join(",") + "|" + absentKeys.join(",");
        "#),
        Value::String(Arc::from("|x"))
    );
}

#[test]
fn for_in_proxy_revocation_and_abrupt_completions_propagate() {
    assert_eq!(
        run(r#"
            var state = Proxy.revocable({ a: 1, b: 2 }, {
              ownKeys: function(target) { return Reflect.ownKeys(target); },
              getOwnPropertyDescriptor: function(target, key) {
                return Reflect.getOwnPropertyDescriptor(target, key);
              },
              getPrototypeOf: function() { return null; }
            });
            var keys = [];
            var errorName = "none";
            try {
              for (var key in state.proxy) {
                keys.push(key);
                state.revoke();
              }
            } catch (error) {
              errorName = error.name;
            }
            keys.join(",") + "|" + errorName;
        "#),
        Value::String(Arc::from("a|TypeError"))
    );

    for trap in ["ownKeys", "getOwnPropertyDescriptor", "getPrototypeOf"] {
        let source = format!(
            r#"
                var marker = {{}};
                var caught;
                var handler = {{
                  ownKeys: function() {{ {own_keys} return ["key"]; }},
                  getOwnPropertyDescriptor: function() {{ {descriptor}
                    return {{ value: 1, enumerable: false, configurable: true }};
                  }},
                  getPrototypeOf: function() {{ {prototype} return null; }}
                }};
                try {{ for (var key in new Proxy({{}}, handler)) {{}} }}
                catch (error) {{ caught = error; }}
                caught === marker;
            "#,
            own_keys = if trap == "ownKeys" {
                "throw marker;"
            } else {
                ""
            },
            descriptor = if trap == "getOwnPropertyDescriptor" {
                "throw marker;"
            } else {
                ""
            },
            prototype = if trap == "getPrototypeOf" {
                "throw marker;"
            } else {
                ""
            },
        );
        assert_eq!(run(&source), Value::Bool(true), "trap: {trap}");
    }

    assert!(run_err(
        "for (var key in new Proxy({}, { ownKeys: function(){ return ['x', 'x']; } })) {}"
    )
    .contains("duplicate"));
}

#[test]
fn for_in_boxes_strings_and_uses_object_properties_not_collection_entries() {
    assert_eq!(
        run(r#"
            var keys = [];
            for (var key in "A\u{1D11E}") keys.push(key);
            for (var nullKey in null) keys.push("null:" + nullKey);
            for (var undefinedKey in undefined) keys.push("undefined:" + undefinedKey);
            keys.join(",");
        "#),
        Value::String(Arc::from("0,1,2"))
    );

    assert_eq!(
        run(r#"
            var map = new Map([["entry", 1]]);
            map.own = 2;
            var keys = [];
            for (var key in map) keys.push(key);
            keys.join(",");
        "#),
        Value::String(Arc::from("own"))
    );
}

#[test]
fn for_in_member_lhs_array_prototype_setter() {
    assert_eq!(
        run("var obj = Object.create(null); var let, value; obj.key = 1; for (let in obj); Object.defineProperty(Array.prototype, '1', { set: function(param) { value = param; }, configurable: true }); for ([let][1] in obj); delete Array.prototype[1]; value;"),
        Value::String(Arc::from("key"))
    );
}

#[test]
fn for_in_of_member_lhs_uses_property_reference() {
    assert_eq!(
        run(r#"
            var s = Symbol("slot");
            var calls = [];
            var target = {};
            var proxy = new Proxy(target, {
              set: function(t, k, v, r) {
                calls.push((k === s) + ":" + v + ":" + (r === proxy));
                t[k] = v;
                return true;
              }
            });
            for (proxy[s] in { name: 1 }) {}
            calls.join("|") + ";" + target[s] + ";" + proxy[s];
        "#),
        Value::String(Arc::from("true:name:true;name;name"))
    );

    assert_eq!(
        run(r#"
            var s = Symbol("slot");
            var calls = [];
            var target = {};
            var proxy = new Proxy(target, {
              set: function(t, k, v, r) {
                calls.push((k === s) + ":" + v + ":" + (r === proxy));
                t[k] = v;
                return true;
              }
            });
            for (proxy[s] of ["value"]) {}
            calls.join("|") + ";" + target[s] + ";" + proxy[s];
        "#),
        Value::String(Arc::from("true:value:true;value;value"))
    );
}

#[test]
fn define_property_redefinition_validation_edges() {
    assert_eq!(
        run("var obj = Object.freeze({ x: 1 }); var threw = false; try { Object.defineProperty(obj, 'x', { value: 2 }); } catch (e) { threw = true; } threw + ':' + obj.x;"),
        Value::String(Arc::from("true:1"))
    );

    assert_eq!(
        run("var obj = {}; Object.preventExtensions(obj); var threw = false; try { Object.defineProperty(obj, 'x', { value: 1 }); } catch (e) { threw = true; } threw + ':' + ('x' in obj);"),
        Value::String(Arc::from("true:false"))
    );

    assert_eq!(
        run("var get1 = function(){ return 1; }; var get2 = function(){ return 2; }; var obj = {}; Object.defineProperty(obj, 'x', { get: get1, configurable: false }); var threw = false; try { Object.defineProperty(obj, 'x', { get: get2 }); } catch (e) { threw = true; } threw + ':' + obj.x;"),
        Value::String(Arc::from("true:1"))
    );
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
        run("let m = new Map([[+0, 1]]); m.set(-0, 42); [m.get(+0), m.get(-0), m.size].join('|');"),
        Value::String(Arc::from("42|42|1"))
    );
    assert_eq!(
        run("let m = new Map([[-0, 1]]); m.set(+0, 42); [m.get(+0), m.get(-0), m.size, Object.is(m.keys().next().value, -0)].join('|');"),
        Value::String(Arc::from("42|42|1|false"))
    );
    assert_eq!(
        run("let d = Object.getOwnPropertyDescriptor(Map.prototype, 'size'); let m = new Map([[1, 2], [3, 4]]); [m.size, d.get.call(m), d.get.name, d.get.length, d.enumerable, d.configurable, typeof d.set].join('|');"),
        Value::String(Arc::from("2|2|get size|0|false|true|undefined"))
    );
    assert_eq!(
        run("let d = Object.getOwnPropertyDescriptor(Map.prototype, 'size'); [typeof d.value, typeof d.writable].join('|');"),
        Value::String(Arc::from("undefined|undefined"))
    );
    assert_eq!(
        run("Object.defineProperty(Map.prototype, 'size', { get: function(){ return 99; }, configurable: true }); new Map([[1, 2]]).size;"),
        Value::Number(99.0)
    );
    assert_eq!(
        run("delete Map.prototype.size; new Map([[1, 2]]).size;"),
        Value::Undefined
    );
    assert!(
        run_err("Object.getOwnPropertyDescriptor(Map.prototype, 'size').get.call({});")
            .contains("TypeError")
    );
    assert!(
        run_err("Object.getOwnPropertyDescriptor(Map.prototype, 'size').get.call(1);")
            .contains("TypeError")
    );
    assert!(
        run_err("Object.getOwnPropertyDescriptor(Map.prototype, 'size').get.call(new Set());")
            .contains("TypeError")
    );
    assert!(run_err(
        "Object.getOwnPropertyDescriptor(Map.prototype, 'size').get.call(new WeakMap());"
    )
    .contains("TypeError"));
    for src in [
        "Map.prototype.set.call({}, 'a', 1);",
        "Map.prototype.get.call({}, 'a');",
        "Map.prototype.has.call({}, 'a');",
        "Map.prototype.delete.call({}, 'a');",
        "Map.prototype.clear.call({});",
        "Map.prototype.entries.call({});",
        "Map.prototype.keys.call({});",
        "Map.prototype.values.call({});",
        "Map.prototype.forEach.call({}, function(){});",
        "Map.prototype.set.call(1, 'a', 1);",
        "Map.prototype.get.call(new Set(), 'a');",
    ] {
        assert!(run_err(src).contains("TypeError"), "{src}");
    }
    assert_eq!(
        run("let m = new Map(); m.set('a', 1); m.has('a');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("let m = new Map(); m.set('a', 1); m.delete('a'); m.has('a');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("let m = new Map([[1, 'a'], [2, 'b']]); let it = m.entries(); let a = it.next(); let b = it.next(); let c = it.next(); [a.value[0], a.value[1], b.value[0], b.value[1], c.done, it[Symbol.iterator]() === it].join('|');"),
        Value::String(Arc::from("1|a|2|b|true|true"))
    );
    assert_eq!(
        run(r#"
            let it = new Map([[1, 2]]).values();
            let proto = Object.getPrototypeOf(it);
            let d = Object.getOwnPropertyDescriptor(proto, "next");
            let tag = Object.getOwnPropertyDescriptor(proto, Symbol.toStringTag);
            [
              Object.prototype.hasOwnProperty.call(it, "next"),
              d.value.name, d.value.length, d.writable, d.enumerable, d.configurable,
              tag.value, tag.writable, tag.enumerable, tag.configurable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "false|next|0|true|false|true|Map Iterator|false|false|true"
        ))
    );
    assert!(
        run_err("Object.getPrototypeOf(new Map().values()).next.call({});").contains("TypeError")
    );
    assert_eq!(
        run("Map.prototype[Symbol.iterator] === Map.prototype.entries;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("let m = new Map([['foo', 0], ['bar', 1]]); let out = []; m.forEach(function(v, k){ if (k === 'foo') { m.delete('foo'); m.set('foo', 2); } out.push(k + ':' + v); }); out.join('|');"),
        Value::String(Arc::from("foo:0|bar:1|foo:2"))
    );
    assert_eq!(
        run("let m = Map.groupBy([1,2,3], function(v, i){ return i % 2 ? 'odd-index' : 'even-index'; }); [m instanceof Map, Array.from(m.keys()).join('|'), m.get('even-index').join(','), m.get('odd-index').join(',')].join(';');"),
        Value::String(Arc::from("true;even-index|odd-index;1,3;2"))
    );
    assert_eq!(
        run(r#"
            var IntrinsicMap = Map;
            var originalSet = IntrinsicMap.prototype.set;
            var setCalls = 0;
            var speciesCalls = 0;
            IntrinsicMap.prototype.set = function() {
              setCalls++;
              throw new Error("must not call set");
            };
            Object.defineProperty(IntrinsicMap, Symbol.species, {
              get: function() {
                speciesCalls++;
                throw new Error("must not get species");
              },
              configurable: true
            });
            globalThis.Map = function ReplacementMap() {};
            var grouped = IntrinsicMap.groupBy([1, 2], function() { return "k"; });
            var intrinsicPrototype =
              Object.getPrototypeOf(grouped) === IntrinsicMap.prototype;
            IntrinsicMap.prototype.set = originalSet;
            [setCalls, speciesCalls, intrinsicPrototype,
              grouped.get("k").join(",")].join("|");
        "#),
        Value::String(Arc::from("0|0|true|1,2"))
    );
    assert_eq!(
        run("let key = { toString(){ throw new Error('no-toPropertyKey'); } }; let m = Map.groupBy([1, '1', key], function(v){ return v; }); [m.get(1).join(','), m.get('1').join(','), m.get(key)[0] === key].join('|');"),
        Value::String(Arc::from("1|1|true"))
    );
    assert_eq!(
        run("let m = Map.groupBy([-0, +0], function(v){ return v; }); [m.size, Object.is(m.keys().next().value, -0), m.get(0).length].join('|');"),
        Value::String(Arc::from("1|false|2"))
    );
    assert_eq!(
        run("let m = Map.groupBy([NaN, NaN], function(v){ return v; }); [m.size, Number.isNaN(m.keys().next().value), m.get(NaN).length].join('|');"),
        Value::String(Arc::from("1|true|2"))
    );
    assert_eq!(
        run("let closed = 0; let iterable = {}; iterable[Symbol.iterator] = function(){ let i = 0; return { next(){ return { value: ++i, done: false }; }, return(){ closed++; return {}; } }; }; try { Map.groupBy(iterable, function(v){ if (v === 2) throw new Error('stop'); return 'k'; }); } catch (e) {} closed;"),
        Value::Number(1.0)
    );
    assert!(run_err("Map();").contains("TypeError"));
    assert!(run_err("Map([]);").contains("TypeError"));
    assert_eq!(
        run("let mapSet = Map.prototype.set; let calls = 0; Map.prototype.set = function(k, v){ calls++; return mapSet.call(this, k, v); }; let m = new Map([[1,2],[3,4]]); calls + '|' + m.get(3);"),
        Value::String(Arc::from("2|4"))
    );
    assert!(run_err("Map.prototype.set = null; new Map([[1,2]]);").contains("TypeError"));
    assert_eq!(
        run("let calls = 0; Object.defineProperty(Map.prototype, 'set', { get(){ calls++; throw new Error('no'); }, configurable: true }); try { new Map(); } catch (e) {} calls;"),
        Value::Number(0.0)
    );
    assert_eq!(
        run("let closed = 0; let iterable = {}; iterable[Symbol.iterator] = function(){ return { next(){ return { value: 1, done: false }; }, return(){ closed++; return {}; } }; }; try { new Map(iterable); } catch (e) {} closed;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("let oldSet = Map.prototype.set; let iterable = {}; iterable[Symbol.iterator] = function(){ return { next(){ return { value: [1, 2], done: false }; }, return(){ throw new Error('close-error'); } }; }; Map.prototype.set = function(){ throw new Error('set-error'); }; let message; try { new Map(iterable); } catch (e) { message = e.message; } Map.prototype.set = oldSet; message;"),
        Value::String(Arc::from("set-error"))
    );
    assert_eq!(
        run("let m = new Map([[1, 'one']]); [m.getOrInsert(1, 'x'), m.getOrInsert(2, 'two'), m.get(2), m.size].join('|');"),
        Value::String(Arc::from("one|two|two|2"))
    );
    assert_eq!(
        run("let m = new Map([[+0, 42]]); let seen; let out = m.getOrInsertComputed(-0, function(k){ seen = k; return 1; }); [out, Object.is(seen, undefined), m.get(0)].join('|');"),
        Value::String(Arc::from("42|true|42"))
    );
    assert_eq!(
        run("let m = new Map(); let seen; let out = m.getOrInsertComputed(-0, function(k){ seen = k; m.set(k, 'mutated'); return 'final'; }); [out, m.get(0), Object.is(seen, -0), Object.is(seen, +0)].join('|');"),
        Value::String(Arc::from("final|final|false|true"))
    );
    assert!(run_err("new Map().getOrInsertComputed(1, 1);").contains("TypeError"));
}

#[test]
fn map_constructor_observes_direct_iterator_close_and_realm_boundaries() {
    assert_eq!(
        run(r#"
            var log = [];
            var nextGets = 0;
            var nextReceiverWasIterator = false;
            var setGets = 0;
            var setReceiverWasMap = false;
            var step = 0;
            var originalSet = Map.prototype.set;
            Object.defineProperty(Map.prototype, "set", {
              configurable: true,
              get: function() {
                setGets++;
                log.push("get-set");
                return function(key, value) {
                  log.push("set:" + arguments.length);
                  setReceiverWasMap = this instanceof Map;
                  return originalSet.call(this, key, value);
                };
              }
            });
            var iterator = {};
            Object.defineProperty(iterator, "next", {
              configurable: true,
              get: function() {
                nextGets++;
                return function() {
                  nextReceiverWasIterator = this === iterator;
                  log.push("next:" + arguments.length);
                  Object.defineProperty(iterator, "next", {
                    value: function() { throw new Error("must use cached next"); },
                    configurable: true
                  });
                  if (step++ === 0) return { done: false, value: ["direct", 7] };
                  return { done: true };
                };
              }
            });
            var iterable = new Proxy({
              [Symbol.iterator]: function() {
                Object.defineProperty(Map.prototype, "set", {
                  value: function() { throw new Error("must use cached set"); },
                  writable: true, configurable: true
                });
                return iterator;
              }
            }, {
              has: function(target, key) { log.push("has"); return key in target; },
              get: function(target, key, receiver) {
                log.push(typeof key === "symbol" ? "get-symbol" : "get-" + key);
                return Reflect.get(target, key, receiver);
              }
            });
            var direct = new Map(iterable);
            Object.defineProperty(Map.prototype, "set", {
              value: originalSet, writable: true, configurable: true
            });

            var arraySource = [["array-original", 6]];
            arraySource[Symbol.iterator] = function() {
              return [["array-override", 7]][Symbol.iterator]();
            };
            var mapSource = new Map([["original", 1]]);
            mapSource[Symbol.iterator] = function() {
              return [["map-override", 2]][Symbol.iterator]();
            };
            var setSource = new Set([1]);
            setSource[Symbol.iterator] = function() {
              return [["set-override", 3]][Symbol.iterator]();
            };
            function* pairs() { yield ["generator-original", 4]; }
            var generator = pairs();
            generator[Symbol.iterator] = function() {
              return [["generator-override", 5]][Symbol.iterator]();
            };
            var fromArray = new Map(arraySource);
            var fromMap = new Map(mapSource);
            var fromSet = new Map(setSource);
            var fromGenerator = new Map(generator);
            [
              log.join(","), nextGets, nextReceiverWasIterator,
              setGets, setReceiverWasMap,
              direct.get("direct"),
              fromArray.get("array-override"), fromArray.has("array-original"),
              fromMap.has("map-override"), fromMap.has("original"),
              fromSet.get("set-override"),
              fromGenerator.get("generator-override"),
              fromGenerator.has("generator-original")
            ].join("|");
            "#,),
        Value::String(Arc::from(
            "get-set,get-symbol,next:0,set:2,next:0|1|true|1|true|7|7|false|true|false|3|5|false"
        ))
    );

    assert_eq!(
        run(
            r#"
            function closeCount(kind) {
              var closed = 0;
              var pair = {};
              if (kind === "key") Object.defineProperty(pair, "0", {
                get: function() { throw new Error("key-error"); }
              });
              else pair[0] = "key";
              if (kind === "value") Object.defineProperty(pair, "1", {
                get: function() { throw new Error("value-error"); }
              });
              else pair[1] = 1;
              var iterator = {
                next: function() {
                  if (kind === "next") throw new Error("next-error");
                  if (kind === "result") return 1;
                  if (kind === "done") return {
                    get done() { throw new Error("done-error"); }
                  };
                  if (kind === "step-value") return {
                    done: false,
                    get value() { throw new Error("step-value-error"); }
                  };
                  return { done: false, value: pair };
                },
                return: function() { closed++; throw new Error("close-error"); }
              };
              var iterable = { [Symbol.iterator]: function() { return iterator; } };
              var originalSet = Map.prototype.set;
              if (kind === "adder") Map.prototype.set = function() {
                throw new Error("adder-error");
              };
              var message;
              try { new Map(iterable); } catch (error) { message = error.message; }
              Map.prototype.set = originalSet;
              return kind + ":" + closed + ":" + message;
            }
            [
              closeCount("next"), closeCount("result"), closeCount("done"),
              closeCount("step-value"),
              closeCount("key"), closeCount("value"), closeCount("adder")
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "next:0:next-error|result:0:Iterator result is not an object|done:0:done-error|step-value:0:step-value-error|key:1:key-error|value:1:value-error|adder:1:adder-error"
        ))
    );

    assert_eq!(
        run(r#"
            function preservesOriginal(mode) {
              var original = { mode: mode };
              var pair = { get 0() { throw original; } };
              var iterator = {
                next: function() { return { done: false, value: pair }; }
              };
              if (mode === "null") iterator.return = null;
              if (mode === "getter") Object.defineProperty(iterator, "return", {
                get: function() { throw new Error("return-getter"); }
              });
              if (mode === "noncallable") iterator.return = 1;
              if (mode === "throw") iterator.return = function() {
                throw new Error("return-call");
              };
              if (mode === "primitive") iterator.return = function() { return 1; };
              if (mode === "object") iterator.return = function() { return {}; };
              var iterable = { [Symbol.iterator]: function() { return iterator; } };
              try { new Map(iterable); } catch (error) { return error === original; }
              return false;
            }
            ["absent", "null", "getter", "noncallable", "throw", "primitive", "object"]
              .map(preservesOriginal).join("|");
            "#,),
        Value::String(Arc::from("true|true|true|true|true|true|true"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var iterator = {
              next: function() { return { done: false, value: 1 }; },
              return: function() { return {}; }
            };
            var iterable = { [Symbol.iterator]: function() { return iterator; } };
            var error;
            try { new other.Map(iterable); } catch (caught) { error = caught; }
            [error instanceof other.TypeError, error instanceof TypeError].join("|");
            "#,),
        Value::String(Arc::from("true|false"))
    );
}

#[test]
fn map_group_by_observes_iterator_close_realm_and_gc_boundaries() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");

    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var effects = [];
            var hasCalls = 0;
            var iteratorGets = 0;
            var iteratorThis = false;
            var iteratorArgs = -1;
            var nextGets = 0;
            var nextThis = true;
            var nextArgs = [];
            var callbackThis = true;
            var callbackArgs = [];
            var kept = { marker: 7 };
            var key = {
              toString: function() { throw new Error("must not coerce"); }
            };
            var iteratorTarget = {
              count: 0,
              next: function() {
                nextThis = nextThis && this === iterator;
                nextArgs.push(arguments.length);
                effects.push("next:" + this.count);
                forceGc();
                return this.count++ === 0
                  ? { done: false, value: kept }
                  : { done: true };
              }
            };
            var iterator = new Proxy(iteratorTarget, {
              get: function(target, property, receiver) {
                if (property === "next") {
                  nextGets++;
                  effects.push("next-get");
                }
                return Reflect.get(target, property, receiver);
              }
            });
            var iterable = new Proxy({}, {
              has: function() { hasCalls++; return true; },
              get: function(target, property) {
                if (property === Symbol.iterator) {
                  iteratorGets++;
                  effects.push("iterator-get");
                  forceGc();
                  return function() {
                    iteratorThis = this === iterable;
                    iteratorArgs = arguments.length;
                    effects.push("iterator-call");
                    forceGc();
                    return iterator;
                  };
                }
                return target[property];
              }
            });
            var grouped = other.Map.groupBy(iterable, function(value, index) {
              "use strict";
              callbackThis = callbackThis && this === undefined;
              callbackArgs.push(arguments.length + ":" + index);
              effects.push("callback:" + index);
              forceGc();
              return key;
            });
            var values = grouped.get(key);
            [
              effects.join(","), hasCalls, iteratorGets, iteratorThis,
              iteratorArgs, nextGets, nextThis, nextArgs.join(":"),
              callbackThis, callbackArgs.join(":"), values[0] === kept,
              Object.getPrototypeOf(grouped) === other.Map.prototype,
              Object.getPrototypeOf(values) === other.Array.prototype,
              grouped instanceof other.Map, !(grouped instanceof Map),
              Object.getPrototypeOf(grouped.keys()) ===
                Object.getPrototypeOf(new other.Map().keys()),
              Object.getPrototypeOf(grouped.keys()) !==
                Object.getPrototypeOf(new Map().keys())
            ].join("|");
            "#,
        )
        .expect("Map.groupBy should preserve direct iterator and Realm roots"),
        Value::String(Arc::from(
            "iterator-get,iterator-call,next-get,next:0,callback:0,next:1|0|1|true|0|1|true|0:0|true|2:0|true|true|true|true|true|true|true"
        ))
    );

    assert_eq!(
        vm.run(
            r#"
            var OtherMap = other.Map;
            var OtherMapPrototype = OtherMap.prototype;
            var NewTarget = new other.Function();
            NewTarget.prototype = null;
            var BoundNewTarget = NewTarget.bind(null);
            other.Map = null;
            other.Object = null;
            forceGc();
            Object.getPrototypeOf(
              Reflect.construct(OtherMap, [], BoundNewTarget)
            ) === OtherMapPrototype;
            "#,
        )
        .expect("Map constructor fallback should use immutable foreign intrinsics"),
        Value::Bool(true)
    );

    assert_eq!(
        vm.run(
            r#"
            function closeCase(kind) {
              var original = {};
              var replacement = {};
              var calls = 0;
              var iterator = {
                next: function() { return { done: false, value: 1 }; }
              };
              if (kind === "null") iterator.return = null;
              if (kind === "getter") {
                Object.defineProperty(iterator, "return", {
                  get: function() { forceGc(); throw replacement; }
                });
              }
              if (kind === "noncallable") iterator.return = 1;
              if (kind === "throw") iterator.return = function() {
                calls++; forceGc(); throw replacement;
              };
              if (kind === "primitive") iterator.return = function() {
                calls++; forceGc(); return 1;
              };
              try {
                Map.groupBy(
                  { [Symbol.iterator]: function() { return iterator; } },
                  function() { throw original; }
                );
              } catch (error) {
                return (error === original) + ":" + calls;
              }
            }
            [
              closeCase("absent"), closeCase("null"), closeCase("getter"),
              closeCase("noncallable"), closeCase("throw"),
              closeCase("primitive")
            ].join(",");
            "#,
        )
        .expect("Map.groupBy close must preserve the callback error"),
        Value::String(Arc::from("true:0,true:0,true:0,true:0,true:1,true:1"))
    );

    assert_eq!(
        vm.run(
            r#"
            function noCloseCase(kind) {
              var original = {};
              var closed = 0;
              var iterator = { return: function() { closed++; return {}; } };
              if (kind === "uncallable") iterator.next = null;
              if (kind === "nonobject") iterator.next = function() { return null; };
              if (kind === "next") iterator.next = function() { throw original; };
              if (kind === "done") iterator.next = function() {
                return { get done() { throw original; } };
              };
              if (kind === "value") iterator.next = function() {
                return { done: false, get value() { throw original; } };
              };
              if (kind === "complete") iterator.next = function() {
                return { done: true, get value() { throw original; } };
              };
              try {
                var result = Map.groupBy(
                  { [Symbol.iterator]: function() { return iterator; } },
                  function() { return "k"; }
                );
                return (kind === "complete" && result.size === 0) + ":" + closed;
              } catch (error) {
                return (kind === "uncallable" || kind === "nonobject"
                  ? error instanceof TypeError
                  : error === original) + ":" + closed;
              }
            }
            ["uncallable", "nonobject", "next", "done", "value", "complete"]
              .map(noCloseCase).join(",");
            "#,
        )
        .expect("Map.groupBy IteratorStepValue failures must not close"),
        Value::String(Arc::from("true:0,true:0,true:0,true:0,true:0,true:0"))
    );

    assert_eq!(
        vm.run(
            r#"
            function override(value) {
              return function() {
                var done = false;
                return { next: function() {
                  if (done) return { done: true };
                  done = true;
                  return { done: false, value: value };
                } };
              };
            }
            function groupedValue(iterable) {
              return Map.groupBy(iterable, function() { return "k"; }).get("k")[0];
            }
            var array = ["native"];
            var map = new Map([["native", 1]]);
            var set = new Set(["native"]);
            function* source() { yield "native"; }
            var generator = source();
            array[Symbol.iterator] = override("array");
            map[Symbol.iterator] = override("map");
            set[Symbol.iterator] = override("set");
            generator[Symbol.iterator] = override("generator");
            [groupedValue(array), groupedValue(map), groupedValue(set),
              groupedValue(generator)].join(",");
            "#,
        )
        .expect("Map.groupBy must observe every overridden iterator"),
        Value::String(Arc::from("array,map,set,generator"))
    );
}

#[test]
fn set_constructor_observes_direct_iterator_close_and_realm_boundaries() {
    assert_eq!(
        run(r#"
            var log = [];
            var nextGets = 0;
            var nextReceiver = false;
            var addGets = 0;
            var addReceiver = false;
            var step = 0;
            var originalAdd = Set.prototype.add;
            Object.defineProperty(Set.prototype, "add", {
              configurable: true,
              get: function() {
                addGets++;
                log.push("get-add");
                return function(value) {
                  addReceiver = this instanceof Set;
                  log.push("add:" + arguments.length);
                  return originalAdd.call(this, value);
                };
              }
            });
            var iterator = {};
            Object.defineProperty(iterator, "next", {
              configurable: true,
              get: function() {
                nextGets++;
                return function() {
                  nextReceiver = this === iterator;
                  log.push("next:" + arguments.length);
                  Object.defineProperty(iterator, "next", {
                    value: function() { throw new Error("must use cached next"); },
                    configurable: true
                  });
                  return step++ === 0
                    ? { done: false, value: "direct" }
                    : { done: true };
                };
              }
            });
            var iterable = new Proxy({
              [Symbol.iterator]: function() {
                Object.defineProperty(Set.prototype, "add", {
                  value: function() { throw new Error("must use cached add"); },
                  writable: true, configurable: true
                });
                return iterator;
              }
            }, {
              has: function(target, key) { log.push("has"); return key in target; },
              get: function(target, key, receiver) {
                log.push(typeof key === "symbol" ? "get-symbol" : "get-" + key);
                return Reflect.get(target, key, receiver);
              }
            });
            var direct = new Set(iterable);
            Object.defineProperty(Set.prototype, "add", {
              value: originalAdd, writable: true, configurable: true
            });

            function override(value) {
              return function() {
                var done = false;
                return { next: function() {
                  if (done) return { done: true };
                  done = true;
                  return { done: false, value: value };
                } };
              };
            }
            var array = ["array-original"];
            var map = new Map([["map-original", 1]]);
            var set = new Set(["set-original"]);
            function* source() { yield "generator-original"; }
            var generator = source();
            array[Symbol.iterator] = override("array");
            map[Symbol.iterator] = override("map");
            set[Symbol.iterator] = override("set");
            generator[Symbol.iterator] = override("generator");
            [
              log.join(","), nextGets, nextReceiver, addGets, addReceiver,
              direct.has("direct"),
              new Set(array).has("array"), new Set(map).has("map"),
              new Set(set).has("set"), new Set(generator).has("generator")
            ].join("|");
            "#,),
        Value::String(Arc::from(
            "get-add,get-symbol,next:0,add:1,next:0|1|true|1|true|true|true|true|true|true"
        ))
    );

    assert_eq!(
        run(
            r#"
            function closeCount(kind) {
              var closed = 0;
              var iterator = {
                next: function() {
                  if (kind === "next") throw new Error("next-error");
                  if (kind === "result") return 1;
                  if (kind === "done") return {
                    get done() { throw new Error("done-error"); }
                  };
                  if (kind === "value") return {
                    done: false,
                    get value() { throw new Error("value-error"); }
                  };
                  return { done: false, value: 1 };
                },
                return: function() { closed++; throw new Error("close-error"); }
              };
              var originalAdd = Set.prototype.add;
              if (kind === "adder") Set.prototype.add = function() {
                throw new Error("adder-error");
              };
              var message;
              try {
                new Set({ [Symbol.iterator]: function() { return iterator; } });
              } catch (error) { message = error.message; }
              Set.prototype.add = originalAdd;
              return kind + ":" + closed + ":" + message;
            }
            ["next", "result", "done", "value", "adder"]
              .map(closeCount).join("|");
            "#,
        ),
        Value::String(Arc::from(
            "next:0:next-error|result:0:Iterator result is not an object|done:0:done-error|value:0:value-error|adder:1:adder-error"
        ))
    );

    assert_eq!(
        run(r#"
            function preservesOriginal(mode) {
              var original = { mode: mode };
              var iterator = {
                next: function() { return { done: false, value: 1 }; }
              };
              if (mode === "null") iterator.return = null;
              if (mode === "getter") Object.defineProperty(iterator, "return", {
                get: function() { throw new Error("return-getter"); }
              });
              if (mode === "noncallable") iterator.return = 1;
              if (mode === "throw") iterator.return = function() {
                throw new Error("return-call");
              };
              if (mode === "primitive") iterator.return = function() { return 1; };
              if (mode === "object") iterator.return = function() { return {}; };
              var originalAdd = Set.prototype.add;
              Set.prototype.add = function() { throw original; };
              try {
                new Set({ [Symbol.iterator]: function() { return iterator; } });
              } catch (error) {
                Set.prototype.add = originalAdd;
                return error === original;
              }
              Set.prototype.add = originalAdd;
              return false;
            }
            ["absent", "null", "getter", "noncallable", "throw", "primitive", "object"]
              .map(preservesOriginal).join("|");
            "#,),
        Value::String(Arc::from("true|true|true|true|true|true|true"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var OtherSet = other.Set;
            var OtherSetPrototype = OtherSet.prototype;
            var otherIterator = new OtherSet([1]).values();
            var OtherSetIteratorPrototype = Object.getPrototypeOf(otherIterator);
            var NewTarget = new other.Function();
            NewTarget.prototype = null;
            var BoundNewTarget = NewTarget.bind(null);
            other.Set = null;
            other.Object = null;
            var fallback = Object.getPrototypeOf(
              Reflect.construct(OtherSet, [], BoundNewTarget)
            ) === OtherSetPrototype;
            OtherSetPrototype.add = null;
            var error;
            try { new OtherSet([1]); } catch (caught) { error = caught; }
            [fallback, error instanceof other.TypeError,
              error instanceof TypeError,
              OtherSetIteratorPrototype !==
                Object.getPrototypeOf(new Set().values()),
              Object.getPrototypeOf(OtherSetIteratorPrototype) ===
                other.Iterator.prototype,
              OtherSetIteratorPrototype.next instanceof other.Function,
              !(OtherSetIteratorPrototype.next instanceof Function)].join("|");
            "#,),
        Value::String(Arc::from("true|true|false|true|true|true|true"))
    );
}

#[test]
fn weak_collection_constructors_observe_direct_iterators_and_realm_boundaries() {
    assert_eq!(
        run(
            r#"
            var log = [];
            var mapKey = {};
            var setValue = {};
            function construct(kind, yielded) {
              var Constructor = kind === "map" ? WeakMap : WeakSet;
              var method = kind === "map" ? "set" : "add";
              var original = Constructor.prototype[method];
              var methodGets = 0;
              var nextGets = 0;
              var step = 0;
              Object.defineProperty(Constructor.prototype, method, {
                configurable: true,
                get: function() {
                  methodGets++;
                  log.push("get-" + method);
                  return function() {
                    log.push(method + ":" + arguments.length + ":" +
                      (this instanceof Constructor));
                    return original.apply(this, arguments);
                  };
                }
              });
              var iterator = {};
              Object.defineProperty(iterator, "next", {
                configurable: true,
                get: function() {
                  nextGets++;
                  return function() {
                    log.push("next:" + arguments.length + ":" +
                      (this === iterator));
                    Object.defineProperty(iterator, "next", {
                      value: function() { throw new Error("uncached next"); },
                      configurable: true
                    });
                    return step++ === 0
                      ? { done: false, value: yielded }
                      : { done: true };
                  };
                }
              });
              var iterable = new Proxy({
                [Symbol.iterator]: function() {
                  Object.defineProperty(Constructor.prototype, method, {
                    value: function() { throw new Error("uncached adder"); },
                    writable: true, configurable: true
                  });
                  return iterator;
                }
              }, {
                has: function() { log.push("has"); return true; },
                get: function(target, key, receiver) {
                  log.push(typeof key === "symbol" ? "get-symbol" : "get-" + key);
                  return Reflect.get(target, key, receiver);
                }
              });
              var result = new Constructor(iterable);
              Object.defineProperty(Constructor.prototype, method, {
                value: original, writable: true, configurable: true
              });
              return [result, methodGets, nextGets];
            }
            var mapResult = construct("map", [mapKey, 7]);
            var setResult = construct("set", setValue);
            [
              log.join(","), mapResult[1], mapResult[2],
              setResult[1], setResult[2], mapResult[0].get(mapKey),
              setResult[0].has(setValue)
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "get-set,get-symbol,next:0:true,set:2:true,next:0:true,get-add,get-symbol,next:0:true,add:1:true,next:0:true|1|1|1|1|7|true"
        ))
    );

    assert_eq!(
        run(
            r#"
            function closeCase(kind, collection) {
              var closed = 0;
              var original = {};
              var Constructor = collection === "map" ? WeakMap : WeakSet;
              var method = collection === "map" ? "set" : "add";
              var saved = Constructor.prototype[method];
              var entry = collection === "map"
                ? { get 0() { throw original; } }
                : {};
              var iterator = {
                next: function() {
                  if (kind === "next") throw original;
                  if (kind === "result") return 1;
                  if (kind === "done") return { get done() { throw original; } };
                  if (kind === "value") return {
                    done: false, get value() { throw original; }
                  };
                  return { done: false, value: entry };
                },
                return: function() { closed++; throw new Error("replacement"); }
              };
              if (kind === "adder") Constructor.prototype[method] = function() {
                throw original;
              };
              var caught;
              try {
                new Constructor({ [Symbol.iterator]: function() { return iterator; } });
              } catch (error) { caught = error; }
              Constructor.prototype[method] = saved;
              return kind + ":" + collection + ":" + closed + ":" +
                (caught === original);
            }
            [
              closeCase("next", "map"), closeCase("result", "map"),
              closeCase("done", "map"),
              closeCase("value", "map"), closeCase("entry", "map"),
              closeCase("adder", "map"), closeCase("next", "set"),
              closeCase("result", "set"), closeCase("value", "set"),
              closeCase("adder", "set")
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "next:map:0:true|result:map:0:false|done:map:0:true|value:map:0:true|entry:map:1:true|adder:map:1:true|next:set:0:true|result:set:0:false|value:set:0:true|adder:set:1:true"
        ))
    );

    assert_eq!(
        run(r#"
            function one(value) {
              return function() {
                var done = false;
                return { next: function() {
                  if (done) return { done: true };
                  done = true;
                  return { done: false, value: value };
                }};
              };
            }
            var arrayKey = {};
            var array = [arrayKey];
            array[Symbol.iterator] = one(arrayKey);
            var mapKey = {};
            var map = new Map([[{}, 1]]);
            map[Symbol.iterator] = one([mapKey, 2]);
            var setKey = {};
            var set = new Set([{}]);
            set[Symbol.iterator] = one(setKey);
            var generatorKey = {};
            function* source() { yield {}; }
            var generator = source();
            generator[Symbol.iterator] = one([generatorKey, 3]);
            [
              new WeakSet(array).has(arrayKey),
              new WeakMap(map).get(mapKey),
              new WeakSet(set).has(setKey),
              new WeakMap(generator).get(generatorKey)
            ].join("|");
            "#,),
        Value::String(Arc::from("true|2|true|3"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var OtherWeakMap = other.WeakMap;
            var OtherWeakSet = other.WeakSet;
            var OtherWeakMapPrototype = OtherWeakMap.prototype;
            var OtherWeakSetPrototype = OtherWeakSet.prototype;
            var NewTarget = new other.Function();
            NewTarget.prototype = null;
            var BoundNewTarget = NewTarget.bind(null);
            other.WeakMap = other.WeakSet = other.Object = null;
            var foreignMap = Reflect.construct(WeakMap, [], BoundNewTarget);
            var foreignSet = Reflect.construct(WeakSet, [], BoundNewTarget);
            var iterator = {
              next: function() { return { done: false, value: 1 }; },
              return: function() { return {}; }
            };
            var mapError;
            var setError;
            try {
              new OtherWeakMap({ [Symbol.iterator]: function() { return iterator; } });
            } catch (error) { mapError = error; }
            try {
              OtherWeakSet.prototype.add = null;
              new OtherWeakSet([{}]);
            } catch (error) { setError = error; }
            [
              Object.getPrototypeOf(foreignMap) === OtherWeakMapPrototype,
              Object.getPrototypeOf(foreignSet) === OtherWeakSetPrototype,
              mapError instanceof other.TypeError, !(mapError instanceof TypeError),
              setError instanceof other.TypeError, !(setError instanceof TypeError),
              OtherWeakMapPrototype.get instanceof other.Function,
              !(OtherWeakMapPrototype.get instanceof Function)
            ].join("|");
            "#,),
        Value::String(Arc::from("true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn weak_collection_methods_cover_brands_symbols_and_upsert_reentry() {
    assert_eq!(
        run(r#"
            var objectKey = {};
            var symbolKey = Symbol("key");
            var wellKnown = Symbol.iterator;
            var registered = Symbol.for("registered");
            var map = new WeakMap([[objectKey, 1], [symbolKey, 2], [wellKnown, 3]]);
            var set = new WeakSet([objectKey, symbolKey, wellKnown]);
            var registeredMapError = false;
            var registeredSetError = false;
            try { map.set(registered, 4); } catch (error) {
              registeredMapError = error instanceof TypeError;
            }
            try { set.add(registered); } catch (error) {
              registeredSetError = error instanceof TypeError;
            }
            var inserted = {};
            var computed = {};
            var calls = 0;
            var first = map.getOrInsert(inserted, 5);
            var second = map.getOrInsert(inserted, 6);
            var computedValue = map.getOrInsertComputed(computed, function(key) {
              "use strict";
              calls++;
              map.set(key, "during");
              return this === undefined && arguments.length === 1 ? "final" : "bad";
            });
            map.getOrInsertComputed(computed, function() { calls++; return "late"; });
            var callbackCheckedFirst = false;
            try { map.getOrInsertComputed(1, 1); } catch (error) {
              callbackCheckedFirst = error.message.indexOf("callback") !== -1;
            }
            var brandErrors = 0;
            for (var method of [
              WeakMap.prototype.get, WeakMap.prototype.set,
              WeakMap.prototype.has, WeakMap.prototype.delete,
              WeakMap.prototype.getOrInsert,
              WeakMap.prototype.getOrInsertComputed,
              WeakSet.prototype.add, WeakSet.prototype.has,
              WeakSet.prototype.delete
            ]) {
              try { method.call({}); } catch (error) {
                if (error instanceof TypeError) brandErrors++;
              }
            }
            [
              map.get(objectKey), map.get(symbolKey), map.get(wellKnown),
              set.has(objectKey), set.has(symbolKey), set.has(wellKnown),
              registeredMapError, registeredSetError,
              first, second, computedValue, map.get(computed), calls,
              callbackCheckedFirst,
              brandErrors, map.delete(symbolKey), set.delete(symbolKey),
              map.has(symbolKey), set.has(symbolKey)
            ].join("|");
            "#,),
        Value::String(Arc::from(
            "1|2|3|true|true|true|true|true|5|5|final|final|1|true|9|true|true|false|false"
        ))
    );
}

#[test]
fn set_basic() {
    assert_eq!(
        run("let s = new Set(); s.add(1); s.add(2); s.add(1); s.size;"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("let d = Object.getOwnPropertyDescriptor(Set.prototype, 'size'); let s = new Set([1, 2]); [s.size, d.get.call(s), d.get.name, d.get.length, d.enumerable, d.configurable, typeof d.set].join('|');"),
        Value::String(Arc::from("2|2|get size|0|false|true|undefined"))
    );
    assert_eq!(
        run("let d = Object.getOwnPropertyDescriptor(Set.prototype, 'size'); [typeof d.value, typeof d.writable].join('|');"),
        Value::String(Arc::from("undefined|undefined"))
    );
    assert_eq!(
        run("Object.defineProperty(Set.prototype, 'size', { get: function(){ return 99; }, configurable: true }); new Set([1, 2]).size;"),
        Value::Number(99.0)
    );
    assert_eq!(
        run("delete Set.prototype.size; new Set([1, 2]).size;"),
        Value::Undefined
    );
    assert!(
        run_err("Object.getOwnPropertyDescriptor(Set.prototype, 'size').get.call({});")
            .contains("TypeError")
    );
    assert!(
        run_err("Object.getOwnPropertyDescriptor(Set.prototype, 'size').get.call(1);")
            .contains("TypeError")
    );
    assert!(
        run_err("Object.getOwnPropertyDescriptor(Set.prototype, 'size').get.call(new Map());")
            .contains("TypeError")
    );
    for src in [
        "Set.prototype.add.call({}, 1);",
        "Set.prototype.has.call({}, 1);",
        "Set.prototype.delete.call({}, 1);",
        "Set.prototype.clear.call({});",
        "Set.prototype.entries.call({});",
        "Set.prototype.keys.call({});",
        "Set.prototype.values.call({});",
        "Set.prototype.forEach.call({}, function(){});",
        "Set.prototype.add.call(1, 1);",
        "Set.prototype.has.call(new Map(), 1);",
    ] {
        assert!(run_err(src).contains("TypeError"), "{src}");
    }
    assert_eq!(
        run("let s = new Set(); s.add(1); s.has(1);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("let s = new Set([1, 2]); s.clear(); [s.size, s.has(1)].join('|');"),
        Value::String(Arc::from("0|false"))
    );
    assert!(run_err("new Set().forEach(1);").contains("TypeError"));
    assert_eq!(
        run("let s = new Set([-0]); s.add(-0); s.add(+0); [s.size, Object.is(s.values().next().value, -0)].join('|');"),
        Value::String(Arc::from("1|false"))
    );
    assert_eq!(
        run("let s = new Set([+0]); s.add(-0); s.size;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("let s = new Set([1, 2]); let it = s.values(); let a = it.next(); let b = it.next(); let c = it.next(); [a.value, b.value, c.done, it[Symbol.iterator]() === it].join('|');"),
        Value::String(Arc::from("1|2|true|true"))
    );
    assert_eq!(
        run(r#"
            let it = new Set([1]).values();
            let proto = Object.getPrototypeOf(it);
            let d = Object.getOwnPropertyDescriptor(proto, "next");
            let tag = Object.getOwnPropertyDescriptor(proto, Symbol.toStringTag);
            [
              Object.prototype.hasOwnProperty.call(it, "next"),
              d.value.name, d.value.length, d.writable, d.enumerable, d.configurable,
              tag.value, tag.writable, tag.enumerable, tag.configurable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "false|next|0|true|false|true|Set Iterator|false|false|true"
        ))
    );
    assert!(
        run_err("Object.getPrototypeOf(new Set().values()).next.call({});").contains("TypeError")
    );
    assert_eq!(
        run("Set.prototype.keys === Set.prototype.values && Set.prototype[Symbol.iterator] === Set.prototype.values;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("let s = new Set([1, 2, 3]); let out = []; s.forEach(function(v){ if (v === 2) s.delete(1); if (v === 3) s.add(1); out.push(v); }); out.join('|');"),
        Value::String(Arc::from("1|2|3|1"))
    );
    assert_eq!(run("new Set([NaN, Number('x')]).size;"), Value::Number(1.0));
    assert_eq!(run("new Set([0, 0n]).size;"), Value::Number(2.0));
    assert_eq!(
        run("let a = new Set([1,2,3]); let b = new Set([3,4]); [Array.from(a.union(b)).join(','), Array.from(a.intersection(b)).join(','), Array.from(a.difference(b)).join(','), Array.from(a.symmetricDifference(b)).join(','), a.isSubsetOf(b), a.isSupersetOf(b), a.isDisjointFrom(new Set([5]))].join('|');"),
        Value::String(Arc::from("1,2,3,4|3|1,2|1,2,4|false|false|true"))
    );
    assert_eq!(
        run("class MySet extends Set { static get [Symbol.species]() { throw new Error('no'); } } let out = new MySet([1]).union(new Set([2])); [out instanceof Set, out instanceof MySet, Array.from(out).join(',')].join('|');"),
        Value::String(Arc::from("true|false|1,2"))
    );
    assert_eq!(
        run("let calls = []; let other = { get size(){ calls.push('size'); return { valueOf(){ calls.push('number'); return 3; } }; }, get has(){ calls.push('has'); return function(v){ calls.push('has:' + v); return v !== 2; }; }, get keys(){ calls.push('keys'); return function(){ throw new Error('no keys'); }; } }; let result = new Set([1,2]).difference(other); Array.from(result).join(',') + '|' + calls.join(',');"),
        Value::String(Arc::from("2|size,number,has,keys,has:1,has:2"))
    );
    assert!(run_err("Set();").contains("TypeError"));
    assert!(run_err("Set([]);").contains("TypeError"));
    assert_eq!(
        run("let setAdd = Set.prototype.add; let calls = 0; Set.prototype.add = function(v){ calls++; return setAdd.call(this, v); }; let s = new Set([1,2]); calls + '|' + Array.from(s).join(',');"),
        Value::String(Arc::from("2|1,2"))
    );
    assert!(run_err("Set.prototype.add = null; new Set([1,2]);").contains("TypeError"));
    assert_eq!(
        run("let calls = 0; Object.defineProperty(Set.prototype, 'add', { get(){ calls++; throw new Error('no'); }, configurable: true }); try { new Set(); } catch (e) {} calls;"),
        Value::Number(0.0)
    );
    assert_eq!(run("[1, 2].values().next().value;"), Value::Number(1.0));
    assert!(run_err(
        "new Set([1]).union({ size: 1, has(){ return false; }, keys(){ return [2]; } });"
    )
    .contains("TypeError"));
    assert!(run_err("new Set([1]).union({ size: 1, has(){ return false; }, keys(){ return { next(){ return 1; } }; } });").contains("TypeError"));
    assert!(run_err("new Set([1]).union({ size: -1, get has(){ throw new Error('late'); }, get keys(){ throw new Error('late'); } });").contains("RangeError"));
    assert_eq!(
        run("new Set([1,2]).isSupersetOf({ size: 2.9, has(){ throw new Error('no has'); }, keys(){ let i = 0; return { next(){ i++; return i <= 2 ? { value: i, done: false } : { done: true }; } }; } });"),
        Value::Bool(true)
    );
    assert!(run_err("new Set([1]).isSupersetOf({ size: 1, has(){ throw new Error('no has'); }, keys(){ return { next(){ return { value: 2, done: false }; }, return(){ return 1; } }; } });").contains("TypeError"));
    assert_eq!(
        run("let s = new Set([1]); let other = { size: 0, has(){ return false; }, keys(){ s.add(2); return [].values(); } }; Array.from(s.union(other)).join(',') + '|' + Array.from(s.symmetricDifference(other)).join(',');"),
        Value::String(Arc::from("1,2|1,2"))
    );
}

#[test]
fn set_algebra_preserves_snapshots_generations_iterators_and_realms() {
    assert_eq!(
        run(r#"
            var source = new Set(["a", "b"]);
            var other = {
              size: 99,
              has(value) {
                if (value === "a") source.delete("b");
                return value === "b";
              },
              keys() { throw new Error("keys must not run"); }
            };
            Array.from(source.difference(other)).join(",");
            "#),
        Value::String(Arc::from("a"))
    );

    assert_eq!(
        run(r#"
            var source = new Set(["a", "b", "c"]);
            var seen = [];
            var other = {
              size: Infinity,
              has(value) {
                seen.push(value);
                if (value === "a") {
                  source.delete("b");
                  source.add("b");
                }
                return !(value === "b" && seen.length === 2);
              },
              keys() { throw new Error("keys must not run"); }
            };
            [source.isSubsetOf(other), seen.join(",")].join("|");
            "#),
        Value::String(Arc::from("true|a,c,b"))
    );

    assert_eq!(
        run(r#"
            var nextGets = 0;
            var iterator = new Set([2]).keys();
            Object.defineProperty(iterator, "next", {
              configurable: true,
              get() {
                nextGets++;
                return function() { return { done: true }; };
              }
            });
            var other = { size: 1, has() {}, keys() { return iterator; } };
            [Array.from(new Set([1]).union(other)).join(","), nextGets].join("|");
            "#),
        Value::String(Arc::from("1|1"))
    );

    assert_eq!(
        run(r#"
            var closed = 0;
            var iterator = new Set([2]).keys();
            iterator.return = function() { closed++; return {}; };
            var result = new Set([1]).isSupersetOf({
              size: 1, has() {}, keys() { return iterator; }
            });
            [result, closed].join("|");
            "#),
        Value::String(Arc::from("false|1"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var source = new Set([1]);
            var operand = new Set([2]);
            var methods = ["union", "intersection", "difference", "symmetricDifference"];
            methods.map(function(name) {
              var result = other.Set.prototype[name].call(source, operand);
              return Object.getPrototypeOf(result) === other.Set.prototype;
            }).join("|");
            "#),
        Value::String(Arc::from("true|true|true|true"))
    );

    assert_eq!(
        run(r#"
            var source = new Set(["a", "b"]);
            var seen = [];
            var moved = false;
            source.forEach(function(value) {
              seen.push(value);
              if (value === "a" && !moved) {
                moved = true;
                source.delete("b");
                source.add("b");
                source.delete("a");
                source.add("a");
              }
            });
            seen.join(",");
            "#),
        Value::String(Arc::from("a,b,a"))
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
        Value::String(Arc::from("Symbol(x)"))
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

#[test]
fn arrow_super_call_rebinds_lexical_constructor_this() {
    assert_eq!(
        run("var count=0; class A{constructor(){count++;}} class B extends A{constructor(){super(); this.af=_=>super();}} var b=new B(); var threw=false; try{b.af();}catch(e){threw=e instanceof ReferenceError;} threw + ':' + count;"),
        Value::String(Arc::from("true:2"))
    );
    assert_eq!(
        run("var count=0; class A{constructor(){count++;}} class B extends A{constructor(){super(); super();}} var threw=false; try{new B();}catch(e){threw=e instanceof ReferenceError;} threw + ':' + count;"),
        Value::String(Arc::from("true:2"))
    );
}

#[test]
fn super_constructor_checks_constructability_after_arguments() {
    assert_eq!(
        run("var evaluated=false,caught; class C extends Object{constructor(){try{super(evaluated=true);}catch(e){caught=e;}}} Object.setPrototypeOf(C, parseInt); try{new C();}catch(e){} typeof caught + ':' + (caught instanceof TypeError) + ':' + evaluated;"),
        Value::String(Arc::from("object:true:true"))
    );
    assert_eq!(
        run("var evaluated=false,caught; class C extends Object{constructor(){try{super(0, ...[evaluated=true]);}catch(e){caught=e;}}} Object.setPrototypeOf(C, parseInt); try{new C();}catch(e){} typeof caught + ':' + (caught instanceof TypeError) + ':' + evaluated;"),
        Value::String(Arc::from("object:true:true"))
    );
}

#[test]
fn super_constructor_call_accepts_mixed_spread_arguments() {
    assert_eq!(
        run("class A{constructor(){this.args=[].slice.call(arguments).join(',');}} class B extends A{constructor(){super(1, ...[2,3], 4);}} new B().args;"),
        Value::String(Arc::from("1,2,3,4"))
    );
    assert_eq!(
        run("class A{constructor(){}} class B extends A{constructor(){var threw=false; try{super(0, ...missing);}catch(e){threw=e instanceof ReferenceError;} return {threw};}} new B().threw;"),
        Value::Bool(true)
    );
}

#[test]
fn computed_super_putvalue_checks_this_before_key_expression() {
    assert_eq!(
        run("var count=0; class A{constructor(){count++; throw new Error('base');}} class B extends A{constructor(){super[super()] += 0;}} var out; try{new B();}catch(e){out=(e instanceof ReferenceError)+':'+count;} out;"),
        Value::String(Arc::from("true:0"))
    );
    assert_eq!(
        run("var count=0; class A{constructor(){count++; throw new Error('base');}} class B extends A{constructor(){super[super()]++;}} var out; try{new B();}catch(e){out=(e instanceof ReferenceError)+':'+count;} out;"),
        Value::String(Arc::from("true:0"))
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
fn for_of_closes_iterator_on_abrupt_completion() {
    assert_eq!(
        run(r#"
            var closed = 0;
            var iter = {
                [Symbol.iterator]() { return this; },
                next() { return { value: 1, done: false }; },
                return() { closed++; return { done: true }; }
            };
            (function(){ for (var v of iter) return "done"; })();
            closed;
        "#),
        Value::Number(1.0)
    );

    let err = common::run_err(
        r#"
        var error = new Error("close");
        var iter = {
            [Symbol.iterator]() { return this; },
            next() { return { value: 1, done: false }; },
            return() { throw error; }
        };
        (function(){ for (var v of iter) return 0; })();
    "#,
    );
    assert!(err.contains("close") || err.contains("Error"), "got {err}");

    assert_eq!(
        run(r#"
            var closed = 0;
            var i = 0;
            var iter = {
                [Symbol.iterator]() { return this; },
                next() { return ++i > 2 ? { done: true } : { value: i, done: false }; },
                return() { closed++; return { done: true }; }
            };
            for (var v of iter) continue;
            closed;
        "#),
        Value::Number(0.0)
    );

    assert_eq!(
        run(r#"
            var closed = 0;
            var seen = 0;
            var i = 0;
            var iter = {
                [Symbol.iterator]() { return this; },
                next() { return ++i > 2 ? { done: true } : { value: i, done: false }; },
                return() { closed++; return { done: true }; }
            };
            for (let v of iter) {
                try { continue; } finally { seen += v; }
            }
            seen + '|' + closed;
        "#),
        Value::String(Arc::from("3|0"))
    );
}

#[test]
fn for_of_iterator_protocol_edges() {
    assert_eq!(
        run(r#"
            var iterator = {};
            var loadNextCount = 0;
            var iterationCount = 0;
            function next() {
              return iterationCount ? { done: true } : { value: 45, done: false };
            }
            Object.defineProperty(iterator, "next", {
              get: function() { loadNextCount++; return next; },
              configurable: true
            });
            var iterable = {};
            iterable[Symbol.iterator] = function() { return iterator; };
            for (var x of iterable) {
              Object.defineProperty(iterator, "next", {
                get: function() { throw new Error("next-reloaded"); }
              });
              iterationCount++;
            }
            [iterationCount, loadNextCount].join("|");
        "#),
        Value::String(Arc::from("1|1"))
    );

    assert!(run_err(
        r#"
            var iterable = {};
            iterable[Symbol.iterator] = function() {
              return { next: function() { return 1; } };
            };
            for (var x of iterable) {}
        "#
    )
    .contains("TypeError"));

    assert_eq!(
        run(r#"
            var count = 0;
            var iterable = {};
            iterable[Symbol.iterator] = function() {
              var first = true;
              return {
                next: function() {
                  if (first) { first = false; return { value: 1, done: 0 }; }
                  return { value: 2, done: "done" };
                }
              };
            };
            for (var x of iterable) count++;
            count;
        "#),
        Value::Number(1.0)
    );

    assert_eq!(
        run(r#"
            var returnCount = 0;
            var iterable = {};
            iterable[Symbol.iterator] = function() {
              return {
                next: function() { throw new Error("next"); },
                return: function() { returnCount++; return {}; }
              };
            };
            try { for (var x of iterable) {} } catch (e) {}
            returnCount;
        "#),
        Value::Number(0.0)
    );

    assert_eq!(
        run(r#"
            var returnCount = 0;
            var iterable = {};
            iterable[Symbol.iterator] = function() {
              return {
                next: function() {
                  return { done: false, get value() { throw new Error("value"); } };
                },
                return: function() { returnCount++; return {}; }
              };
            };
            try { for (var x of iterable) {} } catch (e) {}
            returnCount;
        "#),
        Value::Number(0.0)
    );

    assert!(run_err(
        r#"
            var iterable = {};
            iterable[Symbol.iterator] = function() {
              return {
                next: function() { return { value: 1, done: false }; },
                return: function() { return 1; }
              };
            };
            for (var x of iterable) break;
        "#
    )
    .contains("TypeError"));

    assert!(run_err(
        r#"
            var iterable = {};
            iterable[Symbol.iterator] = function() {
              return {
                next: function() { return { value: 1, done: false }; },
                get return() { throw new Error("close"); }
              };
            };
            for (var x of iterable) break;
        "#
    )
    .contains("close"));

    assert!(run_err(
        r#"
            var iterable = {};
            iterable[Symbol.iterator] = function() {
              return {
                next: function() { return { value: 1, done: false }; },
                get return() { throw new Error("close"); }
              };
            };
            for (var x of iterable) throw new Error("body");
        "#
    )
    .contains("body"));

    assert_eq!(
        run(r#"
            var returnCount = 0;
            var iterable = {};
            iterable[Symbol.iterator] = function() {
              return {
                next: function() { return { value: 1, done: false }; },
                return: function() { returnCount++; return {}; }
              };
            };
            L: do {
              for (var x of iterable) {
                continue L;
              }
            } while (false);
            returnCount;
        "#),
        Value::Number(1.0)
    );
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
fn for_in_of_super_lhs_uses_super_reference_receiver() {
    assert_eq!(
        run(r#"
            var log = [];
            var keyCalls = 0;
            class Base {
              set value(value) {
                log.push((this === receiver) + ":" + value);
              }
            }
            class Derived extends Base {
              runOf() { for (super.value of [11]) {} }
              runComputedOf() {
                for (super[(keyCalls++, "value")] of [12, 13]) {}
              }
              runEmpty() { for (super.value of []) {} }
              runIn() { for (super.value in { key: 1 }) {} }
            }
            class StaticBase {
              static set value(value) {
                log.push("static:" + (this === StaticDerived) + ":" + value);
              }
            }
            class StaticDerived extends StaticBase {
              static run() { for (super.value of [15]) {} }
            }
            var receiver = new Derived();
            receiver.runOf();
            receiver.runComputedOf();
            receiver.runEmpty();
            receiver.runIn();
            StaticDerived.run();
            log.join("|") + ";" + keyCalls;
            "#),
        Value::String(Arc::from(
            "true:11|true:12|true:13|true:key|static:true:15;2"
        ))
    );

    assert_eq!(
        run(r#"
            var observed;
            var parent = {
              set value(value) {
                "use strict";
                observed = typeof this + ":" + this + ":" + value;
              }
            };
            var home = {
              run() { "use strict"; for (super.value of [7]) {} }
            };
            Object.setPrototypeOf(home, parent);
            home.run.call("receiver");
            observed;
            "#),
        Value::String(Arc::from("string:receiver:7"))
    );

    assert_eq!(
        run(r#"
            var log = [];
            var first = { set value(value) { log.push("first:" + value); } };
            var second = { set value(value) { log.push("second:" + value); } };
            var home = {
              run() {
                for (super[(Object.setPrototypeOf(home, second), "value")] of [9]) {}
              }
            };
            Object.setPrototypeOf(home, first);
            home.run();
            log.join("|");
            "#),
        Value::String(Arc::from("second:9"))
    );
}

#[test]
fn for_of_super_lhs_abrupt_assignment_closes_iterator() {
    assert_eq!(
        run(r#"
            var log = [];
            var iterable = {
              [Symbol.iterator]() {
                var done = false;
                return {
                  next() {
                    log.push("next");
                    if (done) return { done: true };
                    done = true;
                    return { value: 1, done: false };
                  },
                  return() { log.push("return"); throw "close-error"; }
                };
              }
            };
            class Base {
              set value(value) { log.push("setter"); throw "setter-error"; }
            }
            class Derived extends Base {
              run() {
                try { for (super.value of iterable) { log.push("body"); } }
                catch (error) { log.push(error); }
              }
            }
            new Derived().run();
            log.join("|");
            "#),
        Value::String(Arc::from("next|setter|return|setter-error"))
    );
}

#[test]
fn for_of_super_lhs_reference_survives_gc() {
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
            var receiver;
            var log = [];
            var target = {};
            var parent = new Proxy(target, {
              set(target, key, value, actualThis) {
                forceGc();
                log.push(key + ":" + value + ":" + (actualThis === receiver));
                return true;
              }
            });
            var home = {
              run() {
                for (super[(
                  forceGc(),
                  { toString() { forceGc(); return "value"; } }
                )] of [21]) {}
              }
            };
            Object.setPrototypeOf(home, parent);
            receiver = Object.create(home);
            receiver.run();
            log.join("|");
            "#,
        )
        .expect("super loop Reference should survive key and Proxy GC"),
        Value::String(Arc::from("value:21:true"))
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
fn for_lexical_head_scope_and_early_errors() {
    assert!(
        run_err("for (let x; false;) { var x; }").contains("SyntaxError"),
        "let head/body var redeclaration should be a SyntaxError"
    );
    assert!(
        run_err("for (const x = 0; false;) { var x; }").contains("SyntaxError"),
        "const head/body var redeclaration should be a SyntaxError"
    );

    assert_eq!(
        run("var i = 0, counter = 0; for (async of => {}; i < 10; ++i) { ++counter; } counter;"),
        Value::Number(10.0)
    );

    assert_eq!(
        run("var value; for (let [x] = [23]; ; ) { value = x; break; } typeof x + ':' + value;"),
        Value::String(Arc::from("undefined:23"))
    );

    assert_eq!(
        run("let x = 'outside'; var run = true, probeTest, probeIncr, probeBody; for (let x = 'inside'; (probeTest = function(){ return x; }) && run; probeIncr = function(){ return x; }) probeBody = function(){ return x; }, run = false; [probeBody(), probeIncr(), probeTest(), x].join(',');"),
        Value::String(Arc::from("inside,inside,inside,outside"))
    );

    assert_eq!(
        run("var probeBefore, probeTest, probeIncr, probeBody; var run = true; for (let x = 'outside', _ = probeBefore = function(){ return x; }; run && (x = 'inside', probeTest = function(){ return x; }); probeIncr = function(){ return x; }) probeBody = function(){ return x; }, run = false; [probeBefore(), probeTest(), probeBody(), probeIncr()].join(',');"),
        Value::String(Arc::from("outside,inside,inside,inside"))
    );

    assert_eq!(
        run("var probeFirst, probeSecond = null; for (let x = 'first'; probeSecond === null; x = 'second') if (!probeFirst) probeFirst = function(){ return x; }; else probeSecond = function(){ return x; }; probeFirst() + ':' + probeSecond();"),
        Value::String(Arc::from("first:second"))
    );
}

#[test]
fn for_in_lexical_head_tdz_and_iteration_scope() {
    let msg = run_err("let x = 1; for (let x in { x }) {}");
    assert!(
        msg.contains("Cannot access 'x' before initialization"),
        "got: {}",
        msg
    );

    assert_eq!(
        run("var obj = Object.create(null); obj.key = 1; var value; for (let [x] in obj) { value = x; } typeof x + ':' + value;"),
        Value::String(Arc::from("undefined:k"))
    );

    assert_eq!(
        run("let x = 'outside'; var probeDecl, probeBody; for (let [x, _ = probeDecl = function(){ return x; }] in { i: 0 }) probeBody = function(){ return x; }; probeDecl() + ':' + probeBody() + ':' + x;"),
        Value::String(Arc::from("i:i:outside"))
    );

    assert_eq!(
        run("let x = 'outside'; var probeExpr; for (let x in { i: probeExpr = function(){ try { typeof x; return 'no'; } catch (e) { return e.name; } } }) ; probeExpr();"),
        Value::String(Arc::from("ReferenceError"))
    );

    assert_eq!(
        run("var probeBefore = function() { return x; }; var x = 1; var probeDecl, probeExpr, probeBody; for (let [_ = probeDecl = function() { return x; }] in { '': (eval('var x = 2;'), probeExpr = function() { return x; }) }) probeBody = function() { return x; }; [probeBefore(), probeDecl(), probeExpr(), probeBody(), x].join(',');"),
        Value::String(Arc::from("2,2,2,2,2"))
    );
}

#[test]
fn loop_unwind_skips_compile_only_scopes() {
    assert_eq!(
        run(
            "eval('5; outer: do { for (var b in { x: 0 }) { 6; continue outer; } } while (false)')"
        ),
        Value::Number(6.0)
    );

    assert_eq!(
        run("eval('5; outer: do { for (var b of [0]) { 6; continue outer; } } while (false)')"),
        Value::Number(6.0)
    );

    assert_eq!(
        run("eval('5; outer: do { for (var i = 0; i < 1; i++) { 6; continue outer; } } while (false)')"),
        Value::Number(6.0)
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
fn computed_accessor_string_line_continuation_key() {
    let src = r#"
        var stringSet;
        var obj = {
          get ['line\
Continuation']() { return 'get string'; },
          set ['line\
Continuation'](param) { stringSet = param; }
        };

        var got = obj['lineContinuation'];
        obj['lineContinuation'] = 'set string';
        got + ':' + stringSet;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("get string:set string")));
}

#[test]
fn computed_property_names_allow_in_inside_for_heads() {
    let src = r#"
        var empty = Object.create(null);
        var obj, value;
        for (obj = { get ["x" in empty]() { return "via get"; } }; ; ) {
            value = obj.false;
            break;
        }
        for (obj = { set ["x" in empty](param) { value += "," + param; } }; ; ) {
            obj.false = "via set";
            break;
        }
        var container = { false: 41 };
        for (value += "," + container["x" in empty]; ; ) {
            break;
        }
        value;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("via get,via set,41")));
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
fn ordinary_function_prototype_descriptors_are_writable_own_data_properties() {
    assert_eq!(
        run("var data = 'data'; Object.defineProperty(Object.prototype, 'constructor', { set: function(v) { data = v; }, configurable: true }); var f = function() {}; f.prototype.constructor = 1; var d = Object.getOwnPropertyDescriptor(f.prototype, 'constructor'); delete Object.prototype.constructor; [f.prototype.constructor, data, d.writable, d.enumerable, d.configurable].join(',');"),
        Value::String(Arc::from("1,data,true,false,true"))
    );

    assert_eq!(
        run("var data = 'data'; Object.defineProperty(Function.prototype, 'prototype', { set: function(v) { data = v; }, configurable: true }); var f = function() {}; f.prototype = {}; var d = Object.getOwnPropertyDescriptor(f, 'prototype'); delete Function.prototype.prototype; [Object.prototype.toString.call(f.prototype), data, d.writable, d.enumerable, d.configurable].join(',');"),
        Value::String(Arc::from("[object Object],data,true,false,false"))
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
        "({ method(x = super()) {} });",
    ] {
        let err = common::run_err(src);
        assert!(
            err.contains("super call") || err.contains("SyntaxError"),
            "{err}"
        );
    }
}

#[test]
fn object_method_parameter_defaults_allow_super_property() {
    let src = r#"
        var obj = {
            method(x = super.toString) { return x; }
        };
        obj.toString = null;
        obj.method() === Object.prototype.toString;
    "#;
    assert_eq!(run(src), Value::Bool(true));

    assert_eq!(
        run("let proto={get x(){return 41;}, m(){return this.n+1;}}; let obj={__proto__:proto,n:4, method(a=super.x,b=super.m()){return a+':'+b;}}; obj.method();"),
        Value::String(Arc::from("41:5"))
    );
}

#[test]
fn class_method_parameter_defaults_allow_super_property() {
    assert_eq!(
        run("class B{get x(){return 3;} m(){return 7;}} class C extends B{method(a=super.x,b=super.m()){return a+':'+b;}} new C().method();"),
        Value::String(Arc::from("3:7"))
    );
}

#[test]
fn regular_function_parameter_defaults_do_not_inherit_method_super() {
    let err = common::run_err("({ method(){ function f(x = super.toString) {} } });");
    assert!(
        err.contains("super keyword") || err.contains("SyntaxError"),
        "{err}"
    );
}

#[test]
fn object_methods_allow_yield_identifier_in_sloppy_non_generator_contexts() {
    let src = r#"
        var yield = "prop";
        var obj = {
            method(yield) { return yield; },
            defaulted(x = yield) { return x; },
            [yield]() { return "key"; }
        };
        obj.method("arg") + ":" + obj.defaulted() + ":" + obj.prop();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("arg:prop:key")));
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
        let originalIterator = Array.prototype[Symbol.iterator];
        Array.prototype[Symbol.iterator] = function() {
            let i = 0; let self = this;
            return { next() {
                if (i < self.length) { let v = self[i]*10; i++; return {value: v, done: false}; }
                return {value: undefined, done: true};
            }};
        };
        let r = [];
        for (let v of [1,2,3]) r.push(v);
        Array.prototype[Symbol.iterator] = originalIterator;
        let r2 = [];
        for (let v of [1,2,3]) r2.push(v);
        r.join(",") + "|" + r2.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("10,20,30|1,2,3")));
}
