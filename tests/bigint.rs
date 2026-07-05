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
    for src in ["1n ** -1n;", "0n ** -1n;", "(-1n) ** -1n;"] {
        let err = run_err(src);
        assert!(err.contains("RangeError"), "{src}: {err}");
    }
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
    assert_eq!(run("0n == '';"), Value::Bool(true));
    assert_eq!(run("0x10n == 16n;"), Value::Bool(true));
    assert_eq!(run("0x10000000000000000n == 0n;"), Value::Bool(false));
}

#[test]
fn bigint_compare() {
    assert_eq!(run("1n < 2n;"), Value::Bool(true));
    assert_eq!(run("3n > 2n;"), Value::Bool(true));
    assert_eq!(run("2n > 3n;"), Value::Bool(false));
    assert_eq!(run("1n < 2;"), Value::Bool(true));
    assert_eq!(run("1n < 1.5;"), Value::Bool(true));
    assert_eq!(run("'0x10' > 15n;"), Value::Bool(true));
    assert_eq!(run("1n > '0.';"), Value::Bool(false));
    assert_eq!(run("0x10000000000000000n > 0n;"), Value::Bool(true));
    assert_eq!(run("0n < true;"), Value::Bool(true));
    assert_eq!(run("true > 0n;"), Value::Bool(true));
    assert_eq!(run("1n > true;"), Value::Bool(false));
    assert_eq!(run("false < 1n;"), Value::Bool(true));
    assert_eq!(run("-3n < false;"), Value::Bool(true));
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
    assert_eq!(
        run("BigInt({ valueOf: function() { return '0x10'; } });"),
        Value::BigInt(num_bigint::BigInt::from(16))
    );
}

#[test]
fn bigint_constructor_rejects_fractional() {
    let err = run_err("BigInt(1.5);");
    assert!(err.contains("RangeError"), "got: {}", err);
}

#[test]
fn bigint_constructor_rejects_missing_and_nullish_with_typeerror() {
    for src in ["BigInt();", "BigInt(undefined);", "BigInt(null);"] {
        let err = run_err(src);
        assert!(err.contains("TypeError"), "{src}: {err}");
    }
}

#[test]
fn bigint_as_int_n_and_as_uint_n_wrap_and_validate_order() {
    assert_eq!(
        run(r#"
            [
              BigInt.asUintN(8, 0xabcdn).toString(),
              BigInt.asIntN(8, 0xabn).toString(),
              BigInt.asUintN(64, -1n).toString(),
              BigInt.asIntN(64, 0xffffffffffffffffn).toString(),
              BigInt.asIntN(3.9, "10").toString(),
              BigInt.asUintN(undefined, 1n).toString(),
              BigInt.asIntN.length,
              BigInt.asUintN.name,
              Object.prototype.propertyIsEnumerable.call(BigInt, "asIntN")
            ].join(",");
            "#),
        Value::String(Arc::from(
            "205,-85,18446744073709551615,-1,2,0,2,asUintN,false"
        ))
    );
    assert_eq!(
        run(r#"
            var i = 0;
            var bits = { valueOf: function() { if (i !== 0) throw new Error("bits"); i++; return 0; } };
            var bigint = { valueOf: function() { if (i !== 1) throw new Error("bigint"); i++; return 0n; } };
            BigInt.asIntN(bits, bigint);
            i;
            "#),
        Value::Number(2.0)
    );
    assert!(
        run_err("BigInt.asUintN(0n, 0n);").contains("TypeError"),
        "bits uses ToIndex and should reject BigInt"
    );
    assert!(
        run_err("BigInt.asIntN(-1, 0n);").contains("RangeError"),
        "negative bits should throw RangeError"
    );
    assert!(
        run_err("BigInt.asIntN(0, 1);").contains("TypeError"),
        "value uses ToBigInt and should reject Number"
    );
}

#[test]
fn bigint_mix_with_number_is_typeerror() {
    let err = run_err("1n + 1;");
    assert!(err.contains("TypeError"), "got: {}", err);
}

#[test]
fn bigint_numeric_operator_coercions() {
    assert_eq!(
        run("0b101n & 0b011n;"),
        Value::BigInt(num_bigint::BigInt::from(1))
    );
    assert_eq!(
        run("0b101n | 0b011n;"),
        Value::BigInt(num_bigint::BigInt::from(7))
    );
    assert_eq!(
        run("0b101n ^ 0b011n;"),
        Value::BigInt(num_bigint::BigInt::from(6))
    );
    assert_eq!(
        run("Object(0b101n) & 0b011n;"),
        Value::BigInt(num_bigint::BigInt::from(1))
    );
    assert_eq!(run("8n >> 1n;"), Value::BigInt(num_bigint::BigInt::from(4)));
    assert_eq!(
        run("1n << 4n;"),
        Value::BigInt(num_bigint::BigInt::from(16))
    );

    for src in ["1n & 1;", "1n | 1;", "1n ^ 1;", "1n >>> 1n;", "+0n;"] {
        let err = run_err(src);
        assert!(err.contains("TypeError"), "{src}: {err}");
    }

    assert_eq!(run("Number(1n);"), Value::Number(1.0));
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
    assert_eq!(
        run("0x10000000000000000n;"),
        Value::BigInt(num_bigint::BigInt::parse_bytes(b"10000000000000000", 16).unwrap())
    );
    assert_eq!(run("0o17n;"), Value::BigInt(num_bigint::BigInt::from(15)));
    assert_eq!(run("0b101n;"), Value::BigInt(num_bigint::BigInt::from(5)));
}
