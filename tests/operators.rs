//! Logical operators (and/or/nullish) and logical/compound assignment,
//! including member and element targets.

mod common;
use common::run;
use ruja::Value;
use std::sync::Arc;

// --- && short-circuit ---

#[test]
fn logical_and_truthy() {
    assert_eq!(run("1 && 2;"), Value::Number(2.0));
    assert_eq!(run("true && 'x';"), Value::String(Arc::from("x")));
}

#[test]
fn logical_and_falsy_keeps_left() {
    assert_eq!(run("0 && 2;"), Value::Number(0.0));
    assert_eq!(run("null && 2;"), Value::Null);
    assert_eq!(run("'' && 'x';"), Value::String(Arc::from("")));
    assert_eq!(run("false && true;"), Value::Bool(false));
    assert_eq!(run("undefined && 1;"), Value::Undefined);
}

#[test]
fn logical_and_chain() {
    assert_eq!(run("1 && 2 && 3;"), Value::Number(3.0));
    assert_eq!(run("1 && 0 && 3;"), Value::Number(0.0));
}

// --- || short-circuit ---

#[test]
fn logical_or_falsy() {
    assert_eq!(run("0 || 2;"), Value::Number(2.0));
    assert_eq!(run("null || 'd';"), Value::String(Arc::from("d")));
    assert_eq!(run("false || true;"), Value::Bool(true));
    assert_eq!(run("'' || 'x';"), Value::String(Arc::from("x")));
}

#[test]
fn logical_or_truthy_keeps_left() {
    assert_eq!(run("1 || 2;"), Value::Number(1.0));
    assert_eq!(run("'a' || 'b';"), Value::String(Arc::from("a")));
}

#[test]
fn logical_or_chain() {
    assert_eq!(run("0 || 0 || 3;"), Value::Number(3.0));
    assert_eq!(run("1 || 2 || 3;"), Value::Number(1.0));
}

// --- ?? nullish coalescing ---

#[test]
fn nullish_null() {
    assert_eq!(run("null ?? 1;"), Value::Number(1.0));
}

#[test]
fn nullish_undefined() {
    assert_eq!(run("undefined ?? 5;"), Value::Number(5.0));
}

#[test]
fn nullish_keeps_falsy_non_nullish() {
    // 0, '', false are NOT nullish -> kept as-is.
    assert_eq!(run("0 ?? 2;"), Value::Number(0.0));
    assert_eq!(run("'' ?? 'x';"), Value::String(Arc::from("")));
    assert_eq!(run("false ?? true;"), Value::Bool(false));
}

#[test]
fn nullish_non_nullish() {
    assert_eq!(run("1 ?? null;"), Value::Number(1.0));
    assert_eq!(run("'a' ?? 'b';"), Value::String(Arc::from("a")));
}

#[test]
fn nullish_chain() {
    assert_eq!(run("null ?? undefined ?? 3;"), Value::Number(3.0));
    assert_eq!(run("1 ?? 2 ?? 3;"), Value::Number(1.0));
}

// --- mixed precedence ---

#[test]
fn nullish_lower_than_or() {
    // ?? binds looser than ||, so this is `1 || (2 ?? 3)`.
    assert_eq!(run("1 || 2 ?? 3;"), Value::Number(1.0));
}

#[test]
fn and_or_mix() {
    assert_eq!(run("0 && 1 || 2;"), Value::Number(2.0));
    assert_eq!(run("1 && 1 || 0;"), Value::Number(1.0));
}

// --- simple assignment ---

#[test]
fn assign_ident() {
    assert_eq!(run("var a; a = 5; a;"), Value::Number(5.0));
}

#[test]
fn assign_member() {
    assert_eq!(run("var o = {n: 0}; o.n = 7; o.n;"), Value::Number(7.0));
}

#[test]
fn assign_element() {
    assert_eq!(run("var a = [0,0,0]; a[1] = 9; a[1];"), Value::Number(9.0));
}

// --- compound assignment (numeric/bitwise) ---

#[test]
fn compound_ident() {
    assert_eq!(run("var a = 1; a += 5; a;"), Value::Number(6.0));
    assert_eq!(run("var a = 10; a -= 3; a;"), Value::Number(7.0));
    assert_eq!(run("var a = 4; a *= 3; a;"), Value::Number(12.0));
    assert_eq!(run("var a = 20; a /= 4; a;"), Value::Number(5.0));
    assert_eq!(run("var a = 17; a %= 5; a;"), Value::Number(2.0));
}

#[test]
fn compound_member() {
    assert_eq!(run("var o = {n: 3}; o.n += 5; o.n;"), Value::Number(8.0));
    assert_eq!(run("var o = {n: 10}; o.n -= 4; o.n;"), Value::Number(6.0));
    assert_eq!(run("var o = {n: 2}; o.n *= 5; o.n;"), Value::Number(10.0));
    assert_eq!(run("var o = {n: 20}; o.n /= 4; o.n;"), Value::Number(5.0));
}

#[test]
fn compound_element() {
    assert_eq!(
        run("var a = [10,20,30]; a[1] += 5; a[1];"),
        Value::Number(25.0)
    );
}

// --- logical assignment ---

#[test]
fn nullish_assign_ident() {
    assert_eq!(run("var a = null; a ??= 5; a;"), Value::Number(5.0));
    assert_eq!(run("var a = 1; a ??= 99; a;"), Value::Number(1.0));
    assert_eq!(run("var a = 0; a ??= 9; a;"), Value::Number(0.0));
}

