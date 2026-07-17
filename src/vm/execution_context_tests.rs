use super::Vm;

#[test]
fn execution_context_stack_restores_after_normal_and_abrupt_reentry() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var other = $262.createRealm().global;
        var callback = other.eval("(function() { return 1; })");
        Array.prototype.map.call([0], callback);
        "#,
    )
    .expect("normal cross-Realm callback should run");
    assert!(vm.execution_contexts.is_empty());

    let error = vm.run(
        r#"
        var throwing = other.eval("(function() { null.value; })");
        Array.prototype.map.call([0], throwing);
        "#,
    );
    assert!(error.is_err());
    assert!(vm.execution_contexts.is_empty());

    vm.run("Array.prototype.map.call([0], callback)[0];")
        .expect("later calls should not observe a stale execution context");
    assert!(vm.execution_contexts.is_empty());

    vm.run(
        r#"
        var rejectAfterAwait = other.eval(
          "(async function() { await 0; null.value; })"
        );
        rejectAfterAwait().catch(function() {});
        "#,
    )
    .expect("async rejection should drain without leaking its execution context");
    assert!(vm.execution_contexts.is_empty());
}
