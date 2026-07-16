//! Regression tests for the execution-fuel mechanism:
//!  - a tight loop is bounded by the fuel budget
//!  - fuel exhaustion is NOT catchable by a JS try/catch (a host-level abort)

use ruja::Vm;

#[test]
fn fuel_bounds_infinite_loop() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(10_000));
    let err = vm.run("var i=0; while(true){i++;}").unwrap_err();
    assert!(
        err.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        err
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn fuel_unbounded_by_default() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    assert_eq!(vm.fuel_remaining(), None);
    // a bounded loop completes fine without a fuel limit
    let v = vm
        .run("var s=0; for(let i=0;i<1000;i++){s+=i;} s;")
        .unwrap();
    assert_eq!(v, ruja::Value::Number(499500.0));
}

#[test]
fn fuel_can_be_refilled_between_runs() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(100));
    let _ = vm.run("while(true){}");
    // exhausted; refill and a fresh run succeeds
    vm.set_fuel(Some(1_000_000));
    let v = vm.run("1+1").unwrap();
    assert_eq!(v, ruja::Value::Number(2.0));
}

#[test]
fn fuel_exhaustion_is_uncatchable() {
    // Untrusted code must not be able to swallow a fuel exhaustion in a
    // try/catch and keep looping. The catch must not fire.
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(5_000));
    let src = "var n=0; for(;;){ try { while(true){} } catch(e){ n++; if(n>2){throw 'done';} } }";
    let err = vm.run(src).unwrap_err();
    // The script never reaches its own `throw 'done'`: fuel exhaustion aborts.
    assert!(
        err.to_string().contains("fuel exhausted"),
        "expected uncatchable fuel exhaustion, got: {}",
        err
    );
}

#[test]
fn iterator_helper_close_does_not_suppress_fuel_exhaustion() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(20_000));
    let err = vm
        .run(
            r#"
            var source = {
              next: function() { return { value: 1, done: false }; },
              return: function() { while (true) {} }
            };
            Iterator.prototype.map.call(source, function() {
              throw "callback";
            }).next();
            "#,
        )
        .expect_err("close-time fuel exhaustion must abort the host run");
    assert!(
        err.to_string().contains("fuel exhausted"),
        "expected close-time fuel exhaustion, got: {}",
        err
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_helper_native_loops_consume_fuel() {
    for expression in [
        "Iterator.prototype.drop.call(source, Infinity).next()",
        "Iterator.prototype.filter.call(source, Boolean).next()",
        "Iterator.prototype.flatMap.call(source, function() { return [].values(); }).next()",
        "Iterator.prototype.reduce.call(source, function(memo) { return memo; }, 0)",
        "Iterator.prototype.forEach.call(source, function() {})",
        "Iterator.prototype.some.call(source, function() { return false; })",
        "Iterator.prototype.every.call(source, function() { return true; })",
        "Iterator.prototype.find.call(source, function() { return false; })",
    ] {
        let mut vm = Vm::new().expect("failed to initialize VM");
        vm.set_fuel(Some(100));
        let source = format!(
            r#"
            var source = {{
              next: Iterator.prototype[Symbol.iterator],
              done: false,
              value: 0
            }};
            {expression};
            "#
        );
        let error = vm
            .run(&source)
            .expect_err("native Iterator helper loop must exhaust fuel");
        assert!(
            error.to_string().contains("fuel exhausted"),
            "expected helper fuel exhaustion, got: {}",
            error
        );
        assert_eq!(vm.fuel_remaining(), Some(0));
    }
}

#[test]
fn iterator_concat_empty_sources_consume_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var empty = {
          [Symbol.iterator]: function() {
            return { next: function() { return { done: true }; } };
          }
        };
        var sources = [];
        for (var i = 0; i < 200; i++) sources.push(empty);
        var concatenated = Iterator.concat(...sources);
        "#,
    )
    .expect("failed to create concatenated iterator");
    vm.set_fuel(Some(100));
    let error = vm
        .run("concatenated.next()")
        .expect_err("empty concat scan should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_zip_eager_setup_consumes_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(1_000));
    let error = vm
        .run(
            r#"
            var inner = { next: function() { return { done: true }; } };
            Iterator.zip({
              [Symbol.iterator]: function() { return this; },
              next: function() { return { value: inner, done: false }; }
            });
            "#,
        )
        .expect_err("infinite Iterator.zip setup should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_zip_inactive_longest_slots_consume_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var inputs = [];
        for (var i = 0; i < 200; i++) {
          inputs.push({ next: function() { return { done: true }; } });
        }
        var liveStep = 0;
        inputs.push({
          next: function() {
            return liveStep++ === 0
              ? { value: 1, done: false }
              : { done: true };
          }
        });
        var zipped = Iterator.zip(inputs, { mode: "longest" });
        zipped.next();
        "#,
    )
    .expect("failed to create and start wide zip helper");
    vm.set_fuel(Some(100));
    let error = vm
        .run("zipped.next()")
        .expect_err("inactive longest slots should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_zip_return_fuel_abort_does_not_leave_helper_running() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var inputs = [];
        for (var i = 0; i < 200; i++) {
          inputs.push({
            next: function() { return { value: 1, done: false }; },
            return: function() { return {}; }
          });
        }
        var zipped = Iterator.zip(inputs);
        zipped.next();
        "#,
    )
    .expect("failed to create and suspend wide zip helper");
    vm.set_fuel(Some(100));
    let error = vm
        .run("zipped.return()")
        .expect_err("wide zip return should exhaust fuel while extracting inputs");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        error
    );

    vm.set_fuel(Some(10_000));
    let result = vm
        .run("zipped.next().done")
        .expect("fuel-aborted return must leave the helper completed");
    assert_eq!(result, ruja::Value::Bool(true));
}

