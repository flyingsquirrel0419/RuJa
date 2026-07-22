mod common;

use common::run;
use ruja::{Value, Vm};
use std::sync::Arc;

#[test]
fn native_to_interpreted_calls_use_the_callee_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            Number.prototype.realmName = "main";
            other.Number.prototype.realmName = "other";

            var callback = other.eval(`(function() {
              nativeToInterpretedLeak = 1;
              return [
                (1).realmName,
                Object.getPrototypeOf(this) === Number.prototype,
                Object.getPrototypeOf(arguments) === Object.prototype
              ].join(",");
            })`);
            var restCallback = other.eval(`(function(value, index, array, ...rest) {
              return Object.getPrototypeOf(rest) === Array.prototype;
            })`);
            var globalCallback = other.eval(`(function() {
              return this === globalThis;
            })`);
            var primitiveCallback = other.eval(`(function() {
              return (1).realmName;
            })`);
            var boundCallback = primitiveCallback.bind(null);
            var proxyCallback = new Proxy(primitiveCallback, {});
            function mainCallback() { return (1).realmName; }

            [
              Array.prototype.map.call([0], callback, 1)[0],
              Array.prototype.map.call([0], restCallback)[0],
              Array.prototype.map.call([0], globalCallback)[0],
              other.nativeToInterpretedLeak,
              globalThis.nativeToInterpretedLeak === undefined,
              other.Array.prototype.map.call([0], mainCallback)[0],
              Array.prototype.map.call([0], boundCallback)[0],
              Array.prototype.map.call([0], proxyCallback)[0]
            ].join("|");
        "#),
        Value::String(Arc::from(
            "other,true,true|true|true|1|true|main|other|other"
        ))
    );
}

#[test]
fn iterative_bound_calls_preserve_realms_this_arguments_and_throw_identity() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            other.mainArrayPrototype = Array.prototype;

            var orderedTarget = other.eval(`(function(first, second, third) {
              return this.label + "|" + first + "," + second + "," + third;
            })`);
            var inner = orderedTarget.bind({ label: "inner-this" }, "inner");
            var transparent = new Proxy(inner, {});
            var outer = transparent.bind({ label: "outer-this" }, "outer");

            var sentinel = {};
            var throwing = other.eval("(function(value) { throw value; })")
              .bind(null, sentinel);
            var sameSentinel = false;
            try { throwing(); }
            catch (error) { sameSentinel = error === sentinel; }

            var foreignTypeErrorTarget = other.eval(
              "(function() { null.value; })"
            ).bind(null);
            var foreignTypeError = false;
            try { foreignTypeErrorTarget(); }
            catch (error) {
              foreignTypeError = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }

            var trap = other.eval(`(function(prefix, target, thisArg, args) {
              return [
                this.label,
                prefix,
                Object.getPrototypeOf(args) === mainArrayPrototype,
                Object.getPrototypeOf(args) === Array.prototype,
                thisArg.label,
                args.join(",")
              ].join("|");
            })`).bind({ label: "trap-this" }, "trap-bound");
            var applyProxy = new Proxy(function () {}, { apply: trap });
            var trapped = applyProxy.bind({ label: "bound-this" }, "bound");

            [
              outer("call"),
              sameSentinel,
              foreignTypeError,
              trapped("call")
            ].join(";");
        "#),
        Value::String(Arc::from(
            "inner-this|inner,outer,call;true;true;trap-this|trap-bound|true|false|bound-this|bound,call"
        ))
    );
}

#[test]
fn nested_native_calls_restore_the_interpreted_context() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            Number.prototype.realmName = "main";
            other.Number.prototype.realmName = "other";
            other.mainNumberValueOf = Number.prototype.valueOf;
            other.MainTypeError = TypeError;

            var callback = other.eval(`(function() {
              var before = (1).realmName;
              var nestedNativeRealm = false;
              try { mainNumberValueOf.call({}); }
              catch (error) {
                nestedNativeRealm = error instanceof MainTypeError &&
                  !(error instanceof TypeError);
              }
              return before + "|" + nestedNativeRealm + "|" + (1).realmName;
            })`);
            var throwing = other.eval("(function() { null.value; })");
            var thrownInOtherRealm = false;
            try { Array.prototype.map.call([0], throwing); }
            catch (error) {
              thrownInOtherRealm = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }

            [
              Array.prototype.map.call([0], callback)[0],
              thrownInOtherRealm,
              (1).realmName
            ].join("|");
        "#),
        Value::String(Arc::from("other|true|other|true|main"))
    );
}

