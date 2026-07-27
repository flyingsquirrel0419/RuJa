use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use indexmap::IndexMap;
use ruja::value::PropertyKey;
use ruja::{Value, Vm};

fn bench_fib(c: &mut Criterion) {
    let src = r#"
        function fib(n) {
            if (n <= 1) return n;
            return fib(n - 1) + fib(n - 2);
        }
        fib(25);
    "#;
    c.bench_function("fib(25)", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(src).expect("fib failed")
        })
    });
}

fn bench_tight_loop(c: &mut Criterion) {
    let src = r#"
        var sum = 0;
        for (var i = 0; i < 100000; i++) {
            sum += i;
        }
        sum;
    "#;
    c.bench_function("loop_100k", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(src).expect("loop failed")
        })
    });
}

fn bench_array_push(c: &mut Criterion) {
    let src = r#"
        var arr = [];
        for (var i = 0; i < 10000; i++) {
            arr.push(i);
        }
        arr.length;
    "#;
    c.bench_function("array_push_10k", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(src).expect("array push failed")
        })
    });
}

fn bench_array_index_set(c: &mut Criterion) {
    let dense_overwrite = r#"
        var arr = [0];
        for (var i = 0; i < 100000; i++) {
            arr[0] = i;
        }
        arr[0];
    "#;
    c.bench_function("array_index_dense_overwrite_100k", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(dense_overwrite).expect("dense overwrite failed")
        })
    });

    let dense_append = r#"
        var arr = [];
        for (var i = 0; i < 10000; i++) {
            arr[i] = i;
        }
        arr.length;
    "#;
    c.bench_function("array_index_dense_append_10k", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(dense_append).expect("dense append failed")
        })
    });

    let sparse_set = r#"
        var arr = [];
        for (var i = 0; i < 10000; i++) {
            arr[1048576 + i] = i;
        }
        arr.length;
    "#;
    c.bench_function("array_index_sparse_set_10k", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(sparse_set).expect("sparse Set failed")
        })
    });
}

fn bench_native_indexed_loops(c: &mut Criterion) {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var indexedArray = [];
        for (var i = 0; i < 10000; i++) indexedArray.push(i);
        function reverseArrayIndexed() {
            return indexedArray.reverse().length;
        }
        function consumeArrayIterator() {
            var iterator = indexedArray.values();
            var count = 0;
            while (!iterator.next().done) count++;
            return count;
        }

        var indexedTypedArray = new Uint32Array(65536);
        function reverseTypedArrayIndexed() {
            return indexedTypedArray.reverse().length;
        }
        "#,
    )
    .expect("native indexed-loop fixtures failed");

    for (name, function_name) in [
        ("array_reverse_indexed_10k", "reverseArrayIndexed"),
        ("array_iterator_values_10k", "consumeArrayIterator"),
        (
            "typed_array_reverse_indexed_64k",
            "reverseTypedArrayIndexed",
        ),
    ] {
        let function = vm.get_global(function_name);
        c.bench_function(name, |b| {
            b.iter(|| {
                black_box(
                    vm.call_function(&function, &[], None)
                        .expect("native indexed-loop benchmark failed"),
                )
            })
        });
    }
}

fn bench_property_key_maps(c: &mut Criterion) {
    const KEY_COUNT: u32 = 10_000;
    let numeric_keys: Vec<_> = (0..KEY_COUNT).map(PropertyKey::from_array_index).collect();
    let numeric_map: IndexMap<_, _> = numeric_keys
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect();
    c.bench_function("property_key_numeric_lookup_10k", |b| {
        b.iter(|| {
            for key in &numeric_keys {
                black_box(numeric_map.get(key));
            }
        })
    });

    let string_keys: Vec<_> = (0..KEY_COUNT)
        .map(|index| PropertyKey::from_string(format!("field{index}")))
        .collect();
    let string_map: IndexMap<_, _> = string_keys
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect();
    c.bench_function("property_key_string_lookup_10k", |b| {
        b.iter(|| {
            for key in &string_keys {
                black_box(string_map.get(key));
            }
        })
    });
}