#[test]
fn iterator_zip_keyed_eager_key_setup_consumes_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var keyedInputs = {};
        for (var i = 0; i < 200; i++) {
          keyedInputs["key" + i] = {
            next: function() { return { done: true }; }
          };
        }
        "#,
    )
    .expect("failed to create wide keyed input object");
    vm.set_fuel(Some(100));
    let error = vm
        .run("Iterator.zipKeyed(keyedInputs)")
        .expect_err("wide Iterator.zipKeyed setup should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected keyed setup fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_zip_keyed_proxy_own_keys_array_like_consumes_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var ownKeyReads = 0;
        var keyList = new Proxy({}, {
          get: function(_target, key) {
            if (key === "length") return 10000;
            ownKeyReads += 1;
            return "key" + key;
          }
        });
        var proxyInputs = new Proxy({}, {
          ownKeys: function() { return keyList; }
        });
        "#,
    )
    .expect("failed to create Proxy ownKeys array-like input");
    vm.set_fuel(Some(100));
    let error = vm
        .run("Iterator.zipKeyed(proxyInputs)")
        .expect_err("Proxy ownKeys array-like traversal should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected Proxy ownKeys fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    vm.set_fuel(None);
    assert_eq!(
        vm.run("ownKeyReads > 0 && ownKeyReads < 10000")
            .expect("failed to inspect bounded Proxy ownKeys traversal"),
        ruja::Value::Bool(true)
    );
}

#[test]
fn iterator_zip_keyed_padding_collection_consumes_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var keyedInputs = {};
        for (var i = 0; i < 200; i++) {
          keyedInputs["key" + i] = {
            next: function() { return { done: true }; }
          };
        }
        var paddingGets = 0;
        var keyedPadding = new Proxy({}, {
          get: function() {
            paddingGets += 1;
            return undefined;
          }
        });
        "#,
    )
    .expect("failed to create keyed padding inputs");
    vm.set_fuel(Some(300));
    let error = vm
        .run(
            r#"
            Iterator.zipKeyed(keyedInputs, {
              mode: "longest",
              padding: keyedPadding
            });
            "#,
        )
        .expect_err("wide Iterator.zipKeyed padding collection should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected keyed padding fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    vm.set_fuel(None);
    assert_eq!(
        vm.run("paddingGets > 0 && paddingGets < 200")
            .expect("failed to inspect partial keyed padding collection"),
        ruja::Value::Bool(true)
    );
}

