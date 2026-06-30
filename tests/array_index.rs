//! Regression tests for array-index coercion and the OOM-DoS that a large
//! index used to trigger. Before the fix, `a[0x80000000]` tried to grow a
//! dense `Vec` to ~2B slots and OOM-killed the host process; `a[0xffffffff]`
//! was treated as a dense index instead of a named property; and a directly
//! set `length` above the dense cap would also explode the backing store.
//!
//! These tests assert ES-spec behavior verified against Node:
//!   - 0..2^32-1 are array indices; 2^32-1 and beyond are named properties.
//!   - Large but valid indices are stored sparsely, so `length` covers them
//!     without allocating holes.

mod common;

use common::run;
use ruja::Value;

/// Run `src` on a worker thread with a large stack. Needed for tests that
/// exercise deep recursion, because the default test-thread stack is small.
fn run_big_stack(src: &str) -> ruja::Value {
    use std::thread;
    let src = src.to_string();
    let worker = thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut vm = ruja::Vm::new();
            vm.run(&src).expect("evaluation errored")
        })
        .expect("failed to spawn worker");
    worker.join().expect("worker panicked")
}

/// `a[0x80000000]` used to OOM-kill the process by materializing ~2B slots.
/// Now it must store sparsely: `length` is 2^31+1 and the value is readable.
#[test]
fn large_index_does_not_oom() {
    let v = run("var a = []; a[0x80000000] = 'x'; a.length");
    assert_eq!(v, Value::Number(2147483649.0));
    let v = run("var a = []; a[0x80000000] = 'x'; a[0x80000000]");
    assert_eq!(v, Value::String(std::sync::Arc::from("x")));
    // Holes between 0 and the sparse index read as undefined.
    let v = run("var a = []; a[0x80000000] = 'x'; a[0]");
    assert_eq!(v, Value::Undefined);
}

/// `2^32 - 1` (0xffffffff) is NOT an array index per ES; it becomes a named
/// property and does not affect `length`.
#[test]
fn max_uint32_minus_one_is_a_property_not_index() {
    let v = run("var a = []; a[0xffffffff] = 'x'; a.length");
    assert_eq!(v, Value::Number(0.0));
    let v = run("var a = []; a[0xffffffff] = 'x'; a[0xffffffff]");
    assert_eq!(v, Value::String(std::sync::Arc::from("x")));
}

/// Negative indices are named properties, not array slots.
#[test]
fn negative_index_is_a_property() {
    let v = run("var a = []; a[-1] = 'x'; a.length");
    assert_eq!(v, Value::Number(0.0));
    let v = run("var a = []; a[-1] = 'x'; a[-1]");
    assert_eq!(v, Value::String(std::sync::Arc::from("x")));
}

