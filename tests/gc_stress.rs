//! GC safety under heavy allocation and Promise chains. The VM previously
//! collected values held in Rust locals (Promise handlers, microtask values)
//! during a GC triggered mid-call; these tests pin that regression.

mod common;
use common::run;
use ruja::Value;

#[test]
fn heavy_object_allocation_survives_gc() {
    let src = r#"
        let arr = [];
        for (let i = 0; i < 2000; i++) arr.push({ x: i, y: "s" + i });
        let sum = 0;
        for (let j = 0; j < arr.length; j++) sum += arr[j].x;
        sum;
    "#;
    assert_eq!(run(src), Value::Number(1999.0 * 2000.0 / 2.0));
}

#[test]
fn promise_chain_with_heavy_allocation() {
    let src = r#"
        async function main() {
            function makeChain(n) {
                let p = Promise.resolve(1);
                for (let i = 0; i < n; i++) {
                    p = p.then(function (v) {
                        let a = [];
                        for (let j = 0; j < 50; j++) a.push({ x: j, y: "s" + j });
                        return v + 1;
                    });
                }
                return p;
            }
            return await makeChain(60);
        }
        await main();
    "#;
    assert_eq!(run(src), Value::Number(61.0));
}

#[test]
fn promise_resolve_reject_values_survive_gc() {
    let src = r#"
        async function main() {
            let obj = { tag: "kept" };
            let p = Promise.resolve(obj);
            // Allocate enough to trigger a collection before the handler runs.
            let junk = [];
            for (let i = 0; i < 3000; i++) junk.push({ i: i });
            let v = await p;
            return v.tag + "|" + (junk.length === 3000);
        }
        await main();
    "#;
    assert_eq!(run(src), Value::String(std::rc::Rc::from("kept|true")));
}

#[test]
fn generator_values_survive_gc_across_yields() {
    let src = r#"
        function* gen() {
            for (let i = 0; i < 100; i++) {
                let held = { n: i };
                // force allocations between yields
                let junk = [];
                for (let j = 0; j < 100; j++) junk.push({ j: j });
                yield held.n;
            }
        }
        let sum = 0;
        for (let v of gen()) sum += v;
        sum;
    "#;
    assert_eq!(run(src), Value::Number(4950.0));
}
