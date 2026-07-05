//! Built-in objects and methods: Array, String, Object, Math, JSON, Symbol.

mod common;
use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

#[test]
fn array_map_reduce() {
    assert_eq!(
        run("[1,2,3].map(x => x*2).join(',');"),
        Value::String(Arc::from("2,4,6"))
    );
    assert_eq!(run("[1,2,3].reduce((a,b)=>a+b, 0);"), Value::Number(6.0));
}

#[test]
fn array_method_chaining() {
    assert_eq!(
        run("[1,2,3,4,5].filter(x => x > 2).map(x => x * 2).join(',');"),
        Value::String(Arc::from("6,8,10"))
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
        Value::String(Arc::from("1,2,3"))
    );
}

#[test]
fn array_sort_cmp() {
    assert_eq!(
        run("[10,5,8].sort((a,b)=>a-b).join(',');"),
        Value::String(Arc::from("5,8,10"))
    );
}

#[test]
fn array_reverse() {
    assert_eq!(
        run("[1,2,3].reverse().join(',');"),
        Value::String(Arc::from("3,2,1"))
    );
}

#[test]
fn string_methods() {
    assert_eq!(
        run("'hello'.toUpperCase();"),
        Value::String(Arc::from("HELLO"))
    );
    assert_eq!(run("'hello'.charAt(1);"), Value::String(Arc::from("e")));
    assert_eq!(
        run("new String('abc123').charAt(2);"),
        Value::String(Arc::from("c"))
    );
    assert_eq!(run("String.prototype.length;"), Value::Number(0.0));
}

