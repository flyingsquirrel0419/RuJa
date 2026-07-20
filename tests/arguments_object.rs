mod common;

use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

#[test]
fn arguments_object_uses_object_prototype_and_stays_iterable() {
    assert_eq!(
        run(r#"
            function f() {
              var values = [];
              for (var v of arguments) values.push(v);
              return [
                Object.getPrototypeOf(arguments) === Object.prototype,
                Array.isArray(arguments),
                values.join(",")
              ].join(":");
            }
            f(1, 2, 3);
            "#),
        Value::String(Arc::from("true:false:1,2,3"))
    );
}

#[test]
fn arguments_iterator_uses_the_immutable_realm_array_values_intrinsic() {
    assert_eq!(
        run(r#"
            var original = Array.prototype.values;
            Array.prototype.values = function replacement() { throw new Error("wrong"); };
            function mapped(a) {
              var descriptor = Object.getOwnPropertyDescriptor(arguments, Symbol.iterator);
              return [
                descriptor.value === original,
                descriptor.writable,
                descriptor.enumerable,
                descriptor.configurable,
                Array.from(arguments).join(":")
              ].join(",");
            }
            function unmapped(a = 1) {
              return arguments[Symbol.iterator] === original;
            }

            var other = $262.createRealm().global;
            var foreign = other.eval("(function () { return arguments; })")();
            [
              mapped(2, 3),
              unmapped(4),
              foreign[Symbol.iterator] === other.Array.prototype.values,
              foreign[Symbol.iterator] !== original
            ].join("|");
        "#),
        Value::String(Arc::from("true,true,false,true,2:3|true|true|true"))
    );
}

#[test]
fn arguments_iteration_observes_deleted_overridden_and_noncallable_methods() {
    assert_eq!(
        run(r#"
            function deleted() {
              delete arguments[Symbol.iterator];
              try {
                for (var value of arguments) {}
              } catch (error) {
                return error instanceof TypeError;
              }
              return false;
            }
            function overridden() {
              arguments[Symbol.iterator] = function() {
                var done = false;
                return {
                  next: function() {
                    if (done) return { value: undefined, done: true };
                    done = true;
                    return { value: 9, done: false };
                  }
                };
              };
              var values = [];
              for (var value of arguments) values.push(value);
              return values.join(":");
            }
            function noncallable() {
              arguments[Symbol.iterator] = 1;
              try {
                for (var value of arguments) {}
              } catch (error) {
                return error instanceof TypeError;
              }
              return false;
            }
            [deleted(1), overridden(1, 2), noncallable(1)].join("|");
        "#),
        Value::String(Arc::from("true|9|true"))
    );
}

#[test]
fn arguments_length_is_configurable_data_property() {
    assert_eq!(
        run(r#"
            function f() {
              var d = Object.getOwnPropertyDescriptor(arguments, "length");
              arguments.length = "custom";
              var assigned = arguments.length;
              var deleted = delete arguments.length;
              return [d.value, d.writable, d.enumerable, d.configurable, assigned, deleted, "length" in arguments].join(",");
            }
            f(1, 2);
            "#),
        Value::String(Arc::from("2,true,false,true,custom,true,false"))
    );
}

#[test]
fn arguments_length_computed_delete_removes_own_property() {
    assert_eq!(
        run(r#"
            function f() {
              var name = "length";
              var deleted = delete arguments[name];
              return [
                deleted,
                Object.prototype.hasOwnProperty.call(arguments, "length"),
                "length" in arguments,
                Object.getOwnPropertyDescriptor(arguments, "length"),
                arguments.length
              ].join(",");
            }
            f(1, 2);
            "#),
        Value::String(Arc::from("true,false,false,,"))
    );
}

#[test]
fn mapped_arguments_nonconfigurable_descriptor_keeps_parameter_map() {
    assert_eq!(
        run(r#"
            function f(a) {
              Object.defineProperty(arguments, "0", { configurable: false });
              a = 2;
              var d = Object.getOwnPropertyDescriptor(arguments, "0");
              return [arguments[0], d.value, d.writable, d.enumerable, d.configurable].join(",");
            }
            f(1);
            "#),
        Value::String(Arc::from("2,2,true,true,false"))
    );
}

#[test]
fn mapped_arguments_index_write_updates_parameter_binding() {
    assert_eq!(
        run(r#"
            function f(a, b, c) {
              arguments[0] = 1;
              arguments[1] = "str";
              arguments[2] = 2.1;
              return [a, b, c, arguments[0], arguments[1], arguments[2]].join(",");
            }
            f(10, "sss", 1);
            "#),
        Value::String(Arc::from("1,str,2.1,1,str,2.1"))
    );
}

#[test]
fn mapped_arguments_index_write_honors_redefined_data_descriptor() {
    assert_eq!(
        run(r#"
            function f(a) {
              Object.defineProperty(arguments, "0", { configurable: false });
              arguments[0] = 2;
              var d = Object.getOwnPropertyDescriptor(arguments, "0");
              return [a, arguments[0], d.value, d.writable, d.enumerable, d.configurable].join(",");
            }
            f(1);
            "#),
        Value::String(Arc::from("2,2,2,true,true,false"))
    );
}

#[test]
fn mapped_arguments_index_write_ignores_prototype_setter() {
    assert_eq!(
        run(r#"
            var data = "data";
            Object.defineProperty(Object.prototype, "0", {
              get: function() { return data; },
              set: function(value) { data = value; },
              configurable: true
            });
            var argObj = (function(a) {
              arguments[0] = 2;
              return [a, arguments[0], data].join(",");
            })(1);
            delete Object.prototype["0"];
            argObj;
            "#),
        Value::String(Arc::from("2,2,data"))
    );
}

#[test]
fn mapped_arguments_writable_false_removes_parameter_map() {
    assert_eq!(
        run(r#"
            function f(a) {
              Object.defineProperty(arguments, "0", { value: 2, writable: false });
              a = 3;
              var d = Object.getOwnPropertyDescriptor(arguments, "0");
              return [arguments[0], a, d.value, d.writable, d.enumerable, d.configurable].join(",");
            }
            f(1);
            "#),
        Value::String(Arc::from("2,3,2,false,true,true"))
    );
}

#[test]
fn strict_delete_nonconfigurable_mapped_argument_throws() {
    let err = run_err(
        r#"
        function f(a) {
          Object.defineProperty(arguments, "0", { configurable: false });
          var args = arguments;
          (function() { "use strict"; delete args[0]; })();
        }
        f(1);
        "#,
    );
    assert!(err.contains("TypeError"), "{err}");
}

#[test]
fn mapped_arguments_accessor_descriptor_removes_parameter_map() {
    assert_eq!(
        run(r#"
            function f(a) {
              var setCalls = 0;
              Object.defineProperty(arguments, "0", {
                set: function(_v) { setCalls += 1; },
                enumerable: true,
                configurable: true
              });
              arguments[0] = "foo";
              var afterSetter = [setCalls, a, arguments[0] === undefined].join(",");

              Object.defineProperty(arguments, "1", {
                get: function() { return "bar"; },
                enumerable: true,
                configurable: true
              });
              return afterSetter + ";" + arguments[1];
            }
            f(0);
            "#),
        Value::String(Arc::from("1,0,true;bar"))
    );
}

#[test]
fn sloppy_arguments_callee_caller_is_available_or_undefined() {
    assert_eq!(
        run(r#"
            var called = false;
            function test1(flag) {
              if (flag !== true) {
                test2();
              } else {
                called = true;
              }
            }
            function test2() {
              if (arguments.callee.caller === undefined) {
                called = true;
              } else {
                arguments.callee.caller(true);
              }
            }
            test1();
            called;
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var called = false;
            function test1(flag) {
              if (flag !== true) {
                test2();
              } else {
                called = true;
              }
            }
            function test2() {
              if (arguments.callee.caller === undefined) {
                called = true;
              } else {
                var explicit = arguments.callee.caller;
                explicit(true);
              }
            }
            test1();
            called;
            "#),
        Value::Bool(true)
    );
}

#[test]
fn sloppy_function_caller_rejects_strict_callers() {
    let err = run_err(
        r#"
        function gNonStrict() {
          return gNonStrict.caller;
        }
        (function() { "use strict"; gNonStrict(); })();
        "#,
    );
    assert!(err.contains("TypeError"), "{err}");
}

#[test]
fn member_call_spread_preserves_this_and_arguments() {
    assert_eq!(
        run(r#"
            var arr = [2, 3];
            var obj = {
              tag: "obj",
              method: function() {
                return [this.tag, arguments.length, arguments[0], arguments[1], arguments[2], arguments[3]].join(",");
              }
            };
            obj.method(42, ...[1], ...arr,);
            "#),
        Value::String(Arc::from("obj,4,42,1,2,3"))
    );

    assert_eq!(
        run(r#"
            var arr = [2, 3];
            class C {
              method() {
                return [this.tag, arguments.length, arguments[0], arguments[1], arguments[2], arguments[3]].join(",");
              }
              static method() {
                return [this.tag, arguments.length, arguments[0], arguments[1], arguments[2], arguments[3]].join(",");
              }
            }
            var c = new C();
            c.tag = "inst";
            C.tag = "ctor";
            c.method(42, ...[1], ...arr,) + ";" + C.method(42, ...[1], ...arr,);
            "#),
        Value::String(Arc::from("inst,4,42,1,2,3;ctor,4,42,1,2,3"))
    );
}
