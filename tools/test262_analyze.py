#!/usr/bin/env python3
"""Analyze test262 failures: collect failing test paths + RuJa stderr,
then bucket by error message pattern to find high-frequency real bugs."""
import os, re, sys, json
from pathlib import Path
from collections import Counter, defaultdict

try:
    from test262_class_computed_field_admission import CLASS_COMPUTED_FIELD_FILES
    from test262_class_default_parameter_admission import CLASS_DEFAULT_PARAMETER_FILES
    from test262_class_destructuring_admission import CLASS_DESTRUCTURING_FILES
    from test262_class_private_admission import CLASS_PRIVATE_FEATURES_BY_FILE
    from test262_class_public_field_admission import CLASS_PUBLIC_FIELD_FILES
    from test262_date_to_primitive_admission import DATE_TO_PRIMITIVE_FILES
    from test262_proxy_get_admission import PROXY_GET_FEATURES, PROXY_GET_FILES
    from test262_reference_primitive_admission import REFERENCE_PRIMITIVE_FILES
    from test262_support import append_async_harness, execute_source
    from test262_dynamic_import_admission import DYNAMIC_IMPORT_FILES
    from test262_import_meta_admission import IMPORT_META_FILES
    from test262_json_parse_admission import JSON_PARSE_FILES
    from test262_json_raw_admission import JSON_RAW_FILES
    from test262_json_stringify_admission import JSON_STRINGIFY_FILES
    from test262_module_admission import (
        MODULE_STATIC_SEMANTICS_FILES, MODULE_TLA_RUNTIME_FILES, MODULE_TLA_SYNTAX_FILES,
    )
except ModuleNotFoundError:
    from tools.test262_class_computed_field_admission import CLASS_COMPUTED_FIELD_FILES
    from tools.test262_class_default_parameter_admission import CLASS_DEFAULT_PARAMETER_FILES
    from tools.test262_class_destructuring_admission import CLASS_DESTRUCTURING_FILES
    from tools.test262_class_private_admission import CLASS_PRIVATE_FEATURES_BY_FILE
    from tools.test262_class_public_field_admission import CLASS_PUBLIC_FIELD_FILES
    from tools.test262_date_to_primitive_admission import DATE_TO_PRIMITIVE_FILES
    from tools.test262_proxy_get_admission import PROXY_GET_FEATURES, PROXY_GET_FILES
    from tools.test262_reference_primitive_admission import REFERENCE_PRIMITIVE_FILES
    from tools.test262_support import append_async_harness, execute_source
    from tools.test262_dynamic_import_admission import DYNAMIC_IMPORT_FILES
    from tools.test262_import_meta_admission import IMPORT_META_FILES
    from tools.test262_json_parse_admission import JSON_PARSE_FILES
    from tools.test262_json_raw_admission import JSON_RAW_FILES
    from tools.test262_json_stringify_admission import JSON_STRINGIFY_FILES
    from tools.test262_module_admission import (
        MODULE_STATIC_SEMANTICS_FILES, MODULE_TLA_RUNTIME_FILES, MODULE_TLA_SYNTAX_FILES,
    )

RUJA = str(Path(__file__).resolve().parent.parent / "target/release/ruja")
TEST262 = os.environ.get("TEST262", "/root/test262")
HARNESS = Path(TEST262) / "harness"
RUN_ASYNC_TESTS = os.environ.get("TEST262_RUN_ASYNC") == "1"