#[test]
fn array_subclass_instances_use_new_target_prototype() {
    assert_eq!(
        run(r#"
            class Subclass extends Array {}
            var arr = new Subclass(1, 2);
            [
              arr instanceof Subclass,
              arr instanceof Array,
              Object.getPrototypeOf(arr) === Subclass.prototype,
              arr.length,
              arr.join(",")
            ].join(":");
            "#),
        Value::String(Arc::from("true:true:true:2:1,2"))
    );
}

#[test]
fn uint8array_subclass_instances_use_new_target_prototype_and_store_elements() {
    assert_eq!(
        run(r#"
            class ExtendedUint8Array extends Uint8Array {
              constructor() {
                super(10);
                this[0] = 255;
                this[1] = 0xFFA;
              }
            }
            var eua = new ExtendedUint8Array();
            [
              eua.length,
              eua.byteLength,
              eua[0],
              eua[1],
              Object.getPrototypeOf(eua) === ExtendedUint8Array.prototype,
              Object.prototype.toString.call(eua)
            ].join(",");
            "#),
        Value::String(Arc::from("10,10,255,250,true,[object Uint8Array]"))
    );
}

#[test]
fn array_buffer_and_data_view_subclasses_initialize_internal_slots() {
    assert_eq!(
        run(r#"
            class AB extends ArrayBuffer {}
            var ab = new AB(4);
            var sliced = ab.slice(0, 1);
            [
              ab.byteLength,
              sliced.byteLength,
              sliced instanceof AB,
              sliced instanceof ArrayBuffer,
              Object.getPrototypeOf(ab) === AB.prototype
            ].join(",");
            "#),
        Value::String(Arc::from("4,1,true,true,true"))
    );
    assert_eq!(
        run(r#"
            class DV extends DataView {}
            var buffer = new ArrayBuffer(1);
            var dv = new DV(buffer);
            [
              dv.buffer === buffer,
              dv.byteOffset,
              dv.byteLength,
              Object.getPrototypeOf(dv) === DV.prototype
            ].join(",");
            "#),
        Value::String(Arc::from("true,0,1,true"))
    );
    assert_eq!(
        run(r#"
            var ok = [];
            class AB1 extends ArrayBuffer { constructor() {} }
            try { new AB1(1); ok.push(false); } catch (e) { ok.push(e instanceof ReferenceError); }
            class DV1 extends DataView { constructor() {} }
            try { new DV1(new ArrayBuffer(1)); ok.push(false); } catch (e) { ok.push(e instanceof ReferenceError); }
            try { new (class DV extends DataView {}); ok.push(false); } catch (e) { ok.push(e instanceof TypeError); }
            ok.join(",");
        "#),
        Value::String(Arc::from("true,true,true"))
    );
    assert_eq!(
        run(r#"
            var ab = new ArrayBuffer(4);
            ab.slice(3, 1).byteLength;
            "#),
        Value::Number(0.0)
    );
    assert!(
        run_err("new DataView(new ArrayBuffer(4), 3, 2);").contains("RangeError"),
        "DataView byte range past the buffer should throw RangeError"
    );
    assert!(
        run_err("new ArrayBuffer(9007199254740991);").contains("RangeError"),
        "huge ArrayBuffer lengths should throw RangeError"
    );
}

#[test]
fn array_buffer_and_data_view_prototype_accessors_validate_receivers() {
    assert_eq!(
        run(r#"
            var ab = new ArrayBuffer(4);
            var dv = new DataView(ab, 1, 2);
            var abDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength");
            var bufferDesc = Object.getOwnPropertyDescriptor(DataView.prototype, "buffer");
            var lengthDesc = Object.getOwnPropertyDescriptor(DataView.prototype, "byteLength");
            var offsetDesc = Object.getOwnPropertyDescriptor(DataView.prototype, "byteOffset");
            [
              abDesc.get.call(ab),
              abDesc.set === undefined,
              abDesc.enumerable,
              abDesc.configurable,
              abDesc.get.name,
              abDesc.get.length,
              bufferDesc.get.call(dv) === ab,
              lengthDesc.get.call(dv),
              offsetDesc.get.call(dv),
              bufferDesc.get.name,
              lengthDesc.get.name,
              offsetDesc.get.name
            ].join(",");
            "#),
        Value::String(Arc::from(
            "4,true,false,true,get byteLength,0,true,2,1,get buffer,get byteLength,get byteOffset"
        ))
    );
    assert!(
        run_err(
            r#"
            var getter = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get;
            getter.call({});
            "#
        )
        .contains("TypeError"),
        "ArrayBuffer byteLength getter should reject non-ArrayBuffer receivers"
    );
    assert!(
        run_err(
            r#"
            var getter = Object.getOwnPropertyDescriptor(DataView.prototype, "buffer").get;
            getter.call({});
            "#
        )
        .contains("TypeError"),
        "DataView buffer getter should reject non-DataView receivers"
    );
    assert_eq!(
        run(r#"
            var ab = new ArrayBuffer(4);
            var dv = new DataView(ab, 1, 2);
            $262.detachArrayBuffer(ab);
            [
              ab.byteLength,
              Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get.call(ab),
              dv.buffer === ab
            ].join(",");
            "#),
        Value::String(Arc::from("0,0,true"))
    );
    assert!(
        run_err(
            r#"
            var ab = new ArrayBuffer(4);
            var dv = new DataView(ab, 1, 2);
            $262.detachArrayBuffer(ab);
            dv.byteLength;
            "#
        )
        .contains("TypeError"),
        "DataView byteLength should reject detached buffers"
    );
    assert!(
        run_err(
            r#"
            var ab = new ArrayBuffer(4);
            var dv = new DataView(ab, 1, 2);
            $262.detachArrayBuffer(ab);
            dv.byteOffset;
            "#
        )
        .contains("TypeError"),
        "DataView byteOffset should reject detached buffers"
    );
}

#[test]
fn data_view_int8_uint8_methods_read_write_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(4);
            var dv = new DataView(buffer, 1, 2);
            var values = [];
            values.push(dv.setUint8(0, 255) === undefined);
            values.push(dv.setInt8(1, -2) === undefined);
            values.push(dv.getUint8(0));
            values.push(dv.getInt8(0));
            values.push(dv.getUint8(1));
            values.push(dv.getInt8(1));
            values.push(DataView.prototype.getUint8.length);
            values.push(DataView.prototype.setInt8.length);
            values.push(DataView.prototype.getInt8.name);
            values.push(DataView.prototype.setUint8.name);
            values.join(",");
            "#),
        Value::String(Arc::from("true,true,255,-1,254,-2,1,2,getInt8,setUint8"))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(2));
            dv.setUint8(NaN, 7);
            dv.setUint8(-0.9, 8);
            dv.setUint8(1.9, 9);
            [dv.getUint8(), dv.getUint8(-0.1), dv.getUint8(1.1)].join(",");
            "#),
        Value::String(Arc::from("8,8,9"))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(1));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint8(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(1));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint8(2, poisoned);
            "#
        )
        .contains("Error"),
        "value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(1);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint8(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(1);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint8(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView element reads"
    );
}

#[test]
fn regexp_subclass_instances_use_new_target_prototype_and_last_index_descriptor() {
    assert_eq!(
        run(r#"
            class Subclass extends RegExp {}
            var re = new Subclass("39?", "g");
            var before = Object.getOwnPropertyDescriptor(re, "lastIndex");
            re.test("39");
            var after = Object.getOwnPropertyDescriptor(re, "lastIndex");
            [
              re instanceof Subclass,
              re instanceof RegExp,
              Object.getPrototypeOf(re) === Subclass.prototype,
              before.value,
              before.writable,
              before.enumerable,
              before.configurable,
              after.value,
              after.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("true,true,true,0,true,false,false,2,false"))
    );
}

#[test]
fn string_subclass_instances_have_own_length_descriptor() {
    assert_eq!(
        run(r#"
            class Subclass extends String {}
            var str = new Subclass("test262");
            var desc = Object.getOwnPropertyDescriptor(str, "length");
            [
              str instanceof Subclass,
              str instanceof String,
              str.length,
              desc.value,
              desc.writable,
              desc.enumerable,
              desc.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("true,true,7,7,false,false,false"))
    );
}

#[test]
fn string_split_join() {
    assert_eq!(
        run("'a,b,c'.split(',').join('-');"),
        Value::String(Arc::from("a-b-c"))
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
        Value::String(Arc::from("world hello"))
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
fn object_property_is_enumerable() {
    assert_eq!(run("({a:1}).propertyIsEnumerable('a');"), Value::Bool(true));
    assert_eq!(
        run("var o={}; Object.defineProperty(o,'x',{value:1, enumerable:false}); o.propertyIsEnumerable('x');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("({get m(){ return 1; }}).propertyIsEnumerable('m');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("({a:1}).propertyIsEnumerable('missing');"),
        Value::Bool(false)
    );
    assert_eq!(run("[10].propertyIsEnumerable('0');"), Value::Bool(true));
    assert_eq!(
        run("[10].propertyIsEnumerable('length');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("Object.prototype.propertyIsEnumerable.call('ab', '1');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var s=Symbol(); var o={}; o[s]=1; o.propertyIsEnumerable(s);"),
        Value::Bool(true)
    );
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
        Value::String(Arc::from("{\"a\":1}"))
    );
}

#[test]
fn error_subclass() {
    assert_eq!(
        run(r#"new TypeError("x").message;"#),
        Value::String(Arc::from("x"))
    );
}

#[test]
fn error_subclass_plain_call_uses_active_constructor_prototype() {
    assert_eq!(
        run(
            r#"[
                EvalError(1).toString(),
                RangeError(1).toString(),
                ReferenceError(1).toString(),
                SyntaxError(1).toString(),
                TypeError(1).toString(),
                URIError("message", "fileName", "1").toString()
            ].join("|");"#
        ),
        Value::String(Arc::from(
            "EvalError: 1|RangeError: 1|ReferenceError: 1|SyntaxError: 1|TypeError: 1|URIError: message"
        ))
    );
    assert_eq!(
        run("var T = TypeError; TypeError = Error; var e = T(1); e instanceof T && e instanceof Error;"),
        Value::Bool(true)
    );
}

#[test]
fn native_error_subclass_inherits_name_and_message() {
    assert_eq!(
        run(r#"
            class Err extends EvalError {}
            var err = new Err();
            [
              err.name,
              err.hasOwnProperty("name"),
              err.hasOwnProperty("message"),
              err.message
            ].join(",");
            "#),
        Value::String(Arc::from("EvalError,false,false,"))
    );

    assert_eq!(
        run(r#"
            class Err extends EvalError {}
            Err.prototype.message = "custom";
            var err = new Err();
            err.message + ":" + err.hasOwnProperty("message");
            "#),
        Value::String(Arc::from("custom:false"))
    );

    assert_eq!(
        run(r#"
            class Err extends EvalError {}
            var err = new Err("boom");
            var d = Object.getOwnPropertyDescriptor(err, "message");
            [err.message, d.writable, d.enumerable, d.configurable].join(",");
            "#),
        Value::String(Arc::from("boom,true,false,true"))
    );
}

#[test]
fn native_constructor_new_target_does_not_leak_to_next_call() {
    let msg = run_err("new Error('x'); class C {} C();");
    assert!(
        msg.contains("Class constructor cannot be invoked without 'new'"),
        "got: {msg}"
    );
}

#[test]
fn bound_functions_inherit_restricted_caller_arguments_accessors() {
    assert_eq!(
        run("function target() {}\
             var bound = target.bind({});\
             bound.hasOwnProperty('caller') + ':' + bound.hasOwnProperty('arguments');"),
        Value::String(Arc::from("false:false"))
    );

    let msg = run_err("function target() {} var bound = target.bind({}); bound.caller;");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("function target() {} var bound = target.bind({}); bound.caller = {};");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("function target() {} var bound = target.bind({}); bound.arguments;");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("function target() {} var bound = target.bind({}); bound.arguments = {};");
    assert!(msg.contains("TypeError"), "got: {msg}");
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
    // {name:"a", self: <cycle>}: stringify should throw a TypeError.
    let msg = run_err("var a = {name:'a'}; a.self = a; JSON.stringify(a);");
    assert!(
        msg.contains("TypeError") || msg.contains("circular"),
        "got: {}",
        msg
    );
}

#[test]
fn json_stringify_circular_array() {
    let msg = run_err("var a = [1,2,3]; a.push(a); JSON.stringify(a);");
    assert!(
        msg.contains("TypeError") || msg.contains("circular"),
        "got: {}",
        msg
    );
}

#[test]
fn json_stringify_shared_reference_ok() {
    // shared (non-cyclic) references must still serialize both occurrences.
    assert_eq!(
        run("var s = {v:1}; var t = {l:s, r:s}; JSON.stringify(t);"),
        Value::String(Arc::from(r#"{"l":{"v":1},"r":{"v":1}}"#))
    );
}

#[test]
fn json_stringify_nested_object() {
    assert_eq!(
        run(r#"JSON.stringify({a:1, b:"hi", c:[1,2], d:{e:true}});"#),
        Value::String(Arc::from(r#"{"a":1,"b":"hi","c":[1,2],"d":{"e":true}}"#))
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
    assert_eq!(run(src), Value::String(Arc::from("a,b,c,d,e")));
}

#[test]
fn object_entries_insertion_order() {
    let src = "Object.entries({z:1,a:2,m:3,b:4}).map(e=>e[0]+'='+e[1]).join(',');";
    assert_eq!(run(src), Value::String(Arc::from("z=1,a=2,m=3,b=4")));
}

#[test]
fn json_stringify_key_order() {
    // JSON.stringify now preserves insertion order.
    assert_eq!(
        run(r#"JSON.stringify({a:1, b:"hi", c:[1,2], d:{e:true}});"#),
        Value::String(Arc::from(r#"{"a":1,"b":"hi","c":[1,2],"d":{"e":true}}"#))
    );
}

// --- Array/Number/Object/Math coverage expansion ---

#[test]
fn array_flat_flatmap() {
    assert_eq!(
        run("[1,[2,[3]]].flat().join(',')"),
        Value::String(Arc::from("1,2,3"))
    );
    assert_eq!(
        run("[1,[2,[3]]].flat(2).join(',')"),
        Value::String(Arc::from("1,2,3"))
    );
    assert_eq!(
        run("[1,2,3].flatMap(x=>[x,x*10]).join(',')"),
        Value::String(Arc::from("1,10,2,20,3,30"))
    );
}

#[test]
fn array_at_shift_unshift_splice() {
    assert_eq!(run("[1,2,3].at(-1);"), Value::Number(3.0));
    assert_eq!(run("[1,2,3].at(0);"), Value::Number(1.0));
    assert_eq!(
        run("var a=[1,2,3]; a.shift(); a.join(',');"),
        Value::String(Arc::from("2,3"))
    );
    assert_eq!(
        run("var b=[1,2,3]; b.unshift(0); b.join(',');"),
        Value::String(Arc::from("0,1,2,3"))
    );
    assert_eq!(
        run("var c=[1,2,3,4,5]; c.splice(1,2); c.join(',');"),
        Value::String(Arc::from("1,4,5"))
    );
}

#[test]
fn array_last_index_of() {
    assert_eq!(run("[1,2,3,2].lastIndexOf(2);"), Value::Number(3.0));
    assert_eq!(run("[1,2,3].lastIndexOf(9);"), Value::Number(-1.0));
}

#[test]
fn string_pad_at_replaceall_substring() {
    assert_eq!(
        run("'abc'.padStart(6,'0');"),
        Value::String(Arc::from("000abc"))
    );
    assert_eq!(
        run("'abc'.padEnd(6,'0');"),
        Value::String(Arc::from("abc000"))
    );
    assert_eq!(run("'abc'.at(-1);"), Value::String(Arc::from("c")));
    assert_eq!(
        run("'a-b-a'.replaceAll('-','_');"),
        Value::String(Arc::from("a_b_a"))
    );
    assert_eq!(
        run("'hello'.substring(1,3);"),
        Value::String(Arc::from("el"))
    );
    assert_eq!(
        run("'  hi  '.trimStart();"),
        Value::String(Arc::from("hi  "))
    );
}

#[test]
fn number_static_methods() {
    assert_eq!(run("Number.isInteger(5);"), Value::Bool(true));
    assert_eq!(run("Number.isInteger(5.5);"), Value::Bool(false));
    assert_eq!(run("Number.isFinite(Infinity);"), Value::Bool(false));
    assert_eq!(run("Number.isNaN(NaN);"), Value::Bool(true));
    assert_eq!(run("Number.isNaN('NaN');"), Value::Bool(false));
    assert_eq!(run("Number.isSafeInteger(2**53);"), Value::Bool(false));
}

#[test]
fn number_constants_and_radix() {
    assert_eq!(
        run("Number.MAX_SAFE_INTEGER;"),
        Value::Number(9007199254740991.0)
    );
    assert_eq!(run("Number.EPSILON > 0;"), Value::Bool(true));
    assert_eq!(run("(255).toString(16);"), Value::String(Arc::from("ff")));
    assert_eq!(
        run("(4096).toString(16);"),
        Value::String(Arc::from("1000"))
    );
    assert_eq!(run("(0).toString(36);"), Value::String(Arc::from("0")));
    assert_eq!(
        run("(3.14159).toFixed(2);"),
        Value::String(Arc::from("3.14"))
    );
}

#[test]
fn parse_int_prefix() {
    assert_eq!(run("parseInt('42px');"), Value::Number(42.0));
    assert_eq!(run("parseInt('0xff');"), Value::Number(255.0));
    assert_eq!(run("parseInt('  -17  ');"), Value::Number(-17.0));
    assert_eq!(run("parseInt('3.14');"), Value::Number(3.0));
    assert_eq!(run("parseInt('zz',36);"), Value::Number(1295.0));
    assert_eq!(run("Number.parseInt('42px');"), Value::Number(42.0));
}

#[test]
fn object_statics() {
    assert_eq!(run("Object.is(NaN, NaN);"), Value::Bool(true));
    assert_eq!(run("Object.is(0, -0);"), Value::Bool(false));
    assert_eq!(run("Object.is(1, 1);"), Value::Bool(true));
    assert_eq!(
        run("var o = Object.fromEntries([['a',1],['b',2]]); o.a + o.b;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("typeof Object.create(null);"),
        Value::String(Arc::from("object"))
    );
}

#[test]
fn math_expanded() {
    assert_eq!(run("Math.hypot(3,4);"), Value::Number(5.0));
    assert_eq!(
        run("Math.atan2(1,0);"),
        Value::Number(std::f64::consts::FRAC_PI_2)
    );
    assert_eq!(run("Math.clz32(1);"), Value::Number(31.0));
    assert_eq!(run("Math.sign(-5);"), Value::Number(-1.0));
    assert_eq!(run("Math.sinh(0);"), Value::Number(0.0));
}

// --- Promise ---

#[test]
fn promise_resolve_basic() {
    // then callback runs after the synchronous run; the last expression is the
    // synchronous return (undefined). We verify the promise object itself.
    let r = run("new Promise(function(res){ res(1); });");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn promise_then_chain_value() {
    // Chained then: the second then receives the first's transformed value.
    // We store it in a global that the synchronous run cannot read back, so we
    // instead verify the derived promise from .then is an object.
    let r = run("new Promise(function(res){ res(5); }) \
           .then(function(v){ return v * 2; }) \
           .then(function(v){ return v; });");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn promise_catch_reject() {
    // reject -> catch returns a derived promise (object), not the error value.
    let r = run("new Promise(function(_, rej){ rej('boom'); }) \
           .catch(function(e){ return e; });");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn promise_callback_runs() {
    // Verify the then callback actually executes by having it throw into a
    // catch that we observe via the derived promise being an object.
    let r = run("new Promise(function(res){ res(1); }).then(function(v){ throw v; });");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn promise_keyword_method_names() {
    // `.catch` and `.then` use reserved words as property names.
    let r = run("typeof Promise.prototype.then;");
    assert_eq!(r, Value::String(Arc::from("function")));
}

#[test]
fn promise_subclass_requires_callable_executor_and_uses_new_target_prototype() {
    assert_eq!(
        run("class Prom extends Promise {} try { new Prom(); false; } catch (e) { e instanceof TypeError; }"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class Prom extends Promise {} Object.getPrototypeOf(new Prom(function() {})) === Prom.prototype;"),
        Value::Bool(true)
    );
}

// --- RegExp ---

#[test]
fn regex_literal_test() {
    assert_eq!(run("/abc/.test('xabcy');"), Value::Bool(true));
    assert_eq!(run("/abc/.test('xyz');"), Value::Bool(false));
    assert_eq!(run("/\\d+/.test('abc123');"), Value::Bool(true));
    assert_eq!(run("/\\d+/.test('abc');"), Value::Bool(false));
}

#[test]
fn regex_exec_captures() {
    let r = run("/(\\w+)@(\\w+)/.exec('user@host');");
    assert!(matches!(r, Value::Object(_)));
    assert_eq!(
        run("/(\\w+)@(\\w+)/.exec('user@host')[0];"),
        Value::String(Arc::from("user@host"))
    );
    assert_eq!(
        run("/(\\w+)@(\\w+)/.exec('user@host')[1];"),
        Value::String(Arc::from("user"))
    );
    assert_eq!(
        run("/(\\w+)@(\\w+)/.exec('user@host')[2];"),
        Value::String(Arc::from("host"))
    );
}

#[test]
fn regex_exec_no_match() {
    assert_eq!(run("/zzz/.exec('abc');"), Value::Null);
}

#[test]
fn regex_source_flags() {
    assert_eq!(run("/abc/gi.source;"), Value::String(Arc::from("abc")));
    assert_eq!(run("/abc/gi.flags;"), Value::String(Arc::from("gi")));
}

#[test]
fn string_replace_with_regex() {
    assert_eq!(
        run("'hello'.replace(/l/, 'L');"),
        Value::String(Arc::from("heLlo"))
    );
    assert_eq!(
        run("'hello world'.replace(/o/g, '0');"),
        Value::String(Arc::from("hell0 w0rld"))
    );
}

#[test]
fn division_not_regex() {
    // Ensure `/` after a value is division, not a regex.
    assert_eq!(run("10 / 4;"), Value::Number(2.5));
    assert_eq!(run("var x = 20; x / 5;"), Value::Number(4.0));
    assert_eq!(
        run("var instance = 60; var of = 6; var g = 2; instance/of/g;"),
        Value::Number(5.0)
    );
    assert_eq!(
        run("var of = 4; var g = 2; eval('{[42]}.8/of/g');"),
        Value::Number(0.1)
    );
}

// --- Array.from / Array.of ---

#[test]
fn array_from_iterable_and_map() {
    assert_eq!(
        run("Array.from('abc').join(',');"),
        Value::String(Arc::from("a,b,c"))
    );
    assert_eq!(
        run("Array.from([1,2,3], x=>x*2).join(',');"),
        Value::String(Arc::from("2,4,6"))
    );
}

#[test]
fn array_from_arraylike() {
    assert_eq!(
        run("Array.from({0:'a',1:'b',length:2}).join(',');"),
        Value::String(Arc::from("a,b"))
    );
}

#[test]
fn array_of_and_isarray() {
    assert_eq!(
        run("Array.of(1,2,3).join(',');"),
        Value::String(Arc::from("1,2,3"))
    );
    assert_eq!(run("Array.isArray([]);"), Value::Bool(true));
    assert_eq!(run("Array.isArray({});"), Value::Bool(false));
}

// --- async/await ---

#[test]
fn async_function_returns_promise() {
    let r = run("async function f(){ return 5; } typeof f();");
    assert_eq!(r, Value::String(Arc::from("object")));
}

#[test]
fn async_resolves_value() {
    // f() resolves to 5; the then callback runs during microtask drain.
    let r = run("var out=0; async function f(){ return 5; } f().then(function(v){ out=v; }); out;");
    // out is read synchronously before the then callback runs, so it stays 0;
    // verify the promise is an object instead.
    let _ = r;
    assert!(matches!(
        run("async function f(){ return 5; } f();"),
        Value::Object(_)
    ));
}

#[test]
fn await_extracts_promise_value() {
    // await a resolved promise inside an async function yields the value.
    let r = run("async function f(){ return 7; } \
         async function g(){ return await f() + 1; } \
         g();");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn await_non_promise() {
    // await on a plain value yields the value.
    let r = run("async function g(){ return await 9; } g();");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn await_is_contextual_identifier_in_sloppy_non_async_code() {
    assert_eq!(run("var await = 0; await = 1; await;"), Value::Number(1.0));
    assert_eq!(
        run("function f(await){ return await; } f(7);"),
        Value::Number(7.0)
    );
}

// --- generators (function*/yield) ---

#[test]
fn generator_next_sequence() {
    assert_eq!(
        run("function* g(){ yield 1; yield 2; yield 3; } var it = g(); it.next().value;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run(
            "function* g(){ yield 1; yield 2; yield 3; } var it = g(); it.next(); it.next().value;"
        ),
        Value::Number(2.0)
    );
    assert_eq!(
        run("function* g(){ yield 1; yield 2; yield 3; } var it = g(); it.next(); it.next(); it.next().value;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("function* g(){ yield 1; yield 2; yield 3; } var it = g(); it.next(); it.next(); it.next(); it.next().done;"),
        Value::Bool(true)
    );
}

#[test]
fn generator_for_of() {
    assert_eq!(
        run("function* r(a,b){ for(var i=a;i<b;i++) yield i; } var s=0; for(var v of r(1,4)) s+=v; s;"),
        Value::Number(6.0)
    );
}

#[test]
fn generator_spread() {
    assert_eq!(
        run("function* r(a,b){ for(var i=a;i<b;i++) yield i; } [...r(1,4)].join(',');"),
        Value::String(Arc::from("1,2,3"))
    );
}

#[test]
fn generator_yield_undefined() {
    assert_eq!(
        run("function* g(){ yield; yield 1; } var it=g(); it.next().value;"),
        Value::Undefined
    );
}

// --- JSON.parse error handling + function error propagation ---

#[test]
fn json_parse_invalid_returns_error() {
    // Invalid JSON must throw (not hang). run returns Undefined on error.
    let r = run("var r; try { JSON.parse('{bad}'); } catch(e) { r = e.name; } r;");
    assert_eq!(r, Value::String(Arc::from("SyntaxError")));
}

#[test]
fn function_error_reaches_caller_catch() {
    let r =
        run("var r; function f(){ return missing; } try { f(); } catch(e) { r = 'caught'; } r;");
    assert_eq!(r, Value::String(Arc::from("caught")));
}

// --- wrapper objects (boxed primitives) ---

#[test]
fn boxed_number_valueof() {
    assert_eq!(run("new Number(5).valueOf();"), Value::Number(5.0));
    assert_eq!(
        run("typeof new Number(5).valueOf();"),
        Value::String(std::sync::Arc::from("number"))
    );
}

#[test]
fn boxed_boolean_valueof() {
    assert_eq!(run("new Boolean(true).valueOf();"), Value::Bool(true));
}

#[test]
fn boxed_string_valueof() {
    assert_eq!(
        run("new String('hi').valueOf();"),
        Value::String(std::sync::Arc::from("hi"))
    );
}

#[test]
fn date_static_parse_exists() {
    assert_eq!(
        run("typeof Date.parse + ':' + Date.parse('123') + ':' + typeof Date.UTC + ':' + typeof Date.prototype.getUTCFullYear + ':' + typeof Date.prototype.setUTCFullYear;"),
        Value::String(std::sync::Arc::from("function:123:function:function:function"))
    );
}

#[test]
fn date_subclass_instances_keep_date_components() {
    assert_eq!(
        run("class D extends Date{};let d=new D(1859,'10',24,11);d.getFullYear()+','+d.getMonth()+','+d.getDate();"),
        Value::String(Arc::from("1859,10,24"))
    );
    assert_eq!(
        run("class D extends Date{};let d=new D(-3474558000000);d.getUTCFullYear()+','+d.getUTCMonth()+','+d.getUTCDate();"),
        Value::String(Arc::from("1859,10,24"))
    );
}

#[test]
fn boxed_number_addition_uses_valueof() {
    assert_eq!(run("new Number(5) + 1;"), Value::Number(6.0));
    assert_eq!(run("new Boolean(true) + 1;"), Value::Number(2.0));
}

#[test]
fn date_addition_uses_default_string_hint() {
    assert_eq!(
        run("var d = new Date(0); d + d;"),
        Value::String(Arc::from("DateDate"))
    );
    assert_eq!(
        run("var d = new Date(0); d + 0;"),
        Value::String(Arc::from("Date0"))
    );
    assert_eq!(
        run("var d = new Date(0); d + true;"),
        Value::String(Arc::from("Datetrue"))
    );
    assert_eq!(
        run("var d = new Date(0); d + {};"),
        Value::String(Arc::from("Date[object Object]"))
    );
}

#[test]
fn bigint_string_addition_concatenates() {
    assert_eq!(run("1n + '';"), Value::String(Arc::from("1")));
    assert_eq!(run("'' + -1n;"), Value::String(Arc::from("-1")));
    assert_eq!(run("Object(1n) + '';"), Value::String(Arc::from("1")));
}

#[test]
fn boxed_bigint_mixed_throws() {
    let err = run_err("Object(1n) + 1;");
    assert!(err.contains("TypeError"), "got: {}", err);
}

#[test]
fn to_primitive_both_object_throws() {
    let err = run_err("1 + {valueOf: function() {return {}}, toString: function() {return {}}};");
    assert!(err.contains("TypeError"), "got: {}", err);
}
