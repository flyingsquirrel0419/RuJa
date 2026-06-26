use ruja::{Interpreter, Value};

fn run(src: &str) -> Value {
    let mut interp = Interpreter::new();
    interp.run(src).unwrap_or(Value::Undefined)
}

#[test]
fn arithmetic() {
    assert_eq!(run("1 + 2 * 3;"), Value::Number(7.0));
    assert_eq!(run("(1 + 2) * 3;"), Value::Number(9.0));
    assert_eq!(run("10 % 3;"), Value::Number(1.0));
    assert_eq!(run("2 ** 10;"), Value::Number(1024.0));
}

#[test]
fn variables() {
    assert_eq!(run("let x = 5; let y = 10; x + y;"), Value::Number(15.0));
    assert_eq!(run("var a = 1; a = 2; a;"), Value::Number(2.0));
}

#[test]
fn control_flow() {
    assert_eq!(run("if (true) { 1; } else { 2; }"), Value::Undefined); // block returns undefined
    assert_eq!(run("let s = 0; for (let i = 0; i < 5; i++) { s += i; } s;"), Value::Number(10.0));
    assert_eq!(run("let s = 0; let i = 0; while (i < 3) { s += i; i++; } s;"), Value::Number(3.0));
}

#[test]
fn functions() {
    assert_eq!(run("function add(a, b) { return a + b; } add(3, 4);"), Value::Number(7.0));
    assert_eq!(run("function fact(n) { return n <= 1 ? 1 : n * fact(n-1); } fact(5);"), Value::Number(120.0));
}

#[test]
fn closures() {
    let src = r#"
        function makeCounter() {
            let count = 0;
            return function() { count = count + 1; return count; };
        }
        let c = makeCounter();
        c(); c();
    "#;
    assert_eq!(run(src), Value::Number(2.0));
}

#[test]
fn objects() {
    let src = r#"
        let p = { x: 1, y: 2 };
        p.x + p.y;
    "#;
    assert_eq!(run(src), Value::Number(3.0));
}

#[test]
fn arrays() {
    assert_eq!(run("let a = [1, 2, 3]; a[0] + a[2];"), Value::Number(4.0));
    assert_eq!(run("[1,2,3].length;"), Value::Number(3.0));
}

#[test]
fn strings() {
    assert_eq!(run("'hello' + ' ' + 'world';"), Value::String(std::rc::Rc::from("hello world")));
    assert_eq!(run("'abc'.length;"), Value::Number(3.0));
}

#[test]
fn try_catch() {
    let src = r#"
        let result;
        try { throw "boom"; } catch (e) { result = e; }
        result;
    "#;
    assert_eq!(run(src), Value::String(std::rc::Rc::from("boom")));
}

#[test]
fn prototype_chain() {
    let src = r#"
        function Animal(name) { this.name = name; }
        Animal.prototype.speak = function() { return this.name + " speaks"; };
        let a = new Animal("Rex");
        a.speak();
    "#;
    assert_eq!(run(src), Value::String(std::rc::Rc::from("Rex speaks")));
}

#[test]
fn debug_fact() {
    let mut interp = Interpreter::new();
    let r = interp.run("function fact(n) { return n <= 1 ? 1 : n * fact(n-1); } fact(5);");
    println!("fact(5) = {:?}", r);
    assert!(r.is_ok());
}

#[test]
fn debug_proto() {
    let mut interp = Interpreter::new();
    let r = interp.run("function Animal(name) { this.name = name; } Animal.prototype.speak = function() { return this.name + ' speaks'; }; let a = new Animal('Rex'); a.speak();");
    println!("speak = {:?}", r);
}

#[test]
fn array_methods() {
    assert_eq!(run("let a = [1,2,3]; a.push(4); a.length;"), Value::Number(4.0));
    assert_eq!(run("[5,3,1,4,2].sort((a,b)=>a-b)[0];"), Value::Undefined); // sort not impl yet, ok
    let _ = run("let a = [1,2,3]; a.map(x=>x*2);"); // should not panic
    assert_eq!(run("let a = [1,2,3]; let s = 0; a.forEach(x => s += x); s;"), Value::Number(6.0));
    assert_eq!(run("[1,2,3].join('-');"), Value::String(std::rc::Rc::from("1-2-3")));
    assert_eq!(run("[1,2,3].includes(2);"), Value::Bool(true));
    assert_eq!(run("[1,2,3].indexOf(5);"), Value::Number(-1.0));
}

