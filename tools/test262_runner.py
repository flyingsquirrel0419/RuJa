#!/usr/bin/env python3
"""Minimal test262 runner for RuJa.

Runs a subset of test262 language tests through the RuJa binary and reports
pass/fail counts. Uses the real test262 harness files (assert.js, sta.js, and
any per-test `includes:`) rather than a hand-rolled stub, so tests relying on
`verifyProperty`, `compareArray`, etc. are exercised correctly.
"""
import os, re, sys
from pathlib import Path

try:
    from test262_support import append_async_harness, execute_source
except ModuleNotFoundError:
    from tools.test262_support import append_async_harness, execute_source

RUJA = str(Path(__file__).resolve().parent.parent / "target/release/ruja")
TEST262 = os.environ.get("TEST262", "/root/test262")
HARNESS = Path(TEST262) / "harness"
RUN_ASYNC_TESTS = os.environ.get("TEST262_RUN_ASYNC") == "1"

SKIP_FEATURES = {
    "AggregateError", "ArrayBuffer", "Atomics", "Atomics.pause", "Atomics.waitAsync", "DataView",
    "Float16Array", "Float32Array", "Float64Array", "Int8Array", "Int16Array",
    "Int32Array", "Intl", "Promise", "SharedArrayBuffer",
    "Symbol", "Symbol.asyncIterator", "Symbol.iterator",
    "TypedArray", "Uint8Array", "Uint8Array-base64", "Uint8Array-hex",
    "Uint8ClampedArray", "Uint16Array", "Uint32Array", "WeakMap", "WeakRef",
    "WeakSet", "arraybuffer", "async-functions", "async-iteration", "atomics",
    "class-fields-private", "class-fields-private-in",
    "class-fields-public", "class-methods-private",
    "class-static-fields-private", "class-static-fields-public",
    "class-static-methods-private", "decorators",
    "default-parameters", "destructuring-binding",
    "dynamic-import", "error-cause", "explicit-resource-management",
    "export-star-as-namespace-from-module",
    "generators", "hashbang", "import-assertions",
    "import-attributes", "import-defer", "import.meta", "iterator-helpers",
    "json-modules", "module",
    "object-rest", "optional-chaining",
    "proxy-missing-checks", "Proxy", "Reflect",
    "Reflect.construct", "regexp-duplicate-named-groups",
    "regexp-named-groups", "regexp-unicode-property-escapes", "regexp-v-flag",
    "resizable-arraybuffer", "shadowrealm",
    "sharedarraybuffer", "source-phase-imports",
    "source-phase-imports-module-source", "tail-call-optimization",
    "top-level-await", "u180e",
}

EXPLICIT_RESOURCE_MANAGEMENT_SYMBOL_PREFIXES = (
    "built-ins/Symbol/asyncDispose/",
    "built-ins/Symbol/dispose/",
)

SYMBOL_FUNCTION_NAME_PREFIXES = (
    "language/expressions/object/fn-name-",
    "language/expressions/object/method-definition/fn-name-",
    "language/statements/class/definition/fn-name-",
)

OBJECT_SPREAD_SYMBOL_PREFIXES = (
    "language/expressions/array/spread-obj-",
    "language/expressions/call/spread-obj-",
    "language/expressions/new/spread-obj-",
)

FOR_OF_SYMBOL_ITERATOR_PREFIXES = (
    "language/statements/for-of/",
)

FOR_OF_STATEMENT_FEATURES = {
    "destructuring-binding",
    "object-rest",
    "optional-chaining",
    "Proxy",
    "Symbol.iterator",
}

FOR_OF_GENERATOR_CLOSE_FILES = {
    "language/statements/for-of/generator-close-via-break.js",
    "language/statements/for-of/generator-close-via-continue.js",
    "language/statements/for-of/generator-close-via-return.js",
    "language/statements/for-of/generator-close-via-throw.js",
}

TYPED_ARRAY_CONSTRUCTORS_PREFIXES = (
    "built-ins/TypedArrayConstructors/",
)

TYPED_ARRAY_CONSTRUCTORS_FEATURES = {
    "TypedArray",
    "ArrayBuffer",
    "DataView",
    "Float16Array",
    "Float32Array",
    "Float64Array",
    "generators",
    "Int8Array",
    "Int16Array",
    "Int32Array",
    "Reflect",
    "Reflect.construct",
    "Symbol",
    "Symbol.iterator",
    "Symbol.toPrimitive",
    "Symbol.toStringTag",
    "Uint8Array",
    "Uint8Array-base64",
    "Uint8Array-hex",
    "Uint8ClampedArray",
    "Uint16Array",
    "Uint32Array",
    "Proxy",
    "resizable-arraybuffer",
}

TYPED_ARRAY_RESIZABLE_PREFIXES = (
    "built-ins/TypedArray/prototype/byteLength/",
    "built-ins/TypedArray/prototype/byteOffset/",
    "built-ins/TypedArray/prototype/length/",
)

