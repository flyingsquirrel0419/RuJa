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
fn decorated_classes_allow_both_export_positions() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run("globalThis.decoratedExportCalls = 0;")
        .expect("counter should initialize");
    for source in [
        "function d() { decoratedExportCalls++; } @d export class C {}",
        "function d() { decoratedExportCalls++; } export @d class C {}",
        "function d() { decoratedExportCalls++; } @d export default class C {}",
        "function d() { decoratedExportCalls++; } export default @d class C {}",
    ] {
        vm.run_module(source)
            .expect("decorated export should evaluate");
    }
    assert_eq!(
        vm.run("decoratedExportCalls;")
            .expect("counter should be readable"),
        Value::Number(4.0)
    );
    assert!(Parser::parse_module("@a export @b class C {}").is_err());
    assert!(Parser::parse_module("@a export default @b class C {}").is_err());
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
fn module_graph_supports_default_bindings_and_reexports() {
    let dir = module_fixture_dir("default-bindings");
    fs::write(
        dir.join("dependency.js"),
        r#"
        export let value = 1;
        export default function named() { return value; }
        globalThis.updateDefaultValue = function() { value = 2; };
        "#,
    )
    .expect("default dependency should be written");
    fs::write(
        dir.join("bridge.js"),
        "export { default, value } from './dependency.js';",
    )
    .expect("default bridge should be written");
    fs::write(
        dir.join("entry.js"),
        r#"
        import read, { value } from './bridge.js';
        var before = read();
        globalThis.updateDefaultValue();
        [before, read(), value, read.name].join('|');
        "#,
    )
    .expect("default entry should be written");
    fs::write(
        dir.join("anonymous.js"),
        "export default function() { return 7; }",
    )
    .expect("anonymous default function should be written");
    fs::write(
        dir.join("anonymous-entry.js"),
        "import fn from './anonymous.js'; [fn(), fn.name].join('|');",
    )
    .expect("anonymous default entry should be written");
    fs::write(
        dir.join("expression.js"),
        "export default class { valueOf() { return 9; } }",
    )
    .expect("default class should be written");
    fs::write(
        dir.join("expression-entry.js"),
        "import C from './expression.js'; [new C().valueOf(), C.name].join('|');",
    )
    .expect("default class entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run_module_file(dir.join("entry.js"))
            .expect("default graph should evaluate"),
        Value::String(Arc::from("1|2|2|named"))
    );
    assert_eq!(
        vm.run_module_file(dir.join("anonymous-entry.js"))
            .expect("anonymous default function should evaluate"),
        Value::String(Arc::from("7|default"))
    );
    assert_eq!(
        vm.run_module_file(dir.join("expression-entry.js"))
            .expect("default class should evaluate"),
        Value::String(Arc::from("9|default"))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn module_graph_exposes_live_namespace_exotic_objects() {
    let dir = module_fixture_dir("namespace");
    fs::write(
        dir.join("dependency.js"),
        r#"
        export let zebra = 1;
        export const alpha = 2;
        export default 3;
        const deseret = 5;
        const fullwidth = 6;
        export { deseret as \u{10400}, fullwidth as \uFF21 };
        globalThis.updateNamespaceValue = function() { zebra = 4; };
        "#,
    )
    .expect("namespace dependency should be written");
    fs::write(
        dir.join("bridge.js"),
        "export * as nested from './dependency.js'; export * from './dependency.js';",
    )
    .expect("namespace bridge should be written");
    fs::write(
        dir.join("entry.js"),
        r#"
        import * as direct from './dependency.js';
        import * as bridge from './bridge.js';
        var before = direct.zebra;
        globalThis.updateNamespaceValue();
        var desc = Object.getOwnPropertyDescriptor(direct, 'zebra');
        var setFailed = false;
        try { direct.zebra = 9; } catch (error) { setFailed = error instanceof TypeError; }
        [
          before,
          direct.zebra,
          desc.value,
          desc.writable,
          desc.enumerable,
          desc.configurable,
          Object.getPrototypeOf(direct) === null,
          Object.isExtensible(direct),
          Object.keys(direct).join(','),
          Object.keys(direct)[3] === '\u{10400}',
          Object.keys(direct)[4] === '\uFF21',
          Reflect.deleteProperty(direct, 'zebra'),
          setFailed,
          bridge.nested === direct,
          bridge.default
        ].join('|');
        "#,
    )
    .expect("namespace entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run_module_file(dir.join("entry.js"))
            .expect("namespace graph should evaluate"),
        Value::String(Arc::from(
            "1|4|4|true|true|false|true|false|alpha,default,zebra,\u{10400},Ａ|true|true|false|true|true|"
        ))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn top_level_await_suspends_and_preserves_module_graph_order() {
    let single = module_fixture_dir("tla-single");
    fs::write(
        single.join("entry.js"),
        r#"
        var log = [];
        Promise.resolve().then(() => log.push('tick'));
        await 1;
        log.push('await');
        log.join(',');
        "#,
    )
    .expect("single TLA fixture should be written");
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run_module_file(single.join("entry.js"))
            .expect("single TLA module should settle"),
        Value::String(Arc::from("tick,await"))
    );
    fs::remove_dir_all(single).expect("single TLA fixtures should be removed");

    let graph = module_fixture_dir("tla-graph");
    fs::write(graph.join("setup.js"), "globalThis.order = [];")
        .expect("TLA setup should be written");
    fs::write(
        graph.join("async.js"),
        "order.push('async-start'); await 1; order.push('async-end');",
    )
    .expect("async sibling should be written");
    fs::write(graph.join("sync.js"), "order.push('sync');")
        .expect("sync sibling should be written");
    fs::write(
        graph.join("entry.js"),
        "import './setup.js'; import './async.js'; import './sync.js'; order.join(',');",
    )
    .expect("TLA graph entry should be written");
    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run_module_file(graph.join("entry.js"))
            .expect("TLA sibling graph should settle"),
        Value::String(Arc::from("async-start,sync,async-end"))
    );
    fs::remove_dir_all(graph).expect("TLA graph fixtures should be removed");
}

#[test]
fn top_level_await_cycles_complete_before_external_importers() {
    let dir = module_fixture_dir("tla-cycle");
    fs::write(dir.join("setup.js"), "globalThis.order = [];")
        .expect("cycle setup should be written");
    fs::write(
        dir.join("leaf.js"),
        "import './root.js'; order.push('leaf-start'); await 1; order.push('leaf-end');",
    )
    .expect("cycle leaf should be written");
    fs::write(
        dir.join("root.js"),
        "import './leaf.js'; order.push('root-start'); await 1; order.push('root-end');",
    )
    .expect("cycle root should be written");
    fs::write(
        dir.join("importer.js"),
        "import './leaf.js'; order.push('importer');",
    )
    .expect("cycle importer should be written");
    fs::write(
        dir.join("entry.js"),
        "import './setup.js'; import './root.js'; import './importer.js'; order.join(',');",
    )
    .expect("cycle entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run_module_file(dir.join("entry.js"))
            .expect("TLA cycle should settle"),
        Value::String(Arc::from(
            "leaf-start,leaf-end,root-start,root-end,importer"
        ))
    );
    fs::remove_dir_all(dir).expect("TLA cycle fixtures should be removed");
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
fn module_graph_instantiates_cycles_before_evaluation_and_preserves_tdz() {
    let dir = module_fixture_dir("cycle");
    fs::write(
        dir.join("a.js"),
        r#"
        import { callA } from './b.js';
        export function value() { return 1; }
        export const result = callA();
        result;
        "#,
    )
    .expect("module a should be written");
    fs::write(
        dir.join("b.js"),
        r#"
        import { value } from './a.js';
        export function callA() { return value() + 1; }
        "#,
    )
    .expect("module b should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    assert_eq!(
        vm.run_module_file(dir.join("a.js"))
            .expect("hoisted functions should be available through a cycle"),
        Value::Number(2.0)
    );

    fs::write(
        dir.join("tdz-a.js"),
        "import { b } from './tdz-b.js'; export const a = b;",
    )
    .expect("TDZ module a should be written");
    fs::write(
        dir.join("tdz-b.js"),
        "import { a } from './tdz-a.js'; export const b = a;",
    )
    .expect("TDZ module b should be written");
    let error = vm
        .run_module_file(dir.join("tdz-a.js"))
        .expect_err("cyclic lexical access before evaluation must stay in TDZ");
    assert_eq!(error.kind, ruja::ErrorKind::Reference);
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn module_cycle_members_cache_the_same_evaluation_error() {
    let dir = module_fixture_dir("cycle-error");
    fs::write(
        dir.join("a.js"),
        r#"
        import { value } from './b.js';
        export function fromA() { return value; }
        throw new TypeError('cycle failure');
        "#,
    )
    .expect("module a should be written");
    fs::write(
        dir.join("b.js"),
        r#"
        import { fromA } from './a.js';
        export const value = 1;
        "#,
    )
    .expect("module b should be written");
    fs::write(dir.join("second-entry.js"), "import './b.js';")
        .expect("second entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    let first = vm
        .run_module_file(dir.join("a.js"))
        .expect_err("cycle root should fail");
    assert_eq!(first.kind, ruja::ErrorKind::Type);
    let cached = vm
        .run_module_file(dir.join("second-entry.js"))
        .expect_err("another cycle member must reuse the SCC failure");
    assert_eq!(cached.kind, ruja::ErrorKind::Type);
    assert_eq!(cached.message, first.message);
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
        dir.join("skipped.js"),
        "globalThis.skippedDependencyRan = true; throw new RangeError('skipped');",
    )
    .expect("skipped dependency should be written");
    fs::write(
        dir.join("abrupt.js"),
        "import './throw.js'; import './skipped.js'; throw new URIError('entry');",
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
    assert_eq!(
        vm.run("globalThis.skippedDependencyRan === undefined;")
            .expect("skipped dependency marker should be readable"),
        Value::Bool(true)
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn async_module_rejection_is_cached_across_dependent_entries() {
    let dir = module_fixture_dir("async-rejection");
    fs::write(
        dir.join("dependency.js"),
        "await Promise.resolve(); throw new TypeError('async dependency');",
    )
    .expect("async dependency should be written");
    fs::write(
        dir.join("first.js"),
        "import './dependency.js'; globalThis.firstImporterRan = true;",
    )
    .expect("first importer should be written");
    fs::write(
        dir.join("second.js"),
        "import './dependency.js'; globalThis.secondImporterRan = true;",
    )
    .expect("second importer should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    let first = vm
        .run_module_file(dir.join("first.js"))
        .expect_err("async dependency rejection should abort its importer");
    assert_eq!(first.kind, ruja::ErrorKind::Type);
    assert_eq!(
        vm.run("globalThis.firstImporterRan === undefined;")
            .expect("first importer marker should be readable"),
        Value::Bool(true)
    );

    let second = vm
        .run_module_file(dir.join("second.js"))
        .expect_err("cached rejection should abort later importers");
    assert_eq!(second.kind, ruja::ErrorKind::Type);
    assert_eq!(second.message, first.message);
    assert_eq!(
        vm.run("globalThis.secondImporterRan === undefined;")
            .expect("second importer marker should be readable"),
        Value::Bool(true)
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn pending_sibling_evaluation_survives_another_dependency_rejection() {
    let dir = module_fixture_dir("pending-sibling");
    fs::write(
        dir.join("reject.js"),
        "await Promise.resolve(); throw new TypeError('async dependency');",
    )
    .expect("rejecting dependency should be written");
    fs::write(
        dir.join("pending.js"),
        r#"
        globalThis.pendingModuleStarted = true;
        await new Promise(resolve => { globalThis.resolvePendingModule = resolve; });
        globalThis.pendingModuleFinished = true;
        "#,
    )
    .expect("pending dependency should be written");
    fs::write(
        dir.join("first.js"),
        "import './reject.js'; import './pending.js';",
    )
    .expect("first entry should be written");
    fs::write(dir.join("second.js"), "import './pending.js';")
        .expect("second entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    let first = vm
        .run_module_file(dir.join("first.js"))
        .expect_err("one async dependency should reject the first entry");
    assert_eq!(first.kind, ruja::ErrorKind::Type);
    assert_eq!(
        vm.run("globalThis.pendingModuleStarted === true;")
            .expect("pending marker should be readable"),
        Value::Bool(true)
    );

    vm.gc();
    vm.run("globalThis.resolvePendingModule();")
        .expect("cached module evaluation Promise should remain live");
    vm.run_module_file(dir.join("second.js"))
        .expect("later importer should reuse the settled module evaluation");
    assert_eq!(
        vm.run("globalThis.pendingModuleFinished === true;")
            .expect("completion marker should be readable"),
        Value::Bool(true)
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn post_await_fuel_does_not_error_an_unrelated_pending_sibling() {
    let dir = module_fixture_dir("fuel-pending-sibling");
    fs::write(
        dir.join("fuel.js"),
        "await Promise.resolve(); while (true) {}",
    )
    .expect("fuel dependency should be written");
    fs::write(
        dir.join("pending.js"),
        r#"
        globalThis.fuelSiblingStarted = true;
        await new Promise(resolve => { globalThis.resolveFuelSibling = resolve; });
        globalThis.fuelSiblingFinished = true;
        "#,
    )
    .expect("pending dependency should be written");
    fs::write(
        dir.join("first.js"),
        "import './fuel.js'; import './pending.js';",
    )
    .expect("first entry should be written");
    fs::write(dir.join("second.js"), "import './pending.js';")
        .expect("second entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.set_fuel(Some(10_000));
    let first = vm
        .run_module_file(dir.join("first.js"))
        .expect_err("fuel dependency should abort the first entry");
    assert_eq!(first.kind, ruja::ErrorKind::Fuel);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("globalThis.fuelSiblingStarted === true")
            .expect("pending sibling marker should be readable"),
        Value::Bool(true)
    );

    vm.run("globalThis.resolveFuelSibling();")
        .expect("pending sibling should remain resumable");
    vm.run_module_file(dir.join("second.js"))
        .expect("later importer should reuse the settled sibling");
    assert_eq!(
        vm.run("globalThis.fuelSiblingFinished === true")
            .expect("pending sibling completion should be readable"),
        Value::Bool(true)
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_from_script_resolves_canonical_module_namespace() {
    let dir = module_fixture_dir("dynamic-import-script");
    fs::write(
        dir.join("dependency.js"),
        r#"
        export let value = 41;
        export function increment() { value += 1; }
        "#,
    )
    .expect("dynamic dependency should be written");
    fs::write(
        dir.join("thenable.js"),
        "export function then(resolve) { resolve('assimilated'); }",
    )
    .expect("thenable namespace dependency should be written");
    fs::write(
        dir.join("entry.js"),
        r#"
        var specifier = { toString() { return './dependency.js'; } };
        var first = import(specifier);
        var second = import('./dependency.js');
        globalThis.dynamicPromisesAreFresh = first !== second;
        first.then(ns => {
            ns.increment();
            globalThis.dynamicImportResult = ns.value;
        });
        second.then(ns => { globalThis.dynamicImportIdentityValue = ns.value; });
        eval("import('./dependency.js').then(ns => { globalThis.evalDynamicValue = ns.value; });");
        import('./thenable.js').then(value => {
            globalThis.thenableNamespaceAssimilated = value === 'assimilated';
        });
        import('./missing.js').catch(error => {
            globalThis.dynamicImportErrorIsSyntax = error instanceof SyntaxError;
        });
        import('./dependency.js', {}).then(ns => {
            globalThis.dynamicImportOptionsValue = ns.value;
        });
        "#,
    )
    .expect("dynamic import entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.run_file(dir.join("entry.js"))
        .expect("script dynamic imports should settle");
    assert_eq!(
        vm.run(
            "[dynamicPromisesAreFresh, dynamicImportResult, dynamicImportIdentityValue, evalDynamicValue, thenableNamespaceAssimilated, dynamicImportErrorIsSyntax, dynamicImportOptionsValue].join('|')"
        )
        .expect("dynamic import results should be readable"),
        Value::String(Arc::from("true|42|42|42|true|true|42"))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_host_errors_use_the_initiating_realm_after_reentry() {
    let dir = module_fixture_dir("dynamic-import-error-realm");
    fs::write(dir.join("invalid.js"), "export const = 1;")
        .expect("invalid module should be written");
    fs::write(dir.join("dependency.js"), "export const present = 1;")
        .expect("link dependency should be written");
    fs::write(
        dir.join("link-error.js"),
        "import { missing } from './dependency.js'; export { missing };",
    )
    .expect("link-error module should be written");
    fs::write(
        dir.join("entry.js"),
        r#"
        var other = $262.createRealm().global;
        other.mainSyntaxError = SyntaxError;
        other.results = [];
        var callback = other.eval(`(function() {
          var imports = [
            import('./missing.js'),
            import('./invalid.js'),
            import('./link-error.js')
          ];
          for (var promise of imports) {
            results.push(promise instanceof Promise);
            promise.catch(error => {
              results.push(
                error instanceof SyntaxError,
                !(error instanceof mainSyntaxError),
                Object.getPrototypeOf(error) === SyntaxError.prototype
              );
            });
          }
        })`);
        Array.prototype.map.call([0], callback);
        forceGc();
        "#,
    )
    .expect("dynamic import entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");
    vm.run_file(dir.join("entry.js"))
        .expect("dynamic import host errors should reject");
    assert_eq!(
        vm.run("other.results.join('|')")
            .expect("dynamic import Realm markers should be readable"),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_does_not_turn_fuel_exhaustion_into_a_rejection() {
    let dir = module_fixture_dir("dynamic-import-fuel");
    fs::write(dir.join("infinite.js"), "while (true) {}")
        .expect("infinite module should be written");
    fs::write(dir.join("entry.js"), "import('./infinite.js');")
        .expect("dynamic import entry should be written");
    fs::write(
        dir.join("specifier.js"),
        "import({ toString() { while (true) {} } });",
    )
    .expect("infinite specifier entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.set_fuel(Some(10_000));
    let error = vm
        .run_file(dir.join("entry.js"))
        .expect_err("dynamic import must not swallow the host fuel abort");
    assert_eq!(error.kind, ruja::error::ErrorKind::Fuel);

    let mut vm = Vm::new().expect("second VM should initialize");
    vm.set_fuel(Some(10_000));
    let error = vm
        .run_file(dir.join("specifier.js"))
        .expect_err("specifier coercion must not swallow the host fuel abort");
    assert_eq!(error.kind, ruja::error::ErrorKind::Fuel);
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_namespace_thenable_uses_function_realm_and_propagates_fuel() {
    let dir = module_fixture_dir("dynamic-import-thenable-job");
    fs::write(
        dir.join("foreign.js"),
        r#"
        globalThis.foreignThenRealm = $262.createRealm().global;
        export const then = foreignThenRealm.eval(`
          new Proxy(function() {}, { apply: 1 })
        `);
        "#,
    )
    .expect("foreign thenable module should be written");
    fs::write(
        dir.join("foreign-entry.js"),
        r#"
        import('./foreign.js').catch(error => {
          globalThis.thenableErrorUsesFunctionRealm =
            error instanceof foreignThenRealm.TypeError &&
            !(error instanceof TypeError);
        });
        "#,
    )
    .expect("foreign thenable entry should be written");
    fs::write(
        dir.join("infinite.js"),
        "export function then() { while (true) {} }",
    )
    .expect("infinite thenable module should be written");
    fs::write(dir.join("fuel-entry.js"), "import('./infinite.js');")
        .expect("fuel thenable entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.run_file(dir.join("foreign-entry.js"))
        .expect("foreign thenable failure should reject");
    assert_eq!(
        vm.run("thenableErrorUsesFunctionRealm")
            .expect("thenable Realm marker should be readable"),
        Value::Bool(true)
    );

    let mut vm = Vm::new().expect("fuel VM should initialize");
    vm.set_fuel(Some(10_000));
    let error = vm
        .run_file(dir.join("fuel-entry.js"))
        .expect_err("thenable job must not swallow the host fuel abort");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_post_await_fuel_marks_the_module_errored() {
    let dir = module_fixture_dir("dynamic-import-post-await-fuel");
    fs::write(
        dir.join("target.js"),
        "globalThis.postAwaitFuelEvaluations = \
         (globalThis.postAwaitFuelEvaluations || 0) + 1; \
         await Promise.resolve(); while (true) {}",
    )
    .expect("async module should be written");
    fs::write(dir.join("entry.js"), "import('./target.js');")
        .expect("dynamic import entry should be written");
    fs::write(
        dir.join("await-target.js"),
        "globalThis.awaitSetupFuelEvaluations = \
         (globalThis.awaitSetupFuelEvaluations || 0) + 1; \
         await 0; await { get then() { while (true) {} } };",
    )
    .expect("await-setup module should be written");
    fs::write(dir.join("await-entry.js"), "import('./await-target.js');")
        .expect("await-setup dynamic import entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.set_fuel(Some(10_000));
    let first = vm
        .run_file(dir.join("entry.js"))
        .expect_err("post-await fuel exhaustion should abort the first import");
    assert_eq!(first.kind, ruja::error::ErrorKind::Fuel);

    vm.set_fuel(Some(10_000));
    let second = vm
        .run_file(dir.join("entry.js"))
        .expect_err("the errored module must not reuse a pending evaluation Promise");
    assert_eq!(second.kind, ruja::error::ErrorKind::Fuel);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("postAwaitFuelEvaluations")
            .expect("evaluation count should be readable"),
        Value::Number(1.0)
    );

    vm.set_fuel(Some(10_000));
    let first = vm
        .run_file(dir.join("await-entry.js"))
        .expect_err("await setup fuel exhaustion should abort the first import");
    assert_eq!(first.kind, ruja::error::ErrorKind::Fuel);
    vm.set_fuel(Some(10_000));
    let second = vm
        .run_file(dir.join("await-entry.js"))
        .expect_err("await setup abort must be cached as a module error");
    assert_eq!(second.kind, ruja::error::ErrorKind::Fuel);
    vm.set_fuel(None);
    assert_eq!(
        vm.run("awaitSetupFuelEvaluations")
            .expect("await setup evaluation count should be readable"),
        Value::Number(1.0)
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_attributes_load_data_without_executing_or_colliding() {
    let dir = module_fixture_dir("dynamic-import-attributes");
    fs::write(dir.join("data.json"), r#"{"answer":42}"#).expect("JSON module should be written");
    fs::write(dir.join("note.txt"), "first\nsecond").expect("text module should be written");
    fs::write(
        dir.join("payload.json"),
        "0); globalThis.dataModuleExecuted = true; (0",
    )
    .expect("invalid JSON payload should be written");
    fs::write(
        dir.join("data.json.__ruja_import_type_json__"),
        "export default 7;",
    )
    .expect("cache-collision probe module should be written");
    fs::write(
        dir.join("entry.js"),
        r#"
        globalThis.dataModuleExecuted = false;
        import('./data.json', { with: { type: 'json' } }).then(ns => {
            globalThis.jsonModuleValue = ns.default.answer;
        });
        import('./note.txt', { with: { type: 'text' } }).then(ns => {
            globalThis.textModuleValue = ns.default;
        });
        import('./data.json.__ruja_import_type_json__').then(ns => {
            globalThis.collisionModuleValue = ns.default;
        });
        import('./payload.json', { with: { type: 'json' } }).catch(error => {
            globalThis.invalidJsonIsSyntaxError = error instanceof SyntaxError;
        });
        import('./data.json', { with: { integrity: 'x' } }).catch(error => {
            globalThis.unknownAttributeIsTypeError = error instanceof TypeError;
        });
        import('./data.json', { with: { type: 'bogus' } }).catch(error => {
            globalThis.unknownTypeIsTypeError = error instanceof TypeError;
        });
        "#,
    )
    .expect("dynamic import attributes entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.run_file(dir.join("entry.js"))
        .expect("data module imports should settle");
    assert_eq!(
        vm.run(
            "[jsonModuleValue, textModuleValue, collisionModuleValue, dataModuleExecuted, invalidJsonIsSyntaxError, unknownAttributeIsTypeError, unknownTypeIsTypeError].join('|')"
        )
        .expect("data module results should be readable"),
        Value::String(Arc::from(
            "42|first\nsecond|7|false|true|true|true"
        ))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_from_module_reuses_static_namespace() {
    let dir = module_fixture_dir("dynamic-import-module-namespace");
    fs::write(dir.join("dependency.js"), "export const value = 42;")
        .expect("dynamic dependency should be written");
    fs::write(
        dir.join("entry.js"),
        r#"
        import * as staticNamespace from './dependency.js';
        import('./dependency.js').then(dynamicNamespace => {
            globalThis.moduleNamespacesMatch = dynamicNamespace === staticNamespace;
        });
        "#,
    )
    .expect("module entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.run_module_file(dir.join("entry.js"))
        .expect("module dynamic import should settle");
    assert_eq!(
        vm.run("moduleNamespacesMatch")
            .expect("namespace identity marker should be readable"),
        Value::Bool(true)
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_of_evaluating_self_waits_without_reentry() {
    let dir = module_fixture_dir("dynamic-import-module-self");
    fs::write(
        dir.join("entry.js"),
        r#"
        globalThis.selfEvaluationCount = (globalThis.selfEvaluationCount || 0) + 1;
        Promise.all([import('./entry.js'), import('./entry.js')]).then(() => {
            globalThis.selfImportsSettled = true;
        });
        "#,
    )
    .expect("self-importing module should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.run_module_file(dir.join("entry.js"))
        .expect("self imports should settle after the module evaluation");
    assert_eq!(
        vm.run("[selfEvaluationCount, selfImportsSettled].join('|')")
            .expect("self-import markers should be readable"),
        Value::String(Arc::from("1|true"))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_of_tla_self_uses_evaluation_reaction() {
    let dir = module_fixture_dir("dynamic-import-module-tla-self");
    fs::write(
        dir.join("entry.js"),
        r#"
        globalThis.tlaSelfEvaluationCount =
            (globalThis.tlaSelfEvaluationCount || 0) + 1;
        await Promise.resolve();
        Promise.all([import('./entry.js'), import('./entry.js')]).then(([a, b]) => {
            globalThis.tlaSelfImportsSettled = a === b;
        });
        "#,
    )
    .expect("TLA self-importing module should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.run_module_file(dir.join("entry.js"))
        .expect("TLA self imports should follow the evaluation Promise");
    assert_eq!(
        vm.run("[tlaSelfEvaluationCount, tlaSelfImportsSettled].join('|')")
            .expect("TLA self-import markers should be readable"),
        Value::String(Arc::from("1|true"))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_of_tla_self_propagates_cached_rejection() {
    let dir = module_fixture_dir("dynamic-import-module-tla-self-reject");
    fs::write(
        dir.join("entry.js"),
        r#"
        globalThis.tlaSelfRejectCount = (globalThis.tlaSelfRejectCount || 0) + 1;
        globalThis.tlaSelfReason = { marker: true };
        await Promise.resolve();
        import('./entry.js').catch(reason => {
            globalThis.firstTlaSelfReason = reason;
        });
        import('./entry.js').catch(reason => {
            globalThis.secondTlaSelfReason = reason;
        });
        throw globalThis.tlaSelfReason;
        "#,
    )
    .expect("rejecting TLA self-importing module should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.run_module_file(dir.join("entry.js"))
        .expect_err("the root module should preserve its rejection");
    assert_eq!(
        vm.run(
            "[tlaSelfRejectCount, firstTlaSelfReason === tlaSelfReason, secondTlaSelfReason === tlaSelfReason].join('|')"
        )
        .expect("TLA self-rejection markers should be readable"),
        Value::String(Arc::from("1|true|true"))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn dynamic_import_pending_continuation_preserves_target_realm_reason() {
    let dir = module_fixture_dir("dynamic-import-cross-realm-pending");
    fs::write(
        dir.join("target.js"),
        r#"
        globalThis.moduleReason = new TypeError('module failure');
        globalThis.other = $262.createRealm().global;
        other.mainGlobal = globalThis;
        other.results = [];
        var callback = other.eval(`(function() {
          var pending = import('./target.js');
          results.push(pending instanceof Promise);
          pending.catch(reason => {
            results.push(
              reason === mainGlobal.moduleReason,
              reason instanceof mainGlobal.TypeError,
              !(reason instanceof TypeError)
            );
          });
        })`);
        Array.prototype.map.call([0], callback);
        await Promise.resolve();
        forceGc();
        throw moduleReason;
        "#,
    )
    .expect("self-importing target should be written");
    fs::write(
        dir.join("entry.js"),
        "import('./target.js').catch(function() {});",
    )
    .expect("dynamic import entry should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");
    vm.run_file(dir.join("entry.js"))
        .expect("dynamic imports should settle through rejection handlers");
    assert_eq!(
        vm.run("other.results.join('|')")
            .expect("pending import markers should be readable"),
        Value::String(Arc::from("true|true|true|true"))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn import_meta_is_canonical_null_prototype_and_rejects_as_specifier() {
    let dir = module_fixture_dir("import-meta-runtime");
    fs::write(
        dir.join("entry.js"),
        r#"
        const first = import.meta;
        first.marker = 42;
        globalThis.importMetaIdentity = first === import.meta;
        globalThis.importMetaPrototypeIsNull = Object.getPrototypeOf(first) === null;
        globalThis.importMetaIsExtensible = Object.isExtensible(first);
        globalThis.importMetaMutationPersists = import.meta.marker;
        import(import.meta).catch(error => {
            globalThis.importMetaSpecifierErrorIsType = error instanceof TypeError;
        });
        "#,
    )
    .expect("import.meta module should be written");

    let mut vm = Vm::new().expect("VM should initialize");
    vm.run_module_file(dir.join("entry.js"))
        .expect("import.meta module should evaluate");
    assert_eq!(
        vm.run(
            "[importMetaIdentity, importMetaPrototypeIsNull, importMetaIsExtensible, importMetaMutationPersists, importMetaSpecifierErrorIsType].join('|')"
        )
        .expect("import.meta markers should be readable"),
        Value::String(Arc::from("true|true|true|42|true"))
    );
    fs::remove_dir_all(dir).expect("module fixtures should be removed");
}

#[test]
fn inline_module_import_meta_is_canonical_per_evaluation() {
    let mut vm = Vm::new().expect("VM should initialize");
    vm.run_module(
        "globalThis.firstInlineMeta = import.meta; \
         globalThis.inlineMetaSame = import.meta === firstInlineMeta; \
         globalThis.getFirstInlineMeta = () => import.meta;",
    )
    .expect("first inline module should evaluate");
    vm.run_module(
        "globalThis.inlineMetaIsFresh = import.meta !== firstInlineMeta; \
         globalThis.getSecondInlineMeta = () => import.meta;",
    )
    .expect("second inline module should evaluate");
    assert_eq!(
        vm.run(
            "[inlineMetaSame, inlineMetaIsFresh, getFirstInlineMeta() === firstInlineMeta, getSecondInlineMeta() !== firstInlineMeta].join('|')"
        )
            .expect("inline import.meta markers should be readable"),
        Value::String(Arc::from("true|true|true|true"))
    );
}