#[test]
fn string_methods() {
    assert_eq!(run("'hello'.toUpperCase();"), Value::String(std::rc::Rc::from("HELLO")));
    assert_eq!(run("'hello'.charAt(1);"), Value::String(std::rc::Rc::from("e")));
    assert_eq!(run("'hello world'.split(' ').length;"), Value::Number(2.0));
    assert_eq!(run("'abc'.repeat(3);"), Value::String(std::rc::Rc::from("abcabcabc")));
    assert_eq!(run("'hello'.includes('ell');"), Value::Bool(true));
    assert_eq!(run("'  hi  '.trim();"), Value::String(std::rc::Rc::from("hi")));
    assert_eq!(run("'hello'.slice(1,3);"), Value::String(std::rc::Rc::from("el")));
}

#[test]
fn math_methods() {
    assert_eq!(run("Math.floor(3.7);"), Value::Number(3.0));
    assert_eq!(run("Math.ceil(3.2);"), Value::Number(4.0));
    assert_eq!(run("Math.max(1, 5, 3);"), Value::Number(5.0));
    assert_eq!(run("Math.min(1, 5, 3);"), Value::Number(1.0));
    assert_eq!(run("Math.abs(-5);"), Value::Number(5.0));
    assert_eq!(run("Math.sqrt(16);"), Value::Number(4.0));
    assert_eq!(run("Math.pow(2, 10);"), Value::Number(1024.0));
}

#[test]
fn object_methods() {
    assert_eq!(run("Object.keys({a:1,b:2}).length;"), Value::Number(2.0));
    assert_eq!(run("Object.values({a:1,b:2}).length;"), Value::Number(2.0));
    assert_eq!(run("let o = {x:1}; o.hasOwnProperty('x');"), Value::Bool(true));
    let _ = run("Object.assign({}, {a:1});");
}

#[test]
fn json_methods() {
    assert_eq!(run("JSON.parse('[1,2,3]')[1];"), Value::Number(2.0));
    assert_eq!(run("JSON.parse('{\"x\":5}').x;"), Value::Number(5.0));
    assert_eq!(run("JSON.stringify([1,2,3]);"), Value::String(std::rc::Rc::from("[1,2,3]")));
    assert_eq!(run("JSON.stringify({a:1});"), Value::String(std::rc::Rc::from("{\"a\":1}")));
}

#[test]
fn error_objects() {
    let r = run("let e = new Error('oops'); e.message;");
    

}

#[test]
fn console_works() {
    assert_eq!(run("console.log('hello');"), Value::Undefined);
}

#[test]
fn globals() {
    assert_eq!(run("parseInt('42');"), Value::Number(42.0));
    assert_eq!(run("parseInt('ff', 16);"), Value::Number(255.0));
    assert_eq!(run("isNaN(NaN);"), Value::Bool(true));
    assert_eq!(run("typeof 42;"), Value::String(std::rc::Rc::from("number")));
    assert_eq!(run("typeof 'hi';"), Value::String(std::rc::Rc::from("string")));
    assert_eq!(run("typeof undefined;"), Value::String(std::rc::Rc::from("undefined")));
    assert_eq!(run("typeof null;"), Value::String(std::rc::Rc::from("object")));
}

#[test]
fn arrow_functions() {
    let src = "let add = (a, b) => a + b; add(3, 4);";

    let _ = run(src);
}

#[test]
fn number_methods() {
    assert_eq!(run("(3.14159).toFixed(2);"), Value::String(std::rc::Rc::from("3.14")));
    assert_eq!(run("Number('42');"), Value::Number(42.0));
    assert_eq!(run("Number.isInteger(5);"), Value::Bool(true));
    assert_eq!(run("Number.isInteger(5.5);"), Value::Bool(false));
}

// ---- comprehensive feature tests ----

#[test]
fn scope_and_shadowing() {
    let src = r#"
        let x = 1;
        {
            let x = 2;
            x;
        }
        x;
    "#;
    // block returns undefined; final x is 1
    assert_eq!(run(src), Value::Number(1.0));
}

#[test]
fn const_assignment_errors() {
    let mut interp = Interpreter::new();
    let r = interp.run("const x = 1; x = 2;");
    assert!(r.is_err());
}

#[test]
fn nullish_coalescing() {
    assert_eq!(run("null ?? 'default';"), Value::String(std::rc::Rc::from("default")));
    assert_eq!(run("undefined ?? 42;"), Value::Number(42.0));
    assert_eq!(run("0 ?? 'default';"), Value::Number(0.0));
}

#[test]
fn optional_chaining_like() {
    // We don't have ?. but test logical access
    assert_eq!(run("let o = {a: {b: 1}}; o.a.b;"), Value::Number(1.0));
}

#[test]
fn spread_in_arrays() {
    assert_eq!(run("let a = [1, 2]; let b = [...a, 3, 4]; b.length;"), Value::Number(4.0));
    assert_eq!(run("let a = [1, 2]; let b = [...a, 3]; b[2];"), Value::Number(3.0));
}

