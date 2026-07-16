//! `for await...of` async iteration.

mod common;
use common::run;
use ruja::{Value, Vm};
use std::sync::Arc;

#[test]
fn for_await_over_async_generator() {
    let src = r#"
        async function* gen() { yield 1; yield 2; yield 3; }
        async function main() {
            let sum = 0;
            for await (let x of gen()) { sum += x; }
            return sum;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::Number(6.0));
}

#[test]
fn for_await_over_custom_async_iterator() {
    let src = r#"
        let obj = {
            [Symbol.asyncIterator]: async function*() { yield 10; yield 20; }
        };
        async function main() {
            let total = 0;
            for await (let v of obj) { total += v; }
            return total;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::Number(30.0));
}

#[test]
fn for_await_over_sync_iterable_fallback() {
    let src = r#"
        async function main() {
            let s = "";
            for await (let c of ["a","b","c"]) { s += c; }
            return s;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("abc")));
}

#[test]
fn for_await_only_unwraps_async_from_sync_values() {
    let src = r#"
        let nativePromise = Promise.resolve("native");
        let nativeAsyncIterable = {
            [Symbol.asyncIterator]() {
                let done = false;
                return {
                    next() {
                        if (done) return { value: undefined, done: true };
                        done = true;
                        return { value: nativePromise, done: false };
                    }
                };
            }
        };
        let nativeValue;
        for await (let value of nativeAsyncIterable) nativeValue = value;

        let syncValue;
        for await (let value of [Promise.resolve("sync")]) syncValue = value;

        [nativeValue === nativePromise, syncValue].join("|");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true|sync")));
}

#[test]
fn for_await_break_exits_early() {
    let src = r#"
        async function* gen() { yield 1; yield 2; yield 3; yield 4; }
        async function main() {
            let collected = [];
            for await (let x of gen()) {
                collected.push(x);
                if (x === 2) break;
            }
            return collected.join(",");
        }
        await main();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1,2")));
}

#[test]
fn for_await_body_await() {
    let src = r#"
        async function* gen() { yield 5; }
        async function double(x) { return x * 2; }
        async function main() {
            let total = 0;
            for await (let x of gen()) { total += await double(x); }
            return total;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::Number(10.0));
}

#[test]
fn for_await_interleaves_with_promise_jobs() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        const actual = [];
        async function* naturalNumbers(start) {
          while (start > 0) yield Promise.resolve(start--);
        }
        async function trigger() {
          for await (const value of naturalNumbers(3)) {
            actual.push("Await: " + value);
          }
        }
        function countdown(counter) {
          actual.push("Promise: " + counter);
          if (counter > 0) {
            return Promise.resolve(counter - 1).then(countdown);
          }
        }
        trigger();
        countdown(6);
        "#,
    )
    .expect("for-await and Promise jobs should complete");
    let result = vm
        .run("actual.join('|');")
        .expect("failed to read interleaved job order");

    assert_eq!(
        result,
        Value::String(Arc::from(
            "Promise: 6|Promise: 5|Await: 3|Promise: 4|Promise: 3|Await: 2|Promise: 2|Promise: 1|Await: 1|Promise: 0"
        ))
    );
}

#[test]
fn for_await_allows_async_identifier_lhs() {
    let source = r#"
        let async;
        async function assign() {
            for await (async of [7]);
        }
        await assign();
        async;
    "#;
    assert_eq!(run(source), Value::Number(7.0));
}

#[test]
fn async_from_sync_observes_constructor_lookup_and_job_ticks() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var actual = [];
        async function iterate() {
            var promise = Promise.resolve(0);
            actual.push("pre");
            for await (var value of [promise]) actual.push("loop");
            actual.push("post");
        }
        Promise.resolve(0)
            .then(() => actual.push("tick 1"))
            .then(() => actual.push("tick 2"))
            .then(() => actual.push("tick 3"))
            .then(() => actual.push("tick 4"));
        Object.defineProperty(Promise.prototype, "constructor", {
            get() {
                actual.push("constructor");
                return Promise;
            },
            configurable: true
        });
        iterate();
        "#,
    )
    .expect("failed to run async-from-sync iterator");

    assert_eq!(
        vm.run("actual.join('|');")
            .expect("failed to read async-from-sync job order"),
        Value::String(Arc::from(
            "pre|constructor|constructor|tick 1|tick 2|loop|constructor|tick 3|tick 4|post"
        ))
    );
}

#[test]
fn async_from_sync_rejects_abrupt_promise_constructor_lookup() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var actual = [];
        var marker = {};
        async function iterate() {
            var promise = Promise.resolve(0);
            Object.defineProperty(promise, "constructor", {
                get() { throw marker; }
            });
            actual.push("start");
            for await (var value of [promise]);
            actual.push("unreachable");
        }
        Promise.resolve(0)
            .then(() => actual.push("tick 1"))
            .then(() => actual.push("tick 2"));
        iterate().catch(error => actual.push("catch:" + (error === marker)));
        "#,
    )
    .expect("failed to run abrupt async-from-sync iterator");

    assert_eq!(
        vm.run("actual.join('|');")
            .expect("failed to read async-from-sync rejection order"),
        Value::String(Arc::from("start|tick 1|tick 2|catch:true"))
    );
}

#[test]
fn for_await_closes_sync_generator_after_yielded_promise_rejection() {
    let source = r#"
        var reason = {};
        var log = [];
        function* values() {
            try {
                yield Promise.reject(reason);
            } finally {
                log.push("close");
            }
        }
        async function iterate() {
            try {
                for await (var value of values()) log.push("unreachable");
            } catch (error) {
                log.push(error === reason ? "reason" : "wrong");
            }
            return log.join("|");
        }
        await iterate();
    "#;
    assert_eq!(run(source), Value::String(Arc::from("close|reason")));
}

#[test]
fn for_await_empty_async_generator() {
    let src = r#"
        async function* gen() {}
        async function main() {
            let count = 0;
            for await (let x of gen()) { count++; }
            return count;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::Number(0.0));
}
