use super::{ExternalPromiseJob, GetPrototypeReservationSite, Microtask, Vm};
use crate::value::{FunctionData, FunctionKind, HeapObj, NativeConstructMode, PromiseStatus};
use crate::Value;
use std::fs;
use std::sync::Arc;

fn cap_heap_at_current_live_count(vm: &mut Vm) -> crate::error::Result<Value> {
    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    Ok(Value::Undefined)
}

fn call_iterator_next_result(vm: &mut Vm, iterator: &Value) -> crate::error::Result<Value> {
    let mut pin_count = vm.pin(iterator);
    let result = (|| {
        let next = vm.get_property(iterator, "next")?;
        pin_count += vm.pin(&next);
        vm.call_function(&next, &[], Some(iterator.clone()))
    })();
    vm.unpin_many(pin_count);
    result
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

fn promise_state_and_handler_count(vm: &Vm, value: &Value) -> (PromiseStatus, usize) {
    let Value::Object(promise) = value else {
        panic!("expected a Promise object");
    };
    vm.heap.with_obj(promise.0, |object| {
        let HeapObj::Promise(data) = object else {
            panic!("expected a Promise heap object");
        };
        (*data.state.lock(), data.handlers.lock().len())
    })
}

fn increment_global_counter(vm: &mut Vm, name: &str) -> crate::error::Result<()> {
    let counter = vm.get_global(name);
    let count = match vm.get_property(&counter, "count")? {
        Value::Number(count) => count,
        _ => 0.0,
    };
    vm.set_property(&counter, "count", Value::Number(count + 1.0))
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

fn direct_bound_function(vm: &mut Vm, target: &Value) -> Value {
    direct_bound_function_with_this(vm, target, Value::Undefined)
}

fn direct_bound_function_with_this(vm: &mut Vm, target: &Value, this_value: Value) -> Value {
    let Value::Object(target_index) = target else {
        panic!("direct Bound target must be an object");
    };
    let constructable = vm.is_constructor_value(target);
    let (closure, prototype) = vm.heap.with_obj(target_index.0, |object| {
        let HeapObj::Function(function) = object else {
            panic!("direct Bound target must be a function");
        };
        (function.closure, function.proto.lock().clone())
    });
    let function = HeapObj::Function(FunctionData {
        name: Some(Arc::from("bound direct")),
        kind: FunctionKind::Bound {
            target: *target_index,
            this_val: this_value.clone(),
            bound_args: Vec::new(),
            constructable,
        },
        closure,
        lexical_new_target: Value::Undefined,
        home_object: parking_lot::Mutex::new(None),
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: parking_lot::Mutex::new(None),
        proto: parking_lot::Mutex::new(prototype),
        props: parking_lot::Mutex::new(indexmap::IndexMap::new()),
        extensible: std::sync::atomic::AtomicBool::new(true),
        private_fields: parking_lot::Mutex::new(std::collections::HashMap::new()),
    });
    vm.try_reserve_gc_pins(Vm::value_root_count(target) + Vm::value_root_count(&this_value))
        .expect("direct Bound roots should reserve");
    let target_pin = vm.pin_many(&[target.clone(), this_value]);
    let result = vm
        .alloc(function)
        .expect("direct Bound function should allocate");
    vm.unpin(target_pin);
    Value::Object(result)
}

fn set_direct_has_instance(vm: &Vm, function: &Value, handler: Value) {
    let Value::Object(function) = function else {
        panic!("direct hasInstance target must be an object");
    };
    vm.heap.with_obj(function.0, |object| {
        let HeapObj::Function(function) = object else {
            panic!("direct hasInstance target must be a function");
        };
        function.props.lock().insert(
            crate::value::PropertyKey::Symbol(vm.well_known_symbols.has_instance),
            crate::value::PropertyDescriptor::data(handler),
        );
    });
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

fn realm_registry_counts(vm: &Vm) -> [usize; 34] {
    [
        vm.realm_globals.len(),
        vm.realm_object_prototypes.len(),
        vm.realm_object_prototype_ids.len(),
        vm.realm_array_constructors.len(),
        vm.realm_array_prototypes.len(),
        vm.realm_array_values_functions.len(),
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
    baseline_registries: [usize; 34],
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
        var expected = {};
        Array.prototype.forEach.call({ 0: expected, 1: {}, length: 2 }, function(value) {
          forceGc();
          if (value === this.expected) throw error;
        }, { expected: expected });
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
fn array_slice_and_with_roots_survive_observable_gc() {
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

    let result = vm.run(
        r#"
        function sourceWithCollectingPrototype(marker) {
          var source = [0];
          source.length = 2;
          Object.setPrototypeOf(source, new Proxy(Array.prototype, {
            has: function(target, key) {
              if (key === "1") { forceGc(); return true; }
              return Reflect.has(target, key);
            },
            get: function(target, key, receiver) {
              if (key === "1") { forceGc(); return { marker: marker }; }
              return Reflect.get(target, key, receiver);
            }
          }));
          return source;
        }

        var sliced = sourceWithCollectingPrototype(41).slice();
        var replacement = { marker: 52 };
        var index = { valueOf: function() { forceGc(); return 0; } };
        var copied = sourceWithCollectingPrototype(63).with(index, replacement);
        [sliced[1].marker, copied[0].marker, copied[1].marker].join(":");
        "#,
    );

    assert_eq!(
        result.expect("copy results and values should survive observable GC"),
        Value::String(Arc::from("41:52:63"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn array_push_pop_and_splice_roots_survive_observable_gc() {
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

    let result = vm.run(
        r#"
        var pushedValue = { marker: 11 };
        var pushTarget = new Proxy({ length: 0 }, {
          set: function(target, key, value, receiver) {
            forceGc();
            return Reflect.set(target, key, value, receiver);
          }
        });
        Array.prototype.push.call(pushTarget, pushedValue);

        var popTarget = new Proxy({ 0: { marker: 22 }, length: 1 }, {
          get: function(target, key, receiver) {
            forceGc();
            return Reflect.get(target, key, receiver);
          },
          deleteProperty: function(target, key) {
            forceGc();
            return Reflect.deleteProperty(target, key);
          },
          set: function(target, key, value, receiver) {
            forceGc();
            return Reflect.set(target, key, value, receiver);
          }
        });
        var poppedValue = Array.prototype.pop.call(popTarget);

        var spliceSource = [{ marker: 31 }, { marker: 32 }, { marker: 33 }];
        spliceSource.constructor = {
          get [Symbol.species]() {
            forceGc();
            return function Species(length) {
              forceGc();
              return new Proxy({}, {
                defineProperty: function(target, key, descriptor) {
                  forceGc();
                  return Reflect.defineProperty(target, key, descriptor);
                },
                set: function(target, key, value, receiver) {
                  forceGc();
                  return Reflect.set(target, key, value, receiver);
                }
              });
            };
          }
        };
        var spliceProxy = new Proxy(spliceSource, {
          has: function(target, key) { forceGc(); return Reflect.has(target, key); },
          get: function(target, key, receiver) {
            forceGc();
            return Reflect.get(target, key, receiver);
          },
          set: function(target, key, value, receiver) {
            forceGc();
            return Reflect.set(target, key, value, receiver);
          },
          deleteProperty: function(target, key) {
            forceGc();
            return Reflect.deleteProperty(target, key);
          }
        });
        var insertedValue = { marker: 44 };
        var removed = Array.prototype.splice.call(spliceProxy, 1, 1, insertedValue);

        [
          pushTarget[0].marker, pushedValue.marker,
          poppedValue.marker, popTarget.length,
          removed[0].marker, spliceSource[1].marker, insertedValue.marker
        ].join(":");
        "#,
    );

    assert_eq!(
        result.expect("mutator roots should survive every observable collection"),
        Value::String(Arc::from("11:11:22:0:32:44:44"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn array_concat_roots_survive_every_observable_gc_boundary() {
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

    let result = vm.run(
        r#"
        var first = { marker: 11 };
        var third = { marker: 33 };
        var target = [first, , third];
        target.constructor = {
          get [Symbol.species]() {
            forceGc();
            return function Species() {
              forceGc();
              return new Proxy({ length: 0 }, {
                defineProperty: function(target, key, descriptor) {
                  forceGc();
                  return Reflect.defineProperty(target, key, descriptor);
                },
                set: function(target, key, value) {
                  forceGc();
                  target[key] = value;
                  return true;
                }
              });
            };
          }
        };
        target[Symbol.isConcatSpreadable] = true;
        var source = new Proxy(target, {
          get: function(target, key, receiver) {
            forceGc();
            return Reflect.get(target, key, receiver);
          },
          has: function(target, key) {
            forceGc();
            return Reflect.has(target, key);
          }
        });
        var fourth = { marker: 44 };
        var result = Array.prototype.concat.call(source, fourth);
        [
          result[0].marker, Object.hasOwn(result, "1"),
          result[2].marker, result[3].marker, result.length
        ].join(":");
        "#,
    );

    assert_eq!(
        result.expect("concat roots should survive observable GC"),
        Value::String(Arc::from("11:false:33:44:4"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn array_concat_restores_gc_pin_depth_after_abrupt_observable_steps() {
    for source in [
        r#"
        var error = {};
        var source = [1];
        source.constructor = {
          get [Symbol.species]() { forceGc(); throw error; }
        };
        source.concat();
        "#,
        r#"
        var error = {};
        var source = [1];
        Object.defineProperty(source, Symbol.isConcatSpreadable, {
          get: function() { forceGc(); throw error; }
        });
        source.concat();
        "#,
        r#"
        var error = {};
        var source = new Proxy([1], {
          has: function() { forceGc(); throw error; }
        });
        Array.prototype.concat.call(source);
        "#,
        r#"
        var error = {};
        function Species() {
          return new Proxy({}, {
            defineProperty: function() { forceGc(); throw error; }
          });
        }
        var source = [1];
        source.constructor = { [Symbol.species]: Species };
        source.concat();
        "#,
        r#"
        var error = {};
        function Species() {
          return new Proxy({}, {
            set: function() { forceGc(); throw error; }
          });
        }
        var source = [];
        source.constructor = { [Symbol.species]: Species };
        source.concat();
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
            .expect_err("the observable concat step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_slice_and_with_restore_gc_pin_depth_after_abrupt_gets() {
    for expression in ["source.slice();", "source.with(0, replacement);"] {
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
        vm.run(
            r#"
            globalThis.error = {};
            globalThis.replacement = {};
            globalThis.source = [0];
            source.length = 2;
            Object.setPrototypeOf(source, new Proxy(Array.prototype, {
              has: function(target, key) {
                if (key === "1") { forceGc(); throw error; }
                return Reflect.has(target, key);
              },
              get: function(target, key, receiver) {
                if (key === "1") { forceGc(); throw error; }
                return Reflect.get(target, key, receiver);
              }
            }));
            "#,
        )
        .expect("abrupt-copy fixture should initialize");
        let baseline = vm.gc_pins.len();

        vm.run(expression)
            .expect_err("the indexed lookup should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline, "pin leak after {expression}");
        assert_eq!(
            vm.run("source[0] === 0 && replacement === replacement")
                .expect("VM should remain reusable"),
            Value::Bool(true)
        );
    }
}

#[test]
fn array_copy_results_preserve_sparse_slice_and_cap_materializing_with() {
    let mut vm = Vm::new().expect("VM should initialize");
    let length = crate::value::MAX_DENSE_ARRAY_LEN + 1;
    let source = crate::builtins::array::array_create_in_current_realm(&mut vm, length)
        .expect("a sparse source should allocate without dense backing");
    let source_pin = vm.pin(&source);

    let Value::Object(source_idx) = source.clone() else {
        panic!("Array creation should return an object");
    };
    vm.heap.with_obj(source_idx.0, |object| {
        let HeapObj::Array(array) = object else {
            panic!("Array creation should allocate ArrayData");
        };
        assert!(array.items.lock().is_empty());
        assert!(array.present.lock().is_empty());
        assert_eq!(*array.sparse_max.lock(), Some(length));
        assert!(array
            .props
            .lock()
            .contains_key(&crate::value::PropertyKey::from("length")));
    });
    assert_eq!(
        vm.get_property(&source, "length")
            .expect("sparse source length should be readable"),
        Value::Number(length as f64)
    );

    let tail = crate::builtins::array::array_slice(
        &mut vm,
        &[
            Value::Number((length - 1) as f64),
            Value::Number(length as f64),
        ],
        Some(source.clone()),
    )
    .expect("slicing a bounded sparse tail should succeed");
    assert_eq!(
        vm.get_property(&tail, "length")
            .expect("sparse tail length should be readable"),
        Value::Number(1.0)
    );
    assert!(!vm.has_own_property(&tail, "0"));

    let baseline_pins = vm.gc_pins.len();
    let copied = crate::builtins::array::array_slice(&mut vm, &[], Some(source.clone()))
        .expect("Slice should preserve a large sparse result without dense backing");
    assert_eq!(
        vm.get_property(&copied, "length")
            .expect("sparse copy length should be readable"),
        Value::Number(length as f64)
    );
    let Value::Object(copied_idx) = copied else {
        panic!("Slice should return an object");
    };
    vm.heap.with_obj(copied_idx.0, |object| {
        let HeapObj::Array(array) = object else {
            panic!("default Slice should allocate ArrayData");
        };
        assert!(array.items.lock().is_empty());
        assert!(array.present.lock().is_empty());
        assert_eq!(*array.sparse_max.lock(), Some(length));
    });
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let concatenated = crate::builtins::array_concat(&mut vm, &[], Some(source.clone()))
        .expect("Concat should preserve a large sparse result without dense backing");
    assert_eq!(
        vm.get_property(&concatenated, "length")
            .expect("sparse concat length should be readable"),
        Value::Number(length as f64)
    );
    let Value::Object(concatenated_idx) = concatenated else {
        panic!("Concat should return an object");
    };
    vm.heap.with_obj(concatenated_idx.0, |object| {
        let HeapObj::Array(array) = object else {
            panic!("default Concat should allocate ArrayData");
        };
        assert!(array.items.lock().is_empty());
        assert!(array.present.lock().is_empty());
        assert_eq!(*array.sparse_max.lock(), Some(length));
    });
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let error = crate::builtins::array::array_with(
        &mut vm,
        &[Value::Number(0.0), Value::Number(1.0)],
        Some(source.clone()),
    )
    .expect_err("With must reject materialization above the dense cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(error.message, "Array.with result too large");
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    vm.unpin(source_pin);
}

#[test]
fn array_is_array_proxy_walk_consumes_exact_fuel_and_restores_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    let value = vm
        .run(
            r#"
            var value = [];
            for (var i = 0; i < 100; i++) value = new Proxy(value, {});
            value;
            "#,
        )
        .expect("deep Proxy array should initialize");
    let value_pin = vm.pin(&value);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(99));
    let error = crate::builtins::array_is_array(&mut vm, std::slice::from_ref(&value), None)
        .expect_err("N-1 fuel must abort the Proxy walk");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(100));
    assert_eq!(
        crate::builtins::array_is_array(&mut vm, &[value], None)
            .expect("exact fuel should reach the target array"),
        Value::Bool(true)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(value_pin);
}

#[test]
fn array_set_length_precharges_dense_work_before_mutation() {
    let mut vm = Vm::new().expect("VM should initialize");
    let array = crate::builtins::array::array_create_in_current_realm(&mut vm, 0)
        .expect("empty Array should allocate");
    let array_pin = vm.pin(&array);
    let Value::Object(array_idx) = array.clone() else {
        panic!("ArrayCreate should return an object");
    };
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(99));
    let error = vm
        .set_array_length(array_idx.0, Value::Number(100.0))
        .expect_err("N-1 fuel must abort before resizing dense storage");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.get_property(&array, "length")
            .expect("failed length update should leave the old length"),
        Value::Number(0.0)
    );

    vm.set_fuel(Some(100));
    vm.set_array_length(array_idx.0, Value::Number(100.0))
        .expect("exact fuel should complete the resize");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.get_property(&array, "length")
            .expect("completed length update should be visible"),
        Value::Number(100.0)
    );
    vm.set_fuel(None);
    vm.unpin(array_pin);
}

#[test]
fn generic_array_method_loops_restore_pin_depth_after_fuel_abort() {
    for expression in [
        "Array.prototype.concat.call(source);",
        "Array.prototype.slice.call(source);",
        "Array.prototype.flat.call(source);",
        "Array.prototype.flatMap.call(source, function(value) { return [value]; });",
        "Array.prototype.forEach.call(source, function() {});",
        "Array.prototype.with.call(source, 0, replacement);",
        "Array.prototype.splice.call(source, 0, 0, replacement);",
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.run(
            r#"
            globalThis.replacement = { marker: 1 };
            globalThis.source = { length: 1000, 0: replacement };
            source[Symbol.isConcatSpreadable] = true;
            "#,
        )
        .expect("fuel fixture should initialize");
        let baseline = vm.gc_pins.len();

        vm.set_fuel(Some(50));
        let error = vm
            .run(expression)
            .expect_err("the native indexed-property loop should exhaust fuel");
        assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
        assert_eq!(vm.gc_pins.len(), baseline, "pin leak after {expression}");

        vm.set_fuel(None);
        assert_eq!(
            vm.run("source[0] === replacement")
                .expect("VM should remain reusable after fuel abort"),
            Value::Bool(true)
        );
    }

    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("globalThis.source = { length: 0 }; source;")
        .expect("generic push receiver should initialize");
    let source_pin = vm.pin(&source);
    let args = vec![Value::Number(1.0); 100];
    let baseline = vm.gc_pins.len();
    vm.set_fuel(Some(50));
    let error = crate::builtins::array_push(&mut vm, &args, Some(source.clone()))
        .expect_err("push should charge each inserted property");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.get_property(&source, "0")
            .expect("partially pushed receiver should remain valid"),
        Value::Number(1.0)
    );
    vm.unpin(source_pin);
}

#[test]
fn array_flat_consumes_exact_fuel_across_nested_indices() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("({ 0: [1, 2], length: 1 })")
        .expect("flat source should initialize");
    let source_pin = vm.pin(&source);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(2));
    let error = crate::builtins::array_flat(&mut vm, &[], Some(source.clone()))
        .expect_err("three visited indices must require three fuel units");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(3));
    let result = crate::builtins::array_flat(&mut vm, &[], Some(source.clone()))
        .expect("exact nested-index fuel should complete flat");
    let result_pin = vm.pin(&result);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(
        vm.get_property(&result, "0")
            .expect("first flattened value should be present"),
        Value::Number(1.0)
    );
    assert_eq!(
        vm.get_property(&result, "1")
            .expect("second flattened value should be present"),
        Value::Number(2.0)
    );
    vm.unpin(result_pin);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(source_pin);
}

#[test]
fn array_copy_within_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let args = [Value::Number(1.0), Value::Number(0.0), Value::Number(3.0)];

    let partial = vm
        .run("({ 0: 'a', 1: 'b', 2: 'c', length: 3 })")
        .expect("partial-copy receiver should initialize");
    let partial_pin = vm.pin(&partial);
    let baseline = vm.gc_pins.len();
    vm.set_fuel(Some(1));
    let error = crate::builtins::array_copy_within(&mut vm, &args, Some(partial.clone()))
        .expect_err("N-1 fuel must abort the overlapping copy");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.get_property(&partial, "0")
            .expect("first partial value should remain readable"),
        Value::String(Arc::from("a"))
    );
    assert_eq!(
        vm.get_property(&partial, "1")
            .expect("second partial value should remain readable"),
        Value::String(Arc::from("b"))
    );
    assert_eq!(
        vm.get_property(&partial, "2")
            .expect("the first backward iteration should be visible"),
        Value::String(Arc::from("b"))
    );
    vm.unpin(partial_pin);

    let complete = vm
        .run("({ 0: 'a', 1: 'b', 2: 'c', length: 3 })")
        .expect("complete-copy receiver should initialize");
    let complete_pin = vm.pin(&complete);
    let baseline = vm.gc_pins.len();
    vm.set_fuel(Some(2));
    let returned = crate::builtins::array_copy_within(&mut vm, &args, Some(complete.clone()))
        .expect("exact fuel should complete the overlapping copy");
    assert_eq!(returned, complete);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.get_property(&complete, "1")
            .expect("completed target value should be readable"),
        Value::String(Arc::from("a"))
    );
    assert_eq!(
        vm.get_property(&complete, "2")
            .expect("completed trailing value should be readable"),
        Value::String(Arc::from("b"))
    );

    vm.set_fuel(Some(0));
    crate::builtins::array_copy_within(
        &mut vm,
        &[Value::Number(0.0), Value::Number(0.0), Value::Number(0.0)],
        Some(complete),
    )
    .expect("an empty copy should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(complete_pin);
}

#[test]
fn array_copy_within_roots_values_and_restores_pin_depth() {
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
    let result = vm.run(
        r#"
        (function () {
          var target = { 0: { marker: 41 }, length: 1 };
          var proxy = new Proxy(target, {
            has: function (target, key) {
              forceGc();
              return Reflect.has(target, key);
            },
            get: function (target, key, receiver) {
              var value = Reflect.get(target, key, receiver);
              if (key === "0") delete target[0];
              forceGc();
              return value;
            },
            set: function (target, key, value, receiver) {
              forceGc();
              return Reflect.set(target, key, value, receiver);
            }
          });
          var returned = Array.prototype.copyWithin.call(
            proxy,
            { valueOf: function () { forceGc(); return 0; } },
            { valueOf: function () { forceGc(); return 0; } },
            { valueOf: function () { forceGc(); return 1; } }
          );
          return [returned === proxy, target[0].marker].join(":");
        })();
        "#,
    );
    assert_eq!(
        result.expect("the fetched value should survive the collecting setter"),
        Value::String(Arc::from("true:41"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        r#"
        var error = {};
        var source = new Proxy({ 0: 1, length: 1 }, {
          has: function () { forceGc(); throw error; }
        });
        Array.prototype.copyWithin.call(source, 0, 0, 1);
        "#,
        r#"
        var error = {};
        var source = new Proxy({ 0: 1, length: 1 }, {
          get: function (target, key, receiver) {
            if (key === "0") { forceGc(); throw error; }
            return Reflect.get(target, key, receiver);
          }
        });
        Array.prototype.copyWithin.call(source, 0, 0, 1);
        "#,
        r#"
        var error = {};
        var source = new Proxy({ 0: 1, length: 1 }, {
          set: function () { forceGc(); throw error; }
        });
        Array.prototype.copyWithin.call(source, 0, 0, 1);
        "#,
        r#"
        var error = {};
        var source = new Proxy({ 0: 1, length: 2 }, {
          deleteProperty: function () { forceGc(); throw error; }
        });
        Array.prototype.copyWithin.call(source, 0, 1, 2);
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
            .expect_err("the observable copy step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }

    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let _garbage = vm
        .new_object()
        .expect("collectible garbage should allocate");
    let baseline = vm.gc_pins.len();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let boxed = crate::builtins::array_copy_within(&mut vm, &[], Some(Value::Bool(true)))
        .expect("primitive boxing should retry after collecting garbage");
    vm.set_max_heap_objects(None);
    let Value::Object(boxed_idx) = boxed else {
        panic!("copyWithin should return the boxed Boolean receiver");
    };
    vm.heap.with_obj(boxed_idx.0, |object| {
        let HeapObj::Object(data) = object else {
            panic!("Boolean boxing should allocate ordinary object data");
        };
        assert_eq!(*data.primitive.lock(), Some(Value::Bool(true)));
    });
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn array_fill_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let args = [Value::String(Arc::from("filled"))];
    let target = vm
        .run("({ 0: null, 1: null, 2: null, length: 3 })")
        .expect("fill target should initialize");
    let target_pin = vm.pin(&target);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(2));
    let error = crate::builtins::array_fill(&mut vm, &args, Some(target.clone()))
        .expect_err("N-1 fuel must abort the fill loop");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.get_property(&target, "0")
            .expect("first partial fill should remain observable"),
        Value::String(Arc::from("filled"))
    );
    assert_eq!(
        vm.get_property(&target, "1")
            .expect("second partial fill should remain observable"),
        Value::String(Arc::from("filled"))
    );
    assert_eq!(
        vm.get_property(&target, "2")
            .expect("unfilled property should remain readable"),
        Value::Null
    );

    let complete = vm
        .run("({ 0: null, 1: null, 2: null, length: 3 })")
        .expect("complete fill target should initialize");
    let complete_pin = vm.pin(&complete);
    let baseline = vm.gc_pins.len();
    vm.set_fuel(Some(3));
    let returned = crate::builtins::array_fill(&mut vm, &args, Some(complete.clone()))
        .expect("exact fuel should complete fill");
    assert_eq!(returned, complete);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    vm.set_fuel(Some(0));
    crate::builtins::array_fill(
        &mut vm,
        &[Value::Number(1.0), Value::Number(2.0), Value::Number(2.0)],
        Some(complete),
    )
    .expect("an empty fill range should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(complete_pin);
    vm.unpin(target_pin);
}

#[test]
fn array_fill_roots_observable_state_and_restores_pin_depth() {
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
    let result = vm.run(
        r#"
        (function () {
          var value = { marker: 41 };
          var target = { length: 2 };
          var proxy = new Proxy(target, {
            get: function (object, key, receiver) {
              if (key === "length") forceGc();
              return Reflect.get(object, key, receiver);
            },
            set: function (object, key, newValue) {
              forceGc();
              object[key] = newValue;
              return true;
            }
          });
          var returned = Array.prototype.fill.call(
            proxy,
            value,
            { valueOf: function () { forceGc(); return 0; } },
            { valueOf: function () { forceGc(); return 2; } }
          );
          return [returned === proxy, target[0].marker, target[1].marker].join(":");
        })();
        "#,
    );
    assert_eq!(
        result.expect("fill state should survive every collecting callback"),
        Value::String(Arc::from("true:41:41"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        r#"
        var error = {};
        var source = new Proxy({}, {
          get: function () { forceGc(); throw error; }
        });
        Array.prototype.fill.call(source, {}, 0, 1);
        "#,
        r#"
        var error = {};
        Array.prototype.fill.call(
          { length: 1 },
          {},
          { valueOf: function () { forceGc(); throw error; } },
          1
        );
        "#,
        r#"
        var error = {};
        Array.prototype.fill.call(
          { length: 1 },
          {},
          0,
          { valueOf: function () { forceGc(); throw error; } }
        );
        "#,
        r#"
        var error = {};
        var source = new Proxy({ length: 1 }, {
          set: function () { forceGc(); throw error; }
        });
        Array.prototype.fill.call(source, {});
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
            .expect_err("the observable fill step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }

    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let _garbage = vm
        .new_object()
        .expect("collectible garbage should allocate");
    let baseline = vm.gc_pins.len();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let boxed = crate::builtins::array_fill(&mut vm, &[], Some(Value::Bool(true)))
        .expect("primitive boxing should retry after collecting garbage");
    vm.set_max_heap_objects(None);
    let Value::Object(boxed_idx) = boxed else {
        panic!("fill should return the boxed Boolean receiver");
    };
    vm.heap.with_obj(boxed_idx.0, |object| {
        let HeapObj::Object(data) = object else {
            panic!("Boolean boxing should allocate ordinary object data");
        };
        assert_eq!(*data.primitive.lock(), Some(Value::Bool(true)));
    });
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn array_filter_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("keep", |_, _, _| Ok(Value::Bool(true)), 3)
        .expect("native predicate should register");
    let callback = vm.run("keep").expect("predicate should be readable");
    let source = vm
        .run("Object.assign(Object.create(null), { 0: 1, 2: 3, length: 3 })")
        .expect("filter source should initialize");
    let source_pin = vm.pin(&source);
    let callback_pin = vm.pin(&callback);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(2));
    let error = crate::builtins::array_filter(
        &mut vm,
        std::slice::from_ref(&callback),
        Some(source.clone()),
    )
    .expect_err("N-1 fuel must abort the logical filter scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(3));
    let result =
        crate::builtins::array_filter(&mut vm, std::slice::from_ref(&callback), Some(source))
            .expect("exact logical-index fuel should complete filter");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.get_property(&result, "length")
            .expect("filter result length should be readable"),
        Value::Number(2.0)
    );
    assert_eq!(
        vm.get_property(&result, "1")
            .expect("second selected value should be readable"),
        Value::Number(3.0)
    );

    let empty = vm
        .run("({ length: 0 })")
        .expect("empty source should initialize");
    vm.set_fuel(Some(0));
    crate::builtins::array_filter(&mut vm, &[callback], Some(empty))
        .expect("empty filter should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(callback_pin);
    vm.unpin(source_pin);
}

#[test]
fn array_map_consumes_exact_result_and_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("mapValue", |_, args, _| Ok(args[0].clone()), 3)
        .expect("native callback should register");
    let callback = vm.run("mapValue").expect("callback should be readable");
    let source = vm
        .run("Object.assign(Object.create(null), { 0: 1, 2: 3, length: 3 })")
        .expect("map source should initialize");
    let source_pin = vm.pin(&source);
    let callback_pin = vm.pin(&callback);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(5));
    let error = crate::builtins::array_map(
        &mut vm,
        std::slice::from_ref(&callback),
        Some(source.clone()),
    )
    .expect_err("N-1 fuel must abort result creation plus the logical map scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(6));
    let result = crate::builtins::array_map(&mut vm, std::slice::from_ref(&callback), Some(source))
        .expect("exact result-creation and logical-index fuel should complete map");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.get_property(&result, "length")
            .expect("map result length should be readable"),
        Value::Number(3.0)
    );
    assert!(!vm
        .has_property(&result, "1")
        .expect("map result hole should be testable"));

    let empty = vm
        .run("({ length: 0 })")
        .expect("empty source should initialize");
    vm.set_fuel(Some(0));
    crate::builtins::array_map(&mut vm, &[callback], Some(empty))
        .expect("empty map should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(callback_pin);
    vm.unpin(source_pin);
}

#[test]
fn array_map_roots_observable_state_and_restores_pin_depth() {
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

    let result = vm
        .run(
            r#"
            (function () {
              var retained = { marker: 41 };
              var source = [retained];
              var target = {};
              source.constructor = { [Symbol.species]: function () {
                return new Proxy(target, {
                  defineProperty: function (object, key, descriptor) {
                    forceGc();
                    return Reflect.defineProperty(object, key, descriptor);
                  }
                });
              }};
              var mapped = source.map(function (value) {
                delete source[0];
                retained = null;
                forceGc();
                return { marker: value.marker };
              });
              forceGc();
              return mapped[0].marker;
            })();
            "#,
        )
        .expect("map roots should survive callbacks and species Proxy definitions");
    assert_eq!(result, Value::Number(41.0));
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        "Array.prototype.map.call(new Proxy({ length: 1 }, { has: function () { throw 'has'; } }), function () {});",
        "Array.prototype.map.call(new Proxy({ 0: 1, length: 1 }, { get: function (target, key, receiver) { if (key === '0') throw 'get'; return Reflect.get(target, key, receiver); } }), function () {});",
        "[1].map(function () { throw 'callback'; });",
    ] {
        vm.run(source)
            .expect_err("the observable map step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_reduce_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("reduceValue", |_, args, _| Ok(args[0].clone()), 4)
        .expect("native callback should register");
    let callback = vm.run("reduceValue").expect("callback should be readable");
    let source = vm
        .run("Object.assign(Object.create(null), { 1: 2, 3: 4, length: 4 })")
        .expect("reduce source should initialize");
    let source_pin = vm.pin(&source);
    let callback_pin = vm.pin(&callback);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(3));
    let error = crate::builtins::array_reduce(
        &mut vm,
        std::slice::from_ref(&callback),
        Some(source.clone()),
    )
    .expect_err("N-1 fuel must abort accumulator search plus the logical scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(4));
    let result =
        crate::builtins::array_reduce(&mut vm, std::slice::from_ref(&callback), Some(source))
            .expect("exact logical-index fuel should complete reduce");
    assert_eq!(result, Value::Number(2.0));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    let object_initial = vm
        .run("({ marker: 9 })")
        .expect("object accumulator should initialize");
    let fuel_source = vm
        .run("({ 0: 1, length: 1 })")
        .expect("fuel source should initialize");
    vm.set_fuel(Some(0));
    let error = crate::builtins::array_reduce(
        &mut vm,
        &[callback.clone(), object_initial],
        Some(fuel_source),
    )
    .expect_err("fuel exhaustion after rooting an object accumulator must clean up");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    let empty = vm
        .run("({ length: 0 })")
        .expect("empty source should initialize");
    vm.set_fuel(Some(0));
    let result =
        crate::builtins::array_reduce(&mut vm, &[callback, Value::Number(9.0)], Some(empty))
            .expect("empty reduce with an initial value should consume no loop fuel");
    assert_eq!(result, Value::Number(9.0));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(callback_pin);
    vm.unpin(source_pin);
}

#[test]
fn array_reduce_roots_accumulator_and_restores_pin_depth() {
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

    let result = vm
        .run(
            r#"
            (function () {
              var retained = { marker: 40 };
              var target = { 0: 1, 1: 2, length: 2 };
              var source = new Proxy(target, {
                has: function (object, key) {
                  if (key === "1") {
                    retained = null;
                    forceGc();
                  }
                  return Reflect.has(object, key);
                }
              });
              return Array.prototype.reduce.call(
                source,
                function (accumulator, value, index) {
                  if (index === 0) return retained;
                  return accumulator.marker + value;
                },
                { marker: 0 }
              );
            })();
            "#,
        )
        .expect("reduce accumulator should survive later observable property work");
    assert_eq!(result, Value::Number(42.0));
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        "Array.prototype.reduce.call(new Proxy({ length: 1 }, { has: function () { throw 'has'; } }), function () {}, 0);",
        "Array.prototype.reduce.call(new Proxy({ 0: 1, length: 1 }, { get: function (target, key, receiver) { if (key === '0') throw 'get'; return Reflect.get(target, key, receiver); } }), function () {}, 0);",
        "Array.prototype.reduce.call(new Proxy({ 0: 1, 1: 2, length: 2 }, { has: function (target, key) { if (key === '1') { forceGc(); throw 'late-has'; } return Reflect.has(target, key); } }), function () { return { marker: 1 }; }, { marker: 0 });",
        "[1].reduce(function () { forceGc(); throw 'callback'; }, { marker: 0 });",
        "Array.prototype.reduce.call({ length: 1 }, function () {});",
    ] {
        vm.run(source)
            .expect_err("the observable reduce step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_reduce_right_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("reduceRightValue", |_, args, _| Ok(args[0].clone()), 4)
        .expect("native callback should register");
    let callback = vm
        .run("reduceRightValue")
        .expect("callback should be readable");
    let source = vm
        .run("Object.assign(Object.create(null), { 0: 4, 2: 2, length: 4 })")
        .expect("reduceRight source should initialize");
    let source_pin = vm.pin(&source);
    let callback_pin = vm.pin(&callback);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(3));
    let error = crate::builtins::array_reduce_right(
        &mut vm,
        std::slice::from_ref(&callback),
        Some(source.clone()),
    )
    .expect_err("N-1 fuel must abort accumulator search plus the logical scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(4));
    let result =
        crate::builtins::array_reduce_right(&mut vm, std::slice::from_ref(&callback), Some(source))
            .expect("exact logical-index fuel should complete reduceRight");
    assert_eq!(result, Value::Number(2.0));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    let object_initial = vm
        .run("({ marker: 9 })")
        .expect("object accumulator should initialize");
    let fuel_source = vm
        .run("({ 0: 1, length: 1 })")
        .expect("fuel source should initialize");
    vm.set_fuel(Some(0));
    let error = crate::builtins::array_reduce_right(
        &mut vm,
        &[callback.clone(), object_initial],
        Some(fuel_source),
    )
    .expect_err("fuel exhaustion after rooting an object accumulator must clean up");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    let empty = vm
        .run("({ length: 0 })")
        .expect("empty source should initialize");
    vm.set_fuel(Some(0));
    let result =
        crate::builtins::array_reduce_right(&mut vm, &[callback, Value::Number(9.0)], Some(empty))
            .expect("empty reduceRight with an initial value should consume no loop fuel");
    assert_eq!(result, Value::Number(9.0));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(callback_pin);
    vm.unpin(source_pin);
}

#[test]
fn array_reduce_right_roots_accumulator_and_restores_pin_depth() {
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

    let result = vm
        .run(
            r#"
            (function () {
              var retained = { marker: 40 };
              var target = { 0: 2, 1: 1, length: 2 };
              var source = new Proxy(target, {
                has: function (object, key) {
                  if (key === "0") {
                    retained = null;
                    forceGc();
                  }
                  return Reflect.has(object, key);
                }
              });
              return Array.prototype.reduceRight.call(
                source,
                function (accumulator, value, index) {
                  if (index === 1) return retained;
                  return accumulator.marker + value;
                },
                { marker: 0 }
              );
            })();
            "#,
        )
        .expect("reduceRight accumulator should survive later observable property work");
    assert_eq!(result, Value::Number(42.0));
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        "Array.prototype.reduceRight.call(new Proxy({ length: 1 }, { has: function () { throw 'has'; } }), function () {}, 0);",
        "Array.prototype.reduceRight.call(new Proxy({ 0: 1, length: 1 }, { get: function (target, key, receiver) { if (key === '0') throw 'get'; return Reflect.get(target, key, receiver); } }), function () {}, 0);",
        "Array.prototype.reduceRight.call(new Proxy({ 0: 1, 1: 2, length: 2 }, { has: function (target, key) { if (key === '0') { forceGc(); throw 'late-has'; } return Reflect.has(target, key); } }), function () { return { marker: 1 }; }, { marker: 0 });",
        "[1].reduceRight(function () { forceGc(); throw 'callback'; }, { marker: 0 });",
        "Array.prototype.reduceRight.call({ length: 1 }, function () {});",
    ] {
        vm.run(source)
            .expect_err("the observable reduceRight step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_reverse_consumes_exact_per_pair_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("Object.assign(Object.create(null), { 0: 1, 3: 4, length: 4 })")
        .expect("reverse source should initialize");
    let source_pin = vm.pin(&source);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(1));
    let error = crate::builtins::array_reverse(&mut vm, &[], Some(source.clone()))
        .expect_err("N-1 pair fuel must abort reverse");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(None);
    let source = vm
        .run("Object.assign(Object.create(null), { 0: 1, 3: 4, length: 4 })")
        .expect("fresh reverse source should initialize");
    vm.set_fuel(Some(2));
    crate::builtins::array_reverse(&mut vm, &[], Some(source))
        .expect("exact pair fuel should complete reverse");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    for expression in [
        "Object.assign(Object.create(null), { length: 0 })",
        "Object.assign(Object.create(null), { 0: 1, length: 1 })",
    ] {
        vm.set_fuel(None);
        let source = vm.run(expression).expect("short source should initialize");
        vm.set_fuel(Some(0));
        crate::builtins::array_reverse(&mut vm, &[], Some(source))
            .expect("zero-pair reverse should consume no loop fuel");
        assert_eq!(vm.fuel_remaining(), Some(0));
        assert_eq!(vm.gc_pins.len(), baseline);
    }
    vm.set_fuel(None);
    vm.unpin(source_pin);
}

#[test]
fn array_reverse_roots_pair_values_and_restores_pin_depth() {
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

    let result = vm
        .run(
            r#"
            (function () {
              var retained = { marker: 42 };
              var target = { 0: retained, length: 2 };
              var source = new Proxy(target, {
                has: function (object, key) {
                  if (key === "1") {
                    delete object[0];
                    retained = null;
                    forceGc();
                  }
                  return Reflect.has(object, key);
                }
              });
              Array.prototype.reverse.call(source);
              return target[1].marker;
            })();
            "#,
        )
        .expect("lower value should survive the observable upper HasProperty");
    assert_eq!(result, Value::Number(42.0));
    assert_eq!(vm.gc_pins.len(), baseline);

    let result = vm
        .run(
            r#"
            (function () {
              var retained = { marker: 43 };
              var target = { 1: retained, length: 2 };
              var source = new Proxy(target, {
                set: function (object, key, value) {
                  if (key === "0") {
                    delete object[1];
                    retained = null;
                    forceGc();
                  }
                  return Reflect.set(object, key, value);
                }
              });
              Array.prototype.reverse.call(source);
              return target[0].marker;
            })();
            "#,
        )
        .expect("upper value should survive the observable lower Set");
    assert_eq!(result, Value::Number(43.0));
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        "Array.prototype.reverse.call(new Proxy({ length: 2 }, { has: function () { throw 'has'; } }));",
        "Array.prototype.reverse.call(new Proxy({ 0: 1, length: 2 }, { get: function (target, key, receiver) { if (key === '0') throw 'get'; return Reflect.get(target, key, receiver); } }));",
        "Array.prototype.reverse.call(new Proxy({ 0: 1, 1: 2, length: 2 }, { set: function () { throw 'set'; } }));",
        "Array.prototype.reverse.call(new Proxy({ 1: 2, length: 2 }, { deleteProperty: function () { return false; } }));",
        "Array.prototype.reverse.call(null);",
    ] {
        vm.run(source)
            .expect_err("the observable reverse step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_to_reversed_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let baseline = vm.gc_pins.len();

    let source = vm
        .run("({ 0: 1, 1: 2, 2: 3, length: 3 })")
        .expect("toReversed source should initialize");
    vm.set_fuel(Some(5));
    let error = crate::builtins::array_to_reversed(&mut vm, &[], Some(source))
        .expect_err("N-1 total loop and property fuel must abort toReversed");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(None);
    let source = vm
        .run("({ 0: 1, 1: 2, 2: 3, length: 3 })")
        .expect("fresh toReversed source should initialize");
    vm.set_fuel(Some(6));
    crate::builtins::array_to_reversed(&mut vm, &[], Some(source))
        .expect("exact loop and property fuel should complete toReversed");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(None);
    let source = vm
        .run("({ length: 0 })")
        .expect("empty toReversed source should initialize");
    vm.set_fuel(Some(0));
    crate::builtins::array_to_reversed(&mut vm, &[], Some(source))
        .expect("empty toReversed should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn array_to_reversed_roots_results_and_restores_pin_depth() {
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

    let result = vm
        .run(
            r#"
            (function () {
              var retained = { marker: 42 };
              var target = { 1: retained, 0: "lower", length: 2 };
              var source = new Proxy(target, {
                get: function (object, key, receiver) {
                  if (key === "0") {
                    delete object[1];
                    retained = null;
                    forceGc();
                  }
                  return Reflect.get(object, key, receiver);
                }
              });
              return Array.prototype.toReversed.call(source)[0].marker;
            })();
            "#,
        )
        .expect("an earlier result element should survive a later observable Get");
    assert_eq!(result, Value::Number(42.0));
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        "Array.prototype.toReversed.call(new Proxy({ length: 1 }, { get: function (target, key, receiver) { if (key === '0') throw 'get'; return Reflect.get(target, key, receiver); } }));",
        "Array.prototype.toReversed.call(new Proxy({}, { get: function () { throw 'length'; } }));",
        "Array.prototype.toReversed.call({ length: 4294967296 });",
        "Array.prototype.toReversed.call(null);",
    ] {
        vm.run(source)
            .expect_err("the observable toReversed step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_to_reversed_result_allocation_obeys_heap_cap_and_gc_retry() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run(
            r#"
            globalThis.toReversedReads = 0;
            ({ get 0() { toReversedReads++; return 1; }, length: 1 });
            "#,
        )
        .expect("allocation-failure source should initialize");
    let source_pin = vm.pin(&source);
    vm.gc();
    let baseline = vm.gc_pins.len();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let error = crate::builtins::array_to_reversed(&mut vm, &[], Some(source))
        .expect_err("result allocation should respect the exact heap cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_max_heap_objects(None);
    assert_eq!(
        vm.run("toReversedReads")
            .expect("indexed getter count should remain readable"),
        Value::Number(0.0),
        "ArrayCreate must fail before indexed Gets"
    );
    vm.unpin(source_pin);

    let mut vm = Vm::new().expect("retry VM should initialize");
    vm.run(
        r#"
        globalThis.other = $262.createRealm().global;
        globalThis.retrySource = [{ marker: 42 }];
        "#,
    )
    .expect("foreign Realm retry fixture should initialize");
    let method = vm
        .run("other.Array.prototype.toReversed")
        .expect("foreign method should be readable");
    let source = vm
        .run("retrySource")
        .expect("retry source should be readable");
    let expected_proto = vm
        .run("other.Array.prototype")
        .expect("foreign Array prototype should be readable");
    let fixture_pins = vm.pin_many(&[method.clone(), source.clone(), expected_proto.clone()]);
    vm.gc();
    let baseline_live = vm.heap.live_count();
    vm.run(
        r#"
        (function () {
          for (var i = 0; i < 100; i++) ({ index: i });
        })();
        "#,
    )
    .expect("collectible retry garbage should initialize");
    let capped_live = vm.heap.live_count();
    assert!(capped_live > baseline_live, "fixture must leave garbage");
    vm.set_max_heap_objects(Some(capped_live));
    let baseline = vm.gc_pins.len();

    let result = vm
        .call_function(&method, &[], Some(source.clone()))
        .expect("result allocation should collect garbage and retry");
    vm.set_max_heap_objects(None);
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.get_prototype_of(&result)
            .expect("result prototype lookup should succeed"),
        Some(expected_proto)
    );
    assert_eq!(
        vm.get_property(&result, "0")
            .and_then(|value| vm.get_property(&value, "marker"))
            .expect("copied source value should survive allocation retry"),
        Value::Number(42.0)
    );
    assert_eq!(
        vm.get_property(&source, "0")
            .and_then(|value| vm.get_property(&value, "marker"))
            .expect("source should survive allocation retry"),
        Value::Number(42.0)
    );
    vm.unpin_many(fixture_pins);
}

#[test]
fn array_to_spliced_consumes_exact_per_result_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let baseline = vm.gc_pins.len();

    let source = vm
        .run("({ 0: 1, 1: 2, 2: 3, length: 3 })")
        .expect("toSpliced source should initialize");
    vm.set_fuel(Some(7));
    let error = crate::builtins::array_to_spliced(
        &mut vm,
        &[
            Value::Number(1.0),
            Value::Number(1.0),
            Value::Number(9.0),
            Value::Number(8.0),
        ],
        Some(source),
    )
    .expect_err("N-1 total loop and property fuel must abort toSpliced");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(None);
    let source = vm
        .run("({ 0: 1, 1: 2, 2: 3, length: 3 })")
        .expect("fresh toSpliced source should initialize");
    vm.set_fuel(Some(8));
    crate::builtins::array_to_spliced(
        &mut vm,
        &[
            Value::Number(1.0),
            Value::Number(1.0),
            Value::Number(9.0),
            Value::Number(8.0),
        ],
        Some(source),
    )
    .expect("exact loop and property fuel should complete toSpliced");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(None);
    let source = vm
        .run("({ length: 0 })")
        .expect("empty toSpliced source should initialize");
    vm.set_fuel(Some(0));
    crate::builtins::array_to_spliced(&mut vm, &[], Some(source))
        .expect("empty toSpliced should consume no result-index fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn array_to_spliced_roots_results_and_restores_pin_depth() {
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

    let result = vm
        .run(
            r#"
            (function () {
              var retained = { marker: 42 };
              var inserted = { marker: 43 };
              var target = { 0: retained, 1: "discard", 2: "tail", length: 3 };
              var source = new Proxy(target, {
                get: function (object, key, receiver) {
                  if (key === "2") {
                    delete object[0];
                    retained = null;
                    forceGc();
                  }
                  return Reflect.get(object, key, receiver);
                }
              });
              var copy = Array.prototype.toSpliced.call(source, 1, 1, inserted);
              return copy[0].marker + copy[1].marker;
            })();
            "#,
        )
        .expect("prior result and inserted argument should survive later observable Gets");
    assert_eq!(result, Value::Number(85.0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.run(
        r#"
        globalThis.coercionSource = { 0: { marker: 40 }, length: 1 };
        globalThis.coercionInserted = { marker: 2 };
        globalThis.coercionStart = {
          valueOf: function () {
            coercionSource = null;
            coercionInserted = null;
            forceGc();
            return 1;
          }
        };
        "#,
    )
    .expect("coercion rooting fixture should initialize");
    let source = vm
        .run("coercionSource")
        .expect("coercion source should be readable");
    let start = vm
        .run("coercionStart")
        .expect("coercion start should be readable");
    let inserted = vm
        .run("coercionInserted")
        .expect("coercion insertion should be readable");
    let copy = crate::builtins::array_to_spliced(
        &mut vm,
        &[start, Value::Number(0.0), inserted],
        Some(source),
    )
    .expect("source and insertion should survive start coercion GC");
    assert_eq!(
        vm.get_property(&copy, "0")
            .and_then(|value| vm.get_property(&value, "marker"))
            .expect("source value should survive start coercion"),
        Value::Number(40.0)
    );
    assert_eq!(
        vm.get_property(&copy, "1")
            .and_then(|value| vm.get_property(&value, "marker"))
            .expect("inserted value should survive start coercion"),
        Value::Number(2.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        "Array.prototype.toSpliced.call(new Proxy({ length: 1 }, { get: function (target, key, receiver) { if (key === '0') throw 'get'; return Reflect.get(target, key, receiver); } }), 0, 0);",
        "Array.prototype.toSpliced.call({ length: 1 }, { valueOf: function () { forceGc(); throw 'start'; } });",
        "Array.prototype.toSpliced.call({ length: 1 }, 0, { valueOf: function () { forceGc(); throw 'skip'; } });",
        "Array.prototype.toSpliced.call({ length: Number.MAX_SAFE_INTEGER }, 0, 0, 1);",
        "Array.prototype.toSpliced.call(null);",
    ] {
        vm.run(source)
            .expect_err("the observable toSpliced step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_to_spliced_result_allocation_obeys_heap_cap_and_gc_retry() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run(
            r#"
            globalThis.toSplicedReads = 0;
            ({ get 0() { toSplicedReads++; return 1; }, length: 1 });
            "#,
        )
        .expect("allocation-failure source should initialize");
    let source_pin = vm.pin(&source);
    vm.gc();
    let baseline = vm.gc_pins.len();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let error = crate::builtins::array_to_spliced(&mut vm, &[], Some(source))
        .expect_err("result allocation should respect the exact heap cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_max_heap_objects(None);
    assert_eq!(
        vm.run("toSplicedReads")
            .expect("indexed getter count should remain readable"),
        Value::Number(0.0),
        "ArrayCreate must fail before indexed Gets"
    );
    vm.unpin(source_pin);

    let mut vm = Vm::new().expect("retry VM should initialize");
    vm.run(
        r#"
        globalThis.other = $262.createRealm().global;
        globalThis.retrySource = [{ marker: 42 }];
        "#,
    )
    .expect("foreign Realm retry fixture should initialize");
    let method = vm
        .run("other.Array.prototype.toSpliced")
        .expect("foreign method should be readable");
    let source = vm
        .run("retrySource")
        .expect("retry source should be readable");
    let expected_proto = vm
        .run("other.Array.prototype")
        .expect("foreign Array prototype should be readable");
    let fixture_pins = vm.pin_many(&[method.clone(), source.clone(), expected_proto.clone()]);
    vm.gc();
    let baseline_live = vm.heap.live_count();
    vm.run(
        r#"
        (function () {
          for (var i = 0; i < 100; i++) ({ index: i });
        })();
        "#,
    )
    .expect("collectible retry garbage should initialize");
    let capped_live = vm.heap.live_count();
    assert!(capped_live > baseline_live, "fixture must leave garbage");
    vm.set_max_heap_objects(Some(capped_live));
    let baseline = vm.gc_pins.len();

    let result = vm
        .call_function(&method, &[], Some(source.clone()))
        .expect("result allocation should collect garbage and retry");
    vm.set_max_heap_objects(None);
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.get_prototype_of(&result)
            .expect("result prototype lookup should succeed"),
        Some(expected_proto)
    );
    assert_eq!(
        vm.get_property(&result, "0")
            .and_then(|value| vm.get_property(&value, "marker"))
            .expect("copied source value should survive allocation retry"),
        Value::Number(42.0)
    );
    vm.unpin_many(fixture_pins);
}

#[test]
fn array_for_each_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("visit", |_, _, _| Ok(Value::Undefined), 3)
        .expect("native callback should register");
    let callback = vm.run("visit").expect("callback should be readable");
    let source = vm
        .run("Object.assign(Object.create(null), { 0: 1, 2: 3, length: 3 })")
        .expect("forEach source should initialize");
    let source_pin = vm.pin(&source);
    let callback_pin = vm.pin(&callback);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(2));
    let error = crate::builtins::array_for_each(
        &mut vm,
        std::slice::from_ref(&callback),
        Some(source.clone()),
    )
    .expect_err("N-1 fuel must abort the logical forEach scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(3));
    let result =
        crate::builtins::array_for_each(&mut vm, std::slice::from_ref(&callback), Some(source))
            .expect("exact logical-index fuel should complete forEach");
    assert_eq!(result, Value::Undefined);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    let empty = vm
        .run("({ length: 0 })")
        .expect("empty source should initialize");
    vm.set_fuel(Some(0));
    crate::builtins::array_for_each(&mut vm, &[callback], Some(empty))
        .expect("empty forEach should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(callback_pin);
    vm.unpin(source_pin);
}

#[test]
fn array_for_each_roots_observable_state_and_restores_pin_depth() {
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
    let result = vm.run(
        r#"
        (function () {
          var value = { marker: 41 };
          var context = { marker: 7 };
          var target = { 0: value };
          Object.defineProperty(target, "length", {
            get: function () { forceGc(); return 1; }
          });
          var source = new Proxy(target, {
            has: function (object, key) {
              forceGc();
              return Reflect.has(object, key);
            },
            get: function (object, key, receiver) {
              forceGc();
              return Reflect.get(object, key, receiver);
            }
          });
          var observed = false;
          Array.prototype.forEach.call(source, function (selected, index, receiver) {
            target[0] = null;
            source = null;
            value = null;
            context = null;
            forceGc();
            observed = selected.marker === 41 && index === 0 &&
                       receiver.length === 1 && this.marker === 7;
          }, context);
          forceGc();
          return observed;
        })();
        "#,
    );
    assert_eq!(
        result.expect("forEach native-frame roots should survive observable GC"),
        Value::Bool(true)
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        r#"
        var error = {};
        Array.prototype.forEach.call(new Proxy({}, {
          get: function () { forceGc(); throw error; }
        }), function () {});
        "#,
        r#"
        var error = {};
        Array.prototype.forEach.call(new Proxy({ length: 1 }, {
          has: function () { forceGc(); throw error; }
        }), function () {});
        "#,
        r#"
        var error = {};
        Array.prototype.forEach.call(new Proxy({ 0: 1, length: 1 }, {
          get: function (target, key, receiver) {
            if (key === "0") { forceGc(); throw error; }
            return Reflect.get(target, key, receiver);
          }
        }), function () {});
        "#,
        r#"
        var error = {};
        Array.prototype.forEach.call({ 0: 1, length: 1 }, function () {
          forceGc();
          throw error;
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
            .expect_err("the observable forEach step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_to_locale_string_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("({ 0: null, 1: undefined, 2: null, length: 3 })")
        .expect("toLocaleString source should initialize");
    let source_pin = vm.pin(&source);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(2));
    let error = crate::builtins::array_to_locale_string(&mut vm, &[], Some(source.clone()))
        .expect_err("N-1 fuel must abort the logical locale scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert!(vm.active_array_joins.is_empty());

    vm.set_fuel(Some(3));
    let result = crate::builtins::array_to_locale_string(&mut vm, &[], Some(source))
        .expect("exact logical-index fuel should complete locale conversion");
    assert_eq!(result, Value::String(Arc::from(",,")));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert!(vm.active_array_joins.is_empty());
    vm.set_fuel(None);

    let empty = vm
        .run("({ length: 0 })")
        .expect("empty source should initialize");
    vm.set_fuel(Some(0));
    crate::builtins::array_to_locale_string(&mut vm, &[], Some(empty))
        .expect("empty locale conversion should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert!(vm.active_array_joins.is_empty());
    vm.set_fuel(None);
    vm.unpin(source_pin);
}

#[test]
fn array_to_locale_string_roots_observable_state_and_restores_pin_depth() {
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
    let result = vm.run(
        r#"
        (function () {
          var localized = {
            marker: 41,
            toString: function () { forceGc(); return String(this.marker); }
          };
          var value = {};
          Object.defineProperty(value, "toLocaleString", {
            get: function () {
              forceGc();
              return function () {
                target[0] = null;
                value = null;
                forceGc();
                var selected = localized;
                localized = null;
                forceGc();
                return selected;
              };
            }
          });
          var target = { 0: value };
          Object.defineProperty(target, "length", {
            get: function () { source = null; forceGc(); return 1; }
          });
          var source = new Proxy(target, {
            get: function (object, key, receiver) {
              forceGc();
              return Reflect.get(object, key, receiver);
            }
          });
          return Array.prototype.toLocaleString.call(source);
        })();
        "#,
    );
    assert_eq!(
        result.expect("locale native-frame roots should survive observable GC"),
        Value::String(Arc::from("41"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);
    assert!(vm.active_array_joins.is_empty());

    let invocation_throw = vm.run(
        r#"
        (function () {
          var thrown = { marker: 73 };
          var source = { length: 1 };
          source[0] = {
            toLocaleString: function () {
              var selected = thrown;
              thrown = null;
              source[0] = null;
              forceGc();
              throw selected;
            }
          };
          try { Array.prototype.toLocaleString.call(source); }
          catch (error) { forceGc(); return error.marker; }
        })();
        "#,
    );
    assert_eq!(
        invocation_throw.expect("thrown locale value should survive forced GC"),
        Value::Number(73.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);
    assert!(vm.active_array_joins.is_empty());

    let conversion_throw = vm.run(
        r#"
        (function () {
          var thrown = { marker: 89 };
          var source = { length: 1 };
          source[0] = {
            toLocaleString: function () {
              return {
                toString: function () {
                  var selected = thrown;
                  thrown = null;
                  source[0] = null;
                  forceGc();
                  throw selected;
                }
              };
            }
          };
          try { Array.prototype.toLocaleString.call(source); }
          catch (error) { forceGc(); return error.marker; }
        })();
        "#,
    );
    assert_eq!(
        conversion_throw.expect("thrown localized conversion should survive forced GC"),
        Value::Number(89.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);
    assert!(vm.active_array_joins.is_empty());

    for source in [
        r#"
        var error = {};
        Array.prototype.toLocaleString.call(new Proxy({}, {
          get: function () { forceGc(); throw error; }
        }));
        "#,
        r#"
        var error = {};
        Array.prototype.toLocaleString.call(new Proxy({ length: 1 }, {
          get: function (target, key, receiver) {
            if (key === "0") { forceGc(); throw error; }
            return Reflect.get(target, key, receiver);
          }
        }));
        "#,
        r#"
        var error = {};
        var value = {};
        Object.defineProperty(value, "toLocaleString", {
          get: function () { forceGc(); throw error; }
        });
        Array.prototype.toLocaleString.call({ 0: value, length: 1 });
        "#,
        r#"
        var error = {};
        Array.prototype.toLocaleString.call({
          0: { toLocaleString: function () { forceGc(); throw error; } },
          length: 1
        });
        "#,
        r#"
        var error = {};
        Array.prototype.toLocaleString.call({
          0: { toLocaleString: function () {
            return { toString: function () { forceGc(); throw error; } };
          } },
          length: 1
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
            .expect_err("the observable locale step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert!(vm.active_array_joins.is_empty());
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn typed_array_join_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("new Uint8Array([1, 2, 3])")
        .expect("TypedArray join source should initialize");
    let source_pin = vm.pin(&source);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(2));
    let error = crate::builtins::typed_array_join(&mut vm, &[], Some(source.clone()))
        .expect_err("N-1 fuel must abort the TypedArray join scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(3));
    let result = crate::builtins::typed_array_join(&mut vm, &[], Some(source))
        .expect("exact logical-index fuel should complete TypedArray join");
    assert_eq!(result, Value::String(Arc::from("1,2,3")));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    let empty = vm
        .run("new Uint8Array(0)")
        .expect("empty TypedArray should initialize");
    vm.set_fuel(Some(0));
    crate::builtins::typed_array_join(&mut vm, &[], Some(empty))
        .expect("empty TypedArray join should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(source_pin);

    let bigint = vm
        .run("new BigInt64Array([1n, 2n])")
        .expect("BigInt TypedArray should initialize");
    let bigint_pin = vm.pin(&bigint);
    let baseline = vm.gc_pins.len();
    vm.set_fuel(Some(1));
    let error = crate::builtins::typed_array_join(&mut vm, &[], Some(bigint.clone()))
        .expect_err("BigInt N-1 fuel must abort the TypedArray join scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(Some(2));
    assert_eq!(
        crate::builtins::typed_array_join(&mut vm, &[], Some(bigint))
            .expect("BigInt TypedArray join should complete with exact fuel"),
        Value::String(Arc::from("1,2"))
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(bigint_pin);
}

#[test]
fn typed_array_join_roots_observable_state_and_restores_pin_depth() {
    for (source, expected) in [
        ("new Uint8Array([1, 2])", "1.2"),
        ("new BigInt64Array([1n, 2n])", "1.2"),
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
        let source = vm
            .run(source)
            .expect("direct join source should initialize");
        let source_pin = vm.pin(&source);
        let separator = vm
            .run("({ toString: function () { forceGc(); return '.'; } })")
            .expect("direct join separator should initialize");
        vm.unpin(source_pin);
        let baseline = vm.gc_pins.len();
        assert_eq!(
            crate::builtins::typed_array_join(&mut vm, &[separator], Some(source))
                .expect("direct native join roots should survive forced GC"),
            Value::String(Arc::from(expected))
        );
        assert_eq!(vm.gc_pins.len(), baseline);
    }

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
    assert_eq!(
        vm.run(
            r#"
            (function () {
              var source = new Uint8Array([1, 2]);
              var separator = {
                toString: function () {
                  source = null;
                  separator = null;
                  forceGc();
                  return ".";
                }
              };
              return source.join(separator);
            })();
            "#,
        )
        .expect("TypedArray join roots should survive separator coercion GC"),
        Value::String(Arc::from("1.2"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    assert_eq!(
        vm.run(
            r#"
            (function () {
              var source = new Uint8Array([1]);
              var thrown = { marker: 97 };
              try {
                source.join({
                  toString: function () {
                    var selected = thrown;
                    source = null;
                    thrown = null;
                    forceGc();
                    throw selected;
                  }
                });
              } catch (error) {
                forceGc();
                return error.marker;
              }
            })();
            "#,
        )
        .expect("thrown separator value should survive TypedArray join GC"),
        Value::Number(97.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        "Uint8Array.prototype.join.call({});",
        r#"
        var buffer = new ArrayBuffer(4, { maxByteLength: 8 });
        var fixed = new Uint8Array(buffer, 0, 4);
        buffer.resize(0);
        fixed.join({ toString: function () { throw "unreachable"; } });
        "#,
        r#"
        new Uint8Array([1]).join({
          toString: function () { forceGc(); throw {}; }
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
            .expect_err("the TypedArray join step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn typed_array_to_string_alias_roots_direct_native_state() {
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

    let source = vm
        .run(
            r#"
            var source = new Uint8Array([1, 2]);
            source.join = function () {
              source = null;
              forceGc();
              return this[0] + "|" + this[1];
            };
            source;
            "#,
        )
        .expect("direct TypedArray toString source should initialize");
    assert_eq!(
        crate::builtins::array::array_to_string(&mut vm, &[], Some(source))
            .expect("call roots should preserve the direct-native receiver"),
        Value::String(Arc::from("1|2"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    let fallback = vm
        .run(
            r#"
            var fallback = new Uint8Array(0);
            fallback.join = null;
            Object.defineProperty(fallback, Symbol.toStringTag, {
              get: function () {
                fallback = null;
                forceGc();
                return "DirectTypedArray";
              }
            });
            fallback;
            "#,
        )
        .expect("direct fallback receiver should initialize");
    assert_eq!(
        crate::builtins::array::array_to_string(&mut vm, &[], Some(fallback))
            .expect("fallback roots should survive observable tag lookup"),
        Value::String(Arc::from("[object DirectTypedArray]"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn typed_array_search_methods_consume_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("new Uint8Array([1, 2, 3])")
        .expect("TypedArray search source should initialize");
    let source_pin = vm.pin(&source);
    let baseline = vm.gc_pins.len();

    for (search, target) in [
        (
            crate::builtins::typed_array_includes
                as fn(&mut Vm, &[Value], Option<Value>) -> crate::error::Result<Value>,
            Value::Number(3.0),
        ),
        (crate::builtins::typed_array_index_of, Value::Number(3.0)),
        (
            crate::builtins::typed_array_last_index_of,
            Value::Number(1.0),
        ),
    ] {
        vm.set_fuel(Some(2));
        let error = search(&mut vm, &[target], Some(source.clone()))
            .expect_err("N-1 fuel must abort a three-index search");
        assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
        assert_eq!(vm.fuel_remaining(), Some(0));
        assert_eq!(vm.gc_pins.len(), baseline);
    }

    vm.set_fuel(Some(3));
    assert_eq!(
        crate::builtins::typed_array_includes(
            &mut vm,
            &[Value::Number(3.0)],
            Some(source.clone()),
        )
        .expect("includes should complete with exact fuel"),
        Value::Bool(true)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(3));
    assert_eq!(
        crate::builtins::typed_array_index_of(
            &mut vm,
            &[Value::Number(3.0)],
            Some(source.clone()),
        )
        .expect("indexOf should complete with exact fuel"),
        Value::Number(2.0)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(3));
    assert_eq!(
        crate::builtins::typed_array_last_index_of(
            &mut vm,
            &[Value::Number(1.0)],
            Some(source.clone()),
        )
        .expect("lastIndexOf should complete with exact fuel"),
        Value::Number(0.0)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(1));
    assert_eq!(
        crate::builtins::typed_array_includes(
            &mut vm,
            &[Value::Number(1.0)],
            Some(source.clone()),
        )
        .expect("an immediate match should consume one index"),
        Value::Bool(true)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(0));
    assert_eq!(
        crate::builtins::typed_array_index_of(
            &mut vm,
            &[Value::Number(1.0), Value::Number(3.0)],
            Some(source.clone()),
        )
        .expect("an empty forward range should consume no index fuel"),
        Value::Number(-1.0)
    );
    assert_eq!(
        crate::builtins::typed_array_last_index_of(
            &mut vm,
            &[Value::Number(1.0), Value::Number(-4.0)],
            Some(source),
        )
        .expect("an empty reverse range should consume no index fuel"),
        Value::Number(-1.0)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(source_pin);

    let empty = vm
        .run("new Uint8Array(0)")
        .expect("empty search source should initialize");
    let empty_pin = vm.pin(&empty);
    let baseline = vm.gc_pins.len();
    for (search, expected) in [
        (
            crate::builtins::typed_array_includes
                as fn(&mut Vm, &[Value], Option<Value>) -> crate::error::Result<Value>,
            Value::Bool(false),
        ),
        (crate::builtins::typed_array_index_of, Value::Number(-1.0)),
        (
            crate::builtins::typed_array_last_index_of,
            Value::Number(-1.0),
        ),
    ] {
        vm.set_fuel(Some(0));
        assert_eq!(
            search(&mut vm, &[Value::Number(1.0)], Some(empty.clone()))
                .expect("an empty TypedArray search should consume no index fuel"),
            expected
        );
        assert_eq!(vm.fuel_remaining(), Some(0));
        assert_eq!(vm.gc_pins.len(), baseline);
    }
    vm.set_fuel(None);
    vm.unpin(empty_pin);

    let bigint = vm
        .run("new BigInt64Array([1n, 2n])")
        .expect("BigInt search source should initialize");
    let target = vm.run("2n").expect("BigInt target should initialize");
    let bigint_pin = vm.pin(&bigint);
    let target_pin = vm.pin(&target);
    let baseline = vm.gc_pins.len();
    vm.set_fuel(Some(1));
    let error = crate::builtins::typed_array_includes(
        &mut vm,
        std::slice::from_ref(&target),
        Some(bigint.clone()),
    )
    .expect_err("BigInt N-1 fuel must abort the search");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(Some(2));
    assert_eq!(
        crate::builtins::typed_array_includes(
            &mut vm,
            std::slice::from_ref(&target),
            Some(bigint),
        )
        .expect("BigInt includes should complete with exact fuel"),
        Value::Bool(true)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(target_pin);
    vm.unpin(bigint_pin);
}

#[test]
fn typed_array_to_locale_string_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("new Uint8Array([1, 2, 3])")
        .expect("TypedArray locale source should initialize");
    let source_pin = vm.pin(&source);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(2));
    let error = crate::builtins::typed_array_to_locale_string(&mut vm, &[], Some(source.clone()))
        .expect_err("N-1 fuel must abort the TypedArray locale scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(3));
    let result = crate::builtins::typed_array_to_locale_string(&mut vm, &[], Some(source))
        .expect("exact logical-index fuel should complete TypedArray locale conversion");
    assert_eq!(result, Value::String(Arc::from("1,2,3")));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    let empty = vm
        .run("new Uint8Array(0)")
        .expect("empty TypedArray should initialize");
    vm.set_fuel(Some(0));
    crate::builtins::typed_array_to_locale_string(&mut vm, &[], Some(empty))
        .expect("empty TypedArray locale conversion should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(source_pin);
}

#[test]
fn typed_array_to_locale_string_roots_state_and_restores_pin_depth() {
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
    let result = vm.run(
        r#"
        (function () {
          var localized = {
            marker: 41,
            toString: function () { forceGc(); return String(this.marker); }
          };
          Object.defineProperty(Number.prototype, "toLocaleString", {
            configurable: true,
            get: function () {
              forceGc();
              return function () {
                source = null;
                forceGc();
                var selected = localized;
                localized = null;
                forceGc();
                return selected;
              };
            }
          });
          var source = new Uint8Array([1]);
          return source.toLocaleString();
        })();
        "#,
    );
    assert_eq!(
        result.expect("TypedArray locale roots should survive observable GC"),
        Value::String(Arc::from("41"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    for (source, expected) in [
        (
            r#"
            (function () {
              var thrown = { marker: 73 };
              Object.defineProperty(Number.prototype, "toLocaleString", {
                configurable: true,
                value: function () {
                  var selected = thrown;
                  thrown = null;
                  source = null;
                  forceGc();
                  throw selected;
                }
              });
              var source = new Uint8Array([1]);
              try { source.toLocaleString(); }
              catch (error) { forceGc(); return error.marker; }
            })();
            "#,
            73.0,
        ),
        (
            r#"
            (function () {
              var thrown = { marker: 89 };
              Object.defineProperty(Number.prototype, "toLocaleString", {
                configurable: true,
                value: function () {
                  return {
                    toString: function () {
                      var selected = thrown;
                      thrown = null;
                      source = null;
                      forceGc();
                      throw selected;
                    }
                  };
                }
              });
              var source = new Uint8Array([1]);
              try { source.toLocaleString(); }
              catch (error) { forceGc(); return error.marker; }
            })();
            "#,
            89.0,
        ),
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
        assert_eq!(
            vm.run(source)
                .expect("thrown TypedArray locale value should survive GC"),
            Value::Number(expected)
        );
        assert_eq!(vm.gc_pins.len(), baseline);
    }

    for source in [
        "Uint8Array.prototype.toLocaleString.call({});",
        r#"
        var buffer = new ArrayBuffer(4, { maxByteLength: 8 });
        var fixed = new Uint8Array(buffer, 0, 4);
        buffer.resize(0);
        fixed.toLocaleString();
        "#,
        r#"
        Object.defineProperty(Number.prototype, "toLocaleString", {
          configurable: true,
          get: function () { forceGc(); throw {}; }
        });
        new Uint8Array([1]).toLocaleString();
        "#,
        r#"
        Number.prototype.toLocaleString = null;
        new Uint8Array([1]).toLocaleString();
        "#,
        r#"
        Number.prototype.toLocaleString = function () { forceGc(); throw {}; };
        new Uint8Array([1]).toLocaleString();
        "#,
        r#"
        Number.prototype.toLocaleString = function () {
          return { toString: function () { forceGc(); throw {}; } };
        };
        new Uint8Array([1]).toLocaleString();
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
            .expect_err("the TypedArray locale step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_join_consumes_exact_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("Object.assign(Object.create(null), { 0: 'a', 2: 'c', length: 3 })")
        .expect("join source should initialize");
    let source_pin = vm.pin(&source);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(2));
    let error = crate::builtins::array_join(&mut vm, &[], Some(source.clone()))
        .expect_err("N-1 fuel must abort the logical join scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(3));
    let result = crate::builtins::array_join(&mut vm, &[], Some(source))
        .expect("exact logical-index fuel should complete join");
    assert_eq!(result, Value::String(Arc::from("a,,c")));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    let empty = vm
        .run("({ length: 0 })")
        .expect("empty source should initialize");
    vm.set_fuel(Some(0));
    crate::builtins::array_join(&mut vm, &[], Some(empty))
        .expect("empty join should consume no loop fuel");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(source_pin);
}

#[test]
fn array_join_roots_observable_state_and_restores_pin_depth() {
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
    let result = vm.run(
        r#"
        (function () {
          var value = {
            marker: 41,
            toString: function () { forceGc(); return String(this.marker); }
          };
          var target = { 0: value };
          Object.defineProperty(target, "length", {
            get: function () { source = null; forceGc(); return 1; }
          });
          var source = new Proxy(target, {
            get: function (object, key, receiver) {
              forceGc();
              var selected = Reflect.get(object, key, receiver);
              if (key === "0") {
                target[0] = null;
                value = null;
                forceGc();
              }
              return selected;
            }
          });
          var separator = {
            marker: "|",
            toString: function () { separator = null; forceGc(); return this.marker; }
          };
          return Array.prototype.join.call(source, separator);
        })();
        "#,
    );
    assert_eq!(
        result.expect("join native-frame roots should survive observable GC"),
        Value::String(Arc::from("41"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    let thrown = vm.run(
        r#"
        (function () {
          var source = { length: 1 };
          var thrown = { marker: 73 };
          source[0] = {
            toString: function () {
              var selected = thrown;
              thrown = null;
              source[0] = null;
              forceGc();
              throw selected;
            }
          };
          try { Array.prototype.join.call(source); }
          catch (error) { forceGc(); return error.marker; }
        })();
        "#,
    );
    assert_eq!(
        thrown.expect("thrown join conversion object should survive forced GC"),
        Value::Number(73.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        r#"
        var error = {};
        Array.prototype.join.call(new Proxy({}, {
          get: function () { forceGc(); throw error; }
        }));
        "#,
        r#"
        var error = {};
        Array.prototype.join.call({ length: 0 }, {
          toString: function () { forceGc(); throw error; }
        });
        "#,
        r#"
        var error = {};
        Array.prototype.join.call(new Proxy({ 0: 1, length: 1 }, {
          get: function (target, key, receiver) {
            if (key === "0") { forceGc(); throw error; }
            return Reflect.get(target, key, receiver);
          }
        }));
        "#,
        r#"
        var error = {};
        Array.prototype.join.call({
          0: { toString: function () { forceGc(); throw error; } },
          length: 1
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
            .expect_err("the observable join step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_filter_roots_observable_state_and_restores_pin_depth() {
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
    let result = vm.run(
        r#"
        (function () {
          var value = { marker: 41 };
          var context = { marker: 7 };
          var sourceTarget = [value];
          sourceTarget.constructor = {};
          sourceTarget.constructor[Symbol.species] = function () {
            forceGc();
            return new Proxy({}, {
              defineProperty: function (target, key, descriptor) {
                forceGc();
                return Reflect.defineProperty(target, key, descriptor);
              }
            });
          };
          var source = new Proxy(sourceTarget, {
            has: function (target, key) {
              forceGc();
              return Reflect.has(target, key);
            },
            get: function (target, key, receiver) {
              forceGc();
              return Reflect.get(target, key, receiver);
            }
          });
          var result = Array.prototype.filter.call(source, function (selected, index, receiver) {
            sourceTarget[0] = null;
            source = null;
            value = null;
            context = null;
            forceGc();
            return selected.marker === 41 && index === 0 &&
                   receiver.length === 1 && this.marker === 7;
          }, context);
          forceGc();
          return result[0].marker;
        })();
        "#,
    );
    assert_eq!(
        result.expect("filter source, result, and selected value should survive GC"),
        Value::Number(41.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        r#"
        var error = {};
        Array.prototype.filter.call(new Proxy({}, {
          get: function () { forceGc(); throw error; }
        }), function () {});
        "#,
        r#"
        var error = {};
        var source = [];
        Object.defineProperty(source, "constructor", {
          get: function () { forceGc(); throw error; }
        });
        source.filter(function () {});
        "#,
        r#"
        var error = {};
        var source = [1];
        source.constructor = {};
        Object.defineProperty(source.constructor, Symbol.species, {
          get: function () { forceGc(); throw error; }
        });
        source.filter(function () {});
        "#,
        r#"
        var error = {};
        Array.prototype.filter.call(new Proxy({ length: 1 }, {
          has: function () { forceGc(); throw error; }
        }), function () {});
        "#,
        r#"
        var error = {};
        Array.prototype.filter.call(new Proxy({ 0: 1, length: 1 }, {
          get: function (target, key, receiver) {
            if (key === "0") { forceGc(); throw error; }
            return Reflect.get(target, key, receiver);
          }
        }), function () {});
        "#,
        r#"
        var error = {};
        [1].filter(function () { forceGc(); throw error; });
        "#,
        r#"
        var error = {};
        var source = [1];
        source.constructor = {};
        source.constructor[Symbol.species] = function () {
          return new Proxy({}, {
            defineProperty: function () { forceGc(); throw error; }
          });
        };
        source.filter(function () { return true; });
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
            .expect_err("the observable filter step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_filter_retries_result_allocation_after_heap_cap_gc() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("keep", |_, _, _| Ok(Value::Bool(true)), 3)
        .expect("native predicate should register");
    let source = vm
        .run("globalThis.source = [{ marker: 1 }]; source;")
        .expect("filter source should initialize");
    let callback = vm.run("keep").expect("predicate should be readable");
    let source_pin = vm.pin(&source);
    let callback_pin = vm.pin(&callback);
    vm.gc();
    let baseline_live = vm.heap.live_count();
    vm.run(
        r#"
        (function () {
          for (var i = 0; i < 100; i++) ({ index: i });
        })();
        "#,
    )
    .expect("garbage fixture should initialize");
    let capped_live = vm.heap.live_count();
    assert!(capped_live > baseline_live, "fixture must leave garbage");
    vm.set_max_heap_objects(Some(capped_live));
    let baseline_pins = vm.gc_pins.len();

    let result = crate::builtins::array_filter(&mut vm, &[callback], Some(source))
        .expect("filter result allocation should collect garbage and retry");
    vm.set_max_heap_objects(None);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(
        vm.get_property(&result, "0")
            .and_then(|value| vm.get_property(&value, "marker"))
            .expect("selected value should survive allocation retry"),
        Value::Number(1.0)
    );
    vm.unpin(callback_pin);
    vm.unpin(source_pin);
}

#[test]
fn array_iterators_preserve_safe_indices_and_advance_before_allocation() {
    let mut vm = Vm::new().expect("VM should initialize");
    let iterator = vm
        .run("Array.prototype.keys.call({ length: Number.MAX_SAFE_INTEGER })")
        .expect("large array-like iterator should initialize");
    let iterator_pin = vm.pin(&iterator);
    let Value::Object(iterator_idx) = iterator else {
        panic!("keys should return an iterator object");
    };
    vm.heap.with_obj(iterator_idx.0, |object| {
        let HeapObj::CollectionIterator(iterator) = object else {
            panic!("keys should return a collection iterator");
        };
        *iterator.index.lock() = 9_007_199_254_740_990;
    });
    let baseline = vm.gc_pins.len();
    let result = call_iterator_next_result(&mut vm, &Value::Object(iterator_idx))
        .expect("safe-integer key should be produced");
    let result_pin = vm.pin(&result);
    assert_eq!(
        vm.get_property(&result, "value")
            .expect("iterator value should be readable"),
        Value::Number(9_007_199_254_740_990.0)
    );
    assert_eq!(
        vm.get_property(&result, "done")
            .expect("iterator done flag should be readable"),
        Value::Bool(false)
    );
    vm.unpin(result_pin);
    let exhausted = call_iterator_next_result(&mut vm, &Value::Object(iterator_idx))
        .expect("safe-integer iterator should exhaust");
    assert_eq!(
        vm.get_property(&exhausted, "done")
            .expect("exhausted done flag should be readable"),
        Value::Bool(true)
    );
    vm.heap.with_obj(iterator_idx.0, |object| {
        let HeapObj::CollectionIterator(iterator) = object else {
            panic!("iterator should retain its internal slots");
        };
        assert!(iterator.source.lock().is_undefined());
        assert_eq!(*iterator.index.lock(), 9_007_199_254_740_991);
    });
    call_iterator_next_result(&mut vm, &Value::Object(iterator_idx))
        .expect("an exhausted iterator should stay exhausted");
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.unpin(iterator_pin);

    let iterator = vm
        .run("Array.prototype.values.call({ 0: 'first', 1: 'second', length: 2 })")
        .expect("value iterator should initialize");
    let iterator_pin = vm.pin(&iterator);
    vm.clear_kept_objects();
    vm.gc();
    let baseline = vm.gc_pins.len();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let error = call_iterator_next_result(&mut vm, &iterator)
        .expect_err("iterator result allocation should respect the exact heap cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_max_heap_objects(None);
    let result = call_iterator_next_result(&mut vm, &iterator)
        .expect("the index must remain advanced after result allocation fails");
    assert_eq!(
        vm.get_property(&result, "value")
            .expect("post-failure value should be readable"),
        Value::String(Arc::from("second"))
    );
    vm.unpin(iterator_pin);

    let empty = vm
        .run("Array.prototype.values.call({ length: 0 })")
        .expect("empty iterator should initialize");
    let empty_pin = vm.pin(&empty);
    vm.clear_kept_objects();
    vm.gc();
    let baseline = vm.gc_pins.len();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let error = call_iterator_next_result(&mut vm, &empty)
        .expect_err("done-result allocation should respect the exact heap cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline);
    let Value::Object(empty_idx) = empty else {
        panic!("values should return an iterator object");
    };
    vm.heap.with_obj(empty_idx.0, |object| {
        let HeapObj::CollectionIterator(iterator) = object else {
            panic!("values should return a collection iterator");
        };
        assert!(iterator.source.lock().is_undefined());
    });
    vm.set_max_heap_objects(None);
    let result = call_iterator_next_result(&mut vm, &Value::Object(empty_idx))
        .expect("completion state should survive result allocation failure");
    assert_eq!(
        vm.get_property(&result, "done")
            .expect("done flag should be readable"),
        Value::Bool(true)
    );
    vm.unpin(empty_pin);
}

#[test]
fn array_iterator_creation_and_entries_root_across_gc_retry() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("({ length: 0 })")
        .expect("iterator source should initialize");
    let source_pin = vm.pin(&source);
    vm.clear_kept_objects();
    vm.gc();
    let _garbage = vm.new_object().expect("garbage object should allocate");
    let limit = vm.heap.live_count();
    let baseline = vm.gc_pins.len();
    vm.set_max_heap_objects(Some(limit));
    let iterator = crate::builtins::array_values(&mut vm, &[], Some(source.clone()))
        .expect("iterator allocation should retry after collecting garbage");
    assert!(matches!(iterator, Value::Object(_)));
    assert!(vm.heap.live_count() <= limit);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_max_heap_objects(None);
    vm.unpin(source_pin);

    let source = vm
        .run("({ 0: { marker: 17 }, length: 1 })")
        .expect("entry source should initialize");
    let iterator = crate::builtins::array_entries(&mut vm, &[], Some(source))
        .expect("entry iterator should initialize");
    let iterator_pin = vm.pin(&iterator);
    vm.clear_kept_objects();
    vm.gc();
    let _garbage_one = vm
        .new_object()
        .expect("first garbage object should allocate");
    let _garbage_two = vm
        .new_object()
        .expect("second garbage object should allocate");
    let limit = vm.heap.live_count();
    let baseline = vm.gc_pins.len();
    vm.set_max_heap_objects(Some(limit));
    let result = call_iterator_next_result(&mut vm, &iterator)
        .expect("entry pair and result should retry across exact-cap collection");
    vm.set_max_heap_objects(None);
    let result_pin = vm.pin(&result);
    let pair = vm
        .get_property(&result, "value")
        .expect("entry result value should be readable");
    let pair_pin = vm.pin(&pair);
    let value = vm
        .get_property(&pair, "1")
        .expect("entry element should be readable");
    assert_eq!(
        vm.get_property(&value, "marker")
            .expect("rooted entry marker should be readable"),
        Value::Number(17.0)
    );
    vm.unpin(pair_pin);
    vm.unpin(result_pin);
    assert!(vm.heap.live_count() <= limit);
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.unpin(iterator_pin);
}

#[test]
fn array_iterator_next_roots_observable_state_and_restores_pin_depth() {
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
    let result = vm.run(
        r#"
        (function () {
          var target = {
            0: { marker: 23 },
            get length() { forceGc(); return 1; }
          };
          var source = new Proxy(target, {
            get: function (target, key, receiver) {
              var value = Reflect.get(target, key, receiver);
              if (key === "0") delete target[0];
              forceGc();
              return value;
            }
          });
          var iterator = Array.prototype.entries.call(source);
          source = null;
          var result = iterator.next();
          forceGc();
          return [result.value[0], result.value[1].marker, result.done].join(":");
        })();
        "#,
    );
    assert_eq!(
        result.expect("source and fetched value should survive observable GC"),
        Value::String(Arc::from("0:23:false"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        r#"
        var error = {};
        var iterator = Array.prototype.values.call(new Proxy({}, {
          get: function (target, key) {
            forceGc();
            if (key === "length") throw error;
          }
        }));
        iterator.next();
        "#,
        r#"
        var error = {};
        var iterator = Array.prototype.values.call(new Proxy({ length: 1 }, {
          get: function (target, key, receiver) {
            forceGc();
            if (key === "0") throw error;
            return Reflect.get(target, key, receiver);
          }
        }));
        iterator.next();
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
            .expect_err("observable iterator step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn array_concat_consumes_exact_per_item_and_per_index_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    let empty = vm
        .run(
            r#"
            var empty = { length: 0 };
            empty[Symbol.isConcatSpreadable] = true;
            empty;
            "#,
        )
        .expect("empty spreadable receiver should initialize");
    let empty_pin = vm.pin(&empty);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(0));
    let error = crate::builtins::array_concat(&mut vm, &[], Some(empty.clone()))
        .expect_err("an empty spreadable item still requires one unit of fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(1));
    let empty_result = crate::builtins::array_concat(&mut vm, &[], Some(empty))
        .expect("exact outer-item fuel should complete empty concat");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(
        vm.get_property(&empty_result, "length")
            .expect("empty concat result length should be readable"),
        Value::Number(0.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(empty_pin);

    let source = vm
        .run(
            r#"
            var source = { length: 2, 0: 1, 1: 2 };
            source[Symbol.isConcatSpreadable] = true;
            source;
            "#,
        )
        .expect("spreadable receiver should initialize");
    let source_pin = vm.pin(&source);
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(2));
    let error = crate::builtins::array_concat(&mut vm, &[], Some(source.clone()))
        .expect_err("N-1 fuel must abort the indexed scan");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(3));
    let result = crate::builtins::array_concat(&mut vm, &[], Some(source))
        .expect("one item plus two indices should consume exactly three fuel units");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(
        vm.get_property(&result, "length")
            .expect("concat result length should be readable"),
        Value::Number(2.0)
    );
    assert_eq!(
        vm.get_property(&result, "1")
            .expect("second concat result value should be readable"),
        Value::Number(2.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    vm.unpin(source_pin);
}

#[test]
fn array_species_allocation_failures_restore_pin_depth_and_preserve_sources() {
    for expression in [
        "source.concat();",
        "source.filter(function () { return true; });",
        "source.flat();",
        "source.flatMap(function (value) { return value; });",
        "source.map(function (value) { return value; });",
        "source.slice();",
        "source.splice(0, 1);",
        "source.with(0, 2);",
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.register_fn("capHeap", |vm, _, _| cap_heap_at_current_live_count(vm), 0)
            .expect("heap-cap hook should register");
        vm.run("globalThis.source = [{ marker: 1 }];")
            .expect("copy source should initialize");
        let baseline = vm.gc_pins.len();

        let error = vm
            .run(&format!("capHeap(); {expression}"))
            .expect_err("Array result allocation should hit the heap cap");
        vm.set_max_heap_objects(None);
        assert_eq!(error.kind, crate::error::ErrorKind::Range);
        assert_eq!(vm.gc_pins.len(), baseline, "pin leak after {expression}");
        assert_eq!(
            vm.run("source.length === 1 && source[0].marker === 1")
                .expect("failed result allocation must not mutate the source"),
            Value::Bool(true)
        );
    }
}

#[test]
fn array_constructor_retries_after_collecting_garbage_at_the_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_live = vm.heap.live_count();
    vm.run(
        r#"
        (function () {
          for (var i = 0; i < 100; i++) ({ index: i });
        })();
        "#,
    )
    .expect("garbage fixture should initialize");
    let capped_live = vm.heap.live_count();
    assert!(capped_live > baseline_live, "fixture must leave garbage");
    vm.set_max_heap_objects(Some(capped_live));
    let baseline_pins = vm.gc_pins.len();

    let array = crate::builtins::array_constructor(
        &mut vm,
        &[Value::Number(
            (crate::value::MAX_DENSE_ARRAY_LEN + 1) as f64,
        )],
        None,
    )
    .expect("Array allocation should collect garbage and retry");
    vm.set_max_heap_objects(None);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(
        vm.get_property(&array, "length")
            .expect("sparse Array length should be readable"),
        Value::Number((crate::value::MAX_DENSE_ARRAY_LEN + 1) as f64)
    );
}

#[test]
fn array_concat_retries_result_allocation_after_heap_cap_gc() {
    let mut vm = Vm::new().expect("VM should initialize");
    let source = vm
        .run("globalThis.source = [{ marker: 1 }]; source;")
        .expect("concat source should initialize");
    let source_pin = vm.pin(&source);
    vm.gc();
    let baseline_live = vm.heap.live_count();
    vm.run(
        r#"
        (function () {
          for (var i = 0; i < 100; i++) ({ index: i });
        })();
        "#,
    )
    .expect("garbage fixture should initialize");
    let capped_live = vm.heap.live_count();
    assert!(capped_live > baseline_live, "fixture must leave garbage");
    vm.set_max_heap_objects(Some(capped_live));
    let baseline_pins = vm.gc_pins.len();

    let result = crate::builtins::array_concat(&mut vm, &[], Some(source))
        .expect("concat result allocation should collect garbage and retry");
    vm.set_max_heap_objects(None);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(
        vm.run("source[0].marker")
            .expect("source should survive the allocation retry"),
        Value::Number(1.0)
    );
    assert_eq!(
        vm.get_property(&result, "length")
            .expect("concat result length should be readable"),
        Value::Number(1.0)
    );
    vm.unpin(source_pin);
}

#[test]
fn flat_map_target_retains_values_across_late_callback_gc() {
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
    let baseline = vm.gc_pins.len();

    let result = vm
        .run(
            r#"
        globalThis.marker = { retained: 41 };
        [1, 2].flatMap(function(value) {
          if (value === 2) { forceGc(); return value; }
          return [marker];
        });
        "#,
        )
        .expect("flatMap target and prior values should survive callback GC");
    let result_pin = vm.pin(&result);
    let marker = vm
        .run("marker")
        .expect("retained marker should remain observable");
    assert_eq!(
        vm.get_property(&result, "0")
            .expect("flatMap result should retain its first value"),
        marker
    );
    assert_eq!(
        vm.get_property(&result, "1")
            .expect("flatMap result should retain its second value"),
        Value::Number(2.0)
    );
    vm.unpin(result_pin);
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn array_flattening_roots_nested_state_and_restores_pins_after_abrupt_steps() {
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

    let result = vm
        .run(
            r#"
            var marker = { value: 41 };
            var nested = new Proxy([marker], {
              get: function(target, key, receiver) {
                if (key === "length") forceGc();
                return Reflect.get(target, key, receiver);
              }
            });
            var flat = [nested].flat()[0];
            var mapped = [1].flatMap(function() { return nested; })[0];
            flat.value + mapped.value;
            "#,
        )
        .expect("nested flattening values should survive observable GC");
    assert_eq!(result, Value::Number(82.0));
    assert_eq!(vm.gc_pins.len(), baseline);

    for source in [
        r#"
        var error = {};
        var source = [1];
        source.constructor = { get [Symbol.species]() { forceGc(); throw error; } };
        source.flat();
        "#,
        r#"
        var error = {};
        Array.prototype.flat.call(new Proxy({ length: 1 }, {
          has: function() { forceGc(); throw error; }
        }));
        "#,
        r#"
        var error = {};
        var nested = new Proxy([1], {
          get: function(target, key, receiver) {
            if (key === "length") { forceGc(); throw error; }
            return Reflect.get(target, key, receiver);
          }
        });
        [nested].flat();
        "#,
        r#"
        var error = {};
        function Species() {
          return new Proxy({}, {
            defineProperty: function() { forceGc(); throw error; }
          });
        }
        var source = [1];
        source.constructor = { [Symbol.species]: Species };
        source.flat();
        "#,
        r#"
        var error = {};
        [1].flatMap(function() { forceGc(); throw error; });
        "#,
    ] {
        vm.run(source)
            .expect_err("the observable flattening step should complete abruptly");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn arguments_objects_retry_allocation_and_restore_pins_at_the_exact_cap() {
    for (setup, function_name, values_name) in [
        (
            "globalThis.f = function(a) { return arguments; }; globalThis.v = Array.prototype.values;",
            "f",
            "v",
        ),
        (
            "globalThis.f = function(a) { 'use strict'; return arguments; }; globalThis.v = Array.prototype.values;",
            "f",
            "v",
        ),
        (
            r#"
            globalThis.realm = $262.createRealm().global;
            globalThis.f = new realm.Function("a", "return arguments;");
            globalThis.v = realm.Array.prototype.values;
            "#,
            "f",
            "v",
        ),
    ] {
        let mut vm = Vm::new().expect("VM should initialize");
        vm.run(setup).expect("arguments fixture should initialize");
        let function = vm
            .run(function_name)
            .expect("arguments function should be available");
        let values = vm
            .run(values_name)
            .expect("Realm Array values intrinsic should be available");
        let marker = Value::Object(vm.new_object().expect("argument marker should allocate"));
        let function_pin = vm.pin(&function);
        let values_pin = vm.pin(&values);
        let marker_pin = vm.pin(&marker);
        vm.gc();
        vm.run(
            "(function () { for (var i = 0; i < 32; i++) ({ index: i }); })();",
        )
        .expect("garbage fixture should initialize");
        let retry_limit = vm.heap.live_count() + 1;
        vm.set_max_heap_objects(Some(retry_limit));
        let baseline_pins = vm.gc_pins.len();

        let arguments = vm
            .call_function(&function, std::slice::from_ref(&marker), None)
            .expect("arguments allocation should collect garbage and retry");
        vm.set_max_heap_objects(None);
        let arguments_pin = vm.pin(&arguments);
        assert_eq!(
            vm.get_property(&arguments, "0")
                .expect("argument value should remain observable"),
            marker
        );
        let iterator = vm
            .get_property_by_key(
                &arguments,
                &crate::value::PropertyKey::Symbol(vm.well_known_symbols.iterator),
            )
            .expect("arguments iterator should be readable");
        assert_eq!(iterator, values);
        vm.unpin(arguments_pin);
        assert_eq!(vm.gc_pins.len(), baseline_pins);

        vm.gc();
        vm.set_max_heap_objects(Some(vm.heap.live_count() + 1));
        let error = vm
            .call_function(&function, std::slice::from_ref(&marker), None)
            .expect_err("arguments allocation should fail after the call environment fills the cap");
        vm.set_max_heap_objects(None);
        assert_eq!(error.kind, crate::error::ErrorKind::Range);
        assert_eq!(vm.gc_pins.len(), baseline_pins);
        assert_eq!(
            vm.run("1 + 1").expect("VM should remain reusable"),
            Value::Number(2.0)
        );

        vm.unpin(marker_pin);
        vm.unpin(values_pin);
        vm.unpin(function_pin);
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
        var deepBoundCalled = deepBoundConstructor() === undefined;
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
    assert_eq!(vm.get_global("deepBoundCalled"), Value::Bool(true));
    assert_eq!(vm.get_global("deepInvariantRead"), Value::Bool(true));
    assert_eq!(vm.get_global("deepDescriptorRead"), Value::Bool(true));
    assert_eq!(vm.get_global("deepTrapExtensible"), Value::Bool(true));
    assert_eq!(vm.get_global("deepTrapDescriptor"), Value::Bool(true));
    assert_eq!(vm.get_global("deepFreshDescriptor"), Value::Bool(true));
}

#[test]
fn constructor_traversals_consume_fuel_per_followed_wrapper_edge() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var fuelBoundConstructor = Object.bind(null).bind(null).bind(null);
        var fuelBoundNonConstructor = Math.abs.bind(null).bind(null);
        var fuelInnerBound = Object.bind(null);
        var fuelMixedConstructor = new Proxy(fuelInnerBound, {}).bind(null);
        var fuelProxyConstructor = new Proxy(new Proxy(new Proxy(Object, {}), {}), {});
        var fuelRevocableConstructor = Proxy.revocable(Object, {});
        var fuelRevokedConstructor = fuelRevocableConstructor.proxy;
        fuelRevocableConstructor.revoke();
        "#,
    )
    .expect("constructor wrappers should initialize");
    let baseline_pins = vm.gc_pins.len();

    let bound = vm.get_global("fuelBoundConstructor");
    vm.set_fuel(Some(0));
    assert!(vm.is_constructor_value(&bound));
    assert!(!vm.is_constructor_value(&vm.get_global("fuelBoundNonConstructor")));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let mixed = vm.get_global("fuelMixedConstructor");
    vm.set_fuel(Some(2));
    let error = vm
        .constructor_realm(&mixed)
        .expect_err("Bound, Proxy, Bound should require three fuel units");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(3));
    assert_eq!(
        vm.constructor_realm(&mixed)
            .expect("mixed constructor Realm traversal should complete"),
        vm.global
    );
    assert_eq!(vm.fuel_remaining(), Some(0));

    let revoked = vm.get_global("fuelRevokedConstructor");
    vm.set_fuel(Some(0));
    assert!(vm.is_constructor_value(&revoked));
    let error = vm
        .constructor_realm(&revoked)
        .expect_err("GetFunctionRealm must reject a revoked Proxy before fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    vm.set_fuel(None);
}

#[test]
fn constructor_dispatch_is_metered_linear_and_roots_bound_arguments() {
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
    vm.run(
        r#"
        var dispatchBoundConstructor = Object.bind(null).bind(null).bind(null);
        var dispatchProxyConstructor = new Proxy(new Proxy(new Proxy(Object, {}), {}), {});
        var dispatchGetterCalls = 0;
        var dispatchApplyGetterCalls = 0;
        var dispatchThrowingGetterProxy = new Proxy(Object, {
          get construct() {
            dispatchGetterCalls += 1;
            throw new Error("construct getter should not run");
          },
          get apply() {
            dispatchApplyGetterCalls += 1;
            throw new Error("apply getter should not run");
          }
        });
        var dispatchEagerNewTarget = Object.bind(null).bind(null).bind(null);
        Object.defineProperty(dispatchEagerNewTarget, "prototype", { value: undefined });
        var dispatchFallbackRevocable = Proxy.revocable(Object, {});
        var dispatchRevokedFallbackNewTarget = dispatchFallbackRevocable.proxy.bind(null);
        Object.defineProperty(dispatchRevokedFallbackNewTarget, "prototype", {
          value: undefined
        });
        dispatchFallbackRevocable.revoke();
        var dispatchRevocableConstructor = Proxy.revocable(Object, {});
        var dispatchRevokedConstructor = dispatchRevocableConstructor.proxy;
        dispatchRevocableConstructor.revoke();
        "#,
    )
    .expect("dispatch wrappers should initialize");
    let baseline_pins = vm.gc_pins.len();
    let object_constructor = vm.get_global("Object");

    let throwing_getter = vm.get_global("dispatchThrowingGetterProxy");
    vm.set_fuel(Some(0));
    let error = vm
        .construct_with_new_target(&throwing_getter, &[], &object_constructor)
        .expect_err("fuel must abort before a live Proxy construct getter");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.get_global("dispatchGetterCalls"), Value::Number(0.0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let proxy = vm.get_global("dispatchProxyConstructor");
    vm.set_fuel(Some(2));
    let error = vm
        .construct_with_new_target(&proxy, &[], &object_constructor)
        .expect_err("three Proxy dispatch edges should exhaust two fuel units");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(Some(3));
    assert!(matches!(
        vm.construct_with_new_target(&proxy, &[], &object_constructor)
            .expect("three Proxy dispatch edges should fit three fuel units"),
        Value::Object(_)
    ));
    assert_eq!(vm.fuel_remaining(), Some(0));

    let bound = vm.get_global("dispatchBoundConstructor");
    vm.set_fuel(Some(2));
    let error = vm
        .construct_with_new_target(&bound, &[], &object_constructor)
        .expect_err("three Bound dispatch edges should exhaust two fuel units");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(Some(3));
    assert!(matches!(
        vm.construct_with_new_target(&bound, &[], &object_constructor)
            .expect("three metered Bound dispatch edges should complete"),
        Value::Object(_)
    ));
    assert_eq!(vm.fuel_remaining(), Some(0));

    let array_constructor = vm.get_global("Array");
    let eager_new_target = vm.get_global("dispatchEagerNewTarget");
    vm.set_fuel(Some(2));
    let error = vm
        .construct_with_new_target(&array_constructor, &[], &eager_new_target)
        .expect_err("eager fallback Realm traversal should exhaust two fuel units");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert!(vm.pending_new_target.is_none());
    assert!(vm.pending_new_target_prototype.is_none());

    vm.set_fuel(Some(3));
    assert!(matches!(
        vm.construct_with_new_target(&array_constructor, &[], &eager_new_target)
            .expect("eager fallback Realm should traverse each Bound edge once"),
        Value::Object(_)
    ));
    assert_eq!(vm.fuel_remaining(), Some(0));

    let revoked_fallback = vm.get_global("dispatchRevokedFallbackNewTarget");
    vm.set_fuel(Some(1));
    let error = vm
        .construct_with_new_target(
            &array_constructor,
            &[Value::Number(-1.0)],
            &revoked_fallback,
        )
        .expect_err("fallback Realm failure must precede Array argument validation");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let revoked = vm.get_global("dispatchRevokedConstructor");
    vm.set_fuel(Some(0));
    let error = vm
        .construct_with_new_target(&revoked, &[], &object_constructor)
        .expect_err("revoked Proxy construction should fail before fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(None);
    let oversized_value = Value::Object(
        vm.new_object()
            .expect("oversized argument fixture should allocate"),
    );
    let oversized_args = vec![
        oversized_value.clone();
        crate::builtins::call_arguments::MAX_MATERIALIZED_CALL_ARGUMENTS + 1
    ];
    let pin_capacity_before_cap_checks = vm.gc_pins.capacity();
    let error = vm
        .construct_with_new_target(&Value::Undefined, &oversized_args, &object_constructor)
        .expect_err("constructor validation must precede the argument cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.capacity(), pin_capacity_before_cap_checks);
    let error = vm
        .construct_with_new_target(&object_constructor, &oversized_args, &Value::Undefined)
        .expect_err("newTarget validation must precede the argument cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.capacity(), pin_capacity_before_cap_checks);
    let error = vm
        .construct_with_new_target(&object_constructor, &oversized_args, &object_constructor)
        .expect_err("constructor argument materialization must enforce the sandbox cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.gc_pins.capacity(), pin_capacity_before_cap_checks);
    let error = vm
        .call_function(&Value::Undefined, &oversized_args, None)
        .expect_err("callability validation must precede the argument cap");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.capacity(), pin_capacity_before_cap_checks);
    let error = vm
        .call_function(&object_constructor, &oversized_args, None)
        .expect_err("direct call arguments must enforce the sandbox cap before pin growth");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.gc_pins.capacity(), pin_capacity_before_cap_checks);

    let bind = vm
        .get_property(&throwing_getter, "bind")
        .expect("Proxy constructor should inherit Function.prototype.bind");
    let layer_len = crate::builtins::call_arguments::MAX_MATERIALIZED_CALL_ARGUMENTS / 2 + 1;
    let mut layer_bind_args = Vec::with_capacity(layer_len + 1);
    layer_bind_args.push(Value::Undefined);
    layer_bind_args.extend(std::iter::repeat_n(oversized_value, layer_len));
    let inner_bound = vm
        .call_function(&bind, &layer_bind_args, Some(throwing_getter.clone()))
        .expect("one bounded argument layer should fit the call cap");
    let oversized_bound = vm
        .call_function(&bind, &layer_bind_args, Some(inner_bound))
        .expect("a second bounded layer should defer the combined cap check");
    vm.set_fuel(Some(2));
    let error = vm
        .construct_with_new_target(&oversized_bound, &[], &object_constructor)
        .expect_err("Bound argument overflow must precede the target Proxy getter");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.get_global("dispatchGetterCalls"), Value::Number(0.0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    vm.set_fuel(Some(2));
    let error = vm
        .call_function(&oversized_bound, &[], None)
        .expect_err("Bound call argument overflow must precede the target Proxy getter");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(
        vm.get_global("dispatchApplyGetterCalls"),
        Value::Number(0.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    drop(oversized_bound);
    drop(layer_bind_args);
    vm.clear_kept_objects();
    vm.gc();
    vm.set_fuel(None);

    assert_eq!(
        vm.run(
            r#"
            function DirectTarget() {
              this.args = Array.from(arguments).join(",");
              this.newTargetMatches = new.target === DirectTarget;
            }
            var directResult = new (
              DirectTarget.bind(null, "inner").bind(null, "outer")
            )("call");

            function ProxyTarget(first, second, third) {
              forceGc();
              this.args = first.label + "," + second.label + "," + third.label;
              this.newTargetMatches = new.target === forwardingConstructor;
            }
            var constructLog = [];
            var constructHandler = {};
            Object.defineProperty(constructHandler, "construct", {
              get: function () {
                constructLog.push("get");
                forceGc();
                return function (target, args, newTarget) {
                  constructLog.push("trap:" + args.length);
                  forceGc();
                  return Reflect.construct(target, args, newTarget);
                };
              }
            });
            var forwardingConstructor = new Proxy(ProxyTarget, constructHandler);
            function constructThroughTemporaryBounds() {
              return new (
                forwardingConstructor
                  .bind(null, { label: "inner" })
                  .bind(null, { label: "outer" })
              )({ label: "call" });
            }
            var proxyResult = constructThroughTemporaryBounds();
            var linearConstructor = Array;
            for (var i = 0; i < 4096; i += 1) {
              linearConstructor = linearConstructor.bind(null, i);
            }
            var linearResult = new linearConstructor(4096);
            var linearOrder =
              linearResult.length === 4097 &&
              linearResult[0] === 0 &&
              linearResult[2048] === 2048 &&
              linearResult[4095] === 4095 &&
              linearResult[4096] === 4096;
            directResult.args + ":" + directResult.newTargetMatches + "|" +
              proxyResult.args + ":" + proxyResult.newTargetMatches + "|" +
              constructLog.join(",") + "|" + linearOrder;
            "#,
        )
        .expect("Bound and Proxy construction should preserve order across GC"),
        Value::String(Arc::from(
            "inner,outer,call:true|inner,outer,call:true|get,trap:3|true"
        ))
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn bound_call_dispatch_is_metered_linear_and_roots_arguments() {
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
    vm.register_fn(
        "boundCallTarget",
        |vm, args, this| {
            vm.gc();
            let mut labels = Vec::with_capacity(4);
            labels.push(vm.get_property(&this.unwrap_or(Value::Undefined), "label")?);
            for argument in args {
                labels.push(vm.get_property(argument, "label")?);
            }
            let labels = labels
                .into_iter()
                .map(|value| match value {
                    Value::String(label) => Ok(label),
                    _ => Err(crate::error::Error::type_err("missing test label")),
                })
                .collect::<crate::error::Result<Vec<_>>>()?;
            Ok(Value::String(Arc::from(
                format!("{}|{},{},{}", labels[0], labels[1], labels[2], labels[3]).as_str(),
            )))
        },
        3,
    )
    .expect("Bound call target should register");
    vm.run(
        r#"
        var boundCallInner = boundCallTarget.bind(
          { label: "inner-this" }, { label: "inner" }
        );
        var boundCallProxy = new Proxy(boundCallInner, {});
        var boundCallOuter = boundCallProxy.bind(
          { label: "outer-this" }, { label: "outer" }
        );
        var boundCallSentinel = {};
        var boundCallAbruptHandler = {};
        Object.defineProperty(boundCallAbruptHandler, "apply", {
          get: function () { forceGc(); throw boundCallSentinel; }
        });
        var boundCallAbrupt = new Proxy(function () {}, boundCallAbruptHandler)
          .bind(null, { kept: true });
        "#,
    )
    .expect("Bound call fixtures should initialize");
    let baseline_pins = vm.gc_pins.len();
    let baseline_pin_capacity = vm.gc_pins.capacity();
    let error = vm
        .try_reserve_gc_pins(usize::MAX)
        .expect_err("impossible root reservations must remain catchable");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.gc_pins.capacity(), baseline_pin_capacity);
    let outer = vm.get_global("boundCallOuter");
    let call_arg = Value::Object(
        vm.new_object()
            .expect("temporary call argument should allocate"),
    );
    vm.set_property(&call_arg, "label", Value::String(Arc::from("call")))
        .expect("temporary call argument should be labelled");

    vm.fail_next_gc_pin_reservation = true;
    let error = crate::builtins::make_value_array_in_current_realm(&mut vm, vec![call_arg.clone()])
        .expect_err("argument-array root reservation failure must remain catchable");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert!(!vm.fail_next_gc_pin_reservation);
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let rooted_item_count = 4096;
    let root_heavy_array = crate::builtins::make_value_array_in_current_realm(
        &mut vm,
        vec![call_arg.clone(); rooted_item_count],
    )
    .expect("argument-array root reservation should remain fallible and balanced");
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert!(vm.gc_pins.capacity() > baseline_pins + rooted_item_count);
    let array_pin = vm.pin(&root_heavy_array);
    vm.gc();
    assert_eq!(
        vm.get_property(
            &root_heavy_array,
            (rooted_item_count - 1).to_string().as_str()
        )
        .expect("rooted argument-array tail should survive collection"),
        call_arg
    );
    vm.unpin(array_pin);
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(Some(2));
    let error = vm
        .call_function(&outer, std::slice::from_ref(&call_arg), None)
        .expect_err("Bound, Proxy, Bound call should require three fuel units");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(Some(3));
    assert_eq!(
        vm.call_function(&outer, &[call_arg], None)
            .expect("exact wrapper fuel should complete the call"),
        Value::String(Arc::from("inner-this|inner,outer,call"))
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(None);
    assert_eq!(
        vm.run(
            r#"
            var boundCallSameError = false;
            try { boundCallAbrupt(); }
            catch (error) { boundCallSameError = error === boundCallSentinel; }
            boundCallSameError;
            "#,
        )
        .expect("abrupt apply getter should remain catchable"),
        Value::Bool(true)
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn instanceof_is_iterative_rooted_metered_and_realm_correct() {
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
    vm.register_fn(
        "failHasInstanceContinuationReserve",
        |vm, _, _| {
            vm.fail_has_instance_continuation_reservation = true;
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("root-reservation failure hook should register");
    let initial_pins = vm.gc_pins.len();
    let initial_contexts = vm.execution_contexts.len();
    let initial_call_depth = vm.active_native_call_depth;
    vm.run(
        r#"
        function InstanceofTarget() {}
        var instanceofValue = Object.create(InstanceofTarget.prototype);
        var instanceofProxyValue = new Proxy(instanceofValue, {});
        var revokedInstanceofValue = Proxy.revocable(instanceofValue, {});
        var instanceofRevokedValue = revokedInstanceofValue.proxy;
        revokedInstanceofValue.revoke();
        var fuelBoundInstanceof = InstanceofTarget.bind(null).bind(null).bind(null);
        var fuelGetterCalls = 0;
        function FuelGetterTarget() {}
        Object.defineProperty(FuelGetterTarget, Symbol.hasInstance, {
          get: function () {
            fuelGetterCalls += 1;
            return function () { return true; };
          }
        });
        var fuelGetterBound = FuelGetterTarget.bind(null);
        var transparentDefaultHandler = new Proxy(
          Function.prototype[Symbol.hasInstance], {}
        );
        var defaultHasInstanceIntrinsic =
          Function.prototype[Symbol.hasInstance];
        var gcTransparentDefaultHandler = new Proxy(
          defaultHasInstanceIntrinsic,
          new Proxy({}, {
            get: function () {
              forceGc();
              return undefined;
            }
          })
        );
        var trappedDefaultHandler = new Proxy(
          defaultHasInstanceIntrinsic,
          { apply: Reflect.apply }
        );
        var realDeepBoundInstanceof = InstanceofTarget;
        for (var i = 0; i < 4096; i += 1) {
          realDeepBoundInstanceof = realDeepBoundInstanceof.bind(null);
        }
        forceGc();
        var realDeepBoundResult =
          instanceofValue instanceof realDeepBoundInstanceof;

        var gcHasInstanceTarget = {};
        Object.defineProperty(gcHasInstanceTarget, Symbol.hasInstance, {
          get: function () {
            forceGc();
            return function (value) {
              forceGc();
              return value.marker === 41;
            };
          }
        });
        var gcHasInstanceResult = { marker: 41 } instanceof gcHasInstanceTarget;

        function FreshPrototypeBase() {}
        var FreshPrototypeTarget = new Proxy(FreshPrototypeBase, {
          get: function (target, key, receiver) {
            if (key === "prototype") return {};
            return Reflect.get(target, key, receiver);
          }
        });
        var freshPrototypeValue = new Proxy({}, {
          getPrototypeOf: function () {
            forceGc();
            return {};
          }
        });
        var freshPrototypeResult = freshPrototypeValue instanceof FreshPrototypeTarget;

        var instanceofSentinel = {};
        var abruptHasInstanceTarget = {};
        Object.defineProperty(abruptHasInstanceTarget, Symbol.hasInstance, {
          get: function () {
            forceGc();
            throw instanceofSentinel;
          }
        });
        var abruptHasInstanceIdentity = false;
        try { ({} instanceof abruptHasInstanceTarget); }
        catch (error) { abruptHasInstanceIdentity = error === instanceofSentinel; }

        var other = $262.createRealm().global;
        other.eval("globalThis.ForeignTarget = function () {}; ForeignTarget.prototype = 1;");
        var realmBoundInstanceof = other.ForeignTarget.bind(null);
        Object.setPrototypeOf(realmBoundInstanceof, Function.prototype);
        var foreignInstanceofError = false;
        try { ({} instanceof realmBoundInstanceof); }
        catch (error) {
          foreignInstanceofError =
            error instanceof other.TypeError && !(error instanceof TypeError);
        }

        var foreignDefaultHasInstance = other.eval(
          "Function.prototype[Symbol.hasInstance]"
        );
        var reserveFailureHandler = new Proxy(foreignDefaultHasInstance, {});
        function ReserveFailureTarget() {}
        Object.defineProperty(ReserveFailureTarget, Symbol.hasInstance, {
          value: reserveFailureHandler
        });
        var ReserveFailureBound = ReserveFailureTarget.bind(null);
        var foreignReserveError = false;
        failHasInstanceContinuationReserve();
        try { ({} instanceof ReserveFailureBound); }
        catch (error) {
          foreignReserveError =
            error instanceof other.RangeError && !(error instanceof RangeError);
        }
        "#,
    )
    .expect("instanceof fixtures should initialize");
    assert_eq!(vm.gc_pins.len(), initial_pins);
    assert_eq!(vm.execution_contexts.len(), initial_contexts);
    assert_eq!(vm.active_native_call_depth, initial_call_depth);
    assert_eq!(vm.get_global("gcHasInstanceResult"), Value::Bool(true));
    assert_eq!(vm.get_global("freshPrototypeResult"), Value::Bool(false));
    assert_eq!(
        vm.get_global("abruptHasInstanceIdentity"),
        Value::Bool(true)
    );
    assert_eq!(vm.get_global("foreignInstanceofError"), Value::Bool(true));
    assert_eq!(vm.get_global("foreignReserveError"), Value::Bool(true));
    assert_eq!(vm.get_global("realDeepBoundResult"), Value::Bool(true));

    let constructor = vm.get_global("InstanceofTarget");
    let object = vm.get_global("instanceofValue");
    let baseline_pins = initial_pins;
    let baseline_contexts = initial_contexts;
    let baseline_call_depth = initial_call_depth;

    vm.fail_next_gc_pin_reservation = true;
    assert!(!vm
        .ordinary_has_instance(&constructor, &Value::Null)
        .expect("an ordinary callable with a primitive value must not reserve roots"));
    assert!(vm.fail_next_gc_pin_reservation);
    let error = vm
        .instanceof_operator(&object, &constructor)
        .expect_err("input root reservation failure must remain catchable");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert!(!vm.fail_next_gc_pin_reservation);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);
    assert_eq!(vm.active_native_call_depth, baseline_call_depth);

    vm.set_fuel(Some(0));
    let error = vm
        .ordinary_has_instance(&constructor, &object)
        .expect_err("an ordinary prototype edge must consume fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(Some(1));
    assert!(vm
        .ordinary_has_instance(&constructor, &object)
        .expect("one ordinary edge should complete instanceof"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let proxy_object = vm.get_global("instanceofProxyValue");
    vm.set_fuel(Some(1));
    assert!(vm
        .ordinary_has_instance(&constructor, &proxy_object)
        .expect("one Proxy edge must not receive a duplicate outer debit"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let revoked_object = vm.get_global("instanceofRevokedValue");
    vm.set_fuel(Some(0));
    let error = vm
        .ordinary_has_instance(&constructor, &revoked_object)
        .expect_err("revocation must precede Proxy edge fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let getter_bound = vm.get_global("fuelGetterBound");
    vm.set_fuel(Some(0));
    let error = vm
        .ordinary_has_instance(&getter_bound, &object)
        .expect_err("Bound fuel must precede target @@hasInstance lookup");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.get_global("fuelGetterCalls"), Value::Number(0.0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(None);
    assert!(vm
        .ordinary_has_instance(&getter_bound, &object)
        .expect("Bound forwarding should reach an own custom handler"));
    assert_eq!(vm.get_global("fuelGetterCalls"), Value::Number(1.0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    let bound = vm.get_global("fuelBoundInstanceof");
    vm.set_fuel(Some(5));
    let error = vm
        .ordinary_has_instance(&bound, &Value::Null)
        .expect_err("three Bound and inherited-method edges require six fuel units");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(Some(6));
    assert!(!vm
        .ordinary_has_instance(&bound, &Value::Null)
        .expect("exact Bound and method-lookup fuel should complete"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_fuel(None);
    let mut deep_bound = constructor.clone();
    for _ in 0..50_000 {
        deep_bound = direct_bound_function(&mut vm, &deep_bound);
    }
    assert!(vm.is_constructor_value(&deep_bound));
    assert!(vm
        .ordinary_has_instance(&deep_bound, &object)
        .expect("deep Bound instanceof must remain stack-safe and true"));
    let unrelated_object =
        Value::Object(vm.new_object().expect("unrelated object should allocate"));
    assert!(!vm
        .ordinary_has_instance(&deep_bound, &unrelated_object)
        .expect("deep Bound instanceof must remain stack-safe and false"));

    let transparent_handler = vm.get_global("transparentDefaultHandler");
    let mut wrapped_default_bound = constructor.clone();
    for _ in 0..10_000 {
        wrapped_default_bound = direct_bound_function(&mut vm, &wrapped_default_bound);
        set_direct_has_instance(&vm, &wrapped_default_bound, transparent_handler.clone());
    }
    assert!(vm
        .instanceof_operator(&object, &wrapped_default_bound)
        .expect("transparent wrapped default handlers must remain stack-safe"));

    let default_intrinsic = vm.get_global("defaultHasInstanceIntrinsic");
    let mut bound_default_bound = constructor.clone();
    for _ in 0..10_000 {
        bound_default_bound = direct_bound_function(&mut vm, &bound_default_bound);
        let handler = direct_bound_function_with_this(
            &mut vm,
            &default_intrinsic,
            bound_default_bound.clone(),
        );
        set_direct_has_instance(&vm, &bound_default_bound, handler);
    }
    assert!(vm
        .instanceof_operator(&object, &bound_default_bound)
        .expect("Bound-wrapped default handlers must remain stack-safe"));

    let gc_transparent_handler = vm.get_global("gcTransparentDefaultHandler");
    let mut gc_wrapped_default_bound = constructor;
    for _ in 0..128 {
        gc_wrapped_default_bound = direct_bound_function(&mut vm, &gc_wrapped_default_bound);
        set_direct_has_instance(
            &vm,
            &gc_wrapped_default_bound,
            gc_transparent_handler.clone(),
        );
    }
    assert!(vm
        .instanceof_operator(&object, &gc_wrapped_default_bound)
        .expect("wrapped default handler state must survive observable GC"));

    let trapped_handler = vm.get_global("trappedDefaultHandler");
    let mut trapped_default_bound = vm.get_global("InstanceofTarget");
    for _ in 0..2_000 {
        trapped_default_bound = direct_bound_function(&mut vm, &trapped_default_bound);
        set_direct_has_instance(&vm, &trapped_default_bound, trapped_handler.clone());
    }
    let error = vm
        .instanceof_operator(&object, &trapped_default_bound)
        .expect_err("re-entrant native apply traps must hit the VM call-depth guard");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);
    assert_eq!(vm.active_native_call_depth, baseline_call_depth);
}

#[test]
fn promise_settlement_precomputes_metered_handler_realms_transactionally() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var fulfilledResolve;
        var fulfilledPromise = new Promise(function (resolve) {
          fulfilledResolve = resolve;
        });
        var fulfilledHandler = function (value) { return value; };
        fulfilledHandler = fulfilledHandler.bind(null).bind(null).bind(null);
        fulfilledPromise.then(fulfilledHandler);

        var rejectedReject;
        var rejectedPromise = new Promise(function (resolve, reject) {
          rejectedReject = reject;
        });
        var rejectedHandler = function (reason) { return reason; };
        rejectedHandler = rejectedHandler.bind(null).bind(null);
        rejectedPromise.then(undefined, rejectedHandler);

        var multiLog = [];
        var multiResolve;
        var multiPromise = new Promise(function (resolve) { multiResolve = resolve; });
        var multiFirst = function () { multiLog.push("first"); }.bind(null);
        var multiSecond = function () { multiLog.push("second"); };
        multiSecond = multiSecond.bind(null).bind(null).bind(null);
        multiPromise.then(multiFirst);
        multiPromise.then(multiSecond);

        var fallbackResolve;
        var fallbackPromise = new Promise(function (resolve) { fallbackResolve = resolve; });
        var fallbackHandlerRecord = Proxy.revocable(function (value) { return value; }, {});
        fallbackPromise.then(fallbackHandlerRecord.proxy);
        fallbackHandlerRecord.revoke();
        "#,
    )
    .expect("pending Promise handlers should initialize");
    let baseline_pins = vm.gc_pins.len();
    let baseline_jobs = vm.microtask_queue.len();

    let fulfilled = vm.get_global("fulfilledPromise");
    let Value::Object(fulfilled_idx) = fulfilled else {
        panic!("fulfilledPromise should be a Promise object");
    };
    let fulfilled_resolve = vm.get_global("fulfilledResolve");
    assert!(!vm.is_constructor_value(&fulfilled_resolve));
    let object_constructor = vm.get_global("Object");
    let error = vm
        .construct_with_new_target(&object_constructor, &[], &fulfilled_resolve)
        .expect_err("internal Promise resolvers must not expose [[Construct]]");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    vm.set_fuel(Some(2));
    let error = vm
        .call_function(
            &fulfilled_resolve,
            &[Value::Number(7.0)],
            Some(Value::Undefined),
        )
        .expect_err("three handler wrapper edges should exhaust two fuel units");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    let (status, handler_count) =
        promise_state_and_handler_count(&vm, &Value::Object(fulfilled_idx));
    assert!(status == PromiseStatus::Pending);
    assert_eq!(handler_count, 1);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::ResolveInRealm { promise, value, .. })
            if *promise == fulfilled_idx && *value == Value::Number(7.0)
    ));
    assert_eq!(vm.microtask_queue.len(), baseline_jobs + 1);
    vm.set_fuel(Some(1));
    vm.call_function(
        &fulfilled_resolve,
        &[Value::Number(70.0)],
        Some(Value::Undefined),
    )
    .expect("the original resolving function should remain one-shot after its Bound edge");
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.microtask_queue.len(), baseline_jobs + 1);

    vm.set_fuel(Some(3));
    assert!(vm
        .tick()
        .expect("the staged fulfillment should fit three fuel units"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    let (status, handler_count) =
        promise_state_and_handler_count(&vm, &Value::Object(fulfilled_idx));
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(handler_count, 0);
    assert_eq!(vm.microtask_queue.len(), baseline_jobs + 1);

    let rejected = vm.get_global("rejectedPromise");
    let Value::Object(rejected_idx) = rejected else {
        panic!("rejectedPromise should be a Promise object");
    };
    let rejected_reject = vm.get_global("rejectedReject");
    vm.set_fuel(Some(1));
    let error = vm
        .call_function(
            &rejected_reject,
            &[Value::String(Arc::from("reason"))],
            Some(Value::Undefined),
        )
        .expect_err("two rejection-handler edges should exhaust one fuel unit");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    let (status, handler_count) =
        promise_state_and_handler_count(&vm, &Value::Object(rejected_idx));
    assert!(status == PromiseStatus::Pending);
    assert_eq!(handler_count, 1);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::RejectInRealm {
            promise,
            reason: Value::String(reason),
            ..
        }) if *promise == rejected_idx && reason.as_ref() == "reason"
    ));

    vm.set_fuel(Some(2));
    assert!(vm
        .tick()
        .expect("the staged rejection should fit two fuel units"));
    let (status, handler_count) =
        promise_state_and_handler_count(&vm, &Value::Object(rejected_idx));
    assert!(status == PromiseStatus::Rejected);
    assert_eq!(handler_count, 0);
    assert_eq!(vm.microtask_queue.len(), baseline_jobs + 2);

    let multi = vm.get_global("multiPromise");
    let Value::Object(multi_idx) = multi else {
        panic!("multiPromise should be a Promise object");
    };
    let multi_resolve = vm.get_global("multiResolve");
    vm.set_fuel(Some(3));
    let error = vm
        .call_function(
            &multi_resolve,
            &[Value::Number(8.0)],
            Some(Value::Undefined),
        )
        .expect_err("a later handler Realm must abort settlement transactionally");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    let (status, handler_count) = promise_state_and_handler_count(&vm, &Value::Object(multi_idx));
    assert!(status == PromiseStatus::Pending);
    assert_eq!(handler_count, 2);
    assert_eq!(vm.microtask_queue.len(), baseline_jobs + 3);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::ResolveInRealm { promise, value, .. })
            if *promise == multi_idx && *value == Value::Number(8.0)
    ));

    vm.set_fuel(Some(4));
    assert!(vm
        .tick()
        .expect("the staged multi-handler settlement should fit exact fuel"));
    let (status, handler_count) = promise_state_and_handler_count(&vm, &Value::Object(multi_idx));
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(handler_count, 0);
    assert_eq!(vm.microtask_queue.len(), baseline_jobs + 4);

    let fallback = vm.get_global("fallbackPromise");
    let Value::Object(fallback_idx) = fallback else {
        panic!("fallbackPromise should be a Promise object");
    };
    let fallback_resolve = vm.get_global("fallbackResolve");
    vm.set_fuel(Some(1));
    vm.call_function(
        &fallback_resolve,
        &[Value::Number(9.0)],
        Some(Value::Undefined),
    )
    .expect(
        "revoked handler Realm lookup should use the current Realm fallback after its Bound edge",
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    let (status, handler_count) =
        promise_state_and_handler_count(&vm, &Value::Object(fallback_idx));
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(handler_count, 0);
    assert_eq!(vm.microtask_queue.len(), baseline_jobs + 5);
    vm.set_fuel(None);
    vm.run_microtasks()
        .expect("queued reactions should retain FIFO order after retry");
    assert_eq!(
        vm.run("multiLog.join(',')")
            .expect("multi-handler order should remain observable"),
        Value::String(Arc::from("first,second"))
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn promise_settlement_jobs_requeue_after_noncatchable_fuel_abort() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "recordReaction",
        |vm, args, _| {
            increment_global_counter(vm, "reactionCounter")?;
            Ok(args.first().cloned().unwrap_or(Value::Undefined))
        },
        1,
    )
    .expect("reaction counter hook should register");
    vm.register_fn(
        "recordThenGetter",
        |vm, _, _| {
            increment_global_counter(vm, "getterCounter")?;
            Ok(vm.get_global("getterThenFunction"))
        },
        0,
    )
    .expect("then getter counter hook should register");
    vm.register_fn(
        "settleGetterThen",
        |vm, args, _| {
            let thenable = args.first().cloned().unwrap_or(Value::Undefined);
            let marker = vm.get_property(&thenable, "marker")?;
            let resolve = args.get(1).cloned().unwrap_or(Value::Undefined);
            vm.call_function(&resolve, &[marker], Some(Value::Undefined))?;
            Ok(Value::Undefined)
        },
        2,
    )
    .expect("retained thenable hook should register");
    vm.register_fn(
        "rejectThenable",
        |vm, _, _| {
            increment_global_counter(vm, "thenableCounter")?;
            Err(crate::error::Error::type_err("thenable marker"))
        },
        2,
    )
    .expect("thenable counter hook should register");
    vm.register_fn(
        "callSuppliedResolve",
        |vm, args, _| {
            increment_global_counter(vm, "suppliedResolveCounter")?;
            let resolve = args.first().cloned().unwrap_or(Value::Undefined);
            vm.call_function(&resolve, &[Value::Number(17.0)], Some(Value::Undefined))?;
            Ok(Value::Undefined)
        },
        2,
    )
    .expect("supplied resolve hook should register");
    vm.register_fn(
        "callSuppliedReject",
        |vm, args, _| {
            increment_global_counter(vm, "suppliedRejectCounter")?;
            let reject = args.get(1).cloned().unwrap_or(Value::Undefined);
            vm.call_function(&reject, &[Value::Number(18.0)], Some(Value::Undefined))?;
            Ok(Value::Undefined)
        },
        2,
    )
    .expect("supplied reject hook should register");
    vm.run(
        r#"
        var externalDrainResolver;
        var externalDrainPromise = new Promise(function (resolve) {
          externalDrainResolver = resolve;
        });
        var externalOrder = [];
        function externalFirstHandler(value) {
          externalOrder.push("first");
          return value;
        }
        externalDrainPromise.then(externalFirstHandler.bind(null).bind(null));

        var externalFollowerResolver;
        var externalFollowerPromise = new Promise(function (resolve) {
          externalFollowerResolver = resolve;
        });
        externalFollowerPromise.then(function (value) {
          externalOrder.push("second");
          return value;
        });

        var externalTickResolver;
        var externalTickPromise = new Promise(function (resolve) {
          externalTickResolver = resolve;
        });
        externalTickPromise.then(Math.abs.bind(null).bind(null));

        var microtaskDrainPromise = new Promise(function () {});
        microtaskDrainPromise.then(Math.abs.bind(null).bind(null));

        var microtaskTickPromise = new Promise(function () {});
        microtaskTickPromise.then(undefined, Math.abs.bind(null).bind(null));

        var reactionCounter = { count: 0 };
        var reactionSourceResolve;
        var reactionSource = new Promise(function (resolve) {
          reactionSourceResolve = resolve;
        });
        var reactionDerived = reactionSource.then(recordReaction);
        reactionDerived.then(Math.abs.bind(null).bind(null));

        var getterCounter = { count: 0 };
        var getterResolve;
        var getterPromise = new Promise(function (resolve) { getterResolve = resolve; });
        getterPromise.then(Math.abs.bind(null).bind(null));
        var getterSentinel = new Promise(function () {});
        var getterResolution = { marker: 16 };
        var getterThenFunction = settleGetterThen.bind(null, getterResolution).bind(null);
        Object.defineProperty(getterResolution, "then", { get: recordThenGetter });

        var thenableCounter = { count: 0 };
        var thenableResolve;
        var thenablePromise = new Promise(function (resolve) {
          thenableResolve = resolve;
        });
        thenablePromise.then(undefined, Math.abs.bind(null).bind(null));
        var throwingThenable = { then: rejectThenable };

        var suppliedResolveCounter = { count: 0 };
        var suppliedResolveOuter;
        var suppliedResolvePromise = new Promise(function (resolve) {
          suppliedResolveOuter = resolve;
        });
        suppliedResolvePromise.then(Math.abs.bind(null).bind(null));
        var resolvingThenable = { then: callSuppliedResolve };

        var suppliedRejectCounter = { count: 0 };
        var suppliedRejectOuter;
        var suppliedRejectPromise = new Promise(function (resolve) {
          suppliedRejectOuter = resolve;
        });
        suppliedRejectPromise.then(undefined, Math.abs.bind(null).bind(null));
        var rejectingThenable = { then: callSuppliedReject };
        "#,
    )
    .expect("retryable Promise job fixtures should initialize");
    let baseline_pins = vm.gc_pins.len();

    let external_drain = vm.get_global("externalDrainPromise");
    let Value::Object(external_drain_idx) = external_drain else {
        panic!("externalDrainPromise should be a Promise object");
    };
    let external_drain_resolver = vm.get_global("externalDrainResolver");
    vm.external_jobs.lock().jobs.push_back(ExternalPromiseJob {
        resolve: external_drain_resolver.clone(),
        value: Value::Number(11.0),
    });
    vm.external_jobs.lock().jobs.push_back(ExternalPromiseJob {
        resolve: vm.get_global("externalFollowerResolver"),
        value: Value::Number(22.0),
    });
    vm.set_fuel(Some(1));
    let error = vm
        .run_microtasks()
        .expect_err("external drain job should transfer settlement ownership");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    {
        let jobs = vm.external_jobs.lock();
        assert_eq!(jobs.jobs.len(), 1);
        assert_eq!(
            jobs.jobs.front().map(|job| &job.value),
            Some(&Value::Number(22.0))
        );
    }
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::ResolveInRealm { promise, value, .. })
            if *promise == external_drain_idx && *value == Value::Number(11.0)
    ));
    let (status, handler_count) =
        promise_state_and_handler_count(&vm, &Value::Object(external_drain_idx));
    assert!(status == PromiseStatus::Pending);
    assert_eq!(handler_count, 1);
    assert_eq!(
        promise_state_and_result(&vm, Value::Object(external_drain_idx)).1,
        Value::Undefined
    );

    vm.set_fuel(Some(100));
    vm.run_microtasks()
        .expect("staged settlement should precede the next external job");
    assert!(vm.external_jobs.lock().jobs.is_empty());
    let (status, result) = promise_state_and_result(&vm, Value::Object(external_drain_idx));
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(result, Value::Number(11.0));
    assert_eq!(
        vm.run("externalOrder.join(',')")
            .expect("external reactions should preserve FIFO order"),
        Value::String(Arc::from("first,second"))
    );

    let external_tick = vm.get_global("externalTickPromise");
    let Value::Object(external_tick_idx) = external_tick else {
        panic!("externalTickPromise should be a Promise object");
    };
    vm.external_jobs.lock().jobs.push_back(ExternalPromiseJob {
        resolve: vm.get_global("externalTickResolver"),
        value: Value::Number(12.0),
    });
    vm.set_fuel(Some(1));
    let error = vm
        .tick()
        .expect_err("external tick job should transfer to staged settlement");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(vm.external_jobs.lock().jobs.is_empty());
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::ResolveInRealm { promise, value, .. })
            if *promise == external_tick_idx && *value == Value::Number(12.0)
    ));
    assert_eq!(
        promise_state_and_result(&vm, Value::Object(external_tick_idx)).1,
        Value::Undefined
    );
    vm.set_fuel(Some(2));
    assert!(vm
        .tick()
        .expect("refilled external tick settlement should run"));
    assert!(vm.external_jobs.lock().jobs.is_empty());
    let (status, result) = promise_state_and_result(&vm, Value::Object(external_tick_idx));
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(result, Value::Number(12.0));
    vm.set_fuel(Some(3));
    assert!(vm
        .tick()
        .expect("native reaction and its capability resolver should consume three Bound edges"));
    assert_eq!(vm.fuel_remaining(), Some(0));

    let microtask_drain = vm.get_global("microtaskDrainPromise");
    let Value::Object(microtask_drain_idx) = microtask_drain else {
        panic!("microtaskDrainPromise should be a Promise object");
    };
    vm.microtask_queue.push_back(Microtask::Resolve {
        promise: microtask_drain_idx,
        value: Value::Number(13.0),
    });
    vm.microtask_queue.push_back(Microtask::Resolve {
        promise: microtask_drain_idx,
        value: Value::Number(99.0),
    });
    vm.set_fuel(Some(1));
    let error = vm
        .run_microtasks()
        .expect_err("Resolve microtask should requeue after handler Realm Fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::Resolve { promise, value })
            if *promise == microtask_drain_idx && *value == Value::Number(13.0)
    ));
    assert!(matches!(
        vm.microtask_queue.back(),
        Some(Microtask::Resolve { promise, value })
            if *promise == microtask_drain_idx && *value == Value::Number(99.0)
    ));
    let (status, handler_count) =
        promise_state_and_handler_count(&vm, &Value::Object(microtask_drain_idx));
    assert!(status == PromiseStatus::Pending);
    assert_eq!(handler_count, 1);
    assert_eq!(
        promise_state_and_result(&vm, Value::Object(microtask_drain_idx)).1,
        Value::Undefined
    );
    vm.set_fuel(Some(5));
    vm.run_microtasks()
        .expect("refilled Resolve microtask should settle and drain reactions");
    assert_eq!(vm.fuel_remaining(), Some(0));
    let (status, result) = promise_state_and_result(&vm, Value::Object(microtask_drain_idx));
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(result, Value::Number(13.0));

    let microtask_tick = vm.get_global("microtaskTickPromise");
    let Value::Object(microtask_tick_idx) = microtask_tick else {
        panic!("microtaskTickPromise should be a Promise object");
    };
    vm.microtask_queue.push_back(Microtask::Reject {
        promise: microtask_tick_idx,
        reason: Value::Number(14.0),
    });
    vm.set_fuel(Some(1));
    let error = vm
        .tick()
        .expect_err("Reject microtask should requeue after handler Realm Fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::Reject { promise, .. }) if *promise == microtask_tick_idx
    ));
    assert_eq!(
        promise_state_and_result(&vm, Value::Object(microtask_tick_idx)).1,
        Value::Undefined
    );
    vm.set_fuel(Some(2));
    assert!(vm.tick().expect("refilled Reject microtask should settle"));
    let (status, result) = promise_state_and_result(&vm, Value::Object(microtask_tick_idx));
    assert!(status == PromiseStatus::Rejected);
    assert_eq!(result, Value::Number(14.0));
    vm.set_fuel(Some(3));
    assert!(vm
        .tick()
        .expect("native rejection reaction and resolver should consume three Bound edges"));
    assert_eq!(vm.fuel_remaining(), Some(0));

    let getter_resolve = vm.get_global("getterResolve");
    let getter_resolution = vm.get_global("getterResolution");
    let getter_promise = vm.get_global("getterPromise");
    let Value::Object(getter_promise_idx) = &getter_promise else {
        panic!("getterPromise should be a Promise object");
    };
    let getter_promise_idx = *getter_promise_idx;
    let getter_sentinel = vm.get_global("getterSentinel");
    let Value::Object(getter_sentinel_idx) = getter_sentinel else {
        panic!("getterSentinel should be a Promise object");
    };
    vm.microtask_queue.push_back(Microtask::Resolve {
        promise: getter_sentinel_idx,
        value: Value::Number(99.0),
    });
    vm.set_fuel(Some(1));
    let error = vm
        .call_function(
            &getter_resolve,
            std::slice::from_ref(&getter_resolution),
            Some(Value::Undefined),
        )
        .expect_err("post-Get fulfillment should preserve its observed then value");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::PromiseResolveAfterThen { .. })
    ));
    assert!(matches!(
        vm.microtask_queue.back(),
        Some(Microtask::Resolve { promise, value })
            if *promise == getter_sentinel_idx && *value == Value::Number(99.0)
    ));
    let (status, result) = promise_state_and_result(&vm, getter_promise.clone());
    assert!(status == PromiseStatus::Pending);
    assert_eq!(result, Value::Undefined);
    vm.set_fuel(None);
    vm.call_function(
        &getter_resolve,
        std::slice::from_ref(&getter_resolution),
        Some(Value::Undefined),
    )
    .expect("the original resolver must remain one-shot while continuation owns resolution");
    assert_eq!(
        vm.run("getterCounter.count")
            .expect("then getter count should remain observable"),
        Value::Number(1.0)
    );
    vm.run("getterResolution = undefined; getterThenFunction = undefined")
        .expect("the observed resolution should be retained only by queued state");
    vm.clear_kept_objects();
    vm.gc();
    vm.set_fuel(Some(100));
    vm.run_microtasks()
        .expect("refilled post-Get continuation should fulfill without a second Get");
    let (status, result) = promise_state_and_result(&vm, getter_promise);
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(result, Value::Number(16.0));

    let thenable_resolve = vm.get_global("thenableResolve");
    let throwing_thenable = vm.get_global("throwingThenable");
    let thenable_promise = vm.get_global("thenablePromise");
    let Value::Object(thenable_promise_idx) = &thenable_promise else {
        panic!("thenablePromise should be a Promise object");
    };
    let thenable_promise_idx = *thenable_promise_idx;
    vm.set_fuel(None);
    vm.call_function(
        &thenable_resolve,
        std::slice::from_ref(&throwing_thenable),
        Some(Value::Undefined),
    )
    .expect("thenable assimilation should enqueue its job");
    vm.microtask_queue.push_back(Microtask::Resolve {
        promise: thenable_promise_idx,
        value: Value::Number(99.0),
    });
    vm.set_fuel(Some(1));
    let error = vm
        .run_microtasks()
        .expect_err("post-thenable rejection should preserve only settlement");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::RejectInRealm { promise, .. }) if *promise == thenable_promise_idx
    ));
    assert!(matches!(
        vm.microtask_queue.back(),
        Some(Microtask::Resolve { promise, value })
            if *promise == thenable_promise_idx && *value == Value::Number(99.0)
    ));
    let (status, result) = promise_state_and_result(&vm, thenable_promise.clone());
    assert!(status == PromiseStatus::Pending);
    assert_eq!(result, Value::Undefined);
    vm.clear_kept_objects();
    vm.gc();
    vm.set_fuel(Some(16));
    vm.run_microtasks()
        .expect("refilled thenable rejection should settle without replay");
    let (status, _) = promise_state_and_result(&vm, thenable_promise);
    assert!(status == PromiseStatus::Rejected);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("thenableCounter.count")
            .expect("thenable call count should remain observable"),
        Value::Number(1.0)
    );

    let supplied_resolve_outer = vm.get_global("suppliedResolveOuter");
    let resolving_thenable = vm.get_global("resolvingThenable");
    let supplied_resolve_promise = vm.get_global("suppliedResolvePromise");
    let Value::Object(supplied_resolve_idx) = supplied_resolve_promise else {
        panic!("suppliedResolvePromise should be a Promise object");
    };
    vm.call_function(
        &supplied_resolve_outer,
        std::slice::from_ref(&resolving_thenable),
        Some(Value::Undefined),
    )
    .expect("resolver-calling thenable should enqueue its job");
    vm.microtask_queue.push_back(Microtask::Resolve {
        promise: supplied_resolve_idx,
        value: Value::Number(99.0),
    });
    vm.set_fuel(Some(1));
    let error = vm
        .run_microtasks()
        .expect_err("a supplied resolve call should transfer settlement ownership");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::ResolveInRealm { promise, value, .. })
            if *promise == supplied_resolve_idx && *value == Value::Number(17.0)
    ));
    assert!(matches!(
        vm.microtask_queue.back(),
        Some(Microtask::Resolve { promise, value })
            if *promise == supplied_resolve_idx && *value == Value::Number(99.0)
    ));
    let (status, result) = promise_state_and_result(&vm, Value::Object(supplied_resolve_idx));
    assert!(status == PromiseStatus::Pending);
    assert_eq!(result, Value::Undefined);
    vm.set_fuel(Some(16));
    vm.run_microtasks()
        .expect("refilled supplied resolve settlement should complete without replay");
    let (status, result) = promise_state_and_result(&vm, Value::Object(supplied_resolve_idx));
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(result, Value::Number(17.0));
    vm.set_fuel(None);
    assert_eq!(
        vm.run("suppliedResolveCounter.count")
            .expect("supplied resolve call count should remain observable"),
        Value::Number(1.0)
    );

    let supplied_reject_outer = vm.get_global("suppliedRejectOuter");
    let rejecting_thenable = vm.get_global("rejectingThenable");
    let supplied_reject_promise = vm.get_global("suppliedRejectPromise");
    let Value::Object(supplied_reject_idx) = supplied_reject_promise else {
        panic!("suppliedRejectPromise should be a Promise object");
    };
    vm.call_function(
        &supplied_reject_outer,
        std::slice::from_ref(&rejecting_thenable),
        Some(Value::Undefined),
    )
    .expect("rejecter-calling thenable should enqueue its job");
    vm.microtask_queue.push_back(Microtask::Resolve {
        promise: supplied_reject_idx,
        value: Value::Number(99.0),
    });
    vm.set_fuel(Some(1));
    let error = vm
        .run_microtasks()
        .expect_err("a supplied reject call should transfer settlement ownership");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::RejectInRealm {
            promise, reason, ..
        })
            if *promise == supplied_reject_idx && *reason == Value::Number(18.0)
    ));
    assert!(matches!(
        vm.microtask_queue.back(),
        Some(Microtask::Resolve { promise, value })
            if *promise == supplied_reject_idx && *value == Value::Number(99.0)
    ));
    let (status, result) = promise_state_and_result(&vm, Value::Object(supplied_reject_idx));
    assert!(status == PromiseStatus::Pending);
    assert_eq!(result, Value::Undefined);
    vm.set_fuel(Some(16));
    vm.run_microtasks()
        .expect("refilled supplied rejection should complete without replay");
    let (status, result) = promise_state_and_result(&vm, Value::Object(supplied_reject_idx));
    assert!(status == PromiseStatus::Rejected);
    assert_eq!(result, Value::Number(18.0));
    vm.set_fuel(None);
    assert_eq!(
        vm.run("suppliedRejectCounter.count")
            .expect("supplied reject call count should remain observable"),
        Value::Number(1.0)
    );

    let reaction_source_resolve = vm.get_global("reactionSourceResolve");
    let reaction_derived = vm.get_global("reactionDerived");
    let Value::Object(reaction_derived_idx) = &reaction_derived else {
        panic!("reactionDerived should be a Promise object");
    };
    let reaction_derived_idx = *reaction_derived_idx;
    vm.call_function(
        &reaction_source_resolve,
        &[Value::Number(15.0)],
        Some(Value::Undefined),
    )
    .expect("source settlement should enqueue its reaction without fuel");
    vm.microtask_queue.push_back(Microtask::Resolve {
        promise: reaction_derived_idx,
        value: Value::Number(99.0),
    });
    vm.set_fuel(Some(1));
    let error = vm
        .run_microtasks()
        .expect_err("post-handler derived settlement should preserve a continuation");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::ResolveInRealm { promise, value, .. })
            if *promise == reaction_derived_idx && *value == Value::Number(15.0)
    ));
    assert!(matches!(
        vm.microtask_queue.back(),
        Some(Microtask::Resolve { promise, value })
            if *promise == reaction_derived_idx && *value == Value::Number(99.0)
    ));
    assert_eq!(
        vm.run("reactionCounter.count")
            .expect_err("the exhausted VM should remain fuel-bounded")
            .kind,
        crate::error::ErrorKind::Fuel
    );
    let (status, handler_count) = promise_state_and_handler_count(&vm, &reaction_derived);
    assert!(status == PromiseStatus::Pending);
    assert_eq!(handler_count, 1);
    assert_eq!(
        promise_state_and_result(&vm, reaction_derived.clone()).1,
        Value::Undefined
    );

    vm.set_fuel(Some(5));
    vm.run_microtasks()
        .expect("refilled post-handler settlement should complete without replay");
    let (status, result) = promise_state_and_result(&vm, reaction_derived);
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(result, Value::Number(15.0));
    vm.set_fuel(None);
    assert_eq!(
        vm.run("reactionCounter.count")
            .expect("reaction count should remain observable"),
        Value::Number(1.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn custom_promise_capability_fuel_abort_is_not_replayed() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "customCapabilityHandler",
        |vm, _, _| {
            increment_global_counter(vm, "customHandlerCounter")?;
            Ok(Value::Number(19.0))
        },
        1,
    )
    .expect("custom handler hook should register");
    vm.register_fn(
        "customCapabilityResolve",
        |vm, _, _| {
            increment_global_counter(vm, "customResolveCounter")?;
            vm.consume_fuel()?;
            Ok(Value::Undefined)
        },
        1,
    )
    .expect("custom capability hook should register");
    vm.run(
        r#"
        var customHandlerCounter = { count: 0 };
        var customResolveCounter = { count: 0 };
        var customSource = Promise.resolve(1);
        var customOutput = new Promise(function () {});
        var customSentinel = new Promise(function () {});
        "#,
    )
    .expect("custom capability fixtures should initialize");
    let baseline_pins = vm.gc_pins.len();

    let custom_source = vm.get_global("customSource");
    let Value::Object(custom_source_idx) = custom_source else {
        panic!("customSource should be a Promise object");
    };
    let custom_output = vm.get_global("customOutput");
    let custom_sentinel = vm.get_global("customSentinel");
    let Value::Object(custom_sentinel_idx) = custom_sentinel else {
        panic!("customSentinel should be a Promise object");
    };
    let custom_resolve = vm.get_global("customCapabilityResolve");
    vm.microtask_queue.push_back(Microtask::Then {
        promise: custom_source_idx,
        on_fulfilled: vm.get_global("customCapabilityHandler"),
        on_rejected: Value::Undefined,
        derived: Some(crate::value::PromiseReactionCapability {
            promise: custom_output.clone(),
            resolve: custom_resolve.clone(),
            reject: custom_resolve,
        }),
        continuation: None,
        realm: Some(vm.global),
    });
    vm.microtask_queue.push_back(Microtask::Resolve {
        promise: custom_sentinel_idx,
        value: Value::Number(20.0),
    });

    vm.set_fuel(Some(0));
    let error = vm
        .run_microtasks()
        .expect_err("an arbitrary capability abort should propagate to the host");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.microtask_queue.len(), 1);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::Resolve { promise, value })
            if *promise == custom_sentinel_idx && *value == Value::Number(20.0)
    ));
    let (status, result) = promise_state_and_result(&vm, custom_output.clone());
    assert!(status == PromiseStatus::Pending);
    assert_eq!(result, Value::Undefined);

    vm.set_fuel(None);
    vm.run_microtasks()
        .expect("only the independent sentinel should remain queued");
    assert_eq!(
        vm.run("customHandlerCounter.count + ':' + customResolveCounter.count")
            .expect("custom capability counters should remain observable"),
        Value::String(Arc::from("1:1"))
    );
    let (status, result) = promise_state_and_result(&vm, custom_output);
    assert!(status == PromiseStatus::Pending);
    assert_eq!(result, Value::Undefined);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn promise_resolution_allocation_failure_retains_selected_rejection_after_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "recordAllocationThen",
        |vm, _, _| {
            increment_global_counter(vm, "allocationThenCounter")?;
            Ok(Value::Undefined)
        },
        2,
    )
    .expect("thenable call counter should register");
    vm.register_fn(
        "capHeapThen",
        |vm, _, _| {
            cap_heap_at_current_live_count(vm)?;
            Ok(vm.get_global("recordAllocationThen"))
        },
        0,
    )
    .expect("heap-cap getter should register");
    vm.run(
        r#"
        var allocationThenCounter = { count: 0 };
        var allocationFailureResolve;
        var allocationFailurePromise = new Promise(function (resolve) {
          allocationFailureResolve = resolve;
        });
        allocationFailurePromise.then(undefined, Math.abs.bind(null).bind(null));
        var allocationFailureThenable = {};
        Object.defineProperty(allocationFailureThenable, "then", { get: capHeapThen });
        "#,
    )
    .expect("allocation-failure Promise fixtures should initialize");
    let baseline_pins = vm.gc_pins.len();

    let promise = vm.get_global("allocationFailurePromise");
    let Value::Object(promise_idx) = &promise else {
        panic!("allocationFailurePromise should be a Promise object");
    };
    let promise_idx = *promise_idx;
    let resolver = vm.get_global("allocationFailureResolve");
    let thenable = vm.get_global("allocationFailureThenable");
    vm.set_fuel(Some(1));
    let error = vm
        .call_function(
            &resolver,
            std::slice::from_ref(&thenable),
            Some(Value::Undefined),
        )
        .expect_err("selected heap-limit rejection should transfer on Fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::RejectInRealm { promise, .. }) if *promise == promise_idx
    ));
    assert!(!matches!(
        vm.microtask_queue.front(),
        Some(Microtask::PromiseResolveAfterThen { .. })
    ));
    let (status, result) = promise_state_and_result(&vm, promise.clone());
    assert!(status == PromiseStatus::Pending);
    assert_eq!(result, Value::Undefined);

    vm.set_max_heap_objects(None);
    vm.clear_kept_objects();
    vm.gc();
    vm.set_fuel(Some(16));
    vm.run_microtasks()
        .expect("refilled settlement must retain the selected rejection phase");
    let (status, reason) = promise_state_and_result(&vm, promise);
    assert!(status == PromiseStatus::Rejected);
    assert_eq!(
        vm.get_property(&reason, "name")
            .expect("selected rejection should remain an Error"),
        Value::String(Arc::from("RangeError"))
    );
    vm.set_fuel(None);
    assert_eq!(
        vm.run("allocationThenCounter.count")
            .expect("the original then function should remain uncalled"),
        Value::Number(0.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn staged_promise_settlement_preserves_selected_realms() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var stagedForeign = $262.createRealm().global;
        var stagedForeignData = stagedForeign.eval(`
          (function () {
            var resolve;
            var promise = new Promise(function (r) { resolve = r; });
            return {
              promise: promise,
              resolve: resolve
            };
          })()
        `);
        var stagedThenChecks = [];
        function stagedThen(resolve) {
          stagedThenChecks.push(
            Object.getPrototypeOf(resolve) === Function.prototype
          );
          resolve(23);
        }
        var stagedResolution = {
          then: stagedThen.bind(null).bind(null)
        };

        var revokedForeignData = stagedForeign.eval(`
          (function () {
            var resolve;
            var promise = new Promise(function (r) { resolve = r; });
            return { promise: promise, resolve: resolve };
          })()
        `);
        var revokedHandlerRecord = Proxy.revocable(function (value) {
          return value;
        }, {});
        var revokedBoundHandler = revokedHandlerRecord.proxy.bind(null);
        var revokedDerived = revokedForeignData.promise.then(revokedBoundHandler);
        revokedHandlerRecord.revoke();
        "#,
    )
    .expect("cross-Realm staged Promise fixtures should initialize");
    let baseline_pins = vm.gc_pins.len();

    let foreign_data = vm.get_global("stagedForeignData");
    let foreign_resolver = vm
        .get_property(&foreign_data, "resolve")
        .expect("foreign resolver should be readable");
    let Value::Object(foreign_resolver_idx) = &foreign_resolver else {
        panic!("foreign resolver should be a function object");
    };
    let foreign_realm = vm.heap.with_obj(foreign_resolver_idx.0, |object| {
        let HeapObj::Function(function) = object else {
            panic!("foreign resolver should be a function");
        };
        function.closure
    });
    let staged_resolution = vm.get_global("stagedResolution");
    vm.set_fuel(Some(1));
    let error = vm
        .call_function(
            &foreign_resolver,
            std::slice::from_ref(&staged_resolution),
            Some(Value::Undefined),
        )
        .expect_err("foreign post-then Realm traversal should stage on Fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::PromiseResolveAfterThen { realm, .. }) if *realm == foreign_realm
    ));
    vm.set_fuel(Some(100));
    vm.run_microtasks()
        .expect("foreign post-then stage should use the selected thenable-job Realm");
    assert_eq!(
        vm.run("stagedThenChecks.join(',')")
            .expect("nested resolving-function Realm check should be observable"),
        Value::String(Arc::from("true"))
    );
    let foreign_promise = vm
        .get_property(&foreign_data, "promise")
        .expect("foreign Promise should be readable");
    let (status, result) = promise_state_and_result(&vm, foreign_promise);
    assert!(status == PromiseStatus::Fulfilled);
    assert_eq!(result, Value::Number(23.0));

    let revoked_data = vm.get_global("revokedForeignData");
    let revoked_resolver = vm
        .get_property(&revoked_data, "resolve")
        .expect("revoked-handler resolver should be readable");
    vm.set_fuel(Some(1));
    let error = vm
        .call_function(
            &revoked_resolver,
            &[Value::Number(24.0)],
            Some(Value::Undefined),
        )
        .expect_err("foreign handler preflight should transfer after the resolver Bound edge");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::ResolveInRealm { realm, .. }) if *realm == foreign_realm
    ));
    vm.set_fuel(Some(1));
    assert!(vm
        .tick()
        .expect("refilled staged settlement should use foreign fallback Realm"));
    assert!(matches!(
        vm.microtask_queue.front(),
        Some(Microtask::Then {
            realm: Some(realm),
            ..
        }) if *realm == foreign_realm
    ));
    vm.set_fuel(None);
    vm.run_microtasks()
        .expect("revoked handler reaction should finish through its derived Promise");
    assert_eq!(vm.gc_pins.len(), baseline_pins);
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
fn proxy_get_prototype_reservations_are_fallible_ordered_and_balanced() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "failNextRootReservation",
        |vm, _, _| {
            vm.fail_next_gc_pin_reservation = true;
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("root-reservation failure hook should register");
    vm.register_fn(
        "failNextGetPrototypeScratchReservation",
        |vm, _, _| {
            vm.fail_next_get_prototype_scratch_reservation = true;
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("scratch-reservation failure hook should register");
    vm.register_fn(
        "failGetPrototypeResultRootReservation",
        |vm, _, _| {
            vm.fail_get_prototype_reservation_site = Some(GetPrototypeReservationSite::ResultRoot);
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("result-root reservation failure hook should register");
    vm.register_fn(
        "failGetPrototypeExpectedRootReservation",
        |vm, _, _| {
            vm.fail_get_prototype_reservation_site =
                Some(GetPrototypeReservationSite::ExpectedRoot);
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("expected-root reservation failure hook should register");
    vm.register_fn(
        "deepExpectedPrototype",
        |vm, _, _| Ok(vm.object_proto.clone()),
        1,
    )
    .expect("deep getPrototypeOf trap should register");
    vm.run(
        r#"
        var reserveOrder = [];

        var targetReserveProxy = new Proxy({}, {
          get getPrototypeOf() {
            reserveOrder.push("target:get");
            return function () { return null; };
          }
        });

        var trapReserveHandler = {};
        Object.defineProperty(trapReserveHandler, "getPrototypeOf", {
          get: function () {
            reserveOrder.push("trap:get");
            failNextRootReservation();
            return function () {
              reserveOrder.push("trap:call");
              return null;
            };
          }
        });
        var trapReserveProxy = new Proxy({}, trapReserveHandler);

        var resultReserveTarget = new Proxy({}, {
          get isExtensible() {
            reserveOrder.push("result:isExtensible");
            return Reflect.isExtensible;
          }
        });
        var resultReserveProxy = new Proxy(resultReserveTarget, {
          getPrototypeOf: function () {
            reserveOrder.push("result:call");
            failGetPrototypeResultRootReservation();
            return {};
          }
        });

        var nestedReserveBase = Object.preventExtensions({});
        var nestedReserveTarget = new Proxy(nestedReserveBase, {
          get isExtensible() {
            reserveOrder.push("nested:isExtensible:get");
            failNextRootReservation();
            return function () {
              reserveOrder.push("nested:isExtensible:call");
              return false;
            };
          }
        });
        var nestedReserveProxy = new Proxy(nestedReserveTarget, {
          getPrototypeOf: function () {
            reserveOrder.push("nested:getPrototypeOf:call");
            return Object.getPrototypeOf(nestedReserveBase);
          }
        });

        var scratchBase = Object.preventExtensions({});
        var scratchTarget = new Proxy(scratchBase, {
          isExtensible: function () {
            reserveOrder.push("scratch:isExtensible");
            failNextGetPrototypeScratchReservation();
            return false;
          },
          getPrototypeOf: function () {
            reserveOrder.push("scratch:targetGetPrototypeOf");
            return Object.getPrototypeOf(scratchBase);
          }
        });
        var scratchProxy = new Proxy(scratchTarget, {
          getPrototypeOf: function () {
            reserveOrder.push("scratch:outerGetPrototypeOf");
            return Object.getPrototypeOf(scratchBase);
          }
        });

        var continuationBase = Object.preventExtensions({});
        var continuationTarget = new Proxy(continuationBase, {
          isExtensible: function () {
            reserveOrder.push("continuation:isExtensible");
            failGetPrototypeExpectedRootReservation();
            return false;
          },
          getPrototypeOf: function () {
            reserveOrder.push("continuation:targetGetPrototypeOf");
            return Object.getPrototypeOf(continuationBase);
          }
        });
        var continuationProxy = new Proxy(continuationTarget, {
          getPrototypeOf: function () {
            reserveOrder.push("continuation:outerGetPrototypeOf");
            return Object.getPrototypeOf(continuationBase);
          }
        });

        var lateExpectedBase = Object.preventExtensions({});
        var lateExpectedPrototype = Object.getPrototypeOf(lateExpectedBase);
        var lateExpectedCalls = 0;
        var lateExpectedProxy = lateExpectedBase;
        for (var lateIndex = 0; lateIndex < 8; lateIndex += 1) {
          lateExpectedProxy = new Proxy(lateExpectedProxy, {
            getPrototypeOf: function () {
              lateExpectedCalls += 1;
              if (lateExpectedCalls === 5) {
                failGetPrototypeExpectedRootReservation();
              }
              return lateExpectedPrototype;
            }
          });
        }

        var nullExpectedBase = Object.preventExtensions(Object.create(null));
        var nullExpectedProxy = new Proxy(nullExpectedBase, {
          getPrototypeOf: function () { return null; }
        });

        var getPrototypeOtherRealm = $262.createRealm().global;
        var foreignReserveHandler = {};
        Object.defineProperty(foreignReserveHandler, "getPrototypeOf", {
          get: function () {
            failNextRootReservation();
            return function () { return null; };
          }
        });
        var foreignReserveProxy = new Proxy({}, foreignReserveHandler);
        var foreignGetPrototypeReserveError = false;
        try {
          getPrototypeOtherRealm.Object.getPrototypeOf(foreignReserveProxy);
        } catch (error) {
          foreignGetPrototypeReserveError =
            error instanceof getPrototypeOtherRealm.RangeError &&
            !(error instanceof RangeError);
        }

        var deepGetPrototypeBase = Object.preventExtensions({});
        var deepGetPrototypeProxy = deepGetPrototypeBase;
        var deepGetPrototypeHandler = {
          getPrototypeOf: deepExpectedPrototype
        };
        for (var i = 0; i < 1024; i += 1) {
          deepGetPrototypeProxy = new Proxy(
            deepGetPrototypeProxy,
            deepGetPrototypeHandler
          );
        }

        var extensibleOrder = [];
        var inconsistentExtensible = new Proxy(
          new Proxy({}, {
            isExtensible: function () {
              extensibleOrder.push("inner");
              return true;
            }
          }),
          {
            isExtensible: function () {
              extensibleOrder.push("outer");
              return false;
            }
          }
        );

        var delayedExtensibleSentinel = {};
        var delayedExtensibleOrder = [];
        var delayedExtensible = new Proxy(
          new Proxy(
            new Proxy({}, {
              isExtensible: function () {
                delayedExtensibleOrder.push("throw");
                throw delayedExtensibleSentinel;
              }
            }),
            {
              isExtensible: function () {
                delayedExtensibleOrder.push("middle");
                return true;
              }
            }
          ),
          {
            isExtensible: function () {
              delayedExtensibleOrder.push("outer");
              return false;
            }
          }
        );
        var delayedExtensibleSentinelWins = false;
        try { Object.isExtensible(delayedExtensible); }
        catch (error) {
          delayedExtensibleSentinelWins = error === delayedExtensibleSentinel;
        }
        "#,
    )
    .expect("getPrototypeOf reservation fixtures should initialize");

    let baseline_pins = vm.gc_pins.len();
    let baseline_contexts = vm.execution_contexts.len();
    assert_eq!(
        vm.get_global("foreignGetPrototypeReserveError"),
        Value::Bool(true)
    );
    assert_eq!(
        vm.get_global("delayedExtensibleSentinelWins"),
        Value::Bool(true)
    );
    assert_eq!(
        vm.run("delayedExtensibleOrder.join('|')")
            .expect("delayed extensibility order should remain readable"),
        Value::String(Arc::from("outer|middle|throw"))
    );

    let target_reserve_proxy = vm.get_global("targetReserveProxy");
    vm.fail_next_gc_pin_reservation = true;
    assert!(!vm
        .is_extensible(&Value::Undefined)
        .expect("primitive IsExtensible must not reserve roots"));
    assert!(vm.fail_next_gc_pin_reservation);
    let error = vm
        .is_extensible(&target_reserve_proxy)
        .expect_err("IsExtensible input-root reservation must be fallible");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert!(!vm.fail_next_gc_pin_reservation);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);

    vm.gc_pin_reservation_failure_countdown = Some(1);
    vm.set_fuel(Some(1));
    let error = vm
        .is_extensible(&target_reserve_proxy)
        .expect_err("IsExtensible target/handler reservation must follow fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);
    vm.set_fuel(None);

    vm.fail_next_gc_pin_reservation = true;
    let error = vm
        .get_prototype_of(&target_reserve_proxy)
        .expect_err("input-root reservation must be fallible");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);
    assert_eq!(
        vm.run("reserveOrder.join('|')")
            .expect("input failure order should remain readable"),
        Value::String(Arc::from(""))
    );

    vm.gc_pin_reservation_failure_countdown = Some(1);
    vm.set_fuel(Some(1));
    let error = vm
        .get_prototype_of(&target_reserve_proxy)
        .expect_err("target/handler reservation must follow the edge debit");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("reserveOrder.join('|')")
            .expect("target reservation order should remain readable"),
        Value::String(Arc::from(""))
    );

    for (name, expected_log) in [
        ("trapReserveProxy", "trap:get"),
        ("resultReserveProxy", "trap:get|result:call"),
        (
            "nestedReserveProxy",
            "trap:get|result:call|nested:getPrototypeOf:call|nested:isExtensible:get",
        ),
        (
            "scratchProxy",
            "trap:get|result:call|nested:getPrototypeOf:call|nested:isExtensible:get|scratch:outerGetPrototypeOf|scratch:isExtensible",
        ),
        (
            "continuationProxy",
            "trap:get|result:call|nested:getPrototypeOf:call|nested:isExtensible:get|scratch:outerGetPrototypeOf|scratch:isExtensible|continuation:outerGetPrototypeOf|continuation:isExtensible",
        ),
    ] {
        let proxy = vm.get_global(name);
        let error = match vm.get_prototype_of(&proxy) {
            Err(error) => error,
            Ok(value) => panic!("{name} unexpectedly returned {value:?}"),
        };
        assert_eq!(error.kind, crate::error::ErrorKind::Range, "{name}");
        assert_eq!(vm.gc_pins.len(), baseline_pins, "{name}");
        assert_eq!(vm.execution_contexts.len(), baseline_contexts, "{name}");
        assert_eq!(
            vm.run("reserveOrder.join('|')")
                .expect("reservation order should remain readable"),
            Value::String(Arc::from(expected_log)),
            "{name}"
        );
    }

    let late_expected = vm.get_global("lateExpectedProxy");
    let error = vm
        .get_prototype_of(&late_expected)
        .expect_err("a later expected-root failure must release earlier deferred roots");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.get_global("lateExpectedCalls"), Value::Number(5.0));
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);

    let null_expected = vm.get_global("nullExpectedProxy");
    assert_eq!(
        vm.get_prototype_of(&null_expected)
            .expect("a deferred null prototype should not require an object root"),
        None
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);

    let deep = vm.get_global("deepGetPrototypeProxy");
    assert_eq!(
        vm.get_prototype_of(&deep)
            .expect("deep validating chain should grow scratch fallibly"),
        Some(vm.object_proto.clone())
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);

    let inconsistent = vm.get_global("inconsistentExtensible");
    let error = vm
        .is_extensible(&inconsistent)
        .expect_err("all nested trap results must be observed before validation");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(
        vm.run("extensibleOrder.join('|')")
            .expect("extensibility order should remain readable"),
        Value::String(Arc::from("outer|inner"))
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);
    assert_eq!(vm.execution_contexts.len(), baseline_contexts);
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
fn proxy_set_and_receiver_define_root_values_and_restore_pin_depth() {
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
        var rootedSetTarget = {};
        var rootedSetHandler = {};
        Object.defineProperty(rootedSetHandler, "set", {
          get: function () {
            forceGc();
            return function (target, key, value, receiver) {
              forceGc();
              return Reflect.set(target, key, value, receiver);
            };
          }
        });
        var rootedSetProxy = new Proxy(rootedSetTarget, rootedSetHandler);

        var abruptSetGetterHandler = {};
        Object.defineProperty(abruptSetGetterHandler, "set", {
          get: function () { forceGc(); throw { marker: 201 }; }
        });
        var abruptSetGetterProxy = new Proxy({}, abruptSetGetterHandler);
        var abruptSetTrapProxy = new Proxy({}, {
          set: function () { forceGc(); throw { marker: 202 }; }
        });
        var abruptSetInvariantTarget = new Proxy({}, {
          getOwnPropertyDescriptor: function () {
            forceGc();
            throw { marker: 203 };
          }
        });
        var abruptSetInvariantProxy = new Proxy(abruptSetInvariantTarget, {
          set: function () { return true; }
        });

        var abruptReceiverSource = Object.create(null);
        var abruptReceiverGetProxy = new Proxy({}, {
          getOwnPropertyDescriptor: function () {
            forceGc();
            throw { marker: 204 };
          }
        });
        var abruptReceiverDefineProxy = new Proxy({}, {
          defineProperty: function () {
            forceGc();
            throw { marker: 205 };
          }
        });
        var nonCallableReceiverTarget = { value: 0 };
        var nonCallableReceiver = new Proxy(nonCallableReceiverTarget, {
          defineProperty: {}
        });
        var nonCallableReceiverSource = { value: 1 };
        "#,
    )
    .expect("collecting Proxy set fixtures should initialize");

    vm.gc();
    let value_idx = vm
        .new_object()
        .expect("unrooted receiver value should allocate");
    vm.heap.with_obj(value_idx.0, |object| {
        object.props().lock().insert(
            crate::value::PropertyKey::from("marker"),
            crate::value::PropertyDescriptor::data(Value::Number(73.0)),
        );
    });
    let value = Value::Object(value_idx);
    let proxy = vm.get_global("rootedSetProxy");
    let baseline = vm.gc_pins.len();
    assert!(vm
        .try_set_property_with_receiver(&proxy, "rooted", value, &proxy)
        .expect("set value should survive trap lookup and call GC"));
    assert_eq!(vm.gc_pins.len(), baseline);
    assert_eq!(
        vm.run("rootedSetTarget.rooted.marker")
            .expect("stored receiver value should remain live"),
        Value::Number(73.0)
    );

    for (base_name, receiver_name, expected_marker) in [
        ("abruptSetGetterProxy", "abruptSetGetterProxy", 201.0),
        ("abruptSetTrapProxy", "abruptSetTrapProxy", 202.0),
        ("abruptSetInvariantProxy", "abruptSetInvariantProxy", 203.0),
        ("abruptReceiverSource", "abruptReceiverGetProxy", 204.0),
        ("abruptReceiverSource", "abruptReceiverDefineProxy", 205.0),
    ] {
        let base = vm.get_global(base_name);
        let receiver = vm.get_global(receiver_name);
        let baseline = vm.gc_pins.len();
        let error = vm
            .try_set_property_with_receiver(&base, "abrupt", Value::Number(1.0), &receiver)
            .expect_err("Proxy set/receiver define should preserve abrupt completion");
        assert_eq!(error.kind, crate::error::ErrorKind::User);
        assert_eq!(vm.gc_pins.len(), baseline);
        let thrown = error
            .thrown_value
            .clone()
            .expect("abrupt completion should retain its marker object");
        let thrown_pin = vm.pin(&thrown);
        assert_eq!(
            vm.get_property(&thrown, "marker")
                .expect("thrown marker should remain readable"),
            Value::Number(expected_marker)
        );
        vm.unpin(thrown_pin);
        assert_eq!(vm.gc_pins.len(), baseline);
    }

    vm.gc();
    vm.set_max_heap_objects(Some(vm.heap.live_count()));
    let source = vm.get_global("nonCallableReceiverSource");
    let receiver = vm.get_global("nonCallableReceiver");
    let baseline = vm.gc_pins.len();
    let error = vm
        .try_set_property_with_receiver(&source, "value", Value::Number(2.0), &receiver)
        .expect_err("GetMethod must reject before partial descriptor allocation");
    vm.set_max_heap_objects(None);
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn proxy_set_and_receiver_define_walks_consume_exact_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var deepSetBase = {};
        var deepSetProxy = deepSetBase;
        var transparentSetHandler = {};
        for (var i = 0; i < 100000; i += 1) {
          deepSetProxy = new Proxy(deepSetProxy, transparentSetHandler);
        }
        var receiverValueSource = { value: 0 };
        "#,
    )
    .expect("deep Proxy set fixtures should initialize");
    let proxy = vm.get_global("deepSetProxy");
    let source = vm.get_global("receiverValueSource");
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(299_999));
    let error = vm
        .try_set_property_with_receiver(&proxy, "value", Value::Number(41.0), &proxy)
        .expect_err("N Proxy Set, GetOwnProperty, and DefineOwnProperty walks need 3N fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(300_000));
    assert!(vm
        .try_set_property_with_receiver(&proxy, "value", Value::Number(41.0), &proxy)
        .expect("exactly 3N fuel should complete transparent assignment"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("deepSetBase.value")
            .expect("transparent assignment should reach the base"),
        Value::Number(41.0)
    );

    vm.set_fuel(Some(199_999));
    let error = vm
        .try_set_property_with_receiver(&source, "value", Value::Number(73.0), &proxy)
        .expect_err("receiver GetOwnProperty plus partial DefineOwnProperty need 2N fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(200_000));
    assert!(vm
        .try_set_property_with_receiver(&source, "value", Value::Number(73.0), &proxy)
        .expect("exactly 2N fuel should complete receiver value definition"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("deepSetBase.value")
            .expect("partial receiver definition should update the base"),
        Value::Number(73.0)
    );
}

#[test]
fn ordinary_property_walks_consume_exact_fuel_and_restore_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var ordinaryFuelSymbol = Symbol("ordinary-fuel");
        var ordinaryFuelRoot = { marker: 17, sink: 0 };
        ordinaryFuelRoot[ordinaryFuelSymbol] = 23;
        var ordinaryFuelLeaf = ordinaryFuelRoot;
        for (var i = 0; i < 5000; i += 1) {
          ordinaryFuelLeaf = Object.create(ordinaryFuelLeaf);
        }
        "#,
    )
    .expect("deep ordinary property fixtures should initialize");
    let leaf = vm.get_global("ordinaryFuelLeaf");
    let key = crate::value::PropertyKey::from("marker");
    let symbol_key = match vm.get_global("ordinaryFuelSymbol") {
        Value::Symbol(id) => crate::value::PropertyKey::Symbol(id),
        value => panic!("expected Symbol fixture, got {value:?}"),
    };
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(4_999));
    let error = vm
        .get_property(&leaf, "marker")
        .expect_err("N-1 fuel must abort an N-edge ordinary Get walk");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(4_999));
    let error = vm
        .get_property_by_key(&leaf, &symbol_key)
        .expect_err("N-1 fuel must abort an N-edge ordinary Symbol Get walk");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(5_000));
    assert_eq!(
        vm.get_property_by_key(&leaf, &symbol_key)
            .expect("exactly N fuel should complete ordinary Symbol Get"),
        Value::Number(23.0)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(5_000));
    assert_eq!(
        vm.get_property(&leaf, "marker")
            .expect("exactly N fuel should complete ordinary Get"),
        Value::Number(17.0)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(4_999));
    let error = vm
        .has_property_key(&leaf, &symbol_key)
        .expect_err("N-1 fuel must abort an N-edge ordinary Symbol HasProperty walk");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(5_000));
    assert!(vm
        .has_property_key(&leaf, &symbol_key)
        .expect("exactly N fuel should complete ordinary Symbol HasProperty"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(4_999));
    let error = vm
        .has_property_key(&leaf, &key)
        .expect_err("N-1 fuel must abort an N-edge ordinary HasProperty walk");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(4_999));
    let error = vm
        .try_set_property_key_with_receiver(&leaf, &symbol_key, Value::Number(47.0), &leaf)
        .expect_err("N-1 fuel must abort an N-edge ordinary Symbol Set walk");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(5_000));
    assert!(vm
        .try_set_property_key_with_receiver(&leaf, &symbol_key, Value::Number(47.0), &leaf,)
        .expect("exactly N fuel should complete ordinary Symbol Set"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(5_000));
    assert!(vm
        .has_property_key(&leaf, &key)
        .expect("exactly N fuel should complete ordinary HasProperty"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(4_999));
    let error = vm
        .try_set_property_with_receiver(&leaf, "sink", Value::Number(31.0), &leaf)
        .expect_err("N-1 fuel must abort an N-edge ordinary Set walk");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(5_000));
    assert!(vm
        .try_set_property_with_receiver(&leaf, "sink", Value::Number(31.0), &leaf)
        .expect("exactly N fuel should complete ordinary Set"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(None);
    assert_eq!(
        vm.get_property(&leaf, "sink")
            .expect("the deep inherited writable property should receive the write"),
        Value::Number(31.0)
    );
    assert_eq!(
        vm.get_property_by_key(&leaf, &symbol_key)
            .expect("the deep inherited Symbol property should receive the write"),
        Value::Number(47.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn inherited_proxy_trap_lookups_consume_exact_edge_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("propertyGetTrap", |_, _, _| Ok(Value::Number(37.0)), 3)
        .expect("native get trap should register");
    vm.register_fn("propertyTrueTrap", |_, _, _| Ok(Value::Bool(true)), 4)
        .expect("native truthy trap should register");
    vm.run(
        r#"
        function deepenPropertyHandler(root) {
          var handler = root;
          for (var i = 0; i < 100; i += 1) {
            handler = Object.create(handler);
          }
          return handler;
        }
        var propertyFuelSymbol = Symbol("property-fuel");
        var inheritedGetProxy = new Proxy(
          {},
          deepenPropertyHandler({ get: propertyGetTrap })
        );
        var inheritedHasProxy = new Proxy(
          {},
          deepenPropertyHandler({ has: propertyTrueTrap })
        );
        var inheritedSetProxy = new Proxy(
          {},
          deepenPropertyHandler({ set: propertyTrueTrap })
        );
        "#,
    )
    .expect("inherited Proxy trap fixtures should initialize");
    let get_proxy = vm.get_global("inheritedGetProxy");
    let has_proxy = vm.get_global("inheritedHasProxy");
    let set_proxy = vm.get_global("inheritedSetProxy");
    let symbol_key = match vm.get_global("propertyFuelSymbol") {
        Value::Symbol(id) => crate::value::PropertyKey::Symbol(id),
        value => panic!("expected Symbol fixture, got {value:?}"),
    };
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(99));
    let error = vm
        .get_property_by_key(&get_proxy, &symbol_key)
        .expect_err("outer Proxy plus M-1 handler edges require M fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(100));
    assert_eq!(
        vm.get_property_by_key(&get_proxy, &symbol_key)
            .expect("exact inherited get-trap fuel should complete"),
        Value::Number(37.0)
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(99));
    let error = vm
        .has_property_key(&has_proxy, &symbol_key)
        .expect_err("outer Proxy plus M-1 inherited has-trap edges require M fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(100));
    assert!(vm
        .has_property_key(&has_proxy, &symbol_key)
        .expect("exact inherited has-trap fuel should complete"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(99));
    let error = vm
        .try_set_property_key_with_receiver(&set_proxy, &symbol_key, Value::Number(1.0), &set_proxy)
        .expect_err("outer Proxy plus M-1 inherited set-trap edges require M fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(100));
    assert!(
        vm.try_set_property_key_with_receiver(
            &set_proxy,
            &symbol_key,
            Value::Number(1.0),
            &set_proxy,
        )
        .expect("exact inherited set-trap fuel should complete")
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn iterative_property_walks_root_values_across_gc_and_reject_ordinary_cycles() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.clear_kept_objects();
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");

    let ordinary = vm
        .run(
            r#"
            (function () {
              var root = {
                get value() {
                  forceGc();
                  return this.marker;
                }
              };
              var leaf = root;
              for (var i = 0; i < 5000; i += 1) {
                leaf = Object.create(leaf);
              }
              leaf.marker = 73;
              return leaf;
            })()
            "#,
        )
        .expect("unrooted ordinary fixture should initialize");
    let baseline = vm.gc_pins.len();
    assert_eq!(
        vm.get_property(&ordinary, "value")
            .expect("the receiver must survive GC in a deep inherited getter"),
        Value::Number(73.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    let proxy = vm
        .run(
            r#"
            (function () {
              var handlerRoot = {};
              Object.defineProperty(handlerRoot, "get", {
                get: function () {
                  forceGc();
                  return function (target, key) {
                    forceGc();
                    return target.marker;
                  };
                }
              });
              var handler = handlerRoot;
              for (var i = 0; i < 5000; i += 1) {
                handler = Object.create(handler);
              }
              return new Proxy({ marker: 89 }, handler);
            })()
            "#,
        )
        .expect("unrooted Proxy fixture should initialize");
    assert_eq!(
        vm.get_property(&proxy, "value")
            .expect("the Proxy target, handler, trap, and receiver must survive GC"),
        Value::Number(89.0)
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    assert_eq!(
        vm.run(
            r#"
            var severedTraversalBase = {};
            var severedTraversalWeak;
            var severedTraversalAlive = false;
            var severedHandlerPrototype = {};
            Object.defineProperty(severedHandlerPrototype, "get", {
              get: function () {
                Reflect.setPrototypeOf(severedTraversalBase, null);
                forceGc();
                severedTraversalAlive = severedTraversalWeak.deref() !== undefined;
                Object.defineProperty(severedTraversalBase, "value", {
                  value: 97,
                  configurable: true
                });
                return undefined;
              }
            });
            (function () {
              var proxy = new Proxy(
                severedTraversalBase,
                Object.create(severedHandlerPrototype)
              );
              severedTraversalWeak = new WeakRef(proxy);
              Reflect.setPrototypeOf(severedTraversalBase, proxy);
            })();
            [severedTraversalBase.value, severedTraversalAlive].join(":");
            "#,
        )
        .expect("followed Proxy nodes must remain rooted after observable edge removal"),
        Value::String(Arc::from("97:true"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    let first = vm.new_object().expect("first cycle object should allocate");
    let first_value = Value::Object(first);
    let first_pin = vm.pin(&first_value);
    let second = vm
        .new_object()
        .expect("second cycle object should allocate");
    vm.heap.with_obj(first.0, |object| {
        *object.proto().lock() = Some(Value::Object(second));
    });
    vm.heap.with_obj(second.0, |object| {
        *object.proto().lock() = Some(first_value.clone());
    });
    vm.unpin(first_pin);

    let error = vm
        .get_property(&first_value, "missing")
        .expect_err("an all-ordinary malformed cycle must not loop forever in Get");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.len(), baseline);

    let error = vm
        .has_property_key(&first_value, &crate::value::PropertyKey::from("missing"))
        .expect_err("an all-ordinary malformed cycle must not loop forever in HasProperty");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.len(), baseline);

    let error = vm
        .try_set_property_with_receiver(&first_value, "missing", Value::Number(1.0), &first_value)
        .expect_err("an all-ordinary malformed cycle must not loop forever in Set");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn proxy_own_keys_walk_is_iterative_metered_and_restores_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var deepOwnKeys = { marker: 1 };
        for (var i = 0; i < 5000; i += 1) {
          deepOwnKeys = new Proxy(deepOwnKeys, {});
        }
        var meteredOwnKeys = { marker: 1 };
        for (var j = 0; j < 100; j += 1) {
          meteredOwnKeys = new Proxy(meteredOwnKeys, {});
        }
        "#,
    )
    .expect("deep ownKeys fixtures should initialize");
    let baseline = vm.gc_pins.len();
    let deep = vm.get_global("deepOwnKeys");
    let keys = crate::builtins::own_property_keys_or_throw(&mut vm, &deep, false, true, true)
        .expect("deep transparent Proxy ownKeys should not recurse on the Rust stack");
    assert_eq!(keys, vec![crate::value::PropertyKey::from("marker")]);
    assert_eq!(vm.gc_pins.len(), baseline);

    let metered = vm.get_global("meteredOwnKeys");
    vm.set_fuel(Some(100));
    let error = crate::builtins::own_property_keys_or_throw(&mut vm, &metered, false, true, true)
        .expect_err("Proxy layers without target-key fuel must still abort ownKeys");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(101));
    let keys = crate::builtins::own_property_keys_or_throw(&mut vm, &metered, false, true, true)
        .expect("exact Proxy-layer plus target-key fuel should complete ownKeys forwarding");
    assert_eq!(keys, vec![crate::value::PropertyKey::from("marker")]);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn for_in_roots_lazy_state_across_proxy_traps_and_heap_retry() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.clear_kept_objects();
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");
    let baseline = vm.gc_pins.len();

    assert_eq!(
        vm.run(
            r#"
            (function () {
              var prototype = Object.create(null);
              prototype.protoKey = 2;
              var proxy = new Proxy({ ownKey: 1 }, {
                ownKeys: function(target) {
                  forceGc();
                  return Reflect.ownKeys(target);
                },
                getOwnPropertyDescriptor: function(target, key) {
                  forceGc();
                  return Reflect.getOwnPropertyDescriptor(target, key);
                },
                getPrototypeOf: function() {
                  var result = prototype;
                  prototype = null;
                  forceGc();
                  return result;
                }
              });
              var keys = [];
              for (var key in proxy) keys.push(key);
              return keys.join(",");
            })()
            "#,
        )
        .expect("for-in state and trap results should survive forced GC"),
        Value::String(Arc::from("ownKey,protoKey"))
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    let prototype = vm
        .new_object()
        .expect("cross-advance prototype should allocate");
    let prototype_value = Value::Object(prototype);
    vm.heap.with_obj(prototype.0, |object| {
        *object.proto().lock() = None;
        object.props().lock().insert(
            crate::value::PropertyKey::from("protoKey"),
            crate::value::PropertyDescriptor::data(Value::Number(2.0)),
        );
    });
    let source = vm
        .new_object()
        .expect("cross-advance source should allocate");
    let source = Value::Object(source);
    if let Value::Object(source_idx) = &source {
        vm.heap.with_obj(source_idx.0, |object| {
            *object.proto().lock() = Some(prototype_value);
            object.props().lock().insert(
                crate::value::PropertyKey::from("ownKey"),
                crate::value::PropertyDescriptor::data(Value::Number(1.0)),
            );
        });
    }
    let iterator = vm
        .make_for_in_keys(&source)
        .expect("cross-advance for-in iterator should initialize");
    assert_eq!(
        vm.iterator_next(&iterator)
            .expect("first for-in advance should succeed"),
        (Value::String(Arc::from("ownKey")), false)
    );
    drop(source);
    let iterator_pin = vm.pin(&iterator);
    vm.gc();
    for _ in 0..16 {
        vm.new_object()
            .expect("post-GC allocations should exercise reclaimed cell identities");
    }
    vm.unpin(iterator_pin);
    assert_eq!(
        vm.iterator_next(&iterator)
            .expect("traced for-in state should survive body-time GC"),
        (Value::String(Arc::from("protoKey")), false)
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.run(
        r#"
        var forInSourceWeak;
        var forInSourceAlive = false;
        var collectingPrototype = new Proxy({}, {
          ownKeys: function() {
            forceGc();
            forInSourceAlive = forInSourceWeak.deref() !== undefined;
            return [];
          },
          getPrototypeOf: function() { return null; }
        });
        "#,
    )
    .expect("collecting prototype fixture should initialize");
    let source = vm
        .run(
            r#"
            (function () {
              var object = Object.create(collectingPrototype);
              forInSourceWeak = new WeakRef(object);
              return object;
            })()
            "#,
        )
        .expect("weak for-in source should initialize");
    let iterator = vm
        .make_for_in_keys(&source)
        .expect("weak for-in source iterator should initialize");
    drop(source);
    assert_eq!(
        vm.iterator_next(&iterator)
            .expect("empty source and prototype should finish"),
        (Value::Undefined, true)
    );
    assert_eq!(vm.get_global("forInSourceAlive"), Value::Bool(true));
    assert_eq!(vm.gc_pins.len(), baseline);

    let source = vm
        .run("({ key: 1 })")
        .expect("for-in allocation source should initialize");
    let _garbage = vm
        .new_object()
        .expect("an unrooted object should provide GC retry capacity");
    let limit = vm.heap.live_count();
    vm.set_max_heap_objects(Some(limit));
    let iterator = vm
        .make_for_in_keys(&source)
        .expect("iterator allocation should root the source across exact-cap GC");
    vm.set_max_heap_objects(None);
    assert!(vm.heap.live_count() <= limit);
    assert_eq!(
        vm.iterator_next(&iterator)
            .expect("the rooted source should remain enumerable"),
        (Value::String(Arc::from("key")), false)
    );
    assert_eq!(vm.gc_pins.len(), baseline);

    let error = vm
        .run(
            r#"
            for (var key in new Proxy({}, {
              ownKeys: function() { forceGc(); throw new Error("for-in-gc-abrupt"); }
            })) {}
            "#,
        )
        .expect_err("an abrupt ownKeys trap should propagate");
    assert!(error.to_string().contains("for-in-gc-abrupt"));
    assert_eq!(vm.gc_pins.len(), baseline);

    for (source, marker) in [
        (
            r#"
            for (var key in new Proxy({ key: 1 }, {
              ownKeys: function(target) { return Reflect.ownKeys(target); },
              getOwnPropertyDescriptor: function() {
                forceGc();
                throw new Error("for-in-descriptor-abrupt");
              }
            })) {}
            "#,
            "for-in-descriptor-abrupt",
        ),
        (
            r#"
            for (var key in new Proxy({}, {
              ownKeys: function() { return []; },
              getPrototypeOf: function() {
                forceGc();
                throw new Error("for-in-prototype-abrupt");
              }
            })) {}
            "#,
            "for-in-prototype-abrupt",
        ),
    ] {
        let error = vm
            .run(source)
            .expect_err("for-in trap error should propagate");
        assert!(error.to_string().contains(marker), "got: {error}");
        assert_eq!(vm.gc_pins.len(), baseline, "pin leak after {marker}");
    }

    let error = vm
        .run(
            r#"
            var invariantTarget = {};
            var invariantInner = new Proxy(invariantTarget, {
              isExtensible: function(target) {
                Object.defineProperty(target, "fixed", {
                  value: 1,
                  configurable: false
                });
                forceGc();
                return true;
              }
            });
            var invariantOuter = new Proxy(invariantInner, {
              ownKeys: function() { return []; }
            });
            for (var key in invariantOuter) {}
            "#,
        )
        .expect_err("nested ownKeys invariant failure should propagate");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn for_in_primitive_boxing_obeys_the_exact_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.gc();
    let baseline_pins = vm.gc_pins.len();
    let primitive = Value::String(Arc::from("ab"));

    let success_limit = vm.heap.live_count() + 2;
    vm.set_max_heap_objects(Some(success_limit));
    let iterator = vm
        .make_for_in_keys(&primitive)
        .expect("one wrapper plus one iterator should fit the exact cap");
    vm.set_max_heap_objects(None);
    assert_eq!(vm.heap.live_count(), success_limit);
    assert_eq!(
        vm.iterator_next(&iterator)
            .expect("the boxed primitive should remain rooted by the iterator"),
        (Value::String(Arc::from("0")), false)
    );
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    drop(iterator);
    vm.gc();
    let failure_limit = vm.heap.live_count() + 1;
    vm.set_max_heap_objects(Some(failure_limit));
    let error = vm
        .make_for_in_keys(&primitive)
        .expect_err("the iterator must not fit after only its wrapper allocates");
    vm.set_max_heap_objects(None);
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline_pins);
}

#[test]
fn for_in_prototype_cycles_are_iterative_and_bounded() {
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run(
            r#"
            var breakingProxy;
            var prototypeCalls = 0;
            breakingProxy = new Proxy({}, {
              ownKeys: function() { return []; },
              getPrototypeOf: function() {
                prototypeCalls += 1;
                return prototypeCalls < 3 ? breakingProxy : null;
              }
            });
            for (var key in breakingProxy) {}
            prototypeCalls;
            "#,
        )
        .expect("an observable Proxy cycle may break itself"),
        Value::Number(3.0)
    );

    vm.run(
        r#"
        var cyclicForInProxy;
        cyclicForInProxy = new Proxy({}, {
          ownKeys: function() { return []; },
          getPrototypeOf: function() { return cyclicForInProxy; }
        });
        "#,
    )
    .expect("cyclic Proxy fixture should initialize");
    let proxy = vm.get_global("cyclicForInProxy");
    let iterator = vm
        .make_for_in_keys(&proxy)
        .expect("cyclic Proxy iterator should allocate");
    let baseline = vm.gc_pins.len();
    vm.set_fuel(Some(100));
    let error = vm
        .iterator_next(&iterator)
        .expect_err("an inert Proxy prototype cycle should exhaust fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
    vm.set_fuel(None);

    let guarded_iterator = vm
        .make_for_in_keys(&proxy)
        .expect("guarded cyclic Proxy iterator should allocate");
    let error = vm
        .iterator_next(&guarded_iterator)
        .expect_err("an unmetered inert Proxy cycle should hit the finite replay guard");
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.gc_pins.len(), baseline);

    let first = vm.new_object().expect("first cycle object should allocate");
    let first_value = Value::Object(first);
    let first_pin = vm.pin(&first_value);
    let second = vm
        .new_object()
        .expect("second cycle object should allocate");
    vm.heap.with_obj(first.0, |object| {
        *object.proto().lock() = Some(Value::Object(second));
    });
    vm.heap.with_obj(second.0, |object| {
        *object.proto().lock() = Some(first_value.clone());
    });
    let iterator = vm
        .make_for_in_keys(&first_value)
        .expect("ordinary cycle iterator should allocate");
    vm.unpin(first_pin);
    let error = vm
        .iterator_next(&iterator)
        .expect_err("a malformed all-ordinary prototype cycle must terminate");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn revoked_property_proxies_fail_before_zero_fuel_and_restore_pin_depth() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var revokedGetState = Proxy.revocable({}, {});
        var revokedGetProxy = revokedGetState.proxy;
        revokedGetState.revoke();
        var revokedHasState = Proxy.revocable({}, {});
        var revokedHasProxy = revokedHasState.proxy;
        revokedHasState.revoke();
        var revokedSetState = Proxy.revocable({}, {});
        var revokedSetProxy = revokedSetState.proxy;
        revokedSetState.revoke();
        "#,
    )
    .expect("revoked Proxy fixtures should initialize");
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(0));
    let get_proxy = vm.get_global("revokedGetProxy");
    let error = vm
        .get_property(&get_proxy, "x")
        .expect_err("revocation must precede Get fuel accounting");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    let has_proxy = vm.get_global("revokedHasProxy");
    let error = vm
        .has_property_key(&has_proxy, &crate::value::PropertyKey::from("x"))
        .expect_err("revocation must precede HasProperty fuel accounting");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    let set_proxy = vm.get_global("revokedSetProxy");
    let error = vm
        .try_set_property_with_receiver(&set_proxy, "x", Value::Number(1.0), &set_proxy)
        .expect_err("revocation must precede Set fuel accounting");
    assert_eq!(error.kind, crate::error::ErrorKind::Type);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);
}

#[test]
fn value_key_apis_preserve_symbols_returned_by_to_primitive() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run(
        r#"
        var coercedSymbol = Symbol("coerced-key");
        var keyCoercions = 0;
        var coercibleKey = {
          [Symbol.toPrimitive]: function() {
            keyCoercions += 1;
            return coercedSymbol;
          }
        };
        var coercedKeyTarget = {};
        coercedKeyTarget[coercedSymbol] = 17;
        "#,
    )
    .expect("coercible Symbol-key fixtures should initialize");
    let target = vm.get_global("coercedKeyTarget");
    let key = vm.get_global("coercibleKey");

    assert_eq!(
        vm.get_property_key(&target, &key)
            .expect("Get must preserve a Symbol returned by ToPropertyKey"),
        Value::Number(17.0)
    );
    assert_eq!(vm.get_global("keyCoercions"), Value::Number(1.0));

    vm.set_property_key(&target, &key, Value::Number(29.0))
        .expect("Set must preserve a Symbol returned by ToPropertyKey");
    assert_eq!(vm.get_global("keyCoercions"), Value::Number(2.0));
    let symbol = vm.get_global("coercedSymbol");
    assert_eq!(
        vm.get_property_key(&target, &symbol)
            .expect("the coerced Symbol property should receive the write"),
        Value::Number(29.0)
    );
}

#[test]
fn proxy_set_invariant_walks_consume_nested_fuel() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn("truthySetTrap", |_, _, _| Ok(Value::Bool(true)), 4)
        .expect("native set trap should register");
    vm.run(
        r#"
        var invariantSetBase = {};
        var invariantSetTarget = invariantSetBase;
        for (var i = 0; i < 64; i += 1) {
          invariantSetTarget = new Proxy(invariantSetTarget, {});
        }
        var invariantSetProxy = new Proxy(invariantSetTarget, {
          set: truthySetTrap
        });
        var callableSetTrap = truthySetTrap;
        for (var j = 0; j < 25000; j += 1) {
          callableSetTrap = new Proxy(callableSetTrap, {});
        }
        var callableSetProxy = new Proxy({}, { set: callableSetTrap });
        "#,
    )
    .expect("nested Proxy set invariant fixture should initialize");
    let proxy = vm.get_global("invariantSetProxy");
    let baseline = vm.gc_pins.len();

    vm.set_fuel(Some(64));
    let error = vm
        .try_set_property_with_receiver(&proxy, "x", Value::Number(1.0), &proxy)
        .expect_err("outer Set plus N-layer target descriptor walk require N+1 fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(65));
    assert!(vm
        .try_set_property_with_receiver(&proxy, "x", Value::Number(1.0), &proxy)
        .expect("exact nested Proxy set invariant fuel should complete"));
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    let callable_proxy = vm.get_global("callableSetProxy");
    vm.set_fuel(Some(25_000));
    let error = vm
        .try_set_property_with_receiver(
            &callable_proxy,
            "callable",
            Value::Number(1.0),
            &callable_proxy,
        )
        .expect_err("outer Set plus N callable Proxy layers require N+1 fuel");
    assert_eq!(error.kind, crate::error::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    assert_eq!(vm.gc_pins.len(), baseline);

    vm.set_fuel(Some(25_001));
    assert!(vm
        .try_set_property_with_receiver(
            &callable_proxy,
            "callable",
            Value::Number(1.0),
            &callable_proxy,
        )
        .expect("exact callable Proxy set-trap fuel should complete"));
    assert_eq!(vm.fuel_remaining(), Some(0));
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

#[test]
fn bound_function_metadata_abrupt_paths_restore_gc_pin_depth() {
    let cases = [
        r#"
        var sentinel = { marker: "prototype" };
        var target = new Proxy(function () {}, {
          getPrototypeOf: function () { forceGc(); throw sentinel; }
        });
        var same = false;
        try { Function.prototype.bind.call(target); }
        catch (error) { same = error === sentinel; }
        same;
        "#,
        r#"
        var sentinel = { marker: "has-length" };
        var target = new Proxy(function () {}, {
          getOwnPropertyDescriptor: function (_, key) {
            if (key === "length") { forceGc(); throw sentinel; }
          }
        });
        var same = false;
        try { Function.prototype.bind.call(target); }
        catch (error) { same = error === sentinel; }
        same;
        "#,
        r#"
        var sentinel = { marker: "get-length" };
        function target() {}
        Object.defineProperty(target, "length", {
          get: function () { forceGc(); throw sentinel; }
        });
        var same = false;
        try { target.bind(); }
        catch (error) { same = error === sentinel; }
        same;
        "#,
        r#"
        var sentinel = { marker: "get-name" };
        function target() {}
        Object.defineProperty(target, "name", {
          get: function () { forceGc(); throw sentinel; }
        });
        var same = false;
        try { target.bind(); }
        catch (error) { same = error === sentinel; }
        same;
        "#,
    ];

    for source in cases {
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
            vm.run(source)
                .expect("bound metadata abrupt completion should stay catchable"),
            Value::Bool(true)
        );
        assert_eq!(vm.gc_pins.len(), baseline_pins, "pin leak after {source}");
    }
}

#[test]
fn bound_function_metadata_allocation_obeys_the_exact_heap_cap() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run("function target(a, b) {}")
        .expect("bound target should initialize");
    let target = vm.get_global("target");
    let bind = vm
        .get_property(&target, "bind")
        .expect("Function.prototype.bind should be readable");
    let retained_pins = vm.pin_many(&[target.clone(), bind.clone()]);

    vm.gc();
    let baseline_pins = vm.gc_pins.len();
    let exact_success_limit = vm.heap.live_count() + 1;
    vm.set_max_heap_objects(Some(exact_success_limit));
    let bound = vm
        .call_function(&bind, &[], Some(target.clone()))
        .expect("one available slot should hold the bound function");
    assert_eq!(
        vm.get_property(&bound, "length")
            .expect("bound length should be readable"),
        Value::Number(2.0)
    );
    assert_eq!(
        vm.get_property(&bound, "name")
            .expect("bound name should be readable"),
        Value::String(Arc::from("bound target"))
    );
    assert_eq!(vm.heap.live_count(), exact_success_limit);
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.set_max_heap_objects(None);
    drop(bound);
    vm.gc();
    let exact_failure_limit = vm.heap.live_count();
    vm.set_max_heap_objects(Some(exact_failure_limit));
    let error = vm
        .call_function(&bind, &[], Some(target))
        .expect_err("a full heap must reject the bound allocation");
    vm.set_max_heap_objects(None);
    assert_eq!(error.kind, crate::error::ErrorKind::Range);
    assert_eq!(vm.heap.live_count(), exact_failure_limit);
    assert_eq!(vm.gc_pins.len(), baseline_pins);

    vm.unpin_many(retained_pins);
}