fn bench_computed_references(c: &mut Criterion) {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        function numericComputedReferences() {
            var object = { 0: 0, 1: 0, 2: 0 };
            for (var i = 0; i < 10000; i++) {
                object[0] += 1;
                object[1] ||= 1;
                object[2]++;
            }
            return object[0] + object[1] + object[2];
        }
        function stringComputedReferences() {
            var object = { 0: 0, 1: 0, 2: 0 };
            for (var i = 0; i < 10000; i++) {
                object["0"] += 1;
                object["1"] ||= 1;
                object["2"]++;
            }
            return object["0"] + object["1"] + object["2"];
        }
        function nonIndexNumericPropertyKeys() {
            var object = { "-1": 1, "1.5": 1, "1e+21": 1 };
            var found = 0;
            for (var i = 0; i < 10000; i++) {
                found += -1 in object;
                found += 1.5 in object;
                found += 1e21 in object;
            }
            return found;
        }
        function nonIndexStringPropertyKeys() {
            var object = { "-1": 1, "1.5": 1, "1e+21": 1 };
            var found = 0;
            for (var i = 0; i < 10000; i++) {
                found += "-1" in object;
                found += "1.5" in object;
                found += "1e+21" in object;
            }
            return found;
        }
        "#,
    )
    .expect("computed Reference fixtures failed");

    let numeric = vm.get_global("numericComputedReferences");
    c.bench_function("computed_reference_numeric_30k", |b| {
        b.iter(|| {
            black_box(
                vm.call_function(&numeric, &[], None)
                    .expect("numeric computed References failed"),
            )
        })
    });

    let string = vm.get_global("stringComputedReferences");
    c.bench_function("computed_reference_string_30k", |b| {
        b.iter(|| {
            black_box(
                vm.call_function(&string, &[], None)
                    .expect("string computed References failed"),
            )
        })
    });

    let numeric_non_index = vm.get_global("nonIndexNumericPropertyKeys");
    assert_eq!(
        vm.call_function(&numeric_non_index, &[], None)
            .expect("non-index numeric PropertyKey fixture failed"),
        Value::Number(30_000.0)
    );
    c.bench_function("non_index_numeric_property_key_30k", |b| {
        b.iter(|| {
            black_box(
                vm.call_function(&numeric_non_index, &[], None)
                    .expect("non-index numeric PropertyKeys failed"),
            )
        })
    });

    let string_non_index = vm.get_global("nonIndexStringPropertyKeys");
    assert_eq!(
        vm.call_function(&string_non_index, &[], None)
            .expect("non-index string PropertyKey fixture failed"),
        Value::Number(30_000.0)
    );
    c.bench_function("non_index_string_property_key_30k", |b| {
        b.iter(|| {
            black_box(
                vm.call_function(&string_non_index, &[], None)
                    .expect("non-index string PropertyKeys failed"),
            )
        })
    });
}

fn bench_inline_cache(c: &mut Criterion) {
    let cached_read = r#"
        var object = { value: 1 };
        var sum = 0;
        for (var i = 0; i < 100000; i++) {
            sum += object.value;
        }
        sum;
    "#;
    c.bench_function("inline_cache_read_hit_100k", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(cached_read).expect("cached read failed")
        })
    });

    let invalidate = r#"
        var object = { value: 0 };
        var sum = 0;
        for (var i = 0; i < 100000; i++) {
            sum += object.value;
            object.value = i;
            object.missing = i;
        }
        sum;
    "#;
    c.bench_function("inline_cache_invalidate_hit_miss_100k", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(invalidate).expect("cache invalidation failed")
        })
    });
}

fn bench_ordinary_set_receiver(c: &mut Criterion) {
    let overwrite = r#"
        var base = { value: 0 };
        var receiver = { value: 0 };
        for (var i = 0; i < 100000; i++) {
            Reflect.set(base, "value", i, receiver);
        }
        receiver.value;
    "#;
    c.bench_function("ordinary_set_receiver_overwrite_100k", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(overwrite).expect("receiver overwrite failed")
        })
    });

    let create = r#"
        var base = Object.create(null);
        var receiver = Object.create(null);
        for (var i = 0; i < 10000; i++) {
            Reflect.set(base, "field" + i, i, receiver);
        }
        receiver.field9999;
    "#;
    c.bench_function("ordinary_set_receiver_create_10k", |b| {
        b.iter(|| {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.run(create).expect("receiver creation failed")
        })
    });
}

