use super::Vm;
use crate::value::{HeapObj, NativeConstructMode, PromiseStatus};
use crate::Value;
use std::fs;

fn cap_heap_at_current_live_count(vm: &mut Vm) -> crate::error::Result<Value> {
    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    Ok(Value::Undefined)
}

fn promise_state_and_result(vm: &Vm, value: Value) -> (PromiseStatus, Value) {
    let Value::Object(promise) = value else {
        panic!("expected a Promise object");
    };
    vm.heap.with_obj(promise.0, |object| {
        let HeapObj::Promise(data) = object else {
            panic!("expected a Promise heap object");
        };
        (*data.state.lock(), data.result.lock().clone())
    })
}

fn native_construct_mode(vm: &Vm, value: &Value) -> Option<NativeConstructMode> {
    let Value::Object(function) = value else {
        panic!("expected a native function object");
    };
    vm.heap.with_obj(function.0, |object| {
        let HeapObj::Function(data) = object else {
            panic!("expected a native function");
        };
        let crate::value::FunctionKind::Native { construct_mode, .. } = &data.kind else {
            panic!("expected a native function kind");
        };
        *construct_mode
    })
}

const EAGER_NATIVE_CONSTRUCTOR_SOURCES: &[&str] = &[
    "Array",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    "AggregateError",
];

const DEFERRED_NATIVE_CONSTRUCTOR_SOURCES: &[&str] = &[
    "Object",
    "Function",
    "(async function () {}).constructor",
    "(function* () {}).constructor",
    "(async function* () {}).constructor",
    "String",
    "Number",
    "Boolean",
    "Date",
    "RegExp",
    "Proxy",
    "BigInt",
    "Symbol",
    "Object.getPrototypeOf(Int8Array)",
    "Iterator",
    "Promise",
    "ArrayBuffer",
    "DataView",
    "WeakRef",
    "FinalizationRegistry",
    "SharedArrayBuffer",
    "Int8Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "Int16Array",
    "Uint16Array",
    "Int32Array",
    "Uint32Array",
    "Float32Array",
    "Float64Array",
    "BigInt64Array",
    "BigUint64Array",
];

const NON_CONSTRUCTIBLE_NATIVE_FUNCTION_SOURCES: &[&str] = &[
    "Function.prototype",
    "Math.abs",
    "Array.prototype.push",
    "BigInt.asIntN",
    "Symbol.for",
];

const FOREIGN_EAGER_NATIVE_CONSTRUCTOR_SOURCES: &[&str] = &[
    "Array",
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    "AggregateError",
];

fn realm_registry_counts(vm: &Vm) -> [usize; 32] {
    [
        vm.realm_globals.len(),
        vm.realm_object_prototypes.len(),
        vm.realm_object_prototype_ids.len(),
        vm.realm_array_prototypes.len(),
        vm.realm_promise_constructors.len(),
        vm.realm_promise_prototypes.len(),
        vm.realm_generator_prototypes.len(),
        vm.realm_generator_function_constructors.len(),
        vm.realm_generator_function_prototypes.len(),
        vm.realm_async_iterator_prototypes.len(),
        vm.realm_async_generator_prototypes.len(),
        vm.realm_async_generator_function_constructors.len(),
        vm.realm_async_generator_function_prototypes.len(),
        vm.realm_primitive_prototypes.len(),
        vm.realm_date_prototypes.len(),
        vm.realm_eval_functions.len(),
        vm.realm_throw_type_errors.len(),
        vm.realm_function_prototypes.len(),
        vm.realm_async_function_prototypes.len(),
        vm.realm_iterator_constructors.len(),
        vm.realm_iterator_prototypes.len(),
        vm.realm_array_iterator_prototypes.len(),
        vm.realm_wrap_for_valid_iterator_prototypes.len(),
        vm.realm_string_iterator_prototypes.len(),
        vm.realm_iterator_helper_prototypes.len(),
        vm.realm_error_prototypes.len(),
        vm.realm_heap_limit_errors.len(),
        vm.realm_regexp_constructors.len(),
        vm.realm_regexp_prototypes.len(),
        vm.realm_regexp_string_iterator_prototypes.len(),
        vm.realm_array_buffer_prototypes.len(),
        vm.realm_typed_array_constructors.len(),
    ]
}

fn realm_creation_live_delta() -> usize {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    let realm = vm
        .run("$262.createRealm();")
        .expect("Realm creation should succeed without a cap");
    let realm_pin = vm.pin(&realm);
    vm.gc();
    let delta = vm.heap.live_count() - baseline_live;
    vm.unpin_many(realm_pin);
    assert!(delta > 1, "Realm creation must allocate a nontrivial graph");
    delta
}

fn assert_main_realm_range_error(vm: &Vm, error: &crate::error::Error) {
    let Value::Object(error_object) = error
        .thrown_value
        .clone()
        .expect("native heap failure should be materialized")
    else {
        panic!("heap failure should throw an Error object");
    };
    let expected_proto = vm.error_prototype_for_env("RangeError", vm.global);
    let actual_proto = vm
        .heap
        .with_obj(error_object.0, |object| object.proto().lock().clone());
    assert_eq!(
        actual_proto,
        Some(expected_proto),
        "Realm construction failure must materialize in the calling Realm"
    );
}

fn assert_failed_realm_attempt(
    vm: &mut Vm,
    baseline_live: usize,
    baseline_registries: [usize; 32],
    baseline_pins: usize,
    extra_capacity: usize,
) {
    vm.set_max_heap_objects(Some(baseline_live + extra_capacity));
    let error = vm
        .run("$262.createRealm();")
        .expect_err("Realm construction should hit the selected heap boundary");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_main_realm_range_error(vm, error.as_ref());
    vm.set_max_heap_objects(None);
    assert_eq!(
        realm_registry_counts(vm),
        baseline_registries,
        "failed Realm must restore every registry at extra capacity {extra_capacity}"
    );
    assert_eq!(
        vm.gc_pins.len(),
        baseline_pins,
        "failed Realm must restore the pin stack at extra capacity {extra_capacity}"
    );
    vm.gc();
    assert_eq!(
        vm.heap.live_count(),
        baseline_live,
        "failed Realm graph must be collectible at extra capacity {extra_capacity}"
    );
}

#[test]
fn function_prototype_survives_collection_before_function_allocation() {
    let dir = std::env::temp_dir().join(format!(
        "ruja-function-prototype-root-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should follow epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("module fixture directory should be created");
    let module = dir.join("entry.js");
    fs::write(
        &module,
        "export function f() {} globalThis.moduleFunction = f;",
    )
    .expect("module fixture should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let _unrooted_garbage = vm.new_object().expect("garbage object should allocate");
    vm.set_max_heap_objects(Some(vm.heap.live_count() + 2));
    vm.link_module_file(&module)
        .expect("module declaration instantiation should succeed");
    vm.set_max_heap_objects(None);
    vm.run_module_file(&module)
        .expect("linked module should evaluate");
    assert_eq!(
        vm.run("moduleFunction.prototype === moduleFunction")
            .expect("module function should be observable"),
        Value::Bool(false)
    );

    fs::remove_dir_all(dir).expect("module fixture directory should be removed");
}

#[test]
fn bound_function_prototype_survives_gc_aware_allocation() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    let result = vm.run(
        r#"
        var target = new Proxy(function () {}, {
          getPrototypeOf: function () {
            var prototype = { marker: 41 };
            var garbage1 = {};
            var garbage2 = {};
            capHeap();
            return prototype;
          }
        });
        var bound = target.bind(null);
        Object.getPrototypeOf(bound).marker;
        "#,
    );
    vm.set_max_heap_objects(None);
    assert_eq!(
        result.expect("trap-produced prototype should survive bound allocation"),
        Value::Number(41.0)
    );
}

#[test]
fn function_creation_failures_restore_gc_pin_depth() {
    for source in [
        "var value = function named() {};",
        "var value = function() {};",
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.gc();
        let pin_depth = vm.gc_pins.len();
        vm.set_max_heap_objects(Some(vm.heap.live_count() + 1));
        let error = vm
            .run(source)
            .expect_err("function creation should hit the heap limit");
        assert_eq!(error.kind, crate::error::ErrorKind::Range);
        assert_eq!(vm.gc_pins.len(), pin_depth);
    }
}

#[test]
fn array_callback_builtins_restore_gc_pin_depth_after_abrupt_completion() {
    for source in [
        r#"
        var error = {};
        [1, 2].map(function(value) {
          if (value === 2) { forceGc(); throw error; }
          return { value: value };
        });
        "#,
        r#"
        var error = {};
        [1, 2].flatMap(function(value) {
          if (value === 2) { forceGc(); throw error; }
          return [{ value: value }];
        });
        "#,
        r#"
        var error = {};
        function C() {
          return new Proxy({}, {
            defineProperty: function(target, key, descriptor) {
              forceGc();
              if (key === "1") throw error;
              return Reflect.defineProperty(target, key, descriptor);
            }
          });
        }
        Array.of.call(C, {}, {});
        "#,
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.register_fn(
            "forceGc",
            |vm, _, _| {
                vm.gc();
                Ok(Value::Undefined)
            },
            0,
        )
        .expect("GC test hook should register");
        let baseline = vm.gc_pins.len();

        vm.run(source)
            .expect_err("the callback should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
    }
}

#[test]
fn array_mapping_allocation_failures_restore_gc_pin_depth() {
    for source in [
        r#"
        [1, 2].map(function(value) {
          if (value === 2) { capHeap(); return value; }
          return { value: value };
        });
        "#,
        r#"
        [1, 2].flatMap(function(value) {
          if (value === 2) { capHeap(); return value; }
          return [{ value: value }];
        });
        "#,
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
            .expect("heap-cap hook should register");
        let baseline = vm.gc_pins.len();

        let error = vm
            .run(source)
            .expect_err("the result Array allocation should hit the heap limit");
        vm.set_max_heap_objects(None);
        assert_eq!(error.kind, crate::error::ErrorKind::Range);
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_sort_methods_restore_gc_pin_depth_after_abrupt_completion() {
    for source in [
        r#"
        var error = {};
        [{ value: 2 }, { value: 1 }].sort(function() {
          forceGc();
          throw error;
        });
        "#,
        r#"
        var error = {};
        [{ value: 2 }, { value: 1 }].toSorted(function() {
          forceGc();
          throw error;
        });
        "#,
        r#"
        var error = {};
        var value = { toString: function() { forceGc(); throw error; } };
        [value, {}].sort();
        "#,
        r#"
        var error = {};
        var value = { toString: function() { forceGc(); throw error; } };
        [value, {}].toSorted();
        "#,
        r#"
        var error = {};
        var source = {
          length: 3,
          0: { value: 2 },
          get 1() { forceGc(); throw error; },
          2: { value: 1 }
        };
        Array.prototype.sort.call(source, function(left, right) {
          return left.value - right.value;
        });
        "#,
        r#"
        var error = {};
        var source = {
          length: 3,
          0: { value: 2 },
          get 1() { forceGc(); throw error; },
          2: { value: 1 }
        };
        Array.prototype.toSorted.call(source, function(left, right) {
          return left.value - right.value;
        });
        "#,
        r#"
        var error = {};
        var first = { value: 2 };
        var source = { length: 2, 1: { value: 1 } };
        Object.defineProperty(source, "0", {
          get: function() { return first; },
          set: function() { forceGc(); throw error; },
          configurable: true
        });
        Array.prototype.sort.call(source, function(left, right) {
          return left.value - right.value;
        });
        "#,
        r#"
        var source = new Proxy({ length: 2, 0: { value: 1 } }, {
          has: function(target, key) { return key !== "1" && key in target; },
          deleteProperty: function() { forceGc(); return false; }
        });
        Array.prototype.sort.call(source, function(left, right) {
          return left.value - right.value;
        });
        "#,
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.register_fn(
            "forceGc",
            |vm, _, _| {
                vm.gc();
                Ok(Value::Undefined)
            },
            0,
        )
        .expect("GC test hook should register");
        let baseline = vm.gc_pins.len();

        vm.run(source)
            .expect_err("sorting should preserve the abrupt completion");
        assert_eq!(vm.gc_pins.len(), baseline);
    }
}

#[test]
fn array_sort_methods_restore_gc_pin_depth_after_fuel_abort() {
    for expression in [
        "Array.prototype.sort.call(source);",
        "Array.prototype.toSorted.call(source);",
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.run("globalThis.source = { length: 1000, 0: { value: 1 } };")
            .expect("fuel fixture should initialize");
        let baseline = vm.gc_pins.len();

        vm.set_fuel(Some(50));
        let error = vm
            .run(expression)
            .expect_err("the native indexed-property scan should exhaust fuel");
        assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
        assert_eq!(vm.gc_pins.len(), baseline, "pin leak after {expression}");

        vm.set_fuel(None);
        assert_eq!(
            vm.run("source[0].value")
                .expect("VM should remain reusable after a fuel abort"),
            Value::Number(1.0)
        );
    }
}

#[test]
fn array_to_sorted_allocation_failure_precedes_comparator_and_restores_gc_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    let baseline = vm.gc_pins.len();

    let error = vm
        .run(
            r#"
            var calls = 0;
            var source = [{ value: 2 }, { value: 1 }];
            var compare = function(left, right) {
              calls++;
              return left.value - right.value;
            };
            capHeap();
            source.toSorted(compare);
            "#,
        )
        .expect_err("ArrayCreate should hit the heap limit before comparison");
    vm.set_max_heap_objects(None);
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.run("calls")
            .expect("comparison count should remain observable"),
        Value::Number(0.0)
    );
    assert_eq!(
        vm.run("1 + 1").expect("VM should remain reusable"),
        Value::Number(2.0)
    );
}

#[test]
fn internally_allocating_array_constructor_uses_one_heap_slot() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let limit = vm.heap.live_count() + 1;
    vm.set_max_heap_objects(Some(limit));

    let result = vm.run("new Array();");
    let live_count = vm.heap.live_count();
    vm.set_max_heap_objects(None);

    let value = result.expect("Array construction should need only the resulting Array slot");
    let Value::Object(array) = value else {
        panic!("Array construction should return an object");
    };
    assert!(vm
        .heap
        .with_obj(array.0, |object| matches!(object, HeapObj::Array(_))));
    assert!(live_count <= limit);
}

#[test]
fn native_constructor_allocation_modes_are_explicit() {
    let mut vm = Vm::new().expect("VM should initialize");
    for &source in EAGER_NATIVE_CONSTRUCTOR_SOURCES {
        let constructor = vm.run(source).expect("eager constructor should resolve");
        assert!(vm.is_constructor_value(&constructor));
        assert_eq!(
            native_construct_mode(&vm, &constructor),
            Some(NativeConstructMode::InternalEagerPrototype),
            "unexpected construct mode for {source}"
        );
    }
    for &source in DEFERRED_NATIVE_CONSTRUCTOR_SOURCES {
        let constructor = vm.run(source).expect("deferred constructor should resolve");
        assert!(vm.is_constructor_value(&constructor));
        assert_eq!(
            native_construct_mode(&vm, &constructor),
            Some(NativeConstructMode::InternalDeferredPrototype),
            "unexpected construct mode for {source}"
        );
    }
    for &source in NON_CONSTRUCTIBLE_NATIVE_FUNCTION_SOURCES {
        let function = vm.run(source).expect("native function should resolve");
        assert!(
            !vm.is_constructor_value(&function),
            "{source} must not construct"
        );
        assert_eq!(
            native_construct_mode(&vm, &function),
            None,
            "unexpected construct metadata for {source}"
        );
    }
}

#[test]
fn constructor_checks_follow_deep_proxy_and_bound_chains_iteratively() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var sharedHandler = {};
        var deepProxyBase = function () {};
        var deepProxyConstructor = deepProxyBase;
        for (var i = 0; i < 20000; i += 1) {
          deepProxyConstructor = new Proxy(deepProxyConstructor, sharedHandler);
        }
        var deepBoundConstructor = function () {};
        for (var j = 0; j < 20000; j += 1) {
          deepBoundConstructor = deepBoundConstructor.bind(null);
        }
        var deepNonConstructor = Math.abs;
        for (var k = 0; k < 1000; k += 1) {
          deepNonConstructor = new Proxy(deepNonConstructor, sharedHandler);
        }
        var ShallowNewTarget = function () {};
        var deepProxyResult = Reflect.construct(
          deepProxyConstructor, [], ShallowNewTarget
        );
        var deepProxyDefaultResult = Reflect.construct(deepProxyConstructor, []);
        var deepBoundResult = Reflect.construct(
          deepBoundConstructor, [], ShallowNewTarget
        );
        var invariantBase = {};
        Object.defineProperty(invariantBase, "fixed", {
          value: 1,
          writable: false,
          configurable: false
        });
        var deepInvariantTarget = invariantBase;
        for (var m = 0; m < 100000; m += 1) {
          deepInvariantTarget = new Proxy(deepInvariantTarget, sharedHandler);
        }
        var invariantProxy = new Proxy(deepInvariantTarget, {
          get: function () { return 1; }
        });
        var deepInvariantRead = invariantProxy.fixed === 1;
        var descriptorProxy = new Proxy(deepInvariantTarget, {
          getOwnPropertyDescriptor: function () { return undefined; }
        });
        var deepDescriptorRead =
          Object.getOwnPropertyDescriptor(descriptorProxy, "missing") === undefined;
        var trapHandler = {
          isExtensible: function () { return true; },
          getOwnPropertyDescriptor: function () { return undefined; }
        };
        var deepTrapTarget = {};
        for (var n = 0; n < 100000; n += 1) {
          deepTrapTarget = new Proxy(deepTrapTarget, trapHandler);
        }
        var deepTrapExtensible = Object.isExtensible(deepTrapTarget);
        var deepTrapDescriptor =
          Object.getOwnPropertyDescriptor(deepTrapTarget, "missing") === undefined;
        var freshDescriptorHandler = {
          getOwnPropertyDescriptor: function () {
            return {
              value: 1,
              writable: true,
              enumerable: true,
              configurable: true
            };
          }
        };
        var freshDescriptorTarget = {};
        for (var q = 0; q < 1000; q += 1) {
          freshDescriptorTarget = new Proxy(
            freshDescriptorTarget, freshDescriptorHandler
          );
        }
        var freshDescriptor =
          Object.getOwnPropertyDescriptor(freshDescriptorTarget, "fresh");
        var deepFreshDescriptor =
          freshDescriptor.value === 1 &&
          freshDescriptor.writable &&
          freshDescriptor.enumerable &&
          freshDescriptor.configurable;
        var deepProxyConstructed =
          Object.getPrototypeOf(deepProxyResult) === ShallowNewTarget.prototype;
        var deepProxyDefaultConstructed =
          Object.getPrototypeOf(deepProxyDefaultResult) === deepProxyBase.prototype;
        var deepBoundConstructed =
          Object.getPrototypeOf(deepBoundResult) === ShallowNewTarget.prototype;
        "#,
    )
    .expect("deep constructor wrappers should allocate");

    assert!(vm.is_constructor_value(&vm.get_global("deepProxyConstructor")));
    assert!(vm.is_constructor_value(&vm.get_global("deepBoundConstructor")));
    assert!(!vm.is_constructor_value(&vm.get_global("deepNonConstructor")));
    assert_eq!(vm.get_global("deepProxyConstructed"), Value::Bool(true));
    assert_eq!(
        vm.get_global("deepProxyDefaultConstructed"),
        Value::Bool(true)
    );
    assert_eq!(vm.get_global("deepBoundConstructed"), Value::Bool(true));
    assert_eq!(vm.get_global("deepInvariantRead"), Value::Bool(true));
    assert_eq!(vm.get_global("deepDescriptorRead"), Value::Bool(true));
    assert_eq!(vm.get_global("deepTrapExtensible"), Value::Bool(true));
    assert_eq!(vm.get_global("deepTrapDescriptor"), Value::Bool(true));
    assert_eq!(vm.get_global("deepFreshDescriptor"), Value::Bool(true));
}

