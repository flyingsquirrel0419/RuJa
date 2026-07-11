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
fn array_some_every_use_array_like_property_access() {
    assert!(run_err("Array.prototype.some.call(null, function() {})")
        .contains("Cannot convert undefined or null to object"));
    assert!(run_err("[].every({})").contains("Array predicate is not callable"));
    assert_eq!(
        run(r#"var seen = [];
               var receiver = 0;
               var result = Array.prototype.some.call({0: 11, 1: 12, length: 2}, function(value, index, object) {
                 receiver = this.valueOf();
                 seen.push(value + ":" + index + ":" + object.length);
                 return value === 12;
               }, 7);
               result + "|" + receiver + "|" + seen.join(",");"#),
        Value::String(Arc::from("true|7|11:0:2,12:1:2"))
    );
    assert_eq!(
        run(r#"Array.prototype[1] = 13;
               var seen = [];
               var result = [, , ,].every(function(value, index) {
                 seen.push(value + ":" + index);
                 return value !== 13;
               });
               delete Array.prototype[1];
               result + "|" + seen.join(",");"#),
        Value::String(Arc::from("false|13:1"))
    );
    assert_eq!(
        run(r#"var arr = [1, 2, 3];
               var seen = [];
               arr.some(function(value, index, object) {
                 seen.push(String(value));
                 if (index === 0) {
                   object.length = 1;
                   object[2] = 9;
                 }
                 return false;
               });
               seen.join(",");"#),
        Value::String(Arc::from("1,9"))
    );
}

#[test]
fn array_includes_nan() {
    assert_eq!(run("[NaN].includes(NaN);"), Value::Bool(true));
}

#[test]
fn global_uri_functions_follow_percent_encoding_rules() {
    assert_eq!(
        run("decodeURI('%3B') + '|' + decodeURIComponent('%3B');"),
        Value::String(Arc::from("%3B|;"))
    );
    assert_eq!(
        run("decodeURI('%5E') + '|' + decodeURIComponent('%2F');"),
        Value::String(Arc::from("^|/"))
    );
    assert_eq!(
        run("encodeURI('http://ru.wikipedia.org/wiki/Юникод');"),
        Value::String(Arc::from(
            "http://ru.wikipedia.org/wiki/%D0%AE%D0%BD%D0%B8%D0%BA%D0%BE%D0%B4"
        ))
    );
    assert_eq!(
        run("encodeURIComponent(';/?:@&=+$,#');"),
        Value::String(Arc::from("%3B%2F%3F%3A%40%26%3D%2B%24%2C%23"))
    );
    assert_eq!(
        run("var s = String.fromCharCode(0xDB80, 0xDC00); s.length + '|' + s.charCodeAt(0).toString(16) + '|' + s.charCodeAt(1).toString(16) + '|' + encodeURI(s);"),
        Value::String(Arc::from("2|db80|dc00|%F3%B0%80%80"))
    );
    assert_eq!(
        run("var s = String.fromCharCode(0xDB80, 0xDC00); var p = String.fromCodePoint(0xF0000); (s === p) + '|' + p.length + '|' + encodeURI(p) + '|' + encodeURI(s.substring(0, 2)) + '|' + encodeURI(s.slice(0, 2));"),
        Value::String(Arc::from(
            "true|2|%F3%B0%80%80|%F3%B0%80%80|%F3%B0%80%80"
        ))
    );
    assert_eq!(
        run("decodeURI('%F3%B0%80%80') === String.fromCharCode(0xDB80, 0xDC00);"),
        Value::Bool(true)
    );
    assert!(run_err("decodeURIComponent('%C0%AF');").contains("URIError"));
    assert!(run_err("decodeURIComponent('%ED%BF%BF');").contains("URIError"));
    assert!(run_err("encodeURI(String.fromCharCode(0xD800));").contains("URIError"));
    assert!(run_err("encodeURIComponent(String.fromCharCode(0xDC00));").contains("URIError"));
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
fn string_constructor_observes_object_to_primitive_string_hint() {
    assert_eq!(
        run(r#"var old = Array.prototype.toString;
               Array.prototype.toString = function() { return "__ARRAY__"; };
               var result = String(new Array);
               Array.prototype.toString = old;
               result;"#),
        Value::String(Arc::from("__ARRAY__"))
    );
}

#[test]
fn string_constructor_skips_non_callable_to_primitive_methods() {
    assert_eq!(
        run(r#"var oldToString = Array.prototype.toString;
               var oldValueOf = Array.prototype.valueOf;
               Array.prototype.toString = {};
               Array.prototype.valueOf = function() { return 5; };
               var result = String([]);
               Array.prototype.toString = oldToString;
               Array.prototype.valueOf = oldValueOf;
               result;"#),
        Value::String(Arc::from("5"))
    );
}

#[test]
fn string_exotic_indices_are_enumerable_read_only_properties() {
    assert_eq!(
        run(r#"var s = new String("abc");
               var d = Object.getOwnPropertyDescriptor(s, "0");
               s[0] = "z";
               [
                 s[0],
                 d.writable,
                 d.enumerable,
                 d.configurable,
                 s.propertyIsEnumerable("0"),
                 delete s[0],
                 s[0]
               ].join("|");"#),
        Value::String(Arc::from("a|false|true|false|true|false|a"))
    );
    assert!(
        run_err(r#""use strict"; var s = new String("abc"); s[0] = "z";"#).contains("read only")
    );
}

#[test]
fn string_locale_compare_uses_canonical_equivalence() {
    assert_eq!(
        run(r#""o\u0308".localeCompare("\u00f6");"#),
        Value::Number(0.0)
    );
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
fn string_match_all_uses_regexp_iterator_semantics() {
    assert_eq!(
        run(r#"
            var matches = Array.from("a1b22".matchAll(/\d+/g));
            [
              matches.length,
              matches[0][0], matches[0].index, matches[0].input,
              matches[1][0], matches[1].index, matches[1].input
            ].join("|");
        "#),
        Value::String(Arc::from("2|1|1|a1b22|22|3|a1b22"))
    );
    assert_eq!(
        run(r#"
            var regexp = /./g;
            regexp.lastIndex = { valueOf: function() { return 2; } };
            var iterator = regexp[Symbol.matchAll]("abcd");
            regexp.lastIndex = 0;
            var first = iterator.next().value;
            first[0] + ":" + first.index;
        "#),
        Value::String(Arc::from("c:2"))
    );
    assert!(run_err(r#""abc".matchAll(/./)"#).contains("TypeError"));
}

#[test]
fn string_match_all_delegates_custom_matcher_before_string_coercion() {
    assert_eq!(
        run(r#"
            var seenThis, seenArg;
            var matcher = {};
            matcher[Symbol.matchAll] = function(value) {
              seenThis = this;
              seenArg = value;
              return "delegated";
            };
            var result = String.prototype.matchAll.call(7, matcher);
            result + ":" + (seenThis === matcher) + ":" + (seenArg === 7);
        "#),
        Value::String(Arc::from("delegated:true:true"))
    );
}

#[test]
fn generated_symbols_do_not_collide_with_well_known_symbols() {
    assert_eq!(run("Symbol() === Symbol.iterator;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.match;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.unscopables;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.species;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.dispose;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.asyncDispose;"), Value::Bool(false));
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
fn disposal_well_known_symbols_are_exposed() {
    assert_eq!(
        run(r#"
            [
              typeof Symbol.dispose,
              typeof Symbol.asyncDispose,
              String(Symbol.dispose),
              String(Symbol.asyncDispose),
              Symbol.keyFor(Symbol.dispose) === undefined,
              Symbol.keyFor(Symbol.asyncDispose) === undefined,
              Symbol.for("Symbol.dispose") !== Symbol.dispose,
              Symbol.for("Symbol.asyncDispose") !== Symbol.asyncDispose
            ].join("|");
        "#),
        Value::String(Arc::from(
            "symbol|symbol|Symbol(Symbol.dispose)|Symbol(Symbol.asyncDispose)|true|true|true|true"
        ))
    );
    assert_eq!(
        run(r#"
            var disposeDesc = Object.getOwnPropertyDescriptor(Symbol, "dispose");
            var asyncDisposeDesc = Object.getOwnPropertyDescriptor(Symbol, "asyncDispose");
            [
              disposeDesc.writable,
              disposeDesc.enumerable,
              disposeDesc.configurable,
              disposeDesc.value === Symbol.dispose,
              asyncDisposeDesc.writable,
              asyncDisposeDesc.enumerable,
              asyncDisposeDesc.configurable,
              asyncDisposeDesc.value === Symbol.asyncDispose
            ].join("|");
        "#),
        Value::String(Arc::from("false|false|false|true|false|false|false|true"))
    );
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global.Symbol;
            [
              Symbol.dispose === other.dispose,
              Symbol.asyncDispose === other.asyncDispose
            ].join("|");
        "#),
        Value::String(Arc::from("true|true"))
    );
}

#[test]
fn symbol_property_keys_drive_function_name_inference() {
    assert_eq!(
        run(r#"
            var method = Symbol("method");
            var fn = Symbol("fn");
            var cls = Symbol("cls");
            var getter = Symbol();
            var setter = Symbol();
            var o = {
              [method]() {},
              [fn]: function() {},
              [cls]: class {},
              get [getter]() { return 1; },
              set [setter](v) {}
            };
            var cover = { x: (0, function() {}) };
            class C {
              [method]() {}
              get [fn]() { return 1; }
              set [getter](v) {}
              static [cls]() {}
            }
            [
              o[method].name,
              o[fn].name,
              o[cls].name,
              Object.getOwnPropertyDescriptor(o, getter).get.name,
              Object.getOwnPropertyDescriptor(o, setter).set.name,
              cover.x.name,
              C.prototype[method].name,
              Object.getOwnPropertyDescriptor(C.prototype, fn).get.name,
              Object.getOwnPropertyDescriptor(C.prototype, getter).set.name,
              C[cls].name
            ].join("|");
        "#),
        Value::String(Arc::from(
            "[method]|[fn]|[cls]|get |set ||[method]|get [fn]|set |[cls]"
        ))
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
    assert_eq!(
        run(r#"
            [
              Symbol.prototype[Symbol.toPrimitive].length,
              Symbol.prototype[Symbol.toPrimitive].name,
              Object.getOwnPropertyDescriptor(Symbol.prototype, Symbol.toPrimitive).writable,
              Object.getOwnPropertyDescriptor(Symbol.prototype, Symbol.toPrimitive).configurable,
              Symbol.prototype[Symbol.toPrimitive].call(Object(Symbol("p")), "default").toString(),
              Symbol.prototype[Symbol.toStringTag],
              Object.getOwnPropertyDescriptor(Symbol.prototype, Symbol.toStringTag).writable,
              Object.getOwnPropertyDescriptor(Symbol.prototype, Symbol.toStringTag).configurable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "1|[Symbol.toPrimitive]|false|true|Symbol(p)|Symbol|false|true"
        ))
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
fn typed_array_constructor_toindex_errors_before_newtarget_prototype() {
    assert_eq!(
        run(r#"
            var newTarget = function() {}.bind(null);
            Object.defineProperty(newTarget, "prototype", {
              get: function() { throw new Error("prototype"); }
            });
            var log = [];
            for (var i = 0; i < 2; i++) {
              var C = i === 0 ? Uint8Array : BigInt64Array;
              try {
                Reflect.construct(C, [Symbol()], newTarget);
                log.push("none");
              } catch (e) {
                log.push(e.name + ":" + e.message);
              }
            }
            log.join("|");
        "#),
        Value::String(Arc::from(
            "TypeError:Cannot convert Symbol to number|TypeError:Cannot convert Symbol to number"
        ))
    );
}

#[test]
fn typed_array_constructors_create_array_buffer_backed_views() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(16);
            var bytes = new Uint8Array(buffer);
            bytes[0] = 7;
            var ints = new Int16Array(buffer, 2, 2);
            ints[0] = 258;
            [
              bytes.length,
              bytes.byteLength,
              bytes.byteOffset,
              bytes.buffer === buffer,
              ints.length,
              ints.byteLength,
              ints.byteOffset,
              ints.buffer === buffer,
              bytes[0],
              bytes[2],
              bytes[3]
            ].join(",");
        "#),
        Value::String(Arc::from("16,16,0,true,2,4,2,true,7,2,1"))
    );
    assert!(
        run_err("new Int16Array(new ArrayBuffer(3));").contains("RangeError"),
        "misaligned ArrayBuffer byte length should throw RangeError"
    );
    assert!(
        run_err("new Int16Array(new ArrayBuffer(4), 1);").contains("RangeError"),
        "misaligned TypedArray byte offset should throw RangeError"
    );
    assert!(
        run_err("new Uint8Array(new ArrayBuffer(4), 3, 2);").contains("RangeError"),
        "TypedArray view past the ArrayBuffer should throw RangeError"
    );
}

#[test]
fn typed_array_array_buffer_constructor_validates_detach_after_coercions() {
    assert_eq!(
        run(r#"
            function thrownName(fn) {
              try {
                fn();
              } catch (e) {
                return e.name;
              }
              return "none";
            }
            var log = [];
            var preOffset = new ArrayBuffer(8);
            $262.detachArrayBuffer(preOffset);
            var r1 = thrownName(function() {
              new Uint8Array(preOffset, {
                valueOf: function() {
                  log.push("pre-offset");
                  return 0;
                }
              });
            });
            var preLength = new ArrayBuffer(8);
            $262.detachArrayBuffer(preLength);
            var r2 = thrownName(function() {
              new Uint8Array(preLength, 0, {
                valueOf: function() {
                  log.push("pre-length");
                  return 1;
                }
              });
            });
            var detachOffset = new ArrayBuffer(8);
            var r3 = thrownName(function() {
              new Uint8Array(detachOffset, {
                valueOf: function() {
                  log.push("detach-offset");
                  $262.detachArrayBuffer(detachOffset);
                  return 0;
                }
              });
            });
            var detachLength = new ArrayBuffer(8);
            var r4 = thrownName(function() {
              new Uint8Array(detachLength, 0, {
                valueOf: function() {
                  log.push("detach-length");
                  $262.detachArrayBuffer(detachLength);
                  return 1;
                }
              });
            });
            var bigintOffset = new ArrayBuffer(16);
            var r5 = thrownName(function() {
              new BigInt64Array(bigintOffset, {
                valueOf: function() {
                  log.push("bigint-offset");
                  $262.detachArrayBuffer(bigintOffset);
                  return 0;
                }
              });
            });
            var rangeLength = new ArrayBuffer(8);
            var r6 = thrownName(function() {
              new Uint8Array(rangeLength, 99, {
                valueOf: function() {
                  log.push("range-length");
                  return 1;
                }
              });
            });
            [r1, r2, r3, r4, r5, r6, log.join("|")].join(",");
        "#),
        Value::String(Arc::from(
            "TypeError,TypeError,TypeError,TypeError,TypeError,RangeError,pre-offset|pre-length|detach-offset|detach-length|bigint-offset|range-length",
        ))
    );
}

#[test]
fn typed_array_bigint_constructors_expose_element_size_and_validate_receivers() {
    assert_eq!(
        run(r#"
            var ctorDesc = Object.getOwnPropertyDescriptor(BigInt64Array, "BYTES_PER_ELEMENT");
            var protoDesc = Object.getOwnPropertyDescriptor(BigUint64Array.prototype, "BYTES_PER_ELEMENT");
            var TypedArrayPrototype = Object.getPrototypeOf(BigInt64Array.prototype);
            var bufferGetter = Object.getOwnPropertyDescriptor(TypedArrayPrototype, "buffer").get;
            var lengthGetter = Object.getOwnPropertyDescriptor(TypedArrayPrototype, "length").get;
            var ta = new BigInt64Array(2);
            var throwsOnProto = false;
            var throwsOnObject = false;
            try { BigInt64Array.prototype.buffer; } catch (e) { throwsOnProto = e instanceof TypeError; }
            try { bufferGetter.call({}); } catch (e) { throwsOnObject = e instanceof TypeError; }
            [
              ctorDesc.value,
              ctorDesc.writable,
              ctorDesc.enumerable,
              ctorDesc.configurable,
              protoDesc.value,
              protoDesc.writable,
              protoDesc.enumerable,
              protoDesc.configurable,
              bufferGetter.call(ta) instanceof ArrayBuffer,
              lengthGetter.call(ta),
              throwsOnProto,
              throwsOnObject
            ].join(",");
            "#),
        Value::String(Arc::from(
            "8,false,false,false,8,false,false,false,true,2,true,true",
        ))
    );
}

#[test]
fn typed_array_constructors_inherit_from_shared_intrinsics() {
    assert_eq!(
        run(r#"
            var TypedArray = Object.getPrototypeOf(Int8Array);
            var TypedArrayPrototype = TypedArray.prototype;
            [
              Int8Array.length,
              BigUint64Array.length,
              Object.getPrototypeOf(Uint8Array) === TypedArray,
              Object.getPrototypeOf(Float64Array) === TypedArray,
              Object.getPrototypeOf(Uint8Array.prototype) === TypedArrayPrototype,
              Object.getPrototypeOf(BigInt64Array.prototype) === TypedArrayPrototype,
              Uint8Array.prototype.hasOwnProperty("buffer"),
              Float64Array.prototype.hasOwnProperty("byteLength"),
              BigInt64Array.prototype.hasOwnProperty("byteOffset"),
              BigUint64Array.prototype.hasOwnProperty("length"),
              typeof Object.getOwnPropertyDescriptor(TypedArrayPrototype, "buffer").get,
              typeof Object.getOwnPropertyDescriptor(TypedArrayPrototype, "length").get
            ].join(",");
            "#),
        Value::String(Arc::from(
            "3,3,true,true,true,true,false,false,false,false,function,function",
        ))
    );
}

#[test]
fn typed_array_static_from_and_of_inherit_from_intrinsic_constructor() {
    assert_eq!(
        run(r#"
            var calls = [];
            var from = Uint8Array.from({0: 7, 1: 260, length: 2}, function(v, i) {
              calls.push(this.tag + ":" + i + ":" + v);
              return v + i;
            }, { tag: "ctx" });
            var of = Int16Array.of(-1, 65535);
            [
              typeof Uint8Array.from,
              Uint8Array.hasOwnProperty("from"),
              Object.getPrototypeOf(Uint8Array).hasOwnProperty("from"),
              from instanceof Uint8Array,
              from.length,
              from[0],
              from[1],
              calls.join("|"),
              of instanceof Int16Array,
              of.length,
              of[0],
              of[1]
            ].join(",");
        "#),
        Value::String(Arc::from(
            "function,false,true,true,2,7,5,ctx:0:7|ctx:1:260,true,2,-1,-1"
        ))
    );
}

#[test]
fn typed_array_static_from_constructs_before_array_like_elements() {
    assert_eq!(
        run(r#"
            var log = [];
            function Custom(length) {
              log.push("construct:" + length);
              return new Uint8Array(length);
            }
            var source = {
              get length() { log.push("length"); return 1; },
              get 0() { log.push("element"); return 9; }
            };
            var result = Uint8Array.from.call(Custom, source);
            [result[0], log.join("|")].join(",");
        "#),
        Value::String(Arc::from("9,length|construct:1|element"))
    );
}

#[test]
fn typed_array_static_from_and_of_reject_immutable_backing_results() {
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try {
                fn();
              } catch (e) {
                return e instanceof TypeError;
              }
              return false;
            }
            function Custom(length) {
              return new Uint8Array(new ArrayBuffer(length).transferToImmutable());
            }
            [
              throwsTypeError(function() { Uint8Array.from.call(Custom, [1]); }),
              throwsTypeError(function() { Uint8Array.of.call(Custom, 1); })
            ].join(",");
        "#),
        Value::String(Arc::from("true,true"))
    );
}

#[test]
fn typed_array_static_from_caches_iterator_next_method() {
    assert_eq!(
        run(r#"
            var nextGets = 0;
            var nextCalls = 0;
            var iterable = {};
            Object.defineProperty(iterable, Symbol.iterator, {
              value: function() {
                var values = [4];
                return {
                  get next() {
                    nextGets += 1;
                    return function() {
                      nextCalls += 1;
                      if (values.length === 0) return { done: true };
                      return { value: values.pop(), done: false };
                    };
                  }
                };
              }
            });
            var result = Uint8Array.from(iterable);
            [result.length, result[0], nextGets, nextCalls].join(",");
        "#),
        Value::String(Arc::from("1,4,1,2"))
    );
}

#[test]
fn typed_array_numeric_proto_set_distinguishes_valid_and_invalid_indices() {
    assert_eq!(
        run(r#"
            var ta = new Int32Array(1);
            ta[0] = 7;
            var obj = Object.create(ta);
            obj[0] = 9;
            obj.NaN = 10;
            [
              ta[0],
              obj[0],
              obj.hasOwnProperty("0"),
              Object.getOwnPropertyDescriptor(obj, "NaN") === undefined
            ].join(",");
        "#),
        Value::String(Arc::from("7,9,true,true"))
    );
}

#[test]
fn typed_array_numeric_set_converts_value_before_index_validation() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([42]);
            var calls = 0;
            var value = {
              valueOf: function() {
                calls += 1;
                return 7;
              }
            };
            sample["1.1"] = value;
            sample["-0"] = value;
            sample["-1"] = value;
            sample["1"] = value;
            sample["2"] = value;
            [calls, sample[0], sample["1.1"], sample["-0"], sample["-1"], sample["1"], sample["2"]].join(",");
        "#),
        Value::String(Arc::from("5,42,,,,,"))
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([42]);
            $262.detachArrayBuffer(sample.buffer);
            sample[0] = { valueOf: function() { throw new Error("boom"); } };
            "#
        )
        .contains("boom"),
        "TypedArray [[Set]] should convert values before detached-buffer validation"
    );
    assert!(
        run_err(r#"new BigInt64Array(1)[0] = "definitely not a bigint";"#).contains("SyntaxError"),
        "BigInt TypedArray [[Set]] should preserve StringToBigInt SyntaxError"
    );
    assert_eq!(
        run(r#"
            var receiver = new Int32Array(10);
            var obj = Object.create(receiver);
            var calls = 0;
            var value = {
              valueOf: function() {
                calls += 1;
                return 1;
              }
            };
            [Reflect.set(obj, 100, value, receiver), calls].join(",");
        "#),
        Value::String(Arc::from("true,1"))
    );
}

#[test]
fn typed_array_length_allocation_keeps_backing_buffer_live_after_gc() {
    assert_eq!(
        run(r#"
            var ta = new Uint8Array(9);
            var trash = [];
            for (var i = 0; i < 2000; i++) trash.push({ i: i });
            [ta.length, ta.byteLength, ta[0], ta[4], ta[8]].join(",");
        "#),
        Value::String(Arc::from("9,9,0,0,0"))
    );
}

#[test]
fn typed_array_constructor_observes_array_iterator_prototype_next_override() {
    assert_eq!(
        run(r#"
            var ArrayIteratorPrototype = Object.getPrototypeOf([].values());
            var oldNext = ArrayIteratorPrototype.next;
            var values;
            ArrayIteratorPrototype.next = function() {
                var done = values.length === 0;
                var value = values.pop();
                return { value: value, done: done };
            };
            try {
                values = [1, 2, 3, 4];
                var ta = new Uint8Array([0]);
                [ta.length, ta[0], ta[1], ta[2], ta[3]].join(",");
            } finally {
                ArrayIteratorPrototype.next = oldNext;
            }
        "#),
        Value::String(Arc::from("4,4,3,2,1"))
    );
}

#[test]
fn typed_array_delete_canonical_numeric_indices_follow_integer_indexed_exotic() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(2);
            var values = [];
            values.push(delete sample[0]);
            values.push(delete sample["0"]);
            values.push(delete sample[1]);
            values.push(delete sample["2"]);
            values.push(delete sample["-1"]);
            values.push(delete sample["1.1"]);
            values.push(delete sample["-0"]);
            values.push(delete sample[-0]);
            values.join(",");
        "#),
        Value::String(Arc::from("false,false,false,true,true,true,true,false"))
    );
    assert_eq!(
        run(r#"
            "use strict";
            var sample = new Uint8Array(1);
            [delete sample["-0"], delete sample["1.1"], delete sample["1"]].join(",");
        "#),
        Value::String(Arc::from("true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(1);
            $262.detachArrayBuffer(sample.buffer);
            [delete sample[0], delete sample["0"], delete sample["1"], delete sample["-0"], delete sample["1.1"], delete sample["Infinity"]].join(",");
        "#),
        Value::String(Arc::from("true,true,true,true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(1);
            var values = [];
            values.push(delete sample[0]);
            values.push(delete sample["-0"]);
            values.push(delete sample["1"]);
            $262.detachArrayBuffer(sample.buffer);
            values.push(delete sample[0]);
            values.join(",");
        "#),
        Value::String(Arc::from("false,true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(0);
            Object.defineProperty(sample, "+1", { value: 1, configurable: false });
            [delete sample["+1"], Object.getOwnPropertyDescriptor(sample, "+1").value].join(",");
        "#),
        Value::String(Arc::from("false,1"))
    );
    assert!(
        run_err(
            r#"
            "use strict";
            var sample = new Uint8Array(1);
            delete sample[0];
            "#
        )
        .contains("TypeError"),
        "strict delete of a valid TypedArray integer index should throw"
    );
}

#[test]
fn typed_array_get_own_property_descriptor_synthesizes_integer_indices() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([42, 43]);
            var d0 = Object.getOwnPropertyDescriptor(sample, "0");
            var d1 = Object.getOwnPropertyDescriptor(sample, "1");
            [
              d0.value, d0.writable, d0.enumerable, d0.configurable,
              d1.value, d1.writable, d1.enumerable, d1.configurable
            ].join(",");
        "#),
        Value::String(Arc::from("42,true,true,true,43,true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(2);
            sample[0] = 42n;
            sample[1] = 43n;
            var d0 = Object.getOwnPropertyDescriptor(sample, "0");
            var d1 = Object.getOwnPropertyDescriptor(sample, "1");
            [
              d0.value === 42n, d0.writable, d0.enumerable, d0.configurable,
              d1.value === 43n, d1.writable, d1.enumerable, d1.configurable
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,true,true,true,true,true"))
    );
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(8);
            var sample = new Uint16Array(buffer, 2, 2);
            sample[0] = 0x1234;
            sample[1] = 0x5678;
            var d0 = Object.getOwnPropertyDescriptor(sample, "0");
            var d1 = Object.getOwnPropertyDescriptor(sample, "1");
            [d0.value, d1.value].join(",");
        "#),
        Value::String(Arc::from("4660,22136"))
    );
}

#[test]
fn typed_array_get_own_property_descriptor_rejects_invalid_integer_indices() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(2);
            Object.defineProperty(sample, "+1", { value: "ordinary", configurable: true });
            [
              Object.getOwnPropertyDescriptor(sample, "-0") === undefined,
              Object.getOwnPropertyDescriptor(sample, "1.1") === undefined,
              Object.getOwnPropertyDescriptor(sample, "2") === undefined,
              Object.getOwnPropertyDescriptor(sample, "+1").value
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,ordinary"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7]);
            $262.detachArrayBuffer(sample.buffer);
            Object.getOwnPropertyDescriptor(sample, "0") === undefined;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_integer_index_descriptors_feed_proxy_invariants() {
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([1]);
            Object.preventExtensions(sample);
            0 in new Proxy(sample, { has: function() { return false; } });
            "#
        )
        .contains("TypeError"),
        "Proxy has must not hide a non-extensible TypedArray integer index"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([1]);
            Object.preventExtensions(sample);
            Reflect.deleteProperty(new Proxy(sample, { deleteProperty: function() { return true; } }), "0");
            "#
        )
        .contains("TypeError"),
        "Proxy deleteProperty must not delete a non-extensible TypedArray integer index"
    );
}

#[test]
fn typed_array_has_property_canonical_numeric_indices_follow_integer_indexed_exotic() {
    assert_eq!(
        run(r#"
            var TypedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
            Object.defineProperty(TypedArrayPrototype, "2", {
              value: "inherited",
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "-1", {
              value: "inherited",
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "1.1", {
              value: "inherited",
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "+1", {
              value: "ordinary",
              configurable: true
            });
            try {
              var sample = new Uint8Array([7, 8]);
              [
                Reflect.has(sample, "0"),
                Reflect.has(sample, "1"),
                Reflect.has(sample, "2"),
                Reflect.has(sample, "-1"),
                Reflect.has(sample, "1.1"),
                Reflect.has(sample, "-0"),
                Reflect.has(sample, "+1")
              ].join(",");
            } finally {
              delete TypedArrayPrototype["2"];
              delete TypedArrayPrototype["-1"];
              delete TypedArrayPrototype["1.1"];
              delete TypedArrayPrototype["+1"];
            }
        "#),
        Value::String(Arc::from("true,true,false,false,false,false,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7]);
            $262.detachArrayBuffer(sample.buffer);
            [Reflect.has(sample, "0"), Reflect.has(sample, "1")].join(",");
        "#),
        Value::String(Arc::from("false,false"))
    );
}

#[test]
fn typed_array_has_property_ordinary_keys_delegate_to_proxy_prototype() {
    assert_eq!(
        run(r#"
            var hits = 0;
            var proxy = new Proxy(Object.getPrototypeOf(Uint8Array.prototype), {
              has: function(target, key) {
                hits++;
                if (key === "foo") throw new Error("has trap");
                return Reflect.has(target, key);
              }
            });
            var sample = new Uint8Array([7]);
            Object.setPrototypeOf(sample, proxy);
            var numeric = [Reflect.has(sample, "0"), Reflect.has(sample, "1")].join(",");
            var threw = false;
            try { Reflect.has(sample, "foo"); } catch (e) { threw = e.message === "has trap"; }
            numeric + ":" + threw + ":" + hits;
        "#),
        Value::String(Arc::from("true,false:true:1"))
    );
}

#[test]
fn typed_array_subarray_creates_shared_offset_views() {
    assert_eq!(
        run(r#"
            var TypedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
            var source = new Uint8Array([10, 20, 30, 40]);
            var view = source.subarray(1, -1);
            view[0] = 99;
            [
              typeof TypedArrayPrototype.subarray,
              Uint8Array.prototype.hasOwnProperty("subarray"),
              view instanceof Uint8Array,
              view.buffer === source.buffer,
              view.byteOffset,
              view.byteLength,
              view.length,
              view[0],
              view[1],
              source[1]
            ].join(",");
        "#),
        Value::String(Arc::from("function,false,true,true,1,2,2,99,30,99"))
    );
    assert_eq!(
        run(r#"
            var source = new BigInt64Array(3);
            source[0] = 1n;
            source[1] = 2n;
            source[2] = 3n;
            var view = source.subarray(-2);
            view[0] = 9n;
            [view.length, view[0], view[1], source[1]].join(",");
        "#),
        Value::String(Arc::from("2,9,3,9"))
    );
}

#[test]
fn typed_array_subarray_uses_species_and_rejects_detached_buffers() {
    assert_eq!(
        run(r#"
            var source = new Uint8Array([10, 20, 30, 40]);
            var calls = [];
            function Species(buffer, offset, length) {
              calls.push(buffer === source.buffer, offset, length);
              return new Uint8Array(buffer, offset, length);
            }
            var holder = {};
            holder[Symbol.species] = Species;
            source.constructor = holder;
            var view = source.subarray(1, 3);
            [
              calls.join(":"),
              view.buffer === source.buffer,
              view.byteOffset,
              view.length,
              view[0],
              view[1]
            ].join(",");
        "#),
        Value::String(Arc::from("true:1:2,true,1,2,20,30"))
    );
    assert!(
        run_err(
            r#"
                var source = new Uint8Array([1]);
                $262.detachArrayBuffer(source.buffer);
                source.subarray(0);
            "#
        )
        .contains("TypeError"),
        "TypedArray.prototype.subarray should reject detached buffers"
    );
}

#[test]
fn typed_array_own_keys_include_integer_indices_first() {
    assert_eq!(
        run(r#"
            var sym = Symbol("s");
            var sample = new Uint8Array([7, 8, 9]);
            sample.extra = 10;
            sample[sym] = 11;
            Reflect.ownKeys(sample).map(function(key) {
              return typeof key === "symbol" ? "symbol:" + key.description : key;
            }).join(",");
        "#),
        Value::String(Arc::from("0,1,2,extra,symbol:s"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7, 8, 9, 10]).subarray(2);
            sample.extra = 11;
            Reflect.ownKeys(sample).join(",");
        "#),
        Value::String(Arc::from("0,1,extra"))
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(2);
            sample.extra = 1;
            Reflect.ownKeys(sample).join(",");
        "#),
        Value::String(Arc::from("0,1,extra"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7, 8]);
            sample.extra = 9;
            $262.detachArrayBuffer(sample.buffer);
            Reflect.ownKeys(sample).join(",");
        "#),
        Value::String(Arc::from("extra"))
    );
}

#[test]
fn typed_array_reflect_set_uses_receiver_for_valid_indices() {
    assert_eq!(
        run(r#"
            var valueOfCalls = 0;
            var value = { valueOf: function() { valueOfCalls++; return 2.3; } };
            var target = new Float64Array([0]);
            var receiver = {};
            var ok = Reflect.set(target, "0", value, receiver);
            [ok, target[0], receiver[0] === value, valueOfCalls].join(",");
        "#),
        Value::String(Arc::from("true,0,true,0"))
    );
    assert_eq!(
        run(r#"
            var target = new Float64Array([0]);
            var receiver = new Float64Array([1]);
            var ok = Reflect.set(target, "0", new Number(2.3), receiver);
            [ok, target[0], receiver[0]].join(",");
        "#),
        Value::String(Arc::from("true,0,2.3"))
    );
    assert_eq!(
        run(r#"
            var valueOfCalls = 0;
            var value = { valueOf: function() { valueOfCalls++; return 2.3; } };
            var target = new Float64Array([0, 0]);
            var receiver = new Float64Array([1]);
            var ok = Reflect.set(target, "1", value, receiver);
            [ok, target[1], Reflect.has(receiver, "1"), valueOfCalls].join(",");
        "#),
        Value::String(Arc::from("false,0,false,0"))
    );
    assert_eq!(
        run(r#"
            var target = new BigInt64Array([0n]);
            var receiver = new BigInt64Array([1n]);
            var ok = Reflect.set(target, "0", Object(2n), receiver);
            [ok, target[0], receiver[0]].join(",");
        "#),
        Value::String(Arc::from("true,0,2"))
    );
    assert_eq!(
        run(r#"
            var symbol = Symbol("slot");
            var sample = new Float64Array([42]);
            Reflect.set(sample, symbol, "first");
            Object.defineProperty(sample, symbol, {
              writable: false,
              value: "locked"
            });
            var ok = Reflect.set(sample, symbol, "second");
            [ok, sample[symbol]].join(",");
        "#),
        Value::String(Arc::from("false,locked"))
    );
    assert_eq!(
        run(r#"
            var symbol = Symbol("slot");
            var sample = new BigInt64Array([42n]);
            Reflect.set(sample, symbol, "first");
            Object.defineProperty(sample, symbol, {
              writable: false,
              value: "locked"
            });
            var ok = Reflect.set(sample, symbol, "second");
            [ok, sample[symbol]].join(",");
        "#),
        Value::String(Arc::from("false,locked"))
    );
}

#[test]
fn typed_array_get_canonical_numeric_indices_follow_integer_indexed_exotic() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([42, 43]);
            [sample["0"], sample["1"], sample[0], sample[-0]].join(",");
        "#),
        Value::String(Arc::from("42,43,42,42"))
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(2);
            sample[0] = 42n;
            sample[1] = 43n;
            [sample["0"] === 42n, sample["1"] === 43n].join(",");
        "#),
        Value::String(Arc::from("true,true"))
    );
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(8);
            var sample = new Uint16Array(buffer, 2, 2);
            sample[0] = 0x1234;
            sample[1] = 0x5678;
            [sample["0"], sample["1"]].join(",");
        "#),
        Value::String(Arc::from("4660,22136"))
    );
}

#[test]
fn typed_array_get_invalid_canonical_indices_skip_ordinary_lookup() {
    assert_eq!(
        run(r#"
            var TypedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
            Object.defineProperty(TypedArrayPrototype, "1.1", {
              get: function() { throw new Error("ordinary get"); },
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "-0", {
              get: function() { throw new Error("ordinary get"); },
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "2", {
              get: function() { throw new Error("ordinary get"); },
              configurable: true
            });
            try {
              var sample = new Uint8Array([7, 8]);
              [
                sample["1.1"] === undefined,
                sample["-0"] === undefined,
                sample["2"] === undefined
              ].join(",");
            } finally {
              delete TypedArrayPrototype["1.1"];
              delete TypedArrayPrototype["-0"];
              delete TypedArrayPrototype["2"];
            }
        "#),
        Value::String(Arc::from("true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7]);
            $262.detachArrayBuffer(sample.buffer);
            sample["0"] === undefined;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_get_noncanonical_numeric_keys_use_ordinary_lookup() {
    assert_eq!(
        run(r#"
            var TypedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
            try {
              var sample = new Uint8Array();
              TypedArrayPrototype["+1"] = "inherited";
              var inherited = sample["+1"];
              sample["+1"] = "own";
              var own = sample["+1"];
              Object.defineProperty(sample, "+1", {
                get: function() { return "accessor"; },
                configurable: true
              });
              [inherited, own, sample["+1"]].join(",");
            } finally {
              delete TypedArrayPrototype["+1"];
            }
        "#),
        Value::String(Arc::from("inherited,own,accessor"))
    );
}

#[test]
fn typed_array_define_own_property_numeric_indices_validate_descriptors() {
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { configurable: false });
            "#
        )
        .contains("TypeError"),
        "TypedArray numeric index descriptors cannot set configurable false"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { enumerable: false });
            "#
        )
        .contains("TypeError"),
        "TypedArray numeric index descriptors cannot set enumerable false"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { writable: false });
            "#
        )
        .contains("TypeError"),
        "TypedArray numeric index descriptors cannot set writable false"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { get: function() { return 1; } });
            "#
        )
        .contains("TypeError"),
        "TypedArray numeric index descriptors cannot be accessors"
    );
}

#[test]
fn typed_array_define_own_property_numeric_indices_write_elements() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { value: 260 });
            sample[0];
        "#),
        Value::Number(4.0)
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(1);
            Object.defineProperty(sample, "0", { value: 42n });
            sample[0] === 42n;
        "#),
        Value::Bool(true)
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", {
              value: { valueOf: function() { throw new Error("boom"); } }
            });
            "#
        )
        .contains("boom"),
        "TypedArray numeric index define must propagate value conversion errors"
    );
}

#[test]
fn typed_array_define_own_property_rejects_invalid_canonical_indices() {
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            var desc = Object.getOwnPropertyDescriptor(sample, "0");
            Object.defineProperty(sample, "1", desc);
            "#
        )
        .contains("TypeError"),
        "out-of-bounds canonical numeric index define should fail"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            var desc = Object.getOwnPropertyDescriptor(sample, "0");
            Object.defineProperty(sample, "-0", desc);
            "#
        )
        .contains("TypeError"),
        "-0 canonical numeric index define should fail"
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([0]);
            var desc = Object.getOwnPropertyDescriptor(sample, "0");
            Reflect.defineProperty(sample, "1.5", desc);
        "#),
        Value::Bool(false)
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            var desc = Object.getOwnPropertyDescriptor(sample, "0");
            $262.detachArrayBuffer(sample.buffer);
            Object.defineProperty(sample, "0", desc);
            "#
        )
        .contains("TypeError"),
        "detached TypedArray numeric index define should fail"
    );
}

#[test]
fn typed_array_define_own_property_noncanonical_keys_are_ordinary() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(0);
            Object.defineProperty(sample, "+1", {
              value: "ordinary",
              configurable: true
            });
            Object.getOwnPropertyDescriptor(sample, "+1").value;
        "#),
        Value::String(Arc::from("ordinary"))
    );
}

#[test]
fn data_view_constructor_length_descriptor() {
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(DataView, "length");
            [desc.value, desc.writable, desc.enumerable, desc.configurable].join(",");
        "#),
        Value::String(Arc::from("1,false,false,true"))
    );
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(DataView.prototype, Symbol.toStringTag);
            [desc.value, desc.writable, desc.enumerable, desc.configurable].join(",");
        "#),
        Value::String(Arc::from("DataView,false,false,true"))
    );
}

#[test]
fn data_view_constructor_validates_before_new_target_prototype() {
    assert_eq!(
        run(r#"
            var newTarget = Object.defineProperty(function(){}.bind(), "prototype", {
              get: function() { throw new Error("prototype"); }
            });
            var log = [];
            try {
              Reflect.construct(DataView, [new ArrayBuffer(0), 10], newTarget);
              log.push("none");
            } catch (e) {
              log.push(e.name + ":" + /prototype/.test(e.message));
            }
            try {
              Reflect.construct(DataView, [new ArrayBuffer(0), 0], newTarget);
              log.push("none");
            } catch (e) {
              log.push(e.name + ":" + /prototype/.test(e.message));
            }
            var buffer = new ArrayBuffer(8);
            var detachingNewTarget = Object.defineProperty(function(){}.bind(), "prototype", {
              get: function() { $262.detachArrayBuffer(buffer); return DataView.prototype; }
            });
            try {
              Reflect.construct(DataView, [buffer, { valueOf: function() { log.push("offset"); return 0; } }], detachingNewTarget);
              log.push("none");
            } catch (e) {
              log.push(e.name);
            }
            log.join("|");
        "#),
        Value::String(Arc::from("RangeError:false|Error:true|offset|TypeError"))
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
    assert_eq!(
        run(r#"
            var calls = 0;
            var length = { valueOf: function() { calls++; return 1; } };
            var threw = false;
            try {
              ArrayBuffer(length);
            } catch (e) {
              threw = e instanceof TypeError;
            }
            [threw, calls].join(",");
            "#),
        Value::String(Arc::from("true,0"))
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var offset = { valueOf: function() { calls++; return 0; } };
            var threw = false;
            try {
              DataView(new ArrayBuffer(1), offset);
            } catch (e) {
              threw = e instanceof TypeError;
            }
            [threw, calls].join(",");
            "#),
        Value::String(Arc::from("true,0"))
    );
    assert_eq!(
        run(r#"
            var ab = new ArrayBuffer(1);
            $262.detachArrayBuffer(ab);
            var calls = 0;
            var offset = { valueOf: function() { calls++; return 0; } };
            var threw = false;
            try {
              new DataView(ab, offset);
            } catch (e) {
              threw = e instanceof TypeError;
            }
            [threw, calls].join(",");
            "#),
        Value::String(Arc::from("true,1"))
    );
}

#[test]
fn array_buffer_static_surface_matches_intrinsics() {
    assert_eq!(
        run(r#"
            var isViewDesc = Object.getOwnPropertyDescriptor(ArrayBuffer, "isView");
            var speciesDesc = Object.getOwnPropertyDescriptor(ArrayBuffer, Symbol.species);
            var tagDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, Symbol.toStringTag);
            var ab = new ArrayBuffer(4);
            var ta = new Uint8Array(ab);
            var dv = new DataView(ab);
            var receiver = {};
            [
              isViewDesc.value.length,
              isViewDesc.value.name,
              isViewDesc.writable,
              isViewDesc.enumerable,
              isViewDesc.configurable,
              ArrayBuffer.isView(ta),
              ArrayBuffer.isView(dv),
              ArrayBuffer.isView(ab),
              ArrayBuffer.isView({}),
              ArrayBuffer.isView(undefined),
              speciesDesc.get.length,
              speciesDesc.get.name,
              speciesDesc.set === undefined,
              speciesDesc.enumerable,
              speciesDesc.configurable,
              speciesDesc.get.call(receiver) === receiver,
              tagDesc.value,
              tagDesc.writable,
              tagDesc.enumerable,
              tagDesc.configurable,
              (function() {
                function F() {}
                F.prototype = null;
                return Object.getPrototypeOf(Reflect.construct(ArrayBuffer, [0], F)) === ArrayBuffer.prototype;
              })(),
              (function() {
                function F() {}
                F.prototype = Array.prototype;
                return Object.getPrototypeOf(Reflect.construct(ArrayBuffer, [0], F)) === Array.prototype;
              })()
            ].join(",");
            "#),
        Value::String(Arc::from(
            "1,isView,true,false,true,true,true,false,false,false,0,get [Symbol.species],true,false,true,true,ArrayBuffer,false,false,true,true,true",
        ))
    );
}

#[test]
fn array_buffer_return_value_survives_frame_boundary_gc() {
    assert_eq!(
        run(r#"
            function make32ByteArrayBuffer() {
              var ab = new ArrayBuffer(32);
              var view = new Uint8Array(ab);
              for (var i = 0; i < 8; i++) view[i] = i + 1;
              return ab;
            }

            var failed = 0;
            for (var n = 0; n < 3000; n++) {
              var source = make32ByteArrayBuffer();
              if (Object.getPrototypeOf(source) !== ArrayBuffer.prototype ||
                  source.byteLength !== 32) {
                failed = n + 1;
                break;
              }
              var start = { valueOf: function() { return "+9"; } };
              var end = { valueOf: function() { return "0o20"; } };
              var dest = source.sliceToImmutable(start, end);
              if (dest.byteLength !== 7 || dest.immutable !== true) {
                failed = -(n + 1);
                break;
              }
            }
            failed;
        "#),
        Value::Number(0.0)
    );
}

#[test]
fn array_buffer_slice_uses_species_constructor_and_validates_result() {
    assert_eq!(
        run(r#"
            var source = new ArrayBuffer(8);
            var sourceBytes = new Uint8Array(source);
            sourceBytes[0] = 7;
            sourceBytes[1] = 9;
            var calls = [];
            var resultBuffer;
            var speciesConstructor = {};
            speciesConstructor[Symbol.species] = function(length) {
              calls.push("species:" + length);
              resultBuffer = new ArrayBuffer(10);
              new Uint8Array(resultBuffer)[0] = 99;
              return resultBuffer;
            };
            source.constructor = speciesConstructor;
            var result = source.slice(0, 2);
            [
              result === resultBuffer,
              result.byteLength,
              new Uint8Array(result)[0],
              new Uint8Array(result)[1],
              new Uint8Array(result)[2],
              calls.join("|")
            ].join(",");
            "#),
        Value::String(Arc::from("true,10,7,9,0,species:2"))
    );
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try { fn(); } catch (e) { return e instanceof TypeError; }
              return false;
            }
            var ab = new ArrayBuffer(8);
            var speciesConstructor = {};
            [
              (function() {
                ab.constructor = undefined;
                return Object.getPrototypeOf(ab.slice()) === ArrayBuffer.prototype;
              })(),
              (function() {
                ab.constructor = speciesConstructor;
                speciesConstructor[Symbol.species] = null;
                return Object.getPrototypeOf(ab.slice()) === ArrayBuffer.prototype;
              })(),
              throwsTypeError(function() { ab.constructor = null; ab.slice(); }),
              throwsTypeError(function() {
                ab.constructor = speciesConstructor;
                speciesConstructor[Symbol.species] = function() { return {}; };
                ab.slice();
              }),
              throwsTypeError(function() {
                ab.constructor = speciesConstructor;
                speciesConstructor[Symbol.species] = function() { return new ArrayBuffer(4); };
                ab.slice();
              }),
              throwsTypeError(function() {
                ab.constructor = speciesConstructor;
                speciesConstructor[Symbol.species] = function() { return ab; };
                ab.slice();
              })
            ].join(",");
            "#),
        Value::String(Arc::from("true,true,true,true,true,true"))
    );
}

#[test]
fn array_buffer_transfer_methods_copy_resize_and_detach_source() {
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try { fn(); } catch (e) { return e instanceof TypeError; }
              return false;
            }
            var source = new ArrayBuffer(4);
            var bytes = new Uint8Array(source);
            bytes[0] = 1;
            bytes[1] = 2;
            bytes[2] = 3;
            bytes[3] = 4;
            var grown = source.transfer(6);
            var grownBytes = new Uint8Array(grown);
            var grownSnapshot = [
              grown.byteLength,
              grownBytes[0],
              grownBytes[1],
              grownBytes[2],
              grownBytes[3],
              grownBytes[4],
              grownBytes[5]
            ];
            var fixed = grown.transferToFixedLength(2);
            var fixedBytes = new Uint8Array(fixed);
            grownSnapshot.concat([
              source.byteLength,
              throwsTypeError(function() { source.slice(); }),
              fixed.byteLength,
              fixedBytes[0],
              fixedBytes[1],
              grown.byteLength
            ]).join(",");
            "#),
        Value::String(Arc::from("6,1,2,3,4,0,0,0,true,2,1,2,0"))
    );
    assert!(
        run_err(
            r#"
            var ab = new ArrayBuffer(1);
            ab.transfer(-1);
            "#
        )
        .contains("RangeError"),
        "negative transfer length should throw RangeError"
    );
}

#[test]
fn resizable_array_buffer_exposes_slots_resizes_and_preserves_transfer_mode() {
    assert_eq!(
        run(r#"
            var fixed = new ArrayBuffer(3);
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var bytes = new Uint8Array(rab);
            bytes[0] = 1;
            bytes[3] = 4;
            rab.resize(2);
            rab.resize(6);
            var resized = new Uint8Array(rab);
            var resizedSnapshot = [resized[0], resized[1], resized[2], resized[5]];
            var transferred = rab.transfer(7);
            var transferSnapshot = [
                transferred.resizable,
                transferred.maxByteLength
            ];
            var fixedTransfer = transferred.transferToFixedLength(5);
            var detached = new ArrayBuffer(1, { maxByteLength: 2 });
            $262.detachArrayBuffer(detached);
            var resize = Object.getOwnPropertyDescriptor(
                ArrayBuffer.prototype,
                "resize"
            );
            [
                fixed.resizable, fixed.maxByteLength,
                resizedSnapshot[0], resizedSnapshot[1],
                resizedSnapshot[2], resizedSnapshot[3],
                transferSnapshot[0], transferSnapshot[1],
                fixedTransfer.resizable, fixedTransfer.maxByteLength,
                detached.resizable, detached.maxByteLength,
                resize.value.name, resize.value.length,
                resize.writable, resize.enumerable, resize.configurable
            ].join("|");
            "#,),
        Value::String(Arc::from(
            "false|3|1|0|0|0|true|8|false|5|true|0|resize|1|true|false|true"
        ))
    );
}

#[test]
fn resizable_array_buffer_rechecks_detachment_after_length_coercion() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var called = false;
            var detachedError = false;
            try {
                rab.resize({ valueOf: function() {
                    called = true;
                    $262.detachArrayBuffer(rab);
                    return 2;
                }});
            } catch (error) { detachedError = error instanceof TypeError; }
            var fixedError = false;
            try { new ArrayBuffer(1).resize(0); }
            catch (error) { fixedError = error instanceof TypeError; }
            [called, detachedError, fixedError].join("|");
            "#,),
        Value::String(Arc::from("true|true|true"))
    );
}

#[test]
fn array_buffer_transfer_to_immutable_and_slice_to_immutable_mark_results() {
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try { fn(); } catch (e) { return e instanceof TypeError; }
              return false;
            }
            var source = new ArrayBuffer(4);
            var bytes = new Uint8Array(source);
            bytes[0] = 11;
            bytes[1] = 12;
            bytes[2] = 13;
            bytes[3] = 14;
            var sliced = source.sliceToImmutable(1, 3);
            var slicedBytes = new Uint8Array(sliced);
            bytes[1] = 99;
            var moved = source.transferToImmutable();
            var movedBytes = new Uint8Array(moved);
            [
              sliced.immutable,
              sliced.byteLength,
              slicedBytes[0],
              slicedBytes[1],
              moved.immutable,
              moved.byteLength,
              movedBytes[0],
              movedBytes[1],
              movedBytes[2],
              movedBytes[3],
              source.byteLength,
              throwsTypeError(function() { moved.transfer(); }),
              throwsTypeError(function() { source.transferToImmutable(); })
            ].join(",");
            "#),
        Value::String(Arc::from("true,2,12,13,true,4,11,99,13,14,0,true,true",))
    );
}

#[test]
fn array_buffer_immutable_surface_and_transfer_validation_order() {
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try { fn(); } catch (e) { return e instanceof TypeError; }
              return false;
            }
            var ab = new ArrayBuffer(2);
            var immutableDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "immutable");
            var detachedDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "detached");
            var transferDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "transfer");
            var fixedDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "transferToFixedLength");
            var immutableTransferDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "transferToImmutable");
            var sliceImmutableDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "sliceToImmutable");
            var order = [];
            var detached = ab.transfer();
            try {
              ab.transfer({ valueOf: function() { order.push("coerce"); return 0; } });
            } catch (e) {
              order.push(e instanceof TypeError);
            }
            [
              immutableDesc.get.name,
              immutableDesc.get.length,
              immutableDesc.set === undefined,
              immutableDesc.enumerable,
              immutableDesc.configurable,
              immutableDesc.get.call(new ArrayBuffer(1)),
              immutableDesc.get.call(detached),
              detachedDesc.get.name,
              detachedDesc.get.length,
              detachedDesc.set === undefined,
              detachedDesc.enumerable,
              detachedDesc.configurable,
              detachedDesc.get.call(new ArrayBuffer(1)),
              detachedDesc.get.call(ab),
              throwsTypeError(function() { immutableDesc.get.call({}); }),
              throwsTypeError(function() { detachedDesc.get.call({}); }),
              transferDesc.value.length,
              fixedDesc.value.length,
              immutableTransferDesc.value.length,
              sliceImmutableDesc.value.length,
              order.join("|")
            ].join(",");
            "#),
        Value::String(Arc::from(
            "get immutable,0,true,false,true,false,false,get detached,0,true,false,true,false,true,true,true,0,0,0,2,coerce|true",
        ))
    );
}

#[test]
fn array_buffer_immutable_argument_helpers_match_array_like_coercions() {
    assert_eq!(
        run(r#"
            var ws = "\t\v\f\uFEFF\u3000\n\r\u2028\u2029";
            var ab = new ArrayBuffer(8);
            var moved = ab.transferToImmutable(ws + "1" + ws);
            var source = new ArrayBuffer(4);
            var bytes = new Uint8Array(source);
            bytes[0] = 5;
            bytes[1] = 6;
            bytes[2] = 7;
            bytes[3] = 8;
            var immutable = source.sliceToImmutable(0, null);
            var array = Array.from(new Uint8Array(source));
            [
              moved.byteLength,
              moved.immutable,
              immutable.byteLength,
              Array.from(new Uint8Array(immutable)).length,
              array.length,
              array[0],
              array[3],
              [1, 2, 3].slice(0, null).length
            ].join(",");
            "#),
        Value::String(Arc::from("1,true,0,0,4,5,8,0"))
    );
}

#[test]
fn data_view_setters_reject_immutable_backing_buffer_before_argument_coercion() {
    assert_eq!(
        run(r#"
            var buffer = (new ArrayBuffer(16)).transferToImmutable();
            var view = new DataView(buffer);
            var calls = [];
            var offset = { valueOf: function() { calls.push("offset"); return 0; } };
            var numberValue = { valueOf: function() { calls.push("number"); return 1; } };
            var bigintValue = { valueOf: function() { calls.push("bigint"); return 1n; } };
            var names = [
              "setInt8", "setUint8", "setInt16", "setUint16", "setInt32",
              "setUint32", "setFloat32", "setFloat64", "setBigInt64",
              "setBigUint64"
            ];
            var ok = [];
            for (var i = 0; i < names.length; i++) {
              try {
                view[names[i]](offset, names[i].startsWith("setBig") ? bigintValue : numberValue, true);
                ok.push(false);
              } catch (e) {
                ok.push(e instanceof TypeError);
              }
            }
            ok.join(",") + "|" + calls.join(",");
        "#),
        Value::String(Arc::from(
            "true,true,true,true,true,true,true,true,true,true|",
        ))
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
fn data_view_float16_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(6);
            var dv = new DataView(buffer);
            dv.setUint8(0, 66);
            dv.setUint8(1, 40);
            dv.setUint8(2, 40);
            dv.setUint8(3, 66);
            var values = [];
            values.push(dv.getFloat16(0));
            values.push(dv.getFloat16(0, true));
            values.push(dv.getFloat16(2));
            values.push(dv.getFloat16(2, true));
            values.push(dv.setFloat16(4, 42, true) === undefined);
            values.push(dv.getFloat16(4));
            values.push(dv.getFloat16(4, true));
            values.push(DataView.prototype.getFloat16.length);
            values.push(DataView.prototype.setFloat16.length);
            values.push(DataView.prototype.getFloat16.name);
            values.push(DataView.prototype.setFloat16.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "3.078125,0.03326416015625,0.03326416015625,3.078125,true,2.158203125,42,1,2,getFloat16,setFloat16"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(2));
            dv.setFloat16(0, -0);
            1 / dv.getFloat16(0);
            "#),
        Value::Number(f64::NEG_INFINITY)
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(10));
            var values = [];
            dv.setFloat16(0, 1.1);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 0.1);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 2.9802322387695312e-8);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 2.980232238769532e-8);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 8.940696716308594e-8);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 1.4901161193847656e-7);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 1.490116119384766e-7);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 2049);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 2051);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 65504);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 65520);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 65519.99999999999);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, NaN);
            values.push(dv.getFloat16(0) !== dv.getFloat16(0));
            values.join(",");
            "#),
        Value::String(Arc::from(
            "1.099609375,0.0999755859375,0,5.960464477539063e-8,1.1920928955078125e-7,1.1920928955078125e-7,1.7881393432617188e-7,2048,2052,65504,Infinity,65504,true"
        ))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(2));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setFloat16(-1, poisoned);
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
            dv.setFloat16(2, poisoned);
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
            var value = { valueOf: function() { $262.detachArrayBuffer(buffer); return 1; } };
            dv.setFloat16(0, value);
            "#
        )
        .contains("TypeError"),
        "detached buffers should be checked after Float16 value conversion"
    );
    assert_eq!(
        run(r#"
            var iab = (new ArrayBuffer(2)).transferToImmutable();
            var dv = new DataView(iab);
            var calls = [];
            var byteOffset = { valueOf: function() { calls.push("byteOffset"); return 0; } };
            var value = { valueOf: function() { calls.push("value"); return 1; } };
            try { dv.setFloat16(byteOffset, value); } catch (e) { calls.push(e instanceof TypeError); }
            calls.join(",");
            "#),
        Value::String(Arc::from("true"))
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
fn object_spread_copies_symbols_in_own_property_key_order() {
    assert_eq!(
        run(r#"
            var calls = [];
            var sym = Symbol("foo");
            var hidden = Symbol("hidden");
            var source = {
              get z() { calls.push("z"); return 2; },
              get a() { calls.push("a"); return 3; }
            };
            Object.defineProperty(source, "1", {
              enumerable: true,
              get: function() { calls.push("1"); return 1; }
            });
            Object.defineProperty(source, sym, {
              enumerable: true,
              get: function() { calls.push("s"); return 4; }
            });
            Object.defineProperty(source, hidden, {
              enumerable: false,
              value: 5
            });
            var out = { ...source };
            [
              calls.join(","),
              out[1],
              out.z,
              out.a,
              out[sym],
              out[hidden] === undefined,
              Object.keys(out).join(","),
              Object.getOwnPropertySymbols(out).length
            ].join("|");
        "#),
        Value::String(Arc::from("1,z,a,s|1|2|3|4|true|1,z,a|1"))
    );
}

#[test]
fn object_spread_rechecks_descriptors_and_propagates_proxy_own_keys() {
    assert_eq!(
        run(r#"
            var log = [];
            var sym = Symbol("s");
            var source = {};
            Object.defineProperty(source, "a", {
              enumerable: true,
              get: function() {
                log.push("a");
                Object.defineProperty(source, sym, { value: 2, enumerable: false });
                return 1;
              }
            });
            Object.defineProperty(source, sym, {
              value: 2,
              enumerable: true,
              configurable: true
            });
            var out = { ...source };
            [log.join(","), out.a, out[sym] === undefined, Object.getOwnPropertySymbols(out).length].join("|");
        "#),
        Value::String(Arc::from("a|1|true|0"))
    );

    assert!(run_err(
        r#"
            var proxy = new Proxy({ a: 1 }, {
              ownKeys: function() { throw new Error("boom"); }
            });
            ({ ...proxy });
        "#
    )
    .contains("boom"));
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
fn error_to_string_requires_object_and_omits_empty_parts() {
    assert_eq!(
        run(r#"
            var e1 = new Error("message");
            e1.name = "";
            var e2 = new Error("");
            var e3 = new Error("");
            e3.name = "";
            [
              e1.toString(),
              e2.toString(),
              e3.toString(),
              Error.prototype.toString.call({ name: undefined, message: "m" }),
              Error.prototype.toString.call({ name: "N", message: undefined })
            ].join("|");
        "#),
        Value::String(Arc::from("message|Error||Error: m|N"))
    );
    for src in [
        "Error.prototype.toString.call(undefined);",
        "Error.prototype.toString.call(null);",
        "Error.prototype.toString.call(1);",
        "Error.prototype.toString.call(true);",
        "Error.prototype.toString.call('x');",
        "Error.prototype.toString.call(Symbol());",
    ] {
        assert!(
            run_err(src).contains("TypeError"),
            "expected TypeError for {src}"
        );
    }
}

#[test]
fn error_cause_uses_has_property_get_and_message_order() {
    assert_eq!(
        run(r#"
            var cause = { message: "root" };
            var err = new Error("msg", { cause: cause });
            var desc = Object.getOwnPropertyDescriptor(err, "cause");
            [
              err.cause === cause,
              desc.value === cause,
              desc.writable,
              desc.enumerable,
              desc.configurable,
              Object.prototype.hasOwnProperty.call(new Error("msg"), "cause"),
              Object.prototype.hasOwnProperty.call(new Error("msg", { cause: undefined }), "cause")
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,false,true,false,true"))
    );

    assert_eq!(
        run(r#"
            var seq = [];
            new Error(
              { toString: function() { seq.push("toString"); return "msg"; } },
              { get cause() { seq.push("cause"); return 1; } }
            );
            seq.join(",");
        "#),
        Value::String(Arc::from("toString,cause"))
    );

    assert!(run_err(
        r#"
        new Error("msg", new Proxy({}, {
          has: function(target, key) {
            if (key === "cause") throw new Error("has boom");
            return key in target;
          }
        }));
    "#
    )
    .contains("has boom"));

    assert!(run_err(
        r#"
        new Error("msg", {
          get cause() { throw new Error("get boom"); }
        });
    "#
    )
    .contains("get boom"));

    assert_eq!(
        run(r#"
            var cause = { message: "root" };
            var err = new AggregateError([1, 2], "agg", { cause: cause });
            var causeDesc = Object.getOwnPropertyDescriptor(err, "cause");
            var errorsDesc = Object.getOwnPropertyDescriptor(err, "errors");
            [
              AggregateError.length,
              err.message,
              err.cause === cause,
              causeDesc.enumerable,
              errorsDesc.enumerable,
              err.errors.join(":")
            ].join(",");
        "#),
        Value::String(Arc::from("2,agg,true,false,false,1:2"))
    );
}

#[test]
fn error_stack_accessor_uses_error_data_and_receiver_property() {
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(Error.prototype, "stack");
            var err = new TypeError("msg");
            var fake = Object.create(Error.prototype);
            var other = $262.createRealm().global;
            var otherDesc = Object.getOwnPropertyDescriptor(other.Error.prototype, "stack");
            var otherOriginalTypeError = other.TypeError;
            desc.set.call(err, "sentinel");
            desc.set.call(fake, "plain");
            [
              typeof desc.get,
              desc.get.name,
              desc.get.length,
              typeof desc.set,
              desc.set.name,
              desc.set.length,
              desc.enumerable,
              desc.configurable,
              Object.prototype.hasOwnProperty.call(new Error("x"), "stack"),
              typeof desc.get.call(new Error("x")),
              desc.get.call({}) === undefined,
              err.stack,
              Object.getOwnPropertyDescriptor(err, "stack").enumerable,
              fake.stack,
              Error.prototype !== other.Error.prototype,
              desc.get !== otherDesc.get,
              desc.set !== otherDesc.set,
              Object.getPrototypeOf(other.TypeError.prototype) === other.Error.prototype,
              typeof desc.get.call(new other.Error("x")),
              (function() {
                try {
                  desc.set.call(other.Error.prototype, "x");
                } catch (e) {
                  return e.constructor === other.TypeError &&
                    Object.getPrototypeOf(e) === other.TypeError.prototype;
                }
                return false;
              })(),
              (function() {
                try {
                  otherDesc.get.call(1);
                } catch (e) {
                  return e.constructor === other.TypeError &&
                    Object.getPrototypeOf(e) === other.TypeError.prototype;
                }
                return false;
              })(),
              (function() {
                var original = TypeError;
                TypeError = function FakeTypeError() {};
                try {
                  desc.set.call(Error.prototype, "x");
                } catch (e) {
                  return e.constructor === original &&
                    Object.getPrototypeOf(e) === original.prototype;
                }
                return false;
              })(),
              (function() {
                other.TypeError = function FakeOtherTypeError() {};
                try {
                  otherDesc.set.call(other.Error.prototype, "x");
                } catch (e) {
                  return e.constructor === otherOriginalTypeError &&
                    Object.getPrototypeOf(e) === otherOriginalTypeError.prototype;
                }
                return false;
              })()
            ].join("|");
        "#),
        Value::String(Arc::from(
            "function|get stack|0|function|set stack|1|false|true|false|string|true|sentinel|true|plain|true|true|true|true|string|true|true|true|true"
        ))
    );
    for src in [
        "Object.getOwnPropertyDescriptor(Error.prototype, 'stack').get.call(1);",
        "Object.getOwnPropertyDescriptor(Error.prototype, 'stack').set.call(1, 'x');",
        "Object.getOwnPropertyDescriptor(Error.prototype, 'stack').set.call(new Error(), 1);",
        "Object.getOwnPropertyDescriptor(Error.prototype, 'stack').set.call(Error.prototype, 'x');",
    ] {
        assert!(
            run_err(src).contains("TypeError"),
            "expected TypeError for {src}"
        );
    }
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
fn error_constructors_use_new_target_realm_default_prototype() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var nt = new Function();
            nt.prototype = undefined;
            var otherNt = new other.Function();
            otherNt.prototype = undefined;
            [
              Object.getPrototypeOf(Reflect.construct(Error, [], nt)) === Error.prototype,
              Object.getPrototypeOf(Reflect.construct(TypeError, [], nt)) === TypeError.prototype,
              Object.getPrototypeOf(Reflect.construct(AggregateError, [[]], nt)) === AggregateError.prototype,
              Object.getPrototypeOf(Reflect.construct(Error, [], otherNt)) === other.Error.prototype,
              Object.getPrototypeOf(Reflect.construct(TypeError, [], otherNt)) === other.TypeError.prototype,
              Object.getPrototypeOf(Reflect.construct(AggregateError, [[]], otherNt)) === other.AggregateError.prototype
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,true,true,true"))
    );

    assert_eq!(
        run(r#"
            var proto = {};
            function NewTarget() {}
            NewTarget.prototype = proto;
            var err = Reflect.construct(AggregateError, [[]], NewTarget);
            Object.getPrototypeOf(err) === proto;
        "#),
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
fn error_is_error_recognizes_real_error_objects_only() {
    assert_eq!(
        run(r#"
            class CustomError extends Error {}
            var other = $262.createRealm().global;
            var fake = {
              __proto__: Error.prototype,
              constructor: Error,
              message: "",
              stack: new Error().stack
            };
            [
              Error.isError(new Error()),
              Error.isError(new TypeError()),
              Error.isError(new CustomError()),
              Error.isError(new other.Error()),
              Error.isError(new other.Array()),
              Error.isError(fake),
              Error.isError(Error),
              Error.isError({}),
              Error.isError(undefined),
              Error.isError(0n),
              Error.isError(Symbol()),
              Object.prototype.propertyIsEnumerable.call(Error, "isError"),
              Object.getOwnPropertyDescriptor(Error, "isError").writable,
              Object.getOwnPropertyDescriptor(Error, "isError").configurable
            ].join(",");
        "#),
        Value::String(Arc::from(
            "true,true,true,true,false,false,false,false,false,false,false,false,true,true"
        ))
    );
    assert!(
        run_err("new Error.isError();").contains("TypeError"),
        "Error.isError must not be constructable"
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
fn throw_type_error_intrinsic_is_frozen_and_anonymous() {
    assert_eq!(
        run(
            r#"var args = function() { "use strict"; return arguments; }();
               var thrower = Object.getOwnPropertyDescriptor(args, "callee").get;
               var length = Object.getOwnPropertyDescriptor(thrower, "length");
               var name = Object.getOwnPropertyDescriptor(thrower, "name");
               [
                 thrower.name,
                 thrower.length,
                 length.writable,
                 length.enumerable,
                 length.configurable,
                 name.value,
                 name.writable,
                 name.enumerable,
                 name.configurable,
                 Object.isExtensible(thrower),
                 Object.isFrozen(thrower)
               ].join("|");"#
        ),
        Value::String(Arc::from(
            "|0|false|false|false||false|false|false|false|true"
        ))
    );
}

#[test]
fn throw_type_error_intrinsic_is_reused_for_unmapped_arguments() {
    assert_eq!(
        run(
            r#"var strictArgs = function() { "use strict"; return arguments; }();
               function nonSimple(a = 0) { return arguments; }
               var strictCallee = Object.getOwnPropertyDescriptor(strictArgs, "callee");
               var nonSimpleCallee = Object.getOwnPropertyDescriptor(nonSimple(), "callee");
               (strictCallee.get === nonSimpleCallee.get) + ":" +
                 (strictCallee.get === nonSimpleCallee.set);"#
        ),
        Value::String(Arc::from("true:true"))
    );
}

#[test]
fn throw_type_error_intrinsic_matches_function_prototype_restricted_accessors() {
    assert_eq!(
        run(
            r#"var functionCaller = Object.getOwnPropertyDescriptor(Function.prototype, "caller");
               var functionArguments = Object.getOwnPropertyDescriptor(Function.prototype, "arguments");
               function outer() {
                 return function() { "use strict"; return arguments; }();
               }
               var thrower = Object.getOwnPropertyDescriptor(outer(), "callee").get;
               [
                 functionCaller.get === functionCaller.set,
                 functionArguments.get === functionArguments.set,
                 functionCaller.get === thrower,
                 functionArguments.get === thrower
               ].join(":");"#
        ),
        Value::String(Arc::from("true:true:true:true"))
    );
}

#[test]
fn throw_type_error_intrinsic_is_distinct_per_test262_realm() {
    assert_eq!(
        run(r#"var other = $262.createRealm().global;
               var localArgs = function() { "use strict"; return arguments; }();
               var otherArgs = (new other.Function('"use strict"; return arguments;'))();
               var otherArgs2 = (new other.Function('"use strict"; return arguments;'))();
               var localThrower = Object.getOwnPropertyDescriptor(localArgs, "callee").get;
               var otherThrower = Object.getOwnPropertyDescriptor(otherArgs, "callee").get;
               var otherThrower2 = Object.getOwnPropertyDescriptor(otherArgs2, "callee").get;
               (localThrower !== otherThrower) + ":" + (otherThrower === otherThrower2);"#),
        Value::String(Arc::from("true:true"))
    );
}

#[test]
fn throw_type_error_intrinsic_matches_cross_realm_function_prototype() {
    assert_eq!(
        run(r#"var other = $262.createRealm().global;
               var protoThrower = Object.getOwnPropertyDescriptor(other.Function.prototype, "caller").get;
               var argsThrower = new other.Function('return (function() { "use strict"; return Object.getOwnPropertyDescriptor(arguments, "callee").get })()')();
               var normalFunction = other.Function('return function nested() { return 1; }')();
               [
                 protoThrower === Object.getOwnPropertyDescriptor(other.Function.prototype, "arguments").set,
                 protoThrower === argsThrower,
                 Object.getPrototypeOf(normalFunction) === other.Function.prototype,
                 protoThrower !== Object.getOwnPropertyDescriptor(Function.prototype, "caller").get
               ].join(":");"#),
        Value::String(Arc::from("true:true:true:true"))
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
fn constructable_proxy_follows_target_and_construct_trap() {
    assert_eq!(
        run(r#"
            function Target(a, b) { this.sum = a + b; }
            var proxy = new Proxy(Target, {});
            new proxy(2, 3).sum;
            "#),
        Value::Number(5.0)
    );
    assert_eq!(
        run(r#"
            var C = $262.createRealm().global.eval(
              "new Proxy(function() {}, { construct: function(_, args) { return args; } })"
            );
            new C(1, 2).constructor === Array;
            "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            function Target() {}
            function NewTarget() {}
            NewTarget.prototype = { marker: true };
            var seen = [];
            var proxy = new Proxy(Target, {
              construct: function(t, args, nt) {
                seen.push(t === Target, args.constructor === Array, args.join(","), nt === NewTarget);
                return Reflect.construct(t, args, nt);
              }
            });
            var result = Reflect.construct(proxy, [4, 5], NewTarget);
            seen.join("|") + "|" + (Object.getPrototypeOf(result) === NewTarget.prototype);
            "#),
        Value::String(Arc::from("true|true|4,5|true|true"))
    );
    assert!(run_err(
        r#"
            var proxy = new Proxy(function() {}, { construct: function() { return 1; } });
            new proxy();
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
fn string_replace_all_test262_regressions() {
    assert_eq!(
        run(r#"'aba'.replaceAll('b', "$$-$&-$`-$'");"#),
        Value::String(Arc::from("a$-b-a-aa"))
    );
    assert_eq!(
        run(r#"'aaa'.replaceAll('a', function(m, pos, s) { return String(pos) + s.length; });"#),
        Value::String(Arc::from("031323"))
    );
    assert_eq!(
        run(r#"'abc abc abc'.replaceAll(/b/g, 'z');"#),
        Value::String(Arc::from("azc azc azc"))
    );
    assert_eq!(
        run(r#"'abcabcabcabc'.replaceAll(/a(b)(ca)/g, '$2-$1');"#),
        Value::String(Arc::from("ca-bbcca-bbc"))
    );
    assert_eq!(
        run(r#"(function(){
                 var re = /b/g;
                 var called = 0;
                 re[Symbol.replace] = function(O, replaceValue) {
                   called++;
                   return O + "|" + replaceValue;
                 };
                 return "abc".replaceAll(re, "z") + "|" + called;
               })();"#),
        Value::String(Arc::from("abc|z|1"))
    );
    assert_eq!(
        run(r#"(function(){
                 var re = /./iyg;
                 re[Symbol.replace] = undefined;
                 return 'aa /./giy /./iyg /./gyi /./giy aa'.replaceAll(re, 'z');
               })();"#),
        Value::String(Arc::from("aa z /./iyg /./gyi z aa"))
    );
    assert!(
        run_err(r#"'abc'.replaceAll(/b/, 'z');"#).contains("non-global RegExp"),
        "replaceAll must reject non-global RegExp search values"
    );
}

#[test]
fn string_normalize_follows_unicode_forms_and_descriptors() {
    assert_eq!(
        run(
            r#"var d = Object.getOwnPropertyDescriptor(String.prototype, "normalize");
               [
                 typeof String.prototype.normalize,
                 d.writable,
                 d.enumerable,
                 d.configurable,
                 String.prototype.normalize.length,
                 String.prototype.normalize.name
               ].join("|");"#
        ),
        Value::String(Arc::from("function|true|false|true|0|normalize"))
    );
    assert_eq!(
        run(r#"var s = "\u1E9B\u0323";
               [
                 s.normalize("NFC") === "\u1E9B\u0323",
                 s.normalize("NFD") === "\u017F\u0323\u0307",
                 s.normalize("NFKC") === "\u1E69",
                 s.normalize("NFKD") === "\u0073\u0323\u0307"
               ].join("|");"#),
        Value::String(Arc::from("true|true|true|true"))
    );
    assert_eq!(
        run(r#"var form = { toString: function() { return "NFD"; } };
               "\u00C5".normalize(form) === "A\u030A";"#),
        Value::Bool(true)
    );
    assert!(run_err(r#""x".normalize("bad");"#).contains("RangeError"));
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
               "abcbbc".match(r).join(",");"#),
        Value::String(Arc::from("b,b,b"))
    );
    assert_eq!(
        run(r#"var r = /b/g;
               var m = r[Symbol.match]("abcbbc");
               [m.join(","), r.lastIndex].join("|");"#),
        Value::String(Arc::from("b,b,b|0"))
    );
    assert_eq!(
        run(r#"var r = /b/;
               var m = r[Symbol.match]("abc");
               [m[0], m.index, m.input].join("|");"#),
        Value::String(Arc::from("b|1|abc"))
    );
    assert!(
        run_err(
            r#"var r = /./g;
               Object.defineProperty(r, "lastIndex", { writable: false });
               r[Symbol.match]("x");"#
        )
        .contains("lastIndex"),
        "RegExp @@match must surface lastIndex write failures"
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
        run(
            r#"var get = Object.getOwnPropertyDescriptor(RegExp.prototype, 'global').get;
               String(get.call(RegExp.prototype));"#
        ),
        Value::String(Arc::from("undefined"))
    );
    assert!(
        run_err(r#"Object.getOwnPropertyDescriptor(RegExp.prototype, 'global').get.call({});"#)
            .contains("RegExp getter"),
        "RegExp flag getters must reject ordinary objects without internal slots"
    );
    assert_eq!(
        run(r#"
            var get = Object.getOwnPropertyDescriptor(RegExp.prototype, 'global').get;
            var other = $262.createRealm().global;
            var otherRegExpProto = other.RegExp.prototype;
            var otherGet = Object.getOwnPropertyDescriptor(otherRegExpProto, 'global').get;
            var ok = [];
            try { get.call(otherRegExpProto); ok.push(false); }
            catch (e) { ok.push(e.constructor === TypeError); }
            try { otherGet.call(RegExp.prototype); ok.push(false); }
            catch (e) { ok.push(e.constructor === other.TypeError); }
            ok.join(',');
            "#),
        Value::String(Arc::from("true,true"))
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
fn regexp_non_unicode_surrogate_escapes_match_code_units() {
    assert_eq!(run("/\\udf06/.test('\\udf06');"), Value::Bool(true));
    assert_eq!(run("/\\udf06/i.test('\\udf06');"), Value::Bool(true));
    assert_eq!(run("/[\\udf06]/.test('\\udf06');"), Value::Bool(true));
    assert_eq!(run("/\\udf06/.test('\\ud834\\udf06');"), Value::Bool(true));
    assert_eq!(
        run("var r = /\\udf06/; Object.defineProperty(r, 'unicode', { value: true }); r[Symbol.match]('\\ud834\\udf06') !== null;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var r = /\\udf06/u; Object.defineProperty(r, 'unicode', { value: false }); r[Symbol.match]('\\ud834\\udf06') === null;"),
        Value::Bool(true)
    );
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
fn async_function_pending_await_resumes_after_fulfillment() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var resolveGate;
        var gate = new Promise(resolve => { resolveGate = resolve; });
        var log = [];
        async function pending() {
            log.push("start");
            let value = await gate;
            log.push("after:" + value);
            return value + 1;
        }
        var result = pending();
        result.then(value => log.push("done:" + value));
        log.push("sync");
        "#,
    )
    .expect("failed to start pending async function");

    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read pre-resolution log"),
        Value::String(Arc::from("start|sync"))
    );

    vm.run("resolveGate(41);")
        .expect("failed to fulfill pending await");
    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read resumed log"),
        Value::String(Arc::from("start|sync|after:41|done:42"))
    );
}

#[test]
fn async_function_pending_await_rejection_reenters_catch() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var rejectGate;
        var marker = {};
        var gate = new Promise((resolve, reject) => { rejectGate = reject; });
        var log = [];
        async function pending() {
            try {
                await gate;
                log.push("unreachable");
            } catch (error) {
                log.push("caught:" + (error === marker));
                return "recovered";
            }
        }
        var result = pending();
        result.then(value => log.push("done:" + value));
        log.push("sync");
        "#,
    )
    .expect("failed to start rejecting async function");

    vm.run("rejectGate(marker);")
        .expect("failed to reject pending await");
    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read rejection log"),
        Value::String(Arc::from("sync|caught:true|done:recovered"))
    );
}

#[test]
fn async_function_pending_await_preserves_finally_state() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var resolveGate;
        var gate = new Promise(resolve => { resolveGate = resolve; });
        var log = [];
        async function pending() {
            try {
                return await gate;
            } finally {
                log.push("finally");
            }
        }
        var result = pending();
        result.then(value => log.push("done:" + value));
        "#,
    )
    .expect("failed to suspend async function with finally");

    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read suspended finally state"),
        Value::String(Arc::from(""))
    );
    vm.run("resolveGate(42);")
        .expect("failed to resume async function with finally");
    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read resumed finally state"),
        Value::String(Arc::from("finally|done:42"))
    );
}

#[test]
fn async_function_await_observes_promise_job_order() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var log = [];
        Promise.resolve()
            .then(() => log.push("tick1"))
            .then(() => log.push("tick2"));
        async function ordered() {
            log.push("start");
            await 0;
            log.push("after");
        }
        ordered();
        log.push("sync");
        "#,
    )
    .expect("failed to run ordered await");

    assert_eq!(
        vm.run("log.join('|');").expect("failed to read job log"),
        Value::String(Arc::from("start|sync|tick1|after|tick2"))
    );
}

#[test]
fn async_function_pending_await_keeps_block_environment_alive_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var resolveGate;
        var gate = new Promise(resolve => { resolveGate = resolve; });
        var state = "initial";
        async function pending(argument) {
            let local = { value: 11 };
            {
                let held = { value: 12 };
                state = "waiting";
                let resumed = await gate;
                return argument.value + local.value + held.value + resumed.value;
            }
        }
        var settled = false;
        var result = pending({ value: 10 });
        result.then(() => { settled = true; });
        "#,
    )
    .expect("failed to suspend async function");

    assert_eq!(
        vm.run("state + '|' + settled;")
            .expect("failed to inspect suspended state"),
        Value::String(Arc::from("waiting|false"))
    );

    vm.gc();
    vm.run("resolveGate({ value: 9 });")
        .expect("failed to resume async function after GC");
    vm.run("var resumed; result.then(value => { resumed = value; });")
        .expect("failed to observe resumed value");

    assert_eq!(
        vm.run("resumed;").expect("failed to read resumed value"),
        Value::Number(42.0)
    );
}

#[test]
fn weak_ref_exposes_spec_shaped_constructor_and_deref() {
    assert_eq!(
        run(
            r#"
            var target = {};
            var ref = new WeakRef(target);
            var descriptor = Object.getOwnPropertyDescriptor(WeakRef, "prototype");
            var tag = Object.getOwnPropertyDescriptor(
                WeakRef.prototype,
                Symbol.toStringTag
            );
            [
                typeof WeakRef,
                WeakRef.length,
                WeakRef.name,
                ref.deref() === target,
                Object.getPrototypeOf(ref) === WeakRef.prototype,
                ref instanceof WeakRef,
                Object.isExtensible(ref),
                descriptor.writable,
                descriptor.enumerable,
                descriptor.configurable,
                WeakRef.prototype.deref.length,
                WeakRef.prototype.deref.name,
                tag.value,
                tag.writable,
                tag.enumerable,
                tag.configurable
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "function|1|WeakRef|true|true|true|true|false|false|false|0|deref|WeakRef|false|false|true"
        ))
    );

    assert_eq!(
        run(r#"
            var symbol = Symbol("target");
            var ref = new WeakRef(symbol);
            var failures = 0;
            for (var value of [undefined, null, 1, "x", true, Symbol.for("registered")]) {
                try { new WeakRef(value); } catch (error) {
                    if (error instanceof TypeError) failures++;
                }
            }
            [ref.deref() === symbol, failures].join("|");
            "#,),
        Value::String(Arc::from("true|6"))
    );
}

#[test]
fn weak_ref_validates_receivers_and_uses_new_target_realm() {
    assert_eq!(
        run(r#"
            var failures = 0;
            for (var value of [undefined, null, true, 1, "x", {}, WeakRef.prototype]) {
                try { WeakRef.prototype.deref.call(value); } catch (error) {
                    if (error instanceof TypeError) failures++;
                }
            }
            try { WeakRef({}); } catch (error) {
                if (error instanceof TypeError) failures++;
            }
            var other = $262.createRealm().global;
            var newTarget = new other.Function();
            newTarget.prototype = undefined;
            var ref = Reflect.construct(WeakRef, [{}], newTarget);
            [failures, Object.getPrototypeOf(ref) === other.WeakRef.prototype].join("|");
            "#,),
        Value::String(Arc::from("8|true"))
    );
}

#[test]
fn weak_ref_target_is_cleared_after_collection() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    assert_eq!(
        vm.run(
            r#"
            var target = { value: 42 };
            var ref = new WeakRef(target);
            var observed = ref.deref() === target;
            target = null;
            observed;
            "#,
        )
        .expect("failed to create WeakRef"),
        Value::Bool(true)
    );

    vm.gc();
    assert_eq!(
        vm.run("ref.deref() === undefined;")
            .expect("failed to dereference collected WeakRef target"),
        Value::Bool(true)
    );
}

#[test]
fn finalization_registry_exposes_spec_shaped_surface() {
    assert_eq!(
        run(
            r#"
            var registry = new FinalizationRegistry(function() {});
            var prototype = Object.getOwnPropertyDescriptor(
                FinalizationRegistry,
                "prototype"
            );
            var tag = Object.getOwnPropertyDescriptor(
                FinalizationRegistry.prototype,
                Symbol.toStringTag
            );
            [
                typeof FinalizationRegistry,
                FinalizationRegistry.length,
                FinalizationRegistry.name,
                registry instanceof FinalizationRegistry,
                Object.isExtensible(registry),
                Object.getPrototypeOf(registry) === FinalizationRegistry.prototype,
                prototype.writable,
                prototype.enumerable,
                prototype.configurable,
                FinalizationRegistry.prototype.register.length,
                FinalizationRegistry.prototype.register.name,
                FinalizationRegistry.prototype.unregister.length,
                FinalizationRegistry.prototype.unregister.name,
                tag.value,
                tag.writable,
                tag.enumerable,
                tag.configurable
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "function|1|FinalizationRegistry|true|true|true|false|false|false|2|register|1|unregister|FinalizationRegistry|false|false|true"
        ))
    );
}

#[test]
fn finalization_registry_validates_cells_tokens_and_realms() {
    assert_eq!(
        run(r#"
            var registry = new FinalizationRegistry(function() {});
            var target = {};
            var token = {};
            var symbolTarget = Symbol("target");
            var symbolToken = Symbol("token");
            var failures = 0;
            for (var value of [undefined, null, true, 1, "x", Symbol.for("registered")]) {
                try { registry.register(value); } catch (error) {
                    if (error instanceof TypeError) failures++;
                }
                try { registry.unregister(value); } catch (error) {
                    if (error instanceof TypeError) failures++;
                }
            }
            try { registry.register(target, target); } catch (error) {
                if (error instanceof TypeError) failures++;
            }
            try { FinalizationRegistry(function() {}); } catch (error) {
                if (error instanceof TypeError) failures++;
            }
            try { new FinalizationRegistry({}); } catch (error) {
                if (error instanceof TypeError) failures++;
            }
            var results = [
                registry.register(target, "held", token),
                registry.register(symbolTarget, 1, symbolToken),
                registry.unregister(token),
                registry.unregister(token),
                registry.unregister(symbolToken),
                registry.unregister(symbolToken)
            ];
            var other = $262.createRealm().global;
            var newTarget = new other.Function();
            newTarget.prototype = undefined;
            var crossRealm = Reflect.construct(
                FinalizationRegistry,
                [function() {}],
                newTarget
            );
            [
                failures,
                results.join(","),
                Object.getPrototypeOf(crossRealm) === other.FinalizationRegistry.prototype
            ].join("|");
            "#,),
        Value::String(Arc::from("15|,,true,false,true,false|true"))
    );
}

#[test]
fn finalization_registry_cleanup_runs_after_gc_and_unregister_suppresses_it() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    vm.run(
        r#"
        var cleaned = [];
        var registry = new FinalizationRegistry(function(held) {
            cleaned.push(held.value);
        });
        var first = { target: 1 };
        registry.register(first, { value: 42 });
        first = null;
        "#,
    )
    .expect("failed to register finalization target");

    vm.gc();
    vm.run_microtasks()
        .expect("failed to run finalization cleanup job");
    assert_eq!(
        vm.run("cleaned.join(',');")
            .expect("failed to inspect cleanup callback"),
        Value::String(Arc::from("42"))
    );

    vm.run(
        r#"
        var second = { target: 2 };
        var token = {};
        registry.register(second, { value: 99 }, token);
        var removed = registry.unregister(token);
        second = null;
        "#,
    )
    .expect("failed to unregister finalization target");
    vm.gc();
    vm.run_microtasks()
        .expect("failed to drain post-unregister jobs");
    assert_eq!(
        vm.run("removed + '|' + cleaned.join(',');")
            .expect("failed to inspect unregister behavior"),
        Value::String(Arc::from("true|42"))
    );

    vm.run(
        r#"
        var nestedCleaned = [];
        var nestedRegistry = new FinalizationRegistry(function(held) {
            if (nestedCleaned.length === 0) forceGc();
            nestedCleaned.push(held.value);
        });
        var nestedFirst = {};
        var nestedSecond = {};
        nestedRegistry.register(nestedFirst, { value: 1 });
        nestedRegistry.register(nestedSecond, { value: 2 });
        nestedFirst = null;
        nestedSecond = null;
        "#,
    )
    .expect("failed to register nested-GC cleanup targets");
    vm.gc();
    vm.run_microtasks()
        .expect("failed to run cleanup callback containing GC");
    assert_eq!(
        vm.run("nestedCleaned.join(',');")
            .expect("failed to inspect nested-GC holdings"),
        Value::String(Arc::from("1,2"))
    );

    vm.run(
        r#"
        var throwingRegistry = new FinalizationRegistry(function() {
            throw new Error("cleanup failure");
        });
        var third = {};
        throwingRegistry.register(third, "held");
        third = null;
        "#,
    )
    .expect("failed to register throwing cleanup callback");
    vm.gc();
    vm.run_microtasks()
        .expect("cleanup callback errors should be host-reported, not propagated");
}

#[test]
fn shared_array_buffer_surface_and_cross_realm_prototype_are_spec_shaped() {
    assert_eq!(
        run(
            r#"
            var buffer = new SharedArrayBuffer(8);
            var length = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                "byteLength"
            );
            var tag = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                Symbol.toStringTag
            );
            var other = $262.createRealm().global;
            var newTarget = new other.Function();
            newTarget.prototype = undefined;
            var crossRealm = Reflect.construct(SharedArrayBuffer, [4], newTarget);
            [
                typeof SharedArrayBuffer,
                SharedArrayBuffer.length,
                buffer.byteLength,
                Object.getPrototypeOf(buffer) === SharedArrayBuffer.prototype,
                Object.getPrototypeOf(SharedArrayBuffer) === Function.prototype,
                length.get.length,
                length.get.name,
                length.enumerable,
                length.configurable,
                tag.value,
                tag.writable,
                tag.enumerable,
                tag.configurable,
                Object.getPrototypeOf(crossRealm) === other.SharedArrayBuffer.prototype
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "function|1|8|true|true|0|get byteLength|false|true|SharedArrayBuffer|false|false|true|true"
        ))
    );
}

#[test]
fn shared_array_buffer_backs_typed_array_and_data_view_without_detachment() {
    assert_eq!(
        run(r#"
            var buffer = new SharedArrayBuffer(4);
            var bytes = new Uint8Array(buffer);
            var view = new DataView(buffer);
            bytes[0] = 7;
            view.setUint8(1, 9);
            var failures = 0;
            try {
                Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength")
                    .get.call(buffer);
            } catch (error) { if (error instanceof TypeError) failures++; }
            try {
                Object.getOwnPropertyDescriptor(SharedArrayBuffer.prototype, "byteLength")
                    .get.call(new ArrayBuffer(1));
            } catch (error) { if (error instanceof TypeError) failures++; }
            try { ArrayBuffer.prototype.transfer.call(buffer); }
            catch (error) { if (error instanceof TypeError) failures++; }
            [bytes[0], bytes[1], view.getUint8(0), failures].join("|");
            "#,),
        Value::String(Arc::from("7|9|7|3"))
    );
}

#[test]
fn shared_array_buffer_slice_uses_shared_species_and_copies_bytes() {
    assert_eq!(
        run(r#"
            var source = new SharedArrayBuffer(4);
            new Uint8Array(source)[1] = 42;
            var sliced = source.slice(1, 3);
            var values = new Uint8Array(sliced);
            var failures = 0;
            source.constructor = { [Symbol.species]: ArrayBuffer };
            try { source.slice(); }
            catch (error) { if (error instanceof TypeError) failures++; }
            try { SharedArrayBuffer.prototype.slice.call(new ArrayBuffer(1)); }
            catch (error) { if (error instanceof TypeError) failures++; }
            try { SharedArrayBuffer(1); }
            catch (error) { if (error instanceof TypeError) failures++; }
            [
                sliced instanceof SharedArrayBuffer,
                sliced.byteLength,
                values[0],
                values[1],
                failures
            ].join("|");
            "#,),
        Value::String(Arc::from("true|2|42|0|3"))
    );
}

#[test]
fn growable_shared_array_buffer_exposes_slots_and_grows_monotonically() {
    assert_eq!(
        run(
            r#"
            var fixed = new SharedArrayBuffer(3);
            var growable = new SharedArrayBuffer(2, { maxByteLength: 6 });
            var before = new Uint8Array(growable);
            before[0] = 11;
            before[1] = 22;
            var result = growable.grow(5);
            var after = new Uint8Array(growable);
            var grow = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                "grow"
            );
            var growableDesc = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                "growable"
            );
            var maxDesc = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                "maxByteLength"
            );
            [
                fixed.growable,
                fixed.maxByteLength,
                growable.growable,
                growable.maxByteLength,
                growable.byteLength,
                result === undefined,
                after[0], after[1], after[2], after[4],
                grow.value.name, grow.value.length,
                grow.writable, grow.enumerable, grow.configurable,
                growableDesc.get.name, growableDesc.get.length,
                maxDesc.get.name, maxDesc.get.length
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "false|3|true|6|5|true|11|22|0|0|grow|1|true|false|true|get growable|0|get maxByteLength|0"
        ))
    );
}

#[test]
fn growable_shared_array_buffer_validates_options_before_prototype_lookup() {
    assert_eq!(
        run(r#"
            var log = [];
            var options = {};
            Object.defineProperty(options, "maxByteLength", {
                get: function() { log.push("max"); return 2; }
            });
            var newTarget = function() {}.bind(null);
            Object.defineProperty(newTarget, "prototype", {
                get: function() { log.push("prototype"); throw new Error("prototype"); }
            });
            var errors = [];
            try { Reflect.construct(SharedArrayBuffer, [3, options], newTarget); }
            catch (error) { errors.push(error instanceof RangeError); }
            try { new SharedArrayBuffer(2, { maxByteLength: 4 }).grow(1); }
            catch (error) { errors.push(error instanceof RangeError); }
            try { new SharedArrayBuffer(2).grow(3); }
            catch (error) { errors.push(error instanceof TypeError); }
            try { SharedArrayBuffer.prototype.grow.call({}); }
            catch (error) { errors.push(error instanceof TypeError); }
            [log.join(","), errors.join(",")].join("|");
            "#,),
        Value::String(Arc::from("max|true,true,true,true"))
    );
}

#[test]
fn atomics_surface_has_spec_shaped_methods_and_tag() {
    assert_eq!(
        run(r#"
            var names = [
              ["add", 3], ["and", 3], ["compareExchange", 4],
              ["exchange", 3], ["isLockFree", 1], ["load", 2],
              ["notify", 3], ["or", 3], ["pause", 0], ["store", 3],
              ["sub", 3], ["wait", 4], ["waitAsync", 4], ["xor", 3]
            ];
            var shaped = names.every(function(entry) {
              var desc = Object.getOwnPropertyDescriptor(Atomics, entry[0]);
              return typeof desc.value === "function" &&
                desc.value.name === entry[0] && desc.value.length === entry[1] &&
                desc.writable && !desc.enumerable && desc.configurable;
            });
            var tag = Object.getOwnPropertyDescriptor(Atomics, Symbol.toStringTag);
            var failures = 0;
            try { Atomics(); } catch (error) { if (error instanceof TypeError) failures++; }
            try { new Atomics(); } catch (error) { if (error instanceof TypeError) failures++; }
            [
              typeof Atomics,
              Object.getPrototypeOf(Atomics) === Object.prototype,
              shaped,
              tag.value,
              tag.writable,
              tag.enumerable,
              tag.configurable,
              failures
            ].join("|");
        "#),
        Value::String(Arc::from("object|true|true|Atomics|false|false|true|2"))
    );
}

#[test]
fn atomics_number_and_bigint_operations_wrap_and_return_old_values() {
    assert_eq!(
        run(r#"
            var bytes = new Int8Array(new SharedArrayBuffer(2));
            var results = [
              Atomics.store(bytes, 0, 127),
              Atomics.add(bytes, 0, 1),
              Atomics.load(bytes, 0),
              Atomics.sub(bytes, 0, 1),
              Atomics.exchange(bytes, 1, 15),
              Atomics.and(bytes, 1, 6),
              Atomics.or(bytes, 1, 8),
              Atomics.xor(bytes, 1, 3),
              Atomics.compareExchange(bytes, 1, 13, 7),
              Atomics.load(bytes, 1)
            ];
            var big = new BigInt64Array(new SharedArrayBuffer(8));
            results.push(Atomics.store(big, 0, 9223372036854775807n));
            results.push(Atomics.add(big, 0, 1n));
            results.push(Atomics.load(big, 0));
            results.join("|");
        "#),
        Value::String(Arc::from(
            "127|127|-128|-128|0|15|6|14|13|7|9223372036854775807|9223372036854775807|-9223372036854775808"
        ))
    );
}

#[test]
fn atomics_accept_array_buffers_and_validate_immutable_and_index_order() {
    assert_eq!(
        run(r#"
            var mutable = new Int32Array(new ArrayBuffer(4));
            var stored = Atomics.store(mutable, 0, -0);
            var immutable = new Int32Array(
              (new ArrayBuffer(4)).transferToImmutable()
            );
            var order = [];
            var index = { valueOf: function() { order.push("index"); return 0; } };
            var value = { valueOf: function() { order.push("value"); return 1; } };
            var failures = 0;
            try { Atomics.store(immutable, index, value); }
            catch (error) { if (error instanceof TypeError) failures++; }
            try { Atomics.load(new Float32Array(new SharedArrayBuffer(4)), index); }
            catch (error) { if (error instanceof TypeError) failures++; }
            var shared = new Uint8Array(new SharedArrayBuffer(4));
            shared.fill(9, 1, 3);
            [
              Object.is(stored, 0),
              Atomics.load(mutable, 0),
              Atomics.load(immutable, 0),
              order.join(","),
              failures,
              [shared[0], shared[1], shared[2], shared[3]].join(","),
              Atomics.isLockFree(4),
              Atomics.isLockFree(3)
            ].join("|");
        "#),
        Value::String(Arc::from("true|0|0||2|0,9,9,0|true|false"))
    );
}

#[test]
fn test262_agents_share_sab_and_notify_wakes_waiters() {
    assert_eq!(
        run(r#"
            $262.agent.start(`
              $262.agent.receiveBroadcast(function(sab) {
                var view = new Int32Array(sab);
                Atomics.add(view, 1, 1);
                $262.agent.report(Atomics.wait(view, 0, 0, 1000));
                $262.agent.leaving();
              });
            `);
            var view = new Int32Array(new SharedArrayBuffer(8));
            $262.agent.broadcast(view.buffer);
            while (Atomics.load(view, 1) !== 1) {}
            $262.agent.sleep(10);
            var notified = Atomics.notify(view, 0, 1);
            var report;
            while ((report = $262.agent.getReport()) === null) {
              $262.agent.sleep(1);
            }
            [Atomics.load(view, 1), report, notified].join("|");
        "#),
        Value::String(Arc::from("1|ok|1"))
    );
}

#[test]
fn atomics_wait_times_out_in_workers_and_main_agent_cannot_suspend() {
    assert_eq!(
        run(r#"
            $262.agent.start(`
              $262.agent.receiveBroadcast(function(sab) {
                var view = new BigInt64Array(sab);
                $262.agent.report(Atomics.wait(view, 0, 0n, 10));
                $262.agent.leaving();
              });
            `);
            var buffer = new SharedArrayBuffer(8);
            $262.agent.broadcast(buffer);
            var report;
            while ((report = $262.agent.getReport()) === null) {
              $262.agent.sleep(1);
            }
            var view = new Int32Array(buffer);
            var mismatch = Atomics.wait(view, 0, 1, 0);
            var blocked = false;
            try { Atomics.wait(view, 0, 0, 0); }
            catch (error) { blocked = error instanceof TypeError; }
            [report, mismatch, blocked].join("|");
        "#),
        Value::String(Arc::from("timed-out|not-equal|true"))
    );
}

#[test]
fn atomics_wait_async_returns_sync_results_for_immediate_outcomes() {
    assert_eq!(
        run(r#"
            var view = new Int32Array(new SharedArrayBuffer(4));
            var mismatch = Atomics.waitAsync(view, 0, 1, 100);
            var timeout = Atomics.waitAsync(view, 0, 0, 0);
            var { async, value } = mismatch;
            [
              async,
              value,
              timeout.async,
              timeout.value,
              Object.keys(mismatch).join(",")
            ].join("|");
        "#),
        Value::String(Arc::from("false|not-equal|false|timed-out|async,value"))
    );
}

#[test]
fn atomics_wait_async_resolves_notify_and_timeout_through_external_jobs() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var asyncResults = [];
        var notified = new Int32Array(new SharedArrayBuffer(4));
        var timed = new BigInt64Array(new SharedArrayBuffer(8));
        var first = Atomics.waitAsync(notified, 0, 0, 1000);
        var second = Atomics.waitAsync(timed, 0, 0n, 10);
        first.value.then(function(value) { asyncResults.push("first:" + value); });
        second.value.then(function(value) { asyncResults.push("second:" + value); });
        for (var i = 0; i < 5000; i++) ({ index: i, payload: [i, i + 1] });
        Atomics.notify(notified, 0, 1);
    "#,
    )
    .expect("waitAsync setup should run");
    vm.run_external_jobs_until_idle()
        .expect("external waitAsync jobs should settle");
    assert_eq!(
        vm.run("asyncResults.sort().join('|')")
            .expect("waitAsync results should remain observable"),
        Value::String(Arc::from("first:ok|second:timed-out"))
    );
}

#[test]
fn nested_async_functions_resume_outward() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var release;
        var gate = new Promise(resolve => { release = resolve; });
        async function leaf() { return (await gate) + 1; }
        async function middle() { return (await leaf()) + 1; }
        async function outer() { return await middle(); }
        var settled = false;
        var observed;
        var result = outer();
        result.then(value => { settled = true; observed = value; });
        "#,
    )
    .expect("failed to suspend nested async functions");

    assert_eq!(
        vm.run("settled;").expect("failed to read pending state"),
        Value::Bool(false)
    );
    vm.run("release(40);")
        .expect("failed to resume nested async functions");
    assert_eq!(
        vm.run("settled + '|' + observed;")
            .expect("failed to read nested result"),
        Value::String(Arc::from("true|42"))
    );
}

#[test]
fn async_function_pending_await_preserves_microtask_fifo() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var release;
        var gate = new Promise(resolve => { release = resolve; });
        var log = [];
        async function pending() {
            log.push("start");
            await gate;
            log.push("resume");
        }
        var result = pending();
        result.then(() => log.push("done"));
        log.push("sync");
        Promise.resolve().then(() => log.push("before"));
        release();
        Promise.resolve().then(() => log.push("after"));
        "#,
    )
    .expect("failed to run interleaved microtasks");

    assert_eq!(
        vm.run("log.join('|');").expect("failed to read FIFO log"),
        Value::String(Arc::from("start|sync|before|resume|after|done"))
    );
}

#[test]
fn async_function_rejections_preserve_error_objects_and_thrown_values() {
    assert_eq!(
        run(r#"
            let marker = {};
            async function later(x = y, y) {}
            async function selfRef(x = x) {}
            async function evalConflict(a = eval("var a = 42")) {}
            async function userThrow() { throw marker; }

            let results = [];
            try { await later(); } catch (error) {
                results.push(error.constructor === ReferenceError);
            }
            try { await selfRef(); } catch (error) {
                results.push(error.constructor === ReferenceError);
            }
            try { await evalConflict(); } catch (error) {
                results.push(error.constructor === SyntaxError);
            }
            try { await userThrow(); } catch (error) {
                results.push(error === marker);
            }
            results.join("|");
        "#),
        Value::String(Arc::from("true|true|true|true"))
    );
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
        run("typeof Date.parse + ':' + Date.parse('1970') + ':' + typeof Date.UTC + ':' + typeof Date.prototype.getUTCFullYear + ':' + typeof Date.prototype.setUTCFullYear;"),
        Value::String(std::sync::Arc::from("function:0:function:function:function"))
    );
}

#[test]
fn date_utc_and_time_clip_follow_spec() {
    assert_eq!(run("Date.UTC(1970);"), Value::Number(0.0));
    assert_eq!(
        run("Date.UTC(2016, 6, 5, 15, 34, 45, 876);"),
        Value::Number(1467732885876.0)
    );
    assert_eq!(
        run("Date.UTC(1970.9, 0.9, 1.9, 0.9, 0.9, 0.9, 0.9);"),
        Value::Number(0.0)
    );
    assert_eq!(run("Date.UTC(70, 0);"), Value::Number(0.0));
    assert_eq!(run("Date.UTC(-1, 0);"), Value::Number(-62198755200000.0));
    assert!(matches!(run("Date.UTC(Infinity, 0);"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Date.UTC(275760, 8, 13, 0, 0, 0, 1);"), Value::Number(n) if n.is_nan()));
    assert_eq!(
        run(r#"
            var log = "";
            function arg(name, value) {
              return { toString: function() { log += name; return value; } };
            }
            Date.UTC(arg("year", 0), arg("month", 0), arg("date", 1), arg("hours", 0), arg("minutes", 0), arg("seconds", 0), arg("ms", 0));
            log;
        "#),
        Value::String(Arc::from("yearmonthdatehoursminutessecondsms"))
    );
    assert_eq!(
        run("new Date(6.54321).valueOf() + ':' + new Date(-0).getTime() + ':' + Object.is(new Date(-0).getTime(), -0);"),
        Value::String(Arc::from("6:0:false"))
    );
    assert!(matches!(
        run("var d = new Date(0); d.setTime(8640000000000001);"),
        Value::Number(n) if n.is_nan()
    ));
    assert!(matches!(run("Date.UTC(1e100, 0);"), Value::Number(n) if n.is_nan()));
}

#[test]
fn date_time_setters_update_components_and_lengths() {
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCHours(6); d.getTime();"),
        Value::Number(1467352800000.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCMinutes(23); d.getTime();"),
        Value::Number(1467332580000.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCSeconds(45, 543); d.getTime();"),
        Value::Number(1467331245543.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCMilliseconds(333); d.getTime();"),
        Value::Number(1467331200333.0)
    );
    assert_eq!(
        run("Date.prototype.setMilliseconds.length + ':' + Date.prototype.setSeconds.length + ':' + Date.prototype.setMinutes.length + ':' + Date.prototype.setHours.length;"),
        Value::String(Arc::from("1:2:3:4"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var log = "";
            function arg(name) { return { valueOf: function() { log += name; return 0; } }; }
            var result = d.setHours(arg("h"), arg("m"), arg("s"), arg("ms"));
            log + ":" + result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("hmsms:NaN:NaN"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var result = d.setMilliseconds({ valueOf: function() { d.setTime(0); return 1; } });
            result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("NaN:0"))
    );
}

#[test]
fn date_date_setters_update_components_and_lengths() {
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1, 2, 3, 4, 5)); d.setUTCDate(15); d.getTime();"),
        Value::Number(1468548184005.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1, 2, 3, 4, 5)); d.setDate(15); d.getTime();"),
        Value::Number(1468548184005.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1, 2, 3, 4, 5)); d.setUTCMonth(8, 20); d.getTime();"),
        Value::Number(1474336984005.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1, 2, 3, 4, 5)); d.setUTCFullYear(2020, 1, 29); d.getTime();"),
        Value::Number(1582941784005.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCFullYear(2, 0, 1); d.getUTCFullYear() + ':' + d.getTime();"),
        Value::String(Arc::from("2:-62104060800000"))
    );
    assert_eq!(
        run("Date.prototype.setDate.length + ':' + Date.prototype.setUTCDate.length + ':' + Date.prototype.setMonth.length + ':' + Date.prototype.setUTCMonth.length + ':' + Date.prototype.setFullYear.length + ':' + Date.prototype.setUTCFullYear.length;"),
        Value::String(Arc::from("1:1:2:2:3:3"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var log = "";
            var result = d.setUTCDate({
              valueOf: function() { log += "date"; d.setTime(0); return 1; }
            });
            log + ":" + result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("date:NaN:0"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var log = "";
            var result = d.setUTCMonth(
              { valueOf: function() { log += "month"; d.setTime(0); return 1; } },
              { valueOf: function() { log += "date"; return 1; } }
            );
            log + ":" + result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("monthdate:NaN:0"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var result = d.setUTCFullYear(2016);
            result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("1451606400000:1451606400000"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(0);
            var result = d.setUTCFullYear(1e100);
            (result !== result) + ":" + (d.getTime() !== d.getTime());
        "#),
        Value::String(Arc::from("true:true"))
    );
}

#[test]
fn date_stringification_parse_and_json_follow_spec() {
    assert_eq!(
        run(r#"
            var d = new Date(0);
            [
              d.toISOString(),
              d.toUTCString(),
              d.toString(),
              d.toDateString(),
              d.toTimeString()
            ].join("|");
        "#),
        Value::String(Arc::from(
            "1970-01-01T00:00:00.000Z|Thu, 01 Jan 1970 00:00:00 GMT|Thu Jan 01 1970 00:00:00 GMT+0000|Thu Jan 01 1970|00:00:00 GMT+0000"
        ))
    );
    assert_eq!(
        run(r#"
            var d = new Date(0);
            [
              Date.parse(d.toISOString()),
              Date.parse(d.toUTCString()),
              Date.parse(d.toString()),
              Date.parse("1970"),
              Date.parse("1970-01-01T00:00:00"),
              Date.parse("+275760-09-13T00:00:00.000Z")
            ].join("|");
        "#),
        Value::String(Arc::from("0|0|0|0|0|8640000000000000"))
    );
    assert!(matches!(
        run(r#"Date.parse("-000000-03-31T00:45Z");"#),
        Value::Number(n) if n.is_nan()
    ));
    assert_eq!(
        run(r#"
            var result = {};
            var obj = {
              toISOString: function() { return result; },
              valueOf: function() { return 0; }
            };
            Date.prototype.toJSON.call(obj) === result;
        "#),
        Value::Bool(true)
    );
    assert_eq!(run("new Date(NaN).toJSON();"), Value::Null);
    assert_eq!(
        run(r#"
            var oldDate = new Date(1438560000000);
            oldDate.valueOf = function() { throw new Error("valueOf"); };
            oldDate.toString = function() { throw new Error("toString"); };
            new Date(oldDate).getTime();
        "#),
        Value::Number(1438560000000.0)
    );
}

#[test]
fn date_to_temporal_instant_returns_epoch_nanoseconds() {
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(Date.prototype, 'toTemporalInstant');
            var lengthDesc = Object.getOwnPropertyDescriptor(Date.prototype.toTemporalInstant, 'length');
            var nameDesc = Object.getOwnPropertyDescriptor(Date.prototype.toTemporalInstant, 'name');
            Date.prototype.toTemporalInstant.length + ':' +
            Date.prototype.toTemporalInstant.name + ':' +
            desc.writable + ':' + desc.enumerable + ':' + desc.configurable + ':' +
            lengthDesc.writable + ':' + lengthDesc.enumerable + ':' + lengthDesc.configurable + ':' +
            nameDesc.writable + ':' + nameDesc.enumerable + ':' + nameDesc.configurable
            "#),
        Value::String(Arc::from(
            "0:toTemporalInstant:true:false:true:false:false:true:false:false:true"
        ))
    );
    assert_eq!(
        run("new Date(123456789).toTemporalInstant().epochNanoseconds === 123456789000000n;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("new Date(-8640000000000000).toTemporalInstant().epochNanoseconds === -8640000000000000000000n;"),
        Value::Bool(true)
    );
    assert!(run_err("new Date(NaN).toTemporalInstant();").contains("RangeError"));
    assert!(run_err("Date.prototype.toTemporalInstant.call({});").contains("TypeError"));
    assert!(
        run_err("Date.prototype.toTemporalInstant.call(Date.prototype);").contains("TypeError")
    );
    assert!(run_err("var d = new Date(0); new d.toTemporalInstant();").contains("TypeError"));
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
    assert_eq!(
        run("class D extends Date{};Object.prototype.toString.call(new D(0));"),
        Value::String(Arc::from("[object Date]"))
    );
}

#[test]
fn date_prototype_methods_require_date_receivers() {
    assert_eq!(
        run(r#"
            [
              Date.prototype.getFullYear.call(new Date(NaN)),
              Date.prototype.getUTCMonth.call(new Date(0)),
              Date.prototype.getUTCDate.call(new Date(0)),
              Object.prototype.toString.call(Date.prototype),
              Object.prototype.toString.call(new Date(0))
            ].join("|");
            "#,),
        Value::String(Arc::from("NaN|0|1|[object Object]|[object Date]"))
    );
    for src in [
        "Date.prototype.getTime.call(Date.prototype);",
        "Date.prototype.getTime.call({ __time__: 0 });",
        "Date.prototype.getDate.call({});",
        "Date.prototype.getUTCFullYear.call([]);",
        "Date.prototype.setFullYear.call(Date.prototype, 2012);",
        "Date.prototype.setTime.call({ __time__: 0 }, 1);",
        "Date.prototype.setUTCFullYear.call({}, { valueOf: function() { throw new Error('coerced'); } });",
    ] {
        assert!(
            run_err(src).contains("TypeError"),
            "expected TypeError for {src}"
        );
    }
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
        Value::String(Arc::from(
            "Thu Jan 01 1970 00:00:00 GMT+0000Thu Jan 01 1970 00:00:00 GMT+0000"
        ))
    );
    assert_eq!(
        run("var d = new Date(0); d + 0;"),
        Value::String(Arc::from("Thu Jan 01 1970 00:00:00 GMT+00000"))
    );
    assert_eq!(
        run("var d = new Date(0); d + true;"),
        Value::String(Arc::from("Thu Jan 01 1970 00:00:00 GMT+0000true"))
    );
    assert_eq!(
        run("var d = new Date(0); d + {};"),
        Value::String(Arc::from(
            "Thu Jan 01 1970 00:00:00 GMT+0000[object Object]"
        ))
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