MODULE_CORE_FILES = {
    f"language/module-code/{name}"
    for name in (
        "comment-multi-line-html-close.js", "comment-single-line-html-close.js",
        "comment-single-line-html-open.js", "early-dup-lables.js", "early-dup-lex.js",
        "early-dup-top-function-async-generator.js", "early-dup-top-function-async.js",
        "early-dup-top-function-generator.js", "early-dup-top-function.js",
        "early-lex-and-var.js", "early-new-target.js", "early-strict-mode.js",
        "early-super.js", "early-undef-break.js", "early-undef-continue.js",
        "eval-gtbndng-local-bndng-cls.js", "eval-gtbndng-local-bndng-const.js",
        "eval-gtbndng-local-bndng-let.js", "eval-gtbndng-local-bndng-var.js",
        "eval-self-abrupt.js", "eval-this.js", "instn-local-bndng-cls.js",
        "instn-local-bndng-const.js", "instn-local-bndng-for-dup.js",
        "instn-local-bndng-for.js", "instn-local-bndng-fun.js",
        "instn-local-bndng-gen.js", "instn-local-bndng-let.js",
        "instn-local-bndng-var-dup.js", "instn-local-bndng-var.js",
        "parse-err-hoist-lex-fun.js", "parse-err-return.js",
        "parse-err-syntax-1.js", "parse-err-syntax-2.js", "parse-err-yield.js",
        "instn-local-bndng-export-var.js", "instn-local-bndng-export-let.js",
        "instn-local-bndng-export-const.js", "instn-local-bndng-export-fun.js",
        "instn-local-bndng-export-gen.js", "instn-local-bndng-export-cls.js",
        "eval-gtbndng-indirect-update.js", "eval-gtbndng-indirect-update-as.js",
        "eval-gtbndng-indirect-trlng-comma.js", "instn-same-global.js",
        "eval-rqstd-abrupt.js",
        "instn-named-bndng-var.js", "instn-named-bndng-let.js",
        "instn-named-bndng-const.js", "instn-named-bndng-fun.js",
        "instn-named-bndng-gen.js", "instn-named-bndng-cls.js",
        "instn-named-bndng-trlng-comma.js", "instn-iee-bndng-var.js",
        "instn-iee-bndng-let.js", "instn-iee-bndng-const.js",
        "instn-iee-bndng-fun.js", "instn-iee-bndng-gen.js",
        "instn-iee-bndng-cls.js", "instn-iee-trlng-comma.js",
        "instn-named-id-name.js", "instn-iee-iee-cycle.js",
        "instn-named-iee-cycle.js", "instn-iee-err-circular.js",
        "instn-iee-err-circular-as.js", "instn-named-star-cycle.js",
        "instn-star-iee-single-cycle-same-name.js",
        "instn-star-iee-multi-cycle-same-name.js",
        "early-dup-export-dflt-id.js", "early-dup-export-dflt.js",
        "eval-export-dflt-cls-anon-semi.js", "eval-export-dflt-cls-anon.js",
        "eval-export-dflt-cls-name-meth.js", "eval-export-dflt-cls-named-semi.js",
        "eval-export-dflt-cls-named.js", "eval-export-dflt-expr-cls-anon.js",
        "eval-export-dflt-expr-cls-name-meth.js", "eval-export-dflt-expr-cls-named.js",
        "eval-export-dflt-expr-err-eval.js", "eval-export-dflt-expr-err-get-value.js",
        "eval-export-dflt-expr-fn-anon.js", "eval-export-dflt-expr-fn-named.js",
        "eval-export-dflt-expr-gen-anon.js", "eval-export-dflt-expr-gen-named.js",
        "eval-export-dflt-expr-in.js", "eval-export-dflt-fun-anon-semi.js",
        "eval-export-dflt-fun-named-semi.js", "eval-export-dflt-gen-anon-semi.js",
        "eval-export-dflt-gen-named-semi.js", "eval-gtbndng-indirect-update-dflt.js",
        "export-default-asyncfunction-declaration-binding-exists.js",
        "export-default-asyncfunction-declaration-binding.js",
        "export-default-asyncgenerator-declaration-binding-exists.js",
        "export-default-asyncgenerator-declaration-binding.js",
        "export-default-function-declaration-binding-exists.js",
        "export-default-function-declaration-binding.js",
        "export-default-generator-declaration-binding-exists.js",
        "export-default-generator-declaration-binding.js",
        "instn-iee-err-dflt-thru-star-as.js", "instn-iee-err-dflt-thru-star.js",
        "instn-named-bndng-dflt-cls.js", "instn-named-bndng-dflt-expr.js",
        "instn-named-bndng-dflt-fun-anon.js", "instn-named-bndng-dflt-fun-named.js",
        "instn-named-bndng-dflt-gen-anon.js", "instn-named-bndng-dflt-gen-named.js",
        "instn-named-bndng-dflt-named.js", "instn-named-err-dflt-thru-star-as.js",
        "instn-named-err-dflt-thru-star-dflt.js",
        "instn-named-err-not-found-dflt.js", "parse-err-export-dflt-const.js",
        "parse-err-export-dflt-expr.js", "parse-err-export-dflt-let.js",
        "parse-err-export-dflt-var.js", "parse-err-semi-dflt-expr.js",
    )
}