#[test]
fn iterator_zip_keyed_wide_step_consumes_fuel_and_completes() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var keyedInputs = {};
        for (var i = 0; i < 200; i++) {
          keyedInputs["key" + i] = {
            next: function() { return { value: 1, done: false }; },
            return: function() { return {}; }
          };
        }
        var keyedZip = Iterator.zipKeyed(keyedInputs);
        "#,
    )
    .expect("failed to create wide keyed zip helper");
    vm.set_fuel(Some(100));
    let error = vm
        .run("keyedZip.next()")
        .expect_err("wide Iterator.zipKeyed step should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected keyed step fuel exhaustion, got: {}",
        error
    );
    vm.set_fuel(Some(10_000));
    assert_eq!(
        vm.run("keyedZip.next().done")
            .expect("fuel-aborted keyed step must leave the helper completed"),
        ruja::Value::Bool(true)
    );
}

#[test]
fn object_prototype_proxy_chain_walks_consume_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var cyclicProxy;
        cyclicProxy = new Proxy({}, {
          getOwnPropertyDescriptor: function() { return undefined; },
          getPrototypeOf: function() { return cyclicProxy; }
        });
        "#,
    )
    .expect("failed to create cyclic prototype proxy");

    for expression in [
        "Object.prototype.isPrototypeOf.call({}, cyclicProxy)",
        "Object.prototype.__lookupGetter__.call(cyclicProxy, 'value')",
    ] {
        vm.set_fuel(Some(100));
        let error = vm
            .run(expression)
            .expect_err("cyclic Proxy prototype walk should exhaust fuel");
        assert!(
            error.to_string().contains("fuel exhausted"),
            "expected fuel exhaustion for {expression}, got: {error}"
        );
    }
}

#[test]
fn normal_errors_remain_catchable() {
    // Fuel change must not break ordinary try/catch of catchable errors.
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(1_000_000));
    let v = vm
        .run("var r; try { null.x; } catch(e) { r = 'caught ' + (e instanceof Error); } r;")
        .unwrap();
    assert_eq!(v, ruja::Value::String(std::sync::Arc::from("caught true")));
}

#[test]
fn heap_limit_is_catchable_not_a_panic() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    // Set a limit slightly above the current live count so only script
    // allocations push past the threshold.
    vm.set_max_heap_objects(Some(500));
    let result = vm.run("var a = []; while(true) { a.push({ x: 1, y: 2 }); }");
    assert!(
        result.is_err(),
        "heap limit should produce a catchable error, not a panic"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("heap limit"),
        "expected 'heap limit' in error, got: {}",
        err
    );
}

/// Heap limit must be enforced even when allocations happen inside builtins
/// (JSON.parse, Array.prototype.map/slice, RegExp). Previously, raw
/// `heap.allocate()` calls in builtins bypassed the limit entirely.
#[test]
fn heap_limit_enforced_in_json_parse() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    vm.set_max_heap_objects(Some(500));
    // Each element of the parsed array is a separate heap object.
    let big_json = format!(
        "[{}]",
        (0..10_000)
            .map(|i| format!("{{\"k\":{}}}", i))
            .collect::<Vec<_>>()
            .join(",")
    );
    let result = vm.run(&format!("JSON.parse({:?})", big_json));
    assert!(result.is_err(), "JSON.parse should hit heap limit");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("heap limit"),
        "expected 'heap limit' in error, got: {}",
        err
    );
}

#[test]
fn heap_limit_enforced_in_array_map() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    vm.set_max_heap_objects(Some(500));
    // map() creates a new array + result objects per element.
    let result = vm.run(
        "var a = [1,2,3,4,5,6,7,8,9,10]; \
         var b = a; \
         for (var i = 0; i < 10000; i++) { b = b.concat(a).map(function(x) { return { v: x }; }); }",
    );
    assert!(result.is_err(), "Array.map should hit heap limit");
}

#[test]
fn heap_limit_enforced_in_regexp() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    vm.set_max_heap_objects(Some(500));
    // Each match creates a result array object on the heap.
    let result = vm.run(
        "var s = 'a'.repeat(10000); \
         var re = /a/g; \
         var matches = []; \
         while (re.exec(s) !== null) { matches.push(re.exec(s)); }",
    );
    assert!(result.is_err(), "RegExp should hit heap limit");
}
