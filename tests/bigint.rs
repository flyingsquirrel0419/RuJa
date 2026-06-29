//! BigInt literal and arithmetic support.

mod common;
use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

#[test]
fn bigint_literal_typeof() {
    assert_eq!(run("typeof 0n;"), Value::String(Arc::from("bigint")));
}

#[test]
fn bigint_add() {
    assert_eq!(
        run("123n + 456n;"),
        Value::BigInt(num_bigint::BigInt::from(579))
    );
}

#[test]
fn bigint_sub_mul_div_mod() {
    assert_eq!(run("10n - 3n;"), Value::BigInt(num_bigint::BigInt::from(7)));
    assert_eq!(run("6n * 7n;"), Value::BigInt(num_bigint::BigInt::from(42)));
    assert_eq!(
        run("100n / 7n;"),
        Value::BigInt(num_bigint::BigInt::from(14))
    );
    assert_eq!(
        run("100n % 7n;"),
        Value::BigInt(num_bigint::BigInt::from(2))
    );
}

#[test]
fn bigint_pow() {
    assert_eq!(
        run("2n ** 10n;"),
        Value::BigInt(num_bigint::BigInt::from(1024))
    );
}

#[test]
fn bigint_neg() {
    assert_eq!(run("-5n;"), Value::BigInt(num_bigint::BigInt::from(-5)));
}

#[test]
fn bigint_strict_eq() {
    assert_eq!(run("123n === 123n;"), Value::Bool(true));
    assert_eq!(run("123n === 456n;"), Value::Bool(false));
    assert_eq!(run("0n === 0;"), Value::Bool(false));
}

#[test]
fn bigint_loose_eq() {
    assert_eq!(run("0n == 0;"), Value::Bool(true));
    assert_eq!(run("123n == 123;"), Value::Bool(true));
}

#[test]
fn bigint_compare() {
    assert_eq!(run("1n < 2n;"), Value::Bool(true));
    assert_eq!(run("3n > 2n;"), Value::Bool(true));
    assert_eq!(run("2n > 3n;"), Value::Bool(false));
    assert_eq!(run("1n < 2;"), Value::Bool(true));
}

#[test]
fn bigint_constructor() {
    assert_eq!(
        run("BigInt(123);"),
        Value::BigInt(num_bigint::BigInt::from(123))
    );
    assert_eq!(
        run("BigInt('456');"),
        Value::BigInt(num_bigint::BigInt::from(456))
    );
    assert_eq!(
        run("BigInt(true);"),
        Value::BigInt(num_bigint::BigInt::from(1))
    );
}

#[test]
fn bigint_constructor_rejects_fractional() {
    let err = run_err("BigInt(1.5);");
    assert!(err.contains("RangeError"), "got: {}", err);
}

#[test]
fn bigint_mix_with_number_is_typeerror() {
    let err = run_err("1n + 1;");
    assert!(err.contains("TypeError"), "got: {}", err);
}

#[test]
fn bigint_to_string() {
    assert_eq!(run("(123n).toString();"), Value::String(Arc::from("123")));
}

#[test]
fn bigint_large_exact() {
    assert_eq!(
        run("9007199254740993n === 9007199254740993n;"),
        Value::Bool(true)
    );
}

#[test]
fn bigint_hex_oct_bin_literals() {
    assert_eq!(run("0xffn;"), Value::BigInt(num_bigint::BigInt::from(255)));
    assert_eq!(run("0o17n;"), Value::BigInt(num_bigint::BigInt::from(15)));
    assert_eq!(run("0b101n;"), Value::BigInt(num_bigint::BigInt::from(5)));
}