MODULE_NAMESPACE_FILES = {
    f"language/module-code/{name}"
    for name in (
        "ambiguous-export-bindings/namespace-unambiguous-if-export-star-as-from.js",
        "ambiguous-export-bindings/namespace-unambiguous-if-import-star-as-and-export.js",
        "ambiguous-export-bindings/omitted-from-namespace.js",
        "eval-rqstd-once.js", "eval-rqstd-order.js", "eval-self-once.js",
        "export-star-as-dflt.js", "instn-named-bndng-dflt-star.js", "instn-once.js",
        "instn-star-as-props-dflt-skip.js", "instn-star-props-circular.js",
        "instn-star-props-dflt-keep-indirect.js", "instn-star-props-dflt-keep-local.js",
        "instn-star-props-dflt-skip.js", "instn-star-props-nrml.js",
        "namespace/Symbol.iterator.js", "namespace/Symbol.toStringTag.js",
        "namespace/internals/define-own-property.js",
        "namespace/internals/delete-exported-init.js",
        "namespace/internals/delete-exported-uninit.js",
        "namespace/internals/delete-non-exported.js",
        "namespace/internals/enumerate-binding-uninit.js",
        "namespace/internals/get-nested-namespace-dflt-skip.js",
        "namespace/internals/get-nested-namespace-props-nrml.js",
        "namespace/internals/get-own-property-str-found-init.js",
        "namespace/internals/get-own-property-str-found-uninit.js",
        "namespace/internals/get-own-property-str-not-found.js",
        "namespace/internals/get-own-property-sym.js", "namespace/internals/get-prototype-of.js",
        "namespace/internals/get-str-found-init.js",
        "namespace/internals/get-str-found-uninit.js",
        "namespace/internals/get-str-initialize.js", "namespace/internals/get-str-not-found.js",
        "namespace/internals/get-str-update.js", "namespace/internals/get-sym-found.js",
        "namespace/internals/get-sym-not-found.js",
        "namespace/internals/has-property-str-found-init.js",
        "namespace/internals/has-property-str-found-uninit.js",
        "namespace/internals/has-property-str-not-found.js",
        "namespace/internals/has-property-sym-found.js",
        "namespace/internals/has-property-sym-not-found.js", "namespace/internals/is-extensible.js",
        "namespace/internals/object-hasOwnProperty-binding-uninit.js",
        "namespace/internals/object-keys-binding-uninit.js",
        "namespace/internals/object-propertyIsEnumerable-binding-uninit.js",
        "namespace/internals/own-property-keys-binding-types.js",
        "namespace/internals/own-property-keys-sort.js",
        "namespace/internals/prevent-extensions.js",
        "namespace/internals/set-prototype-of-null.js", "namespace/internals/set-prototype-of.js",
        "namespace/internals/set.js", "namespace/internals/super-access-to-tdz-binding.js",
        "namespace/internals/super-set-to-tdz-binding-with-accessor.js",
    )
}
MODULE_CORE_FILES.update(MODULE_NAMESPACE_FILES)
MODULE_CORE_FILES.update(MODULE_STATIC_SEMANTICS_FILES)
MODULE_CORE_FILES.update(MODULE_TLA_SYNTAX_FILES)
MODULE_CORE_FILES.update(MODULE_TLA_RUNTIME_FILES)

MODULE_NAMESPACE_FEATURES = {
    "Symbol", "Symbol.iterator", "Symbol.toStringTag", "Reflect",
    "export-star-as-namespace-from-module",
}

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

OBJECT_SPREAD_SYMBOL_PREFIXES = (
    "language/expressions/array/spread-obj-",
    "language/expressions/call/spread-obj-",
    "language/expressions/new/spread-obj-",
)

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

