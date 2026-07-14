//! Class features: static initialization blocks and private methods/fields.

mod common;
use common::{run, run_err};
use ruja::{Value, Vm};
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
        "class C { async constructor() {} }",
        "class C { * constructor() {} }",
        "class C { async * constructor() {} }",
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
        "class C { get x(value = 1) {} }",
        "class C { get #x(value) {} }",
        "class C { set x() {} }",
        "class C { set x(a, b) {} }",
        "class C { set #x(...values) {} }",
        "class C { get #x() {} static set #x(v) {} }",
        "class C { set #x(v) {} static get #x() {} }",
        "class C { static get #x() {} set #x(v) {} }",
        "class C { static set #x(v) {} get #x() {} }",
        "class C { get #x() {} set #x(v) {} get #x() {} }",
        "class C { get #\\u0078() {} static set #x(v) {} }",
        "(class { static get #x() {} set #x(v) {} });",
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
    run("class C { set #x(v) {} get #x() { return 1; } } new C();");
    run("class C { static get #x() { return 1; } static set #x(v) {} } C;");
    run("class C { static set #x(v) {} static get #x() { return 1; } } C;");
    run("class C { set x(value = 1) { this.value = value; } } var c = new C(); c.x = undefined;");
    run("class C { set #x({ value }) { this.value = value; } write(value) { this.#x = value; } } var c = new C(); c.write({ value: 1 });");
    run("class C { get #x() {} set #x(v) {} m() { class D { static get #x() {} static set #x(v) {} } return D; } } new C().m();");
    run("class C { m() { class B { #x() {} } } #x() {} }");
    run("class C { #x = 2; m() { function f(o) { return o.#x; } return f(this); } } new C().m();");
    run("class C { #x = 3; m() { class Inner { read(o) { return o.#x; } } return new Inner().read(this); } } new C().m();");
    run("class C { static async constructor() {} constructor() {} } typeof C.constructor;");
    run("class C { static * constructor() {} constructor() {} } typeof C.constructor;");
}

