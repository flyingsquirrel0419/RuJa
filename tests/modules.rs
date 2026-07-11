use ruja::{Parser, Value, Vm};
use std::sync::Arc;

#[test]
fn module_source_goal_is_strict_and_has_undefined_top_level_this() {
    let program = Parser::parse_module("this;").expect("module should parse");
    assert!(program.is_strict);
    assert_eq!(program.source_type, ruja::ast::SourceType::Module);
    assert!(Parser::parse_module("var public;").is_err());
    assert!(Parser::parse_module("var await;").is_err());
    assert!(Parser::parse_module("await 1;").is_ok());
    assert!(Parser::parse("var await = 1;").is_ok());
    assert!(Parser::parse_module("function f(await) { return await; }").is_ok());
    assert!(Parser::parse_module("function f() { await 1; }").is_err());

    let mut vm = Vm::new().expect("failed to initialize VM");
    assert!(vm.run_module("with ({}) {}").is_err());
    assert_eq!(
        vm.run_module("this === undefined;")
            .expect("module should run"),
        Value::Bool(true)
    );
}

#[test]
fn module_top_level_bindings_are_declarative_and_do_not_leak() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    assert_eq!(
        vm.run_module(
            r#"
            var moduleVar = 1;
            let moduleLet = 2;
            const moduleConst = 3;
            function moduleFunction() { return 4; }
            moduleVar + moduleLet + moduleConst + moduleFunction();
            "#,
        )
        .expect("module bindings should initialize"),
        Value::Number(10.0)
    );
    assert_eq!(
        vm.run(
            "[typeof moduleVar, typeof moduleLet, typeof moduleConst, typeof moduleFunction].join(',');"
        )
        .expect("script should run"),
        Value::String(Arc::from("undefined,undefined,undefined,undefined"))
    );
}

#[test]
fn direct_eval_in_module_uses_module_this_and_bindings() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    assert_eq!(
        vm.run_module(
            r#"
            var value = 7;
            eval("this === undefined && value === 7");
            "#,
        )
        .expect("direct eval should inherit the module context"),
        Value::Bool(true)
    );
}

#[test]
fn duplicate_labels_are_early_errors() {
    assert!(Parser::parse_module("outer: outer: ;").is_err());
    assert!(Parser::parse("outer: outer: ;").is_err());
    assert!(Parser::parse("outer: { inner: ; }").is_ok());
}
