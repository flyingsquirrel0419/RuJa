use super::Vm;
use crate::value::{HeapObj, PromiseStatus};
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

fn realm_registry_counts(vm: &Vm) -> [usize; 28] {
    [
        vm.realm_globals.len(),
        vm.realm_object_prototypes.len(),
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
        vm.realm_regexp_prototypes.len(),
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
    baseline_registries: [usize; 28],
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
