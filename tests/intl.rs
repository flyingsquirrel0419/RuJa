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
    vm.run(
        r#"
            var other = $262.createRealm().global;
            var locale = new Intl.Locale("sh-u-ca-gregory");
            var otherLocale = new other.Intl.Locale("und-Thai");
        "#,
    )
    .expect("failed to create foreign realm");
    vm.gc();
    assert_eq!(
        vm.run(
            r#"
                Intl.getCanonicalLocales("sh")[0] + ":" +
                other.Intl.getCanonicalLocales("und-u-kb-yes")[0] + ":" +
                locale.toString() + ":" + otherLocale.maximize().toString();
            "#,
        )
        .expect("Intl intrinsics should remain live"),
        Value::String(Arc::from(
            "sr-Latn:und-u-kb:sr-Latn-u-ca-gregory:th-Thai-TH"
        ))
    );
}

#[test]
fn locale_constructor_exposes_canonical_slots_and_spec_descriptors() {
    assert_eq!(
        run(r#"
            var locale = new Intl.Locale(
              "EN-latn-us-u-ca-islamicc-kn-true",
              { caseFirst: "upper", numberingSystem: "latn" }
            );
            var constructorDescriptor = Object.getOwnPropertyDescriptor(
              Intl, "Locale"
            );
            var prototypeDescriptor = Object.getOwnPropertyDescriptor(
              Intl.Locale, "prototype"
            );
            var languageDescriptor = Object.getOwnPropertyDescriptor(
              Intl.Locale.prototype, "language"
            );
            var tagDescriptor = Object.getOwnPropertyDescriptor(
              Intl.Locale.prototype, Symbol.toStringTag
            );
            [
              locale.toString(), locale.baseName, locale.language,
              locale.script, locale.region, locale.variants,
              locale.calendar, locale.caseFirst, locale.collation,
              locale.hourCycle, locale.numberingSystem, locale.numeric,
              Object.prototype.toString.call(locale),
              Intl.Locale.name, Intl.Locale.length,
              constructorDescriptor.writable,
              constructorDescriptor.enumerable,
              constructorDescriptor.configurable,
              prototypeDescriptor.writable,
              prototypeDescriptor.enumerable,
              prototypeDescriptor.configurable,
              languageDescriptor.get.name,
              languageDescriptor.get.length,
              languageDescriptor.set,
              languageDescriptor.enumerable,
              languageDescriptor.configurable,
              tagDescriptor.writable,
              tagDescriptor.enumerable,
              tagDescriptor.configurable
            ].join(":");
        "#),
        Value::String(Arc::from(
            "en-Latn-US-u-ca-islamic-civil-kf-upper-kn-nu-latn:en-Latn-US:en:Latn:US::islamic-civil:upper:::latn:true:[object Intl.Locale]:Locale:1:true:false:true:false:false:false:get language:0::false:true:false:false:true"
        ))
    );
}

#[test]
fn locale_constructor_observes_options_in_order_and_recanonicalizes() {
    assert_eq!(
        run(r#"
            var log = [];
            var tag = {
              toString: function () { log.push("tag"); return "und-Armn-SU"; }
            };
            var values = {
              language: "ru", script: undefined, region: undefined,
              variants: undefined, calendar: "gregory", collation: undefined,
              hourCycle: "h23", caseFirst: "false", numeric: "false",
              numberingSystem: "latn"
            };
            var options = new Proxy(values, {
              get: function (target, key) {
                log.push(key);
                return target[key];
              }
            });
            var locale = new Intl.Locale(tag, options);
            [locale.toString(), log.join(",")].join(":");
        "#),
        Value::String(Arc::from(
            "ru-Armn-AM-u-ca-gregory-hc-h23-kf-false-kn-nu-latn:tag,language,script,region,variants,calendar,collation,hourCycle,caseFirst,numeric,numberingSystem"
        ))
    );

    assert_eq!(
        run(r#"
            var log = [];
            try {
              new Intl.Locale({
                toString: function () { log.push("tag"); return "not_a_tag"; }
              }, null);
            } catch (error) {
              log.push(error.name);
            }
            log.join(":");
        "#),
        Value::String(Arc::from("tag:TypeError"))
    );
}

#[test]
fn locale_brand_subclass_and_locale_list_fast_path_are_unforgeable() {
    assert_eq!(
        run(r#"
            class Child extends Intl.Locale {
              toString() { throw new Error("observable"); }
            }
            var locale = new Child("EN-us");
            var brandErrors = 0;
            for (var receiver of [Intl.Locale.prototype, {}, new Proxy(locale, {})]) {
              try { Intl.Locale.prototype.toString.call(receiver); }
              catch (error) { if (error instanceof TypeError) brandErrors++; }
            }
            var descriptor = Object.getOwnPropertyDescriptor(
              Intl.Locale.prototype, Symbol.toStringTag
            );
            delete Intl.Locale.prototype[Symbol.toStringTag];
            var objectTag = Object.prototype.toString.call(locale);
            Object.defineProperty(
              Intl.Locale.prototype, Symbol.toStringTag, descriptor
            );
            [
              Intl.getCanonicalLocales([locale])[0],
              Object.getPrototypeOf(locale) === Child.prototype,
              brandErrors,
              objectTag
            ].join(":");
        "#),
        Value::String(Arc::from("en-US:true:3:[object Object]"))
    );
}

#[test]
fn locale_likely_subtags_preserve_suffix_and_use_method_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var locale = new Intl.Locale("und-Thai-fonipa-u-ca-buddhist-x-test");
            var maximized = locale.maximize();
            var minimized = new Intl.Locale("und-CW").minimize();
            var foreign = other.Intl.Locale.prototype.maximize.call(
              new Intl.Locale("en")
            );
            var ForeignNewTarget = other.Function(
              "return function ForeignNewTarget() {}"
            )();
            ForeignNewTarget.prototype = 1;
            var reflected = Reflect.construct(
              Intl.Locale, ["en"], ForeignNewTarget
            );
            [
              maximized.toString(), minimized.toString(),
              Object.getPrototypeOf(foreign) === other.Intl.Locale.prototype,
              Object.getPrototypeOf(reflected) === other.Intl.Locale.prototype
            ].join(":");
        "#),
        Value::String(Arc::from(
            "th-Thai-TH-fonipa-u-ca-buddhist-x-test:pap:true:true"
        ))
    );
}

#[test]
fn locale_list_and_tag_scans_are_fuel_bounded_and_reusable() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
            var sparseLocales = { length: 1000000 };
            var longLocale = "en-x-" + "abcdefgh-".repeat(10000) + "abcdefgh";
            var longOption = "abcdefgh-".repeat(10000) + "abcdefgh";
            var longLocaleObject = new Intl.Locale(longLocale);
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

    vm.set_fuel(Some(100));
    let option_error = vm
        .run("new Intl.Locale('en', { calendar: longOption })")
        .expect_err("long Locale option should exhaust fuel before grammar scanning");
    assert!(option_error.to_string().contains("fuel"));

    for accessor in ["baseName", "language", "script", "region", "variants"] {
        vm.set_fuel(Some(100));
        let accessor_error = vm
            .run(format!("longLocaleObject.{accessor}").as_str())
            .expect_err("structural Locale accessor should precharge its tag scan");
        assert!(accessor_error.to_string().contains("fuel"), "{accessor}");
    }

    vm.set_fuel(None);
    assert_eq!(
        vm.run("new Intl.Locale(Intl.getCanonicalLocales('en-us')[0]).toString()")
            .expect("VM should remain reusable after Intl fuel aborts"),
        Value::String(Arc::from("en-US"))
    );
}
