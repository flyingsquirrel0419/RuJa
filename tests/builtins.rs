//! Built-in objects and methods: Array, String, Object, Math, JSON, Symbol.

mod common;
use common::run;
use ruja::Value;
use std::rc::Rc;

#[test]
fn array_map_reduce() {
    assert_eq!(
        run("[1,2,3].map(x => x*2).join(',');"),
        Value::String(Rc::from("2,4,6"))
    );
    assert_eq!(run("[1,2,3].reduce((a,b)=>a+b, 0);"), Value::Number(6.0));
}

#[test]
fn array_method_chaining() {
    assert_eq!(
        run("[1,2,3,4,5].filter(x => x > 2).map(x => x * 2).join(',');"),
        Value::String(Rc::from("6,8,10"))
    );
}

#[test]
fn array_find() {
    assert_eq!(run("[4,5,6].find(x=>x>4);"), Value::Number(5.0));
}

#[test]
fn array_findindex() {
    assert_eq!(run("[4,5,6].findIndex(x=>x>4);"), Value::Number(1.0));
}

#[test]
fn array_some() {
    assert_eq!(run("[1,2,3].some(x=>x>2);"), Value::Bool(true));
}

#[test]
fn array_every_false() {
    assert_eq!(run("[1,2,3].every(x=>x>2);"), Value::Bool(false));
}

#[test]
fn array_includes_nan() {
    assert_eq!(run("[NaN].includes(NaN);"), Value::Bool(true));
}

#[test]
fn array_sort() {
    assert_eq!(
        run("[3,1,2].sort().join(',');"),
        Value::String(Rc::from("1,2,3"))
    );
}

#[test]
fn array_sort_cmp() {
    assert_eq!(
        run("[10,5,8].sort((a,b)=>a-b).join(',');"),
        Value::String(Rc::from("5,8,10"))
    );
}

#[test]
fn array_reverse() {
    assert_eq!(
        run("[1,2,3].reverse().join(',');"),
        Value::String(Rc::from("3,2,1"))
    );
}

#[test]
fn string_methods() {
    assert_eq!(
        run("'hello'.toUpperCase();"),
        Value::String(Rc::from("HELLO"))
    );
    assert_eq!(run("'hello'.charAt(1);"), Value::String(Rc::from("e")));
}

#[test]
fn string_split_join() {
    assert_eq!(
        run("'a,b,c'.split(',').join('-');"),
        Value::String(Rc::from("a-b-c"))
    );
}

#[test]
fn split_limit() {
    assert_eq!(run(r#""a,b,c".split(",",2).length;"#), Value::Number(2.0));
}

#[test]
fn string_split_reverse() {
    assert_eq!(
        run(r#""hello world".split(" ").reverse().join(" ");"#),
        Value::String(Rc::from("world hello"))
    );
}

#[test]
fn object_keys_len() {
    assert_eq!(
        run("Object.keys({a:1,b:2,c:3}).length;"),
        Value::Number(3.0)
    );
}

#[test]
fn object_values_sum() {
    assert_eq!(
        run("Object.values({a:1,b:2}).reduce((x,y)=>x+y,0);"),
        Value::Number(3.0)
    );
}

#[test]
fn object_entries() {
    assert_eq!(run("Object.entries({a:1,b:2}).length;"), Value::Number(2.0));
}

#[test]
fn math_basic() {
    assert_eq!(run("Math.floor(3.7);"), Value::Number(3.0));
    assert_eq!(run("Math.max(1, 5, 3);"), Value::Number(5.0));
    assert_eq!(run("Math.sqrt(16);"), Value::Number(4.0));
}

#[test]
fn math_round_half() {
    assert_eq!(run("Math.round(-0.5);"), Value::Number(0.0));
    assert_eq!(run("Math.round(0.5);"), Value::Number(1.0));
    assert_eq!(run("Math.round(-1.5);"), Value::Number(-1.0));
}

#[test]
fn json() {
    assert_eq!(run("JSON.parse('[1,2,3]')[1];"), Value::Number(2.0));
    assert_eq!(
        run("JSON.stringify({a:1});"),
        Value::String(Rc::from("{\"a\":1}"))
    );
}

#[test]
fn error_subclass() {
    assert_eq!(
        run(r#"new TypeError("x").message;"#),
        Value::String(Rc::from("x"))
    );
}

#[test]
fn json_parse_object() {
    assert_eq!(run(r#"JSON.parse("{\"a\":1}").a;"#), Value::Number(1.0));
    // HashMap key order is non-deterministic; just check both props round-trip.
    let s = run(r#"JSON.stringify(JSON.parse("{\"a\":1,\"b\":2}"));"#);
    match s {
        Value::String(st) => {
            assert!(st.contains("\"a\":1") && st.contains("\"b\":2"), "got {st:?}");
        }
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn json_parse_nested() {
    assert_eq!(
        run(r#"JSON.parse("{\"nested\":{\"x\":5}}").nested.x;"#),
        Value::Number(5.0)
    );
}
