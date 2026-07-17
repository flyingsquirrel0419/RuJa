use super::Vm;
use crate::value::{HeapObj, PromiseStatus};
use crate::Value;
use std::fs;

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