TYPED_ARRAY_RESIZABLE_FILES = {
    "built-ins/TypedArray/of/resized-with-out-of-bounds-and-in-bounds-indices.js",
    "built-ins/TypedArray/out-of-bounds-behaves-like-detached.js",
    "built-ins/TypedArray/out-of-bounds-get-and-set.js",
    "built-ins/TypedArray/out-of-bounds-has.js",
    "built-ins/TypedArray/prototype/resizable-and-fixed-have-same-prototype.js",
    "built-ins/TypedArray/resizable-buffer-length-tracking-1.js",
    "built-ins/TypedArray/resizable-buffer-length-tracking-2.js",
}

TYPED_ARRAY_RESIZABLE_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_AT_PREFIXES = (
    "built-ins/TypedArray/prototype/at/",
)

TYPED_ARRAY_AT_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "TypedArray",
    "TypedArray.prototype.at",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_FILL_PREFIXES = (
    "built-ins/TypedArray/prototype/fill/",
)

TYPED_ARRAY_FILL_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "immutable-arraybuffer",
    "resizable-arraybuffer",
}

TYPED_ARRAY_SUBARRAY_PREFIXES = (
    "built-ins/TypedArray/prototype/subarray/",
)

TYPED_ARRAY_SUBARRAY_FEATURES = {
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "Symbol.species",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_SET_PREFIXES = (
    "built-ins/TypedArray/prototype/set/",
)

TYPED_ARRAY_SET_FEATURES = {
    "BigInt",
    "Reflect.construct",
    "SharedArrayBuffer",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "immutable-arraybuffer",
    "resizable-arraybuffer",
}

TYPED_ARRAY_JOIN_PREFIXES = (
    "built-ins/TypedArray/prototype/join/",
)

TYPED_ARRAY_JOIN_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_VALUES_PREFIXES = (
    "built-ins/TypedArray/prototype/values/",
    "built-ins/TypedArray/prototype/Symbol.iterator/",
)

TYPED_ARRAY_VALUES_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "Symbol.iterator",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_KEYS_ENTRIES_PREFIXES = (
    "built-ins/TypedArray/prototype/keys/",
    "built-ins/TypedArray/prototype/entries/",
)

TYPED_ARRAY_KEYS_ENTRIES_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "Symbol.iterator",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_REVERSE_PREFIXES = (
    "built-ins/TypedArray/prototype/reverse/",
)

TYPED_ARRAY_REVERSE_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "immutable-arraybuffer",
    "resizable-arraybuffer",
}

TYPED_ARRAY_TO_REVERSED_PREFIXES = (
    "built-ins/TypedArray/prototype/toReversed/",
)

TYPED_ARRAY_TO_REVERSED_FEATURES = {
    "Reflect.construct",
    "Symbol.species",
    "TypedArray",
    "change-array-by-copy",
}

TYPED_ARRAY_COPY_WITHIN_PREFIXES = (
    "built-ins/TypedArray/prototype/copyWithin/",
)

TYPED_ARRAY_COPY_WITHIN_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "immutable-arraybuffer",
    "resizable-arraybuffer",
}

TYPED_ARRAY_COPY_WITHIN_EXTENDED_TIMEOUT_FILES = {
    "built-ins/TypedArray/prototype/copyWithin/coerced-values-end-detached-prototype.js",
    "built-ins/TypedArray/prototype/copyWithin/coerced-values-end-detached.js",
    "built-ins/TypedArray/prototype/copyWithin/coerced-values-start-detached.js",
}

TYPED_ARRAY_SLICE_PREFIXES = (
    "built-ins/TypedArray/prototype/slice/",
)

TYPED_ARRAY_SLICE_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "Symbol.species",
    "TypedArray",
    "align-detached-buffer-semantics-with-web-reality",
    "arrow-function",
    "immutable-arraybuffer",
    "resizable-arraybuffer",
}

TYPED_ARRAY_FIND_PREFIXES = (
    "built-ins/TypedArray/prototype/find/",
)

TYPED_ARRAY_FIND_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_FIND_INDEX_PREFIXES = (
    "built-ins/TypedArray/prototype/findIndex/",
)

TYPED_ARRAY_FIND_INDEX_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_FIND_LAST_PREFIXES = (
    "built-ins/TypedArray/prototype/findLast/",
)

TYPED_ARRAY_FIND_LAST_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_FIND_LAST_INDEX_PREFIXES = (
    "built-ins/TypedArray/prototype/findLastIndex/",
)

TYPED_ARRAY_FIND_LAST_INDEX_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_SOME_PREFIXES = (
    "built-ins/TypedArray/prototype/some/",
)

TYPED_ARRAY_SOME_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Reflect.set",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_EVERY_PREFIXES = (
    "built-ins/TypedArray/prototype/every/",
)

TYPED_ARRAY_EVERY_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Reflect.set",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_FOR_EACH_PREFIXES = (
    "built-ins/TypedArray/prototype/forEach/",
)

TYPED_ARRAY_FOR_EACH_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Reflect.set",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_INCLUDES_PREFIXES = (
    "built-ins/TypedArray/prototype/includes/",
)

TYPED_ARRAY_INCLUDES_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "align-detached-buffer-semantics-with-web-reality",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_INDEX_OF_PREFIXES = (
    "built-ins/TypedArray/prototype/indexOf/",
)