/// String index access for a negative or out-of-range index returns
/// undefined and must not panic.
#[test]
fn string_negative_index_is_undefined() {
    let v = run(r#""abc"[-1]"#);
    assert_eq!(v, Value::Undefined);
    let v = run(r#""abc"[5]"#);
    assert_eq!(v, Value::Undefined);
    let v = run(r#""abc"[0]"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("a")));
}

/// Setting `length` above the dense cap must not allocate holes; it only
/// advances `length` (stored via the sparse-max tracker).
#[test]
fn large_length_set_does_not_oom() {
    let v = run("var a = []; a.length = 0x80000000; a.length");
    assert_eq!(v, Value::Number(2147483648.0));
}

/// Truncating `length` past a sparse index drops the sparse entry.
#[test]
fn sparse_index_dropped_on_length_truncate() {
    let v = run("var a = []; a[0x80000000] = 'x'; a.length = 10; a.length");
    assert_eq!(v, Value::Number(10.0));
    let v = run("var a = []; a[0x80000000] = 'x'; a.length = 10; a[0x80000000]");
    assert_eq!(v, Value::Undefined);
}

/// Setting `length` to an invalid value throws RangeError, matching V8/Node.
#[test]
fn invalid_array_length_throws() {
    let res = common::run_err("var a = []; a.length = -1");
    assert!(
        res.contains("Invalid array length"),
        "expected RangeError, got: {}",
        res
    );
    let res = common::run_err("var a = []; a.length = 1.5");
    assert!(
        res.contains("Invalid array length"),
        "expected RangeError, got: {}",
        res
    );
}

// --- DoS guards added in the second pass ---

/// `"x".repeat(Infinity)` used to panic the engine with capacity overflow.
#[test]
fn repeat_infinity_throws_not_panic() {
    let res = common::run_err(r#"try { "x".repeat(Infinity); } catch(e){ throw e; }"#);
    assert!(
        res.contains("Invalid count value"),
        "expected RangeError, got: {}",
        res
    );
}

/// Negative repeat count must throw, not silently yield "".
#[test]
fn repeat_negative_throws() {
    let res = common::run_err(r#"try { "x".repeat(-1); } catch(e){ throw e; }"#);
    assert!(
        res.contains("Invalid count value"),
        "expected RangeError, got: {}",
        res
    );
}

/// Fractional repeat count must throw.
#[test]
fn repeat_fractional_throws() {
    let res = common::run_err(r#"try { "x".repeat(2.5); } catch(e){ throw e; }"#);
    assert!(
        res.contains("Invalid count value"),
        "expected RangeError, got: {}",
        res
    );
}

/// Sanity that normal repeat still works.
#[test]
fn repeat_normal_works() {
    let v = run(r#""abc".repeat(3)"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("abcabcabc")));
}

/// `"x".padStart(Infinity, "ab")` used to hang the engine.
#[test]
fn padstart_infinity_throws_not_hang() {
    let res = common::run_err(r#"try { "x".padStart(Infinity, "ab"); } catch(e){ throw e; }"#);
    assert!(
        res.contains("Invalid string length"),
        "expected RangeError, got: {}",
        res
    );
}

#[test]
fn padend_infinity_throws_not_hang() {
    let res = common::run_err(r#"try { "x".padEnd(Infinity, "ab"); } catch(e){ throw e; }"#);
    assert!(
        res.contains("Invalid string length"),
        "expected RangeError, got: {}",
        res
    );
}

/// Negative pad length clamps to 0 (returns the original string).
#[test]
fn padstart_negative_clamps() {
    let v = run(r#""x".padStart(-1)"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("x")));
}

#[test]
fn padstart_normal_works() {
    let v = run(r#""x".padStart(5, "ab")"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("ababx")));
}

/// `JSON.parse` of deeply nested input used to overflow the native
/// stack and abort the process; it must now throw instead of crashing.
/// A return value of "ok" means it was caught as a SyntaxError.
#[test]
fn json_parse_deep_nesting_throws_not_crash() {
    // 5000 levels far exceed the depth cap, so JSON.parse must throw on a
    // large stack instead of aborting with a stack overflow.
    let src = format!(
        "var s='['.repeat({0})+']'.repeat({0});         (function(){{ try {{ JSON.parse(s); return 'no-throw'; }} catch(e) {{ return 'ok'; }} }})()",
        5000
    );
    let v = run_big_stack(&src);
    assert_eq!(v, Value::String(std::sync::Arc::from("ok")), "got {:?}", v);
}

/// Reasonable nesting (under the cap) still parses fine.
#[test]
fn json_parse_normal_nesting_works() {
    let v = run(r#"JSON.parse('[[1,2],{"a":3}]')"#);
    assert!(matches!(v, Value::Object(_)), "got {:?}", v);
}

/// `JSON.stringify` of a deeply nested user-built structure must not
/// overflow the native stack.
#[test]
fn json_stringify_deep_nesting_does_not_crash() {
    let v = run_big_stack(
        "var a=[]; for(var i=0;i<5000;i++){a=[a];} (function(){ try { JSON.stringify(a); return 'ok'; } catch(e){ return 'caught'; } })()",
    );
    match v {
        Value::String(ref s) => assert!(
            s.as_ref() == "ok" || s.as_ref() == "caught",
            "expected ok or caught, got {:?}",
            v
        ),
        other => panic!("expected string, got {:?}", other),
    }
}

// --- Pass 3 guards ---

/// `Array.from({length: 2**26})` used to materialize 64M dense slots and
/// hang/OOM the engine; it must now throw a RangeError quickly.
#[test]
fn array_from_huge_length_throws() {
    let res = common::run_err("Array.from({length: 67108864})");
    assert!(
        res.contains("Invalid array length"),
        "expected RangeError, got: {}",
        res
    );
}

/// Normal `Array.from` of an array-like still works.
#[test]
fn array_from_small_length_works() {
    let v = run("Array.from({length: 3}).length");
    assert_eq!(v, Value::Number(3.0));
    let v = run(r#"Array.from("ab").join("-")"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("a-b")));
}

// --- toFixed / toPrecision range conformance ---

#[test]
fn to_fixed_out_of_range_throws() {
    let res = common::run_err("(1).toFixed(200)");
    assert!(res.contains("between 0 and 100"), "got: {}", res);
    let res = common::run_err("(1).toFixed(-1)");
    assert!(res.contains("between 0 and 100"), "got: {}", res);
    let res = common::run_err("(1).toFixed(101)");
    assert!(res.contains("between 0 and 100"), "got: {}", res);
}

#[test]
fn to_fixed_normal_works() {
    let v = run("(1.1).toFixed(2)");
    assert_eq!(v, Value::String(std::sync::Arc::from("1.10")));
    let v = run("(1).toFixed(0)");
    assert_eq!(v, Value::String(std::sync::Arc::from("1")));
}

#[test]
fn to_precision_out_of_range_throws() {
    let res = common::run_err("(1).toPrecision(101)");
    assert!(res.contains("between 1 and 100"), "got: {}", res);
    let res = common::run_err("(1).toPrecision(-1)");
    assert!(res.contains("between 1 and 100"), "got: {}", res);
    let res = common::run_err("(1).toPrecision(0)");
    assert!(res.contains("between 1 and 100"), "got: {}", res);
}

#[test]
fn to_precision_normal_works() {
    let v = run("(1.1).toPrecision(3)");
    assert_eq!(v, Value::String(std::sync::Arc::from("1.10")));
    // No argument -> toString-like.
    let v = run("(1.1).toPrecision()");
    assert_eq!(v, Value::String(std::sync::Arc::from("1.1")));
}

// --- charCodeAt / codePointAt range conformance ---

#[test]
fn char_code_at_out_of_range_is_nan() {
    let v = run(r#""abc".charCodeAt(-1)"#);
    assert!(matches!(v, Value::Number(n) if n.is_nan()), "got {:?}", v);
    let v = run(r#""abc".charCodeAt(5)"#);
    assert!(matches!(v, Value::Number(n) if n.is_nan()), "got {:?}", v);
}

#[test]
fn char_code_at_in_range_works() {
    let v = run(r#""abc".charCodeAt(0)"#);
    assert_eq!(v, Value::Number(97.0));
    // Missing argument defaults to index 0.
    let v = run(r#""abc".charCodeAt()"#);
    assert_eq!(v, Value::Number(97.0));
}

#[test]
fn code_point_at_out_of_range_is_undefined() {
    let v = run(r#""abc".codePointAt(-1)"#);
    assert_eq!(v, Value::Undefined);
    let v = run(r#""abc".codePointAt(5)"#);
    assert_eq!(v, Value::Undefined);
}

#[test]
fn code_point_at_surrogate_pair() {
    let v = run("String.fromCodePoint(0x1F600).codePointAt(0)");
    assert_eq!(v, Value::Number(128512.0));
}

// --- String.prototype.split limit semantics ---

#[test]
fn split_negative_limit_returns_all() {
    let v = run(r#""a,b,c".split(",", -1).join("|")"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("a|b|c")));
}

#[test]
fn split_zero_limit_returns_empty() {
    let v = run(r#""a,b,c".split(",", 0).length"#);
    assert_eq!(v, Value::Number(0.0));
}

#[test]
fn split_fractional_limit_truncates() {
    let v = run(r#""a,b,c".split(",", 1.5).length"#);
    assert_eq!(v, Value::Number(1.0));
}

#[test]
fn split_nan_limit_returns_empty() {
    let v = run(r#""a,b,c".split(",", "x").length"#);
    assert_eq!(v, Value::Number(0.0));
}

// --- String.prototype.charAt range conformance ---

#[test]
fn char_at_negative_returns_empty() {
    let v = run(r#""abc".charAt(-1)"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("")));
}

#[test]
fn char_at_out_of_range_returns_empty() {
    let v = run(r#""abc".charAt(5)"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("")));
}

#[test]
fn char_at_in_range_works() {
    let v = run(r#""abc".charAt(0)"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("a")));
    let v = run(r#""abc".charAt()"#);
    assert_eq!(v, Value::String(std::sync::Arc::from("a")));
}

// --- GC with_obj reentrancy safety + Array.from iterable ---

/// A self-referential getter used to crash ("use after free") because
/// `with_obj` took the object out of its cell and a reentrant read found
/// it empty. Now it must not panic (it may diverge or throw, but not abort).
#[test]
fn reentrant_with_obj_does_not_panic() {
    // Use a big stack since the getter recurses before any stack guard.
    use std::thread;
    let src = r#"
        var o = { get x() { return o.x; } };
        (function () {
            try { o.x; return 'no-throw'; }
            catch (e) { return 'caught'; }
        })()
    "#;
    let src = src.to_string();
    let worker = thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut vm = ruja::Vm::new();
            vm.run(&src).expect("evaluation errored")
        })
        .expect("failed to spawn worker");
    let v = worker
        .join()
        .expect("worker panicked (use-after-free regression)");
    match v {
        ruja::Value::String(ref s) => assert!(
            s.as_ref() == "no-throw" || s.as_ref() == "caught",
            "got {:?}",
            v
        ),
        other => panic!("expected string, got {:?}", other),
    }
}

/// `Array.from` must drain iterables (generators, sets, user iterables).
/// Previously it returned [] for generators (Symbol.iterator is inherited,
/// not an own prop) and for any iterable that triggered GC the iterator was
/// collected mid-loop.
#[test]
fn array_from_drains_iterables() {
    let v = run("function*g(){yield 1;yield 2;yield 3} Array.from(g()).join(',')");
    assert_eq!(v, Value::String(std::sync::Arc::from("1,2,3")));
    let v = run("Array.from(new Set([1,2,2,3])).join(',')");
    assert_eq!(v, Value::String(std::sync::Arc::from("1,2,3")));
    let v = run(
        r#"var o={}; o[Symbol.iterator]=function(){var i=0;return {next:function(){if(i++>=3)return{done:true};return{value:i,done:false}}}}; Array.from(o).join(',') "#,
    );
    assert_eq!(v, Value::String(std::sync::Arc::from("1,2,3")));
}

/// A large generator must be fully drained (no GC truncation at ~593).
#[test]
fn array_from_large_generator_not_truncated() {
    use std::thread;
    let src = "function*g(){for(var i=0;i<5000;i++)yield i} Array.from(g()).length";
    let src = src.to_string();
    let worker = thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let mut vm = ruja::Vm::new();
            vm.run(&src).expect("evaluation errored")
        })
        .expect("failed to spawn worker");
    let v = worker.join().expect("worker panicked");
    assert_eq!(v, Value::Number(5000.0));
}

/// An infinite iterable must hit the cap and throw instead of hanging/OOMing.
#[test]
fn array_from_infinite_iterable_capped() {
    let res = common::run_err(
        r#"var o={}; o[Symbol.iterator]=function(){return {next:function(){return{value:1,done:false}}}}; Array.from(o);"#,
    );
    assert!(
        res.contains("Invalid array length"),
        "expected RangeError, got: {}",
        res
    );
}
