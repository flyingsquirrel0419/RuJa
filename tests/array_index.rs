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