#[test]
fn body_controlled_native_constructors_skip_automatic_prototype_lookup() {
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run(
            r#"
            var prototypeReads = [];
            var active = "";
            var coercions = 0;
            var NewTarget = new Proxy(function () {}, {
              get: function (target, key) {
                if (key === "prototype") prototypeReads.push(active);
                return target[key];
              }
            });
            function throwsTypeError(label, target, args) {
              active = label;
              try { Reflect.construct(target, args, NewTarget); }
              catch (error) { return error instanceof TypeError; }
              return false;
            }

            var bigintError = throwsTypeError("BigInt", BigInt, [{
              valueOf: function () { coercions += 1; return 1; }
            }]);
            var symbolError = throwsTypeError("Symbol", Symbol, [{
              toString: function () { coercions += 1; return "description"; }
            }]);
            var proxyCallError = false;
            try { Proxy(function () {}, {}); }
            catch (error) { proxyCallError = error instanceof TypeError; }
            active = "Proxy";
            var proxyResult = Reflect.construct(Proxy, [function () {}, {}], NewTarget);
            var TypedArray = Object.getPrototypeOf(Int8Array);
            var typedArrayError = throwsTypeError("TypedArray", TypedArray, []);

            [
              bigintError,
              symbolError,
              proxyCallError,
              typeof proxyResult,
              typedArrayError,
              prototypeReads.join(","),
              coercions
            ].join("|");
            "#,
        )
        .expect("body-controlled constructors should complete with expected errors"),
        Value::String("true|true|true|function|true||0".into())
    );
}

#[test]
fn regexp_constructor_follows_classification_allocation_and_initialization_order() {
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run(
            r#"
            var events = [];
            var pattern = {};
            Object.defineProperty(pattern, Symbol.match, {
              get: function () { events.push("match"); return true; }
            });
            Object.defineProperty(pattern, "source", {
              get: function () {
                events.push("source");
                return { toString: function () {
                  events.push("source-string");
                  return "x";
                } };
              }
            });
            Object.defineProperty(pattern, "flags", {
              get: function () {
                events.push("flags");
                return { toString: function () {
                  events.push("flags-string");
                  return "gi";
                } };
              }
            });
            var NewTarget = (function () {}).bind(null);
            Object.defineProperty(NewTarget, "prototype", {
              get: function () {
                events.push("prototype");
                return RegExp.prototype;
              },
              configurable: true
            });
            var result = Reflect.construct(RegExp, [pattern], NewTarget);
            var constructOrder = events.join(",");

            events = [];
            var shortcutPattern = {};
            Object.defineProperty(shortcutPattern, Symbol.match, {
              get: function () { events.push("match"); return true; }
            });
            Object.defineProperty(shortcutPattern, "constructor", {
              get: function () { events.push("constructor"); return RegExp; }
            });
            Object.defineProperty(shortcutPattern, "source", {
              get: function () { events.push("source"); throw "unreachable"; }
            });
            var shortcut = RegExp(shortcutPattern) === shortcutPattern;
            var shortcutOrder = events.join(",");

            events = [];
            var actual = /a/g;
            Object.defineProperty(actual, Symbol.match, {
              get: function () { events.push("actual-match"); return false; }
            });
            Object.defineProperty(actual, "source", {
              get: function () { events.push("actual-source"); throw "unreachable"; }
            });
            Object.defineProperty(actual, "flags", {
              get: function () { events.push("actual-flags"); throw "unreachable"; }
            });
            var copied = RegExp(actual);
            var actualOrder = events.join(",");

            var constructorReads = 0;
            var regexpLike = { source: "y", flags: "m" };
            regexpLike[Symbol.match] = true;
            Object.defineProperty(regexpLike, "constructor", {
              get: function () { constructorReads += 1; throw "unreachable"; }
            });
            var constructed = new RegExp(regexpLike);

            var originalMatch = RegExp.prototype[Symbol.match];
            RegExp.prototype[Symbol.match] = undefined;
            var fromPrototype = RegExp(RegExp.prototype);
            RegExp.prototype[Symbol.match] = originalMatch;

            events = [];
            var marker = {};
            var abruptPattern = {};
            Object.defineProperty(abruptPattern, Symbol.match, {
              get: function () { events.push("match"); return true; }
            });
            Object.defineProperty(abruptPattern, "source", {
              get: function () { events.push("source"); throw marker; }
            });
            var abrupt = false;
            try { Reflect.construct(RegExp, [abruptPattern], NewTarget); }
            catch (error) { abrupt = error === marker; }
            var abruptOrder = events.join(",");

            [
              constructOrder,
              result.source,
              result.flags,
              shortcutOrder,
              shortcut,
              actualOrder,
              copied.source,
              copied.flags,
              copied !== actual,
              constructorReads,
              constructed.source,
              constructed.flags,
              fromPrototype !== RegExp.prototype && fromPrototype.test("//"),
              abruptOrder,
              abrupt
            ].join("|");
            "#,
        )
        .expect("RegExp construction should follow the specification phases"),
        Value::String(
            "match,source,flags,prototype,source-string,flags-string|x|gi|match,constructor|true|actual-match|a|g|true|0|y|m|true|match,source|true"
                .into()
        )
    );
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
}

#[test]
fn regexp_constructor_uses_immutable_foreign_realm_intrinsics() {
    let mut vm = Vm::new().expect("VM should initialize");
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
            var OtherRegExp = other.RegExp;
            var OtherRegExpPrototype = OtherRegExp.prototype;
            var originalTest = OtherRegExpPrototype.test;

            OtherRegExp.prototype = null;
            var foreignCall = OtherRegExp("call", "i");
            var callUsesIntrinsic =
              Object.getPrototypeOf(foreignCall) === OtherRegExpPrototype;

            var C = new other.Function();
            C.prototype = null;
            var BoundNewTarget = C.bind(null);
            var ProxyNewTarget = new Proxy(C, {});
            other.eval(
              "RegExp = function ReplacementRegExp() {};" +
              "RegExp.prototype = { wrong: true };"
            );
            OtherRegExp = null;
            OtherRegExpPrototype = null;
            foreignCall = null;
            forceGc();

            var plain = Reflect.construct(RegExp, ["plain"], C);
            var bound = Reflect.construct(RegExp, ["bound"], BoundNewTarget);
            var proxied = Reflect.construct(RegExp, ["proxy"], ProxyNewTarget);
            var prototype = Object.getPrototypeOf(plain);
            [
              callUsesIntrinsic,
              prototype.wrong === undefined,
              prototype.test === originalTest,
              Object.getPrototypeOf(prototype) === other.Object.prototype,
              Object.getPrototypeOf(bound) === prototype,
              Object.getPrototypeOf(proxied) === prototype,
              originalTest.call(plain, "plain"),
              originalTest.call(bound, "bound"),
              originalTest.call(proxied, "proxy")
            ].join("|");
            "#,
        )
        .expect("RegExp fallback should use immutable foreign Realm intrinsics"),
        Value::String("true|true|true|true|true|true|true|true|true".into())
    );
}

#[test]
fn regexp_symbol_split_roots_every_observable_intermediate() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");
    let baseline_pins = vm.gc_pins.len();

    assert_eq!(
        vm.run(
            r#"
            var receiver = {};
            Object.defineProperty(receiver, "constructor", {
              get: function () {
                forceGc();
                var holder = {};
                Object.defineProperty(holder, Symbol.species, {
                  get: function () {
                    forceGc();
                    return function () {
                      forceGc();
                      var index = 0;
                      return {
                        set lastIndex(value) { index = value; forceGc(); },
                        get lastIndex() {
                          return {
                            valueOf: function () { forceGc(); return index + 1; }
                          };
                        },
                        exec: function () {
                          forceGc();
                          return {
                            get length() {
                              return {
                                valueOf: function () { forceGc(); return 3; }
                              };
                            },
                            get 1() { return { marker: 41 }; },
                            get 2() { forceGc(); return { marker: 42 }; }
                          };
                        }
                      };
                    };
                  }
                });
                return holder;
              }
            });
            Object.defineProperty(receiver, "flags", {
              get: function () {
                forceGc();
                return {
                  toString: function () { forceGc(); return ""; }
                };
              }
            });
            var input = {
              toString: function () { forceGc(); return "a"; }
            };
            var limit = {
              valueOf: function () { forceGc(); return 4; }
            };
            var result = RegExp.prototype[Symbol.split].call(receiver, input, limit);
            forceGc();
            [result.length, result[0], result[1].marker, result[2].marker, result[3]].join("|");
            "#,
        )
        .expect("RegExp @@split intermediates should survive observable GC"),
        Value::String("4||41|42|".into())
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let error = vm
        .run(
            r#"
            var abrupt = {
              flags: "",
              constructor: {
                [Symbol.species]: function () {
                  return {
                    lastIndex: 0,
                    exec: function () {
                      this.lastIndex = 1;
                      return {
                        length: 2,
                        get 1() { forceGc(); throw new Error("capture-abrupt"); }
                      };
                    }
                  };
                }
              }
            };
            RegExp.prototype[Symbol.split].call(abrupt, "a");
            "#,
        )
        .expect_err("capture getter should remain abrupt");
    assert!(error.to_string().contains("capture-abrupt"));
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn regexp_symbol_split_reclaims_native_match_arrays_at_the_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run("globalThis.regexp = /,/; globalThis.input = 'a,b,c';")
        .expect("RegExp split fixtures should initialize");
    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count() + 3));

    assert_eq!(
        vm.run("regexp[Symbol.split](input).join('|');")
            .expect("RegExp split should reclaim earlier native match arrays"),
        Value::String("a|b|c".into())
    );
    vm.set_max_heap_objects(None);
}

