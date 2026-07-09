//! Destructuring assignment (non-declaration): `[a, b] = expr`,
//! `{a, b} = expr`, swaps, rest, and nested patterns. Declaration-form
//! destructuring (`let [a] = ...`) is covered elsewhere; these tests
//! target assignment to *existing* bindings.

mod common;
use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

#[test]
fn array_swap() {
    assert_eq!(
        run("var a=1, b=2; [a, b] = [b, a]; a + b;"),
        Value::Number(3.0)
    );
}

#[test]
fn array_assign_to_existing() {
    assert_eq!(
        run("var a=0, b=0; [a, b] = [10, 20]; a + b;"),
        Value::Number(30.0)
    );
}

#[test]
fn array_assign_partial() {
    // Fewer targets than sources: extra sources are dropped.
    assert_eq!(run("var a=0; [a] = [1, 2, 3]; a;"), Value::Number(1.0));
}

#[test]
fn array_assign_with_holes() {
    assert_eq!(
        run("var a=0, c=0; [a, , c] = [1, 2, 3]; a + c;"),
        Value::Number(4.0) // 1 + 3
    );
}

#[test]
fn object_assign_empty_pattern_requires_object_coercible() {
    let err = run_err("({} = null);");
    assert!(err.contains("TypeError"), "{err}");

    let err = run_err("({} = undefined);");
    assert!(err.contains("TypeError"), "{err}");
}

#[test]
fn object_assign_rest_only_requires_object_coercible() {
    let err = run_err("var rest; ({...rest} = null);");
    assert!(err.contains("TypeError"), "{err}");

    let err = run_err("var rest; ({...rest} = undefined);");
    assert!(err.contains("TypeError"), "{err}");
}

#[test]
fn array_assign_empty_pattern_closes_iterator_without_stepping() {
    assert_eq!(
        run(r#"
            var nextCount = 0;
            var returnCount = 0;
            var iterable = {};
            var iterator = {
              next: function() {
                nextCount += 1;
                return { done: true };
              },
              return: function() {
                returnCount += 1;
                return {};
              }
            };
            iterable[Symbol.iterator] = function() { return iterator; };
            var result = [] = iterable;
            [nextCount, returnCount, result === iterable].join(":");
            "#),
        Value::String(Arc::from("0:1:true"))
    );
}

#[test]
fn array_assign_partial_pattern_closes_unfinished_iterator() {
    assert_eq!(
        run(r#"
            var nextCount = 0;
            var returnCount = 0;
            var thisIsIterator = false;
            var argCount = -1;
            var iterable = {};
            var iterator = {
              next: function() {
                nextCount += 1;
                return { value: 7, done: false };
              },
              return: function() {
                returnCount += 1;
                thisIsIterator = this === iterator;
                argCount = arguments.length;
                return {};
              }
            };
            iterable[Symbol.iterator] = function() { return iterator; };
            var x;
            [x] = iterable;
            [nextCount, returnCount, x, thisIsIterator, argCount].join(":");
            "#),
        Value::String(Arc::from("1:1:7:true:0"))
    );
}

#[test]
fn array_assign_rest() {
    assert_eq!(
        run("var head=0, rest=0; [head, ...rest] = [1, 2, 3]; head + rest.length;"),
        Value::Number(3.0) // 1 + 2
    );
}

#[test]
fn array_assign_rest_pattern_early_errors() {
    for src in [
        "var x, y; [...x, y] = [];",
        "var x; [...x,] = [];",
        "var x; [...x,,] = [];",
        "var x, y; [...x, ...y] = [];",
        "var x; [...x = 1] = [];",
    ] {
        let err = run_err(src);
        assert!(err.contains("SyntaxError"), "{src}: {err}");
    }
}

#[test]
fn array_assign_uses_iterator_protocol_and_defaults() {
    let src = r#"
        var a = 0, b = 0;
        var iterable = {
            [Symbol.iterator]: function() {
                var i = 0;
                return {
                    next: function() {
                        i += 1;
                        if (i === 1) return { value: undefined, done: false };
                        if (i === 2) return { value: 7, done: false };
                        return { value: undefined, done: true };
                    }
                };
            }
        };
        [a = 3, b] = iterable;
        a + "," + b;
    "#;
    assert_eq!(run(src), Value::String(Arc::from("3,7")));
}

#[test]
fn array_assign_member_target_evaluates_before_iterator_step() {
    let src = r#"
        var log = [];
        function source() {
            log.push("source");
            return {
                [Symbol.iterator]: function() {
                    log.push("iterator");
                    return {
                        next: function() {
                            log.push("iterator-step");
                            return {
                                get done() {
                                    log.push("iterator-done");
                                    return true;
                                },
                                get value() {
                                    log.push("iterator-value");
                                }
                            };
                        }
                    };
                }
            };
        }
        function target() {
            log.push("target");
            return { set q(v) { log.push("set"); } };
        }
        function targetKey() {
            log.push("target-key");
            return { toString: function() { log.push("target-key-tostring"); return "q"; } };
        }
        [target()[targetKey()]] = source();
        log.join(",");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "source,iterator,target,target-key,iterator-step,iterator-done,target-key-tostring,set"
        ))
    );
}

