//! ECMA-402 `%Intl%` and locale-list canonicalization behavior.

mod common;

use common::{run, run_err};
use ruja::{Value, Vm};
use std::sync::Arc;

#[test]
fn intl_intrinsic_has_spec_surface_in_each_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "Intl");
            var methodDescriptor = Object.getOwnPropertyDescriptor(
              Intl, "getCanonicalLocales"
            );
            var tagDescriptor = Object.getOwnPropertyDescriptor(
              Intl, Symbol.toStringTag
            );
            var constructible = true;
            try { Reflect.construct(Intl.getCanonicalLocales, []); }
            catch (error) { constructible = false; }
            [
              typeof Intl,
              Object.getPrototypeOf(Intl) === Object.prototype,
              Intl !== other.Intl,
              Object.getPrototypeOf(other.Intl) === other.Object.prototype,
              Object.prototype.toString.call(Intl),
              globalDescriptor.writable,
              globalDescriptor.enumerable,
              globalDescriptor.configurable,
              methodDescriptor.writable,
              methodDescriptor.enumerable,
              methodDescriptor.configurable,
              tagDescriptor.writable,
              tagDescriptor.enumerable,
              tagDescriptor.configurable,
              Intl.getCanonicalLocales.name,
              Intl.getCanonicalLocales.length,
              constructible
            ].join(":");
        "#),
        Value::String(Arc::from(
            "object:true:true:true:[object Intl]:true:false:true:true:false:true:false:false:true:getCanonicalLocales:1:false"
        ))
    );
}

#[test]
fn get_canonical_locales_observes_locale_list_operations_in_order() {
    assert_eq!(
        run(r#"
            var log = [];
            var element = {
              toString: function() { log.push("toString"); return "EN-us"; }
            };
            var target = { length: 3, 1: element, 2: "en-US" };
            var locales = new Proxy(target, {
              get: function(target, key, receiver) {
                log.push("get:" + key);
                return Reflect.get(target, key, receiver);
              },
              has: function(target, key) {
                log.push("has:" + key);
                return Reflect.has(target, key);
              }
            });
            var result = Intl.getCanonicalLocales(locales);
            [result.join(","), log.join(",")].join(":");
        "#),
        Value::String(Arc::from(
            "en-US:get:length,has:0,has:1,get:1,toString,has:2,get:2"
        ))
    );

    assert!(run_err("Intl.getCanonicalLocales(null)").contains("TypeError"));
    assert!(run_err("Intl.getCanonicalLocales([1])").contains("TypeError"));
    assert!(run_err("Intl.getCanonicalLocales('not_a_tag')").contains("RangeError"));
}

#[test]
fn get_canonical_locales_uses_its_function_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var result = other.Intl.getCanonicalLocales(["EN-us"]);
            var range;
            try { other.Intl.getCanonicalLocales("not_a_tag"); }
            catch (error) { range = error; }
            [
              Object.getPrototypeOf(result) === other.Array.prototype,
              Object.getPrototypeOf(range) === other.RangeError.prototype,
              result[0]
            ].join(":");
        "#),
        Value::String(Arc::from("true:true:en-US"))
    );
}

#[test]
fn detached_foreign_method_ignores_replaced_realm_globals() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var canonicalize = other.Intl.getCanonicalLocales;
            other.Array = function PoisonedArray() {};
            other.RangeError = function PoisonedRangeError() {};
            other.TypeError = function PoisonedTypeError() {};
            delete other.Intl;

            var result = canonicalize("EN-us");
            var range;
            var type;
            try { canonicalize("not_a_tag"); } catch (error) { range = error; }
            try { canonicalize([1]); } catch (error) { type = error; }
            [
              Object.getPrototypeOf(result) === other.Array.prototype,
              Object.getPrototypeOf(range) !== other.RangeError.prototype,
              Object.getPrototypeOf(type) !== other.TypeError.prototype,
              range.name,
              type.name,
              result[0]
            ].join(":");
        "#),
        Value::String(Arc::from("false:true:true:RangeError:TypeError:en-US"))
    );
}

#[test]
fn intl_intrinsics_survive_collection() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run("var other = $262.createRealm().global;")
        .expect("failed to create foreign realm");
    vm.gc();
    assert_eq!(
        vm.run(
            r#"
                Intl.getCanonicalLocales("sh")[0] + ":" +
                other.Intl.getCanonicalLocales("und-u-kb-yes")[0];
            "#,
        )
        .expect("Intl intrinsics should remain live"),
        Value::String(Arc::from("sr-Latn:und-u-kb"))
    );
}

#[test]
fn locale_list_and_tag_scans_are_fuel_bounded_and_reusable() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
            var sparseLocales = { length: 1000000 };
            var longLocale = "en-x-" + "abcdefgh-".repeat(10000) + "abcdefgh";
        "#,
    )
    .expect("failed to initialize Intl fuel fixtures");

    vm.set_fuel(Some(100));
    let sparse_error = vm
        .run("Intl.getCanonicalLocales(sparseLocales)")
        .expect_err("sparse locale scan should exhaust fuel");
    assert!(sparse_error.to_string().contains("fuel"));

    vm.set_fuel(Some(100));
    let tag_error = vm
        .run("Intl.getCanonicalLocales(longLocale)")
        .expect_err("long language tag should exhaust fuel before ICU parsing");
    assert!(tag_error.to_string().contains("fuel"));

    vm.set_fuel(None);
    assert_eq!(
        vm.run("Intl.getCanonicalLocales('en-us')[0]")
            .expect("VM should remain reusable after Intl fuel aborts"),
        Value::String(Arc::from("en-US"))
    );
}
