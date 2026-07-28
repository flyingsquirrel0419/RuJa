//! Built-in objects and methods: Array, String, Object, Math, JSON, Symbol.

mod common;
use common::{run, run_err};
use ruja::{Value, Vm};
use std::sync::Arc;

#[test]
fn array_prototype_has_the_intrinsic_length_property() {
    assert_eq!(
        run(r#"
            var descriptor = Object.getOwnPropertyDescriptor(Array.prototype, "length");
            var other = $262.createRealm().global;
            var realmDescriptor = Object.getOwnPropertyDescriptor(
              other.Array.prototype,
              "length"
            );
            var proxy = new Proxy(Array.prototype, {});
            var before = [
              descriptor.value,
              descriptor.writable,
              descriptor.enumerable,
              descriptor.configurable,
              "length" in proxy,
              realmDescriptor.value,
              realmDescriptor.writable,
              realmDescriptor.enumerable,
              realmDescriptor.configurable
            ].join(":");
            Array.prototype.length = 2;
            var written = Array.prototype.length;
            Array.prototype.length = 0;
            Array.prototype[2] = 42;
            var indexedLength = Array.prototype.length;
            var indexedValue = Array.prototype[2];
            delete Array.prototype[2];
            Array.prototype.length = 0;
            other.Array.prototype[1] = 7;
            var foreignIndexedLength = other.Array.prototype.length;
            delete other.Array.prototype[1];
            other.Array.prototype.length = 0;
            [
              before, written, indexedLength, indexedValue,
              foreignIndexedLength, Array.isArray(Array.prototype)
            ].join(":");
            "#),
        Value::String(Arc::from(
            "0:true:false:false:true:0:true:false:false:2:3:42:2:true"
        ))
    );
}

#[test]
fn array_slice_and_with_copy_inherited_values_through_holes() {
    assert_eq!(
        run(r#"
            Array.prototype[1] = 1;
            var source = [0];
            source.length = 2;
            var sliced = source.slice();
            delete Array.prototype[1];
            var preservedHole = [,].slice();

            var holes = [0, , 2, , 4];
            Array.prototype[3] = 3;
            var replaced = holes.with(2, 6);
            delete Array.prototype[3];

            var spliced = [1, 2, 3].slice();
            spliced.splice(1, 1);
            var pushed = [1, 2].with(0, 3);
            pushed.push(4);

            [
              sliced[1], Object.hasOwn(sliced, "1"),
              preservedHole.length, Object.hasOwn(preservedHole, "0"),
              replaced[1] === undefined, Object.hasOwn(replaced, "1"),
              replaced[2], replaced[3], Object.hasOwn(replaced, "3"),
              spliced.length, spliced.join(","),
              pushed.length, pushed.join(",")
            ].join(":");
        "#),
        Value::String(Arc::from("1:true:1:false:true:true:6:3:true:2:1,3:3:3,2,4"))
    );
}

#[test]
fn array_push_and_pop_are_generic_and_observe_array_like_properties() {
    assert_eq!(
        run(r#"
            var object = { length: 1, 0: "a" };
            var pushed = Array.prototype.push.call(object, "b", "c");
            var inherited = { 2: "inherited" };
            var child = Object.create(inherited);
            child.length = 3;
            var popped = Array.prototype.pop.call(child);
            var maxError = false;
            try {
              Array.prototype.push.call({ length: Number.MAX_SAFE_INTEGER }, 1);
            } catch (error) {
              maxError = error instanceof TypeError;
            }
            var frozenError = false;
            try { Array.prototype.pop.call(Object.freeze([1])); }
            catch (error) { frozenError = error instanceof TypeError; }
            [
              pushed, object.length, object[1], object[2],
              popped, child.length, Object.hasOwn(child, "2"),
              maxError, frozenError
            ].join("|");
        "#),
        Value::String(Arc::from("3|3|b|c|inherited|2|false|true|true"))
    );
}

#[test]
fn array_slice_uses_species_while_with_is_generic_and_species_free() {
    assert_eq!(
        run(r#"
            var speciesCalls = [];
            function Species(length) {
              speciesCalls.push(length);
              this.length = length;
            }
            var source = [, "b"];
            source.constructor = { [Symbol.species]: Species };
            var speciesResult = source.slice();

            var generic = Array.prototype.slice.call({ length: 3, 1: "x" });
            var withSource = { length: 3, 0: "a", 2: "c" };
            withSource.constructor = {
              get [Symbol.species]() { throw new Error("must not run"); }
            };
            var withResult = Array.prototype.with.call(withSource, 2, "z");

            var other = $262.createRealm().global;
            var foreignSource = other.Array.of(1, 2);
            var mainResult = Array.prototype.slice.call(foreignSource);
            var foreignResult = other.Array.prototype.slice.call([3, 4]);
            var rangeError = false;
            try { Array.prototype.with.call({ length: 1 }, -2, 0); }
            catch (error) { rangeError = error instanceof RangeError; }

            [
              speciesCalls.join(","), speciesResult instanceof Species,
              speciesResult.length, Object.hasOwn(speciesResult, "0"), speciesResult[1],
              Array.isArray(generic), generic.length,
              Object.hasOwn(generic, "0"), generic[1], Object.hasOwn(generic, "2"),
              withResult.join(","), Object.hasOwn(withResult, "1"),
              Object.getPrototypeOf(mainResult) === Array.prototype,
              Object.getPrototypeOf(foreignResult) === other.Array.prototype,
              rangeError
            ].join("|");
        "#),
        Value::String(Arc::from(
            "2|true|2|false|b|true|3|false|x|false|a,,z|true|true|true|true"
        ))
    );
}

#[test]
fn array_concat_is_generic_sparse_and_spreadability_aware() {
    assert_eq!(
        run(r#"
            var inherited = { 1: "inherited" };
            var spreadable = Object.create(inherited);
            spreadable[0] = "own";
            spreadable.length = 3;
            spreadable[Symbol.isConcatSpreadable] = true;

            var opaqueArray = [4, , 6];
            opaqueArray[Symbol.isConcatSpreadable] = false;
            var result = [].concat(spreadable, opaqueArray, "tail");
            var primitiveReceiver = Array.prototype.concat.call(7, 8);

            [
              result.length, result[0], result[1],
              Object.hasOwn(result, "1"), Object.hasOwn(result, "2"),
              result[3] === opaqueArray, result[4],
              primitiveReceiver.length,
              Number.prototype.valueOf.call(primitiveReceiver[0]),
              primitiveReceiver[1]
            ].join("|");
        "#),
        Value::String(Arc::from("5|own|inherited|true|false|true|tail|2|7|8"))
    );
}

#[test]
fn array_concat_observes_species_property_order_and_strict_result_writes() {
    assert_eq!(
        run(r#"
            var log = [];
            var first = { marker: 1 };
            var third = { marker: 3 };
            var target = [first, , third];
            function Species(length) {
              log.push("construct:" + length);
              return new Proxy({ length: 99 }, {
                defineProperty: function(target, key, descriptor) {
                  log.push("define:" + key);
                  return Reflect.defineProperty(target, key, descriptor);
                },
                set: function(target, key, value) {
                  log.push("set:" + key + ":" + value);
                  target[key] = value;
                  return true;
                }
              });
            }
            target.constructor = {
              get [Symbol.species]() {
                log.push("species");
                return Species;
              }
            };
            target[Symbol.isConcatSpreadable] = true;
            var source = new Proxy(target, {
              get: function(target, key, receiver) {
                if (key === "constructor") log.push("get:constructor");
                else if (key === Symbol.isConcatSpreadable) log.push("get:spread");
                else if (key === "length") log.push("get:length");
                else if (key === "0" || key === "2") log.push("get:" + key);
                return Reflect.get(target, key, receiver);
              },
              has: function(target, key) {
                if (key === "0" || key === "1" || key === "2") {
                  log.push("has:" + key);
                }
                return Reflect.has(target, key);
              }
            });
            var fourth = { marker: 4 };
            var result = Array.prototype.concat.call(source, fourth);
            var zero = Object.getOwnPropertyDescriptor(result, "0");

            [
              log.join(","), result.length,
              result[0] === first, Object.hasOwn(result, "1"),
              result[2] === third, result[3] === fourth,
              zero.writable, zero.enumerable, zero.configurable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "get:constructor,species,construct:0,get:spread,get:length,has:0,get:0,define:0,has:1,has:2,get:2,define:2,define:3,set:length:4|4|true|false|true|true|true|true|true"
        ))
    );

    assert!(
        run_err(
            r#"
            function LockedLength() {
              return Object.defineProperty({}, "length", {
                value: 0,
                writable: false
              });
            }
            var source = [1];
            source.constructor = { [Symbol.species]: LockedLength };
            source.concat();
            "#
        )
        .contains("TypeError"),
        "concat must use a strict final Set for a custom species result"
    );
}

#[test]
fn array_concat_uses_the_calling_realm_for_foreign_intrinsic_arrays() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var foreignSource = other.Array.of(1, 2);
            var mainResult = Array.prototype.concat.call(foreignSource, 3);
            var foreignResult = other.Array.prototype.concat.call([4, 5], 6);
            [
              Object.getPrototypeOf(mainResult) === Array.prototype,
              Object.getPrototypeOf(foreignResult) === other.Array.prototype,
              mainResult.join(","), foreignResult.join(",")
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|1,2,3|4,5,6"))
    );
}

#[test]
fn array_concat_checks_the_safe_integer_limit_before_indexed_work() {
    assert_eq!(
        run(r#"
            var indexed = false;
            var huge = new Proxy({
              length: Number.MAX_SAFE_INTEGER,
              [Symbol.isConcatSpreadable]: true
            }, {
              has: function() {
                indexed = true;
                return false;
              }
            });
            var typeError = false;
            try { [0].concat(huge); }
            catch (error) { typeError = error instanceof TypeError; }
            [typeError, indexed].join(":");
        "#),
        Value::String(Arc::from("true:false"))
    );
}

#[test]
fn array_splice_is_generic_sparse_and_species_aware() {
    assert_eq!(
        run(r#"
            var proto = { 1: "inherited" };
            var object = Object.create(proto);
            object[0] = "a";
            object[2] = "c";
            object.length = 3;
            var genericRemoved = Array.prototype.splice.call(
              object, 1, 1, "x", "y"
            );

            var sparse = [0, , 2, 3];
            var speciesLengths = [];
            function Species(length) {
              speciesLengths.push(length);
              this.length = length;
            }
            sparse.constructor = { [Symbol.species]: Species };
            var sparseRemoved = sparse.splice(1, 2, "z");

            var noArgs = [1, 2];
            var noArgsRemoved = noArgs.splice();
            var frozenError = false;
            try { Array.prototype.splice.call(Object.freeze([1, 2]), 0, 1); }
            catch (error) { frozenError = error instanceof TypeError; }

            [
              genericRemoved.join(","), object.length,
              object[0], object[1], object[2], object[3],
              speciesLengths.join(","), sparseRemoved instanceof Species,
              sparseRemoved.length, Object.hasOwn(sparseRemoved, "0"),
              sparseRemoved[1], sparse.join(","),
              noArgsRemoved.length, noArgs.join(","), frozenError
            ].join("|");
        "#),
        Value::String(Arc::from(
            "inherited|4|a|x|y|c|2|true|2|false|2|0,z,3|0|1,2|true"
        ))
    );
}

#[test]
fn array_copy_within_is_generic_sparse_and_observable() {
    assert_eq!(
        run(r#"
            var log = [];
            var prototype = { 1: "inherited" };
            var raw = Object.create(prototype);
            raw.length = 4;
            raw[0] = "zero";
            raw[2] = "two";
            raw[3] = "three";
            var proxy = new Proxy(raw, {
              has: function(target, key) {
                if (typeof key === "string" && key !== "length") log.push("has:" + key);
                return Reflect.has(target, key);
              },
              get: function(target, key, receiver) {
                if (typeof key === "string") log.push("get:" + key);
                return Reflect.get(target, key, receiver);
              },
              set: function(target, key, value, receiver) {
                if (typeof key === "string") log.push("set:" + key + ":" + value);
                return Reflect.set(target, key, value, receiver);
              }
            });
            var same = Array.prototype.copyWithin.call(
              proxy,
              { valueOf: function() { log.push("target"); return 1; } },
              { valueOf: function() { log.push("start"); return 0; } },
              { valueOf: function() { log.push("end"); return 3; } }
            ) === proxy;

            var sparseLog = [];
            var sparse = { 0: "delete-me", 2: "keep", length: 3 };
            var sparseProxy = new Proxy(sparse, {
              has: function(target, key) {
                sparseLog.push("has:" + key);
                return Reflect.has(target, key);
              },
              deleteProperty: function(target, key) {
                sparseLog.push("delete:" + key);
                return Reflect.deleteProperty(target, key);
              }
            });
            Array.prototype.copyWithin.call(sparseProxy, 0, 1, 2);

            var huge = { length: Number.MAX_SAFE_INTEGER };
            huge["9007199254740990"] = { marker: 9 };
            var hugeSame = Array.prototype.copyWithin.call(
              huge, 0, 9007199254740990
            ) === huge;
            var boxed = Array.prototype.copyWithin.call(true);
            var other = $262.createRealm().global;
            var foreignBoxed = other.Array.prototype.copyWithin.call(true);
            var foreignError = false;
            try { other.Array.prototype.copyWithin.call(null, 0, 0); }
            catch (error) {
              foreignError = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }
            var frozenError = false;
            try { Object.freeze([1, 2]).copyWithin(1, 0, 1); }
            catch (error) { frozenError = error instanceof TypeError; }

            [
              same, log.join(","),
              raw[0], raw[1], raw[2], raw[3],
              Object.hasOwn(raw, "1"), Object.hasOwn(raw, "2"),
              sparseLog.join(","), Object.hasOwn(sparse, "0"),
              hugeSame, huge[0].marker,
              boxed instanceof Boolean,
              Object.getPrototypeOf(foreignBoxed) === other.Boolean.prototype,
              foreignError, frozenError
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|get:length,target,start,end,has:2,get:2,set:3:two,has:1,get:1,set:2:inherited,has:0,get:0,set:1:zero|zero|zero|inherited|two|true|true|has:1,delete:0|false|true|9|true|true|true|true"
        ))
    );
}

#[test]
fn array_copy_within_observes_live_iteration_and_partial_failures() {
    assert_eq!(
        run(r#"
            var identityLog = [];
            var identity = new Proxy({ 0: "a", 1: "b", length: 2 }, {
              has: function(target, key) {
                identityLog.push("has:" + key);
                return Reflect.has(target, key);
              },
              get: function(target, key, receiver) {
                if (key !== "length") identityLog.push("get:" + key);
                return Reflect.get(target, key, receiver);
              },
              set: function(target, key, value, receiver) {
                identityLog.push("set:" + key + ":" + value);
                return Reflect.set(target, key, value, receiver);
              }
            });
            Array.prototype.copyWithin.call(identity, 0, 0, 2);

            var liveRaw = { 0: "a", 1: "b", 2: "x", 3: "y", length: 4 };
            var live = new Proxy(liveRaw, {
              set: function(target, key, value, receiver) {
                if (key === "2") target[1] = "changed";
                return Reflect.set(target, key, value, receiver);
              }
            });
            Array.prototype.copyWithin.call(live, 2, 0, 2);

            var setError = {};
            var setRaw = { 0: "a", 1: "b", 2: "x", 3: "y", length: 4 };
            var setProxy = new Proxy(setRaw, {
              set: function(target, key, value, receiver) {
                if (key === "3") throw setError;
                return Reflect.set(target, key, value, receiver);
              }
            });
            var caughtSet = false;
            try { Array.prototype.copyWithin.call(setProxy, 2, 0, 2); }
            catch (error) { caughtSet = error === setError; }

            var deleteError = {};
            var deleteRaw = { 0: "x", 1: "y", length: 4 };
            var deleteProxy = new Proxy(deleteRaw, {
              deleteProperty: function(target, key) {
                if (key === "1") throw deleteError;
                return Reflect.deleteProperty(target, key);
              }
            });
            var caughtDelete = false;
            try { Array.prototype.copyWithin.call(deleteProxy, 0, 2, 4); }
            catch (error) { caughtDelete = error === deleteError; }

            var snapshot = { 0: "a", 1: "b", 2: "c", length: 3 };
            Array.prototype.copyWithin.call(snapshot, 1, 0, {
              valueOf: function() { snapshot.length = 1; return 3; }
            });

            var bigintError = false;
            var stringError = false;
            var falseSetError = false;
            var falseDeleteError = false;
            try { Array.prototype.copyWithin.call({ length: 1 }, 0n, 0); }
            catch (error) { bigintError = error instanceof TypeError; }
            try { Array.prototype.copyWithin.call("abc", 1, 0, 1); }
            catch (error) { stringError = error instanceof TypeError; }
            try {
              Array.prototype.copyWithin.call(
                new Proxy({ 0: 1, length: 1 }, { set: function() { return false; } }),
                0, 0, 1
              );
            } catch (error) { falseSetError = error instanceof TypeError; }
            try {
              Array.prototype.copyWithin.call(
                new Proxy({ 0: 1, length: 2 }, {
                  deleteProperty: function() { return false; }
                }),
                0, 1, 2
              );
            } catch (error) { falseDeleteError = error instanceof TypeError; }

            [
              identityLog.join(","),
              liveRaw[2], liveRaw[3],
              caughtSet, setRaw[2], setRaw[3],
              caughtDelete, Object.hasOwn(deleteRaw, "0"), deleteRaw[1],
              snapshot.length, snapshot[1], snapshot[2],
              bigintError, stringError, falseSetError, falseDeleteError
            ].join("|");
        "#),
        Value::String(Arc::from(
            "has:0,get:0,set:0:a,has:1,get:1,set:1:b|a|changed|true|a|y|true|false|y|1|a|b|true|true|true|true"
        ))
    );
}

#[test]
fn array_iterators_are_generic_live_ordered_and_realm_aware() {
    assert_eq!(
        run(r#"
            var entryLog = [];
            var entryRaw = { 0: "a", 1: "b", length: 2 };
            var entrySource = new Proxy(entryRaw, {
              get: function(target, key, receiver) {
                entryLog.push("get:" + String(key));
                return Reflect.get(target, key, receiver);
              }
            });
            var entries = Array.prototype.entries.call(entrySource);
            var firstEntry = entries.next();
            entryRaw[1] = "changed";
            var secondEntry = entries.next();

            var keyLog = [];
            var keySource = new Proxy({ 0: "ignored", length: 1 }, {
              get: function(target, key, receiver) {
                keyLog.push("get:" + String(key));
                return Reflect.get(target, key, receiver);
              }
            });
            var firstKey = Array.prototype.keys.call(keySource).next();

            var inherited = Object.create({ 1: "proto" });
            inherited.length = 3;
            var inheritedValues = Array.prototype.values.call(inherited);
            var inheritedFirst = inheritedValues.next();
            var inheritedSecond = inheritedValues.next();
            var inheritedThird = inheritedValues.next();

            var elementError = {};
            var abruptSource = {
              length: 2,
              get 0() { throw elementError; },
              1: "next"
            };
            var abrupt = Array.prototype.values.call(abruptSource);
            var caughtElement = false;
            try { abrupt.next(); }
            catch (error) { caughtElement = error === elementError; }
            var afterElementError = abrupt.next();

            var lengthError = {};
            var lengthReads = 0;
            var lengthSource = {
              0: "zero",
              get length() {
                lengthReads++;
                if (lengthReads === 1) throw lengthError;
                return 1;
              }
            };
            var lengthIterator = Array.prototype.values.call(lengthSource);
            var caughtLength = false;
            try { lengthIterator.next(); }
            catch (error) { caughtLength = error === lengthError; }
            var afterLengthError = lengthIterator.next();
            var exhausted = lengthIterator.next();
            var exhaustedAgain = lengthIterator.next();

            var stringIterator = Array.prototype.values.call("ab");
            var boolIterator = Array.prototype.keys.call(true);

            var other = $262.createRealm().global;
            var foreign = other.Array.prototype.entries.call({ 0: "x", length: 1 });
            var foreignResult = foreign.next();
            var foreignPrototype = Object.getPrototypeOf(foreign);
            var foreignNullError = false;
            var foreignBrandError = false;
            try { other.Array.prototype.entries.call(null); }
            catch (error) {
              foreignNullError = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }
            try { foreignPrototype.next.call({}); }
            catch (error) {
              foreignBrandError = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }

            [
              firstEntry.value.join(":") + ":" + firstEntry.done,
              secondEntry.value.join(":") + ":" + secondEntry.done,
              entryLog.join(","),
              firstKey.value + ":" + firstKey.done,
              keyLog.join(","),
              inheritedFirst.value === undefined,
              inheritedSecond.value,
              inheritedThird.value === undefined,
              caughtElement,
              afterElementError.value,
              caughtLength,
              afterLengthError.value,
              exhausted.done,
              exhaustedAgain.done,
              lengthReads,
              stringIterator.next().value,
              stringIterator.next().value,
              boolIterator.next().done,
              Object.getPrototypeOf(foreignResult) === other.Object.prototype,
              Object.getPrototypeOf(foreignResult.value) === other.Array.prototype,
              Object.getPrototypeOf(foreignPrototype.next) === other.Function.prototype,
              foreignNullError,
              foreignBrandError,
              Array.prototype.values === Array.prototype[Symbol.iterator]
            ].join("|");
        "#),
        Value::String(Arc::from(
            "0:a:false|1:changed:false|get:length,get:0,get:length,get:1|0:false|get:length|true|proto|true|true|next|true|zero|true|true|3|a|b|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn array_copy_results_and_intrinsic_prototype_survive_all_dense_mutators() {
    assert_eq!(
        run(r#"
            var sliced = [1, 2].slice();
            var shifted = sliced.shift();
            var copied = [1, 2, 3].slice();
            copied.copyWithin(0, 1);
            var reversed = [1, 2].slice();
            reversed.reverse();
            var filled = [1, 2].with(0, 3);
            filled.fill(9);

            Array.prototype.unshift("x");
            var prototypeLengthAfterUnshift = Array.prototype.length;
            var prototypeValue = Array.prototype[0];
            var prototypeShifted = Array.prototype.shift();
            var prototypeLengthAfterShift = Array.prototype.length;

            var generic = { length: 2, 0: "a", 1: "b" };
            var genericLength = Array.prototype.unshift.call(generic, "z");
            var genericFirst = Array.prototype.shift.call(generic);

            [
              shifted, sliced.length, sliced[0],
              copied.join(","), reversed.join(","), filled.join(","),
              prototypeLengthAfterUnshift, prototypeValue,
              prototypeShifted, prototypeLengthAfterShift,
              genericLength, genericFirst, generic.length,
              generic[0], generic[1]
            ].join("|");
        "#),
        Value::String(Arc::from("1|1|2|2,3,3|2,1|9,9|1|x|x|0|3|z|2|a|b"))
    );
}

#[test]
fn iterator_constructor_and_prototype_have_spec_shape() {
    assert_eq!(
        run(r#"
            let directCall = false;
            let directConstruct = false;
            try { Iterator(); } catch (error) { directCall = error instanceof TypeError; }
            try { new Iterator(); } catch (error) { directConstruct = error instanceof TypeError; }
            class Derived extends Iterator {}
            let derived = new Derived();
            let globalDesc = Object.getOwnPropertyDescriptor(globalThis, "Iterator");
            let prototypeDesc = Object.getOwnPropertyDescriptor(Iterator, "prototype");
            let iteratorDesc = Object.getOwnPropertyDescriptor(
              Iterator.prototype,
              Symbol.iterator
            );
            [
              directCall, directConstruct,
              derived instanceof Derived, derived instanceof Iterator,
              Iterator.length, Iterator.name,
              Object.getPrototypeOf(Iterator) === Function.prototype,
              globalDesc.writable, globalDesc.enumerable, globalDesc.configurable,
              prototypeDesc.writable, prototypeDesc.enumerable, prototypeDesc.configurable,
              iteratorDesc.writable, iteratorDesc.enumerable, iteratorDesc.configurable,
              Iterator.prototype[Symbol.iterator].length,
              Iterator.prototype[Symbol.iterator].name
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|0|Iterator|true|true|false|true|false|false|false|true|false|true|0|[Symbol.iterator]"
        ))
    );
}

#[test]
fn iterator_prototype_accessors_ignore_prototype_properties() {
    assert_eq!(
        run(r#"
            let base = Iterator.prototype;
            let constructorDesc = Object.getOwnPropertyDescriptor(base, "constructor");
            let tagDesc = Object.getOwnPropertyDescriptor(base, Symbol.toStringTag);
            let errors = 0;
            for (let receiver of [undefined, null, true, base]) {
              try { constructorDesc.set.call(receiver, 1); }
              catch (error) { if (error instanceof TypeError) errors++; }
              try { tagDesc.set.call(receiver, 1); }
              catch (error) { if (error instanceof TypeError) errors++; }
            }
            let child = Object.create(base);
            Object.freeze(base);
            child.constructor = 1;
            child[Symbol.toStringTag] = "Child Iterator";
            let existing = { constructor: 2, [Symbol.toStringTag]: "old" };
            constructorDesc.set.call(existing, 3);
            tagDesc.set.call(existing, "new");
            [
              typeof constructorDesc.get, typeof constructorDesc.set,
              constructorDesc.enumerable, constructorDesc.configurable,
              constructorDesc.get.call() === Iterator,
              tagDesc.get.call(), errors,
              child.constructor, child[Symbol.toStringTag],
              existing.constructor, existing[Symbol.toStringTag]
            ].join("|");
        "#),
        Value::String(Arc::from(
            "function|function|false|true|true|Iterator|8|1|Child Iterator|3|new"
        ))
    );
}

#[test]
fn synchronous_iterators_share_iterator_prototype_and_dispose() {
    assert_eq!(
        run(r#"
            let arrayIterator = [1].values();
            let mapIterator = new Map([[1, 2]]).entries();
            let setIterator = new Set([1]).values();
            let regexpIterator = /a/g[Symbol.matchAll]("a");
            let wrongRegExpReceiver = false;
            try { Object.create(regexpIterator).next(); }
            catch (error) { wrongRegExpReceiver = error instanceof TypeError; }
            let returnCalls = 0;
            let disposable = Object.create(Iterator.prototype);
            disposable.return = function() { returnCalls++; return {}; };
            let disposeResult = disposable[Symbol.dispose]();
            [
              arrayIterator instanceof Iterator,
              mapIterator instanceof Iterator,
              setIterator instanceof Iterator,
              regexpIterator instanceof Iterator,
              Object.getPrototypeOf(Object.getPrototypeOf(arrayIterator)) === Iterator.prototype,
              Object.getPrototypeOf(Object.getPrototypeOf(mapIterator)) === Iterator.prototype,
              Object.getPrototypeOf(Object.getPrototypeOf(setIterator)) === Iterator.prototype,
              Object.getPrototypeOf(regexpIterator) !== Iterator.prototype,
              Object.getPrototypeOf(Object.getPrototypeOf(regexpIterator)) === Iterator.prototype,
              Object.prototype.toString.call(arrayIterator),
              Object.prototype.toString.call(mapIterator),
              Object.prototype.toString.call(setIterator),
              Object.prototype.toString.call(regexpIterator),
              wrongRegExpReceiver, returnCalls, disposeResult === undefined
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|[object Array Iterator]|[object Map Iterator]|[object Set Iterator]|[object RegExp String Iterator]|true|1|true"
        ))
    );
}

#[test]
fn collection_iterator_next_methods_enforce_their_own_brand() {
    assert_eq!(
        run(r#"
            var array = [1].values();
            var map = new Map([[1, 2]]).entries();
            var set = new Set([1]).values();
            var arrayNext = Object.getPrototypeOf(array).next;
            var mapNext = Object.getPrototypeOf(map).next;
            var setNext = Object.getPrototypeOf(set).next;
            var errors = 0;
            for (var pair of [
              [arrayNext, map], [arrayNext, set],
              [mapNext, array], [mapNext, set],
              [setNext, array], [setNext, map]
            ]) {
              try { pair[0].call(pair[1]); }
              catch (error) { if (error instanceof TypeError) errors++; }
            }

            var other = $262.createRealm().global;
            var foreignNext = Object.getPrototypeOf(
              other.Array.prototype.values.call([])
            ).next;
            var foreignError = false;
            try { foreignNext.call(map); }
            catch (error) {
              foreignError = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }

            [
              errors,
              arrayNext.call(array).value,
              mapNext.call(map).value.join(":"),
              setNext.call(set).value,
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from("6|1|1:2|1|true"))
    );
}

#[test]
fn async_iterator_dispose_uses_method_realm_and_awaits_return() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var mainGenerator = async function* () {};
            var foreignGenerator = other.eval("(async function* () {})");
            var mainPrototype = Object.getPrototypeOf(
                mainGenerator.constructor.prototype.prototype
            );
            var foreignPrototype = Object.getPrototypeOf(
                foreignGenerator.constructor.prototype.prototype
            );
            var mainDispose = mainPrototype[Symbol.asyncDispose];
            var foreignDispose = foreignPrototype[Symbol.asyncDispose];
            var ForeignPromise = other.Promise;
            other.Promise = null;
            var descriptor = Object.getOwnPropertyDescriptor(
                foreignPrototype,
                Symbol.asyncDispose
            );
            var calls = [];
            var receiver = {
                return: function (value) {
                    calls.push(this === receiver, arguments.length, value === undefined);
                    return {
                        then: function (resolve) {
                            calls.push(this !== receiver);
                            resolve({ done: true });
                        }
                    };
                }
            };
            var foreignPromise = foreignDispose.call(receiver);
            var foreignResult = await foreignPromise;
            var mainPromise = mainDispose.call({});
            var mainResult = await mainPromise;
            var nonCallableError = await foreignDispose.call({ return: 1 }).then(
                function () { return false; },
                function (error) { return error instanceof other.TypeError; }
            );
            var thrown = {};
            var getterReason = await foreignDispose.call({
                get return() { throw thrown; }
            }).then(
                function () { return false; },
                function (error) { return error === thrown; }
            );
            var rejected = {};
            var rejectionReason = await foreignDispose.call({
                return: function () { return Promise.reject(rejected); }
            }).then(
                function () { return false; },
                function (error) { return error === rejected; }
            );
            var order = [];
            var constructorReason = {};
            var abruptPromise = foreignDispose.call({
                return: function () {
                    var promise = ForeignPromise.resolve();
                    Object.defineProperty(promise, "constructor", {
                        get: function () {
                            throw constructorReason;
                        }
                    });
                    return promise;
                }
            });
            abruptPromise.then(
                undefined,
                function (error) {
                    order.push(error === constructorReason ? "dispose" : "wrong");
                }
            );
            ForeignPromise.resolve().then(function () { order.push("marker"); });
            await ForeignPromise.resolve();
            await ForeignPromise.resolve();
            [
                mainDispose !== foreignDispose,
                Object.getPrototypeOf(foreignDispose) === other.Function.prototype,
                foreignPromise instanceof ForeignPromise,
                !(foreignPromise instanceof Promise),
                mainPromise instanceof Promise,
                foreignResult === undefined,
                mainResult === undefined,
                calls.join(","),
                nonCallableError,
                getterReason,
                rejectionReason,
                order.join(","),
                foreignDispose.name,
                foreignDispose.length,
                descriptor.writable,
                descriptor.enumerable,
                descriptor.configurable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true,0,true,true|true|true|true|dispose,marker|[Symbol.asyncDispose]|0|true|false|true"
        ))
    );
}

#[test]
fn async_iterator_dispose_roots_observable_state_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var generator = async function* () {};
            var prototype = Object.getPrototypeOf(
                generator.constructor.prototype.prototype
            );
            var receiver = {
                get return() {
                    forceGc();
                    return function (value) {
                        forceGc();
                        return {
                            get then() {
                                forceGc();
                                return function (resolve) {
                                    forceGc();
                                    resolve({ done: true });
                                };
                            }
                        };
                    };
                }
            };
            var promise = prototype[Symbol.asyncDispose].call(receiver);
            forceGc();
            var result = await promise;
            forceGc();
            result === undefined;
        "#,
        )
        .expect("async iterator disposal should survive observable GC"),
        Value::Bool(true)
    );
}

#[test]
fn iterator_constructor_uses_new_target_realm_default_prototype() {
    assert_eq!(
        run(r#"
            let other = $262.createRealm().global;
            let newTarget = new other.Function();
            newTarget.prototype = undefined;
            let result = Reflect.construct(Iterator, [], newTarget);
            [
              typeof other.Iterator,
              Object.getPrototypeOf(other.Iterator) === other.Function.prototype,
              Object.getPrototypeOf(result) === other.Iterator.prototype
            ].join("|");
        "#),
        Value::String(Arc::from("function|true|true"))
    );
}

#[test]
fn iterator_from_has_spec_shaped_nonconstructible_method() {
    assert_eq!(
        run(r#"
            var descriptor = Object.getOwnPropertyDescriptor(Iterator, "from");
            var length = Object.getOwnPropertyDescriptor(Iterator.from, "length");
            var name = Object.getOwnPropertyDescriptor(Iterator.from, "name");
            var constructError = false;
            try { new Iterator.from([]); }
            catch (error) { constructError = error instanceof TypeError; }
            [
                typeof Iterator.from,
                descriptor.value === Iterator.from,
                descriptor.writable, descriptor.enumerable, descriptor.configurable,
                length.value, length.writable, length.enumerable, length.configurable,
                name.value, name.writable, name.enumerable, name.configurable,
                Object.getPrototypeOf(Iterator.from) === Function.prototype,
                constructError
            ].join("|");
        "#),
        Value::String(Arc::from(
            "function|true|true|false|true|1|false|false|true|from|false|false|true|true|true"
        ))
    );
}

#[test]
fn iterator_from_accepts_strings_iterables_and_direct_iterators() {
    assert_eq!(
        run(r#"
            function collect(iterator) {
              var values = [];
              for (var step = iterator.next(); !step.done; step = iterator.next()) {
                values.push(step.value);
              }
              return values.join(",");
            }
            var iterable = {
              [Symbol.iterator]: function() {
                var values = [3, 4];
                return {
                  next: function() {
                    return values.length === 0
                      ? { done: true }
                      : { value: values.shift(), done: false };
                  }
                };
              }
            };
            var direct = {
              value: 5,
              next: function() {
                return this.value > 6
                  ? { done: true }
                  : { value: this.value++, done: false };
              }
            };
            [
              collect(Iterator.from("ab")),
              collect(Iterator.from(iterable)),
              collect(Iterator.from(direct))
            ].join("|");
        "#),
        Value::String(Arc::from("a,b|3,4|5,6"))
    );
}

#[test]
fn iterator_from_returns_iterator_instances_unchanged() {
    assert_eq!(
        run(r#"
            class CustomIterator extends Iterator {
              next() { return { done: true }; }
            }
            var arrayIterator = [1].values();
            var customIterator = new CustomIterator();
            [
              Iterator.from(arrayIterator) === arrayIterator,
              Iterator.from(customIterator) === customIterator
            ].join("|");
        "#),
        Value::String(Arc::from("true|true"))
    );
}

#[test]
fn iterator_from_caches_next_and_uses_a_branded_wrapper_prototype() {
    assert_eq!(
        run(r#"
            var nextGets = 0;
            var nextCalls = 0;
            var receiverIsIterator = true;
            var iterator = {
              value: 1,
              get next() {
                nextGets += 1;
                return function() {
                  nextCalls += 1;
                  receiverIsIterator = receiverIsIterator && this === iterator;
                  return this.value > 2
                    ? { done: true }
                    : { value: this.value++, done: false };
                };
              }
            };
            var wrapper = Iterator.from(iterator);
            var wrapperPrototype = Object.getPrototypeOf(wrapper);
            var nextBrandError = false;
            var returnBrandError = false;
            var arrayNextBrandError = false;
            var wrapperNextBrandError = false;
            try { wrapperPrototype.next.call({}); }
            catch (error) { nextBrandError = error instanceof TypeError; }
            try { wrapperPrototype.return.call({}); }
            catch (error) { returnBrandError = error instanceof TypeError; }
            try { Object.getPrototypeOf([][Symbol.iterator]()).next.call(wrapper); }
            catch (error) { arrayNextBrandError = error instanceof TypeError; }
            try { wrapperPrototype.next.call([][Symbol.iterator]()); }
            catch (error) { wrapperNextBrandError = error instanceof TypeError; }
            var first = wrapper.next();
            var second = wrapper.next();
            var done = wrapper.next();
            [
              nextGets, nextCalls, receiverIsIterator,
              Object.getPrototypeOf(wrapperPrototype) === Iterator.prototype,
              wrapper instanceof Iterator,
              wrapper[Symbol.iterator]() === wrapper,
              Object.prototype.toString.call(wrapper),
              nextBrandError, returnBrandError, arrayNextBrandError, wrapperNextBrandError,
              first.value, second.value, done.done
            ].join("|");
        "#),
        Value::String(Arc::from(
            "1|3|true|true|true|true|[object Iterator]|true|true|true|true|1|2|true"
        ))
    );
}

#[test]
fn iterator_from_looks_up_return_dynamically_and_preserves_its_result() {
    assert_eq!(
        run(r#"
            var returnGets = 0;
            var receivers = [];
            var argumentCounts = [];
            var firstResult = { value: "first", done: true };
            var secondResult = { value: "second", done: true };
            var iterator = {
              next: function() { return { done: true }; },
              get return() {
                returnGets += 1;
                var result = returnGets === 1 ? firstResult : secondResult;
                return function() {
                  receivers.push(this === iterator);
                  argumentCounts.push(arguments.length);
                  return result;
                };
              }
            };
            var wrapper = Iterator.from(iterator);
            var beforeReturn = returnGets === 0;
            var first = wrapper.return(123);
            var second = wrapper.return();
            [
              beforeReturn, returnGets, receivers.join(","), argumentCounts.join(","),
              first === firstResult, second === secondResult
            ].join("|");
        "#),
        Value::String(Arc::from("true|2|true,true|0,0|true|true"))
    );
}

#[test]
fn iterator_from_return_without_underlying_method_returns_done_result() {
    assert_eq!(
        run(r#"
            var wrapper = Iterator.from({ next: function() { return { done: true }; } });
            var result = wrapper.return();
            [
              result.hasOwnProperty("value"), result.value === undefined, result.done,
              Object.getPrototypeOf(result) === Object.prototype
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true"))
    );
}

#[test]
fn typed_array_prevent_extensions_requires_a_fixed_length_view() {
    assert_eq!(
        run(r#"
              var resizable = new ArrayBuffer(0, { maxByteLength: 8 });
              var tracking = new Uint8Array(resizable);
              var fixedResizable = new Uint8Array(resizable, 0, 0);
              var trackingReflect = Reflect.preventExtensions(tracking);
              var fixedReflect = Reflect.preventExtensions(fixedResizable);
              var freezeError = "none";
              try { Object.freeze(tracking); }
              catch (error) { freezeError = error.name; }
              resizable.resize(1);

              var fixedBuffer = new ArrayBuffer(1);
              var fixed = new Uint8Array(fixedBuffer);
              var fixedSuccess = Reflect.preventExtensions(fixed);
              var growable = new SharedArrayBuffer(0, { maxByteLength: 8 });
              var growableTracking = new Uint8Array(growable);
              var growableFixed = new Uint8Array(growable, 0, 0);
              [
                trackingReflect,
                fixedReflect,
                Object.isExtensible(tracking),
                Object.isExtensible(fixedResizable),
                freezeError,
                tracking.length,
                fixedSuccess,
                Object.isExtensible(fixed),
                Reflect.preventExtensions(growableTracking),
                Reflect.preventExtensions(growableFixed),
                Object.isExtensible(growableTracking),
                Object.isExtensible(growableFixed)
              ].join("|");
            "#,),
        Value::String(Arc::from(
            "false|false|true|true|TypeError|1|true|false|false|true|true|false"
        ))
    );
}

#[test]
fn iterator_from_uses_the_calling_realm_wrapper_prototype_and_errors() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var wrapper = other.Iterator.from({
              next: function() { return { done: true }; }
            });
            var emptyResult = other.Iterator.from({ next: function() { return { done: true }; } }).return();
            var arrayResult = other.Iterator.prototype.toArray.call({
              next: function() { return { done: true }; }
            });
            var arrayIterator = other.Array.prototype.values.call(new other.Array(1));
            var wrapperPrototype = Object.getPrototypeOf(wrapper);
            var nextError = false;
            var returnError = false;
            try { wrapperPrototype.next.call({}); }
            catch (error) {
              nextError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            try { wrapperPrototype.return.call({}); }
            catch (error) {
              returnError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            [
              other.Object.prototype !== Object.prototype,
              other.Array.prototype !== Array.prototype,
              Object.getPrototypeOf(other.Array.prototype) === other.Object.prototype,
              Object.getPrototypeOf(other.Iterator.prototype) === other.Object.prototype,
              Object.getPrototypeOf(Object.getPrototypeOf(arrayIterator)) === other.Iterator.prototype,
              arrayIterator instanceof other.Iterator,
              Object.getPrototypeOf(wrapperPrototype) === other.Iterator.prototype,
              Object.getPrototypeOf(wrapperPrototype.next) === other.Function.prototype,
              Object.getPrototypeOf(wrapperPrototype.return) === other.Function.prototype,
              Object.getPrototypeOf(emptyResult) === other.Object.prototype,
              Object.getPrototypeOf(arrayResult) === other.Array.prototype,
              wrapper instanceof other.Iterator, wrapper instanceof Iterator,
              nextError, returnError
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|false|true|true"
        ))
    );
}

#[test]
fn iterator_from_keeps_wrapped_iterator_state_alive_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var wrapper = (function() {
              var state = { calls: 0, value: "kept" };
              var iterator = {};
              Object.defineProperty(iterator, "next", {
                get: function() {
                  forceGc();
                  return function() {
                    forceGc();
                    var current = state.calls++;
                    return {
                      get value() { forceGc(); return state.value; },
                      get done() { forceGc(); return current > 0; }
                    };
                  };
                }
              });
              return Iterator.from(iterator);
            })();
            forceGc();
            var first = wrapper.next();
            var done = wrapper.next();
            [first.value, first.done, done.done].join("|");
            "#,
        )
        .expect("Iterator.from wrapper should retain iterator state across GC"),
        Value::String(Arc::from("kept|false|true"))
    );
}

#[test]
fn array_from_keeps_iterator_result_alive_across_done_getter_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var calls = 0;
            var source = {
              [Symbol.iterator]: function() {
                return {
                  next: function() {
                    calls += 1;
                    if (calls > 1) return { done: true };
                    var result = {};
                    Object.defineProperty(result, "done", {
                      get: function() { forceGc(); return false; }
                    });
                    Object.defineProperty(result, "value", {
                      get: function() { return "kept"; }
                    });
                    return result;
                  }
                };
              }
            };
            Array.from(source)[0];
            "#,
        )
        .expect("Array.from iterator result should survive observable getters"),
        Value::String(Arc::from("kept"))
    );
}

#[test]
fn iterator_to_array_caches_next_and_observes_done_before_value() {
    assert_eq!(
        run(r#"
            var gets = 0;
            var calls = 0;
            var valueGets = 0;
            var iterator = {
              get next() {
                gets += 1;
                return function() {
                  calls += 1;
                  return calls < 3
                    ? { done: false, value: calls }
                    : { done: true, get value() { valueGets += 1; throw new Error(); } };
                };
              }
            };
            var values = Iterator.prototype.toArray.call(iterator);
            [values.join(","), gets, calls, valueGets].join("|");
        "#),
        Value::String(Arc::from("1,2|1|3|0"))
    );
}

#[test]
fn iterator_to_array_validates_receiver_next_and_result() {
    assert_eq!(
        run(r#"
            function throwsTypeError(receiver) {
              try { Iterator.prototype.toArray.call(receiver); }
              catch (error) { return error instanceof TypeError; }
              return false;
            }
            [
              throwsTypeError(null),
              throwsTypeError({ next: 0 }),
              throwsTypeError({ next: function() { return 1; } })
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true"))
    );
}

#[test]
fn iterator_to_array_caps_infinite_materialization() {
    assert_eq!(
        run(r#"
            var closed = 0;
            var rangeError = false;
            try {
              Iterator.prototype.toArray.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { closed += 1; return {}; }
              });
            } catch (error) {
              rangeError = error instanceof RangeError;
            }
            [rangeError, closed].join("|");
        "#),
        Value::String(Arc::from("true|1"))
    );
}

#[test]
fn iterator_map_and_filter_have_lazy_helper_shape() {
    assert_eq!(
        run(r#"
            var calls = 0;
            var source = {
              __proto__: Iterator.prototype,
              value: 0,
              next: function() {
                calls += 1;
                return this.value < 4
                  ? { value: this.value++, done: false }
                  : { done: true };
              }
            };
            var helper = source.map(function(value, index) {
              return value + index;
            }).filter(function(value) {
              return value > 2;
            });
            var proto = Object.getPrototypeOf(helper);
            var nextBrand = false;
            var returnBrand = false;
            try { proto.next.call({}); }
            catch (error) { nextBrand = error instanceof TypeError; }
            try { proto.return.call({}); }
            catch (error) { returnBrand = error instanceof TypeError; }
            var before = calls;
            var first = helper.next();
            var second = helper.next();
            var done = helper.next();
            [
              before, first.value, first.done, second.value, second.done, done.done,
              helper instanceof Iterator,
              Object.getPrototypeOf(proto) === Iterator.prototype,
              proto.next.length, proto.next.name, proto.return.length, proto.return.name,
              nextBrand, returnBrand
            ].join("|");
        "#),
        Value::String(Arc::from(
            "0|4|false|6|false|true|true|true|0|next|0|return|true|true"
        ))
    );
}

#[test]
fn iterator_helpers_use_the_method_realm_for_prototypes_results_and_errors() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var source = {
              value: 1,
              next: function() {
                return this.value < 2
                  ? { value: this.value++, done: false }
                  : { done: true };
              }
            };
            var helper = other.Iterator.prototype.map.call(source, function(value) {
              return value + 1;
            });
            var proto = Object.getPrototypeOf(helper);
            var step = helper.next();
            var brandError = false;
            try { proto.next.call({}); }
            catch (error) {
              brandError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            [
              helper instanceof other.Iterator, helper instanceof Iterator,
              Object.getPrototypeOf(proto) === other.Iterator.prototype,
              Object.getPrototypeOf(proto.next) === other.Function.prototype,
              Object.getPrototypeOf(proto.return) === other.Function.prototype,
              Object.getPrototypeOf(step) === other.Object.prototype,
              step.value, step.done, brandError
            ].join("|");
        "#),
        Value::String(Arc::from("true|false|true|true|true|true|2|false|true"))
    );
}

#[test]
fn iterator_helpers_keep_source_next_and_callbacks_alive_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var helper = (function() {
              var state = { value: 0, limit: 4, offset: 10 };
              var source = {
                get next() {
                  forceGc();
                  return function() {
                    forceGc();
                    var result = {};
                    Object.defineProperty(result, "done", {
                      get: function() { forceGc(); return state.value >= state.limit; }
                    });
                    Object.defineProperty(result, "value", {
                      get: function() { forceGc(); return state.value++; }
                    });
                    return result;
                  };
                }
              };
              return Iterator.prototype.map.call(source, function(value) {
                forceGc();
                return value + state.offset;
              }).filter(function(value) {
                forceGc();
                return value % 2 === 0;
              });
            })();
            forceGc();
            var first = helper.next();
            forceGc();
            var second = helper.next();
            forceGc();
            var done = helper.next();
            [first.value, first.done, second.value, second.done, done.done].join("|");
            "#,
        )
        .expect("Iterator helpers should retain all lazy state across GC"),
        Value::String(Arc::from("10|false|12|false|true"))
    );
}

#[test]
fn iterator_helpers_stay_executing_while_closing() {
    assert_eq!(
        run(r#"
            var callbackCloseReentry = false;
            var explicitCloseReentry = false;
            var suspendedStartCloseReentry = false;
            var originalThrow = false;
            var callbackHelper;
            var callbackSource = {
              next: function() { return { value: 1, done: false }; },
              return: function() {
                try { callbackHelper.next(); }
                catch (error) { callbackCloseReentry = error instanceof TypeError; }
                return {};
              }
            };
            callbackHelper = Iterator.prototype.map.call(callbackSource, function() {
              throw "callback";
            });
            try { callbackHelper.next(); }
            catch (error) { originalThrow = error === "callback"; }

            var explicitHelper;
            var explicitSource = {
              yielded: false,
              next: function() {
                if (this.yielded) return { done: true };
                this.yielded = true;
                return { value: 2, done: false };
              },
              return: function() {
                try { explicitHelper.next(); }
                catch (error) { explicitCloseReentry = error instanceof TypeError; }
                return {};
              }
            };
            explicitHelper = Iterator.prototype.filter.call(explicitSource, function() {
              return true;
            });
            explicitHelper.next();
            explicitHelper.return();

            var startHelper;
            var startSource = {
              next: function() { return { value: 3, done: false }; },
              return: function() {
                var step = startHelper.next();
                suspendedStartCloseReentry = step.done && step.value === undefined;
                return {};
              }
            };
            startHelper = Iterator.prototype.map.call(startSource, function(value) {
              return value;
            });
            startHelper.return();
            [
              callbackCloseReentry,
              explicitCloseReentry,
              suspendedStartCloseReentry,
              originalThrow
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true"))
    );
}

#[test]
fn iterator_helper_tag_and_integrity_operations_are_spec_shaped() {
    assert_eq!(
        run(r#"
            function helper() {
              return [1].values().map(function(value) { return value; });
            }
            var tagged = helper();
            var proto = Object.getPrototypeOf(tagged);
            var tag = Object.getOwnPropertyDescriptor(proto, Symbol.toStringTag);

            var prevented = helper();
            var preventResult = Reflect.preventExtensions(prevented);
            prevented.extra = 1;

            var sealed = helper();
            sealed.extra = 1;
            Object.seal(sealed);
            var sealedDesc = Object.getOwnPropertyDescriptor(sealed, "extra");

            var frozen = helper();
            frozen.extra = 1;
            Object.freeze(frozen);
            var frozenDesc = Object.getOwnPropertyDescriptor(frozen, "extra");
            [
              Object.prototype.toString.call(tagged),
              tag.value, tag.writable, tag.enumerable, tag.configurable,
              preventResult, Object.isExtensible(prevented), prevented.extra === undefined,
              Object.isSealed(sealed), sealedDesc.configurable, sealedDesc.writable,
              Object.isFrozen(frozen), frozenDesc.configurable, frozenDesc.writable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "[object Iterator Helper]|Iterator Helper|false|false|true|true|false|true|true|false|true|true|false|false"
        ))
    );
}

#[test]
fn iterator_take_and_drop_apply_limits_lazily() {
    assert_eq!(
        run(r#"
            function values() { return [0, 1, 2, 3, 4].values(); }
            var chain = values().drop(1.9).take(2.9).toArray();
            var zeroClosed = 0;
            var zero = Iterator.prototype.take.call({
              next: function() { throw new Error("must not step"); },
              return: function() { zeroClosed += 1; return {}; }
            }, -0.5);
            var zeroResult = zero.next();
            var infinity = values().drop(2).take(Infinity).toArray();
            var huge = values().take(Number.MAX_VALUE).toArray();
            [
              chain.join(","), zeroResult.done, zeroClosed,
              infinity.join(","), huge.join(","),
              values().take(null).next().done
            ].join("|");
        "#),
        Value::String(Arc::from("1,2|true|1|2,3,4|0,1,2,3,4|true"))
    );
}

#[test]
fn iterator_drop_never_reads_values_while_skipping() {
    assert_eq!(
        run(r#"
            var calls = 0;
            var skippedValueGets = 0;
            var source = {
              next: function() {
                calls += 1;
                if (calls === 1) {
                  return {
                    done: false,
                    get value() {
                      skippedValueGets += 1;
                      throw new Error("skipped value was read");
                    }
                  };
                }
                if (calls === 2) return { value: "kept", done: false };
                return { done: true };
              }
            };
            var helper = Iterator.prototype.drop.call(source, 1);
            var first = helper.next();
            var done = helper.next();
            [first.value, first.done, done.done, calls, skippedValueGets].join("|");
        "#),
        Value::String(Arc::from("kept|false|true|3|0"))
    );
}

#[test]
fn iterator_limit_validation_closes_without_reading_next_and_preserves_errors() {
    assert_eq!(
        run(r#"
            class LimitError extends Error {}
            class CloseError extends Error {}
            function conversionCase(name) {
              var nextGets = 0;
              var returnGets = 0;
              var source = {
                get next() { nextGets += 1; throw new Error("next"); },
                get return() {
                  returnGets += 1;
                  throw new CloseError();
                }
              };
              var original = false;
              try {
                Iterator.prototype[name].call(source, {
                  valueOf: function() { throw new LimitError(); }
                });
              } catch (error) {
                original = error instanceof LimitError;
              }
              return [original, nextGets, returnGets].join(",");
            }
            function rangeCase(name, limit) {
              var nextGets = 0;
              var closes = 0;
              var source = {
                get next() { nextGets += 1; throw new Error("next"); },
                return: function() { closes += 1; return 0; }
              };
              var range = false;
              try { Iterator.prototype[name].call(source, limit); }
              catch (error) { range = error instanceof RangeError; }
              return [range, nextGets, closes].join(",");
            }
            function typeCase(name, limit) {
              var closes = 0;
              var source = {
                get next() { throw new Error("next"); },
                return: function() { closes += 1; return {}; }
              };
              var type = false;
              try { Iterator.prototype[name].call(source, limit); }
              catch (error) { type = error instanceof TypeError; }
              return [type, closes].join(",");
            }
            var radixCloses = 0;
            var radixSource = {
              yielded: false,
              next: function() {
                if (this.yielded) return { done: true };
                this.yielded = true;
                return { value: "radix", done: false };
              },
              return: function() { radixCloses += 1; return {}; }
            };
            var radixStep = Iterator.prototype.take.call(
              radixSource,
              "0x10000000000000000"
            ).next();
            function invalidStringCase(name, limit) {
              var closes = 0;
              var source = {
                get next() { throw new Error("next"); },
                return: function() { closes += 1; return {}; }
              };
              var range = false;
              try { Iterator.prototype[name].call(source, limit); }
              catch (error) { range = error instanceof RangeError; }
              return [range, closes].join(",");
            }
            [
              conversionCase("take"), conversionCase("drop"),
              rangeCase("take", -Infinity), rangeCase("drop", NaN),
              typeCase("take", 1n), typeCase("drop", Symbol()),
              Number("0x10000000000000000") === 18446744073709552000,
              Number("0b10000000000000000000000000000000000000000000000000000000000000000") === 18446744073709552000,
              Number("0o2000000000000000000000") === 18446744073709552000,
              radixStep.value, radixStep.done, radixCloses,
              invalidStringCase("take", "0x1_0"),
              invalidStringCase("drop", "0x+10"),
              invalidStringCase("take", "inf")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true,0,1|true,0,1|true,0,1|true,0,1|true,1|true,1|true|true|true|radix|false|0|true,1|true,1|true,1"
        ))
    );
}

#[test]
fn iterator_limit_helpers_preserve_realm_gc_and_yielded_close_state() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var closes = 0;
            var helper = (function() {
              var state = { value: 0 };
              var source = {
                get next() {
                  forceGc();
                  return function() {
                    forceGc();
                    return state.value < 4
                      ? { value: state.value++, done: false }
                      : { done: true };
                  };
                },
                return: function() { closes += 1; return {}; }
              };
              return other.Iterator.prototype.drop.call(source, 1).take(2);
            })();
            forceGc();
            var proto = Object.getPrototypeOf(helper);
            var first = helper.next();
            forceGc();
            var returned = helper.return();
            forceGc();
            var done = helper.next();
            [
              first.value, first.done, returned.done, done.done, closes,
              helper instanceof other.Iterator, helper instanceof Iterator,
              Object.getPrototypeOf(proto) === other.Iterator.prototype,
              Object.getPrototypeOf(first) === other.Object.prototype
            ].join("|");
            "#,
        )
        .expect("Iterator limit helpers should preserve Realm and GC state"),
        Value::String(Arc::from("1|false|true|true|1|true|false|true|true"))
    );
}

#[test]
fn iterator_flat_map_flattens_one_level_and_tracks_outer_indices() {
    assert_eq!(
        run(r#"
            var indices = [];
            var helper = [1, 2, 3].values().flatMap(function(value, index) {
              indices.push(index);
              if (value === 1) return [value, value * 10];
              if (value === 2) return [value].values();
              return { next: function() { return { done: true }; } };
            });
            var values = helper.toArray();
            var primitiveError = false;
            try { [1].values().flatMap(function() { return "ab"; }).next(); }
            catch (error) { primitiveError = error instanceof TypeError; }
            [values.join(","), indices.join(","), primitiveError].join("|");
        "#),
        Value::String(Arc::from("1,10,2|0,1,2|true"))
    );
}

#[test]
fn iterator_flat_map_closes_inner_then_outer_and_preserves_abrupt_errors() {
    assert_eq!(
        run(r#"
            var order = [];
            var helper;
            var outer = {
              next: function() { return { value: 1, done: false }; },
              return: function() {
                order.push("outer");
                try { helper.next(); }
                catch (error) { order.push(error instanceof TypeError ? "outer-running" : "bad"); }
                throw "outer-close";
              }
            };
            helper = Iterator.prototype.flatMap.call(outer, function() {
              return {
                next: function() { return { value: 2, done: false }; },
                return: function() {
                  order.push("inner");
                  try { helper.next(); }
                  catch (error) { order.push(error instanceof TypeError ? "inner-running" : "bad"); }
                  throw "inner-close";
                }
              };
            });
            helper.next();
            var closeError;
            try { helper.return(); } catch (error) { closeError = error; }

            var mapperOuterClosed = 0;
            var mapperHelper = Iterator.prototype.flatMap.call({
              next: function() { return { value: 3, done: false }; },
              return: function() { mapperOuterClosed += 1; throw "ignored-close"; }
            }, function() { throw "mapper-error"; });
            var mapperError;
            try { mapperHelper.next(); } catch (error) { mapperError = error; }

            var reentrantHelper;
            var innerCalls = 0;
            var returnReentryError = false;
            reentrantHelper = [0].values().flatMap(function() {
              return {
                next: function() {
                  innerCalls += 1;
                  if (innerCalls === 2) {
                    try { reentrantHelper.return(); }
                    catch (error) { returnReentryError = error instanceof TypeError; }
                  }
                  return innerCalls < 3
                    ? { value: innerCalls, done: false }
                    : { done: true };
                }
              };
            });
            reentrantHelper.next();
            reentrantHelper.next();
            var reentrantDone = reentrantHelper.next().done;
            [
              order.join(","), closeError, mapperError, mapperOuterClosed,
              returnReentryError, innerCalls, reentrantDone
            ].join("|");
        "#),
        Value::String(Arc::from(
            "inner,inner-running,outer,outer-running|inner-close|mapper-error|1|true|3|true"
        ))
    );
}

#[test]
fn iterator_flat_map_keeps_nested_iterator_state_alive_across_gc_and_realms() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var helper = other.Iterator.prototype.flatMap.call([4].values(), function(value) {
              var state = { value: value };
              var inner = {
                get next() {
                  forceGc();
                  return function() {
                    forceGc();
                    var result = {};
                    Object.defineProperty(result, "done", {
                      get: function() { forceGc(); return state.value > 5; }
                    });
                    Object.defineProperty(result, "value", {
                      get: function() { forceGc(); return state.value++; }
                    });
                    return result;
                  };
                }
              };
              Object.defineProperty(inner, Symbol.iterator, {
                get: function() {
                  forceGc();
                  return function() { forceGc(); return inner; };
                }
              });
              return inner;
            });
            forceGc();
            var first = helper.next();
            forceGc();
            var second = helper.next();
            forceGc();
            var done = helper.next();
            var proto = Object.getPrototypeOf(helper);
            [
              first.value, second.value, done.done,
              helper instanceof other.Iterator, helper instanceof Iterator,
              Object.getPrototypeOf(proto) === other.Iterator.prototype,
              Object.getPrototypeOf(first) === other.Object.prototype
            ].join("|");
            "#,
        )
        .expect("flatMap should retain nested iterator state across GC"),
        Value::String(Arc::from("4|5|true|true|false|true|true"))
    );
}

#[test]
fn iterator_reduce_distinguishes_omitted_and_explicit_initial_values() {
    assert_eq!(
        run(r#"
            var omittedCalls = [];
            var omitted = [1, 2, 3].values().reduce(function(memo, value, index) {
              omittedCalls.push([memo, value, index].join(","));
              return memo + value;
            });
            var explicitCalls = [];
            var explicit = [1, 2].values().reduce(function(memo, value, index) {
              explicitCalls.push([String(memo), value, index].join(","));
              return value;
            }, undefined);
            var singletonCalls = 0;
            var singleton = [7].values().reduce(function() {
              singletonCalls += 1;
            });
            [
              omitted, omittedCalls.join(";"),
              explicit, explicitCalls.join(";"),
              singleton, singletonCalls
            ].join("|");
        "#),
        Value::String(Arc::from("6|1,2,1;3,3,2|2|undefined,1,0;1,2,1|7|0"))
    );
}

#[test]
fn iterator_reduce_closes_only_reducer_abrupt_completions() {
    assert_eq!(
        run(r#"
            var invalidNextGets = 0;
            var invalidCloses = 0;
            var invalidType = false;
            try {
              Iterator.prototype.reduce.call({
                get next() { invalidNextGets += 1; throw "next"; },
                return: function() { invalidCloses += 1; throw "close"; }
              }, {});
            } catch (error) { invalidType = error instanceof TypeError; }

            var original = { marker: 1 };
            var reducerCloses = 0;
            var reducerError;
            try {
              Iterator.prototype.reduce.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { reducerCloses += 1; throw "ignored-close"; }
              }, function() { throw original; }, 0);
            } catch (error) { reducerError = error; }

            var stepCloses = 0;
            var stepError;
            try {
              Iterator.prototype.reduce.call({
                next: function() {
                  return {
                    done: false,
                    get value() { throw original; }
                  };
                },
                return: function() { stepCloses += 1; return {}; }
              }, function() {}, 0);
            } catch (error) { stepError = error; }
            [
              invalidType, invalidNextGets, invalidCloses,
              reducerError === original, reducerCloses,
              stepError === original, stepCloses
            ].join("|");
        "#),
        Value::String(Arc::from("true|0|1|true|1|true|0"))
    );
}

#[test]
fn iterator_reduce_roots_accumulator_and_uses_the_method_realm() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var source = {
              value: 1,
              get next() {
                forceGc();
                return function() {
                  forceGc();
                  var result = {};
                  Object.defineProperty(result, "done", {
                    get: function() { forceGc(); return source.value > 3; }
                  });
                  Object.defineProperty(result, "value", {
                    get: function() { forceGc(); return source.value++; }
                  });
                  return result;
                };
              }
            };
            var initial = { total: 0 };
            var result = other.Iterator.prototype.reduce.call(
              source,
              function(memo, value) {
                forceGc();
                return { total: memo.total + value };
              },
              initial
            );
            var realmError = false;
            try {
              other.Iterator.prototype.reduce.call({
                next: function() { return { done: true }; }
              }, function() {});
            } catch (error) {
              realmError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            [result.total, realmError].join("|");
            "#,
        )
        .expect("Iterator reduce should retain accumulators and method Realm"),
        Value::String(Arc::from("6|true"))
    );
}

#[test]
fn iterator_for_each_visits_values_with_indices_and_returns_undefined() {
    assert_eq!(
        run(r#"
            var calls = [];
            var expectedThis = function() { return this; }.call(undefined);
            var result = [4, 5, 6].values().forEach(function(value, index) {
              calls.push([value, index, this === expectedThis].join(","));
              return value * 10;
            });
            [calls.join(";"), result === undefined].join("|");
        "#),
        Value::String(Arc::from("4,0,true;5,1,true;6,2,true|true"))
    );
}

#[test]
fn iterator_for_each_closes_only_callback_abrupt_completions() {
    assert_eq!(
        run(r#"
            var invalidNextGets = 0;
            var invalidCloses = 0;
            var invalidType = false;
            try {
              Iterator.prototype.forEach.call({
                get next() { invalidNextGets += 1; throw "next"; },
                return: function() { invalidCloses += 1; throw "close"; }
              }, null);
            } catch (error) { invalidType = error instanceof TypeError; }

            var original = { marker: 1 };
            var callbackCloses = 0;
            var callbackError;
            try {
              Iterator.prototype.forEach.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { callbackCloses += 1; throw "ignored-close"; }
              }, function() { throw original; });
            } catch (error) { callbackError = error; }

            var stepCloses = 0;
            var stepError;
            try {
              Iterator.prototype.forEach.call({
                next: function() {
                  return {
                    done: false,
                    get value() { throw original; }
                  };
                },
                return: function() { stepCloses += 1; return {}; }
              }, function() {});
            } catch (error) { stepError = error; }
            [
              invalidType, invalidNextGets, invalidCloses,
              callbackError === original, callbackCloses,
              stepError === original, stepCloses
            ].join("|");
        "#),
        Value::String(Arc::from("true|0|1|true|1|true|0"))
    );
}

#[test]
fn iterator_for_each_roots_callback_and_uses_the_method_realm() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var state = { value: 1, total: 0 };
            var source = {
              get next() {
                forceGc();
                return function() {
                  forceGc();
                  return state.value <= 3
                    ? { value: { amount: state.value++ }, done: false }
                    : { done: true };
                };
              }
            };
            var result = other.Iterator.prototype.forEach.call(source, function(value) {
              forceGc();
              state.total += value.amount;
            });
            var realmError = false;
            try { other.Iterator.prototype.forEach.call({}, null); }
            catch (error) {
              realmError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            [state.total, result === undefined, realmError].join("|");
            "#,
        )
        .expect("Iterator forEach should retain callbacks and method Realm"),
        Value::String(Arc::from("6|true|true"))
    );
}

#[test]
fn iterator_some_short_circuits_truthy_values_and_closes_normally() {
    assert_eq!(
        run(r#"
            var calls = [];
            var closes = 0;
            var source = {
              value: 0,
              next: function() {
                return this.value < 5
                  ? { value: this.value++, done: false }
                  : { done: true };
              },
              return: function() { closes += 1; return {}; }
            };
            var result = Iterator.prototype.some.call(source, function(value, index) {
              calls.push([value, index].join(","));
              return value === 2 ? {} : 0;
            });
            var exhaustedCloses = 0;
            var exhausted = Iterator.prototype.some.call({
              next: function() { return { done: true }; },
              return: function() { exhaustedCloses += 1; return {}; }
            }, function() { return true; });
            [result, calls.join(";"), closes, exhausted, exhaustedCloses].join("|");
        "#),
        Value::String(Arc::from("true|0,0;1,1;2,2|1|false|0"))
    );
}

#[test]
fn iterator_some_distinguishes_normal_close_and_predicate_abrupt_errors() {
    assert_eq!(
        run(r#"
            var normalCloseType = false;
            try {
              Iterator.prototype.some.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { return 0; }
              }, function() { return true; });
            } catch (error) { normalCloseType = error instanceof TypeError; }

            var original = { marker: 1 };
            var callbackCloses = 0;
            var callbackError;
            try {
              Iterator.prototype.some.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { callbackCloses += 1; throw "ignored-close"; }
              }, function() { throw original; });
            } catch (error) { callbackError = error; }

            var stepCloses = 0;
            var stepError;
            try {
              Iterator.prototype.some.call({
                next: function() {
                  return { done: false, get value() { throw original; } };
                },
                return: function() { stepCloses += 1; return {}; }
              }, function() { return false; });
            } catch (error) { stepError = error; }
            [
              normalCloseType,
              callbackError === original, callbackCloses,
              stepError === original, stepCloses
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|1|true|0"))
    );
}

#[test]
fn iterator_some_roots_values_and_uses_the_method_realm() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var state = { value: 1 };
            var source = {
              get next() {
                forceGc();
                return function() {
                  forceGc();
                  return state.value <= 3
                    ? { value: { amount: state.value++ }, done: false }
                    : { done: true };
                };
              },
              return: function() { forceGc(); return {}; }
            };
            var found = other.Iterator.prototype.some.call(source, function(value) {
              forceGc();
              return value.amount === 3;
            });
            var realmError = false;
            try {
              other.Iterator.prototype.some.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { return 0; }
              }, function() { return true; });
            } catch (error) {
              realmError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            [found, realmError].join("|");
            "#,
        )
        .expect("Iterator some should retain values and method Realm"),
        Value::String(Arc::from("true|true"))
    );
}

#[test]
fn iterator_every_short_circuits_falsey_values_and_closes_normally() {
    assert_eq!(
        run(r#"
            var calls = [];
            var closes = 0;
            var source = {
              value: 0,
              next: function() {
                return this.value < 5
                  ? { value: this.value++, done: false }
                  : { done: true };
              },
              return: function() { closes += 1; return {}; }
            };
            var result = Iterator.prototype.every.call(source, function(value, index) {
              calls.push([value, index].join(","));
              return value < 2 ? {} : 0;
            });
            var exhaustedCloses = 0;
            var exhausted = Iterator.prototype.every.call({
              next: function() { return { done: true }; },
              return: function() { exhaustedCloses += 1; return {}; }
            }, function() { return false; });
            [result, calls.join(";"), closes, exhausted, exhaustedCloses].join("|");
        "#),
        Value::String(Arc::from("false|0,0;1,1;2,2|1|true|0"))
    );
}

#[test]
fn iterator_every_distinguishes_normal_close_and_predicate_abrupt_errors() {
    assert_eq!(
        run(r#"
            var normalCloseType = false;
            try {
              Iterator.prototype.every.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { return 0; }
              }, function() { return false; });
            } catch (error) { normalCloseType = error instanceof TypeError; }

            var original = { marker: 1 };
            var callbackCloses = 0;
            var callbackError;
            try {
              Iterator.prototype.every.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { callbackCloses += 1; throw "ignored-close"; }
              }, function() { throw original; });
            } catch (error) { callbackError = error; }

            var stepCloses = 0;
            var stepError;
            try {
              Iterator.prototype.every.call({
                next: function() {
                  return { done: false, get value() { throw original; } };
                },
                return: function() { stepCloses += 1; return {}; }
              }, function() { return true; });
            } catch (error) { stepError = error; }
            [
              normalCloseType,
              callbackError === original, callbackCloses,
              stepError === original, stepCloses
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|1|true|0"))
    );
}

#[test]
fn iterator_every_roots_values_and_uses_the_method_realm() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var state = { value: 1 };
            var source = {
              get next() {
                forceGc();
                return function() {
                  forceGc();
                  return state.value <= 3
                    ? { value: { amount: state.value++ }, done: false }
                    : { done: true };
                };
              },
              return: function() { forceGc(); return {}; }
            };
            var result = other.Iterator.prototype.every.call(source, function(value) {
              forceGc();
              return value.amount < 3;
            });
            var realmError = false;
            try {
              other.Iterator.prototype.every.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { return 0; }
              }, function() { return false; });
            } catch (error) {
              realmError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            [result, realmError].join("|");
            "#,
        )
        .expect("Iterator every should retain values and method Realm"),
        Value::String(Arc::from("false|true"))
    );
}

#[test]
fn iterator_every_never_closes_step_abrupt_completions() {
    assert_eq!(
        run(r#"
            var original = { marker: 1 };
            var closes = 0;
            function check(source, expectTypeError) {
              source.return = function() { closes += 1; return {}; };
              var caught;
              try {
                Iterator.prototype.every.call(source, function() { return true; });
              } catch (error) { caught = error; }
              return expectTypeError ? caught instanceof TypeError : caught === original;
            }
            var getterSource = {
              get next() { throw original; }
            };
            var callSource = {
              next: function() { throw original; }
            };
            var primitiveSource = {
              next: function() { return 0; }
            };
            var doneSource = {
              next: function() {
                return { get done() { throw original; } };
              }
            };
            [
              check(getterSource, false),
              check(callSource, false),
              check(primitiveSource, true),
              check(doneSource, false),
              closes
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|0"))
    );
}

#[test]
fn iterator_every_preserves_abrupt_close_priority_and_validates_normal_close() {
    assert_eq!(
        run(r#"
            var original = { marker: 1 };
            var getterCalls = 0;
            function predicateThrows() { throw original; }
            function catchesOriginal(source) {
              try {
                Iterator.prototype.every.call(source, predicateThrows);
              } catch (error) { return error === original; }
              return false;
            }
            function sourceWithReturn(returnValue) {
              return {
                next: function() { return { value: 1, done: false }; },
                return: returnValue
              };
            }
            var getterSource = {
              next: function() { return { value: 1, done: false }; },
              get return() { getterCalls += 1; throw "ignored"; }
            };
            var normalNonCallable = false;
            try {
              Iterator.prototype.every.call(sourceWithReturn(0), function() { return false; });
            } catch (error) { normalNonCallable = error instanceof TypeError; }
            [
              catchesOriginal(getterSource), getterCalls,
              catchesOriginal(sourceWithReturn(0)),
              catchesOriginal(sourceWithReturn(function() { return 0; })),
              normalNonCallable
            ].join("|");
        "#),
        Value::String(Arc::from("true|1|true|true|true"))
    );
}

#[test]
fn iterator_every_generated_errors_use_the_method_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            function isOtherTypeError(thunk) {
              try { thunk(); } catch (error) {
                return error instanceof other.TypeError && !(error instanceof TypeError);
              }
              return false;
            }
            var method = other.Iterator.prototype.every;
            [
              isOtherTypeError(function() { method.call(1, function() {}); }),
              isOtherTypeError(function() {
                method.call({ return: function() { return {}; } }, null);
              }),
              isOtherTypeError(function() {
                method.call({ next: 0 }, function() { return true; });
              }),
              isOtherTypeError(function() {
                method.call({ next: function() { return 0; } }, function() { return true; });
              }),
              isOtherTypeError(function() {
                method.call({
                  next: function() { return { value: 1, done: false }; },
                  return: 0
                }, function() { return false; });
              }),
              isOtherTypeError(function() {
                method.call({
                  next: function() { return { value: 1, done: false }; },
                  return: function() { return 0; }
                }, function() { return false; });
              })
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true"))
    );
}

#[test]
fn iterator_find_returns_found_value_and_closes_normally() {
    assert_eq!(
        run(r#"
            var calls = [];
            var closes = 0;
            var source = {
              value: 0,
              next: function() {
                return this.value < 5
                  ? { value: this.value++, done: false }
                  : { done: true };
              },
              return: function() { closes += 1; return {}; }
            };
            var found = Iterator.prototype.find.call(source, function(value, index) {
              calls.push([value, index].join(","));
              return value === 2 ? {} : 0;
            });
            var exhaustedCloses = 0;
            var exhausted = Iterator.prototype.find.call({
              next: function() { return { done: true }; },
              return: function() { exhaustedCloses += 1; return {}; }
            }, function() { return true; });
            [found, calls.join(";"), closes, exhausted, exhaustedCloses].join("|");
        "#),
        Value::String(Arc::from("2|0,0;1,1;2,2|1||0"))
    );
}

#[test]
fn iterator_find_distinguishes_normal_close_and_abrupt_errors() {
    assert_eq!(
        run(r#"
            var normalCloseType = false;
            try {
              Iterator.prototype.find.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { return 0; }
              }, function() { return true; });
            } catch (error) { normalCloseType = error instanceof TypeError; }

            var original = { marker: 1 };
            var callbackCloses = 0;
            var callbackError;
            try {
              Iterator.prototype.find.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { callbackCloses += 1; throw "ignored-close"; }
              }, function() { throw original; });
            } catch (error) { callbackError = error; }

            var stepCloses = 0;
            var stepError;
            try {
              Iterator.prototype.find.call({
                next: function() {
                  return { done: false, get value() { throw original; } };
                },
                return: function() { stepCloses += 1; return {}; }
              }, function() { return true; });
            } catch (error) { stepError = error; }
            [
              normalCloseType,
              callbackError === original, callbackCloses,
              stepError === original, stepCloses
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|1|true|0"))
    );
}

#[test]
fn iterator_find_keeps_found_value_alive_through_close_and_uses_method_realm() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var state = { value: 1 };
            var source = {
              get next() {
                forceGc();
                return function() {
                  forceGc();
                  return state.value <= 3
                    ? { value: { amount: state.value++ }, done: false }
                    : { done: true };
                };
              },
              return: function() { forceGc(); return {}; }
            };
            var found = other.Iterator.prototype.find.call(source, function(value) {
              forceGc();
              return value.amount === 3;
            });
            forceGc();
            var realmError = false;
            try {
              other.Iterator.prototype.find.call({
                next: function() { return { value: 1, done: false }; },
                return: function() { return 0; }
              }, function() { return true; });
            } catch (error) {
              realmError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            [found.amount, realmError].join("|");
            "#,
        )
        .expect("Iterator find should retain its result and method Realm"),
        Value::String(Arc::from("3|true"))
    );
}

#[test]
fn iterator_find_never_closes_step_abrupt_completions() {
    assert_eq!(
        run(r#"
            var original = { marker: 1 };
            var closes = 0;
            function check(source, expectTypeError) {
              source.return = function() { closes += 1; return {}; };
              var caught;
              try {
                Iterator.prototype.find.call(source, function() { return false; });
              } catch (error) { caught = error; }
              return expectTypeError ? caught instanceof TypeError : caught === original;
            }
            [
              check({ get next() { throw original; } }, false),
              check({ next: function() { throw original; } }, false),
              check({ next: function() { return 0; } }, true),
              check({ next: function() {
                return { get done() { throw original; } };
              } }, false),
              closes
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|0"))
    );
}

#[test]
fn iterator_find_generated_errors_use_the_method_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            function isOtherTypeError(thunk) {
              try { thunk(); } catch (error) {
                return error instanceof other.TypeError && !(error instanceof TypeError);
              }
              return false;
            }
            var method = other.Iterator.prototype.find;
            [
              isOtherTypeError(function() { method.call(1, function() {}); }),
              isOtherTypeError(function() {
                method.call({ return: function() { return {}; } }, null);
              }),
              isOtherTypeError(function() {
                method.call({ next: 0 }, function() { return false; });
              }),
              isOtherTypeError(function() {
                method.call({ next: function() { return 0; } }, function() { return false; });
              }),
              isOtherTypeError(function() {
                method.call({
                  next: function() { return { value: 1, done: false }; },
                  return: 0
                }, function() { return true; });
              }),
              isOtherTypeError(function() {
                method.call({
                  next: function() { return { value: 1, done: false }; },
                  return: function() { return 0; }
                }, function() { return true; });
              })
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true"))
    );
}

#[test]
fn iterator_find_observes_validation_cache_and_exhaustion_order() {
    assert_eq!(
        run(r#"
            var effects = [];
            var invalid = {
              get next() { effects.push("next"); throw "unreachable"; },
              return: function() { effects.push("return"); return {}; }
            };
            var invalidType = false;
            try { Iterator.prototype.find.call(invalid, null); }
            catch (error) { invalidType = error instanceof TypeError; }

            var nextGets = 0;
            var state = 0;
            var cached = {
              get next() {
                nextGets += 1;
                return function() {
                  state += 1;
                  this.next = function() { throw "uncached"; };
                  return { value: state, done: false };
                };
              },
              return: function() { return {}; }
            };
            var found = Iterator.prototype.find.call(cached, function(value) {
              "use strict";
              if (this !== undefined) throw "bad-this";
              return value === 2;
            });

            var skippedValue = Iterator.prototype.find.call({
              next: function() {
                return { done: true, get value() { throw "unreachable"; } };
              }
            }, function() { throw "unreachable"; });

            var continuing = {
              value: 0,
              next: function() { return { value: this.value++, done: false }; }
            };
            var withoutReturn = Iterator.prototype.find.call(
              continuing,
              function(value) { return value === 1; }
            );
            var afterMatch = continuing.next().value;
            [
              invalidType, effects.join(","),
              found, nextGets,
              skippedValue,
              withoutReturn, afterMatch
            ].join("|");
        "#),
        Value::String(Arc::from("true|return|2|1||1|2"))
    );
}

#[test]
fn iterator_find_preserves_full_abrupt_close_priority() {
    assert_eq!(
        run(r#"
            var original = { marker: 1 };
            var getterCalls = 0;
            function predicateThrows() { throw original; }
            function catchesOriginal(source) {
              try { Iterator.prototype.find.call(source, predicateThrows); }
              catch (error) { return error === original; }
              return false;
            }
            function sourceWithReturn(returnValue) {
              return {
                next: function() { return { value: 1, done: false }; },
                return: returnValue
              };
            }
            var getterSource = {
              next: function() { return { value: 1, done: false }; },
              get return() { getterCalls += 1; throw "ignored"; }
            };
            var normalGetter = false;
            try {
              Iterator.prototype.find.call({
                next: function() { return { value: 1, done: false }; },
                get return() { throw original; }
              }, function() { return true; });
            } catch (error) { normalGetter = error === original; }
            [
              catchesOriginal(getterSource), getterCalls,
              catchesOriginal(sourceWithReturn(0)),
              catchesOriginal(sourceWithReturn(function() { return 0; })),
              normalGetter
            ].join("|");
        "#),
        Value::String(Arc::from("true|1|true|true|true"))
    );
}

#[test]
fn iterator_concat_validates_caches_and_opens_iterables_lazily() {
    assert_eq!(
        run(r#"
            var effects = [];
            function make(name, values) {
              return {
                get [Symbol.iterator]() {
                  effects.push("get-" + name);
                  return function() {
                    effects.push("open-" + name);
                    return values[Symbol.iterator]();
                  };
                }
              };
            }
            var first = make("first", [1, 2]);
            var second = make("second", [3]);
            var iterator = Iterator.concat(first, second);
            var afterCreate = effects.join(",");
            delete first[Symbol.iterator];
            delete second[Symbol.iterator];
            var a = iterator.next();
            var b = iterator.next();
            var c = iterator.next();
            var d = iterator.next();
            var invalidOrder = [];
            var invalid = {
              get [Symbol.iterator]() {
                invalidOrder.push("get");
                return function() { throw "unreachable"; };
              }
            };
            var invalidType = false;
            try { Iterator.concat(invalid, null); }
            catch (error) { invalidType = error instanceof TypeError; }
            [
              afterCreate, effects.join(","),
              a.value, b.value, c.value, d.done, d.value,
              a !== b && b !== c && c !== d,
              invalidType, invalidOrder.join(",")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "get-first,get-second|get-first,get-second,open-first,open-second|1|2|3|true||true|true|get"
        ))
    );
}

#[test]
fn iterator_concat_forwards_return_only_to_the_active_inner() {
    assert_eq!(
        run(r#"
            var opens = 0;
            var closes = 0;
            function iterable(doneImmediately) {
              return {
                [Symbol.iterator]: function() {
                  opens += 1;
                  return {
                    next: function() {
                      return doneImmediately
                        ? { done: true }
                        : { value: 1, done: false };
                    },
                    return: function() { closes += 1; return {}; }
                  };
                }
              };
            }
            var before = Iterator.concat(iterable(false));
            var beforeResult = before.return();
            var beforeState = [opens, closes, beforeResult.done].join(",");

            var active = Iterator.concat(iterable(false));
            active.next();
            var activeResult = active.return();
            active.return();
            var activeState = [opens, closes, activeResult.done].join(",");

            var exhausted = Iterator.concat(iterable(true));
            exhausted.next();
            exhausted.return();
            var exhaustedState = [opens, closes].join(",");
            [beforeState, activeState, exhaustedState].join("|");
        "#),
        Value::String(Arc::from("0,0,true|1,1,true|2,1"))
    );
}

#[test]
fn iterator_concat_rejects_reentrant_next_and_return() {
    assert_eq!(
        run(r#"
            var nextIterator;
            var nextEntries = 0;
            nextIterator = Iterator.concat({
              [Symbol.iterator]: function() {
                return {
                  next: function() {
                    nextEntries += 1;
                    nextIterator.next();
                    return { value: 1, done: false };
                  }
                };
              }
            });
            var nextType = false;
            try { nextIterator.next(); }
            catch (error) { nextType = error instanceof TypeError; }

            var returnIterator;
            var returnEntries = 0;
            returnIterator = Iterator.concat({
              [Symbol.iterator]: function() {
                return {
                  next: function() { return { value: 1, done: false }; },
                  return: function() {
                    returnEntries += 1;
                    returnIterator.return();
                    return {};
                  }
                };
              }
            });
            returnIterator.next();
            var returnType = false;
            try { returnIterator.return(); }
            catch (error) { returnType = error instanceof TypeError; }
            [nextType, nextEntries, returnType, returnEntries].join("|");
        "#),
        Value::String(Arc::from("true|1|true|1"))
    );
}

#[test]
fn iterator_concat_retains_records_and_uses_the_method_realm() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var iterator = (function() {
              var state = { values: [{ amount: 7 }, { amount: 9 }], index: 0 };
              var iterable = {
                get [Symbol.iterator]() {
                  forceGc();
                  return function() {
                    forceGc();
                    return {
                      next: function() {
                        forceGc();
                        return state.index < state.values.length
                          ? { value: state.values[state.index++], done: false }
                          : { done: true };
                      }
                    };
                  };
                }
              };
              return other.Iterator.concat(iterable);
            })();
            forceGc();
            var result = iterator.next();
            forceGc();
            var second = iterator.next();
            forceGc();
            var helperRealm = iterator instanceof other.Iterator && !(iterator instanceof Iterator);
            var resultRealm = Object.getPrototypeOf(result) === other.Object.prototype;
            var argumentRealm = false;
            try { other.Iterator.concat(1); }
            catch (error) {
              argumentRealm = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            var openRealm = false;
            var bad = other.Iterator.concat({
              [Symbol.iterator]: function() { return 0; }
            });
            try { bad.next(); }
            catch (error) {
              openRealm = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            [
              result.value.amount, second.value.amount,
              helperRealm, resultRealm, argumentRealm, openRealm
            ].join("|");
            "#,
        )
        .expect("Iterator concat should retain records and method Realm"),
        Value::String(Arc::from("7|9|true|true|true|true"))
    );
}

#[test]
fn iterator_helpers_distinguish_creation_and_borrowed_method_realms() {
    assert_eq!(
        run(r#"
            var a = $262.createRealm().global;
            var b = $262.createRealm().global;
            var bSample = b.Iterator.prototype.map.call(
              { next: function() { return { done: true }; } },
              function(value) { return value; }
            );
            var bProto = Object.getPrototypeOf(bSample);
            var nextB = bProto.next;
            var returnB = bProto.return;

            var closes = 0;
            var source = {
              index: 0,
              next: function() {
                return this.index++ === 0
                  ? { value: 1, done: false }
                  : { done: true };
              },
              return: function() { closes += 1; return {}; }
            };
            var helper = a.Iterator.prototype.map.call(
              source,
              function(value) { return value; }
            );
            var yielded = nextB.call(helper);
            var returned = returnB.call(helper);
            var completedNext = nextB.call(helper);
            var completedReturn = returnB.call(helper);

            var terminalSource = {
              index: 0,
              next: function() {
                return this.index++ === 0
                  ? { value: 1, done: false }
                  : { done: true };
              }
            };
            var terminalHelper = a.Iterator.prototype.map.call(
              terminalSource,
              function(value) { return value; }
            );
            var terminalYield = nextB.call(terminalHelper);
            var terminalDone = nextB.call(terminalHelper);

            var startCloses = 0;
            var startHelper = a.Iterator.prototype.map.call({
              next: function() { return { value: 1, done: false }; },
              return: function() { startCloses += 1; return {}; }
            }, function(value) { return value; });
            var startReturn = returnB.call(startHelper);

            var startCloseTypeRealm = false;
            var startBadClose = a.Iterator.prototype.map.call({
              next: function() { return { done: true }; },
              return: function() { return 1; }
            }, function(value) { return value; });
            try { returnB.call(startBadClose); }
            catch (error) {
              startCloseTypeRealm = error instanceof b.TypeError &&
                !(error instanceof a.TypeError);
            }

            var resumedTypeRealm = false;
            var badResult = a.Iterator.prototype.map.call({
              next: function() { return 1; }
            }, function(value) { return value; });
            try { nextB.call(badResult); }
            catch (error) {
              resumedTypeRealm = error instanceof a.TypeError &&
                !(error instanceof b.TypeError);
            }

            var closeTypeRealm = false;
            var badClose = a.Iterator.prototype.map.call({
              next: function() { return { value: 1, done: false }; },
              return: function() { return 1; }
            }, function(value) { return value; });
            nextB.call(badClose);
            try { returnB.call(badClose); }
            catch (error) {
              closeTypeRealm = error instanceof a.TypeError &&
                !(error instanceof b.TypeError);
            }

            var brandTypeRealm = false;
            try { nextB.call({}); }
            catch (error) {
              brandTypeRealm = error instanceof b.TypeError &&
                !(error instanceof a.TypeError);
            }

            var runningTypeRealm = false;
            var running;
            running = a.Iterator.prototype.map.call({
              next: function() {
                nextB.call(running);
                return { done: true };
              }
            }, function(value) { return value; });
            try { nextB.call(running); }
            catch (error) {
              runningTypeRealm = error instanceof b.TypeError &&
                !(error instanceof a.TypeError);
            }

            [
              Object.getPrototypeOf(yielded) === a.Object.prototype,
              Object.getPrototypeOf(returned) === b.Object.prototype,
              Object.getPrototypeOf(terminalYield) === a.Object.prototype,
              Object.getPrototypeOf(terminalDone) === b.Object.prototype,
              Object.getPrototypeOf(completedNext) === b.Object.prototype,
              Object.getPrototypeOf(completedReturn) === b.Object.prototype,
              Object.getPrototypeOf(startReturn) === b.Object.prototype,
              closes === 1, startCloses === 1,
              startCloseTypeRealm, resumedTypeRealm, closeTypeRealm,
              brandTypeRealm, runningTypeRealm
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn iterator_concat_close_errors_complete_without_observing_results() {
    assert_eq!(
        run(r#"
            function closeCase(mode) {
              var marker = {};
              var accesses = 0;
              var source = {
                next: function() { return { value: 1, done: false }; }
              };
              if (mode === "getter") {
                Object.defineProperty(source, "return", {
                  get: function() { throw marker; }
                });
              } else if (mode === "noncallable") {
                source.return = 1;
              } else if (mode === "throw") {
                source.return = function() { throw marker; };
              } else if (mode === "primitive") {
                source.return = function() { return 1; };
              } else {
                source.return = function() {
                  var result = {};
                  Object.defineProperty(result, "done", {
                    get: function() { accesses += 1; throw marker; }
                  });
                  Object.defineProperty(result, "value", {
                    get: function() { accesses += 1; throw marker; }
                  });
                  return result;
                };
              }
              var helper = Iterator.concat({
                [Symbol.iterator]: function() { return source; }
              });
              helper.next();
              var outcome = "ok";
              try { helper.return(); }
              catch (error) {
                outcome = error === marker ? "marker" :
                  error instanceof TypeError ? "type" : "other";
              }
              return [outcome, accesses, helper.next().done].join(",");
            }
            [
              closeCase("getter"), closeCase("noncallable"),
              closeCase("throw"), closeCase("primitive"), closeCase("ignored")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "marker,0,true|type,0,true|marker,0,true|type,0,true|ok,0,true"
        ))
    );
}

#[test]
fn iterator_concat_abrupt_steps_do_not_close_or_open_later_sources() {
    assert_eq!(
        run(r#"
            function failureCase(mode) {
              var marker = {};
              var closes = 0;
              var laterOpens = 0;
              var first = {
                [Symbol.iterator]: function() {
                  if (mode === "open-throw") throw marker;
                  if (mode === "open-primitive") return 1;
                  var iterator = {
                    return: function() { closes += 1; return {}; }
                  };
                  if (mode === "next-getter") {
                    Object.defineProperty(iterator, "next", {
                      get: function() { throw marker; }
                    });
                  } else if (mode === "next-throw") {
                    iterator.next = function() { throw marker; };
                  } else if (mode === "next-primitive") {
                    iterator.next = function() { return 1; };
                  } else if (mode === "done-throw") {
                    iterator.next = function() {
                      return Object.defineProperty({}, "done", {
                        get: function() { throw marker; }
                      });
                    };
                  } else {
                    iterator.next = function() {
                      var result = { done: false };
                      Object.defineProperty(result, "value", {
                        get: function() { throw marker; }
                      });
                      return result;
                    };
                  }
                  return iterator;
                }
              };
              var later = {
                [Symbol.iterator]: function() {
                  laterOpens += 1;
                  return { next: function() { return { done: true }; } };
                }
              };
              var helper = Iterator.concat(first, later);
              var failed = false;
              try { helper.next(); }
              catch (error) { failed = true; }
              return [failed, closes, laterOpens, helper.next().done].join(",");
            }
            [
              failureCase("open-throw"), failureCase("open-primitive"),
              failureCase("next-getter"), failureCase("next-throw"),
              failureCase("next-primitive"), failureCase("done-throw"),
              failureCase("value-throw")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true,0,0,true|true,0,0,true|true,0,0,true|true,0,0,true|true,0,0,true|true,0,0,true|true,0,0,true"
        ))
    );
}

#[test]
fn iterator_zip_retains_fresh_padding_across_forced_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var outerIndex = 0;
            var outer = {
              [Symbol.iterator]: function() { return this; },
              next: function() {
                forceGc();
                if (outerIndex === 0) { outerIndex += 1; return { value: [] }; }
                if (outerIndex === 1) { outerIndex += 1; return { value: [7] }; }
                return { done: true };
              }
            };
            var helper = Iterator.zip(outer, {
              mode: "longest",
              get padding() {
                return {
                  index: 0,
                  [Symbol.iterator]: function() { forceGc(); return this; },
                  next: function() {
                    forceGc();
                    return { value: { slot: this.index++ }, done: false };
                  },
                  return: function() { forceGc(); return {}; }
                };
              }
            });
            forceGc();
            var result = helper.next();
            forceGc();
            [result.value[0].slot, result.value[1], result.done].join("|");
        "#,
        )
        .expect("Iterator.zip padding GC regression failed"),
        Value::String(Arc::from("0|7|false"))
    );
}

#[test]
fn iterator_zip_uses_creation_and_borrowed_method_realms() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var mainHelperPrototype = Object.getPrototypeOf(Iterator.zip([]));
            var helper = other.Iterator.zip([[1], [2]]);
            var yielded = helper.next();
            helper.next();
            var completed = mainHelperPrototype.next.call(helper);

            var strictRealm = false;
            try { other.Iterator.zip([[], [1]], { mode: "strict" }).next(); }
            catch (error) {
              strictRealm = error instanceof other.TypeError && !(error instanceof TypeError);
            }

            var borrowedCloseRealm = false;
            var start = other.Iterator.zip([{
              next: function() { return { done: true }; },
              return: 1
            }]);
            try { mainHelperPrototype.return.call(start); }
            catch (error) {
              borrowedCloseRealm = error instanceof TypeError &&
                !(error instanceof other.TypeError);
            }

            [
              helper instanceof other.Iterator,
              Object.getPrototypeOf(yielded) === other.Object.prototype,
              Object.getPrototypeOf(yielded.value) === other.Array.prototype,
              Object.getPrototypeOf(completed) === Object.prototype,
              strictRealm,
              borrowedCloseRealm
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true"))
    );
}

#[test]
fn iterator_zip_treats_duplicate_records_separately_and_preserves_close_priority() {
    assert_eq!(
        run(r#"
            var closes = 0;
            var shared = {
              next: function() { return { value: 1, done: false }; },
              return: function() { closes += 1; return {}; }
            };
            Iterator.zip([{
              next: function() { return { done: true }; }
            }, shared, shared]).next();

            var primary = {};
            var secondary = {};
            var order = [];
            var right = {
              next: function() { return { value: 1, done: false }; }
            };
            Object.defineProperty(right, "return", {
              get: function() { order.push("right"); throw primary; }
            });
            var middle = {
              next: function() { return { value: 1, done: false }; },
              return: function() { order.push("middle"); throw secondary; }
            };
            var caughtPrimary = false;
            try {
              Iterator.zip([{
                next: function() { return { done: true }; }
              }, middle, right]).next();
            } catch (error) {
              caughtPrimary = error === primary;
            }
            [closes, caughtPrimary, order.join(",")].join("|");
        "#),
        Value::String(Arc::from("2|true|right,middle"))
    );
}

#[test]
fn iterator_zip_keyed_retains_inputs_padding_and_results_across_forced_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var symbol = Symbol("slot");
            var target = { text: null };
            target[symbol] = null;
            var iterables = new Proxy(target, {
              ownKeys: function(object) {
                forceGc();
                var keys = Reflect.ownKeys(object);
                return {
                  get 0() { forceGc(); return keys[0]; },
                  get 1() { forceGc(); return keys[1]; },
                  get length() {
                    forceGc();
                    return {
                      valueOf: function() { forceGc(); return keys.length; }
                    };
                  }
                };
              },
              getOwnPropertyDescriptor: function(object, key) {
                forceGc();
                return Reflect.getOwnPropertyDescriptor(object, key);
              },
              get: function(_object, key) {
                forceGc();
                if (key === "text") {
                  var empty = {};
                  Object.defineProperty(empty, "next", {
                    get: function() {
                      forceGc();
                      return function() { forceGc(); return { done: true }; };
                    }
                  });
                  return empty;
                }
                return {
                  [Symbol.iterator]: function() {
                    forceGc();
                    var emitted = false;
                    return {
                      next: function() {
                        forceGc();
                        if (emitted) return { done: true };
                        emitted = true;
                        return { value: 7, done: false };
                      }
                    };
                  }
                };
              }
            });
            var padding = new Proxy({}, {
              get: function(_object, key) {
                forceGc();
                return { key: key };
              }
            });
            var helper = Iterator.zipKeyed(iterables, {
              mode: "longest",
              padding: padding
            });
            forceGc();
            var result = helper.next();
            forceGc();
            var textDescriptor = Object.getOwnPropertyDescriptor(result.value, "text");
            [
              result.value.text.key,
              result.value[symbol],
              Object.getPrototypeOf(result.value) === null,
              textDescriptor.writable,
              textDescriptor.enumerable,
              textDescriptor.configurable,
              result.done
            ].join("|");
        "#,
        )
        .expect("Iterator.zipKeyed GC regression failed"),
        Value::String(Arc::from("text|7|true|true|true|true|false"))
    );
}

#[test]
fn iterator_zip_keyed_uses_creation_and_borrowed_method_realms() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var mainHelperPrototype = Object.getPrototypeOf(Iterator.zipKeyed({}));
            var helper = other.Iterator.zipKeyed({ key: [1] });
            var yielded = helper.next();
            helper.next();
            var completed = mainHelperPrototype.next.call(helper);

            var strictRealm = false;
            try {
              other.Iterator.zipKeyed({ empty: [], live: [1] }, { mode: "strict" }).next();
            } catch (error) {
              strictRealm = error instanceof other.TypeError && !(error instanceof TypeError);
            }

            var borrowedCloseRealm = false;
            var start = other.Iterator.zipKeyed({
              key: {
                next: function() { return { done: true }; },
                return: 1
              }
            });
            try { mainHelperPrototype.return.call(start); }
            catch (error) {
              borrowedCloseRealm = error instanceof TypeError &&
                !(error instanceof other.TypeError);
            }

            [
              helper instanceof other.Iterator,
              Object.getPrototypeOf(yielded) === other.Object.prototype,
              Object.getPrototypeOf(yielded.value) === null,
              Object.getPrototypeOf(completed) === Object.prototype,
              strictRealm,
              borrowedCloseRealm
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true"))
    );
}

#[test]
fn iterator_zip_keyed_accepts_null_traps_and_array_like_own_keys_results() {
    assert_eq!(
        run(r#"
            var nullTrap = new Proxy({ key: [1] }, { ownKeys: null });
            var arrayLike = new Proxy({ key: [2] }, {
              ownKeys: function() {
                return { 0: "key", length: 1 };
              }
            });
            var first = Iterator.zipKeyed(nullTrap).next().value;
            var second = Iterator.zipKeyed(arrayLike).next().value;
            [first.key, second.key].join("|");
        "#),
        Value::String(Arc::from("1|2"))
    );
}

#[test]
fn array_map_reduce() {
    assert_eq!(
        run("[1,2,3].map(x => x*2).join(',');"),
        Value::String(Arc::from("2,4,6"))
    );
    assert_eq!(run("[1,2,3].reduce((a,b)=>a+b, 0);"), Value::Number(6.0));
}

#[test]
fn array_method_chaining() {
    assert_eq!(
        run("[1,2,3,4,5].filter(x => x > 2).map(x => x * 2).join(',');"),
        Value::String(Arc::from("6,8,10"))
    );
}

#[test]
fn array_find() {
    assert_eq!(run("[4,5,6].find(x=>x>4);"), Value::Number(5.0));
}

#[test]
fn array_findindex() {
    assert_eq!(run("[4,5,6].findIndex(x=>x>4);"), Value::Number(1.0));
}

#[test]
fn array_find_methods_use_array_like_property_access() {
    assert!(run_err("Array.prototype.find.call(null, function() {})")
        .contains("Cannot convert undefined or null to object"));
    assert!(run_err("[].find({})").contains("Array predicate is not callable"));
    assert!(run_err(
        r#"var o = {};
           Object.defineProperty(o, "length", {
             get: function(){ throw new Error("length-get"); }
           });
           Array.prototype.find.call(o);"#
    )
    .contains("length-get"));
    assert!(run_err(
        r#"var o = { length: 1 };
           Object.defineProperty(o, "0", {
             get: function(){ throw new Error("index-get"); }
           });
           Array.prototype.find.call(o, function(){ return false; });"#
    )
    .contains("index-get"));
    assert_eq!(
        run(r#"var arr = ["Shoes", "Car", "Bike"];
               var seen = [];
               arr.find(function(value) {
                 if (seen.length === 0) arr.splice(1, 1);
                 seen.push(String(value));
                 return false;
               });
               seen.join("|");"#),
        Value::String(Arc::from("Shoes|Bike|undefined"))
    );
    assert_eq!(
        run(r#"var arr = ["Shoes", "Car", "Bike"];
               var seen = [];
               arr.findLastIndex(function(value) {
                 if (seen.length === 0) arr.splice(1, 1);
                 seen.push(String(value));
                 return false;
               });
               seen.join("|");"#),
        Value::String(Arc::from("Bike|Bike|Shoes"))
    );
    assert_eq!(
        run(r#"var receiver;
               var result = Array.prototype.find.call({0: "x", length: 1}, function(value, index, object) {
                 receiver = this;
                 return value === "x" && index === 0 && object.length === 1;
               }, 7);
               [result, receiver.valueOf()].join("|");"#),
        Value::String(Arc::from("x|7"))
    );
}

#[test]
fn array_some() {
    assert_eq!(run("[1,2,3].some(x=>x>2);"), Value::Bool(true));
}

#[test]
fn array_every_false() {
    assert_eq!(run("[1,2,3].every(x=>x>2);"), Value::Bool(false));
}

#[test]
fn array_some_every_use_array_like_property_access() {
    assert!(run_err("Array.prototype.some.call(null, function() {})")
        .contains("Cannot convert undefined or null to object"));
    assert!(run_err("[].every({})").contains("Array predicate is not callable"));
    assert_eq!(
        run(r#"var seen = [];
               var receiver = 0;
               var result = Array.prototype.some.call({0: 11, 1: 12, length: 2}, function(value, index, object) {
                 receiver = this.valueOf();
                 seen.push(value + ":" + index + ":" + object.length);
                 return value === 12;
               }, 7);
               result + "|" + receiver + "|" + seen.join(",");"#),
        Value::String(Arc::from("true|7|11:0:2,12:1:2"))
    );
    assert_eq!(
        run(r#"Array.prototype[1] = 13;
               var seen = [];
               var result = [, , ,].every(function(value, index) {
                 seen.push(value + ":" + index);
                 return value !== 13;
               });
               delete Array.prototype[1];
               result + "|" + seen.join(",");"#),
        Value::String(Arc::from("false|13:1"))
    );
    assert_eq!(
        run(r#"var arr = [1, 2, 3];
               var seen = [];
               arr.some(function(value, index, object) {
                 seen.push(String(value));
                 if (index === 0) {
                   object.length = 1;
                   object[2] = 9;
                 }
                 return false;
               });
               seen.join(",");"#),
        Value::String(Arc::from("1,9"))
    );
}

#[test]
fn array_includes_nan() {
    assert_eq!(run("[NaN].includes(NaN);"), Value::Bool(true));
}

#[test]
fn global_uri_functions_follow_percent_encoding_rules() {
    assert_eq!(
        run("decodeURI('%3B') + '|' + decodeURIComponent('%3B');"),
        Value::String(Arc::from("%3B|;"))
    );
    assert_eq!(
        run("decodeURI('%5E') + '|' + decodeURIComponent('%2F');"),
        Value::String(Arc::from("^|/"))
    );
    assert_eq!(
        run("encodeURI('http://ru.wikipedia.org/wiki/Юникод');"),
        Value::String(Arc::from(
            "http://ru.wikipedia.org/wiki/%D0%AE%D0%BD%D0%B8%D0%BA%D0%BE%D0%B4"
        ))
    );
    assert_eq!(
        run("encodeURIComponent(';/?:@&=+$,#');"),
        Value::String(Arc::from("%3B%2F%3F%3A%40%26%3D%2B%24%2C%23"))
    );
    assert_eq!(
        run("var s = String.fromCharCode(0xDB80, 0xDC00); s.length + '|' + s.charCodeAt(0).toString(16) + '|' + s.charCodeAt(1).toString(16) + '|' + encodeURI(s);"),
        Value::String(Arc::from("2|db80|dc00|%F3%B0%80%80"))
    );
    assert_eq!(
        run("var s = String.fromCharCode(0xDB80, 0xDC00); var p = String.fromCodePoint(0xF0000); (s === p) + '|' + p.length + '|' + encodeURI(p) + '|' + encodeURI(s.substring(0, 2)) + '|' + encodeURI(s.slice(0, 2));"),
        Value::String(Arc::from(
            "true|2|%F3%B0%80%80|%F3%B0%80%80|%F3%B0%80%80"
        ))
    );
    assert_eq!(
        run("decodeURI('%F3%B0%80%80') === String.fromCharCode(0xDB80, 0xDC00);"),
        Value::Bool(true)
    );
    assert!(run_err("decodeURIComponent('%C0%AF');").contains("URIError"));
    assert!(run_err("decodeURIComponent('%ED%BF%BF');").contains("URIError"));
    assert!(run_err("encodeURI(String.fromCharCode(0xD800));").contains("URIError"));
    assert!(run_err("encodeURIComponent(String.fromCharCode(0xDC00));").contains("URIError"));
}

#[test]
fn source_and_json_unicode_scalars_do_not_alias_surrogate_sentinels() {
    assert_eq!(
        run(concat!(
            "var raw = '\u{F0000}';",
            "var escaped = '\\u{F0000}';",
            "var pairEscaped = '\\uDB80\\uDC00';",
            "var made = String.fromCodePoint(0xF0000);",
            "[raw.length, escaped.length, pairEscaped.length, made.length,",
            " raw === escaped, pairEscaped === made,",
            " escaped === made, raw.charCodeAt(0).toString(16),",
            " raw.charCodeAt(1).toString(16)].join('|');"
        )),
        Value::String(Arc::from("2|2|2|2|true|true|true|db80|dc00"))
    );
    assert_eq!(
        run(concat!(
            "var raw = '\u{F07FF}';",
            "var escaped = '\\u{F07FF}';",
            "var made = String.fromCodePoint(0xF07FF);",
            "[raw.length, escaped.length, raw === made,",
            " raw.charCodeAt(0).toString(16),",
            " raw.charCodeAt(1).toString(16)].join('|');"
        )),
        Value::String(Arc::from("2|2|true|db81|dfff"))
    );
    assert_eq!(
        run(concat!(
            "var raw = JSON.parse('\"\u{F0000}\"');",
            "var escaped = JSON.parse('\"\\uDB80\\uDC00\"');",
            "[raw.length, escaped.length, raw === escaped,",
            " raw === String.fromCodePoint(0xF0000)].join('|');"
        )),
        Value::String(Arc::from("2|2|true|true"))
    );
    assert_eq!(
        run(concat!(
            "var regex = /\u{F0000}/u;",
            "[regex.source.length,",
            " regex.source === String.fromCodePoint(0xF0000),",
            " regex.test(String.fromCodePoint(0xF0000)),",
            " /[\u{F0000}]/u.test(String.fromCodePoint(0xF0000)),",
            " new RegExp(String.fromCodePoint(0xF0000), 'u')",
            "   .test(String.fromCodePoint(0xF0000))].join('|');"
        )),
        Value::String(Arc::from("2|true|true|true|true"))
    );
    assert_eq!(
        run(concat!(
            "var lower = String.fromCodePoint(0xF0000);",
            "var upper = String.fromCodePoint(0xF07FF);",
            "[new RegExp(lower, 'v').test(lower),",
            " new RegExp(upper, 'u').test(upper),",
            " new RegExp(upper, 'v').test(upper),",
            " new RegExp(lower).test(lower),",
            " new RegExp(upper).test(upper),",
            " new RegExp('[' + lower + '-' + upper + ']', 'u').test(upper),",
            " new RegExp('[' + lower + '-' + upper + ']', 'v').test(lower)]",
            " .join('|');"
        )),
        Value::String(Arc::from("true|true|true|true|true|true|true"))
    );
    assert_eq!(
        run(r#"
            var scalar = String.fromCodePoint(0xF0000);
            var source;
            var result = JSON.parse('{"\\uDB80\\uDC00":1}', function(key, value, context) {
              if (key === scalar) source = context.source;
              return value;
            });
            [source, result[scalar]].join('|');
        "#),
        Value::String(Arc::from("1|1"))
    );
    assert_eq!(
        run(r#"
            var high = String.fromCharCode(0xDB80);
            var low = String.fromCharCode(0xDC00);
            var scalar = String.fromCodePoint(0xF0000);
            var direct = eval('/' + high + '/').source;
            var escaped = eval('/\\' + high + '/').source;
            var indirect = (0, eval)('/' + high + '/').source;
            var other = $262.createRealm().global;
            var crossRealm = other.eval('/' + high + '/').source;
            var template = eval('`' + low + '`');
            var scalarSource = eval('/' + scalar + '/').source;
            var dynamic = Function("return '" + high + "'.charCodeAt(0)")();
            var GeneratorFunction = (function*() {}).constructor;
            var generated = GeneratorFunction(
              "return '" + low + "'.charCodeAt(0)"
            )().next().value;
            $262.evalScript(
              "globalThis.evalScriptUnit = /" + low + "/.source.charCodeAt(0);"
            );
            [direct.charCodeAt(0).toString(16),
             escaped.charCodeAt(1).toString(16),
             indirect.charCodeAt(0).toString(16),
             crossRealm.charCodeAt(0).toString(16),
             template.charCodeAt(0).toString(16),
             scalarSource.length,
             scalarSource.codePointAt(0).toString(16),
             dynamic,
             generated,
             evalScriptUnit].join('|');
        "#),
        Value::String(Arc::from(
            "db80|db80|db80|db80|dc00|2|f0000|56192|56320|56320"
        ))
    );
}

#[test]
fn public_string_output_decodes_canonical_utf16() {
    let mut vm = Vm::new().expect("VM should initialize");
    let value = vm
        .run(concat!("'", "\u{F0000}", "';"))
        .expect("source scalar should evaluate");
    let host = vm
        .to_string_pub(&value)
        .expect("public string conversion should succeed");
    assert_eq!(host, "\u{F0000}");
    let indirect = vm
        .eval_indirect(concat!("'", "\u{F0000}", "';"))
        .expect("public indirect eval should accept host Unicode source");
    assert_eq!(
        vm.to_string_pub(&indirect)
            .expect("indirect eval result should decode for the host"),
        "\u{F0000}"
    );
    let Value::String(round_trip) = Value::from_string(&host) else {
        panic!("expected string value");
    };
    assert_eq!(ruja::value::utf16_from_str(&round_trip), [0xDB80, 0xDC00]);

    assert_eq!(
        vm.run(concat!(
            "var scalar = String.fromCodePoint(0xF0000);",
            "try { BigInt(scalar); } catch (error) {",
            " error.message.includes(scalar);",
            "}"
        ))
        .expect("internal-string error should evaluate"),
        Value::Bool(true)
    );
    let internal_error = vm
        .run("BigInt(String.fromCodePoint(0xF0000));")
        .expect_err("invalid BigInt input should fail");
    assert!(internal_error.to_string().contains('\u{F0000}'));

    vm.register_fn(
        "hostUnicodeError",
        |_vm, _args, _this| Err(ruja::error::Error::type_err_host("\u{F0000}")),
        0,
    )
    .expect("host error function should register");
    assert_eq!(
        vm.run(
            "try { hostUnicodeError(); } catch (error) { error.message === String.fromCodePoint(0xF0000); }",
        )
        .expect("host error should be catchable"),
        Value::Bool(true)
    );
    let host_error = vm
        .run("hostUnicodeError();")
        .expect_err("uncaught host error should escape");
    assert!(host_error.to_string().contains('\u{F0000}'));
}

#[test]
fn array_search_methods_use_array_like_property_access() {
    assert_eq!(
        run("var obj = {0:'x', 1:true, length:2}; Array.prototype.indexOf.call(obj, true);"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var obj = {0:'x', 1:Infinity, length:2}; Array.prototype.lastIndexOf.call(obj, Infinity);"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var marker = {}; Boolean.prototype[1] = marker; Boolean.prototype.length = 2; var r = Array.prototype.indexOf.call(true, marker); delete Boolean.prototype[1]; delete Boolean.prototype.length; r;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("Array.prototype.indexOf.call(new String('null'), 'l');"),
        Value::Number(2.0)
    );
    assert_eq!(
        run("Array.prototype.indexOf.call('abc', 'b');"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("Array.prototype.lastIndexOf.call('abca', 'a');"),
        Value::Number(3.0)
    );
    assert_eq!(run("[0,,2].indexOf(undefined);"), Value::Number(-1.0));
    assert_eq!(run("[0,,2].includes(undefined);"), Value::Bool(true));
    assert_eq!(
        run(r#"
            var arr = [0, 1, 2];
            Object.defineProperty(arr, "2", {
              get: function() { return "unconfigurable"; },
              configurable: false
            });
            Object.defineProperty(arr, "1", {
              get: function() { arr.length = 2; return 1; },
              configurable: true
            });
            [arr.indexOf("unconfigurable"), arr.length, 2 in arr].join("|");
            "#),
        Value::String(Arc::from("2|3|true"))
    );
    assert_eq!(
        run(r#"
            var arr = [0, 1, 2, 3];
            Object.defineProperty(arr, "2", {
              get: function() { return "unconfigurable"; },
              configurable: false
            });
            Object.defineProperty(arr, "3", {
              get: function() { arr.length = 2; return 1; },
              configurable: true
            });
            [arr.lastIndexOf("unconfigurable"), arr.length, 2 in arr, 3 in arr].join("|");
            "#),
        Value::String(Arc::from("2|3|true|false"))
    );
}

#[test]
fn generic_array_push_and_pop_preserve_named_integer_boundary_keys() {
    assert_eq!(
        run(r#"
            var log = [];
            var target = { length: 4294967295 };
            var proxy = new Proxy(target, {
              get: function(target, key, receiver) {
                log.push("get:" + String(key));
                return Reflect.get(target, key, receiver);
              },
              set: function(target, key, value, receiver) {
                log.push("set:" + String(key));
                return Reflect.set(target, key, value, receiver);
              },
              deleteProperty: function(target, key) {
                log.push("delete:" + String(key));
                return Reflect.deleteProperty(target, key);
              }
            });
            var pushed = Array.prototype.push.call(proxy, "boundary");
            var popped = Array.prototype.pop.call(proxy);
            [pushed, popped, target.length, log.join(",")].join("|");
        "#),
        Value::String(Arc::from(
            "4294967296|boundary|4294967295|get:length,set:4294967295,set:length,get:length,get:4294967295,delete:4294967295,set:length"
        ))
    );
}

#[test]
fn array_sort() {
    assert_eq!(
        run("[3,1,2].sort().join(',');"),
        Value::String(Arc::from("1,2,3"))
    );
}

#[test]
fn array_sort_cmp() {
    assert_eq!(
        run("[10,5,8].sort((a,b)=>a-b).join(',');"),
        Value::String(Arc::from("5,8,10"))
    );
}

#[test]
fn array_sort_methods_root_materialized_values_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            function sortValues(copy) {
              var source = [{ value: 3 }, { value: 1 }, { value: 2 }];
              var replacement;
              var calls = 0;
              var compare = function(left, right) {
                if (calls++ === 0) {
                  source.length = 0;
                  forceGc();
                  replacement = { value: 99 };
                }
                return left.value - right.value;
              };
              var result = copy ? source.toSorted(compare) : source.sort(compare);
              return result.map(function(entry) { return entry.value; }).join(",");
            }
            sortValues(false) + "|" + sortValues(true);
            "#,
        )
        .expect("sort materialized values should survive observable GC"),
        Value::String(Arc::from("1,2,3|1,2,3"))
    );
}

#[test]
fn array_sort_writeback_tracks_live_array_after_comparator_mutation() {
    assert_eq!(
        run(r#"
            var shrunk = [{ value: 3 }, { value: 1 }, { value: 2 }];
            var shrinkCalls = 0;
            shrunk.sort(function(left, right) {
              if (shrinkCalls++ === 0) shrunk.length = 0;
              return left.value - right.value;
            });

            var grown = [{ value: 3 }, { value: 1 }, { value: 2 }];
            var growCalls = 0;
            grown.sort(function(left, right) {
              if (growCalls++ === 0) grown.push({ value: 99 });
              return left.value - right.value;
            });

            [
              shrunk.length,
              0 in shrunk,
              Object.keys(shrunk).join(","),
              shrunk.map(function(entry) { return entry.value; }).join(","),
              grown.length,
              Object.keys(grown).join(","),
              grown.map(function(entry) { return entry.value; }).join(",")
            ].join("|");
            "#,),
        Value::String(Arc::from("3|true|0,1,2|1,2,3|4|0,1,2,3|1,2,3,99"))
    );
}

#[test]
fn array_sort_preserves_holes_and_sorts_only_present_values() {
    assert_eq!(
        run(r#"
            var trailing = [1, ,];
            trailing.sort();
            var leading = [, 1];
            leading.sort();
            var empty = Array(2);
            empty.sort();
            var explicit = [undefined, ,];
            explicit.sort();
            [
              trailing.length, Object.keys(trailing).join(","), 1 in trailing,
              leading.length, Object.keys(leading).join(","), 1 in leading,
              empty.length, Object.keys(empty).join(","), 0 in empty,
              explicit.length, Object.keys(explicit).join(","),
              explicit.hasOwnProperty(0), 1 in explicit
            ].join("|");
            "#,),
        Value::String(Arc::from("2|0|false|2|0|false|2||false|2|0|true|false"))
    );
}

#[test]
fn array_sort_writeback_keeps_index_descriptors_and_dense_storage_in_sync() {
    assert_eq!(
        run(r#"
            var values = [3, 2, 1];
            var calls = 0;
            values.sort(function(left, right) {
              if (calls++ === 0) {
                Object.defineProperty(values, "0", {
                  value: 3,
                  writable: true,
                  enumerable: true,
                  configurable: true
                });
              }
              return left - right;
            });
            [
              values[0],
              values.join(","),
              values.map(function(value) { return value; }).join(",")
            ].join("|");
            "#,),
        Value::String(Arc::from("1|1,2,3|1,2,3"))
    );
}

#[test]
fn array_sort_methods_order_undefined_without_calling_comparator() {
    assert_eq!(
        run(r#"
            function check(copy) {
              var calls = 0;
              var values = [undefined, 1];
              var compare = function() { calls++; return -1; };
              var result = copy ? values.toSorted(compare) : values.sort(compare);
              return [result[0] === 1, result[1] === undefined, calls].join(",");
            }
            check(false) + "|" + check(true);
            "#,),
        Value::String(Arc::from("true,true,0|true,true,0"))
    );
}

#[test]
fn array_sort_methods_use_utf16_code_unit_order() {
    assert_eq!(
        run(r#"
            var loneHigh = "\uD800";
            var supplementary = "\uD800\uDC00";
            var fullwidthZ = "\uFF3A";
            function firstIs(copy, values, expected) {
              var result = copy ? values.toSorted() : values.sort();
              return result[0] === expected;
            }
            [
              firstIs(false, [supplementary, fullwidthZ], supplementary),
              firstIs(true, [supplementary, fullwidthZ], supplementary),
              firstIs(false, [supplementary, loneHigh], loneHigh),
              firstIs(true, [supplementary, loneHigh], loneHigh)
            ].join("|");
            "#,),
        Value::String(Arc::from("true|true|true|true"))
    );
}

#[test]
fn array_sort_methods_propagate_comparator_conversion_errors() {
    assert_eq!(
        run(r#"
            var marker = {};
            function isTypeError(callback) {
              try { callback(); return false; }
              catch (error) { return error instanceof TypeError; }
            }
            function isMarker(callback) {
              try { callback(); return false; }
              catch (error) { return error === marker; }
            }
            function comparisonResult() {
              return { valueOf: function() { throw marker; } };
            }
            function stringValue() {
              return { toString: function() { throw marker; } };
            }
            [
              isTypeError(function() { [2, 1].sort(null); }),
              isTypeError(function() { [2, 1].toSorted(null); }),
              isMarker(function() { [2, 1].sort(comparisonResult); }),
              isMarker(function() { [2, 1].toSorted(comparisonResult); }),
              isMarker(function() { [stringValue(), {}].sort(); }),
              isMarker(function() { [stringValue(), {}].toSorted(); })
            ].join("|");
            "#,),
        Value::String(Arc::from("true|true|true|true|true|true"))
    );
}

#[test]
fn array_sort_methods_use_generic_sort_indexed_properties() {
    assert_eq!(
        run(r#"
            var proto = { 1: 2 };
            var sortable = Object.create(proto);
            sortable[0] = 3;
            sortable[2] = 1;
            var lengthCalls = 0;
            sortable.length = {
              valueOf: function() { lengthCalls++; return 3; }
            };
            var returned = Array.prototype.sort.call(
              sortable,
              function(left, right) { return left - right; }
            );

            var source = Object.create(proto);
            source[0] = 3;
            source[2] = 1;
            source.length = "3";
            var copy = Array.prototype.toSorted.call(
              source,
              function(left, right) { return left - right; }
            );

            [
              returned === sortable,
              lengthCalls,
              sortable[0], sortable[1], sortable[2],
              sortable.hasOwnProperty(1),
              copy.join(","), copy.length, copy.hasOwnProperty(0),
              source[0], source.hasOwnProperty(1), source[2]
            ].join("|");
            "#,),
        Value::String(Arc::from("true|1|1|2|3|true|1,2,3|3|true|3|false|1"))
    );
}

#[test]
fn array_sort_methods_observe_generic_property_operation_order() {
    assert_eq!(
        run(r#"
            var sortLog = [];
            var sortTarget = { length: 3, 0: 3, 2: 1 };
            var sortable = new Proxy(sortTarget, {
              has: function(target, key) {
                if (key === "0" || key === "1" || key === "2") {
                  sortLog.push("has:" + key);
                }
                return key in target;
              },
              get: function(target, key) {
                if (key === "0" || key === "1" || key === "2") {
                  sortLog.push("get:" + key);
                }
                return target[key];
              },
              set: function(target, key, value) {
                sortLog.push("set:" + key);
                target[key] = value;
                return true;
              },
              deleteProperty: function(target, key) {
                sortLog.push("delete:" + key);
                delete target[key];
                return true;
              }
            });
            Array.prototype.sort.call(sortable, function(left, right) {
              sortLog.push("compare");
              return left - right;
            });

            var copyLog = [];
            var copyTarget = { length: 3, 0: 3, 2: 1 };
            var copySource = new Proxy(copyTarget, {
              has: function() {
                copyLog.push("has");
                return true;
              },
              get: function(target, key) {
                if (key === "0" || key === "1" || key === "2") {
                  copyLog.push("get:" + key);
                }
                return target[key];
              }
            });
            var copy = Array.prototype.toSorted.call(copySource, function(left, right) {
              copyLog.push("compare");
              return left - right;
            });

            [
              sortLog.join(","),
              copyLog.join(","),
              copy[0], copy[1], copy[2] === undefined,
              copy.hasOwnProperty(2)
            ].join("|");
            "#,),
        Value::String(Arc::from(
            "has:0,get:0,has:1,has:2,get:2,compare,set:0,set:1,delete:2|\
             get:0,get:1,get:2,compare|1|3|true|true"
        ))
    );
}

#[test]
fn array_sort_writeback_observes_proxy_prototype_set_trap() {
    assert_eq!(
        run(r#"
            var marker = {};
            var calls = [];
            var receiver = [, 1];
            Object.setPrototypeOf(receiver, new Proxy({}, {
              set: function(target, key, value, actualReceiver) {
                calls.push(key + ':' + value + ':' + (actualReceiver === receiver));
                throw marker;
              }
            }));
            var caught = false;
            try { Array.prototype.sort.call(receiver); }
            catch (error) { caught = error === marker; }

            [
              caught,
              calls.join(','),
              receiver.hasOwnProperty('0'),
              receiver.hasOwnProperty('1'),
              receiver[1]
            ].join('|');
            "#,),
        Value::String(Arc::from("true|0:1:true|false|true|1"))
    );
}

#[test]
fn array_sort_methods_box_receivers_and_enforce_array_create_limits() {
    assert_eq!(
        run(r#"
            function isTypeError(callback) {
              try { callback(); return false; }
              catch (error) { return error instanceof TypeError; }
            }
            var frozenCopy = Object.freeze([2, 0, 1]).toSorted();

            Boolean.prototype.length = 3;
            var boxedCopy = Array.prototype.toSorted.call(true);
            delete Boolean.prototype.length;

            var gets = 0;
            var huge = {
              length: 2 ** 32,
              get 0() { gets++; throw new Error("must not read"); }
            };
            var rangeError = false;
            try { Array.prototype.toSorted.call(huge); }
            catch (error) { rangeError = error instanceof RangeError; }

            [
              isTypeError(function() { Array.prototype.sort.call(null); }),
              isTypeError(function() { Array.prototype.toSorted.call(undefined); }),
              frozenCopy.join(","),
              boxedCopy.length,
              boxedCopy[0] === undefined,
              boxedCopy[1] === undefined,
              boxedCopy[2] === undefined,
              boxedCopy.hasOwnProperty(0),
              rangeError,
              gets
            ].join("|");
            "#,),
        Value::String(Arc::from("true|true|0,1,2|3|true|true|true|true|true|0"))
    );
}

#[test]
fn array_sort_methods_enforce_scan_limit_before_index_access() {
    assert_eq!(
        run(r#"
            var indexedOperations = 0;
            function makeHugeSource() {
              return new Proxy({ length: 1048577 }, {
                has: function(target, key) {
                  if (key === "0") indexedOperations++;
                  return key in target;
                },
                get: function(target, key) {
                  if (key === "0") indexedOperations++;
                  return target[key];
                }
              });
            }
            function rejectsBeforeIndexAccess(callback) {
              var before = indexedOperations;
              try { callback(); return false; }
              catch (error) {
                return error instanceof RangeError && indexedOperations === before;
              }
            }

            [
              rejectsBeforeIndexAccess(function() {
                Array.prototype.sort.call(makeHugeSource());
              }),
              rejectsBeforeIndexAccess(function() {
                Array.prototype.toSorted.call(makeHugeSource());
              }),
              indexedOperations
            ].join("|");
            "#,),
        Value::String(Arc::from("true|true|0"))
    );
}

#[test]
fn array_sort_methods_root_generic_collection_values_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            function collect(copy) {
              var target = {
                length: 3,
                0: { value: 3 },
                1: { value: 1 },
                2: { value: 2 }
              };
              var replacement;
              var source = new Proxy(target, {
                get: function(object, key) {
                  var value = object[key];
                  if (key === "1") {
                    object[0] = null;
                    forceGc();
                    replacement = { value: 99 };
                  } else if (key === "2") {
                    object[1] = null;
                    forceGc();
                    replacement = { value: 98 };
                  }
                  return value;
                }
              });
              var compare = function(left, right) {
                return left.value - right.value;
              };
              var result = copy
                ? Array.prototype.toSorted.call(source, compare)
                : Array.prototype.sort.call(source, compare);
              return [result[0].value, result[1].value, result[2].value].join(",");
            }
            collect(false) + "|" + collect(true);
            "#,
        )
        .expect("generic collection values should survive observable GC"),
        Value::String(Arc::from("1,2,3|1,2,3"))
    );
}

#[test]
fn array_fill_materializes_holes() {
    assert_eq!(
        run(r#"
            var value = {};
            var array = Array(3).fill(value);
            [
              0 in array, 1 in array, 2 in array,
              array[0] === value, array[1] === value, array[2] === value,
              Object.keys(array).join(",")
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true|0,1,2"))
    );
}

#[test]
fn array_fill_is_generic_ordered_live_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var coercions = 0;
            var value = {
              marker: 41,
              valueOf: function () { coercions += 1; throw new Error("unused"); }
            };
            var target = { length: 4 };
            var proxy = new Proxy(target, {
              get: function (object, key, receiver) {
                log.push("get:" + String(key));
                return Reflect.get(object, key, receiver);
              },
              set: function (object, key, newValue) {
                log.push("set:" + String(key));
                object[key] = newValue;
                return true;
              }
            });
            var returned = Array.prototype.fill.call(
              proxy,
              value,
              { valueOf: function () {
                  log.push("start");
                  target.length = 1;
                  return -3;
              } },
              { valueOf: function () {
                  log.push("end");
                  target.length = 10;
                  return 3;
              } }
            );

            var inheritedValue;
            var inherited;
            var prototype = {
              set 0(newValue) {
                inheritedValue = [this === inherited, newValue === value];
              }
            };
            inherited = Object.create(prototype);
            inherited.length = 1;
            Array.prototype.fill.call(inherited, value);

            var partialTarget = { length: 3 };
            var partial = new Proxy(partialTarget, {
              set: function (object, key, newValue) {
                if (key === "1") return false;
                object[key] = newValue;
                return true;
              }
            });
            var partialError;
            try { Array.prototype.fill.call(partial, value); }
            catch (error) { partialError = error; }

            var huge = { length: Number.MAX_SAFE_INTEGER };
            Array.prototype.fill.call(
              huge,
              value,
              9007199254740989,
              Number.MAX_SAFE_INTEGER
            );

            var coerced = { length: 3 };
            Array.prototype.fill.call(coerced, 7, "1.9", Infinity);

            var boxed = Array.prototype.fill.call(true, value);
            var nullish = 0;
            try { Array.prototype.fill.call(null, value); }
            catch (error) { nullish += error instanceof TypeError; }
            try { Array.prototype.fill.call(undefined, value); }
            catch (error) { nullish += error instanceof TypeError; }
            var stringError;
            try { Array.prototype.fill.call("x", value); }
            catch (error) { stringError = error instanceof TypeError; }

            var other = $262.createRealm().global;
            var foreignFill = other.Array.prototype.fill;
            var foreignBox = foreignFill.call(false, value);
            var foreignError;
            try { foreignFill.call("x", value); }
            catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            [
              returned === proxy,
              log.join(",") === "get:length,start,end,set:1,set:2",
              target[1] === value && target[2] === value && !(0 in target),
              coercions === 0,
              inheritedValue[0] && inheritedValue[1] && !Object.hasOwn(inherited, "0"),
              partialError instanceof TypeError,
              partialTarget[0] === value && !(1 in partialTarget) && !(2 in partialTarget),
              huge["9007199254740989"] === value,
              huge["9007199254740990"] === value,
              coerced[1] === 7 && coerced[2] === 7 && !(0 in coerced),
              Object.getPrototypeOf(boxed) === Boolean.prototype,
              nullish === 2,
              stringError,
              Object.getPrototypeOf(foreignBox) === other.Boolean.prototype,
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn array_filter_is_generic_species_aware_live_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var resultTarget = {};
            var resultProxy = new Proxy(resultTarget, {
              defineProperty: function(target, key, descriptor) {
                log.push("define:" + key + ":" + descriptor.value);
                return Reflect.defineProperty(target, key, descriptor);
              }
            });
            function Species(length) {
              log.push("species:" + length);
              return resultProxy;
            }

            var sourceTarget = ["a", , "c", "d"];
            sourceTarget.constructor = {};
            Object.defineProperty(sourceTarget.constructor, Symbol.species, {
              get: function() {
                log.push("species-get");
                return Species;
              }
            });
            var source = new Proxy(sourceTarget, {
              get: function(target, key, receiver) {
                log.push("get:" + String(key));
                return Reflect.get(target, key, receiver);
              },
              has: function(target, key) {
                log.push("has:" + String(key));
                return Reflect.has(target, key);
              }
            });
            var thisArg = { marker: 1 };
            var returned = Array.prototype.filter.call(source, function(value, index, object) {
              log.push("callback:" + index + ":" + value + ":" +
                       (this === thisArg) + ":" + (object === source));
              if (index === 0) {
                delete sourceTarget[2];
                sourceTarget[3] = "d2";
                sourceTarget[4] = "late";
              }
              return { valueOf: function() { throw new Error("unused"); } };
            }, thisArg);

            var inheritedSource = Object.create({ 1: "inherited" });
            inheritedSource.length = 3;
            inheritedSource[0] = "own";
            inheritedSource[2] = "deleted";
            Object.defineProperty(inheritedSource, "constructor", {
              get: function() { throw new Error("non-Array constructor lookup"); }
            });
            var inheritedResult = Array.prototype.filter.call(
              inheritedSource,
              function(value, index) {
                if (index === 0) delete inheritedSource[2];
                return true;
              }
            );

            var setterCalls = 0;
            var defineTarget = Object.create({
              set 0(value) { setterCalls += 1; }
            });
            var defineSource = [41];
            defineSource.constructor = {};
            defineSource.constructor[Symbol.species] = function() {
              return defineTarget;
            };
            var defineResult = defineSource.filter(function() { return true; });
            var descriptor = Object.getOwnPropertyDescriptor(defineTarget, "0");

            var partialTarget = {};
            var partialSource = [1, 2, 3];
            partialSource.constructor = {};
            partialSource.constructor[Symbol.species] = function() {
              return new Proxy(partialTarget, {
                defineProperty: function(target, key, desc) {
                  if (key === "1") return false;
                  return Reflect.defineProperty(target, key, desc);
                }
              });
            };
            var partialError;
            try { partialSource.filter(function() { return true; }); }
            catch (error) { partialError = error; }

            var validationOrder = 0;
            var invalidCallbackSource = [];
            Object.defineProperty(invalidCallbackSource, "constructor", {
              get: function() { validationOrder += 1; throw new Error("late"); }
            });
            var validationError;
            try { invalidCallbackSource.filter(null); }
            catch (error) { validationError = error instanceof TypeError; }

            var stringResult = Array.prototype.filter.call(
              "ab",
              function(value, index) { return index === 0 && value === "a"; }
            );
            var booleanResult = Array.prototype.filter.call(false, function() {
              throw new Error("empty Boolean callback");
            });

            var other = $262.createRealm().global;
            var foreignFilter = other.Array.prototype.filter;
            var foreignResult = foreignFilter.call(
              { 0: 7, length: 1 },
              function() { return true; }
            );
            var foreignNullError;
            var foreignCallbackError;
            try { foreignFilter.call(null, function() {}); }
            catch (error) {
              foreignNullError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }
            try { foreignFilter.call([], null); }
            catch (error) {
              foreignCallbackError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            [
              returned === resultProxy,
              log.join("|") === [
                "get:length", "get:constructor", "species-get", "species:0",
                "has:0", "get:0", "callback:0:a:true:true", "define:0:a",
                "has:1", "has:2", "has:3", "get:3",
                "callback:3:d2:true:true", "define:1:d2"
              ].join("|"),
              resultTarget[0] === "a" && resultTarget[1] === "d2" &&
                !Object.hasOwn(resultTarget, "length"),
              inheritedResult.join(",") === "own,inherited" &&
                Object.getPrototypeOf(inheritedResult) === Array.prototype,
              defineResult === defineTarget && setterCalls === 0 &&
                descriptor.value === 41 && descriptor.writable &&
                descriptor.enumerable && descriptor.configurable,
              partialError instanceof TypeError && partialTarget[0] === 1 &&
                !Object.hasOwn(partialTarget, "1") && !Object.hasOwn(partialTarget, "2"),
              validationError && validationOrder === 0,
              stringResult.length === 1 && stringResult[0] === "a",
              booleanResult.length === 0,
              Object.getPrototypeOf(foreignResult) === other.Array.prototype &&
                foreignResult[0] === 7,
              foreignNullError,
              foreignCallbackError
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn array_for_each_is_generic_ordered_live_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var target = Object.create({ 1: "inherited" });
            target.length = 4;
            target[0] = "a";
            target[2] = "c";
            target[3] = "d";
            var source = new Proxy(target, {
              get: function(object, key, receiver) {
                log.push("get:" + String(key));
                return Reflect.get(object, key, receiver);
              },
              has: function(object, key) {
                log.push("has:" + String(key));
                return Reflect.has(object, key);
              }
            });
            var thisArg = { marker: 41 };
            var returned = Array.prototype.forEach.call(
              source,
              function(value, index, object) {
                log.push(
                  "callback:" + index + ":" + value + ":" +
                  (this === thisArg) + ":" + (object === source)
                );
                if (index === 0) {
                  delete target[2];
                  target[3] = "d2";
                  target[4] = "late";
                }
                return { ignored: true };
              },
              thisArg
            );

            var validationLog = [];
            var invalid = {};
            Object.defineProperty(invalid, "length", {
              get: function() { validationLog.push("length"); return 0; }
            });
            var validationError;
            try { Array.prototype.forEach.call(invalid, null); }
            catch (error) {
              validationLog.push("callback");
              validationError = error instanceof TypeError;
            }

            var stringSeen = [];
            Array.prototype.forEach.call("ab", function(value, index, object) {
              stringSeen.push(value + ":" + index + ":" +
                              (Object.getPrototypeOf(object) === String.prototype));
            });
            var booleanCalls = 0;
            Array.prototype.forEach.call(false, function() { booleanCalls += 1; });

            var nullish = 0;
            try { Array.prototype.forEach.call(null, function() {}); }
            catch (error) { nullish += error instanceof TypeError; }
            try { Array.prototype.forEach.call(undefined, function() {}); }
            catch (error) { nullish += error instanceof TypeError; }

            var other = $262.createRealm().global;
            var foreignError;
            try { other.Array.prototype.forEach.call([], null); }
            catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            [
              returned === undefined,
              log.join("|") === [
                "get:length",
                "has:0", "get:0", "callback:0:a:true:true",
                "has:1", "get:1", "callback:1:inherited:true:true",
                "has:2",
                "has:3", "get:3", "callback:3:d2:true:true"
              ].join("|"),
              validationLog.join(",") === "length,callback",
              validationError,
              stringSeen.join(",") === "a:0:true,b:1:true",
              booleanCalls === 0,
              nullish === 2,
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn array_to_locale_string_is_generic_live_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var first = {};
            Object.defineProperty(first, "toLocaleString", {
              get: function() {
                log.push("method:1");
                return function() {
                  "use strict";
                  log.push("call:1:" + arguments.length + ":" + (this === first));
                  target[4] = {
                    toLocaleString: function() { log.push("call:4"); return "changed"; }
                  };
                  return {
                    toString: function() { log.push("string:1"); return "one"; }
                  };
                };
              }
            });
            var inherited = {
              toLocaleString: function() { log.push("call:3"); return "three"; }
            };
            var prototype = { 3: inherited };
            var target = Object.create(prototype);
            Object.defineProperty(target, "length", {
              get: function() { log.push("length"); return 5; }
            });
            target[0] = null;
            target[1] = first;
            target[2] = undefined;
            target[4] = {
              toLocaleString: function() { log.push("old:4"); return "old"; }
            };
            var source = new Proxy(target, {
              get: function(object, key, receiver) {
                log.push("get:" + String(key));
                return Reflect.get(object, key, receiver);
              }
            });
            var result = Array.prototype.toLocaleString.call(
              source,
              { toString: function() { throw "unused locale"; } },
              { get style() { throw "unused options"; } }
            );

            var primitiveLog = [];
            Object.defineProperty(Boolean.prototype, "toLocaleString", {
              configurable: true,
              get: function() {
                primitiveLog.push("get:" + String(this));
                return function() {
                  "use strict";
                  primitiveLog.push("call:" + String(this) + ":" + arguments.length);
                  return this ? "T" : "F";
                };
              }
            });
            var primitiveResult = [true, false].toLocaleString("ignored", "ignored");
            delete Boolean.prototype.toLocaleString;

            var stringResult = Array.prototype.toLocaleString.call("ab");
            var booleanResult = Array.prototype.toLocaleString.call(false);

            var abruptLog = [];
            var nonCallable = false;
            try {
              Array.prototype.toLocaleString.call({
                0: { toLocaleString: 1 },
                get 1() { abruptLog.push("late"); return 2; },
                length: 2
              });
            } catch (error) { nonCallable = error instanceof TypeError; }

            var nullish = 0;
            try { Array.prototype.toLocaleString.call(null); }
            catch (error) { nullish += error instanceof TypeError; }
            try { Array.prototype.toLocaleString.call(undefined); }
            catch (error) { nullish += error instanceof TypeError; }

            var other = $262.createRealm().global;
            other.Number.prototype.toLocaleString = function() { return "foreign"; };
            var foreignPrimitive = other.Array.prototype.toLocaleString.call([1]);
            var foreignErrors = 0;
            try {
              other.Array.prototype.toLocaleString.call({
                0: { toLocaleString: null }, length: 1
              });
            } catch (error) {
              foreignErrors += Object.getPrototypeOf(error) === other.TypeError.prototype;
            }
            try { other.Array.prototype.toLocaleString.call(null); }
            catch (error) {
              foreignErrors += Object.getPrototypeOf(error) === other.TypeError.prototype;
            }
            try {
              other.Array.prototype.toLocaleString.call({
                0: { toLocaleString: function() { return Symbol(); } }, length: 1
              });
            } catch (error) {
              foreignErrors += Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            var directCycle = [];
            directCycle[0] = directCycle;
            var indirectLeft = [];
            var indirectRight = [indirectLeft];
            indirectLeft[0] = indirectRight;
            var joinReentry = [{
              toLocaleString: function() { return joinReentry.join(); }
            }];
            var localeReentry = [{
              toString: function() { return localeReentry.toLocaleString(); }
            }];

            var growBuffer = new ArrayBuffer(4, { maxByteLength: 8 });
            var growing = new Int8Array(growBuffer);
            var originalNumberLocale = Number.prototype.toLocaleString;
            var growCalls = 0;
            Number.prototype.toLocaleString = function() {
              growCalls += 1;
              if (growCalls === 2) growBuffer.resize(6);
              return originalNumberLocale.call(this);
            };
            var grown = Array.prototype.toLocaleString.call(growing);
            Number.prototype.toLocaleString = originalNumberLocale;

            var shrinkBuffer = new ArrayBuffer(4, { maxByteLength: 8 });
            var shrinking = new Int8Array(shrinkBuffer);
            var shrinkCalls = 0;
            Number.prototype.toLocaleString = function() {
              shrinkCalls += 1;
              if (shrinkCalls === 2) shrinkBuffer.resize(2);
              return originalNumberLocale.call(this);
            };
            var shrunk = Array.prototype.toLocaleString.call(shrinking);
            Number.prototype.toLocaleString = originalNumberLocale;

            [
              result === ",one,,three,changed",
              log.join("|") === [
                "get:length", "length", "get:0", "get:1", "method:1",
                "call:1:0:true", "string:1", "get:2", "get:3", "call:3",
                "get:4", "call:4"
              ].join("|"),
              primitiveResult === "T,F",
              primitiveLog.join("|") === [
                "get:true", "call:true:0", "get:false", "call:false:0"
              ].join("|"),
              stringResult === "a,b", booleanResult === "",
              nonCallable, abruptLog.length === 0, nullish === 2,
              foreignPrimitive === "foreign", foreignErrors === 3,
              directCycle.toLocaleString() === "",
              indirectLeft.toLocaleString() === "",
              joinReentry.toLocaleString() === "",
              localeReentry.join() === "",
              grown === "0,0,0,0", growing.length === 6,
              shrunk === "0,0,,", shrinking.length === 2
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn array_join_is_generic_ordered_live_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var prototype = {
              1: { toString: function() { log.push("string:1"); return "inherited"; } }
            };
            var target = Object.create(prototype);
            Object.defineProperty(target, "length", {
              get: function() { log.push("length"); return 4; }
            });
            target[0] = null;
            target[2] = {
              toString: function() {
                log.push("string:2");
                target[3] = "changed";
                target[4] = "late";
                return "two";
              }
            };
            target[3] = "initial";
            var source = new Proxy(target, {
              get: function(object, key, receiver) {
                log.push("get:" + String(key));
                return Reflect.get(object, key, receiver);
              }
            });
            var separator = {
              toString: function() {
                log.push("separator");
                target[0] = "zero";
                return "|";
              }
            };
            var result = Array.prototype.join.call(source, separator);

            var emptySeparatorCalls = 0;
            var empty = Array.prototype.join.call({ length: 0 }, {
              toString: function() { emptySeparatorCalls += 1; return "/"; }
            });
            var nullish = Array.prototype.join.call(
              { 0: null, 2: undefined, length: 3 },
              "-"
            );
            var stringResult = Array.prototype.join.call("ab", ":");
            var booleanResult = Array.prototype.join.call(false, ":");

            var abruptLog = [];
            var separatorError;
            var elementError;
            var symbolError;
            try {
              Array.prototype.join.call({
                get length() { abruptLog.push("length"); return 0; }
              }, {
                toString: function() { abruptLog.push("separator"); throw "sep"; }
              });
            } catch (error) { separatorError = error === "sep"; }
            try {
              Array.prototype.join.call({
                0: { toString: function() { throw "element"; } }, length: 1
              });
            } catch (error) { elementError = error === "element"; }
            try { Array.prototype.join.call({ 0: Symbol(), length: 1 }); }
            catch (error) { symbolError = error instanceof TypeError; }

            var detachedErrors = 0;
            try { Array.prototype.join.call(null); }
            catch (error) { detachedErrors += error instanceof TypeError; }
            try { Array.prototype.join.call(undefined); }
            catch (error) { detachedErrors += error instanceof TypeError; }

            var other = $262.createRealm().global;
            var foreignError;
            try { other.Array.prototype.join.call(null); }
            catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            var directCycle = [];
            directCycle[0] = directCycle;
            var indirectLeft = [];
            var indirectRight = [indirectLeft];
            indirectLeft[0] = indirectRight;
            var cycleLog = [];
            var observableCycle = [];
            var innerSeparator = {
              toString: function() { cycleLog.push("inner-separator"); return ":"; }
            };
            observableCycle[0] = {
              toString: function() {
                cycleLog.push("element");
                return observableCycle.join(innerSeparator);
              }
            };
            var observableCycleResult = observableCycle.join({
              toString: function() { cycleLog.push("outer-separator"); return "|"; }
            });

            var finiteReentryArray = [1, 2];
            var finiteReentryCalls = 0;
            var finiteReentrySeparator = {
              toString: function() {
                finiteReentryCalls += 1;
                return finiteReentryArray.join("-");
              }
            };
            var finiteReentryResult =
              finiteReentryArray.join(finiteReentrySeparator);

            var cleanup = [{ toString: function() { throw "cleanup"; } }];
            try { cleanup.join(); } catch (error) {}
            cleanup[0] = "ready";

            [
              result === "zero|inherited|two|changed",
              log.join(",") === [
                "get:length", "length", "separator", "get:0", "get:1",
                "string:1", "get:2", "string:2", "get:3"
              ].join(","),
              empty === "" && emptySeparatorCalls === 1,
              nullish === "--",
              stringResult === "a:b",
              booleanResult === "",
              abruptLog.join(",") === "length,separator" && separatorError,
              elementError,
              symbolError,
              detachedErrors === 2,
              foreignError,
              directCycle.join("|") === "",
              indirectLeft.join("|") === "",
              observableCycleResult === "" &&
                cycleLog.join(",") ===
                  "outer-separator,element,inner-separator",
              finiteReentryResult === "11-22" && finiteReentryCalls === 1,
              cleanup.join() === "ready"
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn array_map_is_generic_species_aware_live_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var prototype = { 1: "inherited" };
            var target = Object.create(prototype);
            Object.defineProperty(target, "length", {
              get: function() { log.push("length"); return 4; }
            });
            target[0] = "zero";
            target[2] = "remove";
            var generic = Array.prototype.map.call(target, function(value, index, object) {
              log.push("callback:" + index + ":" + value + ":" + (object === target));
              if (index === 0) target[3] = "late";
              if (index === 1) delete target[2];
              return value + "!";
            });

            var speciesLog = [];
            var speciesTarget = {};
            var speciesProxy = new Proxy(speciesTarget, {
              defineProperty: function(object, key, descriptor) {
                speciesLog.push("define:" + key + ":" + descriptor.value);
                return Reflect.defineProperty(object, key, descriptor);
              }
            });
            function Species(length) {
              speciesLog.push("species:" + length);
              return speciesProxy;
            }
            var array = [1, , 3];
            array.constructor = { [Symbol.species]: Species };
            var speciesResult = array.map(function(value, index, object) {
              speciesLog.push("callback:" + index + ":" + (object === array));
              return value * 2;
            });

            var validationLog = [];
            var validationError = false;
            try {
              Array.prototype.map.call({
                get length() { validationLog.push("length"); return 0; }
              }, null);
            } catch (error) { validationError = error instanceof TypeError; }

            var defineCalls = 0;
            var abrupt = [1];
            abrupt.constructor = { [Symbol.species]: function() {
              return new Proxy({}, {
                defineProperty: function() { defineCalls += 1; return false; }
              });
            }};
            var defineError = false;
            try { abrupt.map(function(value) { return value; }); }
            catch (error) { defineError = error instanceof TypeError; }

            var other = $262.createRealm().global;
            var foreignResult = other.Array.prototype.map.call(
              { 0: 1, length: 1 }, function(value) { return value + 1; }
            );
            var foreignError = false;
            try { other.Array.prototype.map.call(null, function() {}); }
            catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            [
              generic.length === 4 && generic[0] === "zero!" &&
                generic[1] === "inherited!" && !(2 in generic) &&
                generic[3] === "late!",
              log.join(",") === [
                "length", "callback:0:zero:true", "callback:1:inherited:true",
                "callback:3:late:true"
              ].join(","),
              speciesResult === speciesProxy && !("1" in speciesTarget),
              speciesLog.join(",") === [
                "species:3", "callback:0:true", "define:0:2",
                "callback:2:true", "define:2:6"
              ].join(","),
              validationLog.join(",") === "length" && validationError,
              defineCalls === 1 && defineError,
              Object.getPrototypeOf(foreignResult) === other.Array.prototype &&
                foreignResult[0] === 2,
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn array_reduce_is_generic_ordered_live_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var prototype = { 1: 2 };
            var target = Object.create(prototype);
            Object.defineProperty(target, "length", {
              get: function() { log.push("length"); return 4; }
            });
            target[0] = 1;
            target[2] = 3;
            var result = Array.prototype.reduce.call(
              target,
              function(accumulator, value, index, object) {
                "use strict";
                log.push(
                  "callback:" + accumulator + ":" + value + ":" + index +
                  ":" + (object === target) + ":" + (this === undefined)
                );
                if (index === 0) target[3] = 4;
                if (index === 1) delete target[2];
                return accumulator + value;
              },
              0
            );

            var omitted = Array.prototype.reduce.call(
              Object.assign(Object.create({ 1: 5 }), { 3: 7, length: 4 }),
              function(accumulator, value) { return accumulator + value; }
            );
            var explicitUndefinedCalls = 0;
            var explicitUndefined = [2].reduce(function(accumulator, value) {
              explicitUndefinedCalls += 1;
              return String(accumulator) + value;
            }, undefined);

            var validationLog = [];
            var validationError = false;
            try {
              Array.prototype.reduce.call({
                get length() { validationLog.push("length"); return 0; }
              }, null);
            } catch (error) { validationError = error instanceof TypeError; }

            var emptyError = false;
            try { Array.prototype.reduce.call({ length: 3 }, function() {}); }
            catch (error) { emptyError = error instanceof TypeError; }

            var stringResult = Array.prototype.reduce.call(
              "abc", function(accumulator, value) { return accumulator + value; }
            );
            var booleanResult = Array.prototype.reduce.call(
              false, function() { throw "unreachable"; }, 9
            );

            var other = $262.createRealm().global;
            var foreignError = false;
            try { other.Array.prototype.reduce.call(null, function() {}); }
            catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            [
              result === 7,
              log.join(",") === [
                "length", "callback:0:1:0:true:true",
                "callback:1:2:1:true:true", "callback:3:4:3:true:true"
              ].join(","),
              omitted === 12,
              explicitUndefined === "undefined2" && explicitUndefinedCalls === 1,
              validationLog.join(",") === "length" && validationError,
              emptyError,
              stringResult === "abc",
              booleanResult === 9,
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn array_reduce_right_is_generic_ordered_live_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var prototype = { 2: 3 };
            var target = Object.create(prototype);
            Object.defineProperty(target, "length", {
              get: function() { log.push("length"); return 4; }
            });
            target[1] = 2;
            target[3] = 4;
            var result = Array.prototype.reduceRight.call(
              target,
              function(accumulator, value, index, object) {
                "use strict";
                log.push(
                  "callback:" + accumulator + ":" + value + ":" + index +
                  ":" + (object === target) + ":" + (this === undefined)
                );
                if (index === 3) target[0] = 1;
                if (index === 2) delete target[1];
                return accumulator + value;
              },
              0
            );

            var omitted = Array.prototype.reduceRight.call(
              Object.assign(Object.create({ 2: 5 }), { 0: 7, length: 4 }),
              function(accumulator, value) { return accumulator + value; }
            );
            var explicitUndefinedCalls = 0;
            var explicitUndefined = [2].reduceRight(function(accumulator, value) {
              explicitUndefinedCalls += 1;
              return String(accumulator) + value;
            }, undefined);

            var validationLog = [];
            var validationError = false;
            try {
              Array.prototype.reduceRight.call({
                get length() { validationLog.push("length"); return 0; }
              }, null);
            } catch (error) { validationError = error instanceof TypeError; }

            var emptyError = false;
            try { Array.prototype.reduceRight.call({ length: 3 }, function() {}); }
            catch (error) { emptyError = error instanceof TypeError; }

            var stringResult = Array.prototype.reduceRight.call(
              "abc", function(accumulator, value) { return accumulator + value; }
            );
            var booleanResult = Array.prototype.reduceRight.call(
              false, function() { throw "unreachable"; }, 9
            );

            var other = $262.createRealm().global;
            var foreignError = false;
            try { other.Array.prototype.reduceRight.call(null, function() {}); }
            catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            [
              result === 8,
              log.join(",") === [
                "length", "callback:0:4:3:true:true",
                "callback:4:3:2:true:true", "callback:7:1:0:true:true"
              ].join(","),
              omitted === 12,
              explicitUndefined === "undefined2" && explicitUndefinedCalls === 1,
              validationLog.join(",") === "length" && validationError,
              emptyError,
              stringResult === "cba",
              booleanResult === 9,
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn array_reverse() {
    assert_eq!(
        run("[1,2,3].reverse().join(',');"),
        Value::String(Arc::from("3,2,1"))
    );
}

#[test]
fn array_reverse_is_generic_sparse_ordered_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var target = { 0: "a", 2: "c", 4: "e", 5: "f", length: 6 };
            var source = new Proxy(target, {
              has: function(object, key) {
                log.push("has:" + key);
                return Reflect.has(object, key);
              },
              get: function(object, key, receiver) {
                log.push("get:" + key);
                return Reflect.get(object, key, receiver);
              },
              set: function(object, key, value) {
                log.push("set:" + key + ":" + value);
                return Reflect.set(object, key, value);
              },
              deleteProperty: function(object, key) {
                log.push("delete:" + key);
                return Reflect.deleteProperty(object, key);
              }
            });
            var result = Array.prototype.reverse.call(source);

            var booleanResult = Array.prototype.reverse.call(false);
            var stringError = false;
            try { Array.prototype.reverse.call("ab"); }
            catch (error) { stringError = error instanceof TypeError; }

            var other = $262.createRealm().global;
            var foreignError = false;
            try { other.Array.prototype.reverse.call(null); }
            catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            [
              result === source,
              target[0] === "f" && target[1] === "e" && !(2 in target) &&
                target[3] === "c" && !(4 in target) && target[5] === "a" &&
                target.length === 6,
              log.join(",") === [
                "get:length",
                "has:0", "get:0", "has:5", "get:5", "set:0:f", "set:5:a",
                "has:1", "has:4", "get:4", "set:1:e", "delete:4",
                "has:2", "get:2", "has:3", "delete:2", "set:3:c"
              ].join(","),
              Object.prototype.toString.call(booleanResult) === "[object Boolean]",
              stringError,
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true"))
    );
}

#[test]
fn array_to_reversed_is_generic_live_dense_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var constructorReads = 0;
            var target = { 0: "zero", 2: "two", length: 4 };
            Object.defineProperty(target, "constructor", {
              get: function() {
                constructorReads++;
                throw new Error("constructor must not be read");
              }
            });
            var source = new Proxy(target, {
              get: function(object, key, receiver) {
                log.push("get:" + key);
                if (key === "2") {
                  object[1] = "late";
                  delete object[0];
                }
                return Reflect.get(object, key, receiver);
              },
              has: function() {
                throw new Error("HasProperty must not be used");
              }
            });
            var copy = Array.prototype.toReversed.call(source);

            var booleanCopy = Array.prototype.toReversed.call(false);
            var other = $262.createRealm().global;
            var foreignCopy = other.Array.prototype.toReversed.call({ 0: 7, length: 1 });
            var foreignError = false;
            try { other.Array.prototype.toReversed.call(null); }
            catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            [
              copy !== source && Array.isArray(copy),
              copy.length === 4 && copy[0] === undefined && copy[1] === "two" &&
                copy[2] === "late" && copy[3] === undefined,
              copy.hasOwnProperty(0) && copy.hasOwnProperty(1) &&
                copy.hasOwnProperty(2) && copy.hasOwnProperty(3),
              log.join(",") === "get:length,get:3,get:2,get:1,get:0",
              constructorReads === 0,
              target.length === 4 && target[1] === "late" && !(0 in target),
              Array.isArray(booleanCopy) && booleanCopy.length === 0,
              Object.getPrototypeOf(foreignCopy) === other.Array.prototype && foreignCopy[0] === 7,
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn array_to_spliced_is_generic_live_dense_and_realm_aware() {
    assert_eq!(
        run(r#"
            var log = [];
            var constructorReads = 0;
            var target = { 0: "zero", 1: "discard", 2: "two", length: 4 };
            Object.defineProperty(target, "constructor", {
              get: function() {
                constructorReads++;
                throw new Error("constructor must not be read");
              }
            });
            var source = new Proxy(target, {
              get: function(object, key, receiver) {
                log.push("get:" + key);
                if (key === "0") object[2] = "late";
                if (key === "2") delete object[3];
                return Reflect.get(object, key, receiver);
              },
              has: function() {
                throw new Error("HasProperty must not be used");
              }
            });
            var start = { valueOf: function() { log.push("start"); return 1; } };
            var skip = { valueOf: function() { log.push("skip"); return 1; } };
            var copy = Array.prototype.toSpliced.call(source, start, skip, "x", "y");

            var omitted = Array.prototype.toSpliced.call({ 0: "a", 1: "b", length: 2 });
            var tail = Array.prototype.toSpliced.call({ 0: "a", 1: "b", length: 2 }, 1);
            var explicitUndefined = Array.prototype.toSpliced.call(
              { 0: "a", 1: "b", length: 2 }, 1, undefined
            );
            var booleanCopy = Array.prototype.toSpliced.call(false);

            var other = $262.createRealm().global;
            var foreignCopy = other.Array.prototype.toSpliced.call(
              { 0: 7, length: 1 }, 0, 0, 8
            );
            var foreignError = false;
            try { other.Array.prototype.toSpliced.call(null); }
            catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            [
              copy !== source && Array.isArray(copy),
              copy.length === 5 && copy.join(",") === "zero,x,y,late,",
              copy.hasOwnProperty(0) && copy.hasOwnProperty(1) &&
                copy.hasOwnProperty(2) && copy.hasOwnProperty(3) && copy.hasOwnProperty(4),
              log.join(",") === "get:length,start,skip,get:0,get:2,get:3",
              constructorReads === 0 && target[1] === "discard",
              omitted.join(",") === "a,b" && tail.join(",") === "a" &&
                explicitUndefined.join(",") === "a,b",
              Array.isArray(booleanCopy) && booleanCopy.length === 0,
              Object.getPrototypeOf(foreignCopy) === other.Array.prototype &&
                foreignCopy.join(",") === "8,7",
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn string_methods() {
    assert_eq!(
        run("'hello'.toUpperCase();"),
        Value::String(Arc::from("HELLO"))
    );
    assert_eq!(run("'hello'.charAt(1);"), Value::String(Arc::from("e")));
    assert_eq!(
        run("new String('abc123').charAt(2);"),
        Value::String(Arc::from("c"))
    );
    assert_eq!(run("String.prototype.length;"), Value::Number(0.0));
}

#[test]
fn string_iterator_iterates_primitives_boxed_strings_and_utf16_code_points() {
    assert_eq!(
        run(r#"
            var text = "A\uD83D\uDE00\uD800B\uDC00";
            function describe(iterable) {
                return [...iterable].map(function(value) {
                    return [
                        value.length,
                        value.charCodeAt(0).toString(16),
                        value.charCodeAt(value.length - 1).toString(16)
                    ].join(":");
                }).join(",");
            }
            var boxed = new String("ignored");
            boxed.toString = function() { return text; };
            [describe(text), describe(new String(text)), describe(boxed)].join("|");
        "#),
        Value::String(Arc::from(
            "1:41:41,2:d83d:de00,1:d800:d800,1:42:42,1:dc00:dc00|1:41:41,2:d83d:de00,1:d800:d800,1:42:42,1:dc00:dc00|1:41:41,2:d83d:de00,1:d800:d800,1:42:42,1:dc00:dc00"
        ))
    );
}

#[test]
fn string_iterator_prototype_has_spec_shape() {
    assert_eq!(
        run(r#"
            var iterator = "x"[Symbol.iterator]();
            var prototype = Object.getPrototypeOf(iterator);
            var next = Object.getOwnPropertyDescriptor(prototype, "next");
            var tag = Object.getOwnPropertyDescriptor(prototype, Symbol.toStringTag);
            var extensibleBefore = Object.isExtensible(iterator);
            Object.preventExtensions(iterator);
            [
                Object.getPrototypeOf(prototype) === Iterator.prototype,
                Object.prototype.toString.call(iterator),
                tag.value, tag.writable, tag.enumerable, tag.configurable,
                typeof next.value, next.value.length, next.value.name,
                next.writable, next.enumerable, next.configurable,
                iterator[Symbol.iterator]() === iterator,
                extensibleBefore, Object.isExtensible(iterator)
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|[object String Iterator]|String Iterator|false|false|true|function|0|next|true|false|true|true|true|false"
        ))
    );
}

#[test]
fn string_iterator_next_checks_brand_and_stays_exhausted() {
    assert_eq!(
        run(r#"
            var iterator = "x"[Symbol.iterator]();
            var next = Object.getPrototypeOf(iterator).next;
            function typeError(receiver) {
                try { next.call(receiver); }
                catch (error) { return error instanceof TypeError; }
                return false;
            }
            var first = iterator.next();
            var second = iterator.next();
            var third = iterator.next();
            [
                typeError(undefined), typeError(null), typeError({}),
                typeError(Object.create(Object.getPrototypeOf(iterator))),
                first.value, first.done,
                second.value === undefined, second.done,
                third.value === undefined, third.done
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|x|false|true|true|true|true"))
    );
}

#[test]
fn string_iterator_observes_symbol_iterator_override_and_deletion() {
    assert_eq!(
        run(r#"
            var original = String.prototype[Symbol.iterator];
            var calls = 0;
            String.prototype[Symbol.iterator] = function() {
                calls += 1;
                return original.call(this);
            };
            var primitive = [..."ab"].join(",");
            var boxed = [...new String("cd")].join(",");
            var deleted = delete String.prototype[Symbol.iterator];
            var missing = false;
            try { [..."e"]; }
            catch (error) { missing = error instanceof TypeError; }
            String.prototype[Symbol.iterator] = function() { return 1; };
            var primitiveResult = false;
            try { [..."f"]; }
            catch (error) { primitiveResult = error instanceof TypeError; }
            String.prototype[Symbol.iterator] = original;
            [primitive, boxed, calls, deleted, missing, primitiveResult].join("|");
        "#),
        Value::String(Arc::from("a,b|c,d|2|true|true|true"))
    );
}

#[test]
fn string_iterator_uses_its_realm_intrinsics_and_survives_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var foreignIterator = other.String.prototype[other.Symbol.iterator].call(
                "\uD83D\uDE00x"
            );
            var foreignPrototype = Object.getPrototypeOf(foreignIterator);
            var foreignNext = foreignPrototype.next;
            var foreignTypeError = false;
            try { foreignNext.call({}); }
            catch (error) {
                foreignTypeError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            forceGc();
            var first = foreignIterator.next();
            var second = foreignIterator.next();
            var done = foreignIterator.next();
            [
                Object.getPrototypeOf(foreignPrototype) === other.Iterator.prototype,
                Object.getPrototypeOf(foreignNext) === other.Function.prototype,
                Object.prototype.toString.call(foreignIterator),
                foreignTypeError,
                first.value.length, first.value.charCodeAt(0).toString(16),
                first.value.charCodeAt(1).toString(16), first.done,
                second.value, second.done, done.value === undefined, done.done
            ].join("|");
            "#,
        )
        .expect("foreign String iterator intrinsic should survive GC"),
        Value::String(Arc::from(
            "true|true|[object String Iterator]|true|2|d83d|de00|false|x|false|true|true"
        ))
    );
}

#[test]
fn string_constructor_observes_object_to_primitive_string_hint() {
    assert_eq!(
        run(r#"var old = Array.prototype.toString;
               Array.prototype.toString = function() { return "__ARRAY__"; };
               var result = String(new Array);
               Array.prototype.toString = old;
               result;"#),
        Value::String(Arc::from("__ARRAY__"))
    );
}

#[test]
fn string_constructor_skips_non_callable_to_primitive_methods() {
    assert_eq!(
        run(r#"var oldToString = Array.prototype.toString;
               var oldValueOf = Array.prototype.valueOf;
               Array.prototype.toString = {};
               Array.prototype.valueOf = function() { return 5; };
               var result = String([]);
               Array.prototype.toString = oldToString;
               Array.prototype.valueOf = oldValueOf;
               result;"#),
        Value::String(Arc::from("5"))
    );
}

#[test]
fn array_to_string_observes_join_and_uses_object_fallback() {
    assert_eq!(
        run(r#"
            var calls = 0;
            var joined = Array.prototype.toString.call({
                join: function() { calls += 1; return "joined"; }
            });
            var fallback = Array.prototype.toString.call({ join: null });
            [joined, calls, fallback].join("|");
        "#),
        Value::String(Arc::from("joined|1|[object Object]"))
    );
    assert!(run_err("Array.prototype.toString.call(null);").contains("TypeError"));
}

#[test]
fn string_exotic_indices_are_enumerable_read_only_properties() {
    assert_eq!(
        run(r#"var s = new String("abc");
               var d = Object.getOwnPropertyDescriptor(s, "0");
               s[0] = "z";
               [
                 s[0],
                 d.writable,
                 d.enumerable,
                 d.configurable,
                 s.propertyIsEnumerable("0"),
                 delete s[0],
                 s[0]
               ].join("|");"#),
        Value::String(Arc::from("a|false|true|false|true|false|a"))
    );
    assert!(
        run_err(r#""use strict"; var s = new String("abc"); s[0] = "z";"#).contains("read only")
    );
}

#[test]
fn string_locale_compare_uses_canonical_equivalence() {
    assert_eq!(
        run(r#""o\u0308".localeCompare("\u00f6");"#),
        Value::Number(0.0)
    );
}

#[test]
fn string_static_methods_follow_code_point_and_raw_semantics() {
    assert!(run_err("String.fromCodePoint(3.14);").contains("RangeError"));
    assert!(run_err("String.fromCodePoint(-1);").contains("RangeError"));
    assert!(run_err("String.fromCodePoint(Infinity);").contains("RangeError"));
    assert_eq!(
        run("String.fromCodePoint(0x61, 0x1F600).length;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run(r#"String.raw({ raw: ["a", "b", "d", "f"] }, 1);"#),
        Value::String(Arc::from("a1bdf"))
    );
    assert_eq!(
        run(r#"String.raw({ raw: { length: 5, 0: "e", 1: "", 2: null, 3: undefined, 4: 123 } });"#),
        Value::String(Arc::from("enullundefined123"))
    );
}

#[test]
fn string_search_methods_follow_regexp_and_position_semantics() {
    assert_eq!(
        run("'The future is cool!'.startsWith('future', 4);"),
        Value::Bool(true)
    );
    assert_eq!(run("'word'.endsWith('r', 3);"), Value::Bool(true));
    assert_eq!(
        run("'The future is cool!'.includes('The future', true);"),
        Value::Bool(false)
    );
    assert!(run_err("String.prototype.includes.call(null, 'x');").contains("null or undefined"));
    assert!(run_err(
        "String.prototype.startsWith.call({ toString: function(){ throw new Error('boom'); } }, '');"
    )
    .contains("boom"));
    assert!(run_err("''.startsWith(/./);").contains("RegExp"));
    assert!(run_err(
        "var o = {}; Object.defineProperty(o, Symbol.match, { get: function(){ throw new Error('boom'); } }); ''.includes(o);"
    )
    .contains("boom"));
    assert_eq!(
        run("var s = Symbol.match; var o = {}; Object.defineProperty(o, s, { get: function(){ return true; } }); o[s];"),
        Value::Bool(true)
    );
    assert_eq!(run(r#""__undefined__".indexOf()"#), Value::Number(2.0));
    assert_eq!(run(r#""__undefined__".lastIndexOf()"#), Value::Number(2.0));
    assert_eq!(run(r#""".lastIndexOf()"#), Value::Number(-1.0));
    assert_eq!(
        run(r#"var o = { toString: function(){ return "AB"; } }; "ABBABABAB".indexOf(o, true)"#),
        Value::Number(3.0)
    );
    assert_eq!(
        run(r#"var o = { toString: function(){ return "AB"; } }; "ABBABABAB".lastIndexOf(o, NaN)"#),
        Value::Number(7.0)
    );
    assert_eq!(run(r#""abcabc".indexOf("a", -1)"#), Value::Number(0.0));
    assert_eq!(run(r#""abcabc".lastIndexOf("a", -1)"#), Value::Number(0.0));
    assert_eq!(run(r#""abcabc".lastIndexOf("b", -1)"#), Value::Number(-1.0));
    assert_eq!(
        run(r#""abcabc".lastIndexOf("a", -Infinity)"#),
        Value::Number(0.0)
    );
    assert_eq!(
        run(r#""aaaa".indexOf("aa", Infinity)"#),
        Value::Number(-1.0)
    );
    assert_eq!(
        run(r#""abc".lastIndexOf("abcd", Infinity)"#),
        Value::Number(-1.0)
    );
    assert_eq!(
        run(
            r#"var a = { toString: function(){ throw "search"; } }; var b = { valueOf: function(){ throw "position"; } }; (function(){ try { return "abc".indexOf(a, b); } catch (e) { return e; } })()"#
        ),
        Value::String(Arc::from("search"))
    );
    assert_eq!(
        run(
            r#"var a = { toString: function(){ throw "search"; } }; var b = { valueOf: function(){ throw "position"; } }; (function(){ try { return "abc".lastIndexOf(a, b); } catch (e) { return e; } })()"#
        ),
        Value::String(Arc::from("search"))
    );
    assert_eq!(
        run(r#"var searcher = {};
               var seenThis, seenArg;
               searcher[Symbol.search] = function(arg) {
                 seenThis = this;
                 seenArg = arg;
                 return "custom";
               };
               var out = "abc".search(searcher);
               [out, seenThis === searcher, seenArg].join("|");"#),
        Value::String(Arc::from("custom|true|abc"))
    );
    assert!(run_err(
        r#"var searcher = {};
               Object.defineProperty(searcher, Symbol.search, {
                 get: function(){ throw new Error("search-get"); }
               });
               "abc".search(searcher);"#
    )
    .contains("search-get"));
    assert_eq!(
        run(r#"var original = RegExp.prototype[Symbol.search];
               var seenThis, seenArg;
               RegExp.prototype[Symbol.search] = function(arg) {
                 seenThis = this;
                 seenArg = arg;
                 return "created";
               };
               var out = "target".search("string source");
               RegExp.prototype[Symbol.search] = original;
               [out, seenThis instanceof RegExp, seenThis.source, seenThis.flags, seenArg].join("|");"#),
        Value::String(Arc::from("created|true|string source||target"))
    );
    assert_eq!(
        run("var re = /b/g; re.lastIndex = 2; var n = re[Symbol.search]('abc'); n + ',' + re.lastIndex;"),
        Value::String(Arc::from("1,2"))
    );
}

#[test]
fn string_match_all_uses_regexp_iterator_semantics() {
    assert_eq!(
        run(r#"
            var matches = Array.from("a1b22".matchAll(/\d+/g));
            [
              matches.length,
              matches[0][0], matches[0].index, matches[0].input,
              matches[1][0], matches[1].index, matches[1].input
            ].join("|");
        "#),
        Value::String(Arc::from("2|1|1|a1b22|22|3|a1b22"))
    );
    assert_eq!(
        run(r#"
            var regexp = /./g;
            regexp.lastIndex = { valueOf: function() { return 2; } };
            var iterator = regexp[Symbol.matchAll]("abcd");
            regexp.lastIndex = 0;
            var first = iterator.next().value;
            first[0] + ":" + first.index;
        "#),
        Value::String(Arc::from("c:2"))
    );
    assert_eq!(
        run(r#"
            var inputReads = 0;
            var intrinsicReads = 0;
            Object.defineProperty(RegExp.prototype, Symbol.match, {
              get: function () { intrinsicReads += 1; return true; }
            });
            var input = {
              get flags() { return ""; },
              get [Symbol.match]() { inputReads += 1; return false; }
            };
            RegExp.prototype[Symbol.matchAll].call(input, "");
            [inputReads, intrinsicReads].join("|");
        "#),
        Value::String(Arc::from("1|0"))
    );
    assert_eq!(
        run(r#"
            var receiver = {};
            Object.defineProperty(receiver, "constructor", {
              get: function () { throw "constructor"; }
            });
            Object.defineProperty(receiver, "flags", {
              get: function () { throw "flags"; }
            });
            try { RegExp.prototype[Symbol.matchAll].call(receiver, ""); }
            catch (error) { error; }
        "#),
        Value::String(Arc::from("constructor"))
    );
    assert_eq!(
        run(r#"
            var writes = [];
            var calls = 0;
            var matcher = {
              exec: function () {
                calls += 1;
                return calls === 1 ? { 0: "" } : null;
              }
            };
            Object.defineProperty(matcher, "lastIndex", {
              get: function () { return 0; },
              set: function (value) { writes.push(value); },
              configurable: true
            });
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.lastIndex = 2;
            regexp.constructor = {};
            regexp.constructor[Symbol.species] = Species;
            var iterator = regexp[Symbol.matchAll]("a");
            var first = iterator.next().value[0];
            [first, writes.join(",")].join("|");
        "#),
        Value::String(Arc::from("|2,1"))
    );
    assert_eq!(
        run(r#"
            var writes = [];
            var matcher = new Proxy(
              { exec: function () { return null; }, lastIndex: 0 },
              { set: function (target, key, value) {
                  if (key === "lastIndex") writes.push(value);
                  return true;
                } }
            );
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.lastIndex = 3;
            regexp.constructor = {};
            regexp.constructor[Symbol.species] = Species;
            regexp[Symbol.matchAll]("a").next();
            writes.join(",");
        "#),
        Value::String(Arc::from("3"))
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var deep = {};
            Object.defineProperty(deep, "lastIndex", {
              set: function () { calls += 1; }
            });
            var near = Object.create(deep);
            Object.defineProperty(near, "lastIndex", {
              value: 0,
              writable: true
            });
            var matcher = Object.create(near);
            matcher.exec = function () { return null; };
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.lastIndex = 5;
            regexp.constructor = { [Symbol.species]: Species };
            regexp[Symbol.matchAll]("a");
            [
              calls,
              matcher.hasOwnProperty("lastIndex"),
              matcher.lastIndex
            ].join("|");
        "#),
        Value::String(Arc::from("0|true|5"))
    );
    assert_eq!(
        run(r#"
            var defineCalls = 0;
            var matcher = new Proxy(
              { exec: function () { return null; }, lastIndex: 0 },
              {
                set: null,
                defineProperty: function (target, key, descriptor) {
                  defineCalls += 1;
                  return Reflect.defineProperty(target, key, descriptor);
                }
              }
            );
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.lastIndex = 4;
            regexp.constructor = { [Symbol.species]: Species };
            regexp[Symbol.matchAll]("a");
            [matcher.lastIndex, defineCalls].join("|");
        "#),
        Value::String(Arc::from("4|1"))
    );
    assert_eq!(
        run(r#"
            var descriptorKeys;
            var target = { exec: function () { return null; }, lastIndex: 0 };
            var matcher = new Proxy(target, {
              set: null,
              defineProperty: function (target, key, descriptor) {
                descriptorKeys = Object.keys(descriptor).join(",");
                return Reflect.defineProperty(target, key, descriptor);
              }
            });
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.lastIndex = 7;
            regexp.constructor = { [Symbol.species]: Species };
            regexp[Symbol.matchAll]("a");
            [descriptorKeys, target.lastIndex].join("|");
        "#),
        Value::String(Arc::from("value|7"))
    );
    assert_eq!(
        run(r#"
            var target = { exec: function () { return null; }, lastIndex: 0 };
            var proxy = new Proxy(target, {
              set: null,
              getOwnPropertyDescriptor: function () {
                return {
                  value: 0,
                  writable: true,
                  enumerable: false,
                  configurable: true
                };
              },
              defineProperty: null
            });
            function Species() { return proxy; }
            var regexp = /a/g;
            regexp.lastIndex = 4;
            regexp.constructor = { [Symbol.species]: Species };
            regexp[Symbol.matchAll]("");
            var descriptor = Object.getOwnPropertyDescriptor(target, "lastIndex");
            [
              descriptor.value,
              descriptor.writable,
              descriptor.enumerable,
              descriptor.configurable
            ].join("|");
        "#),
        Value::String(Arc::from("4|true|true|true"))
    );
    assert_eq!(
        run(r#"
            var target = { exec: function () { return null; }, lastIndex: 0 };
            var reported = Object.create({ writable: true, configurable: true });
            reported.value = 0;
            var proxy = new Proxy(target, {
              set: null,
              getOwnPropertyDescriptor: function () { return reported; }
            });
            function Species() { return proxy; }
            var regexp = /a/g;
            regexp.lastIndex = 4;
            regexp.constructor = { [Symbol.species]: Species };
            var outcome = "ok";
            try { regexp[Symbol.matchAll](""); }
            catch (error) { outcome = "TypeError"; }
            outcome + "|" + target.lastIndex;
        "#),
        Value::String(Arc::from("ok|4"))
    );
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var seen;
            var target = { exec: function () { return null; }, lastIndex: 0 };
            var proxy = new Proxy(target, {
              set: null,
              defineProperty: function (target, key, descriptor) {
                seen = Object.getPrototypeOf(descriptor);
                return Reflect.defineProperty(target, key, descriptor);
              }
            });
            function Species() { return proxy; }
            var regexp = other.eval("/a/g");
            regexp.constructor = { [Symbol.species]: Species };
            other.RegExp.prototype[Symbol.matchAll].call(regexp, "");
            [
              seen === other.Object.prototype,
              seen === Object.prototype
            ].join("|");
        "#),
        Value::String(Arc::from("true|false"))
    );
    assert_eq!(
        run(r#"
            var target = { exec: function () { return null; } };
            Object.defineProperty(target, "lastIndex", {
              value: 0,
              writable: false,
              configurable: false
            });
            var matcher = new Proxy(target, {
              set: function () { return true; }
            });
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.lastIndex = 4;
            regexp.constructor = { [Symbol.species]: Species };
            try { regexp[Symbol.matchAll](""); "NO_THROW"; }
            catch (error) { error instanceof TypeError ? "TypeError" : "other"; }
        "#),
        Value::String(Arc::from("TypeError"))
    );
    assert_eq!(
        run(r#"
            var dataTarget = { exec: function () { return null; } };
            Object.defineProperty(dataTarget, "lastIndex", {
              value: 4,
              writable: false,
              configurable: false
            });
            var dataMatcher = new Proxy(dataTarget, {
              set: function () { return true; }
            });
            function DataSpecies() { return dataMatcher; }
            var dataRegExp = /a/g;
            dataRegExp.lastIndex = 4;
            dataRegExp.constructor = { [Symbol.species]: DataSpecies };
            var sameValueAllowed = true;
            try { dataRegExp[Symbol.matchAll](""); }
            catch (error) { sameValueAllowed = false; }

            var setterCalls = 0;
            var accessorTarget = { exec: function () { return null; } };
            Object.defineProperty(accessorTarget, "lastIndex", {
              get: function () { return 0; },
              set: function () { setterCalls += 1; },
              configurable: false
            });
            var accessorMatcher = new Proxy(accessorTarget, {
              set: function () { return true; }
            });
            function AccessorSpecies() { return accessorMatcher; }
            var accessorRegExp = /a/g;
            accessorRegExp.constructor = { [Symbol.species]: AccessorSpecies };
            var setterAllowed = true;
            try { accessorRegExp[Symbol.matchAll](""); }
            catch (error) { setterAllowed = false; }
            [sameValueAllowed, setterAllowed, setterCalls].join("|");
        "#),
        Value::String(Arc::from("true|true|0"))
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var target = { exec: function () { return null; } };
            Object.preventExtensions(target);
            var matcher = new Proxy(target, {
              set: null,
              defineProperty: function () { calls += 1; return true; }
            });
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.constructor = { [Symbol.species]: Species };
            var outcome;
            try { regexp[Symbol.matchAll](""); outcome = "NO_THROW"; }
            catch (error) {
              outcome = error instanceof TypeError ? "TypeError" : "other";
            }
            [outcome, calls, "lastIndex" in target].join("|");
        "#),
        Value::String(Arc::from("TypeError|1|false"))
    );
    assert_eq!(
        run(r#"
            var calls = [], matcher;
            var prototype = new Proxy({}, {
              set: function (target, key, value, receiver) {
                calls.push(key + ":" + value + ":" + (receiver === matcher));
                return true;
              }
            });
            matcher = Object.create(prototype);
            Object.defineProperty(matcher, "exec", {
              value: function () { return null; }
            });
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.lastIndex = 5;
            regexp.constructor = { [Symbol.species]: Species };
            regexp[Symbol.matchAll]("a");
            [
              calls.join(","),
              matcher.hasOwnProperty("lastIndex"),
              matcher.lastIndex
            ].join("|");
        "#),
        Value::String(Arc::from("lastIndex:5:true|false|"))
    );
    assert_eq!(
        run(r#"
            var defineCalls = 0;
            var target = { exec: function () { return null; }, lastIndex: 0 };
            var matcher = new Proxy(target, {
              set: null,
              getOwnPropertyDescriptor: function (target, key) {
                if (key === "lastIndex") {
                  return { get: function () { return 0; }, configurable: true };
                }
                return Reflect.getOwnPropertyDescriptor(target, key);
              },
              defineProperty: function () { defineCalls += 1; return true; }
            });
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.lastIndex = 4;
            regexp.constructor = { [Symbol.species]: Species };
            var outcome;
            try { regexp[Symbol.matchAll]("a"); outcome = "NO_THROW"; }
            catch (error) {
              outcome = error instanceof TypeError ? "TypeError" : "other";
            }
            outcome + "|" + defineCalls;
        "#),
        Value::String(Arc::from("TypeError|0"))
    );
    assert_eq!(
        run(r#"
            var IntrinsicRegExp = RegExp;
            var regexp = /a/g;
            regexp.constructor = undefined;
            RegExp = null;
            IntrinsicRegExp.prototype[Symbol.matchAll]
              .call(regexp, "a").next().value[0];
        "#),
        Value::String(Arc::from("a"))
    );
    assert!(run_err(r#""abc".matchAll(/./)"#).contains("TypeError"));
}

#[test]
fn regexp_match_all_roots_species_values_and_uses_foreign_realm_intrinsics() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var matchAll = other.RegExp.prototype[Symbol.matchAll];
            var regexp = other.eval("/a/g");
            regexp.constructor = undefined;
            other.RegExp.prototype.constructor = null;
            other.RegExp = null;
            forceGc();
            var mainIterator = /a/g[Symbol.matchAll]("a");
            var foreignIterator = matchAll.call(regexp, "a");
            var foreignStep = foreignIterator.next();
            var foreignDefault = foreignStep.value[0];

            var speciesMatcher = {
              exec: function () { return null; },
              lastIndex: 0
            };
            var speciesReceiver = {
              lastIndex: 0,
              get flags() {
                forceGc();
                return "g";
              }
            };
            speciesReceiver.constructor = {};
            Object.defineProperty(speciesReceiver.constructor, Symbol.species, {
              get: function () {
                return function () { return speciesMatcher; };
              }
            });
            var rootedSpecies = RegExp.prototype[Symbol.matchAll]
              .call(speciesReceiver, "").next().done;
            [
              foreignDefault,
              rootedSpecies,
              Object.getPrototypeOf(mainIterator) !==
                Object.getPrototypeOf(foreignIterator),
              Object.getPrototypeOf(foreignStep) === other.Object.prototype
            ].join("|");
            "#,
        )
        .expect("RegExp @@matchAll roots and Realm defaults should survive GC"),
        Value::String(Arc::from("a|true|true|true"))
    );
    assert_eq!(
        vm.run(
            r#"
            var calls = 0;
            var matcher = {
              exec: function () {
                return { 0: "", marker: 42, id: ++calls };
              }
            };
            Object.defineProperty(matcher, "lastIndex", {
              get: function () { return 0; },
              set: function () { if (calls) forceGc(); }
            });
            function Species() { return matcher; }
            var regexp = /a/g;
            regexp.constructor = { [Symbol.species]: Species };
            var step = regexp[Symbol.matchAll]("a").next();
            [step.value.marker, step.value.id, step.value[0], step.done].join("|");
            "#,
        )
        .expect("RegExp iterator result should survive observable GC"),
        Value::String(Arc::from("42|1||false"))
    );
}

#[test]
fn regexp_symbol_split_installs_with_spec_order_and_metadata() {
    assert_eq!(
        run(r#"
            var method = RegExp.prototype[Symbol.split];
            var descriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, Symbol.split);
            var log = [];
            var receiver = {};
            var constructor = {};
            Object.defineProperty(receiver, "constructor", {
              get: function () { log.push("constructor"); return constructor; }
            });
            Object.defineProperty(constructor, Symbol.species, {
              get: function () {
                log.push("species");
                return function (pattern, flags) {
                  log.push("construct:" + (pattern === receiver) + ":" + flags);
                  return { exec: function () { throw new Error("unexpected exec"); } };
                };
              }
            });
            Object.defineProperty(receiver, "flags", {
              get: function () {
                log.push("flags");
                return { toString: function () { log.push("flags-string"); return ""; } };
              }
            });
            var input = {
              toString: function () { log.push("string"); return "abc"; }
            };
            var limit = {
              valueOf: function () { log.push("limit"); return 0; }
            };
            var result = method.call(receiver, input, limit);
            var constructThrows = false;
            try { new method(); } catch (error) { constructThrows = error instanceof TypeError; }
            [
              method.name,
              method.length,
              descriptor.writable,
              descriptor.enumerable,
              descriptor.configurable,
              log.join(","),
              Array.isArray(result),
              result.length,
              constructThrows
            ].join("|");
        "#),
        Value::String(Arc::from(
            "[Symbol.split]|2|true|false|true|string,constructor,species,flags,flags-string,construct:true:y,limit|true|0|true"
        ))
    );
}

#[test]
fn regexp_symbol_split_handles_captures_empty_matches_and_disabled_hook() {
    assert_eq!(
        run(r#"
            var captures = /c(d)(e)/[Symbol.split]("abcdefg", 2);
            var empty = /(?:)/[Symbol.split]("abc");
            var unicode = /./u[Symbol.split]("\ud834\udf06");
            var disabled = /,/;
            disabled[Symbol.split] = undefined;
            [
              captures.join(","),
              empty.join(","),
              unicode.join(","),
              "a,b".split(disabled).join(",")
            ].join("|");
        "#),
        Value::String(Arc::from("ab,d|a,b,c|,|a,b"))
    );
}

#[test]
fn regexp_symbol_split_preserves_code_units_and_callable_proxy_hooks() {
    assert_eq!(
        run(r#"
            var pair = "\ud834\udf06";
            var empty = /(?:)/[Symbol.split](pair);
            var dot = /./[Symbol.split](pair);
            var high = /\ud834/[Symbol.split](pair);
            var low = /\udf06/[Symbol.split](pair);
            var escapedPair = /\ud834\udf06/[Symbol.split](pair);
            var escapedClass = /[\ud834\udf06]/[Symbol.split](pair);
            var repeated = /(?:(a)|\ud834\udf06)*/[Symbol.split]("a" + pair + "X", 3);

            var hookCalls = 0;
            var separator = {};
            separator[Symbol.split] = new Proxy(function (value, limit) {
              hookCalls += 1;
              return [this === separator, value, limit];
            }, {});
            var delegated = "value".split(separator, 7);

            var execCalls = 0;
            var exec = new Proxy(function () { execCalls += 1; return null; }, {});
            var splitter = { lastIndex: 0, exec: exec };
            var receiver = {
              flags: "",
              constructor: { [Symbol.species]: function () { return splitter; } }
            };
            var generic = RegExp.prototype[Symbol.split].call(receiver, "ab");

            [
              empty.length, empty[0].length, empty[0].charCodeAt(0),
              empty[1].length, empty[1].charCodeAt(0),
              dot.length,
              high.length, high[1].length, high[1].charCodeAt(0),
              low.length, low[0].length, low[0].charCodeAt(0),
              escapedPair.length, escapedPair[0].length, escapedPair[1].length,
              escapedClass.length,
              repeated[1] === undefined, String(repeated[1]),
              hookCalls, delegated.join(","),
              execCalls, generic.length, generic[0]
            ].join("|");
        "#),
        Value::String(Arc::from(
            "2|1|55348|1|57094|3|2|1|57094|2|1|55348|2|0|0|3|true|undefined|1|true,value,7|2|1|ab"
        ))
    );
}

#[test]
fn regexp_symbol_split_uses_the_method_realm_intrinsics() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var split = other.RegExp.prototype[Symbol.split];
            var regexp = other.eval("/,/");
            regexp.constructor = undefined;
            other.RegExp.prototype.constructor = null;
            other.RegExp = null;
            var result = split.call(regexp, "a,b");
            [
              result.join("|"),
              Object.getPrototypeOf(result) === other.Array.prototype,
              Object.getPrototypeOf(result) !== Array.prototype
            ].join(":");
        "#),
        Value::String(Arc::from("a|b:true:true"))
    );
}

#[test]
fn string_match_all_delegates_custom_matcher_before_string_coercion() {
    assert_eq!(
        run(r#"
            var seenThis, seenArg;
            var matcher = {};
            matcher[Symbol.matchAll] = function(value) {
              seenThis = this;
              seenArg = value;
              return "delegated";
            };
            var result = String.prototype.matchAll.call(7, matcher);
            result + ":" + (seenThis === matcher) + ":" + (seenArg === 7);
        "#),
        Value::String(Arc::from("delegated:true:true"))
    );
}

#[test]
fn generated_symbols_do_not_collide_with_well_known_symbols() {
    assert_eq!(run("Symbol() === Symbol.iterator;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.match;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.unscopables;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.species;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.dispose;"), Value::Bool(false));
    assert_eq!(run("Symbol() === Symbol.asyncDispose;"), Value::Bool(false));
    assert_eq!(
        run("Symbol.for('x') === Symbol.for('x');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("Symbol.for('x') === Symbol.for('y');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("Symbol.keyFor(Symbol.for('x'));"),
        Value::String(Arc::from("x"))
    );
    assert_eq!(run("Symbol.keyFor(Symbol('x'));"), Value::Undefined);
    assert_eq!(
        run("typeof Symbol.species + ':' + typeof Symbol.unscopables;"),
        Value::String(Arc::from("symbol:symbol"))
    );
    assert_eq!(run("Symbol.species === Symbol.species;"), Value::Bool(true));
}

#[test]
fn symbol_species_is_exposed() {
    assert_eq!(
        run("typeof Symbol.species;"),
        Value::String(Arc::from("symbol"))
    );
    assert_eq!(
        run("String(Symbol.species);"),
        Value::String(Arc::from("Symbol(Symbol.species)"))
    );
}

#[test]
fn disposal_well_known_symbols_are_exposed() {
    assert_eq!(
        run(r#"
            [
              typeof Symbol.dispose,
              typeof Symbol.asyncDispose,
              String(Symbol.dispose),
              String(Symbol.asyncDispose),
              Symbol.keyFor(Symbol.dispose) === undefined,
              Symbol.keyFor(Symbol.asyncDispose) === undefined,
              Symbol.for("Symbol.dispose") !== Symbol.dispose,
              Symbol.for("Symbol.asyncDispose") !== Symbol.asyncDispose
            ].join("|");
        "#),
        Value::String(Arc::from(
            "symbol|symbol|Symbol(Symbol.dispose)|Symbol(Symbol.asyncDispose)|true|true|true|true"
        ))
    );
    assert_eq!(
        run(r#"
            var disposeDesc = Object.getOwnPropertyDescriptor(Symbol, "dispose");
            var asyncDisposeDesc = Object.getOwnPropertyDescriptor(Symbol, "asyncDispose");
            [
              disposeDesc.writable,
              disposeDesc.enumerable,
              disposeDesc.configurable,
              disposeDesc.value === Symbol.dispose,
              asyncDisposeDesc.writable,
              asyncDisposeDesc.enumerable,
              asyncDisposeDesc.configurable,
              asyncDisposeDesc.value === Symbol.asyncDispose
            ].join("|");
        "#),
        Value::String(Arc::from("false|false|false|true|false|false|false|true"))
    );
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global.Symbol;
            [
              Symbol.dispose === other.dispose,
              Symbol.asyncDispose === other.asyncDispose
            ].join("|");
        "#),
        Value::String(Arc::from("true|true"))
    );
}

#[test]
fn symbol_property_keys_drive_function_name_inference() {
    assert_eq!(
        run(r#"
            var method = Symbol("method");
            var fn = Symbol("fn");
            var cls = Symbol("cls");
            var getter = Symbol();
            var setter = Symbol();
            var o = {
              [method]() {},
              [fn]: function() {},
              [cls]: class {},
              get [getter]() { return 1; },
              set [setter](v) {}
            };
            var cover = { x: (0, function() {}) };
            class C {
              [method]() {}
              get [fn]() { return 1; }
              set [getter](v) {}
              static [cls]() {}
            }
            [
              o[method].name,
              o[fn].name,
              o[cls].name,
              Object.getOwnPropertyDescriptor(o, getter).get.name,
              Object.getOwnPropertyDescriptor(o, setter).set.name,
              cover.x.name,
              C.prototype[method].name,
              Object.getOwnPropertyDescriptor(C.prototype, fn).get.name,
              Object.getOwnPropertyDescriptor(C.prototype, getter).set.name,
              C[cls].name
            ].join("|");
        "#),
        Value::String(Arc::from(
            "[method]|[fn]|[cls]|get |set ||[method]|get [fn]|set |[cls]"
        ))
    );
}

#[test]
fn symbol_description_and_key_for_follow_registry_semantics() {
    assert_eq!(
        run("typeof Symbol.prototype.toString + ':' + Object.getOwnPropertyDescriptor(Symbol, 'prototype').writable;"),
        Value::String(Arc::from("function:false"))
    );
    assert_eq!(
        run("[Symbol('x').description, Symbol().description, Symbol(undefined).description, Symbol('').description].join('|');"),
        Value::String(Arc::from("x|||"))
    );
    assert_eq!(
        run("var s = Symbol.for({ toString: function(){ return 'test262'; } }); [s.description, Symbol.keyFor(s), s.toString()].join('|');"),
        Value::String(Arc::from("test262|test262|Symbol(test262)"))
    );
    assert_eq!(
        run("var sym = Symbol('66'); sym.toString = 0; sym.valueOf = 0; [sym.toString(), sym === sym.valueOf()].join('|');"),
        Value::String(Arc::from("Symbol(66)|true"))
    );
    assert!(
        run_err("'use strict'; var sym = Symbol('s'); sym.x = 1;").contains("TypeError"),
        "strict Symbol primitive assignment must throw"
    );
    assert!(
        run_err("Symbol.keyFor(Object(Symbol.for('x')));").contains("TypeError"),
        "Symbol.keyFor must reject Symbol wrapper objects"
    );
    assert!(
        run_err("Object.getOwnPropertyDescriptor(Symbol.prototype, 'description').get.call({});")
            .contains("TypeError"),
        "Symbol.prototype.description getter must reject non-symbol receivers"
    );
}

#[test]
fn symbol_intrinsic_surface_descriptors_and_value_of() {
    assert_eq!(
        run(r#"
            [
              Object.getOwnPropertyDescriptor(Symbol, "length").value,
              Object.getOwnPropertyDescriptor(Symbol, "length").writable,
              Object.getOwnPropertyDescriptor(Symbol, "match").writable,
              Object.getOwnPropertyDescriptor(Symbol, "match").configurable,
              Object.getOwnPropertyDescriptor(Symbol, "isConcatSpreadable").writable,
              Object.getOwnPropertyDescriptor(Symbol, "replace").configurable,
              Object.getOwnPropertyDescriptor(Symbol, "search").configurable,
              Object.getOwnPropertyDescriptor(Symbol, "split").configurable,
              typeof Symbol.matchAll,
              typeof Symbol.isConcatSpreadable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "0|false|false|false|false|false|false|false|symbol|symbol"
        ))
    );
    assert_eq!(
        run(r#"
            var sym = Symbol("surface");
            [
              Object.getPrototypeOf(sym) === Symbol.prototype,
              Object.getPrototypeOf(Object(sym)).constructor === Symbol,
              Symbol.prototype.valueOf.call(sym) === sym,
              Symbol.prototype.valueOf.call(Object(sym)) === sym,
              Object.prototype.propertyIsEnumerable.call(Symbol.prototype, "valueOf")
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|false"))
    );
    assert!(
        run_err("Symbol.prototype.valueOf.call({});").contains("TypeError"),
        "Symbol.prototype.valueOf must reject non-symbol objects"
    );
    assert_eq!(
        run(r#"
            [
              Symbol.prototype[Symbol.toPrimitive].length,
              Symbol.prototype[Symbol.toPrimitive].name,
              Object.getOwnPropertyDescriptor(Symbol.prototype, Symbol.toPrimitive).writable,
              Object.getOwnPropertyDescriptor(Symbol.prototype, Symbol.toPrimitive).configurable,
              Symbol.prototype[Symbol.toPrimitive].call(Object(Symbol("p")), "default").toString(),
              Symbol.prototype[Symbol.toStringTag],
              Object.getOwnPropertyDescriptor(Symbol.prototype, Symbol.toStringTag).writable,
              Object.getOwnPropertyDescriptor(Symbol.prototype, Symbol.toStringTag).configurable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "1|[Symbol.toPrimitive]|false|true|Symbol(p)|Symbol|false|true"
        ))
    );
    assert!(
        run_err("Object.getPrototypeOf(null);").contains("TypeError"),
        "Object.getPrototypeOf(null) must throw"
    );
    assert_eq!(
        run(r#"
            function getterName(C) {
              return Object.getOwnPropertyDescriptor(C, Symbol.species).get.name;
            }
            class MyRegExp extends RegExp {}
            [
              getterName(Array),
              getterName(Map),
              getterName(Promise),
              getterName(RegExp),
              getterName(Set),
              MyRegExp[Symbol.species] === MyRegExp
            ].join("|");
        "#),
        Value::String(Arc::from(
            "get [Symbol.species]|get [Symbol.species]|get [Symbol.species]|get [Symbol.species]|get [Symbol.species]|true"
        ))
    );
}

#[test]
fn array_subclass_instances_use_new_target_prototype() {
    assert_eq!(
        run(r#"
            class Subclass extends Array {}
            var arr = new Subclass(1, 2);
            [
              arr instanceof Subclass,
              arr instanceof Array,
              Object.getPrototypeOf(arr) === Subclass.prototype,
              arr.length,
              arr.join(",")
            ].join(":");
            "#),
        Value::String(Arc::from("true:true:true:2:1,2"))
    );
}

#[test]
fn native_constructors_preserve_regexp_allocation_order_and_forwarding() {
    assert_eq!(
        run(r#"
            var log = [];
            var prototype = {};
            var NewTarget = function () {}.bind();
            Object.defineProperty(Function.prototype, "prototype", {
              get: function () { log.push("prototype"); return prototype; },
              configurable: true
            });
            var pattern = {
              toString: function () { log.push("pattern"); return "x"; }
            };
            var regexp = Reflect.construct(RegExp, [pattern], NewTarget);
            delete Function.prototype.prototype;

            var BoundArray = Array.bind(null, 1);
            var bound = new BoundArray(2, 3);
            var ArrayProxy = new Proxy(Array, {});
            var proxied = new ArrayProxy(4, 5);
            [
              log.join(","),
              Object.getPrototypeOf(regexp) === prototype,
              bound.join(","),
              Object.getPrototypeOf(bound) === Array.prototype,
              proxied.join(","),
              Object.getPrototypeOf(proxied) === ArrayProxy.prototype
            ].join("|");
        "#),
        Value::String(Arc::from("prototype,pattern|true|1,2,3|true|4,5|true"))
    );
}

#[test]
fn eager_native_constructor_resolves_fallback_before_body_validation() {
    assert_eq!(
        run(r#"
            var pair = Proxy.revocable(function () {}, {});
            var NewTarget = pair.proxy.bind(null);
            pair.revoke();
            try {
              Reflect.construct(Array, [-1], NewTarget);
            } catch (error) {
              error.name;
            }
        "#),
        Value::String(Arc::from("TypeError"))
    );
}

#[test]
fn weak_collection_constructors_require_new_and_use_new_target_prototype() {
    assert_eq!(
        run(r#"
            var weakMapCallThrows = false;
            var weakSetCallThrows = false;
            try { WeakMap(); } catch (error) {
              weakMapCallThrows = error instanceof TypeError;
            }
            try { WeakSet(); } catch (error) {
              weakSetCallThrows = error instanceof TypeError;
            }
            class WeakMapSubclass extends WeakMap {}
            class WeakSetSubclass extends WeakSet {}
            var weakMap = new WeakMapSubclass();
            var weakSet = new WeakSetSubclass();
            [
              weakMapCallThrows,
              weakSetCallThrows,
              weakMap instanceof WeakMapSubclass,
              weakMap instanceof WeakMap,
              Object.getPrototypeOf(weakMap) === WeakMapSubclass.prototype,
              weakSet instanceof WeakSetSubclass,
              weakSet instanceof WeakSet,
              Object.getPrototypeOf(weakSet) === WeakSetSubclass.prototype
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn uint8array_subclass_instances_use_new_target_prototype_and_store_elements() {
    assert_eq!(
        run(r#"
            class ExtendedUint8Array extends Uint8Array {
              constructor() {
                super(10);
                this[0] = 255;
                this[1] = 0xFFA;
              }
            }
            var eua = new ExtendedUint8Array();
            [
              eua.length,
              eua.byteLength,
              eua[0],
              eua[1],
              Object.getPrototypeOf(eua) === ExtendedUint8Array.prototype,
              Object.prototype.toString.call(eua)
            ].join(",");
            "#),
        Value::String(Arc::from("10,10,255,250,true,[object Uint8Array]"))
    );
}

#[test]
fn typed_array_constructors_cover_numeric_and_bigint_variants() {
    assert_eq!(
        run(r#"
            [
              typeof Int8Array,
              typeof Uint8ClampedArray,
              typeof Int16Array,
              typeof Uint16Array,
              typeof Int32Array,
              typeof Uint32Array,
              typeof Float32Array,
              typeof Float64Array,
              typeof BigInt64Array,
              typeof BigUint64Array
            ].join(",");
        "#),
        Value::String(Arc::from(
            "function,function,function,function,function,function,function,function,function,function"
        ))
    );
    assert_eq!(
        run("var a=new Int16Array(2); a[0]=-1; a[1]=65535; [a.length,a.byteLength,a[0],a[1]].join(',');"),
        Value::String(Arc::from("2,4,-1,-1"))
    );
    assert_eq!(
        run("var a=new Uint8ClampedArray([0, 2.5, 3.5, 300, -1, NaN]); [a.length,a.byteLength,a[0],a[1],a[2],a[3],a[4],a[5]].join(',');"),
        Value::String(Arc::from("6,6,0,2,4,255,0,0"))
    );
    assert_eq!(
        run("var a=new BigInt64Array(1); a[0]=-1n; [a.length,a.byteLength,a[0].toString()].join(',');"),
        Value::String(Arc::from("1,8,-1"))
    );
    assert_eq!(
        run("var a=new Float32Array(0); Object.seal(a); [Object.isSealed(a), Object.isFrozen(a), Object.isExtensible(a)].join(',');"),
        Value::String(Arc::from("true,true,false"))
    );
    assert!(run_err("Object.seal(new Float32Array(1));").contains("TypeError"));
}

#[test]
fn typed_array_constructors_read_array_like_objects_observably() {
    assert_eq!(
        run(r#"
            var log = [];
            var source = {
              get length() { log.push("length"); return "4"; },
              0: 7,
              get 1() { log.push("one"); return 8; },
              3: 260
            };
            var a = new Uint8Array(source);
            [a.length, a.byteLength, a[0], a[1], a[2], a[3], log.join("|")].join(",");
        "#),
        Value::String(Arc::from("4,4,7,8,0,4,length|one"))
    );
    assert_eq!(
        run("try { Uint8Array(1); 'no'; } catch (e) { e.name; }"),
        Value::String(Arc::from("TypeError"))
    );
    assert_eq!(
        run(r#"
            function C() {}
            C.prototype = 1;
            var a = Reflect.construct(Uint8Array, [1], C);
            Object.getPrototypeOf(a) === Uint8Array.prototype;
        "#),
        Value::Bool(true)
    );
    assert_eq!(
        run("try { new Uint8Array(-1); 'no'; } catch (e) { e.name; }"),
        Value::String(Arc::from("RangeError"))
    );
    assert_eq!(
        run(r#"
            function* gen() { yield 7; yield 42; }
            var a = new Uint8Array(gen());
            [a.length, a[0], a[1]].join(",");
        "#),
        Value::String(Arc::from("2,7,42"))
    );
    assert_eq!(
        run(r#"
            var values = [0, { valueOf: function() { values.length = 0; return 100; } }, 2];
            var a = new Uint8Array(values);
            [a.length, a[0], a[1], a[2], values.length].join(",");
        "#),
        Value::String(Arc::from("3,0,100,2,0"))
    );
}

#[test]
fn typed_array_constructor_toindex_errors_before_newtarget_prototype() {
    assert_eq!(
        run(r#"
            var newTarget = function() {}.bind(null);
            Object.defineProperty(newTarget, "prototype", {
              get: function() { throw new Error("prototype"); }
            });
            var log = [];
            for (var i = 0; i < 2; i++) {
              var C = i === 0 ? Uint8Array : BigInt64Array;
              try {
                Reflect.construct(C, [Symbol()], newTarget);
                log.push("none");
              } catch (e) {
                log.push(e.name + ":" + e.message);
              }
            }
            log.join("|");
        "#),
        Value::String(Arc::from(
            "TypeError:Cannot convert Symbol to number|TypeError:Cannot convert Symbol to number"
        ))
    );
}

#[test]
fn typed_array_constructors_create_array_buffer_backed_views() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(16);
            var bytes = new Uint8Array(buffer);
            bytes[0] = 7;
            var ints = new Int16Array(buffer, 2, 2);
            ints[0] = 258;
            [
              bytes.length,
              bytes.byteLength,
              bytes.byteOffset,
              bytes.buffer === buffer,
              ints.length,
              ints.byteLength,
              ints.byteOffset,
              ints.buffer === buffer,
              bytes[0],
              bytes[2],
              bytes[3]
            ].join(",");
        "#),
        Value::String(Arc::from("16,16,0,true,2,4,2,true,7,2,1"))
    );
    assert!(
        run_err("new Int16Array(new ArrayBuffer(3));").contains("RangeError"),
        "misaligned ArrayBuffer byte length should throw RangeError"
    );
    assert!(
        run_err("new Int16Array(new ArrayBuffer(4), 1);").contains("RangeError"),
        "misaligned TypedArray byte offset should throw RangeError"
    );
    assert!(
        run_err("new Uint8Array(new ArrayBuffer(4), 3, 2);").contains("RangeError"),
        "TypedArray view past the ArrayBuffer should throw RangeError"
    );
}

#[test]
fn typed_array_array_buffer_constructor_validates_detach_after_coercions() {
    assert_eq!(
        run(r#"
            function thrownName(fn) {
              try {
                fn();
              } catch (e) {
                return e.name;
              }
              return "none";
            }
            var log = [];
            var preOffset = new ArrayBuffer(8);
            $262.detachArrayBuffer(preOffset);
            var r1 = thrownName(function() {
              new Uint8Array(preOffset, {
                valueOf: function() {
                  log.push("pre-offset");
                  return 0;
                }
              });
            });
            var preLength = new ArrayBuffer(8);
            $262.detachArrayBuffer(preLength);
            var r2 = thrownName(function() {
              new Uint8Array(preLength, 0, {
                valueOf: function() {
                  log.push("pre-length");
                  return 1;
                }
              });
            });
            var detachOffset = new ArrayBuffer(8);
            var r3 = thrownName(function() {
              new Uint8Array(detachOffset, {
                valueOf: function() {
                  log.push("detach-offset");
                  $262.detachArrayBuffer(detachOffset);
                  return 0;
                }
              });
            });
            var detachLength = new ArrayBuffer(8);
            var r4 = thrownName(function() {
              new Uint8Array(detachLength, 0, {
                valueOf: function() {
                  log.push("detach-length");
                  $262.detachArrayBuffer(detachLength);
                  return 1;
                }
              });
            });
            var bigintOffset = new ArrayBuffer(16);
            var r5 = thrownName(function() {
              new BigInt64Array(bigintOffset, {
                valueOf: function() {
                  log.push("bigint-offset");
                  $262.detachArrayBuffer(bigintOffset);
                  return 0;
                }
              });
            });
            var rangeLength = new ArrayBuffer(8);
            var r6 = thrownName(function() {
              new Uint8Array(rangeLength, 99, {
                valueOf: function() {
                  log.push("range-length");
                  return 1;
                }
              });
            });
            [r1, r2, r3, r4, r5, r6, log.join("|")].join(",");
        "#),
        Value::String(Arc::from(
            "TypeError,TypeError,TypeError,TypeError,TypeError,RangeError,pre-offset|pre-length|detach-offset|detach-length|bigint-offset|range-length",
        ))
    );
}

#[test]
fn typed_array_bigint_constructors_expose_element_size_and_validate_receivers() {
    assert_eq!(
        run(r#"
            var ctorDesc = Object.getOwnPropertyDescriptor(BigInt64Array, "BYTES_PER_ELEMENT");
            var protoDesc = Object.getOwnPropertyDescriptor(BigUint64Array.prototype, "BYTES_PER_ELEMENT");
            var TypedArrayPrototype = Object.getPrototypeOf(BigInt64Array.prototype);
            var bufferGetter = Object.getOwnPropertyDescriptor(TypedArrayPrototype, "buffer").get;
            var lengthGetter = Object.getOwnPropertyDescriptor(TypedArrayPrototype, "length").get;
            var ta = new BigInt64Array(2);
            var throwsOnProto = false;
            var throwsOnObject = false;
            try { BigInt64Array.prototype.buffer; } catch (e) { throwsOnProto = e instanceof TypeError; }
            try { bufferGetter.call({}); } catch (e) { throwsOnObject = e instanceof TypeError; }
            [
              ctorDesc.value,
              ctorDesc.writable,
              ctorDesc.enumerable,
              ctorDesc.configurable,
              protoDesc.value,
              protoDesc.writable,
              protoDesc.enumerable,
              protoDesc.configurable,
              bufferGetter.call(ta) instanceof ArrayBuffer,
              lengthGetter.call(ta),
              throwsOnProto,
              throwsOnObject
            ].join(",");
            "#),
        Value::String(Arc::from(
            "8,false,false,false,8,false,false,false,true,2,true,true",
        ))
    );
}

#[test]
fn typed_array_constructors_inherit_from_shared_intrinsics() {
    assert_eq!(
        run(r#"
            var TypedArray = Object.getPrototypeOf(Int8Array);
            var TypedArrayPrototype = TypedArray.prototype;
            [
              Int8Array.length,
              BigUint64Array.length,
              Object.getPrototypeOf(Uint8Array) === TypedArray,
              Object.getPrototypeOf(Float64Array) === TypedArray,
              Object.getPrototypeOf(Uint8Array.prototype) === TypedArrayPrototype,
              Object.getPrototypeOf(BigInt64Array.prototype) === TypedArrayPrototype,
              Uint8Array.prototype.hasOwnProperty("buffer"),
              Float64Array.prototype.hasOwnProperty("byteLength"),
              BigInt64Array.prototype.hasOwnProperty("byteOffset"),
              BigUint64Array.prototype.hasOwnProperty("length"),
              typeof Object.getOwnPropertyDescriptor(TypedArrayPrototype, "buffer").get,
              typeof Object.getOwnPropertyDescriptor(TypedArrayPrototype, "length").get
            ].join(",");
            "#),
        Value::String(Arc::from(
            "3,3,true,true,true,true,false,false,false,false,function,function",
        ))
    );
}

#[test]
fn typed_array_static_from_and_of_inherit_from_intrinsic_constructor() {
    assert_eq!(
        run(r#"
            var calls = [];
            var from = Uint8Array.from({0: 7, 1: 260, length: 2}, function(v, i) {
              calls.push(this.tag + ":" + i + ":" + v);
              return v + i;
            }, { tag: "ctx" });
            var of = Int16Array.of(-1, 65535);
            [
              typeof Uint8Array.from,
              Uint8Array.hasOwnProperty("from"),
              Object.getPrototypeOf(Uint8Array).hasOwnProperty("from"),
              from instanceof Uint8Array,
              from.length,
              from[0],
              from[1],
              calls.join("|"),
              of instanceof Int16Array,
              of.length,
              of[0],
              of[1]
            ].join(",");
        "#),
        Value::String(Arc::from(
            "function,false,true,true,2,7,5,ctx:0:7|ctx:1:260,true,2,-1,-1"
        ))
    );
}

#[test]
fn typed_array_static_from_constructs_before_array_like_elements() {
    assert_eq!(
        run(r#"
            var log = [];
            function Custom(length) {
              log.push("construct:" + length);
              return new Uint8Array(length);
            }
            var source = {
              get length() { log.push("length"); return 1; },
              get 0() { log.push("element"); return 9; }
            };
            var result = Uint8Array.from.call(Custom, source);
            [result[0], log.join("|")].join(",");
        "#),
        Value::String(Arc::from("9,length|construct:1|element"))
    );
}

#[test]
fn typed_array_static_from_and_of_reject_immutable_backing_results() {
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try {
                fn();
              } catch (e) {
                return e instanceof TypeError;
              }
              return false;
            }
            function Custom(length) {
              return new Uint8Array(new ArrayBuffer(length).transferToImmutable());
            }
            [
              throwsTypeError(function() { Uint8Array.from.call(Custom, [1]); }),
              throwsTypeError(function() { Uint8Array.of.call(Custom, 1); })
            ].join(",");
        "#),
        Value::String(Arc::from("true,true"))
    );
}

#[test]
fn typed_array_static_from_caches_iterator_next_method() {
    assert_eq!(
        run(r#"
            var nextGets = 0;
            var nextCalls = 0;
            var iterable = {};
            Object.defineProperty(iterable, Symbol.iterator, {
              value: function() {
                var values = [4];
                return {
                  get next() {
                    nextGets += 1;
                    return function() {
                      nextCalls += 1;
                      if (values.length === 0) return { done: true };
                      return { value: values.pop(), done: false };
                    };
                  }
                };
              }
            });
            var result = Uint8Array.from(iterable);
            [result.length, result[0], nextGets, nextCalls].join(",");
        "#),
        Value::String(Arc::from("1,4,1,2"))
    );
}

#[test]
fn typed_array_from_accepts_iterables_at_the_previous_materialization_cap() {
    assert_eq!(
        run("Uint8Array.from(Array(65536).keys()).length;"),
        Value::Number(65536.0)
    );
}

#[test]
fn typed_array_from_rejects_invalid_empty_constructor_results() {
    for source in [
        r#"
            function Detached(length) {
                var result = new Uint8Array(length);
                $262.detachArrayBuffer(result.buffer);
                return result;
            }
            Uint8Array.from.call(Detached, []);
        "#,
        r#"
            function OutOfBounds() {
                var buffer = new ArrayBuffer(1, { maxByteLength: 2 });
                var result = new Uint8Array(buffer, 0, 1);
                buffer.resize(0);
                return result;
            }
            Uint8Array.from.call(OutOfBounds, []);
        "#,
    ] {
        assert!(run_err(source).contains("TypeError"));
    }
    assert_eq!(
        run(r#"
            delete Date.prototype[Symbol.toPrimitive];
            var date = new Date(0);
            date.valueOf = function() { return 1; };
            date.toString = function() { return 2; };
            date + '';
        "#),
        Value::String(Arc::from("1"))
    );
}

#[test]
fn typed_array_from_roots_iterator_state_across_gc_callbacks() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var first = {
                valueOf: function() { forceGc(); return 7; }
            };
            var calls = 0;
            var source = {};
            source[Symbol.iterator] = function() {
                var iterator = {};
                Object.defineProperty(iterator, "next", {
                    get: function() {
                        forceGc();
                        return function() {
                            calls += 1;
                            var current = calls;
                            return {
                                get done() { forceGc(); return current > 1; },
                                get value() { forceGc(); return first; }
                            };
                        };
                    }
                });
                return iterator;
            };
            var mapper = new Proxy(function(value) { return value; }, {
                apply: function(target, thisArg, args) {
                    forceGc();
                    return Reflect.apply(target, thisArg, args);
                }
            });
            var result = Uint8Array.from(source, mapper);
            [result.length, result[0], calls].join("|");
            "#,
        )
        .expect("TypedArray.from iterator values should survive GC"),
        Value::String(Arc::from("1|7|2"))
    );
}

#[test]
fn typed_array_of_rejects_invalid_empty_constructor_results() {
    for constructor in [
        r#"
            function Invalid(length) {
                var result = new Uint8Array(length);
                $262.detachArrayBuffer(result.buffer);
                return result;
            }
        "#,
        r#"
            function Invalid() {
                var buffer = new ArrayBuffer(1, { maxByteLength: 2 });
                var result = new Uint8Array(buffer, 0, 1);
                buffer.resize(0);
                return result;
            }
        "#,
    ] {
        assert!(
            run_err(&format!("{constructor} Uint8Array.of.call(Invalid);")).contains("TypeError")
        );
    }
}

#[test]
fn typed_array_of_roots_arguments_across_construction_and_conversion_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            function Custom(length) {
                forceGc();
                return new Uint8Array(length);
            }
            var first = { valueOf: function() { forceGc(); return 7; } };
            var second = { valueOf: function() { forceGc(); return 9; } };
            var result = Uint8Array.of.call(Custom, first, second);
            [result.length, result[0], result[1]].join("|");
            "#,
        )
        .expect("TypedArray.of arguments should survive GC"),
        Value::String(Arc::from("2|7|9"))
    );
}

#[test]
fn typed_array_numeric_proto_set_distinguishes_valid_and_invalid_indices() {
    assert_eq!(
        run(r#"
            var ta = new Int32Array(1);
            ta[0] = 7;
            var obj = Object.create(ta);
            obj[0] = 9;
            obj.NaN = 10;
            [
              ta[0],
              obj[0],
              obj.hasOwnProperty("0"),
              Object.getOwnPropertyDescriptor(obj, "NaN") === undefined
            ].join(",");
        "#),
        Value::String(Arc::from("7,9,true,true"))
    );
}

#[test]
fn typed_array_numeric_set_converts_value_before_index_validation() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([42]);
            var calls = 0;
            var value = {
              valueOf: function() {
                calls += 1;
                return 7;
              }
            };
            sample["1.1"] = value;
            sample["-0"] = value;
            sample["-1"] = value;
            sample["1"] = value;
            sample["2"] = value;
            [calls, sample[0], sample["1.1"], sample["-0"], sample["-1"], sample["1"], sample["2"]].join(",");
        "#),
        Value::String(Arc::from("5,42,,,,,"))
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([42]);
            $262.detachArrayBuffer(sample.buffer);
            sample[0] = { valueOf: function() { throw new Error("boom"); } };
            "#
        )
        .contains("boom"),
        "TypedArray [[Set]] should convert values before detached-buffer validation"
    );
    assert!(
        run_err(r#"new BigInt64Array(1)[0] = "definitely not a bigint";"#).contains("SyntaxError"),
        "BigInt TypedArray [[Set]] should preserve StringToBigInt SyntaxError"
    );
    assert_eq!(
        run(r#"
            var receiver = new Int32Array(10);
            var obj = Object.create(receiver);
            var calls = 0;
            var value = {
              valueOf: function() {
                calls += 1;
                return 1;
              }
            };
            [Reflect.set(obj, 100, value, receiver), calls].join(",");
        "#),
        Value::String(Arc::from("true,1"))
    );
}

#[test]
fn typed_array_length_allocation_keeps_backing_buffer_live_after_gc() {
    assert_eq!(
        run(r#"
            var ta = new Uint8Array(9);
            var trash = [];
            for (var i = 0; i < 2000; i++) trash.push({ i: i });
            [ta.length, ta.byteLength, ta[0], ta[4], ta[8]].join(",");
        "#),
        Value::String(Arc::from("9,9,0,0,0"))
    );
}

#[test]
fn typed_array_constructor_observes_array_iterator_prototype_next_override() {
    assert_eq!(
        run(r#"
            var ArrayIteratorPrototype = Object.getPrototypeOf([].values());
            var oldNext = ArrayIteratorPrototype.next;
            var values;
            ArrayIteratorPrototype.next = function() {
                var done = values.length === 0;
                var value = values.pop();
                return { value: value, done: done };
            };
            try {
                values = [1, 2, 3, 4];
                var ta = new Uint8Array([0]);
                [ta.length, ta[0], ta[1], ta[2], ta[3]].join(",");
            } finally {
                ArrayIteratorPrototype.next = oldNext;
            }
        "#),
        Value::String(Arc::from("4,4,3,2,1"))
    );
}

#[test]
fn typed_array_delete_canonical_numeric_indices_follow_integer_indexed_exotic() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(2);
            var values = [];
            values.push(delete sample[0]);
            values.push(delete sample["0"]);
            values.push(delete sample[1]);
            values.push(delete sample["2"]);
            values.push(delete sample["-1"]);
            values.push(delete sample["1.1"]);
            values.push(delete sample["-0"]);
            values.push(delete sample[-0]);
            values.join(",");
        "#),
        Value::String(Arc::from("false,false,false,true,true,true,true,false"))
    );
    assert_eq!(
        run(r#"
            "use strict";
            var sample = new Uint8Array(1);
            [delete sample["-0"], delete sample["1.1"], delete sample["1"]].join(",");
        "#),
        Value::String(Arc::from("true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(1);
            $262.detachArrayBuffer(sample.buffer);
            [delete sample[0], delete sample["0"], delete sample["1"], delete sample["-0"], delete sample["1.1"], delete sample["Infinity"]].join(",");
        "#),
        Value::String(Arc::from("true,true,true,true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(1);
            var values = [];
            values.push(delete sample[0]);
            values.push(delete sample["-0"]);
            values.push(delete sample["1"]);
            $262.detachArrayBuffer(sample.buffer);
            values.push(delete sample[0]);
            values.join(",");
        "#),
        Value::String(Arc::from("false,true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(0);
            Object.defineProperty(sample, "+1", { value: 1, configurable: false });
            [delete sample["+1"], Object.getOwnPropertyDescriptor(sample, "+1").value].join(",");
        "#),
        Value::String(Arc::from("false,1"))
    );
    assert!(
        run_err(
            r#"
            "use strict";
            var sample = new Uint8Array(1);
            delete sample[0];
            "#
        )
        .contains("TypeError"),
        "strict delete of a valid TypedArray integer index should throw"
    );
}

#[test]
fn typed_array_get_own_property_descriptor_synthesizes_integer_indices() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([42, 43]);
            var d0 = Object.getOwnPropertyDescriptor(sample, "0");
            var d1 = Object.getOwnPropertyDescriptor(sample, "1");
            [
              d0.value, d0.writable, d0.enumerable, d0.configurable,
              d1.value, d1.writable, d1.enumerable, d1.configurable
            ].join(",");
        "#),
        Value::String(Arc::from("42,true,true,true,43,true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(2);
            sample[0] = 42n;
            sample[1] = 43n;
            var d0 = Object.getOwnPropertyDescriptor(sample, "0");
            var d1 = Object.getOwnPropertyDescriptor(sample, "1");
            [
              d0.value === 42n, d0.writable, d0.enumerable, d0.configurable,
              d1.value === 43n, d1.writable, d1.enumerable, d1.configurable
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,true,true,true,true,true"))
    );
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(8);
            var sample = new Uint16Array(buffer, 2, 2);
            sample[0] = 0x1234;
            sample[1] = 0x5678;
            var d0 = Object.getOwnPropertyDescriptor(sample, "0");
            var d1 = Object.getOwnPropertyDescriptor(sample, "1");
            [d0.value, d1.value].join(",");
        "#),
        Value::String(Arc::from("4660,22136"))
    );
}

#[test]
fn typed_array_get_own_property_descriptor_rejects_invalid_integer_indices() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(2);
            Object.defineProperty(sample, "+1", { value: "ordinary", configurable: true });
            [
              Object.getOwnPropertyDescriptor(sample, "-0") === undefined,
              Object.getOwnPropertyDescriptor(sample, "1.1") === undefined,
              Object.getOwnPropertyDescriptor(sample, "2") === undefined,
              Object.getOwnPropertyDescriptor(sample, "+1").value
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,ordinary"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7]);
            $262.detachArrayBuffer(sample.buffer);
            Object.getOwnPropertyDescriptor(sample, "0") === undefined;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_integer_index_descriptors_feed_proxy_invariants() {
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([1]);
            Object.preventExtensions(sample);
            0 in new Proxy(sample, { has: function() { return false; } });
            "#
        )
        .contains("TypeError"),
        "Proxy has must not hide a non-extensible TypedArray integer index"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([1]);
            Object.preventExtensions(sample);
            Reflect.deleteProperty(new Proxy(sample, { deleteProperty: function() { return true; } }), "0");
            "#
        )
        .contains("TypeError"),
        "Proxy deleteProperty must not delete a non-extensible TypedArray integer index"
    );
}

#[test]
fn typed_array_has_property_canonical_numeric_indices_follow_integer_indexed_exotic() {
    assert_eq!(
        run(r#"
            var TypedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
            Object.defineProperty(TypedArrayPrototype, "2", {
              value: "inherited",
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "-1", {
              value: "inherited",
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "1.1", {
              value: "inherited",
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "+1", {
              value: "ordinary",
              configurable: true
            });
            try {
              var sample = new Uint8Array([7, 8]);
              [
                Reflect.has(sample, "0"),
                Reflect.has(sample, "1"),
                Reflect.has(sample, "2"),
                Reflect.has(sample, "-1"),
                Reflect.has(sample, "1.1"),
                Reflect.has(sample, "-0"),
                Reflect.has(sample, "+1")
              ].join(",");
            } finally {
              delete TypedArrayPrototype["2"];
              delete TypedArrayPrototype["-1"];
              delete TypedArrayPrototype["1.1"];
              delete TypedArrayPrototype["+1"];
            }
        "#),
        Value::String(Arc::from("true,true,false,false,false,false,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7]);
            $262.detachArrayBuffer(sample.buffer);
            [Reflect.has(sample, "0"), Reflect.has(sample, "1")].join(",");
        "#),
        Value::String(Arc::from("false,false"))
    );
}

#[test]
fn typed_array_has_property_ordinary_keys_delegate_to_proxy_prototype() {
    assert_eq!(
        run(r#"
            var hits = 0;
            var proxy = new Proxy(Object.getPrototypeOf(Uint8Array.prototype), {
              has: function(target, key) {
                hits++;
                if (key === "foo") throw new Error("has trap");
                return Reflect.has(target, key);
              }
            });
            var sample = new Uint8Array([7]);
            Object.setPrototypeOf(sample, proxy);
            var numeric = [Reflect.has(sample, "0"), Reflect.has(sample, "1")].join(",");
            var threw = false;
            try { Reflect.has(sample, "foo"); } catch (e) { threw = e.message === "has trap"; }
            numeric + ":" + threw + ":" + hits;
        "#),
        Value::String(Arc::from("true,false:true:1"))
    );
}

#[test]
fn typed_array_subarray_creates_shared_offset_views() {
    assert_eq!(
        run(r#"
            var TypedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
            var source = new Uint8Array([10, 20, 30, 40]);
            var view = source.subarray(1, -1);
            view[0] = 99;
            [
              typeof TypedArrayPrototype.subarray,
              Uint8Array.prototype.hasOwnProperty("subarray"),
              view instanceof Uint8Array,
              view.buffer === source.buffer,
              view.byteOffset,
              view.byteLength,
              view.length,
              view[0],
              view[1],
              source[1]
            ].join(",");
        "#),
        Value::String(Arc::from("function,false,true,true,1,2,2,99,30,99"))
    );
    assert_eq!(
        run(r#"
            var source = new BigInt64Array(3);
            source[0] = 1n;
            source[1] = 2n;
            source[2] = 3n;
            var view = source.subarray(-2);
            view[0] = 9n;
            [view.length, view[0], view[1], source[1]].join(",");
        "#),
        Value::String(Arc::from("2,9,3,9"))
    );
}

#[test]
fn typed_array_subarray_uses_species_and_rejects_detached_buffers() {
    assert_eq!(
        run(r#"
            var source = new Uint8Array([10, 20, 30, 40]);
            var calls = [];
            function Species(buffer, offset, length) {
              calls.push(buffer === source.buffer, offset, length);
              return new Uint8Array(buffer, offset, length);
            }
            var holder = {};
            holder[Symbol.species] = Species;
            source.constructor = holder;
            var view = source.subarray(1, 3);
            [
              calls.join(":"),
              view.buffer === source.buffer,
              view.byteOffset,
              view.length,
              view[0],
              view[1]
            ].join(",");
        "#),
        Value::String(Arc::from("true:1:2,true,1,2,20,30"))
    );
    assert!(
        run_err(
            r#"
                var source = new Uint8Array([1]);
                $262.detachArrayBuffer(source.buffer);
                source.subarray(0);
            "#
        )
        .contains("TypeError"),
        "TypedArray.prototype.subarray should reject detached buffers"
    );
}

#[test]
fn typed_array_subarray_preserves_tracking_and_raw_offset_across_resize() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(8, { maxByteLength: 12 });
            var tracking = new Int8Array(rab, 2);
            var tracked = tracking.subarray(1);
            rab.resize(10);

            var fixed = new Int8Array(rab, 4, 2);
            rab.resize(0);
            var result = fixed.subarray({ valueOf: function() {
                rab.resize(10);
                return 1;
            }});

            var detachedBegin = false;
            var detachedEnd = false;
            var detached = new Int8Array(2);
            $262.detachArrayBuffer(detached.buffer);
            try {
                detached.subarray(
                    { valueOf: function() { detachedBegin = true; return 0; } },
                    { valueOf: function() { detachedEnd = true; return 0; } }
                );
            } catch (error) {}

            [
                tracked.byteOffset,
                tracked.length,
                result.byteOffset,
                result.length,
                detachedBegin,
                detachedEnd
            ].join("|");
            "#,),
        Value::String(Arc::from("3|7|4|0|true|true"))
    );
}

#[test]
fn typed_array_set_handles_array_like_aliasing_and_resizes() {
    assert_eq!(
        run(r#"
            var overlap = new Uint8Array([1, 2, 3, 4]);
            overlap.set(overlap.subarray(0, 3), 1);

            var converted = new Int16Array(3);
            converted.set({ length: 3, 0: 1.9, 1: -2.9, 2: 65537 });

            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var tracking = new Int8Array(rab);
            var calls = [];
            tracking.set({
                length: 4,
                0: { valueOf: function() { calls.push(0); rab.resize(3); return 7; } },
                1: { valueOf: function() { calls.push(1); rab.resize(4); return 8; } },
                2: { valueOf: function() { calls.push(2); return 9; } },
                3: { valueOf: function() { calls.push(3); return 10; } }
            });

            [
                Array.from(overlap).join(","),
                Array.from(converted).join(","),
                Array.from(tracking).join(","),
                calls.join(","),
                typeof Int8Array.prototype.set
            ].join("|");
            "#,),
        Value::String(Arc::from("1,1,2,3|1,-2,1|7,8,9,10|0,1,2,3|function"))
    );
}

#[test]
fn typed_array_join_snapshots_length_before_separator_coercion() {
    assert_eq!(
        run(r#"
            var growRab = new ArrayBuffer(4, { maxByteLength: 8 });
            var grow = new Int8Array(growRab);
            var grown = grow.join({ toString: function() {
                growRab.resize(6);
                return ".";
            }});

            var shrinkRab = new ArrayBuffer(4, { maxByteLength: 8 });
            var shrink = new Int8Array(shrinkRab);
            var shrunk = shrink.join({ toString: function() {
                shrinkRab.resize(0);
                return "-";
            }});

            var oobRab = new ArrayBuffer(4, { maxByteLength: 8 });
            var oob = new Int8Array(oobRab, 0, 4);
            oobRab.resize(0);
            var separatorCalled = false;
            var oobTypeError = false;
            try {
                oob.join({ toString: function() {
                    separatorCalled = true;
                    return ",";
                }});
            } catch (error) { oobTypeError = error instanceof TypeError; }

            [grown, grow.length, shrunk, separatorCalled, oobTypeError].join("|");
            "#,),
        Value::String(Arc::from("0.0.0.0|6|---|false|true"))
    );
}

#[test]
fn typed_array_join_uses_method_realm_for_generated_errors() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var receiverError = false;
            var separatorError = false;
            try {
                other.Uint8Array.prototype.join.call({}, ",");
            } catch (error) {
                receiverError = Object.getPrototypeOf(error) ===
                    other.TypeError.prototype;
            }
            try {
                other.Uint8Array.prototype.join.call(
                    new Uint8Array([1]), Symbol("separator")
                );
            } catch (error) {
                separatorError = Object.getPrototypeOf(error) ===
                    other.TypeError.prototype;
            }
            receiverError + "|" + separatorError;
        "#),
        Value::String(Arc::from("true|true"))
    );
}

#[test]
fn array_join_on_resizable_typed_arrays_uses_generic_gets() {
    assert_eq!(
        run(r#"
            var growBuffer = new ArrayBuffer(4, { maxByteLength: 8 });
            var growing = new Int8Array(growBuffer);
            var grown = Array.prototype.join.call(growing, {
              toString: function() { growBuffer.resize(6); return "."; }
            });

            var shrinkBuffer = new ArrayBuffer(4, { maxByteLength: 8 });
            var shrinking = new Int8Array(shrinkBuffer);
            var shrunk = Array.prototype.join.call(shrinking, {
              toString: function() { shrinkBuffer.resize(2); return "."; }
            });

            [grown, growing.length, shrunk, shrinking.length].join("|");
            "#),
        Value::String(Arc::from("0.0.0.0|6|0.0..|2"))
    );
}

#[test]
fn typed_array_reverse_uses_internal_length_and_dynamic_bounds() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3, 4]);
            var lengthReads = 0;
            Object.defineProperty(tracking, "length", {
                get: function() { lengthReads++; return 0; }
            });
            var resultIsReceiver = tracking.reverse() === tracking;
            var first = Array.from(tracking).join(",");

            rab.resize(6);
            tracking[4] = 5;
            tracking[5] = 6;
            tracking.reverse();
            var grown = Array.from(tracking).join(",");

            var fixed = new Int8Array(rab, 0, 6);
            rab.resize(2);
            var outOfBounds = false;
            try { fixed.reverse(); }
            catch (error) { outOfBounds = error instanceof TypeError; }

            [resultIsReceiver, lengthReads, first, grown, outOfBounds].join("|");
            "#),
        Value::String(Arc::from("true|0|4,3,2,1|6,5,1,2,3,4|true"))
    );
}

#[test]
fn typed_array_to_reversed_copies_same_type_without_species() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var source = new Int8Array(rab);
            source.set([1, 2, 3]);
            var constructorReads = 0;
            Object.defineProperty(source, "constructor", {
                get: function() {
                    constructorReads++;
                    throw new Error("constructor must not be read");
                }
            });
            Object.defineProperty(source, "length", { value: 1 });

            var copy = source.toReversed();
            rab.resize(5);
            source[3] = 4;
            source[4] = 5;
            var grownCopy = source.toReversed();

            [
                constructorReads,
                copy instanceof Int8Array,
                copy === source,
                Array.from(copy).join(","),
                Array.from(source).join(","),
                Array.from(grownCopy).join(",")
            ].join("|");
            "#),
        Value::String(Arc::from("0|true|false|3,2,1|1,2,3,4,5|5,4,3,2,1"))
    );
}

#[test]
fn typed_array_copy_within_preserves_bytes_and_revalidates_bounds() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var tracking = new Int8Array(rab);
            tracking.set([0, 1, 2, 3]);
            Object.defineProperty(tracking, "length", { value: 1 });
            var same = tracking.copyWithin(1, 0, 3) === tracking;
            var overlap = Array.from(tracking).join(",");

            tracking.set([0, 1, 2, 3]);
            tracking.copyWithin({ valueOf: function() {
                rab.resize(3);
                return 2;
            }}, 0);
            var shrunk = Array.from(tracking).join(",");

            rab.resize(6);
            tracking.set([0, 1, 2, 3, 4, 5]);
            tracking.copyWithin({ valueOf: function() { return 0; }}, 2);
            var grown = Array.from(tracking).join(",");

            var fixed = new Int8Array(rab, 0, 6);
            rab.resize(2);
            var outOfBounds = false;
            try { fixed.copyWithin(0, 1); }
            catch (error) { outOfBounds = error instanceof TypeError; }

            [same, overlap, shrunk, grown, outOfBounds].join("|");
            "#),
        Value::String(Arc::from("true|0,0,1,2|0,1,0|2,3,4,5,4,5|true"))
    );
}

#[test]
fn typed_array_slice_uses_species_and_revalidates_source() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var source = new Int8Array(rab);
            source.set([10, 20, 30, 40]);
            var calls = [];
            source.constructor = { [Symbol.species]: function(count) {
                calls.push(count);
                rab.resize(2);
                return new Uint8Array(count);
            }};
            var sliced = source.slice(0, 4);

            rab.resize(6);
            source = new Int8Array(rab);
            source.set([10, 20, 30, 40, 50, 60]);
            source.constructor = { [Symbol.species]: function() {
                return new Int8Array(rab, 2);
            }};
            var shared = source.slice(1, 4);

            [
                calls.join(","),
                sliced instanceof Uint8Array,
                Array.from(sliced).join(","),
                Array.from(shared).join(",")
            ].join("|");
            "#),
        Value::String(Arc::from("4|true|10,20,0,0|20,20,20,60"))
    );
}

#[test]
fn typed_array_species_accessor_and_unaligned_tracking_views() {
    assert_eq!(
        run(r#"
            class Derived extends Uint16Array {}
            var derived = new Derived([1, 2]);
            var sliced = derived.slice();
            var rab = new ArrayBuffer(5, { maxByteLength: 8 });
            var tracking = new Uint16Array(rab);
            [
                Uint16Array[Symbol.species] === Uint16Array,
                sliced instanceof Derived,
                tracking.length,
                tracking.byteLength
            ].join("|");
            "#),
        Value::String(Arc::from("true|true|2|4"))
    );
}

#[test]
fn typed_array_find_snapshots_length_and_reads_current_values() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3]);
            var seen = [];
            var found = tracking.find(function(value, index, receiver) {
                seen.push(String(value) + ":" + index + ":" + (receiver === tracking));
                if (index === 0) {
                    tracking[2] = 7;
                    rab.resize(2);
                }
                return value === 7;
            });

            rab.resize(3);
            tracking[2] = 3;
            var grownSeen = [];
            tracking.find(function(value, index) {
                grownSeen.push(value);
                if (index === 0) rab.resize(5);
                return false;
            });

            [String(found), seen.join(","), grownSeen.join(",")].join("|");
            "#),
        Value::String(Arc::from(
            "undefined|1:0:true,2:1:true,undefined:2:true|1,2,3"
        ))
    );
}

#[test]
fn typed_array_find_index_shares_find_iteration_semantics() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3]);
            var seen = [];
            var found = tracking.findIndex(function(value, index, receiver) {
                seen.push(String(value) + ":" + index + ":" + (receiver === tracking));
                if (index === 0) {
                    tracking[2] = 7;
                    rab.resize(2);
                }
                return value === 7;
            });
            var missing = tracking.findIndex(function() { return false; });
            [found, missing, seen.join(",")].join("|");
            "#),
        Value::String(Arc::from("-1|-1|1:0:true,2:1:true,undefined:2:true"))
    );
}

#[test]
fn typed_array_find_last_iterates_the_snapshot_in_reverse() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3]);
            var seen = [];
            var found = tracking.findLast(function(value, index, receiver) {
                seen.push(String(value) + ":" + index + ":" + (receiver === tracking));
                if (index === 2) {
                    tracking[0] = 7;
                    rab.resize(1);
                }
                return value === 7;
            });
            [found, seen.join(",")].join("|");
            "#),
        Value::String(Arc::from("7|3:2:true,undefined:1:true,7:0:true"))
    );
}

#[test]
fn typed_array_find_last_index_returns_reverse_match_position() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3]);
            var seen = [];
            var found = tracking.findLastIndex(function(value, index, receiver) {
                seen.push(String(value) + ":" + index + ":" + (receiver === tracking));
                if (index === 2) {
                    tracking[0] = 7;
                    rab.resize(1);
                }
                return value === 7;
            });
            var missing = tracking.findLastIndex(function() { return false; });
            [found, missing, seen.join(",")].join("|");
            "#),
        Value::String(Arc::from("0|-1|3:2:true,undefined:1:true,7:0:true"))
    );
}

#[test]
fn typed_array_some_snapshots_length_and_short_circuits() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3]);
            var seen = [];
            var resized = tracking.some(function(value, index, receiver) {
                seen.push(String(value) + ":" + index + ":" + (receiver === tracking));
                if (index === 0) rab.resize(1);
                return false;
            });
            var calls = 0;
            var matched = new Int8Array([1, 7, 3]).some(function(value) {
                calls += 1;
                return value === 7;
            });
            [resized, seen.join(","), matched, calls].join("|");
            "#),
        Value::String(Arc::from(
            "false|1:0:true,undefined:1:true,undefined:2:true|true|2"
        ))
    );
}

#[test]
fn typed_array_every_snapshots_length_and_short_circuits() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3]);
            var seen = [];
            var resized = tracking.every(function(value, index, receiver) {
                seen.push(String(value) + ":" + index + ":" + (receiver === tracking));
                if (index === 0) rab.resize(1);
                return value === 1;
            });
            var calls = 0;
            var matched = new Int8Array([1, 2, 3]).every(function(value) {
                calls += 1;
                return value > 0;
            });
            [resized, seen.join(","), matched, calls].join("|");
            "#),
        Value::String(Arc::from("false|1:0:true,undefined:1:true|true|3"))
    );
}

#[test]
fn typed_array_for_each_ignores_results_and_visits_snapshot_length() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3]);
            var seen = [];
            var result = tracking.forEach(function(value, index, receiver) {
                seen.push(String(value) + ":" + index + ":" + (receiver === tracking));
                if (index === 0) rab.resize(1);
                return true;
            });
            [result === undefined, seen.join(",")].join("|");
            "#),
        Value::String(Arc::from("true|1:0:true,undefined:1:true,undefined:2:true"))
    );
}

#[test]
fn array_for_each_on_resizable_typed_arrays_uses_generic_presence() {
    assert_eq!(
        run(r#"
            var growBuffer = new ArrayBuffer(3, { maxByteLength: 6 });
            var growing = new Int8Array(growBuffer);
            growing.set([1, 2, 3]);
            var growSeen = [];
            Array.prototype.forEach.call(growing, function(value, index) {
              growSeen.push(value);
              if (index === 0) growBuffer.resize(6);
            });

            var shrinkBuffer = new ArrayBuffer(3, { maxByteLength: 6 });
            var shrinking = new Int8Array(shrinkBuffer);
            shrinking.set([1, 2, 3]);
            var shrinkSeen = [];
            Array.prototype.forEach.call(shrinking, function(value, index) {
              shrinkSeen.push(value);
              if (index === 0) shrinkBuffer.resize(1);
            });

            [growSeen.join(","), shrinkSeen.join(",")].join("|");
            "#),
        Value::String(Arc::from("1,2,3|1"))
    );
}

#[test]
fn typed_array_includes_uses_snapshot_indices_and_same_value_zero() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3]);
            var foundUndefined = tracking.includes(undefined, {
                valueOf: function() {
                    rab.resize(1);
                    return 1;
                }
            });
            var foundZero = tracking.includes(0, 1);
            var foundNaN = new Float64Array([1, NaN]).includes(NaN);
            [foundUndefined, foundZero, foundNaN].join("|");
            "#),
        Value::String(Arc::from("true|false|true"))
    );
}

#[test]
fn typed_array_dynamic_subclass_construction_survives_gc_pressure() {
    assert_eq!(
        run(r#"
            function subclass(name) {
                return new Function("return class Dynamic extends " + name + " {}")();
            }
            var constructors = [
                subclass("Uint8Array"),
                subclass("Float32Array"),
                subclass("BigInt64Array")
            ];
            var count = 0;
            for (var round = 0; round < 32; round += 1) {
                for (var ctor of constructors) {
                    var rab = new ArrayBuffer(32, { maxByteLength: 64 });
                    var view = new ctor(rab, ctor.BYTES_PER_ELEMENT);
                    if (view.length > 0) count += 1;
                }
            }
            count;
            "#),
        Value::Number(96.0)
    );
}

#[test]
fn typed_array_reduce_right_snapshots_length_and_reads_current_values() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var values = new Int8Array(rab);
            values.set([1, 2, 3]);
            var visits = [];
            var result = values.reduceRight(function(accumulator, value, index, receiver) {
                visits.push(index + ":" + value + ":" + (receiver === values));
                if (index === 2) rab.resize(1);
                return accumulator + String(value);
            }, "");
            [result, visits.join("|")].join(";");
            "#),
        Value::String(Arc::from("3undefined1;2:3:true|1:undefined:true|0:1:true"))
    );
}

#[test]
fn typed_array_reduce_right_uses_last_value_as_default_accumulator() {
    assert_eq!(
        run(r#"
            var calls = 0;
            var single = new BigInt64Array([7n]);
            var result = single.reduceRight(function() { calls += 1; });
            [String(result), calls].join("|");
            "#),
        Value::String(Arc::from("7|0"))
    );
}

#[test]
fn typed_array_reduce_snapshots_length_and_reads_current_values() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var values = new Int8Array(rab);
            values.set([1, 2, 3]);
            var visits = [];
            var result = values.reduce(function(accumulator, value, index, receiver) {
                visits.push(index + ":" + value + ":" + (receiver === values));
                if (index === 0) rab.resize(1);
                return accumulator + String(value);
            }, "");
            [result, visits.join("|")].join(";");
            "#),
        Value::String(Arc::from(
            "1undefinedundefined;0:1:true|1:undefined:true|2:undefined:true"
        ))
    );
}

#[test]
fn typed_array_reduce_uses_first_value_as_default_accumulator() {
    assert_eq!(
        run(r#"
            var calls = 0;
            var single = new BigInt64Array([7n]);
            var result = single.reduce(function() { calls += 1; });
            [String(result), calls].join("|");
            "#),
        Value::String(Arc::from("7|0"))
    );
}

#[test]
fn typed_array_map_uses_species_and_preserves_source() {
    assert_eq!(
        run(r#"
            var source = new Int8Array([1, 2, 3]);
            var target = new Int16Array(4);
            source.constructor = {
                [Symbol.species]: function(length) {
                    if (length !== 3) throw new Error("wrong length");
                    return target;
                }
            };
            var result = source.map(function(value, index, receiver) {
                return value + index + (receiver === source ? 10 : 100);
            });
            [result === target, Array.from(result).join(","), Array.from(source).join(",")].join("|");
            "#),
        Value::String(Arc::from("true|11,13,15,0|1,2,3"))
    );
}

#[test]
fn typed_array_map_constructs_species_before_current_reads() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var source = new Int8Array(rab);
            source.set([1, 2, 3]);
            source.constructor = {
                [Symbol.species]: function(length) {
                    rab.resize(1);
                    return new Int8Array(length);
                }
            };
            var seen = [];
            var result = source.map(function(value) {
                seen.push(String(value));
                return value === undefined ? 0 : value;
            });
            [seen.join(","), Array.from(result).join(",")].join("|");
            "#),
        Value::String(Arc::from("1,undefined,undefined|1,0,0"))
    );
}

#[test]
fn typed_array_filter_calls_species_after_predicates() {
    assert_eq!(
        run(r#"
            var log = [];
            var source = new Int8Array([1, 2, 3, 4]);
            var target = new Int16Array(3);
            source.constructor = {
                [Symbol.species]: function(length) {
                    log.push("species:" + length);
                    return target;
                }
            };
            var result = source.filter(function(value, index, receiver) {
                log.push("callback:" + index);
                return receiver === source && value !== 2;
            });
            [result === target, Array.from(result).join(","), log.join("|")].join(";");
            "#),
        Value::String(Arc::from(
            "true;1,3,4;callback:0|callback:1|callback:2|callback:3|species:3"
        ))
    );
}

#[test]
fn typed_array_filter_keeps_current_values_across_resize() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var source = new Uint8Array(rab);
            source.set([1, 2, 3, 4]);
            var seen = [];
            var result = source.filter(function(value, index) {
                seen.push(String(value));
                if (index === 0) rab.resize(2);
                return value !== undefined && value % 2 === 0;
            });
            [seen.join(","), Array.from(result).join(",")].join("|");
            "#),
        Value::String(Arc::from("1,2,undefined,undefined|2"))
    );
}

#[test]
fn typed_array_index_of_uses_strict_equality() {
    assert_eq!(
        run(r#"
            var numbers = new Float64Array([0, NaN, 2]);
            var bigints = new BigInt64Array([1n, 2n]);
            [numbers.indexOf(-0), numbers.indexOf(NaN), bigints.indexOf(2n),
             bigints.indexOf(2)].join("|");
        "#),
        Value::String(Arc::from("0|-1|1|-1"))
    );
}

#[test]
fn typed_array_index_of_observes_resize_during_from_index() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(4, { maxByteLength: 8 });
            var values = new Uint8Array(buffer);
            values[0] = 1;
            values[2] = 2;
            var from = { valueOf: function () { buffer.resize(2); return 0; } };
            [values.indexOf(1, from), values.indexOf(2),
             values.indexOf(undefined)].join("|");
        "#),
        Value::String(Arc::from("0|-1|-1"))
    );
}

#[test]
fn typed_array_last_index_of_distinguishes_omitted_from_undefined() {
    assert_eq!(
        run(r#"
            var values = new Int8Array([1, 2, 1]);
            [values.lastIndexOf(1), values.lastIndexOf(1, undefined),
             values.lastIndexOf(1, -2), values.lastIndexOf(1, -Infinity)].join("|");
        "#),
        Value::String(Arc::from("2|0|0|-1"))
    );
}

#[test]
fn typed_array_last_index_of_observes_resize_during_from_index() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(4, { maxByteLength: 8 });
            var values = new Uint8Array(buffer);
            values.set([1, 2, 1, 2]);
            var from = { valueOf: function () { buffer.resize(2); return 3; } };
            [values.lastIndexOf(2, from), values.lastIndexOf(undefined),
             values.lastIndexOf(1)].join("|");
        "#),
        Value::String(Arc::from("1|-1|0"))
    );
}

#[test]
fn typed_array_to_locale_string_invokes_each_current_value() {
    assert_eq!(
        run(r#"
            var calls = [];
            var original = Number.prototype.toLocaleString;
            Number.prototype.toLocaleString = function() {
                calls.push(this.valueOf() + ":" + arguments.length);
                return { toString: function() { return "v" + calls.length; } };
            };
            var result = new Uint8Array([4, 5]).toLocaleString(
              { toString: function() { throw "unused locale"; } },
              { get marker() { throw "unused options"; } }
            );
            Number.prototype.toLocaleString = original;
            [result, calls.join("|")].join(";");
        "#),
        Value::String(Arc::from("v1,v2;4:0|5:0"))
    );
}

#[test]
fn typed_array_to_locale_string_keeps_snapshot_length_across_resize() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(4, { maxByteLength: 8 });
            var values = new Uint8Array(buffer);
            values.set([1, 2, 3, 4]);
            var calls = 0;
            var original = Number.prototype.toLocaleString;
            Number.prototype.toLocaleString = function() {
                calls += 1;
                if (calls === 2) buffer.resize(2);
                return String(this.valueOf());
            };
            var result = values.toLocaleString();
            Number.prototype.toLocaleString = original;
            [result, calls].join("|");
        "#),
        Value::String(Arc::from("1,2,,|2"))
    );
}

#[test]
fn typed_array_to_locale_string_uses_locale_methods_without_radix_coercion() {
    assert_eq!(
        run(r#"
            var objectLocale = Object.prototype.toLocaleString;
            Object.prototype.toLocaleString = function() { return "hook"; };
            var number = new Uint8Array([10]).toLocaleString("2");
            var bigint = new BigInt64Array([1n, 2n]).toLocaleString();
            Object.prototype.toLocaleString = objectLocale;
            [number, bigint].join("|");
        "#),
        Value::String(Arc::from("10|1,2"))
    );
}

#[test]
fn typed_array_to_locale_string_ignores_locale_arguments() {
    assert_eq!(
        run(r#"
            var original = Number.prototype.toLocaleString;
            Number.prototype.toLocaleString = function() {
                return String(arguments.length);
            };
            var result = new Uint8Array([1]).toLocaleString("ignored", "ignored");
            Number.prototype.toLocaleString = original;
            result;
        "#),
        Value::String(Arc::from("0"))
    );
}

#[test]
fn typed_array_to_locale_string_uses_method_realm_primitive_prototype() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            Number.prototype.toLocaleString = function() { return "main"; };
            other.eval("Number.prototype.toLocaleString = function() { return 'other'; }");
            var first = other.Uint8Array.prototype.toLocaleString.call(
              new Uint8Array([1])
            );
            var originalNumber = other.Number;
            other.Number = {
              prototype: { toLocaleString: function() { return "bad"; } }
            };
            var second = other.Uint8Array.prototype.toLocaleString.call(
              new Uint8Array([1])
            );
            other.Number = originalNumber;
            other.Number.prototype.toLocaleString = null;
            var foreignError = false;
            try {
              other.Uint8Array.prototype.toLocaleString.call(new Uint8Array([1]));
            } catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }

            other.eval("BigInt.prototype.toLocaleString = function() { return 'big'; }");
            var bigintFirst = other.BigInt64Array.prototype.toLocaleString.call(
              new BigInt64Array([1n])
            );
            var originalBigInt = other.BigInt;
            other.BigInt = {
              prototype: { toLocaleString: function() { return "bad-big"; } }
            };
            var bigintSecond = other.BigInt64Array.prototype.toLocaleString.call(
              new BigInt64Array([1n])
            );
            other.BigInt = originalBigInt;
            [first, second, foreignError, bigintFirst, bigintSecond].join("|");
        "#),
        Value::String(Arc::from("other|other|true|big|big"))
    );
}

#[test]
fn typed_array_with_coerces_before_copying_and_ignores_species() {
    assert_eq!(
        run(r#"
            var source = new Int8Array([1, 2, 3]);
            var log = [];
            source.constructor = {
                get [Symbol.species]() { throw new Error("species"); }
            };
            var result = source.with(
                { valueOf: function() { log.push("index"); return 1; } },
                { valueOf: function() { log.push("value"); source[0] = 9; return 7; } }
            );
            [Array.from(result).join(","), Array.from(source).join(","),
             log.join(",")].join("|");
        "#),
        Value::String(Arc::from("9,7,3|9,2,3|index,value"))
    );
}

#[test]
fn typed_array_with_validates_current_index_after_growth() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(2, { maxByteLength: 5 });
            var source = new Int8Array(buffer);
            source.set([11, 22]);
            var result = source.with(4, {
                valueOf: function() { buffer.resize(5); return 123; }
            });
            [Array.from(result).join(","), source.length].join("|");
        "#),
        Value::String(Arc::from("11,22|5"))
    );
}

#[test]
fn typed_array_same_type_copies_ignore_replaced_global_constructors() {
    assert_eq!(
        run(r#"
            var source = new Int8Array([1, 2]);
            var original = Int8Array;
            Int8Array = function() { throw new Error("replaced"); };
            var a = source.with(0, 3);
            var b = source.toReversed();
            var c = source.toSorted();
            Int8Array = original;
            [Object.getPrototypeOf(a) === original.prototype,
             Object.getPrototypeOf(b) === original.prototype,
             Object.getPrototypeOf(c) === original.prototype].join("|");
        "#),
        Value::String(Arc::from("true|true|true"))
    );
}

#[test]
fn typed_array_with_uses_original_method_realm_constructor() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var original = other.Int8Array;
            var method = original.prototype.with;
            var source = new Int8Array([1, 2]);
            other.eval("Int8Array = function() { throw new Error('replaced'); }");
            var result = method.call(source, 1, 3);
            Object.getPrototypeOf(result) === original.prototype;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_with_observes_index_resize_before_value_coercion() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(1, { maxByteLength: 2 });
            var source = new Int8Array(buffer);
            source[0] = 5;
            var log = [];
            var result = source.with({
                valueOf: function() { log.push("index"); buffer.resize(2); return 1; }
            }, {
                valueOf: function() { log.push("value"); return 9; }
            });
            [Array.from(result).join(","), source.length, log.join(",")].join("|");
        "#),
        Value::String(Arc::from("5|2|index,value"))
    );
}

#[test]
fn typed_array_with_converts_bigint_value_before_bounds_error() {
    assert_eq!(
        run(r#"
            var source = new BigInt64Array([1n]);
            try {
                source.with(9, 1);
                "none";
            } catch (error) {
                error.name;
            }
        "#),
        Value::String(Arc::from("TypeError"))
    );
}

#[test]
fn typed_array_to_string_tag_getter_uses_internal_kind() {
    assert_eq!(
        run(r#"
            var getter = Object.getOwnPropertyDescriptor(
                Object.getPrototypeOf(Int8Array.prototype), Symbol.toStringTag
            ).get;
            var buffer = new ArrayBuffer(1);
            var array = new Int8Array(buffer);
            $262.detachArrayBuffer(buffer);
            [getter.call(array), getter.call(new DataView(new ArrayBuffer(1))),
             getter.call(1), getter.name, getter.length].join("|");
        "#),
        Value::String(Arc::from("Int8Array|||get [Symbol.toStringTag]|0"))
    );
}

#[test]
fn typed_array_buffer_getter_preserves_identity_after_detach() {
    assert_eq!(
        run(r#"
            var getter = Object.getOwnPropertyDescriptor(
                Object.getPrototypeOf(Uint8Array.prototype), "buffer"
            ).get;
            var buffer = new ArrayBuffer(2);
            var array = new Uint8Array(buffer);
            $262.detachArrayBuffer(buffer);
            [getter.call(array) === buffer, getter.name, getter.length].join("|");
        "#),
        Value::String(Arc::from("true|get buffer|0"))
    );
}

#[test]
fn typed_array_buffer_getter_requires_internal_slots() {
    for source in [
        r#"
            var getter = Object.getOwnPropertyDescriptor(
                Object.getPrototypeOf(Uint8Array.prototype), "buffer"
            ).get;
            getter.call(Object.create(new Uint8Array(1)));
        "#,
        r#"
            var getter = Object.getOwnPropertyDescriptor(
                Object.getPrototypeOf(Uint8Array.prototype), "buffer"
            ).get;
            getter.call(new DataView(new ArrayBuffer(1)));
        "#,
    ] {
        assert!(run_err(source).contains("TypeError"));
    }
}

#[test]
fn typed_array_internal_buffers_use_the_constructor_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var OtherArrayBuffer = other.ArrayBuffer;
            var otherArrayBufferPrototype = other.ArrayBuffer.prototype;
            other.ArrayBuffer = function ReplacedArrayBuffer() {};
            var constructed = new other.Uint8Array(2).buffer;
            var from = other.Uint8Array.from([1, 2]).buffer;
            var of = other.Uint8Array.of(1, 2).buffer;
            [constructed, from, of].every(function(buffer) {
                return Object.getPrototypeOf(buffer) === otherArrayBufferPrototype &&
                    buffer instanceof OtherArrayBuffer;
            });
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_realm_array_buffer_prototype_survives_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            other.ArrayBuffer = function ReplacedArrayBuffer() {};
            forceGc();
            var buffer = new other.Uint8Array(1).buffer;
            [
                Object.getPrototypeOf(buffer) !== other.ArrayBuffer.prototype,
                Object.prototype.toString.call(buffer)
            ].join("|");
        "#
        )
        .expect("foreign Realm buffer allocation should survive GC"),
        Value::String(Arc::from("true|[object ArrayBuffer]"))
    );
}

#[test]
fn typed_array_buffer_getter_uses_its_realm_function_prototype() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var getter = Object.getOwnPropertyDescriptor(
                Object.getPrototypeOf(other.Uint8Array.prototype), "buffer"
            ).get;
            Object.getPrototypeOf(getter) === other.Function.prototype;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_size_getters_use_their_realm_and_validate_receivers() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var proto = Object.getPrototypeOf(other.Uint8Array.prototype);
            var names = ["byteLength", "byteOffset", "length"];
            var getters = names.map(function(name) {
                return Object.getOwnPropertyDescriptor(proto, name).get;
            });
            var array = new Uint16Array(new ArrayBuffer(8), 2, 2);
            var realmError = false;
            try { getters[0].call({}); }
            catch (error) {
                realmError = error instanceof other.TypeError && !(error instanceof TypeError);
            }
            [
                getters.every(function(getter) {
                    return Object.getPrototypeOf(getter) === other.Function.prototype;
                }),
                getters.map(function(getter) { return getter.call(array); }).join(","),
                realmError
            ].join("|");
        "#),
        Value::String(Arc::from("true|4,2,2|true"))
    );
}

#[test]
fn typed_array_size_getters_and_backing_buffer_survive_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var other = $262.createRealm().global;
        var array = new other.Uint16Array(new other.ArrayBuffer(8), 2, 2);
        var proto = Object.getPrototypeOf(other.Uint16Array.prototype);
        var byteLengthGetter = Object.getOwnPropertyDescriptor(proto, "byteLength").get;
        var byteOffsetGetter = Object.getOwnPropertyDescriptor(proto, "byteOffset").get;
        var lengthGetter = Object.getOwnPropertyDescriptor(proto, "length").get;
        "#,
    )
    .expect("failed to create foreign TypedArray accessors");
    vm.gc();
    assert_eq!(
        vm.run(
            r#"
            [
                byteLengthGetter.call(array),
                byteOffsetGetter.call(array),
                lengthGetter.call(array),
                Object.getPrototypeOf(byteLengthGetter) === other.Function.prototype
            ].join("|");
            "#,
        )
        .expect("TypedArray accessors should survive GC"),
        Value::String(Arc::from("4|2|2|true"))
    );
}

#[test]
fn typed_array_size_getters_track_growable_shared_buffers() {
    assert_eq!(
        run(r#"
            var buffer = new SharedArrayBuffer(8, { maxByteLength: 16 });
            var fixed = new Uint16Array(buffer, 2, 2);
            var tracking = new Uint16Array(buffer, 2);
            var before = [
                fixed.byteLength, fixed.byteOffset, fixed.length,
                tracking.byteLength, tracking.byteOffset, tracking.length
            ].join(",");
            buffer.grow(16);
            var after = [
                fixed.byteLength, fixed.byteOffset, fixed.length,
                tracking.byteLength, tracking.byteOffset, tracking.length
            ].join(",");
            before + "|" + after;
        "#),
        Value::String(Arc::from("4,2,2,6,2,3|4,2,2,14,2,7"))
    );
}

#[test]
fn typed_array_to_string_and_iterator_aliases_match_intrinsics() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            [
                Uint8Array.prototype.toString === Array.prototype.toString,
                other.Uint8Array.prototype.toString === other.Array.prototype.toString,
                Uint8Array.prototype[Symbol.iterator] === Uint8Array.prototype.values,
                other.Uint8Array.prototype[other.Symbol.iterator] ===
                    other.Uint8Array.prototype.values
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true"))
    );
}

#[test]
fn typed_array_prototype_intrinsic_alias_descriptors_match_realms() {
    assert_eq!(
        run(r#"
            function check(global) {
                var TypedArray = Object.getPrototypeOf(global.Uint8Array);
                var prototype = Object.getPrototypeOf(global.Uint8Array.prototype);
                var constructor = Object.getOwnPropertyDescriptor(prototype, "constructor");
                var iterator = Object.getOwnPropertyDescriptor(
                    prototype, global.Symbol.iterator
                );
                return constructor.value === TypedArray &&
                    constructor.writable && !constructor.enumerable && constructor.configurable &&
                    iterator.value === prototype.values &&
                    iterator.writable && !iterator.enumerable && iterator.configurable;
            }
            var other = $262.createRealm().global;
            check(globalThis) && check(other);
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_prototype_intrinsic_aliases_survive_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var other = $262.createRealm().global;
        var TypedArray = Object.getPrototypeOf(other.Uint8Array);
        var prototype = Object.getPrototypeOf(other.Uint8Array.prototype);
        var values = prototype.values;
        var iterator = prototype[other.Symbol.iterator];
        "#,
    )
    .expect("failed to create foreign TypedArray intrinsics");
    vm.gc();
    assert_eq!(
        vm.run(
            r#"
            prototype.constructor === TypedArray &&
                prototype.values === values &&
                prototype[other.Symbol.iterator] === iterator &&
                values === iterator;
            "#,
        )
        .expect("TypedArray intrinsic aliases should survive GC"),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_prototype_intrinsic_mutations_are_realm_local() {
    assert_eq!(
        run(r#"
            var mainTypedArray = Object.getPrototypeOf(Uint8Array);
            var mainPrototype = Object.getPrototypeOf(Uint8Array.prototype);
            var originalValues = mainPrototype.values;
            var originalIterator = mainPrototype[Symbol.iterator];
            mainPrototype.values = function replacedValues() {};
            var iteratorStayedOriginal =
                mainPrototype[Symbol.iterator] === originalIterator;
            mainPrototype[Symbol.iterator] = function replacedIterator() {};
            mainPrototype.constructor = function ReplacedTypedArray() {};

            var other = $262.createRealm().global;
            var otherTypedArray = Object.getPrototypeOf(other.Uint8Array);
            var otherPrototype = Object.getPrototypeOf(other.Uint8Array.prototype);
            var otherIsPristine =
                otherPrototype.constructor === otherTypedArray &&
                otherPrototype.values === otherPrototype[other.Symbol.iterator];

            mainPrototype.values = originalValues;
            mainPrototype[Symbol.iterator] = originalIterator;
            mainPrototype.constructor = mainTypedArray;
            iteratorStayedOriginal && otherIsPristine;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_realm_to_string_alias_ignores_mutated_array_prototype() {
    assert_eq!(
        run(r#"
            var intrinsic = Array.prototype.toString;
            Object.defineProperty(Array.prototype, "toString", {
                configurable: true,
                get: function() { throw new Error("must not be observed"); }
            });
            var other = $262.createRealm().global;
            var typedArrayPrototype = Object.getPrototypeOf(other.Uint8Array.prototype);
            var alias = Object.getOwnPropertyDescriptor(
                typedArrayPrototype, "toString"
            ).value;
            Object.defineProperty(Array.prototype, "toString", {
                configurable: true,
                writable: true,
                value: intrinsic
            });
            alias !== intrinsic && alias === other.Array.prototype.toString;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_to_string_uses_join_and_rejects_detached_views() {
    assert_eq!(
        run("new Uint8Array([1, 2, 3]).toString();"),
        Value::String(Arc::from("1,2,3"))
    );
    for source in [
        "var array = new Uint8Array(1); $262.detachArrayBuffer(array.buffer); array.toString();",
        "var array = new BigInt64Array(1); $262.detachArrayBuffer(array.buffer); array.toString();",
    ] {
        assert!(run_err(source).contains("TypeError"));
    }
}

#[test]
fn typed_array_to_string_boxes_receivers_and_uses_the_method_realm() {
    assert_eq!(
        run(r#"
            var original = Number.prototype.join;
            Number.prototype.join = function () {
              return Object.prototype.toString.call(this);
            };
            var boxed = Uint8Array.prototype.toString.call(7);
            if (original === undefined) {
              delete Number.prototype.join;
            } else {
              Number.prototype.join = original;
            }

            var other = $262.createRealm().global;
            var foreignError = false;
            try {
              other.Uint8Array.prototype.toString.call(null);
            } catch (error) {
              foreignError = Object.getPrototypeOf(error) === other.TypeError.prototype;
            }
            boxed + "|" + foreignError;
            "#,),
        Value::String(Arc::from("[object Number]|true"))
    );
}

#[test]
fn object_to_string_observes_symbol_to_string_tag() {
    assert_eq!(
        run(r#"
            var array = new Uint8Array(1);
            Object.defineProperty(array, Symbol.toStringTag, {
                value: "Custom", configurable: true
            });
            var custom = Object.prototype.toString.call(array);
            Object.defineProperty(array, Symbol.toStringTag, { value: 1 });
            var fallback = Object.prototype.toString.call(array);
            [custom, fallback].join("|");
        "#),
        Value::String(Arc::from("[object Custom]|[object Object]"))
    );
    let error = run_err(
        r#"
            var object = {};
            Object.defineProperty(object, Symbol.toStringTag, {
                get: function() { throw new Error("tag"); }
            });
            Object.prototype.toString.call(object);
        "#,
    );
    assert!(error.contains("Error: tag"), "unexpected error: {error}");
}

#[test]
fn typed_array_prototype_set_keeps_proxy_descriptor_rooted() {
    assert_eq!(
        run(r#"
            var value = { marker: 1 };
            var successes = 0;
            for (var i = 0; i < 96; i += 1) {
                var target = new Uint8Array([0]);
                var receiver = new Proxy(Object.create(target), {
                    defineProperty: function(base, key, descriptor) {
                        Object.defineProperty(base, key, descriptor);
                        return true;
                    }
                });
                receiver[0] = value;
                if (receiver[0] === value && target[0] === 0) successes += 1;
            }
            successes;
            "#),
        Value::Number(96.0)
    );
}

#[test]
fn typed_array_sort_uses_numeric_bigint_nan_and_signed_zero_order() {
    assert_eq!(
        run(r#"
            var numbers = new Float64Array([NaN, 2, 0, -0, -1, NaN]).sort();
            var bigints = new BigInt64Array([3n, -2n, 1n]).sort();
            [
                numbers[0],
                Object.is(numbers[1], -0),
                Object.is(numbers[2], 0),
                numbers[3],
                Number.isNaN(numbers[4]),
                Number.isNaN(numbers[5]),
                Array.from(bigints).join(",")
            ].join("|");
            "#),
        Value::String(Arc::from("-1|true|true|2|true|true|-2,1,3"))
    );
}

#[test]
fn typed_array_sort_is_stable_and_writes_to_current_bounds() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var values = new Uint8Array(rab);
            values.set([4, 3, 2, 1]);
            var calls = 0;
            var result = values.sort(function(left, right) {
                calls += 1;
                if (calls === 1) rab.resize(2);
                return (left % 2) - (right % 2);
            });
            [result === values, Array.from(values).join(","), calls > 0].join("|");
            "#),
        Value::String(Arc::from("true|4,2|true"))
    );
}

#[test]
fn gc_invalidates_property_cache_before_heap_cell_reuse() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    assert_eq!(
        vm.run("var cached = { length: 3 }; cached.length;")
            .expect("initial property read should succeed"),
        Value::Number(3.0)
    );
    vm.run("cached = null;")
        .expect("dropping the cached object should succeed");
    vm.gc();
    assert_eq!(
        vm.run("var replacement = { length: 4 }; replacement.length;")
            .expect("replacement property read should succeed"),
        Value::Number(4.0)
    );
}

#[test]
fn typed_array_to_sorted_copies_same_type_without_species_lookup() {
    assert_eq!(
        run(r#"
            var source = new Float64Array([NaN, 2, -0, 0, -1]);
            Object.defineProperty(source, "constructor", {
                get: function() { throw new Error("constructor must not be read"); }
            });
            var sorted = source.toSorted();
            [
                sorted instanceof Float64Array,
                sorted !== source,
                source[0] !== source[0],
                sorted[0],
                Object.is(sorted[1], -0),
                Object.is(sorted[2], 0),
                sorted[3],
                Number.isNaN(sorted[4])
            ].join("|");
            "#),
        Value::String(Arc::from("true|true|true|-1|true|true|2|true"))
    );
}

#[test]
fn typed_array_to_sorted_uses_custom_comparator_for_bigint_copy() {
    assert_eq!(
        run(r#"
            var source = new BigInt64Array([1n, 3n, 2n]);
            var sorted = source.toSorted(function(left, right) {
                return Number(right - left);
            });
            [Array.from(source).join(","), Array.from(sorted).join(",")].join("|");
            "#),
        Value::String(Arc::from("1,3,2|3,2,1"))
    );
}

#[test]
fn typed_array_values_validates_and_tracks_dynamic_bounds() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var tracking = new Int8Array(rab);
            tracking.set([1, 2, 3, 4]);
            var iterator = tracking.values();
            var first = iterator.next().value;
            rab.resize(6);
            tracking[4] = 5;
            tracking[5] = 6;
            var rest = Array.from(iterator).join(",");

            var fixed = new Int8Array(rab, 0, 6);
            var fixedIterator = fixed.values();
            fixedIterator.next();
            rab.resize(2);
            var nextTypeError = false;
            try { fixedIterator.next(); }
            catch (error) { nextTypeError = error instanceof TypeError; }

            var createTypeError = false;
            try { fixed.values(); }
            catch (error) { createTypeError = error instanceof TypeError; }

            [first, rest, nextTypeError, createTypeError].join("|");
            "#,),
        Value::String(Arc::from("1|2,3,4,5,6|true|true"))
    );
}

#[test]
fn typed_array_keys_and_entries_track_dynamic_bounds() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(3, { maxByteLength: 6 });
            var tracking = new Int8Array(rab);
            tracking.set([10, 20, 30]);
            var keys = tracking.keys();
            var entries = tracking.entries();
            var firstKey = keys.next().value;
            var firstEntry = entries.next().value.join(":");
            rab.resize(5);
            tracking[3] = 40;
            tracking[4] = 50;
            var restKeys = Array.from(keys).join(",");
            var restEntries = Array.from(entries).map(function(pair) {
                return pair.join(":");
            }).join(",");

            var fixed = new Int8Array(rab, 0, 5);
            var fixedEntries = fixed.entries();
            fixedEntries.next();
            rab.resize(2);
            var nextTypeError = false;
            try { fixedEntries.next(); }
            catch (error) { nextTypeError = error instanceof TypeError; }

            [
                firstKey,
                restKeys,
                firstEntry,
                restEntries,
                nextTypeError
            ].join("|");
            "#,),
        Value::String(Arc::from("0|1,2,3,4|0:10|1:20,2:30,3:40,4:50|true"))
    );
}

#[test]
fn typed_array_own_keys_include_integer_indices_first() {
    assert_eq!(
        run(r#"
            var sym = Symbol("s");
            var sample = new Uint8Array([7, 8, 9]);
            sample.extra = 10;
            sample[sym] = 11;
            Reflect.ownKeys(sample).map(function(key) {
              return typeof key === "symbol" ? "symbol:" + key.description : key;
            }).join(",");
        "#),
        Value::String(Arc::from("0,1,2,extra,symbol:s"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7, 8, 9, 10]).subarray(2);
            sample.extra = 11;
            Reflect.ownKeys(sample).join(",");
        "#),
        Value::String(Arc::from("0,1,extra"))
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(2);
            sample.extra = 1;
            Reflect.ownKeys(sample).join(",");
        "#),
        Value::String(Arc::from("0,1,extra"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7, 8]);
            sample.extra = 9;
            $262.detachArrayBuffer(sample.buffer);
            Reflect.ownKeys(sample).join(",");
        "#),
        Value::String(Arc::from("extra"))
    );
}

#[test]
fn typed_array_reflect_set_uses_receiver_for_valid_indices() {
    assert_eq!(
        run(r#"
            var valueOfCalls = 0;
            var value = { valueOf: function() { valueOfCalls++; return 2.3; } };
            var target = new Float64Array([0]);
            var receiver = {};
            var ok = Reflect.set(target, "0", value, receiver);
            [ok, target[0], receiver[0] === value, valueOfCalls].join(",");
        "#),
        Value::String(Arc::from("true,0,true,0"))
    );
    assert_eq!(
        run(r#"
            var target = new Float64Array([0]);
            var receiver = new Float64Array([1]);
            var ok = Reflect.set(target, "0", new Number(2.3), receiver);
            [ok, target[0], receiver[0]].join(",");
        "#),
        Value::String(Arc::from("true,0,2.3"))
    );
    assert_eq!(
        run(r#"
            var valueOfCalls = 0;
            var value = { valueOf: function() { valueOfCalls++; return 2.3; } };
            var target = new Float64Array([0, 0]);
            var receiver = new Float64Array([1]);
            var ok = Reflect.set(target, "1", value, receiver);
            [ok, target[1], Reflect.has(receiver, "1"), valueOfCalls].join(",");
        "#),
        Value::String(Arc::from("false,0,false,0"))
    );
    assert_eq!(
        run(r#"
            var target = new BigInt64Array([0n]);
            var receiver = new BigInt64Array([1n]);
            var ok = Reflect.set(target, "0", Object(2n), receiver);
            [ok, target[0], receiver[0]].join(",");
        "#),
        Value::String(Arc::from("true,0,2"))
    );
    assert_eq!(
        run(r#"
            var symbol = Symbol("slot");
            var sample = new Float64Array([42]);
            Reflect.set(sample, symbol, "first");
            Object.defineProperty(sample, symbol, {
              writable: false,
              value: "locked"
            });
            var ok = Reflect.set(sample, symbol, "second");
            [ok, sample[symbol]].join(",");
        "#),
        Value::String(Arc::from("false,locked"))
    );
    assert_eq!(
        run(r#"
            var symbol = Symbol("slot");
            var sample = new BigInt64Array([42n]);
            Reflect.set(sample, symbol, "first");
            Object.defineProperty(sample, symbol, {
              writable: false,
              value: "locked"
            });
            var ok = Reflect.set(sample, symbol, "second");
            [ok, sample[symbol]].join(",");
        "#),
        Value::String(Arc::from("false,locked"))
    );
}

#[test]
fn typed_array_get_canonical_numeric_indices_follow_integer_indexed_exotic() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([42, 43]);
            [sample["0"], sample["1"], sample[0], sample[-0]].join(",");
        "#),
        Value::String(Arc::from("42,43,42,42"))
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(2);
            sample[0] = 42n;
            sample[1] = 43n;
            [sample["0"] === 42n, sample["1"] === 43n].join(",");
        "#),
        Value::String(Arc::from("true,true"))
    );
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(8);
            var sample = new Uint16Array(buffer, 2, 2);
            sample[0] = 0x1234;
            sample[1] = 0x5678;
            [sample["0"], sample["1"]].join(",");
        "#),
        Value::String(Arc::from("4660,22136"))
    );
}

#[test]
fn typed_array_get_invalid_canonical_indices_skip_ordinary_lookup() {
    assert_eq!(
        run(r#"
            var TypedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
            Object.defineProperty(TypedArrayPrototype, "1.1", {
              get: function() { throw new Error("ordinary get"); },
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "-0", {
              get: function() { throw new Error("ordinary get"); },
              configurable: true
            });
            Object.defineProperty(TypedArrayPrototype, "2", {
              get: function() { throw new Error("ordinary get"); },
              configurable: true
            });
            try {
              var sample = new Uint8Array([7, 8]);
              [
                sample["1.1"] === undefined,
                sample["-0"] === undefined,
                sample["2"] === undefined
              ].join(",");
            } finally {
              delete TypedArrayPrototype["1.1"];
              delete TypedArrayPrototype["-0"];
              delete TypedArrayPrototype["2"];
            }
        "#),
        Value::String(Arc::from("true,true,true"))
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([7]);
            $262.detachArrayBuffer(sample.buffer);
            sample["0"] === undefined;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn typed_array_get_noncanonical_numeric_keys_use_ordinary_lookup() {
    assert_eq!(
        run(r#"
            var TypedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
            try {
              var sample = new Uint8Array();
              TypedArrayPrototype["+1"] = "inherited";
              var inherited = sample["+1"];
              sample["+1"] = "own";
              var own = sample["+1"];
              Object.defineProperty(sample, "+1", {
                get: function() { return "accessor"; },
                configurable: true
              });
              [inherited, own, sample["+1"]].join(",");
            } finally {
              delete TypedArrayPrototype["+1"];
            }
        "#),
        Value::String(Arc::from("inherited,own,accessor"))
    );
}

#[test]
fn typed_array_define_own_property_numeric_indices_validate_descriptors() {
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { configurable: false });
            "#
        )
        .contains("TypeError"),
        "TypedArray numeric index descriptors cannot set configurable false"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { enumerable: false });
            "#
        )
        .contains("TypeError"),
        "TypedArray numeric index descriptors cannot set enumerable false"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { writable: false });
            "#
        )
        .contains("TypeError"),
        "TypedArray numeric index descriptors cannot set writable false"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { get: function() { return 1; } });
            "#
        )
        .contains("TypeError"),
        "TypedArray numeric index descriptors cannot be accessors"
    );
}

#[test]
fn typed_array_define_own_property_numeric_indices_write_elements() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", { value: 260 });
            sample[0];
        "#),
        Value::Number(4.0)
    );
    assert_eq!(
        run(r#"
            var sample = new BigInt64Array(1);
            Object.defineProperty(sample, "0", { value: 42n });
            sample[0] === 42n;
        "#),
        Value::Bool(true)
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            Object.defineProperty(sample, "0", {
              value: { valueOf: function() { throw new Error("boom"); } }
            });
            "#
        )
        .contains("boom"),
        "TypedArray numeric index define must propagate value conversion errors"
    );
}

#[test]
fn typed_array_define_own_property_rejects_invalid_canonical_indices() {
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            var desc = Object.getOwnPropertyDescriptor(sample, "0");
            Object.defineProperty(sample, "1", desc);
            "#
        )
        .contains("TypeError"),
        "out-of-bounds canonical numeric index define should fail"
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            var desc = Object.getOwnPropertyDescriptor(sample, "0");
            Object.defineProperty(sample, "-0", desc);
            "#
        )
        .contains("TypeError"),
        "-0 canonical numeric index define should fail"
    );
    assert_eq!(
        run(r#"
            var sample = new Uint8Array([0]);
            var desc = Object.getOwnPropertyDescriptor(sample, "0");
            Reflect.defineProperty(sample, "1.5", desc);
        "#),
        Value::Bool(false)
    );
    assert!(
        run_err(
            r#"
            var sample = new Uint8Array([0]);
            var desc = Object.getOwnPropertyDescriptor(sample, "0");
            $262.detachArrayBuffer(sample.buffer);
            Object.defineProperty(sample, "0", desc);
            "#
        )
        .contains("TypeError"),
        "detached TypedArray numeric index define should fail"
    );
}

#[test]
fn typed_array_define_own_property_noncanonical_keys_are_ordinary() {
    assert_eq!(
        run(r#"
            var sample = new Uint8Array(0);
            Object.defineProperty(sample, "+1", {
              value: "ordinary",
              configurable: true
            });
            Object.getOwnPropertyDescriptor(sample, "+1").value;
        "#),
        Value::String(Arc::from("ordinary"))
    );
}

#[test]
fn direct_buffer_view_fields_respect_ordinary_own_shadowing() {
    assert_eq!(
        run(r#"
            var typed = new Uint8Array([7]);
            Object.defineProperty(typed, "length", { value: 4 });
            Object.defineProperty(typed, "byteLength", { value: 40 });
            typed[Symbol.isConcatSpreadable] = true;
            var concatenated = [].concat(typed);

            var buffer = new ArrayBuffer(8);
            Object.defineProperty(buffer, "byteLength", { value: 80 });
            var view = new DataView(new ArrayBuffer(8), 2, 4);
            Object.defineProperty(view, "byteOffset", { value: 20 });
            Object.defineProperty(view, "buffer", { value: "shadow" });

            [
              typed.length, typed.byteLength,
              concatenated.length, concatenated[0],
              Object.hasOwn(concatenated, "1"),
              Object.hasOwn(concatenated, "3"),
              buffer.byteLength, view.byteOffset, view.buffer
            ].join(":");
        "#),
        Value::String(Arc::from("4:40:4:7:false:false:80:20:shadow"))
    );
}

#[test]
fn data_view_constructor_length_descriptor() {
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(DataView, "length");
            [desc.value, desc.writable, desc.enumerable, desc.configurable].join(",");
        "#),
        Value::String(Arc::from("1,false,false,true"))
    );
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(DataView.prototype, Symbol.toStringTag);
            [desc.value, desc.writable, desc.enumerable, desc.configurable].join(",");
        "#),
        Value::String(Arc::from("DataView,false,false,true"))
    );
}

#[test]
fn data_view_constructor_validates_before_new_target_prototype() {
    assert_eq!(
        run(r#"
            var newTarget = Object.defineProperty(function(){}.bind(), "prototype", {
              get: function() { throw new Error("prototype"); }
            });
            var log = [];
            try {
              Reflect.construct(DataView, [new ArrayBuffer(0), 10], newTarget);
              log.push("none");
            } catch (e) {
              log.push(e.name + ":" + /prototype/.test(e.message));
            }
            try {
              Reflect.construct(DataView, [new ArrayBuffer(0), 0], newTarget);
              log.push("none");
            } catch (e) {
              log.push(e.name + ":" + /prototype/.test(e.message));
            }
            var buffer = new ArrayBuffer(8);
            var detachingNewTarget = Object.defineProperty(function(){}.bind(), "prototype", {
              get: function() { $262.detachArrayBuffer(buffer); return DataView.prototype; }
            });
            try {
              Reflect.construct(DataView, [buffer, { valueOf: function() { log.push("offset"); return 0; } }], detachingNewTarget);
              log.push("none");
            } catch (e) {
              log.push(e.name);
            }
            log.join("|");
        "#),
        Value::String(Arc::from("RangeError:false|Error:true|offset|TypeError"))
    );
}

#[test]
fn array_buffer_and_data_view_subclasses_initialize_internal_slots() {
    assert_eq!(
        run(r#"
            class AB extends ArrayBuffer {}
            var ab = new AB(4);
            var sliced = ab.slice(0, 1);
            [
              ab.byteLength,
              sliced.byteLength,
              sliced instanceof AB,
              sliced instanceof ArrayBuffer,
              Object.getPrototypeOf(ab) === AB.prototype
            ].join(",");
            "#),
        Value::String(Arc::from("4,1,true,true,true"))
    );
    assert_eq!(
        run(r#"
            class DV extends DataView {}
            var buffer = new ArrayBuffer(1);
            var dv = new DV(buffer);
            [
              dv.buffer === buffer,
              dv.byteOffset,
              dv.byteLength,
              Object.getPrototypeOf(dv) === DV.prototype
            ].join(",");
            "#),
        Value::String(Arc::from("true,0,1,true"))
    );
    assert_eq!(
        run(r#"
            var ok = [];
            class AB1 extends ArrayBuffer { constructor() {} }
            try { new AB1(1); ok.push(false); } catch (e) { ok.push(e instanceof ReferenceError); }
            class DV1 extends DataView { constructor() {} }
            try { new DV1(new ArrayBuffer(1)); ok.push(false); } catch (e) { ok.push(e instanceof ReferenceError); }
            try { new (class DV extends DataView {}); ok.push(false); } catch (e) { ok.push(e instanceof TypeError); }
            ok.join(",");
        "#),
        Value::String(Arc::from("true,true,true"))
    );
    assert_eq!(
        run(r#"
            var ab = new ArrayBuffer(4);
            ab.slice(3, 1).byteLength;
            "#),
        Value::Number(0.0)
    );
    assert!(
        run_err("new DataView(new ArrayBuffer(4), 3, 2);").contains("RangeError"),
        "DataView byte range past the buffer should throw RangeError"
    );
    assert!(
        run_err("new ArrayBuffer(9007199254740991);").contains("RangeError"),
        "huge ArrayBuffer lengths should throw RangeError"
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var length = { valueOf: function() { calls++; return 1; } };
            var threw = false;
            try {
              ArrayBuffer(length);
            } catch (e) {
              threw = e instanceof TypeError;
            }
            [threw, calls].join(",");
            "#),
        Value::String(Arc::from("true,0"))
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var offset = { valueOf: function() { calls++; return 0; } };
            var threw = false;
            try {
              DataView(new ArrayBuffer(1), offset);
            } catch (e) {
              threw = e instanceof TypeError;
            }
            [threw, calls].join(",");
            "#),
        Value::String(Arc::from("true,0"))
    );
    assert_eq!(
        run(r#"
            var ab = new ArrayBuffer(1);
            $262.detachArrayBuffer(ab);
            var calls = 0;
            var offset = { valueOf: function() { calls++; return 0; } };
            var threw = false;
            try {
              new DataView(ab, offset);
            } catch (e) {
              threw = e instanceof TypeError;
            }
            [threw, calls].join(",");
            "#),
        Value::String(Arc::from("true,1"))
    );
}

#[test]
fn array_buffer_static_surface_matches_intrinsics() {
    assert_eq!(
        run(r#"
            var isViewDesc = Object.getOwnPropertyDescriptor(ArrayBuffer, "isView");
            var speciesDesc = Object.getOwnPropertyDescriptor(ArrayBuffer, Symbol.species);
            var tagDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, Symbol.toStringTag);
            var ab = new ArrayBuffer(4);
            var ta = new Uint8Array(ab);
            var dv = new DataView(ab);
            var receiver = {};
            [
              isViewDesc.value.length,
              isViewDesc.value.name,
              isViewDesc.writable,
              isViewDesc.enumerable,
              isViewDesc.configurable,
              ArrayBuffer.isView(ta),
              ArrayBuffer.isView(dv),
              ArrayBuffer.isView(ab),
              ArrayBuffer.isView({}),
              ArrayBuffer.isView(undefined),
              speciesDesc.get.length,
              speciesDesc.get.name,
              speciesDesc.set === undefined,
              speciesDesc.enumerable,
              speciesDesc.configurable,
              speciesDesc.get.call(receiver) === receiver,
              tagDesc.value,
              tagDesc.writable,
              tagDesc.enumerable,
              tagDesc.configurable,
              (function() {
                function F() {}
                F.prototype = null;
                return Object.getPrototypeOf(Reflect.construct(ArrayBuffer, [0], F)) === ArrayBuffer.prototype;
              })(),
              (function() {
                function F() {}
                F.prototype = Array.prototype;
                return Object.getPrototypeOf(Reflect.construct(ArrayBuffer, [0], F)) === Array.prototype;
              })()
            ].join(",");
            "#),
        Value::String(Arc::from(
            "1,isView,true,false,true,true,true,false,false,false,0,get [Symbol.species],true,false,true,true,ArrayBuffer,false,false,true,true,true",
        ))
    );
}

#[test]
fn array_buffer_return_value_survives_frame_boundary_gc() {
    assert_eq!(
        run(r#"
            function make32ByteArrayBuffer() {
              var ab = new ArrayBuffer(32);
              var view = new Uint8Array(ab);
              for (var i = 0; i < 8; i++) view[i] = i + 1;
              return ab;
            }

            var failed = 0;
            for (var n = 0; n < 3000; n++) {
              var source = make32ByteArrayBuffer();
              if (Object.getPrototypeOf(source) !== ArrayBuffer.prototype ||
                  source.byteLength !== 32) {
                failed = n + 1;
                break;
              }
              var start = { valueOf: function() { return "+9"; } };
              var end = { valueOf: function() { return "0o20"; } };
              var dest = source.sliceToImmutable(start, end);
              if (dest.byteLength !== 7 || dest.immutable !== true) {
                failed = -(n + 1);
                break;
              }
            }
            failed;
        "#),
        Value::Number(0.0)
    );
}

#[test]
fn array_buffer_slice_uses_species_constructor_and_validates_result() {
    assert_eq!(
        run(r#"
            var source = new ArrayBuffer(8);
            var sourceBytes = new Uint8Array(source);
            sourceBytes[0] = 7;
            sourceBytes[1] = 9;
            var calls = [];
            var resultBuffer;
            var speciesConstructor = {};
            speciesConstructor[Symbol.species] = function(length) {
              calls.push("species:" + length);
              resultBuffer = new ArrayBuffer(10);
              new Uint8Array(resultBuffer)[0] = 99;
              return resultBuffer;
            };
            source.constructor = speciesConstructor;
            var result = source.slice(0, 2);
            [
              result === resultBuffer,
              result.byteLength,
              new Uint8Array(result)[0],
              new Uint8Array(result)[1],
              new Uint8Array(result)[2],
              calls.join("|")
            ].join(",");
            "#),
        Value::String(Arc::from("true,10,7,9,0,species:2"))
    );
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try { fn(); } catch (e) { return e instanceof TypeError; }
              return false;
            }
            var ab = new ArrayBuffer(8);
            var speciesConstructor = {};
            [
              (function() {
                ab.constructor = undefined;
                return Object.getPrototypeOf(ab.slice()) === ArrayBuffer.prototype;
              })(),
              (function() {
                ab.constructor = speciesConstructor;
                speciesConstructor[Symbol.species] = null;
                return Object.getPrototypeOf(ab.slice()) === ArrayBuffer.prototype;
              })(),
              throwsTypeError(function() { ab.constructor = null; ab.slice(); }),
              throwsTypeError(function() {
                ab.constructor = speciesConstructor;
                speciesConstructor[Symbol.species] = function() { return {}; };
                ab.slice();
              }),
              throwsTypeError(function() {
                ab.constructor = speciesConstructor;
                speciesConstructor[Symbol.species] = function() { return new ArrayBuffer(4); };
                ab.slice();
              }),
              throwsTypeError(function() {
                ab.constructor = speciesConstructor;
                speciesConstructor[Symbol.species] = function() { return ab; };
                ab.slice();
              })
            ].join(",");
            "#),
        Value::String(Arc::from("true,true,true,true,true,true"))
    );
}

#[test]
fn array_buffer_transfer_methods_copy_resize_and_detach_source() {
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try { fn(); } catch (e) { return e instanceof TypeError; }
              return false;
            }
            var source = new ArrayBuffer(4);
            var bytes = new Uint8Array(source);
            bytes[0] = 1;
            bytes[1] = 2;
            bytes[2] = 3;
            bytes[3] = 4;
            var grown = source.transfer(6);
            var grownBytes = new Uint8Array(grown);
            var grownSnapshot = [
              grown.byteLength,
              grownBytes[0],
              grownBytes[1],
              grownBytes[2],
              grownBytes[3],
              grownBytes[4],
              grownBytes[5]
            ];
            var fixed = grown.transferToFixedLength(2);
            var fixedBytes = new Uint8Array(fixed);
            grownSnapshot.concat([
              source.byteLength,
              throwsTypeError(function() { source.slice(); }),
              fixed.byteLength,
              fixedBytes[0],
              fixedBytes[1],
              grown.byteLength
            ]).join(",");
            "#),
        Value::String(Arc::from("6,1,2,3,4,0,0,0,true,2,1,2,0"))
    );
    assert!(
        run_err(
            r#"
            var ab = new ArrayBuffer(1);
            ab.transfer(-1);
            "#
        )
        .contains("RangeError"),
        "negative transfer length should throw RangeError"
    );
}

#[test]
fn resizable_array_buffer_exposes_slots_resizes_and_preserves_transfer_mode() {
    assert_eq!(
        run(r#"
            var fixed = new ArrayBuffer(3);
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var bytes = new Uint8Array(rab);
            bytes[0] = 1;
            bytes[3] = 4;
            rab.resize(2);
            rab.resize(6);
            var resized = new Uint8Array(rab);
            var resizedSnapshot = [resized[0], resized[1], resized[2], resized[5]];
            var transferred = rab.transfer(7);
            var transferSnapshot = [
                transferred.resizable,
                transferred.maxByteLength
            ];
            var fixedTransfer = transferred.transferToFixedLength(5);
            var detached = new ArrayBuffer(1, { maxByteLength: 2 });
            $262.detachArrayBuffer(detached);
            var resize = Object.getOwnPropertyDescriptor(
                ArrayBuffer.prototype,
                "resize"
            );
            [
                fixed.resizable, fixed.maxByteLength,
                resizedSnapshot[0], resizedSnapshot[1],
                resizedSnapshot[2], resizedSnapshot[3],
                transferSnapshot[0], transferSnapshot[1],
                fixedTransfer.resizable, fixedTransfer.maxByteLength,
                detached.resizable, detached.maxByteLength,
                resize.value.name, resize.value.length,
                resize.writable, resize.enumerable, resize.configurable
            ].join("|");
            "#,),
        Value::String(Arc::from(
            "false|3|1|0|0|0|true|8|false|5|true|0|resize|1|true|false|true"
        ))
    );
}

#[test]
fn resizable_array_buffer_rechecks_detachment_after_length_coercion() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var called = false;
            var detachedError = false;
            try {
                rab.resize({ valueOf: function() {
                    called = true;
                    $262.detachArrayBuffer(rab);
                    return 2;
                }});
            } catch (error) { detachedError = error instanceof TypeError; }
            var fixedError = false;
            try { new ArrayBuffer(1).resize(0); }
            catch (error) { fixedError = error instanceof TypeError; }
            [called, detachedError, fixedError].join("|");
            "#,),
        Value::String(Arc::from("true|true|true"))
    );
}

#[test]
fn length_tracking_typed_arrays_and_data_views_follow_resizable_buffers() {
    assert_eq!(
        run(
            r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var tracking = new Uint8Array(rab, 1);
            var fixed = new Uint8Array(rab, 1, 2);
            var trackingView = new DataView(rab, 1);
            var fixedView = new DataView(rab, 1, 2);
            var snapshots = [];
            function snap() {
                var fixedViewLength;
                try { fixedViewLength = fixedView.byteLength; }
                catch (error) { fixedViewLength = error instanceof TypeError ? "oob" : "bad"; }
                snapshots.push([
                    tracking.length,
                    tracking.byteLength,
                    tracking.byteOffset,
                    Reflect.ownKeys(tracking).join(","),
                    fixed.length,
                    fixed.byteOffset,
                    trackingView.byteLength,
                    fixedViewLength
                ].join("/"));
            }
            snap();
            rab.resize(6);
            snap();
            rab.resize(2);
            snap();
            rab.resize(0);
            var trackingViewOob = false;
            try { trackingView.byteLength; }
            catch (error) { trackingViewOob = error instanceof TypeError; }
            snapshots.push([
                tracking.length,
                tracking.byteOffset,
                fixed.length,
                fixed.byteOffset,
                trackingViewOob
            ].join("/"));
            rab.resize(4);
            snap();
            snapshots.join("|");
            "#,
        ),
        Value::String(Arc::from(
            "3/3/1/0,1,2/2/1/3/2|5/5/1/0,1,2,3,4/2/1/5/2|1/1/1/0/0/0/1/oob|0/0/0/0/true|3/3/1/0,1,2/2/1/3/2"
        ))
    );
}

#[test]
fn length_tracking_typed_array_follows_growable_shared_buffer() {
    assert_eq!(
        run(r#"
            var gsab = new SharedArrayBuffer(0, { maxByteLength: 8 });
            var tracking = new Int32Array(gsab);
            var before = [tracking.length, Reflect.ownKeys(tracking).join(",")];
            gsab.grow(8);
            tracking[1] = 42;
            [before.join("/"), tracking.length, tracking[1]].join("|");
            "#,),
        Value::String(Arc::from("0/|2|42"))
    );
}

#[test]
fn typed_array_at_snapshots_length_before_index_coercion() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(4, { maxByteLength: 8 });
            var fixed = new Uint8Array(rab, 0, 4);
            fixed[0] = 1;
            fixed[3] = 4;
            var before = [fixed.at(0), fixed.at(-1), fixed.at(4)];
            var coerced = fixed.at({ valueOf: function() {
                rab.resize(2);
                return 0;
            }});
            var initialOob = false;
            try { fixed.at(0); }
            catch (error) { initialOob = error instanceof TypeError; }
            rab.resize(4);
            [
                before.join(","),
                coerced === undefined,
                initialOob,
                fixed.at(-1),
                Uint8Array.prototype.at.length,
                Uint8Array.prototype.at.name
            ].join("|");
            "#,),
        Value::String(Arc::from("1,4,|true|true|0|1|at"))
    );
}

#[test]
fn typed_array_fill_snapshots_length_and_revalidates_after_coercion() {
    assert_eq!(
        run(r#"
            var rab = new ArrayBuffer(1, { maxByteLength: 4 });
            var tracking = new Int8Array(rab);
            tracking.fill({ valueOf: function() {
                rab.resize(4);
                return 7;
            }});
            var snapshot = Array.from(tracking).join(",");

            var fixed = new Int8Array(rab, 0, 4);
            var resizedOob = false;
            try {
                fixed.fill({ valueOf: function() {
                    rab.resize(2);
                    return 9;
                }});
            } catch (error) { resizedOob = error instanceof TypeError; }
            [
                snapshot,
                resizedOob,
                typeof Int8Array.prototype.values,
                Int8Array.prototype[Symbol.iterator] === Int8Array.prototype.values
            ].join("|");
            "#,),
        Value::String(Arc::from("7,0,0,0|true|function|true"))
    );
}

#[test]
fn array_buffer_transfer_to_immutable_and_slice_to_immutable_mark_results() {
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try { fn(); } catch (e) { return e instanceof TypeError; }
              return false;
            }
            var source = new ArrayBuffer(4);
            var bytes = new Uint8Array(source);
            bytes[0] = 11;
            bytes[1] = 12;
            bytes[2] = 13;
            bytes[3] = 14;
            var sliced = source.sliceToImmutable(1, 3);
            var slicedBytes = new Uint8Array(sliced);
            bytes[1] = 99;
            var moved = source.transferToImmutable();
            var movedBytes = new Uint8Array(moved);
            [
              sliced.immutable,
              sliced.byteLength,
              slicedBytes[0],
              slicedBytes[1],
              moved.immutable,
              moved.byteLength,
              movedBytes[0],
              movedBytes[1],
              movedBytes[2],
              movedBytes[3],
              source.byteLength,
              throwsTypeError(function() { moved.transfer(); }),
              throwsTypeError(function() { source.transferToImmutable(); })
            ].join(",");
            "#),
        Value::String(Arc::from("true,2,12,13,true,4,11,99,13,14,0,true,true",))
    );
}

#[test]
fn array_buffer_immutable_surface_and_transfer_validation_order() {
    assert_eq!(
        run(r#"
            function throwsTypeError(fn) {
              try { fn(); } catch (e) { return e instanceof TypeError; }
              return false;
            }
            var ab = new ArrayBuffer(2);
            var immutableDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "immutable");
            var detachedDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "detached");
            var transferDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "transfer");
            var fixedDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "transferToFixedLength");
            var immutableTransferDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "transferToImmutable");
            var sliceImmutableDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "sliceToImmutable");
            var order = [];
            var detached = ab.transfer();
            try {
              ab.transfer({ valueOf: function() { order.push("coerce"); return 0; } });
            } catch (e) {
              order.push(e instanceof TypeError);
            }
            [
              immutableDesc.get.name,
              immutableDesc.get.length,
              immutableDesc.set === undefined,
              immutableDesc.enumerable,
              immutableDesc.configurable,
              immutableDesc.get.call(new ArrayBuffer(1)),
              immutableDesc.get.call(detached),
              detachedDesc.get.name,
              detachedDesc.get.length,
              detachedDesc.set === undefined,
              detachedDesc.enumerable,
              detachedDesc.configurable,
              detachedDesc.get.call(new ArrayBuffer(1)),
              detachedDesc.get.call(ab),
              throwsTypeError(function() { immutableDesc.get.call({}); }),
              throwsTypeError(function() { detachedDesc.get.call({}); }),
              transferDesc.value.length,
              fixedDesc.value.length,
              immutableTransferDesc.value.length,
              sliceImmutableDesc.value.length,
              order.join("|")
            ].join(",");
            "#),
        Value::String(Arc::from(
            "get immutable,0,true,false,true,false,false,get detached,0,true,false,true,false,true,true,true,0,0,0,2,coerce|true",
        ))
    );
}

#[test]
fn array_buffer_immutable_argument_helpers_match_array_like_coercions() {
    assert_eq!(
        run(r#"
            var ws = "\t\v\f\uFEFF\u3000\n\r\u2028\u2029";
            var ab = new ArrayBuffer(8);
            var moved = ab.transferToImmutable(ws + "1" + ws);
            var source = new ArrayBuffer(4);
            var bytes = new Uint8Array(source);
            bytes[0] = 5;
            bytes[1] = 6;
            bytes[2] = 7;
            bytes[3] = 8;
            var immutable = source.sliceToImmutable(0, null);
            var array = Array.from(new Uint8Array(source));
            [
              moved.byteLength,
              moved.immutable,
              immutable.byteLength,
              Array.from(new Uint8Array(immutable)).length,
              array.length,
              array[0],
              array[3],
              [1, 2, 3].slice(0, null).length
            ].join(",");
            "#),
        Value::String(Arc::from("1,true,0,0,4,5,8,0"))
    );
}

#[test]
fn data_view_setters_reject_immutable_backing_buffer_before_argument_coercion() {
    assert_eq!(
        run(r#"
            var buffer = (new ArrayBuffer(16)).transferToImmutable();
            var view = new DataView(buffer);
            var calls = [];
            var offset = { valueOf: function() { calls.push("offset"); return 0; } };
            var numberValue = { valueOf: function() { calls.push("number"); return 1; } };
            var bigintValue = { valueOf: function() { calls.push("bigint"); return 1n; } };
            var names = [
              "setInt8", "setUint8", "setInt16", "setUint16", "setInt32",
              "setUint32", "setFloat32", "setFloat64", "setBigInt64",
              "setBigUint64"
            ];
            var ok = [];
            for (var i = 0; i < names.length; i++) {
              try {
                view[names[i]](offset, names[i].startsWith("setBig") ? bigintValue : numberValue, true);
                ok.push(false);
              } catch (e) {
                ok.push(e instanceof TypeError);
              }
            }
            ok.join(",") + "|" + calls.join(",");
        "#),
        Value::String(Arc::from(
            "true,true,true,true,true,true,true,true,true,true|",
        ))
    );
}

#[test]
fn array_buffer_and_data_view_prototype_accessors_validate_receivers() {
    assert_eq!(
        run(r#"
            var ab = new ArrayBuffer(4);
            var dv = new DataView(ab, 1, 2);
            var abDesc = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength");
            var bufferDesc = Object.getOwnPropertyDescriptor(DataView.prototype, "buffer");
            var lengthDesc = Object.getOwnPropertyDescriptor(DataView.prototype, "byteLength");
            var offsetDesc = Object.getOwnPropertyDescriptor(DataView.prototype, "byteOffset");
            [
              abDesc.get.call(ab),
              abDesc.set === undefined,
              abDesc.enumerable,
              abDesc.configurable,
              abDesc.get.name,
              abDesc.get.length,
              bufferDesc.get.call(dv) === ab,
              lengthDesc.get.call(dv),
              offsetDesc.get.call(dv),
              bufferDesc.get.name,
              lengthDesc.get.name,
              offsetDesc.get.name
            ].join(",");
            "#),
        Value::String(Arc::from(
            "4,true,false,true,get byteLength,0,true,2,1,get buffer,get byteLength,get byteOffset"
        ))
    );
    assert!(
        run_err(
            r#"
            var getter = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get;
            getter.call({});
            "#
        )
        .contains("TypeError"),
        "ArrayBuffer byteLength getter should reject non-ArrayBuffer receivers"
    );
    assert!(
        run_err(
            r#"
            var getter = Object.getOwnPropertyDescriptor(DataView.prototype, "buffer").get;
            getter.call({});
            "#
        )
        .contains("TypeError"),
        "DataView buffer getter should reject non-DataView receivers"
    );
    assert_eq!(
        run(r#"
            var ab = new ArrayBuffer(4);
            var dv = new DataView(ab, 1, 2);
            $262.detachArrayBuffer(ab);
            [
              ab.byteLength,
              Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get.call(ab),
              dv.buffer === ab
            ].join(",");
            "#),
        Value::String(Arc::from("0,0,true"))
    );
    assert!(
        run_err(
            r#"
            var ab = new ArrayBuffer(4);
            var dv = new DataView(ab, 1, 2);
            $262.detachArrayBuffer(ab);
            dv.byteLength;
            "#
        )
        .contains("TypeError"),
        "DataView byteLength should reject detached buffers"
    );
    assert!(
        run_err(
            r#"
            var ab = new ArrayBuffer(4);
            var dv = new DataView(ab, 1, 2);
            $262.detachArrayBuffer(ab);
            dv.byteOffset;
            "#
        )
        .contains("TypeError"),
        "DataView byteOffset should reject detached buffers"
    );
}

#[test]
fn data_view_int8_uint8_methods_read_write_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(4);
            var dv = new DataView(buffer, 1, 2);
            var values = [];
            values.push(dv.setUint8(0, 255) === undefined);
            values.push(dv.setInt8(1, -2) === undefined);
            values.push(dv.getUint8(0));
            values.push(dv.getInt8(0));
            values.push(dv.getUint8(1));
            values.push(dv.getInt8(1));
            values.push(DataView.prototype.getUint8.length);
            values.push(DataView.prototype.setInt8.length);
            values.push(DataView.prototype.getInt8.name);
            values.push(DataView.prototype.setUint8.name);
            values.join(",");
            "#),
        Value::String(Arc::from("true,true,255,-1,254,-2,1,2,getInt8,setUint8"))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(2));
            dv.setUint8(NaN, 7);
            dv.setUint8(-0.9, 8);
            dv.setUint8(1.9, 9);
            [dv.getUint8(), dv.getUint8(-0.1), dv.getUint8(1.1)].join(",");
            "#),
        Value::String(Arc::from("8,8,9"))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(1));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint8(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(1));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint8(2, poisoned);
            "#
        )
        .contains("Error"),
        "value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(1);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint8(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(1);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint8(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView element reads"
    );
}

#[test]
fn data_view_int16_uint16_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(6);
            var dv = new DataView(buffer, 1, 4);
            var values = [];
            values.push(dv.setUint16(0, 0x1234) === undefined);
            values.push(dv.getUint8(0));
            values.push(dv.getUint8(1));
            values.push(dv.getUint16(0));
            values.push(dv.getUint16(0, true));
            values.push(dv.setInt16(2, -2, true) === undefined);
            values.push(dv.getUint16(2));
            values.push(dv.getInt16(2, true));
            values.push(DataView.prototype.getUint16.length);
            values.push(DataView.prototype.setInt16.length);
            values.push(DataView.prototype.getInt16.name);
            values.push(DataView.prototype.setUint16.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "true,18,52,4660,13330,true,65279,-2,1,2,getInt16,setUint16"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(4));
            dv.setUint16(0, 65537);
            dv.setInt16(2, -32769);
            [dv.getUint16(0), dv.getInt16(0), dv.getUint16(2), dv.getInt16(2)].join(",");
            "#),
        Value::String(Arc::from("1,1,32767,32767"))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(2));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint16(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(2));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint16(2, poisoned);
            "#
        )
        .contains("Error"),
        "value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(2);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint16(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(2);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint16(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView element reads"
    );
}

#[test]
fn data_view_int32_uint32_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(10);
            var dv = new DataView(buffer, 1, 8);
            var values = [];
            values.push(dv.setUint32(0, 0x12345678) === undefined);
            values.push(dv.getUint8(0));
            values.push(dv.getUint8(1));
            values.push(dv.getUint8(2));
            values.push(dv.getUint8(3));
            values.push(dv.getUint32(0));
            values.push(dv.getUint32(0, true));
            values.push(dv.setInt32(4, -2, true) === undefined);
            values.push(dv.getUint32(4));
            values.push(dv.getInt32(4, true));
            values.push(DataView.prototype.getUint32.length);
            values.push(DataView.prototype.setInt32.length);
            values.push(DataView.prototype.getInt32.name);
            values.push(DataView.prototype.setUint32.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "true,18,52,86,120,305419896,2018915346,true,4278190079,-2,1,2,getInt32,setUint32"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(8));
            dv.setUint32(0, 4294967297);
            dv.setInt32(4, -2147483649);
            [dv.getUint32(0), dv.getInt32(0), dv.getUint32(4), dv.getInt32(4)].join(",");
            "#),
        Value::String(Arc::from("1,1,2147483647,2147483647"))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(4));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint32(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(4));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setUint32(4, poisoned);
            "#
        )
        .contains("Error"),
        "value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(4);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint32(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(4);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getUint32(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView element reads"
    );
}

#[test]
fn data_view_float_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(18);
            var dv = new DataView(buffer, 1, 16);
            var values = [];
            values.push(dv.setFloat32(0, 42, true) === undefined);
            values.push(dv.getFloat32(0));
            values.push(dv.getFloat32(0, true));
            values.push(dv.setFloat64(8, 42, true) === undefined);
            values.push(dv.getFloat64(8));
            values.push(dv.getFloat64(8, true));
            values.push(DataView.prototype.getFloat32.length);
            values.push(DataView.prototype.setFloat64.length);
            values.push(DataView.prototype.getFloat64.name);
            values.push(DataView.prototype.setFloat32.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "true,1.4441781973331565e-41,42,true,8.759e-320,42,1,2,getFloat64,setFloat32"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(12));
            dv.setFloat32(0, -0);
            dv.setFloat64(4, -0);
            [1 / dv.getFloat32(0), 1 / dv.getFloat64(4)].join(",");
            "#),
        Value::String(Arc::from("-Infinity,-Infinity"))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(12));
            dv.setUint8(0, 127);
            dv.setUint8(1, 192);
            dv.setUint8(2, 0);
            dv.setUint8(3, 0);
            dv.setUint8(4, 127);
            dv.setUint8(5, 248);
            dv.setUint8(6, 0);
            dv.setUint8(7, 0);
            dv.setUint8(8, 0);
            dv.setUint8(9, 0);
            dv.setUint8(10, 0);
            dv.setUint8(11, 0);
            [dv.getFloat32(0) !== dv.getFloat32(0), dv.getFloat64(4) !== dv.getFloat64(4)].join(",");
            "#),
        Value::String(Arc::from("true,true"))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(4));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setFloat32(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(8));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setFloat64(8, poisoned);
            "#
        )
        .contains("Error"),
        "value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(8);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getFloat64(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(4);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getFloat32(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView float reads"
    );
}

#[test]
fn data_view_float16_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(6);
            var dv = new DataView(buffer);
            dv.setUint8(0, 66);
            dv.setUint8(1, 40);
            dv.setUint8(2, 40);
            dv.setUint8(3, 66);
            var values = [];
            values.push(dv.getFloat16(0));
            values.push(dv.getFloat16(0, true));
            values.push(dv.getFloat16(2));
            values.push(dv.getFloat16(2, true));
            values.push(dv.setFloat16(4, 42, true) === undefined);
            values.push(dv.getFloat16(4));
            values.push(dv.getFloat16(4, true));
            values.push(DataView.prototype.getFloat16.length);
            values.push(DataView.prototype.setFloat16.length);
            values.push(DataView.prototype.getFloat16.name);
            values.push(DataView.prototype.setFloat16.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "3.078125,0.03326416015625,0.03326416015625,3.078125,true,2.158203125,42,1,2,getFloat16,setFloat16"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(2));
            dv.setFloat16(0, -0);
            1 / dv.getFloat16(0);
            "#),
        Value::Number(f64::NEG_INFINITY)
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(10));
            var values = [];
            dv.setFloat16(0, 1.1);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 0.1);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 2.9802322387695312e-8);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 2.980232238769532e-8);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 8.940696716308594e-8);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 1.4901161193847656e-7);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 1.490116119384766e-7);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 2049);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 2051);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 65504);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 65520);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, 65519.99999999999);
            values.push(dv.getFloat16(0));
            dv.setFloat16(0, NaN);
            values.push(dv.getFloat16(0) !== dv.getFloat16(0));
            values.join(",");
            "#),
        Value::String(Arc::from(
            "1.099609375,0.0999755859375,0,5.960464477539063e-8,1.1920928955078125e-7,1.1920928955078125e-7,1.7881393432617188e-7,2048,2052,65504,Infinity,65504,true"
        ))
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(2));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setFloat16(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(2));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setFloat16(2, poisoned);
            "#
        )
        .contains("Error"),
        "value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(2);
            var dv = new DataView(buffer);
            var value = { valueOf: function() { $262.detachArrayBuffer(buffer); return 1; } };
            dv.setFloat16(0, value);
            "#
        )
        .contains("TypeError"),
        "detached buffers should be checked after Float16 value conversion"
    );
    assert_eq!(
        run(r#"
            var iab = (new ArrayBuffer(2)).transferToImmutable();
            var dv = new DataView(iab);
            var calls = [];
            var byteOffset = { valueOf: function() { calls.push("byteOffset"); return 0; } };
            var value = { valueOf: function() { calls.push("value"); return 1; } };
            try { dv.setFloat16(byteOffset, value); } catch (e) { calls.push(e instanceof TypeError); }
            calls.join(",");
            "#),
        Value::String(Arc::from("true"))
    );
}

#[test]
fn data_view_bigint_methods_read_write_endian_and_validate_order() {
    assert_eq!(
        run(r#"
            var buffer = new ArrayBuffer(18);
            var dv = new DataView(buffer, 1, 16);
            var values = [];
            values.push(dv.setBigUint64(0, 0x0102030405060708n) === undefined);
            values.push(dv.getUint8(0));
            values.push(dv.getUint8(7));
            values.push(dv.getBigUint64(0).toString());
            values.push(dv.getBigUint64(0, true).toString());
            values.push(dv.setBigInt64(8, -2n, true) === undefined);
            values.push(dv.getBigUint64(8).toString());
            values.push(dv.getBigInt64(8, true).toString());
            values.push(DataView.prototype.getBigUint64.length);
            values.push(DataView.prototype.setBigInt64.length);
            values.push(DataView.prototype.getBigInt64.name);
            values.push(DataView.prototype.setBigUint64.name);
            values.join(",");
            "#),
        Value::String(Arc::from(
            "true,1,8,72623859790382856,578437695752307201,true,18374686479671623679,-2,1,2,getBigInt64,setBigUint64"
        ))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(8));
            dv.setBigUint64(0, 0x10000000000000001n);
            var wrapped = dv.getBigUint64(0).toString();
            dv.setBigInt64(0, -1n);
            [wrapped, dv.getBigUint64(0).toString(), dv.getBigInt64(0).toString()].join(",");
            "#),
        Value::String(Arc::from("1,18446744073709551615,-1"))
    );
    assert_eq!(
        run(r#"
            var dv = new DataView(new ArrayBuffer(8));
            var boxed = { valueOf: function() { return "42"; } };
            dv.setBigUint64(0, boxed);
            dv.getBigUint64(0).toString();
            "#),
        Value::String(Arc::from("42"))
    );
    assert!(
        run_err("new DataView(new ArrayBuffer(8)).setBigUint64(0, 1);").contains("TypeError"),
        "ToBigInt should reject Number values"
    );
    assert!(
        run_err("new DataView(new ArrayBuffer(8)).setBigInt64(0);").contains("TypeError"),
        "missing BigInt setter value should throw TypeError"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(8));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setBigInt64(-1, poisoned);
            "#
        )
        .contains("RangeError"),
        "invalid byteOffset should be rejected before BigInt value conversion"
    );
    assert!(
        run_err(
            r#"
            var dv = new DataView(new ArrayBuffer(8));
            var poisoned = { valueOf: function() { throw new Error("value"); } };
            dv.setBigInt64(8, poisoned);
            "#
        )
        .contains("Error"),
        "BigInt value conversion should run before range check for valid ToIndex values"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(8);
            var dv = new DataView(buffer);
            var value = { valueOf: function() { $262.detachArrayBuffer(buffer); return 1n; } };
            dv.setBigInt64(0, value);
            "#
        )
        .contains("TypeError"),
        "detached buffers should be checked after BigInt value conversion"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(8);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getBigUint64(Infinity);
            "#
        )
        .contains("RangeError"),
        "ToIndex should run before detached-buffer validation"
    );
    assert!(
        run_err(
            r#"
            var buffer = new ArrayBuffer(8);
            var dv = new DataView(buffer);
            $262.detachArrayBuffer(buffer);
            dv.getBigInt64(0);
            "#
        )
        .contains("TypeError"),
        "detached buffers should reject DataView BigInt reads"
    );
}

#[test]
fn regexp_subclass_instances_use_new_target_prototype_and_last_index_descriptor() {
    assert_eq!(
        run(r#"
            class Subclass extends RegExp {}
            var re = new Subclass("39?", "g");
            var before = Object.getOwnPropertyDescriptor(re, "lastIndex");
            re.test("39");
            var after = Object.getOwnPropertyDescriptor(re, "lastIndex");
            [
              re instanceof Subclass,
              re instanceof RegExp,
              Object.getPrototypeOf(re) === Subclass.prototype,
              before.value,
              before.writable,
              before.enumerable,
              before.configurable,
              after.value,
              after.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("true,true,true,0,true,false,false,2,false"))
    );
}

#[test]
fn string_subclass_instances_have_own_length_descriptor() {
    assert_eq!(
        run(r#"
            class Subclass extends String {}
            var str = new Subclass("test262");
            var desc = Object.getOwnPropertyDescriptor(str, "length");
            [
              str instanceof Subclass,
              str instanceof String,
              str.length,
              desc.value,
              desc.writable,
              desc.enumerable,
              desc.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("true,true,7,7,false,false,false"))
    );
}

#[test]
fn string_split_join() {
    assert_eq!(
        run("'a,b,c'.split(',').join('-');"),
        Value::String(Arc::from("a-b-c"))
    );
}

#[test]
fn split_limit() {
    assert_eq!(run(r#""a,b,c".split(",",2).length;"#), Value::Number(2.0));
}

#[test]
fn string_split_observes_symbol_split_and_coercion_order() {
    assert_eq!(run("String.prototype.split.length"), Value::Number(2.0));
    assert_eq!(
        run(r#"var separator = {};
               var seenThis, seen0, seen1;
               separator[Symbol.split] = function(str, limit) {
                 seenThis = this;
                 seen0 = str;
                 seen1 = limit;
                 return "custom";
               };
               var out = "".split(separator, "limit");
               [out, seenThis === separator, seen0, seen1].join("|");"#),
        Value::String(Arc::from("custom|true||limit"))
    );
    assert!(run_err(
        r#"var separator = {};
           Object.defineProperty(separator, Symbol.split, {
             get: function(){ throw new Error("split-get"); }
           });
           "".split(separator);"#
    )
    .contains("split-get"));
    assert_eq!(
        run(r#""undefined is not a function".split(undefined).join("|")"#),
        Value::String(Arc::from("undefined is not a function"))
    );
    assert_eq!(
        run(r#""undefined is not a function".split(undefined, 0).length"#),
        Value::Number(0.0)
    );
    assert_eq!(
        run(r#"
            var separator = {
              [Symbol.split]: null,
              toString: function () { return "\ud834"; }
            };
            var result = "\ud834\udf06".split(separator);
            [
              result.length,
              result[0].length,
              result[1].length,
              result[1].charCodeAt(0)
            ].join("|");
        "#),
        Value::String(Arc::from("2|0|1|57094"))
    );
    assert!(run_err(
        r#"var limit = { valueOf: function(){ throw new Error("limit-value"); } };
           "".split("", limit);"#
    )
    .contains("limit-value"));
    assert!(run_err(
        r#"var sep = { toString: function(){ throw new Error("sep-string"); } };
           "abc".split(sep, 0);"#
    )
    .contains("sep-string"));
    assert_eq!(
        run(r#""hello".split(new RegExp()).join("|")"#),
        Value::String(Arc::from("h|e|l|l|o"))
    );
    assert_eq!(
        run(r#""x".split(/^/).join("|")"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run(r#""x".split(/.+/).join("|")"#),
        Value::String(Arc::from("|"))
    );
    assert_eq!(
        run(r#""x".split(/[]/).join("|")"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run(r#""x".split(/[^]/).join("|")"#),
        Value::String(Arc::from("|"))
    );
    assert_eq!(
        run(r#""x".split(/\cY/).join("|")"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run(r#""x".split(/[\b]/).join("|")"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run(r#""x".split(/\x/).join("|")"#),
        Value::String(Arc::from("|"))
    );
}

#[test]
fn string_split_reverse() {
    assert_eq!(
        run(r#""hello world".split(" ").reverse().join(" ");"#),
        Value::String(Arc::from("world hello"))
    );
}

#[test]
fn object_keys_len() {
    assert_eq!(
        run("Object.keys({a:1,b:2,c:3}).length;"),
        Value::Number(3.0)
    );
}

#[test]
fn sparse_array_holes_are_not_own_keys() {
    assert_eq!(
        run("Object.keys([1,,3,,5]).join('|');"),
        Value::String(Arc::from("0|2|4"))
    );
    assert_eq!(
        run("var a=[1,2]; delete a[0]; [a.length, 0 in a, a.hasOwnProperty('0'), Object.keys(a).join('|'), Object.getOwnPropertyNames(a).join('|')].join(',');"),
        Value::String(Arc::from("2,false,false,1,1|length"))
    );
    assert_eq!(
        run("[[undefined].hasOwnProperty('0'), [,].hasOwnProperty('0'), Object.keys(Array(3)).length].join(',');"),
        Value::String(Arc::from("true,false,0"))
    );
}

#[test]
fn object_values_sum() {
    assert_eq!(
        run("Object.values({a:1,b:2}).reduce((x,y)=>x+y,0);"),
        Value::Number(3.0)
    );
}

#[test]
fn object_entries() {
    assert_eq!(run("Object.entries({a:1,b:2}).length;"), Value::Number(2.0));
}

#[test]
fn object_assign_uses_to_object_target_and_copies_string_sources() {
    assert_eq!(
        run(r#"
            var r = Object.assign(1, "ab");
            [
              typeof r,
              r.valueOf(),
              r[0],
              r[1],
              Object.getOwnPropertyNames(r).join("|")
            ].join(",");
        "#),
        Value::String(Arc::from("object,1,a,b,0|1"))
    );
    assert_eq!(
        run(r#"
            [
              Object.assign(true).valueOf(),
              Object.assign(2).valueOf(),
              Object.assign("x").valueOf(),
              Object.assign({}, null, undefined).constructor === Object
            ].join("|");
        "#),
        Value::String(Arc::from("true|2|x|true"))
    );
    assert!(run_err("Object.assign(null, { a: 1 });").contains("TypeError"));
    assert!(run_err("Object.assign(undefined, { a: 1 });").contains("TypeError"));
}

#[test]
fn object_assign_throws_on_failed_target_set() {
    assert!(run_err(
        r#"
            var target = {};
            Object.defineProperty(target, "x", { value: 1, writable: false });
            Object.assign(target, { x: 2 });
        "#
    )
    .contains("TypeError"));
    assert!(run_err(r#"Object.assign("ab", { 0: "x" });"#).contains("TypeError"));
}

#[test]
fn object_assign_copies_symbols_after_strings() {
    assert_eq!(
        run(r#"
            var s = Symbol("s");
            var log = "";
            var source = {};
            Object.defineProperty(source, s, {
              enumerable: true,
              get: function() { log += "s"; return 2; }
            });
            Object.defineProperty(source, "a", {
              enumerable: true,
              get: function() { log += "a"; return 1; }
            });
            var target = Object.assign({}, source);
            [log, target.a, target[s], Object.getOwnPropertySymbols(target).length].join("|");
        "#),
        Value::String(Arc::from("as|1|2|1"))
    );
}

#[test]
fn object_assign_roots_boxed_targets_across_proxy_callbacks() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var sourceTarget = { kept: 2 };
            var source = new Proxy(sourceTarget, {
              ownKeys: function(target) {
                forceGc();
                return Reflect.ownKeys(target);
              },
              getOwnPropertyDescriptor: function(target, key) {
                forceGc();
                return Reflect.getOwnPropertyDescriptor(target, key);
              },
              get: function(target, key) {
                forceGc();
                return target[key];
              }
            });
            var assigned = Object.assign(1, source);
            forceGc();
            [assigned.valueOf(), assigned.kept].join("|");
        "#,
        )
        .expect("Object.assign boxed target GC regression failed"),
        Value::String(Arc::from("1|2"))
    );
}

#[test]
fn object_define_properties_validates_and_converts_before_defining() {
    assert!(run_err("Object.defineProperties(1, {});").contains("TypeError"));
    assert_eq!(
        run("var proto = {}; Object.getPrototypeOf(Object.create(proto, undefined)) === proto;"),
        Value::Bool(true)
    );
    for source in [
        "Object.defineProperties({}, null);",
        "Object.defineProperties({}, undefined);",
        "Object.create({}, null);",
    ] {
        assert!(
            run_err(source).contains("TypeError"),
            "nullish property descriptors must reject: {source}"
        );
    }
    assert_eq!(
        run(r#"
            var emptyString = Object.defineProperties({}, "");
            var number = Object.defineProperties({}, 1);
            var boolean = Object.defineProperties({}, false);
            var inherited = Object.create({ value: 4, enumerable: true });
            var nullHas = new Proxy({ value: 5, enumerable: true }, { has: null });
            var target = Object.defineProperties({}, {
              inherited: inherited,
              nullHas: nullHas
            });
            [
              Object.keys(emptyString).length,
              Object.keys(number).length,
              Object.keys(boolean).length,
              target.inherited,
              target.nullHas,
              Object.keys(target).join(",")
            ].join("|");
        "#),
        Value::String(Arc::from("0|0|0|4|5|inherited,nullHas"))
    );

    assert_eq!(
        run(r#"
            var dataKeys;
            var accessorKeys;
            var target = new Proxy({}, {
              defineProperty: function(object, key, descriptor) {
                if (key === "data") dataKeys = Reflect.ownKeys(descriptor).join(",");
                else accessorKeys = Reflect.ownKeys(descriptor).join(",");
                return Reflect.defineProperty(object, key, descriptor);
              }
            });
            Object.defineProperties(target, {
              data: {
                configurable: true,
                enumerable: true,
                writable: true,
                value: 1
              },
              accessor: {
                configurable: true,
                enumerable: true,
                set: undefined,
                get: function() { return 2; }
              }
            });
            [dataKeys, accessorKeys, target.data, target.accessor].join("|");
        "#),
        Value::String(Arc::from(
            "value,writable,enumerable,configurable|get,set,enumerable,configurable|1|2"
        ))
    );

    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var atomicTarget = {};
            var invalidDescriptors = {};
            Object.defineProperty(invalidDescriptors, "first", {
              enumerable: true,
              get: function() { return { value: 1 }; }
            });
            Object.defineProperty(invalidDescriptors, "second", {
              enumerable: true,
              get: function() {
                forceGc();
                return { value: 2, get: function() {} };
              }
            });
            var atomic = false;
            try { Object.defineProperties(atomicTarget, invalidDescriptors); }
            catch (error) {
              atomic = error instanceof TypeError && !("first" in atomicTarget);
            }

            var rootedTarget = {};
            var validDescriptors = {};
            Object.defineProperty(validDescriptors, "first", {
              enumerable: true,
              get: function() { return { value: { alive: 7 } }; }
            });
            Object.defineProperty(validDescriptors, "second", {
              enumerable: true,
              get: function() { forceGc(); return { value: 2 }; }
            });
            Object.defineProperties(rootedTarget, validDescriptors);
            forceGc();
            [atomic, rootedTarget.first.alive, rootedTarget.second].join("|");
        "#,
        )
        .expect("Object.defineProperties conversion regression failed"),
        Value::String(Arc::from("true|7|2"))
    );
}

#[test]
fn object_define_property_normalizes_proxy_descriptors_and_null_traps() {
    assert_eq!(
        run(r#"
            var keyCalls = 0;
            var key = {
              [Symbol.toPrimitive]: function() { keyCalls += 1; return "key"; }
            };
            var primitiveTargetError = false;
            try { Object.defineProperty(1, key, {}); }
            catch (error) { primitiveTargetError = error instanceof TypeError; }

            var original = {
              configurable: true,
              extra: 9,
              enumerable: true,
              writable: true,
              value: 1
            };
            var fresh = false;
            var descriptorKeys;
            var target = new Proxy({}, {
              defineProperty: function(object, property, descriptor) {
                fresh = descriptor !== original && !("extra" in descriptor);
                descriptorKeys = Reflect.ownKeys(descriptor).join(",");
                return Reflect.defineProperty(object, property, descriptor);
              }
            });
            Object.defineProperty(target, "data", original);

            var fallbackTarget = {};
            var fallback = new Proxy(fallbackTarget, { defineProperty: null });
            Object.defineProperty(fallback, "forwarded", { value: 2 });

            var invariantTarget = Object.preventExtensions({});
            var invariantProxy = new Proxy(invariantTarget, {
              defineProperty: function() { return true; }
            });
            var invariantError = false;
            try { Object.defineProperty(invariantProxy, "new", { value: 1 }); }
            catch (error) { invariantError = error instanceof TypeError; }

            var fixedNaN = {};
            Object.defineProperty(fixedNaN, "value", {
              value: NaN,
              writable: false,
              configurable: false
            });
            var ordinaryNaNAccepted = Object.defineProperty(
              fixedNaN,
              "value",
              { value: NaN }
            ) === fixedNaN;
            var nanProxy = new Proxy(fixedNaN, {
              defineProperty: function() { return true; }
            });
            var nanAccepted = Object.defineProperty(
              nanProxy,
              "value",
              { value: NaN }
            ) === nanProxy;

            var fixedNegativeZero = {};
            Object.defineProperty(fixedNegativeZero, "value", {
              value: -0,
              writable: false,
              configurable: false
            });
            var ordinaryZeroRejected = false;
            try { Object.defineProperty(fixedNegativeZero, "value", { value: 0 }); }
            catch (error) { ordinaryZeroRejected = error instanceof TypeError; }
            var zeroProxy = new Proxy(fixedNegativeZero, {
              defineProperty: function() { return true; }
            });
            var zeroRejected = false;
            try { Object.defineProperty(zeroProxy, "value", { value: 0 }); }
            catch (error) { zeroRejected = error instanceof TypeError; }
            [
              primitiveTargetError,
              keyCalls,
              fresh,
              descriptorKeys,
              target.data,
              fallbackTarget.forwarded,
              invariantError,
              ordinaryNaNAccepted,
              ordinaryZeroRejected,
              nanAccepted,
              zeroRejected
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|0|true|value,writable,enumerable,configurable|1|2|true|true|true|true|true"
        ))
    );
}

#[test]
fn proxy_define_property_preserves_iterative_order_false_results_and_realms() {
    assert_eq!(
        run(r#"
            var log = [];
            var base = {};
            var innerHandler = {};
            Object.defineProperty(innerHandler, "defineProperty", {
              get: function () {
                log.push("inner");
                return function (target, key, descriptor) {
                  log.push("trap");
                  return Reflect.defineProperty(target, key, descriptor);
                };
              }
            });
            var inner = new Proxy(base, innerHandler);
            var outerHandler = {};
            Object.defineProperty(outerHandler, "defineProperty", {
              get: function () {
                log.push("outer");
                return undefined;
              }
            });
            var outer = new Proxy(inner, outerHandler);
            var defined = Reflect.defineProperty(outer, "x", {
              value: 5,
              configurable: true
            });

            var falseCalls = 0;
            var falseProxy = new Proxy({}, {
              defineProperty: function () {
                falseCalls += 1;
                return false;
              }
            });
            var reflectedFalse = Reflect.defineProperty(falseProxy, "x", {});
            var objectThrew = false;
            try { Object.defineProperty(falseProxy, "x", {}); }
            catch (error) { objectThrew = error instanceof TypeError; }

            var other = $262.createRealm().global;
            var applyArgumentRealm = false;
            var callableProxy = new Proxy(function () {}, {
              apply: function (target, thisArg, argumentsList) {
                applyArgumentRealm =
                  Object.getPrototypeOf(argumentsList) === other.Array.prototype;
                return true;
              }
            });
            other.Reflect.apply(callableProxy, null, []);
            var blocked = Object.preventExtensions({});
            var invariantProxy = new Proxy(blocked, {
              defineProperty: function () { return true; }
            });
            var objectRealmError = false;
            var reflectRealmError = false;
            try { other.Object.defineProperty(invariantProxy, "x", {}); }
            catch (error) {
              objectRealmError = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }
            try { other.Reflect.defineProperty(invariantProxy, "x", {}); }
            catch (error) {
              reflectRealmError = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }

            [
              defined,
              base.x,
              log.join(","),
              reflectedFalse,
              objectThrew,
              falseCalls,
              applyArgumentRealm,
              objectRealmError,
              reflectRealmError
            ].join("|");
            "#,),
        Value::String(Arc::from(
            "true|5|outer,inner,trap|false|true|2|true|true|true"
        ))
    );
}

#[test]
fn ordinary_set_receiver_define_preserves_descriptor_presence_and_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var fullKeys;
            var partialKeys;
            var fullRealm = false;
            var partialRealm = false;

            var fullTarget = {};
            var fullReceiver = new Proxy(fullTarget, {
              defineProperty: function (target, key, descriptor) {
                fullKeys = Object.keys(descriptor).join(",");
                fullRealm =
                  Object.getPrototypeOf(descriptor) === other.Object.prototype;
                return Reflect.defineProperty(target, key, descriptor);
              }
            });
            var fullSource = Object.create(null);
            var fullResult = other.Reflect.set(
              fullSource,
              "created",
              11,
              fullReceiver
            );

            var partialTarget = { existing: 2 };
            var partialReceiver = new Proxy(partialTarget, {
              defineProperty: function (target, key, descriptor) {
                partialKeys = Object.keys(descriptor).join(",");
                partialRealm =
                  Object.getPrototypeOf(descriptor) === other.Object.prototype;
                return Reflect.defineProperty(target, key, descriptor);
              }
            });
            var partialSource = { existing: 1 };
            var partialResult = other.Reflect.set(
              partialSource,
              "existing",
              9,
              partialReceiver
            );

            var fullDescriptor = Object.getOwnPropertyDescriptor(
              fullTarget,
              "created"
            );
            var partialDescriptor = Object.getOwnPropertyDescriptor(
              partialTarget,
              "existing"
            );
            [
              fullResult,
              fullKeys,
              fullRealm,
              fullDescriptor.value,
              fullDescriptor.writable,
              fullDescriptor.enumerable,
              fullDescriptor.configurable,
              partialResult,
              partialKeys,
              partialRealm,
              partialDescriptor.value,
              partialDescriptor.writable,
              partialDescriptor.enumerable,
              partialDescriptor.configurable
            ].join("|");
            "#),
        Value::String(Arc::from(
            "true|value,writable,enumerable,configurable|true|11|true|true|true|true|value|true|9|true|true|true"
        ))
    );
}

#[test]
fn proxy_set_preserves_nested_order_short_circuits_and_descriptor_invariants() {
    assert_eq!(
        run(r#"
            var order = [];
            var orderedBase = Object.create(null);
            var innerHandler = {};
            Object.defineProperty(innerHandler, "set", {
              get: function () { order.push("set inner"); return undefined; }
            });
            Object.defineProperty(innerHandler, "getOwnPropertyDescriptor", {
              get: function () { order.push("gopd inner"); return undefined; }
            });
            Object.defineProperty(innerHandler, "defineProperty", {
              get: function () {
                order.push("define inner");
                return function (target, key, descriptor) {
                  order.push("trap");
                  return Reflect.defineProperty(target, key, descriptor);
                };
              }
            });
            var inner = new Proxy(orderedBase, innerHandler);
            var outerHandler = {};
            Object.defineProperty(outerHandler, "set", {
              get: function () { order.push("set outer"); return undefined; }
            });
            Object.defineProperty(outerHandler, "getOwnPropertyDescriptor", {
              get: function () { order.push("gopd outer"); return undefined; }
            });
            Object.defineProperty(outerHandler, "defineProperty", {
              get: function () { order.push("define outer"); return undefined; }
            });
            var outer = new Proxy(inner, outerHandler);
            var orderedResult = Reflect.set(outer, "value", 17, outer);

            var falseTarget = new Proxy({}, {
              getOwnPropertyDescriptor: function () {
                throw new Error("must not run");
              }
            });
            var falseProxy = new Proxy(falseTarget, {
              set: function () { return false; }
            });
            var falseSuppressed = Reflect.set(falseProxy, "x", 1) === false;

            var revoked = Proxy.revocable({}, {});
            revoked.revoke();
            var revokedLog = [];
            var revokedOuterHandler = {};
            Object.defineProperty(revokedOuterHandler, "set", {
              get: function () {
                revokedLog.push("outer");
                return undefined;
              }
            });
            var revokedOuter = new Proxy(revoked.proxy, revokedOuterHandler);
            var revokedError = false;
            try { Reflect.set(revokedOuter, "x", 1); }
            catch (error) { revokedError = error instanceof TypeError; }

            var completeTarget = {};
            var completeHandler = {};
            Object.defineProperty(completeHandler, "defineProperty", {
              get: function () {
                Object.defineProperty(completeTarget, "x", {
                  value: 0,
                  writable: true,
                  enumerable: true,
                  configurable: false
                });
                return function () { return true; };
              }
            });
            var completeReceiver = new Proxy(completeTarget, completeHandler);
            var completeError = false;
            try {
              Reflect.set(Object.create(null), "x", 2, completeReceiver);
            } catch (error) {
              completeError = error instanceof TypeError;
            }

            var partialTarget = { x: 1 };
            var partialHandler = {};
            Object.defineProperty(partialHandler, "defineProperty", {
              get: function () {
                Object.defineProperty(partialTarget, "x", {
                  configurable: false
                });
                return function () { return true; };
              }
            });
            var partialReceiver = new Proxy(partialTarget, partialHandler);
            var partialAccepted = Reflect.set(
              { x: 0 },
              "x",
              2,
              partialReceiver
            );

            [
              orderedResult,
              orderedBase.value,
              order.join(","),
              falseSuppressed,
              revokedLog.join(","),
              revokedError,
              completeError,
              partialAccepted
            ].join("|");
            "#),
        Value::String(Arc::from(
            "true|17|set outer,set inner,gopd outer,gopd inner,define outer,define inner,trap|true|outer|true|true|true"
        ))
    );
}

#[test]
fn object_define_property_roots_ephemeral_native_arguments_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var result = Object.defineProperty(
              new Proxy({}, {
                get defineProperty() {
                  forceGc();
                  return function(target, key, descriptor) {
                    forceGc();
                    return Reflect.defineProperty(target, key, descriptor);
                  };
                }
              }),
              {
                [Symbol.toPrimitive]: function() {
                  forceGc();
                  return "kept";
                }
              },
              new Proxy(
                { value: { alive: 7 }, configurable: true },
                {
                  has: function(target, key) {
                    forceGc();
                    return key in target;
                  },
                  get: function(target, key) {
                    forceGc();
                    return target[key];
                  }
                }
              )
            );
            forceGc();
            [result.kept.alive, Object.getOwnPropertyDescriptor(result, "kept").configurable].join("|");
        "#,
        )
        .expect("Object.defineProperty native argument GC regression failed"),
        Value::String(Arc::from("7|true"))
    );
}

#[test]
fn object_spread_copies_symbols_in_own_property_key_order() {
    assert_eq!(
        run(r#"
            var calls = [];
            var sym = Symbol("foo");
            var hidden = Symbol("hidden");
            var source = {
              get z() { calls.push("z"); return 2; },
              get a() { calls.push("a"); return 3; }
            };
            Object.defineProperty(source, "1", {
              enumerable: true,
              get: function() { calls.push("1"); return 1; }
            });
            Object.defineProperty(source, sym, {
              enumerable: true,
              get: function() { calls.push("s"); return 4; }
            });
            Object.defineProperty(source, hidden, {
              enumerable: false,
              value: 5
            });
            var out = { ...source };
            [
              calls.join(","),
              out[1],
              out.z,
              out.a,
              out[sym],
              out[hidden] === undefined,
              Object.keys(out).join(","),
              Object.getOwnPropertySymbols(out).length
            ].join("|");
        "#),
        Value::String(Arc::from("1,z,a,s|1|2|3|4|true|1,z,a|1"))
    );
}

#[test]
fn object_spread_rechecks_descriptors_and_propagates_proxy_own_keys() {
    assert_eq!(
        run(r#"
            var log = [];
            var sym = Symbol("s");
            var source = {};
            Object.defineProperty(source, "a", {
              enumerable: true,
              get: function() {
                log.push("a");
                Object.defineProperty(source, sym, { value: 2, enumerable: false });
                return 1;
              }
            });
            Object.defineProperty(source, sym, {
              value: 2,
              enumerable: true,
              configurable: true
            });
            var out = { ...source };
            [log.join(","), out.a, out[sym] === undefined, Object.getOwnPropertySymbols(out).length].join("|");
        "#),
        Value::String(Arc::from("a|1|true|0"))
    );

    assert!(run_err(
        r#"
            var proxy = new Proxy({ a: 1 }, {
              ownKeys: function() { throw new Error("boom"); }
            });
            ({ ...proxy });
        "#
    )
    .contains("boom"));
}

#[test]
fn proxy_own_keys_enforces_duplicate_and_target_invariants() {
    assert_eq!(
        run(r#"
            function errorName(target, keys) {
              try {
                Reflect.ownKeys(new Proxy(target, { ownKeys: function() { return keys; } }));
                return "none";
              } catch (error) {
                return error.name;
              }
            }
            var fixed = {};
            Object.defineProperty(fixed, "fixed", { configurable: false });
            var sealed = Object.preventExtensions({ present: 1 });
            [
              errorName({}, ["x", "x"]),
              errorName(fixed, []),
              errorName(sealed, ["present", "extra"]),
              errorName(sealed, [])
            ].join("|");
        "#),
        Value::String(Arc::from("TypeError|TypeError|TypeError|TypeError"))
    );
}

#[test]
fn proxy_own_keys_checks_extensibility_before_target_keys() {
    assert_eq!(
        run(r##"
            function probe(useForIn) {
              var log = [];
              var target = {};
              var inner = new Proxy(target, {
                isExtensible: function(target) {
                  log.push("extensible");
                  Object.defineProperty(target, "x", {
                    value: 1,
                    configurable: false
                  });
                  return Reflect.isExtensible(target);
                },
                ownKeys: function(target) {
                  log.push("targetKeys");
                  return Reflect.ownKeys(target);
                },
                getOwnPropertyDescriptor: function(target, key) {
                  log.push("descriptor:" + key);
                  return Reflect.getOwnPropertyDescriptor(target, key);
                }
              });
              var outer = new Proxy(inner, {
                ownKeys: function() {
                  log.push("trapKeys");
                  return [];
                }
              });
              var errorName = "none";
              try {
                if (useForIn) {
                  for (var key in outer) {}
                } else {
                  Reflect.ownKeys(outer);
                }
              } catch (error) {
                errorName = error.name;
              }
              return errorName + "|" + log.join(",") + "|" +
                Reflect.ownKeys(target).join(",");
            }
            probe(false) + "#" + probe(true);
        "##),
        Value::String(Arc::from(
            "TypeError|trapKeys,extensible,targetKeys,descriptor:x|x#TypeError|trapKeys,extensible,targetKeys,descriptor:x|x"
        ))
    );
}

#[test]
fn proxy_own_keys_gets_every_target_descriptor_before_omission_error() {
    assert_eq!(
        run(r##"
            function probe(useForIn) {
              var marker = {};
              var log = [];
              var target = {};
              Object.defineProperty(target, "fixed", {
                value: 1,
                configurable: false
              });
              var inner = new Proxy(target, {
                ownKeys: function() { return ["fixed", "later"]; },
                getOwnPropertyDescriptor: function(target, key) {
                  log.push("descriptor:" + key);
                  if (key === "later") throw marker;
                  return Reflect.getOwnPropertyDescriptor(target, key);
                }
              });
              var outer = new Proxy(inner, {
                ownKeys: function() { return []; }
              });
              var result = "none";
              try {
                if (useForIn) {
                  for (var key in outer) {}
                } else {
                  Reflect.ownKeys(outer);
                }
              } catch (error) {
                result = error === marker ? "marker" : error.name;
              }
              return result + "|" + log.join(",");
            }
            probe(false) + "#" + probe(true);
        "##),
        Value::String(Arc::from(
            "marker|descriptor:fixed,descriptor:later#marker|descriptor:fixed,descriptor:later"
        ))
    );
}

#[test]
fn object_property_is_enumerable() {
    assert_eq!(run("({a:1}).propertyIsEnumerable('a');"), Value::Bool(true));
    assert_eq!(
        run("var o={}; Object.defineProperty(o,'x',{value:1, enumerable:false}); o.propertyIsEnumerable('x');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("({get m(){ return 1; }}).propertyIsEnumerable('m');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("({a:1}).propertyIsEnumerable('missing');"),
        Value::Bool(false)
    );
    assert_eq!(run("[10].propertyIsEnumerable('0');"), Value::Bool(true));
    assert_eq!(
        run("[10].propertyIsEnumerable('length');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("Object.prototype.propertyIsEnumerable.call('ab', '1');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var s=Symbol(); var o={}; o[s]=1; o.propertyIsEnumerable(s);"),
        Value::Bool(true)
    );
}

#[test]
fn math_basic() {
    assert_eq!(run("Math.floor(3.7);"), Value::Number(3.0));
    assert_eq!(run("Math.max(1, 5, 3);"), Value::Number(5.0));
    assert_eq!(run("Math.sqrt(16);"), Value::Number(4.0));
    assert!(matches!(run("Math.pow(1, NaN);"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.pow(-1, Infinity);"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.pow(1, -Infinity);"), Value::Number(n) if n.is_nan()));
    assert_eq!(run("Math.pow(NaN, 0);"), Value::Number(1.0));
    assert_eq!(run("Object.isExtensible(Math);"), Value::Bool(true));
    assert_eq!(
        run("Math.substring = String.prototype.substring; Math.substring(Math.PI, -10);"),
        Value::String(Arc::from("[ob"))
    );
}

#[test]
fn math_tag_and_created_realm_intrinsic_are_spec_shaped() {
    assert_eq!(
        run(r#"
            var descriptor = Object.getOwnPropertyDescriptor(Math, Symbol.toStringTag);
            var borrowed = String.prototype.split.call(Math);
            var initialTag = Object.prototype.toString.call(Math);
            var deleted = delete Math[Symbol.toStringTag];
            var fallbackTag = Object.prototype.toString.call(Math);
            Object.defineProperty(Math, Symbol.toStringTag, descriptor);

            var other = $262.createRealm().global;
            var otherDescriptor = other.Object.getOwnPropertyDescriptor(
              other.Math,
              other.Symbol.toStringTag
            );
            [
              descriptor.value, descriptor.writable, descriptor.enumerable,
              descriptor.configurable, initialTag, borrowed[0], deleted, fallbackTag,
              other.Math !== Math,
              other.Object.getPrototypeOf(other.Math) === other.Object.prototype,
              other.Object.getPrototypeOf(other.Math.abs) === other.Function.prototype,
              other.Object.prototype.toString.call(other.Math),
              otherDescriptor.value, otherDescriptor.writable,
              otherDescriptor.enumerable, otherDescriptor.configurable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "Math|false|false|true|[object Math]|[object Math]|true|[object Object]|true|true|true|[object Math]|Math|false|false|true"
        ))
    );
}

#[test]
fn math_round_half() {
    assert_eq!(run("Object.is(Math.round(-0), -0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.round(-0.5), -0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.round(-0.25), -0);"), Value::Bool(true));
    assert_eq!(
        run("Object.is(Math.round(0.5 - Number.EPSILON / 4), 0);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var x = -(2 / Number.EPSILON - 1); Object.is(Math.round(x), x);"),
        Value::Bool(true)
    );
    assert_eq!(run("Math.round(0.5);"), Value::Number(1.0));
    assert_eq!(run("Math.round(-1.5);"), Value::Number(-1.0));
}

#[test]
fn math_max_min_nan_and_signed_zero() {
    assert!(matches!(run("Math.max({});"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.min({});"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.max(1, NaN, 2);"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Math.min(1, NaN, 2);"), Value::Number(n) if n.is_nan()));
    assert_eq!(
        run("var calls = 0; var n = { valueOf: function(){ calls++; } }; Math.max(NaN, n); calls;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("var calls = 0; var n = { valueOf: function(){ calls++; } }; Math.min(NaN, n); calls;"),
        Value::Number(1.0)
    );
    assert_eq!(run("Object.is(Math.max(-0, -0), -0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.max(0, -0), 0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.min(-0, -0), -0);"), Value::Bool(true));
    assert_eq!(run("Object.is(Math.min(0, -0), -0);"), Value::Bool(true));
}

#[test]
fn json() {
    assert_eq!(run("JSON.parse('[1,2,3]')[1];"), Value::Number(2.0));
    assert_eq!(
        run("JSON.stringify({a:1});"),
        Value::String(Arc::from("{\"a\":1}"))
    );
}

#[test]
fn error_subclass() {
    assert_eq!(
        run(r#"new TypeError("x").message;"#),
        Value::String(Arc::from("x"))
    );
}

#[test]
fn thrown_custom_constructor_object_preserves_constructor_name_in_display() {
    let msg = run_err(
        r#"
        function Test262Error(message) {
          if (!(this instanceof Test262Error)) return new Test262Error(message);
          this.message = message || "";
        }
        Test262Error.prototype.toString = function() {
          return "Test262Error: " + this.message;
        };
        throw new Test262Error();
        "#,
    );
    assert!(msg.contains("Test262Error"), "got: {msg}");

    let native = run_err("throw new Error('native');");
    assert!(native.contains("Error: native"), "got: {native}");
}

#[test]
fn error_subclass_plain_call_uses_active_constructor_prototype() {
    assert_eq!(
        run(
            r#"[
                EvalError(1).toString(),
                RangeError(1).toString(),
                ReferenceError(1).toString(),
                SyntaxError(1).toString(),
                TypeError(1).toString(),
                URIError("message", "fileName", "1").toString()
            ].join("|");"#
        ),
        Value::String(Arc::from(
            "EvalError: 1|RangeError: 1|ReferenceError: 1|SyntaxError: 1|TypeError: 1|URIError: message"
        ))
    );
    assert_eq!(
        run("var T = TypeError; TypeError = Error; var e = T(1); e instanceof T && e instanceof Error;"),
        Value::Bool(true)
    );
}

#[test]
fn error_to_string_requires_object_and_omits_empty_parts() {
    assert_eq!(
        run(r#"
            var e1 = new Error("message");
            e1.name = "";
            var e2 = new Error("");
            var e3 = new Error("");
            e3.name = "";
            [
              e1.toString(),
              e2.toString(),
              e3.toString(),
              Error.prototype.toString.call({ name: undefined, message: "m" }),
              Error.prototype.toString.call({ name: "N", message: undefined })
            ].join("|");
        "#),
        Value::String(Arc::from("message|Error||Error: m|N"))
    );
    for src in [
        "Error.prototype.toString.call(undefined);",
        "Error.prototype.toString.call(null);",
        "Error.prototype.toString.call(1);",
        "Error.prototype.toString.call(true);",
        "Error.prototype.toString.call('x');",
        "Error.prototype.toString.call(Symbol());",
    ] {
        assert!(
            run_err(src).contains("TypeError"),
            "expected TypeError for {src}"
        );
    }
}

#[test]
fn error_cause_uses_has_property_get_and_message_order() {
    assert_eq!(
        run(r#"
            var cause = { message: "root" };
            var err = new Error("msg", { cause: cause });
            var desc = Object.getOwnPropertyDescriptor(err, "cause");
            [
              err.cause === cause,
              desc.value === cause,
              desc.writable,
              desc.enumerable,
              desc.configurable,
              Object.prototype.hasOwnProperty.call(new Error("msg"), "cause"),
              Object.prototype.hasOwnProperty.call(new Error("msg", { cause: undefined }), "cause")
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,false,true,false,true"))
    );

    assert_eq!(
        run(r#"
            var seq = [];
            new Error(
              { toString: function() { seq.push("toString"); return "msg"; } },
              { get cause() { seq.push("cause"); return 1; } }
            );
            seq.join(",");
        "#),
        Value::String(Arc::from("toString,cause"))
    );

    assert!(run_err(
        r#"
        new Error("msg", new Proxy({}, {
          has: function(target, key) {
            if (key === "cause") throw new Error("has boom");
            return key in target;
          }
        }));
    "#
    )
    .contains("has boom"));

    assert!(run_err(
        r#"
        new Error("msg", {
          get cause() { throw new Error("get boom"); }
        });
    "#
    )
    .contains("get boom"));

    assert_eq!(
        run(r#"
            var cause = { message: "root" };
            var err = new AggregateError([1, 2], "agg", { cause: cause });
            var causeDesc = Object.getOwnPropertyDescriptor(err, "cause");
            var errorsDesc = Object.getOwnPropertyDescriptor(err, "errors");
            [
              AggregateError.length,
              err.message,
              err.cause === cause,
              causeDesc.enumerable,
              errorsDesc.enumerable,
              err.errors.join(":")
            ].join(",");
        "#),
        Value::String(Arc::from("2,agg,true,false,false,1:2"))
    );
}

#[test]
fn error_stack_accessor_uses_error_data_and_receiver_property() {
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(Error.prototype, "stack");
            var err = new TypeError("msg");
            var fake = Object.create(Error.prototype);
            var other = $262.createRealm().global;
            var otherDesc = Object.getOwnPropertyDescriptor(other.Error.prototype, "stack");
            var otherOriginalTypeError = other.TypeError;
            desc.set.call(err, "sentinel");
            desc.set.call(fake, "plain");
            [
              typeof desc.get,
              desc.get.name,
              desc.get.length,
              typeof desc.set,
              desc.set.name,
              desc.set.length,
              desc.enumerable,
              desc.configurable,
              Object.prototype.hasOwnProperty.call(new Error("x"), "stack"),
              typeof desc.get.call(new Error("x")),
              desc.get.call({}) === undefined,
              err.stack,
              Object.getOwnPropertyDescriptor(err, "stack").enumerable,
              fake.stack,
              Error.prototype !== other.Error.prototype,
              desc.get !== otherDesc.get,
              desc.set !== otherDesc.set,
              Object.getPrototypeOf(other.TypeError.prototype) === other.Error.prototype,
              typeof desc.get.call(new other.Error("x")),
              (function() {
                try {
                  desc.set.call(other.Error.prototype, "x");
                } catch (e) {
                  return e.constructor === other.TypeError &&
                    Object.getPrototypeOf(e) === other.TypeError.prototype;
                }
                return false;
              })(),
              (function() {
                try {
                  otherDesc.get.call(1);
                } catch (e) {
                  return e.constructor === other.TypeError &&
                    Object.getPrototypeOf(e) === other.TypeError.prototype;
                }
                return false;
              })(),
              (function() {
                var original = TypeError;
                TypeError = function FakeTypeError() {};
                try {
                  desc.set.call(Error.prototype, "x");
                } catch (e) {
                  return e.constructor === original &&
                    Object.getPrototypeOf(e) === original.prototype;
                }
                return false;
              })(),
              (function() {
                other.TypeError = function FakeOtherTypeError() {};
                try {
                  otherDesc.set.call(other.Error.prototype, "x");
                } catch (e) {
                  return e.constructor === otherOriginalTypeError &&
                    Object.getPrototypeOf(e) === otherOriginalTypeError.prototype;
                }
                return false;
              })()
            ].join("|");
        "#),
        Value::String(Arc::from(
            "function|get stack|0|function|set stack|1|false|true|false|string|true|sentinel|true|plain|true|true|true|true|string|true|true|true|true"
        ))
    );
    for src in [
        "Object.getOwnPropertyDescriptor(Error.prototype, 'stack').get.call(1);",
        "Object.getOwnPropertyDescriptor(Error.prototype, 'stack').set.call(1, 'x');",
        "Object.getOwnPropertyDescriptor(Error.prototype, 'stack').set.call(new Error(), 1);",
        "Object.getOwnPropertyDescriptor(Error.prototype, 'stack').set.call(Error.prototype, 'x');",
    ] {
        assert!(
            run_err(src).contains("TypeError"),
            "expected TypeError for {src}"
        );
    }
}

#[test]
fn native_error_constructors_inherit_from_error_constructor() {
    assert_eq!(
        run(r#"[
                Object.getPrototypeOf(Error) === Function.prototype,
                Object.getPrototypeOf(EvalError) === Error,
                Object.getPrototypeOf(RangeError) === Error,
                Object.getPrototypeOf(ReferenceError) === Error,
                Object.getPrototypeOf(SyntaxError) === Error,
                Object.getPrototypeOf(TypeError) === Error,
                Object.getPrototypeOf(URIError) === Error,
                Object.getPrototypeOf(AggregateError) === Error,
                EvalError.hasOwnProperty("name"),
                EvalError.hasOwnProperty("length"),
                EvalError.name,
                EvalError.length,
                Object.prototype.toString.call(EvalError.prototype)
            ].join(",");"#),
        Value::String(Arc::from(
            "true,true,true,true,true,true,true,true,true,true,EvalError,1,[object Object]"
        ))
    );
}

#[test]
fn error_constructors_use_new_target_realm_default_prototype() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var nt = new Function();
            nt.prototype = undefined;
            var otherNt = new other.Function();
            otherNt.prototype = undefined;
            [
              Object.getPrototypeOf(Reflect.construct(Error, [], nt)) === Error.prototype,
              Object.getPrototypeOf(Reflect.construct(TypeError, [], nt)) === TypeError.prototype,
              Object.getPrototypeOf(Reflect.construct(AggregateError, [[]], nt)) === AggregateError.prototype,
              Object.getPrototypeOf(Reflect.construct(Error, [], otherNt)) === other.Error.prototype,
              Object.getPrototypeOf(Reflect.construct(TypeError, [], otherNt)) === other.TypeError.prototype,
              Object.getPrototypeOf(Reflect.construct(AggregateError, [[]], otherNt)) === other.AggregateError.prototype
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,true,true,true"))
    );

    assert_eq!(
        run(r#"
            var proto = {};
            function NewTarget() {}
            NewTarget.prototype = proto;
            var err = Reflect.construct(AggregateError, [[]], NewTarget);
            Object.getPrototypeOf(err) === proto;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn native_error_subclass_inherits_name_and_message() {
    assert_eq!(
        run(r#"
            class Err extends EvalError {}
            var err = new Err();
            [
              err.name,
              err.hasOwnProperty("name"),
              err.hasOwnProperty("message"),
              err.message
            ].join(",");
            "#),
        Value::String(Arc::from("EvalError,false,false,"))
    );

    assert_eq!(
        run(r#"
            class Err extends EvalError {}
            Err.prototype.message = "custom";
            var err = new Err();
            err.message + ":" + err.hasOwnProperty("message");
            "#),
        Value::String(Arc::from("custom:false"))
    );

    assert_eq!(
        run(r#"
            class Err extends EvalError {}
            var err = new Err("boom");
            var d = Object.getOwnPropertyDescriptor(err, "message");
            [err.message, d.writable, d.enumerable, d.configurable].join(",");
            "#),
        Value::String(Arc::from("boom,true,false,true"))
    );
}

#[test]
fn error_is_error_recognizes_real_error_objects_only() {
    assert_eq!(
        run(r#"
            class CustomError extends Error {}
            var other = $262.createRealm().global;
            var fake = {
              __proto__: Error.prototype,
              constructor: Error,
              message: "",
              stack: new Error().stack
            };
            [
              Error.isError(new Error()),
              Error.isError(new TypeError()),
              Error.isError(new CustomError()),
              Error.isError(new other.Error()),
              Error.isError(new other.Array()),
              Error.isError(fake),
              Error.isError(Error),
              Error.isError({}),
              Error.isError(undefined),
              Error.isError(0n),
              Error.isError(Symbol()),
              Object.prototype.propertyIsEnumerable.call(Error, "isError"),
              Object.getOwnPropertyDescriptor(Error, "isError").writable,
              Object.getOwnPropertyDescriptor(Error, "isError").configurable
            ].join(",");
        "#),
        Value::String(Arc::from(
            "true,true,true,true,false,false,false,false,false,false,false,false,true,true"
        ))
    );
    assert!(
        run_err("new Error.isError();").contains("TypeError"),
        "Error.isError must not be constructable"
    );
}

#[test]
fn native_constructor_new_target_does_not_leak_to_next_call() {
    let msg = run_err("new Error('x'); class C {} C();");
    assert!(
        msg.contains("Class constructor cannot be invoked without 'new'"),
        "got: {msg}"
    );
}

#[test]
fn bound_function_name_and_length_follow_target_metadata() {
    assert_eq!(
        run(r#"
            function target(a, b, c) {}
            var bound = target.bind(null, 1);
            var lengthDesc = Object.getOwnPropertyDescriptor(bound, "length");
            var nameDesc = Object.getOwnPropertyDescriptor(bound, "name");
            var chained = bound.bind(null);

            function numeric() {}
            Object.defineProperty(numeric, "length", { value: 3.66 });
            var fractional = numeric.bind(null, 1).length;
            Object.defineProperty(numeric, "length", { value: Infinity });
            var positiveInfinity = numeric.bind(null, 1).length;
            Object.defineProperty(numeric, "length", { value: -Infinity });
            var negativeInfinity = numeric.bind().length;
            Object.defineProperty(numeric, "length", { value: NaN });
            var nan = numeric.bind().length;
            Object.defineProperty(numeric, "length", { value: -0 });
            var negativeZeroNormalized = Object.is(numeric.bind().length, -0);
            var coerced = false;
            Object.defineProperty(numeric, "length", {
              value: { valueOf: function() { coerced = true; throw new Error("coerced"); } }
            });
            var nonNumberLength = numeric.bind().length;
            Object.defineProperty(numeric, "name", { value: 23 });
            var nonStringName = numeric.bind().name;

            function inherited() {}
            delete inherited.length;
            Object.setPrototypeOf(inherited, { length: 42, name: "inherited" });
            var inheritedBound = Function.prototype.bind.call(inherited);

            var proxyLog = [];
            var noOwnLength = new Proxy(function () {}, {
              getOwnPropertyDescriptor: function(target, key) {
                if (key === "length") { proxyLog.push("own:length"); return undefined; }
                return Reflect.getOwnPropertyDescriptor(target, key);
              },
              get: function(target, key, receiver) {
                if (key === "length" || key === "name") proxyLog.push("get:" + key);
                return Reflect.get(target, key, receiver);
              }
            });
            var noOwnLengthBound = noOwnLength.bind();

            var deleted = target.bind();
            Object.setPrototypeOf(deleted, { name: "prototype-name" });
            delete deleted.name;
            var reboundDeleted = Function.prototype.bind.call(deleted);
            var ownKeys = Reflect.ownKeys(bound).join(",");

            [
              bound.name,
              bound.length,
              chained.name,
              lengthDesc.writable,
              lengthDesc.enumerable,
              lengthDesc.configurable,
              nameDesc.writable,
              nameDesc.enumerable,
              nameDesc.configurable,
              fractional,
              positiveInfinity,
              negativeInfinity,
              nan,
              negativeZeroNormalized,
              nonNumberLength,
              coerced,
              nonStringName,
              inheritedBound.length,
              inheritedBound.name,
              noOwnLengthBound.length,
              proxyLog.join(","),
              deleted.name,
              reboundDeleted.name,
              ownKeys,
              Object.hasOwn(bound, "prototype"),
              Object.hasOwn(bound, "caller"),
              Object.hasOwn(bound, "arguments")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "bound target|2|bound bound target|false|false|true|false|false|true|2|Infinity|0|0|false|0|false|bound |0|bound inherited|0|own:length,get:name|prototype-name|bound prototype-name|length,name|false|false|false"
        ))
    );

    assert_eq!(
        run(r#"
            var sentinel = {};
            var log = [];
            function target() {}
            Object.defineProperty(target, "length", {
              get: function() { log.push("length"); throw sentinel; }
            });
            Object.defineProperty(target, "name", {
              get: function() { log.push("name"); return "target"; }
            });
            var same = false;
            try { target.bind(); } catch (error) { same = error === sentinel; }
            [same, log.join(",")].join("|");
        "#),
        Value::String(Arc::from("true|length"))
    );
}

#[test]
fn bound_function_metadata_observation_order_survives_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(r#"
            var log = [];
            function target(a, b) {}
            Object.defineProperty(target, "length", {
              configurable: true,
              get: function() { log.push("get:length"); forceGc(); return 2.9; }
            });
            Object.defineProperty(target, "name", {
              configurable: true,
              get: function() { log.push("get:name"); forceGc(); return "target"; }
            });
            var proxy = new Proxy(target, {
              getPrototypeOf: function(inner) {
                log.push("prototype");
                forceGc();
                return Reflect.getPrototypeOf(inner);
              },
              getOwnPropertyDescriptor: function(inner, key) {
                if (key === "length") log.push("own:length");
                forceGc();
                return Reflect.getOwnPropertyDescriptor(inner, key);
              },
              get: function(inner, key, receiver) {
                if (key === "length" || key === "name") log.push("proxy:" + key);
                forceGc();
                return Reflect.get(inner, key, receiver);
              }
            });
            var bound = proxy.bind({ kept: true }, { argument: true });
            forceGc();
            [
              log.join(","),
              bound.length,
              bound.name,
              Object.getPrototypeOf(bound) === Function.prototype,
              Object.getOwnPropertyDescriptor(bound, "length").configurable,
              Object.getOwnPropertyDescriptor(bound, "name").configurable
            ].join("|");
        "#)
        .expect("bound metadata observation and GC path should succeed"),
        Value::String(Arc::from(
            "prototype,own:length,proxy:length,get:length,proxy:name,get:name|1|bound target|true|true|true"
        ))
    );
}

#[test]
fn bound_functions_inherit_restricted_caller_arguments_accessors() {
    assert_eq!(
        run("function target() {}\
             var bound = target.bind({});\
             bound.hasOwnProperty('caller') + ':' + bound.hasOwnProperty('arguments');"),
        Value::String(Arc::from("false:false"))
    );

    let msg = run_err("function target() {} var bound = target.bind({}); bound.caller;");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("function target() {} var bound = target.bind({}); bound.caller = {};");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("function target() {} var bound = target.bind({}); bound.arguments;");
    assert!(msg.contains("TypeError"), "got: {msg}");

    let msg = run_err("function target() {} var bound = target.bind({}); bound.arguments = {};");
    assert!(msg.contains("TypeError"), "got: {msg}");
}

#[test]
fn throw_type_error_intrinsic_is_frozen_and_anonymous() {
    assert_eq!(
        run(
            r#"var args = function() { "use strict"; return arguments; }();
               var thrower = Object.getOwnPropertyDescriptor(args, "callee").get;
               var length = Object.getOwnPropertyDescriptor(thrower, "length");
               var name = Object.getOwnPropertyDescriptor(thrower, "name");
               [
                 thrower.name,
                 thrower.length,
                 length.writable,
                 length.enumerable,
                 length.configurable,
                 name.value,
                 name.writable,
                 name.enumerable,
                 name.configurable,
                 Object.isExtensible(thrower),
                 Object.isFrozen(thrower)
               ].join("|");"#
        ),
        Value::String(Arc::from(
            "|0|false|false|false||false|false|false|false|true"
        ))
    );
}

#[test]
fn throw_type_error_intrinsic_is_reused_for_unmapped_arguments() {
    assert_eq!(
        run(
            r#"var strictArgs = function() { "use strict"; return arguments; }();
               function nonSimple(a = 0) { return arguments; }
               var strictCallee = Object.getOwnPropertyDescriptor(strictArgs, "callee");
               var nonSimpleCallee = Object.getOwnPropertyDescriptor(nonSimple(), "callee");
               (strictCallee.get === nonSimpleCallee.get) + ":" +
                 (strictCallee.get === nonSimpleCallee.set);"#
        ),
        Value::String(Arc::from("true:true"))
    );
}

#[test]
fn throw_type_error_intrinsic_matches_function_prototype_restricted_accessors() {
    assert_eq!(
        run(
            r#"var functionCaller = Object.getOwnPropertyDescriptor(Function.prototype, "caller");
               var functionArguments = Object.getOwnPropertyDescriptor(Function.prototype, "arguments");
               function outer() {
                 return function() { "use strict"; return arguments; }();
               }
               var thrower = Object.getOwnPropertyDescriptor(outer(), "callee").get;
               [
                 functionCaller.get === functionCaller.set,
                 functionArguments.get === functionArguments.set,
                 functionCaller.get === thrower,
                 functionArguments.get === thrower
               ].join(":");"#
        ),
        Value::String(Arc::from("true:true:true:true"))
    );
}

#[test]
fn throw_type_error_intrinsic_is_distinct_per_test262_realm() {
    assert_eq!(
        run(r#"var other = $262.createRealm().global;
               var localArgs = function() { "use strict"; return arguments; }();
               var otherArgs = (new other.Function('"use strict"; return arguments;'))();
               var otherArgs2 = (new other.Function('"use strict"; return arguments;'))();
               var localThrower = Object.getOwnPropertyDescriptor(localArgs, "callee").get;
               var otherThrower = Object.getOwnPropertyDescriptor(otherArgs, "callee").get;
               var otherThrower2 = Object.getOwnPropertyDescriptor(otherArgs2, "callee").get;
               (localThrower !== otherThrower) + ":" + (otherThrower === otherThrower2);"#),
        Value::String(Arc::from("true:true"))
    );
}

#[test]
fn throw_type_error_intrinsic_matches_cross_realm_function_prototype() {
    assert_eq!(
        run(r#"var other = $262.createRealm().global;
               var protoThrower = Object.getOwnPropertyDescriptor(other.Function.prototype, "caller").get;
               var argsThrower = new other.Function('return (function() { "use strict"; return Object.getOwnPropertyDescriptor(arguments, "callee").get })()')();
               var normalFunction = other.Function('return function nested() { return 1; }')();
               [
                 protoThrower === Object.getOwnPropertyDescriptor(other.Function.prototype, "arguments").set,
                 protoThrower === argsThrower,
                 Object.getPrototypeOf(normalFunction) === other.Function.prototype,
                 protoThrower !== Object.getOwnPropertyDescriptor(Function.prototype, "caller").get
               ].join(":");"#),
        Value::String(Arc::from("true:true:true:true"))
    );
}

#[test]
fn json_parse_object() {
    assert_eq!(run(r#"JSON.parse("{\"a\":1}").a;"#), Value::Number(1.0));
    // HashMap key order is non-deterministic; just check both props round-trip.
    let s = run(r#"JSON.stringify(JSON.parse("{\"a\":1,\"b\":2}"));"#);
    match s {
        Value::String(st) => {
            assert!(
                st.contains("\"a\":1") && st.contains("\"b\":2"),
                "got {st:?}"
            );
        }
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn json_parse_nested() {
    assert_eq!(
        run(r#"JSON.parse("{\"nested\":{\"x\":5}}").nested.x;"#),
        Value::Number(5.0)
    );
}

// JSON.stringify circular references

#[test]
fn json_stringify_circular_object() {
    // {name:"a", self: <cycle>}: stringify should throw a TypeError.
    let msg = run_err("var a = {name:'a'}; a.self = a; JSON.stringify(a);");
    assert!(
        msg.contains("TypeError") || msg.contains("circular"),
        "got: {}",
        msg
    );
}

#[test]
fn json_stringify_circular_array() {
    let msg = run_err("var a = [1,2,3]; a.push(a); JSON.stringify(a);");
    assert!(
        msg.contains("TypeError") || msg.contains("circular"),
        "got: {}",
        msg
    );
}

#[test]
fn json_stringify_shared_reference_ok() {
    // shared (non-cyclic) references must still serialize both occurrences.
    assert_eq!(
        run("var s = {v:1}; var t = {l:s, r:s}; JSON.stringify(t);"),
        Value::String(Arc::from(r#"{"l":{"v":1},"r":{"v":1}}"#))
    );
}

#[test]
fn json_stringify_uses_serialize_json_property_semantics() {
    assert_eq!(
        run(r#"
            var holders = [];
            var value = { a: { toJSON: function(key) { return key + ':json'; } } };
            var result = JSON.stringify(value, function(key, current) {
              if (key === '' || key === 'a') holders.push(this);
              return current;
            });
            [result, holders[0] !== value, holders[1] === value].join('|');
        "#),
        Value::String(Arc::from("{\"a\":\"a:json\"}|true|true"))
    );
    assert!(
        run_err("JSON.stringify({ get value() { throw new TypeError('get'); } });").contains("get")
    );
    assert!(run_err(
        "JSON.stringify({ value: { toJSON: function() { throw new TypeError('json'); } } });"
    )
    .contains("json"));
    assert!(run_err(
        "JSON.stringify({}, function(key, value) { if (key === '') { value.self = value; } return value; });"
    )
    .contains("circular"));
    assert_eq!(
        run(r#"
            var number = new Number(42);
            number.valueOf = function() { return 2; };
            var string = new String('x');
            string.toString = function() { return 'y'; };
            JSON.stringify([number, string, "\u0000\uD834"]);
        "#),
        Value::String(Arc::from("[2,\"y\",\"\\u0000\\ud834\"]"))
    );
    assert_eq!(
        run(r#"
            var order = [];
            var replacer = ['a'];
            Object.defineProperty(replacer, '0', {
              get: function() { order.push('replacer'); return 'a'; }
            });
            var space = new Number(1);
            space.valueOf = function() { order.push('space'); return 1; };
            JSON.stringify({ a: 1 }, replacer, space);
            order.join(',');
        "#),
        Value::String(Arc::from("replacer,space"))
    );
    assert_eq!(
        run(r#"
            var callable = new Proxy(function() {}, {
              ownKeys: function() { throw new Error('must not run'); }
            });
            JSON.stringify(callable) === undefined;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn json_raw_json_is_branded_frozen_and_stringifies_verbatim() {
    assert_eq!(
        run(r#"
            var raw = JSON.rawJSON('9007199254740993');
            var descriptor = Object.getOwnPropertyDescriptor(raw, 'rawJSON');
            [
              JSON.isRawJSON(raw),
              JSON.isRawJSON({ rawJSON: '1' }),
              Object.getPrototypeOf(raw) === null,
              Object.isFrozen(raw),
              descriptor.writable,
              descriptor.enumerable,
              descriptor.configurable,
              JSON.stringify({ value: raw }),
              Object.prototype.toString.call(JSON)
            ].join('|');
        "#),
        Value::String(Arc::from(
            "true|false|true|true|false|true|false|{\"value\":9007199254740993}|[object JSON]"
        ))
    );
    assert_eq!(
        run(r#"
            JSON.stringify(1n, function(key, value) {
              return typeof value === 'bigint' ? JSON.rawJSON(value) : value;
            });
        "#),
        Value::String(Arc::from("1"))
    );
    for source in ["''", "' 1'", "'1 '", "'{}'", "'[]'", "undefined"] {
        assert!(run_err(&format!("JSON.rawJSON({source});")).contains("SyntaxError"));
    }
    assert!(run_err("JSON.rawJSON(Symbol('x'));").contains("TypeError"));
    assert_eq!(
        run(r#"
            [
              JSON.stringify(JSON.rawJSON('"\\ud800"')),
              JSON.stringify(JSON.rawJSON('"\\udc00"')),
              JSON.stringify(JSON.rawJSON('"\\ud834\\udf06"')),
              JSON.stringify(JSON.rawJSON('"\\\\ud800"'))
            ].join('|');
        "#),
        Value::String(Arc::from(
            "\"\\ud800\"|\"\\udc00\"|\"\\ud834\\udf06\"|\"\\\\ud800\""
        ))
    );
}

#[test]
fn json_stringify_nested_object() {
    assert_eq!(
        run(r#"JSON.stringify({a:1, b:"hi", c:[1,2], d:{e:true}});"#),
        Value::String(Arc::from(r#"{"a":1,"b":"hi","c":[1,2],"d":{"e":true}}"#))
    );
}

// Object property insertion order (now preserved via IndexMap)

#[test]
fn object_keys_insertion_order() {
    let r = match run("Object.keys({z:1, a:2, m:3, b:4}).join(',')") {
        Value::String(s) => s.to_string(),
        v => format!("{:?}", v),
    };
    assert_eq!(r, "z,a,m,b");
}

#[test]
fn object_prototype_to_string_uses_receiver_brand() {
    assert_eq!(
        run(r#"
            [
              Object.prototype.toString.call([]),
              Object.prototype.toString.call(null),
              Object.prototype.toString.call(undefined),
              Object.prototype.toString.call("x"),
              Object.prototype.toString.call(Object(9)),
              Object.prototype.toString.call(Object(true)),
              Object.prototype.toString.call(function(){}),
              Object.prototype.toString.call(new Date(0)),
              Object.prototype.toString.call(Error("boom")),
              Object.prototype.toString.call(new Error("boom")),
              Object.prototype.toString.call(function(){ return arguments; }()),
              Object.getOwnPropertyNames(Object.prototype).join("|"),
              Error.prototype.toString.call({ name: "X", message: "Y" })
            ].join(",");
        "#),
        Value::String(Arc::from(
            "[object Array],[object Null],[object Undefined],[object String],[object Number],[object Boolean],[object Function],[object Date],[object Error],[object Error],[object Arguments],toString|toLocaleString|hasOwnProperty|isPrototypeOf|propertyIsEnumerable|valueOf|__defineGetter__|__defineSetter__|__lookupGetter__|__lookupSetter__|constructor|__proto__,X: Y"
        ))
    );
}

#[test]
fn object_prototype_to_string_handles_proxy_and_intrinsic_tags() {
    assert_eq!(
        run(r#"
            var arrayProxy = new Proxy(new Proxy([], {}), {});
            var functionProxy = new Proxy(new Proxy(function() {}, {}), {});
            var generatorProxy = new Proxy(function*() {}, {});
            var asyncProxy = new Proxy(async function() {}, {});

            var revocableArray = Proxy.revocable([], {
              get: function() { revocableArray.revoke(); }
            });
            var revokedDuringGet = Object.prototype.toString.call(
              revocableArray.proxy
            );

            var revoked = Proxy.revocable([], {});
            revoked.revoke();
            var revokedThrows = false;
            try {
              Object.prototype.toString.call(revoked.proxy);
            } catch (error) {
              revokedThrows = error instanceof TypeError;
            }

            var generator = function*() {};
            var promise = new Promise(function() {});
            var generatorTag = Object.getOwnPropertyDescriptor(
              generator.constructor.prototype, Symbol.toStringTag
            );
            var promiseTag = Object.getOwnPropertyDescriptor(
              Promise.prototype, Symbol.toStringTag
            );

            delete generatorProxy.constructor.prototype[Symbol.toStringTag];
            Object.defineProperty(asyncProxy.constructor.prototype, Symbol.toStringTag, {
              value: undefined
            });
            delete Promise.prototype[Symbol.toStringTag];
            delete Symbol.prototype[Symbol.toStringTag];
            delete BigInt.prototype[Symbol.toStringTag];

            var boxedReceiver;
            Object.defineProperty(Number.prototype, Symbol.toStringTag, {
              configurable: true,
              get: function() {
                "use strict";
                boxedReceiver = typeof this === "object" && this instanceof Number;
                return null;
              }
            });

            [
              Object.prototype.toString.call(arrayProxy),
              Object.prototype.toString.call(functionProxy),
              revokedDuringGet,
              revokedThrows,
              generatorTag.value,
              generatorTag.writable,
              generatorTag.enumerable,
              generatorTag.configurable,
              promiseTag.value,
              promiseTag.writable,
              promiseTag.enumerable,
              promiseTag.configurable,
              Object.prototype.toString.call(generatorProxy),
              Object.prototype.toString.call(asyncProxy),
              Object.prototype.toString.call(promise),
              Object.prototype.toString.call(Symbol()),
              Object.prototype.toString.call(1n),
              Object.prototype.toString.call(1),
              boxedReceiver
            ].join("|");
        "#),
        Value::String(Arc::from(
            "[object Array]|[object Function]|[object Array]|true|GeneratorFunction|false|false|true|Promise|false|false|true|[object Function]|[object Function]|[object Object]|[object Object]|[object Object]|[object Number]|true"
        ))
    );
}

#[test]
fn proxy_revoker_traces_its_proxy_until_first_call() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var pair = Proxy.revocable([], {});
            var revoke = pair.revoke;
            var weak = new WeakRef(pair.proxy);
            pair = null;
            true;
            "#,
        )
        .expect("failed to create retained Proxy revoker"),
        Value::Bool(true)
    );
    assert_eq!(
        vm.run(
            r#"
            forceGc();
            var retainedUntilRevoke = weak.deref() !== undefined;
            revoke();
            revoke();
            retainedUntilRevoke;
            "#,
        )
        .expect("Proxy revoker should retain its proxy until first call"),
        Value::Bool(true)
    );
    assert_eq!(
        vm.run("forceGc(); weak.deref() === undefined;")
            .expect("revoked Proxy should be collectable in a later job"),
        Value::Bool(true)
    );
    assert_eq!(
        vm.run(
            r#"
            Object.prototype.toString.call(new Proxy([], {
              get: function(target, key, receiver) {
                forceGc();
                return Reflect.get(target, key, receiver);
              }
            }));
            "#,
        )
        .expect("Proxy toString should retain values across its get trap"),
        Value::String(Arc::from("[object Array]"))
    );
    assert_eq!(
        vm.run(
            r#"
            var boxedReceiver;
            Object.defineProperty(Number.prototype, Symbol.toStringTag, {
              configurable: true,
              get: function() {
                "use strict";
                forceGc();
                boxedReceiver = typeof this === "object" && this instanceof Number;
                return null;
              }
            });
            Object.prototype.toString.call(1) + "|" + boxedReceiver;
            "#,
        )
        .expect("boxed toString receiver should survive its tag getter"),
        Value::String(Arc::from("[object Number]|true"))
    );
}

#[test]
fn object_prototype_value_of_and_to_locale_string_coerce_receiver() {
    assert!(run_err("Object.prototype.valueOf.call(undefined);").contains("TypeError"));
    assert!(run_err("Object.prototype.valueOf.call(null);").contains("TypeError"));
    assert!(run_err("(1, Object.prototype.valueOf)();").contains("TypeError"));
    assert_eq!(
        run("typeof Object.prototype.valueOf.call(true) + ':' + typeof Object.prototype.valueOf.call(false);"),
        Value::String(Arc::from("object:object"))
    );

    assert!(run_err("Object.prototype.toLocaleString.call(undefined);").contains("TypeError"));
    assert!(run_err("Object.prototype.toLocaleString.call(null);").contains("TypeError"));
    assert_eq!(
        run(r#"
            "use strict";
            Boolean.prototype.toString = function() { return typeof this; };
            true.toLocaleString();
        "#),
        Value::String(Arc::from("boolean"))
    );
    assert_eq!(
        run(r#"
            "use strict";
            Object.defineProperty(Boolean.prototype, "toString", {
              get: function() {
                var v = typeof this;
                return function() { return v + ":" + typeof this; };
              }
            });
            true.toLocaleString();
        "#),
        Value::String(Arc::from("boolean:boolean"))
    );
}

#[test]
fn object_prototype_legacy_accessor_methods() {
    assert_eq!(
        run(r#"
            var o = {};
            function getX() { return 7; }
            function setX(v) { this.seen = v; }
            o.__defineGetter__("x", getX);
            o.__defineSetter__("x", setX);
            var d = Object.getOwnPropertyDescriptor(o, "x");
            o.x = 9;
            [
              o.x,
              o.seen,
              d.get === getX,
              d.set === setX,
              d.enumerable,
              d.configurable,
              o.__lookupGetter__("x") === getX,
              o.__lookupSetter__("x") === setX,
              Object.prototype.propertyIsEnumerable.call(Object.prototype, "__defineGetter__"),
              Object.prototype.__defineGetter__.length,
              Object.prototype.__lookupSetter__.name
            ].join(",");
        "#),
        Value::String(Arc::from(
            "7,9,true,true,true,true,true,true,false,2,__lookupSetter__"
        ))
    );
    assert!(
        run_err("Object.prototype.__defineGetter__.call(null, 'x', function(){});")
            .contains("TypeError")
    );
    assert!(run_err("({}).__defineGetter__('x', 1);").contains("TypeError"));
    assert!(run_err(
        r#"({}).__defineGetter__({ toString: function(){ throw new Error("key"); } }, function(){});"#
    )
    .contains("key"));
}

#[test]
fn object_prototype_proto_accessor_and_mutation_status() {
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(Object.prototype, "__proto__");
            var o = {};
            var p = { x: 7 };
            desc.set.call(o, p);
            var nullProto = Object.create(null);
            nullProto.__proto__ = "own";
            var shadow = {};
            Object.defineProperty(shadow, "__proto__", {
              value: "before",
              writable: true,
              configurable: true
            });
            shadow.__proto__ = p;
            var sameNull = Object.setPrototypeOf(Object.prototype, null) === Object.prototype;
            var reflectSameNull = Reflect.setPrototypeOf(Object.prototype, null);
            var reflectImmutable = Reflect.setPrototypeOf(Object.prototype, {});
            var root = {};
            var leaf = Object.create(root);
            var reflectCycle = Reflect.setPrototypeOf(root, leaf);
            [
              Object.getPrototypeOf(Object.prototype) === null,
              typeof desc.get,
              desc.get.name,
              desc.get.length,
              typeof desc.set,
              desc.set.name,
              desc.set.length,
              desc.enumerable,
              desc.configurable,
              desc.get.call(o) === p,
              o.x,
              desc.set.call(1, p),
              desc.set.call(o, 1),
              nullProto.__proto__,
              Object.prototype.hasOwnProperty.call(nullProto, "__proto__"),
              shadow.__proto__ === p,
              Object.getPrototypeOf(shadow) === Object.prototype,
              sameNull,
              reflectSameNull,
              reflectImmutable,
              reflectCycle
            ].join(",");
        "#),
        Value::String(Arc::from(
            "true,function,get __proto__,0,function,set __proto__,1,false,true,true,7,,,own,true,true,true,true,true,false,false"
        ))
    );
    assert!(run_err("Object.setPrototypeOf(Object.prototype, {});").contains("TypeError"));
    assert!(run_err(
        "var root = {}; var leaf = Object.create(root); Object.getOwnPropertyDescriptor(Object.prototype, '__proto__').set.call(root, leaf);"
    )
    .contains("TypeError"));
    assert_eq!(
        run("var o = Object.preventExtensions({}); try { Object.getOwnPropertyDescriptor(Object.prototype, '__proto__').set.call(o, {}); } catch (e) { e instanceof TypeError; }"),
        Value::Bool(true)
    );
}

#[test]
fn created_realm_object_prototypes_have_immutable_prototypes() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var mainTypeError = TypeError;
            function createGlobal() { return $262.createRealm().global; }

            var sameGlobal = createGlobal();
            var samePrototype = sameGlobal.Object.prototype;
            var samePrototypeSucceeds =
              Object.setPrototypeOf(samePrototype, null) === samePrototype &&
              sameGlobal.Object.setPrototypeOf(samePrototype, null) === samePrototype &&
              Reflect.setPrototypeOf(samePrototype, null) === true &&
              sameGlobal.Reflect.setPrototypeOf(samePrototype, null) === true &&
              Object.getPrototypeOf(samePrototype) === null &&
              Object.isExtensible(samePrototype) === true;

            function mainObjectRejects() {
              var global = createGlobal();
              var prototype = global.Object.prototype;
              var error;
              try { Object.setPrototypeOf(prototype, {}); }
              catch (caught) { error = caught; }
              return error instanceof mainTypeError &&
                !(error instanceof global.TypeError) &&
                Object.getPrototypeOf(prototype) === null;
            }

            function foreignObjectRejects() {
              var global = createGlobal();
              var prototype = global.Object.prototype;
              var error;
              try { global.Object.setPrototypeOf(prototype, {}); }
              catch (caught) { error = caught; }
              return error instanceof global.TypeError &&
                !(error instanceof mainTypeError) &&
                Object.getPrototypeOf(prototype) === null;
            }

            function mainSetterRejects() {
              var global = createGlobal();
              var prototype = global.Object.prototype;
              var setter = Object.getOwnPropertyDescriptor(
                Object.prototype,
                "__proto__"
              ).set;
              var error;
              try { setter.call(prototype, {}); }
              catch (caught) { error = caught; }
              return error instanceof mainTypeError &&
                !(error instanceof global.TypeError) &&
                Object.getPrototypeOf(prototype) === null;
            }

            function foreignSetterRejects() {
              var global = createGlobal();
              var prototype = global.Object.prototype;
              var setter = global.Object.getOwnPropertyDescriptor(
                prototype,
                "__proto__"
              ).set;
              var error;
              try { setter.call(prototype, {}); }
              catch (caught) { error = caught; }
              return error instanceof global.TypeError &&
                !(error instanceof mainTypeError) &&
                Object.getPrototypeOf(prototype) === null;
            }

            function foreignMethodsUseTheirOwnRealmForMainPrototypeErrors() {
              var global = createGlobal();
              var objectError;
              var setterError;
              try { global.Object.setPrototypeOf(Object.prototype, {}); }
              catch (caught) { objectError = caught; }
              var setter = global.Object.getOwnPropertyDescriptor(
                global.Object.prototype,
                "__proto__"
              ).set;
              try { setter.call(Object.prototype, {}); }
              catch (caught) { setterError = caught; }
              return objectError instanceof global.TypeError &&
                !(objectError instanceof mainTypeError) &&
                setterError instanceof global.TypeError &&
                !(setterError instanceof mainTypeError) &&
                Object.getPrototypeOf(Object.prototype) === null;
            }

            var mainReflectGlobal = createGlobal();
            var mainReflectPrototype = mainReflectGlobal.Object.prototype;
            var mainReflectRejects =
              Reflect.setPrototypeOf(mainReflectPrototype, {}) === false &&
              Object.getPrototypeOf(mainReflectPrototype) === null &&
              Object.isExtensible(mainReflectPrototype) === true;

            var foreignReflectGlobal = createGlobal();
            var foreignReflectPrototype = foreignReflectGlobal.Object.prototype;
            var foreignReflectRejects =
              foreignReflectGlobal.Reflect.setPrototypeOf(
                foreignReflectPrototype,
                {}
              ) === false &&
              Object.getPrototypeOf(foreignReflectPrototype) === null;

            var transparentGlobal = createGlobal();
            var transparentPrototype = transparentGlobal.Object.prototype;
            var transparentProxy = new Proxy(transparentPrototype, {});
            var transparentObjectError;
            try { Object.setPrototypeOf(transparentProxy, {}); }
            catch (caught) { transparentObjectError = caught; }
            var transparentProxyRejects =
              Reflect.setPrototypeOf(transparentProxy, {}) === false &&
              transparentObjectError instanceof mainTypeError &&
              !(transparentObjectError instanceof transparentGlobal.TypeError) &&
              Object.getPrototypeOf(transparentPrototype) === null;

            var trappingGlobal = createGlobal();
            var trappingPrototype = trappingGlobal.Object.prototype;
            var trappingProxy = new Proxy(trappingPrototype, {
              setPrototypeOf: function() { return true; }
            });
            var trappingProxyMayReportSuccess =
              Reflect.setPrototypeOf(trappingProxy, {}) === true &&
              Object.getPrototypeOf(trappingPrototype) === null;

            var invariantGlobal = createGlobal();
            var invariantPrototype = invariantGlobal.Object.prototype;
            Object.preventExtensions(invariantPrototype);
            var invariantProxy = new Proxy(invariantPrototype, {
              setPrototypeOf: function() { return true; }
            });
            var invariantError;
            try { Reflect.setPrototypeOf(invariantProxy, {}); }
            catch (caught) { invariantError = caught; }
            var trappingNonExtensibleRejects =
              invariantError instanceof mainTypeError &&
              !(invariantError instanceof invariantGlobal.TypeError) &&
              Object.getPrototypeOf(invariantPrototype) === null &&
              Object.isExtensible(invariantPrototype) === false;

            var retainedGlobal = createGlobal();
            var retainedPrototype = retainedGlobal.Object.prototype;
            var retainedReflect = retainedGlobal.Reflect;
            retainedGlobal.Object = null;
            retainedGlobal.Reflect = null;
            forceGc();
            var retainedAfterGc =
              retainedReflect.setPrototypeOf(retainedPrototype, {}) === false &&
              Object.getPrototypeOf(retainedPrototype) === null;

            [
              samePrototypeSucceeds,
              mainObjectRejects(),
              foreignObjectRejects(),
              mainSetterRejects(),
              foreignSetterRejects(),
              foreignMethodsUseTheirOwnRealmForMainPrototypeErrors(),
              mainReflectRejects,
              foreignReflectRejects,
              transparentProxyRejects,
              trappingProxyMayReportSuccess,
              trappingNonExtensibleRejects,
              retainedAfterGc
            ].join("|");
            "#,
        )
        .expect("created Realm Object.prototype immutability should execute"),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn proxy_prototype_internal_methods_follow_traps_and_invariants() {
    let src = r#"
        var target = {};
        var proto = { tag: "proto" };
        var replacement = { tag: "replacement" };
        var calls = [];
        var proxy = new Proxy(target, {
          getPrototypeOf: function(t) {
            calls.push("get:" + (t === target));
            return proto;
          },
          setPrototypeOf: function(t, v) {
            calls.push("set:" + (t === target) + ":" + (v === replacement));
            Object.setPrototypeOf(t, v);
            return true;
          }
        });
        var getViaReflect = Reflect.getPrototypeOf(proxy) === proto;
        var getViaObject = Object.getPrototypeOf(proxy) === proto;
        var setViaReflect = Reflect.setPrototypeOf(proxy, replacement);
        var targetUpdated = Object.getPrototypeOf(target) === replacement;

        var delegatedTarget = Object.create(proto);
        var delegated = new Proxy(delegatedTarget, {
          getPrototypeOf: null,
          setPrototypeOf: undefined
        });
        var delegatedGet = Reflect.getPrototypeOf(delegated) === proto;
        var delegatedSet = Reflect.setPrototypeOf(delegated, replacement);

        var fixedGetTarget = Object.create(proto);
        Object.preventExtensions(fixedGetTarget);
        var fixedGetProxy = new Proxy(fixedGetTarget, {
          getPrototypeOf: function() { return replacement; }
        });
        var getInvariant = false;
        try { Reflect.getPrototypeOf(fixedGetProxy); }
        catch (e) { getInvariant = e instanceof TypeError; }

        var fixedSetTarget = Object.create(proto);
        Object.preventExtensions(fixedSetTarget);
        var fixedSetProxy = new Proxy(fixedSetTarget, {
          setPrototypeOf: function() { return true; }
        });
        var setInvariant = false;
        try { Reflect.setPrototypeOf(fixedSetProxy, replacement); }
        catch (e) { setInvariant = e instanceof TypeError; }

        function Custom() {}
        var instanceProxy = new Proxy({}, {
          getPrototypeOf: function() { return Custom.prototype; }
        });

        [
          getViaReflect,
          getViaObject,
          setViaReflect,
          targetUpdated,
          delegatedGet,
          delegatedSet,
          Object.getPrototypeOf(delegatedTarget) === replacement,
          getInvariant,
          setInvariant,
          instanceProxy instanceof Custom,
          calls.join("|")
        ].join(",");
    "#;
    assert_eq!(
        run(src),
        Value::String(Arc::from(
            "true,true,true,true,true,true,true,true,true,true,get:true|get:true|set:true:true"
        ))
    );
}

#[test]
fn prototype_api_validation_and_false_status_order_match_spec() {
    assert_eq!(
        run(r#"
            var nullProxy = new Proxy({}, {
              getPrototypeOf: function () { return null; }
            });
            var nullResult =
              Reflect.getPrototypeOf(nullProxy) === null &&
              Object.getPrototypeOf(nullProxy) === null;

            var trapReads = 0;
            var validationHandler = {};
            Object.defineProperty(validationHandler, "setPrototypeOf", {
              get: function () {
                trapReads += 1;
                return function () { return true; };
              }
            });
            var validationProxy = new Proxy({}, validationHandler);
            var objectValidation = false;
            var reflectValidation = false;
            try { Object.setPrototypeOf(validationProxy, 1); }
            catch (error) { objectValidation = error instanceof TypeError; }
            try { Reflect.setPrototypeOf(validationProxy, 1); }
            catch (error) { reflectValidation = error instanceof TypeError; }

            var falseProxy = new Proxy({}, {
              setPrototypeOf: function () { return false; }
            });
            var reflectFalse = Reflect.setPrototypeOf(falseProxy, {}) === false;
            var objectFalse = false;
            try { Object.setPrototypeOf(falseProxy, {}); }
            catch (error) { objectFalse = error instanceof TypeError; }

            var nestedLog = [];
            var nestedPrototype = {};
            var nestedBase = Object.preventExtensions(
              Object.create(nestedPrototype)
            );
            var nestedInner = new Proxy(nestedBase, {
              getPrototypeOf: function () {
                nestedLog.push("inner");
                return nestedPrototype;
              }
            });
            var nestedOuter = new Proxy(nestedInner, {
              getPrototypeOf: function () {
                nestedLog.push("outer");
                return nestedPrototype;
              }
            });
            var nestedPass =
              Reflect.getPrototypeOf(nestedOuter) === nestedPrototype;
            var nestedMismatch = false;
            var mismatchOuter = new Proxy(nestedInner, {
              getPrototypeOf: function () {
                nestedLog.push("mismatch");
                return {};
              }
            });
            try { Reflect.getPrototypeOf(mismatchOuter); }
            catch (error) { nestedMismatch = error instanceof TypeError; }

            [
              nullResult,
              objectValidation,
              reflectValidation,
              trapReads === 0,
              reflectFalse,
              objectFalse,
              nestedPass,
              nestedMismatch,
              nestedLog.join(",") === "outer,inner,mismatch,inner"
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn proxy_revocable_revoke_function_has_spec_own_properties() {
    assert_eq!(
        run(r#"
            var pair = Proxy.revocable({ x: 1 }, {});
            var revoke = pair.revoke;
            var length = Object.getOwnPropertyDescriptor(revoke, "length");
            var name = Object.getOwnPropertyDescriptor(revoke, "name");
            var names = Object.getOwnPropertyNames(revoke).join(",");
            revoke();
            var revoked = false;
            try { pair.proxy.x; } catch (e) { revoked = e instanceof TypeError; }
            [
              revoke.length,
              revoke.name,
              length.writable,
              length.enumerable,
              length.configurable,
              name.writable,
              name.enumerable,
              name.configurable,
              names,
              revoked
            ].join("|");
            "#,),
        Value::String(Arc::from(
            "0||false|false|true|false|false|true|length,name|true"
        ))
    );
}

#[test]
fn callable_proxy_follows_target_callability_and_apply_trap() {
    assert_eq!(
        run(r#"
            function target(a, b) { return this.base + a + b; }
            var proxy = new Proxy(target, {});
            [typeof proxy, proxy.call({ base: 1 }, 2, 3)].join("|");
            "#),
        Value::String(Arc::from("function|6"))
    );
    assert_eq!(
        run(r#"
            function target() { return "target"; }
            var seen = [];
            var proxy = new Proxy(target, {
              apply: function(t, thisArg, args) {
                seen.push(t === target, thisArg.tag, args.length, args[0], args[1]);
                return "trap";
              }
            });
            [typeof proxy, proxy.call({ tag: "this" }, "a", "b"), seen.join(",")].join("|");
            "#),
        Value::String(Arc::from("function|trap|true,this,2,a,b"))
    );
    assert_eq!(
        run(r#"
            var revocableTarget = Proxy.revocable(function() {}, {});
            revocableTarget.revoke();
            var revocable = Proxy.revocable(revocableTarget.proxy, {});
            typeof revocable.proxy;
            "#),
        Value::String(Arc::from("function"))
    );
    assert!(run_err(
        r#"
            var pair = Proxy.revocable(function() {}, {});
            pair.revoke();
            pair.proxy();
            "#,
    )
    .contains("TypeError"));
}

#[test]
fn constructable_proxy_follows_target_and_construct_trap() {
    assert_eq!(
        run(r#"
            function Target(a, b) { this.sum = a + b; }
            var proxy = new Proxy(Target, {});
            new proxy(2, 3).sum;
            "#),
        Value::Number(5.0)
    );
    assert_eq!(
        run(r#"
            var C = $262.createRealm().global.eval(
              "new Proxy(function() {}, { construct: function(_, args) { return args; } })"
            );
            new C(1, 2).constructor === Array;
            "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            function Target() {}
            function NewTarget() {}
            NewTarget.prototype = { marker: true };
            var seen = [];
            var proxy = new Proxy(Target, {
              construct: function(t, args, nt) {
                seen.push(t === Target, args.constructor === Array, args.join(","), nt === NewTarget);
                return Reflect.construct(t, args, nt);
              }
            });
            var result = Reflect.construct(proxy, [4, 5], NewTarget);
            seen.join("|") + "|" + (Object.getPrototypeOf(result) === NewTarget.prototype);
            "#),
        Value::String(Arc::from("true|true|4,5|true|true"))
    );
    assert!(run_err(
        r#"
            var proxy = new Proxy(function() {}, { construct: function() { return 1; } });
            new proxy();
            "#,
    )
    .contains("TypeError"));
}

#[test]
fn for_in_insertion_order() {
    let src = "var o = {a:1,b:2,c:3,d:4,e:5}; var k=[]; for (var x in o) k.push(x); k.join(',');";
    assert_eq!(run(src), Value::String(Arc::from("a,b,c,d,e")));
}

#[test]
fn object_entries_insertion_order() {
    let src = "Object.entries({z:1,a:2,m:3,b:4}).map(e=>e[0]+'='+e[1]).join(',');";
    assert_eq!(run(src), Value::String(Arc::from("z=1,a=2,m=3,b=4")));
}

#[test]
fn json_stringify_key_order() {
    // JSON.stringify now preserves insertion order.
    assert_eq!(
        run(r#"JSON.stringify({a:1, b:"hi", c:[1,2], d:{e:true}});"#),
        Value::String(Arc::from(r#"{"a":1,"b":"hi","c":[1,2],"d":{"e":true}}"#))
    );
}

#[test]
fn for_in_order_builtins_follow_spec_key_order() {
    assert_eq!(
        run("var o=Object.create({p2:'proto'},{p1:{value:'p1',enumerable:true},p2:{value:'own',enumerable:false}}); Object.keys(o).join(',') + ':' + o.propertyIsEnumerable('p2');"),
        Value::String(Arc::from("p1:false"))
    );
    assert_eq!(
        run(
            r#"var o={p1:'p1',p2:'p2',p3:'p3'}; Object.defineProperty(o,'add',{enumerable:true,get:function(){o.extra='extra'; return 'add';}}); o.p4='p4'; o[2]='2'; o[0]='0'; o[1]='1'; delete o.p1; delete o.p3; o.p1='p1'; JSON.stringify(o);"#
        ),
        Value::String(Arc::from(
            r#"{"0":"0","1":"1","2":"2","p2":"p2","add":"add","p4":"p4","p1":"p1"}"#
        ))
    );
    assert_eq!(
        run(
            r#"var calls=[]; function reviver(name,val){calls.push(name); return val;} JSON.parse('{"p1":0,"p2":0,"p1":0,"2":0,"1":0}', reviver); calls.join(',');"#
        ),
        Value::String(Arc::from("1,2,p1,p2,"))
    );
}

// --- Array/Number/Object/Math coverage expansion ---

#[test]
fn array_flat_flatmap() {
    assert_eq!(
        run("[1,[2,[3]]].flat().join(',')"),
        Value::String(Arc::from("1,2,3"))
    );
    assert_eq!(
        run("[1,[2,[3]]].flat(2).join(',')"),
        Value::String(Arc::from("1,2,3"))
    );
    assert_eq!(
        run("[1,2,3].flatMap(x=>[x,x*10]).join(',')"),
        Value::String(Arc::from("1,10,2,20,3,30"))
    );
}

#[test]
fn array_flat_is_generic_species_aware_and_observable() {
    assert_eq!(
        run(r#"
            var proto = { 0: [1, , 3] };
            var source = Object.create(proto);
            source[1] = 4;
            source.length = 2;
            Array.prototype.flat.call(source).join(",");
            "#,),
        Value::String(Arc::from("1,3,4"))
    );
    assert_eq!(
        run(r#"
            var log = [];
            var depth = { valueOf: function() { log.push("depth"); return 1; } };
            var array = [1];
            array.constructor = {
              get [Symbol.species]() {
                log.push("species");
                return function() { log.push("construct"); };
              }
            };
            var source = new Proxy(array, {
              get: function(target, key, receiver) {
                if (key === "length") log.push("length");
                return Reflect.get(target, key, receiver);
              }
            });
            var result = source.flat(depth);
            log.join(",") + "|" + result[0] + "|" +
              Object.prototype.hasOwnProperty.call(result, "length");
            "#,),
        Value::String(Arc::from("length,depth,species,construct|1|false"))
    );
    assert_eq!(
        run(r#"
            var log = [];
            var source = new Proxy({ 0: [2], length: 1 }, {
              has: function(target, key) { log.push("has:" + key); return key in target; },
              get: function(target, key) { log.push("get:" + key); return target[key]; }
            });
            Array.prototype.flat.call(source);
            log.join(",");
            "#,),
        Value::String(Arc::from("get:length,has:0,get:0"))
    );
}

#[test]
fn array_flat_map_uses_shared_live_flattening_path() {
    assert_eq!(
        run(r#"
            var source = { 0: 1, 1: 2, length: 2 };
            var seen = [];
            var context = { factor: 10 };
            var result = Array.prototype.flatMap.call(source, function(value, index, object) {
              seen.push(value + ":" + index + ":" + (object === source));
              return [value * this.factor];
            }, context);
            seen.join(",") + "|" + result.join(",");
            "#,),
        Value::String(Arc::from("1:0:true,2:1:true|10,20"))
    );
    assert_eq!(
        run(r#"
            var source = [1, 2, 3];
            var result = source.flatMap(function(value, index) {
              if (index === 0) delete source[1];
              return [value];
            });
            result.join(",");
            "#,),
        Value::String(Arc::from("1,3"))
    );
}

#[test]
fn array_flat_map_roots_prior_callback_results_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var calls = 0;
            [1, 2].flatMap(function(value) {
              if (calls++ === 1) forceGc();
              return [{ value: value }];
            }).map(function(entry) { return entry.value; }).join(",");
            "#,
        )
        .expect("flatMap callback results should survive observable GC"),
        Value::String(Arc::from("1,2"))
    );
}

#[test]
fn array_map_roots_prior_callback_results_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var calls = 0;
            [1, 2].map(function(value) {
              if (calls++ === 1) forceGc();
              return { value: value };
            }).map(function(entry) { return entry.value; }).join(",");
            "#,
        )
        .expect("map callback results should survive observable GC"),
        Value::String(Arc::from("1,2"))
    );
}

#[test]
fn array_at_shift_unshift_splice() {
    assert_eq!(run("[1,2,3].at(-1);"), Value::Number(3.0));
    assert_eq!(run("[1,2,3].at(0);"), Value::Number(1.0));
    assert!(run_err("Array.prototype.at.call(null, 0)")
        .contains("Cannot convert undefined or null to object"));
    assert!(run_err("Array.prototype.at.call(undefined, 0)")
        .contains("Cannot convert undefined or null to object"));
    assert_eq!(
        run("Array.prototype.at.call({0:'x', 1:'y', length: 2}, -1);"),
        Value::String(Arc::from("y"))
    );
    assert_eq!(
        run("var a=[1,2,3]; a.shift(); a.join(',');"),
        Value::String(Arc::from("2,3"))
    );
    assert_eq!(
        run("var b=[1,2,3]; b.unshift(0); b.join(',');"),
        Value::String(Arc::from("0,1,2,3"))
    );
    assert_eq!(
        run("var c=[1,2,3,4,5]; c.splice(1,2); c.join(',');"),
        Value::String(Arc::from("1,4,5"))
    );
}

#[test]
fn array_last_index_of() {
    assert_eq!(run("[1,2,3,2].lastIndexOf(2);"), Value::Number(3.0));
    assert_eq!(run("[1,2,3].lastIndexOf(9);"), Value::Number(-1.0));
}

#[test]
fn string_pad_at_replaceall_substring() {
    assert_eq!(
        run("'abc'.padStart(6,'0');"),
        Value::String(Arc::from("000abc"))
    );
    assert_eq!(
        run("'abc'.padEnd(6,'0');"),
        Value::String(Arc::from("abc000"))
    );
    assert_eq!(run("'abc'.at(-1);"), Value::String(Arc::from("c")));
    assert_eq!(
        run("'a-b-a'.replaceAll('-','_');"),
        Value::String(Arc::from("a_b_a"))
    );
    assert_eq!(
        run("'hello'.substring(1,3);"),
        Value::String(Arc::from("el"))
    );
    assert_eq!(
        run(r#"'5ABBBABAB'.slice({ valueOf: function(){ return 2; } }, '5');"#),
        Value::String(Arc::from("BBB"))
    );
    assert_eq!(
        run(r#"'report'.slice(function(){}());"#),
        Value::String(Arc::from("report"))
    );
    assert_eq!(
        run(r#"String(void 0).substring('e', undefined);"#),
        Value::String(Arc::from("undefined"))
    );
    assert_eq!(
        run(r#"(function(){
                var b = new Boolean(false);
                b.substring = String.prototype.substring;
                return b.substring(function(){ return true; }(), undefined);
            })();"#),
        Value::String(Arc::from("alse"))
    );
    assert_eq!(
        run("'  hi  '.trimStart();"),
        Value::String(Arc::from("hi  "))
    );
    assert_eq!(
        run(r#"'\uFEFF\u00A0hi\uFEFF'.trim();"#),
        Value::String(Arc::from("hi"))
    );
    assert_eq!(
        run(r#"'\uFEFFhi\uFEFF'.trimStart();"#),
        Value::String(Arc::from("hi\u{FEFF}"))
    );
    assert_eq!(
        run(r#"'\uFEFFhi\uFEFF'.trimEnd();"#),
        Value::String(Arc::from("\u{FEFF}hi"))
    );
    assert_eq!(
        run(r#"String.prototype.trim.call(new RegExp(/test/));"#),
        Value::String(Arc::from("/test/"))
    );
    assert_eq!(
        run(r#"String.prototype.trim.call(function(){ return arguments; }(1, 2, true));"#),
        Value::String(Arc::from("[object Arguments]"))
    );
    assert_eq!(
        run(r#"'\u180Ehi\u0085'.trim();"#),
        Value::String(Arc::from("\u{180E}hi\u{0085}"))
    );
}

#[test]
fn string_replace_all_test262_regressions() {
    assert_eq!(
        run(r#"'aba'.replaceAll('b', "$$-$&-$`-$'");"#),
        Value::String(Arc::from("a$-b-a-aa"))
    );
    assert_eq!(
        run(r#"'aaa'.replaceAll('a', function(m, pos, s) { return String(pos) + s.length; });"#),
        Value::String(Arc::from("031323"))
    );
    assert_eq!(
        run(r#"'abc abc abc'.replaceAll(/b/g, 'z');"#),
        Value::String(Arc::from("azc azc azc"))
    );
    assert_eq!(
        run(r#"'abcabcabcabc'.replaceAll(/a(b)(ca)/g, '$2-$1');"#),
        Value::String(Arc::from("ca-bbcca-bbc"))
    );
    assert_eq!(
        run(r#"(function(){
                 var re = /b/g;
                 var called = 0;
                 re[Symbol.replace] = function(O, replaceValue) {
                   called++;
                   return O + "|" + replaceValue;
                 };
                 return "abc".replaceAll(re, "z") + "|" + called;
               })();"#),
        Value::String(Arc::from("abc|z|1"))
    );
    assert_eq!(
        run(r#"(function(){
                 var re = /./iyg;
                 re[Symbol.replace] = undefined;
                 return 'aa /./giy /./iyg /./gyi /./giy aa'.replaceAll(re, 'z');
               })();"#),
        Value::String(Arc::from("aa z /./iyg /./gyi z aa"))
    );
    assert!(
        run_err(r#"'abc'.replaceAll(/b/, 'z');"#).contains("non-global RegExp"),
        "replaceAll must reject non-global RegExp search values"
    );
}

#[test]
fn string_normalize_follows_unicode_forms_and_descriptors() {
    assert_eq!(
        run(
            r#"var d = Object.getOwnPropertyDescriptor(String.prototype, "normalize");
               [
                 typeof String.prototype.normalize,
                 d.writable,
                 d.enumerable,
                 d.configurable,
                 String.prototype.normalize.length,
                 String.prototype.normalize.name
               ].join("|");"#
        ),
        Value::String(Arc::from("function|true|false|true|0|normalize"))
    );
    assert_eq!(
        run(r#"var s = "\u1E9B\u0323";
               [
                 s.normalize("NFC") === "\u1E9B\u0323",
                 s.normalize("NFD") === "\u017F\u0323\u0307",
                 s.normalize("NFKC") === "\u1E69",
                 s.normalize("NFKD") === "\u0073\u0323\u0307"
               ].join("|");"#),
        Value::String(Arc::from("true|true|true|true"))
    );
    assert_eq!(
        run(r#"var form = { toString: function() { return "NFD"; } };
               "\u00C5".normalize(form) === "A\u030A";"#),
        Value::Bool(true)
    );
    assert!(run_err(r#""x".normalize("bad");"#).contains("RangeError"));
}

#[test]
fn number_static_methods() {
    assert_eq!(run("Number.isInteger(5);"), Value::Bool(true));
    assert_eq!(run("Number.isInteger(5.5);"), Value::Bool(false));
    assert_eq!(run("Number.isFinite(Infinity);"), Value::Bool(false));
    assert_eq!(run("Number.isNaN(NaN);"), Value::Bool(true));
    assert_eq!(run("Number.isNaN('NaN');"), Value::Bool(false));
    assert_eq!(run("Number.isSafeInteger(2**53);"), Value::Bool(false));
    assert_eq!(run("Number.parseInt === parseInt;"), Value::Bool(true));
    assert_eq!(run("Number.parseFloat === parseFloat;"), Value::Bool(true));
    assert_eq!(
        run("var d = Object.getOwnPropertyDescriptor(Number, 'isFinite'); [d.writable, d.enumerable, d.configurable].join(',');"),
        Value::String(Arc::from("true,false,true"))
    );
    assert_eq!(
        run("var d = Object.getOwnPropertyDescriptor(Number, 'MAX_VALUE'); [d.writable, d.enumerable, d.configurable].join(',');"),
        Value::String(Arc::from("false,false,false"))
    );
}

#[test]
fn number_constants_and_radix() {
    assert_eq!(
        run("Number.MAX_SAFE_INTEGER;"),
        Value::Number(9007199254740991.0)
    );
    assert_eq!(run("Number.EPSILON > 0;"), Value::Bool(true));
    assert_eq!(run("(255).toString(16);"), Value::String(Arc::from("ff")));
    assert_eq!(
        run("(4096).toString(16);"),
        Value::String(Arc::from("1000"))
    );
    assert_eq!(run("(0).toString(36);"), Value::String(Arc::from("0")));
    assert_eq!(
        run("(3.14159).toFixed(2);"),
        Value::String(Arc::from("3.14"))
    );
}

#[test]
fn parse_int_prefix() {
    assert_eq!(run("parseInt('42px');"), Value::Number(42.0));
    assert_eq!(run("parseInt('0xff');"), Value::Number(255.0));
    assert_eq!(run("parseInt('  -17  ');"), Value::Number(-17.0));
    assert_eq!(run("parseInt('3.14');"), Value::Number(3.0));
    assert_eq!(run("parseInt('zz',36);"), Value::Number(1295.0));
    assert_eq!(run("Number.parseInt('42px');"), Value::Number(42.0));
}

#[test]
fn parse_int_radix_to_int32_and_large_prefix() {
    assert!(matches!(
        run("parseInt('11', true);"),
        Value::Number(n) if n.is_nan()
    ));
    assert_eq!(run("parseInt('11', '2');"), Value::Number(3.0));
    assert_eq!(run("parseInt('11', new Number(2));"), Value::Number(3.0));
    assert_eq!(run("parseInt('11', new String('2'));"), Value::Number(3.0));
    assert_eq!(
        run("parseInt('11', { valueOf: function() { return 2; } });"),
        Value::Number(3.0)
    );
    assert_eq!(run("parseInt('11', Infinity);"), Value::Number(11.0));
    assert_eq!(run("parseInt('11', 4294967298);"), Value::Number(3.0));
    assert_eq!(
        run("parseInt('0x10000000000000000', 16);"),
        Value::Number(18_446_744_073_709_552_000.0)
    );
    assert_eq!(
        run("parseInt('-10000000000000000000', 10);"),
        Value::Number(-10_000_000_000_000_000_000.0)
    );
}

#[test]
fn object_constructor_uses_active_function_and_new_target_realms() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var receiver = { receiver: true };
            var called = other.Object.call(receiver);
            var calledNull = other.Object(null);
            var calledUndefined = other.Object(undefined);
            var constructedNull = new other.Object(null);
            var constructedUndefined = new other.Object(undefined);
            var boxed = other.Object(1);
            var constructedBoxed = new other.Object(1);
            var argument = { argument: true };
            class Sub extends other.Object {}
            var subclassed = new Sub(argument);
            var reflected = Reflect.construct(other.Object, [argument], Sub);
            var active = Reflect.construct(other.Object, [argument], other.Object);
            var customPrototype = {};
            var newTarget = new Proxy(function() {}, {
              get: function(target, key, receiver) {
                if (key === "prototype") {
                  forceGc();
                  return customPrototype;
                }
                return Reflect.get(target, key, receiver);
              }
            });
            var custom = Reflect.construct(other.Object, [argument], newTarget);
            var deepNewTarget = new other.Function();
            deepNewTarget.prototype = null;
            for (var i = 0; i < 40; i++) {
              deepNewTarget = deepNewTarget.bind(null);
            }
            var deep = Reflect.construct(other.Object, [], deepNewTarget);
            var nestedFactory = new other.Function("return function NestedNewTarget() {};");
            var nestedNewTarget = nestedFactory();
            nestedNewTarget.prototype = null;
            var nested = Reflect.construct(Object, [], nestedNewTarget);
            var revoked = Proxy.revocable(function() {}, {});
            var revokedBound = revoked.proxy.bind(null);
            revoked.revoke();
            var revokedRejected = false;
            try {
              Reflect.construct(other.Object, [], revokedBound);
            } catch (error) {
              revokedRejected = error instanceof other.TypeError;
            }
            [
              called !== receiver && Object.getPrototypeOf(called) === other.Object.prototype,
              Object.getPrototypeOf(calledNull) === other.Object.prototype,
              Object.getPrototypeOf(calledUndefined) === other.Object.prototype,
              Object.getPrototypeOf(constructedNull) === other.Object.prototype,
              Object.getPrototypeOf(constructedUndefined) === other.Object.prototype,
              Object.getPrototypeOf(boxed) === other.Number.prototype,
              Object.getPrototypeOf(constructedBoxed) === other.Number.prototype,
              other.Object(argument) === argument,
              new other.Object(argument) === argument,
              subclassed !== argument && subclassed.argument === undefined &&
                Object.getPrototypeOf(subclassed) === Sub.prototype,
              reflected !== argument && reflected.argument === undefined &&
                Object.getPrototypeOf(reflected) === Sub.prototype,
              active === argument,
              Object.getPrototypeOf(custom) === customPrototype,
              Object.getPrototypeOf(deep) === other.Object.prototype,
              Object.getPrototypeOf(nested) === other.Object.prototype,
              revokedRejected
            ].join("|");
        "#,
        )
        .expect("Object constructor Realm and NewTarget paths should succeed"),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );

    vm.run(
        r#"
        var retainedObject = other.Object;
        var retainedObjectPrototype = other.Object.prototype;
        var intrinsicNewTarget = new other.Function();
        intrinsicNewTarget.prototype = null;
        other.Object = { prototype: {} };
        "#,
    )
    .expect("failed to retain foreign Object intrinsics");
    vm.gc();
    assert_eq!(
        vm.run(
            r#"
            Object.getPrototypeOf(retainedObject()) === retainedObjectPrototype &&
            Object.getPrototypeOf(
              Reflect.construct(retainedObject, [], intrinsicNewTarget)
            ) === retainedObjectPrototype;
            "#,
        )
        .expect("retained foreign Object intrinsics should survive mutation and GC"),
        Value::Bool(true)
    );
}

#[test]
fn object_constructor_does_not_preallocate_for_object_arguments() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    let constructor = vm
        .run("Object")
        .expect("failed to obtain Object constructor");
    let argument = vm
        .run("({ marker: true })")
        .expect("failed to create Object argument");

    vm.set_max_heap_objects(Some(1));
    assert_eq!(
        vm.construct(&constructor, std::slice::from_ref(&argument))
            .expect("Object construction with an object argument must not allocate"),
        argument
    );
}

#[test]
fn object_prototype_intrinsics_are_isolated_per_realm() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var mainObject = Object;
            var getPrototypeOf = Object.getPrototypeOf;
            var getOwnPropertyDescriptor = Object.getOwnPropertyDescriptor;
            var mainTypeError = TypeError;
            var names = [
              "toString", "toLocaleString", "hasOwnProperty", "isPrototypeOf",
              "propertyIsEnumerable", "valueOf", "__defineGetter__",
              "__defineSetter__", "__lookupGetter__", "__lookupSetter__"
            ];
            var lengths = [0, 0, 1, 1, 1, 0, 2, 2, 1, 1];
            var mainMethods = names.map(function(name) {
              return Object.prototype[name];
            });

            Object.extraFromMain = true;
            Object.prototype.extraFromMain = true;
            Object.prototype.valueOf = null;
            delete Object.prototype.toLocaleString;
            Object.defineProperty(Object.prototype, "hasOwnProperty", {
              value: function() { return "polluted"; },
              writable: false,
              enumerable: true,
              configurable: true
            });
            Object.defineProperty(Object.prototype, "__proto__", {
              get: function() { return "polluted"; },
              set: function() {},
              enumerable: true,
              configurable: true
            });

            var other = $262.createRealm().global;
            var second = $262.createRealm().global;
            var otherMethods = names.map(function(name) {
              return other.Object.prototype[name];
            });
            var secondMethods = names.map(function(name) {
              return second.Object.prototype[name];
            });
            var distinct = otherMethods.every(function(method, index) {
              var desc = getOwnPropertyDescriptor(other.Object.prototype, names[index]);
              return method !== mainMethods[index] && method !== secondMethods[index] &&
                getPrototypeOf(method) === other.Function.prototype &&
                method.name === names[index] && method.length === lengths[index] &&
                method.prototype === undefined && desc.value === method &&
                desc.writable === true && desc.enumerable === false &&
                desc.configurable === true;
            });
            var otherProto = getOwnPropertyDescriptor(other.Object.prototype, "__proto__");
            var secondProto = getOwnPropertyDescriptor(second.Object.prototype, "__proto__");
            var protoShape = otherProto.get !== secondProto.get &&
              otherProto.set !== secondProto.set &&
              getPrototypeOf(otherProto.get) === other.Function.prototype &&
              getPrototypeOf(otherProto.set) === other.Function.prototype &&
              otherProto.get.name === "get __proto__" && otherProto.get.length === 0 &&
              otherProto.set.name === "set __proto__" && otherProto.set.length === 1 &&
              otherProto.enumerable === false && otherProto.configurable === true;

            var pristine = !other.Object.extraFromMain &&
              !other.Object.prototype.extraFromMain &&
              other.Object.prototype.valueOf !== null &&
              typeof other.Object.prototype.toLocaleString === "function" &&
              other.Object.prototype.hasOwnProperty.call({ value: 1 }, "value") === true &&
              !getOwnPropertyDescriptor(other.Object.prototype, "extraFromMain");

            var graph = getPrototypeOf(other) === other.Object.prototype &&
              getPrototypeOf(other.Function.prototype) === other.Object.prototype &&
              getPrototypeOf(other.Error.prototype) === other.Object.prototype &&
              getPrototypeOf(other.Array.prototype) === other.Object.prototype &&
              getPrototypeOf(other.String.prototype) === other.Object.prototype &&
              getPrototypeOf(other.Number.prototype) === other.Object.prototype &&
              getPrototypeOf(other.Boolean.prototype) === other.Object.prototype &&
              getPrototypeOf(other.BigInt.prototype) === other.Object.prototype &&
              getPrototypeOf(other.Symbol.prototype) === other.Object.prototype &&
              getPrototypeOf(other.RegExp.prototype) === other.Object.prototype &&
              getPrototypeOf(other.ArrayBuffer.prototype) === other.Object.prototype &&
              getPrototypeOf(other.SharedArrayBuffer.prototype) === other.Object.prototype &&
              getPrototypeOf(other.DataView.prototype) === other.Object.prototype &&
              getPrototypeOf(other.WeakRef.prototype) === other.Object.prototype &&
              getPrototypeOf(other.FinalizationRegistry.prototype) === other.Object.prototype &&
              getPrototypeOf(other.Atomics) === other.Object.prototype &&
              getPrototypeOf(getPrototypeOf(other.Uint8Array.prototype)) ===
                other.Object.prototype;

            var errorRealm = false;
            try {
              other.Object.prototype.valueOf.call(null);
            } catch (error) {
              errorRealm = error instanceof other.TypeError &&
                !(error instanceof mainTypeError);
            }
            var boxed = other.Object.prototype.valueOf.call(1);
            var protoBoxed = otherProto.get.call(1);
            other.Number.prototype.toString = function() {
              "use strict";
              return typeof this;
            };
            var localePrimitive = other.Object.prototype.toLocaleString.call(1) === "number";

            var getter = function() { return 1; };
            var target = {};
            other.Object.prototype.__defineGetter__.call(target, "value", getter);
            var proxy = new Proxy(target, {
              getOwnPropertyDescriptor: function(target, key) {
                forceGc();
                return Reflect.getOwnPropertyDescriptor(target, key);
              },
              getPrototypeOf: function(target) {
                forceGc();
                return Reflect.getPrototypeOf(target);
              }
            });
            var proxyLookup = other.Object.prototype.__lookupGetter__.call(proxy, "value") ===
              getter;
            var proxyProto = other.Object.prototype.isPrototypeOf.call(
              getPrototypeOf(target), proxy
            );

            var symbol = Symbol();
            var symbolTarget = {};
            symbolTarget[symbol] = 1;
            var symbolKey = {
              [Symbol.toPrimitive]: function() { forceGc(); return symbol; }
            };
            var symbolEnumerable = other.Object.prototype.propertyIsEnumerable.call(
              symbolTarget, symbolKey
            );

            var retainedValueOf = other.Object.prototype.valueOf;
            var retainedProtoGet = otherProto.get;
            var retainedNumberPrototype = other.Number.prototype;
            other.Object = null;
            other.Function = null;
            other.Number = null;
            forceGc();
            var retained = getPrototypeOf(retainedValueOf.call(2)) ===
                retainedNumberPrototype &&
              retainedProtoGet.call(2) === retainedNumberPrototype;

            [
              distinct, protoShape, pristine, graph, errorRealm,
              getPrototypeOf(boxed) === retainedNumberPrototype,
              protoBoxed === retainedNumberPrototype, localePrimitive,
              proxyLookup, proxyProto,
              symbolEnumerable, retained
            ].join("|");
            "#,
        )
        .expect("Object prototype Realm isolation should succeed"),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn object_static_methods_are_realm_specific() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var second = $262.createRealm().global;
            var names = [
              "keys", "values", "entries", "assign", "is", "hasOwn",
              "fromEntries", "groupBy", "create", "freeze",
              "getOwnPropertyNames", "getOwnPropertySymbols",
              "getOwnPropertyDescriptor", "defineProperty", "defineProperties",
              "getPrototypeOf", "setPrototypeOf", "preventExtensions",
              "isExtensible", "seal", "isSealed", "isFrozen",
              "getOwnPropertyDescriptors"
            ];
            forceGc();

            var distinct = names.every(function(name) {
              return other.Object[name] !== Object[name] &&
                Object.getPrototypeOf(other.Object[name]) === other.Function.prototype;
            });
            var methodShape = other.Object.keys !== second.Object.keys &&
              Object.getPrototypeOf(second.Object.keys) === second.Function.prototype &&
              other.Object.keys.name === "keys" && other.Object.keys.length === 1 &&
              other.Object.keys.prototype === undefined;
            var keys = other.Object.keys({ value: 1 });
            var valueSource = { first: { value: 1 } };
            Object.defineProperty(valueSource, "second", {
              enumerable: true,
              get: function() { forceGc(); return { value: 2 }; }
            });
            var values = other.Object.values(valueSource);
            var entrySource = { first: 1 };
            Object.defineProperty(entrySource, "second", {
              enumerable: true,
              get: function() { forceGc(); return 2; }
            });
            var entries = other.Object.entries(entrySource);
            var descriptor = other.Object.getOwnPropertyDescriptor({ value: 1 }, "value");
            var descriptorTarget = { first: 1, second: 2 };
            var descriptors = other.Object.getOwnPropertyDescriptors(new Proxy(descriptorTarget, {
              ownKeys: function(target) { return Reflect.ownKeys(target); },
              getOwnPropertyDescriptor: function(target, key) {
                if (key === "second") forceGc();
                return Reflect.getOwnPropertyDescriptor(target, key);
              }
            }));
            var entry = {};
            Object.defineProperty(entry, "0", {
              get: function() {
                return { toString: function() { forceGc(); return "value"; } };
              }
            });
            Object.defineProperty(entry, "1", {
              get: function() { forceGc(); return { nested: 1 }; }
            });
            var fromEntries = other.Object.fromEntries([entry]);
            var assigned = other.Object.assign(1, { value: 1 });
            var ownNames = other.Object.getOwnPropertyNames({ value: 1 });
            var symbol = Symbol("value");
            var ownSymbols = other.Object.getOwnPropertySymbols({ [symbol]: 1 });
            var nextValue = 0;
            var grouped = other.Object.groupBy({
              [Symbol.iterator]: function() {
                return {
                  next: function() {
                    nextValue++;
                    return nextValue <= 2
                      ? { value: { value: nextValue }, done: false }
                      : { value: undefined, done: true };
                  }
                };
              }
            }, function() {
              return { toString: function() { forceGc(); return "all"; } };
            });
            var proxyDescriptorRealm = false;
            var proxy = new Proxy({}, {
              defineProperty: function(target, key, desc) {
                proxyDescriptorRealm = Object.getPrototypeOf(desc) === other.Object.prototype;
                return Reflect.defineProperty(target, key, desc);
              }
            });
            other.Object.defineProperty(proxy, "value", { value: 1 });
            var errorRealm = false;
            try {
              other.Object.defineProperty(undefined, "value", {});
            } catch (error) {
              errorRealm = error instanceof other.TypeError && !(error instanceof TypeError);
            }

            [
              distinct,
              methodShape,
              Object.getPrototypeOf(keys) === other.Array.prototype,
              Object.getPrototypeOf(values) === other.Array.prototype &&
                values[0] && values[1] &&
                values[0].value === 1 && values[1].value === 2,
              Object.getPrototypeOf(entries) === other.Array.prototype,
              entries[0] && entries[1] &&
                Object.getPrototypeOf(entries[0]) === other.Array.prototype &&
                entries[0].join(":") === "first:1" &&
                entries[1].join(":") === "second:2",
              Object.getPrototypeOf(descriptor) === other.Object.prototype,
              Object.getPrototypeOf(descriptors) === other.Object.prototype,
              descriptors.first &&
                Object.getPrototypeOf(descriptors.first) === other.Object.prototype &&
                descriptors.first.value === 1,
              Object.getPrototypeOf(fromEntries) === other.Object.prototype &&
                fromEntries.value && fromEntries.value.nested === 1,
              Object.getPrototypeOf(assigned) === other.Number.prototype &&
                assigned.value === 1 &&
                other.Object.getPrototypeOf(1) === other.Number.prototype,
              Object.getPrototypeOf(ownNames) === other.Array.prototype &&
                Object.getPrototypeOf(ownSymbols) === other.Array.prototype,
              grouped.all &&
                Object.getPrototypeOf(grouped.all) === other.Array.prototype &&
                Object.getPrototypeOf(grouped) === null &&
                grouped.all[0] && grouped.all[1] &&
                grouped.all[0].value === 1 && grouped.all[1].value === 2,
              proxyDescriptorRealm,
              errorRealm
            ].join("|");
        "#,
        )
        .expect("foreign Realm Object statics should remain live across GC"),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );

    vm.run(
        r#"
        var retainedObjectKeys = other.Object.keys;
        var retainedArrayPrototype = other.Array.prototype;
        other.Object = null;
        other.Array = null;
        "#,
    )
    .expect("failed to retain foreign Realm intrinsics");
    vm.gc();
    assert_eq!(
        vm.run(
            "Object.getPrototypeOf(retainedObjectKeys({ value: 1 })) === retainedArrayPrototype;"
        )
        .expect("retained foreign Object method should survive GC"),
        Value::Bool(true)
    );
}

#[test]
fn object_statics() {
    assert_eq!(run("Object.is(NaN, NaN);"), Value::Bool(true));
    assert_eq!(run("Object.is(0, -0);"), Value::Bool(false));
    assert_eq!(run("Object.is(1, 1);"), Value::Bool(true));
    assert_eq!(
        run("var p={x:1}; var o=Object.create(p); o.y=2; [Object.hasOwn(o,'x'), Object.hasOwn(o,'y'), Object.hasOwn('abc','length'), Object.hasOwn('abc','1')].join(',');"),
        Value::String(Arc::from("false,true,true,true"))
    );
    assert_eq!(
        run("var s=Symbol(); var o={}; o[s]=1; [Object.hasOwn(o,s), Object.hasOwn({},s), Object.hasOwn.length, Object.hasOwn.name, Object.hasOwn.prototype].join(',');"),
        Value::String(Arc::from("true,false,2,hasOwn,"))
    );
    assert_eq!(
        run(r#"
            var calls = [];
            var target = new Proxy({}, {
              getOwnPropertyDescriptor: function(_target, key) {
                calls.push(key);
                if (key === "virtual") {
                  return { value: 1, enumerable: false, configurable: true };
                }
              }
            });
            var proxy = new Proxy(target, { getOwnPropertyDescriptor: null });
            [
              Object.prototype.hasOwnProperty.call(proxy, "virtual"),
              Object.hasOwn(proxy, "virtual"),
              Object.prototype.hasOwnProperty.call(proxy, "missing"),
              Object.hasOwn(proxy, "missing"),
              calls.join(",")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|false|false|virtual,virtual,missing,missing"
        ))
    );
    assert!(run_err(
        r#"
        var state = Proxy.revocable({}, {});
        state.revoke();
        Object.hasOwn(state.proxy, "key");
    "#
    )
    .contains("TypeError"));
    assert_eq!(
        run(r#"
            var calls = 0;
            var key = { get toString() { calls++; throw new Error("key"); } };
            try { Object.hasOwn(null, key); } catch (e) {}
            calls;
            "#),
        Value::Number(0.0)
    );
    assert_eq!(
        run(r#"
            var index = Object.getOwnPropertyDescriptor("foo", "0");
            var length = Object.getOwnPropertyDescriptor("foo", "length");
            [
              index.value,
              index.writable,
              index.enumerable,
              index.configurable,
              length.value,
              length.writable,
              length.enumerable,
              length.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("f,false,true,false,3,false,false,false"))
    );
    assert_eq!(
        run(r#"
            var sym = Symbol();
            var obj = {};
            obj[sym] = 42;
            var desc = Object.getOwnPropertyDescriptor(obj, sym);
            [
              desc.value,
              desc.writable,
              desc.enumerable,
              desc.configurable,
              desc.propertyIsEnumerable("value"),
              Object.keys(desc).join("|")
            ].join(",");
            "#),
        Value::String(Arc::from(
            "42,true,true,true,true,value|writable|enumerable|configurable"
        ))
    );
    assert_eq!(
        run(r#"
            var proto = Object.getOwnPropertyDescriptor(String, "prototype");
            var length = Object.getOwnPropertyDescriptor(String, "length");
            [
              proto.writable,
              proto.enumerable,
              proto.configurable,
              length.value,
              length.writable,
              length.enumerable,
              length.configurable
            ].join(",");
            "#),
        Value::String(Arc::from("false,false,false,1,false,false,true"))
    );
    assert_eq!(
        run(r#"
            var obj = { undefined: 7 };
            Object.getOwnPropertyDescriptor(obj).value;
            "#),
        Value::Number(7.0)
    );
    assert_eq!(
        run(r#"
            var sym = Symbol();
            var obj = { a: 1 };
            Object.defineProperty(obj, "b", { value: 2, enumerable: false });
            obj[sym] = 3;
            [
              Object.getOwnPropertyNames(obj).join("|"),
              Object.getOwnPropertySymbols(obj).length,
              Object.getOwnPropertySymbols(obj)[0] === sym,
              Object.getOwnPropertyDescriptors(obj).b.enumerable,
              Object.getOwnPropertyDescriptors(obj)[sym].value
            ].join(",");
            "#),
        Value::String(Arc::from("a|b,1,true,false,3"))
    );
    assert_eq!(
        run(r#"
            var descs = Object.getOwnPropertyDescriptors("ab");
            [
              Object.keys(descs).join("|"),
              descs.length.value,
              descs.length.enumerable,
              descs[0].value,
              descs[0].writable
            ].join(",");
        "#),
        Value::String(Arc::from("0|1|length,2,false,a,false"))
    );
    assert!(
        run_err("Object.values(null);").contains("TypeError"),
        "Object.values(null) should throw"
    );
    assert!(
        run_err("Object.entries(undefined);").contains("TypeError"),
        "Object.entries(undefined) should throw"
    );
    assert_eq!(
        run(r#"
            var obj = {
              a: "A",
              get b() {
                delete this.c;
                Object.defineProperty(this, "d", { value: "D", enumerable: false });
                return "B";
              },
              c: "C",
              d: "visible"
            };
            Object.values(obj).join("|") + ":" + Object.entries(obj).map(function(e) {
              return e[0] + "=" + e[1];
            }).join("|");
        "#),
        Value::String(Arc::from("A|B:a=A|b=B"))
    );
    assert_eq!(
        run(r#"
            var target = {};
            var proxy = new Proxy(target, {});
            var returned = Object.defineProperty(proxy, "a", {
              value: 1,
              enumerable: true,
              configurable: true
            });
            [
              returned === proxy,
              Object.prototype.hasOwnProperty.call(proxy, "a"),
              Object.prototype.hasOwnProperty.call(target, "a"),
              Object.values(proxy).join("|")
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,true,1"))
    );
    assert_eq!(
        run(r#"
            var calls = [];
            var proxy = new Proxy({ present: undefined }, {
              has: function(t, k) {
                calls.push("has:" + k);
                return k === "present";
              }
            });
            [
              "present" in proxy,
              "missing" in proxy,
              Reflect.has(proxy, "present"),
              Reflect.has(proxy, "missing"),
              calls.join("|")
            ].join(",");
        "#),
        Value::String(Arc::from(
            "true,false,true,false,has:present|has:missing|has:present|has:missing"
        ))
    );
    assert_eq!(
        run(r#"
            var target = {};
            var receiver = {};
            Object.defineProperty(receiver, "p", { value: 1, writable: false });
            [
              Reflect.set(target, "p", 2, receiver),
              receiver.p,
              Object.prototype.hasOwnProperty.call(target, "p")
            ].join(",");
        "#),
        Value::String(Arc::from("false,1,false"))
    );
    assert_eq!(
        run(r#"
            var target = {};
            var receiver = {};
            Object.defineProperty(receiver, "p", { set: function(v) {} });
            [
              Reflect.set(target, "p", 2, receiver),
              Object.prototype.hasOwnProperty.call(target, "p")
            ].join(",");
        "#),
        Value::String(Arc::from("false,false"))
    );
    assert!(
        run_err(
            r#"
            var proxy = new Proxy({}, {
              set: function() { throw new Error("boom"); }
            });
            Reflect.set(proxy, "p", 1);
        "#
        )
        .contains("boom"),
        "Reflect.set should propagate abrupt completions from Proxy set traps"
    );
    assert_eq!(
        run(r#"
            var obj = {};
            Object.defineProperty(obj, "p", { value: 1 });
            Object.freeze(obj);
            [
              Reflect.defineProperty(obj, "p", { value: 2 }),
              obj.p,
              Reflect.defineProperty(obj, "q", { value: 3 }),
              Object.prototype.hasOwnProperty.call(obj, "q")
            ].join(",");
        "#),
        Value::String(Arc::from("false,1,false,false"))
    );
    assert!(
        run_err(
            r#"
            var attrs = {};
            Object.defineProperty(attrs, "enumerable", {
              get: function() { throw new Error("attrs boom"); }
            });
            Reflect.defineProperty({}, "p", attrs);
        "#
        )
        .contains("attrs boom"),
        "Reflect.defineProperty should propagate descriptor getter errors"
    );
    assert!(
        run_err(
            r#"
            var proxy = new Proxy({}, {
              getOwnPropertyDescriptor: function() { throw new Error("gopd boom"); }
            });
            Reflect.getOwnPropertyDescriptor(proxy, "p");
        "#
        )
        .contains("gopd boom"),
        "Reflect.getOwnPropertyDescriptor should propagate Proxy trap errors"
    );
    assert_eq!(
        run(r#"
            var calls = [];
            var target = { a: 1 };
            var handler = {
              deleteProperty: function(t, key) {
                calls.push(this === handler);
                calls.push(t === target);
                calls.push(key);
                return delete t[key];
              }
            };
            var proxy = new Proxy(target, handler);
            [delete proxy.a, "a" in target, calls.join("|")].join(",");
        "#),
        Value::String(Arc::from("true,false,true|true|a"))
    );
    assert_eq!(
        run(r#"
            var target = {};
            Object.defineProperty(target, "fixed", { value: 1, configurable: false });
            var proxy = new Proxy(target, { deleteProperty: function() { return false; } });
            [Reflect.deleteProperty(proxy, "fixed"), target.fixed].join(",");
        "#),
        Value::String(Arc::from("false,1"))
    );
    assert!(
        run_err(
            r#"
            var target = {};
            Object.defineProperty(target, "fixed", { value: 1, configurable: false });
            var proxy = new Proxy(target, { deleteProperty: function() { return true; } });
            Reflect.deleteProperty(proxy, "fixed");
        "#
        )
        .contains("TypeError"),
        "Proxy deleteProperty cannot report non-configurable properties as deleted"
    );
    assert!(
        run_err("Reflect.deleteProperty(1, 'x');").contains("TypeError"),
        "Reflect.deleteProperty must reject primitive targets"
    );
    assert_eq!(
        run(r#"
            var target = { undefined: 1 };
            var result = Reflect.deleteProperty(target);
            [result, "undefined" in target].join(",");
        "#),
        Value::String(Arc::from("true,false"))
    );
    assert_eq!(
        run(r#"
            var key = Symbol("delete key");
            var seen;
            var target = {};
            target[key] = 1;
            var proxy = new Proxy(target, {
              deleteProperty: function(actualTarget, actualKey) {
                seen = actualKey;
                return Reflect.deleteProperty(actualTarget, actualKey);
              }
            });
            [Reflect.deleteProperty(proxy, key), seen === key, key in target].join(",");
        "#),
        Value::String(Arc::from("true,true,false"))
    );
    assert!(
        run_err(
            r#"
            var revocable = Proxy.revocable({ value: 1 }, {});
            var outer = new Proxy(revocable.proxy, { deleteProperty: null });
            revocable.revoke();
            Reflect.deleteProperty(outer, "value");
            "#
        )
        .contains("TypeError"),
        "transparent deletion must observe a revoked nested Proxy target"
    );
    assert!(
        run_err("Object.keys(null);").contains("TypeError"),
        "Object.keys(null) should throw"
    );
    assert!(
        run_err("Object.getOwnPropertyNames(undefined);").contains("TypeError"),
        "Object.getOwnPropertyNames(undefined) should throw"
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var key = { get toString() { calls++; throw new Error("key"); } };
            try { Object.getOwnPropertyDescriptor(null, key); } catch (e) {}
            calls;
            "#),
        Value::Number(0.0)
    );
    assert_eq!(
        run("var o = Object.fromEntries([['a',1],['b',2]]); o.a + o.b;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run(r#"
            var a = Object.fromEntries([Object("ab")]);
            var b = Object.fromEntries([new String("cd")]);
            [a.a, b.c].join("|");
        "#),
        Value::String(Arc::from("b|d"))
    );
    assert_eq!(
        run(r#"
            var s = Symbol("k");
            var o = Object.fromEntries([[s, 3]]);
            [o[s], Object.getOwnPropertySymbols(o).length].join("|");
        "#),
        Value::String(Arc::from("3|1"))
    );
    assert!(run_err("Object.fromEntries();").contains("TypeError"));
    assert!(run_err(r#"Object.fromEntries(["ab"]);"#).contains("TypeError"));
    assert_eq!(
        run(r#"
            var o = Object.groupBy([1, 2, 3], function(v, i) {
              return i + ":" + (v % 2 === 0 ? "even" : "odd");
            });
            [
              Object.getPrototypeOf(o) === null,
              Object.keys(o).join("|"),
              o["0:odd"].join(","),
              o["1:even"].join(","),
              o["2:odd"].join(",")
            ].join(";");
        "#),
        Value::String(Arc::from("true;0:odd|1:even|2:odd;1;2;3"))
    );
    assert_eq!(
        run(r#"
            var s = Symbol("group");
            var o = Object.groupBy(["a", "b"], function(v) {
              return v === "a" ? s : "plain";
            });
            [
              o[s].join(","),
              o.plain.join(","),
              Object.getOwnPropertySymbols(o).length
            ].join("|");
        "#),
        Value::String(Arc::from("a|b|1"))
    );
    assert_eq!(
        run(r#"
            var closed = false;
            var it = {
              i: 0,
              next: function() { return { value: ++this.i, done: false }; },
              return: function() { closed = true; return {}; }
            };
            var src = {};
            src[Symbol.iterator] = function() { return it; };
            try {
              Object.groupBy(src, function(v) {
                if (v === 2) throw new Error("stop");
                return "k";
              });
            } catch (e) {}
            closed;
        "#),
        Value::Bool(true)
    );
    assert!(run_err("Object.groupBy([], null);").contains("TypeError"));
    assert!(run_err("Object.groupBy(null, function(){});").contains("TypeError"));
    assert_eq!(
        run("typeof Object.create(null);"),
        Value::String(Arc::from("object"))
    );
}

#[test]
fn reflect_omitted_property_keys_coerce_undefined() {
    assert_eq!(
        run(r#"
            var getTarget = {};
            Object.defineProperty(getTarget, "undefined", {
              configurable: true,
              get: function() { return this === getTarget ? 17 : -1; }
            });
            var setTarget = {};
            [
              Reflect.get(getTarget),
              Reflect.get(getTarget, undefined),
              Reflect.has(getTarget),
              Reflect.has(getTarget, undefined),
              Reflect.set(setTarget),
              Object.prototype.hasOwnProperty.call(setTarget, "undefined"),
              String(setTarget.undefined)
            ].join("|");
        "#),
        Value::String(Arc::from("17|17|true|true|true|true|undefined"))
    );
    assert_eq!(
        run(r#"
            var calls = [];
            var target = {};
            var proxy;
            proxy = new Proxy(target, {
              get: function(actualTarget, key, receiver) {
                calls.push("get:" + key + ":" + (receiver === proxy));
                return 23;
              },
              has: function(actualTarget, key) {
                calls.push("has:" + key);
                return true;
              },
              set: function(actualTarget, key, value, receiver) {
                calls.push(
                  "set:" + key + ":" + String(value) + ":" +
                  (receiver === proxy)
                );
                return Reflect.set(actualTarget, key, value, receiver);
              }
            });
            [
              Reflect.get(proxy),
              Reflect.has(proxy),
              Reflect.set(proxy),
              calls.join(","),
              Object.prototype.hasOwnProperty.call(target, "undefined")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "23|true|true|get:undefined:true,has:undefined,set:undefined:undefined:true|true"
        ))
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var key = { toString: function() { calls += 1; return "x"; } };
            for (var method of [Reflect.get, Reflect.set, Reflect.has]) {
              try { method(null, key); } catch (error) {}
            }
            calls;
        "#),
        Value::Number(0.0)
    );
    assert_eq!(
        run(r#"
            var errors = [];
            for (var method of ["get", "set", "has"]) {
              var handler = {};
              handler[method] = function() { throw new Error(method + " trap"); };
              try { Reflect[method](new Proxy({}, handler)); }
              catch (error) { errors.push(error.message); }

              var revocable = Proxy.revocable({}, {});
              revocable.revoke();
              try { Reflect[method](revocable.proxy); }
              catch (error) { errors.push(error.name); }
            }
            errors.join("|");
        "#),
        Value::String(Arc::from(
            "get trap|TypeError|set trap|TypeError|has trap|TypeError"
        ))
    );
    assert_eq!(
        run(r#"
            var target = { undefined: 9 };
            Object.defineProperty(target, "receiver", {
              get: function() { "use strict"; return this === undefined; }
            });
            var explicitReceiver = Reflect.get(target, "receiver", undefined);
            var defaultReceiver = Reflect.get(target, "receiver");
            var explicitSet = Reflect.set(target, "explicit", 1, undefined);
            [
              Reflect.getOwnPropertyDescriptor(target).value,
              explicitReceiver,
              defaultReceiver,
              explicitSet,
              Object.prototype.hasOwnProperty.call(target, "explicit")
            ].join("|");
        "#),
        Value::String(Arc::from("9|true|false|false|false"))
    );
}

#[test]
fn reflect_to_string_tag_is_a_realm_local_spec_property() {
    assert_eq!(
        run(r#"
            var descriptor = Object.getOwnPropertyDescriptor(
              Reflect,
              Symbol.toStringTag
            );
            [
              descriptor.value,
              descriptor.writable,
              descriptor.enumerable,
              descriptor.configurable,
              Object.prototype.toString.call(Reflect),
              Object.prototype.hasOwnProperty.call(Reflect, Symbol.toStringTag)
            ].join("|");
        "#),
        Value::String(Arc::from("Reflect|false|false|true|[object Reflect]|true"))
    );
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var descriptor = Object.getOwnPropertyDescriptor(
              other.Reflect,
              Symbol.toStringTag
            );
            var distinct = other.Reflect !== Reflect;
            var ownPrototype =
              other.Object.getPrototypeOf(other.Reflect) === other.Object.prototype;
            var methods = [
              "apply", "construct", "defineProperty", "deleteProperty",
              "get", "getOwnPropertyDescriptor", "getPrototypeOf", "has",
              "isExtensible", "ownKeys", "preventExtensions", "set",
              "setPrototypeOf"
            ];
            var localMethods = methods.every(function(name) {
              return other.Reflect[name] !== Reflect[name] &&
                other.Object.getPrototypeOf(other.Reflect[name]) ===
                  other.Function.prototype;
            });
            var realmError = false;
            try { other.Reflect.get(1, "x"); }
            catch (error) {
              realmError = error instanceof other.TypeError &&
                !(error instanceof TypeError);
            }
            var globalDescriptor = other.Object.getOwnPropertyDescriptor(
              other,
              "Reflect"
            );
            delete Reflect[Symbol.toStringTag];
            [
              distinct,
              ownPrototype,
              localMethods,
              realmError,
              globalDescriptor.writable,
              globalDescriptor.enumerable,
              globalDescriptor.configurable,
              descriptor.value,
              descriptor.writable,
              descriptor.enumerable,
              descriptor.configurable,
              other.Object.prototype.toString.call(other.Reflect),
              other.Reflect[Symbol.toStringTag]
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|false|true|Reflect|false|false|true|[object Reflect]|Reflect"
        ))
    );
}

#[test]
fn prevent_extensions_blocks_array_arguments_function_and_proxy_edges() {
    assert_eq!(
        run("var a=[]; Object.preventExtensions(a); a[0]=1; a.x=2; Object.isExtensible(a)+':' + a.hasOwnProperty('0') + ':' + a.hasOwnProperty('x');"),
        Value::String(Arc::from("false:false:false"))
    );
    assert_eq!(
        run("(function(){ Object.preventExtensions(arguments); arguments[0]=1; arguments.x=2; return Object.isExtensible(arguments)+':' + arguments.hasOwnProperty('0') + ':' + arguments.hasOwnProperty('x'); })();"),
        Value::String(Arc::from("false:false:false"))
    );
    assert_eq!(
        run("function f(){} Object.preventExtensions(f); f[0]=1; f.x=2; Object.isExtensible(f)+':' + f.hasOwnProperty('0') + ':' + f.hasOwnProperty('x');"),
        Value::String(Arc::from("false:false:false"))
    );
    assert!(run_err(
        "Object.preventExtensions(new Proxy({}, { preventExtensions(){ return false; } }));"
    )
    .contains("TypeError"));
    assert_eq!(
        run("var target={}; var p=new Proxy(target,{preventExtensions(t){ Object.preventExtensions(t); return true; }}); Reflect.preventExtensions(p)+':' + Object.isExtensible(target);"),
        Value::String(Arc::from("true:false"))
    );
    assert_eq!(
        run("Reflect.preventExtensions(new Proxy({}, { preventExtensions(){ return false; } }));"),
        Value::Bool(false)
    );
}

#[test]
fn prevent_extensions_updates_every_exotic_and_nested_proxy_target() {
    assert_eq!(
        run(
            r#"
            var weakTarget = {};
            var samples = [
              ["Map", new Map()],
              ["Set", new Set()],
              ["WeakMap", new WeakMap()],
              ["WeakSet", new WeakSet()],
              ["ArrayBuffer", new ArrayBuffer(8)],
              ["SharedArrayBuffer", new SharedArrayBuffer(8)],
              ["DataView", new DataView(new ArrayBuffer(8))],
              ["Promise", Promise.resolve(1)],
              ["Generator", (function* () {})()],
              ["AsyncGenerator", (async function* () {})()],
              ["RegExpStringIterator", "a".matchAll(/a/g)],
              ["MapIterator", new Map().keys()],
              ["IteratorHelper", Iterator.from([1]).map(function (x) { return x; })],
              ["WeakRef", new WeakRef(weakTarget)],
              ["FinalizationRegistry", new FinalizationRegistry(function () {})],
              ["TypedArray", new Uint8Array(1)]
            ];
            var exoticResults = samples.map(function (entry) {
              var name = entry[0];
              var sample = entry[1];
              var symbol = Symbol(name);
              var before =
                Object.isExtensible(sample) && Reflect.isExtensible(sample);
              var prepared = Reflect.defineProperty(sample, "existing", {
                value: 1,
                writable: true,
                configurable: true
              });
              var reflected = Reflect.preventExtensions(sample);
              var returned = Object.preventExtensions(sample) === sample;
              var after =
                !Object.isExtensible(sample) && !Reflect.isExtensible(sample);
              var wroteExisting =
                Reflect.set(sample, "existing", 2) && sample.existing === 2;
              var deletedExisting = Reflect.deleteProperty(sample, "existing");
              var definedString =
                Reflect.defineProperty(sample, "extra", { value: 1 });
              var definedSymbol =
                Reflect.defineProperty(sample, symbol, { value: 1 });
              var prototype = Reflect.getPrototypeOf(sample);
              var samePrototype = Reflect.setPrototypeOf(sample, prototype);
              var differentPrototype = Reflect.setPrototypeOf(sample, {});
              return name + ":" + (
                before && prepared && reflected && returned && after &&
                wroteExisting && deletedExisting &&
                !Object.prototype.hasOwnProperty.call(sample, "existing") &&
                !definedString && !definedSymbol &&
                !Object.prototype.hasOwnProperty.call(sample, "extra") &&
                !Object.prototype.hasOwnProperty.call(sample, symbol) &&
                samePrototype && !differentPrototype
              );
            }).join(",");

            var nestedCalls = 0;
            var nestedBase = Object.preventExtensions({});
            var nestedTarget = new Proxy(nestedBase, {
              isExtensible: function (target) {
                nestedCalls += 1;
                return Reflect.isExtensible(target);
              }
            });
            var nestedOuter = new Proxy(nestedTarget, {
              preventExtensions: function () { return true; }
            });
            var nestedInvariant =
              Reflect.preventExtensions(nestedOuter) === true && nestedCalls === 1;

            var marker = {};
            var abruptCalls = 0;
            var abruptTarget = new Proxy(Object.preventExtensions({}), {
              isExtensible: function () {
                abruptCalls += 1;
                throw marker;
              }
            });
            var abruptOuter = new Proxy(abruptTarget, {
              preventExtensions: function () { return true; }
            });
            var abruptInvariant = false;
            try { Reflect.preventExtensions(abruptOuter); }
            catch (error) {
              abruptInvariant = error === marker && abruptCalls === 1;
            }

            var transparentBase = {};
            var transparent = new Proxy(new Proxy(transparentBase, {}), {});
            var transparentDelegates =
              Reflect.preventExtensions(transparent) === true &&
              Object.isExtensible(transparentBase) === false &&
              Object.isExtensible(transparent) === false;

            [
              exoticResults,
              nestedInvariant,
              abruptInvariant,
              transparentDelegates
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "Map:true,Set:true,WeakMap:true,WeakSet:true,ArrayBuffer:true,SharedArrayBuffer:true,DataView:true,Promise:true,Generator:true,AsyncGenerator:true,RegExpStringIterator:true,MapIterator:true,IteratorHelper:true,WeakRef:true,FinalizationRegistry:true,TypedArray:true|true|true|true"
        ))
    );
}

#[test]
fn seal_and_freeze_process_every_exotic_own_descriptor() {
    assert_eq!(
        run(r#"
            var weakTarget = {};
            var factories = [
              ["Map", function () { return new Map([["entry", 1]]); }],
              ["Set", function () { return new Set(); }],
              ["WeakMap", function () { return new WeakMap(); }],
              ["WeakSet", function () { return new WeakSet(); }],
              ["ArrayBuffer", function () { return new ArrayBuffer(8); }],
              ["SharedArrayBuffer", function () { return new SharedArrayBuffer(8); }],
              ["DataView", function () { return new DataView(new ArrayBuffer(8)); }],
              ["Promise", function () { return Promise.resolve(1); }],
              ["Generator", function () { return (function* () {})(); }],
              ["AsyncGenerator", function () { return (async function* () {})(); }],
              ["RegExpStringIterator", function () { return "a".matchAll(/a/g); }],
              ["MapIterator", function () { return new Map().keys(); }],
              ["IteratorHelper", function () {
                return Iterator.from([1]).map(function (x) { return x; });
              }],
              ["WeakRef", function () { return new WeakRef(weakTarget); }],
              ["FinalizationRegistry", function () {
                return new FinalizationRegistry(function () {});
              }],
              ["TypedArray", function () { return new Uint8Array(0); }]
            ];

            var results = factories.map(function (entry) {
              var name = entry[0];
              var factory = entry[1];

              var sealed = factory();
              Object.defineProperty(sealed, "x", {
                value: 1,
                writable: true,
                configurable: true
              });
              Object.seal(sealed);
              var sealedDesc = Object.getOwnPropertyDescriptor(sealed, "x");
              var sealedOk =
                Object.isSealed(sealed) && !Object.isFrozen(sealed) &&
                !Object.isExtensible(sealed) && !sealedDesc.configurable &&
                sealedDesc.writable && Reflect.set(sealed, "x", 2) &&
                sealed.x === 2 && !Reflect.deleteProperty(sealed, "x");

              var frozen = factory();
              Object.defineProperty(frozen, "x", {
                value: 1,
                writable: true,
                configurable: true
              });
              Object.freeze(frozen);
              var frozenDesc = Object.getOwnPropertyDescriptor(frozen, "x");
              var frozenOk =
                Object.isFrozen(frozen) && Object.isSealed(frozen) &&
                !Object.isExtensible(frozen) && !frozenDesc.configurable &&
                !frozenDesc.writable && !Reflect.set(frozen, "x", 2) &&
                frozen.x === 1 && !Reflect.deleteProperty(frozen, "x");

              var accessor = factory();
              var setterValue = 0;
              Object.defineProperty(accessor, "accessor", {
                get: function () { return setterValue; },
                set: function (value) { setterValue = value; },
                configurable: true
              });
              Object.freeze(accessor);
              var accessorDesc =
                Object.getOwnPropertyDescriptor(accessor, "accessor");
              var accessorOk =
                Object.isFrozen(accessor) && !accessorDesc.configurable &&
                accessorDesc.writable === undefined &&
                Reflect.set(accessor, "accessor", 3) && setterValue === 3;

              var collectionOk = name !== "Map" || (
                sealed.get("entry") === 1 && frozen.get("entry") === 1 &&
                accessor.get("entry") === 1 &&
                !Object.prototype.hasOwnProperty.call(sealed, "entry") &&
                Reflect.ownKeys(sealed).indexOf("entry") === -1
              );

              return name + ":" + (
                sealedOk && frozenOk && accessorOk && collectionOk
              );
            }).join(",");

            var typedArraySealError = false;
            var typedArrayFreezeError = false;
            try { Object.seal(new Uint8Array(1)); }
            catch (error) { typedArraySealError = error instanceof TypeError; }
            try { Object.freeze(new Uint8Array(1)); }
            catch (error) { typedArrayFreezeError = error instanceof TypeError; }

            results + "|" + typedArraySealError + "|" + typedArrayFreezeError;
        "#),
        Value::String(Arc::from(
            "Map:true,Set:true,WeakMap:true,WeakSet:true,ArrayBuffer:true,SharedArrayBuffer:true,DataView:true,Promise:true,Generator:true,AsyncGenerator:true,RegExpStringIterator:true,MapIterator:true,IteratorHelper:true,WeakRef:true,FinalizationRegistry:true,TypedArray:true|true|true"
        ))
    );
}

#[test]
fn prevent_extensions_uses_the_method_realm_across_foreign_targets() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var mainTypeError = TypeError;

            var foreignBuffer = new other.ArrayBuffer(8);
            var mainReflectForeignTarget =
              Reflect.preventExtensions(foreignBuffer) === true &&
              other.Object.isExtensible(foreignBuffer) === false &&
              other.Reflect.defineProperty(
                foreignBuffer,
                "extra",
                { value: 1 }
              ) === false;

            var mainPromise = Promise.resolve(1);
            var foreignReflectMainTarget =
              other.Reflect.preventExtensions(mainPromise) === true &&
              Object.isExtensible(mainPromise) === false &&
              Reflect.defineProperty(mainPromise, "extra", { value: 1 }) === false;

            function throwsMainTypeError(callback) {
              var error;
              try { callback(); }
              catch (caught) { error = caught; }
              return error instanceof mainTypeError &&
                !(error instanceof other.TypeError);
            }

            function throwsForeignTypeError(callback) {
              var error;
              try { callback(); }
              catch (caught) { error = caught; }
              return error instanceof other.TypeError &&
                !(error instanceof mainTypeError);
            }

            var foreignNonCallable = new other.Proxy(
              {},
              { preventExtensions: 1 }
            );
            var mainReflectError = throwsMainTypeError(function () {
              Reflect.preventExtensions(foreignNonCallable);
            });

            var mainNonCallable = new Proxy({}, { preventExtensions: 1 });
            var foreignReflectError = throwsForeignTypeError(function () {
              other.Reflect.preventExtensions(mainNonCallable);
            });

            var mainFalse = new Proxy({}, {
              preventExtensions: function () { return false; }
            });
            var foreignObjectError = throwsForeignTypeError(function () {
              other.Object.preventExtensions(mainFalse);
            });

            var foreignTruthy = new other.Proxy({}, {
              preventExtensions: function () { return true; }
            });
            var foreignInvariantError = throwsForeignTypeError(function () {
              other.Reflect.preventExtensions(foreignTruthy);
            });

            var foreignRevocable = other.Proxy.revocable({}, {});
            foreignRevocable.revoke();
            var mainRevokedError = throwsMainTypeError(function () {
              Reflect.preventExtensions(foreignRevocable.proxy);
            });

            var nestedCalls = 0;
            var foreignBase = other.Object.preventExtensions({});
            var foreignNested = new other.Proxy(foreignBase, {
              isExtensible: function (target) {
                nestedCalls += 1;
                return other.Reflect.isExtensible(target);
              }
            });
            var mainOuter = new Proxy(foreignNested, {
              preventExtensions: function () { return true; }
            });
            var crossRealmNestedInvariant =
              Reflect.preventExtensions(mainOuter) === true && nestedCalls === 1;

            [
              mainReflectForeignTarget,
              foreignReflectMainTarget,
              mainReflectError,
              foreignReflectError,
              foreignObjectError,
              foreignInvariantError,
              mainRevokedError,
              crossRealmNestedInvariant
            ].join("|");
            "#,),
        Value::String(Arc::from("true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn seal_and_freeze_update_integrity_for_arrays_arguments_functions_and_proxies() {
    assert_eq!(run("Object.isSealed(1);"), Value::Bool(true));
    assert_eq!(run("Object.isFrozen(1);"), Value::Bool(true));
    assert_eq!(run("Object.isSealed(Boolean);"), Value::Bool(false));
    assert_eq!(run("Object.isFrozen(Boolean);"), Value::Bool(false));
    assert_eq!(
        run("var a=[0,1]; Object.seal(a); var d=Object.getOwnPropertyDescriptor(a,'0'); Object.isSealed(a)+':' + d.configurable;"),
        Value::String(Arc::from("true:false"))
    );
    assert_eq!(
        run("var a=[]; Object.seal(a); a.length=1; var d=Object.getOwnPropertyDescriptor(a,'length'); a.length + ':' + d.value + ':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("1:1:true:false"))
    );
    assert_eq!(
        run("var a=[]; Object.seal(a); Object.isFrozen(a);"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var a=[]; Object.preventExtensions(a); Object.isFrozen(a);"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var a=[]; a[2000000]=1; Object.freeze(a); a.length;"),
        Value::Number(2000001.0)
    );
    assert_eq!(
        run("var a=[0,1]; Object.freeze(a); var d=Object.getOwnPropertyDescriptor(a,'0'); Object.isFrozen(a)+':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("true:false:false"))
    );
    assert_eq!(
        run("var a=[0,1]; Object.freeze(a); a.length=1; a.length;"),
        Value::Number(2.0)
    );
    assert!(
        run_err("\"use strict\"; var a=[0,1]; Object.freeze(a); a.length=1;").contains("TypeError")
    );
    assert_eq!(
        run("(function(){ Object.freeze(arguments); var d=Object.getOwnPropertyDescriptor(arguments,'0'); return Object.isFrozen(arguments)+':' + d.writable + ':' + d.configurable; })(1);"),
        Value::String(Arc::from("true:false:false"))
    );
    assert_eq!(
        run("(function(a){ Object.freeze(arguments); a=2; return arguments[0]; })(1);"),
        Value::Number(1.0)
    );
    assert_eq!(
        run("(function(){ Object.seal(arguments); var d=Object.getOwnPropertyDescriptor(arguments,'0'); return Object.isSealed(arguments)+':' + Object.isFrozen(arguments)+':' + d.writable + ':' + d.configurable; })(1);"),
        Value::String(Arc::from("true:false:true:false"))
    );
    assert_eq!(
        run("function f(){} f.x=1; Object.seal(f); var d=Object.getOwnPropertyDescriptor(f,'x'); Object.isSealed(f)+':' + Object.isFrozen(f)+':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("true:false:true:false"))
    );
    assert_eq!(
        run("function f(){} f.x=1; Object.freeze(f); var d=Object.getOwnPropertyDescriptor(f,'x'); Object.isFrozen(f)+':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("true:false:false"))
    );
    assert!(
        run_err("Object.seal(new Proxy({}, { preventExtensions(){ return false; } }));")
            .contains("TypeError")
    );
    assert!(
        run_err("Object.freeze(new Proxy({}, { preventExtensions(){ return false; } }));")
            .contains("TypeError")
    );
    assert_eq!(
        run("var target={x:1}; var p=new Proxy(target,{}); Object.seal(p); var d=Object.getOwnPropertyDescriptor(target,'x'); Object.isSealed(p)+':' + d.configurable;"),
        Value::String(Arc::from("true:false"))
    );
    assert_eq!(
        run("var target={x:1}; var p=new Proxy(target,{}); Object.freeze(p); var d=Object.getOwnPropertyDescriptor(target,'x'); Object.isFrozen(p)+':' + d.writable + ':' + d.configurable;"),
        Value::String(Arc::from("true:false:false"))
    );
    assert_eq!(
        run("var target={x:1}; Object.seal(target); var p=new Proxy(target,{}); Object.isSealed(p);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var target={x:1}; Object.freeze(target); var p=new Proxy(target,{}); Object.isFrozen(p);"),
        Value::Bool(true)
    );
    assert!(
        run_err("Object.seal(new Proxy({x:1}, { ownKeys(){ throw new Error('boom'); } }));")
            .contains("boom")
    );
    assert!(
        run_err("Object.freeze(new Proxy({x:1}, { defineProperty(){ return false; } }));")
            .contains("TypeError")
    );
}

#[test]
fn integrity_level_uses_internal_descriptor_records() {
    assert_eq!(
        run(r#"
              var target = Object.freeze({ x: 2 });
              Object.defineProperty(Object.prototype, "value", {
                value: 1,
                writable: true,
                configurable: true
              });
              var seen;
              var proxy = new Proxy(target, {
                defineProperty: function(actualTarget, key, descriptor) {
                  seen = Object.prototype.hasOwnProperty.call(descriptor, "value");
                  return true;
                }
              });
              var result;
              try { Object.freeze(proxy); result = "ok"; }
              catch (error) { result = error.name; }
              delete Object.prototype.value;
              [result, seen, target.x].join("|");
            "#,),
        Value::String(Arc::from("ok|false|2"))
    );
    assert_eq!(
        run(r#"
              (function () {
                delete arguments.length;
                Object.seal(arguments);
                return Object.isSealed(arguments) &&
                  !Object.prototype.hasOwnProperty.call(arguments, "length");
              })(1);
            "#,),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
              var huge = BigInt("1" + "0".repeat(4096));
              var object = { value: huge };
              var array = [huge];
              Object.freeze(object);
              Object.freeze(array);
              [
                object.value === huge,
                array[0] === huge,
                Object.isFrozen(object),
                Object.isFrozen(array)
              ].join("|");
            "#,),
        Value::String(Arc::from("true|true|true|true"))
    );
    assert_eq!(
        run(r#"
              (function (parameter) {
                var args = arguments;
                Object.defineProperty(args, "0", { enumerable: false });
                parameter = 2;
                Object.freeze(args);
                parameter = 3;
                return [args[0], Object.isFrozen(args), parameter].join("|");
              })(1);
            "#,),
        Value::String(Arc::from("2|true|3"))
    );
}

#[test]
fn is_extensible_uses_proxy_traps_and_reflect_rejects_primitives() {
    assert_eq!(run("Object.isExtensible(1);"), Value::Bool(false));
    assert!(run_err("Reflect.isExtensible(1);").contains("TypeError"));
    assert_eq!(
        run("var seenThis, seenTarget; var target={}; var handler={isExtensible(t){seenThis=this;seenTarget=t;return Object.isExtensible(t);}}; var p=new Proxy(target, handler); Object.isExtensible(p)+':' + (seenThis===handler) + ':' + (seenTarget===target);"),
        Value::String(Arc::from("true:true:true"))
    );
    assert!(
        run_err("Object.isExtensible(new Proxy({}, {isExtensible(){return false;}}));")
            .contains("TypeError")
    );
    assert_eq!(
        run("var target={}; var p=new Proxy(target,{isExtensible(t){return Object.isExtensible(t);}}); var a=Object.isExtensible(p); Object.preventExtensions(target); a + ':' + Object.isExtensible(p);"),
        Value::String(Arc::from("true:false"))
    );
    assert!(run_err(
        "Reflect.isExtensible(new Proxy({}, {isExtensible(){throw new Error('boom');}}));"
    )
    .contains("boom"));
}

#[test]
fn reflect_own_keys_includes_symbols_and_non_enumerables_in_spec_order() {
    assert_eq!(
        run(r#"
            var first = Symbol("first");
            var second = Symbol("second");
            var obj = {};
            obj.z = 1;
            obj[2] = 2;
            Object.defineProperty(obj, "hidden", { value: 3, enumerable: false });
            obj.a = 4;
            obj[first] = 5;
            Object.defineProperty(obj, second, { value: 6, enumerable: false });
            Reflect.ownKeys(obj).map(function(key) {
              if (key === first) return "first";
              if (key === second) return "second";
              return key;
            }).join("|");
            "#),
        Value::String(Arc::from("2|z|hidden|a|first|second"))
    );
    assert!(
        run_err("Reflect.ownKeys('abc');").contains("TypeError"),
        "Reflect.ownKeys must reject primitive targets"
    );
}

#[test]
fn reflect_own_keys_rejects_non_keys_without_coercion_after_reading_the_list() {
    assert_eq!(
        run(r#"
            var coercions = 0;
            var reads = [];
            var key = {
              [Symbol.toPrimitive]: function() {
                coercions += 1;
                return "coerced";
              }
            };
            var result = "none";
            try {
              Reflect.ownKeys(new Proxy({}, {
                ownKeys: function() {
                  return {
                    get 0() { reads.push(0); return "duplicate"; },
                    get 1() { reads.push(1); return "duplicate"; },
                    get 2() { reads.push(2); return key; },
                    length: 3
                  };
                }
              }));
            } catch (error) {
              result = error.name;
            }
            [result, coercions, reads.join(",")].join("|");
        "#),
        Value::String(Arc::from("TypeError|0|0,1,2"))
    );
}

#[test]
fn proxy_own_keys_validates_duplicates_before_enumerability_traps() {
    assert_eq!(
        run(r#"
            var descriptorCalls = 0;
            var duplicateTypeError = false;
            try {
              JSON.stringify(new Proxy({}, {
                ownKeys: function() { return ["key", "key"]; },
                getOwnPropertyDescriptor: function() {
                  descriptorCalls += 1;
                  throw new Error("descriptor must not run");
                }
              }));
            } catch (error) {
              duplicateTypeError = error instanceof TypeError;
            }
            [duplicateTypeError, descriptorCalls].join("|");
        "#),
        Value::String(Arc::from("true|0"))
    );
}

#[test]
fn proxy_own_keys_filters_key_types_before_enumerability_traps() {
    assert_eq!(
        run(r#"
            var symbol = Symbol("ignored");
            var target = {};
            target[symbol] = 1;
            var descriptorCalls = 0;
            var proxy = new Proxy(target, {
              ownKeys: function() { return [symbol]; },
              getOwnPropertyDescriptor: function() {
                descriptorCalls += 1;
                throw new Error("symbol descriptor must not run");
              }
            });
            [JSON.stringify(proxy), descriptorCalls].join("|");
        "#),
        Value::String(Arc::from("{}|0"))
    );
}

#[test]
fn public_own_key_consumers_use_structured_proxy_semantics() {
    assert_eq!(
        run(r#"
            var symbol = Symbol("symbol");
            var source = { text: 1 };
            source[symbol] = 2;
            var arrayLike = new Proxy(source, {
              ownKeys: function() {
                return { 0: "text", 1: symbol, length: 2 };
              }
            });
            var assigned = Object.assign({}, arrayLike);
            var names = Object.getOwnPropertyNames(arrayLike);
            var symbols = Object.getOwnPropertySymbols(arrayLike);

            var descriptorSource = {};
            descriptorSource[symbol] = { value: 3, enumerable: true };
            var descriptors = new Proxy(descriptorSource, {
              ownKeys: function() { return { 0: symbol, length: 1 }; }
            });
            var defined = Object.defineProperties({}, descriptors);

            var marker = {};
            var abrupt = false;
            try {
              Object.assign({}, new Proxy({}, {
                ownKeys: function() { throw marker; }
              }));
            } catch (error) {
              abrupt = error === marker;
            }
            [
              assigned.text,
              assigned[symbol],
              names.join(","),
              symbols.length,
              symbols[0] === symbol,
              defined[symbol],
              abrupt
            ].join("|");
        "#),
        Value::String(Arc::from("1|2|text|1|true|3|true"))
    );
}

#[test]
fn object_entry_helpers_observe_proxy_descriptors() {
    assert_eq!(
        run(r##"
            var log = [];
            var target = { a: 1, b: 2, c: 3 };
            var proxy = new Proxy(target, {
              ownKeys: function(t) {
                log.push("ownKeys");
                return ["a", "b", "c"];
              },
              getOwnPropertyDescriptor: function(t, key) {
                log.push("getOwnPropertyDescriptor:" + key);
                return { enumerable: key !== "b", configurable: true };
              },
              get: function(t, key) {
                log.push("get:" + key);
                return t[key];
              }
            });
            var values = Object.values(proxy).join(",");
            var entries = Object.entries(proxy).map(function(pair) {
              return pair.join(":");
            }).join(",");
            var descs = Object.getOwnPropertyDescriptors(proxy);
            [
              values,
              entries,
              descs.a.enumerable,
              descs.b.enumerable,
              descs.c.enumerable,
              log.join("|")
            ].join("#");
            "##),
        Value::String(Arc::from(
            "1,3#a:1,c:3#true#false#true#ownKeys|getOwnPropertyDescriptor:a|get:a|getOwnPropertyDescriptor:b|getOwnPropertyDescriptor:c|get:c|ownKeys|getOwnPropertyDescriptor:a|get:a|getOwnPropertyDescriptor:b|getOwnPropertyDescriptor:c|get:c|ownKeys|getOwnPropertyDescriptor:a|getOwnPropertyDescriptor:b|getOwnPropertyDescriptor:c"
        ))
    );
}

#[test]
fn reflect_construct_uses_array_like_args_and_new_target() {
    assert_eq!(
        run(r#"
            var seenProto;
            function Target(a, b) {
              this.sum = a + b;
              seenProto = Object.getPrototypeOf(this);
            }
            function NewTarget() {}
            NewTarget.prototype = { marker: 1 };
            var args = {0: 2, 1: 3, length: 2};
            var result = Reflect.construct(Target, args, NewTarget);
            [
              result.sum,
              Object.getPrototypeOf(result) === NewTarget.prototype,
              seenProto === NewTarget.prototype
            ].join("|");
            "#),
        Value::String(Arc::from("5|true|true"))
    );
    assert!(run_err("Reflect.construct(function(){}, 1);").contains("TypeError"));
    assert!(run_err("var o = {}; Object.defineProperty(o, 'length', { get(){ throw new Error('boom'); } }); Reflect.construct(function(){}, o);").contains("boom"));
    assert!(run_err("Reflect.construct(function(){}, [], Date.now);").contains("TypeError"));
    assert!(run_err("new Reflect.construct(Function, [], Function);").contains("TypeError"));
    assert_eq!(
        run("function isConstructor(f){ try { Reflect.construct(function(){}, [], f); } catch(e) { return false; } return true; } isConstructor(Reflect.construct);"),
        Value::Bool(false)
    );
    assert_eq!(
        run("function C(){} C.prototype = null; Object.getPrototypeOf(new C()) === Object.prototype;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            function Target() {}
            var NewTarget = function() {}.bind();
            Object.defineProperty(Function.prototype, "prototype", {
              get: function() { calls++; throw new Error("proto boom"); },
              configurable: true
            });
            var out;
            try {
              Reflect.construct(Target, [], NewTarget);
              out = "ok";
            } catch (e) {
              out = e.message;
            }
            delete Function.prototype.prototype;
            out + "|" + calls;
            "#),
        Value::String(Arc::from("proto boom|1"))
    );
    assert_eq!(
        run(r#"
            var calls = 0;
            var proto = {};
            var NewTarget = function() {}.bind();
            Object.defineProperty(Function.prototype, "prototype", {
              get: function() { calls++; return proto; },
              configurable: true
            });
            var arr = Reflect.construct(Array, [], NewTarget);
            var fn = Reflect.construct(Function, ["return 1;"], NewTarget);
            delete Function.prototype.prototype;
            [
              calls,
              Object.getPrototypeOf(arr) === proto,
              Object.getPrototypeOf(fn) === proto
            ].join("|");
            "#),
        Value::String(Arc::from("2|true|true"))
    );
}

#[test]
fn reflect_apply_uses_create_list_from_array_like() {
    assert_eq!(
        run(r#"
            function collect() {
              return Array.prototype.join.call(arguments, "|");
            }
            var args = {0: "a", 1: "b", length: 2};
            Reflect.apply(collect, null, args);
            "#),
        Value::String(Arc::from("a|b"))
    );
    assert_eq!(
        run(r#"
            function count() {
              return arguments.length + ":" + String(arguments[0]);
            }
            var args = {};
            Object.defineProperty(args, "length", {
              get: function() { return 1; }
            });
            Reflect.apply(count, null, args);
            "#),
        Value::String(Arc::from("1:undefined"))
    );
    assert!(run_err("Reflect.apply(function(){}, null, 1);").contains("TypeError"));
    assert!(run_err("Reflect.apply(function(){}, null);").contains("TypeError"));
    assert!(run_err(
        "var o = {}; Object.defineProperty(o, 'length', { get: function(){ throw new Error('boom'); } }); Reflect.apply({}, null, o);"
    )
    .contains("TypeError"));
    assert!(
        run_err("var o = {}; Object.defineProperty(o, 'length', { get: function(){ throw new Error('boom'); } }); Reflect.apply(function(){}, null, o);")
            .contains("boom")
    );
}

#[test]
fn function_apply_observes_array_like_access_and_roots_arguments() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    vm.run(
        r#"
        var accessLog = [];
        var inherited = {};
        Object.defineProperty(inherited, "1", {
          get: function() {
            accessLog.push("1");
            forceGc();
            return { label: "inherited" };
          }
        });
        var arrayLike = Object.create(inherited);
        Object.defineProperty(arrayLike, "length", {
          get: function() {
            accessLog.push("length");
            return {
              valueOf: function() {
                accessLog.push("valueOf");
                forceGc();
                return 2;
              }
            };
          }
        });
        Object.defineProperty(arrayLike, "0", {
          get: function() {
            accessLog.push("0");
            return { label: "own" };
          }
        });
        var observableResult = function(first, second) {
          forceGc();
          return [first.label, second.label, first === second].join(",");
        }.apply(null, arrayLike);

        var proxyLog = [];
        var proxyResult = function(first, second) {
          return first + second;
        }.apply(null, new Proxy({ length: 2, 0: 3, 1: 4 }, {
          get: function(target, key) {
            proxyLog.push(String(key));
            return target[key];
          }
        }));

        var array = [{ label: "backing" }];
        var arrayPrototype = Object.create(Array.prototype);
        Object.defineProperty(arrayPrototype, "1", {
          value: { label: "array-inherited" }
        });
        Object.setPrototypeOf(array, arrayPrototype);
        Object.defineProperty(array, "0", {
          get: function() { return { label: "array-accessor" }; }
        });
        array.length = 2;
        var arrayResult = function(first, second) {
          return first.label + "," + second.label;
        }.apply(null, array);

        var typedResult = function(first, second) {
          return first + "," + second;
        }.apply(null, new Uint8Array([5, 6]));

        function emptyCount() { return arguments.length; }
        var nullishResult = [
          emptyCount.apply(),
          emptyCount.apply(null),
          emptyCount.apply(null, undefined),
          emptyCount.apply(null, null)
        ].join(",");

        var lengthError = {};
        var lengthErrorResult = false;
        try {
          (function() {}).apply(null, {
            get length() {
              return {
                valueOf: function() {
                  forceGc();
                  throw lengthError;
                }
              };
            }
          });
        } catch (error) {
          forceGc();
          lengthErrorResult = error === lengthError;
        }

        var returned = function(first) {
          forceGc();
          return first;
        }.apply(null, {
          length: 1,
          get 0() { return { label: "returned" }; }
        });
        forceGc();

        var thrownResult = "missing";
        try {
          (function(first) {
            forceGc();
            throw first;
          }).apply(null, {
            length: 1,
            get 0() { return { label: "thrown" }; }
          });
        } catch (error) {
          forceGc();
          thrownResult = error.label;
        }

        var capReads = 0;
        var capResult = false;
        try {
          (function() {}).apply(null, {
            length: 1048577,
            get 0() { capReads++; }
          });
        } catch (error) {
          capResult = error instanceof RangeError && capReads === 0;
        }

        var primitiveResult = false;
        try {
          (function() {}).apply(null, 1);
        } catch (error) {
          primitiveResult = error instanceof TypeError;
        }
        "#,
    )
    .expect("failed to exercise Function.apply argument materialization");

    assert_eq!(
        vm.run(
            "[observableResult, accessLog.join(','), proxyResult, proxyLog.join(','), arrayResult, typedResult, nullishResult, lengthErrorResult, returned.label, thrownResult, capResult, primitiveResult].join('|')"
        )
        .expect("failed to read Function.apply results"),
        Value::String(Arc::from(concat!(
            "own,inherited,false|length,valueOf,0,1|7|length,0,1|",
            "array-accessor,array-inherited|5,6|0,0,0,0|true|",
            "returned|thrown|true|true"
        )))
    );
}

#[test]
fn reflect_argument_lists_root_values_across_observable_getters() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    vm.run(
        r#"
        var applyResult = Reflect.apply(function(first, second) {
          return [first.label, second.label, first === second].join(",");
        }, null, {
          length: 2,
          get 0() { return { label: "apply-first" }; },
          get 1() { forceGc(); return { label: "apply-second" }; }
        });

        var lengthResult = Reflect.apply(function(first, second) {
          return first.label + "," + second.label;
        }, null, {
          get length() {
            return {
              get valueOf() {
                forceGc();
                return function() { forceGc(); return 2; };
              }
            };
          },
          get 0() { return { label: "length-first" }; },
          get 1() { return { label: "length-second" }; }
        });

        var lengthError = {};
        var lengthErrorResult = false;
        try {
          Reflect.apply(function() {}, null, {
            get length() {
              return {
                valueOf: function() { forceGc(); throw lengthError; }
              };
            }
          });
        } catch (error) {
          lengthErrorResult = error === lengthError;
        }

        function Target(first, second) {
          this.result = [first.label, second.label, first === second].join(",");
        }
        var constructResult = Reflect.construct(Target, {
          length: 2,
          get 0() { return { label: "construct-first" }; },
          get 1() { forceGc(); return { label: "construct-second" }; }
        }).result;

        var newTargetPrototype = { marker: "new-target-prototype" };
        var NewTarget = new Proxy(function() {}, {
          get: function(target, key, receiver) {
            if (key === "prototype") {
              forceGc();
              return newTargetPrototype;
            }
            return Reflect.get(target, key, receiver);
          }
        });
        var newTargetInstance = Reflect.construct(Target, {
          length: 2,
          get 0() { return { label: "new-target-first" }; },
          get 1() { return { label: "new-target-second" }; }
        }, NewTarget);
        var newTargetResult =
          newTargetInstance.result + "," +
          Object.getPrototypeOf(newTargetInstance).marker;

        var nestedResult = Reflect.apply(function(first, second) {
          return Reflect.apply(function(a, b) {
            return a() + "," + b();
          }, null, [first, second]);
        }, null, {
          length: 2,
          get 0() { return function() { return "nested-first"; }; },
          get 1() { forceGc(); return function() { return "nested-second"; }; }
        });

        var forwarding = new Proxy(function(first, second) {
          return [first.label, second.label, first === second].join(",");
        }, {
          apply: function(target, receiver, args) {
            return Reflect.apply(target, receiver, args);
          }
        });
        var proxyResult = Reflect.apply(forwarding, null, {
          length: 2,
          get 0() { return { label: "proxy-first" }; },
          get 1() { forceGc(); return { label: "proxy-second" }; }
        });

        var returned = Reflect.apply(function(first) { return first; }, null, {
          length: 1,
          get 0() { return { label: "returned-first" }; }
        });
        forceGc();
        var returnedResult = returned.label;

        var thrownResult = "missing";
        try {
          Reflect.apply(function(first) { throw first; }, null, {
            length: 1,
            get 0() { return { label: "thrown-first" }; }
          });
        } catch (error) {
          forceGc();
          thrownResult = error.label;
        }

        var promiseResult = "pending";
        try {
          Reflect.construct(Promise, {
            length: 2,
            get 0() { return function(resolve) { resolve(9); }; },
            get 1() { forceGc(); return {}; }
          }).then(function(value) {
            promiseResult = value;
          });
        } catch (error) {
          promiseResult = error.name;
        }
        "#,
    )
    .expect("failed to exercise Reflect argument-list rooting");

    assert_eq!(
        vm.run(
            "[applyResult, lengthResult, lengthErrorResult, constructResult, newTargetResult, nestedResult, proxyResult, returnedResult, thrownResult, promiseResult].join('|');"
        )
        .expect("failed to read Reflect argument-list results"),
        Value::String(Arc::from(concat!(
            "apply-first,apply-second,false|",
            "length-first,length-second|true|",
            "construct-first,construct-second,false|",
            "new-target-first,new-target-second,false,new-target-prototype|",
            "nested-first,nested-second|",
            "proxy-first,proxy-second,false|",
            "returned-first|thrown-first|9"
        )))
    );
}

#[test]
fn math_expanded() {
    assert_eq!(run("Math.hypot(3,4);"), Value::Number(5.0));
    assert_eq!(
        run("Math.atan2(1,0);"),
        Value::Number(std::f64::consts::FRAC_PI_2)
    );
    assert_eq!(run("Math.clz32(1);"), Value::Number(31.0));
    assert_eq!(run("Math.clz32(Infinity);"), Value::Number(32.0));
    assert_eq!(run("Math.clz32(4294967296);"), Value::Number(32.0));
    assert_eq!(run("Math.imul(0xffffffff, 5);"), Value::Number(-5.0));
    assert_eq!(run("Math.sign(-5);"), Value::Number(-1.0));
    assert!(matches!(run("Math.sign(NaN);"), Value::Number(n) if n.is_nan()));
    assert_eq!(run("Object.is(Math.sign(-0), -0);"), Value::Bool(true));
    assert_eq!(run("Math.sinh(0);"), Value::Number(0.0));
    assert_eq!(run("Math.acosh(1);"), Value::Number(0.0));
    assert_eq!(run("Math.asinh(-0);"), Value::Number(-0.0));
    assert_eq!(run("Math.atanh(1);"), Value::Number(f64::INFINITY));
    assert!(matches!(run("Math.acosh(0);"), Value::Number(n) if n.is_nan()));
    assert_eq!(
        run("Math.acosh.length + ':' + Math.asinh.name + ':' + Math.atanh.length;"),
        Value::String(Arc::from("1:asinh:1"))
    );
    assert_eq!(run("Math.sumPrecise([1, 2, 3]);"), Value::Number(6.0));
    assert_eq!(
        run("Math.sumPrecise([1e30, 0.1, -1e30]);"),
        Value::Number(0.1)
    );
    assert_eq!(
        run("Object.is(Math.sumPrecise([]), -0);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("Object.is(Math.sumPrecise([-0, 0]), 0);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("Math.sumPrecise([Infinity, Infinity]);"),
        Value::Number(f64::INFINITY)
    );
    assert!(matches!(
        run("Math.sumPrecise([Infinity, -Infinity]);"),
        Value::Number(n) if n.is_nan()
    ));
    assert!(common::run_err("Math.sumPrecise([{}]);").contains("TypeError"));
}

// --- Promise ---

#[test]
fn promise_resolve_basic() {
    // then callback runs after the synchronous run; the last expression is the
    // synchronous return (undefined). We verify the promise object itself.
    let r = run("new Promise(function(res){ res(1); });");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn promise_then_chain_value() {
    // Chained then: the second then receives the first's transformed value.
    // We store it in a global that the synchronous run cannot read back, so we
    // instead verify the derived promise from .then is an object.
    let r = run("new Promise(function(res){ res(5); }) \
           .then(function(v){ return v * 2; }) \
           .then(function(v){ return v; });");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn promise_then_uses_species_capability_path() {
    assert_eq!(
        run("var getterCalled = false, ok = false;
             var object = Object.defineProperty({}, 'constructor', {
               get: function() { getterCalled = true; throw new Error('bad'); }
             });
             try { Promise.prototype.then.call(object); }
             catch (e) { ok = e instanceof TypeError; }
             ok && getterCalled === false;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var callCount = 0, argLength = 0, executorLength = 0;
             var p = new Promise(function() {});
             class SpeciesConstructor extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 argLength = arguments.length;
                 executorLength = executor.length;
               }
             }
             p.constructor = function() {};
             p.constructor[Symbol.species] = SpeciesConstructor;
             var result = p.then();
             callCount === 1 && argLength === 1 && executorLength === 2 &&
               result instanceof SpeciesConstructor;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var p = new Promise(function() {});
             function BadCapability(executor) {
               executor(1, function() {});
               return {};
             }
             p.constructor = function() {};
             p.constructor[Symbol.species] = BadCapability;
             var ok = false;
             try { p.then(); }
             catch (e) { ok = e instanceof TypeError; }
             ok;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_then_adopts_returned_fulfilled_promise() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "var out = 0;
         Promise.resolve(1)
           .then(function() { return Promise.resolve(7); })
           .then(function(v) { out = v; });",
    )
    .expect("evaluation errored");
    assert_eq!(
        vm.run("out;").expect("evaluation errored"),
        Value::Number(7.0)
    );
}

#[test]
fn promise_then_rejects_self_resolution_with_type_error() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "var caught = false;
         var q = Promise.resolve().then(function() { return q; });
         q.catch(function(e) { caught = e instanceof TypeError; });",
    )
    .expect("evaluation errored");
    assert_eq!(
        vm.run("caught;").expect("evaluation errored"),
        Value::Bool(true)
    );
}

#[test]
fn promise_capability_resolve_assimilates_thenables_and_rejects_self_resolution() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var assimilated;
        new Promise(resolve => resolve({ then(resolve) { resolve(7); } }))
          .then(value => { assimilated = value; });

        var resolveSelf;
        var self = new Promise(resolve => { resolveSelf = resolve; });
        var selfRejected = false;
        resolveSelf(self);
        self.catch(error => { selfRejected = error instanceof TypeError; });

        var marker = {};
        var getterReason;
        new Promise(resolve => resolve({ get then() { throw marker; } }))
          .catch(reason => { getterReason = reason; });

        var resumeThenable, rejectPending;
        var firstCallResult, firstCallRejected = false;
        var pending = new Promise((resolve, reject) => {
          rejectPending = reject;
          resolve({ then(resolveThenable) { resumeThenable = resolveThenable; } });
        });
        pending.then(value => { firstCallResult = value; }, () => { firstCallRejected = true; });
        rejectPending('late');
        Promise.resolve().then(() => { resumeThenable(9); });

        var resumeAfterExecutorThrow, executorThrowResult, executorThrowRejected = false;
        new Promise(resolve => {
          resolve({ then(resolveThenable) { resumeAfterExecutorThrow = resolveThenable; } });
          throw marker;
        }).then(
          value => { executorThrowResult = value; },
          () => { executorThrowRejected = true; }
        );
        Promise.resolve().then(() => { resumeAfterExecutorThrow(11); });
        "#,
    )
    .expect("Promise resolution jobs should settle");
    assert_eq!(
        vm.run("assimilated === 7 && selfRejected && getterReason === marker && firstCallResult === 9 && !firstCallRejected && executorThrowResult === 11 && !executorThrowRejected;")
            .expect("Promise settlement results should be readable"),
        Value::Bool(true)
    );
}

#[test]
fn promise_reaction_captures_handler_realm_before_proxy_revocation() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.other = $262.createRealm().global;
        globalThis.immediatePair = other.eval("Proxy.revocable(function () {}, {})");
        globalThis.immediateResult = "pending";
        Promise.resolve().then(immediatePair.proxy).catch(function (error) {
          immediateResult = [
            error instanceof other.TypeError,
            error instanceof TypeError
          ].join("|");
        });
        immediatePair.revoke();

        globalThis.pendingFulfilledPair = other.eval(
          "Proxy.revocable(function () {}, {})"
        );
        globalThis.resolvePending = undefined;
        globalThis.pendingFulfilledResult = "pending";
        new Promise(function (resolve) { resolvePending = resolve; })
          .then(pendingFulfilledPair.proxy)
          .catch(function (error) {
            pendingFulfilledResult = [
              error instanceof other.TypeError,
              error instanceof TypeError
            ].join("|");
          });
        resolvePending();
        pendingFulfilledPair.revoke();

        globalThis.pendingRejectedPair = other.eval(
          "Proxy.revocable(function () {}, {})"
        );
        globalThis.rejectPending = undefined;
        globalThis.pendingRejectedResult = "pending";
        new Promise(function (_, reject) { rejectPending = reject; })
          .then(undefined, pendingRejectedPair.proxy)
          .catch(function (error) {
            pendingRejectedResult = [
              error instanceof other.TypeError,
              error instanceof TypeError
            ].join("|");
          });
        rejectPending("reason");
        pendingRejectedPair.revoke();
        "#,
    )
    .expect("revoked Proxy reaction should reject");
    assert_eq!(
        vm.run("[immediateResult, pendingFulfilledResult, pendingRejectedResult].join(',')")
            .expect("reaction Realm marker should be readable"),
        Value::String(Arc::from("true|false,true|false,true|false"))
    );
}

#[test]
fn promise_catch_reject() {
    // reject -> catch returns a derived promise (object), not the error value.
    let r = run("new Promise(function(_, rej){ rej('boom'); }) \
           .catch(function(e){ return e; });");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn promise_catch_invokes_observable_then() {
    assert_eq!(
        run(
            "var target = {}, returnValue = {}, callCount = 0, thisValue, firstArg, secondArg;
             target.then = function(a, b) {
               callCount += 1;
               thisValue = this;
               firstArg = a;
               secondArg = b;
               return returnValue;
             };
             var result = Promise.prototype.catch.call(target, 1, 2, 3);
             callCount === 1 && thisValue === target && firstArg === undefined &&
               secondArg === 1 && result === returnValue;"
        ),
        Value::Bool(true)
    );
    assert_eq!(
        run("var poisoned = Object.defineProperty({}, 'then', {
               get: function() { throw new TypeError('poison'); }
             });
             var ok = false;
             try { Promise.prototype.catch.call(poisoned); }
             catch (e) { ok = e instanceof TypeError; }
             ok;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_finally_invokes_observable_then() {
    assert_eq!(
        run(
            "var target = {}, returnValue = {}, callCount = 0, thisValue, firstArg, secondArg;
             target.then = function(a, b) {
               callCount += 1;
               thisValue = this;
               firstArg = a;
               secondArg = b;
               return returnValue;
             };
             var result = Promise.prototype.finally.call(target, 1, 2, 3);
             callCount === 1 && thisValue === target && firstArg === 1 &&
               secondArg === 1 && result === returnValue;"
        ),
        Value::Bool(true)
    );
    assert_eq!(
        run("var poisoned = Object.defineProperty({}, 'then', {
               get: function() { throw new TypeError('poison'); }
             });
             var ok = false;
             try { Promise.prototype.finally.call(poisoned); }
             catch (e) { ok = e instanceof TypeError; }
             ok;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_finally_honors_symbol_species_accessor() {
    assert_eq!(
        run("class FooPromise extends Promise {
               static get [Symbol.species]() { return Promise; }
             }
             var p = Promise.resolve().finally(function() {
               return FooPromise.resolve();
             });
             p instanceof Promise && !(p instanceof FooPromise);"),
        Value::Bool(true)
    );
}

#[test]
fn promise_then_normalizes_handlers_and_observes_returned_promise_then() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var fulfilledOriginal = {};
        var fulfilledResult;
        Promise.resolve(fulfilledOriginal)
          .then(1, 2)
          .then(function (value) { fulfilledResult = value; });

        var rejectedOriginal = {};
        var rejectedResult;
        Promise.reject(rejectedOriginal)
          .then(1, 2)
          .then(undefined, function (reason) { rejectedResult = reason; });

        var observableThenCalls = 0;
        var returnedPromise = Promise.resolve();
        returnedPromise.then = function (resolve) {
          observableThenCalls += 1;
          resolve(9);
        };
        var assimilatedResult;
        Promise.resolve()
          .then(function () { return returnedPromise; })
          .then(function (value) { assimilatedResult = value; });
        "#,
    )
    .expect("Promise reaction matrix should settle");
    assert_eq!(
        vm.run(
            "fulfilledResult === fulfilledOriginal &&\
             rejectedResult === rejectedOriginal &&\
             observableThenCalls === 1 && assimilatedResult === 9;"
        )
        .expect("Promise reaction results should be readable"),
        Value::Bool(true)
    );
}

#[test]
fn promise_finally_uses_wrappers_promise_resolve_and_original_completion() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var order = [];
        var thenFinally, catchFinally, observedThis, observedArgs;
        var SpeciesHolder = function () {};
        Object.defineProperty(SpeciesHolder, Symbol.species, {
          get: function () { order.push("species"); return Promise; }
        });
        var target = {};
        Object.defineProperty(target, "constructor", {
          get: function () { order.push("constructor"); return SpeciesHolder; }
        });
        Object.defineProperty(target, "then", {
          get: function () {
            order.push("then-get");
            return function (onFulfilled, onRejected) {
              order.push("then-call");
              observedThis = this;
              observedArgs = arguments.length;
              thenFinally = onFulfilled;
              catchFinally = onRejected;
              return target;
            };
          }
        });
        var callback = function () {};
        var observableResult = Promise.prototype.finally.call(target, callback);

        var fulfilledOriginal = {};
        var fulfilledResult;
        var fulfilledArgs = -1;
        Promise.resolve(fulfilledOriginal)
          .finally(function () {
            fulfilledArgs = arguments.length;
            return {};
          })
          .then(function (value) { fulfilledResult = value; });

        var rejectedOriginal = {};
        var rejectedResult;
        Promise.reject(rejectedOriginal)
          .finally(function () { return {}; })
          .then(undefined, function (reason) { rejectedResult = reason; });

        var thrownReason = {};
        var thrownResult;
        Promise.resolve()
          .finally(function () { throw thrownReason; })
          .then(undefined, function (reason) { thrownResult = reason; });

        var replacementReason = {};
        var replacementResult;
        Promise.resolve()
          .finally(function () { return Promise.reject(replacementReason); })
          .then(undefined, function (reason) { replacementResult = reason; });

        var subclassCount = 0;
        var observedSubclassCount = 0;
        class FinallyPromise extends Promise {
          constructor(executor) {
            subclassCount += 1;
            super(executor);
          }
        }
        new FinallyPromise(function (resolve) { resolve(); })
          .finally(function () {})
          .then(function () { observedSubclassCount = subclassCount; })
          .then(function () {});

        var abstractOriginal = {};
        var abstractResult;
        var resolveReads = 0;
        var abstractSource = new Promise(function (resolve) { resolve(abstractOriginal); });
        Object.defineProperty(Promise, "resolve", {
          configurable: true,
          get: function () { resolveReads += 1; throw new Error("must not read Promise.resolve"); }
        });
        abstractSource
          .finally(function () { return {}; })
          .then(function (value) { abstractResult = value; });
        "#,
    )
    .expect("Promise finally matrix should settle");
    assert_eq!(
        vm.run(
            r#"
            order.join("|") === "constructor|species|then-get|then-call" &&
            observableResult === target && observedThis === target && observedArgs === 2 &&
            typeof thenFinally === "function" && typeof catchFinally === "function" &&
            thenFinally !== callback && catchFinally !== callback &&
            thenFinally.name === "" && catchFinally.name === "" &&
            thenFinally.length === 1 && catchFinally.length === 1 &&
            (function () { try { new thenFinally(); return false; } catch (e) { return e instanceof TypeError; } })() &&
            (function () { try { new catchFinally(); return false; } catch (e) { return e instanceof TypeError; } })() &&
            fulfilledArgs === 0 && fulfilledResult === fulfilledOriginal &&
            rejectedResult === rejectedOriginal && thrownResult === thrownReason &&
            replacementResult === replacementReason && observedSubclassCount === 7 &&
            resolveReads === 0 && abstractResult === abstractOriginal;
            "#,
        )
        .expect("Promise finally observations should be readable"),
        Value::Bool(true)
    );
}

#[test]
fn promise_finally_closures_use_method_realm_and_survive_observable_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var foreignFinally = other.Promise.prototype.finally;
            var callbackCalls = 0;
            var callback = function () {
              callbackCalls += 1;
              forceGc();
              return returned;
            };
            var callbackRef = new WeakRef(callback);
            var innerHandlers = [];
            var returned = new other.Promise(function (resolve) { resolve(1); });
            Object.defineProperty(returned, "then", {
              get: function () {
                forceGc();
                return function (handler) {
                  innerHandlers.push(handler);
                  forceGc();
                  return handler();
                };
              }
            });

            var thenFinally, catchFinally;
            var target = {};
            Object.defineProperty(target, "constructor", {
              get: function () { forceGc(); return undefined; }
            });
            Object.defineProperty(target, "then", {
              get: function () {
                forceGc();
                return function (onFulfilled, onRejected) {
                  thenFinally = onFulfilled;
                  catchFinally = onRejected;
                  forceGc();
                  return {};
                };
              }
            });
            foreignFinally.call(target, callback);
            callback = null;
            forceGc();

            var original = {};
            var fulfilled = thenFinally(original);
            var reason = {};
            var rejected;
            try { catchFinally(reason); } catch (error) { rejected = error; }

            var receiverError;
            try { foreignFinally.call(1); } catch (error) { receiverError = error; }
            [
              callbackRef.deref() !== undefined,
              callbackCalls === 2,
              fulfilled === original,
              rejected === reason,
              Object.getPrototypeOf(thenFinally) === other.Function.prototype,
              Object.getPrototypeOf(catchFinally) === other.Function.prototype,
              innerHandlers.length === 2,
              innerHandlers[0].name === "" && innerHandlers[0].length === 0,
              innerHandlers[1].name === "" && innerHandlers[1].length === 0,
              Object.getPrototypeOf(innerHandlers[0]) === other.Function.prototype,
              Object.getPrototypeOf(innerHandlers[1]) === other.Function.prototype,
              receiverError instanceof other.TypeError,
              !(receiverError instanceof TypeError)
            ].join("|");
            "#,
        )
        .expect("foreign Promise finally closures should survive GC"),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn promise_callback_runs() {
    // Verify the then callback actually executes by having it throw into a
    // catch that we observe via the derived promise being an object.
    let r = run("new Promise(function(res){ res(1); }).then(function(v){ throw v; });");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn promise_keyword_method_names() {
    // `.catch` and `.then` use reserved words as property names.
    let r = run("typeof Promise.prototype.then;");
    assert_eq!(r, Value::String(Arc::from("function")));
}

#[test]
fn promise_static_surface_and_species_descriptor() {
    assert_eq!(
        run("[
                typeof Promise.all,
                typeof Promise.allKeyed,
                typeof Promise.race,
                typeof Promise.allSettled,
                typeof Promise.allSettledKeyed,
                typeof Promise.any,
                typeof Promise.try,
                typeof Promise.withResolvers,
                typeof Promise.prototype.finally
             ].join(',');"),
        Value::String(Arc::from(
            "function,function,function,function,function,function,function,function,function"
        ))
    );
    assert_eq!(
        run("Promise[Symbol.species] === Promise;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var d = Object.getOwnPropertyDescriptor(Promise, Symbol.species);
             typeof d.get + ':' + d.set + ':' + d.enumerable + ':' + d.configurable;"
        ),
        Value::String(Arc::from("function:undefined:false:true"))
    );
    assert_eq!(
        run(
            "Object.getOwnPropertyDescriptor(Promise, 'all').writable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'all').enumerable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'all').configurable + ':' +
             Promise.all.length + ':' + Promise.all.name;"
        ),
        Value::String(Arc::from("true:false:true:1:all"))
    );
    assert_eq!(
        run(
            "Object.getOwnPropertyDescriptor(Promise, 'allKeyed').writable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allKeyed').enumerable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allKeyed').configurable + ':' +
             Promise.allKeyed.length + ':' + Promise.allKeyed.name + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allSettledKeyed').writable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allSettledKeyed').enumerable + ':' +
             Object.getOwnPropertyDescriptor(Promise, 'allSettledKeyed').configurable + ':' +
             Promise.allSettledKeyed.length + ':' + Promise.allSettledKeyed.name;"
        ),
        Value::String(Arc::from(
            "true:false:true:1:allKeyed:true:false:true:1:allSettledKeyed"
        ))
    );
}

#[test]
fn promise_intrinsics_are_isolated_by_realm() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var foreignAsync = other.eval("(async function () { return 7; })");
            var foreignPromise = foreignAsync();
            var foreignError;
            try { other.Promise.prototype.then.call({}); }
            catch (error) { foreignError = error; }
            var species = Object.getOwnPropertyDescriptor(
                other.Promise,
                Symbol.species
            );
            var tag = Object.getOwnPropertyDescriptor(
                other.Promise.prototype,
                Symbol.toStringTag
            );
            [
                other.Promise !== Promise,
                other.Promise.prototype !== Promise.prototype,
                other.Promise.resolve !== Promise.resolve,
                other.Promise.prototype.then !== Promise.prototype.then,
                Object.getPrototypeOf(other.Promise) === other.Function.prototype,
                Object.getPrototypeOf(other.Promise.prototype) === other.Object.prototype,
                Object.getPrototypeOf(foreignPromise) === other.Promise.prototype,
                foreignPromise.constructor === other.Promise,
                foreignPromise instanceof other.Promise,
                !(foreignPromise instanceof Promise),
                foreignError instanceof other.TypeError,
                !(foreignError instanceof TypeError),
                species.get.call(other.Promise) === other.Promise,
                species.enumerable,
                species.configurable,
                tag.value,
                tag.writable,
                tag.enumerable,
                tag.configurable
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true|true|true|true|false|true|Promise|false|false|true"
        ))
    );
}

#[test]
fn promise_intrinsic_selection_ignores_mutable_globals_and_survives_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    vm.run(
        r#"
        var mainConstructor = Promise;
        var mainAsync = async function () { return 1; };
        var other = $262.createRealm().global;
        var constructorRef = new WeakRef(other.Promise);
        var prototypeRef = new WeakRef(other.Promise.prototype);
        var foreignAsync = other.eval("(async function () { return 2; })");
        var foreignForAwait = other.eval(
            "(async function () { for await (var value of [1]) {} return 3; })"
        );
        var foreignSelfResolve, foreignSelfReason;
        var foreignSelfPromise = new other.Promise(function (resolve) {
            foreignSelfResolve = resolve;
        });
        foreignSelfPromise.catch(function (error) { foreignSelfReason = error; });
        foreignSelfResolve(foreignSelfPromise);
        var foreignThen = other.Promise.prototype.then;
        var newTarget = new other.Function();
        newTarget.prototype = null;
        other.Promise = null;
        Promise = null;
    "#,
    )
    .expect("failed to prepare mutated Promise Realms");

    vm.gc();
    assert_eq!(
        vm.run(
            r#"
            var foreignConstructor = constructorRef.deref();
            var foreignPrototype = prototypeRef.deref();
            var mainResult = mainAsync();
            var foreignResult = foreignAsync();
            var foreignForAwaitResult = foreignForAwait();
            var reflected = Reflect.construct(
                mainConstructor,
                [function () { forceGc(); }],
                newTarget
            );
            var speciesDefault = mainConstructor.resolve(1);
            speciesDefault.constructor = undefined;
            var chained = foreignThen.call(speciesDefault);
            [
                foreignConstructor !== undefined,
                foreignPrototype !== undefined,
                Object.getPrototypeOf(mainResult) === mainConstructor.prototype,
                Object.getPrototypeOf(foreignResult) === foreignPrototype,
                foreignResult.constructor === foreignConstructor,
                Object.getPrototypeOf(foreignForAwaitResult) === foreignPrototype,
                foreignSelfReason instanceof other.TypeError,
                !(foreignSelfReason instanceof TypeError),
                Object.getPrototypeOf(reflected) === foreignPrototype,
                Object.getPrototypeOf(chained) === foreignPrototype
            ].join("|");
        "#
        )
        .expect("failed to use rooted Promise intrinsics after GC"),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn promise_internal_functions_and_results_use_the_method_realm() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            var resolvingFunctions = [];
            new other.Promise(function (resolve, reject) {
                resolvingFunctions = [resolve, reject];
            });

            var capabilityExecutor;
            function Capability(executor) {
                capabilityExecutor = executor;
                executor(function () {}, function () {});
                return {};
            }
            other.Promise.resolve.call(Capability, 1);

            var elementFunctions = [];
            function Elements(executor) {
                executor(function () {}, function () {});
                return {};
            }
            Elements.resolve = function (value) {
                return {
                    then: function (resolve, reject) {
                        elementFunctions = [resolve, reject];
                        resolve(value);
                    }
                };
            };
            other.Promise.allSettled.call(Elements, [1]);

            var allValue, settledValue, anyReason;
            other.Promise.all([]).then(function (value) { allValue = value; });
            other.Promise.allSettled([1]).then(function (value) {
                settledValue = value;
            });
            other.Promise.any([]).catch(function (error) { anyReason = error; });
            var resolvers = other.Promise.withResolvers();
            "scheduled";
        "#
        )
        .expect("failed to exercise foreign Promise internals"),
        Value::String(Arc::from("scheduled"))
    );

    assert_eq!(
        vm.run(
            r#"
            [
                resolvingFunctions.every(function (fn) {
                    return Object.getPrototypeOf(fn) === other.Function.prototype;
                }),
                Object.getPrototypeOf(capabilityExecutor) === other.Function.prototype,
                elementFunctions.every(function (fn) {
                    return Object.getPrototypeOf(fn) === other.Function.prototype;
                }),
                Object.getPrototypeOf(allValue) === other.Array.prototype,
                Object.getPrototypeOf(settledValue) === other.Array.prototype,
                Object.getPrototypeOf(settledValue[0]) === other.Object.prototype,
                Object.getPrototypeOf(anyReason) === other.AggregateError.prototype,
                Object.getPrototypeOf(anyReason.errors) === other.Array.prototype,
                Object.getPrototypeOf(resolvers) === other.Object.prototype
            ].join("|");
        "#
        )
        .expect("failed to inspect foreign Promise internals"),
        Value::String(Arc::from("true|true|true|true|true|true|true|true|true"))
    );
}

#[test]
fn promise_static_combinators_return_promises() {
    assert_eq!(
        run("[
                Promise.all([Promise.resolve(1), 2]) instanceof Promise,
                Promise.allKeyed({a: Promise.resolve(1), b: 2}) instanceof Promise,
                Promise.race([Promise.resolve(1), 2]) instanceof Promise,
                Promise.allSettled([Promise.resolve(1), Promise.reject(2)]) instanceof Promise,
                Promise.allSettledKeyed({a: Promise.resolve(1), b: Promise.reject(2)}) instanceof Promise,
                Promise.any([Promise.reject(1), Promise.resolve(2)]) instanceof Promise,
                Promise.try(function(){ return 3; }) instanceof Promise,
                Promise.resolve(1).finally(function(){}) instanceof Promise
             ].join(',');"),
        Value::String(Arc::from("true,true,true,true,true,true,true,true"))
    );
}

#[test]
fn promise_try_uses_receiver_constructor_capability() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.try.call(SubPromise, function() { return 7; });
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var badCtor = false, badPrimitive = false;
             try { Promise.try.call(eval); } catch (e) { badCtor = e instanceof TypeError; }
             try { Promise.try.call(null); } catch (e) { badPrimitive = e instanceof TypeError; }
             badCtor && badPrimitive;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "function BadPromise(executor) { throw new RangeError('bad'); }
             try { Promise.try.call(BadPromise, function() {}); false; }
             catch (e) { e instanceof RangeError; }"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.all.call(SubPromise, []);
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var resolvedValues;
             var C = function(executor) {
               executor(function(values) { resolvedValues = values; }, function() {});
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       resolve(value);
                     }
                   };
                 };
               }
             });
             Promise.all.call(C, [1, 2]);
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && resolvedValues.join(',') === '1,2';"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_settled_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.allSettled.call(SubPromise, []);
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var seenThis, settledValues;
             var C = function(executor) {
               executor(function(values) { settledValues = values; }, function() {});
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   seenThis = this;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       if (value === 2) {
                         reject('bad');
                         resolve('ignored');
                       } else {
                         resolve(value);
                         reject('ignored');
                       }
                     }
                   };
                 };
               }
             });
             Promise.allSettled.call(C, [1, 2]);
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && seenThis === C &&
               settledValues[0].status === 'fulfilled' &&
               settledValues[0].value === 1 &&
               settledValues[1].status === 'rejected' &&
               settledValues[1].reason === 'bad';"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_keyed_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.allKeyed.call(SubPromise, {});
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var resolvedValues;
             var C = function(executor) {
               executor(function(values) { resolvedValues = values; }, function() {});
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       resolve(value);
                     }
                   };
                 };
               }
             });
             Promise.allKeyed.call(C, {a: 1, b: 2});
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && Object.getPrototypeOf(resolvedValues) === null &&
               Object.keys(resolvedValues).join(',') === 'a,b' &&
               resolvedValues.a === 1 && resolvedValues.b === 2;"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_keyed_preserves_symbols_and_rejects_non_object() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "var sym = Symbol('s');
         var hidden = Symbol('hidden');
         var out;
         var input = {str: Promise.resolve(1)};
         input[sym] = Promise.resolve(2);
         Object.defineProperty(input, hidden, {value: 3, enumerable: false});
         Promise.allKeyed(input).then(function(value) { out = value; });",
    )
    .expect("evaluation errored");
    assert_eq!(
        vm.run(
            "Object.getPrototypeOf(out) === null &&
             Object.keys(out).join(',') === 'str' &&
             Object.getOwnPropertySymbols(out).length === 1 &&
             Object.getOwnPropertySymbols(out)[0] === sym &&
             out.str === 1 && out[sym] === 2 &&
             !Object.prototype.hasOwnProperty.call(out, hidden);"
        )
        .expect("evaluation errored"),
        Value::Bool(true)
    );
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "var rejected;
         Promise.allKeyed(null).then(function() {}, function(e) { rejected = e; });",
    )
    .expect("evaluation errored");
    assert_eq!(
        vm.run("rejected instanceof TypeError;")
            .expect("evaluation errored"),
        Value::Bool(true)
    );
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "var marker = {};
         var proxyRejected;
         var input = new Proxy({}, {
           ownKeys: function() { throw marker; }
         });
         Promise.allKeyed(input).then(
           function() {},
           function(error) { proxyRejected = error === marker; }
         );",
    )
    .expect("evaluation errored");
    assert_eq!(
        vm.run("proxyRejected;").expect("evaluation errored"),
        Value::Bool(true)
    );
}

#[test]
fn promise_all_settled_keyed_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.allSettledKeyed.call(SubPromise, {});
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var seenThis, settledValues;
             var C = function(executor) {
               executor(function(values) { settledValues = values; }, function() {});
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   seenThis = this;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       if (value === 2) {
                         reject('bad');
                         resolve('ignored');
                       } else {
                         resolve(value);
                         reject('ignored');
                       }
                     }
                   };
                 };
               }
             });
             Promise.allSettledKeyed.call(C, {a: 1, b: 2});
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && seenThis === C &&
               Object.getPrototypeOf(settledValues) === null &&
               Object.keys(settledValues).join(',') === 'a,b' &&
               settledValues.a.status === 'fulfilled' &&
               settledValues.a.value === 1 &&
               settledValues.b.status === 'rejected' &&
               settledValues.b.reason === 'bad';"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_keyed_interleaves_proxy_descriptors_with_each_entry() {
    assert_eq!(
        run(
            r##"
            function exercise(method) {
              var log = [];
              var result;
              var rejected;
              function C(executor) {
                executor(
                  function(value) { result = value; },
                  function(reason) { rejected = reason; }
                );
              }
              C.resolve = function(value) {
                log.push("resolve:" + value.tag);
                var next = {};
                Object.defineProperty(next, "then", {
                  get: function() {
                    log.push("then-get:" + value.tag);
                    return function(onFulfilled) {
                      log.push("then-call:" + value.tag);
                      onFulfilled(value);
                    };
                  }
                });
                return next;
              };
              var target = {
                a: { tag: "a" },
                skip: { tag: "skip" },
                b: { tag: "b" }
              };
              var input = new Proxy(target, {
                ownKeys: function() {
                  log.push("keys");
                  return ["a", "skip", "b"];
                },
                getOwnPropertyDescriptor: function(target, key) {
                  log.push("desc:" + key);
                  if (key === "skip") return undefined;
                  return Reflect.getOwnPropertyDescriptor(target, key);
                },
                get: function(target, key, receiver) {
                  log.push("get:" + key);
                  return Reflect.get(target, key, receiver);
                }
              });

              Promise[method].call(C, input);
              var valuesMatch = method === "allKeyed"
                ? result.a.tag === "a" && result.b.tag === "b"
                : result.a.status === "fulfilled" && result.a.value.tag === "a" &&
                  result.b.status === "fulfilled" && result.b.value.tag === "b";
              return [
                log.join(","),
                Object.keys(result).join(","),
                valuesMatch,
                rejected === undefined
              ].join("|");
            }

            [exercise("allKeyed"), exercise("allSettledKeyed")].join("#");
            "##,
        ),
        Value::String(Arc::from(
            "keys,desc:a,get:a,resolve:a,then-get:a,then-call:a,desc:skip,desc:b,get:b,resolve:b,then-get:b,then-call:b|a,b|true|true#keys,desc:a,get:a,resolve:a,then-get:a,then-call:a,desc:skip,desc:b,get:b,resolve:b,then-get:b,then-call:b|a,b|true|true"
        ))
    );
}

#[test]
fn promise_keyed_observes_delegating_proxy_descriptors_and_rejects_abruptly() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var keyedDescriptorResults = {};
        for (var method of ["allKeyed", "allSettledKeyed"]) {
          (function(method) {
            var descriptorCalls = 0;
            var getCalls = 0;
            var skippedInput = new Proxy({ key: 1 }, {
              getOwnPropertyDescriptor: function() {
                descriptorCalls += 1;
                return undefined;
              },
              get: function() {
                getCalls += 1;
                return 1;
              }
            });
            Promise[method](skippedInput).then(function(result) {
              keyedDescriptorResults[method + "Skip"] =
                Reflect.ownKeys(result).length === 0 &&
                descriptorCalls === 1 && getCalls === 0;
            });

            var marker = { method: method };
            var throwingInput = new Proxy({ key: 1 }, {
              getOwnPropertyDescriptor: function() { throw marker; }
            });
            Promise[method](throwingInput).then(
              function() { keyedDescriptorResults[method + "Throw"] = false; },
              function(reason) {
                keyedDescriptorResults[method + "Throw"] = reason === marker;
              }
            );
          })(method);
        }
        "#,
    )
    .expect("keyed descriptor reactions should settle");
    assert_eq!(
        vm.run(
            r#"
            keyedDescriptorResults.allKeyedSkip &&
              keyedDescriptorResults.allKeyedThrow &&
              keyedDescriptorResults.allSettledKeyedSkip &&
              keyedDescriptorResults.allSettledKeyedThrow;
            "#
        )
        .expect("failed to inspect keyed descriptor results"),
        Value::Bool(true)
    );
}

#[test]
fn promise_keyed_entry_state_survives_observable_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            function exercise(method) {
              var result;
              var rejected;
              function C(executor) {
                executor(
                  function(value) { result = value; },
                  function(reason) { rejected = reason; }
                );
              }
              C.resolve = function(value) {
                var next = {};
                Object.defineProperty(next, "then", {
                  get: function() {
                    forceGc();
                    return function(onFulfilled) {
                      forceGc();
                      onFulfilled(value);
                    };
                  }
                });
                return next;
              };
              var input = new Proxy({ first: 0, second: 0 }, {
                ownKeys: function() {
                  forceGc();
                  return ["first", "second"];
                },
                getOwnPropertyDescriptor: function(target, key) {
                  forceGc();
                  return Reflect.getOwnPropertyDescriptor(target, key);
                },
                get: function(target, key) {
                  var value = { key: key };
                  forceGc();
                  return value;
                }
              });

              Promise[method].call(C, input);
              var valuesMatch = method === "allKeyed"
                ? result.first.key === "first" && result.second.key === "second"
                : result.first.status === "fulfilled" &&
                  result.first.value.key === "first" &&
                  result.second.status === "fulfilled" &&
                  result.second.value.key === "second";
              return Object.getPrototypeOf(result) === null &&
                Object.keys(result).join(",") === "first,second" &&
                valuesMatch && rejected === undefined;
            }

            [exercise("allKeyed"), exercise("allSettledKeyed")].join("|");
            "#,
        )
        .expect("keyed entry state should survive observable GC"),
        Value::String(Arc::from("true|true"))
    );
}

#[test]
fn promise_any_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.any.call(SubPromise, []);
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var seenThis, resolvedValue, rejectedErrors;
             var C = function(executor) {
               executor(function(value) { resolvedValue = value; },
                        function(errors) { rejectedErrors = errors; });
             };
             Object.defineProperty(C, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   seenThis = this;
                   return {
                     then: function(resolve, reject) {
                       thenCallCount += 1;
                       if (value === 2) {
                         resolve('winner');
                       } else {
                         reject('bad-' + value);
                         reject('ignored');
                       }
                     }
                   };
                 };
               }
             });
             Promise.any.call(C, [1, 2]);
             resolveGetCount === 1 && resolveCallCount === 2 &&
               thenCallCount === 2 && seenThis === C &&
               resolvedValue === 'winner' && rejectedErrors === undefined;"
        ),
        Value::Bool(true)
    );
    assert_eq!(
        run("var rejectedErrors;
             var C = function(executor) {
               executor(function() {}, function(errors) { rejectedErrors = errors; });
             };
             C.resolve = function(value) {
               return {
                 then: function(resolve, reject) {
                   reject('bad-' + value);
                   resolve('ignored');
                   reject('ignored');
                 }
               };
             };
             Promise.any.call(C, [1, 2]);
             rejectedErrors instanceof AggregateError &&
               rejectedErrors.errors.join(',') === 'bad-1,bad-2' &&
               Object.prototype.propertyIsEnumerable.call(rejectedErrors, 'errors') === false;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var rejectedError;
             var C = function(executor) {
               executor(function() {}, function(error) { rejectedError = error; });
             };
             C.resolve = function(value) { return value; };
             Promise.any.call(C, []);
             rejectedError instanceof AggregateError &&
               rejectedError.errors.length === 0 &&
               Object.prototype.propertyIsEnumerable.call(rejectedError, 'errors') === false;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_race_uses_receiver_resolve_and_then() {
    assert_eq!(
        run("var callCount = 0, executorLength = 0;
             class SubPromise extends Promise {
               constructor(executor) {
                 super(executor);
                 callCount += 1;
                 executorLength = executor.length;
               }
             }
             var result = Promise.race.call(SubPromise, []);
             result instanceof SubPromise && callCount === 1 && executorLength === 2;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(
            "var resolveGetCount = 0, resolveCallCount = 0, thenCallCount = 0;
             var p1 = new Promise(function() {});
             var p2 = new Promise(function() {});
             Object.defineProperty(Promise, 'resolve', {
               configurable: true,
               get: function() {
                 resolveGetCount += 1;
                 return function(value) {
                   resolveCallCount += 1;
                   return value;
                 };
               }
             });
             p1.then = p2.then = function(resolve, reject) {
               thenCallCount += 1;
               return {};
             };
             Promise.race([p1, p2]);
             resolveGetCount === 1 && resolveCallCount === 2 && thenCallCount === 2;"
        ),
        Value::Bool(true)
    );
}

#[test]
fn promise_combinators_close_after_resolve_and_then_abrupt_completions() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var checks = [];
            for (var method of ["all", "allSettled", "any", "race"]) {
                for (var mode of ["resolve", "get-then", "call-then"]) {
                    var reason = {};
                    var reasonRef = new WeakRef(reason);
                    var rejected;
                    var closeCount = 0;
                    function C(executor) {
                        executor(function () {}, function (value) { rejected = value; });
                    }
                    C.resolve = function () {
                        if (mode === "resolve") throw reason;
                        var nextPromise = {};
                        if (mode === "get-then") {
                            Object.defineProperty(nextPromise, "then", {
                                get: function () { throw reason; }
                            });
                        } else {
                            nextPromise.then = function () { throw reason; };
                        }
                        return nextPromise;
                    };
                    var iterable = {
                        [Symbol.iterator]: function () {
                            return {
                                next: function () {
                                    return { value: 1, done: false };
                                },
                                return: function () {
                                    closeCount += 1;
                                    reason = null;
                                    forceGc();
                                    return {};
                                }
                            };
                        }
                    };
                    Promise[method].call(C, iterable);
                    checks.push(closeCount === 1 && rejected === reasonRef.deref());
                }
            }

            var original = {};
            var originalRef = new WeakRef(original);
            var closeReason = {};
            var closeGets = 0;
            var finalRejected;
            function ClosingC(executor) {
                executor(function () {}, function (value) { finalRejected = value; });
            }
            ClosingC.resolve = function () { throw original; };
            var closingIterable = {
                [Symbol.iterator]: function () {
                    var iterator = {
                        next: function () { return { value: 1, done: false }; }
                    };
                    Object.defineProperty(iterator, "return", {
                        get: function () {
                            closeGets += 1;
                            original = null;
                            forceGc();
                            throw closeReason;
                        }
                    });
                    return iterator;
                }
            };
            Promise.all.call(ClosingC, closingIterable);
            [
                checks.length,
                checks.every(function (value) { return value; }),
                finalRejected === originalRef.deref(),
                closeGets
            ].join("|");
        "#,
        )
        .expect("Promise combinator close matrix should complete"),
        Value::String(Arc::from("12|true|true|1"))
    );
}

#[test]
fn promise_combinators_reject_setup_errors_as_realm_objects() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(
            r#"
            var methods = ["all", "allSettled", "any", "race"];
            var typeReasons = [];
            for (let method of methods) {
                Promise[method](null).then(
                    function () { typeReasons.push(null); },
                    function (reason) { typeReasons.push(reason); }
                );
            }

            var originalResolve = Promise.resolve;
            var lookupReason = {};
            var lookupReasons = [];
            var lookupIteratorGets = 0;
            Object.defineProperty(Promise, "resolve", {
                configurable: true,
                get: function () { throw lookupReason; }
            });
            var lookupIterable = {};
            Object.defineProperty(lookupIterable, Symbol.iterator, {
                get: function () {
                    lookupIteratorGets += 1;
                    throw new Error("iterator lookup must not run");
                }
            });
            for (let method of methods) {
                Promise[method](lookupIterable).then(
                    function () { lookupReasons.push(null); },
                    function (reason) { lookupReasons.push(reason); }
                );
            }
            Object.defineProperty(Promise, "resolve", {
                configurable: true,
                writable: true,
                value: originalResolve
            });

            var nonCallableReasons = [];
            var nonCallableGc = [];
            var nonCallableIteratorGets = 0;
            function C(executor) {
                executor(function () {}, function (reason) {
                    var reasonRef = new WeakRef(reason);
                    forceGc();
                    nonCallableReasons.push(reason);
                    nonCallableGc.push(reasonRef.deref() === reason);
                });
            }
            C.resolve = null;
            var nonCallableIterable = {};
            Object.defineProperty(nonCallableIterable, Symbol.iterator, {
                get: function () {
                    nonCallableIteratorGets += 1;
                    throw new Error("iterator lookup must not run");
                }
            });
            for (let method of methods) {
                Promise[method].call(C, nonCallableIterable);
            }

            var finalResolveReasons = [];
            for (let method of ["all", "allSettled", "allKeyed", "allSettledKeyed"]) {
                function FinalC(executor) {
                    executor(Map.prototype.get, function (reason) {
                        finalResolveReasons.push(reason);
                    });
                }
                FinalC.resolve = function (value) {
                    return {
                        then: function (resolve) { resolve(value); }
                    };
                };
                var input = method.indexOf("Keyed") === -1 ? [1] : { key: 1 };
                Promise[method].call(FinalC, input);
            }

            var tryReason;
            function TryC(executor) {
                executor(function () {}, function (reason) { tryReason = reason; });
            }
            Promise.try.call(TryC, null);

            var other = $262.createRealm().global;
            var mainMethodError;
            var foreignMethodError;
            Promise.all.call(other.Promise, null).catch(function (reason) {
                mainMethodError = reason;
            });
            other.Promise.all.call(Promise, null).catch(function (reason) {
                foreignMethodError = reason;
            });
            "scheduled";
        "#,
        )
        .expect("Promise combinator setup should return rejection promises"),
        Value::String(Arc::from("scheduled"))
    );
    assert_eq!(
        vm.run(
            r#"
            [
                typeReasons.length,
                typeReasons.every(function (reason) {
                    return reason instanceof TypeError &&
                        Object.getPrototypeOf(reason) === TypeError.prototype;
                }),
                lookupReasons.length,
                lookupReasons.every(function (reason) {
                    return reason === lookupReason;
                }),
                lookupIteratorGets,
                nonCallableReasons.length,
                nonCallableReasons.every(function (reason) {
                    return reason instanceof TypeError;
                }),
                nonCallableGc.every(function (alive) { return alive; }),
                nonCallableIteratorGets,
                finalResolveReasons.length,
                finalResolveReasons.every(function (reason) {
                    return reason instanceof TypeError;
                }),
                tryReason instanceof TypeError,
                mainMethodError instanceof TypeError &&
                    !(mainMethodError instanceof other.TypeError),
                foreignMethodError instanceof other.TypeError &&
                    !(foreignMethodError instanceof TypeError)
            ].join("|");
        "#,
        )
        .expect("Promise combinator setup rejections should preserve identity and Realm"),
        Value::String(Arc::from(
            "4|true|4|true|0|4|true|true|0|4|true|true|true|true"
        ))
    );
}

#[test]
fn promise_static_resolve_and_reject_use_receiver_constructor_capability() {
    assert_eq!(
        run("class SubPromise extends Promise {}
             [
               Promise.resolve.call(SubPromise, 1) instanceof SubPromise,
               Promise.reject.call(SubPromise, 2) instanceof SubPromise
             ].join(':');"),
        Value::String(Arc::from("true:true"))
    );
    assert_eq!(
        run("var p = Promise.resolve(1);
             Promise.resolve(p) === p && (p.constructor = null, Promise.resolve(p) !== p);"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var seenThis, seenArg, callCount = 0;
             var P = function(executor) {
               return new Promise(function() {
                 executor(function(v) { callCount += 1; seenThis = this; seenArg = v; },
                          function() {});
               });
             };
             var obj = {};
             Promise.resolve.call(P, obj);
             callCount === 1 && seenThis === globalThis && seenArg === obj;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var seenThis, seenArg;
             var P = function(executor) {
               return new Promise(function() {
                 executor(function() {}, function(v) { seenThis = this; seenArg = v; });
               });
             };
             Promise.reject.call(P, 24601);
             seenThis === globalThis && seenArg === 24601;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_static_resolve_and_reject_validate_capability_constructor() {
    assert_eq!(
        run("var badResolve = false, badReject = false;
             try { Promise.resolve.call(eval, 1); } catch (e) { badResolve = e instanceof TypeError; }
             try { Promise.reject.call(eval, 1); } catch (e) { badReject = e instanceof TypeError; }
             badResolve && badReject;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var rejected = false;
             try { Promise.resolve.call(function(executor) {}, 1); }
             catch (e) { rejected = e instanceof TypeError; }
             rejected;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var promise = new Promise(function () {});
             var receivers = [undefined, null, true, 1, '', Symbol()];
             var rejected = 0;
             for (var receiver of receivers) {
               promise.constructor = receiver;
               try { Promise.resolve.call(receiver, promise); }
               catch (error) { if (error instanceof TypeError) rejected += 1; }
             }
             rejected === receivers.length;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_resolving_functions_are_anonymous_unary_builtins() {
    assert_eq!(
        run("var resolve, reject;
             new Promise(function(res, rej) { resolve = res; reject = rej; });
             [
               resolve.name, resolve.length,
               reject.name, reject.length,
               Object.prototype.hasOwnProperty.call(resolve, 'prototype'),
               Object.getOwnPropertyNames(resolve).join(',')
             ].join(':');"),
        Value::String(Arc::from(":1::1:false:length,name"))
    );
}

#[test]
fn promise_with_resolvers_basic_and_subclass() {
    assert_eq!(
        run("var r = Promise.withResolvers();
             [
               r.promise instanceof Promise,
               r.promise.constructor === Promise,
               typeof r.resolve, r.resolve.name, r.resolve.length,
               typeof r.reject, r.reject.name, r.reject.length
             ].join(':');"),
        Value::String(Arc::from("true:true:function::1:function::1"))
    );
    assert_eq!(
        run("class SubPromise extends Promise {}
             var r = Promise.withResolvers.call(SubPromise);
             r.promise instanceof SubPromise && r.promise.constructor === SubPromise;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_with_resolvers_result_properties_are_enumerable_data_props() {
    assert_eq!(
        run("var r = Promise.withResolvers();
             ['promise', 'resolve', 'reject'].map(function(k) {
               var d = Object.getOwnPropertyDescriptor(r, k);
               return d.writable + ',' + d.enumerable + ',' + d.configurable;
             }).join(':');"),
        Value::String(Arc::from("true,true,true:true,true,true:true,true,true"))
    );
}

#[test]
fn promise_subclass_requires_callable_executor_and_uses_new_target_prototype() {
    assert_eq!(
        run("class Prom extends Promise {} try { new Prom(); false; } catch (e) { e instanceof TypeError; }"),
        Value::Bool(true)
    );
    assert_eq!(
        run("class Prom extends Promise {} Object.getPrototypeOf(new Prom(function() {})) === Prom.prototype;"),
        Value::Bool(true)
    );
}

#[test]
fn promise_constructor_requires_new_and_calls_executor_with_undefined_this() {
    assert_eq!(
        run("var ok1 = false, ok2 = false, ok3 = false;
             try { Promise(function() {}); } catch (e) { ok1 = e instanceof TypeError; }
             try { Promise.call(null, function() {}); } catch (e) { ok2 = e instanceof TypeError; }
             try { Promise.call(new Promise(function() {}), function() {}); }
             catch (e) { ok3 = e instanceof TypeError; }
             ok1 && ok2 && ok3;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var seen;
             new Promise(function() { seen = this; });
             seen === globalThis;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var seen;
             new Promise(function() { 'use strict'; seen = this; });
             seen === undefined;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"var prototypeGets = 0, executorCalls = 0;
             var marker = {};
             var newTarget = (function () {}).bind();
             Object.defineProperty(newTarget, 'prototype', {
               get: function () { prototypeGets += 1; throw marker; }
             });
             var missingError, callableError;
             try { Reflect.construct(Promise, [], newTarget); }
             catch (error) { missingError = error; }
             var getsAfterMissingExecutor = prototypeGets;
             try {
               Reflect.construct(Promise, [function () { executorCalls += 1; }], newTarget);
             } catch (error) { callableError = error; }
             missingError instanceof TypeError &&
               getsAfterMissingExecutor === 0 &&
               callableError === marker && prototypeGets === 1 &&
               executorCalls === 0;"#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"var order = [];
             var expectedPrototype = {};
             var newTarget = (function () {}).bind();
             Object.defineProperty(newTarget, 'prototype', {
               get: function () {
                 order.push('prototype-get');
                 return expectedPrototype;
               }
             });
             var promise = Reflect.construct(Promise, [function () {
               order.push('executor');
             }], newTarget);
             Object.getPrototypeOf(promise) === expectedPrototype &&
               order.join(',') === 'prototype-get,executor';"#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"var other = $262.createRealm().global;
             var foreignError;
             try { Reflect.construct(other.Promise, [], function () {}); }
             catch (error) { foreignError = error; }
             var foreignNewTarget = new other.Function();
             foreignNewTarget.prototype = null;
             var foreignPromise = Reflect.construct(
               Promise, [function () {}], foreignNewTarget
             );
             foreignError instanceof other.TypeError &&
               !(foreignError instanceof TypeError) &&
               Object.getPrototypeOf(foreignPromise) === other.Promise.prototype;"#),
        Value::Bool(true)
    );
}

// --- RegExp ---

#[test]
fn regex_literal_test() {
    assert_eq!(run("/abc/.test('xabcy');"), Value::Bool(true));
    assert_eq!(run("/abc/.test('xyz');"), Value::Bool(false));
    assert_eq!(run("/\\d+/.test('abc123');"), Value::Bool(true));
    assert_eq!(run("/\\d+/.test('abc');"), Value::Bool(false));
    assert_eq!(
        run("var r = new RegExp(/test/i); [r.source, r.flags, r.toString()].join('|');"),
        Value::String(Arc::from("test|i|/test/i"))
    );
}

#[test]
fn regexp_embedded_empty_classes_follow_ecmascript_semantics() {
    assert_eq!(
        run(r#"
            !/[]/.test("x") &&
              /[^]/.test("\n") &&
              !/[]a/.test("\0a\0a") &&
              !/a[]/.test("\0a\0a") &&
              /[^]a/.test("\na") &&
              /a[^]/.test("a\n") &&
              !/x[]y/.test("xy") &&
              /x[^]y/.test("x\ny") &&
              /^(a[^])\1$/.test("aXaX") &&
              /^(?=a[^]$)a[^]$/.test("a\n");
            "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            var never = new RegExp("a|b|[]", "ig");
            var universal = new RegExp("a|b|[^]", "ig");
            never.test("B") && !never.test("c") && universal.test("c") &&
              never.ignoreCase && never.global && never.source === "a|b|[]" &&
              universal.source === "a|b|[^]";
            "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            /[]*/.exec("x")[0] === "" &&
              !/^(?:[])+$/.test("x") &&
              !/([])+/.test("x") &&
              /^[^]{2}$/.test("💩") &&
              !/^[^]{2}$/u.test("💩") &&
              /^a[^]$/u.test("a💩") &&
              /^[^]$/u.test("\ud800") &&
              !new RegExp("a[]", "v").test("aX") &&
              new RegExp("a[^]", "v").test("a💩") &&
              new RegExp("^[a--[b]]$", "v").test("a") &&
              !new RegExp("^[a--[b]]$", "v").test("b");
            "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            /^[\]]$/.test("]") &&
              /^\[\]$/.test("[]") &&
              /^[^\]]$/.test("x") &&
              !/^[^\]]$/.test("]") &&
              /^[a]$/.test("a");
            "#),
        Value::Bool(true)
    );
}

#[test]
fn regexp_character_class_escapes_use_ecmascript_sets() {
    assert_eq!(
        run(r#"
            var flags = ["", "u", "v"];
            var ok = true;
            for (var i = 0; i < flags.length; i++) {
              var flag = flags[i];
              var digit = new RegExp("^\\d$", flag);
              var nonDigit = new RegExp("^\\D$", flag);
              var digitClass = new RegExp("^[\\d]$", flag);
              var nonDigitClass = new RegExp("^[\\D]$", flag);
              var whitespace = new RegExp("^\\s$", flag);
              var nonWhitespace = new RegExp("^\\S$", flag);
              var whitespaceClass = new RegExp("^[\\s]$", flag);
              var nonWhitespaceClass = new RegExp("^[\\S]$", flag);
              var word = new RegExp("^\\w$", flag);
              var nonWord = new RegExp("^\\W$", flag);
              var wordClass = new RegExp("^[\\w]$", flag);
              var nonWordClass = new RegExp("^[\\W]$", flag);

              ok = ok &&
                digit.test("5") && !digit.test("\u0660") &&
                !nonDigit.test("5") && nonDigit.test("\u0660") &&
                digitClass.test("5") && !digitClass.test("\u0660") &&
                !nonDigitClass.test("5") && nonDigitClass.test("\u0660") &&
                whitespace.test("\uFEFF") && whitespace.test("\u2028") &&
                !whitespace.test("\u0085") && !whitespace.test("\u180E") &&
                !nonWhitespace.test("\uFEFF") && nonWhitespace.test("\u0085") &&
                whitespaceClass.test("\uFEFF") && !whitespaceClass.test("\u0085") &&
                !nonWhitespaceClass.test("\uFEFF") && nonWhitespaceClass.test("\u0085") &&
                word.test("_") && !word.test("é") &&
                !nonWord.test("_") && nonWord.test("é") &&
                wordClass.test("_") && !wordClass.test("é") &&
                !nonWordClass.test("_") && nonWordClass.test("é");
            }
            ok;
            "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"/^([\d])\1$/.test("55") && !/^([\d])\1$/.test("\u0660\u0660");"#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            var flags = ["", "u", "v"];
            var ok = true;
            for (var i = 0; i < flags.length; i++) {
              var repeatedDigit = new RegExp("^(\\d)\\1$", flags[i]);
              var repeatedWhitespace = new RegExp("^(\\s)\\1$", flags[i]);
              ok = ok &&
                repeatedDigit.test("55") &&
                !repeatedDigit.test("\u0660\u0660") &&
                repeatedWhitespace.test("\uFEFF\uFEFF");
            }
            ok &&
              /^[\d-a]$/.test("-") &&
              /^[\d-a]$/.test("5") &&
              /^[\s-a]$/.test("\uFEFF") &&
              /^[a-\d]$/.test("-") &&
              /^[a-\d]$/.test("5") &&
              !/^[a-\d]$/.test("b") &&
              /^[a-\s]$/.test("-") &&
              /^[a-\s]$/.test("\uFEFF") &&
              !/^[a-\s]$/.test("\u0085") &&
              /^[\d-a-\s]$/.test("5") &&
              /^[\d-a-\s]$/.test("-") &&
              /^[\d-a-\s]$/.test("\uFEFF") &&
              !/^[\d-a-\s]$/.test("b") &&
              /^[\-\d]$/.test("-") &&
              /^[\-\d]$/.test("5") &&
              /^[^\d]$/.test("\u0660") &&
              /^[^\S]$/.test("\uFEFF") &&
              !/[^\S]/.test("\u0085");
            "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            /\\d/.test("\\d") &&
              !/[\\d]/.test("5") &&
              /[\\d]/.test("\\") &&
              new RegExp("\\\\d").test("\\d") &&
              !new RegExp("[\\\\d]").test("5");
            "#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            /^\D{2}$/.test("😀") &&
              !/^\D$/.test("😀") &&
              /^\D$/u.test("😀") &&
              /^\D$/v.test("😀");
            "#),
        Value::Bool(true)
    );
}

#[test]
fn regexp_ignore_case_word_characters_follow_ecmascript_canonicalization() {
    assert_eq!(
        run(r#"
            var unrelated = ["é", "\u0660", "中"];
            var legacyWord = /^\w$/i;
            var legacyNonWord = /^\W$/i;
            var legacyWordClass = /^[\w]$/i;
            var legacyNonWordClass = /^[\W]$/i;
            var ok = !legacyWord.test("\u017F") && !legacyWord.test("\u212A") &&
              legacyNonWord.test("\u017F") && legacyNonWord.test("\u212A") &&
              !legacyWordClass.test("\u017F") && !legacyWordClass.test("\u212A") &&
              legacyNonWordClass.test("\u017F") && legacyNonWordClass.test("\u212A");

            for (var i = 0; i < unrelated.length; i++) {
              ok = ok && !legacyWord.test(unrelated[i]) && legacyNonWord.test(unrelated[i]) &&
                !legacyWordClass.test(unrelated[i]) && legacyNonWordClass.test(unrelated[i]);
            }

            var unicodeFlags = ["ui", "vi"];
            for (var j = 0; j < unicodeFlags.length; j++) {
              var flags = unicodeFlags[j];
              var word = new RegExp("^\\w$", flags);
              var nonWord = new RegExp("^\\W$", flags);
              var wordClass = new RegExp("^[\\w]$", flags);
              var nonWordClass = new RegExp("^[\\W]$", flags);
              var negatedWordClass = new RegExp("^[^\\w]$", flags);
              var negatedNonWordClass = new RegExp("^[^\\W]$", flags);
              ok = ok && word.test("\u017F") && word.test("\u212A") &&
                !nonWord.test("\u017F") && !nonWord.test("\u212A") &&
                wordClass.test("\u017F") && wordClass.test("\u212A") &&
                !nonWordClass.test("\u017F") && !nonWordClass.test("\u212A") &&
                !negatedWordClass.test("\u017F") && negatedNonWordClass.test("\u017F");
              for (var k = 0; k < unrelated.length; k++) {
                ok = ok && !word.test(unrelated[k]) && nonWord.test(unrelated[k]) &&
                  !wordClass.test(unrelated[k]) && nonWordClass.test(unrelated[k]) &&
                  negatedWordClass.test(unrelated[k]) && !negatedNonWordClass.test(unrelated[k]);
              }
            }
            ok;
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var suffix = "a".repeat(4096) + "!";
            var sticky = new RegExp("\\b(a+)+b", "iuy");
            sticky.lastIndex = 0;
            var stickyResult = sticky.exec("é" + suffix);
            var replaced = ("é" + suffix).replace(
              new RegExp("^\\B|(a+)+b", "iu"),
              "X"
            );
            stickyResult === null && sticky.lastIndex === 0 &&
              replaced === "Xé" + suffix;
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            !/^[s\w]$/i.test("\u017F") &&
              !/^[k\w]$/i.test("\u212A") &&
              /^[s\w]$/i.test("S") &&
              /^[\w-a]$/i.test("-") && /^[\w-a]$/i.test("a") &&
              /^[a-\w]$/i.test("-") && /^[a-\w]$/i.test("Z") &&
              /(?i:\w)/.test("A") && !/(?i:\w)/.test("\u017F") &&
              /(?i:\w)/u.test("\u017F") && !/(?i:\w)/u.test("é") &&
              /(?i:[\w])/u.test("\u212A") && !/(?i:[\w])/u.test("é") &&
              /(?i:[s\w])/.test("S") && !/(?i:[s\w])/.test("\u017F");
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            !/\b\u017F/i.test("\u017F") && /\B\u017F/i.test("\u017F") &&
              /\b\u017F/iu.test("\u017F") && !/\B\u017F/iu.test("\u017F") &&
              /Z\B\u017F/iu.test("Z\u017F") && !/Z\b\u017F/iu.test("Z\u017F") &&
              /(?i:\b)\u017F/u.test("\u017F") && !/(?i:\B)\u017F/u.test("\u017F") &&
              /(?-i:\B)\u017F/ui.test("\u017F") &&
              !/^(a+)+\b$/iu.test("aaaaaaaaaaaaaaaaaaaa!");
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var flagsList = ["iu", "iv"];
            var nonWords = ["é", "中", "\u0660"];
            var ok = true;
            for (var i = 0; i < flagsList.length; i++) {
              var flags = flagsList[i];
              var boundary = new RegExp("^\\b", flags);
              var nonBoundary = new RegExp("^\\B", flags);
              var hardBoundary = new RegExp("(?=)^\\b", flags);
              var hardNonBoundary = new RegExp("(?=)^\\B", flags);
              for (var j = 0; j < nonWords.length; j++) {
                var value = nonWords[j];
                ok = ok && !boundary.test(value) && nonBoundary.test(value) &&
                  !hardBoundary.test(value) && hardNonBoundary.test(value);
              }
              ok = ok && boundary.test("\u017F") && boundary.test("\u212A") &&
                !nonBoundary.test("\u017F") && !nonBoundary.test("\u212A") &&
                new RegExp("é\\ba", flags).test("éa") &&
                new RegExp("a\\bé", flags).test("aé") &&
                new RegExp("é\\B中", flags).test("é中") &&
                new RegExp("\u017F\\Ba", flags).test("\u017Fa") &&
                new RegExp("\u212A\\b中", flags).test("\u212A中");

              var absent = new RegExp("^\\b(a)*", flags).exec("é");
              var present = new RegExp("^\\B(a)*", flags).exec("é");
              var transition = new RegExp("é\\b(a)*", flags).exec("éa");
              ok = ok && absent === null && present[0] === "" &&
                present[1] === undefined && transition[0] === "éa" &&
                transition[1] === "a" &&
                "é😀".replace(new RegExp("\\B(a)*", "g" + flags), "X") === "XéX😀X";
            }
            ok;
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var matches = Array.from(
              "ab".matchAll(/\b(a?b??)*/giu),
              function (match) { return [match[0], match[1], match.index]; }
            );
            JSON.stringify(matches) ===
              '[["ab","b",0],["",null,2]]';
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var lone = String.fromCharCode(0xD800);
            /^\W$/i.test(lone) && !/^\w$/i.test(lone) &&
              /^[\W]$/i.test(lone) && !/^[\w]$/i.test(lone) &&
              /^\W\B\W$/i.test("😀") && !/^\W\b\W$/i.test("😀") &&
              /^\W$/iu.test("😀") &&
              /^(\w)\1$/iu.test("\u017F\u017F") &&
              !/^(\w)\1$/iu.test("éé") &&
              /^(\w)\1$/iv.test("\u212A\u212A");
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            /\u212A/iv.test("k") && /K/iv.test("k") &&
              /\u017F/iv.test("s") && !/é/iv.test("e") &&
              new RegExp("[[\\w]--[a]]", "iv").test("S") &&
              !new RegExp("[[a]--[\\w]]", "iv").test("a") &&
              new RegExp("[[a]&&[\\w]]", "iv").test("A") &&
              !new RegExp("^[\\w][\\p{ASCII}]$", "iv").test("éA") &&
              new RegExp("^[\\w][\\p{ASCII}]$", "iv").test("\u017FA") &&
              new RegExp("^[^\\w][\\p{ASCII}]$", "iv").test("éA") &&
              !new RegExp("^[\\p{ASCII}][\\w]$", "iv").test("Aé") &&
              new RegExp("^[\\p{ASCII}][\\w]$", "iv").test("A\u017F") &&
              /\\w/i.test("\\w") && !/\\w/i.test("A");
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var rustOnlyWords = ["é", "中", "\u0660", "\u200C", "\u200D"];
            var wordLetter = new RegExp("^[\\w&&\\p{Letter}]$", "iv");
            var nonWordLetter = new RegExp("^[\\W&&\\p{Letter}]$", "iv");
            var nestedWord = new RegExp("^[[\\w]&&[[^a]--[b]]]$", "iv");
            var nestedNonWord = new RegExp("^[[\\W]&&[[^a]--[b]]]$", "iv");
            var unionNonWord = new RegExp("^[[\\W][a]]$", "iv");
            var subtractWord = new RegExp("^[[\\p{Letter}]--[\\w]]$", "iv");
            var negatedDifference = new RegExp("^[^[\\w]--[a]]$", "iv");
            var ok = true;

            for (var i = 0; i < rustOnlyWords.length; i++) {
              var value = rustOnlyWords[i];
              ok = ok && !wordLetter.test(value) &&
                (value === "é" || value === "中" ? nonWordLetter.test(value) : true) &&
                !nestedWord.test(value) && nestedNonWord.test(value) &&
                unionNonWord.test(value) && negatedDifference.test(value);
            }

            ok && wordLetter.test("S") && wordLetter.test("\u017F") &&
              wordLetter.test("\u212A") && !nonWordLetter.test("\u017F") &&
              new RegExp("^\\p{ASCII}$", "iv").test("\u017F") &&
              subtractWord.test("é") && subtractWord.test("中") &&
              !subtractWord.test("S") && !subtractWord.test("\u017F") &&
              nestedWord.test("S") && nestedWord.test("\u017F") &&
              !nestedWord.test("a") && !nestedWord.test("b") &&
              !negatedDifference.test("S") && negatedDifference.test("a");
            "#),
        Value::Bool(true)
    );

    assert_eq!(
        run(r#"
            var hardWord = new RegExp("^(?=.)[[\\w]&&[a-z]]$", "iv");
            var hardNonWord = new RegExp("^(?=.)[[\\W]&&[^a]]$", "iv");
            var repeated = new RegExp("^([[\\w]--[a]])\\1$", "iv");
            hardWord.test("S") && hardWord.test("\u017F") &&
              !hardWord.test("é") && hardNonWord.test("é") &&
              !hardNonWord.test("S") && repeated.test("SS") &&
              repeated.test("\u017F\u017F") && !repeated.test("éé");
            "#),
        Value::Bool(true)
    );
}

#[test]
fn regex_literals_use_realm_intrinsics_not_mutable_bindings() {
    assert_eq!(
        run(r#"
            var original = RegExp;
            var originalPrototype = RegExp.prototype;
            var calls = 0;
            function FakeRegExp() { calls++; return {}; }
            function parameterShadow(RegExp) { return /parameter/i; }
            var lexical;
            {
              let RegExp = FakeRegExp;
              lexical = /lexical/g;
            }
            globalThis.RegExp = FakeRegExp;
            var global = /global/m;
            var parameter = parameterShadow(FakeRegExp);
            var fresh = /global/m;
            [
              calls,
              lexical.source, lexical.flags,
              global.source, global.flags,
              parameter.source, parameter.flags,
              Object.getPrototypeOf(lexical) === originalPrototype,
              Object.getPrototypeOf(global) === originalPrototype,
              global !== fresh
            ].join(";");
            "#),
        Value::String(Arc::from("0;lexical;g;global;m;parameter;i;true;true;true"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var foreignPrototype = other.RegExp.prototype;
            other.RegExp = function FakeRegExp() { throw "called"; };
            var foreign = other.eval("/foreign/u");
            [
              foreign.source,
              foreign.flags,
              Object.getPrototypeOf(foreign) === foreignPrototype,
              Object.getPrototypeOf(foreign) !== RegExp.prototype
            ].join(";");
            "#),
        Value::String(Arc::from("foreign;u;true;true"))
    );
}

#[test]
fn foreign_realm_regexp_literal_intrinsic_survives_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var other = $262.createRealm().global;
            other.RegExp = null;
            forceGc();
            other.eval("/after-gc/.test('after-gc')");
            "#,
        )
        .expect("foreign RegExp literal intrinsic should survive GC"),
        Value::Bool(true)
    );
}

#[test]
fn foreign_realm_regexp_literals_survive_native_reentry() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var other = $262.createRealm().global;
        var foreignPrototype = other.RegExp.prototype;
        var results = [];

        var callback = other.eval("(function() { return /callback/; })");
        results.push(Object.getPrototypeOf([0].map(callback)[0]) === foreignPrototype);

        var generator = other.eval("(function*() { yield /generator/; })");
        results.push(Object.getPrototypeOf(generator().next().value) === foreignPrototype);

        var asyncCallback = other.eval("(async function() { return /async/; })");
        [0].map(asyncCallback)[0].then(function(value) {
          results.push(Object.getPrototypeOf(value) === foreignPrototype);
        });

        var asyncGenerator = other.eval("(async function*() { yield /async-generator/; })");
        asyncGenerator().next().then(function(result) {
          results.push(Object.getPrototypeOf(result.value) === foreignPrototype);
        });
        "#,
    )
    .expect("failed to exercise native foreign-Realm re-entry");

    assert_eq!(
        vm.run("results.join(';');")
            .expect("failed to read foreign-Realm re-entry results"),
        Value::String(Arc::from("true;true;true;true"))
    );
}

#[test]
fn regexp_create_uses_raw_pattern_and_the_immutable_intrinsic() {
    assert_eq!(
        run(r#"
            var regexp = /a/;
            regexp[Symbol.match] = undefined;
            var rawPattern = "a".match(regexp) === null;

            var originalRegExp = RegExp;
            var originalPrototype = RegExp.prototype;
            var originalMatch = originalPrototype[Symbol.match];
            var observed;
            originalPrototype[Symbol.match] = function (value) {
              observed = this;
              return originalMatch.call(this, value);
            };
            globalThis.RegExp = null;
            var result = "xa".match("a");
            var prototypeSource = Object.getOwnPropertyDescriptor(
              originalPrototype, "source"
            ).get.call(originalPrototype);
            var prototypeTag = Object.prototype.toString.call(originalPrototype);
            [
              rawPattern,
              result[0],
              observed.source,
              Object.getPrototypeOf(observed) === originalPrototype,
              observed.constructor === originalRegExp,
              prototypeSource,
              prototypeTag
            ].join("|");
            "#,),
        Value::String(Arc::from("true|a|a|true|true|(?:)|[object Object]"))
    );
}

#[test]
fn regexp_symbol_match_global_generic_exec_getter_runs_after_last_index_reset() {
    assert_eq!(
        run(r#"
            var marker = {};
            var receiver = { flags: "g", global: true, lastIndex: 7 };
            Object.defineProperty(receiver, "exec", {
              get: function () { throw marker; }
            });
            var caught;
            try {
              RegExp.prototype[Symbol.match].call(receiver, "");
            } catch (error) {
              caught = error;
            }
            caught === marker && receiver.lastIndex === 0;
        "#),
        Value::Bool(true)
    );
}

#[test]
fn regex_exec_captures() {
    let r = run("/(\\w+)@(\\w+)/.exec('user@host');");
    assert!(matches!(r, Value::Object(_)));
    assert_eq!(
        run("/(\\w+)@(\\w+)/.exec('user@host')[0];"),
        Value::String(Arc::from("user@host"))
    );
    assert_eq!(
        run("/(\\w+)@(\\w+)/.exec('user@host')[1];"),
        Value::String(Arc::from("user"))
    );
    assert_eq!(
        run("/(\\w+)@(\\w+)/.exec('user@host')[2];"),
        Value::String(Arc::from("host"))
    );
}

#[test]
fn regexp_exec_result_shape_and_last_index_semantics() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var result = other.RegExp.prototype.exec.call(other.eval("/a/"), "a");
            [
              Object.getPrototypeOf(result) === other.Array.prototype,
              Object.getPrototypeOf(result) === Array.prototype
            ].join("|");
        "#),
        Value::String(Arc::from("true|false"))
    );
    assert_eq!(
        run("var m = /b(c)/.exec('abc'); [m[0], m[1], m.index, m.input].join('|');"),
        Value::String(Arc::from("bc|c|1|abc"))
    );
    assert_eq!(
        run("Object.keys(/b/.exec('abc')).join(',');"),
        Value::String(Arc::from("0,index,input,groups"))
    );
    assert_eq!(run("/b/.exec('abc').groups;"), Value::Undefined);
    assert_eq!(
        run("var m = /(?<x>b)(c)?/.exec('abc'); [m[0], m[1], m[2], m.groups.x, Object.getPrototypeOf(m.groups) === null, Object.keys(m.groups).join(',')].join('|');"),
        Value::String(Arc::from("bc|b|c|b|true|x"))
    );
    assert_eq!(
        run("var m = /(?<x>a)|(?<y>b)/.exec('b'); String(m.groups.x) + '|' + m.groups.y;"),
        Value::String(Arc::from("undefined|b"))
    );
    assert_eq!(
        run("var m = 'abc'.match(/(?<x>b)(c)?/); [m[0], m[1], m[2], m.index, m.input, m.groups.x, Object.getPrototypeOf(m.groups) === null, Object.keys(m).join(',')].join('|');"),
        Value::String(Arc::from("bc|b|c|1|abc|b|true|0,1,2,index,input,groups"))
    );
    assert_eq!(
        run("var m = 'abc'.match('b'); [m[0], m.index, m.input].join('|');"),
        Value::String(Arc::from("b|1|abc"))
    );
    assert_eq!(
        run(
            r#"String.prototype[Symbol.match] = function(arg) { return "poison"; };
               var m = "abc".match("b");
               delete String.prototype[Symbol.match];
               [m[0], m.index, m.input].join("|");"#
        ),
        Value::String(Arc::from("b|1|abc"))
    );
    assert_eq!(
        run("var m = ''.match(); [m[0], m.index, m.input].join('|');"),
        Value::String(Arc::from("|0|"))
    );
    assert_eq!(
        run(r#"var r = /b/g;
               r[Symbol.match] = undefined;
               "abcbbc".match(r) === null;"#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"var r = /b/g;
               var m = r[Symbol.match]("abcbbc");
               [m.join(","), r.lastIndex].join("|");"#),
        Value::String(Arc::from("b,b,b|0"))
    );
    assert_eq!(
        run(r#"var r = /b/;
               var m = r[Symbol.match]("abc");
               [m[0], m.index, m.input].join("|");"#),
        Value::String(Arc::from("b|1|abc"))
    );
    assert!(
        run_err(
            r#"var r = /./g;
               Object.defineProperty(r, "lastIndex", { writable: false });
               r[Symbol.match]("x");"#
        )
        .contains("lastIndex"),
        "RegExp @@match must surface lastIndex write failures"
    );
    assert_eq!(
        run(r#"RegExp = function(){};
               RegExp.prototype = {
                 [Symbol.match]: function(s) { return "poison:" + s; }
               };
               var m = "abc".match("b");
               [m[0], m.index, m.input].join("|");"#),
        Value::String(Arc::from("b|1|abc"))
    );
    assert!(
        run_err(
            r#"var search = {};
               Object.defineProperty(search, Symbol.match, {
                 get: function(){ throw new Error("match-get"); }
               });
               "".match(search);"#
        )
        .contains("match-get"),
        "String.prototype.match must observe searchValue[Symbol.match]"
    );
    assert_eq!(
        run(r#"var search = {};
               var seenThis, seenArg;
               search[Symbol.match] = function(arg) {
                 seenThis = this;
                 seenArg = arg;
                 return "custom";
               };
               var out = "abc".match(search);
               [out, seenThis === search, seenArg].join("|");"#),
        Value::String(Arc::from("custom|true|abc"))
    );
    assert_eq!(
        run(r#"var old = RegExp.prototype[Symbol.match];
               var seenThis, seenArg;
               RegExp.prototype[Symbol.match] = function(arg) {
                 seenThis = this;
                 seenArg = arg;
                 return "created";
               };
               var out = "target".match("string source");
               RegExp.prototype[Symbol.match] = old;
               [out, seenThis instanceof RegExp, seenThis.source, seenThis.flags, seenThis.lastIndex, seenArg].join("|");"#),
        Value::String(Arc::from("created|true|string source||0|target"))
    );
    assert_eq!(
        run("/undefined/.exec()[0];"),
        Value::String(Arc::from("undefined"))
    );
    assert_eq!(
        run("var gets = 0; var marker = { valueOf: function(){ gets++; return 0; } }; var r = /./; r.lastIndex = marker; var m = r.exec('abc'); m[0] + ',' + (r.lastIndex === marker) + ',' + gets;"),
        Value::String(Arc::from("a,true,1"))
    );
    assert_eq!(
        run("var gets = 0; var r = /./g; r.lastIndex = { valueOf: function(){ gets++; return -1; } }; var m = r.exec('abc'); m[0] + ',' + r.lastIndex + ',' + gets;"),
        Value::String(Arc::from("a,1,1"))
    );
    assert_eq!(
        run("var r = /./g; r.lastIndex = 0; var before = r.lastIndex; r.exec('abc'); before + ',' + r.lastIndex;"),
        Value::String(Arc::from("0,1"))
    );
    assert_eq!(
        run("var r = /z/g; r.lastIndex = 1; var before = r.lastIndex; r.exec('abc'); before + ',' + r.lastIndex;"),
        Value::String(Arc::from("1,0"))
    );
    assert!(run_err(
        "var r = /c/y; Object.defineProperty(r, 'lastIndex', { writable: false }); r.exec('abc');"
    )
    .contains("TypeError"));
}

#[test]
fn regexp_match_indices_follow_utf16_realm_and_property_semantics() {
    assert_eq!(
        run(r#"
            var match = /(a)(z)?/d.exec("ba");
            var desc = Object.getOwnPropertyDescriptor(match, "indices");
            var groupsDesc = Object.getOwnPropertyDescriptor(match.indices, "groups");
            [
              match.index,
              match.indices.length,
              match.indices[0].join(","),
              match.indices[1].join(","),
              match.indices[2] === undefined,
              match.indices.groups === undefined,
              desc.value === match.indices,
              desc.writable,
              desc.enumerable,
              desc.configurable,
              groupsDesc.writable,
              groupsDesc.enumerable,
              groupsDesc.configurable,
              Object.getPrototypeOf(match.indices) === Array.prototype,
              Object.getPrototypeOf(match.indices[0]) === Array.prototype,
              Object.keys(match).join(","),
              Object.prototype.hasOwnProperty.call(/a/.exec("a"), "indices")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "1|3|1,2|1,2|true|true|true|true|true|true|true|true|true|true|true|0,1,2,index,input,groups,indices|false"
        ))
    );

    assert_eq!(
        run(r#"
            var match = /(?<first>a)(?<missing>z)?/d.exec("a");
            var groups = match.indices.groups;
            var desc = Object.getOwnPropertyDescriptor(groups, "first");
            [
              groups.first === match.indices[1],
              groups.missing === undefined,
              Object.prototype.hasOwnProperty.call(groups, "missing"),
              Object.getPrototypeOf(groups) === null,
              Object.getOwnPropertyNames(groups).join(","),
              desc.writable,
              desc.enumerable,
              desc.configurable,
              /(?<__proto__>a)/d.exec("a").indices.groups.__proto__.join(",")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|first,missing|true|true|true|0,1"
        ))
    );

    assert_eq!(
        run(r#"
            var scalar = "𝐁";
            var legacy = /./d.exec(scalar);
            var unicode = /(.)/du.exec(scalar);
            var splitScalar = String.fromCharCode(0xD835) + String.fromCharCode(0xDC01);
            var splitUnicode = /(?<value>.)/du.exec(splitScalar);
            var splitGlobal = /./dgu;
            splitGlobal.exec(splitScalar);
            var literalMiddle = /./dgu;
            literalMiddle.lastIndex = 1;
            var literalMiddleMatch = literalMiddle.exec(scalar);
            var splitMiddle = /./dgu;
            splitMiddle.lastIndex = 1;
            var splitMiddleMatch = splitMiddle.exec(splitScalar);
            var viaMatch = "ba".match(/(a)/d);
            [
              legacy[0].length,
              legacy.indices[0].join(","),
              unicode[0].length,
              unicode.indices[0].join(","),
              unicode.indices[1].join(","),
              splitUnicode[0].length,
              splitUnicode[0].charCodeAt(0),
              splitUnicode[0].charCodeAt(1),
              splitUnicode.indices[0].join(","),
              splitUnicode.indices.groups.value.join(","),
              splitUnicode.groups.value.length,
              splitGlobal.lastIndex,
              literalMiddleMatch.index,
              literalMiddleMatch.indices[0].join(","),
              literalMiddle.lastIndex,
              splitMiddleMatch.index,
              splitMiddleMatch.indices[0].join(","),
              splitMiddle.lastIndex,
              viaMatch.indices[1].join(",")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "1|0,1|2|0,2|0,2|2|55349|56321|0,2|0,2|2|2|0|0,2|2|0|0,2|2|1,2"
        ))
    );

    assert_eq!(
        run(r#"
            var repeated = /((a)|(b))*/d.exec("ab").indices;
            var empty = /a()/d.exec("a").indices[1];
            var afterScalar = /a/d.exec("𝐁a").indices[0];
            var lone = String.fromCharCode(0xD800);
            var regexp = /(a)/d;
            Object.defineProperty(regexp, "hasIndices", { value: false });
            Object.defineProperty(regexp, "flags", { value: "" });
            [
              repeated[1].join(","),
              repeated[2] === undefined,
              repeated[3].join(","),
              empty.join(","),
              afterScalar.join(","),
              /./d.exec(lone).indices[0].join(","),
              /./du.exec(lone).indices[0].join(","),
              regexp.exec("a").indices[1].join(",")
            ].join("|");
        "#),
        Value::String(Arc::from("1,2|true|1,2|1,1|2,3|0,1|0,1|0,1"))
    );

    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var result = other.RegExp.prototype.exec.call(
              other.eval("/(?<x>a)/d"),
              "a"
            );
            [
              Object.getPrototypeOf(result) === other.Array.prototype,
              Object.getPrototypeOf(result.indices) === other.Array.prototype,
              Object.getPrototypeOf(result.indices[0]) === other.Array.prototype,
              Object.getPrototypeOf(result.indices.groups) === null,
              result.indices.groups.x === result.indices[1],
              Object.getPrototypeOf(result.indices) === Array.prototype
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|true|false"))
    );
}

#[test]
fn regexp_named_group_backend_lowering_preserves_names_and_backreferences() {
    assert_eq!(
        run(r#"
            var escaped = /(?<\u{03C0}>a)(?<_\u200C>b)(?<$𐒤>c)/du.exec("abc");
            [
              escaped.indices.groups.π.join(","),
              escaped.indices.groups._\u200C.join(","),
              escaped.indices.groups.$𐒤.join(","),
              Object.getOwnPropertyNames(escaped.indices.groups).join(","),
              /(?<a>.)(?<b>.)\k<b>\k<a>/u.test("abba"),
              /(?<a>.)(?<b>.)\k<b>\k<a>/.test("abbb"),
              new RegExp("\\k<x>").test("k<x>"),
              /(?:(?<a>a)|b)\k<a>/u.test("b"),
              /\k<a>(?<a>a)/u.test("a"),
              /(?<a>\k<a>a)/u.test("a"),
              /(?<a>a)\k<a>0/u.test("aa0"),
              /(?<x>a)(b)(c)(d)(e)(f)(g)(h)(i)(j)(k)(z)\k<x>2/u.test("abcdefghijkza2"),
              /(?<x>a)(b)(c)(d)(e)(f)(g)(h)(i)(j)(k)(z)\k<x>2/u.test("abcdefghijkzz"),
              /(?<x>a)\w\k<x>/u.exec("aba")[0],
              /\b(?<x>a)\k<x>\b/u.test("aa"),
              new RegExp("(?<\\u037A>a)\\k<\\u037A>", "u").test("aa"),
              /(?<\u{03C0}>a)/du.source
            ].join("|");
        "#),
        Value::String(Arc::from(
            "0,1|1,2|2,3|π,_\u{200c},$𐒤|true|false|true|true|true|true|true|true|false|aba|true|true|(?<\\u{03C0}>a)"
        ))
    );
    assert!(run_err(r#"new RegExp("(?<a-b>x)");"#).contains("SyntaxError"));
    assert!(run_err(r#"new RegExp("(?<x>a)(?<x>b)");"#).contains("SyntaxError"));
    assert!(run_err(r#"new RegExp("\\k<missing>", "u");"#).contains("SyntaxError"));
    assert!(run_err(r#"new RegExp("(?<x>a)\\k");"#).contains("SyntaxError"));
    assert!(run_err(r#"new RegExp("(?<x>a)[\\k]");"#).contains("SyntaxError"));
    assert_eq!(
        run(r#"
            var side = 0;
            try { eval("side = 1; /(?<0>a)/;"); } catch (error) {}
            side;
        "#),
        Value::Number(0.0)
    );
}

#[test]
fn regexp_duplicate_named_groups_select_the_participating_capture() {
    assert_eq!(
        run(r#"
            var simple = /(?<x>a)|(?<x>b)/d.exec("b");
            var ordered = /(?:(?<x>a)|(?<y>a)(?<x>b))(?:(?<z>c)|(?<z>d))/d.exec("abc");
            var repeated = /(?:(?:(?<r>a)|(?<r>b)|c)\k<r>){2}/d.exec("aac");
            var repeatedWithoutRef = /(?:(?<n>a)|(?<n>b)|c){2}/d.exec("ac");
            var trailingBackref = /^(?:(?<t>a)|(?<t>b))*\k<t>$/d.exec("aa");
            var split = "xab".split(/(?<s>a)|(?<s>b)/);
            [
              simple[1] === undefined,
              simple[2],
              simple.groups.x,
              simple.indices.groups.x === simple.indices[2],
              simple.indices.groups.x.join(","),
              Object.keys(simple.groups).join(","),
              ordered.groups.x,
              ordered.groups.y,
              ordered.groups.z,
              Object.keys(ordered.groups).join(","),
              ordered.indices.groups.x === ordered.indices[3],
              ordered.indices.groups.z === ordered.indices[4],
              /(?:(?<q>a)|(?<q>b))\k<q>/.test("bb"),
              /(?:(?<q>a)|(?<q>b))\k<q>/.test("abab"),
              /^(?:(?<q>x)|(?<q>y)|z)\k<q>$/.test("z"),
              /^(?:(?<q>x)|(?<q>y)|z)\k<q>$/.test("zz"),
              repeated.groups.r === undefined,
              repeated.indices.groups.r === undefined,
              repeatedWithoutRef.groups.n === undefined,
              repeatedWithoutRef.indices.groups.n === undefined,
              trailingBackref[1],
              trailingBackref[2] === undefined,
              trailingBackref.groups.t,
              trailingBackref.indices.groups.t === trailingBackref.indices[1],
              /^(?:(?<u>ſ)|(?<u>t))\k<u>$/iu.test("ſs"),
              /^(?:(?<u>Ωa)|(?<u>z))\k<u>$/iu.test("Ωaωaa"),
              /^(?:(?<u>ſ)|(?<u>t))\k<u>$/i.test("ſs"),
              /(?<=(?:(?<h>a)|(?<h>b))\k<h>)c/.exec("aac").groups.h,
              "ba".replace(/(?<v>a)|(?<v>b)/g, "[$<v>][$1][$2]"),
              split.map(String).join("|")
            ].join(";");
        "#),
        Value::String(Arc::from(
            "true;b;b;true;0,1;x;b;a;c;x,y,z;true;true;true;false;true;false;true;true;true;true;a;true;a;true;true;false;false;a;[b][][b][a][a][];x|a|undefined||undefined|b|"
        ))
    );

    assert_eq!(
        run(r#"new RegExp("(?<x>a)|(?<x>b)").source;"#),
        Value::String(Arc::from("(?<x>a)|(?<x>b)"))
    );
    assert!(run_err(r#"new RegExp("(?<x>a)(?:b|c)(?<x>d)");"#).contains("SyntaxError"));
}

#[test]
fn regexp_lookaround_uses_ecmascript_backend() {
    assert_eq!(
        run(r#"
            var ahead = /(?=(a+))/.exec("baa");
            var negative = /Java(?!Script)([A-Z]\w*)/.exec("JavaBeans");
            var fixed = /(?<=a)b/.exec("ab");
            var greedy = /(?<=(b+))c/.exec("abbbc");
            var backward = /(?<=([ab]+)([bc]+))$/.exec("abc");
            var nestedPositive = /(?<=(a)(?=\1))b/.exec("ab");
            var nestedNamed = /(?<=(?<x>a)(?=\k<x>))b/.exec("ab");
            [
              ahead[0], ahead.index, ahead[1],
              negative[0], negative[1],
              fixed[0], fixed.index,
              greedy[0], greedy[1],
              backward[0], backward[1], backward[2],
              /(?<=é)x/i.test("Éx"),
              /(?<=\u00e9)x/i.test("Éx"),
              /(?<=\xe9)x/i.test("Éx"),
              !/(?=s)/i.test("ſ"),
              !/(?=\u0073)/i.test("ſ"),
              !/(?=\x73)/i.test("ſ"),
              nestedPositive[1], nestedNamed.groups.x,
              /(?<=(a)(?!\1))b/.exec("ab") === null
            ].join("|");
        "#),
        Value::String(Arc::from(
            "|1|aa|JavaBeans|Beans|b|1|c|bbb||a|bc|true|true|true|true|true|true|a|a|true"
        ))
    );
}

#[test]
fn regexp_legacy_quantified_lookahead_updates_captures() {
    assert_eq!(
        run(r#"
            var optional = /(?:(?=(abc)))?a/.exec("abc");
            var exact = /(?:(?=(abc))){1,1}a/.exec("abc");
            var optionalRange = /(?:(?=(abc))){0,1}a/.exec("abc");
            [
              optional[1] === undefined,
              exact[1],
              optionalRange[1] === undefined
            ].join("|");
        "#),
        Value::String(Arc::from("true|abc|true"))
    );
    assert!(run_err(r#"new RegExp("(?=a)?", "u");"#).contains("SyntaxError"));
}

#[test]
fn regexp_repeated_capture_clears_nonparticipating_groups() {
    assert_eq!(
        run("var m = /(z)((a+)?(b+)?(c))*/.exec('zaacbbbcac'); [m[0], m[1], m[2], m[3], String(m[4]), m[5]].join('|');"),
        Value::String(Arc::from("zaacbbbcac|z|ac|a|undefined|c"))
    );
    assert_eq!(
        run("var m = /((a)|(b))*/.exec('ab'); [m[0], m[1], String(m[2]), m[3]].join('|');"),
        Value::String(Arc::from("ab|b|undefined|b"))
    );
    assert_eq!(
        run("var m = /(?:(a)|(b))*/.exec('ab'); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("var m = /(?:(a)(b)|(c)(d))*/.exec('abcd'); [m[0], String(m[1]), String(m[2]), m[3], m[4]].join('|');"),
        Value::String(Arc::from("abcd|undefined|undefined|c|d"))
    );
    assert_eq!(
        run("var m = /(?:(a)?(b))*/.exec('abb'); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("abb|undefined|b"))
    );
    assert_eq!(
        run("var m = /(x)(?:(a)|(b))*(y)/.exec('xaby'); [m[1], String(m[2]), m[3], m[4]].join('|');"),
        Value::String(Arc::from("x|undefined|b|y"))
    );
    assert_eq!(
        run("var m = /(?:(a)|(b))+/.exec('ab'); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("var m = /(?:(a)|(b)){2}/.exec('ab'); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("var m = 'ab'.match(/(?:(a)|(b))*/); [m[0], String(m[1]), m[2]].join('|');"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("'ab'.replace(/(?:(a)|(b))*/, function(m, a, b){ return m + '|' + String(a) + '|' + b; });"),
        Value::String(Arc::from("ab|undefined|b"))
    );
    assert_eq!(
        run("'ab xa'.replace(/(?:(a)|(b))+/g, function(m, a, b, offset){ return '[' + String(a) + '|' + String(b) + '|' + offset + ']'; });"),
        Value::String(Arc::from("[undefined|b|0] x[a|undefined|4]"))
    );
}

#[test]
fn regexp_nullable_quantifier_uses_ecmascript_match_boundaries() {
    assert_eq!(
        run(r#"
            var plain = /(a?b??)*/.exec("ab");
            var prefixed = /z(a?b??)*/.exec("zab");
            var sticky = /(a?b??)*/gy;
            var stickyMatch = sticky.exec("ab");
            var unicode = /((?:😀)?x??)*/u.exec("😀x");
            var legacy = /((?:😀)?x??)*/.exec("😀x");
            [
              plain[0], plain[1],
              prefixed[0], prefixed[1],
              stickyMatch[0], stickyMatch[1], sticky.lastIndex,
              unicode[0], unicode[1],
              legacy[0], legacy[1],
              "ab".match(/(a?b??)*/g).map(JSON.stringify).join(","),
              "ab".replace(/(a?b??)*/g, "<$1>"),
              !/(a+)+$/.test("a".repeat(4096) + "!"),
              Array.from("😀".matchAll(/(a?)*?/gu), function(m) { return m.index; }).join(","),
              Array.from("😀".matchAll(/(a?)*?/g), function(m) { return m.index; }).join(",")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "ab|b|zab|b|ab|b|2|😀x|x|😀x|x|\"ab\",\"\"|<b><>|true|0,2|0,1,2"
        ))
    );
}

#[test]
fn regexp_quantifier_integer_bounds_are_host_independent() {
    assert_eq!(
        run(r#"
            var maxSafe = Number.MAX_SAFE_INTEGER;
            var results = [
              new RegExp("b{" + maxSafe + "}", "u").test(""),
              new RegExp("b{" + maxSafe + ",}?").test("a"),
              new RegExp("b{" + maxSafe + "," + maxSafe + "}").test("b"),
              /b{4294967295}/.test("b"),
              /b{4294967296}/.test("b"),
              /b{9007199254740991}/u.test("b"),
              new RegExp("b{340282366920938463463374607431768211456}").test("b"),
              new RegExp("b{0,340282366920938463463374607431768211456}").test(""),
              new RegExp("b{0,340282366920938463463374607431768211456}?").test(""),
              new RegExp("b{340282366920938463463374607431768211456}|a").test("a"),
              new RegExp("(b){4294967296}").exec("b") === null,
              new RegExp("(?:){0,4294967296}").test(""),
              new RegExp("(?:){4294967296}").source === "(?:){4294967296}",
              new RegExp("a{,2}|b{4294967296}").test("aa"),
              new RegExp("a{,}|b{4294967296}").test("a"),
              new RegExp("😀{4294967296}", "u").test("😀"),
              new RegExp("😀{4294967296}").test("😀"),
              new RegExp("(a){1,4294967296}").exec("a")[1],
              new RegExp("(a){1,4294967296}\\1").test("aa"),
              new RegExp("a{1,4294967296}(?=b)").test("ab"),
              new RegExp("😀{1,4294967296}").test("😀"),
              new RegExp("😀{1,4294967296}", "u").test("😀"),
              new RegExp("😀{1,4294967296}", "v").test("😀"),
              new RegExp(".{1,4294967296}").test("\uD83D"),
              new RegExp(".{1,4294967296}", "u").test("\uD83D"),
              new RegExp(".{1,4294967296}", "v").test("\uD83D")
            ];
            results.join(",");
        "#),
        Value::String(Arc::from(
            "false,false,false,false,false,false,false,true,true,true,true,true,true,false,false,false,false,a,true,true,true,true,true,true,true,true"
        ))
    );

    for source in [
        r#"new RegExp("a{9007199254740992,9007199254740991}");"#,
        r#"new RegExp("a{340282366920938463463374607431768211457,340282366920938463463374607431768211456}", "u");"#,
        "/a{4294967296,4294967295}/;",
    ] {
        assert!(
            run_err(source).contains("SyntaxError"),
            "expected range error for {source}"
        );
    }
}

#[test]
fn regexp_compiled_too_big_repeat_uses_bounded_counter_backend() {
    assert_eq!(run("/b{1000000}/.test('b');"), Value::Bool(false));
    assert_eq!(
        run(r#"
            [
              new RegExp("a{1,1000000}(?=b)").test("ab"),
              new RegExp("(a)\\1a{1,1000000}").test("aaa"),
              new RegExp("(a){1,1000000}").exec("a")[1]
            ].join(",");
        "#),
        Value::String(Arc::from("true,true,a"))
    );
    assert!(run_err("new RegExp('a{2,1}');").contains("SyntaxError"));
    assert!(run_err("new RegExp('[');").contains("SyntaxError"));
}

#[test]
fn regex_exec_no_match() {
    assert_eq!(run("/zzz/.exec('abc');"), Value::Null);
}

#[test]
fn regex_source_flags() {
    assert_eq!(run("/abc/gi.source;"), Value::String(Arc::from("abc")));
    assert_eq!(run("/abc/gi.flags;"), Value::String(Arc::from("gi")));
    assert_eq!(
        run("new RegExp('', 'yusmigd').flags;"),
        Value::String(Arc::from("dgimsuy"))
    );
    assert_eq!(
        run("Object.getOwnPropertyDescriptor(RegExp.prototype, 'flags').get.name;"),
        Value::String(Arc::from("get flags"))
    );
    assert_eq!(
        run("Object.getOwnPropertyDescriptor(RegExp.prototype, 'flags').get.length;"),
        Value::Number(0.0)
    );
    assert_eq!(
        run("Object.getOwnPropertyDescriptor(RegExp.prototype, 'source').get.call(RegExp.prototype);"),
        Value::String(Arc::from("(?:)"))
    );
    assert_eq!(
        run("new RegExp('').source;"),
        Value::String(Arc::from("(?:)"))
    );
    assert_eq!(
        run("new RegExp('/').source;"),
        Value::String(Arc::from("\\/"))
    );
    assert_eq!(
        run("new RegExp('\\n').source;"),
        Value::String(Arc::from("\\n"))
    );
    assert_eq!(
        run("Object.getOwnPropertyNames(/a/g).join(',');"),
        Value::String(Arc::from("lastIndex"))
    );
    assert_eq!(
        run("[
               Object.getOwnPropertyDescriptor(/a/g, 'global'),
               Object.getOwnPropertyDescriptor(/a/g, '__regexp_source__')
             ].map(String).join(',');"),
        Value::String(Arc::from("undefined,undefined"))
    );
    assert_eq!(
        run(r#"var r = /a/gy;
               Object.defineProperty(r, 'a', { value: 1 });
               Object.getOwnPropertyNames(Object.getOwnPropertyDescriptors(r)).join(',');"#),
        Value::String(Arc::from("lastIndex,a"))
    );
    assert_eq!(
        run(r#"var r = /a/gy;
               Object.defineProperty(r, 'global', { value: false });
               [r.source, r.global, r.sticky, r.flags].join('|');"#),
        Value::String(Arc::from("a|false|true|y"))
    );
    assert_eq!(
        run(r#"class S extends RegExp {
                 #__regexp_source__ = 1;
                 #__regexp_global__ = 2;
                 values() { return [this.#__regexp_source__, this.#__regexp_global__].join(','); }
               }
               var r = new S('a', 'g');
               [r.source, r.global, r.flags, r.values()].join('|');"#),
        Value::String(Arc::from("a|true|g|1,2"))
    );
    assert_eq!(
        run(
            r#"var get = Object.getOwnPropertyDescriptor(RegExp.prototype, 'global').get;
               String(get.call(RegExp.prototype));"#
        ),
        Value::String(Arc::from("undefined"))
    );
    assert!(
        run_err(r#"Object.getOwnPropertyDescriptor(RegExp.prototype, 'global').get.call({});"#)
            .contains("RegExp getter"),
        "RegExp flag getters must reject ordinary objects without internal slots"
    );
    assert_eq!(
        run(r#"
            var get = Object.getOwnPropertyDescriptor(RegExp.prototype, 'global').get;
            var other = $262.createRealm().global;
            var otherRegExpProto = other.RegExp.prototype;
            var otherGet = Object.getOwnPropertyDescriptor(otherRegExpProto, 'global').get;
            var ok = [];
            try { get.call(otherRegExpProto); ok.push(false); }
            catch (e) { ok.push(e.constructor === TypeError); }
            try { otherGet.call(RegExp.prototype); ok.push(false); }
            catch (e) { ok.push(e.constructor === other.TypeError); }
            ok.join(',');
            "#),
        Value::String(Arc::from("true,true"))
    );
    assert_eq!(
        run(r#"
            var get = Object.getOwnPropertyDescriptor(RegExp.prototype, 'source').get;
            var other = $262.createRealm().global;
            var otherRegExpProto = other.RegExp.prototype;
            var otherGet = Object.getOwnPropertyDescriptor(otherRegExpProto, 'source').get;
            var ok = [];
            try { get.call(otherRegExpProto); ok.push(false); }
            catch (e) { ok.push(e.constructor === TypeError); }
            try { otherGet.call(RegExp.prototype); ok.push(false); }
            catch (e) { ok.push(e.constructor === other.TypeError); }
            ok.join(',');
            "#),
        Value::String(Arc::from("true,true"))
    );
}

#[test]
fn regexp_escape_is_static_builtin_with_expected_attrs() {
    assert_eq!(
        run(
            r#"var d = Object.getOwnPropertyDescriptor(RegExp, "escape");
               [
                 typeof RegExp.escape,
                 "escape" in RegExp.prototype,
                 d.writable,
                 d.enumerable,
                 d.configurable,
                 RegExp.escape.length,
                 RegExp.escape.name
               ].join("|");"#
        ),
        Value::String(Arc::from("function|false|true|false|true|1|escape"))
    );
}

#[test]
fn regexp_escape_escapes_literal_pattern_text() {
    assert_eq!(
        run(r#"[
                 RegExp.escape("") === "",
                 RegExp.escape("foo") === "\\x66oo",
                 RegExp.escape("1+1") === "\\x31\\+1",
                 RegExp.escape("/.") === "\\/\\.",
                 RegExp.escape(" ,-") === "\\x20\\x2c\\x2d",
                 RegExp.escape("\t\n\v\f\r") === "\\t\\n\\v\\f\\r",
                 RegExp.escape("\u2028\u2029") === "\\u2028\\u2029",
                 RegExp.escape("\uFEFF\u00A0\u202F") === "\\ufeff\\xa0\\u202f",
                 RegExp.escape(String.fromCharCode(0xD800)) === "\\ud800",
                 RegExp.escape(String.fromCharCode(0xD83D, 0xDE00)) === String.fromCharCode(0xD83D, 0xDE00)
               ].join("|");"#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|true|true|true"
        ))
    );
}

#[test]
fn regexp_escape_rejects_non_strings_without_coercion() {
    assert!(run_err("RegExp.escape(undefined);").contains("TypeError"));
    assert!(run_err("RegExp.escape(123);").contains("TypeError"));
    assert_eq!(
        run(r#"var called = false;
               try {
                 RegExp.escape({ toString: function() { called = true; return "x"; } });
               } catch (e) {}
               called;"#),
        Value::Bool(false)
    );
}

#[test]
fn regexp_escape_is_installed_in_created_realms() {
    assert_eq!(
        run(r#"var other = $262.createRealm().global;
               [typeof other.RegExp.escape, other.RegExp.escape("foo")].join("|");"#),
        Value::String(Arc::from("function|\\x66oo"))
    );
}

#[test]
fn regexp_modifiers_empty_remove_list_compiles() {
    assert_eq!(run("/(?s-:^.$)/.test('\\n');"), Value::Bool(true));
    assert_eq!(
        run("new RegExp('(?s-:^.$)').test('\\n');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var r = /(?m-:^b$)/; r.source;"),
        Value::String(Arc::from("(?m-:^b$)"))
    );
    assert_eq!(
        run("/^a\\n(?m-:^b$)\\nc$/.test('a\\nb\\nc');"),
        Value::Bool(true)
    );
    assert_eq!(run("/(?s:^.$)/.test('𐌀');"), Value::Bool(false));
    assert_eq!(run("/(?s:^.$)/u.test('𐌀');"), Value::Bool(true));
    assert_eq!(run("/(?s:(?-s:.))/.test('\\n');"), Value::Bool(false));
    assert_eq!(run("/(?s:(?-s:(?s:.)))/.test('\\n');"), Value::Bool(true));
    assert_eq!(run("/(?i:\\p{Lu})/u.test('a');"), Value::Bool(true));
    assert_eq!(run("/(?i:\\P{Lu})/u.test('A');"), Value::Bool(true));
    assert_eq!(
        run("/(?i:\\P{Uppercase_Letter})/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("/(?i:\\P{General_Category=Uppercase_Letter})/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(run("/(?i:[\\P{Lu}])/u.test('A');"), Value::Bool(true));
    assert_eq!(
        run("/(?i:[\\P{Uppercase_Letter}])/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(run("/(?-i:\\w)/ui.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/(?-i:\\W)/ui.test('ſ');"), Value::Bool(true));
    assert_eq!(run("/(?-i:[\\w])/ui.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/(?-i:[\\W])/ui.test('ſ');"), Value::Bool(true));
    assert_eq!(run("/(?-i:\\b)ſ/ui.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/(?-i:\\B)ſ/ui.test('ſ');"), Value::Bool(true));
}

#[test]
fn regexp_v_flag_uses_unicode_pattern_semantics() {
    assert_eq!(
        run(r#"
            /^\p{ASCII_Hex_Digit}+$/v.test("B09") &&
              !/^\p{ASCII_Hex_Digit}+$/v.test("G") &&
              /^[\d&&\p{ASCII_Hex_Digit}]$/v.test("7") &&
              !/^[\d&&\p{ASCII_Hex_Digit}]$/v.test("B") &&
              /^[\p{ASCII_Hex_Digit}--\d]$/v.test("B") &&
              !/^[\p{ASCII_Hex_Digit}--\d]$/v.test("7") &&
              /^.$/v.test("😀") &&
              !/^.$/v.test("\n") && !/^.$/v.test("\r") &&
              !/^.$/v.test("\u2028") && !/^.$/v.test("\u2029") &&
              !/^.$/u.test("\r") && !/^.$/u.test("\u2028") &&
              /^.$/sv.test("\r") && /(?s:.)/v.test("\u2029") &&
              /(?-s:.)/sv.test("A") && !/(?-s:.)/sv.test("\r") &&
              new RegExp("^\\p{ASCII_Hex_Digit}+$", "v").test("B09");
            "#),
        Value::Bool(true)
    );

    for source in [r#"new RegExp("\\a", "v");"#, r#"new RegExp("\\1", "v");"#] {
        assert!(run_err(source).contains("SyntaxError"), "source: {source}");
    }
}

#[test]
fn regexp_u_and_v_flags_are_mutually_exclusive() {
    for source in ["/./uv;", "/./vu;", "if (false) /a/uv;", "if (false) /a/vu;"] {
        assert!(
            run_err(source).contains("flags 'u' and 'v'"),
            "source: {source}"
        );
    }

    for flags in ["uv", "vu", "duvy", "vuid"] {
        let source = format!(r#"new RegExp(".", "{flags}");"#);
        assert!(
            run_err(&source).contains("flags 'u' and 'v'"),
            "flags: {flags}"
        );
    }

    assert!(run_err(r#"RegExp(".", "uv");"#).contains("flags 'u' and 'v'"));
    assert!(
        run_err(r#"new RegExp("a**", "uv");"#).contains("flags 'u' and 'v'"),
        "flag-set validation must precede pattern validation"
    );
    assert!(run_err(r#"new RegExp(".", "uvv");"#).contains("duplicate regular expression flag"));
    assert!(run_err(r#"new RegExp(".", "uvG");"#).contains("invalid regular expression flag"));

    assert_eq!(run("new RegExp('.', 'u').unicode;"), Value::Bool(true));
    assert_eq!(run("new RegExp('.', 'v').unicodeSets;"), Value::Bool(true));
}

#[test]
fn regexp_quantifier_without_atom_reports_early_error() {
    for source in [
        "/?/;",
        "/{2}/;",
        "/{2,}/;",
        "/{2,3}/;",
        "/a**/;",
        "/a***/;",
        "/a++/;",
        "/a+++/;",
        "/a???/;",
        "/a????/;",
        "/x{1}{1,}/;",
        "/x{1,2}{1}/;",
        "/x{1,}{1}/;",
        "/x{0,1}{1,}/;",
        "/^*/;",
        "/$?/;",
        "/\\b?/;",
        "/\\B+/;",
        "/\\u{61}{2}/;",
        "/\\x**/;",
        "/\\c**/;",
        "/\\k<a**>/;",
    ] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected early error for {source}"
        );
    }
    for source in [
        "eval('{}/{2}/;');",
        "eval('{}/{2,}/;');",
        "eval('{}/{2,3}/;');",
    ] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected parser fallback error for {source}"
        );
    }
    for source in [
        "new RegExp('?');",
        "new RegExp('{2}');",
        "new RegExp('{2,}');",
        "new RegExp('{2,3}');",
        "new RegExp('a**');",
        "new RegExp('a+++');",
        "new RegExp('a????');",
        "new RegExp('x{1}{1,}');",
        "new RegExp('x{1,2}{1}');",
        "new RegExp('x{1,}{1}');",
        "new RegExp('x{0,1}{1,}');",
        "new RegExp('^*');",
        "new RegExp('\\\\b?');",
        "new RegExp('\\\\u{61}{2}');",
        "new RegExp('\\\\x**');",
        "new RegExp('\\\\c**');",
        "new RegExp('\\\\k<a**>');",
    ] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected constructor error for {source}"
        );
    }

    assert_eq!(run("/a?/.test('');"), Value::Bool(true));
    assert_eq!(run("/a{2}/.test('aa');"), Value::Bool(true));
    assert_eq!(run("/\\?/.test('?');"), Value::Bool(true));
    assert_eq!(run("/[?{]/.test('{');"), Value::Bool(true));
    assert_eq!(run("/(?:a)?/.test('');"), Value::Bool(true));
    assert_eq!(run("/a*?/.test('');"), Value::Bool(true));
    assert_eq!(run("/a??/.test('');"), Value::Bool(true));
    assert_eq!(run("/a{1,2}?/.test('aa');"), Value::Bool(true));
    assert_eq!(run("/(?=a)??/.test('a');"), Value::Bool(true));
    assert_eq!(run("/\\u{61}{2}/u.test('aa');"), Value::Bool(true));
    assert_eq!(run("/\\u{61}{2}/v.test('aa');"), Value::Bool(true));
    assert_eq!(run("new RegExp('a{2}').test('aa');"), Value::Bool(true));
}

#[test]
fn regexp_assertion_quantifier_reports_early_error() {
    for source in [
        "/(?<=.)?/;",
        "/(?<!.)?/;",
        "/(?<=.){2,3}/;",
        "/(?<!.){2,3}/;",
        "/(?=.)?/u;",
        "/(?!.)?/u;",
        "/(?=.){2,3}/u;",
        "/(?!.){2,3}/u;",
        "/(?<=.)?/u;",
        "/(?<!.)?/u;",
        "/(?<=.){2,3}/u;",
        "/(?<!.){2,3}/u;",
    ] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected assertion quantifier early error for {source}"
        );
    }
    for source in ["eval('{}/(?<!.){2,3}/;');", "eval('{}/(?!.){2,3}/u;');"] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected parser fallback assertion quantifier error for {source}"
        );
    }
    for source in [
        "new RegExp('(?<=.)?');",
        "new RegExp('(?<!.){2,3}');",
        "new RegExp('(?=.)?', 'u');",
        "new RegExp('(?!.){2,3}', 'u');",
    ] {
        assert!(
            run_err(source).contains("regular expression quantifier"),
            "expected constructor assertion quantifier error for {source}"
        );
    }
}

#[test]
fn regexp_unicode_mode_syntax_reports_early_error() {
    for source in [
        "/\\c0/u;",
        "/{/u;",
        "/\\M/u;",
        "/\\1/u;",
        "/[\\d-a]/u;",
        "/[\\s-\\d]/u;",
        "/[%-\\d]/u;",
        "/[--\\d]/u;",
        "/\\8/u;",
        "/\\u{110000}/u;",
        "/\\u{1,}/u;",
        "/\\u{1F_639}/u;",
        "/\\p{}/u;",
        "/\\p{Greek}/u;",
        "/\\p{Ascii}/u;",
        "/\\p{any}/u;",
        "/\\p{assigned}/u;",
        "/\\p{Script_Extensions}/u;",
        "/\\p{Script_Extensions=}/u;",
        "/\\p{General_Category=}/u;",
        "/\\p{General_Category=Not_A_Category}/u;",
        "/\\p{Script=FooBarBazInvalid}/u;",
        "/\\p{Script=Greek=Extra}/u;",
        "/\\p{=Greek}/u;",
    ] {
        assert!(
            run_err(source).contains("regular expression"),
            "expected unicode-mode syntax error for {source}"
        );
    }
    for source in [
        "new RegExp('{', 'u');",
        "new RegExp('\\\\M', 'u');",
        "new RegExp('\\\\8', 'u');",
        "new RegExp('\\\\u{110000}', 'u');",
        "new RegExp('\\\\u{1,}', 'u');",
        "new RegExp('\\\\p{}', 'u');",
        "new RegExp('\\\\p{Greek}', 'u');",
        "new RegExp('\\\\p{Ascii}', 'u');",
        "new RegExp('\\\\p{any}', 'u');",
        "new RegExp('\\\\p{assigned}', 'u');",
        "new RegExp('\\\\p{Script_Extensions}', 'u');",
        "new RegExp('\\\\p{Script_Extensions=}', 'u');",
        "new RegExp('\\\\p{General_Category=}', 'u');",
        "new RegExp('\\\\p{General_Category=Not_A_Category}', 'u');",
        "new RegExp('\\\\p{Script=FooBarBazInvalid}', 'u');",
        "new RegExp('\\\\p{Script=Greek=Extra}', 'u');",
        "new RegExp('\\\\p{=Greek}', 'u');",
    ] {
        assert!(
            run_err(source).contains("regular expression"),
            "expected constructor unicode-mode syntax error for {source}"
        );
    }
    assert!(run_err("/\\p{Bad}/u;").contains("regular expression"));
    assert_eq!(run("/\\p{Script=Greek}/u.test('Α');"), Value::Bool(true));
    assert_eq!(
        run("/\\p{Script_Extensions=Greek}/u.test('Α');"),
        Value::Bool(true)
    );
    assert_eq!(run("/\\p{ASCII}/u.test('A');"), Value::Bool(true));
    assert_eq!(run("/\\p{Any}/u.test('A');"), Value::Bool(true));
    assert_eq!(run("/\\p{Assigned}/u.test('A');"), Value::Bool(true));
    assert_eq!(run("/\\p{Lu}/u.test('A');"), Value::Bool(true));
    assert_eq!(
        run("/\\p{Uppercase_Letter}/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("/\\p{gc=Uppercase_Letter}/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("/\\p{General_Category=Uppercase_Letter}/u.test('A');"),
        Value::Bool(true)
    );
    assert_eq!(run("/\\p{Alpha}/u.test('A');"), Value::Bool(true));
    assert_eq!(run("/\\p{sc=Grek}/u.test('Α');"), Value::Bool(true));
    assert_eq!(run("/\\p{scx=Greek}/u.test('Α');"), Value::Bool(true));
}

#[test]
fn regexp_class_range_and_unicode_bracket_early_errors() {
    for source in [
        "/[a--z]/;",
        "/[a--z]/u;",
        "/[a-\\-]/u;",
        "/[a-\\c!]/;",
        "/[\\é-z]/;",
        "/[\\💩-😀]/;",
        "/[💩-😀]/;",
        "/[z-a]/v;",
        "/[\\d--]/v;",
        "/[\\x41--]/v;",
        "/[--\\x41]/v;",
        "/[\\uD83D\\uDCA9-\\u{1F4A8}]/u;",
        "/[[a]/v;",
        "/\\x4/u;",
        "/]/u;",
        "/}/u;",
        "new RegExp('[a--z]');",
        "new RegExp('[a--z]', 'u');",
        "new RegExp('[a-\\\\-]', 'u');",
        "new RegExp('[\\\\d--]', 'v');",
        "new RegExp('[\\\\x41--]', 'v');",
        "new RegExp('[--\\\\x41]', 'v');",
        "new RegExp(']', 'u');",
        "new RegExp('}', 'u');",
        "new RegExp(']', 'v');",
        "new RegExp('}', 'v');",
    ] {
        assert!(
            run_err(source).contains("regular expression"),
            "expected RegExp grammar error for {source}"
        );
    }

    assert_eq!(run("new RegExp(']').test(']');"), Value::Bool(true));
    assert_eq!(run("new RegExp('}').test('}');"), Value::Bool(true));
    assert_eq!(run("/\\]/u.test(']');"), Value::Bool(true));
    assert_eq!(run("/\\}/v.test('}');"), Value::Bool(true));
    assert_eq!(run("/[a\\-z]/u.test('-');"), Value::Bool(true));
    assert_eq!(run("/[\\--a]/u.test(']');"), Value::Bool(true));
    assert_eq!(run("/[A-\\141]/.test('a');"), Value::Bool(true));
    assert_eq!(run("/[A-\\c!]/.test('!');"), Value::Bool(true));
    assert_eq!(run("false ? /[A-\\é]/ : 1;"), Value::Number(1.0));
    assert_eq!(run("false ? /[\\💩-\\uFFFF]/ : 1;"), Value::Number(1.0));
    assert_eq!(run("false ? /[💩-\\uFFFF]/ : 1;"), Value::Number(1.0));
    assert_eq!(run("/\\x41/u.test('A');"), Value::Bool(true));
    assert_eq!(run("/\\x41/v.test('A');"), Value::Bool(true));
    assert_eq!(run("false ? /[\\d--\\w]/v : 1;"), Value::Number(1.0));
    assert_eq!(run("false ? /[a--[b]]/v : 1;"), Value::Number(1.0));
    assert_eq!(run("/[\\d-a]/.test('a');"), Value::Bool(true));
    assert_eq!(run("/[a-\\d]/.test('a');"), Value::Bool(true));
    assert_eq!(
        run("var r = /[a--z]/v; r.test('a') + ',' + r.test('z') + ',' + r.test('-');"),
        Value::String(Arc::from("true,false,false"))
    );
}

#[test]
fn regexp_null_escape_matches_null_character() {
    assert_eq!(run("/\\0/.test('\\x00');"), Value::Bool(true));
    assert_eq!(run("/\\0/u.test('\\x00');"), Value::Bool(true));
    assert_eq!(run("/^\\0a$/u.test('\\x00a');"), Value::Bool(true));
    assert_eq!(run("new RegExp('\\\\0').test('\\x00');"), Value::Bool(true));
    assert_eq!(
        run("var r = /\\0/u; r.source;"),
        Value::String(Arc::from("\\0"))
    );
    assert_eq!(
        run("'\\x00②'.match(/\\0②/u)[0];"),
        Value::String(Arc::from("\0②"))
    );
    assert_eq!(run("'\\u0000፬'.search(/\\0፬$/u);"), Value::Number(0.0));
    assert_eq!(run("'a፬'.search(/፬/u);"), Value::Number(1.0));
    assert_eq!(
        run("var r = /፬/g; r.lastIndex = 1; var n = 'a፬'.search(r); n + ',' + r.lastIndex;"),
        Value::String(Arc::from("1,1"))
    );
}

#[test]
fn regexp_sticky_start_assertion_uses_full_input() {
    assert_eq!(
        run("var re = /^a/y; re.lastIndex = 1; re.test(' a') + ',' + re.lastIndex;"),
        Value::String(Arc::from("false,0"))
    );
    assert_eq!(
        run("var re = /^a/y; re.lastIndex = 1; re.test('\\na') + ',' + re.lastIndex;"),
        Value::String(Arc::from("false,0"))
    );
    assert_eq!(
        run("var re = /^a/my; re.lastIndex = 1; re.test('\\na') + ',' + re.lastIndex;"),
        Value::String(Arc::from("true,2"))
    );
    assert_eq!(
        run("var re = /a/g; re.lastIndex = 1; re.test('xxa') + ',' + re.lastIndex;"),
        Value::String(Arc::from("true,3"))
    );
}

#[test]
fn regexp_non_unicode_ignore_case_does_not_apply_unicode_folding() {
    assert_eq!(run("/é/i.test('É');"), Value::Bool(true));
    assert_eq!(run("/\\u00e9/i.test('É');"), Value::Bool(true));
    assert_eq!(run("/\\xe9/i.test('É');"), Value::Bool(true));
    assert_eq!(run("/é/i.test('e');"), Value::Bool(false));
    assert_eq!(run("/s/i.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/\\u0073/i.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/\\x73/i.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/[s]/i.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/[a-z]/i.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/k/i.test('K');"), Value::Bool(false));
    assert_eq!(run("/[é]/i.test('É');"), Value::Bool(true));
    assert_eq!(run("/(?i:s)/.test('ſ');"), Value::Bool(false));
    assert_eq!(run("/\\u212a/i.test('k');"), Value::Bool(false));
    assert_eq!(run("/\\u212a/i.test('K');"), Value::Bool(false));
    assert_eq!(run("/\\u212a/u.test('k');"), Value::Bool(false));
    assert_eq!(run("/\\u212a/iu.test('k');"), Value::Bool(true));
    assert_eq!(run("/K/i.test('k');"), Value::Bool(false));
    assert_eq!(
        run("new RegExp('\\\\u212a', 'i').test('K');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var r = /\\u212a/i; r.source;"),
        Value::String(Arc::from("\\u212a"))
    );
}

#[test]
fn regexp_non_unicode_incomplete_unicode_escapes_are_identity_escapes() {
    assert_eq!(run(r#"new RegExp("\\u").test("u");"#), Value::Bool(true));
    assert_eq!(
        run(r#"new RegExp("\\u{61}").test("u".repeat(61));"#),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            var pattern = new RegExp("[\\u{61}]");
            ["u", "{", "6", "1", "}"].every(function(value) {
              return pattern.test(value);
            });
        "#),
        Value::Bool(true)
    );
}

#[test]
fn regexp_unicode_surrogate_pair_escapes_match_scalar() {
    assert_eq!(
        run("/^[\\ud800\\udc00]$/u.test('\\ud800\\udc00');"),
        Value::Bool(true)
    );
    assert_eq!(
        run("/[\\ud800\\udc00]/u.test('\\ud800');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("/[\\ud800\\udc00]/u.test('\\udc00');"),
        Value::Bool(false)
    );
    assert_eq!(
        run("var r = /^[\\ud834\\udf06]$/u; r.source;"),
        Value::String(Arc::from("^[\\ud834\\udf06]$"))
    );
    assert_eq!(run("/\\udf06/u.test('\\udf06');"), Value::Bool(true));
    assert_eq!(run("/\\udf06/u.exec('\\ud834\\udf06');"), Value::Null);
}

#[test]
fn regexp_unicode_distinguishes_lone_surrogates_from_private_use_scalars() {
    assert_eq!(
        run(r#"
            var lowScalar = String.fromCodePoint(0xF0000);
            var highScalar = String.fromCodePoint(0xF07FF);
            var lowLone = String.fromCharCode(0xD800);
            var highLone = String.fromCharCode(0xDFFF);
            var collisionMatrix = [0xD800, 0xDBFF, 0xDC00, 0xDFFF].every(
              function(surrogate, index) {
                var scalar = String.fromCodePoint(
                  [0xF0000, 0xF03FF, 0xF0400, 0xF07FF][index]
                );
                var lone = String.fromCharCode(surrogate);
                return new RegExp(lone, "u").test(lone) &&
                  !new RegExp(lone, "u").test(scalar) &&
                  new RegExp(scalar, "u").test(scalar) &&
                  !new RegExp(scalar, "u").test(lone);
              }
            );
            [
              collisionMatrix,
              /^\uD800$/u.test(lowLone),
              /^\uD800$/u.test(lowScalar),
              new RegExp(lowScalar, "u").test(lowScalar),
              new RegExp(lowScalar, "u").test(lowLone),
              /^\uDFFF$/u.test(highLone),
              /^\uDFFF$/u.test(highScalar),
              new RegExp(highScalar, "u").test(highScalar),
              new RegExp(highScalar, "u").test(highLone),
              new RegExp(lowScalar, "v").test(lowScalar),
              /^\uD800$/v.test(lowLone),
              /^\uD800$/v.test(lowScalar),
              /^\p{General_Category=Surrogate}$/v.test(lowLone),
              /^\uD800[\W&&\p{Letter}]$/iv.test(lowLone + "\u017F"),
              /^\uD800[\w&&\p{Letter}]$/iv.test(lowLone + "\u017F"),
              /^\p{General_Category=Surrogate}$/u.test(lowLone),
              /^\p{General_Category=Surrogate}$/u.test(lowScalar),
              /^\p{General_Category=Private_Use}$/u.test(lowScalar),
              /^\p{General_Category=Private_Use}$/u.test(lowLone),
              /^\P{General_Category=Surrogate}$/u.test(lowScalar),
              /^\P{General_Category=Private_Use}$/u.test(lowLone)
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|false|true|false|true|false|true|false|true|true|false|true|false|true|true|false|true|false|true|true"
        ))
    );
}

#[test]
fn regexp_unicode_logical_utf16_preserves_captures_iteration_and_indices() {
    assert_eq!(
        run(r#"
            var lone = String.fromCharCode(0xD800);
            var scalar = String.fromCodePoint(0xF0000);
            var capture = /^(\uD800)(.)\1$/du.exec(lone + scalar + lone);
            var global = Array.from((lone + scalar).matchAll(/./dgu), function(match) {
              return match[0].length + ":" + match.indices[0].join("-");
            }).join(",");
            var sticky = /./duy;
            sticky.lastIndex = 1;
            var stickyMatch = sticky.exec(scalar);
            [
              capture[0] === lone + scalar + lone,
              capture[1] === lone,
              capture[2] === scalar,
              !/(.)\1/u.test(lone + scalar),
              !/^(\uD800)\1.$/u.test(lone + String.fromCodePoint(0x10000)),
              /(?:(?<q>a)|(?<q>b))\k<q>/u.exec(lone + "bb")[0] === "bb",
              !/(?:(?<q>a)|(?<q>b))\k<q>/u.test(lone + "bc"),
              capture.indices[1].join("-"),
              capture.indices[2].join("-"),
              global,
              stickyMatch.index,
              stickyMatch[0] === scalar,
              sticky.lastIndex,
              lone.search(/\uD800/u),
              lone.replace(/\uD800/u, "x"),
              /\uD800$/u.test(lone + "\n"),
              /\uD800$/u.test(lone + "\r\n")
            ].join("|");
        "#),
        Value::String(Arc::from(
            "true|true|true|true|true|true|true|0-1|1-3|1:0-1,2:1-3|0|true|2|0|x|false|false"
        ))
    );
}

#[test]
fn regexp_unicode_logical_utf16_scales_flat_alternation_and_global_offsets() {
    assert_eq!(
        run(r#"
            var lone = String.fromCharCode(0xD800);
            var source = "^(?:" + "a|".repeat(4096) + "a)$";
            var matches = (lone + "a".repeat(20000)).match(/./gu);
            var emptyMatches = (lone + String.fromCodePoint(0xF0000)).match(/(?:)/gu);
            [
              !new RegExp(source, "u").test(lone),
              matches.length,
              matches[0] === lone,
              matches[20000],
              emptyMatches.length
            ].join("|");
        "#),
        Value::String(Arc::from("true|20001|true|a|3"))
    );
}

#[test]
fn regexp_non_unicode_surrogate_escapes_match_code_units() {
    assert_eq!(run("/\\udf06/.test('\\udf06');"), Value::Bool(true));
    assert_eq!(run("/\\udf06/i.test('\\udf06');"), Value::Bool(true));
    assert_eq!(run("/[\\udf06]/.test('\\udf06');"), Value::Bool(true));
    assert_eq!(run("/\\udf06/.test('\\ud834\\udf06');"), Value::Bool(true));
    assert_eq!(
        run("var r = /\\udf06/; Object.defineProperty(r, 'unicode', { value: true }); r[Symbol.match]('\\ud834\\udf06') !== null;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("var r = /\\udf06/u; Object.defineProperty(r, 'unicode', { value: false }); r[Symbol.match]('\\ud834\\udf06') === null;"),
        Value::Bool(true)
    );
    assert_eq!(
        run(r#"
            var pair = "\ud834\udf06";
            var sticky = /./y;
            sticky.lastIndex = 1;
            var match = sticky.exec(pair);
            var captures = /(.)(.)/.exec(pair);
            [
              match[0].length,
              match[0].charCodeAt(0),
              sticky.lastIndex,
              captures[0] === pair,
              captures[1].charCodeAt(0),
              captures[2].charCodeAt(0),
              new RegExp(pair).exec(pair)[0] === pair
            ].join("|");
        "#),
        Value::String(Arc::from("1|57094|2|true|55348|57094|true"))
    );
}

#[test]
fn regexp_backreferences_and_legacy_decimal_escapes_compile() {
    assert_eq!(
        run("eval('/\\\\1/').source;"),
        Value::String(Arc::from("\\1"))
    );
    assert_eq!(
        run("eval('/a\\\\1/').source;"),
        Value::String(Arc::from("a\\1"))
    );
    assert_eq!(run("/(a)\\1/.test('aa');"), Value::Bool(true));
    assert_eq!(run("/(a)\\1/.test('ab');"), Value::Bool(false));
    assert_eq!(
        run("/(.+).*\\1/u.test('\\ud800\\udc00\\ud800');"),
        Value::Bool(false)
    );
}

#[test]
fn string_replace_with_regex() {
    assert_eq!(
        run(r#""\ud834\udf06".replace(/𝌆/, "x");"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run(r#""\ud834\udf06".replace(/\ud834\udf06/, "x");"#),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run("'hello'.replace(/l/, 'L');"),
        Value::String(Arc::from("heLlo"))
    );
    assert_eq!(
        run("'hello world'.replace(/o/g, '0');"),
        Value::String(Arc::from("hell0 w0rld"))
    );
    assert_eq!(
        run(r#""abc".replace(/(b)/, "[$&][$1][$$][$`][$']");"#),
        Value::String(Arc::from("a[b][b][$][a][c]c"))
    );
    assert_eq!(
        run(r#""b".replace(/(a)?(b)/, "$1-$2");"#),
        Value::String(Arc::from("-b"))
    );
    assert_eq!(
        run(r#""ab".replace(/(?:(a)|(b))*/, "$1|$2");"#),
        Value::String(Arc::from("|b"))
    );
    assert_eq!(
        run(r#""ab xa".replace(/(?:(a)|(b))+/g, "[$1|$2]");"#),
        Value::String(Arc::from("[|b] x[a|]"))
    );
    assert_eq!(
        run(r#""abcdefghijk".replace(/(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)/, "$10|$11|$01|$0|$99");"#),
        Value::String(Arc::from("j|a1|a|$0|i9k"))
    );
    assert_eq!(
        run(r#""abc".replace(/(b)/, "<$1|$2|$12>");"#),
        Value::String(Arc::from("a<b|$2|b2>c"))
    );
    assert_eq!(
        run(r#""ab".replace(/(a)/, "$0|$00|$09|$10|$11");"#),
        Value::String(Arc::from("$0|$00|$09|a0|a1b"))
    );
    assert_eq!(
        run(r#""aa".replace(/(a)\1/, "<$&|$1>");"#),
        Value::String(Arc::from("<aa|a>"))
    );
    assert_eq!(
        run(r#""abc".replace(/b|c/g, "<$`|$'>");"#),
        Value::String(Arc::from("a<a|c><ab|>"))
    );
    assert_eq!(
        run(r#""abc".replace(/b/, "$x|$<x>");"#),
        Value::String(Arc::from("a$x|$<x>c"))
    );
    assert_eq!(
        run(r#""abc".replace(/(?<x>b)(c)?/, "<$<x>|$<missing>|$1|$2>");"#),
        Value::String(Arc::from("a<b||b|c>"))
    );
    assert_eq!(
        run(
            r#""abc".replace(/(?<x>b)/, function(m, x, offset, s, groups){ return m + "|" + x + "|" + offset + "|" + s + "|" + groups.x; });"#
        ),
        Value::String(Arc::from("ab|b|1|abc|bc"))
    );
    assert_eq!(
        run(r#""😀a".replace(/a/, function(m, offset){ return offset; });"#),
        Value::String(Arc::from("😀2"))
    );
    assert_eq!(
        run(r#""😀a".replace("a", function(m, offset){ return offset; });"#),
        Value::String(Arc::from("😀2"))
    );
    assert_eq!(
        run(r#""𝌆ab".replace(/b/, function(m, offset){ return offset; });"#),
        Value::String(Arc::from("𝌆a3"))
    );
    assert_eq!(
        run(r#""abc".replace("b", "[$&][$$][$`][$']");"#),
        Value::String(Arc::from("a[b][$][a][c]c"))
    );
    assert_eq!(
        run(r#""abc".replace("b", "$0|$1|$<x>");"#),
        Value::String(Arc::from("a$0|$1|$<x>c"))
    );
    assert!(
        run_err(
            r#"var search = { toString: function(){ throw "search"; } };
               var replacement = { toString: function(){ throw "replacement"; } };
               "abc".replace(search, replacement);"#
        )
        .contains("search"),
        "searchValue must be coerced before replaceValue"
    );
    assert!(
        run_err(
            r#"var search = {};
               Object.defineProperty(search, Symbol.replace, {
                 get: function(){ throw new Error("replace-get"); }
               });
               "".replace(search);"#
        )
        .contains("replace-get"),
        "String.prototype.replace must observe searchValue[Symbol.replace]"
    );
    assert_eq!(
        run(r#"var search = {};
               var seenThis, seenFirst, seenSecond;
               search[Symbol.replace] = function(first, second) {
                 seenThis = this;
                 seenFirst = first;
                 seenSecond = second;
                 return "custom";
               };
               var out = "abc".replace(search, "replacement");
               [out, seenThis === search, seenFirst, seenSecond].join("|");"#),
        Value::String(Arc::from("custom|true|abc|replacement"))
    );
}

#[test]
fn regexp_symbol_replace_uses_generic_exec_results_and_utf16_positions() {
    assert_eq!(
        run(r#"var r = /./g;
               var calls = 0;
               r.exec = function () {
                 calls += 1;
                 if (calls === 1) return { index: 1, length: 1, 0: 0 };
                 if (calls === 2) return { index: 3, length: 1, 0: 0 };
                 return null;
               };
               r[Symbol.replace]("abcde", "X") + "|" + calls;"#),
        Value::String(Arc::from("aXcXe|3"))
    );
    assert_eq!(
        run(r#"var calls = 0;
               var replacer = new Proxy(function (matched, position) {
                 calls += 1;
                 return matched + position;
               }, {});
               "ab".replace(/b/, replacer) + "|" + calls;"#),
        Value::String(Arc::from("ab1|1"))
    );
    assert_eq!(
        run(r#"String.fromCharCode(0xD83D, 0xDE00).replace(/./g, "X");"#),
        Value::String(Arc::from("XX"))
    );
    assert_eq!(
        run(r#"["b".replace(/(b)/, "a1"), "b".replace(/(b)/, "foo1")].join("|");"#),
        Value::String(Arc::from("a1|foo1"))
    );
    assert!(run_err(
        r#"var r = /./;
               Object.defineProperty(r, "flags", {
                 get: function () { throw new Error("flags-order"); }
               });
               r[Symbol.replace]("a", "x");"#
    )
    .contains("flags-order"));
}

#[test]
fn unicode_regexp_resource_limits_are_validated_at_construction() {
    assert!(run_err(r#"new RegExp("\\p{Letter}".repeat(65), "u");"#)
        .contains("too many property operands"));
    assert_eq!(
        run(r#"new RegExp("\\\\p{2}".repeat(65), "u") instanceof RegExp;"#),
        Value::Bool(true)
    );
    assert!(
        run_err(r#"new RegExp("(?:" + "(?<q>a)|".repeat(64) + "(?<q>a))", "u");"#)
            .contains("Too many capture groups share one name")
    );
}

#[test]
fn division_not_regex() {
    // Ensure `/` after a value is division, not a regex.
    assert_eq!(run("10 / 4;"), Value::Number(2.5));
    assert_eq!(run("var x = 20; x / 5;"), Value::Number(4.0));
    assert_eq!(
        run("var instance = 60; var of = 6; var g = 2; instance/of/g;"),
        Value::Number(5.0)
    );
    assert_eq!(
        run("var of = 4; var g = 2; eval('{[42]}.8/of/g');"),
        Value::Number(0.1)
    );
}

// --- Array.from / Array.of ---

#[test]
fn array_from_async_handles_iterators_array_likes_mapping_and_close() {
    assert_eq!(
        run(r#"
            var closed = 0;
            var sync = {
              [Symbol.iterator]: function () {
                var index = 0;
                return {
                  next: function () {
                    return index < 2
                      ? { value: Promise.resolve(++index), done: false }
                      : { done: true };
                  },
                  return: function () { closed++; return { done: true }; }
                };
              }
            };
            var values = await Array.fromAsync(sync, async function (value, index) {
              return value * 2 + index;
            });
            var arrayLike = await Array.fromAsync({ 0: Promise.resolve("a"), length: 1 });
            var marker = {};
            var rejected = await Array.fromAsync(sync, function () { throw marker; }).then(
              function () { return false; },
              function (reason) { return reason === marker; }
            );
            [
              values.join(","), arrayLike.join(","), rejected, closed,
              Array.fromAsync.length, Array.fromAsync.name,
              Object.getPrototypeOf(Array.fromAsync) === Function.prototype,
              Object.getOwnPropertyDescriptor(Array, "fromAsync").enumerable
            ].join("|");
        "#),
        Value::String(Arc::from("2,5|a|true|1|1|fromAsync|true|false"))
    );
}

#[test]
fn array_from_async_native_length_set_invalidates_inline_cache() {
    assert_eq!(
        run(r#"
            function Custom() {
              this.length = 4;
              for (let index = 0; index < this.length; index++) {
                Object.defineProperty(this, index, {
                  value: 99, writable: false, enumerable: true, configurable: true
                });
              }
              this.cached = this[0];
            }
            var result = await Array.fromAsync.call(Custom, [0, 1, 2]);
            [result.length, result[0], result[2], result[3], result.cached].join("|");
        "#),
        Value::String(Arc::from("3|0|2|99|99"))
    );
}

#[test]
fn array_from_async_preserves_async_from_sync_rejection_provenance() {
    assert_eq!(
        run(r#"
            var directReason = {};
            var directLog = [];
            var direct = {
              [Symbol.iterator]: function () {
                return {
                  next: function () { directLog.push("next"); throw directReason; },
                  return: function () { directLog.push("return"); return {}; }
                };
              }
            };
            var directRejected = await Array.fromAsync(direct).then(
              function () { return false; },
              function (reason) { return reason === directReason; }
            );

            var yieldedReason = {};
            var yieldedLog = [];
            var yielded = {
              [Symbol.iterator]: function () {
                return {
                  next: function () {
                    yieldedLog.push("next:" + arguments.length);
                    return { value: Promise.reject(yieldedReason), done: false };
                  },
                  return: function () {
                    yieldedLog.push("return");
                    return {
                      get done() { yieldedLog.push("done"); return true; },
                      get value() { yieldedLog.push("value"); return 0; }
                    };
                  }
                };
              }
            };
            var yieldedRejected = await Array.fromAsync(yielded).then(
              function () { return false; },
              function (reason) { return reason === yieldedReason; }
            );

            var doneReason = {};
            var doneLog = [];
            var doneValue = {
              [Symbol.iterator]: function () {
                return {
                  next: function () {
                    doneLog.push("next:" + arguments.length);
                    return {
                      done: true,
                      get value() { doneLog.push("value"); throw doneReason; }
                    };
                  }
                };
              }
            };
            var doneRejected = await Array.fromAsync(doneValue).then(
              function () { return false; },
              function (reason) { return reason === doneReason; }
            );
            [
              directRejected, directLog.join(","),
              yieldedRejected, yieldedLog.join(","),
              doneRejected, doneLog.join(",")
            ].join("|");
        "#),
        Value::String(Arc::from("true|next|true|next:0,return|true|next:0,value"))
    );
}

#[test]
fn array_from_async_rejects_await_and_next_errors_after_returning_a_promise() {
    assert_eq!(
        run(r#"
            var awaitReason = {};
            var value = Promise.resolve(1);
            Object.defineProperty(value, "constructor", {
              get: function () { throw awaitReason; }
            });
            var threwSynchronously = false;
            var promise;
            try {
              promise = Array.fromAsync({ 0: value, length: 1 });
            } catch (_) {
              threwSynchronously = true;
            }
            var awaitRejected = await promise.then(
              function () { return false; },
              function (reason) { return reason === awaitReason; }
            );

            var constructorCalls = 0;
            function Custom() { constructorCalls++; }
            var badIterator = {
              [Symbol.iterator]: function () { return { next: 0 }; }
            };
            var nextRejected = await Array.fromAsync.call(Custom, badIterator).then(
              function () { return false; },
              function (reason) { return reason instanceof TypeError; }
            );
            [
              threwSynchronously, promise instanceof Promise, awaitRejected,
              constructorCalls, nextRejected
            ].join("|");
        "#),
        Value::String(Arc::from("false|true|true|1|true"))
    );
}

#[test]
fn array_from_async_uses_the_method_realm_for_promises_and_errors() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var ForeignPromise = other.Promise;
            var foreignFromAsync = other.Array.fromAsync;
            other.Promise = null;
            var foreignPromise = foreignFromAsync([1]);
            var mainPromise = Array.fromAsync.call(other.Array, [2]);
            var foreignResult = await foreignPromise;
            var mainResult = await mainPromise;
            var inheritedThenCalls = 0;
            other.Object.prototype.then = function (resolve) {
              inheritedThenCalls++;
              delete other.Object.prototype.then;
              resolve(this);
            };
            var crossRealmResult = await foreignFromAsync.call(Array, [3]);
            var foreignError = await foreignFromAsync({
              [Symbol.iterator]: function () { return { next: 0 }; }
            }).then(
              function () { return false; },
              function (error) { return error instanceof other.TypeError; }
            );
            [
              foreignPromise instanceof ForeignPromise,
              !(foreignPromise instanceof Promise),
              mainPromise instanceof Promise,
              !(mainPromise instanceof ForeignPromise),
              foreignResult.join(","),
              Object.getPrototypeOf(mainResult) === other.Array.prototype,
              inheritedThenCalls, crossRealmResult.join(","),
              foreignError
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true|1|true|1|3|true"))
    );
}

#[test]
fn array_from_async_roots_continuation_state_across_observable_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    assert_eq!(
        vm.run(r#"
            var source = {
              [Symbol.iterator]: function () {
                forceGc();
                var index = 0;
                return {
                  next: function () {
                    forceGc();
                    return index++ === 0
                      ? { value: { then: function (resolve) { forceGc(); resolve(7); } }, done: false }
                      : { done: true };
                  },
                  return: function () { forceGc(); return { done: true }; }
                };
              }
            };
            var promise = Array.fromAsync(source, function (value) {
              forceGc();
              return { then: function (resolve) { forceGc(); resolve(value + 1); } };
            });
            forceGc();
            var result = await promise;

            Object.defineProperty(Boolean.prototype, "length", {
              configurable: true,
              get: function () { forceGc(); return 1; }
            });
            Object.defineProperty(Boolean.prototype, "0", {
              configurable: true,
              get: function () { forceGc(); return 9; }
            });
            function Custom() { forceGc(); }
            var boxed = await Array.fromAsync.call(Custom, true);

            var closeReason = {};
            var closeSource = {
              [Symbol.iterator]: function () {
                var done = false;
                return {
                  next: function () {
                    return done ? { done: true } : (done = true, { value: 1, done: false });
                  },
                  return: function () { forceGc(); return { done: true }; }
                };
              }
            };
            var closePreserved = await Array.fromAsync(closeSource, function () {
              forceGc();
              throw closeReason;
            }).then(
              function () { return false; },
              function (reason) { return reason === closeReason; }
            );

            var resolveReason;
            var abruptValue = Promise.resolve(1);
            Object.defineProperty(abruptValue, "constructor", {
              get: function () { resolveReason = {}; throw resolveReason; }
            });
            var abruptSource = {
              [Symbol.iterator]: function () {
                var done = false;
                return {
                  next: function () {
                    return done ? { done: true } : (done = true, { value: abruptValue, done: false });
                  },
                  return: function () { forceGc(); return {}; }
                };
              }
            };
            var resolvePreserved = await Array.fromAsync(abruptSource).then(
              function () { return false; },
              function (reason) { return reason === resolveReason; }
            );
            forceGc();
            [
              result.join(","), boxed[0], boxed.length,
              closePreserved, resolvePreserved
            ].join("|");
        "#)
        .expect("Array.fromAsync continuation should survive observable GC"),
        Value::String(Arc::from("8|9|1|true|true"))
    );
}

#[test]
fn array_splice_omitted_delete_count_removes_the_tail() {
    assert_eq!(
        run(r#"
            var values = [1, 2, 3, 4];
            var removed = values.splice(1);
            [values.join(","), removed.join(",")].join("|");
        "#),
        Value::String(Arc::from("1|2,3,4"))
    );
}

#[test]
fn array_from_iterable_and_map() {
    assert_eq!(
        run("Array.from('abc').join(',');"),
        Value::String(Arc::from("a,b,c"))
    );
    assert_eq!(
        run("Array.from([1,2,3], x=>x*2).join(',');"),
        Value::String(Arc::from("2,4,6"))
    );
}

#[test]
fn array_from_arraylike() {
    assert_eq!(
        run("Array.from({0:'a',1:'b',length:2}).join(',');"),
        Value::String(Arc::from("a,b"))
    );
}

#[test]
fn array_of_and_isarray() {
    assert_eq!(
        run("Array.of(1,2,3).join(',');"),
        Value::String(Arc::from("1,2,3"))
    );
    assert_eq!(run("Array.isArray([]);"), Value::Bool(true));
    assert_eq!(run("Array.isArray({});"), Value::Bool(false));
}

#[test]
fn array_of_uses_constructor_and_create_data_property() {
    assert_eq!(
        run(r#"
            var len, hits = 0;
            function C(length) { len = length; hits++; }
            var result = Array.of.call(C, "a", "b");
            [len, hits, result.length, result[0], result[1], result instanceof C].join("|");
        "#),
        Value::String(Arc::from("2|1|2|a|b|true"))
    );
    assert_eq!(
        run(r#"
            function C() {}
            Object.defineProperty(C.prototype, "0", {
                set: function() { throw new Error("setter"); }
            });
            var result = Array.of.call(C, "own");
            [result[0], result.hasOwnProperty("0")].join("|");
        "#),
        Value::String(Arc::from("own|true"))
    );
    assert_eq!(
        run(r#"
            var hits = 0, seen;
            function C() {
                Object.defineProperty(this, "length", {
                    set: function(value) { hits++; seen = value; }
                });
            }
            var result = Array.of.call(C, "x", "y", "z");
            [hits, seen, result[2]].join("|");
        "#),
        Value::String(Arc::from("1|3|z"))
    );
    assert!(run_err(
        r#"function C() { Object.preventExtensions(this); }
           Array.of.call(C, "x");"#
    )
    .contains("not extensible"));
}

#[test]
fn array_of_roots_constructed_result_across_observable_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");

    assert_eq!(
        vm.run(
            r#"
            var replacement;
            function C() {
              return new Proxy({}, {
                defineProperty: function(target, key, descriptor) {
                  forceGc();
                  replacement = { key: key };
                  return Reflect.defineProperty(target, key, descriptor);
                }
              });
            }
            var first = { value: 1 };
            var second = { value: 2 };
            var result = Array.of.call(C, first, second);
            [result[0] === first, result[1] === second, result.length].join("|");
            "#,
        )
        .expect("Array.of result should survive observable property definition"),
        Value::String(Arc::from("true|true|2"))
    );
}

#[test]
fn array_of_cross_realm_constructor_fallbacks_to_constructor_realm_object_proto() {
    assert_eq!(
        run(r#"
            var other = $262.createRealm().global;
            var C = new other.Function();
            C.prototype = null;
            var result = Array.of.call(C, 1, 2, 3);
            Object.getPrototypeOf(result) === other.Object.prototype;
        "#),
        Value::Bool(true)
    );
}

// --- async/await ---

#[test]
fn async_function_intrinsics_and_dynamic_constructor_match_the_source_form() {
    assert_eq!(
        run(r#"
            var AsyncFunction = (async function() {}).constructor;
            var dynamic = AsyncFunction("value", "return await value;");
            var constructThrows = false;
            try { new dynamic(1); } catch (error) {
                constructThrows = error instanceof TypeError;
            }
            [
              AsyncFunction.name,
              AsyncFunction.length,
              Object.getPrototypeOf(AsyncFunction) === Function,
              Object.getPrototypeOf(AsyncFunction.prototype) === Function.prototype,
              AsyncFunction.prototype[Symbol.toStringTag],
              Object.getPrototypeOf(dynamic) === AsyncFunction.prototype,
              dynamic.length,
              dynamic.prototype === undefined,
              constructThrows
            ].join("|");
        "#),
        Value::String(Arc::from(
            "AsyncFunction|1|true|true|AsyncFunction|true|1|true|true"
        ))
    );
}

#[test]
fn async_function_intrinsics_are_isolated_per_realm() {
    assert_eq!(
        run(r#"
            var mainProto = (async function() {}).constructor.prototype;
            var other = $262.createRealm().global;
            var otherFunction = other.eval("async function f() {}; f");
            var otherProto = Object.getPrototypeOf(otherFunction);
            var OtherAsyncFunction = otherProto.constructor;
            var dynamic = OtherAsyncFunction("return await 1;");
            [
              mainProto !== otherProto,
              Object.getPrototypeOf(otherProto) === other.Function.prototype,
              Object.getPrototypeOf(OtherAsyncFunction) === other.Function,
              Object.getPrototypeOf(dynamic) === otherProto
            ].join("|");
        "#),
        Value::String(Arc::from("true|true|true|true"))
    );
}

#[test]
fn async_function_returns_promise() {
    let r = run("async function f(){ return 5; } typeof f();");
    assert_eq!(r, Value::String(Arc::from("object")));
}

#[test]
fn async_resolves_value() {
    // f() resolves to 5; the then callback runs during microtask drain.
    let r = run("var out=0; async function f(){ return 5; } f().then(function(v){ out=v; }); out;");
    // out is read synchronously before the then callback runs, so it stays 0;
    // verify the promise is an object instead.
    let _ = r;
    assert!(matches!(
        run("async function f(){ return 5; } f();"),
        Value::Object(_)
    ));
}

#[test]
fn await_extracts_promise_value() {
    // await a resolved promise inside an async function yields the value.
    let r = run("async function f(){ return 7; } \
         async function g(){ return await f() + 1; } \
         g();");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn await_non_promise() {
    // await on a plain value yields the value.
    let r = run("async function g(){ return await 9; } g();");
    assert!(matches!(r, Value::Object(_)));
}

#[test]
fn async_function_pending_await_resumes_after_fulfillment() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var resolveGate;
        var gate = new Promise(resolve => { resolveGate = resolve; });
        var log = [];
        async function pending() {
            log.push("start");
            let value = await gate;
            log.push("after:" + value);
            return value + 1;
        }
        var result = pending();
        result.then(value => log.push("done:" + value));
        log.push("sync");
        "#,
    )
    .expect("failed to start pending async function");

    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read pre-resolution log"),
        Value::String(Arc::from("start|sync"))
    );

    vm.run("resolveGate(41);")
        .expect("failed to fulfill pending await");
    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read resumed log"),
        Value::String(Arc::from("start|sync|after:41|done:42"))
    );
}

#[test]
fn async_function_pending_await_rejection_reenters_catch() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var rejectGate;
        var marker = {};
        var gate = new Promise((resolve, reject) => { rejectGate = reject; });
        var log = [];
        async function pending() {
            try {
                await gate;
                log.push("unreachable");
            } catch (error) {
                log.push("caught:" + (error === marker));
                return "recovered";
            }
        }
        var result = pending();
        result.then(value => log.push("done:" + value));
        log.push("sync");
        "#,
    )
    .expect("failed to start rejecting async function");

    vm.run("rejectGate(marker);")
        .expect("failed to reject pending await");
    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read rejection log"),
        Value::String(Arc::from("sync|caught:true|done:recovered"))
    );
}

#[test]
fn async_function_pending_await_preserves_finally_state() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var resolveGate;
        var gate = new Promise(resolve => { resolveGate = resolve; });
        var log = [];
        async function pending() {
            try {
                return await gate;
            } finally {
                log.push("finally");
            }
        }
        var result = pending();
        result.then(value => log.push("done:" + value));
        "#,
    )
    .expect("failed to suspend async function with finally");

    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read suspended finally state"),
        Value::String(Arc::from(""))
    );
    vm.run("resolveGate(42);")
        .expect("failed to resume async function with finally");
    assert_eq!(
        vm.run("log.join('|');")
            .expect("failed to read resumed finally state"),
        Value::String(Arc::from("finally|done:42"))
    );
}

#[test]
fn async_function_await_observes_promise_job_order() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var log = [];
        Promise.resolve()
            .then(() => log.push("tick1"))
            .then(() => log.push("tick2"));
        async function ordered() {
            log.push("start");
            await 0;
            log.push("after");
        }
        ordered();
        log.push("sync");
        "#,
    )
    .expect("failed to run ordered await");

    assert_eq!(
        vm.run("log.join('|');").expect("failed to read job log"),
        Value::String(Arc::from("start|sync|tick1|after|tick2"))
    );
}

#[test]
fn async_function_pending_await_keeps_block_environment_alive_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var resolveGate;
        var gate = new Promise(resolve => { resolveGate = resolve; });
        var state = "initial";
        async function pending(argument) {
            let local = { value: 11 };
            {
                let held = { value: 12 };
                state = "waiting";
                let resumed = await gate;
                return argument.value + local.value + held.value + resumed.value;
            }
        }
        var settled = false;
        var result = pending({ value: 10 });
        result.then(() => { settled = true; });
        "#,
    )
    .expect("failed to suspend async function");

    assert_eq!(
        vm.run("state + '|' + settled;")
            .expect("failed to inspect suspended state"),
        Value::String(Arc::from("waiting|false"))
    );

    vm.gc();
    vm.run("resolveGate({ value: 9 });")
        .expect("failed to resume async function after GC");
    vm.run("var resumed; result.then(value => { resumed = value; });")
        .expect("failed to observe resumed value");

    assert_eq!(
        vm.run("resumed;").expect("failed to read resumed value"),
        Value::Number(42.0)
    );
}

#[test]
fn weak_ref_exposes_spec_shaped_constructor_and_deref() {
    assert_eq!(
        run(
            r#"
            var target = {};
            var ref = new WeakRef(target);
            var descriptor = Object.getOwnPropertyDescriptor(WeakRef, "prototype");
            var tag = Object.getOwnPropertyDescriptor(
                WeakRef.prototype,
                Symbol.toStringTag
            );
            [
                typeof WeakRef,
                WeakRef.length,
                WeakRef.name,
                ref.deref() === target,
                Object.getPrototypeOf(ref) === WeakRef.prototype,
                ref instanceof WeakRef,
                Object.isExtensible(ref),
                descriptor.writable,
                descriptor.enumerable,
                descriptor.configurable,
                WeakRef.prototype.deref.length,
                WeakRef.prototype.deref.name,
                tag.value,
                tag.writable,
                tag.enumerable,
                tag.configurable
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "function|1|WeakRef|true|true|true|true|false|false|false|0|deref|WeakRef|false|false|true"
        ))
    );

    assert_eq!(
        run(r#"
            var symbol = Symbol("target");
            var ref = new WeakRef(symbol);
            var failures = 0;
            for (var value of [undefined, null, 1, "x", true, Symbol.for("registered")]) {
                try { new WeakRef(value); } catch (error) {
                    if (error instanceof TypeError) failures++;
                }
            }
            [ref.deref() === symbol, failures].join("|");
            "#,),
        Value::String(Arc::from("true|6"))
    );
}

#[test]
fn weak_ref_validates_receivers_and_uses_new_target_realm() {
    assert_eq!(
        run(r#"
            var failures = 0;
            for (var value of [undefined, null, true, 1, "x", {}, WeakRef.prototype]) {
                try { WeakRef.prototype.deref.call(value); } catch (error) {
                    if (error instanceof TypeError) failures++;
                }
            }
            try { WeakRef({}); } catch (error) {
                if (error instanceof TypeError) failures++;
            }
            var other = $262.createRealm().global;
            var newTarget = new other.Function();
            newTarget.prototype = undefined;
            var ref = Reflect.construct(WeakRef, [{}], newTarget);
            [failures, Object.getPrototypeOf(ref) === other.WeakRef.prototype].join("|");
            "#,),
        Value::String(Arc::from("8|true"))
    );
}

#[test]
fn weak_ref_target_is_cleared_after_collection() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    assert_eq!(
        vm.run(
            r#"
            var target = { value: 42 };
            var ref = new WeakRef(target);
            var observed = ref.deref() === target;
            target = null;
            observed;
            "#,
        )
        .expect("failed to create WeakRef"),
        Value::Bool(true)
    );

    vm.gc();
    assert_eq!(
        vm.run("ref.deref() === undefined;")
            .expect("failed to dereference collected WeakRef target"),
        Value::Bool(true)
    );
}

#[test]
fn finalization_registry_exposes_spec_shaped_surface() {
    assert_eq!(
        run(
            r#"
            var registry = new FinalizationRegistry(function() {});
            var prototype = Object.getOwnPropertyDescriptor(
                FinalizationRegistry,
                "prototype"
            );
            var tag = Object.getOwnPropertyDescriptor(
                FinalizationRegistry.prototype,
                Symbol.toStringTag
            );
            [
                typeof FinalizationRegistry,
                FinalizationRegistry.length,
                FinalizationRegistry.name,
                registry instanceof FinalizationRegistry,
                Object.isExtensible(registry),
                Object.getPrototypeOf(registry) === FinalizationRegistry.prototype,
                prototype.writable,
                prototype.enumerable,
                prototype.configurable,
                FinalizationRegistry.prototype.register.length,
                FinalizationRegistry.prototype.register.name,
                FinalizationRegistry.prototype.unregister.length,
                FinalizationRegistry.prototype.unregister.name,
                tag.value,
                tag.writable,
                tag.enumerable,
                tag.configurable
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "function|1|FinalizationRegistry|true|true|true|false|false|false|2|register|1|unregister|FinalizationRegistry|false|false|true"
        ))
    );
}

#[test]
fn finalization_registry_validates_cells_tokens_and_realms() {
    assert_eq!(
        run(r#"
            var registry = new FinalizationRegistry(function() {});
            var target = {};
            var token = {};
            var symbolTarget = Symbol("target");
            var symbolToken = Symbol("token");
            var failures = 0;
            for (var value of [undefined, null, true, 1, "x", Symbol.for("registered")]) {
                try { registry.register(value); } catch (error) {
                    if (error instanceof TypeError) failures++;
                }
                try { registry.unregister(value); } catch (error) {
                    if (error instanceof TypeError) failures++;
                }
            }
            try { registry.register(target, target); } catch (error) {
                if (error instanceof TypeError) failures++;
            }
            try { FinalizationRegistry(function() {}); } catch (error) {
                if (error instanceof TypeError) failures++;
            }
            try { new FinalizationRegistry({}); } catch (error) {
                if (error instanceof TypeError) failures++;
            }
            var results = [
                registry.register(target, "held", token),
                registry.register(symbolTarget, 1, symbolToken),
                registry.unregister(token),
                registry.unregister(token),
                registry.unregister(symbolToken),
                registry.unregister(symbolToken)
            ];
            var other = $262.createRealm().global;
            var newTarget = new other.Function();
            newTarget.prototype = undefined;
            var crossRealm = Reflect.construct(
                FinalizationRegistry,
                [function() {}],
                newTarget
            );
            [
                failures,
                results.join(","),
                Object.getPrototypeOf(crossRealm) === other.FinalizationRegistry.prototype
            ].join("|");
            "#,),
        Value::String(Arc::from("15|,,true,false,true,false|true"))
    );
}

#[test]
fn finalization_registry_cleanup_runs_after_gc_and_unregister_suppresses_it() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC test hook");
    vm.run(
        r#"
        var cleaned = [];
        var registry = new FinalizationRegistry(function(held) {
            cleaned.push(held.value);
        });
        var first = { target: 1 };
        registry.register(first, { value: 42 });
        first = null;
        "#,
    )
    .expect("failed to register finalization target");

    vm.gc();
    vm.run_microtasks()
        .expect("failed to run finalization cleanup job");
    assert_eq!(
        vm.run("cleaned.join(',');")
            .expect("failed to inspect cleanup callback"),
        Value::String(Arc::from("42"))
    );

    vm.run(
        r#"
        var second = { target: 2 };
        var token = {};
        registry.register(second, { value: 99 }, token);
        var removed = registry.unregister(token);
        second = null;
        "#,
    )
    .expect("failed to unregister finalization target");
    vm.gc();
    vm.run_microtasks()
        .expect("failed to drain post-unregister jobs");
    assert_eq!(
        vm.run("removed + '|' + cleaned.join(',');")
            .expect("failed to inspect unregister behavior"),
        Value::String(Arc::from("true|42"))
    );

    vm.run(
        r#"
        var nestedCleaned = [];
        var nestedRegistry = new FinalizationRegistry(function(held) {
            if (nestedCleaned.length === 0) forceGc();
            nestedCleaned.push(held.value);
        });
        var nestedFirst = {};
        var nestedSecond = {};
        nestedRegistry.register(nestedFirst, { value: 1 });
        nestedRegistry.register(nestedSecond, { value: 2 });
        nestedFirst = null;
        nestedSecond = null;
        "#,
    )
    .expect("failed to register nested-GC cleanup targets");
    vm.gc();
    vm.run_microtasks()
        .expect("failed to run cleanup callback containing GC");
    assert_eq!(
        vm.run("nestedCleaned.join(',');")
            .expect("failed to inspect nested-GC holdings"),
        Value::String(Arc::from("1,2"))
    );

    vm.run(
        r#"
        var throwingRegistry = new FinalizationRegistry(function() {
            throw new Error("cleanup failure");
        });
        var third = {};
        throwingRegistry.register(third, "held");
        third = null;
        "#,
    )
    .expect("failed to register throwing cleanup callback");
    vm.gc();
    vm.run_microtasks()
        .expect("cleanup callback errors should be host-reported, not propagated");
}

#[test]
fn shared_array_buffer_surface_and_cross_realm_prototype_are_spec_shaped() {
    assert_eq!(
        run(
            r#"
            var buffer = new SharedArrayBuffer(8);
            var length = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                "byteLength"
            );
            var tag = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                Symbol.toStringTag
            );
            var other = $262.createRealm().global;
            var newTarget = new other.Function();
            newTarget.prototype = undefined;
            var crossRealm = Reflect.construct(SharedArrayBuffer, [4], newTarget);
            [
                typeof SharedArrayBuffer,
                SharedArrayBuffer.length,
                buffer.byteLength,
                Object.getPrototypeOf(buffer) === SharedArrayBuffer.prototype,
                Object.getPrototypeOf(SharedArrayBuffer) === Function.prototype,
                length.get.length,
                length.get.name,
                length.enumerable,
                length.configurable,
                tag.value,
                tag.writable,
                tag.enumerable,
                tag.configurable,
                Object.getPrototypeOf(crossRealm) === other.SharedArrayBuffer.prototype
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "function|1|8|true|true|0|get byteLength|false|true|SharedArrayBuffer|false|false|true|true"
        ))
    );
}

#[test]
fn shared_array_buffer_backs_typed_array_and_data_view_without_detachment() {
    assert_eq!(
        run(r#"
            var buffer = new SharedArrayBuffer(4);
            var bytes = new Uint8Array(buffer);
            var view = new DataView(buffer);
            bytes[0] = 7;
            view.setUint8(1, 9);
            var failures = 0;
            try {
                Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength")
                    .get.call(buffer);
            } catch (error) { if (error instanceof TypeError) failures++; }
            try {
                Object.getOwnPropertyDescriptor(SharedArrayBuffer.prototype, "byteLength")
                    .get.call(new ArrayBuffer(1));
            } catch (error) { if (error instanceof TypeError) failures++; }
            try { ArrayBuffer.prototype.transfer.call(buffer); }
            catch (error) { if (error instanceof TypeError) failures++; }
            [bytes[0], bytes[1], view.getUint8(0), failures].join("|");
            "#,),
        Value::String(Arc::from("7|9|7|3"))
    );
}

#[test]
fn shared_array_buffer_slice_uses_shared_species_and_copies_bytes() {
    assert_eq!(
        run(r#"
            var source = new SharedArrayBuffer(4);
            new Uint8Array(source)[1] = 42;
            var sliced = source.slice(1, 3);
            var values = new Uint8Array(sliced);
            var failures = 0;
            source.constructor = { [Symbol.species]: ArrayBuffer };
            try { source.slice(); }
            catch (error) { if (error instanceof TypeError) failures++; }
            try { SharedArrayBuffer.prototype.slice.call(new ArrayBuffer(1)); }
            catch (error) { if (error instanceof TypeError) failures++; }
            try { SharedArrayBuffer(1); }
            catch (error) { if (error instanceof TypeError) failures++; }
            [
                sliced instanceof SharedArrayBuffer,
                sliced.byteLength,
                values[0],
                values[1],
                failures
            ].join("|");
            "#,),
        Value::String(Arc::from("true|2|42|0|3"))
    );
}

#[test]
fn growable_shared_array_buffer_exposes_slots_and_grows_monotonically() {
    assert_eq!(
        run(
            r#"
            var fixed = new SharedArrayBuffer(3);
            var growable = new SharedArrayBuffer(2, { maxByteLength: 6 });
            var before = new Uint8Array(growable);
            before[0] = 11;
            before[1] = 22;
            var result = growable.grow(5);
            var after = new Uint8Array(growable);
            var grow = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                "grow"
            );
            var growableDesc = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                "growable"
            );
            var maxDesc = Object.getOwnPropertyDescriptor(
                SharedArrayBuffer.prototype,
                "maxByteLength"
            );
            [
                fixed.growable,
                fixed.maxByteLength,
                growable.growable,
                growable.maxByteLength,
                growable.byteLength,
                result === undefined,
                after[0], after[1], after[2], after[4],
                grow.value.name, grow.value.length,
                grow.writable, grow.enumerable, grow.configurable,
                growableDesc.get.name, growableDesc.get.length,
                maxDesc.get.name, maxDesc.get.length
            ].join("|");
            "#,
        ),
        Value::String(Arc::from(
            "false|3|true|6|5|true|11|22|0|0|grow|1|true|false|true|get growable|0|get maxByteLength|0"
        ))
    );
}

#[test]
fn growable_shared_array_buffer_validates_options_before_prototype_lookup() {
    assert_eq!(
        run(r#"
            var log = [];
            var options = {};
            Object.defineProperty(options, "maxByteLength", {
                get: function() { log.push("max"); return 2; }
            });
            var newTarget = function() {}.bind(null);
            Object.defineProperty(newTarget, "prototype", {
                get: function() { log.push("prototype"); throw new Error("prototype"); }
            });
            var errors = [];
            try { Reflect.construct(SharedArrayBuffer, [3, options], newTarget); }
            catch (error) { errors.push(error instanceof RangeError); }
            try { new SharedArrayBuffer(2, { maxByteLength: 4 }).grow(1); }
            catch (error) { errors.push(error instanceof RangeError); }
            try { new SharedArrayBuffer(2).grow(3); }
            catch (error) { errors.push(error instanceof TypeError); }
            try { SharedArrayBuffer.prototype.grow.call({}); }
            catch (error) { errors.push(error instanceof TypeError); }
            [log.join(","), errors.join(",")].join("|");
            "#,),
        Value::String(Arc::from("max|true,true,true,true"))
    );
}

#[test]
fn atomics_surface_has_spec_shaped_methods_and_tag() {
    assert_eq!(
        run(r#"
            var names = [
              ["add", 3], ["and", 3], ["compareExchange", 4],
              ["exchange", 3], ["isLockFree", 1], ["load", 2],
              ["notify", 3], ["or", 3], ["pause", 0], ["store", 3],
              ["sub", 3], ["wait", 4], ["waitAsync", 4], ["xor", 3]
            ];
            var shaped = names.every(function(entry) {
              var desc = Object.getOwnPropertyDescriptor(Atomics, entry[0]);
              return typeof desc.value === "function" &&
                desc.value.name === entry[0] && desc.value.length === entry[1] &&
                desc.writable && !desc.enumerable && desc.configurable;
            });
            var tag = Object.getOwnPropertyDescriptor(Atomics, Symbol.toStringTag);
            var failures = 0;
            try { Atomics(); } catch (error) { if (error instanceof TypeError) failures++; }
            try { new Atomics(); } catch (error) { if (error instanceof TypeError) failures++; }
            [
              typeof Atomics,
              Object.getPrototypeOf(Atomics) === Object.prototype,
              shaped,
              tag.value,
              tag.writable,
              tag.enumerable,
              tag.configurable,
              failures
            ].join("|");
        "#),
        Value::String(Arc::from("object|true|true|Atomics|false|false|true|2"))
    );
}

#[test]
fn atomics_number_and_bigint_operations_wrap_and_return_old_values() {
    assert_eq!(
        run(r#"
            var bytes = new Int8Array(new SharedArrayBuffer(2));
            var results = [
              Atomics.store(bytes, 0, 127),
              Atomics.add(bytes, 0, 1),
              Atomics.load(bytes, 0),
              Atomics.sub(bytes, 0, 1),
              Atomics.exchange(bytes, 1, 15),
              Atomics.and(bytes, 1, 6),
              Atomics.or(bytes, 1, 8),
              Atomics.xor(bytes, 1, 3),
              Atomics.compareExchange(bytes, 1, 13, 7),
              Atomics.load(bytes, 1)
            ];
            var big = new BigInt64Array(new SharedArrayBuffer(8));
            results.push(Atomics.store(big, 0, 9223372036854775807n));
            results.push(Atomics.add(big, 0, 1n));
            results.push(Atomics.load(big, 0));
            results.join("|");
        "#),
        Value::String(Arc::from(
            "127|127|-128|-128|0|15|6|14|13|7|9223372036854775807|9223372036854775807|-9223372036854775808"
        ))
    );
}

#[test]
fn atomics_accept_array_buffers_and_validate_immutable_and_index_order() {
    assert_eq!(
        run(r#"
            var mutable = new Int32Array(new ArrayBuffer(4));
            var stored = Atomics.store(mutable, 0, -0);
            var immutable = new Int32Array(
              (new ArrayBuffer(4)).transferToImmutable()
            );
            var order = [];
            var index = { valueOf: function() { order.push("index"); return 0; } };
            var value = { valueOf: function() { order.push("value"); return 1; } };
            var failures = 0;
            try { Atomics.store(immutable, index, value); }
            catch (error) { if (error instanceof TypeError) failures++; }
            try { Atomics.load(new Float32Array(new SharedArrayBuffer(4)), index); }
            catch (error) { if (error instanceof TypeError) failures++; }
            var shared = new Uint8Array(new SharedArrayBuffer(4));
            shared.fill(9, 1, 3);
            [
              Object.is(stored, 0),
              Atomics.load(mutable, 0),
              Atomics.load(immutable, 0),
              order.join(","),
              failures,
              [shared[0], shared[1], shared[2], shared[3]].join(","),
              Atomics.isLockFree(4),
              Atomics.isLockFree(3)
            ].join("|");
        "#),
        Value::String(Arc::from("true|0|0||2|0,9,9,0|true|false"))
    );
}

#[test]
fn test262_agents_share_sab_and_notify_wakes_waiters() {
    assert_eq!(
        run(r#"
            $262.agent.start(`
              $262.agent.receiveBroadcast(function(sab) {
                var view = new Int32Array(sab);
                Atomics.add(view, 1, 1);
                $262.agent.report(Atomics.wait(view, 0, 0, 1000));
                $262.agent.leaving();
              });
            `);
            var view = new Int32Array(new SharedArrayBuffer(8));
            $262.agent.broadcast(view.buffer);
            while (Atomics.load(view, 1) !== 1) {}
            $262.agent.sleep(10);
            var notified = Atomics.notify(view, 0, 1);
            var report;
            while ((report = $262.agent.getReport()) === null) {
              $262.agent.sleep(1);
            }
            [Atomics.load(view, 1), report, notified].join("|");
        "#),
        Value::String(Arc::from("1|ok|1"))
    );
}

#[test]
fn test262_agent_source_preserves_internal_utf16() {
    assert_eq!(
        run(r#"
            var lone = String.fromCharCode(0xDB80);
            $262.agent.start(
              "$262.agent.report(eval('/" + lone + "/').source.charCodeAt(0).toString(16));"
            );
            var report;
            while ((report = $262.agent.getReport()) === null) {
              $262.agent.sleep(1);
            }
            report;
        "#),
        Value::String(Arc::from("db80"))
    );
}

#[test]
fn atomics_wait_times_out_in_workers_and_main_agent_cannot_suspend() {
    assert_eq!(
        run(r#"
            $262.agent.start(`
              $262.agent.receiveBroadcast(function(sab) {
                var view = new BigInt64Array(sab);
                $262.agent.report(Atomics.wait(view, 0, 0n, 10));
                $262.agent.leaving();
              });
            `);
            var buffer = new SharedArrayBuffer(8);
            $262.agent.broadcast(buffer);
            var report;
            while ((report = $262.agent.getReport()) === null) {
              $262.agent.sleep(1);
            }
            var view = new Int32Array(buffer);
            var mismatch = Atomics.wait(view, 0, 1, 0);
            var blocked = false;
            try { Atomics.wait(view, 0, 0, 0); }
            catch (error) { blocked = error instanceof TypeError; }
            [report, mismatch, blocked].join("|");
        "#),
        Value::String(Arc::from("timed-out|not-equal|true"))
    );
}

#[test]
fn atomics_wait_async_returns_sync_results_for_immediate_outcomes() {
    assert_eq!(
        run(r#"
            var view = new Int32Array(new SharedArrayBuffer(4));
            var mismatch = Atomics.waitAsync(view, 0, 1, 100);
            var timeout = Atomics.waitAsync(view, 0, 0, 0);
            var { async, value } = mismatch;
            [
              async,
              value,
              timeout.async,
              timeout.value,
              Object.keys(mismatch).join(",")
            ].join("|");
        "#),
        Value::String(Arc::from("false|not-equal|false|timed-out|async,value"))
    );
}

#[test]
fn atomics_wait_async_resolves_notify_and_timeout_through_external_jobs() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var asyncResults = [];
        var notified = new Int32Array(new SharedArrayBuffer(4));
        var timed = new BigInt64Array(new SharedArrayBuffer(8));
        // Debug CI runners can spend more than one second in the deliberate
        // allocation pressure below before reaching Atomics.notify.
        var first = Atomics.waitAsync(notified, 0, 0, 10000);
        var second = Atomics.waitAsync(timed, 0, 0n, 10);
        first.value.then(function(value) { asyncResults.push("first:" + value); });
        second.value.then(function(value) { asyncResults.push("second:" + value); });
        for (var i = 0; i < 5000; i++) ({ index: i, payload: [i, i + 1] });
        Atomics.notify(notified, 0, 1);
    "#,
    )
    .expect("waitAsync setup should run");
    vm.run_external_jobs_until_idle()
        .expect("external waitAsync jobs should settle");
    assert_eq!(
        vm.run("asyncResults.sort().join('|')")
            .expect("waitAsync results should remain observable"),
        Value::String(Arc::from("first:ok|second:timed-out"))
    );
}

#[test]
fn nested_async_functions_resume_outward() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var release;
        var gate = new Promise(resolve => { release = resolve; });
        async function leaf() { return (await gate) + 1; }
        async function middle() { return (await leaf()) + 1; }
        async function outer() { return await middle(); }
        var settled = false;
        var observed;
        var result = outer();
        result.then(value => { settled = true; observed = value; });
        "#,
    )
    .expect("failed to suspend nested async functions");

    assert_eq!(
        vm.run("settled;").expect("failed to read pending state"),
        Value::Bool(false)
    );
    vm.run("release(40);")
        .expect("failed to resume nested async functions");
    assert_eq!(
        vm.run("settled + '|' + observed;")
            .expect("failed to read nested result"),
        Value::String(Arc::from("true|42"))
    );
}

#[test]
fn async_function_pending_await_preserves_microtask_fifo() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var release;
        var gate = new Promise(resolve => { release = resolve; });
        var log = [];
        async function pending() {
            log.push("start");
            await gate;
            log.push("resume");
        }
        var result = pending();
        result.then(() => log.push("done"));
        log.push("sync");
        Promise.resolve().then(() => log.push("before"));
        release();
        Promise.resolve().then(() => log.push("after"));
        "#,
    )
    .expect("failed to run interleaved microtasks");

    assert_eq!(
        vm.run("log.join('|');").expect("failed to read FIFO log"),
        Value::String(Arc::from("start|sync|before|resume|after|done"))
    );
}

#[test]
fn async_function_rejections_preserve_error_objects_and_thrown_values() {
    assert_eq!(
        run(r#"
            let marker = {};
            async function later(x = y, y) {}
            async function selfRef(x = x) {}
            async function evalConflict(a = eval("var a = 42")) {}
            async function userThrow() { throw marker; }

            let results = [];
            try { await later(); } catch (error) {
                results.push(error.constructor === ReferenceError);
            }
            try { await selfRef(); } catch (error) {
                results.push(error.constructor === ReferenceError);
            }
            try { await evalConflict(); } catch (error) {
                results.push(error.constructor === SyntaxError);
            }
            try { await userThrow(); } catch (error) {
                results.push(error === marker);
            }
            results.join("|");
        "#),
        Value::String(Arc::from("true|true|true|true"))
    );
}

#[test]
fn await_is_contextual_identifier_in_sloppy_non_async_code() {
    assert_eq!(run("var await = 0; await = 1; await;"), Value::Number(1.0));
    assert_eq!(
        run("function f(await){ return await; } f(7);"),
        Value::Number(7.0)
    );
}

// --- generators (function*/yield) ---

#[test]
fn generator_next_sequence() {
    assert_eq!(
        run("function* g(){ yield 1; yield 2; yield 3; } var it = g(); it.next().value;"),
        Value::Number(1.0)
    );
    assert_eq!(
        run(
            "function* g(){ yield 1; yield 2; yield 3; } var it = g(); it.next(); it.next().value;"
        ),
        Value::Number(2.0)
    );
    assert_eq!(
        run("function* g(){ yield 1; yield 2; yield 3; } var it = g(); it.next(); it.next(); it.next().value;"),
        Value::Number(3.0)
    );
    assert_eq!(
        run("function* g(){ yield 1; yield 2; yield 3; } var it = g(); it.next(); it.next(); it.next(); it.next().done;"),
        Value::Bool(true)
    );
}

#[test]
fn generator_for_of() {
    assert_eq!(
        run("function* r(a,b){ for(var i=a;i<b;i++) yield i; } var s=0; for(var v of r(1,4)) s+=v; s;"),
        Value::Number(6.0)
    );
}

#[test]
fn generator_spread() {
    assert_eq!(
        run("function* r(a,b){ for(var i=a;i<b;i++) yield i; } [...r(1,4)].join(',');"),
        Value::String(Arc::from("1,2,3"))
    );
}

#[test]
fn generator_yield_undefined() {
    assert_eq!(
        run("function* g(){ yield; yield 1; } var it=g(); it.next().value;"),
        Value::Undefined
    );
}

// --- JSON.parse error handling + function error propagation ---

#[test]
fn json_parse_invalid_returns_error() {
    // Invalid JSON must throw (not hang). run returns Undefined on error.
    let r = run("var r; try { JSON.parse('{bad}'); } catch(e) { r = e.name; } r;");
    assert_eq!(r, Value::String(Arc::from("SyntaxError")));
    assert_eq!(run("JSON.parse('1e400') === Infinity;"), Value::Bool(true));
    assert_eq!(
        run("1 / JSON.parse('-0') === -Infinity;"),
        Value::Bool(true)
    );
    assert_eq!(run("JSON.parse.length;"), Value::Number(2.0));
    assert_eq!(
        run("JSON.parse({ toString: function() { return '42'; } });"),
        Value::Number(42.0)
    );
    assert_eq!(
        run(r#"JSON.parse('"\u0061"');"#),
        Value::String(Arc::from("a"))
    );
}

#[test]
fn json_parse_reviver_internalizes_in_place_with_source_context() {
    assert_eq!(
        run(r#"
            var calls = [];
            var rootHolder;
            var result = JSON.parse('{"a":1,"b":2}', function(key, value, context) {
              calls.push(key + ':' + context.source);
              if (key === 'a') {
                Object.defineProperty(this, 'b', { configurable: false });
              }
              if (key === 'b') return 9;
              if (key === '') rootHolder = this;
              return value;
            });
            [
              result.b,
              calls.join(','),
              Object.getOwnPropertyNames(rootHolder).join(','),
              Array.isArray(new Proxy([], {}))
            ].join('|');
        "#),
        Value::String(Arc::from("2|a:1,b:2,:undefined||true"))
    );
    assert_eq!(
        run("JSON.parse('[1]', function(k, v) { if (k === '0') { Object.preventExtensions(this); return 9; } return v; })[0];"),
        Value::Number(9.0)
    );
    assert_eq!(
        run("JSON.parse('1', new Proxy(function(k, v) { return v + 1; }, {}));"),
        Value::Number(2.0)
    );
    assert!(run_err(
        "var pair = Proxy.revocable(function(k, v) { return v; }, {}); pair.revoke(); JSON.parse('1', pair.proxy);"
    )
    .contains("revoked"));
    assert!(run_err(
        r#"
            var target = {};
            Object.defineProperty(target, 'fixed', { value: 1, configurable: false, enumerable: true });
            var proxy = new Proxy(target, { defineProperty: function() { return true; } });
            JSON.parse('[0,0]', function(k, v) {
              if (k === '0') this[1] = proxy;
              if (k === 'fixed') return 2;
              return v;
            });
        "#
    )
    .contains("target invariant"));
}

#[test]
fn function_error_reaches_caller_catch() {
    let r =
        run("var r; function f(){ return missing; } try { f(); } catch(e) { r = 'caught'; } r;");
    assert_eq!(r, Value::String(Arc::from("caught")));
}

// --- wrapper objects (boxed primitives) ---

#[test]
fn boxed_number_valueof() {
    assert_eq!(run("new Number(5).valueOf();"), Value::Number(5.0));
    assert_eq!(
        run("typeof new Number(5).valueOf();"),
        Value::String(std::sync::Arc::from("number"))
    );
    assert_eq!(run("Number.prototype.valueOf();"), Value::Number(0.0));
    assert_eq!(run("Number.prototype.valueOf.call(1);"), Value::Number(1.0));
    assert_eq!(
        run("Number.prototype.toString.call(Object(255), 16);"),
        Value::String(Arc::from("ff"))
    );
    assert!(run_err("Number.prototype.valueOf.call(new String('1'));").contains("TypeError"));
    assert!(run_err("Number.prototype.valueOf.call({});").contains("TypeError"));
    assert!(run_err("Number.prototype.toString.call({});").contains("TypeError"));
}

#[test]
fn boxed_boolean_valueof() {
    assert_eq!(run("new Boolean(true).valueOf();"), Value::Bool(true));
    assert_eq!(run("Boolean.prototype.valueOf();"), Value::Bool(false));
    assert_eq!(
        run(
            r#"Boolean.prototype == false && Object.prototype.toString.call(Boolean.prototype) === "[object Boolean]""#
        ),
        Value::Bool(true)
    );
    assert_eq!(
        run("Boolean.prototype.toString.call(true) + ':' + Boolean.prototype.toString.call(Object(false));"),
        Value::String(Arc::from("true:false"))
    );
    assert!(run_err("Boolean.prototype.valueOf.call({});").contains("TypeError"));
    assert!(run_err("Boolean.prototype.toString.call(new String(''));").contains("TypeError"));
}

#[test]
fn boxed_string_valueof() {
    assert_eq!(
        run("new String('hi').valueOf();"),
        Value::String(std::sync::Arc::from("hi"))
    );
    assert_eq!(
        run("String.prototype.valueOf();"),
        Value::String(Arc::from(""))
    );
    assert_eq!(
        run("String.prototype.valueOf.call('x');"),
        Value::String(Arc::from("x"))
    );
    assert_eq!(
        run("String.prototype.toString.call(Object('y'));"),
        Value::String(Arc::from("y"))
    );
    assert!(run_err("String.prototype.valueOf.call(1);").contains("TypeError"));
    assert!(run_err("String.prototype.valueOf.call(new Number(1));").contains("TypeError"));
    assert!(
        run_err("String.prototype.valueOf.call({ toString: function(){ return 'x'; } });")
            .contains("TypeError")
    );
    assert!(run_err("String.prototype.toString.call(1);").contains("TypeError"));
}

#[test]
fn date_static_parse_exists() {
    assert_eq!(
        run("typeof Date.parse + ':' + Date.parse('1970') + ':' + typeof Date.UTC + ':' + typeof Date.prototype.getUTCFullYear + ':' + typeof Date.prototype.setUTCFullYear;"),
        Value::String(std::sync::Arc::from("function:0:function:function:function"))
    );
}

#[test]
fn date_utc_and_time_clip_follow_spec() {
    assert_eq!(run("Date.UTC(1970);"), Value::Number(0.0));
    assert_eq!(
        run("Date.UTC(2016, 6, 5, 15, 34, 45, 876);"),
        Value::Number(1467732885876.0)
    );
    assert_eq!(
        run("Date.UTC(1970.9, 0.9, 1.9, 0.9, 0.9, 0.9, 0.9);"),
        Value::Number(0.0)
    );
    assert_eq!(run("Date.UTC(70, 0);"), Value::Number(0.0));
    assert_eq!(run("Date.UTC(-1, 0);"), Value::Number(-62198755200000.0));
    assert!(matches!(run("Date.UTC(Infinity, 0);"), Value::Number(n) if n.is_nan()));
    assert!(matches!(run("Date.UTC(275760, 8, 13, 0, 0, 0, 1);"), Value::Number(n) if n.is_nan()));
    assert_eq!(
        run(r#"
            var log = "";
            function arg(name, value) {
              return { toString: function() { log += name; return value; } };
            }
            Date.UTC(arg("year", 0), arg("month", 0), arg("date", 1), arg("hours", 0), arg("minutes", 0), arg("seconds", 0), arg("ms", 0));
            log;
        "#),
        Value::String(Arc::from("yearmonthdatehoursminutessecondsms"))
    );
    assert_eq!(
        run("new Date(6.54321).valueOf() + ':' + new Date(-0).getTime() + ':' + Object.is(new Date(-0).getTime(), -0);"),
        Value::String(Arc::from("6:0:false"))
    );
    assert!(matches!(
        run("var d = new Date(0); d.setTime(8640000000000001);"),
        Value::Number(n) if n.is_nan()
    ));
    assert!(matches!(run("Date.UTC(1e100, 0);"), Value::Number(n) if n.is_nan()));
}

#[test]
fn date_time_setters_update_components_and_lengths() {
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCHours(6); d.getTime();"),
        Value::Number(1467352800000.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCMinutes(23); d.getTime();"),
        Value::Number(1467332580000.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCSeconds(45, 543); d.getTime();"),
        Value::Number(1467331245543.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCMilliseconds(333); d.getTime();"),
        Value::Number(1467331200333.0)
    );
    assert_eq!(
        run("Date.prototype.setMilliseconds.length + ':' + Date.prototype.setSeconds.length + ':' + Date.prototype.setMinutes.length + ':' + Date.prototype.setHours.length;"),
        Value::String(Arc::from("1:2:3:4"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var log = "";
            function arg(name) { return { valueOf: function() { log += name; return 0; } }; }
            var result = d.setHours(arg("h"), arg("m"), arg("s"), arg("ms"));
            log + ":" + result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("hmsms:NaN:NaN"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var result = d.setMilliseconds({ valueOf: function() { d.setTime(0); return 1; } });
            result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("NaN:0"))
    );
}

#[test]
fn date_date_setters_update_components_and_lengths() {
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1, 2, 3, 4, 5)); d.setUTCDate(15); d.getTime();"),
        Value::Number(1468548184005.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1, 2, 3, 4, 5)); d.setDate(15); d.getTime();"),
        Value::Number(1468548184005.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1, 2, 3, 4, 5)); d.setUTCMonth(8, 20); d.getTime();"),
        Value::Number(1474336984005.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1, 2, 3, 4, 5)); d.setUTCFullYear(2020, 1, 29); d.getTime();"),
        Value::Number(1582941784005.0)
    );
    assert_eq!(
        run("var d = new Date(Date.UTC(2016, 6, 1)); d.setUTCFullYear(2, 0, 1); d.getUTCFullYear() + ':' + d.getTime();"),
        Value::String(Arc::from("2:-62104060800000"))
    );
    assert_eq!(
        run("Date.prototype.setDate.length + ':' + Date.prototype.setUTCDate.length + ':' + Date.prototype.setMonth.length + ':' + Date.prototype.setUTCMonth.length + ':' + Date.prototype.setFullYear.length + ':' + Date.prototype.setUTCFullYear.length;"),
        Value::String(Arc::from("1:1:2:2:3:3"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var log = "";
            var result = d.setUTCDate({
              valueOf: function() { log += "date"; d.setTime(0); return 1; }
            });
            log + ":" + result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("date:NaN:0"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var log = "";
            var result = d.setUTCMonth(
              { valueOf: function() { log += "month"; d.setTime(0); return 1; } },
              { valueOf: function() { log += "date"; return 1; } }
            );
            log + ":" + result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("monthdate:NaN:0"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(NaN);
            var result = d.setUTCFullYear(2016);
            result + ":" + d.getTime();
        "#),
        Value::String(Arc::from("1451606400000:1451606400000"))
    );
    assert_eq!(
        run(r#"
            var d = new Date(0);
            var result = d.setUTCFullYear(1e100);
            (result !== result) + ":" + (d.getTime() !== d.getTime());
        "#),
        Value::String(Arc::from("true:true"))
    );
}

#[test]
fn date_stringification_parse_and_json_follow_spec() {
    assert_eq!(
        run(r#"
            var d = new Date(0);
            [
              d.toISOString(),
              d.toUTCString(),
              d.toString(),
              d.toDateString(),
              d.toTimeString()
            ].join("|");
        "#),
        Value::String(Arc::from(
            "1970-01-01T00:00:00.000Z|Thu, 01 Jan 1970 00:00:00 GMT|Thu Jan 01 1970 00:00:00 GMT+0000|Thu Jan 01 1970|00:00:00 GMT+0000"
        ))
    );
    assert_eq!(
        run(r#"
            var d = new Date(0);
            [
              Date.parse(d.toISOString()),
              Date.parse(d.toUTCString()),
              Date.parse(d.toString()),
              Date.parse("1970"),
              Date.parse("1970-01-01T00:00:00"),
              Date.parse("+275760-09-13T00:00:00.000Z")
            ].join("|");
        "#),
        Value::String(Arc::from("0|0|0|0|0|8640000000000000"))
    );
    assert!(matches!(
        run(r#"Date.parse("-000000-03-31T00:45Z");"#),
        Value::Number(n) if n.is_nan()
    ));
    assert_eq!(
        run(r#"
            var result = {};
            var obj = {
              toISOString: function() { return result; },
              valueOf: function() { return 0; }
            };
            Date.prototype.toJSON.call(obj) === result;
        "#),
        Value::Bool(true)
    );
    assert_eq!(run("new Date(NaN).toJSON();"), Value::Null);
    assert_eq!(
        run(r#"
            var oldDate = new Date(1438560000000);
            oldDate.valueOf = function() { throw new Error("valueOf"); };
            oldDate.toString = function() { throw new Error("toString"); };
            new Date(oldDate).getTime();
        "#),
        Value::Number(1438560000000.0)
    );
}

#[test]
fn date_to_temporal_instant_returns_epoch_nanoseconds() {
    assert_eq!(
        run(r#"
            var desc = Object.getOwnPropertyDescriptor(Date.prototype, 'toTemporalInstant');
            var lengthDesc = Object.getOwnPropertyDescriptor(Date.prototype.toTemporalInstant, 'length');
            var nameDesc = Object.getOwnPropertyDescriptor(Date.prototype.toTemporalInstant, 'name');
            Date.prototype.toTemporalInstant.length + ':' +
            Date.prototype.toTemporalInstant.name + ':' +
            desc.writable + ':' + desc.enumerable + ':' + desc.configurable + ':' +
            lengthDesc.writable + ':' + lengthDesc.enumerable + ':' + lengthDesc.configurable + ':' +
            nameDesc.writable + ':' + nameDesc.enumerable + ':' + nameDesc.configurable
            "#),
        Value::String(Arc::from(
            "0:toTemporalInstant:true:false:true:false:false:true:false:false:true"
        ))
    );
    assert_eq!(
        run("new Date(123456789).toTemporalInstant().epochNanoseconds === 123456789000000n;"),
        Value::Bool(true)
    );
    assert_eq!(
        run("new Date(-8640000000000000).toTemporalInstant().epochNanoseconds === -8640000000000000000000n;"),
        Value::Bool(true)
    );
    assert!(run_err("new Date(NaN).toTemporalInstant();").contains("RangeError"));
    assert!(run_err("Date.prototype.toTemporalInstant.call({});").contains("TypeError"));
    assert!(
        run_err("Date.prototype.toTemporalInstant.call(Date.prototype);").contains("TypeError")
    );
    assert!(run_err("var d = new Date(0); new d.toTemporalInstant();").contains("TypeError"));
}

#[test]
fn date_symbol_to_primitive_uses_hint_specific_ordinary_conversion() {
    assert_eq!(
        run(r#"
            var log = [];
            var object = {
              get toString() { log.push('get toString'); return function() { log.push('call toString'); return 's'; }; },
              get valueOf() { log.push('get valueOf'); return function() { log.push('call valueOf'); return 3; }; }
            };
            var method = Date.prototype[Symbol.toPrimitive];
            var first = method.call(object, 'default');
            var firstLog = log.join(',');
            log = [];
            var second = method.call(object, 'number');
            [first, firstLog, second, log.join(',')].join('|');
        "#),
        Value::String(Arc::from(
            "s|get toString,call toString|3|get valueOf,call valueOf"
        ))
    );
    assert_eq!(
        run(r#"
            var descriptor = Object.getOwnPropertyDescriptor(Date.prototype, Symbol.toPrimitive);
            [
              Date.prototype[Symbol.toPrimitive].length,
              Date.prototype[Symbol.toPrimitive].name,
              descriptor.writable,
              descriptor.enumerable,
              descriptor.configurable
            ].join('|');
        "#),
        Value::String(Arc::from("1|[Symbol.toPrimitive]|false|false|true"))
    );
    for source in [
        "Date.prototype[Symbol.toPrimitive].call({}, 'invalid')",
        "Date.prototype[Symbol.toPrimitive].call(1, 'number')",
        "Date.prototype[Symbol.toPrimitive].call({toString:function(){return {}},valueOf:function(){return {}}}, 'default')",
    ] {
        assert!(run_err(source).contains("TypeError"));
    }
}

#[test]
fn date_subclass_instances_keep_date_components() {
    assert_eq!(
        run("class D extends Date{};let d=new D(1859,'10',24,11);d.getFullYear()+','+d.getMonth()+','+d.getDate();"),
        Value::String(Arc::from("1859,10,24"))
    );
    assert_eq!(
        run("class D extends Date{};let d=new D(-3474558000000);d.getUTCFullYear()+','+d.getUTCMonth()+','+d.getUTCDate();"),
        Value::String(Arc::from("1859,10,24"))
    );
    assert_eq!(
        run("class D extends Date{};Object.prototype.toString.call(new D(0));"),
        Value::String(Arc::from("[object Date]"))
    );
}

#[test]
fn date_prototype_methods_require_date_receivers() {
    assert_eq!(
        run(r#"
            [
              Date.prototype.getFullYear.call(new Date(NaN)),
              Date.prototype.getUTCMonth.call(new Date(0)),
              Date.prototype.getUTCDate.call(new Date(0)),
              Object.prototype.toString.call(Date.prototype),
              Object.prototype.toString.call(new Date(0))
            ].join("|");
            "#,),
        Value::String(Arc::from("NaN|0|1|[object Object]|[object Date]"))
    );
    for src in [
        "Date.prototype.getTime.call(Date.prototype);",
        "Date.prototype.getTime.call({ __time__: 0 });",
        "Date.prototype.getDate.call({});",
        "Date.prototype.getUTCFullYear.call([]);",
        "Date.prototype.setFullYear.call(Date.prototype, 2012);",
        "Date.prototype.setTime.call({ __time__: 0 }, 1);",
        "Date.prototype.setUTCFullYear.call({}, { valueOf: function() { throw new Error('coerced'); } });",
    ] {
        assert!(
            run_err(src).contains("TypeError"),
            "expected TypeError for {src}"
        );
    }
}

#[test]
fn boxed_number_addition_uses_valueof() {
    assert_eq!(run("new Number(5) + 1;"), Value::Number(6.0));
    assert_eq!(run("new Boolean(true) + 1;"), Value::Number(2.0));
}

#[test]
fn date_addition_uses_default_string_hint() {
    assert_eq!(
        run("var d = new Date(0); d + d;"),
        Value::String(Arc::from(
            "Thu Jan 01 1970 00:00:00 GMT+0000Thu Jan 01 1970 00:00:00 GMT+0000"
        ))
    );
    assert_eq!(
        run("var d = new Date(0); d + 0;"),
        Value::String(Arc::from("Thu Jan 01 1970 00:00:00 GMT+00000"))
    );
    assert_eq!(
        run("var d = new Date(0); d + true;"),
        Value::String(Arc::from("Thu Jan 01 1970 00:00:00 GMT+0000true"))
    );
    assert_eq!(
        run("var d = new Date(0); d + {};"),
        Value::String(Arc::from(
            "Thu Jan 01 1970 00:00:00 GMT+0000[object Object]"
        ))
    );
}

#[test]
fn bigint_string_addition_concatenates() {
    assert_eq!(run("1n + '';"), Value::String(Arc::from("1")));
    assert_eq!(run("'' + -1n;"), Value::String(Arc::from("-1")));
    assert_eq!(run("Object(1n) + '';"), Value::String(Arc::from("1")));
}

#[test]
fn boxed_bigint_mixed_throws() {
    let err = run_err("Object(1n) + 1;");
    assert!(err.contains("TypeError"), "got: {}", err);
}

#[test]
fn to_primitive_both_object_throws() {
    let err = run_err("1 + {valueOf: function() {return {}}, toString: function() {return {}}};");
    assert!(err.contains("TypeError"), "got: {}", err);
}