#[test]
fn array_assign_closes_iterator_on_default_throw_preserving_original_throw() {
    let src = r#"
        var log = [];
        function MyError() {}
        function thrower() {
            throw new MyError();
        }
        var iterator = {
            [Symbol.iterator]: function() {
                return this;
            },
            next: function() {
                return { value: undefined, done: false };
            },
            get return() {
                log.push("return-get");
                throw "ignored";
            }
        };
        try {
            var a;
            [a = thrower()] = iterator;
        } catch (e) {
            log.push(e instanceof MyError);
        }
        log.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("return-get,true")));
}

#[test]
fn array_assign_rest_target_error_closes_before_iterator_step() {
    assert_eq!(
        run(r#"
            var nextCount = 0;
            var returnCount = 0;
            var caught = "";
            var iterable = {};
            var iterator = {
              next: function() {
                nextCount += 1;
                return { done: true };
              },
              return: function() {
                returnCount += 1;
                return {};
              }
            };
            var thrower = function() {
              throw "target";
            };
            iterable[Symbol.iterator] = function() { return iterator; };
            try {
              0, [...{}[thrower()]] = iterable;
            } catch (e) {
              caught = e;
            }
            [nextCount, returnCount, caught].join(":");
            "#),
        Value::String(Arc::from("0:1:target"))
    );
}

#[test]
fn array_assign_rest_iterator_error_closes_iterator() {
    assert_eq!(
        run(r#"
            var nextCount = 0;
            var returnCount = 0;
            var caught = "";
            var iterable = {};
            var iterator = {
              next: function() {
                nextCount += 1;
                throw "next";
              },
              return: function() {
                returnCount += 1;
                return {};
              }
            };
            iterable[Symbol.iterator] = function() { return iterator; };
            var rest;
            try {
              [...rest] = iterable;
            } catch (e) {
              caught = e;
            }
            [nextCount, returnCount, caught].join(":");
            "#),
        Value::String(Arc::from("1:1:next"))
    );
}

#[test]
fn array_assign_closes_iterator_on_target_throw_preserving_original_throw() {
    let src = r#"
        var log = [];
        function MyError() {}
        var target = {
            set a(v) {
                throw new MyError();
            }
        };
        var iterator = {
            [Symbol.iterator]: function() {
                return this;
            },
            next: function() {
                return { value: 1, done: false };
            },
            return: 0
        };
        try {
            [target.a] = iterator;
        } catch (e) {
            log.push(e instanceof MyError);
        }
        log.join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true")));
}

#[test]
fn object_assign_shorthand() {
    assert_eq!(
        run("var x=0, y=0; ({x, y} = {x: 5, y: 7}); x + y;"),
        Value::Number(12.0)
    );
}

#[test]
fn object_assign_rename() {
    assert_eq!(
        run("var p=0, q=0; ({a: p, b: q} = {a: 1, b: 2}); p + q;"),
        Value::Number(3.0)
    );
}

#[test]
fn object_assign_member_target_evaluates_before_source_get() {
    let src = r#"
        var log = [];
        function source() {
            log.push("source");
            return { get p() { log.push("get"); } };
        }
        function target() {
            log.push("target");
            return { set q(v) { log.push("set"); } };
        }
        function sourceKey() {
            log.push("source-key");
            return { toString: function() { log.push("source-key-tostring"); return "p"; } };
        }
        function targetKey() {
            log.push("target-key");
            return { toString: function() { log.push("target-key-tostring"); return "q"; } };
        }
        ({[sourceKey()]: target()[targetKey()]} = source());
        log.join(",");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "source,source-key,source-key-tostring,target,target-key,get,target-key-tostring,set"
        ))
    );
}

