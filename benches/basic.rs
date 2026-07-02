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
            let mut vm = Vm::new();
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
            let mut vm = Vm::new();
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
            let mut vm = Vm::new();
            vm.run(src).expect("array push failed")
        })
    });
}

criterion_group!(benches, bench_fib, bench_tight_loop, bench_array_push);
criterion_main!(benches);