TYPED_ARRAY_INDEX_OF_FEATURES = {
    "ArrayBuffer",
    "Array.prototype.includes",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "align-detached-buffer-semantics-with-web-reality",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_LAST_INDEX_OF_PREFIXES = (
    "built-ins/TypedArray/prototype/lastIndexOf/",
)

TYPED_ARRAY_LAST_INDEX_OF_FEATURES = TYPED_ARRAY_INDEX_OF_FEATURES

TYPED_ARRAY_TO_LOCALE_STRING_PREFIXES = (
    "built-ins/TypedArray/prototype/toLocaleString/",
)

TYPED_ARRAY_TO_LOCALE_STRING_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_WITH_PREFIXES = (
    "built-ins/TypedArray/prototype/with/",
)

TYPED_ARRAY_WITH_FEATURES = {
    "BigInt",
    "Reflect.construct",
    "Symbol.species",
    "TypedArray",
    "change-array-by-copy",
    "resizable-arraybuffer",
}

TYPED_ARRAY_TO_STRING_TAG_PREFIXES = (
    "built-ins/TypedArray/prototype/Symbol.toStringTag/",
)

TYPED_ARRAY_TO_STRING_TAG_FEATURES = {
    "BigInt",
    "DataView",
    "Symbol",
    "Symbol.toStringTag",
    "TypedArray",
}

TYPED_ARRAY_REDUCE_RIGHT_PREFIXES = (
    "built-ins/TypedArray/prototype/reduceRight/",
)

TYPED_ARRAY_REDUCE_RIGHT_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Reflect.set",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_REDUCE_PREFIXES = (
    "built-ins/TypedArray/prototype/reduce/",
)

TYPED_ARRAY_REDUCE_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Reflect.set",
    "Symbol",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_MAP_PREFIXES = (
    "built-ins/TypedArray/prototype/map/",
)

TYPED_ARRAY_MAP_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Reflect.set",
    "Symbol",
    "Symbol.species",
    "TypedArray",
    "arrow-function",
    "immutable-arraybuffer",
    "resizable-arraybuffer",
}

TYPED_ARRAY_FILTER_PREFIXES = (
    "built-ins/TypedArray/prototype/filter/",
)

TYPED_ARRAY_FILTER_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "Reflect.construct",
    "Reflect.set",
    "Symbol",
    "Symbol.species",
    "TypedArray",
    "arrow-function",
    "immutable-arraybuffer",
    "resizable-arraybuffer",
}

TYPED_ARRAY_SORT_PREFIXES = (
    "built-ins/TypedArray/prototype/sort/",
)

