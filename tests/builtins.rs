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
            assert!(
                st.contains("\"a\":1") && st.contains("\"b\":2"),
                "got {st:?}"
            );
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

// JSON.stringify circular references

#[test]
fn json_stringify_circular_object() {
    // {name:"a", self: <cycle>}: stringify should fail (undefined on error).
    let r = run("var a = {name:'a'}; a.self = a; JSON.stringify(a);");
    assert_eq!(r, Value::Undefined);
}

#[test]
fn json_stringify_circular_array() {
    let r = run("var a = [1,2,3]; a.push(a); JSON.stringify(a);");
    assert_eq!(r, Value::Undefined);
}

#[test]
fn json_stringify_shared_reference_ok() {
    // shared (non-cyclic) references must still serialize both occurrences.
    assert_eq!(
        run("var s = {v:1}; var t = {l:s, r:s}; JSON.stringify(t);"),
        Value::String(Rc::from(r#"{"l":{"v":1},"r":{"v":1}}"#))
    );
}

#[test]
fn json_stringify_nested_object() {
    assert_eq!(
        run(r#"JSON.stringify({a:1, b:"hi", c:[1,2], d:{e:true}});"#),
        Value::String(Rc::from(r#"{"a":1,"b":"hi","c":[1,2],"d":{"e":true}}"#))
    );
}

// Object property insertion order (now preserved via IndexMap)

#[test]
fn object_keys_insertion_order() {
    let r = match run("Object.keys({z:1, a:2, m:3, b:4}).join(',')") {
        Value::String(s) => s.to_string(),
        v => format!("{:?}", v),
    };
    assert_eq!(r, "z,a,m,b");
}

#[test]
fn for_in_insertion_order() {
    let src = "var o = {a:1,b:2,c:3,d:4,e:5}; var k=[]; for (var x in o) k.push(x); k.join(',');";
    assert_eq!(run(src), Value::String(Rc::from("a,b,c,d,e")));
}

#[test]
fn object_entries_insertion_order() {
    let src = "Object.entries({z:1,a:2,m:3,b:4}).map(e=>e[0]+'='+e[1]).join(',');";
    assert_eq!(run(src), Value::String(Rc::from("z=1,a=2,m=3,b=4")));
}

#[test]
fn json_stringify_key_order() {
    // JSON.stringify now preserves insertion order.
    assert_eq!(
        run(r#"JSON.stringify({a:1, b:"hi", c:[1,2], d:{e:true}});"#),
        Value::String(Rc::from(r#"{"a":1,"b":"hi","c":[1,2],"d":{"e":true}}"#))
    );
}
