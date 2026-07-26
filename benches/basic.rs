use criterion::{criterion_group, criterion_main, Criterion};
use ruja::Vm;

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

criterion_group!(
    benches,
    bench_fib,
    bench_tight_loop,
    bench_array_push,
    bench_array_index_set,
    bench_inline_cache
);
criterion_main!(benches);
