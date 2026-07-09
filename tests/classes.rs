//! Class features: static initialization blocks and private methods/fields.

mod common;
use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

// --- static initialization blocks ---

#[test]
fn static_block_sets_this() {
    assert_eq!(run("class A{static{this.x=42;}}A.x;"), Value::Number(42.0));
}

#[test]
fn static_block_multiple_in_order() {
    assert_eq!(
        run("class A{static{this.c=10;}static{this.c=this.c+5;}}A.c;"),
        Value::Number(15.0)
    );
}

#[test]
fn static_block_references_class_name() {
    assert_eq!(
        run("class A{static{A.tagged=true;}}A.tagged;"),
        Value::Bool(true)
    );
}

#[test]
fn class_declaration_outer_name_is_mutable_inner_name_is_immutable() {
    assert_eq!(
        run(r#"
            class C {
              probe() { return C; }
              modify() { C = null; }
            }
            var cls = C;
            C = null;
            var outer = C === null;
            var inner = cls.prototype.probe() === cls;
            var rejected = false;
            try { cls.prototype.modify(); } catch (e) { rejected = e instanceof TypeError; }
            outer && inner && rejected;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn class_static_elements_cannot_redefine_non_configurable_prototype() {
    assert!(run_err("class C { static ['prototype']() {} }").contains("TypeError"));
    assert!(run_err("class C { static get ['prototype']() {} }").contains("TypeError"));
    assert!(run_err("class C { static set ['prototype'](v) {} }").contains("TypeError"));

    assert_eq!(
        run("class C { static ['x'](){ return 1; } } C.x();"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("class C { static ['name'](){ return 4; } } C.name();"),
        Value::Number(4.0)
    );
    assert_eq!(
        run("class C { static ['length'](){ return 5; } } C.length();"),
        Value::Number(5.0)
    );
    assert_eq!(
        run("class C { static x(){ return 1; } static x(){ return 2; } } C.x();"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("class C { static get x(){ return 2; } static set x(v){ this.v = v; } } var before = C.x; C.x = 3; before + ':' + C.v;"),
        Value::String(Arc::from("2:3"))
    );
}

#[test]
fn class_numeric_method_names_use_js_number_to_string() {
    let src = r#"
        var setValue;
        class Methods {
            0.0000001() { return "method"; }
            static 0.0000001() { return "static"; }
        }
        class Accessors {
            get 0.0000001() { return "get"; }
            set 0.0000001(v) { setValue = v; }
        }
        var methods = new Methods();
        var accessors = new Accessors();
        var getter = accessors["1e-7"];
        accessors["1e-7"] = "set";
        [methods["1e-7"](), Methods["1e-7"](), getter, setValue].join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("method,static,get,set")));
}

#[test]
fn public_class_fields_define_own_data_properties() {
    let src = r#"
        var setterCalled = false;
        class Base {
          set x(v) { setterCalled = true; }
        }
        class C extends Base {
          x = 1;
          y;
          ["z"] = 3;
          constructor() {
            super();
            this.after = this.x + this.z;
          }
        }
        var c = new C();
        [
          setterCalled,
          c.hasOwnProperty("x"),
          c.hasOwnProperty("y"),
          c.y === undefined,
          c.z,
          c.after
        ].join(",");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("false,true,true,true,3,4"))
    );
}

#[test]
fn public_class_field_direct_eval_uses_initializer_context() {
    assert_eq!(
        run(r#"
            var rejectedArguments = false;
            class ArgumentsCase {
              x = eval("arguments");
            }
            try { new ArgumentsCase(); } catch (e) {
              rejectedArguments = e instanceof SyntaxError;
            }
            rejectedArguments;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var rejectedSuperCall = false;
            class Base {}
            class Derived extends Base {
              x = eval("super()");
            }
            try { new Derived(); } catch (e) {
              rejectedSuperCall = e instanceof SyntaxError;
            }
            rejectedSuperCall;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            class C {
              x = eval("new.target");
            }
            new C().x === undefined;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            class Base {
              get value() { return 42; }
            }
            class Derived extends Base {
              x = eval("super.value");
            }
            new Derived().x;
        "#),
        Value::Number(42.0)
    );

    assert_eq!(
        run(r#"
            var rejectedArguments = false;
            try {
              class StaticArgumentsCase {
                static x = eval("arguments");
              }
            } catch (e) {
              rejectedArguments = e instanceof SyntaxError;
            }
            rejectedArguments;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            try {
              class ThrowsDuringStaticField {
                static x = (function() { throw new Error("boom"); })();
              }
            } catch (e) {}
            var ok = false;
            try {
              eval("var arguments = 1");
              ok = arguments === 1;
            } catch (e) {
              ok = false;
            }
            ok;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var rejectedNestedArguments = false;
            class NestedFunctionCase {
              x = function() { return eval("arguments"); };
            }
            try {
              new NestedFunctionCase().x();
            } catch (e) {
              rejectedNestedArguments = e instanceof SyntaxError;
            }
            rejectedNestedArguments;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn public_class_field_arrow_super_uses_field_home_object() {
    assert_eq!(
        run(r#"
            class C {
              func = () => { super.prop = "test262"; };
              static staticFunc = () => { super.staticProp = "static test262"; };
            }
            var c = new C();
            c.func();
            C.staticFunc();
            c.prop + ":" + C.staticProp;
        "#),
        Value::String(Arc::from("test262:static test262"))
    );

    assert_eq!(
        run(r#"
            class Base {
              get value() { return this.x; }
              static get staticValue() { return this.x; }
            }
            class Derived extends Base {
              x = 7;
              read = () => super.value;
              static x = 9;
              static staticRead = () => super.staticValue;
            }
            new Derived().read() + ":" + Derived.staticRead();
        "#),
        Value::String(Arc::from("7:9"))
    );

    assert!(run_err(
        "class Base{} class Derived extends Base { x = () => super(); } new Derived().x();"
    )
    .contains("SyntaxError"));
}

#[test]
fn public_class_fields_use_define_own_property_semantics() {
    assert_eq!(
        run("class C{f=Object.freeze(this);g=1;}try{new C();false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            var called = false;
            function Base() {
              return new Proxy(this, {
                defineProperty(target, key, desc) {
                  called = key === "f" && desc.value === 1;
                  throw new Error("define");
                }
              });
            }
            class C extends Base { f = 1; }
            try { new C(); false; } catch (e) { called && e.message === "define"; }
        "#),
        Value::Bool(true)
    );
}

#[test]
fn public_static_fields_and_static_name_ambiguity() {
    let src = r#"
        class A {
          static = "instance";
          static value = 2;
          static ["dyn"] = 3;
        }
        var a = new A();
        [a.static, A.value, A.dyn].join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("instance,2,3")));
}

#[test]
fn public_field_names_do_not_force_method_prefix_parsing() {
    assert_eq!(
        run("class C { set\n*g(){} } var c = new C(); [c.hasOwnProperty('set'), typeof C.prototype.g].join(',');"),
        Value::String(Arc::from("true,function"))
    );
    assert_eq!(
        run("class C { get\nx() { return 1; } } var c = new C(); [c.hasOwnProperty('get'), c.x()].join(',');"),
        Value::String(Arc::from("true,1"))
    );
    assert_eq!(
        run("class C { async } new C().hasOwnProperty('async');"),
        Value::Bool(true)
    );
}

#[test]
fn static_constructor_is_ordinary_static_method() {
    let src = r#"
        class C {
            static constructor() { return "static"; }
            constructor() { this.tag = "instance"; }
        }
        [
            C.hasOwnProperty("constructor"),
            C.prototype.hasOwnProperty("constructor"),
            C.constructor(),
            new C().tag,
            C.prototype.constructor === C.constructor
        ].join(",");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from("true,true,static,instance,false"))
    );
}

#[test]
fn class_element_early_errors_follow_static_semantics() {
    for src in [
        "class C extends () => {} {}",
        "class C extends async () => {} {}",
        "class C { constructor() {} constructor() {} }",
        "class C { get constructor() {} }",
        "class C { set constructor(v) {} }",
        "class C { static prototype() {} }",
        "class C { static get prototype() {} }",
        "class C { static set prototype(v) {} }",
        "class C { #x; m() { delete this.#x; } }",
        "class C { #x; m() { delete (this.#x); } }",
        "class C { #x; m() { var g = this.f; delete g().#x; } f() { return this; } }",
        "class C { #x; m() { var g = this.f; delete (g().#x); } f() { return this; } }",
        "class C { #constructor; }",
        "class C { static #constructor; }",
        "class C { #constructor() {} }",
        "class C { static #constructor() {} }",
        "class C { get #constructor() {} }",
        "class C { set #constructor(v) {} }",
        "class C { #x; #x; }",
        "class C { #x; #x() {} }",
        "class C { get #x() {} #x; }",
        "class C { #x() {} set #x(v) {} }",
        "class C { static #x; #x() {} }",
        "class C { get #x() {} get #x() {} }",
        "class C { set #x(v) {} set #x(v) {} }",
        "class C { m() { this.#missing; } }",
        "class C { m() { (() => this)().#missing; } }",
        "class Parent { #x; } class C extends Parent { m() { this.#x; } }",
        "class C extends B { #x() {} m() { super.#x(); } }",
    ] {
        let err = run_err(src);
        assert!(
            err.contains("SyntaxError"),
            "expected SyntaxError for {src}, got {err}"
        );
    }

    for src in [
        "try { class C extends (() => {}) {} } catch (e) { e instanceof TypeError; }",
        "try { class C extends (async () => {}) {} } catch (e) { e instanceof TypeError; }",
    ] {
        assert_eq!(run(src), Value::Bool(true));
    }

    run("class C { get #x() { return 1; } set #x(v) {} value() { return this.#x; } } new C().value();");
    run("class C { m() { class B { #x() {} } } #x() {} }");
    run("class C { #x = 2; m() { function f(o) { return o.#x; } return f(this); } } new C().m();");
    run("class C { #x = 3; m() { class Inner { read(o) { return o.#x; } } return new Inner().read(this); } } new C().m();");
}

#[test]
fn static_block_local_bindings() {
    assert_eq!(
        run("class A{static{let x=100,y=200;this.sum=x+y;}}A.sum;"),
        Value::Number(300.0)
    );
}

#[test]
fn static_block_does_not_leak_locals() {
    // locals declared in a static block must not be visible outside it
    assert_eq!(
        run("class A{static{let secret=7;this.pub=secret;}}typeof secret;"),
        Value::String(Arc::from("undefined"))
    );
}

#[test]
fn static_block_allows_super_property() {
    assert_eq!(
        run("class B{static get x(){return 4;}}class C extends B{static{this.y=super.x;}}C.y;"),
        Value::Number(4.0)
    );
}

#[test]
fn static_field_initializers_bind_this_to_constructor() {
    assert_eq!(
        run("class C{static f='test';static g=this.f+'262';static h=(()=>this.g)()+'test';}C.g+':'+C.h;"),
        Value::String(Arc::from("test262:test262test"))
    );
    assert_eq!(
        run("class C{static #self=this;static ok(){return this.#self===C;}}C.ok();"),
        Value::Bool(true)
    );
}

#[test]
fn class_field_initializers_infer_anonymous_function_names() {
    assert_eq!(
        run(r#"
            class C {
                static #sf = () => 1;
                static sf = function() {};
                #if = class {};
                inf = () => 2;
                static readStatic() { return this.#sf.name + ":" + this.sf.name; }
                readInstance() { return this.#if.name + ":" + this.inf.name; }
            }
            C.readStatic() + ":" + new C().readInstance();
        "#),
        Value::String(Arc::from("#sf:sf:#if:inf"))
    );
}

#[test]
fn class_field_initializers_reject_contains_arguments() {
    for src in [
        "class C { x = arguments; }",
        "class C { x = () => arguments; }",
        "class C { static #x = () => { var f = () => arguments; }; }",
        "class C { x = class { [arguments]() {} }; }",
        "(class { #x = typeof arguments; });",
    ] {
        assert!(run_err(src).contains("SyntaxError"), "{src}");
    }

    assert_eq!(
        run("class C { x = function() { return arguments[0]; }; } new C().x(7);"),
        Value::Number(7.0)
    );
}

#[test]
fn class_fields_reject_literal_constructor_name() {
    for src in [
        "class C { constructor; }",
        "class C { constructor = 1; }",
        "class C { 'constructor'; }",
        "class C { static constructor; }",
        "class C { static 'constructor' = 1; }",
    ] {
        assert!(run_err(src).contains("SyntaxError"), "{src}");
    }

    assert_eq!(
        run(
            "var name='constructor'; class C { [name] = 1; static [name] = 2; } var c = new C(); c.constructor + C.constructor;"
        ),
        Value::Number(3.0)
    );
}

#[test]
fn public_class_field_computed_names_evaluate_once() {
    assert_eq!(
        run(r#"
            var x = 1;
            class C {
                [x++] = x++;
                [x++] = x++;
            }
            var c1 = new C();
            var c2 = new C();
            [x, c1["1"], c1["2"], c2["1"], c2["2"]].join(",");
        "#),
        Value::String(Arc::from("7,3,4,5,6"))
    );
}

#[test]
fn public_class_field_computed_names_interleave_static_and_instance() {
    assert_eq!(
        run(r#"
            let i = 0;
            class C {
                [i++] = i++;
                static [i++] = i++;
                [i++] = i++;
            }
            let c = new C();
            [
                i,
                c["0"],
                C["1"],
                c["2"],
                c.hasOwnProperty("1"),
                C.hasOwnProperty("0"),
                C.hasOwnProperty("2")
            ].join(",");
        "#),
        Value::String(Arc::from("6,4,3,5,false,false,false"))
    );
}

#[test]
fn public_class_field_computed_name_abrupt_completion_prevents_initializers() {
    assert_eq!(
        run(r#"
            var hit = false;
            try {
                class C {
                    [missing] = (hit = true);
                    static ok = (hit = true);
                }
                false;
            } catch (e) {
                e instanceof ReferenceError && hit === false;
            }
        "#),
        Value::Bool(true)
    );
}

#[test]
fn class_computed_method_and_field_names_follow_source_order() {
    assert_eq!(
        run(r#"
            var log = [];
            class C {
                [log.push("field") || "f"];
                [log.push("method") || "m"]() {}
            }
            log.join(",");
        "#),
        Value::String(Arc::from("field,method"))
    );

    assert_eq!(
        run(r#"
            class C {
                [C.name]() { return 1; }
            }
            C.prototype.C();
        "#),
        Value::Number(1.0)
    );
}

#[test]
fn class_static_blocks_and_fields_follow_static_element_order() {
    assert_eq!(
        run(r#"
            var log = [];
            class C {
                static { log.push("block"); }
                static [log.push("key") || "x"] = log.push("value");
            }
            log.join(",");
        "#),
        Value::String(Arc::from("key,block,value"))
    );
}

#[test]
fn class_instance_fields_follow_source_order_across_public_and_private() {
    assert_eq!(
        run(r#"
            var log = [];
            class C {
                a = log.push("public");
                #b = log.push("private");
                c = log.push("public2");
            }
            new C();
            log.join(",");
        "#),
        Value::String(Arc::from("public,private,public2"))
    );
}

#[test]
fn static_block_await_identifier_contexts() {
    assert_eq!(
        run("var ok=false;class C{static{(()=>{class await{} ok=true;})();(()=>{const await=1; ok=ok&&await===1;})();}}ok;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var await=3, seen=0;class C{static{new (class{constructor(x=await){seen=x;}});}}seen;"
        ),
        Value::Number(3.0)
    );
}

#[test]
fn static_block_rejects_early_error_names_and_control() {
    for src in [
        "class C{static{await: 0;}}",
        "class C{static{class await{}}}",
        "function *g(){class C{static{yield;}}}",
        "function f(){class C{static{return;}}}",
        "class C{static{({await});}}",
        "class C{static{((x=await)=>0);}}",
        "class C{static{(class{[arguments](){}});}}",
        "class C{static{x:x:0;}}",
    ] {
        assert!(run_err(src).contains("SyntaxError"), "{src}");
    }
}

// --- private methods ---

#[test]
fn private_method_called() {
    assert_eq!(
        run("class C{#inc(){return 1;}g(){return this.#inc();}}new C().g();"),
        Value::Number(1.0)
    );
}

#[test]
fn private_method_function_names_include_hash() {
    assert_eq!(
        run("class C{#m(){} get(){return this.#m;}}new C().get().name;"),
        Value::String(Arc::from("#m"))
    );
    assert_eq!(
        run("class C{static #m(){} static get(){return this.#m;}}C.get().name;"),
        Value::String(Arc::from("#m"))
    );
}

#[test]
fn private_method_functions_are_shared_across_instances() {
    assert_eq!(
        run("class C{#m(){return 42;}get ref(){return this.#m;}}let a=new C(),b=new C();a.ref===b.ref&&a.ref.name==='#m';"),
        Value::Bool(true)
    );
}

#[test]
fn shared_private_methods_keep_super_home_object() {
    assert_eq!(
        run("class B{m(){return this.x;}}class C extends B{constructor(x){super();this.x=x;}#m(){return super.m();}call(){return this.#m;}value(){return this.#m();}}let a=new C(1),b=new C(2);a.call()===b.call()&&a.value()===1&&b.value()===2;"),
        Value::Bool(true)
    );
}

#[test]
fn private_async_and_generator_method_heads_parse() {
    assert_eq!(
        run("class C{async #m(){return 1;} get(){return this.#m;}}new C().get().name;"),
        Value::String(Arc::from("#m"))
    );
    assert_eq!(
        run("class C{* #m(){yield 1;} get(){return this.#m;}}new C().get().name;"),
        Value::String(Arc::from("#m"))
    );
    assert_eq!(
        run("class C{async * #m(){yield 1;} get(){return this.#m;}}new C().get().name;"),
        Value::String(Arc::from("#m"))
    );
    assert_eq!(
        run("class C{static async #m(){return 1;} static get(){return this.#m;}}C.get().name;"),
        Value::String(Arc::from("#m"))
    );
}

#[test]
fn private_method_mutates_field() {
    assert_eq!(
        run("class C{#c=0;#inc(){this.#c++;}bump(){this.#inc();this.#inc();}get v(){return this.#c;}}let c=new C();c.bump();c.v;"),
        Value::Number(2.0)
    );
}

#[test]
fn private_method_with_args() {
    assert_eq!(
        run("class C{#add(a,b){return a+b;}sum(){return this.#add(3,4);}}new C().sum();"),
        Value::Number(7.0)
    );
}

#[test]
fn private_method_calls_another_private() {
    assert_eq!(
        run(
            "class C{#a(){return 1;}#b(){return this.#a()+1;}c(){return this.#b()+1;}}new C().c();"
        ),
        Value::Number(3.0)
    );
}

#[test]
fn private_field_increment() {
    assert_eq!(
        run("class C{#c=5;inc(){this.#c++;}get v(){return this.#c;}}let c=new C();c.inc();c.v;"),
        Value::Number(6.0)
    );
}

#[test]
fn private_field_update_evaluates_object_once() {
    assert_eq!(
        run("var calls=0; class C{#c=1;m(){function f(){calls++;return this;} f.call(this).#c++; return calls + ':' + this.#c;}} new C().m();"),
        Value::String(Arc::from("1:2"))
    );
}

#[test]
fn private_field_compound_assignment_updates_field() {
    assert_eq!(
        run("class C{#c=1;m(){this.#c+=2;return this.#c;}} new C().m();"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("var calls=0; class C{#c=4;m(){function f(){calls++;return this;} f.call(this).#c*=3; return calls + ':' + this.#c;}} new C().m();"),
        Value::String(Arc::from("1:12"))
    );
    assert_eq!(
        run("class C{#v=1;get #c(){return this.#v;}set #c(v){this.#v=v;}m(){this.#c+=2;return this.#v;}} new C().m();"),
        Value::Number(3.0)
    );
}

#[test]
fn private_name_slash_is_division_or_divide_assignment() {
    assert_eq!(
        run("class C { #x = 4; m() { return this.#x / 2; } } new C().m();"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("class C { #x = 4; m() { return this.#x /= 2; } } new C().m();"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("class C { #v = 4; get #x() { return this.#v; } set #x(v) { this.#v = v; } m() { return this.#x /= 2; } value() { return this.#v; } } var c = new C(); [c.m(), c.value()].join(',');"),
        Value::String(Arc::from("2,2"))
    );
}

#[test]
fn private_field_logical_assignment_updates_and_short_circuits() {
    assert_eq!(
        run("class C{#c=0;m(){this.#c ||= 5; return this.#c;}} new C().m();"),
        Value::Number(5.0)
    );
    assert_eq!(
        run("class C{#c=1;m(){this.#c &&= 5; return this.#c;}} new C().m();"),
        Value::Number(5.0)
    );
    assert_eq!(
        run("var calls=0; class C{#c=7;m(){function rhs(){calls++;return 9;} var r = (this.#c ||= rhs()); return r + ':' + this.#c + ':' + calls;}} new C().m();"),
        Value::String(Arc::from("7:7:0"))
    );
    assert_eq!(
        run("var calls=0; class C{#c=1;m(){function f(){calls++;return this;} f.call(this).#c &&= 9; return calls + ':' + this.#c;}} new C().m();"),
        Value::String(Arc::from("1:9"))
    );
}

#[test]
fn private_field_set_in_method() {
    assert_eq!(
        run("class C{#c=0;set(v){this.#c=v;}get v(){return this.#c;}}let c=new C();c.set(99);c.v;"),
        Value::Number(99.0)
    );
}

#[test]
fn private_accessors_and_non_extensible_private_slots() {
    assert_eq!(
        run("class C{get #x(){return 42;}get y(){return this.#x;}}new C().y;"),
        Value::Number(42.0)
    );
    assert_eq!(
        run("class C{get #x(){return 1;}setX(){this.#x=2;}}try{new C().setX();false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class C{set #x(v){}getX(){return this.#x;}}try{new C().getX();false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );

    assert_eq!(
        run("class B{constructor(){Object.preventExtensions(this);}}class C extends B{#x;constructor(){super();}}try{new C();false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );

    assert_eq!(
        run("try{class C{static #x=(Object.preventExtensions(C),1);}false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
}

#[test]
fn private_elements_reject_duplicate_initialization_on_same_object() {
    assert_eq!(
        run("class B{constructor(o){return o;}}class C extends B{#x;}var o={};new C(o);try{new C(o);false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class B{constructor(o){return o;}}class C extends B{#m(){}}var o={};new C(o);try{new C(o);false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class B{constructor(o){return o;}}class C extends B{get #x(){return 1;}}var o={};new C(o);try{new C(o);false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class B{constructor(o){return o;}}class C extends B{get #x(){return 1;}set #x(v){}}var o={};new C(o);try{new C(o);false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
}

#[test]
fn private_brand_checks_reject_missing_slots() {
    assert_eq!(
        run("class C{#x=1;read(o){return o.#x;}}try{new C().read({});false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class C{#x=1;write(o){o.#x=2;}}try{new C().write({});false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class C{#x=1;inc(o){o.#x++;}}try{new C().inc({});false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class C{#m(){return 1;}call(o){return o.#m();}}try{new C().call({});false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class C{get #x(){return 1;}read(o){return o.#x;}}try{new C().read({});false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class C{static #x=1;static read(o){return o.#x;}}try{C.read({});false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class C{#x=1;read(o){return o.#x;}}try{new C().read(1);false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
}

#[test]
fn private_names_are_unique_per_class_evaluation() {
    assert_eq!(
        run(r#"
            function factory() {
              return class {
                #x = 1;
                read(o) { return o.#x; }
              };
            }
            var C1 = factory();
            var C2 = factory();
            var c1 = new C1();
            var c2 = new C2();
            var ok = c1.read(c1) === 1 && c2.read(c2) === 1;
            try { c1.read(c2); ok = false; } catch (e) { ok = ok && e instanceof TypeError; }
            try { c2.read(c1); ok = false; } catch (e) { ok = ok && e instanceof TypeError; }
            ok;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            function make() {
              class C {
                #m() { return "ok"; }
                access(o) { return o.#m(); }
              }
              return new C();
            }
            var c1 = make();
            var c2 = make();
            var ok = c1.access(c1) === "ok" && c2.access(c2) === "ok";
            try { c1.access(c2); ok = false; } catch (e) { ok = ok && e instanceof TypeError; }
            try { c2.access(c1); ok = false; } catch (e) { ok = ok && e instanceof TypeError; }
            ok;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            function make() {
              class C {
                get #x() { return "get"; }
                read(o) { return o.#x; }
              }
              return new C();
            }
            var c1 = make();
            var c2 = make();
            var ok = c1.read(c1) === "get" && c2.read(c2) === "get";
            try { c1.read(c2); ok = false; } catch (e) { ok = ok && e instanceof TypeError; }
            try { c2.read(c1); ok = false; } catch (e) { ok = ok && e instanceof TypeError; }
            ok;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn private_names_with_same_spelling_coexist_across_inheritance() {
    assert_eq!(
        run(r#"
            class Base {
              #x = "base";
              readBase(o) { return o.#x; }
            }
            class Sub extends Base {
              #x = "sub";
              readSub(o) { return o.#x; }
            }
            var s = new Sub();
            s.readBase(s) + ":" + s.readSub(s);
        "#),
        Value::String(Arc::from("base:sub"))
    );
}

#[test]
fn private_methods_are_not_writable() {
    assert_eq!(
        run("class C{#m(){}set(){this.#m=1;}}try{new C().set();false;}catch(e){e instanceof TypeError;}"),
        Value::Bool(true)
    );
}

#[test]
fn private_names_follow_identifier_name_grammar() {
    assert_eq!(
        run(
            "class C { get #\\u{6F}() { return 1; } value() { return this.#o; } } new C().value();"
        ),
        Value::Number(1.0)
    );
    assert_eq!(
        run("class C { #℘ = 2; get() { return this.#℘; } } new C().get();"),
        Value::Number(2.0)
    );
    assert_eq!(
        run(
            "class C { #ZW_\\u200C_NJ = 3; get() { return this.#ZW_\u{200C}_NJ; } } new C().get();"
        ),
        Value::Number(3.0)
    );
    assert_eq!(
        run("class C { static #ZW_\\u200D_J = 4; static get() { return this.#ZW_\u{200D}_J; } } C.get();"),
        Value::Number(4.0)
    );
}
