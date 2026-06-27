//! Control flow, operators, and recently-fixed correctness bugs:
//! break/continue, switch, finally, hoisting, increment/decrement, typeof,
//! unary +, in/instanceof/delete, comparisons, loose equality.

mod common;
use common::run;
use ruja::Value;
use std::rc::Rc;

// --- break / continue ---

#[test]
fn for_break() {
    assert_eq!(
        run("let s=0; for(let i=0;i<10;i++){ if(i==3) break; s+=i; } s;"),
        Value::Number(3.0)
    );
}

#[test]
fn for_continue() {
    assert_eq!(
        run("let s=0; for(let i=0;i<5;i++){ if(i==2) continue; s+=i; } s;"),
        Value::Number(8.0)
    );
}

#[test]
fn while_break() {
    assert_eq!(
        run("let i=0,s=0; while(i<10){ i++; if(i==4) break; s+=i; } s;"),
        Value::Number(6.0)
    );
}

#[test]
fn break_in_for_var() {
    assert_eq!(
        run("var s=0;for(var i=0;i<10;i++){if(i>=3)break;s+=i}s;"),
        Value::Number(3.0)
    );
}

#[test]
fn continue_in_for_var() {
    assert_eq!(
        run("var s=0;for(var i=0;i<5;i++){if(i==2)continue;s+=i}s;"),
        Value::Number(8.0)
    );
}

#[test]
fn nested_break() {
    assert_eq!(
        run("var s=0;for(var i=0;i<3;i++){for(var j=0;j<3;j++){if(j==1)break;s++}}s;"),
        Value::Number(3.0)
    );
}

// --- switch ---

#[test]
fn switch_fallthrough() {
    assert_eq!(
        run("let r=''; switch(2){case 1: r+='a'; case 2: r+='b'; case 3: r+='c'; break; default: r+='d';} r;"),
        Value::String(Rc::from("bc"))
    );
}

#[test]
fn switch_default() {
    assert_eq!(
        run("let r=''; switch(99){case 1: r+='a'; default: r+='d'; case 3: r+='c';} r;"),
        Value::String(Rc::from("dc"))
    );
}

#[test]
fn switch_break() {
    assert_eq!(
        run("let r=''; switch(1){case 1: r+='a'; break; case 2: r+='b';} r;"),
        Value::String(Rc::from("a"))
    );
}

// --- try / catch / finally ---

#[test]
fn finally_executes_after_try() {
    assert_eq!(run("let r=0;try{r=1;}finally{r=2;}r;"), Value::Number(2.0));
}

#[test]
fn finally_executes_after_catch() {
    assert_eq!(
        run("let r=0;try{throw 1;}catch(e){r=1;}finally{r=r+10;}r;"),
        Value::Number(11.0)
    );
}

// --- operators ---

#[test]
fn typeof_undeclared() {
    assert_eq!(
        run("typeof noSuchVar;"),
        Value::String(Rc::from("undefined"))
    );
}

#[test]
fn unary_plus() {
    assert_eq!(run(r#"+"5";"#), Value::Number(5.0));
    assert_eq!(run("+true;"), Value::Number(1.0));
    assert_eq!(run("+(-5);"), Value::Number(-5.0));
}

#[test]
fn void_operator() {
    assert_eq!(run("void 5;"), Value::Undefined);
    assert_eq!(run("typeof void 0;"), Value::String(Rc::from("undefined")));
}

#[test]
fn in_operator() {
    assert_eq!(run(r#""a" in {a:1};"#), Value::Bool(true));
    assert_eq!(run(r#""b" in {a:1};"#), Value::Bool(false));
    assert_eq!(run("0 in [1,2];"), Value::Bool(true));
}

#[test]
fn delete_operator() {
    assert_eq!(run("delete ({a:1}).a;"), Value::Bool(true));
}

#[test]
fn instanceof_basic() {
    assert_eq!(run("new Error() instanceof Error;"), Value::Bool(true));
}

#[test]
fn string_gt_comparison() {
    assert_eq!(run(r#""b" > "a";"#), Value::Bool(true));
    assert_eq!(run(r#""ab" >= "a";"#), Value::Bool(true));
    assert_eq!(run(r#""a" > "b";"#), Value::Bool(false));
}

#[test]
fn loose_eq_array_bool() {
    assert_eq!(run("[] == false;"), Value::Bool(true));
}

// --- increment / decrement ---

#[test]
fn increment_postfix() {
    assert_eq!(run("var c=5; c++;"), Value::Number(5.0));
    assert_eq!(run("var c=5; c++; c;"), Value::Number(6.0));
}

#[test]
fn increment_prefix() {
    assert_eq!(run("var c=5; ++c;"), Value::Number(6.0));
    assert_eq!(run("var c=5; ++c; c;"), Value::Number(6.0));
}

#[test]
fn increment_in_expression() {
    assert_eq!(run("var c=0; c++; c++; ++c; c;"), Value::Number(3.0));
}

#[test]
fn decrement() {
    assert_eq!(run("var c=5; c--; c;"), Value::Number(4.0));
    assert_eq!(run("var c=5; --c;"), Value::Number(4.0));
}

#[test]
fn var_hoisting_toplevel() {
    assert_eq!(run("console.log(v); var v=5; v;"), Value::Number(5.0));
}

#[test]
fn var_hoisting_function() {
    // console.log prints "undefined" then returns 5; check the return value.
    assert_eq!(run("function f(){ var x=5; return x; } f();"), Value::Number(5.0));
}

#[test]
fn var_function_scope() {
    assert_eq!(run("function f(){ if(true){ var y=10; } return y; } f();"), Value::Number(10.0));
}

#[test]
fn let_block_scope() {
    // inner let shadows outer; outer retains its value.
    let r = run("{let x=1;{let x=2;} x;}");
    assert_eq!(r, Value::Number(1.0));
}
