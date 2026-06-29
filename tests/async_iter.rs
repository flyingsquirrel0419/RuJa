//! `for await...of` async iteration.

mod common;
use common::run;
use ruja::Value;
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