#[test]
fn regexp_symbol_replace_roots_every_observable_intermediate() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");
    let baseline_pins = vm.gc_pins.len();

    assert_eq!(
        vm.run(
            r#"
            var calls = 0;
            var observedLastIndex = -1;
            var receiver = {
              get flags() {
                forceGc();
                return { toString: function () { forceGc(); return "g"; } };
              },
              get lastIndex() { forceGc(); return observedLastIndex; },
              set lastIndex(value) { observedLastIndex = value; forceGc(); },
              get exec() {
                forceGc();
                return function () {
                  forceGc();
                  calls += 1;
                  if (calls > 1) return null;
                  return {
                    get length() {
                      forceGc();
                      return { valueOf: function () { forceGc(); return 2; } };
                    },
                    get 0() {
                      forceGc();
                      return { toString: function () { forceGc(); return "a"; } };
                    },
                    get 1() {
                      forceGc();
                      return { toString: function () { forceGc(); return "capture"; } };
                    },
                    get index() {
                      forceGc();
                      return { valueOf: function () { forceGc(); return 1; } };
                    },
                    get groups() {
                      forceGc();
                      return {
                        get name() {
                          forceGc();
                          return {
                            toString: function () { forceGc(); return "named"; }
                          };
                        }
                      };
                    }
                  };
                };
              }
            };
            var input = { toString: function () { forceGc(); return "xa"; } };
            var replacement = {
              toString: function () { forceGc(); return "[$1|$<name>]"; }
            };
            var output = RegExp.prototype[Symbol.replace].call(
              receiver, input, replacement
            );
            [output, calls, observedLastIndex].join("|");
            "#,
        )
        .expect("RegExp @@replace intermediates should survive observable GC"),
        Value::String("x[capture|named]|2|0".into())
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let error = vm
        .run(
            r#"
            var abrupt = {
              flags: "",
              exec: function () {
                return {
                  length: 2,
                  0: "",
                  index: 0,
                  get 1() { forceGc(); throw new Error("capture-abrupt"); }
                };
              }
            };
            RegExp.prototype[Symbol.replace].call(abrupt, "a", "x");
            "#,
        )
        .expect_err("capture getter should remain abrupt");
    assert!(error.to_string().contains("capture-abrupt"));
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn regexp_symbol_replace_named_groups_obey_the_exact_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run("globalThis.replaceRegexp = /(?<name>a)/; globalThis.replaceInput = 'a';")
        .expect("RegExp replace fixtures should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();

    vm.set_max_heap_objects(Some(baseline_live + 1));
    let error = vm
        .run("replaceRegexp[Symbol.replace](replaceInput, '$<name>');")
        .expect_err("the named-groups object must obey the heap cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert!(vm.heap.live_count() <= baseline_live + 1);

    vm.set_max_heap_objects(None);
    vm.gc();
    let exact_limit = vm.heap.live_count() + 2;
    vm.set_max_heap_objects(Some(exact_limit));
    assert_eq!(
        vm.run("replaceRegexp[Symbol.replace](replaceInput, '$<name>');")
            .expect("one result array and one groups object should fit exactly"),
        Value::String("a".into())
    );
    assert!(vm.heap.live_count() <= exact_limit);
    vm.set_max_heap_objects(None);
}

#[test]
fn regexp_constructor_roots_intermediates_across_observable_gc() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");
    vm.register_fn(
        "fillHeapWithGarbage",
        |vm, _, _| {
            vm.gc();
            vm.set_max_heap_objects(Some(vm.heap.live_count() + 1));
            let _garbage = vm.new_object()?;
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("heap-cap hook should register");
    vm.register_fn(
        "uncapHeap",
        |vm, _, _| {
            vm.set_max_heap_objects(None);
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("heap uncap hook should register");

    let result = vm.run(
        r#"
        var events = [];
        var pattern = {};
        Object.defineProperty(pattern, Symbol.match, {
          get: function () { events.push("match"); return true; }
        });
        Object.defineProperty(pattern, "source", {
          get: function () {
            events.push("source");
            return { toString: function () {
              events.push("source-string");
              forceGc();
              return "rooted";
            } };
          }
        });
        Object.defineProperty(pattern, "flags", {
          get: function () {
            events.push("flags");
            forceGc();
            return { toString: function () {
              events.push("flags-string");
              forceGc();
              return "g";
            } };
          }
        });
        var NewTarget = (function () {}).bind(null);
        Object.defineProperty(NewTarget, "prototype", {
          get: function () {
            events.push("prototype");
            var prototype = Object.create(RegExp.prototype);
            prototype.marker = 41;
            forceGc();
            return prototype;
          },
          configurable: true
        });
        var regexp = Reflect.construct(RegExp, [pattern], NewTarget);

        var CapNewTarget = (function () {}).bind(null);
        Object.defineProperty(CapNewTarget, "prototype", {
          get: function () {
            var prototype = Object.create(RegExp.prototype);
            prototype.marker = 42;
            fillHeapWithGarbage();
            return prototype;
          },
          configurable: true
        });
        var capped = Reflect.construct(RegExp, ["capped"], CapNewTarget);
        uncapHeap();
        [
          events.join(","),
          regexp.source,
          regexp.flags,
          Object.getPrototypeOf(regexp).marker,
          regexp.test("rooted"),
          capped.source,
          Object.getPrototypeOf(capped).marker
        ].join("|");
        "#,
    );
    vm.set_max_heap_objects(None);
    assert_eq!(
        result.expect("RegExp intermediates should survive re-entrant collection"),
        Value::String(
            "match,source,flags,prototype,source-string,flags-string|rooted|g|41|true|capped|42"
                .into()
        )
    );
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
}

#[test]
fn regexp_allocation_retries_gc_and_obeys_the_exact_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    let baseline_pins = vm.gc_pins.len();
    for _ in 0..64 {
        let _garbage = vm.new_object().expect("garbage object should allocate");
    }
    let limit = baseline_live + 1;
    vm.set_max_heap_objects(Some(limit));
    assert_eq!(
        vm.run("new RegExp('exact').source;")
            .expect("RegExp allocation should collect garbage and use one cell"),
        Value::String("exact".into())
    );
    assert!(vm.heap.live_count() <= limit);

    vm.set_max_heap_objects(None);
    vm.run(
        "var regexpCoercions = 0; var regexpPattern = { toString: function () { regexpCoercions += 1; return 'x'; } };",
    )
    .expect("saturated-order fixture should initialize");
    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let error = vm
        .run("new RegExp(regexpPattern);")
        .expect_err("a saturated heap must reject RegExp allocation");
    vm.set_max_heap_objects(None);

    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_main_realm_range_error(&vm, error.as_ref());
    assert_eq!(
        vm.run("regexpCoercions;")
            .expect("coercion counter should remain readable"),
        Value::Number(0.0),
        "RegExpAlloc must fail before RegExpInitialize coercion"
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
}

#[test]
fn regexp_match_indices_root_nested_arrays_and_obey_the_exact_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run("globalThis.indicesRegexp = /(?<a>a)(?<b>b)?/d; globalThis.indicesInput = 'a';")
        .expect("match-indices fixtures should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    let baseline_pins = vm.gc_pins.len();
    for _ in 0..64 {
        let _garbage = vm.new_object().expect("garbage object should allocate");
    }

    // Result Array, string groups, two matched index pairs, indices groups,
    // and the outer indices Array are the six live objects retained by exec.
    let exact_limit = baseline_live + 6;
    vm.set_max_heap_objects(Some(exact_limit));
    vm.run("globalThis.indicesResult = indicesRegexp.exec(indicesInput);")
        .expect("nested match-indices objects should survive exact-cap GC");
    assert!(vm.heap.live_count() <= exact_limit);
    vm.set_max_heap_objects(None);
    assert_eq!(
        vm.run(
            "[indicesResult.indices[0].join(','), indicesResult.indices.groups.a === indicesResult.indices[1], indicesResult.indices.groups.b === undefined].join('|');"
        )
        .expect("match-indices result should remain intact"),
        Value::String("0,1|true|true".into())
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.run("indicesResult = null;")
        .expect("match-indices result should be releasable");
    vm.gc();
    let failure_baseline = vm.heap.live_count();
    vm.set_max_heap_objects(Some(failure_baseline + 5));
    let error = vm
        .run("indicesRegexp.exec(indicesInput);")
        .expect_err("one cell below the exact requirement must fail");
    vm.set_max_heap_objects(None);
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_main_realm_range_error(&vm, error.as_ref());
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn proxy_define_property_trap_survives_descriptor_allocation_gc() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "makeFreshTrap",
        |vm, _, _| {
            vm.set_max_heap_objects(None);
            vm.gc();
            let trap = vm.new_native_function(
                "fresh defineProperty trap",
                |vm, _args, _this| {
                    vm.set_max_heap_objects(None);
                    Ok(Value::Bool(true))
                },
                3,
            )?;
            let trap = Value::Object(trap);
            let trap_pin = vm.pin(&trap);
            vm.gc();
            vm.set_max_heap_objects(Some(vm.heap.live_count() + 1));
            let _garbage = vm.new_object()?;
            vm.unpin(trap_pin);
            Ok(trap)
        },
        0,
    )
    .expect("heap-cap hook should register");
    let baseline_pins = vm.gc_pins.len();

    let result = vm.run(
        r#"
        var calls = 0;
        var target = {};
        var handler = {
          get defineProperty() {
            calls += 1;
            return makeFreshTrap();
          }
        };
        var proxy = new Proxy(target, handler);
        var assignmentCalls = 0;
        var assignmentTarget = {};
        var assignmentHandler = {
          set: null,
          getOwnPropertyDescriptor: function () { return undefined; },
          get defineProperty() {
            assignmentCalls += 1;
            return makeFreshTrap();
          }
        };
        var assignmentProxy = new Proxy(assignmentTarget, assignmentHandler);
        Object.defineProperty(proxy, "x", {
          value: 42,
          writable: true,
          enumerable: true,
          configurable: true
        });
        var assignmentResult = Reflect.set(assignmentProxy, "y", 7);
        [
          calls,
          "x" in target,
          assignmentResult,
          assignmentCalls,
          "y" in assignmentTarget
        ].join("|");
        "#,
    );
    vm.set_max_heap_objects(None);
    assert_eq!(
        result.expect("fresh defineProperty trap should survive descriptor allocation GC"),
        Value::String("1|false|true|1|false".into())
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn exotic_integrity_descriptors_retry_gc_at_exact_cap_and_restore_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        globalThis.pendingSealTarget = new Map([["entry", 1]]);
        pendingSealTarget.first = 1;
        pendingSealTarget.second = 2;
        globalThis.pendingFreezeTarget = Promise.resolve(1);
        pendingFreezeTarget.first = 1;
        pendingFreezeTarget.second = 2;
        function takeSealTarget() {
          var target = pendingSealTarget;
          pendingSealTarget = null;
          return target;
        }
        function takeFreezeTarget() {
          var target = pendingFreezeTarget;
          pendingFreezeTarget = null;
          return target;
        }
        "#,
    )
    .expect("integrity fixtures should initialize");
    vm.gc();
    let exact_live = vm.heap.live_count();
    let baseline_pins = vm.gc_pins.len();
    vm.set_max_heap_objects(Some(exact_live + 2));

    vm.run("globalThis.sealedResult = Object.seal(takeSealTarget());")
        .expect("two reusable descriptor cells should complete exotic sealing");
    assert!(vm.heap.live_count() <= exact_live + 2);
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_max_heap_objects(None);
    vm.gc();
    let freeze_live = vm.heap.live_count();
    vm.set_max_heap_objects(Some(freeze_live + 2));
    vm.run("globalThis.frozenResult = Object.freeze(takeFreezeTarget());")
        .expect("two reusable descriptor cells should complete exotic freezing");
    assert!(vm.heap.live_count() <= freeze_live + 2);
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_max_heap_objects(None);
    assert_eq!(
        vm.run(
            r#"
            Object.isSealed(sealedResult) &&
              !Object.isFrozen(sealedResult) &&
              sealedResult.get("entry") === 1 &&
              !Object.prototype.hasOwnProperty.call(sealedResult, "entry") &&
              Object.isFrozen(frozenResult) &&
              sealedResult.first === 1 && frozenResult.first === 1
            "#,
        )
        .expect("integrity results should remain live after cap-triggered GC"),
        Value::Bool(true)
    );

    vm.run("globalThis.failureTarget = new Map(); failureTarget.first = 1;")
        .expect("failure fixture should initialize");
    vm.gc();
    let saturated_live = vm.heap.live_count();
    vm.set_max_heap_objects(Some(saturated_live));
    let error = vm
        .run("Object.freeze(failureTarget);")
        .expect_err("a saturated heap must reject the integrity descriptor allocation");
    vm.set_max_heap_objects(None);

    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_main_realm_range_error(&vm, error.as_ref());
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(
        vm.run("Object.isExtensible(failureTarget);")
            .expect("the partially completed target should remain usable"),
        Value::Bool(false)
    );
}

#[test]
fn reflect_omitted_property_keys_root_proxy_arguments_across_gc() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");
    let baseline_pins = vm.gc_pins.len();

    let result = vm.run(
        r#"
        var calls = [];
        function makeGetProxy() {
          var proxy;
          proxy = new Proxy({}, {
            get: function(target, key, receiver) {
              forceGc();
              calls.push("get:" + key + ":" + (receiver === proxy));
              return 41;
            }
          });
          return proxy;
        }
        function makeHasProxy() {
          return new Proxy({}, {
            has: function(target, key) {
              forceGc();
              calls.push("has:" + key);
              return true;
            }
          });
        }
        var setTarget = {};
        function makeSetProxy() {
          var proxy;
          proxy = new Proxy(setTarget, {
            set: function(target, key, value, receiver) {
              forceGc();
              calls.push(
                "set:" + key + ":" + String(value) + ":" +
                (receiver === proxy)
              );
              return Reflect.set(target, key, value, receiver);
            }
          });
          return proxy;
        }
        [
          Reflect.get(makeGetProxy()),
          Reflect.has(makeHasProxy()),
          Reflect.set(makeSetProxy()),
          calls.join(","),
          Object.prototype.hasOwnProperty.call(setTarget, "undefined")
        ].join("|");
        "#,
    );
    assert_eq!(
        result.expect("omitted-key Reflect operations should survive trap GC"),
        Value::String(
            "41|true|true|get:undefined:true,has:undefined,set:undefined:undefined:true|true"
                .into()
        )
    );

    let abrupt = vm.run(
        r#"
        var errors = [];
        for (var method of ["get", "set", "has"]) {
          var handler = {};
          handler[method] = function() {
            forceGc();
            throw new Error(method + " abrupt");
          };
          try { Reflect[method](new Proxy({}, handler)); }
          catch (error) { errors.push(error.message); }
        }
        errors.join("|");
        "#,
    );
    assert_eq!(
        abrupt.expect("omitted-key trap errors should survive GC"),
        Value::String("get abrupt|set abrupt|has abrupt".into())
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn proxy_delete_property_roots_observable_intermediates_and_restores_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC test hook should register");

    let transparent_proxy = vm
        .run(
            r#"
            var transparentDeleteGets = 0;
            var collectingTransparentHandler = {};
            Object.defineProperty(collectingTransparentHandler, "deleteProperty", {
              get: function() {
                transparentDeleteGets += 1;
                forceGc();
                return null;
              }
            });
            (function() {
              var proxy = { value: 1 };
              for (var i = 0; i < 4; i += 1) {
                proxy = new Proxy(proxy, collectingTransparentHandler);
              }
              return proxy;
            })();
            "#,
        )
        .expect("collecting transparent Proxy fixture should initialize");
    let baseline = vm.gc_pins.len();
    assert!(vm
        .delete_property(&transparent_proxy, "value")
        .expect("transparent deletion should survive collection at every hop"));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(vm.get_global("transparentDeleteGets"), Value::Number(4.0));
    assert_eq!(
        vm.get_property(&transparent_proxy, "value")
            .expect("the transparent chain should remain usable"),
        Value::Undefined
    );

    let proxy = vm
        .run(
            r#"
            var deleteLog = [];
            function deleteTrap() {
              deleteLog.push("delete-call");
              forceGc();
              return true;
            }
            function descriptorTrap(target, key) {
              deleteLog.push("descriptor-call");
              forceGc();
              return Reflect.getOwnPropertyDescriptor(target, key);
            }
            function extensibleTrap(target) {
              deleteLog.push("extensible-call");
              forceGc();
              return Reflect.isExtensible(target);
            }
            var deleteHandler = {};
            Object.defineProperty(deleteHandler, "deleteProperty", {
              get: function() {
                deleteLog.push("delete-get");
                forceGc();
                return deleteTrap;
              }
            });
            var invariantHandler = {};
            Object.defineProperty(invariantHandler, "getOwnPropertyDescriptor", {
              get: function() {
                deleteLog.push("descriptor-get");
                forceGc();
                return descriptorTrap;
              }
            });
            Object.defineProperty(invariantHandler, "isExtensible", {
              get: function() {
                deleteLog.push("extensible-get");
                forceGc();
                return extensibleTrap;
              }
            });
            (function() {
              var base = {};
              Object.defineProperty(base, "fixed", {
                value: 1,
                configurable: true
              });
              Object.preventExtensions(base);
              var invariantTarget = new Proxy(base, invariantHandler);
              return new Proxy(invariantTarget, deleteHandler);
            })();
            "#,
        )
        .expect("Proxy fixture should initialize");
    let baseline = vm.gc_pins.len();
    let error = vm
        .delete_property(&proxy, "fixed")
        .expect_err("the non-extensible target invariant should throw");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.run("deleteLog.join('|')")
            .expect("every invariant hook should remain observable"),
        Value::String(
            "delete-get|delete-call|descriptor-get|descriptor-call|extensible-get|extensible-call"
                .into()
        )
    );
    assert_eq!(
        vm.get_property(&proxy, "fixed")
            .expect("the rejected deletion must preserve the target property"),
        Value::Number(1.0)
    );

    for source in [
        r#"
        (function() {
          var marker = {};
          var handler = {};
          Object.defineProperty(handler, "deleteProperty", {
            get: function() { forceGc(); throw marker; }
          });
          return new Proxy({ value: 1 }, handler);
        })();
        "#,
        r#"
        (function() {
          var marker = {};
          return new Proxy({ value: 1 }, {
            get deleteProperty() {
              forceGc();
              return function() { forceGc(); throw marker; };
            }
          });
        })();
        "#,
        r#"
        (function() {
          return new Proxy({ value: 1 }, { deleteProperty: 1 });
        })();
        "#,
        r#"
        (function() {
          var revocable = Proxy.revocable({ value: 1 }, {});
          var outer = new Proxy(revocable.proxy, { deleteProperty: null });
          revocable.revoke();
          return outer;
        })();
        "#,
        r#"
        (function() {
          var target = new Proxy({ value: 1 }, {
            getOwnPropertyDescriptor: function() { forceGc(); throw {}; }
          });
          return new Proxy(target, {
            deleteProperty: function() { return true; }
          });
        })();
        "#,
        r#"
        (function() {
          var target = new Proxy({ value: 1 }, {
            getOwnPropertyDescriptor: function(actualTarget, key) {
              return Reflect.getOwnPropertyDescriptor(actualTarget, key);
            },
            isExtensible: function() { forceGc(); throw {}; }
          });
          return new Proxy(target, {
            deleteProperty: function() { return true; }
          });
        })();
        "#,
    ] {
        let proxy = vm
            .run(source)
            .expect("abrupt Proxy fixture should initialize");
        let baseline = vm.gc_pins.len();
        vm.delete_property(&proxy, "value")
            .expect_err("Proxy deletion should preserve the abrupt completion");
        assert_eq!(vm.gc_pins.len(), baseline);
    }
}

#[test]
fn proxy_delete_property_transparent_chain_consumes_fuel_and_restores_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var deleteTarget = { value: 1 };
        var deleteProxy = deleteTarget;
        var transparentHandler = {};
        for (var i = 0; i < 100; i += 1) {
          deleteProxy = new Proxy(deleteProxy, transparentHandler);
        }
        "#,
    )
    .expect("transparent Proxy fixture should initialize");
    let proxy = vm.get_global("deleteProxy");
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(20));
    let error = vm
        .delete_property(&proxy, "value")
        .expect_err("transparent Proxy traversal should exhaust fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(200));
    assert!(vm
        .delete_property(&proxy, "value")
        .expect("refilled fuel should complete the same deletion"));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("Object.prototype.hasOwnProperty.call(deleteTarget, 'value')")
            .expect("the refilled VM should delete the target property"),
        Value::Bool(false)
    );
}

#[test]
fn proxy_delete_property_nested_proxy_walks_consume_fuel_and_restore_pin_depth() {
    let mut handler_vm = Vm::new().expect("handler VM should initialize");
    handler_vm
        .run(
            r#"
            var nestedHandlerTarget = { value: 1 };
            var nestedDeleteHandler = {};
            for (var i = 0; i < 100; i += 1) {
              nestedDeleteHandler = new Proxy(nestedDeleteHandler, {});
            }
            var nestedHandlerProxy = new Proxy(
              nestedHandlerTarget, nestedDeleteHandler
            );
            "#,
        )
        .expect("deep handler fixture should initialize");
    let proxy = handler_vm.get_global("nestedHandlerProxy");
    let baseline = handler_vm.gc_pins.len();

    handler_vm.set_fuel(Some(20));
    let error = handler_vm
        .delete_property(&proxy, "value")
        .expect_err("deep Proxy handler lookup should exhaust fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(handler_vm.fuel_remaining(), Some(0));
    assert_eq!(handler_vm.gc_pins.len(), baseline);
    handler_vm.set_fuel(Some(200));
    assert!(handler_vm
        .delete_property(&proxy, "value")
        .expect("refilled handler traversal should complete"));
    assert_eq!(handler_vm.gc_pins.len(), baseline);
    handler_vm.set_fuel(None);
    assert_eq!(
        handler_vm
            .run("Object.prototype.hasOwnProperty.call(nestedHandlerTarget, 'value')")
            .expect("handler VM should remain reusable"),
        Value::Bool(false)
    );

    let mut invariant_vm = Vm::new().expect("invariant VM should initialize");
    invariant_vm
        .register_fn("truthyDelete", |_, _, _| Ok(Value::Bool(true)), 2)
        .expect("native delete trap should register");
    invariant_vm
        .run(
            r#"
            var nestedInvariantBase = { value: 1 };
            var nestedInvariantTarget = nestedInvariantBase;
            var invariantHandler = {};
            for (var i = 0; i < 100; i += 1) {
              nestedInvariantTarget = new Proxy(
                nestedInvariantTarget, invariantHandler
              );
            }
            var nestedInvariantProxy = new Proxy(nestedInvariantTarget, {
              deleteProperty: truthyDelete
            });
            "#,
        )
        .expect("deep invariant fixture should initialize");
    let proxy = invariant_vm.get_global("nestedInvariantProxy");
    let baseline = invariant_vm.gc_pins.len();

    invariant_vm.set_fuel(Some(101));
    let error = invariant_vm
        .delete_property(&proxy, "value")
        .expect_err("descriptor plus extensibility traversal should exhaust fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(invariant_vm.fuel_remaining(), Some(0));
    assert_eq!(invariant_vm.gc_pins.len(), baseline);
    invariant_vm.set_fuel(Some(300));
    assert!(invariant_vm
        .delete_property(&proxy, "value")
        .expect("refilled invariant traversal should complete"));
    assert_eq!(invariant_vm.gc_pins.len(), baseline);
    invariant_vm.set_fuel(None);
    assert_eq!(
        invariant_vm
            .run("nestedInvariantBase.value")
            .expect("invariant VM should remain reusable"),
        Value::Number(1.0)
    );
}

#[test]
fn proxy_prevent_extensions_roots_observable_intermediates_and_restores_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC test hook should register");

    let transparent = vm
        .run(
            r#"
            var transparentPreventGets = 0;
            var collectingPreventHandler = {};
            Object.defineProperty(collectingPreventHandler, "preventExtensions", {
              get: function () {
                transparentPreventGets += 1;
                forceGc();
                return null;
              }
            });
            var transparentPreventBase = {};
            var transparentPreventProxy = transparentPreventBase;
            for (var i = 0; i < 4; i += 1) {
              transparentPreventProxy = new Proxy(
                transparentPreventProxy,
                collectingPreventHandler
              );
            }
            transparentPreventProxy;
            "#,
        )
        .expect("collecting transparent Proxy fixture should initialize");
    let baseline = vm.gc_pins.len();
    assert!(vm
        .prevent_extensions(&transparent)
        .expect("transparent preventExtensions should survive collection"));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.run("transparentPreventGets + ':' + Object.isExtensible(transparentPreventBase)")
            .expect("transparent target should remain observable"),
        Value::String("4:false".into())
    );

    let invariant = vm
        .run(
            r#"
            var preventLog = [];
            function collectingPreventTrap() {
              preventLog.push("prevent-call");
              forceGc();
              return true;
            }
            function collectingExtensibleTrap(target) {
              preventLog.push("extensible-call");
              forceGc();
              return Reflect.isExtensible(target);
            }
            var collectingOuterHandler = {};
            Object.defineProperty(
              collectingOuterHandler,
              "preventExtensions",
              {
                get: function () {
                  preventLog.push("prevent-get");
                  forceGc();
                  return collectingPreventTrap;
                }
              }
            );
            var collectingInvariantHandler = {};
            Object.defineProperty(
              collectingInvariantHandler,
              "isExtensible",
              {
                get: function () {
                  preventLog.push("extensible-get");
                  forceGc();
                  return collectingExtensibleTrap;
                }
              }
            );
            var collectingPreventBase = Object.preventExtensions({});
            var collectingPreventTarget = new Proxy(
              collectingPreventBase,
              collectingInvariantHandler
            );
            new Proxy(collectingPreventTarget, collectingOuterHandler);
            "#,
        )
        .expect("collecting invariant fixture should initialize");
    let baseline = vm.gc_pins.len();
    assert!(vm
        .prevent_extensions(&invariant)
        .expect("nested invariant should survive collection"));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.run("preventLog.join('|')")
            .expect("every observable hook should remain callable"),
        Value::String("prevent-get|prevent-call|extensible-get|extensible-call".into())
    );

    for source in [
        r#"
        (function () {
          var marker = {};
          var handler = {};
          Object.defineProperty(handler, "preventExtensions", {
            get: function () { forceGc(); throw marker; }
          });
          return new Proxy({}, handler);
        })();
        "#,
        r#"
        (function () {
          var marker = {};
          return new Proxy({}, {
            get preventExtensions() {
              forceGc();
              return function () { forceGc(); throw marker; };
            }
          });
        })();
        "#,
        r#"
        (function () {
          return new Proxy({}, { preventExtensions: 1 });
        })();
        "#,
        r#"
        (function () {
          var revocable = Proxy.revocable({}, {});
          var outer = new Proxy(revocable.proxy, { preventExtensions: null });
          revocable.revoke();
          return outer;
        })();
        "#,
        r#"
        (function () {
          var target = new Proxy(Object.preventExtensions({}), {
            isExtensible: function () { forceGc(); throw {}; }
          });
          return new Proxy(target, {
            preventExtensions: function () { forceGc(); return true; }
          });
        })();
        "#,
    ] {
        let proxy = vm
            .run(source)
            .expect("abrupt Proxy fixture should initialize");
        let baseline = vm.gc_pins.len();
        vm.prevent_extensions(&proxy)
            .expect_err("preventExtensions should preserve abrupt completion");
        assert_eq!(vm.gc_pins.len(), baseline);
    }
}

