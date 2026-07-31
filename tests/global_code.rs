mod common;

use common::{run, run_err};
use ruja::Value;
use std::sync::Arc;

#[test]
fn for_in_var_key_uses_the_global_binding_reference() {
    assert_eq!(
        run(r#"
            var key = "outer";
            for (var key = "initial" in { final: 1 }) {}
            key + ":" + globalThis.key;
        "#),
        Value::String(Arc::from("final:final"))
    );
}

#[test]
fn global_function_declaration_redefines_configurable_property() {
    assert_eq!(
        run(r#"
            Object.defineProperty(this, "rujaGlobalFn", {
              value: 0,
              writable: false,
              enumerable: false,
              configurable: true
            });
            Object.preventExtensions(this);
            $262.evalScript("function rujaGlobalFn() {}");
            var d = Object.getOwnPropertyDescriptor(this, "rujaGlobalFn");
            [typeof rujaGlobalFn, d.writable, d.enumerable, d.configurable].join(",");
            "#),
        Value::String(Arc::from("function,true,true,false"))
    );
}

#[test]
fn global_declarations_validate_before_script_execution() {
    assert_eq!(
        run(r#"
            var executed = false;
            Object.preventExtensions(this);
            try {
              $262.evalScript("executed = true; var rujaNoGlobalVar;");
            } catch (e) {}
            executed + ":" + typeof rujaNoGlobalVar;
            "#),
        Value::String(Arc::from("false:undefined"))
    );

    assert_eq!(
        run(r#"
            var executed = false;
            Object.preventExtensions(this);
            try {
              $262.evalScript("executed = true; function rujaNoGlobalFn() {}");
            } catch (e) {}
            executed + ":" + typeof rujaNoGlobalFn;
            "#),
        Value::String(Arc::from("false:undefined"))
    );
}

#[test]
fn global_declaration_instantiation_rejects_existing_lexical_collisions() {
    let err = run_err(
        r#"
        let rujaExistingLexical;
        $262.evalScript("var rujaTemp; var rujaExistingLexical;");
        "#,
    );
    assert!(
        err.contains("SyntaxError"),
        "expected SyntaxError, got {err}"
    );

    let err = run_err(
        r#"
        var rujaExistingVar;
        $262.evalScript("var rujaTemp; let rujaExistingVar;");
        "#,
    );
    assert!(
        err.contains("SyntaxError"),
        "expected SyntaxError, got {err}"
    );
}

#[test]
fn global_lexical_declarations_shadow_configurable_globals_and_eval_vars() {
    assert_eq!(
        run(r#"
            let Array;
            var d = Object.getOwnPropertyDescriptor(this, "Array");
            [Array === undefined, typeof this.Array, d.configurable].join(",");
            "#),
        Value::String(Arc::from("true,function,true"))
    );

    assert_eq!(
        run(r#"
            eval("var rujaEvalVar; function rujaEvalFn() {}");
            $262.evalScript("let rujaEvalVar = 1;");
            $262.evalScript("const rujaEvalFn = 2;");
            rujaEvalVar + rujaEvalFn;
            "#),
        Value::Number(3.0)
    );
}

#[test]
fn strict_global_block_function_declaration_stays_block_scoped() {
    assert_eq!(
        run(r#""use strict";
            var before, after;
            try { rujaBlockFn; } catch (e) { before = e.constructor === ReferenceError; }
            { function rujaBlockFn() {} }
            try { rujaBlockFn; } catch (e) { after = e.constructor === ReferenceError; }
            before && after;
            "#),
        Value::Bool(true)
    );
}

#[test]
fn annex_b_global_mirror_is_suppressed_when_global_var_cannot_be_declared() {
    assert_eq!(
        run(r#"
            Object.preventExtensions(this);
            $262.evalScript('{ function rujaBlockedAnnexBGlobal() {} }');
            typeof rujaBlockedAnnexBGlobal;
        "#),
        Value::String(Arc::from("undefined"))
    );
}

#[test]
fn annex_b_global_mirror_uses_realm_global_set_semantics() {
    assert_eq!(
        run(r#"
            var setterValue = "unset";
            Object.defineProperty(this, "rujaAnnexAccessor", {
              configurable: true,
              get: function() { return 17; },
              set: function(value) { setterValue = typeof value; }
            });
            $262.evalScript('{ function rujaAnnexAccessor() {} }');
            setterValue + "," + rujaAnnexAccessor;
        "#),
        Value::String(Arc::from("function,17"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            other.eval('{ function rujaForeignAnnex() {} } globalThis.same = rujaForeignAnnex === globalThis.rujaForeignAnnex;');
            other.same;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn annex_b_global_var_creation_preserves_source_order() {
    assert_eq!(
        run(r#"
            { function rujaAnnexOrderFirst() {} }
            { function rujaAnnexOrderSecond() {} }
            Object.keys(this).filter(function(name) {
              return name === "rujaAnnexOrderFirst" || name === "rujaAnnexOrderSecond";
            }).join(",");
        "#),
        Value::String(Arc::from("rujaAnnexOrderFirst,rujaAnnexOrderSecond"))
    );
}