#[test]
fn object_assign_allows_duplicate_proto_properties() {
    let src = r#"
        var value = Object.defineProperty({}, "__proto__", { value: 123 });
        var result, x, y;
        result = { __proto__: x, __proto__: y } = value;
        var first = result === value;
        result = ({ __proto__: x, __proto__: y } = value);
        [first, result === value, x, y].join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true,true,123,123")));
}

#[test]
fn destructuring_assignment_expression_result_is_rhs() {
    let src = r#"
        var obj = { a: 1 };
        var arr = [2];
        var a, b;
        var r1 = ({ a } = obj);
        var r2 = ([b] = arr);
        [r1 === obj, r2 === arr, a, b].join(",");
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true,true,1,2")));
}

#[test]
fn fib_via_destructure_assignment() {
    // The classic infinite fibonacci generator using destructuring swap.
    assert_eq!(
        run(
            "function* fib(){ let [a,b]=[0,1]; while(true){ yield a; [a,b]=[b,a+b]; } } var it=fib(); var s=0; for(var i=0;i<6;i++) s+=it.next().value; s;"
        ),
        Value::Number(12.0) // 0+1+1+2+3+5
    );
}

#[test]
fn nested_array_assign() {
    assert_eq!(
        run("var a=0, b=0; [[a, b]] = [[1, 2]]; a + b;"),
        Value::Number(3.0)
    );
}

#[test]
fn object_shorthand_literal() {
    // `{x, y}` object literal shorthand (not assignment).
    assert_eq!(
        run("var x=1, y=2; var o = {x, y}; o.x + o.y;"),
        Value::Number(3.0)
    );
}

#[test]
fn object_assignment_shorthand_defaults() {
    assert_eq!(
        run("var x, vals={}; var result; result = {x = 1} = vals; x + ':' + (result === vals);"),
        Value::String(Arc::from("1:true"))
    );
    assert_eq!(
        run("var x, vals={x:undefined}; var result; result = {x = 1} = vals; x + ':' + (result === vals);"),
        Value::String(Arc::from("1:true"))
    );
    assert_eq!(
        run("var x, vals={x:null}; var result; result = {x = 1} = vals; String(x) + ':' + (result === vals);"),
        Value::String(Arc::from("null:true"))
    );
    assert_eq!(
        run("var x, vals={x:2}; var result; result = {x = 1} = vals; x + ':' + (result === vals);"),
        Value::String(Arc::from("2:true"))
    );
}