TYPED_ARRAY_SORT_FEATURES = {
    "ArrayBuffer",
    "Array.prototype.includes",
    "Reflect.construct",
    "Symbol",
    "TypedArray",
    "immutable-arraybuffer",
    "stable-typedarray-sort",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_TO_SORTED_PREFIXES = (
    "built-ins/TypedArray/prototype/toSorted/",
)

TYPED_ARRAY_TO_SORTED_FEATURES = {
    "Reflect.construct",
    "Symbol.species",
    "TypedArray",
    "change-array-by-copy",
}

ARRAY_BUFFER_PREFIXES = (
    "built-ins/ArrayBuffer/",
)

ARRAY_BUFFER_FEATURES = {
    "ArrayBuffer",
    "Reflect.construct",
    "Symbol",
    "resizable-arraybuffer",
}

ARRAY_BUFFER_RESIZABLE_FEATURES = {
    "DataView",
    "Int8Array",
    "SharedArrayBuffer",
}

DATA_VIEW_PREFIXES = (
    "built-ins/DataView/",
)

DATA_VIEW_FEATURES = {
    "ArrayBuffer",
    "DataView",
    "Float16Array",
    "Int8Array",
    "Reflect",
    "Reflect.construct",
    "Symbol",
    "Symbol.toPrimitive",
    "Symbol.toStringTag",
    "Uint8Array",
    "resizable-arraybuffer",
}

SHARED_ARRAY_BUFFER_PREFIXES = (
    "built-ins/SharedArrayBuffer/",
)

SHARED_ARRAY_BUFFER_FEATURES = {
    "ArrayBuffer",
    "DataView",
    "Int8Array",
    "Reflect",
    "Reflect.construct",
    "SharedArrayBuffer",
    "Symbol",
    "Symbol.species",
    "Symbol.toStringTag",
    "TypedArray",
    "resizable-arraybuffer",
}

ATOMICS_SYNC_PREFIXES = tuple(
    f"built-ins/Atomics/{name}/"
    for name in (
        "add",
        "and",
        "compareExchange",
        "exchange",
        "isLockFree",
        "load",
        "notify",
        "or",
        "pause",
        "store",
        "sub",
        "wait",
        "waitAsync",
        "xor",
    )
)

ATOMICS_SYNC_FILES = {
    "built-ins/Atomics/Symbol.toStringTag.js",
    "built-ins/Atomics/prop-desc.js",
    "built-ins/Atomics/proto.js",
}

ATOMICS_SYNC_FEATURES = {
    "ArrayBuffer",
    "Atomics",
    "Atomics.waitAsync",
    "Atomics.pause",
    "BigInt",
    "DataView",
    "Float32Array",
    "Float64Array",
    "Int8Array",
    "Reflect.construct",
    "SharedArrayBuffer",
    "Symbol",
    "Symbol.toStringTag",
    "TypedArray",
    "Uint16Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "arrow-function",
    "async-functions",
    "destructuring-binding",
    "immutable-arraybuffer",
    "resizable-arraybuffer",
}

WEAK_REF_PREFIXES = (
    "built-ins/WeakRef/",
)

WEAK_REF_FEATURES = {
    "FinalizationRegistry",
    "Reflect",
    "Reflect.construct",
    "Symbol",
    "Symbol.toStringTag",
    "WeakMap",
    "WeakRef",
    "WeakSet",
}

FINALIZATION_REGISTRY_PREFIXES = (
    "built-ins/FinalizationRegistry/",
)

FINALIZATION_REGISTRY_FEATURES = {
    "FinalizationRegistry",
    "Reflect",
    "Reflect.construct",
    "Symbol",
    "Symbol.toStringTag",
    "WeakMap",
    "WeakRef",
    "WeakSet",
}

ERROR_STACK_PREFIXES = (
    "built-ins/Error/prototype/stack/",
)

ERROR_STACK_FEATURES = {
    "error-stack-accessor",
    "Proxy",
    "Reflect",
    "Reflect.construct",
}

ERROR_CAUSE_FILES = {
    "built-ins/AggregateError/cause-property.js",
    "built-ins/Error/cause_abrupt.js",
    "built-ins/Error/cause_property.js",
    "built-ins/Error/constructor.js",
    "built-ins/NativeErrors/cause_property_native_error.js",
}

ERROR_CAUSE_FEATURES = {
    "AggregateError",
    "error-cause",
}

AGGREGATE_ERROR_PREFIXES = (
    "built-ins/AggregateError/",
)

AGGREGATE_ERROR_FEATURES = {
    "AggregateError",
    "error-cause",
    "Reflect",
    "Reflect.construct",
    "Symbol",
    "Symbol.iterator",
}

ERROR_CONSTRUCTOR_REALM_FILES = {
    "built-ins/Error/proto-from-ctor-realm.js",
    "built-ins/NativeErrors/EvalError/proto-from-ctor-realm.js",
    "built-ins/NativeErrors/RangeError/proto-from-ctor-realm.js",
    "built-ins/NativeErrors/ReferenceError/proto-from-ctor-realm.js",
    "built-ins/NativeErrors/SyntaxError/proto-from-ctor-realm.js",
    "built-ins/NativeErrors/TypeError/proto-from-ctor-realm.js",
    "built-ins/NativeErrors/URIError/proto-from-ctor-realm.js",
}

ERROR_CONSTRUCTOR_REALM_FEATURES = {
    "Reflect",
    "Symbol",
}

WITH_STATEMENT_PREFIXES = (
    "language/statements/with/",
)

WITH_STATEMENT_FEATURES = {
    "async-functions",
    "async-iteration",
    "generators",
    "Proxy",
    "Reflect",
    "TypedArray",
}

ASSIGNMENT_EXPRESSION_PREFIXES = (
    "language/expressions/assignment/",
)

ASSIGNMENT_EXPRESSION_FEATURES = {
    "destructuring-binding",
    "generators",
    "object-rest",
    "optional-chaining",
    "Proxy",
    "Symbol",
    "Symbol.iterator",
}

REFERENCE_PRIVATE_EXPRESSION_PREFIXES = (
    "language/expressions/compound-assignment/",
    "language/expressions/logical-assignment/",
)

REFERENCE_PRIVATE_EXPRESSION_FEATURES = {
    "class-fields-private",
}

CLASS_ELEMENTS_PREFIXES = (
    "language/expressions/class/elements/",
    "language/statements/class/elements/",
)

CLASS_ELEMENTS_FEATURES = {
    "async-functions",
    "async-iteration",
    "class-fields-private",
    "class-fields-private-in",
    "class-fields-public",
    "class-methods-private",
    "class-static-fields-private",
    "class-static-fields-public",
    "class-static-methods-private",
    "destructuring-binding",
    "generators",
    "optional-chaining",
    "Proxy",
    "Symbol",
    "Symbol.asyncIterator",
    "Symbol.iterator",
}

OPTIONAL_CHAINING_PREFIXES = (
    "language/expressions/optional-chaining/",
)

OPTIONAL_CHAINING_FEATURES = {
    "optional-chaining",
}

CLASS_DEFINITION_PREFIXES = (
    "language/statements/class/definition/",
)

CLASS_DEFINITION_FEATURES = {
    "async-functions",
    "generators",
}

ARROW_FUNCTION_PREFIXES = (
    "language/expressions/arrow-function/",
)

ARROW_FUNCTION_FEATURES = {
    "default-parameters",
    "destructuring-binding",
    "generators",
    "object-rest",
    "Symbol.iterator",
}

ASYNC_ARROW_FUNCTION_PREFIXES = (
    "language/expressions/async-arrow-function/",
)

ASYNC_ARROW_FUNCTION_FEATURES = {
    "async-functions",
    "default-parameters",
}

ASYNC_FUNCTION_PREFIXES = (
    "language/expressions/async-function/",
    "language/statements/async-function/",
)

ASYNC_FUNCTION_FEATURES = {
    "async-functions",
    "default-parameters",
}

AWAIT_EXPRESSION_PREFIXES = (
    "language/expressions/await/",
)

AWAIT_EXPRESSION_FEATURES = {
    "async-functions",
    "async-iteration",
    "generators",
}

FOR_AWAIT_OF_PREFIXES = (
    "language/statements/for-await-of/",
)

FOR_AWAIT_OF_FEATURES = {
    "async-iteration",
    "Symbol.asyncIterator",
}

ASYNC_GENERATOR_PREFIXES = (
    "language/expressions/async-generator/",
    "language/statements/async-generator/",
)

ASYNC_GENERATOR_FEATURES = {
    "Reflect.construct",
    "Symbol",
    "Symbol.asyncIterator",
    "Symbol.iterator",
    "async-functions",
    "async-iteration",
    "default-parameters",
    "generators",
    "object-rest",
}

OBJECT_METHOD_DEFINITION_PREFIXES = (
    "language/expressions/object/method-definition/",
)

OBJECT_METHOD_DEFINITION_FEATURES = {
    "async-functions",
    "async-iteration",
    "class-fields-public",
    "class-methods-private",
    "default-parameters",
    "generators",
    "Symbol",
    "Symbol.asyncIterator",
    "Symbol.iterator",
}

YIELD_EXPRESSION_PREFIXES = (
    "language/expressions/yield/",
)

YIELD_EXPRESSION_FEATURES = {
    "generators",
    "Symbol.iterator",
}

GENERATOR_PREFIXES = (
    "language/expressions/generators/",
    "language/statements/generators/",
)

GENERATOR_FEATURES = {
    "default-parameters",
    "destructuring-binding",
    "generators",
    "object-rest",
    "Symbol",
    "Symbol.iterator",
}

FUNCTION_PREFIXES = (
    "language/expressions/function/",
    "language/statements/function/",
)

FUNCTION_FEATURES = {
    "class-fields-private",
    "default-parameters",
    "destructuring-binding",
    "generators",
    "object-rest",
    "Symbol.iterator",
}

CLASS_SUBCLASS_PREFIXES = (
    "language/statements/class/subclass/",
)

CLASS_SUBCLASS_FEATURES = {
    "async-functions",
    "async-iteration",
    "generators",
    "Proxy",
    "Symbol",
    "Symbol.iterator",
    "TypedArray",
    "WeakMap",
    "WeakSet",
}

CLASS_SUBCLASS_BUILTINS_PREFIXES = (
    "language/expressions/class/subclass-builtins/",
    "language/statements/class/subclass-builtins/",
)

CLASS_SUBCLASS_BUILTINS_FEATURES = {
    "AggregateError",
    "ArrayBuffer",
    "DataView",
    "Float32Array",
    "Float64Array",
    "Int8Array",
    "Int16Array",
    "Int32Array",
    "Promise",
    "TypedArray",
    "Uint8Array",
    "Uint8ClampedArray",
    "Uint16Array",
    "Uint32Array",
    "WeakMap",
    "WeakSet",
}

def parse_meta(src):
    """Parse the /*--- ... ---*/ metadata block, handling multi-line lists."""
    m = re.search(r'/\*---\n(.*?)\n---\*/', src, re.DOTALL)
    if not m:
        return {}
    meta = {}
    block = m.group(1)
    # YAML-ish: we capture flags/features/includes as bracketed or bare lists.
    for key in ('flags', 'features', 'includes'):
        # match `key: [a, b]` or `key: [a]`
        m2 = re.search(rf'^{key}:\s*\[(.*?)\]', block, re.MULTILINE | re.DOTALL)
        if m2:
            meta[key] = [x.strip() for x in m2.group(1).split(',') if x.strip()]
    # negative: { phase: <parse|runtime|resolution>, type: <ErrorName> }
    mn = re.search(
        r'^negative:\s*\n(\s+phase:\s*(\w+)\n\s+type:\s*(\w+)|\s+type:\s*(\w+)\n\s+phase:\s*(\w+))',
        block,
        re.MULTILINE,
    )
    if mn:
        phase = mn.group(2) or mn.group(5)
        typ = mn.group(3) or mn.group(4)
        meta['negative'] = {'phase': phase, 'type': typ}
    return meta

def explicit_resource_management_symbols_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(EXPLICIT_RESOURCE_MANAGEMENT_SYMBOL_PREFIXES)

def symbol_function_name_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(SYMBOL_FUNCTION_NAME_PREFIXES)

def object_spread_symbol_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(OBJECT_SPREAD_SYMBOL_PREFIXES)

def for_of_symbol_iterator_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(FOR_OF_SYMBOL_ITERATOR_PREFIXES)

def for_of_generator_close_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in FOR_OF_GENERATOR_CLOSE_FILES

def typed_array_constructors_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(TYPED_ARRAY_CONSTRUCTORS_PREFIXES)

def typed_array_resizable_path(path, meta):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return (
        (rel_text.startswith(TYPED_ARRAY_RESIZABLE_PREFIXES)
         or rel_text in TYPED_ARRAY_RESIZABLE_FILES)
        and "resizable-arraybuffer" in meta.get("features", [])
    )

def typed_array_at_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_AT_PREFIXES)