#[test]
fn proxy_prevent_extensions_walks_consume_fuel_and_restore_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var deepPreventBase = {};
        var deepPreventProxy = deepPreventBase;
        var transparentHandler = {};
        for (var i = 0; i < 100000; i += 1) {
          deepPreventProxy = new Proxy(deepPreventProxy, transparentHandler);
        }
        "#,
    )
    .expect("transparent preventExtensions fixture should initialize");
    let proxy = vm.get_global("deepPreventProxy");
    let base = vm.get_global("deepPreventBase");
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(99_999));
    let error = vm
        .prevent_extensions(&proxy)
        .expect_err("N-1 fuel must abort before the transparent target");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert!(vm
        .is_extensible(&base)
        .expect("the aborted traversal must not change the target"));

    vm.set_fuel(Some(100_000));
    assert!(vm
        .prevent_extensions(&proxy)
        .expect("exactly N fuel should complete preventExtensions"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("Object.isExtensible(deepPreventBase)")
            .expect("refilled VM should expose the target state"),
        Value::Bool(false)
    );

    let mut invariant_vm = Vm::new().expect("invariant VM should initialize");
    invariant_vm
        .register_fn("truthyPrevent", |_, _, _| Ok(Value::Bool(true)), 1)
        .expect("native preventExtensions trap should register");
    invariant_vm
        .run(
            r#"
            var deepPreventInvariantBase = Object.preventExtensions({});
            var deepPreventInvariantTarget = deepPreventInvariantBase;
            for (var i = 0; i < 100; i += 1) {
              deepPreventInvariantTarget = new Proxy(
                deepPreventInvariantTarget,
                {}
              );
            }
            var deepPreventInvariantProxy = new Proxy(
              deepPreventInvariantTarget,
              { preventExtensions: truthyPrevent }
            );
            "#,
        )
        .expect("nested preventExtensions invariant fixture should initialize");
    let proxy = invariant_vm.get_global("deepPreventInvariantProxy");
    let baseline = invariant_vm.gc_pins.len();

    invariant_vm.set_fuel(Some(100));
    let error = invariant_vm
        .prevent_extensions(&proxy)
        .expect_err("outer plus N-1 invariant fuel should abort");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(invariant_vm.fuel_remaining(), Some(0));
    assert_eq!(invariant_vm.gc_pins.len(), baseline);

    invariant_vm.set_fuel(Some(101));
    assert!(invariant_vm
        .prevent_extensions(&proxy)
        .expect("outer plus N invariant fuel should complete"));
    assert_eq!(invariant_vm.fuel_remaining(), Some(0));
    assert_eq!(invariant_vm.gc_pins.len(), baseline);
}

#[test]
fn proxy_prototype_internal_methods_root_intermediates_and_restore_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            // Model a new host job so WeakRef's same-job keep cannot mask
            // temporary roots owned by the internal method under test.
            vm.clear_kept_objects();
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC test hook should register");

    vm.run(
        r#"
        var collectingGetBase = {};
        var collectingGetTarget = new Proxy(collectingGetBase, {
          get isExtensible() {
            forceGc();
            return function (target) {
              forceGc();
              return Reflect.isExtensible(target);
            };
          }
        });
        var collectingGetHandler = {};
        Object.defineProperty(collectingGetHandler, "getPrototypeOf", {
          get: function () {
            forceGc();
            return function () {
              forceGc();
              return { marker: 73 };
            };
          }
        });
        var collectingGetProxy = new Proxy(
          collectingGetTarget,
          collectingGetHandler
        );

        var deferredExpectedWeakRef;
        var deferredGetBase = Object.preventExtensions({});
        var deferredGetTarget = new Proxy(deferredGetBase, {
          getPrototypeOf: function (target) {
            forceGc();
            return Reflect.getPrototypeOf(target);
          }
        });
        var deferredGetProxy = new Proxy(deferredGetTarget, {
          getPrototypeOf: function () {
            var expected = { marker: 137 };
            deferredExpectedWeakRef = new WeakRef(expected);
            return expected;
          }
        });

        var collectingSetBase = {};
        var collectingSetHandler = {};
        Object.defineProperty(collectingSetHandler, "setPrototypeOf", {
          get: function () {
            forceGc();
            return function (target, prototype) {
              forceGc();
              return Reflect.setPrototypeOf(target, prototype);
            };
          }
        });
        var collectingSetProxy = new Proxy(
          collectingSetBase,
          collectingSetHandler
        );
        "#,
    )
    .expect("collecting prototype fixtures should initialize");

    let get_proxy = vm.get_global("collectingGetProxy");
    let baseline = vm.gc_pins.len();
    let result = vm
        .get_prototype_of(&get_proxy)
        .expect("getPrototypeOf result should survive nested observable GC")
        .expect("trap should return an object");
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.get_property(&result, "marker")
            .expect("fresh prototype should remain readable"),
        Value::Number(73.0)
    );

    let deferred_get_proxy = vm.get_global("deferredGetProxy");
    let baseline = vm.gc_pins.len();
    let error = vm
        .get_prototype_of(&deferred_get_proxy)
        .expect_err("outer deferred prototype should mismatch after nested GC");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.run("deferredExpectedWeakRef.deref().marker")
            .expect("deferred expected prototype should survive nested collection"),
        Value::Number(137.0)
    );

    let proposed = vm
        .run("({ marker: 91 })")
        .expect("unpublished proposed prototype should allocate");
    let set_proxy = vm.get_global("collectingSetProxy");
    let set_base = vm.get_global("collectingSetBase");
    let baseline = vm.gc_pins.len();
    assert!(vm
        .set_prototype_of(&set_proxy, Some(proposed.clone()))
        .expect("setPrototypeOf argument should survive trap lookup GC"));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.get_prototype_of(&set_base)
            .expect("updated target prototype should remain readable"),
        Some(proposed)
    );

    for source in [
        r#"
        (function () {
          var handler = {};
          Object.defineProperty(handler, "getPrototypeOf", {
            get: function () { forceGc(); throw {}; }
          });
          return new Proxy({}, handler);
        })();
        "#,
        r#"
        (function () {
          return new Proxy({}, {
            getPrototypeOf: function () { forceGc(); throw {}; }
          });
        })();
        "#,
    ] {
        let proxy = vm
            .run(source)
            .expect("abrupt get fixture should initialize");
        let baseline = vm.gc_pins.len();
        vm.get_prototype_of(&proxy)
            .expect_err("getPrototypeOf should preserve abrupt completion");
        assert_eq!(vm.gc_pins.len(), baseline);
    }

    for source in [
        r#"
        (function () {
          var handler = {};
          Object.defineProperty(handler, "setPrototypeOf", {
            get: function () { forceGc(); throw {}; }
          });
          return new Proxy({}, handler);
        })();
        "#,
        r#"
        (function () {
          return new Proxy({}, {
            setPrototypeOf: function () { forceGc(); throw {}; }
          });
        })();
        "#,
    ] {
        let proxy = vm
            .run(source)
            .expect("abrupt set fixture should initialize");
        let proposed = vm
            .run("({})")
            .expect("abrupt proposed prototype should initialize");
        let baseline = vm.gc_pins.len();
        vm.set_prototype_of(&proxy, Some(proposed))
            .expect_err("setPrototypeOf should preserve abrupt completion");
        assert_eq!(vm.gc_pins.len(), baseline);
    }
}

#[test]
fn proxy_prototype_walks_consume_exact_fuel_and_reject_deep_cycles() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var deepPrototypeBase = {};
        var deepPrototypeProxy = deepPrototypeBase;
        var transparentPrototypeHandler = {};
        for (var i = 0; i < 100000; i += 1) {
          deepPrototypeProxy = new Proxy(
            deepPrototypeProxy,
            transparentPrototypeHandler
          );
        }

        var cycleRoot = {};
        var cycleLeaf = cycleRoot;
        for (var j = 0; j < 5000; j += 1) {
          cycleLeaf = Object.create(cycleLeaf);
        }
        "#,
    )
    .expect("deep prototype fixtures should initialize");

    let proxy = vm.get_global("deepPrototypeProxy");
    let base = vm.get_global("deepPrototypeBase");
    let expected = vm
        .get_prototype_of(&base)
        .expect("ordinary base prototype should be readable");
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(99_999));
    let error = vm
        .get_prototype_of(&proxy)
        .expect_err("N-1 fuel must abort transparent getPrototypeOf");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(100_000));
    assert_eq!(
        vm.get_prototype_of(&proxy)
            .expect("exactly N fuel should complete getPrototypeOf"),
        expected
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(99_999));
    let error = vm
        .set_prototype_of(&proxy, None)
        .expect_err("N-1 fuel must abort transparent setPrototypeOf");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.get_prototype_of(&base)
            .expect("aborted set must preserve the base prototype"),
        expected
    );

    vm.set_fuel(Some(100_000));
    assert!(vm
        .set_prototype_of(&proxy, None)
        .expect("exactly N fuel should complete setPrototypeOf"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.get_prototype_of(&base)
            .expect("successful transparent set should remain readable"),
        None
    );

    let root = vm.get_global("cycleRoot");
    let leaf = vm.get_global("cycleLeaf");
    let original = vm
        .get_prototype_of(&root)
        .expect("cycle root prototype should be readable");
    vm.set_fuel(Some(5_000));
    let error = vm
        .set_prototype_of(&root, Some(leaf.clone()))
        .expect_err("fuel must cover every visited cycle candidate");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(5_001));
    assert!(!vm
        .set_prototype_of(&root, Some(leaf))
        .expect("deep cycle should be rejected without a depth cap"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.get_prototype_of(&root)
            .expect("rejected cycle must preserve the root prototype"),
        original
    );
}

#[test]
fn proxy_prototype_invariant_walks_consume_nested_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("prototypeTrap", |vm, _, _| Ok(vm.object_proto.clone()), 1)
        .expect("native getPrototypeOf trap should register");
    vm.register_fn("truthyPrototypeTrap", |_, _, _| Ok(Value::Bool(true)), 2)
        .expect("native setPrototypeOf trap should register");
    vm.run(
        r#"
        var invariantPrototypeBase = Object.preventExtensions({});
        var invariantPrototypeTarget = invariantPrototypeBase;
        for (var i = 0; i < 64; i += 1) {
          invariantPrototypeTarget = new Proxy(invariantPrototypeTarget, {});
        }
        var invariantGetPrototypeProxy = new Proxy(
          invariantPrototypeTarget,
          { getPrototypeOf: prototypeTrap }
        );
        var invariantSetPrototypeProxy = new Proxy(
          invariantPrototypeTarget,
          { setPrototypeOf: truthyPrototypeTrap }
        );
        "#,
    )
    .expect("nested prototype invariant fixtures should initialize");
    let get_proxy = vm.get_global("invariantGetPrototypeProxy");
    let set_proxy = vm.get_global("invariantSetPrototypeProxy");
    let proposed = vm.object_proto.clone();
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(128));
    let error = vm
        .get_prototype_of(&get_proxy)
        .expect_err("outer plus two N-layer invariant walks require 129 fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(129));
    assert_eq!(
        vm.get_prototype_of(&get_proxy)
            .expect("exact invariant fuel should complete"),
        Some(proposed.clone())
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(128));
    let error = vm
        .set_prototype_of(&set_proxy, Some(proposed.clone()))
        .expect_err("set invariant must charge both nested internal methods");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(129));
    assert!(vm
        .set_prototype_of(&set_proxy, Some(proposed))
        .expect("exact set invariant fuel should complete"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn proxy_define_own_property_roots_intermediates_and_restores_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.clear_kept_objects();
            vm.gc();
            vm.new_object().map(Value::Object)
        },
        0,
    )
    .expect("GC test hook should register");
    vm.run(
        r#"
        var internalDefineBase = {};
        var internalDefineHandler = {};
        Object.defineProperty(internalDefineHandler, "defineProperty", {
          get: function () {
            forceGc();
            return function (target, key, descriptor) {
              forceGc();
              return Reflect.defineProperty(target, key, descriptor);
            };
          }
        });
        var internalDefineProxy = new Proxy(
          internalDefineBase,
          internalDefineHandler
        );

        var publicDefineBase = {};
        var publicDefineHandler = {};
        Object.defineProperty(publicDefineHandler, "defineProperty", {
          get: function () {
            forceGc();
            return function (target, key, descriptor) {
              forceGc();
              return Reflect.defineProperty(target, key, descriptor);
            };
          }
        });
        var publicDefineProxy = new Proxy(publicDefineBase, publicDefineHandler);
        "#,
    )
    .expect("collecting defineProperty fixtures should initialize");

    let internal_proxy = vm.get_global("internalDefineProxy");
    vm.gc();
    let internal_value_idx = vm
        .new_object()
        .expect("internal descriptor value should allocate");
    vm.heap.with_obj(internal_value_idx.0, |object| {
        object.props().lock().insert(
            crate::value::PropertyKey::from("marker"),
            crate::value::PropertyDescriptor::data(Value::Number(73.0)),
        );
    });
    let internal_value = Value::Object(internal_value_idx);
    let baseline = vm.gc_pins.len();
    assert!(vm
        .define_own_property(
            &internal_proxy,
            crate::value::PropertyKey::from("x"),
            crate::value::PropertyDescriptor::data(internal_value),
        )
        .expect("complete descriptor should survive trap lookup and call GC"));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.run("internalDefineBase.x.marker")
            .expect("stored internal descriptor value should remain live"),
        Value::Number(73.0)
    );

    let public_proxy = vm.get_global("publicDefineProxy");
    let public_descriptor = vm
        .run(
            r#"({
              value: { marker: 91 },
              writable: true,
              enumerable: true,
              configurable: true
            })"#,
        )
        .expect("public descriptor should allocate");
    let baseline = vm.gc_pins.len();
    assert!(crate::builtins::object_define_property_result(
        &mut vm,
        &[public_proxy, Value::String("y".into()), public_descriptor,],
        false,
    )
    .expect("partial descriptor should survive trap lookup and call GC"));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.run("publicDefineBase.y.marker")
            .expect("stored public descriptor value should remain live"),
        Value::Number(91.0)
    );

    for (source, expected_marker) in [
        (
            r#"
        (function () {
          var handler = {};
          Object.defineProperty(handler, "defineProperty", {
            get: function () { forceGc(); throw { marker: 101 }; }
          });
          return new Proxy({}, handler);
        })();
        "#,
            101.0,
        ),
        (
            r#"
        (function () {
          return new Proxy({}, {
            defineProperty: function () { forceGc(); throw { marker: 102 }; }
          });
        })();
        "#,
            102.0,
        ),
        (
            r#"
        (function () {
          var target = new Proxy({}, {
            getOwnPropertyDescriptor: function () {
              forceGc();
              throw { marker: 103 };
            }
          });
          return new Proxy(target, {
            defineProperty: function () { return true; }
          });
        })();
        "#,
            103.0,
        ),
        (
            r#"
        (function () {
          var target = new Proxy({}, {
            getOwnPropertyDescriptor: function () { return undefined; },
            isExtensible: function () { forceGc(); throw { marker: 104 }; }
          });
          return new Proxy(target, {
            defineProperty: function () { return true; }
          });
        })();
        "#,
            104.0,
        ),
    ] {
        let proxy = vm
            .run(source)
            .expect("abrupt defineProperty fixture should initialize");
        let baseline = vm.gc_pins.len();
        let error = vm
            .define_own_property(
                &proxy,
                crate::value::PropertyKey::from("abrupt"),
                crate::value::PropertyDescriptor::data(Value::Number(1.0)),
            )
            .expect_err("defineProperty should preserve abrupt completion");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(error.kind, crate::error::ErrorKind::User);
        let thrown = error
            .thrown_value
            .clone()
            .expect("abrupt completion should retain its thrown object");
        let thrown_pin = vm.pin(&thrown);
        assert_eq!(
            vm.get_property(&thrown, "marker")
                .expect("thrown marker should remain readable"),
            Value::Number(expected_marker)
        );
        vm.unpin(thrown_pin);
        assert_eq!(vm.gc_pins.len(), baseline);
    }
}

