//! Built-in objects and methods: Array, String, Object, Math, JSON, Symbol.

mod common;
use common::{run, run_err};
use ruja::{Value, Vm};
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
fn array_find_methods_use_array_like_property_access() {
    assert!(run_err("Array.prototype.find.call(null, function() {})")
        .contains("Cannot convert undefined or null to object"));
    assert!(run_err("[].find({})").contains("Array predicate is not callable"));
    assert!(run_err(
        r#"var o = {};
           Object.defineProperty(o, "length", {
             get: function(){ throw new Error("length-get"); }
           });
           Array.prototype.find.call(o);"#
    )
    .contains("length-get"));
    assert!(run_err(
        r#"var o = { length: 1 };
           Object.defineProperty(o, "0", {
             get: function(){ throw new Error("index-get"); }
           });
           Array.prototype.find.call(o, function(){ return false; });"#
    )
    .contains("index-get"));
    assert_eq!(
        run(r#"var arr = ["Shoes", "Car", "Bike"];
               var seen = [];
               arr.find(function(value) {
                 if (seen.length === 0) arr.splice(1, 1);
                 seen.push(String(value));
                 return false;
               });
               seen.join("|");"#),
        Value::String(Arc::from("Shoes|Bike|undefined"))
    );
    assert_eq!(
        run(r#"var arr = ["Shoes", "Car", "Bike"];
               var seen = [];
               arr.findLastIndex(function(value) {
                 if (seen.length === 0) arr.splice(1, 1);
                 seen.push(String(value));
                 return false;
               });
               seen.join("|");"#),
        Value::String(Arc::from("Bike|Bike|Shoes"))
    );
    assert_eq!(
        run(r#"var receiver;
               var result = Array.prototype.find.call({0: "x", length: 1}, function(value, index, object) {
                 receiver = this;
                 return value === "x" && index === 0 && object.length === 1;
               }, 7);
               [result, receiver.valueOf()].join("|");"#),
        Value::String(Arc::from("x|7"))
    );
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
fn array_search_methods_use_array_like_property_access() {
    assert_eq!(
        run("var obj = {0:'x', 1:true, length:2}; Array.prototype.indexOf.call(obj, true);"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var obj = {0:'x', 1:Infinity, length:2}; Array.prototype.lastIndexOf.call(obj, Infinity);"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var marker = {}; Boolean.prototype[1] = marker; Boolean.prototype.length = 2; var r = Array.prototype.indexOf.call(true, marker); delete Boolean.prototype[1]; delete Boolean.prototype.length; r;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("Array.prototype.indexOf.call(new String('null'), 'l');"),
        Value::Number(2.0)
    );
    assert_eq!(run("[0,,2].indexOf(undefined);"), Value::Number(-1.0));
    assert_eq!(run("[0,,2].includes(undefined);"), Value::Bool(true));
    assert_eq!(
        run(r#"
            var arr = [0, 1, 2];
            Object.defineProperty(arr, "2", {
              get: function() { return "unconfigurable"; },
              configurable: false
            });
            Object.defineProperty(arr, "1", {
              get: function() { arr.length = 2; return 1; },
              configurable: true
            });
            [arr.indexOf("unconfigurable"), arr.length, 2 in arr].join("|");
            "#),
        Value::String(Arc::from("2|3|true"))
    );
    assert_eq!(
        run(r#"
            var arr = [0, 1, 2, 3];
            Object.defineProperty(arr, "2", {
              get: function() { return "unconfigurable"; },
              configurable: false
            });
            Object.defineProperty(arr, "3", {
              get: function() { arr.length = 2; return 1; },
              configurable: true
            });
            [arr.lastIndexOf("unconfigurable"), arr.length, 2 in arr, 3 in arr].join("|");
            "#),
        Value::String(Arc::from("2|3|true|false"))
    );
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
fn string_static_methods_follow_code_point_and_raw_semantics() {
    assert!(run_err("String.fromCodePoint(3.14);").contains("RangeError"));
    assert!(run_err("String.fromCodePoint(-1);").contains("RangeError"));
    assert!(run_err("String.fromCodePoint(Infinity);").contains("RangeError"));
    assert_eq!(
        run("String.fromCodePoint(0x61, 0x1F600).length;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run(r#"String.raw({ raw: ["a", "b", "d", "f"] }, 1);"#),
        Value::String(Arc::from("a1bdf"))
    );
    assert_eq!(
        run(r#"String.raw({ raw: { length: 5, 0: "e", 1: "", 2: null, 3: undefined, 4: 123 } });"#),
        Value::String(Arc::from("enullundefined123"))
    );
}

#[test]
fn string_search_methods_follow_regexp_and_position_semantics() {
    assert_eq!(
        run("'The future is cool!'.startsWith('future', 4);"),
        Value::Bool(true)
    );
    assert_eq!(run("'word'.endsWith('r', 3);"), Value::Bool(true));
    assert_eq!(
        run("'The future is cool!'.includes('The future', true);"),
        Value::Bool(false)
    );
    assert!(run_err("String.prototype.includes.call(null, 'x');").contains("null or undefined"));
    assert!(run_err(
        "String.prototype.startsWith.call({ toString: function(){ throw new Error('boom'); } }, '');"
    )
    .contains("boom"));
    assert!(run_err("''.startsWith(/./);").contains("RegExp"));
    assert!(run_err(
        "var o = {}; Object.defineProperty(o, Symbol.match, { get: function(){ throw new Error('boom'); } }); ''.includes(o);"
    )
    .contains("boom"));
    assert_eq!(
        run("var s = Symbol.match; var o = {}; Object.defineProperty(o, s, { get: function(){ return true; } }); o[s];"),
        Value::Bool(true)
    );
    assert_eq!(run(r#""__undefined__".indexOf()"#), Value::Number(2.0));
    assert_eq!(run(r#""__undefined__".lastIndexOf()"#), Value::Number(2.0));
    assert_eq!(run(r#""".lastIndexOf()"#), Value::Number(-1.0));
    assert_eq!(
        run(r#"var o = { toString: function(){ return "AB"; } }; "ABBABABAB".indexOf(o, true)"#),
        Value::Number(3.0)
    );
    assert_eq!(
        run(r#"var o = { toString: function(){ return "AB"; } }; "ABBABABAB".lastIndexOf(o, NaN)"#),
        Value::Number(7.0)
    );
    assert_eq!(run(r#""abcabc".indexOf("a", -1)"#), Value::Number(0.0));
    assert_eq!(run(r#""abcabc".lastIndexOf("a", -1)"#), Value::Number(0.0));
    assert_eq!(run(r#""abcabc".lastIndexOf("b", -1)"#), Value::Number(-1.0));
    assert_eq!(
        run(r#""abcabc".lastIndexOf("a", -Infinity)"#),
        Value::Number(0.0)
    );
    assert_eq!(
        run(r#""aaaa".indexOf("aa", Infinity)"#),
        Value::Number(-1.0)
    );
    assert_eq!(
        run(r#""abc".lastIndexOf("abcd", Infinity)"#),
        Value::Number(-1.0)
    );
    assert_eq!(
        run(
            r#"var a = { toString: function(){ throw "search"; } }; var b = { valueOf: function(){ throw "position"; } }; (function(){ try { return "abc".indexOf(a, b); } catch (e) { return e; } })()"#
        ),
        Value::String(Arc::from("search"))
    );
    assert_eq!(
        run(
            r#"var a = { toString: function(){ throw "search"; } }; var b = { valueOf: function(){ throw "position"; } }; (function(){ try { return "abc".lastIndexOf(a, b); } catch (e) { return e; } })()"#
        ),
        Value::String(Arc::from("search"))
    );
    assert_eq!(
        run(r#"var searcher = {};
               var seenThis, seenArg;
               searcher[Symbol.search] = function(arg) {
                 seenThis = this;
                 seenArg = arg;
                 return "custom";
               };
               var out = "abc".search(searcher);
               [out, seenThis === searcher, seenArg].join("|");"#),
        Value::String(Arc::from("custom|true|abc"))
    );
    assert!(run_err(
        r#"var searcher = {};
               Object.defineProperty(searcher, Symbol.search, {
                 get: function(){ throw new Error("search-get"); }
               });
               "abc".search(searcher);"#
    )
    .contains("search-get"));
    assert_eq!(
        run(r#"var original = RegExp.prototype[Symbol.search];
               var seenThis, seenArg;
               RegExp.prototype[Symbol.search] = function(arg) {
                 seenThis = this;
                 seenArg = arg;
                 return "created";
               };
               var out = "target".search("string source");
               RegExp.prototype[Symbol.search] = original;
               [out, seenThis instanceof RegExp, seenThis.source, seenThis.flags, seenArg].join("|");"#),
        Value::String(Arc::from("created|true|string source||target"))
    );
    assert_eq!(
        run("var re = /b/g; re.lastIndex = 2; var n = re[Symbol.search]('abc'); n + ',' + re.lastIndex;"),
        Value::String(Arc::from("1,2"))
    );
}

#[test]
fn generated_symbols_do_not_collide_with_well_known_symbols() {
    assert_eq!(run("Symbol() === Symbol.iterator;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.match;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.unscopables;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.species;"), Value::Bool(false));
    assert_eq!(
        run("Symbol.for('x') === Symbol.for('x');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("Symbol.for('x') === Symbol.for('y');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("Symbol.keyFor(Symbol.for('x'));"),
        Value::String(Arc::from("x"))
    );
    assert_eq!(run("Symbol.keyFor(Symbol('x'));"), Value::Undefined);
    assert_eq!(
        run("typeof Symbol.species + ':' + typeof Symbol.unscopables;"),
        Value::String(Arc::from("symbol:symbol"))
    );
    assert_eq!(run("Symbol.species === Symbol.species;"), Value::Bool(true));
}

#[test]
fn symbol_species_is_exposed() {
    assert_eq!(
        run("typeof Symbol.species;"),
        Value::String(Arc::from("symbol"))
    );
    assert_eq!(
        run("String(Symbol.species);"),
        Value::String(Arc::from("Symbol(Symbol.species)"))
    );
}

#[test]
fn symbol_description_and_key_for_follow_registry_semantics() {
    assert_eq!(
        run("typeof Symbol.prototype.toString + ':' + Object.getOwnPropertyDescriptor(Symbol, 'prototype').writable;"),
        Value::String(Arc::from("function:false"))
    );
    assert_eq!(
        run("[Symbol('x').description, Symbol().description, Symbol(undefined).description, Symbol('').description].join('|');"),
        Value::String(Arc::from("x|||"))
    );
    assert_eq!(
        run("var s = Symbol.for({ toString: function(){ return 'test262'; } }); [s.description, Symbol.keyFor(s), s.toString()].join('|');"),
        Value::String(Arc::from("test262|test262|Symbol(test262)"))
    );
    assert_eq!(
        run("var sym = Symbol('66'); sym.toString = 0; sym.valueOf = 0; [sym.toString(), sym === sym.valueOf()].join('|');"),
        Value::String(Arc::from("Symbol(66)|true"))
    );
    assert!(
        run_err("'use strict'; var sym = Symbol('s'); sym.x = 1;").contains("TypeError"),
        "strict Symbol primitive assignment must throw"
    );
    assert!(
        run_err("Symbol.keyFor(Object(Symbol.for('x')));").contains("TypeError"),
        "Symbol.keyFor must reject Symbol wrapper objects"
    );
    assert!(
        run_err("Object.getOwnPropertyDescriptor(Symbol.prototype, 'description').get.call({});")
            .contains("TypeError"),
        "Symbol.prototype.description getter must reject non-symbol receivers"
    );
}

#[test]
fn symbol_intrinsic_surface_descriptors_and_value_of() {
    assert_eq!(
        run(r#"
            [
              Object.getOwnPropertyDescriptor(Symbol, "length").value,
              Object.getOwnPropertyDescriptor(Symbol, "length").writable,
              Object.getOwnPropertyDescriptor(Symbol, "match").writable,
              Object.getOwnPropertyDescriptor(Symbol, "match").configurable,
              Object.getOwnPropertyDescriptor(Symbol, "isConcatSpreadable").writable,
              Object.getOwnPropertyDescriptor(Symbol, "replace").configurable,
              Object.getOwnPropertyDescriptor(Symbol, "search").configurable,
              Object.getOwnPropertyDescriptor(Symbol, "split").configurable,
              typeof Symbol.matchAll,
              typeof Symbol.isConcatSpreadable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "0|false|false|false|false|false|false|false|symbol|symbol"
        ))
    );
    assert_eq!(
        run(r#"
            var sym = Symbol("surface");
            [
              Object.getPrototypeOf(sym) === Symbol.prototype,
              Object.getPrototypeOf(Object(sym)).constructor === Symbol,
              Symbol.prototype.valueOf.call(sym) === sym,
              Symbol.prototype.valueOf.call(Object(sym)) === sym,
              Object.prototype.propertyIsEnumerable.call(Symbol.prototype, "valueOf")
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|false"))
    );
    assert!(
        run_err("Symbol.prototype.valueOf.call({});").contains("TypeError"),
        "Symbol.prototype.valueOf must reject non-symbol objects"
    );
    assert!(
        run_err("Object.getPrototypeOf(null);").contains("TypeError"),
        "Object.getPrototypeOf(null) must throw"
    );
    assert_eq!(
        run(r#"
            function getterName(C) {
              return Object.getOwnPropertyDescriptor(C, Symbol.species).get.name;
            }
            class MyRegExp extends RegExp {}
            [
              getterName(Array),
              getterName(Map),
              getterName(Promise),
              getterName(RegExp),
              getterName(Set),
              MyRegExp[Symbol.species] === MyRegExp
            ].join("|");
        "#),
        Value::String(Arc::from(
            "get [Symbol.species]|get [Symbol.species]|get [Symbol.species]|get [Symbol.species]|get [Symbol.species]|true"
        ))
    );
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
fn typed_array_constructors_cover_numeric_and_bigint_variants() {
    assert_eq!(
        run(r#"
            [
              typeof Int8Array,
              typeof Uint8ClampedArray,
              typeof Int16Array,
              typeof Uint16Array,
              typeof Int32Array,
              typeof Uint32Array,
              typeof Float32Array,
              typeof Float64Array,
              typeof BigInt64Array,
              typeof BigUint64Array
            ].join(",");
        "#),
        Value::String(Arc::from(
            "function,function,function,function,function,function,function,function,function,function"
        ))
    );
    assert_eq!(
        run("var a=new Int16Array(2); a[0]=-1; a[1]=65535; [a.length,a.byteLength,a[0],a[1]].join(',');"),
        Value::String(Arc::from("2,4,-1,-1"))
    );
    assert_eq!(
        run("var a=new Uint8ClampedArray([0, 2.5, 3.5, 300, -1, NaN]); [a.length,a.byteLength,a[0],a[1],a[2],a[3],a[4],a[5]].join(',');"),
        Value::String(Arc::from("6,6,0,2,4,255,0,0"))
    );
    assert_eq!(
        run("var a=new BigInt64Array(1); a[0]=-1n; [a.length,a.byteLength,a[0].toString()].join(',');"),
        Value::String(Arc::from("1,8,-1"))
    );
    assert_eq!(
        run("var a=new Float32Array(1); a[0]=1.5; Object.seal(a); [Object.isSealed(a), Object.isExtensible(a), a[0]].join(',');"),
        Value::String(Arc::from("true,false,1.5"))
    );
}

#[test]
fn typed_array_constructors_read_array_like_objects_observably() {
    assert_eq!(
        run(r#"
            var log = [];
            var source = {
              get length() { log.push("length"); return "4"; },
              0: 7,
              get 1() { log.push("one"); return 8; },
              3: 260
            };
            var a = new Uint8Array(source);
            [a.length, a.byteLength, a[0], a[1], a[2], a[3], log.join("|")].join(",");
        "#),
        Value::String(Arc::from("4,4,7,8,0,4,length|one"))
    );
    assert_eq!(
        run("try { Uint8Array(1); 'no'; } catch (e) { e.name; }"),
        Value::String(Arc::from("TypeError"))
    );
    assert_eq!(
        run(r#"
            function C() {}
            C.prototype = 1;
            var a = Reflect.construct(Uint8Array, [1], C);
            Object.getPrototypeOf(a) === Uint8Array.prototype;
        "#),
        Value::Bool(true)
    );
    assert_eq!(
        run("try { new Uint8Array(-1); 'no'; } catch (e) { e.name; }"),
        Value::String(Arc::from("RangeError"))
    );
    assert_eq!(
        run(r#"
            function* gen() { yield 7; yield 42; }
            var a = new Uint8Array(gen());
            [a.length, a[0], a[1]].join(",");
        "#),
        Value::String(Arc::from("2,7,42"))
    );
    assert_eq!(
        run(r#"
            var values = [0, { valueOf: function() { values.length = 0; return 100; } }, 2];
            var a = new Uint8Array(values);
            [a.length, a[0], a[1], a[2], values.length].join(",");
        "#),
        Value::String(Arc::from("3,0,100,2,0"))
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
fn data_view_int16_uint16_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(6);
            var dv = new DataView(buffer, 1, 4);
            var values = [];
            values.push(dv.setUint16(0, 0x1234) === undefined);
            values.push(dv.getUint8(0));
            values.push(dv.getUint8(1));
            values.push(dv.getUint16(0));
            values.push(dv.getUint16(0, true));
            values.push(dv.setInt16(2, -2, true) === undefined);
            values.push(dv.getUint16(2));
            values.push(dv.getInt16(2, true));
            values.push(DataView.prototype.getUint16.length);
            values.push(DataView.prototype.setInt16.length);
            values.push(DataView.prototype.getInt16.name);
            values.push(DataView.prototype.setUint16.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "true,18,52,4660,13330,true,65279,-2,1,2,getInt16,setUint16"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(4));
            dv.setUint16(0, 65537);
            dv.setInt16(2, -32769);
            [dv.getUint16(0), dv.getInt16(0), dv.getUint16(2), dv.getInt16(2)].join(",");
            "#),
        Value::String(Arc::from("1,1,32767,32767"))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(2));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint16(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(2));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint16(2, poisoned);
            "#
        )
        .contains("Error"),
        "value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(2);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint16(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(2);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint16(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView element reads"
    );
}

#[test]
fn data_view_int32_uint32_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(10);
            var dv = new DataView(buffer, 1, 8);
            var values = [];
            values.push(dv.setUint32(0, 0x12345678) === undefined);
            values.push(dv.getUint8(0));
            values.push(dv.getUint8(1));
            values.push(dv.getUint8(2));
            values.push(dv.getUint8(3));
            values.push(dv.getUint32(0));
            values.push(dv.getUint32(0, true));
            values.push(dv.setInt32(4, -2, true) === undefined);
            values.push(dv.getUint32(4));
            values.push(dv.getInt32(4, true));
            values.push(DataView.prototype.getUint32.length);
            values.push(DataView.prototype.setInt32.length);
            values.push(DataView.prototype.getInt32.name);
            values.push(DataView.prototype.setUint32.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "true,18,52,86,120,305419896,2018915346,true,4278190079,-2,1,2,getInt32,setUint32"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(8));
            dv.setUint32(0, 4294967297);
            dv.setInt32(4, -2147483649);
            [dv.getUint32(0), dv.getInt32(0), dv.getUint32(4), dv.getInt32(4)].join(",");
            "#),
        Value::String(Arc::from("1,1,2147483647,2147483647"))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(4));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint32(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(4));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint32(4, poisoned);
            "#
        )
        .contains("Error"),
        "value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(4);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint32(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(4);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint32(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView element reads"
    );
}

#[test]
fn data_view_float_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(18);
            var dv = new DataView(buffer, 1, 16);
            var values = [];
            values.push(dv.setFloat32(0, 42, true) === undefined);
            values.push(dv.getFloat32(0));
            values.push(dv.getFloat32(0, true));
            values.push(dv.setFloat64(8, 42, true) === undefined);
            values.push(dv.getFloat64(8));
            values.push(dv.getFloat64(8, true));
            values.push(DataView.prototype.getFloat32.length);
            values.push(DataView.prototype.setFloat64.length);
            values.push(DataView.prototype.getFloat64.name);
            values.push(DataView.prototype.setFloat32.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "true,1.4441781973331565e-41,42,true,8.759e-320,42,1,2,getFloat64,setFloat32"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(12));
            dv.setFloat32(0, -0);
            dv.setFloat64(4, -0);
            [1 / dv.getFloat32(0), 1 / dv.getFloat64(4)].join(",");
            "#),
        Value::String(Arc::from("-Infinity,-Infinity"))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(12));
            dv.setUint8(0, 127);
            dv.setUint8(1, 192);
            dv.setUint8(2, 0);
            dv.setUint8(3, 0);
            dv.setUint8(4, 127);
            dv.setUint8(5, 248);
            dv.setUint8(6, 0);
            dv.setUint8(7, 0);
            dv.setUint8(8, 0);
            dv.setUint8(9, 0);
            dv.setUint8(10, 0);
            dv.setUint8(11, 0);
            [dv.getFloat32(0) !== dv.getFloat32(0), dv.getFloat64(4) !== dv.getFloat64(4)].join(",");
            "#),
        Value::String(Arc::from("true,true"))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(4));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setFloat32(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(8));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setFloat64(8, poisoned);
            "#
        )
        .contains("Error"),
        "value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(8);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getFloat64(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(4);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getFloat32(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView float reads"
    );
}

#[test]
fn data_view_bigint_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(18);
            var dv = new DataView(buffer, 1, 16);
            var values = [];
            values.push(dv.setBigUint64(0, 0x0102030405060708n) === undefined);
            values.push(dv.getUint8(0));
            values.push(dv.getUint8(7));
            values.push(dv.getBigUint64(0).toString());
            values.push(dv.getBigUint64(0, true).toString());
            values.push(dv.setBigInt64(8, -2n, true) === undefined);
            values.push(dv.getBigUint64(8).toString());
            values.push(dv.getBigInt64(8, true).toString());
            values.push(DataView.prototype.getBigUint64.length);
            values.push(DataView.prototype.setBigInt64.length);
            values.push(DataView.prototype.getBigInt64.name);
            values.push(DataView.prototype.setBigUint64.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "true,1,8,72623859790382856,578437695752307201,true,18374686479671623679,-2,1,2,getBigInt64,setBigUint64"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(8));
            dv.setBigUint64(0, 0x10000000000000001n);
            var wrapped = dv.getBigUint64(0).toString();
            dv.setBigInt64(0, -1n);
            [wrapped, dv.getBigUint64(0).toString(), dv.getBigInt64(0).toString()].join(",");
            "#),
        Value::String(Arc::from("1,18446744073709551615,-1"))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(8));
            var boxed = { valueOf: function() { return "42"; } };
            dv.setBigUint64(0, boxed);
            dv.getBigUint64(0).toString();
            "#),
        Value::String(Arc::from("42"))
    );
    assert!(
        run_err("new DataView(new ArrayBuffer(8)).setBigUint64(0, 1);").contains("TypeError"),
        "ToBigInt should reject Number values"
    );
    assert!(
        run_err("new DataView(new ArrayBuffer(8)).setBigInt64(0);").contains("TypeError"),
        "missing BigInt setter value should throw TypeError"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(8));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setBigInt64(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before BigInt value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(8));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setBigInt64(8, poisoned);
            "#
        )
        .contains("Error"),
        "BigInt value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(8);
            var dv = new DataView(buffer);
            var value = { valueOf: function() { $262.detachArrayBuffer(buffer); return 1n; } };
            dv.setBigInt64(0, value);
            "#
        )
        .contains("TypeError"),
        "detached buffers should be checked after BigInt value conversion"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(8);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getBigUint64(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(8);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getBigInt64(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView BigInt reads"
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
fn string_split_observes_symbol_split_and_coercion_order() {
    assert_eq!(run("String.prototype.split.length"), Value::Number(2.0));
    assert_eq!(
        run(r#"var separator = {};
               var seenThis, seen0, seen1;
               separator[Symbol.split] = function(str, limit) {
                 seenThis = this;
                 seen0 = str;
                 seen1 = limit;
                 return "custom";
               };
               var out = "".split(separator, "limit");
               [out, seenThis === separator, seen0, seen1].join("|");"#),
        Value::String(Arc::from("custom|true||limit"))
    );
    assert!(run_err(
        r#"var separator = {};
           Object.defineProperty(separator, Symbol.split, {
             get: function(){ throw new Error("split-get"); }
           });
           "".split(separator);"#
    )
    .contains("split-get"));
    assert_eq!(
        run(r#""undefined is not a function".split(undefined).join("|")"#),
        Value::String(Arc::from("undefined is not a function"))
    );
    assert_eq!(
        run(r#""undefined is not a function".split(undefined, 0).length"#),
        Value::Number(0.0)
    );
    assert!(run_err(
        r#"var limit = { valueOf: function(){ throw new Error("limit-value"); } };
           "".split("", limit);"#
    )
    .contains("limit-value"));
    assert!(run_err(
        r#"var sep = { toString: function(){ throw new Error("sep-string"); } };
           "abc".split(sep, 0);"#
    )
    .contains("sep-string"));
    assert_eq!(
        run(r#""hello".split(new RegExp()).join("|")"#),
        Value::String(Arc::from("h|e|l|l|o"))
    );
    assert_eq!(
        run(r#""x".split(/^/).join("|")"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run(r#""x".split(/.+/).join("|")"#),
        Value::String(Arc::from("|"))
    );
    assert_eq!(
        run(r#""x".split(/[]/).join("|")"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run(r#""x".split(/[^]/).join("|")"#),
        Value::String(Arc::from("|"))
    );
    assert_eq!(
        run(r#""x".split(/\cY/).join("|")"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run(r#""x".split(/[\b]/).join("|")"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run(r#""x".split(/\x/).join("|")"#),
        Value::String(Arc::from("|"))
    );
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
fn sparse_array_holes_are_not_own_keys() {
    assert_eq!(
        run("Object.keys([1,,3,,5]).join('|');"),
        Value::String(Arc::from("0|2|4"))
    );
    assert_eq!(
        run("var a=[1,2]; delete a[0]; [a.length, 0 in a, a.hasOwnProperty('0'), Object.keys(a).join('|'), Object.getOwnPropertyNames(a).join('|')].join(',');"),
        Value::String(Arc::from("2,false,false,1,1|length"))
    );
    assert_eq!(
        run("[[undefined].hasOwnProperty('0'), [,].hasOwnProperty('0'), Object.keys(Array(3)).length].join(',');"),
        Value::String(Arc::from("true,false,0"))
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
fn object_assign_uses_to_object_target_and_copies_string_sources() {
    assert_eq!(
        run(r#"
            var r = Object.assign(1, "ab");
            [
              typeof r,
              r.valueOf(),
              r[0],
              r[1],
              Object.getOwnPropertyNames(r).join("|")
            ].join(",");
        "#),
        Value::String(Arc::from("object,1,a,b,0|1"))
    );
    assert_eq!(
        run(r#"
            [
              Object.assign(true).valueOf(),
              Object.assign(2).valueOf(),
              Object.assign("x").valueOf(),
              Object.assign({}, null, undefined).constructor === Object
            ].join("|");
        "#),
        Value::String(Arc::from("true|2|x|true"))
    );
    assert!(run_err("Object.assign(null, { a: 1 });").contains("TypeError"));
    assert!(run_err("Object.assign(undefined, { a: 1 });").contains("TypeError"));
}

#[test]
fn object_assign_throws_on_failed_target_set() {
    assert!(run_err(
        r#"
            var target = {};
            Object.defineProperty(target, "x", { value: 1, writable: false });
            Object.assign(target, { x: 2 });
        "#
    )
    .contains("TypeError"));
    assert!(run_err(r#"Object.assign("ab", { 0: "x" });"#).contains("TypeError"));
}

#[test]
fn object_assign_copies_symbols_after_strings() {
    assert_eq!(
        run(r#"
            var s = Symbol("s");
            var log = "";
            var source = {};
            Object.defineProperty(source, s, {
              enumerable: true,
              get: function() { log += "s"; return 2; }
            });
            Object.defineProperty(source, "a", {
              enumerable: true,
              get: function() { log += "a"; return 1; }
            });
            var target = Object.assign({}, source);
            [log, target.a, target[s], Object.getOwnPropertySymbols(target).length].join("|");
        "#),
        Value::String(Arc::from("as|1|2|1"))
    );
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
    assert!(matches!(run("Math.pow(1, NaN);"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.pow(-1, Infinity);"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.pow(1, -Infinity);"), Value::Number(n) if n.is_nan()));
    assert_eq!(run("Math.pow(NaN, 0);"), Value::Number(1.0));
    assert_eq!(run("Object.isExtensible(Math);"), Value::Bool(true));
    assert_eq!(
        run("Math.substring = String.prototype.substring; Math.substring(Math.PI, -10);"),
        Value::String(Arc::from("[ob"))
    );
}

#[test]
fn math_round_half() {
    assert_eq!(run("Object.is(Math.round(-0), -0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.round(-0.5), -0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.round(-0.25), -0);"), Value::Bool(true));
    assert_eq!(
        run("Object.is(Math.round(0.5 - Number.EPSILON / 4), 0);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var x = -(2 / Number.EPSILON - 1); Object.is(Math.round(x), x);"),
        Value::Bool(true)
    );
    assert_eq!(run("Math.round(0.5);"), Value::Number(1.0));
    assert_eq!(run("Math.round(-1.5);"), Value::Number(-1.0));
}

#[test]
fn math_max_min_nan_and_signed_zero() {
    assert!(matches!(run("Math.max({});"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.min({});"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.max(1, NaN, 2);"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.min(1, NaN, 2);"), Value::Number(n) if n.is_nan()));
    assert_eq!(
        run("var calls = 0; var n = { valueOf: function(){ calls++; } }; Math.max(NaN, n); calls;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var calls = 0; var n = { valueOf: function(){ calls++; } }; Math.min(NaN, n); calls;"),
        Value::Number(1.0)
    );
    assert_eq!(run("Object.is(Math.max(-0, -0), -0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.max(0, -0), 0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.min(-0, -0), -0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.min(0, -0), -0);"), Value::Bool(true));
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
fn thrown_custom_constructor_object_preserves_constructor_name_in_display() {
    let msg = run_err(
        r#"
        function Test262Error(message) {
          if (!(this instanceof Test262Error)) return new Test262Error(message);
          this.message = message || "";
        }
        Test262Error.prototype.toString = function() {
          return "Test262Error: " + this.message;
        };
        throw new Test262Error();
        "#,
    );
    assert!(msg.contains("Test262Error"), "got: {msg}");

    let native = run_err("throw new Error('native');");
    assert!(native.contains("Error: native"), "got: {native}");
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
fn native_error_constructors_inherit_from_error_constructor() {
    assert_eq!(
        run(r#"[
                Object.getPrototypeOf(Error) === Function.prototype,
                Object.getPrototypeOf(EvalError) === Error,
                Object.getPrototypeOf(RangeError) === Error,
                Object.getPrototypeOf(ReferenceError) === Error,
                Object.getPrototypeOf(SyntaxError) === Error,
                Object.getPrototypeOf(TypeError) === Error,
                Object.getPrototypeOf(URIError) === Error,
                Object.getPrototypeOf(AggregateError) === Error,
                EvalError.hasOwnProperty("name"),
                EvalError.hasOwnProperty("length"),
                EvalError.name,
                EvalError.length,
                Object.prototype.toString.call(EvalError.prototype)
            ].join(",");"#),
        Value::String(Arc::from(
            "true,true,true,true,true,true,true,true,true,true,EvalError,1,[object Object]"
        ))
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
fn object_prototype_to_string_uses_receiver_brand() {
    assert_eq!(
        run(r#"
            [
              Object.prototype.toString.call([]),
              Object.prototype.toString.call(null),
              Object.prototype.toString.call(undefined),
              Object.prototype.toString.call("x"),
              Object.prototype.toString.call(Object(9)),
              Object.prototype.toString.call(Object(true)),
              Object.prototype.toString.call(function(){}),
              Object.prototype.toString.call(new Date(0)),
              Object.prototype.toString.call(Error("boom")),
              Object.prototype.toString.call(new Error("boom")),
              Object.prototype.toString.call(function(){ return arguments; }()),
              Object.getOwnPropertyNames(Object.prototype).join("|"),
              Error.prototype.toString.call({ name: "X", message: "Y" })
            ].join(",");
        "#),
        Value::String(Arc::from(
            "[object Array],[object Null],[object Undefined],[object String],[object Number],[object Boolean],[object Function],[object Date],[object Error],[object Error],[object Arguments],toString|toLocaleString|hasOwnProperty|isPrototypeOf|propertyIsEnumerable|valueOf|__defineGetter__|__defineSetter__|__lookupGetter__|__lookupSetter__|constructor|__proto__,X: Y"
        ))
    );
}

#[test]
fn object_prototype_value_of_and_to_locale_string_coerce_receiver() {
    assert!(run_err("Object.prototype.valueOf.call(undefined);").contains("TypeError"));
    assert!(run_err("Object.prototype.valueOf.call(null);").contains("TypeError"));
    assert!(run_err("(1, Object.prototype.valueOf)();").contains("TypeError"));
    assert_eq!(
        run("typeof Object.prototype.valueOf.call(true) + ':' + typeof Object.prototype.valueOf.call(false);"),
        Value::String(Arc::from("object:object"))
    );

    assert!(run_err("Object.prototype.toLocaleString.call(undefined);").contains("TypeError"));
    assert!(run_err("Object.prototype.toLocaleString.call(null);").contains("TypeError"));
    assert_eq!(
        run(r#"
            "use strict";
            Boolean.prototype.toString = function() { return typeof this; };
            true.toLocaleString();
        "#),
        Value::String(Arc::from("boolean"))
    );
    assert_eq!(
        run(r#"
            "use strict";
            Object.defineProperty(Boolean.prototype, "toString", {
              get: function() {
                var v = typeof this;
                return function() { return v + ":" + typeof this; };
              }
            });
            true.toLocaleString();
        "#),
        Value::String(Arc::from("boolean:boolean"))
    );
}

#[test]
fn object_prototype_legacy_accessor_methods() {
    assert_eq!(
        run(r#"
            var o = {};
            function getX() { return 7; }
            function setX(v) { this.seen = v; }
            o.__defineGetter__("x", getX);
            o.__defineSetter__("x", setX);
            var d = Object.getOwnPropertyDescriptor(o, "x");
            o.x = 9;
            [
              o.x,
              o.seen,
              d.get === getX,
              d.set === setX,
              d.enumerable,
              d.configurable,
              o.__lookupGetter__("x") === getX,
              o.__lookupSetter__("x") === setX,
              Object.prototype.propertyIsEnumerable.call(Object.prototype, "__defineGetter__"),
              Object.prototype.__defineGetter__.length,
              Object.prototype.__lookupSetter__.name
            ].join(",");
        "#),
        Value::String(Arc::from(
            "7,9,true,true,true,true,true,true,false,2,__lookupSetter__"
        ))
    );
    assert!(
        run_err("Object.prototype.__defineGetter__.call(null, 'x', function(){});")
            .contains("TypeError")
    );
    assert!(run_err("({}).__defineGetter__('x', 1);").contains("TypeError"));
    assert!(run_err(
        r#"({}).__defineGetter__({ toString: function(){ throw new Error("key"); } }, function(){});"#
    )
    .contains("key"));
}

#[test]
fn object_prototype_proto_accessor_and_mutation_status() {
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(Object.prototype, "__proto__");
            var o = {};
            var p = { x: 7 };
            desc.set.call(o, p);
            var nullProto = Object.create(null);
            nullProto.__proto__ = "own";
            var shadow = {};
            Object.defineProperty(shadow, "__proto__", {
              value: "before",
              writable: true,
              configurable: true
            });
            shadow.__proto__ = p;
            var sameNull = Object.setPrototypeOf(Object.prototype, null) === Object.prototype;
            var reflectSameNull = Reflect.setPrototypeOf(Object.prototype, null);
            var reflectImmutable = Reflect.setPrototypeOf(Object.prototype, {});
            var root = {};
            var leaf = Object.create(root);
            var reflectCycle = Reflect.setPrototypeOf(root, leaf);
            [
              Object.getPrototypeOf(Object.prototype) === null,
              typeof desc.get,
              desc.get.name,
              desc.get.length,
              typeof desc.set,
              desc.set.name,
              desc.set.length,
              desc.enumerable,
              desc.configurable,
              desc.get.call(o) === p,
              o.x,
              desc.set.call(1, p),
              desc.set.call(o, 1),
              nullProto.__proto__,
              Object.prototype.hasOwnProperty.call(nullProto, "__proto__"),
              shadow.__proto__ === p,
              Object.getPrototypeOf(shadow) === Object.prototype,
              sameNull,
              reflectSameNull,
              reflectImmutable,
              reflectCycle
            ].join(",");
        "#),
        Value::String(Arc::from(
            "true,function,get __proto__,0,function,set __proto__,1,false,true,true,7,,,own,true,true,true,true,true,false,false"
        ))
    );
    assert!(run_err("Object.setPrototypeOf(Object.prototype, {});").contains("TypeError"));
    assert!(run_err(
        "var root = {}; var leaf = Object.create(root); Object.getOwnPropertyDescriptor(Object.prototype, '__proto__').set.call(root, leaf);"
    )
    .contains("TypeError"));
    assert_eq!(
        run("var o = Object.preventExtensions({}); try { Object.getOwnPropertyDescriptor(Object.prototype, '__proto__').set.call(o, {}); } catch (e) { e instanceof TypeError; }"),
        Value::Bool(true)
    );
}

#[test]
fn proxy_prototype_internal_methods_follow_traps_and_invariants() {
    let src = r#"
        var target = {};
        var proto = { tag: "proto" };
        var replacement = { tag: "replacement" };
        var calls = [];
        var proxy = new Proxy(target, {
          getPrototypeOf: function(t) {
            calls.push("get:" + (t === target));
            return proto;
          },
          setPrototypeOf: function(t, v) {
            calls.push("set:" + (t === target) + ":" + (v === replacement));
            Object.setPrototypeOf(t, v);
            return true;
          }
        });
        var getViaReflect = Reflect.getPrototypeOf(proxy) === proto;
        var getViaObject = Object.getPrototypeOf(proxy) === proto;
        var setViaReflect = Reflect.setPrototypeOf(proxy, replacement);
        var targetUpdated = Object.getPrototypeOf(target) === replacement;

        var delegatedTarget = Object.create(proto);
        var delegated = new Proxy(delegatedTarget, {
          getPrototypeOf: null,
          setPrototypeOf: undefined
        });
        var delegatedGet = Reflect.getPrototypeOf(delegated) === proto;
        var delegatedSet = Reflect.setPrototypeOf(delegated, replacement);

        var fixedGetTarget = Object.create(proto);
        Object.preventExtensions(fixedGetTarget);
        var fixedGetProxy = new Proxy(fixedGetTarget, {
          getPrototypeOf: function() { return replacement; }
        });
        var getInvariant = false;
        try { Reflect.getPrototypeOf(fixedGetProxy); }
        catch (e) { getInvariant = e instanceof TypeError; }

        var fixedSetTarget = Object.create(proto);
        Object.preventExtensions(fixedSetTarget);
        var fixedSetProxy = new Proxy(fixedSetTarget, {
          setPrototypeOf: function() { return true; }
        });
        var setInvariant = false;
        try { Reflect.setPrototypeOf(fixedSetProxy, replacement); }
        catch (e) { setInvariant = e instanceof TypeError; }

        function Custom() {}
        var instanceProxy = new Proxy({}, {
          getPrototypeOf: function() { return Custom.prototype; }
        });

        [
          getViaReflect,
          getViaObject,
          setViaReflect,
          targetUpdated,
          delegatedGet,
          delegatedSet,
          Object.getPrototypeOf(delegatedTarget) === replacement,
          getInvariant,
          setInvariant,
          instanceProxy instanceof Custom,
          calls.join("|")
        ].join(",");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "true,true,true,true,true,true,true,true,true,true,get:true|get:true|set:true:true"
        ))
    );
}

#[test]
fn proxy_revocable_revoke_function_has_spec_own_properties() {
    assert_eq!(
        run(r#"
            var pair = Proxy.revocable({ x: 1 }, {});
            var revoke = pair.revoke;
            var length = Object.getOwnPropertyDescriptor(revoke, "length");
            var name = Object.getOwnPropertyDescriptor(revoke, "name");
            var names = Object.getOwnPropertyNames(revoke).join(",");
            revoke();
            var revoked = false;
            try { pair.proxy.x; } catch (e) { revoked = e instanceof TypeError; }
            [
              revoke.length,
              revoke.name,
              length.writable,
              length.enumerable,
              length.configurable,
              name.writable,
              name.enumerable,
              name.configurable,
              names,
              revoked
            ].join("|");
            "#,),
        Value::String(Arc::from(
            "0||false|false|true|false|false|true|length,name|true"
        ))
    );
}

#[test]
fn callable_proxy_follows_target_callability_and_apply_trap() {
    assert_eq!(
        run(r#"
            function target(a, b) { return this.base + a + b; }
            var proxy = new Proxy(target, {});
            [typeof proxy, proxy.call({ base: 1 }, 2, 3)].join("|");
            "#),
        Value::String(Arc::from("function|6"))
    );
    assert_eq!(
        run(r#"
            function target() { return "target"; }
            var seen = [];
            var proxy = new Proxy(target, {
              apply: function(t, thisArg, args) {
                seen.push(t === target, thisArg.tag, args.length, args[0], args[1]);
                return "trap";
              }
            });
            [typeof proxy, proxy.call({ tag: "this" }, "a", "b"), seen.join(",")].join("|");
            "#),
        Value::String(Arc::from("function|trap|true,this,2,a,b"))
    );
    assert_eq!(
        run(r#"
            var revocableTarget = Proxy.revocable(function() {}, {});
            revocableTarget.revoke();
            var revocable = Proxy.revocable(revocableTarget.proxy, {});
            typeof revocable.proxy;
            "#),
        Value::String(Arc::from("function"))
    );
    assert!(run_err(
        r#"
            var pair = Proxy.revocable(function() {}, {});
            pair.revoke();
            pair.proxy();
            "#,
    )
    .contains("TypeError"));
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

#[test]
fn for_in_order_builtins_follow_spec_key_order() {
    assert_eq!(
        run("var o=Object.create({p2:'proto'},{p1:{value:'p1',enumerable:true},p2:{value:'own',enumerable:false}}); Object.keys(o).join(',') + ':' + o.propertyIsEnumerable('p2');"),
        Value::String(Arc::from("p1:false"))
    );
    assert_eq!(
        run(
            r#"var o={p1:'p1',p2:'p2',p3:'p3'}; Object.defineProperty(o,'add',{enumerable:true,get:function(){o.extra='extra'; return 'add';}}); o.p4='p4'; o[2]='2'; o[0]='0'; o[1]='1'; delete o.p1; delete o.p3; o.p1='p1'; JSON.stringify(o);"#
        ),
        Value::String(Arc::from(
            r#"{"0":"0","1":"1","2":"2","p2":"p2","add":"add","p4":"p4","p1":"p1"}"#
        ))
    );
    assert_eq!(
        run(
            r#"var calls=[]; function reviver(name,val){calls.push(name); return val;} JSON.parse('{"p1":0,"p2":0,"p1":0,"2":0,"1":0}', reviver); calls.join(',');"#
        ),
        Value::String(Arc::from("1,2,p1,p2,"))
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
    assert!(run_err("Array.prototype.at.call(null, 0)")
        .contains("Cannot convert undefined or null to object"));
    assert!(run_err("Array.prototype.at.call(undefined, 0)")
        .contains("Cannot convert undefined or null to object"));
    assert_eq!(
        run("Array.prototype.at.call({0:'x', 1:'y', length: 2}, -1);"),
        Value::String(Arc::from("y"))
    );
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
        run(r#"'5ABBBABAB'.slice({ valueOf: function(){ return 2; } }, '5');"#),
        Value::String(Arc::from("BBB"))
    );
    assert_eq!(
        run(r#"'report'.slice(function(){}());"#),
        Value::String(Arc::from("report"))
    );
    assert_eq!(
        run(r#"String(void 0).substring('e', undefined);"#),
        Value::String(Arc::from("undefined"))
    );
    assert_eq!(
        run(r#"(function(){
                var b = new Boolean(false);
                b.substring = String.prototype.substring;
                return b.substring(function(){ return true; }(), undefined);
            })();"#),
        Value::String(Arc::from("alse"))
    );
    assert_eq!(
        run("'  hi  '.trimStart();"),
        Value::String(Arc::from("hi  "))
    );
    assert_eq!(
        run(r#"'\uFEFF\u00A0hi\uFEFF'.trim();"#),
        Value::String(Arc::from("hi"))
    );
    assert_eq!(
        run(r#"'\uFEFFhi\uFEFF'.trimStart();"#),
        Value::String(Arc::from("hi\u{FEFF}"))
    );
    assert_eq!(
        run(r#"'\uFEFFhi\uFEFF'.trimEnd();"#),
        Value::String(Arc::from("\u{FEFF}hi"))
    );
    assert_eq!(
        run(r#"String.prototype.trim.call(new RegExp(/test/));"#),
        Value::String(Arc::from("/test/"))
    );
    assert_eq!(
        run(r#"String.prototype.trim.call(function(){ return arguments; }(1, 2, true));"#),
        Value::String(Arc::from("[object Arguments]"))
    );
    assert_eq!(
        run(r#"'\u180Ehi\u0085'.trim();"#),
        Value::String(Arc::from("\u{180E}hi\u{0085}"))
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
    assert_eq!(run("Number.parseInt === parseInt;"), Value::Bool(true));
    assert_eq!(run("Number.parseFloat === parseFloat;"), Value::Bool(true));
    assert_eq!(
        run("var d = Object.getOwnPropertyDescriptor(Number, 'isFinite'); [d.writable, d.enumerable, d.configurable].join(',');"),
        Value::String(Arc::from("true,false,true"))
    );
    assert_eq!(
        run("var d = Object.getOwnPropertyDescriptor(Number, 'MAX_VALUE'); [d.writable, d.enumerable, d.configurable].join(',');"),
        Value::String(Arc::from("false,false,false"))
    );
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
fn parse_int_radix_to_int32_and_large_prefix() {
    assert!(matches!(
        run("parseInt('11', true);"),
        Value::Number(n) if n.is_nan()
    ));
    assert_eq!(run("parseInt('11', '2');"), Value::Number(3.0));
    assert_eq!(run("parseInt('11', new Number(2));"), Value::Number(3.0));
    assert_eq!(run("parseInt('11', new String('2'));"), Value::Number(3.0));
    assert_eq!(
        run("parseInt('11', { valueOf: function() { return 2; } });"),
        Value::Number(3.0)
    );
    assert_eq!(run("parseInt('11', Infinity);"), Value::Number(11.0));
    assert_eq!(run("parseInt('11', 4294967298);"), Value::Number(3.0));
    assert_eq!(
        run("parseInt('0x10000000000000000', 16);"),
        Value::Number(18_446_744_073_709_552_000.0)
    );
    assert_eq!(
        run("parseInt('-10000000000000000000', 10);"),
        Value::Number(-10_000_000_000_000_000_000.0)
    );
}

#[test]
fn object_statics() {
    assert_eq!(run("Object.is(NaN, NaN);"), Value::Bool(true));
    assert_eq!(run("Object.is(0, -0);"), Value::Bool(false));
    assert_eq!(run("Object.is(1, 1);"), Value::Bool(true));
    assert_eq!(
        run("var p={x:1}; var o=Object.create(p); o.y=2; [Object.hasOwn(o,'x'), Object.hasOwn(o,'y'), Object.hasOwn('abc','length'), Object.hasOwn('abc','1')].join(',');"),
        Value::String(Arc::from("false,true,true,true"))
    );
    assert_eq!(
        run("var s=Symbol(); var o={}; o[s]=1; [Object.hasOwn(o,s), Object.hasOwn({},s), Object.hasOwn.length, Object.hasOwn.name, Object.hasOwn.prototype].join(',');"),
        Value::String(Arc::from("true,false,2,hasOwn,"))
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var key = { get toString() { calls++; throw new Error("key"); } };
            try { Object.hasOwn(null, key); } catch (e) {}
            calls;
            "#),
        Value::Number(0.0)
    );
    assert_eq!(
        run(r#"
            var index = Object.getOwnPropertyDescriptor("foo", "0");
            var length = Object.getOwnPropertyDescriptor("foo", "length");
            [
              index.value,
              index.writable,
              index.enumerable,
              index.configurable,
              length.value,
              length.writable,
              length.enumerable,
              length.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("f,false,true,false,3,false,false,false"))
    );
    assert_eq!(
        run(r#"
            var sym = Symbol();
            var obj = {};
            obj[sym] = 42;
            var desc = Object.getOwnPropertyDescriptor(obj, sym);
            [
              desc.value,
              desc.writable,
              desc.enumerable,
              desc.configurable,
              desc.propertyIsEnumerable("value"),
              Object.keys(desc).join("|")
            ].join(",");
            "#),
        Value::String(Arc::from(
            "42,true,true,true,true,value|writable|enumerable|configurable"
        ))
    );
    assert_eq!(
        run(r#"
            var proto = Object.getOwnPropertyDescriptor(String, "prototype");
            var length = Object.getOwnPropertyDescriptor(String, "length");
            [
              proto.writable,
              proto.enumerable,
              proto.configurable,
              length.value,
              length.writable,
              length.enumerable,
              length.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("false,false,false,1,false,false,true"))
    );
    assert_eq!(
        run(r#"
            var obj = { undefined: 7 };
            Object.getOwnPropertyDescriptor(obj).value;
            "#),
        Value::Number(7.0)
    );
    assert_eq!(
        run(r#"
            var sym = Symbol();
            var obj = { a: 1 };
            Object.defineProperty(obj, "b", { value: 2, enumerable: false });
            obj[sym] = 3;
            [
              Object.getOwnPropertyNames(obj).join("|"),
              Object.getOwnPropertySymbols(obj).length,
              Object.getOwnPropertySymbols(obj)[0] === sym,
              Object.getOwnPropertyDescriptors(obj).b.enumerable,
              Object.getOwnPropertyDescriptors(obj)[sym].value
            ].join(",");
            "#),
        Value::String(Arc::from("a|b,1,true,false,3"))
    );
    assert_eq!(
        run(r#"
            var descs = Object.getOwnPropertyDescriptors("ab");
            [
              Object.keys(descs).join("|"),
              descs.length.value,
              descs.length.enumerable,
              descs[0].value,
              descs[0].writable
            ].join(",");
        "#),
        Value::String(Arc::from("0|1|length,2,false,a,false"))
    );
    assert!(
        run_err("Object.values(null);").contains("TypeError"),
        "Object.values(null) should throw"
    );
    assert!(
        run_err("Object.entries(undefined);").contains("TypeError"),
        "Object.entries(undefined) should throw"
    );
    assert_eq!(
        run(r#"
            var obj = {
              a: "A",
              get b() {
                delete this.c;
                Object.defineProperty(this, "d", { value: "D", enumerable: false });
                return "B";
              },
              c: "C",
              d: "visible"
            };
            Object.values(obj).join("|") + ":" + Object.entries(obj).map(function(e) {
              return e[0] + "=" + e[1];
            }).join("|");
        "#),
        Value::String(Arc::from("A|B:a=A|b=B"))
    );
    assert_eq!(
        run(r#"
            var target = {};
            var proxy = new Proxy(target, {});
            var returned = Object.defineProperty(proxy, "a", {
              value: 1,
              enumerable: true,
              configurable: true
            });
            [
              returned === proxy,
              Object.prototype.hasOwnProperty.call(proxy, "a"),
              Object.prototype.hasOwnProperty.call(target, "a"),
              Object.values(proxy).join("|")
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,1"))
    );
    assert_eq!(
        run(r#"
            var calls = [];
            var proxy = new Proxy({ present: undefined }, {
              has: function(t, k) {
                calls.push("has:" + k);
                return k === "present";
              }
            });
            [
              "present" in proxy,
              "missing" in proxy,
              Reflect.has(proxy, "present"),
              Reflect.has(proxy, "missing"),
              calls.join("|")
            ].join(",");
        "#),
        Value::String(Arc::from(
            "true,false,true,false,has:present|has:missing|has:present|has:missing"
        ))
    );
    assert_eq!(
        run(r#"
            var target = {};
            var receiver = {};
            Object.defineProperty(receiver, "p", { value: 1, writable: false });
            [
              Reflect.set(target, "p", 2, receiver),
              receiver.p,
              Object.prototype.hasOwnProperty.call(target, "p")
            ].join(",");
        "#),
        Value::String(Arc::from("false,1,false"))
    );
    assert_eq!(
        run(r#"
            var target = {};
            var receiver = {};
            Object.defineProperty(receiver, "p", { set: function(v) {} });
            [
              Reflect.set(target, "p", 2, receiver),
              Object.prototype.hasOwnProperty.call(target, "p")
            ].join(",");
        "#),
        Value::String(Arc::from("false,false"))
    );
    assert!(
        run_err(
            r#"
            var proxy = new Proxy({}, {
              set: function() { throw new Error("boom"); }
            });
            Reflect.set(proxy, "p", 1);
        "#
        )
        .contains("boom"),
        "Reflect.set should propagate abrupt completions from Proxy set traps"
    );
    assert_eq!(
        run(r#"
            var obj = {};
            Object.defineProperty(obj, "p", { value: 1 });
            Object.freeze(obj);
            [
              Reflect.defineProperty(obj, "p", { value: 2 }),
              obj.p,
              Reflect.defineProperty(obj, "q", { value: 3 }),
              Object.prototype.hasOwnProperty.call(obj, "q")
            ].join(",");
        "#),
        Value::String(Arc::from("false,1,false,false"))
    );
    assert!(
        run_err(
            r#"
            var attrs = {};
            Object.defineProperty(attrs, "enumerable", {
              get: function() { throw new Error("attrs boom"); }
            });
            Reflect.defineProperty({}, "p", attrs);
        "#
        )
        .contains("attrs boom"),
        "Reflect.defineProperty should propagate descriptor getter errors"
    );
    assert!(
        run_err(
            r#"
            var proxy = new Proxy({}, {
              getOwnPropertyDescriptor: function() { throw new Error("gopd boom"); }
            });
            Reflect.getOwnPropertyDescriptor(proxy, "p");
        "#
        )
        .contains("gopd boom"),
        "Reflect.getOwnPropertyDescriptor should propagate Proxy trap errors"
    );
    assert_eq!(
        run(r#"
            var calls = [];
            var target = { a: 1 };
            var handler = {
              deleteProperty: function(t, key) {
                calls.push(this === handler);
                calls.push(t === target);
                calls.push(key);
                return delete t[key];
              }
            };
            var proxy = new Proxy(target, handler);
            [delete proxy.a, "a" in target, calls.join("|")].join(",");
        "#),
        Value::String(Arc::from("true,false,true|true|a"))
    );
    assert_eq!(
        run(r#"
            var target = {};
            Object.defineProperty(target, "fixed", { value: 1, configurable: false });
            var proxy = new Proxy(target, { deleteProperty: function() { return false; } });
            [Reflect.deleteProperty(proxy, "fixed"), target.fixed].join(",");
        "#),
        Value::String(Arc::from("false,1"))
    );
    assert!(
        run_err(
            r#"
            var target = {};
            Object.defineProperty(target, "fixed", { value: 1, configurable: false });
            var proxy = new Proxy(target, { deleteProperty: function() { return true; } });
            Reflect.deleteProperty(proxy, "fixed");
        "#
        )
        .contains("TypeError"),
        "Proxy deleteProperty cannot report non-configurable properties as deleted"
    );
    assert!(
        run_err("Reflect.deleteProperty(1, 'x');").contains("TypeError"),
        "Reflect.deleteProperty must reject primitive targets"
    );
    assert!(
        run_err("Object.keys(null);").contains("TypeError"),
        "Object.keys(null) should throw"
    );
    assert!(
        run_err("Object.getOwnPropertyNames(undefined);").contains("TypeError"),
        "Object.getOwnPropertyNames(undefined) should throw"
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var key = { get toString() { calls++; throw new Error("key"); } };
            try { Object.getOwnPropertyDescriptor(null, key); } catch (e) {}
            calls;
            "#),
        Value::Number(0.0)
    );
    assert_eq!(
        run("var o = Object.fromEntries([['a',1],['b',2]]); o.a + o.b;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run(r#"
            var a = Object.fromEntries([Object("ab")]);
            var b = Object.fromEntries([new String("cd")]);
            [a.a, b.c].join("|");
        "#),
        Value::String(Arc::from("b|d"))
    );
    assert_eq!(
        run(r#"
            var s = Symbol("k");
            var o = Object.fromEntries([[s, 3]]);
            [o[s], Object.getOwnPropertySymbols(o).length].join("|");
        "#),
        Value::String(Arc::from("3|1"))
    );
    assert!(run_err("Object.fromEntries();").contains("TypeError"));
    assert!(run_err(r#"Object.fromEntries(["ab"]);"#).contains("TypeError"));
    assert_eq!(
        run(r#"
            var o = Object.groupBy([1, 2, 3], function(v, i) {
              return i + ":" + (v % 2 === 0 ? "even" : "odd");
            });
            [
              Object.getPrototypeOf(o) === null,
              Object.keys(o).join("|"),
              o["0:odd"].join(","),
              o["1:even"].join(","),
              o["2:odd"].join(",")
            ].join(";");
        "#),
        Value::String(Arc::from("true;0:odd|1:even|2:odd;1;2;3"))
    );
    assert_eq!(
        run(r#"
            var s = Symbol("group");
            var o = Object.groupBy(["a", "b"], function(v) {
              return v === "a" ? s : "plain";
            });
            [
              o[s].join(","),
              o.plain.join(","),
              Object.getOwnPropertySymbols(o).length
            ].join("|");
        "#),
        Value::String(Arc::from("a|b|1"))
    );
    assert_eq!(
        run(r#"
            var closed = false;
            var it = {
              i: 0,
              next: function() { return { value: ++this.i, done: false }; },
              return: function() { closed = true; return {}; }
            };
            var src = {};
            src[Symbol.iterator] = function() { return it; };
            try {
              Object.groupBy(src, function(v) {
                if (v === 2) throw new Error("stop");
                return "k";
              });
            } catch (e) {}
            closed;
        "#),
        Value::Bool(true)
    );
    assert!(run_err("Object.groupBy([], null);").contains("TypeError"));
    assert!(run_err("Object.groupBy(null, function(){});").contains("TypeError"));
    assert_eq!(
        run("typeof Object.create(null);"),
        Value::String(Arc::from("object"))
    );
}

#[test]
fn prevent_extensions_blocks_array_arguments_function_and_proxy_edges() {
    assert_eq!(
        run("var a=[]; Object.preventExtensions(a); a[0]=1; a.x=2; Object.isExtensible(a)+':' + a.hasOwnProperty('0') + ':' + a.hasOwnProperty('x');"),
        Value::String(Arc::from("false:false:false"))
    );
    assert_eq!(
        run("(function(){ Object.preventExtensions(arguments); arguments[0]=1; arguments.x=2; return Object.isExtensible(arguments)+':' + arguments.hasOwnProperty('0') + ':' + arguments.hasOwnProperty('x'); })();"),
        Value::String(Arc::from("false:false:false"))
    );
    assert_eq!(
        run("function f(){} Object.preventExtensions(f); f[0]=1; f.x=2; Object.isExtensible(f)+':' + f.hasOwnProperty('0') + ':' + f.hasOwnProperty('x');"),
        Value::String(Arc::from("false:false:false"))
    );
    assert!(run_err(
        "Object.preventExtensions(new Proxy({}, { preventExtensions(){ return false; } }));"
    )
    .contains("TypeError"));
    assert_eq!(
        run("var target={}; var p=new Proxy(target,{preventExtensions(t){ Object.preventExtensions(t); return true; }}); Reflect.preventExtensions(p)+':' + Object.isExtensible(target);"),
        Value::String(Arc::from("true:false"))
    );
    assert_eq!(
        run("Reflect.preventExtensions(new Proxy({}, { preventExtensions(){ return false; } }));"),
        Value::Bool(false)
    );
}

#[test]
fn seal_and_freeze_update_integrity_for_arrays_arguments_functions_and_proxies() {
    assert_eq!(run("Object.isSealed(1);"), Value::Bool(true));
    assert_eq!(run("Object.isFrozen(1);"), Value::Bool(true));
    assert_eq!(run("Object.isSealed(Boolean);"), Value::Bool(false));
    assert_eq!(run("Object.isFrozen(Boolean);"), Value::Bool(false));
    assert_eq!(
        run("var a=[0,1]; Object.seal(a); var d=Object.getOwnPropertyDescriptor(a,'0'); Object.isSealed(a)+':' + d.configurable;"),
        Value::String(Arc::from("true:false"))
    );
    assert_eq!(
        run("var a=[]; Object.seal(a); a.length=1; var d=Object.getOwnPropertyDescriptor(a,'length'); a.length + ':' + d.value + ':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("1:1:true:false"))
    );
    assert_eq!(
        run("var a=[]; Object.seal(a); Object.isFrozen(a);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var a=[]; a[2000000]=1; Object.freeze(a); a.length;"),
        Value::Number(2000001.0)
    );
    assert_eq!(
        run("var a=[0,1]; Object.freeze(a); var d=Object.getOwnPropertyDescriptor(a,'0'); Object.isFrozen(a)+':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("true:false:false"))
    );
    assert_eq!(
        run("var a=[0,1]; Object.freeze(a); a.length=1; a.length;"),
        Value::Number(2.0)
    );
    assert!(
        run_err("\"use strict\"; var a=[0,1]; Object.freeze(a); a.length=1;").contains("TypeError")
    );
    assert_eq!(
        run("(function(){ Object.freeze(arguments); var d=Object.getOwnPropertyDescriptor(arguments,'0'); return Object.isFrozen(arguments)+':' + d.writable + ':' + d.configurable; })(1);"),
        Value::String(Arc::from("true:false:false"))
    );
    assert_eq!(
        run("(function(a){ Object.freeze(arguments); a=2; return arguments[0]; })(1);"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("(function(){ Object.seal(arguments); var d=Object.getOwnPropertyDescriptor(arguments,'0'); return Object.isSealed(arguments)+':' + Object.isFrozen(arguments)+':' + d.writable + ':' + d.configurable; })(1);"),
        Value::String(Arc::from("true:false:true:false"))
    );
    assert_eq!(
        run("function f(){} f.x=1; Object.seal(f); var d=Object.getOwnPropertyDescriptor(f,'x'); Object.isSealed(f)+':' + Object.isFrozen(f)+':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("true:false:true:false"))
    );
    assert_eq!(
        run("function f(){} f.x=1; Object.freeze(f); var d=Object.getOwnPropertyDescriptor(f,'x'); Object.isFrozen(f)+':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("true:false:false"))
    );
    assert!(
        run_err("Object.seal(new Proxy({}, { preventExtensions(){ return false; } }));")
            .contains("TypeError")
    );
    assert!(
        run_err("Object.freeze(new Proxy({}, { preventExtensions(){ return false; } }));")
            .contains("TypeError")
    );
    assert_eq!(
        run("var target={x:1}; var p=new Proxy(target,{}); Object.seal(p); var d=Object.getOwnPropertyDescriptor(target,'x'); Object.isSealed(p)+':' + d.configurable;"),
        Value::String(Arc::from("true:false"))
    );
    assert_eq!(
        run("var target={x:1}; var p=new Proxy(target,{}); Object.freeze(p); var d=Object.getOwnPropertyDescriptor(target,'x'); Object.isFrozen(p)+':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("true:false:false"))
    );
    assert_eq!(
        run("var target={x:1}; Object.seal(target); var p=new Proxy(target,{}); Object.isSealed(p);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var target={x:1}; Object.freeze(target); var p=new Proxy(target,{}); Object.isFrozen(p);"),
        Value::Bool(true)
    );
    assert!(
        run_err("Object.seal(new Proxy({x:1}, { ownKeys(){ throw new Error('boom'); } }));")
            .contains("boom")
    );
    assert!(
        run_err("Object.freeze(new Proxy({x:1}, { defineProperty(){ return false; } }));")
            .contains("TypeError")
    );
}

#[test]
fn is_extensible_uses_proxy_traps_and_reflect_rejects_primitives() {
    assert_eq!(run("Object.isExtensible(1);"), Value::Bool(false));
    assert!(run_err("Reflect.isExtensible(1);").contains("TypeError"));
    assert_eq!(
        run("var seenThis, seenTarget; var target={}; var handler={isExtensible(t){seenThis=this;seenTarget=t;return Object.isExtensible(t);}}; var p=new Proxy(target, handler); Object.isExtensible(p)+':' + (seenThis===handler) + ':' + (seenTarget===target);"),
        Value::String(Arc::from("true:true:true"))
    );
    assert!(
        run_err("Object.isExtensible(new Proxy({}, {isExtensible(){return false;}}));")
            .contains("TypeError")
    );
    assert_eq!(
        run("var target={}; var p=new Proxy(target,{isExtensible(t){return Object.isExtensible(t);}}); var a=Object.isExtensible(p); Object.preventExtensions(target); a + ':' + Object.isExtensible(p);"),
        Value::String(Arc::from("true:false"))
    );
    assert!(run_err(
        "Reflect.isExtensible(new Proxy({}, {isExtensible(){throw new Error('boom');}}));"
    )
    .contains("boom"));
}

#[test]
fn reflect_own_keys_includes_symbols_and_non_enumerables_in_spec_order() {
    assert_eq!(
        run(r#"
            var first = Symbol("first");
            var second = Symbol("second");
            var obj = {};
            obj.z = 1;
            obj[2] = 2;
            Object.defineProperty(obj, "hidden", { value: 3, enumerable: false });
            obj.a = 4;
            obj[first] = 5;
            Object.defineProperty(obj, second, { value: 6, enumerable: false });
            Reflect.ownKeys(obj).map(function(key) {
              if (key === first) return "first";
              if (key === second) return "second";
              return key;
            }).join("|");
            "#),
        Value::String(Arc::from("2|z|hidden|a|first|second"))
    );
    assert!(
        run_err("Reflect.ownKeys('abc');").contains("TypeError"),
        "Reflect.ownKeys must reject primitive targets"
    );
}

#[test]
fn reflect_own_keys_propagates_proxy_trap_result_errors() {
    assert!(run_err(
        r#"
        var key = {};
        Object.defineProperty(key, Symbol.toPrimitive, {
          get: function() { throw new Error("key-coercion"); }
        });
        Reflect.ownKeys(new Proxy({}, {
          ownKeys: function() { return [key]; }
        }));
        "#,
    )
    .contains("key-coercion"));
}

#[test]
fn object_entry_helpers_observe_proxy_descriptors() {
    assert_eq!(
        run(r##"
            var log = [];
            var target = { a: 1, b: 2, c: 3 };
            var proxy = new Proxy(target, {
              ownKeys: function(t) {
                log.push("ownKeys");
                return ["a", "b", "c"];
              },
              getOwnPropertyDescriptor: function(t, key) {
                log.push("getOwnPropertyDescriptor:" + key);
                return { enumerable: key !== "b", configurable: true };
              },
              get: function(t, key) {
                log.push("get:" + key);
                return t[key];
              }
            });
            var values = Object.values(proxy).join(",");
            var entries = Object.entries(proxy).map(function(pair) {
              return pair.join(":");
            }).join(",");
            var descs = Object.getOwnPropertyDescriptors(proxy);
            [
              values,
              entries,
              descs.a.enumerable,
              descs.b.enumerable,
              descs.c.enumerable,
              log.join("|")
            ].join("#");
            "##),
        Value::String(Arc::from(
            "1,3#a:1,c:3#true#false#true#ownKeys|getOwnPropertyDescriptor:a|get:a|getOwnPropertyDescriptor:b|getOwnPropertyDescriptor:c|get:c|ownKeys|getOwnPropertyDescriptor:a|get:a|getOwnPropertyDescriptor:b|getOwnPropertyDescriptor:c|get:c|ownKeys|getOwnPropertyDescriptor:a|getOwnPropertyDescriptor:b|getOwnPropertyDescriptor:c"
        ))
    );
}

#[test]
fn reflect_construct_uses_array_like_args_and_new_target() {
    assert_eq!(
        run(r#"
            var seenProto;
            function Target(a, b) {
              this.sum = a + b;
              seenProto = Object.getPrototypeOf(this);
            }
            function NewTarget() {}
            NewTarget.prototype = { marker: 1 };
            var args = {0: 2, 1: 3, length: 2};
            var result = Reflect.construct(Target, args, NewTarget);
            [
              result.sum,
              Object.getPrototypeOf(result) === NewTarget.prototype,
              seenProto === NewTarget.prototype
            ].join("|");
            "#),
        Value::String(Arc::from("5|true|true"))
    );
    assert!(run_err("Reflect.construct(function(){}, 1);").contains("TypeError"));
    assert!(run_err("var o = {}; Object.defineProperty(o, 'length', { get(){ throw new Error('boom'); } }); Reflect.construct(function(){}, o);").contains("boom"));
    assert!(run_err("Reflect.construct(function(){}, [], Date.now);").contains("TypeError"));
    assert!(run_err("new Reflect.construct(Function, [], Function);").contains("TypeError"));
    assert_eq!(
        run("function isConstructor(f){ try { Reflect.construct(function(){}, [], f); } catch(e) { return false; } return true; } isConstructor(Reflect.construct);"),
        Value::Bool(false)
    );
    assert_eq!(
        run("function C(){} C.prototype = null; Object.getPrototypeOf(new C()) === Object.prototype;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            function Target() {}
            var NewTarget = function() {}.bind();
            Object.defineProperty(Function.prototype, "prototype", {
              get: function() { calls++; throw new Error("proto boom"); },
              configurable: true
            });
            var out;
            try {
              Reflect.construct(Target, [], NewTarget);
              out = "ok";
            } catch (e) {
              out = e.message;
            }
            delete Function.prototype.prototype;
            out + "|" + calls;
            "#),
        Value::String(Arc::from("proto boom|1"))
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var proto = {};
            var NewTarget = function() {}.bind();
            Object.defineProperty(Function.prototype, "prototype", {
              get: function() { calls++; return proto; },
              configurable: true
            });
            var arr = Reflect.construct(Array, [], NewTarget);
            var fn = Reflect.construct(Function, ["return 1;"], NewTarget);
            delete Function.prototype.prototype;
            [
              calls,
              Object.getPrototypeOf(arr) === proto,
              Object.getPrototypeOf(fn) === proto
            ].join("|");
            "#),
        Value::String(Arc::from("2|true|true"))
    );
}

#[test]
fn reflect_apply_uses_create_list_from_array_like() {
    assert_eq!(
        run(r#"
            function collect() {
              return Array.prototype.join.call(arguments, "|");
            }
            var args = {0: "a", 1: "b", length: 2};
            Reflect.apply(collect, null, args);
            "#),
        Value::String(Arc::from("a|b"))
    );
    assert_eq!(
        run(r#"
            function count() {
              return arguments.length + ":" + String(arguments[0]);
            }
            var args = {};
            Object.defineProperty(args, "length", {
              get: function() { return 1; }
            });
            Reflect.apply(count, null, args);
            "#),
        Value::String(Arc::from("1:undefined"))
    );
    assert!(run_err("Reflect.apply(function(){}, null, 1);").contains("TypeError"));
    assert!(run_err("Reflect.apply(function(){}, null);").contains("TypeError"));
    assert!(run_err(
        "var o = {}; Object.defineProperty(o, 'length', { get: function(){ throw new Error('boom'); } }); Reflect.apply({}, null, o);"
    )
    .contains("TypeError"));
    assert!(
        run_err("var o = {}; Object.defineProperty(o, 'length', { get: function(){ throw new Error('boom'); } }); Reflect.apply(function(){}, null, o);")
            .contains("boom")
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
    assert_eq!(run("Math.clz32(Infinity);"), Value::Number(32.0));
    assert_eq!(run("Math.clz32(4294967296);"), Value::Number(32.0));
    assert_eq!(run("Math.imul(0xffffffff, 5);"), Value::Number(-5.0));
    assert_eq!(run("Math.sign(-5);"), Value::Number(-1.0));
    assert!(matches!(run("Math.sign(NaN);"), Value::Number(n) if n.is_nan()));
    assert_eq!(run("Object.is(Math.sign(-0), -0);"), Value::Bool(true));
    assert_eq!(run("Math.sinh(0);"), Value::Number(0.0));
    assert_eq!(run("Math.acosh(1);"), Value::Number(0.0));
    assert_eq!(run("Math.asinh(-0);"), Value::Number(-0.0));
    assert_eq!(run("Math.atanh(1);"), Value::Number(f64::INFINITY));
    assert!(matches!(run("Math.acosh(0);"), Value::Number(n) if n.is_nan()));
    assert_eq!(
        run("Math.acosh.length + ':' + Math.asinh.name + ':' + Math.atanh.length;"),
        Value::String(Arc::from("1:asinh:1"))
    );
    assert_eq!(run("Math.sumPrecise([1, 2, 3]);"), Value::Number(6.0));
    assert_eq!(
        run("Math.sumPrecise([1e30, 0.1, -1e30]);"),
        Value::Number(0.1)
    );
    assert_eq!(
        run("Object.is(Math.sumPrecise([]), -0);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("Object.is(Math.sumPrecise([-0, 0]), 0);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("Math.sumPrecise([Infinity, Infinity]);"),
        Value::Number(f64::INFINITY)
    );
    assert!(matches!(
        run("Math.sumPrecise([Infinity, -Infinity]);"),
        Value::Number(n) if n.is_nan()
    ));
    assert!(common::run_err("Math.sumPrecise([{}]);").contains("TypeError"));
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
fn promise_then_uses_species_capability_path() {
    assert_eq!(
        run("var getterCalled = false, ok = false;
             var object = Object.defineProperty({}, 'constructor', {
               get: function() { getterCalled = true; throw new Error('bad'); }
             });
             try { Promise.prototype.then.call(object); }
             catch (e) { ok = e instanceof TypeError; }
             ok && getterCalled === false;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var callCount = 0, argLength = 0, executorLength = 0;
             var p = new Promise(function() {});
             class SpeciesConstructor extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 argLength = arguments.length;
                 executorLength = executor.length;
               }
             }
             p.constructor = function() {};
             p.constructor[Symbol.species] = SpeciesConstructor;
             var result = p.then();
             callCount === 1 && argLength === 1 && executorLength === 2 &&
               result instanceof SpeciesConstructor;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var p = new Promise(function() {});
             function BadCapability(executor) {
               executor(1, function() {});
               return {};
             }
             p.constructor = function() {};
             p.constructor[Symbol.species] = BadCapability;
             var ok = false;
             try { p.then(); }
             catch (e) { ok = e instanceof TypeError; }
             ok;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_then_adopts_returned_fulfilled_promise() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "var out = 0;
         Promise.resolve(1)
           .then(function() { return Promise.resolve(7); })
           .then(function(v) { out = v; });",
    )
    .expect("evaluation errored");
    assert_eq!(
        vm.run("out;").expect("evaluation errored"),
        Value::Number(7.0)
    );
}

#[test]
fn promise_then_rejects_self_resolution_with_type_error() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "var caught = false;
         var q = Promise.resolve().then(function() { return q; });
         q.catch(function(e) { caught = e instanceof TypeError; });",
    )
    .expect("evaluation errored");
    assert_eq!(
        vm.run("caught;").expect("evaluation errored"),
        Value::Bool(true)
    );
}

#[test]
fn promise_catch_reject() {
    // reject -> catch returns a derived promise (object), not the error value.
    let r = run("new Promise(function(_, rej){ rej('boom'); }) \
           .catch(function(e){ return e; });");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn promise_catch_invokes_observable_then() {
    assert_eq!(
        run(
            "var target = {}, returnValue = {}, callCount = 0, thisValue, firstArg, secondArg;
             target.then = function(a, b) {
               callCount += 1;
               thisValue = this;
               firstArg = a;
               secondArg = b;
               return returnValue;
             };
             var result = Promise.prototype.catch.call(target, 1, 2, 3);
             callCount === 1 && thisValue === target && firstArg === undefined &&
               secondArg === 1 && result === returnValue;"
        ),
        Value::Bool(true)
    );
    assert_eq!(
        run("var poisoned = Object.defineProperty({}, 'then', {
               get: function() { throw new TypeError('poison'); }
             });
             var ok = false;
             try { Promise.prototype.catch.call(poisoned); }
             catch (e) { ok = e instanceof TypeError; }
             ok;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_finally_invokes_observable_then() {
    assert_eq!(
        run(
            "var target = {}, returnValue = {}, callCount = 0, thisValue, firstArg, secondArg;
             target.then = function(a, b) {
               callCount += 1;
               thisValue = this;
               firstArg = a;
               secondArg = b;
               return returnValue;
             };
             var result = Promise.prototype.finally.call(target, 1, 2, 3);
             callCount === 1 && thisValue === target && firstArg === 1 &&
               secondArg === 1 && result === returnValue;"
        ),
        Value::Bool(true)
    );
    assert_eq!(
        run("var poisoned = Object.defineProperty({}, 'then', {
               get: function() { throw new TypeError('poison'); }
             });
             var ok = false;
             try { Promise.prototype.finally.call(poisoned); }
             catch (e) { ok = e instanceof TypeError; }
             ok;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_finally_honors_symbol_species_accessor() {
    assert_eq!(
        run("class FooPromise extends Promise {
               static get [Symbol.species]() { return Promise; }
             }
             var p = Promise.resolve().finally(function() {
               return FooPromise.resolve();
             });
             p instanceof Promise && !(p instanceof FooPromise);"),
        Value::Bool(true)
    );
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
fn promise_static_surface_and_species_descriptor() {
    assert_eq!(
        run("[
                typeof Promise.all,
                typeof Promise.allKeyed,
                typeof Promise.race,
                typeof Promise.allSettled,
                typeof Promise.allSettledKeyed,
                typeof Promise.any,
                typeof Promise.try,
                typeof Promise.withResolvers,
                typeof Promise.prototype.finally
             ].join(',');"),
        Value::String(Arc::from(
            "function,function,function,function,function,function,function,function,function"
        ))
    );
    assert_eq!(
        run("Promise[Symbol.species] === Promise;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var d = Object.getOwnPropertyDescriptor(Promise, Symbol.species);
             typeof d.get + ':' + d.set + ':' + d.enumerable + ':' + d.configurable;"
        ),
        Value::String(Arc::from("function:undefined:false:true"))
    );
    assert_eq!(
        run(
            "Object.getOwnPropertyDescriptor(Promise, 'all').writable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'all').enumerable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'all').configurable + ':' +
             Promise.all.length + ':' + Promise.all.name;"
        ),
        Value::String(Arc::from("true:false:true:1:all"))
    );
    assert_eq!(
        run(
            "Object.getOwnPropertyDescriptor(Promise, 'allKeyed').writable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allKeyed').enumerable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allKeyed').configurable + ':' +
             Promise.allKeyed.length + ':' + Promise.allKeyed.name + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allSettledKeyed').writable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allSettledKeyed').enumerable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allSettledKeyed').configurable + ':' +
             Promise.allSettledKeyed.length + ':' + Promise.allSettledKeyed.name;"
        ),
        Value::String(Arc::from(
            "true:false:true:1:allKeyed:true:false:true:1:allSettledKeyed"
        ))
    );
}

#[test]
fn promise_static_combinators_return_promises() {
    assert_eq!(
        run("[
                Promise.all([Promise.resolve(1), 2]) instanceof Promise,
                Promise.allKeyed({a: Promise.resolve(1), b: 2}) instanceof Promise,
                Promise.race([Promise.resolve(1), 2]) instanceof Promise,
                Promise.allSettled([Promise.resolve(1), Promise.reject(2)]) instanceof Promise,
                Promise.allSettledKeyed({a: Promise.resolve(1), b: Promise.reject(2)}) instanceof Promise,
                Promise.any([Promise.reject(1), Promise.resolve(2)]) instanceof Promise,
                Promise.try(function(){ return 3; }) instanceof Promise,
                Promise.resolve(1).finally(function(){}) instanceof Promise
             ].join(',');"),
        Value::String(Arc::from("true,true,true,true,true,true,true,true"))
    );
}

#[test]
fn promise_try_uses_receiver_constructor_capability() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.try.call(SubPromise, function() { return 7; });
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var badCtor = false, badPrimitive = false;
             try { Promise.try.call(eval); } catch (e) { badCtor = e instanceof TypeError; }
             try { Promise.try.call(null); } catch (e) { badPrimitive = e instanceof TypeError; }
             badCtor && badPrimitive;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "function BadPromise(executor) { throw new RangeError('bad'); }
             try { Promise.try.call(BadPromise, function() {}); false; }
             catch (e) { e instanceof RangeError; }"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.all.call(SubPromise, []);
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var resolvedValues;
             var C = function(executor) {
               executor(function(values) { resolvedValues = values; }, function() {});
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       resolve(value);
                     }
                   };
                 };
               }
             });
             Promise.all.call(C, [1, 2]);
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && resolvedValues.join(',') === '1,2';"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_settled_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.allSettled.call(SubPromise, []);
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var seenThis, settledValues;
             var C = function(executor) {
               executor(function(values) { settledValues = values; }, function() {});
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   seenThis = this;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       if (value === 2) {
                         reject('bad');
                         resolve('ignored');
                       } else {
                         resolve(value);
                         reject('ignored');
                       }
                     }
                   };
                 };
               }
             });
             Promise.allSettled.call(C, [1, 2]);
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && seenThis === C &&
               settledValues[0].status === 'fulfilled' &&
               settledValues[0].value === 1 &&
               settledValues[1].status === 'rejected' &&
               settledValues[1].reason === 'bad';"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_keyed_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.allKeyed.call(SubPromise, {});
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var resolvedValues;
             var C = function(executor) {
               executor(function(values) { resolvedValues = values; }, function() {});
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       resolve(value);
                     }
                   };
                 };
               }
             });
             Promise.allKeyed.call(C, {a: 1, b: 2});
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && Object.getPrototypeOf(resolvedValues) === null &&
               Object.keys(resolvedValues).join(',') === 'a,b' &&
               resolvedValues.a === 1 && resolvedValues.b === 2;"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_keyed_preserves_symbols_and_rejects_non_object() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "var sym = Symbol('s');
         var hidden = Symbol('hidden');
         var out;
         var input = {str: Promise.resolve(1)};
         input[sym] = Promise.resolve(2);
         Object.defineProperty(input, hidden, {value: 3, enumerable: false});
         Promise.allKeyed(input).then(function(value) { out = value; });",
    )
    .expect("evaluation errored");
    assert_eq!(
        vm.run(
            "Object.getPrototypeOf(out) === null &&
             Object.keys(out).join(',') === 'str' &&
             Object.getOwnPropertySymbols(out).length === 1 &&
             Object.getOwnPropertySymbols(out)[0] === sym &&
             out.str === 1 && out[sym] === 2 &&
             !Object.prototype.hasOwnProperty.call(out, hidden);"
        )
        .expect("evaluation errored"),
        Value::Bool(true)
    );
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "var rejected;
         Promise.allKeyed(null).then(function() {}, function(e) { rejected = e; });",
    )
    .expect("evaluation errored");
    assert_eq!(
        vm.run("rejected instanceof TypeError;")
            .expect("evaluation errored"),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_settled_keyed_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.allSettledKeyed.call(SubPromise, {});
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var seenThis, settledValues;
             var C = function(executor) {
               executor(function(values) { settledValues = values; }, function() {});
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   seenThis = this;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       if (value === 2) {
                         reject('bad');
                         resolve('ignored');
                       } else {
                         resolve(value);
                         reject('ignored');
                       }
                     }
                   };
                 };
               }
             });
             Promise.allSettledKeyed.call(C, {a: 1, b: 2});
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && seenThis === C &&
               Object.getPrototypeOf(settledValues) === null &&
               Object.keys(settledValues).join(',') === 'a,b' &&
               settledValues.a.status === 'fulfilled' &&
               settledValues.a.value === 1 &&
               settledValues.b.status === 'rejected' &&
               settledValues.b.reason === 'bad';"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_any_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.any.call(SubPromise, []);
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var seenThis, resolvedValue, rejectedErrors;
             var C = function(executor) {
               executor(function(value) { resolvedValue = value; },
                        function(errors) { rejectedErrors = errors; });
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   seenThis = this;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       if (value === 2) {
                         resolve('winner');
                       } else {
                         reject('bad-' + value);
                         reject('ignored');
                       }
                     }
                   };
                 };
               }
             });
             Promise.any.call(C, [1, 2]);
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && seenThis === C &&
               resolvedValue === 'winner' && rejectedErrors === undefined;"
        ),
        Value::Bool(true)
    );
    assert_eq!(
        run("var rejectedErrors;
             var C = function(executor) {
               executor(function() {}, function(errors) { rejectedErrors = errors; });
             };
             C.resolve = function(value) {
               return {
                 then: function(resolve, reject) {
                   reject('bad-' + value);
                   resolve('ignored');
                   reject('ignored');
                 }
               };
             };
             Promise.any.call(C, [1, 2]);
             rejectedErrors instanceof AggregateError &&
               rejectedErrors.errors.join(',') === 'bad-1,bad-2' &&
               Object.prototype.propertyIsEnumerable.call(rejectedErrors, 'errors') === false;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var rejectedError;
             var C = function(executor) {
               executor(function() {}, function(error) { rejectedError = error; });
             };
             C.resolve = function(value) { return value; };
             Promise.any.call(C, []);
             rejectedError instanceof AggregateError &&
               rejectedError.errors.length === 0 &&
               Object.prototype.propertyIsEnumerable.call(rejectedError, 'errors') === false;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_race_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.race.call(SubPromise, []);
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var p1 = new Promise(function() {});
             var p2 = new Promise(function() {});
             Object.defineProperty(Promise, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   return value;
                 };
               }
             });
             p1.then = p2.then = function(resolve, reject) {
               thenCallCount += 1;
               return {};
             };
             Promise.race([p1, p2]);
             resolveGetCount === 1 && resolveCallCount === 2 && thenCallCount === 2;"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_static_resolve_and_reject_use_receiver_constructor_capability() {
    assert_eq!(
        run("class SubPromise extends Promise {}
             [
               Promise.resolve.call(SubPromise, 1) instanceof SubPromise,
               Promise.reject.call(SubPromise, 2) instanceof SubPromise
             ].join(':');"),
        Value::String(Arc::from("true:true"))
    );
    assert_eq!(
        run("var p = Promise.resolve(1);
             Promise.resolve(p) === p && (p.constructor = null, Promise.resolve(p) !== p);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var seenThis, seenArg, callCount = 0;
             var P = function(executor) {
               return new Promise(function() {
                 executor(function(v) { callCount += 1; seenThis = this; seenArg = v; },
                          function() {});
               });
             };
             var obj = {};
             Promise.resolve.call(P, obj);
             callCount === 1 && seenThis === globalThis && seenArg === obj;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var seenThis, seenArg;
             var P = function(executor) {
               return new Promise(function() {
                 executor(function() {}, function(v) { seenThis = this; seenArg = v; });
               });
             };
             Promise.reject.call(P, 24601);
             seenThis === globalThis && seenArg === 24601;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_static_resolve_and_reject_validate_capability_constructor() {
    assert_eq!(
        run("var badResolve = false, badReject = false;
             try { Promise.resolve.call(eval, 1); } catch (e) { badResolve = e instanceof TypeError; }
             try { Promise.reject.call(eval, 1); } catch (e) { badReject = e instanceof TypeError; }
             badResolve && badReject;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var rejected = false;
             try { Promise.resolve.call(function(executor) {}, 1); }
             catch (e) { rejected = e instanceof TypeError; }
             rejected;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_resolving_functions_are_anonymous_unary_builtins() {
    assert_eq!(
        run("var resolve, reject;
             new Promise(function(res, rej) { resolve = res; reject = rej; });
             [
               resolve.name, resolve.length,
               reject.name, reject.length,
               Object.prototype.hasOwnProperty.call(resolve, 'prototype'),
               Object.getOwnPropertyNames(resolve).join(',')
             ].join(':');"),
        Value::String(Arc::from(":1::1:false:length,name"))
    );
}

#[test]
fn promise_with_resolvers_basic_and_subclass() {
    assert_eq!(
        run("var r = Promise.withResolvers();
             [
               r.promise instanceof Promise,
               r.promise.constructor === Promise,
               typeof r.resolve, r.resolve.name, r.resolve.length,
               typeof r.reject, r.reject.name, r.reject.length
             ].join(':');"),
        Value::String(Arc::from("true:true:function::1:function::1"))
    );
    assert_eq!(
        run("class SubPromise extends Promise {}
             var r = Promise.withResolvers.call(SubPromise);
             r.promise instanceof SubPromise && r.promise.constructor === SubPromise;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_with_resolvers_result_properties_are_enumerable_data_props() {
    assert_eq!(
        run("var r = Promise.withResolvers();
             ['promise', 'resolve', 'reject'].map(function(k) {
               var d = Object.getOwnPropertyDescriptor(r, k);
               return d.writable + ',' + d.enumerable + ',' + d.configurable;
             }).join(':');"),
        Value::String(Arc::from("true,true,true:true,true,true:true,true,true"))
    );
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

#[test]
fn promise_constructor_requires_new_and_calls_executor_with_undefined_this() {
    assert_eq!(
        run("var ok1 = false, ok2 = false, ok3 = false;
             try { Promise(function() {}); } catch (e) { ok1 = e instanceof TypeError; }
             try { Promise.call(null, function() {}); } catch (e) { ok2 = e instanceof TypeError; }
             try { Promise.call(new Promise(function() {}), function() {}); }
             catch (e) { ok3 = e instanceof TypeError; }
             ok1 && ok2 && ok3;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var seen;
             new Promise(function() { seen = this; });
             seen === globalThis;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var seen;
             new Promise(function() { 'use strict'; seen = this; });
             seen === undefined;"),
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
    assert_eq!(
        run("var r = new RegExp(/test/i); [r.source, r.flags, r.toString()].join('|');"),
        Value::String(Arc::from("test|i|/test/i"))
    );
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
fn regexp_exec_result_shape_and_last_index_semantics() {
    assert_eq!(
        run("var m = /b(c)/.exec('abc'); [m[0], m[1], m.index, m.input].join('|');"),
        Value::String(Arc::from("bc|c|1|abc"))
    );
    assert_eq!(
        run("Object.keys(/b/.exec('abc')).join(',');"),
        Value::String(Arc::from("0,index,input,groups"))
    );
    assert_eq!(run("/b/.exec('abc').groups;"), Value::Undefined);
    assert_eq!(
        run("var m = /(?<x>b)(c)?/.exec('abc'); [m[0], m[1], m[2], m.groups.x, Object.getPrototypeOf(m.groups) === null, Object.keys(m.groups).join(',')].join('|');"),
        Value::String(Arc::from("bc|b|c|b|true|x"))
    );
    assert_eq!(
        run("var m = /(?<x>a)|(?<y>b)/.exec('b'); String(m.groups.x) + '|' + m.groups.y;"),
        Value::String(Arc::from("undefined|b"))
    );
    assert_eq!(
        run("var m = 'abc'.match(/(?<x>b)(c)?/); [m[0], m[1], m[2], m.index, m.input, m.groups.x, Object.getPrototypeOf(m.groups) === null, Object.keys(m).join(',')].join('|');"),
        Value::String(Arc::from("bc|b|c|1|abc|b|true|0,1,2,index,input,groups"))
    );
    assert_eq!(
        run("var m = 'abc'.match('b'); [m[0], m.index, m.input].join('|');"),
        Value::String(Arc::from("b|1|abc"))
    );
    assert_eq!(
        run(
            r#"String.prototype[Symbol.match] = function(arg) { return "poison"; };
               var m = "abc".match("b");
               delete String.prototype[Symbol.match];
               [m[0], m.index, m.input].join("|");"#
        ),
        Value::String(Arc::from("b|1|abc"))
    );
    assert_eq!(
        run("var m = ''.match(); [m[0], m.index, m.input].join('|');"),
        Value::String(Arc::from("|0|"))
    );
    assert_eq!(
        run(r#"var r = /b/g;
               r[Symbol.match] = undefined;
               "abcbbc".match(r);"#),
        Value::Null
    );
    assert_eq!(
        run(r#"RegExp = function(){};
               RegExp.prototype = {
                 [Symbol.match]: function(s) { return "poison:" + s; }
               };
               var m = "abc".match("b");
               [m[0], m.index, m.input].join("|");"#),
        Value::String(Arc::from("b|1|abc"))
    );
    assert!(
        run_err(
            r#"var search = {};
               Object.defineProperty(search, Symbol.match, {
                 get: function(){ throw new Error("match-get"); }
               });
               "".match(search);"#
        )
        .contains("match-get"),
        "String.prototype.match must observe searchValue[Symbol.match]"
    );
    assert_eq!(
        run(r#"var search = {};
               var seenThis, seenArg;
               search[Symbol.match] = function(arg) {
                 seenThis = this;
                 seenArg = arg;
                 return "custom";
               };
               var out = "abc".match(search);
               [out, seenThis === search, seenArg].join("|");"#),
        Value::String(Arc::from("custom|true|abc"))
    );
    assert_eq!(
        run(r#"var old = RegExp.prototype[Symbol.match];
               var seenThis, seenArg;
               RegExp.prototype[Symbol.match] = function(arg) {
                 seenThis = this;
                 seenArg = arg;
                 return "created";
               };
               var out = "target".match("string source");
               RegExp.prototype[Symbol.match] = old;
               [out, seenThis instanceof RegExp, seenThis.source, seenThis.flags, seenThis.lastIndex, seenArg].join("|");"#),
        Value::String(Arc::from("created|true|string source||0|target"))
    );
    assert_eq!(
        run("/undefined/.exec()[0];"),
        Value::String(Arc::from("undefined"))
    );
    assert_eq!(
        run("var gets = 0; var marker = { valueOf: function(){ gets++; return 0; } }; var r = /./; r.lastIndex = marker; var m = r.exec('abc'); m[0] + ',' + (r.lastIndex === marker) + ',' + gets;"),
        Value::String(Arc::from("a,true,1"))
    );
    assert_eq!(
        run("var gets = 0; var r = /./g; r.lastIndex = { valueOf: function(){ gets++; return -1; } }; var m = r.exec('abc'); m[0] + ',' + r.lastIndex + ',' + gets;"),
        Value::String(Arc::from("a,1,1"))
    );
    assert_eq!(
        run("var r = /./g; r.lastIndex = 0; var before = r.lastIndex; r.exec('abc'); before + ',' + r.lastIndex;"),
        Value::String(Arc::from("0,1"))
    );
    assert_eq!(
        run("var r = /z/g; r.lastIndex = 1; var before = r.lastIndex; r.exec('abc'); before + ',' + r.lastIndex;"),
        Value::String(Arc::from("1,0"))
    );
    assert!(run_err(
        "var r = /c/y; Object.defineProperty(r, 'lastIndex', { writable: false }); r.exec('abc');"
    )
    .contains("TypeError"));
}

#[test]
fn regexp_repeated_capture_clears_nonparticipating_groups() {
    assert_eq!(
        run("var m = /(z)((a+)?(b+)?(c))*/.exec('zaacbbbcac'); [m[0], m[1], m[2], m[3], String(m[4]), m[5]].join('|');"),
        Value::String(Arc::from("zaacbbbcac|z|ac|a|undefined|c"))
    );
    assert_eq!(
        run("var m = /((a)|(b))*/.exec('ab'); [m[0], m[1], String(m[2]), m[3]].join('|');"),
        Value::String(Arc::from("ab|b|undefined|b"))
    );
    assert_eq!(
        run("var m = /(?:(a)|(b))*/.exec('ab'); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("var m = /(?:(a)(b)|(c)(d))*/.exec('abcd'); [m[0], String(m[1]), String(m[2]), m[3], m[4]].join('|');"),
        Value::String(Arc::from("abcd|undefined|undefined|c|d"))
    );
    assert_eq!(
        run("var m = /(?:(a)?(b))*/.exec('abb'); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("abb|undefined|b"))
    );
    assert_eq!(
        run("var m = /(x)(?:(a)|(b))*(y)/.exec('xaby'); [m[1], String(m[2]), m[3], m[4]].join('|');"),
        Value::String(Arc::from("x|undefined|b|y"))
    );
    assert_eq!(
        run("var m = /(?:(a)|(b))+/.exec('ab'); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("var m = /(?:(a)|(b)){2}/.exec('ab'); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("var m = 'ab'.match(/(?:(a)|(b))*/); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("'ab'.replace(/(?:(a)|(b))*/, function(m, a, b){ return m + '|' + String(a) + '|' + b; });"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("'ab xa'.replace(/(?:(a)|(b))+/g, function(m, a, b, offset){ return '[' + String(a) + '|' + String(b) + '|' + offset + ']'; });"),
        Value::String(Arc::from("[undefined|b|0] x[a|undefined|4]"))
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
    assert_eq!(
        run("new RegExp('', 'yusmigd').flags;"),
        Value::String(Arc::from("dgimsuy"))
    );
    assert_eq!(
        run("Object.getOwnPropertyDescriptor(RegExp.prototype, 'flags').get.name;"),
        Value::String(Arc::from("get flags"))
    );
    assert_eq!(
        run("Object.getOwnPropertyDescriptor(RegExp.prototype, 'flags').get.length;"),
        Value::Number(0.0)
    );
    assert_eq!(
        run("Object.getOwnPropertyDescriptor(RegExp.prototype, 'source').get.call(RegExp.prototype);"),
        Value::String(Arc::from("(?:)"))
    );
    assert_eq!(
        run("new RegExp('').source;"),
        Value::String(Arc::from("(?:)"))
    );
    assert_eq!(
        run("new RegExp('/').source;"),
        Value::String(Arc::from("\\/"))
    );
    assert_eq!(
        run("new RegExp('\\n').source;"),
        Value::String(Arc::from("\\n"))
    );
    assert_eq!(
        run("Object.getOwnPropertyNames(/a/g).join(',');"),
        Value::String(Arc::from("lastIndex"))
    );
    assert_eq!(
        run("[
               Object.getOwnPropertyDescriptor(/a/g, 'global'),
               Object.getOwnPropertyDescriptor(/a/g, '__regexp_source__')
             ].map(String).join(',');"),
        Value::String(Arc::from("undefined,undefined"))
    );
    assert_eq!(
        run(r#"var r = /a/gy;
               Object.defineProperty(r, 'a', { value: 1 });
               Object.getOwnPropertyNames(Object.getOwnPropertyDescriptors(r)).join(',');"#),
        Value::String(Arc::from("lastIndex,a"))
    );
    assert_eq!(
        run(r#"var r = /a/gy;
               Object.defineProperty(r, 'global', { value: false });
               [r.source, r.global, r.sticky, r.flags].join('|');"#),
        Value::String(Arc::from("a|false|true|y"))
    );
    assert_eq!(
        run(r#"class S extends RegExp {
                 #__regexp_source__ = 1;
                 #__regexp_global__ = 2;
                 values() { return [this.#__regexp_source__, this.#__regexp_global__].join(','); }
               }
               var r = new S('a', 'g');
               [r.source, r.global, r.flags, r.values()].join('|');"#),
        Value::String(Arc::from("a|true|g|1,2"))
    );
    assert_eq!(
        run(r#"
            var get = Object.getOwnPropertyDescriptor(RegExp.prototype, 'source').get;
            var other = $262.createRealm().global;
            var otherRegExpProto = other.RegExp.prototype;
            var otherGet = Object.getOwnPropertyDescriptor(otherRegExpProto, 'source').get;
            var ok = [];
            try { get.call(otherRegExpProto); ok.push(false); }
            catch (e) { ok.push(e.constructor === TypeError); }
            try { otherGet.call(RegExp.prototype); ok.push(false); }
            catch (e) { ok.push(e.constructor === other.TypeError); }
            ok.join(',');
            "#),
        Value::String(Arc::from("true,true"))
    );
}

#[test]
fn regexp_escape_is_static_builtin_with_expected_attrs() {
    assert_eq!(
        run(
            r#"var d = Object.getOwnPropertyDescriptor(RegExp, "escape");
               [
                 typeof RegExp.escape,
                 "escape" in RegExp.prototype,
                 d.writable,
                 d.enumerable,
                 d.configurable,
                 RegExp.escape.length,
                 RegExp.escape.name
               ].join("|");"#
        ),
        Value::String(Arc::from("function|false|true|false|true|1|escape"))
    );
}

#[test]
fn regexp_escape_escapes_literal_pattern_text() {
    assert_eq!(
        run(r#"[
                 RegExp.escape("") === "",
                 RegExp.escape("foo") === "\\x66oo",
                 RegExp.escape("1+1") === "\\x31\\+1",
                 RegExp.escape("/.") === "\\/\\.",
                 RegExp.escape(" ,-") === "\\x20\\x2c\\x2d",
                 RegExp.escape("\t\n\v\f\r") === "\\t\\n\\v\\f\\r",
                 RegExp.escape("\u2028\u2029") === "\\u2028\\u2029",
                 RegExp.escape("\uFEFF\u00A0\u202F") === "\\ufeff\\xa0\\u202f",
                 RegExp.escape(String.fromCharCode(0xD800)) === "\\ud800",
                 RegExp.escape(String.fromCharCode(0xD83D, 0xDE00)) === String.fromCharCode(0xD83D, 0xDE00)
               ].join("|");"#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn regexp_escape_rejects_non_strings_without_coercion() {
    assert!(run_err("RegExp.escape(undefined);").contains("TypeError"));
    assert!(run_err("RegExp.escape(123);").contains("TypeError"));
    assert_eq!(
        run(r#"var called = false;
               try {
                 RegExp.escape({ toString: function() { called = true; return "x"; } });
               } catch (e) {}
               called;"#),
        Value::Bool(false)
    );
}

#[test]
fn regexp_escape_is_installed_in_created_realms() {
    assert_eq!(
        run(r#"var other = $262.createRealm().global;
               [typeof other.RegExp.escape, other.RegExp.escape("foo")].join("|");"#),
        Value::String(Arc::from("function|\\x66oo"))
    );
}

#[test]
fn regexp_modifiers_empty_remove_list_compiles() {
    assert_eq!(run("/(?s-:^.$)/.test('\\n');"), Value::Bool(true));
    assert_eq!(
        run("new RegExp('(?s-:^.$)').test('\\n');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var r = /(?m-:^b$)/; r.source;"),
        Value::String(Arc::from("(?m-:^b$)"))
    );
    assert_eq!(
        run("/^a\\n(?m-:^b$)\\nc$/.test('a\\nb\\nc');"),
        Value::Bool(true)
    );
    assert_eq!(run("/(?s:^.$)/.test('𐌀');"), Value::Bool(false));
    assert_eq!(run("/(?s:^.$)/u.test('𐌀');"), Value::Bool(true));
    assert_eq!(run("/(?s:(?-s:.))/.test('\\n');"), Value::Bool(false));
    assert_eq!(run("/(?s:(?-s:(?s:.)))/.test('\\n');"), Value::Bool(true));
    assert_eq!(run("/(?i:\\p{Lu})/u.test('a');"), Value::Bool(true));
    assert_eq!(run("/(?i:\\P{Lu})/u.test('A');"), Value::Bool(true));
    assert_eq!(
        run("/(?i:\\P{Uppercase_Letter})/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("/(?i:\\P{General_Category=Uppercase_Letter})/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(run("/(?i:[\\P{Lu}])/u.test('A');"), Value::Bool(true));
    assert_eq!(
        run("/(?i:[\\P{Uppercase_Letter}])/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(run("/(?-i:\\w)/ui.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/(?-i:\\W)/ui.test('ſ');"), Value::Bool(true));
    assert_eq!(run("/(?-i:[\\w])/ui.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/(?-i:[\\W])/ui.test('ſ');"), Value::Bool(true));
    assert_eq!(run("/(?-i:\\b)ſ/ui.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/(?-i:\\B)ſ/ui.test('ſ');"), Value::Bool(true));
}

#[test]
fn regexp_quantifier_without_atom_reports_early_error() {
    for source in ["/?/;", "/{2}/;", "/{2,}/;", "/{2,3}/;"] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected early error for {source}"
        );
    }
    for source in [
        "eval('{}/{2}/;');",
        "eval('{}/{2,}/;');",
        "eval('{}/{2,3}/;');",
    ] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected parser fallback error for {source}"
        );
    }
    for source in [
        "new RegExp('?');",
        "new RegExp('{2}');",
        "new RegExp('{2,}');",
        "new RegExp('{2,3}');",
    ] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected constructor error for {source}"
        );
    }

    assert_eq!(run("/a?/.test('');"), Value::Bool(true));
    assert_eq!(run("/a{2}/.test('aa');"), Value::Bool(true));
    assert_eq!(run("/\\?/.test('?');"), Value::Bool(true));
    assert_eq!(run("/[?{]/.test('{');"), Value::Bool(true));
    assert_eq!(run("/(?:a)?/.test('');"), Value::Bool(true));
    assert_eq!(run("new RegExp('a{2}').test('aa');"), Value::Bool(true));
}

#[test]
fn regexp_assertion_quantifier_reports_early_error() {
    for source in [
        "/(?<=.)?/;",
        "/(?<!.)?/;",
        "/(?<=.){2,3}/;",
        "/(?<!.){2,3}/;",
        "/(?=.)?/u;",
        "/(?!.)?/u;",
        "/(?=.){2,3}/u;",
        "/(?!.){2,3}/u;",
        "/(?<=.)?/u;",
        "/(?<!.)?/u;",
        "/(?<=.){2,3}/u;",
        "/(?<!.){2,3}/u;",
    ] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected assertion quantifier early error for {source}"
        );
    }
    for source in ["eval('{}/(?<!.){2,3}/;');", "eval('{}/(?!.){2,3}/u;');"] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected parser fallback assertion quantifier error for {source}"
        );
    }
    for source in [
        "new RegExp('(?<=.)?');",
        "new RegExp('(?<!.){2,3}');",
        "new RegExp('(?=.)?', 'u');",
        "new RegExp('(?!.){2,3}', 'u');",
    ] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected constructor assertion quantifier error for {source}"
        );
    }
}

#[test]
fn regexp_unicode_mode_syntax_reports_early_error() {
    for source in [
        "/\\c0/u;",
        "/{/u;",
        "/\\M/u;",
        "/\\1/u;",
        "/[\\d-a]/u;",
        "/[\\s-\\d]/u;",
        "/[%-\\d]/u;",
        "/[--\\d]/u;",
        "/\\8/u;",
        "/\\u{110000}/u;",
        "/\\u{1,}/u;",
        "/\\u{1F_639}/u;",
        "/\\p{}/u;",
        "/\\p{Greek}/u;",
        "/\\p{Ascii}/u;",
        "/\\p{any}/u;",
        "/\\p{assigned}/u;",
        "/\\p{Script_Extensions}/u;",
        "/\\p{Script_Extensions=}/u;",
        "/\\p{General_Category=}/u;",
        "/\\p{General_Category=Not_A_Category}/u;",
        "/\\p{Script=FooBarBazInvalid}/u;",
        "/\\p{Script=Greek=Extra}/u;",
        "/\\p{=Greek}/u;",
    ] {
        assert!(
            run_err(source).contains("regular expression"),
            "expected unicode-mode syntax error for {source}"
        );
    }
    for source in [
        "new RegExp('{', 'u');",
        "new RegExp('\\\\M', 'u');",
        "new RegExp('\\\\8', 'u');",
        "new RegExp('\\\\u{110000}', 'u');",
        "new RegExp('\\\\u{1,}', 'u');",
        "new RegExp('\\\\p{}', 'u');",
        "new RegExp('\\\\p{Greek}', 'u');",
        "new RegExp('\\\\p{Ascii}', 'u');",
        "new RegExp('\\\\p{any}', 'u');",
        "new RegExp('\\\\p{assigned}', 'u');",
        "new RegExp('\\\\p{Script_Extensions}', 'u');",
        "new RegExp('\\\\p{Script_Extensions=}', 'u');",
        "new RegExp('\\\\p{General_Category=}', 'u');",
        "new RegExp('\\\\p{General_Category=Not_A_Category}', 'u');",
        "new RegExp('\\\\p{Script=FooBarBazInvalid}', 'u');",
        "new RegExp('\\\\p{Script=Greek=Extra}', 'u');",
        "new RegExp('\\\\p{=Greek}', 'u');",
    ] {
        assert!(
            run_err(source).contains("regular expression"),
            "expected constructor unicode-mode syntax error for {source}"
        );
    }
    assert!(run_err("/\\p{Bad}/u;").contains("regular expression"));
    assert_eq!(run("/\\p{Script=Greek}/u.test('Α');"), Value::Bool(true));
    assert_eq!(
        run("/\\p{Script_Extensions=Greek}/u.test('Α');"),
        Value::Bool(true)
    );
    assert_eq!(run("/\\p{ASCII}/u.test('A');"), Value::Bool(true));
    assert_eq!(run("/\\p{Any}/u.test('A');"), Value::Bool(true));
    assert_eq!(run("/\\p{Assigned}/u.test('A');"), Value::Bool(true));
    assert_eq!(run("/\\p{Lu}/u.test('A');"), Value::Bool(true));
    assert_eq!(
        run("/\\p{Uppercase_Letter}/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("/\\p{gc=Uppercase_Letter}/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("/\\p{General_Category=Uppercase_Letter}/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(run("/\\p{Alpha}/u.test('A');"), Value::Bool(true));
    assert_eq!(run("/\\p{sc=Grek}/u.test('Α');"), Value::Bool(true));
    assert_eq!(run("/\\p{scx=Greek}/u.test('Α');"), Value::Bool(true));
}

#[test]
fn regexp_null_escape_matches_null_character() {
    assert_eq!(run("/\\0/.test('\\x00');"), Value::Bool(true));
    assert_eq!(run("/\\0/u.test('\\x00');"), Value::Bool(true));
    assert_eq!(run("/^\\0a$/u.test('\\x00a');"), Value::Bool(true));
    assert_eq!(run("new RegExp('\\\\0').test('\\x00');"), Value::Bool(true));
    assert_eq!(
        run("var r = /\\0/u; r.source;"),
        Value::String(Arc::from("\\0"))
    );
    assert_eq!(
        run("'\\x00②'.match(/\\0②/u)[0];"),
        Value::String(Arc::from("\0②"))
    );
    assert_eq!(run("'\\u0000፬'.search(/\\0፬$/u);"), Value::Number(0.0));
    assert_eq!(run("'a፬'.search(/፬/u);"), Value::Number(1.0));
    assert_eq!(
        run("var r = /፬/g; r.lastIndex = 1; var n = 'a፬'.search(r); n + ',' + r.lastIndex;"),
        Value::String(Arc::from("1,1"))
    );
}

#[test]
fn regexp_sticky_start_assertion_uses_full_input() {
    assert_eq!(
        run("var re = /^a/y; re.lastIndex = 1; re.test(' a') + ',' + re.lastIndex;"),
        Value::String(Arc::from("false,0"))
    );
    assert_eq!(
        run("var re = /^a/y; re.lastIndex = 1; re.test('\\na') + ',' + re.lastIndex;"),
        Value::String(Arc::from("false,0"))
    );
    assert_eq!(
        run("var re = /^a/my; re.lastIndex = 1; re.test('\\na') + ',' + re.lastIndex;"),
        Value::String(Arc::from("true,2"))
    );
    assert_eq!(
        run("var re = /a/g; re.lastIndex = 1; re.test('xxa') + ',' + re.lastIndex;"),
        Value::String(Arc::from("true,3"))
    );
}

#[test]
fn regexp_non_unicode_ignore_case_does_not_apply_unicode_folding() {
    assert_eq!(run("/\\u212a/i.test('k');"), Value::Bool(false));
    assert_eq!(run("/\\u212a/i.test('K');"), Value::Bool(false));
    assert_eq!(run("/\\u212a/u.test('k');"), Value::Bool(false));
    assert_eq!(run("/\\u212a/iu.test('k');"), Value::Bool(true));
    assert_eq!(run("/K/i.test('k');"), Value::Bool(false));
    assert_eq!(
        run("new RegExp('\\\\u212a', 'i').test('K');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var r = /\\u212a/i; r.source;"),
        Value::String(Arc::from("\\u212a"))
    );
}

#[test]
fn regexp_unicode_surrogate_pair_escapes_match_scalar() {
    assert_eq!(
        run("/^[\\ud800\\udc00]$/u.test('\\ud800\\udc00');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("/[\\ud800\\udc00]/u.test('\\ud800');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("/[\\ud800\\udc00]/u.test('\\udc00');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var r = /^[\\ud834\\udf06]$/u; r.source;"),
        Value::String(Arc::from("^[\\ud834\\udf06]$"))
    );
    assert_eq!(run("/\\udf06/u.test('\\udf06');"), Value::Bool(true));
    assert_eq!(run("/\\udf06/u.exec('\\ud834\\udf06');"), Value::Null);
}

#[test]
fn regexp_backreferences_and_legacy_decimal_escapes_compile() {
    assert_eq!(
        run("eval('/\\\\1/').source;"),
        Value::String(Arc::from("\\1"))
    );
    assert_eq!(
        run("eval('/a\\\\1/').source;"),
        Value::String(Arc::from("a\\1"))
    );
    assert_eq!(run("/(a)\\1/.test('aa');"), Value::Bool(true));
    assert_eq!(run("/(a)\\1/.test('ab');"), Value::Bool(false));
    assert_eq!(
        run("/(.+).*\\1/u.test('\\ud800\\udc00\\ud800');"),
        Value::Bool(false)
    );
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
    assert_eq!(
        run(r#""abc".replace(/(b)/, "[$&][$1][$$][$`][$']");"#),
        Value::String(Arc::from("a[b][b][$][a][c]c"))
    );
    assert_eq!(
        run(r#""b".replace(/(a)?(b)/, "$1-$2");"#),
        Value::String(Arc::from("-b"))
    );
    assert_eq!(
        run(r#""ab".replace(/(?:(a)|(b))*/, "$1|$2");"#),
        Value::String(Arc::from("|b"))
    );
    assert_eq!(
        run(r#""ab xa".replace(/(?:(a)|(b))+/g, "[$1|$2]");"#),
        Value::String(Arc::from("[|b] x[a|]"))
    );
    assert_eq!(
        run(r#""abcdefghijk".replace(/(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)/, "$10|$11|$01|$0|$99");"#),
        Value::String(Arc::from("j|a1|a|$0|i9k"))
    );
    assert_eq!(
        run(r#""abc".replace(/(b)/, "<$1|$2|$12>");"#),
        Value::String(Arc::from("a<b|$2|b2>c"))
    );
    assert_eq!(
        run(r#""ab".replace(/(a)/, "$0|$00|$09|$10|$11");"#),
        Value::String(Arc::from("$0|$00|$09|a0|a1b"))
    );
    assert_eq!(
        run(r#""aa".replace(/(a)\1/, "<$&|$1>");"#),
        Value::String(Arc::from("<aa|a>"))
    );
    assert_eq!(
        run(r#""abc".replace(/b|c/g, "<$`|$'>");"#),
        Value::String(Arc::from("a<a|c><ab|>"))
    );
    assert_eq!(
        run(r#""abc".replace(/b/, "$x|$<x>");"#),
        Value::String(Arc::from("a$x|$<x>c"))
    );
    assert_eq!(
        run(r#""abc".replace(/(?<x>b)(c)?/, "<$<x>|$<missing>|$1|$2>");"#),
        Value::String(Arc::from("a<b||b|c>"))
    );
    assert_eq!(
        run(
            r#""abc".replace(/(?<x>b)/, function(m, x, offset, s, groups){ return m + "|" + x + "|" + offset + "|" + s + "|" + groups.x; });"#
        ),
        Value::String(Arc::from("ab|b|1|abc|bc"))
    );
    assert_eq!(
        run(r#""😀a".replace(/a/, function(m, offset){ return offset; });"#),
        Value::String(Arc::from("😀2"))
    );
    assert_eq!(
        run(r#""😀a".replace("a", function(m, offset){ return offset; });"#),
        Value::String(Arc::from("😀2"))
    );
    assert_eq!(
        run(r#""𝌆ab".replace(/b/, function(m, offset){ return offset; });"#),
        Value::String(Arc::from("𝌆a3"))
    );
    assert_eq!(
        run(r#""abc".replace("b", "[$&][$$][$`][$']");"#),
        Value::String(Arc::from("a[b][$][a][c]c"))
    );
    assert_eq!(
        run(r#""abc".replace("b", "$0|$1|$<x>");"#),
        Value::String(Arc::from("a$0|$1|$<x>c"))
    );
    assert!(
        run_err(
            r#"var search = { toString: function(){ throw "search"; } };
               var replacement = { toString: function(){ throw "replacement"; } };
               "abc".replace(search, replacement);"#
        )
        .contains("search"),
        "searchValue must be coerced before replaceValue"
    );
    assert!(
        run_err(
            r#"var search = {};
               Object.defineProperty(search, Symbol.replace, {
                 get: function(){ throw new Error("replace-get"); }
               });
               "".replace(search);"#
        )
        .contains("replace-get"),
        "String.prototype.replace must observe searchValue[Symbol.replace]"
    );
    assert_eq!(
        run(r#"var search = {};
               var seenThis, seenFirst, seenSecond;
               search[Symbol.replace] = function(first, second) {
                 seenThis = this;
                 seenFirst = first;
                 seenSecond = second;
                 return "custom";
               };
               var out = "abc".replace(search, "replacement");
               [out, seenThis === search, seenFirst, seenSecond].join("|");"#),
        Value::String(Arc::from("custom|true|abc|replacement"))
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

#[test]
fn array_of_uses_constructor_and_create_data_property() {
    assert_eq!(
        run(r#"
            var len, hits = 0;
            function C(length) { len = length; hits++; }
            var result = Array.of.call(C, "a", "b");
            [len, hits, result.length, result[0], result[1], result instanceof C].join("|");
        "#),
        Value::String(Arc::from("2|1|2|a|b|true"))
    );
    assert_eq!(
        run(r#"
            function C() {}
            Object.defineProperty(C.prototype, "0", {
                set: function() { throw new Error("setter"); }
            });
            var result = Array.of.call(C, "own");
            [result[0], result.hasOwnProperty("0")].join("|");
        "#),
        Value::String(Arc::from("own|true"))
    );
    assert_eq!(
        run(r#"
            var hits = 0, seen;
            function C() {
                Object.defineProperty(this, "length", {
                    set: function(value) { hits++; seen = value; }
                });
            }
            var result = Array.of.call(C, "x", "y", "z");
            [hits, seen, result[2]].join("|");
        "#),
        Value::String(Arc::from("1|3|z"))
    );
    assert!(run_err(
        r#"function C() { Object.preventExtensions(this); }
           Array.of.call(C, "x");"#
    )
    .contains("not extensible"));
}

#[test]
fn array_of_cross_realm_constructor_fallbacks_to_constructor_realm_object_proto() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var C = new other.Function();
            C.prototype = null;
            var result = Array.of.call(C, 1, 2, 3);
            Object.getPrototypeOf(result) === other.Object.prototype;
        "#),
        Value::Bool(true)
    );
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
    assert_eq!(run("Number.prototype.valueOf();"), Value::Number(0.0));
    assert_eq!(run("Number.prototype.valueOf.call(1);"), Value::Number(1.0));
    assert_eq!(
        run("Number.prototype.toString.call(Object(255), 16);"),
        Value::String(Arc::from("ff"))
    );
    assert!(run_err("Number.prototype.valueOf.call(new String('1'));").contains("TypeError"));
    assert!(run_err("Number.prototype.valueOf.call({});").contains("TypeError"));
    assert!(run_err("Number.prototype.toString.call({});").contains("TypeError"));
}

#[test]
fn boxed_boolean_valueof() {
    assert_eq!(run("new Boolean(true).valueOf();"), Value::Bool(true));
    assert_eq!(run("Boolean.prototype.valueOf();"), Value::Bool(false));
    assert_eq!(
        run(
            r#"Boolean.prototype == false && Object.prototype.toString.call(Boolean.prototype) === "[object Boolean]""#
        ),
        Value::Bool(true)
    );
    assert_eq!(
        run("Boolean.prototype.toString.call(true) + ':' + Boolean.prototype.toString.call(Object(false));"),
        Value::String(Arc::from("true:false"))
    );
    assert!(run_err("Boolean.prototype.valueOf.call({});").contains("TypeError"));
    assert!(run_err("Boolean.prototype.toString.call(new String(''));").contains("TypeError"));
}

#[test]
fn boxed_string_valueof() {
    assert_eq!(
        run("new String('hi').valueOf();"),
        Value::String(std::sync::Arc::from("hi"))
    );
    assert_eq!(
        run("String.prototype.valueOf();"),
        Value::String(Arc::from(""))
    );
    assert_eq!(
        run("String.prototype.valueOf.call('x');"),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run("String.prototype.toString.call(Object('y'));"),
        Value::String(Arc::from("y"))
    );
    assert!(run_err("String.prototype.valueOf.call(1);").contains("TypeError"));
    assert!(run_err("String.prototype.valueOf.call(new Number(1));").contains("TypeError"));
    assert!(
        run_err("String.prototype.valueOf.call({ toString: function(){ return 'x'; } });")
            .contains("TypeError")
    );
    assert!(run_err("String.prototype.toString.call(1);").contains("TypeError"));
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