def typed_array_fill_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_FILL_PREFIXES)

def typed_array_subarray_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_SUBARRAY_PREFIXES)

def typed_array_set_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_SET_PREFIXES)

def typed_array_join_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_JOIN_PREFIXES)

def typed_array_values_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_VALUES_PREFIXES)

def typed_array_keys_entries_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_KEYS_ENTRIES_PREFIXES)

def typed_array_reverse_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_REVERSE_PREFIXES)

def typed_array_to_reversed_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_TO_REVERSED_PREFIXES)

def typed_array_copy_within_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_COPY_WITHIN_PREFIXES)

def typed_array_copy_within_extended_timeout_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in TYPED_ARRAY_COPY_WITHIN_EXTENDED_TIMEOUT_FILES

def typed_array_slice_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_SLICE_PREFIXES)

def typed_array_find_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_FIND_PREFIXES)

def typed_array_find_index_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_FIND_INDEX_PREFIXES)

def typed_array_find_last_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_FIND_LAST_PREFIXES)

def typed_array_find_last_index_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_FIND_LAST_INDEX_PREFIXES)

def typed_array_some_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_SOME_PREFIXES)

def typed_array_every_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_EVERY_PREFIXES)

def typed_array_for_each_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_FOR_EACH_PREFIXES)

def typed_array_includes_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_INCLUDES_PREFIXES)

