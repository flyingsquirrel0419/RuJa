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