TYPED_ARRAY_ACCESSOR_FILES = {
    f"built-ins/TypedArray/prototype/{name}/{suffix}"
    for name, suffixes in {
        "byteLength": (
            "BigInt/detached-buffer.js", "BigInt/resizable-array-buffer-auto.js",
            "BigInt/resizable-array-buffer-fixed.js", "BigInt/return-bytelength.js",
            "detached-buffer.js", "invoked-as-accessor.js", "invoked-as-func.js",
            "length.js", "name.js", "prop-desc.js", "resizable-array-buffer-auto.js",
            "resizable-array-buffer-fixed.js", "resizable-buffer-assorted.js",
            "resized-out-of-bounds-1.js", "resized-out-of-bounds-2.js",
            "return-bytelength.js", "this-has-no-typedarrayname-internal.js",
            "this-is-not-object.js",
        ),
        "byteOffset": (
            "BigInt/detached-buffer.js", "BigInt/resizable-array-buffer-auto.js",
            "BigInt/resizable-array-buffer-fixed.js", "BigInt/return-byteoffset.js",
            "detached-buffer.js", "invoked-as-accessor.js", "invoked-as-func.js",
            "length.js", "name.js", "prop-desc.js", "resizable-array-buffer-auto.js",
            "resizable-array-buffer-fixed.js", "resized-out-of-bounds.js",
            "return-byteoffset.js", "this-has-no-typedarrayname-internal.js",
            "this-is-not-object.js",
        ),
        "length": (
            "BigInt/detached-buffer.js", "BigInt/resizable-array-buffer-auto.js",
            "BigInt/resizable-array-buffer-fixed.js", "BigInt/return-length.js",
            "detached-buffer.js", "invoked-as-accessor.js", "invoked-as-func.js",
            "length.js", "name.js", "prop-desc.js", "resizable-array-buffer-auto.js",
            "resizable-array-buffer-fixed.js", "resizable-buffer-assorted.js",
            "resized-out-of-bounds-1.js", "resized-out-of-bounds-2.js",
            "return-length.js", "this-has-no-typedarrayname-internal.js",
            "this-is-not-object.js",
        ),
    }.items()
    for suffix in suffixes
}

TYPED_ARRAY_ACCESSOR_FEATURES = {
    "ArrayBuffer",
    "BigInt",
    "DataView",
    "Symbol",
    "TypedArray",
    "resizable-arraybuffer",
}

TYPED_ARRAY_TO_STRING_FILES = {
    "built-ins/TypedArray/prototype/toString.js",
    "built-ins/TypedArray/prototype/toString/BigInt/detached-buffer.js",
    "built-ins/TypedArray/prototype/toString/detached-buffer.js",
    "built-ins/TypedArray/prototype/toString/not-a-constructor.js",
}

TYPED_ARRAY_TO_STRING_FEATURES = {
    "BigInt",
    "Reflect.construct",
    "TypedArray",
    "arrow-function",
}

TYPED_ARRAY_PROTOTYPE_INTRINSIC_FILES = {
    "built-ins/TypedArray/prototype/Symbol.iterator.js",
    "built-ins/TypedArray/prototype/constructor.js",
}

TYPED_ARRAY_PROTOTYPE_INTRINSIC_FEATURES = {
    "Symbol.iterator",
    "TypedArray",
}

TYPED_ARRAY_FROM_FILES = {
    f"built-ins/TypedArray/from/{name}"
    for name in (
        "arylk-get-length-error.js",
        "arylk-to-length-error.js",
        "from-array-mapper-detaches-result.js",
        "from-array-mapper-makes-result-out-of-bounds.js",
        "from-typedarray-into-itself-mapper-detaches-result.js",
        "from-typedarray-into-itself-mapper-makes-result-out-of-bounds.js",
        "from-typedarray-mapper-detaches-result.js",
        "from-typedarray-mapper-makes-result-out-of-bounds.js",
        "invoked-as-func.js",
        "invoked-as-method.js",
        "iter-access-error.js",
        "iter-invoke-error.js",
        "iter-next-error.js",
        "iter-next-value-error.js",
        "iterated-array-changed-by-tonumber.js",
        "length.js",
        "mapfn-is-not-callable.js",
        "name.js",
        "not-a-constructor.js",
        "prop-desc.js",
        "this-is-not-constructor.js",
    )
}