#[test]
fn proxy_define_own_property_walks_consume_exact_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var deepDefineBase = {};
        var deepDefineProxy = deepDefineBase;
        var transparentDefineHandler = {};
        for (var i = 0; i < 100000; i += 1) {
          deepDefineProxy = new Proxy(deepDefineProxy, transparentDefineHandler);
        }
        var deepPublicDescriptor = {
          value: 41,
          writable: true,
          enumerable: true,
          configurable: true
        };
        "#,
    )
    .expect("deep defineProperty fixtures should initialize");
    let proxy = vm.get_global("deepDefineProxy");
    let public_descriptor = vm.get_global("deepPublicDescriptor");
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(99_999));
    let error = crate::builtins::object_define_property_result(
        &mut vm,
        &[
            proxy.clone(),
            Value::String("publicValue".into()),
            public_descriptor.clone(),
        ],
        false,
    )
    .expect_err("N-1 fuel must abort public transparent DefineOwnProperty");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(100_000));
    assert!(crate::builtins::object_define_property_result(
        &mut vm,
        &[
            proxy.clone(),
            Value::String("publicValue".into()),
            public_descriptor,
        ],
        false,
    )
    .expect("exactly N fuel should complete public DefineOwnProperty"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("deepDefineBase.publicValue")
            .expect("public transparent definition should reach the base"),
        Value::Number(41.0)
    );

    vm.set_fuel(Some(99_999));
    let error = vm
        .define_own_property(
            &proxy,
            crate::value::PropertyKey::from("internalValue"),
            crate::value::PropertyDescriptor::data(Value::Number(73.0)),
        )
        .expect_err("N-1 fuel must abort internal transparent DefineOwnProperty");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(100_000));
    assert!(vm
        .define_own_property(
            &proxy,
            crate::value::PropertyKey::from("internalValue"),
            crate::value::PropertyDescriptor::data(Value::Number(73.0)),
        )
        .expect("exactly N fuel should complete internal DefineOwnProperty"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("deepDefineBase.internalValue")
            .expect("internal transparent definition should reach the base"),
        Value::Number(73.0)
    );
}

#[test]
fn proxy_define_own_property_invariant_walks_consume_nested_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("truthyDefineTrap", |_, _, _| Ok(Value::Bool(true)), 3)
        .expect("native defineProperty trap should register");
    vm.run(
        r#"
        var invariantDefineBase = {};
        var invariantDefineTarget = invariantDefineBase;
        for (var i = 0; i < 64; i += 1) {
          invariantDefineTarget = new Proxy(invariantDefineTarget, {});
        }
        var invariantDefineProxy = new Proxy(
          invariantDefineTarget,
          { defineProperty: truthyDefineTrap }
        );
        "#,
    )
    .expect("nested defineProperty invariant fixtures should initialize");
    let proxy = vm.get_global("invariantDefineProxy");
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(128));
    let error = vm
        .define_own_property(
            &proxy,
            crate::value::PropertyKey::from("x"),
            crate::value::PropertyDescriptor::data(Value::Number(1.0)),
        )
        .expect_err("outer plus two N-layer invariant walks require 129 fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(129));
    assert!(vm
        .define_own_property(
            &proxy,
            crate::value::PropertyKey::from("x"),
            crate::value::PropertyDescriptor::data(Value::Number(1.0)),
        )
        .expect("exact nested invariant fuel should complete"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn proxy_define_own_property_callable_proxy_traps_are_iterative_and_ordered() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("terminalDefineTrap", |_, _, _| Ok(Value::Bool(true)), 3)
        .expect("terminal defineProperty trap should register");
    vm.run(
        r#"
        var callableDefineTrap = terminalDefineTrap;
        for (var i = 0; i < 25000; i += 1) {
          callableDefineTrap = new Proxy(callableDefineTrap, {});
        }
        var callableDefineProxy = new Proxy(
          {},
          { defineProperty: callableDefineTrap }
        );
        var trappedCallableDefineTrap = terminalDefineTrap;
        for (var j = 0; j < 4096; j += 1) {
          trappedCallableDefineTrap = new Proxy(function () {}, {
            apply: trappedCallableDefineTrap
          });
        }
        var trappedCallableDefineProxy = new Proxy(
          {},
          { defineProperty: trappedCallableDefineTrap }
        );
        var nonCallableDefineProxy = new Proxy(
          {},
          { defineProperty: {} }
        );
        "#,
    )
    .expect("callable Proxy trap fixtures should initialize");
    let callable_proxy = vm.get_global("callableDefineProxy");
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(25_000));
    let error = vm
        .define_own_property(
            &callable_proxy,
            crate::value::PropertyKey::from("x"),
            crate::value::PropertyDescriptor::data(Value::Number(1.0)),
        )
        .expect_err("outer define plus N callable Proxy layers require N+1 fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(25_001));
    assert!(vm
        .define_own_property(
            &callable_proxy,
            crate::value::PropertyKey::from("x"),
            crate::value::PropertyDescriptor::data(Value::Number(1.0)),
        )
        .expect("exact callable Proxy trap fuel should complete"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    let trapped_callable_proxy = vm.get_global("trappedCallableDefineProxy");
    vm.set_fuel(Some(4_096));
    let error = vm
        .define_own_property(
            &trapped_callable_proxy,
            crate::value::PropertyKey::from("x"),
            crate::value::PropertyDescriptor::data(Value::Number(1.0)),
        )
        .expect_err("outer define plus N trapped Proxy calls require N+1 fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(4_097));
    assert!(vm
        .define_own_property(
            &trapped_callable_proxy,
            crate::value::PropertyKey::from("x"),
            crate::value::PropertyDescriptor::data(Value::Number(1.0)),
        )
        .expect("exact trapped callable Proxy fuel should complete"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(None);
    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let non_callable_proxy = vm.get_global("nonCallableDefineProxy");
    let error = vm
        .define_own_property(
            &non_callable_proxy,
            crate::value::PropertyKey::from("x"),
            crate::value::PropertyDescriptor::data(Value::Number(1.0)),
        )
        .expect_err("GetMethod must reject a non-callable trap before allocation");
    vm.set_max_heap_objects(None);
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn proxy_descriptor_conversion_roots_get_results_across_gc() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");
    let baseline_pins = vm.gc_pins.len();

    let result = vm.run(
        r#"
        var dataTarget = {};
        Object.defineProperty(dataTarget, "x", {
          value: 0,
          writable: true,
          configurable: true
        });
        var dataDescriptor = new Proxy({}, {
          has: function (_, key) {
            if (key === "writable") {
              forceGc();
              globalThis.dataReuse = { marker: 99 };
            }
            return key === "configurable" || key === "value" || key === "writable";
          },
          get: function (_, key) {
            if (key === "configurable" || key === "writable") return true;
            if (key === "value") return { marker: 41 };
          }
        });
        var dataProxy = new Proxy(dataTarget, {
          getOwnPropertyDescriptor: function () { return dataDescriptor; }
        });
        var dataValue = Object.getOwnPropertyDescriptor(dataProxy, "x").value;

        var accessorTarget = {};
        Object.defineProperty(accessorTarget, "x", {
          get: function () { return 0; },
          configurable: true
        });
        var accessorDescriptor = new Proxy({}, {
          has: function (_, key) {
            if (key === "set") {
              forceGc();
              globalThis.accessorReuse = function () { return 99; };
            }
            return key === "configurable" || key === "get" || key === "set";
          },
          get: function (_, key) {
            if (key === "configurable") return true;
            if (key === "get") return function () { return 42; };
            if (key === "set") return undefined;
          }
        });
        var accessorProxy = new Proxy(accessorTarget, {
          getOwnPropertyDescriptor: function () { return accessorDescriptor; }
        });
        var accessor = Object.getOwnPropertyDescriptor(accessorProxy, "x").get;
        [dataValue.marker, accessor()].join("|");
        "#,
    );

    assert_eq!(
        result.expect("descriptor Get results should survive later Proxy traps"),
        Value::String("41|42".into())
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn primitive_wrapper_constructors_defer_prototype_lookup_until_after_conversion() {
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run(
            r#"
            var events = [];
            function newTarget(label, prototype) {
              return new Proxy(function () {}, {
                get: function (target, key) {
                  if (key === "prototype") {
                    events.push(label + "-prototype");
                    return prototype;
                  }
                  return target[key];
                }
              });
            }

            var stringPrototype = {};
            var stringValue = {
              toString: function () {
                events.push("string-coercion");
                return "\uD83D\uDE00";
              }
            };
            var boxedString = Reflect.construct(
              String, [stringValue], newTarget("string", stringPrototype)
            );

            var numberPrototype = {};
            var numberValue = {
              valueOf: function () {
                events.push("number-coercion");
                return 7;
              }
            };
            var boxedNumber = Reflect.construct(
              Number, [numberValue], newTarget("number", numberPrototype)
            );

            var booleanPrototype = {};
            var boxedBoolean = Reflect.construct(
              Boolean, [0], newTarget("boolean", booleanPrototype)
            );

            var marker = {};
            var abruptPrototypeReads = 0;
            var AbruptNewTarget = new Proxy(function () {}, {
              get: function (target, key) {
                if (key === "prototype") abruptPrototypeReads += 1;
                return target[key];
              }
            });
            var abruptBeforePrototype = false;
            try {
              Reflect.construct(Number, [{
                valueOf: function () {
                  events.push("number-abrupt");
                  throw marker;
                }
              }], AbruptNewTarget);
            } catch (error) {
              abruptBeforePrototype = error === marker;
            }

            var symbolPrototypeReads = 0;
            var SymbolNewTarget = new Proxy(function () {}, {
              get: function (target, key) {
                if (key === "prototype") symbolPrototypeReads += 1;
                return target[key];
              }
            });
            var symbolConstructThrows = false;
            try { Reflect.construct(String, [Symbol("x")], SymbolNewTarget); }
            catch (error) { symbolConstructThrows = error instanceof TypeError; }

            var stringReceiver = {};
            var numberReceiver = {};
            var booleanReceiver = {};
            var stringCall = String.call(stringReceiver, "plain");
            var numberCall = Number.call(numberReceiver, 11);
            var booleanCall = Boolean.call(booleanReceiver, 1);
            var callReceiversUnmodified = 0;
            try { String.prototype.valueOf.call(stringReceiver); }
            catch (error) { if (error instanceof TypeError) callReceiversUnmodified += 1; }
            try { Number.prototype.valueOf.call(numberReceiver); }
            catch (error) { if (error instanceof TypeError) callReceiversUnmodified += 1; }
            try { Boolean.prototype.valueOf.call(booleanReceiver); }
            catch (error) { if (error instanceof TypeError) callReceiversUnmodified += 1; }

            var stringLength = Object.getOwnPropertyDescriptor(boxedString, "length");
            [
              events.join(","),
              Object.getPrototypeOf(boxedString) === stringPrototype,
              Object.getPrototypeOf(boxedNumber) === numberPrototype,
              Object.getPrototypeOf(boxedBoolean) === booleanPrototype,
              String.prototype.valueOf.call(boxedString) === "\uD83D\uDE00",
              Number.prototype.valueOf.call(boxedNumber) === 7,
              Boolean.prototype.valueOf.call(boxedBoolean) === false,
              stringLength.value === 2,
              !stringLength.writable && !stringLength.enumerable &&
                !stringLength.configurable,
              abruptBeforePrototype,
              abruptPrototypeReads,
              symbolConstructThrows,
              symbolPrototypeReads,
              String(Symbol("x")),
              stringCall === "plain",
              numberCall === 11,
              booleanCall === true,
              callReceiversUnmodified === 3
            ].join("|");
            "#,
        )
        .expect("primitive wrapper construction should follow specification order"),
        Value::String(
            "string-coercion,string-prototype,number-coercion,number-prototype,boolean-prototype,number-abrupt|true|true|true|true|true|true|true|true|true|0|true|0|Symbol(x)|true|true|true|true"
                .into()
        )
    );
}

#[test]
fn primitive_wrapper_fallbacks_use_the_new_target_realm_intrinsics() {
    let mut vm = Vm::new().expect("VM should initialize");
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
            var PlainNewTarget = new other.Function();
            PlainNewTarget.prototype = null;
            var BoundNewTarget = PlainNewTarget.bind(null);
            var ProxyNewTarget = new Proxy(PlainNewTarget, {});

            other.eval(
              "String = function ReplacementString() {};" +
              "String.prototype = { wrong: true };" +
              "Number = function ReplacementNumber() {};" +
              "Number.prototype = { wrong: true };" +
              "Boolean = function ReplacementBoolean() {};" +
              "Boolean.prototype = { wrong: true };"
            );
            forceGc();

            var boxedString = Reflect.construct(String, ["ok"], PlainNewTarget);
            var boxedNumber = Reflect.construct(Number, [9], BoundNewTarget);
            var boxedBoolean = Reflect.construct(Boolean, [1], ProxyNewTarget);
            var stringPrototype = Object.getPrototypeOf(boxedString);
            var numberPrototype = Object.getPrototypeOf(boxedNumber);
            var booleanPrototype = Object.getPrototypeOf(boxedBoolean);

            [
              stringPrototype.wrong === undefined,
              numberPrototype.wrong === undefined,
              booleanPrototype.wrong === undefined,
              Object.getPrototypeOf(stringPrototype.valueOf) === other.Function.prototype,
              Object.getPrototypeOf(numberPrototype.valueOf) === other.Function.prototype,
              Object.getPrototypeOf(booleanPrototype.valueOf) === other.Function.prototype,
              stringPrototype.valueOf.call(boxedString) === "ok",
              numberPrototype.valueOf.call(boxedNumber) === 9,
              booleanPrototype.valueOf.call(boxedBoolean) === true
            ].join("|");
            "#,
        )
        .expect("primitive wrappers should use immutable foreign Realm intrinsics"),
        Value::String("true|true|true|true|true|true|true|true|true".into())
    );
}

#[test]
fn primitive_wrapper_prototype_result_survives_observable_gc() {
    let mut vm = Vm::new().expect("VM should initialize");
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
            var NewTarget = new Proxy(function () {}, {
              get: function (target, key) {
                if (key === "prototype") {
                  var prototype = { marker: 42 };
                  forceGc();
                  return prototype;
                }
                return target[key];
              }
            });
            var wrapper = Reflect.construct(Number, [1], NewTarget);
            Object.getPrototypeOf(wrapper).marker;
            "#,
        )
        .expect("prototype getter result should survive wrapper allocation"),
        Value::Number(42.0)
    );
}

#[test]
fn primitive_wrapper_allocation_obeys_the_exact_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    let baseline_pins = vm.gc_pins.len();

    vm.set_max_heap_objects(Some(baseline_live + 1));
    assert_eq!(
        vm.run("new String('x').valueOf();")
            .expect("one free cell should be enough for a String wrapper"),
        Value::String("x".into())
    );
    assert!(vm.heap.live_count() <= baseline_live + 1);

    vm.set_max_heap_objects(None);
    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let error = vm
        .run("new Number(1);")
        .expect_err("a saturated heap must reject wrapper allocation");
    vm.set_max_heap_objects(None);

    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_main_realm_range_error(&vm, error.as_ref());
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
}

#[test]
fn date_constructor_defers_prototype_and_hides_date_value() {
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run(
            r#"
            var receiver = {};
            var callCoercions = 0;
            var poison = {
              valueOf: function () {
                callCoercions += 1;
                throw new Error("coerced");
              }
            };
            var direct = Date.call(receiver, poison);
            var applied = Date.apply(receiver, [poison]);
            var bound = Date.bind(receiver, poison)();
            var receiverRejected = false;
            try { Date.prototype.getTime.call(receiver); }
            catch (error) { receiverRejected = error instanceof TypeError; }

            function newTarget(log) {
              var target = (function () {}).bind(null);
              Object.defineProperty(target, "prototype", {
                get: function () {
                  log.push("prototype");
                  return Date.prototype;
                },
                configurable: true
              });
              return target;
            }

            var log = [];
            var one = Reflect.construct(Date, [{
              valueOf: function () { log.push("value"); return 0; }
            }], newTarget(log));
            var oneLog = log.join(",");

            log = [];
            try {
              Reflect.construct(Date, [{
                valueOf: function () {
                  log.push("abrupt");
                  throw new Error("boom");
                }
              }], newTarget(log));
            } catch (error) {}
            var abruptLog = log.join(",");

            log = [];
            function value(name, number) {
              return { valueOf: function () { log.push(name); return number; } };
            }
            var many = Reflect.construct(Date, [
              value("year", 1970), value("month", 0), value("day", 1)
            ], newTarget(log));

            one.__time__ = 99;
            delete one.__time__;
            one.setTime(8);
            [
              typeof direct,
              typeof applied,
              typeof bound,
              callCoercions,
              receiverRejected,
              Object.getOwnPropertyNames(receiver).length,
              oneLog,
              abruptLog,
              log.join(","),
              typeof Date.prototype.getTime.call(many) === "number",
              one.getTime(),
              Object.getOwnPropertyNames(one).length
            ].join("|");
            "#,
        )
        .expect("Date call/construct order should be observable"),
        Value::String(
            "string|string|string|0|true|0|value,prototype|abrupt|year,month,day,prototype|true|8|0"
                .into()
        )
    );
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
}

#[test]
fn date_constructor_uses_immutable_foreign_realm_intrinsics() {
    let mut vm = Vm::new().expect("VM should initialize");
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
            var OtherDate = other.Date;
            var OtherDatePrototype = OtherDate.prototype;
            var C = new other.Function();
            C.prototype = null;
            var BoundNewTarget = C.bind(null);
            var ProxyNewTarget = new Proxy(C, {});
            var getTime = OtherDatePrototype.getTime;
            var constructorShape =
              Object.getPrototypeOf(OtherDate) === other.Function.prototype;
            var methodShape =
              Object.getPrototypeOf(getTime) === other.Function.prototype;
            var callCoercions = 0;
            var callResult = OtherDate.call({}, {
              valueOf: function () { callCoercions += 1; throw new Error("coerced"); }
            });
            other.eval("Date = null;");
            var bindingCleared = other.eval("Date === null;");
            other.Date = { prototype: { wrong: true } };
            OtherDate = null;
            OtherDatePrototype = null;
            forceGc();

            var plain = Reflect.construct(Date, [0], C);
            var bound = Reflect.construct(Date, [1], BoundNewTarget);
            var proxied = Reflect.construct(Date, [2], ProxyNewTarget);
            var plainPrototype = Object.getPrototypeOf(plain);
            var realmError = false;
            try { getTime.call({}); }
            catch (error) { realmError = error instanceof other.TypeError; }

            [
              constructorShape,
              methodShape,
              bindingCleared,
              plainPrototype !== plain,
              plainPrototype.wrong === undefined,
              Object.getPrototypeOf(plainPrototype) === other.Object.prototype,
              plainPrototype.getTime === getTime,
              Object.getPrototypeOf(bound) === plainPrototype,
              Object.getPrototypeOf(proxied) === plainPrototype,
              getTime.call(proxied),
              typeof callResult,
              callCoercions,
              realmError
            ].join("|");
            "#,
        )
        .expect("Date fallback should use immutable foreign Realm intrinsics"),
        Value::String("true|true|true|true|true|true|true|true|true|2|string|0|true".into())
    );
}