def typed_array_index_of_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_INDEX_OF_PREFIXES)

def typed_array_last_index_of_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_LAST_INDEX_OF_PREFIXES)

def typed_array_to_locale_string_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_TO_LOCALE_STRING_PREFIXES)

def typed_array_with_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_WITH_PREFIXES)

def typed_array_to_string_tag_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_TO_STRING_TAG_PREFIXES)

def typed_array_reduce_right_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_REDUCE_RIGHT_PREFIXES)

def typed_array_reduce_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_REDUCE_PREFIXES)

def typed_array_map_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_MAP_PREFIXES)

def typed_array_filter_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_FILTER_PREFIXES)

def typed_array_sort_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_SORT_PREFIXES)

def typed_array_to_sorted_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_TO_SORTED_PREFIXES)

def array_buffer_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(ARRAY_BUFFER_PREFIXES)

def data_view_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(DATA_VIEW_PREFIXES)

def shared_array_buffer_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(SHARED_ARRAY_BUFFER_PREFIXES)

def atomics_sync_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(ATOMICS_SYNC_PREFIXES) or rel_text in ATOMICS_SYNC_FILES

def weak_ref_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(WEAK_REF_PREFIXES)

def finalization_registry_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(FINALIZATION_REGISTRY_PREFIXES)

def error_stack_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(ERROR_STACK_PREFIXES)

def error_cause_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in ERROR_CAUSE_FILES

def aggregate_error_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(AGGREGATE_ERROR_PREFIXES)

def error_constructor_realm_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in ERROR_CONSTRUCTOR_REALM_FILES

def with_statement_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(WITH_STATEMENT_PREFIXES)

def assignment_expression_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(ASSIGNMENT_EXPRESSION_PREFIXES)

def reference_private_expression_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(REFERENCE_PRIVATE_EXPRESSION_PREFIXES)

def class_elements_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(CLASS_ELEMENTS_PREFIXES)

def optional_chaining_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(OPTIONAL_CHAINING_PREFIXES)

def class_definition_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(CLASS_DEFINITION_PREFIXES)

def arrow_function_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(ARROW_FUNCTION_PREFIXES)

def async_arrow_function_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(ASYNC_ARROW_FUNCTION_PREFIXES)

def async_function_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(ASYNC_FUNCTION_PREFIXES)

def await_expression_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(AWAIT_EXPRESSION_PREFIXES)

def for_await_of_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(FOR_AWAIT_OF_PREFIXES)

def async_generator_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(ASYNC_GENERATOR_PREFIXES)

def object_method_definition_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(OBJECT_METHOD_DEFINITION_PREFIXES)

def yield_expression_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(YIELD_EXPRESSION_PREFIXES)

def generator_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(GENERATOR_PREFIXES)

def function_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(FUNCTION_PREFIXES)

def class_subclass_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(CLASS_SUBCLASS_PREFIXES)

def class_subclass_builtins_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(CLASS_SUBCLASS_BUILTINS_PREFIXES)