#[test]
fn symbol_can_be_extended_but_symbol_construction_throws() {
    assert_eq!(
        run(r#"
            class SymbolSubclass extends Symbol {}
            var direct = false;
            var derived = false;
            try { new Symbol(); } catch (error) { direct = error instanceof TypeError; }
            try { new SymbolSubclass(); } catch (error) { derived = error instanceof TypeError; }
            direct && derived && typeof Symbol("ok") === "symbol";
        "#),
        Value::Bool(true)
    );
}

#[test]
fn async_and_generator_superclasses_fail_before_prototype_lookup() {
    assert_eq!(
        run(r#"
            var prototypeGets = 0;
            function rejects(value) {
              try { class Derived extends value {} return false; }
              catch (error) { return error instanceof TypeError; }
            }
            function check(value) {
              if (!rejects(value)) return false;
              var bound = value.bind();
              Object.defineProperty(bound, "prototype", {
                get() { prototypeGets++; throw new Error("unreachable"); }
              });
              if (!rejects(bound)) return false;
              var proxy = new Proxy(value, {
                get() { prototypeGets++; throw new Error("unreachable"); }
              });
              return rejects(proxy);
            }

            var asyncFunction = async function() {};
            var generatorFunction = function*() {};
            var asyncGeneratorFunction = async function*() {};
            var prototypeShape =
              !asyncFunction.hasOwnProperty("prototype") &&
              generatorFunction.hasOwnProperty("prototype") &&
              asyncGeneratorFunction.hasOwnProperty("prototype");
            prototypeShape && check(asyncFunction) && check(generatorFunction) &&
              check(asyncGeneratorFunction) && prototypeGets === 0;
        "#),
        Value::Bool(true)
    );
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
            try {
                class C {
                    [C.name]() { return 1; }
                }
                false;
            } catch (error) {
                error instanceof ReferenceError;
            }
        "#),
        Value::Bool(true)
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
fn private_methods_and_accessors_initialize_before_fields() {
    assert_eq!(
        run(r#"
            var log = [];
            class C {
              a = (log.push("a"), this.#m());
              seen = 0;
              b = (log.push("b"), this.#g);
              trigger = (this.#s = 9);
              #m() { return 42; }
              get #g() { return 7; }
              set #s(value) { this.seen = value; }
            }
            var c = new C();
            [c.a, c.b, c.seen, log.join(",")].join(":");
        "#),
        Value::String(Arc::from("42:7:9:a,b"))
    );
    assert_eq!(
        run(r#"
            class Base { constructor(receiver) { return receiver; } }
            class Derived extends Base {
              value = this.#m();
              #m() { return 42; }
              constructor(receiver) { super(receiver); }
            }
            new Derived({}).value;
        "#),
        Value::Number(42.0)
    );
}

#[test]
fn private_destructuring_targets_preserve_reference_evaluation_order() {
    assert_eq!(
        run(r#"
            class Base { constructor(receiver) { return receiver; } }
            class C extends Base {
              #field;
              assign() {
                var initialize = () => new C(this);
                var source = { get value() { initialize(); return "pass"; } };
                ({ value: this.#field } = source);
                return this.#field;
              }
            }
            C.prototype.assign.call({});
        "#),
        Value::String(Arc::from("pass"))
    );
    assert_eq!(
        run(r#"
            var getterCalled = false;
            class C extends class {} {
              #field;
              constructor() {
                var source = { get value() { getterCalled = true; } };
                ({ value: this.#field } = source);
              }
            }
            var referenceError = false;
            try { new C(); } catch (error) { referenceError = error instanceof ReferenceError; }
            referenceError && !getterCalled;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn private_assignment_targets_apply_brand_checks_through_put_value() {
    assert_eq!(
        run(r#"
            class C {
              #field;
              array(object) { [object.#field] = [1]; }
              arrayRest(object) { [...object.#field] = []; }
              object(object) { ({ value: object.#field } = { value: 1 }); }
              objectRest(object) { ({ ...object.#field } = {}); }
              forOf(object) { for (object.#field of [1]) {} }
              forIn(object) { for (object.#field in { value: 1 }) {} }
            }
            var instance = new C();
            var methods = ["array", "arrayRest", "object", "objectRest", "forOf", "forIn"];
            methods.every(function(name) {
              try { instance[name]({}); return false; }
              catch (error) { return error instanceof TypeError; }
            });
        "#),
        Value::Bool(true)
    );
}

#[test]
fn private_elements_stamp_proxy_and_exotic_receivers() {
    assert_eq!(
        run(r#"
            class Base { constructor(receiver) { return receiver; } }
            class Stamp extends Base {
              #field = { value: 40 };
              #method() { return this.#field.value + 1; }
              get #accessor() { return this.#method() + 1; }
              set #accessor(value) { this.#field = { value: value - 2 }; }
              constructor(receiver) { super(receiver); }
              static read(receiver) { return receiver.#accessor; }
              static write(receiver, value) { receiver.#accessor = value; }
            }

            var trapCalls = 0;
            var proxy = new Proxy({}, {
              get() { trapCalls++; },
              set() { trapCalls++; },
              defineProperty() { trapCalls++; },
              isExtensible() { trapCalls++; }
            });
            var revoked = Proxy.revocable({}, {});
            revoked.revoke();
            var receivers = [
              proxy,
              revoked.proxy,
              [],
              new Map(),
              new Set(),
              new WeakMap(),
              new WeakSet(),
              Promise.resolve(1),
              new ArrayBuffer(4),
              new DataView(new ArrayBuffer(4)),
              new Uint8Array(4),
              [1][Symbol.iterator](),
              function() {}
            ];
            var ok = receivers.every(function(receiver) {
              new Stamp(receiver);
              if (Stamp.read(receiver) !== 42) return false;
              Stamp.write(receiver, 52);
              return Stamp.read(receiver) === 52;
            });
            ok && trapCalls === 0;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            class Base { constructor(receiver) { return receiver; } }
            class Stamp extends Base {
              #field = { value: 42 };
              constructor(receiver) { super(receiver); }
              static read(receiver) { return receiver.#field.value; }
            }
            var receiver = new Proxy({}, {});
            new Stamp(receiver);
            for (var i = 0; i < 3000; i++) ({ index: i });
            Stamp.read(receiver);
        "#),
        Value::Number(42.0)
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
fn async_private_method_results_are_assimilated_by_wrappers() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        class C {
            async #instance(value) { return async () => value; }
            async instance(value) { return this.#instance(value); }

            static async #static(value) {
                return async function() { return value; };
            }
            static async static(value) { return this.#static(value); }
        }
        async function thenable() {
            return { then(resolve) { resolve(3); } };
        }

        var actual = [];
        new C().instance(1).then(fn => fn()).then(value => actual.push(value));
        C.static(2).then(fn => fn()).then(value => actual.push(value));
        thenable().then(value => actual.push(value));
        "#,
    )
    .expect("async result assimilation failed");

    assert_eq!(
        vm.run("actual.sort().join('|');")
            .expect("failed to read async assimilation results"),
        Value::String(Arc::from("1|2|3"))
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
fn private_calls_use_retained_references() {
    assert_eq!(
        run(r#"
            var log = [];
            var instance;
            class C {
              #callable = function() { return 1; };
              #join(...values) {
                return (this === instance ? "bound:" : "unbound:") +
                  values.join(",");
              }
              get #accessor() {
                log.push("get");
                return function(value) {
                  log.push("call:" + (this === instance));
                  return value;
                };
              }
              snapshot() {
                return this.#callable(
                  this.#callable = function() { return 2; }
                );
              }
              rejectBeforeArguments(value) {
                return value.#join(log.push("argument"));
              }
              callAccessor() {
                return this.#accessor((log.push("argument"), "ok"));
              }
              directSpread() { return this.#join(0, ...[1, 2], 3); }
              optionalSpread() { return this.#join?.(...[4, 5]); }
              groupedOptionalSpread() { return (this?.#join)(...[6, 7]); }
              static nullable(value) { return value?.#join(...[8]); }
            }
            instance = new C();
            var snapshot = instance.snapshot();
            try { instance.rejectBeforeArguments({}); }
            catch (error) { log.push(error.name); }
            [
              snapshot,
              instance.callAccessor(),
              instance.directSpread(),
              instance.optionalSpread(),
              instance.groupedOptionalSpread(),
              C.nullable(null),
              C.nullable(instance),
              log.join("|")
            ].join(";");
            "#),
        Value::String(Arc::from(
            "1;ok;bound:0,1,2,3;bound:4,5;bound:6,7;;bound:8;TypeError|get|argument|call:true"
        ))
    );
}

#[test]
fn optional_private_calls_preserve_chain_boundaries_and_order() {
    assert_eq!(
        run(r#"
            var log = [];
            class C {
              #nullable = null;
              #nonCallable = 1;
              #mutable = function() { return 1; };
              #method() { return { value: 3 }; }
              get #throwing() {
                log.push("get:throwing");
                throw "boom";
              }
              nullishCalls() {
                return [
                  this.#nullable?.(log.push("direct-argument")),
                  this.#nullable?.(...[(log.push("spread-argument"), 1)])
                ];
              }
              snapshot() {
                return this.#mutable?.(...[
                  (this.#mutable = function() { return 2; }, 0)
                ]);
              }
              continuation() {
                return [this.#method().value, this.#nullable?.().value];
              }
              nonCallable() {
                try { this.#nonCallable?.(log.push("noncallable-argument")); }
                catch (error) { log.push(error.name); }
              }
              throwing() {
                try { this.#throwing?.(log.push("throwing-argument")); }
                catch (error) { log.push(error); }
              }
              static combined(value) {
                return value?.#nullable?.(log.push("combined-argument"));
              }
              static grouped(value) {
                return (value?.#nullable)?.(log.push("grouped-argument"));
              }
              static continued(value) { return value?.#method().value; }
              static groupedBreak(value) { return (value?.#method)().value; }
              static wrongBrand(value) {
                return value?.#nullable?.(log.push("wrong-brand-argument"));
              }
            }
            var instance = new C();
            var nullish = instance.nullishCalls();
            var continuation = instance.continuation();
            var groupedBreak;
            var wrongBrand;
            instance.nonCallable();
            instance.throwing();
            try { C.groupedBreak(null); }
            catch (error) { groupedBreak = error.name; }
            try { C.wrongBrand({}); }
            catch (error) { wrongBrand = error.name; }
            [
              nullish[0], nullish[1],
              C.combined(null), C.combined(instance),
              C.grouped(null), C.grouped(instance),
              C.continued(null), C.continued(instance),
              continuation[0], continuation[1],
              groupedBreak, wrongBrand, instance.snapshot(),
              log.join("|")
            ].join(";");
            "#),
        Value::String(Arc::from(
            ";;;;;;;3;3;;TypeError;TypeError;1;noncallable-argument|TypeError|get:throwing|boom"
        ))
    );
}

#[test]
fn private_call_references_survive_argument_gc() {
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
            class C {
              #value = 9;
              get #callable() {
                return function(...values) { return this.#value + values.length; };
              }
              static calls(make) {
                return [
                  make().#callable(...[(forceGc(), 1)]),
                  make().#callable?.(...[(forceGc(), 1), (forceGc(), 2)]),
                  (make()?.#callable)(
                    ...[(forceGc(), 1), (forceGc(), 2), (forceGc(), 3)]
                  )
                ];
              }
            }
            C.calls(function() { return new C(); }).join(",");
            "#,
        )
        .expect("private Reference and callee should survive argument GC"),
        Value::String(Arc::from("10,11,12"))
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
fn private_field_updates_use_retained_references() {
    assert_eq!(
        run(r#"
            var log = [];
            class C {
              #value = 1;
              #big = 1n;
              #accessorValue = 0;
              base() { log.push("base"); return this; }
              get #accessor() {
                log.push("get");
                return { valueOf() { log.push("coerce"); return 4; } };
              }
              set #accessor(value) {
                log.push("set:" + value);
                this.#accessorValue = value;
              }
              updates() {
                var post = this.base().#value++;
                var pre = ++this.base().#value;
                var bigPost = this.#big++;
                var bigPre = --this.#big;
                var accessorPost = this.#accessor++;
                return [
                  post, pre, this.#value,
                  bigPost, bigPre, this.#big,
                  accessorPost, this.#accessorValue,
                  log.join("|")
                ].join(";");
              }
              static wrongBrand(value) { return value.#value++; }
            }
            var result = new C().updates();
            var wrongBrand;
            try { C.wrongBrand({}); }
            catch (error) { wrongBrand = error.name; }
            result + ";" + wrongBrand;
            "#),
        Value::String(Arc::from(
            "1;3;3;1;1;1;4;5;base|base|get|coerce|set:5;TypeError"
        ))
    );
}

#[test]
fn private_field_update_number_and_bigint_matrix() {
    assert_eq!(
        run(r#"
            class C {
              #number = 10;
              #bigint = 10n;
              matrix() {
                var values = [
                  this.#number++, ++this.#number,
                  this.#number--, --this.#number, this.#number,
                  this.#bigint++, ++this.#bigint,
                  this.#bigint--, --this.#bigint, this.#bigint
                ];
                return values.join(",");
              }
            }
            new C().matrix();
            "#),
        Value::String(Arc::from("10,12,12,10,10,10,12,12,10,10"))
    );
}

#[test]
fn private_field_update_coercion_errors_preserve_order() {
    assert_eq!(
        run(r#"
            var log = [];
            class C {
              #value;
              constructor() {
                this.#value = {
                  valueOf: () => {
                    log.push("coerce-mutate");
                    this.#value = 99;
                    return 4;
                  }
                };
              }
              mutate() {
                var old = this.#value++;
                return old + ":" + this.#value;
              }
              resetThrowing() {
                this.#value = {
                  valueOf() { log.push("coerce-throw"); throw "boom"; }
                };
              }
              throwing() { return ++this.#value; }
              get #readonly() { log.push("get-readonly"); return 1; }
              readonly() { return this.#readonly++; }
              static wrongBrand(value) { return ++value.#value; }
            }
            var instance = new C();
            var mutated = instance.mutate();
            instance.resetThrowing();
            var throwing;
            var readonly;
            var wrongBrand;
            try { instance.throwing(); } catch (error) { throwing = error; }
            try { instance.readonly(); } catch (error) { readonly = error.name; }
            try { C.wrongBrand({}); } catch (error) { wrongBrand = error.name; }
            [mutated, throwing, readonly, wrongBrand, log.join("|")].join(";");
            "#),
        Value::String(Arc::from(
            "4:5;boom;TypeError;TypeError;coerce-mutate|coerce-throw|get-readonly"
        ))
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
fn private_compound_assignments_preserve_reference_and_order() {
    assert_eq!(
        run(r#"
            var log = [];
            class C {
              #backing = 0;
              #value = 2;
              get #accessor() {
                log.push("get");
                return { valueOf() { log.push("left"); return 2; } };
              }
              set #accessor(value) {
                log.push("set:" + value);
                this.#backing = value;
              }
              #method() {}
              ordered() {
                var result = this.#accessor += (
                  log.push("rhs"),
                  { valueOf() { log.push("right"); return 3; } }
                );
                return result + ":" + this.#backing;
              }
              mutated() {
                var result = this.#value += (this.#value = 100, 3);
                return result + ":" + this.#value;
              }
              readonly() {
                try { this.#method += (log.push("method-rhs"), 1); }
                catch (error) { return error.name; }
              }
              static wrongBrand(value) {
                return value.#value += log.push("wrong-brand-rhs");
              }
            }
            var instance = new C();
            var wrongBrand;
            try { C.wrongBrand({}); }
            catch (error) { wrongBrand = error.name; }
            [
              instance.ordered(), instance.mutated(), instance.readonly(),
              wrongBrand, log.join("|")
            ].join(";");
            "#),
        Value::String(Arc::from(
            "5:5;5:5;TypeError;TypeError;get|rhs|left|right|set:5|method-rhs"
        ))
    );
}

#[test]
fn private_compound_assignment_errors_skip_setters() {
    assert_eq!(
        run(r#"
            var log = [];
            class C {
              get #numeric() { log.push("get-numeric"); return 2n; }
              set #numeric(value) { log.push("set-numeric"); }
              get #power() { log.push("get-power"); return 2n; }
              set #power(value) { log.push("set-power"); }
              mixed() { return this.#numeric += 1; }
              negativeExponent() { return this.#power **= -1n; }
            }
            var instance = new C();
            var mixed;
            var exponent;
            try { instance.mixed(); } catch (error) { mixed = error.name; }
            try { instance.negativeExponent(); }
            catch (error) { exponent = error.name; }
            [mixed, exponent, log.join("|")].join(";");
            "#),
        Value::String(Arc::from("TypeError;RangeError;get-numeric|get-power"))
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
fn private_logical_assignments_preserve_reference_and_short_circuit() {
    assert_eq!(
        run(r#"
            var log = [];
            function rhs(name, value) { log.push("rhs:" + name); return value; }
            class C {
              #backing = 0;
              #value = 0;
              get #accessor() { log.push("get:" + this.#backing); return this.#backing; }
              set #accessor(value) { log.push("set:" + value); this.#backing = value; }
              assignments() {
                var andSkip = this.#accessor &&= rhs("and", 1);
                var orAssign = this.#accessor ||= rhs("or", 2);
                var nullishSkip = this.#accessor ??= rhs("nullish-skip", 3);
                this.#backing = null;
                var nullishAssign = this.#accessor ??= rhs("nullish", 4);
                var mutated = this.#value ||= (this.#value = 99, 5);
                return [
                  andSkip, orAssign, nullishSkip, nullishAssign,
                  mutated, this.#value
                ].join(":");
              }
              static wrongBrand(value) {
                return value.#value ||= log.push("wrong-brand-rhs");
              }
              static wrongBrandAnd(value) {
                return value.#value &&= log.push("wrong-brand-and-rhs");
              }
              static wrongBrandNullish(value) {
                return value.#value ??= log.push("wrong-brand-nullish-rhs");
              }
            }
            var instance = new C();
            var wrongBrand = [];
            try { C.wrongBrand({}); }
            catch (error) { wrongBrand.push(error.name); }
            try { C.wrongBrandAnd({}); }
            catch (error) { wrongBrand.push(error.name); }
            try { C.wrongBrandNullish({}); }
            catch (error) { wrongBrand.push(error.name); }
            [instance.assignments(), wrongBrand.join(","), log.join("|")].join(";");
            "#),
        Value::String(Arc::from(
            "0:2:2:4:5:5;TypeError,TypeError,TypeError;get:0|get:0|rhs:or|set:2|get:2|get:null|rhs:nullish|set:4"
        ))
    );
}

#[test]
fn private_read_modify_write_errors_use_the_callee_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var ForeignClass = other.eval(`(class {
              #value = 1;
              #method() {}
              update(object) { return object.#value++; }
              compound(object) { return object.#value += 1; }
              logical(object) { return object.#value ||= 1; }
              readonly() { return this.#method += 1; }
            })`);
            var instance = new ForeignClass();
            [
              function() { instance.update({}); },
              function() { instance.compound({}); },
              function() { instance.logical({}); },
              function() { instance.readonly(); }
            ].every(function(check) {
              try { check(); return false; }
              catch (error) {
                return error instanceof other.TypeError &&
                  !(error instanceof TypeError);
              }
            });
            "#),
        Value::Bool(true)
    );
}

#[test]
fn private_read_modify_write_references_survive_gc() {
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
            class C {
              #xValue = 1;
              #yValue = 2;
              #zValue = 0;
              get #x() {
                return { valueOf() { forceGc(); return 1; } };
              }
              set #x(value) { forceGc(); this.#xValue = value; }
              get #y() { forceGc(); return this.#yValue; }
              set #y(value) { forceGc(); this.#yValue = value; }
              get #z() { forceGc(); return this.#zValue; }
              set #z(value) { forceGc(); this.#zValue = value; }
              static update(make) { return make().#x++; }
              static compound(make) { return make().#y += (forceGc(), 3); }
              static logical(make) { return make().#z ||= (forceGc(), 4); }
            }
            function make() { return new C(); }
            [C.update(make), C.compound(make), C.logical(make)].join(",");
            "#,
        )
        .expect("private read-modify-write References should survive GC"),
        Value::String(Arc::from("1,5,4"))
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
fn interpreted_runtime_errors_use_the_callee_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var classBody = `class {
              #m() { return 1; }
              get #g() { return 2; }
              set #s(value) {}
              method(object) { return object.#m(); }
              getter(object) { return object.#g; }
              setter(object) { object.#s = 3; }
            }`;
            var EvalClass = other.eval("(" + classBody + ")");
            var FunctionClass = (new other.Function("return " + classBody))();

            function hasForeignBrandErrors(Class) {
              var instance = new Class();
              var checks = [
                function() { instance.method({}); },
                function() { instance.getter({}); },
                function() { instance.setter({}); }
              ];
              return checks.every(function(check) {
                try {
                  check();
                  return false;
                } catch (error) {
                  return error instanceof other.TypeError &&
                    !(error instanceof TypeError);
                }
              });
            }

            var readNull = other.eval("(function(object) { return object.x; })");
            var ordinaryRealm = false;
            try { readNull(null); }
            catch (error) {
              ordinaryRealm = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }

            var marker = other.eval("({ marker: true })");
            var throwMarker = other.eval("(function(value) { throw value; })");
            var explicitThrow = false;
            try { throwMarker(marker); }
            catch (error) { explicitThrow = error === marker; }

            var catchInsideNativeCallback = other.eval(`(function(MainTypeError) {
              try { null.x; }
              catch (error) {
                return error instanceof TypeError &&
                  !(error instanceof MainTypeError);
              }
            })`);
            var nestedNativeCallback = [0].every(function() {
              return catchInsideNativeCallback(TypeError);
            });

            var foreignGenerator = other.eval("(function*() { null.x; })")();
            var mainGeneratorNext = Object.getPrototypeOf(function*() {}()).next;
            var generatorRealm = false;
            try { mainGeneratorNext.call(foreignGenerator); }
            catch (error) {
              generatorRealm = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }

            var ForeignGenerator = other.eval(
              "(function*(value = null.x) {})"
            );
            var generatorParameterRealm = false;
            try { ForeignGenerator(); }
            catch (error) {
              generatorParameterRealm = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }

            hasForeignBrandErrors(EvalClass) &&
              hasForeignBrandErrors(FunctionClass) &&
              ordinaryRealm && explicitThrow && nestedNativeCallback &&
              generatorRealm && generatorParameterRealm;
            "#),
        Value::Bool(true)
    );

    let mut async_vm = Vm::new().expect("failed to initialize async Realm VM");
    async_vm
        .run(
            r#"
            var other = $262.createRealm().global;
            var asyncFail = other.eval("(async function() { null.x; })");
            var asyncAfterAwait = other.eval(
              "(async function() { await 0; null.x; })"
            );
            var asyncGenerator = other.eval(
              "(async function*() { await 0; null.x; })"
            )();
            var mainAsyncGeneratorNext = Object.getPrototypeOf(
              (async function*() {})()
            ).next;
            var asyncRealm = false;
            var asyncAfterAwaitRealm = false;
            var asyncGeneratorRealm = false;
            asyncFail().catch(function(error) {
              asyncRealm = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            });
            asyncAfterAwait().catch(function(error) {
              asyncAfterAwaitRealm = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            });
            mainAsyncGeneratorNext.call(asyncGenerator).catch(function(error) {
              asyncGeneratorRealm = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            });
            "#,
        )
        .expect("foreign async rejection should run");
    assert_eq!(
        async_vm
            .run("asyncRealm && asyncAfterAwaitRealm && asyncGeneratorRealm;")
            .expect("foreign async rejection handler should settle"),
        Value::Bool(true)
    );

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
            var other = $262.createRealm().global;
            var Class = other.eval(`(class {
              #x = 1;
              read(object, collect) {
                collect();
                return object.#x;
              }
            })`);
            var instance = new Class();
            try {
              instance.read({}, forceGc);
              false;
            } catch (error) {
              error instanceof other.TypeError && !(error instanceof TypeError);
            }
            "#
        )
        .expect("foreign Realm error should survive frame-boundary GC"),
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
fn private_names_are_visible_to_direct_eval_in_class_contexts() {
    assert_eq!(
        run(r#"
            class Field {
              #m = 44;
              read(o) { return eval("o.#m"); }
            }
            class Other { #m = 44; }
            var field = new Field();
            var ok = field.read(field) === 44;
            try { field.read(new Other()); ok = false; } catch (e) { ok = ok && e instanceof TypeError; }
            ok;
        "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            class Initializer {
              #m = 41;
              value = eval("this.#m + 1");
            }
            new Initializer().value;
        "#),
        Value::Number(42.0)
    );

    assert_eq!(
        run(r#"
            class Methods {
              #m() { return "method"; }
              get #g() { return "get"; }
              set #s(v) { this.value = v; }
              read() {
                eval("this.#s = this.#m() + ':' + this.#g");
                return this.value;
              }
            }
            new Methods().read();
        "#),
        Value::String(Arc::from("method:get"))
    );

    assert_eq!(
        run(r#"
            class StaticElements {
              static #f = 1;
              static #m() { return 2; }
              static get #g() { return 3; }
              static set #s(v) { this.value = v; }
              static read() {
                eval("this.#s = this.#f + this.#m() + this.#g");
                return this.value;
              }
            }
            class Other {
              static #f = 1;
              static #m() { return 2; }
              static get #g() { return 3; }
              static set #s(v) { this.value = v; }
            }
            var ok = StaticElements.read() === 6;
            try { StaticElements.read.call(Other); ok = false; } catch (e) { ok = ok && e instanceof TypeError; }
            ok;
        "#),
        Value::Bool(true)
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
fn optional_chains_support_private_fields_and_methods() {
    assert_eq!(
        run(r#"
            class C {
              #field = "field";
              #method() { return this.#field; }
              static readField(value) { return value?.#field; }
              static callMethod(value) { return value?.#method(); }
              readNested(value) { return value?.receiver.#field; }
            }
            var instance = new C();
            [
              C.readField(instance),
              C.readField(null),
              C.callMethod(instance),
              C.callMethod(undefined),
              instance.readNested({ receiver: instance }),
              instance.readNested(null)
            ].join("|");
        "#),
        Value::String(Arc::from("field||field||field|"))
    );

    assert_eq!(
        run(r#"
            class C {
              #field;
              static read(value) { return value?.receiver.#field; }
            }
            try { C.read({ receiver: {} }); } catch (error) { error.name; }
        "#),
        Value::String(Arc::from("TypeError"))
    );
}

#[test]
fn optional_super_method_call_preserves_receiver() {
    assert_eq!(
        run(r#"
            var receiver;
            class Base {
              method() { receiver = this; }
            }
            class Derived extends Base {
              method() { super.method?.(); }
            }
            var instance = new Derived();
            instance.method();
            receiver === instance;
        "#),
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

#[test]
fn decorators_evaluate_in_source_order_and_apply_in_reverse_order() {
    assert_eq!(
        run(r#"
            let log = [];
            function decorator(name) {
              log.push("evaluate " + name);
              return function(value, context) {
                "use strict";
                log.push("apply " + name + " " + context.kind + " " + context.name + " " + (this === undefined));
              };
            }
            @decorator("class-a")
            @decorator("class-b")
            class C {
              @decorator("method-a")
              @decorator("method-b")
              method() {}
              @decorator("field-a")
              @decorator("field-b")
              field;
              static initialized = log.push("static field");
            }
            log.join("|");
        "#),
        Value::String(Arc::from(
            "evaluate class-a|evaluate class-b|evaluate method-a|evaluate method-b|evaluate field-a|evaluate field-b|apply method-b method method true|apply method-a method method true|apply field-b field field true|apply field-a field field true|apply class-b class C true|apply class-a class C true|static field"
        ))
    );
}

#[test]
fn decorators_replace_methods_classes_and_field_initializers() {
    assert_eq!(
        run(r#"
            function replaceMethod(value, context) {
              return function() { return context.name + ":replacement"; };
            }
            function initializeField(offset) {
              return function(value, context) {
                return function(initial) { return initial + offset + context.name.length; };
              };
            }
            function replaceClass(value, context) {
              return class extends value { static decorated = context.name; };
            }
            @replaceClass
            class C {
              @replaceMethod method() { return "original"; }
              @initializeField(2) field = 3;
              @initializeField(4) static value = 5;
            }
            let instance = new C();
            [instance.method(), instance.field, C.value, C.decorated, instance instanceof C].join("|");
        "#),
        Value::String(Arc::from("method:replacement|10|14|C|true"))
    );
    assert_eq!(
        run(r#"
            let instanceThis;
            let staticThis;
            function capture(value, context) {
              return function(initial) {
                if (context.static) staticThis = this;
                else instanceThis = this;
                return initial;
              };
            }
            Function.prototype.call = function() { throw new Error("must not be observed"); };
            class C {
              @capture field = 1;
              @capture static field = 2;
            }
            let instance = new C();
            instanceThis === instance && staticThis === C;
        "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            let log = [];
            function append(name) {
              return function() {
                log.push("apply " + name);
                return function(value) {
                  log.push("init " + name);
                  return value + name;
                };
              };
            }
            class C { @append("A") @append("B") field = ""; }
            let instance = new C();
            log.join(",") + "|" + instance.field;
        "#),
        Value::String(Arc::from("apply B,apply A,init A,init B|AB"))
    );
}

#[test]
fn decorator_replacements_are_type_checked() {
    assert!(run_err("@(() => 1) class C {}").contains("Class decorator"));
    assert!(run_err("class C { @(() => 1) method() {} }").contains("Element decorator"));
    assert!(run_err("class C { @(() => 1) field; }").contains("Field decorator"));
    assert_eq!(
        run("@(function() { return function() { return 7; }; }) class C {} C();"),
        Value::Number(7.0)
    );
    assert!(run_err("@(() => (() => 7)) class C {}").contains("Class decorator"));
}

#[test]
fn throwing_decorators_do_not_leak_caught_operand_roots() {
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
    vm.gc();
    vm.set_max_heap_objects(Some(5_000));
    assert_eq!(
        vm.run(
            r#"
            function fail() { throw 1; }
            let caught = 0;
            for (let i = 0; i < 10000; i++) {
              try { @fail class C {} }
              catch (error) {
                caught++;
                if (i % 50 === 0) forceGc();
              }
            }
            caught;
        "#
        )
        .expect("caught decorator failures must not remain GC roots"),
        Value::Number(10000.0)
    );
}

#[test]
fn decorator_member_expressions_preserve_factory_receivers() {
    assert_eq!(
        run(r#"
            let receiver;
            let namespace = {
              factory() {
                receiver = this;
                return function() {};
              }
            };
            @namespace.factory() class C {}
            receiver === namespace;
        "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            let receiver;
            let holder = {
              decorator(value, context) { receiver = this; }
            };
            @holder.decorator class C {}
            receiver === holder;
        "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            let symbol = Symbol("decorated");
            let methodName;
            let fieldName;
            function methodDecorator(value, context) { methodName = context.name; }
            function fieldDecorator(value, context) { fieldName = context.name; }
            class C {
              @methodDecorator [symbol]() {}
              @fieldDecorator [symbol] = 1;
            }
            methodName === symbol && fieldName === symbol;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn auto_accessors_use_hidden_backing_storage() {
    assert_eq!(
        run(r#"
            let keyCalls = 0;
            let key = { toString() { keyCalls++; return "computed"; } };
            class C {
              accessor value = 1;
              accessor [key] = 2;
              accessor #private = 3;
              static accessor shared = 4;
              static accessor #secret = 5;
              readPrivate() { return this.#private; }
              writePrivate(value) { this.#private = value; }
              static readSecret() { return this.#secret; }
            }
            let first = new C();
            let second = new C();
            first.value = 6;
            first.computed = 7;
            first.writePrivate(8);
            C.shared = 9;
            let descriptor = Object.getOwnPropertyDescriptor(C.prototype, "value");
            [
              first.value, second.value, first.computed, second.computed,
              first.readPrivate(), second.readPrivate(), C.shared, C.readSecret(),
              keyCalls, descriptor.enumerable, typeof descriptor.get, typeof descriptor.set,
              Object.prototype.hasOwnProperty.call(first, "value")
            ].join("|");
        "#),
        Value::String(Arc::from("6|1|7|2|8|3|9|5|1|false|function|function|false"))
    );
    assert_eq!(
        run(r#"
            class C {
              accessor value = function() {};
              accessor #private = class {};
              readPrivate() { return this.#private.name; }
            }
            let instance = new C();
            [instance.value.name, instance.readPrivate()].join("|");
        "#),
        Value::String(Arc::from("value|#private"))
    );
    assert!(run_err("class C { accessor constructor = 1; }").contains("constructor"));
    assert!(run_err("class C { static accessor constructor = 1; }").contains("constructor"));
    assert_eq!(
        run("class C { accessor ['constructor'] = 1; } new C().constructor;"),
        Value::Number(1.0)
    );
}

#[test]
fn decorated_public_auto_accessors_compose_replacements_and_context() {
    assert_eq!(
        run(r#"
            let captured;
            let extraValue;
            function decorate(value, context) {
              captured = [Object.keys(value).join(","), context];
              context.addInitializer(function() { extraValue = this.value; });
              return {
                get() { return value.get.call(this) * 2; },
                set(next) { value.set.call(this, next + 1); },
                init(initial) { return initial + 3; }
              };
            }
            class C { @decorate accessor value = 4; }
            let instance = new C();
            let initial = instance.value;
            instance.value = 5;
            [
              captured[0], Object.keys(captured[1]).join(","),
              Object.keys(captured[1].access).join(","), captured[1].kind,
              captured[1].name, captured[1].static, captured[1].private,
              captured[1].access.get(instance), initial, extraValue, instance.value
            ].join("|");
        "#),
        Value::String(Arc::from(
            "get,set|kind,access,static,private,name,addInitializer|get,set,has|accessor|value|false|false|12|14|14|12"
        ))
    );
    assert_eq!(
        run(r#"
            let order = [];
            function outer(value, context) {
              order.push("apply outer");
              return { init(value) { order.push("init outer"); return value * 2; } };
            }
            function inner(value, context) {
              order.push("apply inner");
              return { init(value) { order.push("init inner"); return value + 3; } };
            }
            class C { @outer @inner accessor value = 1; }
            let instance = new C();
            [order.join(","), instance.value].join("|");
        "#),
        Value::String(Arc::from("apply inner,apply outer,init outer,init inner|5"))
    );
}

#[test]
fn decorated_public_auto_accessors_support_static_computed_names() {
    assert_eq!(
        run(r#"
            let key = Symbol("value");
            let context;
            function decorate(value, nextContext) {
              context = nextContext;
              return { get() { return value.get.call(this) + 1; } };
            }
            class C { @decorate static accessor [key] = 8; }
            [
              context.name === key, context.static, context.private,
              context.access.has(C), context.access.get(C), C[key]
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|false|true|9|9"))
    );
}

#[test]
fn decorated_auto_accessors_use_method_application_phases() {
    assert_eq!(
        run(r#"
            let order = [];
            function mark(value) { return () => { order.push(value); }; }
            @mark(9)
            class C {
              @mark(8) field = 1;
              @mark(4) method() {}
              @mark(1) static accessor staticAccessor = 2;
              @mark(2) static get staticGetter() {}
              @mark(5) accessor accessor = 3;
              @mark(6) get getter() {}
              @mark(7) static staticField = 4;
              @mark(3) static staticMethod() {}
            }
            order.join(",");
        "#),
        Value::String(Arc::from("1,2,3,4,5,6,7,8,9"))
    );
}

#[test]
fn class_decorators_observe_static_private_methods() {
    assert_eq!(
        run(r#"
            let observed;
            function inspect(value) { observed = value.read(); }
            @inspect
            class C {
              static #method() { return 7; }
              static get #accessor() { return 8; }
              static read() { return this.#method() + this.#accessor; }
            }
            observed;
        "#),
        Value::Number(15.0)
    );
}

#[test]
fn decorated_public_auto_accessors_validate_return_records() {
    assert!(run_err("class C { @(() => 1) accessor value; }").contains("object or undefined"));
    for property in ["get", "set", "init"] {
        let source = format!("class C {{ @(() => ({{ {property}: 1 }})) accessor value; }}");
        let error = run_err(&source);
        assert!(error.contains("must be callable"), "{property}: {error}");
    }
    assert_eq!(
        run(r#"
            let order = [];
            function decorate() {
              return {
                get get() { order.push("get"); return undefined; },
                get set() { order.push("set"); return undefined; },
                get init() { order.push("init"); return undefined; }
              };
            }
            class C { @decorate accessor value; }
            order.join(",");
        "#),
        Value::String(Arc::from("get,set,init"))
    );
}

#[test]
fn accessor_keyword_requires_no_line_terminator() {
    assert_eq!(
        run(r#"
            class C {
              accessor
              value;
              static accessor
              other;
            }
            let instance = new C();
            [
              Object.prototype.hasOwnProperty.call(instance, "accessor"),
              Object.prototype.hasOwnProperty.call(instance, "value"),
              Object.prototype.hasOwnProperty.call(C, "accessor"),
              Object.prototype.hasOwnProperty.call(C, "other")
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|false"))
    );
    assert_eq!(
        run(r#"
            class C {
              accessor() { return "method"; }
              accessor = "field";
            }
            let instance = new C();
            [instance.accessor, Object.prototype.hasOwnProperty.call(instance, "accessor")].join("|");
        "#),
        Value::String(Arc::from("field|true"))
    );
}

#[test]
fn decorator_grammar_rejects_unrestricted_and_invalid_targets() {
    assert_eq!(
        run("let C = @(value => value) class {}; typeof C;"),
        Value::String(Arc::from("function"))
    );
    assert!(run_err("@foo + bar class C {}").contains("followed by a class"));
    assert!(run_err("class C { @foo; }").contains("must precede a class element"));
    assert!(run_err("class C { @foo static {} }").contains("cannot be decorated"));
    assert!(run_err("class C { @foo constructor() {} }").contains("cannot be decorated"));
    assert!(run_err("class C { @foo #method() {} }").contains("not implemented"));
    assert!(run_err("class C { @foo accessor #value; }").contains("not implemented"));
}

#[test]
fn decorator_context_exposes_public_access_and_exact_shape() {
    assert_eq!(
        run(r#"
            let contexts = [];
            function capture(value, context) { contexts.push(context); }
            let symbol = Symbol("computed");
            class C {
              @capture method() { return 1; }
              @capture get value() { return this._value; }
              @capture set value(next) { this._value = next; }
              @capture field = 2;
              @capture [symbol]() { return 3; }
              @capture static staticMethod() { return 4; }
            }
            let instance = new C();
            let staticContext = contexts[0];
            let methodContext = contexts[1];
            let getterContext = contexts[2];
            let setterContext = contexts[3];
            let symbolContext = contexts[4];
            let fieldContext = contexts[5];
            methodContext.access.get(instance) === C.prototype.method;
            getterContext.access.get(instance) === undefined;
            setterContext.access.set(instance, 7);
            fieldContext.access.set(instance, 8);
            let fieldValue = fieldContext.access.get(instance);
            let symbolValue = contexts[4].access.get(instance);
            let staticValue = staticContext.access.get(C);
            let primitiveErrors = 0;
            for (let operation of [
              () => methodContext.access.has(1),
              () => methodContext.access.get(1),
              () => fieldContext.access.set(1, 2),
              () => new methodContext.access.get(instance)
            ]) {
              try { operation(); } catch (error) { if (error instanceof TypeError) primitiveErrors++; }
            }
            [
              Object.keys(methodContext).join(","), Object.keys(methodContext.access).join(","),
              Object.keys(getterContext.access).join(","), Object.keys(setterContext.access).join(","),
              Object.keys(fieldContext.access).join(","), symbolContext.name === symbol,
              methodContext.access.get.name, setterContext.access.set.name,
              methodContext.addInitializer.name, methodContext.access.has(instance),
              methodContext.access.get.length,
              setterContext.access.set.length, methodContext.addInitializer.length,
              instance._value, fieldValue, symbolValue(), staticValue(), primitiveErrors
            ].join("|");
        "#),
        Value::String(Arc::from(
            "kind,access,static,private,name,addInitializer|get,has|get,has|set,has|get,set,has|true|||addInitializer|true|1|2|1|7|8|3|4|4"
        ))
    );
    assert_eq!(
        run(r#"
            let named;
            let anonymous;
            function captureNamed(value, context) { named = context; }
            function captureAnonymous(value, context) { anonymous = context; }
            @captureNamed class C {}
            (@captureAnonymous class {});
            [
              Object.keys(named).join(","), named.kind, named.name,
              "access" in named, "static" in named, "private" in named,
              anonymous.name === undefined
            ].join("|");
        "#),
        Value::String(Arc::from(
            "kind,name,addInitializer|class|C|false|false|false|true"
        ))
    );
    assert_eq!(
        run(r#"
            let access;
            function replace(value, context) {
              access = context.access;
              return function replacement() { return 9; };
            }
            class C { @replace method() { return 1; } }
            let instance = new C();
            access.get(instance) === C.prototype.method && access.get(instance)() === 9;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn decorator_context_preserves_private_field_identity_and_brand() {
    assert_eq!(
        run(r#"
            let contexts = [];
            let extras = [];
            function decorate(value, context) {
              contexts.push(context);
              context.addInitializer(function() {
                extras.push(context.static ? this.readStatic() : this.readInstance());
              });
              return function(initial) { return initial + 1; };
            }
            class C {
              @decorate #instanceValue = 1;
              @decorate static #staticValue = 2;
              value = 99;
              static value = 100;
              readInstance() { return this.#instanceValue; }
              static readStatic() { return this.#staticValue; }
            }
            let instance = new C();
            let staticContext = contexts[0];
            let instanceContext = contexts[1];
            let wrongBrand = {};
            let primitiveErrors = 0;
            for (let operation of [
              () => instanceContext.access.has(1),
              () => instanceContext.access.get(1),
              () => instanceContext.access.set(1, 2)
            ]) {
              try { operation(); } catch (error) {
                if (error instanceof TypeError) primitiveErrors++;
              }
            }
            instanceContext.access.set(instance, 7);
            staticContext.access.set(C, 8);
            [
              Object.keys(instanceContext).join(","),
              Object.keys(instanceContext.access).join(","),
              instanceContext.kind, instanceContext.name,
              instanceContext.private, instanceContext.static,
              staticContext.name, staticContext.private, staticContext.static,
              instanceContext.access.has(instance),
              instanceContext.access.has(wrongBrand),
              staticContext.access.has(C), staticContext.access.has(class D {}),
              instanceContext.access.get(instance), staticContext.access.get(C),
              instance.value, C.value, extras.join(","), primitiveErrors
            ].join("|");
        "#),
        Value::String(Arc::from(
            "kind,access,static,private,name,addInitializer|get,set,has|field|#instanceValue|true|false|#staticValue|true|true|true|false|true|false|7|8|99|100|3,2|3"
        ))
    );
}

#[test]
fn private_field_decorator_access_rejects_wrong_brands() {
    assert!(run_err(
        r#"
        let access;
        function capture(value, context) { access = context.access; }
        class C { @capture #value = 1; }
        access.get({});
    "#
    )
    .contains("Private field is not present"));
    assert!(run_err(
        r#"
        let access;
        function capture(value, context) { access = context.access; }
        class C { @capture #value = 1; }
        access.set({}, 2);
    "#
    )
    .contains("Private field is not present"));
}

#[test]
fn decorator_add_initializer_runs_at_specified_boundaries() {
    assert_eq!(
        run(r#"
            let log = [];
            function extra(label) {
              return function(value, context) {
                log.push("apply " + label);
                context.addInitializer(function() {
                  log.push("extra " + label + ":" + context.kind + ":" + this.marker + ":" + this.field);
                });
              };
            }
            @extra("class-a") @extra("class-b")
            class C {
              @extra("instance-method-a") @extra("instance-method-b") method() {}
              marker = (log.push("instance marker"), "instance");
              @extra("instance-field-a") @extra("instance-field-b") field = (log.push("instance field"), "field");
              @extra("static-method-a") @extra("static-method-b") static method() {}
              static marker = (log.push("static marker"), "static");
              @extra("static-field-a") @extra("static-field-b") static field = (log.push("static field"), "field");
            }
            log.push("before instance");
            new C();
            log.join("|");
        "#),
        Value::String(Arc::from(
            "apply static-method-b|apply static-method-a|apply instance-method-b|apply instance-method-a|apply static-field-b|apply static-field-a|apply instance-field-b|apply instance-field-a|apply class-b|apply class-a|extra static-method-b:method:undefined:undefined|extra static-method-a:method:undefined:undefined|static marker|static field|extra static-field-b:field:static:field|extra static-field-a:field:static:field|extra class-b:class:static:field|extra class-a:class:static:field|before instance|extra instance-method-b:method:undefined:undefined|extra instance-method-a:method:undefined:undefined|instance marker|instance field|extra instance-field-b:field:instance:field|extra instance-field-a:field:instance:field"
        ))
    );
}

#[test]
fn decorator_add_initializer_validates_callable_and_lifetime() {
    assert!(
        run_err("class C { @((value, context) => context.addInitializer(1)) method() {} }")
            .contains("callable initializer")
    );
    assert!(run_err(
        "class C { @((value, context) => new context.addInitializer(function() {})) method() {} }"
    )
    .contains("not a constructor"));
    assert_eq!(
        run(r#"
            let lateFromSuccess;
            let lateFromThrow;
            function success(value, context) { lateFromSuccess = context.addInitializer; }
            function fail(value, context) {
              lateFromThrow = context.addInitializer;
              throw 1;
            }
            class C { @success method() {} }
            let successRejected = false;
            let throwRejected = false;
            try { lateFromSuccess(function() {}); } catch (error) { successRejected = error instanceof TypeError; }
            try { @fail class D {} } catch (error) {}
            try { lateFromThrow(function() {}); } catch (error) { throwRejected = error instanceof TypeError; }
            successRejected && throwRejected;
        "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            let previous;
            let rejected = false;
            function first(value, context) { previous = context.addInitializer; }
            function second(value, context) {
              try { previous(function() {}); } catch (error) { rejected = error instanceof TypeError; }
              context.addInitializer(function() {});
            }
            class C { @second @first method() {} }
            rejected;
        "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            let receiver;
            function replace(value, context) {
              context.addInitializer(function() { receiver = this; });
              return class Replacement extends value {};
            }
            @replace class C {}
            receiver === C && C.name === "Replacement";
        "#),
        Value::Bool(true)
    );
}

#[test]
fn decorator_initializers_ignore_mutated_function_call_and_propagate_errors() {
    assert_eq!(
        run(r#"
            let receivers = [];
            function record(value, context) {
              context.addInitializer(function() { receivers.push(this); });
            }
            Function.prototype.call = function() { throw new Error("must not run"); };
            @record class C {
              @record method() {}
              @record field = 1;
              @record static method() {}
              @record static field = 2;
            }
            let instance = new C();
            receivers.length === 5 && receivers[0] === C && receivers[1] === C &&
              receivers[2] === C && receivers[3] === instance && receivers[4] === instance;
        "#),
        Value::Bool(true)
    );
    assert!(run_err(
        r#"
        function fail(value, context) {
          context.addInitializer(function() { throw new Error("static initializer failed"); });
        }
        class C { @fail static method() {} }
    "#
    )
    .contains("static initializer failed"));
    assert!(run_err(
        r#"
        function fail(value, context) {
          context.addInitializer(function() { throw new Error("instance initializer failed"); });
        }
        class C { @fail method() {} }
        new C();
    "#
    )
    .contains("instance initializer failed"));
}

#[test]
fn decorator_instance_initializer_queues_survive_gc() {
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
            let calls = 0;
            function count(value, context) {
              context.addInitializer(function() { calls++; });
            }
            class C {
              @count method() {}
              @count field = 1;
            }
            forceGc();
            new C();
            calls;
        "#
        )
        .expect("decorator initializer queues should remain reachable"),
        Value::Number(2.0)
    );
    assert_eq!(
        vm.run(
            r#"
            function preserve(value) {
              return {
                get get() {
                  forceGc();
                  return function() { return value.get.call(this) + 1; };
                },
                get set() {
                  forceGc();
                  return value.set;
                },
                get init() {
                  forceGc();
                  return function(initial) { return initial + 1; };
                }
              };
            }
            class D { @preserve accessor value = 1; }
            new D().value;
        "#,
        )
        .expect("accessor decorator result should remain rooted during property access"),
        Value::Number(3.0)
    );
}

#[test]
fn derived_constructor_initializes_elements_immediately_after_nested_super() {
    assert_eq!(
        run(r#"
            function extra(value, context) {
              context.addInitializer(function() { this.log.push("extra"); });
            }
            class Base {
              constructor() { this.log = []; }
            }
            class Derived extends Base {
              @extra field = (this.log.push("field"), 1);
              constructor(flag) {
                if (flag) {
                  super();
                  this.log.push("body");
                } else {
                  super();
                }
              }
            }
            [new Derived(true).log.join(","), new Derived(false).log.join(",")].join("|");
        "#),
        Value::String(Arc::from("field,extra,body|field,extra"))
    );
}