#[test]
fn date_prototype_result_survives_cap_triggered_gc() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");

    let result = vm.run(
        r#"
            var NewTarget = (function () {}).bind(null);
            Object.defineProperty(NewTarget, "prototype", {
              get: function () {
                var prototype = { marker: 42 };
                capHeap();
                return prototype;
              },
              configurable: true
            });
            var date = Reflect.construct(Date, [{
              valueOf: function () { return 1; }
            }], NewTarget);
            Object.getPrototypeOf(date).marker + Date.prototype.getTime.call(date);
            "#,
    );
    vm.set_max_heap_objects(None);
    assert_eq!(
        result.expect("Date inputs and prototype should survive observable GC"),
        Value::Number(43.0)
    );
}

#[test]
fn date_allocation_obeys_gc_retry_and_the_exact_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    let baseline_pins = vm.gc_pins.len();
    for _ in 0..64 {
        let _garbage = vm.new_object().expect("garbage object should allocate");
    }
    let limit = baseline_live + 1;
    vm.set_max_heap_objects(Some(limit));
    assert_eq!(
        vm.run("new Date(123).getTime();")
            .expect("Date allocation should collect garbage and use one cell"),
        Value::Number(123.0)
    );
    assert!(vm.heap.live_count() <= limit);

    vm.set_max_heap_objects(None);
    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let error = vm
        .run("new Date(1);")
        .expect_err("a saturated heap must reject Date allocation");
    vm.set_max_heap_objects(None);

    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_main_realm_range_error(&vm, error.as_ref());
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
    assert!(matches!(
        vm.run("Date.call({}, { valueOf: function () { throw 1; } });"),
        Ok(Value::String(_))
    ));
}

#[test]
fn dynamic_function_constructors_follow_conversion_and_parse_order() {
    let mut vm = Vm::new().expect("VM should initialize");
    let baseline_pins = vm.gc_pins.len();
    let baseline_contexts = vm.execution_contexts.len();
    assert_eq!(
        vm.run(
            r#"
            var constructors = [
              Function,
              (async function () {}).constructor,
              (function* () {}).constructor,
              (async function* () {}).constructor
            ];
            var all = true;
            for (var i = 0; i < constructors.length; i++) {
              var C = constructors[i];
              var log = [];
              function source(label, text) {
                return {
                  toString: function () {
                    log.push(label);
                    return text;
                  }
                };
              }
              var NewTarget = (function () {}).bind(null);
              Object.defineProperty(NewTarget, "prototype", {
                get: function () {
                  log.push("prototype");
                  return C.prototype;
                },
                configurable: true
              });
              var created = Reflect.construct(C, [
                source("p1", "a"),
                source("p2", "b"),
                source("body", "return 1;")
              ], NewTarget);
              all = all && log.join(",") === "p1,p2,body,prototype";
              all = all && Object.getPrototypeOf(created) === C.prototype;
              var constructable = true;
              try { Reflect.construct(function () {}, [], created); }
              catch (error) { constructable = false; }
              all = all && constructable === (i === 0);
              all = all && (i === 1 ? created.prototype === undefined :
                Object.prototype.hasOwnProperty.call(created, "prototype"));

              var marker = {};
              log = [];
              var parameterAbrupt = false;
              try {
                Reflect.construct(C, [{
                  toString: function () {
                    log.push("p1");
                    throw marker;
                  }
                }, source("p2", "b"), source("body", "")], NewTarget);
              } catch (error) {
                parameterAbrupt = error === marker;
              }
              all = all && parameterAbrupt && log.join(",") === "p1";

              log = [];
              var bodyAbrupt = false;
              try {
                Reflect.construct(C, [
                  source("p1", "a"),
                  source("p2", "b"),
                  { toString: function () { log.push("body"); throw marker; } }
                ], NewTarget);
              } catch (error) {
                bodyAbrupt = error === marker;
              }
              all = all && bodyAbrupt && log.join(",") === "p1,p2,body";

              log = [];
              var syntaxBeforePrototype = false;
              try { Reflect.construct(C, ["("], NewTarget); }
              catch (error) { syntaxBeforePrototype = error instanceof SyntaxError; }
              all = all && syntaxBeforePrototype && log.length === 0;

              log = [];
              var ThrowingNewTarget = (function () {}).bind(null);
              Object.defineProperty(ThrowingNewTarget, "prototype", {
                get: function () { log.push("prototype"); throw marker; },
                configurable: true
              });
              var getterAbrupt = false;
              try { Reflect.construct(C, [""], ThrowingNewTarget); }
              catch (error) { getterAbrupt = error === marker; }
              all = all && getterAbrupt && log.join(",") === "prototype";

              var separateParse = false;
              try { C("/*", "*/ ) {"); }
              catch (error) { separateParse = error instanceof SyntaxError; }
              all = all && separateParse;

              var parameterInjection = false;
              try { C(") {} function x(", ""); }
              catch (error) { parameterInjection = error instanceof SyntaxError; }
              var bodyInjection = false;
              try { C("", "} function x() {"); }
              catch (error) { bodyInjection = error instanceof SyntaxError; }
              all = all && parameterInjection && bodyInjection;

              var objectParameter = C("{x}", "return x;");
              var arrayParameter = C("[x]", "return x;");
              var undefinedParameter = C(undefined, "return undefined;");
              var restUndefinedParameter = C("...undefined", "return undefined.length;");
              var lineCommentBoundary = C("x //", "return x //");
              var contextualParameters = C(
                "async", "of", "static", "let", "get", "set",
                "return async + of + static + let + get + set;"
              );
              var bodyArrow = C("return (undefined) => undefined;");
              all = all && objectParameter.length === 1;
              all = all && arrayParameter.length === 1;
              all = all && undefinedParameter.length === 1;
              all = all && restUndefinedParameter.length === 0;
              all = all && lineCommentBoundary.length === 1;
              all = all && contextualParameters.length === 6;
              all = all && bodyArrow.length === 0;
              if (i === 0) {
                all = all && objectParameter({ x: 7 }) === 7;
                all = all && arrayParameter([8]) === 8;
                all = all && undefinedParameter(9) === 9;
                all = all && restUndefinedParameter(1, 2, 3) === 3;
                all = all && lineCommentBoundary(10) === 10;
                all = all && contextualParameters(1, 2, 3, 4, 5, 6) === 21;
                all = all && bodyArrow()(11) === 11;
              } else if (i === 2) {
                all = all && objectParameter({ x: 12 }).next().value === 12;
                all = all && arrayParameter([13]).next().value === 13;
                all = all && undefinedParameter(14).next().value === 14;
                all = all && restUndefinedParameter(1, 2).next().value === 2;
                all = all && lineCommentBoundary(15).next().value === 15;
                all = all && contextualParameters(1, 2, 3, 4, 5, 6).next().value === 21;
                all = all && bodyArrow().next().value(16) === 16;
              }

              var strictNonSimple = false;
              try { C("x = 1", '"use strict";'); }
              catch (error) { strictNonSimple = error instanceof SyntaxError; }
              all = all && strictNonSimple;
              var strictReserved = true;
              var reservedNames = ["public", "package", "yield"];
              for (var r = 0; r < reservedNames.length; r++) {
                try {
                  C(reservedNames[r], '"use strict";');
                  strictReserved = false;
                } catch (error) {
                  strictReserved = strictReserved && error instanceof SyntaxError;
                }
              }
              all = all && strictReserved;

              var called = C.call({ marker: "ignored" }, "return 1;");
              var boundCalled = C.bind({ marker: "ignored" }, "return 1;")();
              all = all && Object.getPrototypeOf(called) === C.prototype;
              all = all && Object.getPrototypeOf(boundCalled) === C.prototype;
            }
            all;
            "#,
        )
        .expect("dynamic Function families should follow CreateDynamicFunction order"),
        Value::Bool(true)
    );
    // The script above contains ordinary function syntax, whose compiled
    // definitions legitimately remain in the VM table. Take the checkpoint
    // after that compilation, then isolate successful dynamic constructors
    // with a source string that contains no additional function definitions.
    let baseline_functions = vm.functions.len();
    assert_eq!(
        vm.run(
            "for (var i = 0; i < constructors.length; i++) constructors[i](''); constructors.length;"
        )
        .expect("simple dynamic functions should compile without table entries"),
        Value::Number(4.0)
    );
    assert_eq!(vm.functions.len(), baseline_functions);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
}

#[test]
fn dynamic_function_constructors_use_immutable_new_target_realm_fallbacks() {
    let mut vm = Vm::new().expect("VM should initialize");
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
            var realmA = $262.createRealm().global;
            var realmB = $262.createRealm().global;
            var evalA = realmA.eval;
            var evalB = realmB.eval;
            var nestedTarget = new Proxy(function () {}, {
              isExtensible: function () {
                forceGc();
                return true;
              }
            });
            var nestedProxy = new Proxy(nestedTarget, {
              getPrototypeOf: function () {
                return { marker: 23 };
              }
            });
            var nestedBound = Function.prototype.bind.call(nestedProxy, null);
            var nestedPrototype = Object.getPrototypeOf(nestedBound);
            var nestedProxyPrototypeOk = nestedPrototype.marker === 23;
            var constructors = [
              realmA.Function,
              Object.getPrototypeOf(evalA("(async function () {})")).constructor,
              Object.getPrototypeOf(evalA("(function* () {})")).constructor,
              Object.getPrototypeOf(evalA("(async function* () {})")).constructor
            ];
            var expected = [
              realmB.Function.prototype,
              Object.getPrototypeOf(evalB("(async function () {})")),
              Object.getPrototypeOf(evalB("(function* () {})")),
              Object.getPrototypeOf(evalB("(async function* () {})"))
            ];
            var expectedRefs = expected.map(function (value) {
              return new WeakRef(value);
            });
            var ownPrototypeParents = [
              realmA.Object.prototype,
              undefined,
              Object.getPrototypeOf(evalA("(function* () {}).prototype")),
              Object.getPrototypeOf(evalA("(async function* () {}).prototype"))
            ];
            var inheritedPrototype = { marker: 37 };
            realmB.Function.prototype.prototype = inheritedPrototype;
            var BoundTarget = new realmB.Function();
            var BoundNewTarget = BoundTarget.bind(null);
            var boundPrototypeOk =
              Object.getPrototypeOf(BoundNewTarget) === realmB.Function.prototype;
            for (var b = 0; b < constructors.length; b++) {
              var boundGenerated = Reflect.construct(constructors[b], [], BoundNewTarget);
              boundPrototypeOk = boundPrototypeOk &&
                Object.getPrototypeOf(boundGenerated) === inheritedPrototype;
            }
            delete realmB.Function.prototype.prototype;

            var NewTarget = new realmB.Function();
            NewTarget.prototype = null;
            var targets = [
              NewTarget,
              NewTarget.bind(null),
              new Proxy(NewTarget, {})
            ];

            delete constructors[2].prototype.prototype;
            delete constructors[3].prototype.prototype;
            evalA("Function = null; Object = null;");
            evalB("Function = null; Object = null;");
            expected = null;
            forceGc();

            var all = boundPrototypeOk && nestedProxyPrototypeOk;
            for (var i = 0; i < constructors.length; i++) {
              var expectedPrototype = expectedRefs[i].deref();
              all = all && expectedPrototype !== undefined;
              for (var j = 0; j < targets.length; j++) {
                var generated = Reflect.construct(constructors[i], [], targets[j]);
                all = all && Object.getPrototypeOf(generated) === expectedPrototype;
                if (i !== 1) {
                  all = all && Object.getPrototypeOf(generated.prototype) ===
                    ownPrototypeParents[i];
                } else {
                  all = all && generated.prototype === undefined;
                }
              }
              var syntaxRealm = false;
              try { constructors[i]("("); }
              catch (error) { syntaxRealm = error instanceof realmA.SyntaxError; }
              all = all && syntaxRealm;
            }
            all;
            "#,
        )
        .expect("dynamic Function fallbacks should use immutable Realm intrinsics"),
        Value::Bool(true)
    );
}

#[test]
fn dynamic_function_allocation_retries_gc_and_obeys_the_exact_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    let baseline_pins = vm.gc_pins.len();
    let baseline_functions = vm.functions.len();
    for _ in 0..64 {
        let _garbage = vm.new_object().expect("garbage object should allocate");
    }
    let limit = baseline_live + 2;
    vm.set_max_heap_objects(Some(limit));
    assert_eq!(
        vm.run(
            "var dynamic = Function(''); Object.getPrototypeOf(dynamic.prototype) === Object.prototype;"
        )
        .expect("normal Function should collect garbage and allocate exactly two cells"),
        Value::Bool(true)
    );
    assert!(vm.heap.live_count() <= limit);

    vm.set_max_heap_objects(None);
    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let error = vm
        .run("Function('');")
        .expect_err("a saturated heap must reject dynamic Function allocation");
    vm.set_max_heap_objects(None);
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_main_realm_range_error(&vm, error.as_ref());
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.functions.len(), baseline_functions);
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());

    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    let baseline_pins = vm.gc_pins.len();
    let baseline_functions = vm.functions.len();
    let limit = baseline_live + 1;
    vm.set_max_heap_objects(Some(limit));
    let error = vm
        .run("Function('return function nested() {};');")
        .expect_err("one free cell must fail when normal Function needs its prototype");
    vm.set_max_heap_objects(None);
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_main_realm_range_error(&vm, error.as_ref());
    assert!(vm.heap.live_count() <= limit);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.functions.len(), baseline_functions);
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
    vm.gc();
    assert_eq!(vm.heap.live_count(), baseline_live);

    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    vm.run(
        r#"
        var OuterNewTarget = (function () {}).bind(null);
        Object.defineProperty(OuterNewTarget, "prototype", {
          get: function () {
            globalThis.savedDynamic = Function(
              "return function inner() { return 17; };"
            );
            var captured = savedDynamic;
            globalThis.keepGetterEnvironment = function () { return captured; };
            capHeap();
            return Function.prototype;
          },
          configurable: true
        });
        "#,
    )
    .expect("reentrant allocation fixture should initialize");
    let baseline_functions = vm.functions.len();
    let baseline_pins = vm.gc_pins.len();
    let baseline_contexts = vm.execution_contexts.len();
    let error = vm
        .run("Reflect.construct(Function, ['return function outer() {};'], OuterNewTarget);")
        .expect_err("the outer allocation should fail after reentrant compilation");
    vm.set_max_heap_objects(None);
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_main_realm_range_error(&vm, error.as_ref());
    assert_eq!(vm.functions.len(), baseline_functions + 1);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
    assert_eq!(
        vm.run("savedDynamic()();")
            .expect("the successful reentrant function should remain usable"),
        Value::Number(17.0)
    );

    for (setup, create, label) in [
        (
            "var DynamicConstructor = (function* () {}).constructor;",
            "var dynamic = DynamicConstructor(''); Object.getPrototypeOf(dynamic.prototype) === DynamicConstructor.prototype.prototype;",
            "GeneratorFunction",
        ),
        (
            "var DynamicConstructor = (async function* () {}).constructor;",
            "var dynamic = DynamicConstructor(''); Object.getPrototypeOf(dynamic.prototype) === DynamicConstructor.prototype.prototype;",
            "AsyncGeneratorFunction",
        ),
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.run(setup)
            .unwrap_or_else(|error| panic!("{label} setup should run: {error}"));
        vm.gc();
        let baseline_live = vm.heap.live_count();
        let baseline_pins = vm.gc_pins.len();
        let baseline_functions = vm.functions.len();
        for _ in 0..64 {
            let _garbage = vm.new_object().expect("garbage object should allocate");
        }
        let limit = baseline_live + 2;
        vm.set_max_heap_objects(Some(limit));
        assert_eq!(
            vm.run(create)
                .unwrap_or_else(|error| panic!("{label} exact-cap allocation failed: {error}")),
            Value::Bool(true)
        );
        vm.set_max_heap_objects(None);
        assert!(vm.heap.live_count() <= limit);
        assert_eq!(vm.gc_pins.len(), baseline_pins);
        assert_eq!(vm.functions.len(), baseline_functions);
        assert!(vm.pending_new_target.is_none());
        assert!(vm.pending_new_target_prototype.is_none());
    }

    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    let result = vm.run(
        r#"
        var AsyncFunction = (async function () {}).constructor;
        var NewTarget = (function () {}).bind(null);
        Object.defineProperty(NewTarget, "prototype", {
          get: function () {
            var prototype = { marker: 41 };
            var garbage1 = {};
            var garbage2 = {};
            capHeap();
            return prototype;
          },
          configurable: true
        });
        var dynamic = Reflect.construct(AsyncFunction, ["return 1;"], NewTarget);
        Object.getPrototypeOf(dynamic).marker;
        "#,
    );
    vm.set_max_heap_objects(None);
    assert_eq!(
        result.expect("getter-produced prototype should survive collecting allocation"),
        Value::Number(41.0)
    );
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
}