def should_skip(meta, path=None):
    feats = set(meta.get('features', []))
    if path is not None and explicit_resource_management_symbols_path(path):
        feats.discard("explicit-resource-management")
    if path is not None and symbol_function_name_path(path):
        feats.discard("Symbol")
    if path is not None and object_spread_symbol_path(path):
        feats.discard("Symbol")
    if path is not None and for_of_symbol_iterator_path(path):
        feats.difference_update(FOR_OF_STATEMENT_FEATURES)
    if path is not None and for_of_generator_close_path(path):
        feats.discard("generators")
    if path is not None and typed_array_constructors_path(path):
        feats.difference_update(TYPED_ARRAY_CONSTRUCTORS_FEATURES)
    if path is not None and typed_array_resizable_path(path, meta):
        feats.difference_update(TYPED_ARRAY_RESIZABLE_FEATURES)
    if path is not None and typed_array_at_path(path):
        feats.difference_update(TYPED_ARRAY_AT_FEATURES)
    if path is not None and typed_array_fill_path(path):
        feats.difference_update(TYPED_ARRAY_FILL_FEATURES)
    if path is not None and typed_array_subarray_path(path):
        feats.difference_update(TYPED_ARRAY_SUBARRAY_FEATURES)
    if path is not None and typed_array_set_path(path):
        feats.difference_update(TYPED_ARRAY_SET_FEATURES)
    if path is not None and typed_array_join_path(path):
        feats.difference_update(TYPED_ARRAY_JOIN_FEATURES)
    if path is not None and typed_array_values_path(path):
        feats.difference_update(TYPED_ARRAY_VALUES_FEATURES)
    if path is not None and typed_array_keys_entries_path(path):
        feats.difference_update(TYPED_ARRAY_KEYS_ENTRIES_FEATURES)
    if path is not None and typed_array_reverse_path(path):
        feats.difference_update(TYPED_ARRAY_REVERSE_FEATURES)
    if path is not None and typed_array_to_reversed_path(path):
        feats.difference_update(TYPED_ARRAY_TO_REVERSED_FEATURES)
    if path is not None and typed_array_copy_within_path(path):
        feats.difference_update(TYPED_ARRAY_COPY_WITHIN_FEATURES)
    if path is not None and typed_array_slice_path(path):
        feats.difference_update(TYPED_ARRAY_SLICE_FEATURES)
    if path is not None and typed_array_find_path(path):
        feats.difference_update(TYPED_ARRAY_FIND_FEATURES)
    if path is not None and typed_array_find_index_path(path):
        feats.difference_update(TYPED_ARRAY_FIND_INDEX_FEATURES)
    if path is not None and typed_array_find_last_path(path):
        feats.difference_update(TYPED_ARRAY_FIND_LAST_FEATURES)
    if path is not None and typed_array_find_last_index_path(path):
        feats.difference_update(TYPED_ARRAY_FIND_LAST_INDEX_FEATURES)
    if path is not None and typed_array_some_path(path):
        feats.difference_update(TYPED_ARRAY_SOME_FEATURES)
    if path is not None and typed_array_every_path(path):
        feats.difference_update(TYPED_ARRAY_EVERY_FEATURES)
    if path is not None and typed_array_for_each_path(path):
        feats.difference_update(TYPED_ARRAY_FOR_EACH_FEATURES)
    if path is not None and typed_array_includes_path(path):
        feats.difference_update(TYPED_ARRAY_INCLUDES_FEATURES)
    if path is not None and typed_array_index_of_path(path):
        feats.difference_update(TYPED_ARRAY_INDEX_OF_FEATURES)
    if path is not None and typed_array_last_index_of_path(path):
        feats.difference_update(TYPED_ARRAY_LAST_INDEX_OF_FEATURES)
    if path is not None and typed_array_to_locale_string_path(path):
        feats.difference_update(TYPED_ARRAY_TO_LOCALE_STRING_FEATURES)
    if path is not None and typed_array_with_path(path):
        feats.difference_update(TYPED_ARRAY_WITH_FEATURES)
    if path is not None and typed_array_to_string_tag_path(path):
        feats.difference_update(TYPED_ARRAY_TO_STRING_TAG_FEATURES)
    if path is not None and typed_array_reduce_right_path(path):
        feats.difference_update(TYPED_ARRAY_REDUCE_RIGHT_FEATURES)
    if path is not None and typed_array_reduce_path(path):
        feats.difference_update(TYPED_ARRAY_REDUCE_FEATURES)
    if path is not None and typed_array_map_path(path):
        feats.difference_update(TYPED_ARRAY_MAP_FEATURES)
    if path is not None and typed_array_filter_path(path):
        feats.difference_update(TYPED_ARRAY_FILTER_FEATURES)
    if path is not None and typed_array_sort_path(path):
        feats.difference_update(TYPED_ARRAY_SORT_FEATURES)
    if path is not None and typed_array_to_sorted_path(path):
        feats.difference_update(TYPED_ARRAY_TO_SORTED_FEATURES)
    if path is not None and array_buffer_path(path):
        feats.difference_update(ARRAY_BUFFER_FEATURES)
        if "resizable-arraybuffer" in meta.get("features", []):
            feats.difference_update(ARRAY_BUFFER_RESIZABLE_FEATURES)
    if path is not None and data_view_path(path):
        feats.difference_update(DATA_VIEW_FEATURES)
    if path is not None and shared_array_buffer_path(path):
        feats.difference_update(SHARED_ARRAY_BUFFER_FEATURES)
    if path is not None and atomics_sync_path(path):
        feats.difference_update(ATOMICS_SYNC_FEATURES)
    if path is not None and weak_ref_path(path):
        feats.difference_update(WEAK_REF_FEATURES)
    if path is not None and finalization_registry_path(path):
        feats.difference_update(FINALIZATION_REGISTRY_FEATURES)
    if path is not None and error_stack_path(path):
        feats.difference_update(ERROR_STACK_FEATURES)
    if path is not None and aggregate_error_path(path):
        feats.difference_update(AGGREGATE_ERROR_FEATURES)
    if path is not None and error_constructor_realm_path(path):
        feats.difference_update(ERROR_CONSTRUCTOR_REALM_FEATURES)
    if path is not None and error_cause_path(path):
        feats.difference_update(ERROR_CAUSE_FEATURES)
    if path is not None and with_statement_path(path):
        feats.difference_update(WITH_STATEMENT_FEATURES)
    if path is not None and assignment_expression_path(path):
        feats.difference_update(ASSIGNMENT_EXPRESSION_FEATURES)
    if path is not None and reference_private_expression_path(path):
        feats.difference_update(REFERENCE_PRIVATE_EXPRESSION_FEATURES)
    if path is not None and class_elements_path(path):
        feats.difference_update(CLASS_ELEMENTS_FEATURES)
    if path is not None and optional_chaining_path(path):
        feats.difference_update(OPTIONAL_CHAINING_FEATURES)
    if path is not None and class_definition_path(path):
        feats.difference_update(CLASS_DEFINITION_FEATURES)
    if path is not None and arrow_function_path(path):
        feats.difference_update(ARROW_FUNCTION_FEATURES)
    if path is not None and async_arrow_function_path(path):
        feats.difference_update(ASYNC_ARROW_FUNCTION_FEATURES)
    if path is not None and async_function_path(path):
        feats.difference_update(ASYNC_FUNCTION_FEATURES)
    if path is not None and await_expression_path(path):
        feats.difference_update(AWAIT_EXPRESSION_FEATURES)
    if path is not None and for_await_of_path(path):
        feats.difference_update(FOR_AWAIT_OF_FEATURES)
    if path is not None and async_generator_path(path):
        feats.difference_update(ASYNC_GENERATOR_FEATURES)
    if path is not None and object_method_definition_path(path):
        feats.difference_update(OBJECT_METHOD_DEFINITION_FEATURES)
    if path is not None and yield_expression_path(path):
        feats.difference_update(YIELD_EXPRESSION_FEATURES)
    if path is not None and generator_path(path):
        feats.difference_update(GENERATOR_FEATURES)
    if path is not None and function_path(path):
        feats.difference_update(FUNCTION_FEATURES)
    if path is not None and class_subclass_path(path):
        feats.difference_update(CLASS_SUBCLASS_FEATURES)
    if path is not None and class_subclass_builtins_path(path):
        feats.difference_update(CLASS_SUBCLASS_BUILTINS_FEATURES)
    if feats & SKIP_FEATURES:
        return True
    flags = meta.get('flags', [])
    async_admitted = path is not None and (
        class_elements_path(path)
        or optional_chaining_path(path)
        or class_definition_path(path)
        or object_method_definition_path(path)
        or async_arrow_function_path(path)
        or async_function_path(path)
        or await_expression_path(path)
        or for_await_of_path(path)
        or async_generator_path(path)
        or atomics_sync_path(path)
    )
    if 'module' in flags or (
        'async' in flags and not (RUN_ASYNC_TESTS or async_admitted)
    ):
        return True
    return False