#[test]
fn nullish_assign_member() {
    assert_eq!(
        run("var p = {n: null}; p.n ??= 10; p.n;"),
        Value::Number(10.0)
    );
    assert_eq!(run("var q = {n: 1}; q.n ??= 99; q.n;"), Value::Number(1.0));
}

#[test]
fn nullish_assign_element() {
    assert_eq!(
        run("var a = [null, 1, 0]; a[0] ??= 5; a[2] ??= 9; a[0];"),
        Value::Number(5.0)
    );
}

#[test]
fn and_assign_ident() {
    assert_eq!(run("var a = 0; a &&= 2; a;"), Value::Number(0.0));
    assert_eq!(run("var a = 5; a &&= a + 1; a;"), Value::Number(6.0));
}

#[test]
fn and_assign_member() {
    assert_eq!(run("var r = {n: 0}; r.n &&= 2; r.n;"), Value::Number(0.0));
}

#[test]
fn or_assign_ident() {
    assert_eq!(run("var a = 0; a ||= 2; a;"), Value::Number(2.0));
    assert_eq!(run("var a = 1; a ||= 99; a;"), Value::Number(1.0));
}

#[test]
fn or_assign_member() {
    assert_eq!(run("var s = {n: 0}; s.n ||= 9; s.n;"), Value::Number(9.0));
}

#[test]
fn or_assign_element() {
    assert_eq!(
        run("var a = [0, 1]; a[0] ||= 99; a[1] ||= 99; a[0];"),
        Value::Number(99.0)
    );
}

// --- optional chaining (?.) ---

#[test]
fn optional_member_present() {
    assert_eq!(
        run("var o = {a:{b:{c:42}}}; o?.a?.b?.c;"),
        Value::Number(42.0)
    );
    assert_eq!(run("var o = {x: 7}; o?.x;"), Value::Number(7.0));
}

#[test]
fn optional_member_null() {
    assert_eq!(run("null?.foo;"), Value::Undefined);
    assert_eq!(run("undefined?.foo;"), Value::Undefined);
}

#[test]
fn optional_member_missing() {
    assert_eq!(run("var o = {a:1}; o?.b?.c;"), Value::Undefined);
    assert_eq!(run("var o = {a:{b:1}}; o?.a?.b?.c;"), Value::Undefined);
}

#[test]
fn optional_computed() {
    assert_eq!(
        run("var o = {a:{b:5}}; o?.[\"a\"]?.[\"b\"];"),
        Value::Number(5.0)
    );
    assert_eq!(run("var o = {a:1}; o?.[\"x\"]?.[\"y\"];"), Value::Undefined);
}

#[test]
fn optional_method_call() {
    assert_eq!(
        run("var o = {greet: function(){return 'hi';}}; o?.greet();"),
        Value::String(Arc::from("hi"))
    );
}

#[test]
fn optional_method_on_null() {
    // null?.greet() short-circuits the whole chain to undefined.
    assert_eq!(run("null?.greet();"), Value::Undefined);
}

#[test]
fn optional_call_null() {
    assert_eq!(run("var f = null; f?.();"), Value::Undefined);
}

#[test]
fn optional_call_present() {
    assert_eq!(
        run("var g = function(){return 99;}; g?.();"),
        Value::Number(99.0)
    );
}

#[test]
fn optional_chain_deep() {
    assert_eq!(
        run("var d = {a:{b:{c:{d:5}}}}; d?.a?.b?.c?.d;"),
        Value::Number(5.0)
    );
    assert_eq!(
        run("var d = {a:{b:{c:{d:5}}}}; d?.a?.x?.y?.z;"),
        Value::Undefined
    );
}

// --- Number toString (exponential notation) ---

#[test]
fn number_to_string_large() {
    assert_eq!(run("1e21 + '';"), Value::String(Arc::from("1e21")));
    assert_eq!(run("1e22 + '';"), Value::String(Arc::from("1e22")));
}

#[test]
fn number_to_string_small() {
    assert_eq!(run("1e-7 + '';"), Value::String(Arc::from("1e-7")));
    assert_eq!(run("0.0000001 + '';"), Value::String(Arc::from("1e-7")));
    assert_eq!(run("5e-8 + '';"), Value::String(Arc::from("5e-8")));
}

#[test]
fn number_to_string_normal() {
    assert_eq!(run("(1.5e3) + '';"), Value::String(Arc::from("1500")));
    assert_eq!(run("42 + '';"), Value::String(Arc::from("42")));
    assert_eq!(run("0 + '';"), Value::String(Arc::from("0")));
    assert_eq!(run("3.14 + '';"), Value::String(Arc::from("3.14")));
}

// --- deep optional method chains ---

#[test]
fn optional_method_chain_missing() {
    assert_eq!(
        run("var o = {g: function(){return 1;}}; o?.missing?.();"),
        Value::Undefined
    );
}

#[test]
fn optional_method_chain_null_root() {
    assert_eq!(run("null?.missing?.();"), Value::Undefined);
}

#[test]
fn optional_method_chain_present() {
    assert_eq!(
        run("var o = {greet: function(){return 'hi';}}; o?.greet?.();"),
        Value::String(Arc::from("hi"))
    );
}