fn bench_integrity_level(c: &mut Criterion) {
    c.bench_function("object_freeze_10k_properties", |b| {
        b.iter_batched(
            || {
                let mut vm = Vm::new().expect("failed to initialize VM");
                vm.run(
                    r#"
                    var object = Object.create(null);
                    for (var i = 0; i < 10000; i++) object["field" + i] = i;
                    "#,
                )
                .expect("object fixture failed");
                let object_constructor = vm.get_global("Object");
                let freeze = vm
                    .get_property(&object_constructor, "freeze")
                    .expect("Object.freeze should exist");
                let object = vm.get_global("object");
                (vm, object_constructor, freeze, object)
            },
            |(mut vm, object_constructor, freeze, object)| {
                vm.call_function(&freeze, &[object], Some(object_constructor))
                    .expect("object freeze failed")
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("array_freeze_10k_elements", |b| {
        b.iter_batched(
            || {
                let mut vm = Vm::new().expect("failed to initialize VM");
                vm.run(
                    r#"
                    var array = [];
                    for (var i = 0; i < 10000; i++) array.push(i);
                    "#,
                )
                .expect("Array fixture failed");
                let object_constructor = vm.get_global("Object");
                let freeze = vm
                    .get_property(&object_constructor, "freeze")
                    .expect("Object.freeze should exist");
                let array = vm.get_global("array");
                (vm, object_constructor, freeze, array)
            },
            |(mut vm, object_constructor, freeze, array)| {
                vm.call_function(&freeze, &[array], Some(object_constructor))
                    .expect("Array freeze failed")
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("object_is_frozen_10k_properties", |b| {
        b.iter_batched(
            || {
                let mut vm = Vm::new().expect("failed to initialize VM");
                vm.run(
                    r#"
                    var object = Object.create(null);
                    for (var i = 0; i < 10000; i++) object["field" + i] = i;
                    Object.freeze(object);
                    "#,
                )
                .expect("object fixture failed");
                let object_constructor = vm.get_global("Object");
                let is_frozen = vm
                    .get_property(&object_constructor, "isFrozen")
                    .expect("Object.isFrozen should exist");
                let object = vm.get_global("object");
                (vm, object_constructor, is_frozen, object)
            },
            |(mut vm, object_constructor, is_frozen, object)| {
                vm.call_function(&is_frozen, &[object], Some(object_constructor))
                    .expect("predicate failed")
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("array_is_frozen_10k_elements", |b| {
        b.iter_batched(
            || {
                let mut vm = Vm::new().expect("failed to initialize VM");
                vm.run(
                    r#"
                    var array = [];
                    for (var i = 0; i < 10000; i++) array.push(i);
                    Object.freeze(array);
                    "#,
                )
                .expect("Array fixture failed");
                let object_constructor = vm.get_global("Object");
                let is_frozen = vm
                    .get_property(&object_constructor, "isFrozen")
                    .expect("Object.isFrozen should exist");
                let array = vm.get_global("array");
                (vm, object_constructor, is_frozen, array)
            },
            |(mut vm, object_constructor, is_frozen, array)| {
                vm.call_function(&is_frozen, &[array], Some(object_constructor))
                    .expect("predicate failed")
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_bigint_storage(c: &mut Criterion) {
    let bigint = Value::bigint(
        num_bigint::BigInt::parse_bytes(&vec![b'9'; 16 * 1024], 10)
            .expect("benchmark BigInt should parse"),
    );
    c.bench_function("bigint_value_clone_16k_digits", |b| {
        b.iter(|| black_box(bigint.clone()))
    });

    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        function smallBigIntArithmetic() {
          var value = 1n;
          for (var i = 0; i < 10000; i++) value = (value + 3n) ^ 1n;
          return value;
        }
        "#,
    )
    .expect("BigInt arithmetic fixture failed");
    let function = vm.get_global("smallBigIntArithmetic");
    c.bench_function("bigint_small_arithmetic_10k", |b| {
        b.iter(|| {
            vm.call_function(&function, &[], None)
                .expect("BigInt arithmetic failed")
        })
    });
}

criterion_group!(
    benches,
    bench_fib,
    bench_tight_loop,
    bench_array_push,
    bench_array_index_set,
    bench_native_indexed_loops,
    bench_property_key_maps,
    bench_computed_references,
    bench_inline_cache,
    bench_ordinary_set_receiver,
    bench_integrity_level,
    bench_bigint_storage
);
criterion_main!(benches);