# Harness files always loaded (the minimum test262 requires).
BASE_HARNESS = ['sta.js', 'assert.js']

def build_source(path):
    """Build the full source: harness files + the test."""
    src = Path(path).read_text()
    meta = parse_meta(src)
    flags = meta.get('flags', [])
    if 'raw' in flags:
        return src, meta
    parts = []
    # onlyStrict: prepend 'use strict' at the very start so the parser
    # recognizes it as a directive prologue (before any harness code).
    if 'onlyStrict' in flags:
        parts.append("'use strict';")
    # Base harness (sta.js defines Test262Error; assert.js needs it).
    for inc in BASE_HARNESS:
        p = HARNESS / inc
        if p.exists():
            parts.append(p.read_text())
    append_async_harness(parts, HARNESS, flags)
    # Per-test includes (propertyHelper.js, compareArray.js, etc.).
    for inc in meta.get('includes', []):
        p = HARNESS / inc
        if p.exists():
            parts.append(p.read_text())
    parts.append(src)
    return "\n".join(parts), meta

def run_test(path):
    full, meta = build_source(path)
    if should_skip(meta, path):
        return 'skip'
    timeout = 600 if typed_array_copy_within_extended_timeout_path(path) else 8
    status, _ = execute_source(full, meta, RUJA, timeout=timeout)
    return status

def main():
    dirs = sys.argv[1:] if len(sys.argv) > 1 else ['language/expressions']
    counts = {'pass': 0, 'fail': 0, 'skip': 0, 'timeout': 0, 'error': 0}
    total = 0
    for d in dirs:
        base = Path(TEST262) / 'test' / d
        if not base.exists():
            continue
        for f in sorted(base.rglob('*.js')):
            if '_FIXTURE' in f.name:
                continue
            total += 1
            if total % 100 == 0:
                sys.stderr.write(f"  ...{total} tests, {counts['pass']} pass, {counts['fail']} fail\n")
            counts[run_test(f)] += 1
    ran = counts['pass'] + counts['fail']
    print(f"\nResults over {total} tests (ran {ran}):")
    for k in ['pass', 'fail', 'skip', 'timeout', 'error']:
        print(f"  {k}: {counts[k]}")
    if ran > 0:
        rate = 100 * counts['pass'] / ran
        print(f"  pass rate (of run): {rate:.1f}%")
        print(f"RATE={rate:.1f} PASS={counts['pass']} FAIL={counts['fail']} "
              f"SKIP={counts['skip']} TOTAL={total} RAN={ran}")
    else:
        print("RATE=0.0 PASS=0 FAIL=0 SKIP=0 TOTAL=0 RAN=0")

if __name__ == '__main__':
    main()