#[test]
fn created_realm_native_constructor_modes_match_main_registrations() {
    let mut vm = Vm::new().expect("VM should initialize");
    let global = vm
        .run("$262.createRealm().global;")
        .expect("Realm global should be created");
    let eval = vm
        .get_property(&global, "eval")
        .expect("Realm global should expose eval");
    let pin_count = vm.pin_many(&[global.clone(), eval.clone()]);

    for &source in FOREIGN_EAGER_NATIVE_CONSTRUCTOR_SOURCES {
        let constructor = vm
            .call_function(&eval, &[Value::String(source.into())], Some(global.clone()))
            .expect("foreign eager constructor should resolve");
        assert!(vm.is_constructor_value(&constructor));
        assert_eq!(
            native_construct_mode(&vm, &constructor),
            Some(NativeConstructMode::InternalEagerPrototype),
            "unexpected foreign construct mode for {source}"
        );
    }
    for &source in DEFERRED_NATIVE_CONSTRUCTOR_SOURCES {
        let constructor = vm
            .call_function(&eval, &[Value::String(source.into())], Some(global.clone()))
            .expect("foreign deferred constructor should resolve");
        assert!(vm.is_constructor_value(&constructor));
        assert_eq!(
            native_construct_mode(&vm, &constructor),
            Some(NativeConstructMode::InternalDeferredPrototype),
            "unexpected foreign construct mode for {source}"
        );
    }
    for &source in NON_CONSTRUCTIBLE_NATIVE_FUNCTION_SOURCES {
        let function = vm
            .call_function(&eval, &[Value::String(source.into())], Some(global.clone()))
            .expect("foreign native function should resolve");
        assert!(
            !vm.is_constructor_value(&function),
            "{source} must not construct"
        );
        assert_eq!(
            native_construct_mode(&vm, &function),
            None,
            "unexpected foreign construct metadata for {source}"
        );
    }

    vm.unpin_many(pin_count);
}

#[test]
fn created_realm_body_controlled_constructors_preserve_realm_and_order() {
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var prototypeReads = [];
            var coercions = 0;
            var active = "";
            var NewTarget = new Proxy(function () {}, {
              get: function (target, key) {
                if (key === "prototype") prototypeReads.push(active);
                return target[key];
              }
            });
            function foreignTypeError(label, target, args) {
              active = label;
              try { Reflect.construct(target, args, NewTarget); }
              catch (error) { return error instanceof other.TypeError; }
              return false;
            }

            var bigintError = foreignTypeError("BigInt", other.BigInt, [{
              valueOf: function () { coercions += 1; return 1; }
            }]);
            var symbolError = foreignTypeError("Symbol", other.Symbol, [{
              toString: function () { coercions += 1; return "description"; }
            }]);
            var OtherTypedArray = Object.getPrototypeOf(other.Int8Array);
            var typedArrayError = foreignTypeError("TypedArray", OtherTypedArray, []);

            [
              bigintError,
              symbolError,
              typedArrayError,
              prototypeReads.join(","),
              coercions
            ].join("|");
            "#,
        )
        .expect("foreign constructors should throw in their own Realm"),
        Value::String("true|true|true||0".into())
    );
}

#[test]
fn created_realm_proxy_intrinsic_and_revocable_results_are_realm_local() {
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var callError = other.eval(
              "try { Proxy({}, {}); false; } catch (error) { error instanceof TypeError; }"
            );
            var newTargetResult = Reflect.construct(function () {}, [], other.Proxy);
            var pair = other.Proxy.revocable(function () {}, {});
            var trapArrayIsRealmLocal = other.eval(
              "var seen; var P = new Proxy(function () {}, {" +
              "construct: function (target, args) {" +
              "seen = Object.getPrototypeOf(args) === Array.prototype; return {};" +
              "}}); new P(); seen;"
            );
            [
              other.Proxy !== Proxy,
              callError,
              Object.getPrototypeOf(other.Proxy) === other.Function.prototype,
              Object.getPrototypeOf(other.Proxy.revocable) === other.Function.prototype,
              Object.getPrototypeOf(newTargetResult) === other.Object.prototype,
              Object.getPrototypeOf(pair) === other.Object.prototype,
              Object.getPrototypeOf(pair.revoke) === other.Function.prototype,
              trapArrayIsRealmLocal
            ].join("|");
            "#,
        )
        .expect("created Realm Proxy surface should be observable"),
        Value::String("true|true|true|true|true|true|true|true".into())
    );
}

#[test]
fn proxy_revocable_roots_intermediates_across_exact_cap_collection() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        "globalThis.proxyTarget = function () {}; proxyTarget.x = 1; globalThis.proxyHandler = {};",
    )
    .expect("Proxy fixture should initialize");
    vm.gc();
    let baseline = vm.heap.live_count();
    let _unrooted_garbage = vm.new_object().expect("garbage object should allocate");
    let limit = baseline + 3;
    vm.set_max_heap_objects(Some(limit));

    let pair = vm
        .run("Proxy.revocable(proxyTarget, proxyHandler);")
        .expect("garbage collection should leave room for the rooted result");
    vm.set_max_heap_objects(None);
    let pair_pin = vm.pin(&pair);
    let proxy = vm
        .get_property(&pair, "proxy")
        .expect("result should retain its proxy");
    let revoke = vm
        .get_property(&pair, "revoke")
        .expect("result should retain its revoker");
    assert_eq!(
        vm.get_property(&proxy, "x")
            .expect("live proxy should forward target properties"),
        Value::Number(1.0)
    );
    assert_eq!(
        vm.call_function(&revoke, &[], None)
            .expect("live revoker should remain callable"),
        Value::Undefined
    );
    let error = vm
        .get_property(&proxy, "x")
        .expect_err("revoked proxy should reject later access");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert!(vm.heap.live_count() <= limit);
    vm.unpin_many(pair_pin);
}

#[test]
fn construction_state_restores_after_pre_dispatch_depth_error() {
    let mut vm = Vm::new().expect("VM should initialize");
    let chunk = std::sync::Arc::new(crate::bytecode::Chunk::default());
    for _ in 0..512 {
        vm.frames.push(super::CallFrame::new(
            chunk.clone(),
            0,
            0,
            Vec::new(),
            vm.global,
            Value::Undefined,
        ));
    }
    let array = vm.get_global("Array");
    let error = vm
        .construct(&array, &[])
        .expect_err("native dispatch should hit the call-depth guard");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    vm.frames.clear();
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());
    let error = vm
        .run("class C {} C();")
        .expect_err("plain class call must remain rejected after depth failure");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
}

#[test]
fn eager_constructor_prototype_getter_keeps_arguments_rooted() {
    let mut vm = Vm::new().expect("VM should initialize");
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
            var Constructor = new Proxy(Array, {
              get: function (target, key) {
                if (key === "prototype") {
                  forceGc();
                  return Array.prototype;
                }
                return target[key];
              }
            });
            var result = new Constructor({ marker: 42 });
            result[0].marker;
            "#,
        )
        .expect("constructor argument should survive the prototype getter"),
        Value::Number(42.0)
    );
}

#[test]
fn failed_realm_construction_rolls_back_every_heap_boundary() {
    let required_capacity = realm_creation_live_delta();
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    let baseline_registries = realm_registry_counts(&vm);
    let baseline_pins = vm.gc_pins.len();

    for extra_capacity in 0..required_capacity {
        assert_failed_realm_attempt(
            &mut vm,
            baseline_live,
            baseline_registries,
            baseline_pins,
            extra_capacity,
        );
    }

    let wrapper_boundary = required_capacity - 1;
    for _ in 0..2 {
        assert_failed_realm_attempt(
            &mut vm,
            baseline_live,
            baseline_registries,
            baseline_pins,
            wrapper_boundary,
        );
    }

    vm.set_max_heap_objects(Some(baseline_live + required_capacity));
    let realm = vm
        .run("$262.createRealm();")
        .expect("exact required capacity should create a Realm after every rollback");
    vm.set_max_heap_objects(None);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert!(
        realm_registry_counts(&vm)
            .iter()
            .zip(baseline_registries)
            .all(|(populated, baseline)| populated > &baseline),
        "successful Realm creation must publish every registry family"
    );
    let global = vm
        .get_property(&realm, "global")
        .expect("Realm wrapper should expose its global");
    let eval = vm
        .get_property(&global, "eval")
        .expect("Realm global should expose its intrinsic eval");
    assert_eq!(
        vm.call_function(&eval, &[Value::String("1 + 1".into())], Some(global))
            .expect("Realm should remain functional after rollback sweep"),
        Value::Number(2.0)
    );
}

#[test]
fn realm_environment_survives_collection_before_intrinsic_publication() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_registries = realm_registry_counts(&vm);
    let baseline_realm_envs: Vec<_> = vm.realm_globals.keys().copied().collect();
    let baseline_pins = vm.gc_pins.len();

    let realm = crate::builtins::make_test262_realm_after_environment_gc(&mut vm)
        .expect("the pinned Realm environment should survive pre-publication collection");
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert!(
        realm_registry_counts(&vm)
            .iter()
            .zip(baseline_registries)
            .all(|(populated, baseline)| populated > &baseline),
        "the collected construction must publish every Realm registry family"
    );
    let global = vm
        .get_property(&realm, "global")
        .expect("collected Realm wrapper should expose its global");
    let realm_env = vm
        .realm_globals
        .keys()
        .copied()
        .find(|realm| !baseline_realm_envs.contains(realm))
        .expect("successful construction should register one new Realm environment");
    vm.heap.with_obj(realm_env, |object| {
        assert!(
            matches!(object, HeapObj::Environment(_)),
            "the pinned environment cell must not be collected and reused"
        );
    });
    assert_eq!(
        crate::environment::get(&vm.heap, crate::value::GcIdx(realm_env), "globalThis"),
        Some(global.clone()),
        "the surviving Realm environment must retain its global binding"
    );
    let eval = vm
        .get_property(&global, "eval")
        .expect("collected Realm global should expose eval");
    assert_eq!(
        vm.call_function(&eval, &[Value::String("20 + 22".into())], Some(global))
            .expect("the pre-publication-collected Realm should remain functional"),
        Value::Number(42.0)
    );
}

#[test]
fn realm_construction_survives_collection_of_preexisting_garbage() {
    let required_capacity = realm_creation_live_delta();
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    let baseline_pins = vm.gc_pins.len();
    // One extra dead cell makes the final wrapper allocation collect while
    // every provisional Realm root must survive, without exhausting a direct
    // Heap allocation earlier in intrinsic setup.
    let garbage_count = 1;
    for _ in 0..garbage_count {
        vm.new_object()
            .expect("unreachable garbage fixture should allocate");
    }
    let live_with_garbage = vm.heap.live_count();

    vm.set_max_heap_objects(Some(baseline_live + required_capacity));
    let realm = vm
        .run("$262.createRealm();")
        .expect("Realm construction should collect garbage and stay within the exact cap");
    vm.set_max_heap_objects(None);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert!(
        vm.heap.live_count() < live_with_garbage + required_capacity,
        "construction must trigger a collection instead of retaining the garbage fixture"
    );
    let global = vm
        .get_property(&realm, "global")
        .expect("collected construction should return a valid Realm wrapper");
    let eval = vm
        .get_property(&global, "eval")
        .expect("collected Realm should retain its intrinsic eval");
    assert_eq!(
        vm.call_function(&eval, &[Value::String("6 * 7".into())], Some(global))
            .expect("collected Realm should evaluate scripts"),
        Value::Number(42.0)
    );
}

#[test]
fn promise_resolution_heap_error_rejects_instead_of_remaining_pending() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    vm.run(
        r#"
        globalThis.pendingPromise = new Promise(function (resolve) {
          globalThis.resolvePending = resolve;
        });
        globalThis.thenable = {
          get then() {
            capHeap();
            return {};
          }
        };
        "#,
    )
    .expect("Promise fixture should initialize");

    let execution = vm.run("resolvePending(thenable);");
    vm.set_max_heap_objects(None);
    let global_this = vm.global_this.clone();
    let promise = vm
        .get_property(&global_this, "pendingPromise")
        .expect("Promise should remain reachable");
    let (state, reason) = promise_state_and_result(&vm, promise);

    assert!(
        execution.is_ok(),
        "Promise resolution must consume the catchable heap error; got {execution:?}"
    );
    assert!(
        state == PromiseStatus::Rejected,
        "Promise must be rejected instead of remaining pending"
    );
    assert_eq!(
        vm.get_property(&reason, "name")
            .expect("rejection name should be readable"),
        Value::String("RangeError".into())
    );
    assert_eq!(
        vm.get_property(&reason, "message")
            .expect("rejection message should be readable"),
        Value::String("heap limit exceeded".into())
    );
}

#[test]
fn promise_thenable_job_setup_heap_error_rejects_instead_of_remaining_pending() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    vm.run(
        r#"
        globalThis.callableThen = function () {};
        globalThis.pendingPromise = new Promise(function (resolve) {
          globalThis.resolvePending = resolve;
        });
        globalThis.thenable = {
          get then() {
            capHeap();
            return callableThen;
          }
        };
        "#,
    )
    .expect("callable thenable fixture should initialize");

    let execution = vm.run("resolvePending(thenable);");
    vm.set_max_heap_objects(None);
    let global_this = vm.global_this.clone();
    let promise = vm
        .get_property(&global_this, "pendingPromise")
        .expect("Promise should remain reachable");
    let (state, reason) = promise_state_and_result(&vm, promise);

    assert!(
        execution.is_ok(),
        "thenable-job setup must consume the catchable heap error; got {execution:?}"
    );
    assert!(
        state == PromiseStatus::Rejected,
        "Promise must be rejected after thenable-job setup fails"
    );
    assert_eq!(
        vm.get_property(&reason, "name")
            .expect("rejection name should be readable"),
        Value::String("RangeError".into())
    );
    assert_eq!(
        vm.get_property(&reason, "message")
            .expect("rejection message should be readable"),
        Value::String("heap limit exceeded".into())
    );
}

#[test]
fn promise_self_resolution_heap_error_uses_the_emergency_reserve() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    vm.run(
        r#"
        globalThis.pendingPromise = new Promise(function (resolve) {
          globalThis.resolvePending = resolve;
        });
        "#,
    )
    .expect("self-resolution fixture should initialize");

    vm.run("capHeap(); resolvePending(pendingPromise);")
        .expect("self-resolution TypeError should reject at the exact cap");
    let limit = vm.max_heap_objects;
    assert!(vm.heap.live_count() <= limit);
    let global_this = vm.global_this.clone();
    let promise = vm
        .get_property(&global_this, "pendingPromise")
        .expect("Promise should remain reachable");
    let (state, reason) = promise_state_and_result(&vm, promise);
    assert!(state == PromiseStatus::Rejected);
    assert_eq!(
        reason,
        vm.realm_heap_limit_errors
            .get(&vm.global.0)
            .cloned()
            .expect("main Realm reserve should exist")
    );
    vm.set_max_heap_objects(None);
}