#[test]
fn object_assignment_shorthand_default_infers_function_names() {
    assert_eq!(
        run("var arrow; var vals={}; result = {arrow = () => {}} = vals; arrow.name;"),
        Value::String(Arc::from("arrow"))
    );
    assert_eq!(
        run("var fn; var vals={}; result = {fn = function() {}} = vals; fn.name;"),
        Value::String(Arc::from("fn"))
    );
    assert_eq!(
        run("var cls; var vals={}; result = {cls = class {}} = vals; cls.name;"),
        Value::String(Arc::from("cls"))
    );
    assert_eq!(
        run("var cover; var vals={}; result = {cover = (function() {})} = vals; cover.name;"),
        Value::String(Arc::from("cover"))
    );
}

#[test]
fn object_assignment_shorthand_default_keeps_existing_function_names() {
    assert_eq!(
        run("var xFn; var vals={}; result = {xFn = function x() {}} = vals; xFn.name;"),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run("var xCover; var vals={}; result = {xCover = (0, function() {})} = vals; xCover.name;"),
        Value::String(Arc::from(""))
    );
    assert_eq!(
        run("var xCls; var vals={}; result = {xCls = class { static name() {} }} = vals; typeof xCls.name;"),
        Value::String(Arc::from("function"))
    );
}

#[test]
fn object_literal_rejects_assignment_shorthand_defaults() {
    assert!(run_err("var x = 0; ({x = 1});").contains("SyntaxError"));
    assert!(run_err("var x = 0; var o = {x = 1};").contains("SyntaxError"));
    assert!(run_err("var x = 0; ({x = 1}) += {};").contains("SyntaxError"));
}

// ---- array destructuring via iterator protocol (#5) ----

#[test]
fn destructure_custom_iterable() {
    let src = r#"
        let custom = { [Symbol.iterator]: function*(){ yield 1; yield 2; yield 3; } };
        let [a, b, c] = custom;
        a + b + c;
    "#;
    assert_eq!(run(src), Value::Number(6.0));
}

#[test]
fn destructure_generator() {
    let src = r#"
        function* gen() { yield 10; yield 20; yield 30; }
        let [a, b, c] = gen();
        a + b + c;
    "#;
    assert_eq!(run(src), Value::Number(60.0));
}

#[test]
fn destructure_generator_rest() {
    let src = r#"
        function* gen() { yield 10; yield 20; yield 30; }
        let [first, ...rest] = gen();
        first + "," + rest.length + "," + rest[0] + "," + rest[1];
    "#;
    assert_eq!(run(src), Value::String(std::sync::Arc::from("10,2,20,30")));
}

#[test]
fn destructure_string_iterable() {
    // Strings are iterable (code points).
    let src = r#"
        let [a, b] = "hi";
        a + b;
    "#;
    assert_eq!(run(src), Value::String(std::sync::Arc::from("hi")));

    let astral = r#"
        let [a, b] = "\uD801\uDC28";
        a.length + ":" + (b === undefined);
    "#;
    assert_eq!(run(astral), Value::String(std::sync::Arc::from("2:true")));
}

#[test]
fn destructure_short_iterable_pads_undefined() {
    // Fewer values than targets: missing elements bind undefined.
    let src = r#"
        let custom = { [Symbol.iterator]: function*(){ yield 1; } };
        let [a, b, c] = custom;
        a + "|" + (b === undefined) + "|" + (c === undefined);
    "#;
    assert_eq!(run(src), Value::String(std::sync::Arc::from("1|true|true")));
}

#[test]
fn plain_array_destructure_still_works() {
    // Regression: arrays must still destructure by index-equivalent iteration.
    assert_eq!(run("let [a, b] = [5, 6]; a + b;"), Value::Number(11.0));
}

#[test]
fn nested_array_binding_does_not_clobber_outer_iterator() {
    assert_eq!(
        run(r#"var out = [];
               const samples = [[["b", "g"], "abc abc abc", "z"]];
               for (const [[pattern, flags], input, replacement] of samples) {
                 out.push(pattern + "|" + flags + "|" + input + "|" + replacement);
               }
               out.join(",");"#),
        Value::String(Arc::from("b|g|abc abc abc|z"))
    );
}