#[test]
fn generator_prologue_and_resume_use_the_generator_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            Number.prototype.realmName = "main";
            other.Number.prototype.realmName = "other";

            var parameterFunction = other.eval(
              "(function*(a, b, c, fallback = (1).realmName) { yield fallback; })"
            );
            var parameterGenerator = Array.prototype.map.call(
              [0], parameterFunction
            )[0];
            var throwingParameterFunction = other.eval(
              "(function*(a, b, c, fallback = null.value) {})"
            );
            var parameterErrorIsForeign = false;
            try {
              Array.prototype.map.call([0], throwingParameterFunction);
            } catch (error) {
              parameterErrorIsForeign = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }
            var foreignGenerator = other.eval(
              "(function*() { yield (1).realmName; })"
            )();
            var mainNext = Object.getPrototypeOf((function*() {}).prototype).next;
            var foreignNext = Object.getPrototypeOf(
              other.eval("(function*() {}).prototype")
            ).next;
            var mainGeneratorPrototype = Object.getPrototypeOf(
              (function*() {}).prototype
            );
            var mainGenerator = (function*() { yield (1).realmName; })();
            var foreignThrowGenerator = other.eval(`(function*() {
              try { yield "start"; }
              catch (error) { yield (1).realmName; }
            })`)();
            foreignThrowGenerator.next();
            var foreignReturnGenerator = other.eval(`(function*() {
              try { yield "start"; }
              finally { yield (1).realmName; }
            })`)();
            foreignReturnGenerator.next();

            [
              parameterGenerator.next().value,
              parameterErrorIsForeign,
              mainNext.call(foreignGenerator).value,
              foreignNext.call(mainGenerator).value,
              mainGeneratorPrototype.throw.call(
                foreignThrowGenerator, "marker"
              ).value,
              mainGeneratorPrototype.return.call(
                foreignReturnGenerator, 9
              ).value,
              foreignReturnGenerator.next().value
            ].join("|");
        "#),
        Value::String(Arc::from("other|true|other|main|other|other|9"))
    );
}

#[test]
fn async_resumption_keeps_the_suspended_function_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            Number.prototype.realmName = "main";
            other.Number.prototype.realmName = "other";
            var asyncCallback = other.eval(`(async function() {
              var before = (1).realmName;
              await 0;
              return before + "|" + (1).realmName;
            })`);
            await Array.prototype.map.call([0], asyncCallback)[0];
        "#),
        Value::String(Arc::from("other|other"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            Number.prototype.realmName = "main";
            other.Number.prototype.realmName = "other";
            var generator = other.eval(`(async function*() {
              var before = (1).realmName;
              await 0;
              yield before + "|" + (1).realmName;
            })`)();
            var mainNext = Object.getPrototypeOf((async function*() {})()).next;
            (await mainNext.call(generator)).value;
        "#),
        Value::String(Arc::from("other|other"))
    );
}

#[test]
fn active_execution_contexts_are_gc_roots() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC hook");

    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            Number.prototype.realmName = "main";
            other.Number.prototype.realmName = "other";
            other.forceGc = forceGc;
            var callback = other.eval(`(function() {
              var before = (1).realmName;
              forceGc();
              return before + "|" + (1).realmName + "|" +
                (Object.getPrototypeOf(arguments) === Object.prototype);
            })`);
            Array.prototype.map.call([0], callback)[0];
            "#,
        )
        .expect("cross-Realm callback should survive collection"),
        Value::String(Arc::from("other|other|true"))
    );

    vm.gc();
    assert_eq!(
        vm.run("Array.prototype.map.call([0], callback)[0];")
            .expect("callback context should remain valid after later collection"),
        Value::String(Arc::from("other|other|true"))
    );
}