#[test]
fn dynamic_import_heap_error_rejects_instead_of_remaining_pending() {
    let dir = std::env::temp_dir().join(format!(
        "ruja-dynamic-import-heap-limit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should follow epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("module fixture directory should be created");
    fs::write(dir.join("target.js"), "export const value = {};")
        .expect("dynamic import target should be written");
    fs::write(
        dir.join("entry.js"),
        "globalThis.importPromise = import('./target.js'); capHeap();",
    )
    .expect("dynamic import entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    let execution = vm.run_file(dir.join("entry.js"));
    vm.set_max_heap_objects(None);
    let global_this = vm.global_this.clone();
    let promise = vm
        .get_property(&global_this, "importPromise")
        .expect("dynamic import Promise should remain reachable");
    let (state, reason) = promise_state_and_result(&vm, promise);

    assert!(
        execution.is_ok(),
        "dynamic import must consume the catchable heap error; got {execution:?}"
    );
    assert!(
        state == PromiseStatus::Rejected,
        "dynamic import Promise must be rejected instead of remaining pending"
    );
    assert_eq!(
        vm.get_property(&reason, "name")
            .expect("rejection name should be readable"),
        Value::String("RangeError".into())
    );
    assert_eq!(
        vm.get_property(&reason, "message")
            .expect("rejection message should be readable"),
        Value::String("heap limit exceeded".into())
    );

    fs::remove_dir_all(dir).expect("module fixture directory should be removed");
}

#[test]
fn dynamic_import_continuation_heap_error_rejects_instead_of_remaining_pending() {
    let dir = std::env::temp_dir().join(format!(
        "ruja-dynamic-import-continuation-heap-limit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should follow epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("module fixture directory should be created");
    fs::write(
        dir.join("target.js"),
        r#"
        globalThis.innerImportPromise = import('./target.js');
        await 0;
        globalThis.targetReachedEnd = true;
        Promise.resolve().then(capHeap);
        export const value = 1;
        "#,
    )
    .expect("self-importing async module should be written");
    fs::write(dir.join("entry.js"), "import './target.js';")
        .expect("static module entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    let execution = vm.run_module_file(dir.join("entry.js"));
    let global_this = vm.global_this.clone();
    let promise = vm
        .get_property(&global_this, "innerImportPromise")
        .expect("inner dynamic import Promise should remain reachable");
    let (state, reason) = promise_state_and_result(&vm, promise);
    assert!(
        execution.is_ok(),
        "dynamic import continuation must consume the heap error; got {execution:?}"
    );
    assert_eq!(
        vm.get_property(&global_this, "targetReachedEnd")
            .expect("module completion marker should be readable"),
        Value::Bool(true)
    );
    assert!(
        state == PromiseStatus::Rejected,
        "continuation failure must reject the inner import Promise"
    );
    assert_eq!(
        vm.get_property(&reason, "message")
            .expect("continuation rejection message should be readable"),
        Value::String(crate::gc::HEAP_LIMIT_MESSAGE.into())
    );
    assert!(vm.max_heap_objects > 0);
    assert!(vm.heap.live_count() <= vm.max_heap_objects);
    vm.set_max_heap_objects(None);

    fs::remove_dir_all(dir).expect("module fixture directory should be removed");
}

#[test]
fn error_materialization_collects_before_using_the_emergency_reserve() {
    let mut vm = Vm::new().expect("VM should initialize");
    let emergency = vm
        .realm_heap_limit_errors
        .get(&vm.global.0)
        .cloned()
        .expect("main Realm should have a heap-limit reserve");
    for _ in 0..128 {
        let _ = vm.new_object().expect("unrooted garbage should allocate");
    }
    let limit = vm.heap.live_count();
    vm.set_max_heap_objects(Some(limit));

    let error = crate::error::Error::range("ordinary range failure");
    let materialized = vm
        .make_error_value(&error)
        .expect("rooted GC should free a cell for a fresh error");

    assert!(
        materialized != emergency,
        "reclaimable garbage must avoid observable emergency identity reuse"
    );
    assert!(vm.heap.live_count() <= limit);
    vm.set_max_heap_objects(None);
    assert_eq!(
        vm.get_property(&materialized, "message")
            .expect("fresh error message should be readable"),
        Value::String("ordinary range failure".into())
    );
}

#[test]
fn saturated_heap_reuses_an_immutable_realm_reserve_without_exceeding_the_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let limit = vm.heap.live_count();
    vm.set_max_heap_objects(Some(limit));
    let error = crate::error::Error::range(crate::gc::HEAP_LIMIT_MESSAGE);
    let first_reason = vm
        .make_error_value(&error)
        .expect("saturated heap should return the emergency reserve");
    let second_reason = vm
        .make_error_value(&error)
        .expect("repeated saturation should reuse the emergency reserve");

    let expected_reserve = vm
        .realm_heap_limit_errors
        .get(&vm.global.0)
        .cloned()
        .expect("main Realm reserve should remain rooted");
    assert_eq!(first_reason, expected_reserve);
    assert_eq!(second_reason, expected_reserve);
    assert!(vm.heap.live_count() <= limit);
    let expected_proto = vm
        .realm_error_prototypes
        .get(&(vm.global.0, "RangeError".into()))
        .cloned()
        .expect("RangeError prototype should remain rooted");
    vm.heap.with_obj(
        match first_reason {
            Value::Object(index) => index.0,
            _ => panic!("heap-limit reserve should be an object"),
        },
        |object| {
            let HeapObj::Object(data) = object else {
                panic!("heap-limit reserve should be ordinary Error data");
            };
            assert!(!object.is_extensible());
            assert_eq!(*data.proto.lock(), Some(expected_proto));
            let properties = data.props.lock();
            for name in ["name", "message", "stack"] {
                let descriptor = properties
                    .get(&crate::value::PropertyKey::from(name))
                    .expect("reserve property should exist");
                assert!(!descriptor.writable);
                assert!(descriptor.enumerable);
                assert!(!descriptor.configurable);
            }
        },
    );
    vm.set_max_heap_objects(None);
}

#[test]
fn heap_limit_reserve_uses_the_failing_promise_realm() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    vm.run(
        r#"
        globalThis.other = $262.createRealm().global;
        other.capHeap = capHeap;
        other.eval(`
          globalThis.pendingPromise = new Promise(function (resolve) {
            globalThis.resolvePending = resolve;
          });
          globalThis.thenable = {
            get then() { capHeap(); return {}; }
          };
        `);
        "#,
    )
    .expect("foreign Realm fixture should initialize");

    vm.run("other.resolvePending(other.thenable);")
        .expect("foreign Promise should consume the heap error");
    let limit = vm.max_heap_objects;
    assert!(limit > 0);
    assert!(vm.heap.live_count() <= limit);
    let global_this = vm.global_this.clone();
    let other = vm
        .get_property(&global_this, "other")
        .expect("foreign global should be readable");
    let promise = vm
        .get_property(&other, "pendingPromise")
        .expect("foreign Promise should be readable");
    let (state, reason) = promise_state_and_result(&vm, promise);
    assert!(state == PromiseStatus::Rejected);
    assert!(
        reason
            != vm
                .realm_heap_limit_errors
                .get(&vm.global.0)
                .cloned()
                .expect("main Realm reserve should exist")
    );

    vm.set_max_heap_objects(None);
    vm.set_property(&other, "heapReason", reason)
        .expect("foreign rejection reason should be exposed for inspection");
    assert_eq!(
        vm.run(
            "other.heapReason instanceof other.RangeError && \
             !(other.heapReason instanceof RangeError) && \
             Object.getPrototypeOf(other.heapReason) === other.RangeError.prototype"
        )
        .expect("foreign reserve Realm should be observable"),
        Value::Bool(true)
    );
}

#[test]
fn explicit_throw_identity_survives_an_exact_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
        .expect("heap-cap hook should register");
    vm.run(
        r#"
        globalThis.reason = { marker: 123 };
        globalThis.pendingPromise = new Promise(function (resolve) {
          globalThis.resolvePending = resolve;
        });
        globalThis.thenable = {
          get then() { capHeap(); throw reason; }
        };
        "#,
    )
    .expect("explicit throw fixture should initialize");

    vm.run("resolvePending(thenable);")
        .expect("explicit thrown value should reject without materialization");
    let limit = vm.max_heap_objects;
    assert!(vm.heap.live_count() <= limit);
    let global_this = vm.global_this.clone();
    let promise = vm
        .get_property(&global_this, "pendingPromise")
        .expect("Promise should be readable");
    let expected = vm
        .get_property(&global_this, "reason")
        .expect("explicit reason should be readable");
    let (state, actual) = promise_state_and_result(&vm, promise);
    assert!(state == PromiseStatus::Rejected);
    assert_eq!(actual, expected);
    vm.set_max_heap_objects(None);
}

#[test]
fn intrinsic_error_prototypes_remain_roots_after_global_replacement() {
    let mut vm = Vm::new().expect("VM should initialize");
    let expected_proto = vm
        .realm_error_prototypes
        .get(&(vm.global.0, "TypeError".into()))
        .cloned()
        .expect("intrinsic TypeError prototype should exist");
    vm.run("TypeError = undefined; globalThis.TypeError = undefined;")
        .expect("TypeError globals should be replaceable");
    vm.gc();

    let error = crate::error::Error::type_err("root check");
    let materialized = vm
        .make_error_value(&error)
        .expect("native TypeError should still materialize");
    let Value::Object(materialized) = materialized else {
        panic!("materialized TypeError should be an object");
    };
    let actual_proto = vm
        .heap
        .with_obj(materialized.0, |object| object.proto().lock().clone());
    assert_eq!(actual_proto, Some(expected_proto));
}

#[test]
fn promise_and_async_fuel_aborts_restore_gc_pin_depth() {
    let cases = [
        "new Promise(function () { while (true) {} });",
        r#"
        globalThis.inner = { get then() { while (true) {} } };
        Promise.resolve().then(function () { return Promise.resolve(inner); });
        "#,
        r#"
        Array.fromAsync({
          0: { get then() { while (true) {} } },
          length: 1
        });
        "#,
        r#"
        var asyncIteratorPrototype = Object.getPrototypeOf(
          (async function* () {}).constructor.prototype.prototype
        );
        asyncIteratorPrototype[Symbol.asyncDispose].call({
          return: function () { while (true) {} }
        });
        "#,
    ];

    for source in cases {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.run("0").expect("baseline script should run");
        let pin_depth = vm.gc_pins.len();
        let frame_depth = vm.frames.len();
        vm.set_fuel(Some(20_000));
        let error = vm
            .run(source)
            .expect_err("Promise/async boundary should propagate fuel exhaustion");
        assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
        assert_eq!(vm.gc_pins.len(), pin_depth, "pin leak after {source}");
        assert_eq!(
            vm.frames.len(),
            frame_depth + 1,
            "async frame leak after {source}"
        );
        vm.set_fuel(None);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn async_await_fuel_aborts_restore_suspended_frame_and_stack() {
    for source in [
        r#"
        (async function () {
          await { get then() { while (true) {} } };
        })();
        "#,
        r#"
        (async function () {
          await 0;
          await { get then() { while (true) {} } };
        })();
        "#,
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.run("0").expect("baseline script should run");
        let pin_depth = vm.gc_pins.len();
        let frame_depth = vm.frames.len();
        let stack_depth = vm.stack.len();

        vm.set_fuel(Some(20_000));
        let error = vm
            .run(source)
            .expect_err("async Await should propagate fuel exhaustion");
        assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
        assert_eq!(vm.gc_pins.len(), pin_depth, "pin leak after {source}");
        assert_eq!(
            vm.frames.len(),
            frame_depth + 1,
            "suspended frame leak after {source}"
        );
        assert_eq!(vm.stack.len(), stack_depth, "stack leak after {source}");
    }
}

#[test]
fn await_rejection_reason_survives_capability_allocation_gc() {
    let mut vm = Vm::new().expect("VM should initialize");
    let reason = Value::Object(vm.new_object().expect("reason object should allocate"));
    vm.set_property(&reason, "marker", Value::Number(123.0))
        .expect("reason marker should be defined");
    let error = crate::error::Error::thrown(reason, &vm.heap);
    for _ in 0..512 {
        let _ = vm.new_object().expect("unrooted garbage should allocate");
    }
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let promise = vm
        .rejected_promise_for_await_error_in_env(&error, vm.global)
        .expect("rejected await Promise should survive forced GC");
    vm.set_max_heap_objects(None);
    let (state, result) = vm.heap.with_obj(promise.0, |object| {
        let HeapObj::Promise(data) = object else {
            panic!("await helper should return a Promise");
        };
        (*data.state.lock(), data.result.lock().clone())
    });
    assert!(state == PromiseStatus::Rejected);
    assert_eq!(
        vm.get_property(&result, "marker")
            .expect("await reason marker should survive GC"),
        Value::Number(123.0)
    );
}

#[test]
fn async_generator_drain_pins_generator_after_job_pop() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        globalThis.generator = (async function* () {
          await 0;
          while (true) {}
        })();
        "#,
    )
    .expect("async generator should be created");
    vm.set_fuel(Some(20_000));
    let error = vm
        .run(
            r#"
            globalThis.firstRequest = generator.next();
            globalThis.secondRequest = generator.next();
            "#,
        )
        .expect_err("resumed generator must propagate fuel exhaustion");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    vm.set_fuel(None);

    let global_this = vm.global_this.clone();
    let second_request = vm
        .get_property(&global_this, "secondRequest")
        .expect("second request should be readable");
    let second_request_pin = vm.pin(&second_request);
    crate::environment::set(&vm.heap, vm.global, "generator", Value::Undefined);
    vm.set_property(&global_this, "generator", Value::Undefined)
        .expect("global generator root should be cleared");
    let generator = match vm
        .microtask_queue
        .pop_front()
        .expect("host abort should schedule a generator drain")
    {
        super::Microtask::AsyncGeneratorDrain { generator } => generator,
        _ => panic!("expected an async-generator drain job"),
    };

    for _ in 0..512 {
        let _ = vm.new_object().expect("unrooted garbage should allocate");
    }
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    crate::builtins::regexp::drain_async_generator_queue(&mut vm, generator)
        .expect("drain should pin the generator across allocation-triggered GC");
    vm.set_max_heap_objects(None);

    let Value::Object(second) = second_request else {
        panic!("second request should remain a Promise");
    };
    let (state, result) = vm.heap.with_obj(second.0, |object| {
        let HeapObj::Promise(data) = object else {
            panic!("second request should remain a Promise");
        };
        (*data.state.lock(), data.result.lock().clone())
    });
    assert!(state == PromiseStatus::Fulfilled);
    assert_eq!(
        vm.get_property(&result, "done")
            .expect("drained iterator result should be readable"),
        Value::Bool(true)
    );
    vm.unpin(second_request_pin);
}

#[test]
fn async_generator_drain_reschedules_after_catchable_heap_error() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        globalThis.generator = (async function* () {
          await 0;
          while (true) {}
        })();
        "#,
    )
    .expect("async generator should be created");
    vm.set_fuel(Some(20_000));
    let error = vm
        .run(
            r#"
            globalThis.firstRequest = generator.next();
            globalThis.secondRequest = generator.next();
            "#,
        )
        .expect_err("resumed generator must propagate fuel exhaustion");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    vm.set_fuel(None);

    let generator = match vm
        .microtask_queue
        .pop_front()
        .expect("host abort should schedule a generator drain")
    {
        super::Microtask::AsyncGeneratorDrain { generator } => generator,
        _ => panic!("expected an async-generator drain job"),
    };
    let pin_depth = vm.gc_pins.len();
    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let error = crate::builtins::regexp::drain_async_generator_queue(&mut vm, generator)
        .expect_err("iterator-result allocation should hit the hard heap limit");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), pin_depth);

    let (processing, queue_len) = vm.heap.with_obj(generator.0, |object| {
        let HeapObj::LazyGenerator(data) = object else {
            panic!("generator should remain allocated");
        };
        (
            data.async_processing
                .load(std::sync::atomic::Ordering::Acquire),
            data.async_queue.lock().len(),
        )
    });
    assert!(
        !processing,
        "catchable failure must release queue ownership"
    );
    assert_eq!(queue_len, 1, "the active request must remain queued");
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(super::Microtask::AsyncGeneratorDrain {
            generator: scheduled
        }) if *scheduled == generator
    ));

    vm.set_max_heap_objects(None);
    vm.run_microtasks()
        .expect("rescheduled drain should settle the queued request");
    let global_this = vm.global_this.clone();
    let second_request = vm
        .get_property(&global_this, "secondRequest")
        .expect("second request should be readable");
    let Value::Object(second) = second_request else {
        panic!("second request should remain a Promise");
    };
    let (state, result) = vm.heap.with_obj(second.0, |object| {
        let HeapObj::Promise(data) = object else {
            panic!("second request should remain a Promise");
        };
        (*data.state.lock(), data.result.lock().clone())
    });
    assert!(state == PromiseStatus::Fulfilled);
    assert_eq!(
        vm.get_property(&result, "done")
            .expect("drained iterator result should be readable"),
        Value::Bool(true)
    );
}

#[test]
fn async_generator_reaction_does_not_replay_after_catchable_heap_error() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        globalThis.generator = (async function* () {
          yield 1;
        })();
        "#,
    )
    .expect("async generator should be created");
    let global_this = vm.global_this.clone();
    let generator = vm
        .get_property(&global_this, "generator")
        .expect("generator should be readable");
    let first_request =
        crate::builtins::regexp::async_generator_next(&mut vm, &[], Some(generator.clone()))
            .expect("first request should suspend at AsyncGeneratorYield");
    let second_request =
        crate::builtins::regexp::async_generator_next(&mut vm, &[], Some(generator))
            .expect("second request should queue behind the first");
    let request_pins = vm.pin_many(&[first_request.clone(), second_request.clone()]);

    vm.gc();
    vm.set_max_heap_objects(Some(1));
    let error = vm
        .run_microtasks()
        .expect_err("yield result allocation should hit the hard heap limit");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    vm.set_max_heap_objects(None);
    vm.run_microtasks()
        .expect("the queued sibling should drain after the host error");

    let Value::Object(first) = first_request else {
        panic!("first request should be a Promise");
    };
    let Value::Object(second) = second_request else {
        panic!("second request should be a Promise");
    };
    let first_state = vm.heap.with_obj(first.0, |object| {
        let HeapObj::Promise(data) = object else {
            panic!("first request should remain a Promise");
        };
        *data.state.lock()
    });
    assert!(
        first_state == PromiseStatus::Pending,
        "a state-advanced request must not be replayed after a host error"
    );
    let (second_state, second_result) = vm.heap.with_obj(second.0, |object| {
        let HeapObj::Promise(data) = object else {
            panic!("second request should remain a Promise");
        };
        (*data.state.lock(), data.result.lock().clone())
    });
    assert!(second_state == PromiseStatus::Fulfilled);
    assert_eq!(
        vm.get_property(&second_result, "done")
            .expect("drained sibling result should be readable"),
        Value::Bool(true)
    );
    vm.unpin_many(request_pins);
}
