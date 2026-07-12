use ruja::{Parser, Value, Vm};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

fn module_fixture_dir(name: &str) -> PathBuf {
    let unique = format!(
        "ruja-module-{}-{}-{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should follow epoch")
            .as_nanos()
    );
    let dir = std::env::temp_dir().join(unique);
    fs::create_dir_all(&dir).expect("module fixture directory should be created");
    dir
}

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
    assert!(vm.run_module("import './dependency.js';").is_err());
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

#[test]
fn module_graph_preserves_named_import_live_bindings_and_aliases() {
    let dir = module_fixture_dir("live-binding");
    fs::write(
        dir.join("dependency.js"),
        r#"
        export let value = 1;
        globalThis.updateModuleValue = function() { value = 2; };
        "#,
    )
    .expect("dependency should be written");
    fs::write(
        dir.join("entry.js"),
        r#"
        import { value as current } from "./dependency.js";
        var before = current;
        globalThis.updateModuleValue();
        [before, current].join("|");
        "#,
    )
    .expect("entry should be written");
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run_module_file(dir.join("entry.js"))
            .expect("module graph should evaluate"),
        Value::String(Arc::from("1|2"))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn module_graph_evaluates_each_dependency_once_and_supports_named_reexports() {
    let dir = module_fixture_dir("once-reexport");
    fs::write(
        dir.join("dependency.js"),
        r#"
        globalThis.moduleEvaluationCount =
          (globalThis.moduleEvaluationCount || 0) + 1;
        export const value = 41;
        "#,
    )
    .expect("dependency should be written");
    fs::write(
        dir.join("bridge.js"),
        "export { value as answer } from './dependency.js';",
    )
    .expect("bridge should be written");
    fs::write(
        dir.join("entry.js"),
        r#"
        import "./dependency.js";
        import "./dependency.js";
        import { answer } from "./bridge.js";
        answer + globalThis.moduleEvaluationCount;
        "#,
    )
    .expect("entry should be written");
    fs::write(
        dir.join("second-entry.js"),
        r#"
        import { value } from "./dependency.js";
        value + globalThis.moduleEvaluationCount;
        "#,
    )
    .expect("second entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run_module_file(dir.join("entry.js"))
            .expect("module graph should evaluate"),
        Value::Number(42.0)
    );
    assert_eq!(
        vm.run_module_file(dir.join("entry.js"))
            .expect("cached module should not evaluate again"),
        Value::Undefined
    );
    assert_eq!(
        vm.run("globalThis.moduleEvaluationCount;")
            .expect("global count should remain observable"),
        Value::Number(1.0)
    );
    assert_eq!(
        vm.run_module_file(dir.join("second-entry.js"))
            .expect("a second graph should reuse the dependency instance"),
        Value::Number(42.0)
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn module_graph_resolves_star_exports_and_validates_unused_reexports() {
    let dir = module_fixture_dir("star-reexport");
    fs::write(dir.join("value.js"), "export const value = 9;")
        .expect("value dependency should be written");
    fs::write(dir.join("star.js"), "export * from './value.js';")
        .expect("star bridge should be written");
    fs::write(
        dir.join("entry.js"),
        "import { value } from './star.js'; value;",
    )
    .expect("star entry should be written");
    fs::write(dir.join("empty.js"), "export const present = 1;")
        .expect("empty dependency should be written");
    fs::write(
        dir.join("invalid.js"),
        "export { missing } from './empty.js'; 1;",
    )
    .expect("invalid re-export should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run_module_file(dir.join("entry.js"))
            .expect("star export should resolve"),
        Value::Number(9.0)
    );
    let error = vm
        .run_module_file(dir.join("invalid.js"))
        .expect_err("unused missing re-export must fail during linking");
    assert_eq!(error.kind, ruja::ErrorKind::Syntax);
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn module_graph_rejects_cycles_until_declaration_instantiation_is_split() {
    let dir = module_fixture_dir("cycle");
    fs::write(
        dir.join("a.js"),
        "import { b } from './b.js'; export const a = b;",
    )
    .expect("module a should be written");
    fs::write(
        dir.join("b.js"),
        "import { a } from './a.js'; export const b = a;",
    )
    .expect("module b should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    let error = vm
        .run_module_file(dir.join("a.js"))
        .expect_err("cycles must not run with partial instantiation");
    assert_eq!(error.kind, ruja::ErrorKind::Syntax);
    assert!(error.message.contains("Cyclic module graph"));
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn module_graph_rejects_import_assignment_and_propagates_dependency_errors() {
    let dir = module_fixture_dir("errors");
    fs::write(dir.join("value.js"), "export let value = 1;")
        .expect("value dependency should be written");
    fs::write(
        dir.join("assign.js"),
        "import { value } from './value.js'; value = 2;",
    )
    .expect("assignment entry should be written");
    fs::write(dir.join("throw.js"), "throw new TypeError('dependency');")
        .expect("throwing dependency should be written");
    fs::write(
        dir.join("abrupt.js"),
        "import './throw.js'; throw new RangeError('entry');",
    )
    .expect("abrupt entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    let assignment = vm
        .run_module_file(dir.join("assign.js"))
        .expect_err("imports must be immutable");
    assert_eq!(assignment.kind, ruja::ErrorKind::Type);

    let mut vm = Vm::new().expect("VM should initialize");
    let abrupt = vm
        .run_module_file(dir.join("abrupt.js"))
        .expect_err("dependency error should abort entry evaluation");
    assert_eq!(abrupt.kind, ruja::ErrorKind::Type);
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}