TYPED_ARRAY_FROM_FEATURES = {
    "Reflect.construct",
    "Symbol",
    "Symbol.iterator",
    "TypedArray",
    "arrow-function",
    "resizable-arraybuffer",
}

TYPED_ARRAY_STATIC_FILES = {
    f"built-ins/TypedArray/{name}"
    for name in (
        "Symbol.species/prop-desc.js",
        "Symbol.species/result.js",
        "invoked.js",
        "length.js",
        "name.js",
        "of/invoked-as-func.js",
        "of/invoked-as-method.js",
        "of/length.js",
        "of/name.js",
        "of/not-a-constructor.js",
        "of/prop-desc.js",
        "of/this-is-not-constructor.js",
        "prototype.js",
    )
}

TYPED_ARRAY_STATIC_FEATURES = {
    "Reflect.construct",
    "Symbol.species",
    "TypedArray",
    "arrow-function",
}

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

REGEXP_LITERAL_EXTENDED_TIMEOUT_FILES = {
    "language/literals/regexp/S7.8.5_A1.1_T2.js",
    "language/literals/regexp/S7.8.5_A1.4_T2.js",
    "language/literals/regexp/S7.8.5_A2.1_T2.js",
    "language/literals/regexp/S7.8.5_A2.4_T2.js",
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

TYPED_ARRAY_BUFFER_PREFIXES = (
    "built-ins/TypedArray/prototype/buffer/",
)

TYPED_ARRAY_BUFFER_FEATURES = {
    "BigInt",
    "DataView",
    "Symbol",
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

CLASS_PRIVATE_BRAND_REALM_FILES = {
    "language/expressions/class/private-getter-brand-check-multiple-evaluations-of-class-realm-function-ctor.js",
    "language/expressions/class/private-getter-brand-check-multiple-evaluations-of-class-realm.js",
    "language/expressions/class/private-method-brand-check-multiple-evaluations-of-class-realm-function-ctor.js",
    "language/expressions/class/private-method-brand-check-multiple-evaluations-of-class-realm.js",
    "language/expressions/class/private-setter-brand-check-multiple-evaluations-of-class-realm-function-ctor.js",
    "language/expressions/class/private-setter-brand-check-multiple-evaluations-of-class-realm.js",
}

CLASS_PRIVATE_BRAND_REALM_FEATURES = {
    "class-methods-private",
}

CLASS_PUBLIC_FIELD_FEATURES = {
    "class-fields-public",
    "class-static-fields-public",
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
    m = re.search(r'/\*---\n(.*?)\n---\*/', src, re.DOTALL)
    if not m:
        return {}
    meta = {}
    block = m.group(1)
    for key in ('flags', 'features', 'includes'):
        m2 = re.search(rf'^{key}:\s*\[(.*?)\]', block, re.MULTILINE | re.DOTALL)
        if m2:
            meta[key] = [x.strip() for x in m2.group(1).split(',') if x.strip()]
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

def object_spread_symbol_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(OBJECT_SPREAD_SYMBOL_PREFIXES)

def typed_array_constructors_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(TYPED_ARRAY_CONSTRUCTORS_PREFIXES)

def typed_array_accessor_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in TYPED_ARRAY_ACCESSOR_FILES

def typed_array_to_string_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in TYPED_ARRAY_TO_STRING_FILES

def typed_array_prototype_intrinsic_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in TYPED_ARRAY_PROTOTYPE_INTRINSIC_FILES

def typed_array_from_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in TYPED_ARRAY_FROM_FILES

def typed_array_static_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in TYPED_ARRAY_STATIC_FILES

def typed_array_resizable_path(path, meta):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return (
        rel_text in TYPED_ARRAY_RESIZABLE_FILES
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

def regexp_literal_extended_timeout_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in REGEXP_LITERAL_EXTENDED_TIMEOUT_FILES

def test_timeout_seconds(path):
    if typed_array_copy_within_extended_timeout_path(path):
        return 600
    if regexp_literal_extended_timeout_path(path):
        return 20
    return 8

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

def typed_array_buffer_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix().startswith(TYPED_ARRAY_BUFFER_PREFIXES)

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

def class_private_brand_realm_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in CLASS_PRIVATE_BRAND_REALM_FILES

def class_computed_field_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in CLASS_COMPUTED_FIELD_FILES

def class_default_parameter_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in CLASS_DEFAULT_PARAMETER_FILES

def class_destructuring_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in CLASS_DESTRUCTURING_FILES

def class_private_path_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return frozenset()
    return CLASS_PRIVATE_FEATURES_BY_FILE.get(rel.as_posix(), frozenset())

def class_public_field_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in CLASS_PUBLIC_FIELD_FILES

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

def module_core_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in MODULE_CORE_FILES

def module_namespace_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in MODULE_NAMESPACE_FILES

def module_tla_syntax_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in MODULE_TLA_SYNTAX_FILES

def module_tla_runtime_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in MODULE_TLA_RUNTIME_FILES

def dynamic_import_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in DYNAMIC_IMPORT_FILES

def import_meta_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in IMPORT_META_FILES

def json_parse_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in JSON_PARSE_FILES

def json_stringify_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in JSON_STRINGIFY_FILES

def json_raw_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in JSON_RAW_FILES

def date_to_primitive_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in DATE_TO_PRIMITIVE_FILES

def proxy_get_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in PROXY_GET_FILES

def proxy_get_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return PROXY_GET_FEATURES.get(rel.as_posix(), frozenset())

def reference_primitive_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in REFERENCE_PRIMITIVE_FILES

def should_skip(meta, path=None):
    feats = set(meta.get('features', []))
    if path is not None and module_core_path(path):
        feats.discard("generators")
    if path is not None and module_namespace_path(path):
        feats.difference_update(MODULE_NAMESPACE_FEATURES)
    if path is not None and module_tla_syntax_path(path):
        feats.difference_update({"top-level-await", "async-iteration"})
    if path is not None and module_tla_runtime_path(path):
        feats.discard("top-level-await")
    if path is not None and dynamic_import_path(path):
        feats.difference_update({
            "dynamic-import", "generators", "async-iteration",
            "export-star-as-namespace-from-module", "top-level-await",
            "Symbol", "Symbol.iterator", "Symbol.toStringTag", "Reflect",
            "import-attributes", "async-functions", "Proxy", "json-modules",
            "import-text",
        })
    if path is not None and import_meta_path(path):
        feats.difference_update({
            "import.meta", "dynamic-import", "generators", "async-functions",
            "async-iteration", "object-rest",
        })
    if path is not None and json_parse_path(path):
        feats.difference_update({
            "Proxy", "Reflect.construct", "Symbol", "json-parse-with-source",
        })
    if path is not None and json_stringify_path(path):
        feats.difference_update({"Proxy", "Reflect.construct", "Symbol", "cross-realm"})
    if path is not None and json_raw_path(path):
        feats.difference_update({
            "Reflect.construct", "Symbol.toStringTag", "json-parse-with-source",
        })
    if path is not None and date_to_primitive_path(path):
        feats.discard("Symbol")
    if path is not None and proxy_get_path(path):
        feats.difference_update(proxy_get_features(path))
    if path is not None and reference_primitive_path(path):
        feats.difference_update({"cross-realm", "Symbol", "Proxy"})
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
    if path is not None and typed_array_accessor_path(path):
        feats.difference_update(TYPED_ARRAY_ACCESSOR_FEATURES)
    if path is not None and typed_array_to_string_path(path):
        feats.difference_update(TYPED_ARRAY_TO_STRING_FEATURES)
    if path is not None and typed_array_prototype_intrinsic_path(path):
        feats.difference_update(TYPED_ARRAY_PROTOTYPE_INTRINSIC_FEATURES)
    if path is not None and typed_array_from_path(path):
        feats.difference_update(TYPED_ARRAY_FROM_FEATURES)
    if path is not None and typed_array_static_path(path):
        feats.difference_update(TYPED_ARRAY_STATIC_FEATURES)
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
    if path is not None and typed_array_buffer_path(path):
        feats.difference_update(TYPED_ARRAY_BUFFER_FEATURES)
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
    if path is not None and class_private_brand_realm_path(path):
        feats.difference_update(CLASS_PRIVATE_BRAND_REALM_FEATURES)
    if path is not None and class_computed_field_path(path):
        feats.difference_update(CLASS_PUBLIC_FIELD_FEATURES)
    if path is not None and class_default_parameter_path(path):
        feats.discard("default-parameters")
    if path is not None and class_destructuring_path(path):
        feats.discard("destructuring-binding")
    if path is not None:
        feats.difference_update(class_private_path_features(path))
    if path is not None and class_public_field_path(path):
        feats.difference_update(CLASS_PUBLIC_FIELD_FEATURES)
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
        or module_tla_runtime_path(path)
        or dynamic_import_path(path)
        or import_meta_path(path)
    )
    module_admitted = path is not None and (
        module_core_path(path) or dynamic_import_path(path) or import_meta_path(path)
    )
    if ('module' in flags and not module_admitted) or (
        'async' in flags and not (RUN_ASYNC_TESTS or async_admitted)
    ):
        return True
    return False

BASE_HARNESS = ['sta.js', 'assert.js']

def build_source(path):
    src = Path(path).read_text()
    meta = parse_meta(src)
    flags = meta.get('flags', [])
    if 'raw' in flags:
        return src, meta
    parts = []
    # onlyStrict must be a directive prologue before any harness code, matching
    # test262_runner.py. Otherwise strict-mode negative tests are misbucketed.
    if 'onlyStrict' in flags:
        parts.append("'use strict';")
    for inc in BASE_HARNESS:
        p = HARNESS / inc
        if p.exists():
            parts.append(p.read_text())
    append_async_harness(parts, HARNESS, flags)
    for inc in meta.get('includes', []):
        p = HARNESS / inc
        if p.exists():
            parts.append(p.read_text())
    parts.append(src)
    return "\n".join(parts), meta

def run_test(path):
    """Return (status, err). For negative tests a thrown error of the
    expected type counts as pass. RuJa reports errors via stderr/stdout and
    may exit 0 or nonzero, so we judge by error text, not exit code."""
    full, meta = build_source(path)
    if should_skip(meta, path):
        return 'skip', ''
    timeout = test_timeout_seconds(path)
    source_path = path if "module" in meta.get("flags", []) or dynamic_import_path(path) else None
    return execute_source(
        full, meta, RUJA, timeout=timeout, source_path=source_path
    )

def bucket(err):
    if not err:
        return 'OTHER: (no output)'
    # normalize paths/ids
    err = re.sub(r"'[^']{5,}'", "'<value>'", err)
    err = re.sub(r'\([^)]*\)', '()', err)
    err = re.sub(r'at line \d+', 'at line <n>', err)
    err = re.sub(r'\[[^\]]+\]', '[]', err)
    err = err.strip().split('\n')[0]
    return err[:200]

def main():
    dirs = sys.argv[1:] if len(sys.argv) > 1 else ['language/expressions']
    fails = defaultdict(list)
    counts = Counter()
    for d in dirs:
        base = Path(TEST262) / 'test' / d
        if not base.exists():
            print(f"SKIP missing: {base}", file=sys.stderr)
            continue
        files = sorted(base.rglob('*.js'))
        print(f"Scanning {len(files)} files under {d} ...", file=sys.stderr)
        for f in files:
            if '_FIXTURE' in f.name:
                continue
            status, err = run_test(f)
            if status == 'fail':
                b = bucket(err)
                fails[b].append((str(f.relative_to(Path(TEST262) / 'test')), err))
                counts[b] += 1

    # Sort buckets by frequency
    print("\n=== SUMMARY ===")
    for b, c in counts.most_common():
        print(f"{c:>4} {b}")
    print("\n=== SAMPLE FAILS PER BUCKET ===")
    for b, items in fails.items():
        print(f"\n--- {b} ({len(items)}) ---")
        for p, e in items[:3]:
            print(f"  {p}")
            print(f"      {e[:200]}")

    out = '/tmp/ruja_test262_fails.json'
    with open(out, 'w') as f:
        json.dump({b: items for b, items in fails.items()}, f, indent=2)
    print(f"\nfull dump -> {out}")

if __name__ == '__main__':
    main()