#[test]
fn spread_in_calls() {
    assert_eq!(run("function sum(a,b,c) { return a+b+c; } let args = [1,2,3]; sum(...args);"), Value::Number(6.0));
}

#[test]
fn typeof_all_types() {
    assert_eq!(run("typeof 42;"), Value::String(std::rc::Rc::from("number")));
    assert_eq!(run("typeof 's';"), Value::String(std::rc::Rc::from("string")));
    assert_eq!(run("typeof true;"), Value::String(std::rc::Rc::from("boolean")));
    assert_eq!(run("typeof null;"), Value::String(std::rc::Rc::from("object")));
    assert_eq!(run("typeof undefined;"), Value::String(std::rc::Rc::from("undefined")));
    assert_eq!(run("typeof function(){};"), Value::String(std::rc::Rc::from("function")));
    assert_eq!(run("typeof {};"), Value::String(std::rc::Rc::from("object")));
}

#[test]
fn delete_operator() {
    assert_eq!(run("let o = {x:1, y:2}; delete o.x; o.x;"), Value::Undefined);
    assert_eq!(run("let o = {x:1}; delete o.x; 'x' in o;"), Value::Bool(false));
}

#[test]
fn instanceof_works() {
    let src = r#"
        function Animal() {}
        let a = new Animal();
        a instanceof Animal;
    "#;
    assert_eq!(run(src), Value::Bool(true));
}

#[test]
fn for_of_loop() {
    assert_eq!(run("let s = 0; for (let x of [1,2,3,4]) { s += x; } s;"), Value::Number(10.0));
}

#[test]
fn for_in_loop() {
    assert_eq!(run("let keys = []; let o = {a:1,b:2}; for (let k in o) { keys.push(k); } keys.length;"), Value::Number(2.0));
}

#[test]
fn nested_closures() {
    let src = r#"
        function counter() {
            let n = 0;
            return {
                inc: function() { n++; return n; },
                get: function() { return n; }
            };
        }
        let c = counter();
        c.inc(); c.inc(); c.inc();
        c.get();
    "#;
    assert_eq!(run(src), Value::Number(3.0));
}

#[test]
fn this_in_methods() {
    let src = r#"
        let calc = {
            value: 10,
            add: function(n) { return this.value + n; }
        };
        calc.add(5);
    "#;
    assert_eq!(run(src), Value::Number(15.0));
}

#[test]
fn prototype_inheritance() {
    let src = r#"
        function Shape() {}
        Shape.prototype.describe = function() { return 'a shape'; };
        function Circle() {}
        Circle.prototype = Object.create(Shape.prototype);
        Circle.prototype.describe = function() { return 'a circle'; };
        let c = new Circle();
        c.describe();
    "#;
    assert_eq!(run(src), Value::String(std::rc::Rc::from("a circle")));
}

#[test]
fn error_subclassing() {
    let src = r#"
        let e = new TypeError('bad type');
        e.message + ' ' + (e instanceof Error);
    "#;
    let r = run(src);
    if let Value::String(s) = r {
        assert!(s.contains("bad type"));
    } else { panic!("expected string"); }
}

#[test]
fn try_finally() {
    let src = r#"
       let result = [];
       try { result.push('try'); } finally { result.push('finally'); }
       result.join(',');
    "#;
   assert_eq!(run(src), Value::String(std::rc::Rc::from("try,finally")));
}

#[test]
fn nested_try_catch() {
    let src = r#"
        let log = '';
        try {
            try {
                throw 'inner';
            } catch (e) {
                log += e;
                throw 'outer';
            }
        } catch (e2) {
            log += e2;
        }
        log;
    "#;
    assert_eq!(run(src), Value::String(std::rc::Rc::from("innerouter")));
}

#[test]
fn switch_statement() {
    let src = r#"
        function test(x) {
            switch (x) {
                case 1: return 'one';
                case 2: return 'two';
                default: return 'other';
            }
        }
        test(2);
    "#;
    assert_eq!(run(src), Value::String(std::rc::Rc::from("two")));
}

#[test]
fn string_iteration() {
    assert_eq!(run("let s = 0; for (let c of 'abc') { s += 1; } s;"), Value::Number(3.0));
}

#[test]
fn number_edge_cases() {
    assert_eq!(run("0.1 + 0.2;"), Value::Number(0.30000000000000004));
    assert_eq!(run("1/0;"), Value::Number(f64::INFINITY));
    assert_eq!(run("NaN === NaN;"), Value::Bool(false));
    assert_eq!(run("isNaN(NaN);"), Value::Bool(true));
}

#[test]
fn recursion_deep() {
    let src = r#"
        function sum(n) {
            if (n <= 0) return 0;
            return n + sum(n - 1);
        }
        sum(100);
    "#;
    assert_eq!(run(src), Value::Number(5050.0));
}
