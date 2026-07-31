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
    from test262_class_computed_field_admission import CLASS_COMPUTED_FIELD_FILES
    from test262_class_default_parameter_admission import CLASS_DEFAULT_PARAMETER_FILES
    from test262_class_destructuring_admission import CLASS_DESTRUCTURING_FILES
    from test262_decorator_admission import DECORATOR_FILES
    from test262_class_private_admission import CLASS_PRIVATE_FEATURES_BY_FILE
    from test262_class_subclass_builtin_admission import (
        CLASS_SUBCLASS_BUILTIN_FEATURES_BY_FILE,
    )
    from test262_class_public_field_admission import CLASS_PUBLIC_FIELD_FILES
    from test262_date_to_primitive_admission import DATE_TO_PRIMITIVE_FILES
    from test262_generator_function_admission import (
        GENERATOR_FUNCTION_FEATURES, GENERATOR_FUNCTION_FILES,
    )
    from test262_async_generator_realm_admission import (
        ASYNC_GENERATOR_REALM_FEATURES, ASYNC_GENERATOR_REALM_FILES,
    )
    from test262_async_iterator_dispose_admission import (
        ASYNC_ITERATOR_DISPOSE_FEATURES, ASYNC_ITERATOR_DISPOSE_FILES,
    )
    from test262_async_from_sync_iterator_admission import (
        ASYNC_FROM_SYNC_ITERATOR_FEATURES, ASYNC_FROM_SYNC_ITERATOR_FILES,
    )
    from test262_array_from_async_admission import (
        ARRAY_FROM_ASYNC_FEATURES, ARRAY_FROM_ASYNC_FILES,
    )
    from test262_object_constructor_admission import (
        OBJECT_CONSTRUCTOR_FEATURES, OBJECT_CONSTRUCTOR_FILES,
    )
    from test262_object_from_entries_admission import (
        OBJECT_FROM_ENTRIES_FEATURES, OBJECT_FROM_ENTRIES_FILES,
    )
    from test262_object_group_by_admission import (
        OBJECT_GROUP_BY_FEATURES, OBJECT_GROUP_BY_FILES,
    )
    from test262_map_group_by_admission import (
        MAP_GROUP_BY_FEATURES, MAP_GROUP_BY_FILES,
    )
    from test262_map_constructor_admission import (
        MAP_CONSTRUCTOR_FEATURES, MAP_CONSTRUCTOR_FILES,
    )
    from test262_set_constructor_admission import (
        SET_CONSTRUCTOR_FEATURES, SET_CONSTRUCTOR_FILES,
    )
    from test262_set_algebra_admission import SET_ALGEBRA_FEATURES, SET_ALGEBRA_FILES
    from test262_weak_collection_admission import (
        WEAK_COLLECTION_FEATURES, WEAK_COLLECTION_FILES,
    )
    from test262_weak_reference_admission import (
        WEAK_REFERENCE_FILES, weak_reference_features,
    )
    from test262_native_construct_admission import (
        NATIVE_CONSTRUCT_FEATURES, NATIVE_CONSTRUCT_FILES,
    )
    from test262_object_prototype_admission import (
        OBJECT_PROTOTYPE_FEATURES_BY_FILE, OBJECT_PROTOTYPE_FILES,
    )
    from test262_promise_realm_admission import (
        PROMISE_REALM_FEATURES, PROMISE_REALM_FILES,
    )
    from test262_promise_combinator_close_admission import (
        PROMISE_COMBINATOR_CLOSE_FEATURES, PROMISE_COMBINATOR_CLOSE_FILES,
    )
    from test262_promise_combinator_rejection_admission import (
        PROMISE_COMBINATOR_REJECTION_FEATURES, PROMISE_COMBINATOR_REJECTION_FILES,
    )
    from test262_promise_keyed_admission import (
        PROMISE_KEYED_FEATURES, PROMISE_KEYED_FILES,
    )
    from test262_promise_constructor_order_admission import (
        PROMISE_CONSTRUCTOR_ORDER_FEATURES, PROMISE_CONSTRUCTOR_ORDER_FILES,
    )
    from test262_promise_finally_admission import (
        PROMISE_FINALLY_FEATURES, PROMISE_FINALLY_FILES,
    )
    from test262_regexp_match_indices_admission import (
        REGEXP_MATCH_INDICES_FEATURES, REGEXP_MATCH_INDICES_FILES,
    )
    from test262_regexp_named_groups_admission import REGEXP_NAMED_GROUPS_FEATURES
    from test262_regexp_duplicate_named_groups_admission import (
        REGEXP_DUPLICATE_NAMED_GROUPS_FEATURES,
        REGEXP_DUPLICATE_NAMED_GROUPS_FILES,
    )
    from test262_regexp_unicode_sets_admission import (
        REGEXP_UNICODE_SETS_FEATURES,
        REGEXP_UNICODE_SETS_FILES,
    )
    from test262_regexp_uv_flags_admission import (
        REGEXP_UV_FLAGS_FEATURES,
        REGEXP_UV_FLAGS_FILES,
    )
    from test262_regexp_logical_utf16_admission import (
        REGEXP_LOGICAL_UTF16_FEATURES,
        REGEXP_LOGICAL_UTF16_FILES,
    )
    from test262_proxy_get_admission import PROXY_GET_FEATURES, PROXY_GET_FILES
    from test262_proxy_has_admission import PROXY_HAS_FEATURES, PROXY_HAS_FILES
    from test262_proxy_delete_admission import (
        PROXY_DELETE_FEATURES, PROXY_DELETE_FILES,
    )
    from test262_extensibility_admission import (
        EXTENSIBILITY_FEATURES,
        EXTENSIBILITY_FILES,
        EXTENSIBILITY_MODULE_FILES,
    )
    from test262_prototype_internal_admission import (
        PROTOTYPE_INTERNAL_FEATURES,
        PROTOTYPE_INTERNAL_FILES,
    )
    from test262_proxy_define_property_admission import (
        PROXY_DEFINE_PROPERTY_FEATURES,
        PROXY_DEFINE_PROPERTY_FILES,
    )
    from test262_proxy_set_admission import PROXY_SET_FEATURES, PROXY_SET_FILES
    from test262_proxy_own_keys_admission import (
        PROXY_OWN_KEYS_FEATURES, PROXY_OWN_KEYS_FILES,
    )
    from test262_proxy_for_in_admission import (
        PROXY_FOR_IN_FEATURES, PROXY_FOR_IN_FILES,
    )
    from test262_array_exotic_admission import (
        ARRAY_EXOTIC_FEATURES, ARRAY_EXOTIC_FILES,
    )
    from test262_array_concat_admission import (
        ARRAY_CONCAT_FEATURES, ARRAY_CONCAT_FILES,
    )
    from test262_array_copy_within_admission import (
        ARRAY_COPY_WITHIN_FEATURES, ARRAY_COPY_WITHIN_FILES,
    )
    from test262_array_fill_admission import ARRAY_FILL_FEATURES, ARRAY_FILL_FILES
    from test262_array_filter_admission import ARRAY_FILTER_FEATURES, ARRAY_FILTER_FILES
    from test262_array_map_admission import ARRAY_MAP_FEATURES, ARRAY_MAP_FILES
    from test262_array_for_each_admission import (
        ARRAY_FOR_EACH_FEATURES, ARRAY_FOR_EACH_FILES,
    )
    from test262_array_reduce_admission import (
        ARRAY_REDUCE_FEATURES, ARRAY_REDUCE_FILES,
    )
    from test262_array_reduce_right_admission import (
        ARRAY_REDUCE_RIGHT_FEATURES, ARRAY_REDUCE_RIGHT_FILES,
    )
    from test262_array_reverse_admission import (
        ARRAY_REVERSE_FEATURES, ARRAY_REVERSE_FILES,
    )
    from test262_array_to_reversed_admission import (
        ARRAY_TO_REVERSED_FEATURES, ARRAY_TO_REVERSED_FILES,
    )
    from test262_array_to_spliced_admission import (
        ARRAY_TO_SPLICED_FEATURES, ARRAY_TO_SPLICED_FILES,
    )
    from test262_array_to_locale_string_admission import (
        ARRAY_TO_LOCALE_STRING_FEATURES, ARRAY_TO_LOCALE_STRING_FILES,
    )
    from test262_typed_array_to_locale_string_admission import (
        TYPED_ARRAY_TO_LOCALE_STRING_FEATURES,
        TYPED_ARRAY_TO_LOCALE_STRING_FILES,
    )
    from test262_typed_array_join_admission import (
        TYPED_ARRAY_JOIN_FEATURES, TYPED_ARRAY_JOIN_FILES,
    )
    from test262_typed_array_to_string_admission import (
        TYPED_ARRAY_TO_STRING_FEATURES, TYPED_ARRAY_TO_STRING_FILES,
    )
    from test262_array_join_admission import ARRAY_JOIN_FEATURES, ARRAY_JOIN_FILES
    from test262_array_flat_admission import (
        ARRAY_FLAT_FEATURES, ARRAY_FLAT_FILES,
        ARRAY_FLAT_MAP_FEATURES, ARRAY_FLAT_MAP_FILES,
    )
    from test262_array_iterator_admission import (
        ARRAY_ITERATOR_FEATURES, ARRAY_ITERATOR_FILES,
    )
    from test262_reflect_set_has_admission import (
        REFLECT_SET_HAS_FEATURES, REFLECT_SET_HAS_FILES,
    )
    from test262_reflect_remaining_admission import (
        REFLECT_REMAINING_FEATURES, REFLECT_REMAINING_FILES,
    )
    from test262_reflect_call_admission import (
        REFLECT_CALL_FEATURES, REFLECT_CALL_FILES,
    )
    from test262_function_apply_admission import (
        FUNCTION_APPLY_FEATURES, FUNCTION_APPLY_FILES,
    )
    from test262_function_bind_admission import (
        FUNCTION_BIND_FEATURES, FUNCTION_BIND_FILES,
    )
    from test262_language_early_error_admission import (
        LANGUAGE_EARLY_ERROR_FEATURES,
        LANGUAGE_EARLY_ERROR_FILES,
        LANGUAGE_EARLY_ERROR_MODULE_FILES,
    )
    from test262_reference_primitive_admission import REFERENCE_PRIMITIVE_FILES
    from test262_support import (
        STRICT_PREFIX,
        append_async_harness,
        combine_variant_results,
        execute_source,
        execution_variants,
    )
    from test262_dynamic_import_admission import DYNAMIC_IMPORT_FILES
    from test262_static_import_attributes_admission import (
        STATIC_IMPORT_ATTRIBUTES_FILES,
        static_import_attributes_features,
    )
    from test262_intl_canonical_locales_admission import (
        INTL_CANONICAL_LOCALES_FILES,
        intl_canonical_locales_features,
    )
    from test262_intl_supported_values_admission import (
        INTL_SUPPORTED_VALUES_FILES,
        intl_supported_values_features,
    )
    from test262_intl_locale_admission import INTL_LOCALE_FILES, intl_locale_features
    from test262_intl_collator_admission import (
        INTL_COLLATOR_FILES,
        intl_collator_features,
    )
    from test262_import_meta_admission import IMPORT_META_FILES
    from test262_iterator_admission import ITERATOR_CORE_FEATURES, ITERATOR_CORE_FILES
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
    from tools.test262_decorator_admission import DECORATOR_FILES
    from tools.test262_class_private_admission import CLASS_PRIVATE_FEATURES_BY_FILE
    from tools.test262_class_subclass_builtin_admission import (
        CLASS_SUBCLASS_BUILTIN_FEATURES_BY_FILE,
    )
    from tools.test262_class_public_field_admission import CLASS_PUBLIC_FIELD_FILES
    from tools.test262_date_to_primitive_admission import DATE_TO_PRIMITIVE_FILES
    from tools.test262_generator_function_admission import (
        GENERATOR_FUNCTION_FEATURES, GENERATOR_FUNCTION_FILES,
    )
    from tools.test262_async_generator_realm_admission import (
        ASYNC_GENERATOR_REALM_FEATURES, ASYNC_GENERATOR_REALM_FILES,
    )
    from tools.test262_async_iterator_dispose_admission import (
        ASYNC_ITERATOR_DISPOSE_FEATURES, ASYNC_ITERATOR_DISPOSE_FILES,
    )
    from tools.test262_async_from_sync_iterator_admission import (
        ASYNC_FROM_SYNC_ITERATOR_FEATURES, ASYNC_FROM_SYNC_ITERATOR_FILES,
    )
    from tools.test262_array_from_async_admission import (
        ARRAY_FROM_ASYNC_FEATURES, ARRAY_FROM_ASYNC_FILES,
    )
    from tools.test262_object_constructor_admission import (
        OBJECT_CONSTRUCTOR_FEATURES, OBJECT_CONSTRUCTOR_FILES,
    )
    from tools.test262_object_from_entries_admission import (
        OBJECT_FROM_ENTRIES_FEATURES, OBJECT_FROM_ENTRIES_FILES,
    )
    from tools.test262_object_group_by_admission import (
        OBJECT_GROUP_BY_FEATURES, OBJECT_GROUP_BY_FILES,
    )
    from tools.test262_map_group_by_admission import (
        MAP_GROUP_BY_FEATURES, MAP_GROUP_BY_FILES,
    )
    from tools.test262_map_constructor_admission import (
        MAP_CONSTRUCTOR_FEATURES, MAP_CONSTRUCTOR_FILES,
    )
    from tools.test262_set_constructor_admission import (
        SET_CONSTRUCTOR_FEATURES, SET_CONSTRUCTOR_FILES,
    )
    from tools.test262_set_algebra_admission import SET_ALGEBRA_FEATURES, SET_ALGEBRA_FILES
    from tools.test262_weak_collection_admission import (
        WEAK_COLLECTION_FEATURES, WEAK_COLLECTION_FILES,
    )
    from tools.test262_weak_reference_admission import (
        WEAK_REFERENCE_FILES, weak_reference_features,
    )
    from tools.test262_native_construct_admission import (
        NATIVE_CONSTRUCT_FEATURES, NATIVE_CONSTRUCT_FILES,
    )
    from tools.test262_object_prototype_admission import (
        OBJECT_PROTOTYPE_FEATURES_BY_FILE, OBJECT_PROTOTYPE_FILES,
    )
    from tools.test262_promise_realm_admission import (
        PROMISE_REALM_FEATURES, PROMISE_REALM_FILES,
    )
    from tools.test262_promise_combinator_close_admission import (
        PROMISE_COMBINATOR_CLOSE_FEATURES, PROMISE_COMBINATOR_CLOSE_FILES,
    )
    from tools.test262_promise_combinator_rejection_admission import (
        PROMISE_COMBINATOR_REJECTION_FEATURES, PROMISE_COMBINATOR_REJECTION_FILES,
    )
    from tools.test262_promise_keyed_admission import (
        PROMISE_KEYED_FEATURES, PROMISE_KEYED_FILES,
    )
    from tools.test262_promise_constructor_order_admission import (
        PROMISE_CONSTRUCTOR_ORDER_FEATURES, PROMISE_CONSTRUCTOR_ORDER_FILES,
    )
    from tools.test262_promise_finally_admission import (
        PROMISE_FINALLY_FEATURES, PROMISE_FINALLY_FILES,
    )
    from tools.test262_regexp_match_indices_admission import (
        REGEXP_MATCH_INDICES_FEATURES, REGEXP_MATCH_INDICES_FILES,
    )
    from tools.test262_regexp_named_groups_admission import REGEXP_NAMED_GROUPS_FEATURES
    from tools.test262_regexp_duplicate_named_groups_admission import (
        REGEXP_DUPLICATE_NAMED_GROUPS_FEATURES,
        REGEXP_DUPLICATE_NAMED_GROUPS_FILES,
    )
    from tools.test262_regexp_unicode_sets_admission import (
        REGEXP_UNICODE_SETS_FEATURES,
        REGEXP_UNICODE_SETS_FILES,
    )
    from tools.test262_regexp_uv_flags_admission import (
        REGEXP_UV_FLAGS_FEATURES,
        REGEXP_UV_FLAGS_FILES,
    )
    from tools.test262_regexp_logical_utf16_admission import (
        REGEXP_LOGICAL_UTF16_FEATURES,
        REGEXP_LOGICAL_UTF16_FILES,
    )
    from tools.test262_proxy_get_admission import PROXY_GET_FEATURES, PROXY_GET_FILES
    from tools.test262_proxy_has_admission import PROXY_HAS_FEATURES, PROXY_HAS_FILES
    from tools.test262_proxy_delete_admission import (
        PROXY_DELETE_FEATURES, PROXY_DELETE_FILES,
    )
    from tools.test262_extensibility_admission import (
        EXTENSIBILITY_FEATURES,
        EXTENSIBILITY_FILES,
        EXTENSIBILITY_MODULE_FILES,
    )
    from tools.test262_prototype_internal_admission import (
        PROTOTYPE_INTERNAL_FEATURES,
        PROTOTYPE_INTERNAL_FILES,
    )
    from tools.test262_proxy_define_property_admission import (
        PROXY_DEFINE_PROPERTY_FEATURES,
        PROXY_DEFINE_PROPERTY_FILES,
    )
    from tools.test262_proxy_set_admission import PROXY_SET_FEATURES, PROXY_SET_FILES
    from tools.test262_proxy_own_keys_admission import (
        PROXY_OWN_KEYS_FEATURES, PROXY_OWN_KEYS_FILES,
    )
    from tools.test262_proxy_for_in_admission import (
        PROXY_FOR_IN_FEATURES, PROXY_FOR_IN_FILES,
    )
    from tools.test262_array_exotic_admission import (
        ARRAY_EXOTIC_FEATURES, ARRAY_EXOTIC_FILES,
    )
    from tools.test262_array_concat_admission import (
        ARRAY_CONCAT_FEATURES, ARRAY_CONCAT_FILES,
    )
    from tools.test262_array_copy_within_admission import (
        ARRAY_COPY_WITHIN_FEATURES, ARRAY_COPY_WITHIN_FILES,
    )
    from tools.test262_array_fill_admission import ARRAY_FILL_FEATURES, ARRAY_FILL_FILES
    from tools.test262_array_filter_admission import ARRAY_FILTER_FEATURES, ARRAY_FILTER_FILES
    from tools.test262_array_map_admission import ARRAY_MAP_FEATURES, ARRAY_MAP_FILES
    from tools.test262_array_for_each_admission import (
        ARRAY_FOR_EACH_FEATURES, ARRAY_FOR_EACH_FILES,
    )
    from tools.test262_array_reduce_admission import (
        ARRAY_REDUCE_FEATURES, ARRAY_REDUCE_FILES,
    )
    from tools.test262_array_reduce_right_admission import (
        ARRAY_REDUCE_RIGHT_FEATURES, ARRAY_REDUCE_RIGHT_FILES,
    )
    from tools.test262_array_reverse_admission import (
        ARRAY_REVERSE_FEATURES, ARRAY_REVERSE_FILES,
    )
    from tools.test262_array_to_reversed_admission import (
        ARRAY_TO_REVERSED_FEATURES, ARRAY_TO_REVERSED_FILES,
    )
    from tools.test262_array_to_spliced_admission import (
        ARRAY_TO_SPLICED_FEATURES, ARRAY_TO_SPLICED_FILES,
    )
    from tools.test262_array_to_locale_string_admission import (
        ARRAY_TO_LOCALE_STRING_FEATURES, ARRAY_TO_LOCALE_STRING_FILES,
    )
    from tools.test262_typed_array_to_locale_string_admission import (
        TYPED_ARRAY_TO_LOCALE_STRING_FEATURES,
        TYPED_ARRAY_TO_LOCALE_STRING_FILES,
    )
    from tools.test262_typed_array_join_admission import (
        TYPED_ARRAY_JOIN_FEATURES, TYPED_ARRAY_JOIN_FILES,
    )
    from tools.test262_typed_array_to_string_admission import (
        TYPED_ARRAY_TO_STRING_FEATURES, TYPED_ARRAY_TO_STRING_FILES,
    )
    from tools.test262_array_join_admission import ARRAY_JOIN_FEATURES, ARRAY_JOIN_FILES
    from tools.test262_array_flat_admission import (
        ARRAY_FLAT_FEATURES, ARRAY_FLAT_FILES,
        ARRAY_FLAT_MAP_FEATURES, ARRAY_FLAT_MAP_FILES,
    )
    from tools.test262_array_iterator_admission import (
        ARRAY_ITERATOR_FEATURES, ARRAY_ITERATOR_FILES,
    )
    from tools.test262_reflect_set_has_admission import (
        REFLECT_SET_HAS_FEATURES, REFLECT_SET_HAS_FILES,
    )
    from tools.test262_reflect_remaining_admission import (
        REFLECT_REMAINING_FEATURES, REFLECT_REMAINING_FILES,
    )
    from tools.test262_reflect_call_admission import (
        REFLECT_CALL_FEATURES, REFLECT_CALL_FILES,
    )
    from tools.test262_function_apply_admission import (
        FUNCTION_APPLY_FEATURES, FUNCTION_APPLY_FILES,
    )
    from tools.test262_function_bind_admission import (
        FUNCTION_BIND_FEATURES, FUNCTION_BIND_FILES,
    )
    from tools.test262_language_early_error_admission import (
        LANGUAGE_EARLY_ERROR_FEATURES,
        LANGUAGE_EARLY_ERROR_FILES,
        LANGUAGE_EARLY_ERROR_MODULE_FILES,
    )
    from tools.test262_reference_primitive_admission import REFERENCE_PRIMITIVE_FILES
    from tools.test262_support import (
        STRICT_PREFIX,
        append_async_harness,
        combine_variant_results,
        execute_source,
        execution_variants,
    )
    from tools.test262_dynamic_import_admission import DYNAMIC_IMPORT_FILES
    from tools.test262_static_import_attributes_admission import (
        STATIC_IMPORT_ATTRIBUTES_FILES,
        static_import_attributes_features,
    )
    from tools.test262_intl_canonical_locales_admission import (
        INTL_CANONICAL_LOCALES_FILES,
        intl_canonical_locales_features,
    )
    from tools.test262_intl_supported_values_admission import (
        INTL_SUPPORTED_VALUES_FILES,
        intl_supported_values_features,
    )
    from tools.test262_intl_locale_admission import (
        INTL_LOCALE_FILES,
        intl_locale_features,
    )
    from tools.test262_intl_collator_admission import (
        INTL_COLLATOR_FILES,
        intl_collator_features,
    )
    from tools.test262_import_meta_admission import IMPORT_META_FILES
    from tools.test262_iterator_admission import ITERATOR_CORE_FEATURES, ITERATOR_CORE_FILES
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
    "Int32Array", "Intl", "Intl-enumeration", "Intl.Locale", "Intl.Locale-info", "IsHTMLDDA", "Promise", "SharedArrayBuffer",
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
    "import-attributes", "import-defer", "import-text", "import.meta", "iterator-helpers",
    "iterator-sequencing", "joint-iteration",
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

REGEXP_CHARACTER_CLASS_ESCAPE_EXTENDED_TIMEOUT_FILES = {
    "built-ins/RegExp/CharacterClassEscapes/character-class-digit-class-escape-negative-cases.js",
    "built-ins/RegExp/CharacterClassEscapes/character-class-non-digit-class-escape-positive-cases.js",
    "built-ins/RegExp/CharacterClassEscapes/character-class-non-whitespace-class-escape-positive-cases.js",
    "built-ins/RegExp/CharacterClassEscapes/character-class-non-word-class-escape-positive-cases.js",
    "built-ins/RegExp/CharacterClassEscapes/character-class-whitespace-class-escape-negative-cases.js",
    "built-ins/RegExp/CharacterClassEscapes/character-class-word-class-escape-negative-cases.js",
}

REGEXP_CHARACTER_CLASS_ESCAPE_EXHAUSTIVE_TIMEOUT_FILES = {
    "built-ins/RegExp/character-class-escape-non-whitespace.js",
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

def typed_array_accessor_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in TYPED_ARRAY_ACCESSOR_FILES

def typed_array_to_string_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in TYPED_ARRAY_TO_STRING_FILES

def typed_array_to_string_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return TYPED_ARRAY_TO_STRING_FEATURES.get(rel.as_posix(), frozenset())

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
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in TYPED_ARRAY_JOIN_FILES

def typed_array_join_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return TYPED_ARRAY_JOIN_FEATURES.get(rel.as_posix(), frozenset())

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

def regexp_character_class_escape_extended_timeout_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in REGEXP_CHARACTER_CLASS_ESCAPE_EXTENDED_TIMEOUT_FILES

def regexp_character_class_escape_exhaustive_timeout_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in REGEXP_CHARACTER_CLASS_ESCAPE_EXHAUSTIVE_TIMEOUT_FILES

def test_timeout_seconds(path):
    if typed_array_copy_within_extended_timeout_path(path):
        return 600
    if regexp_logical_utf16_features(path):
        return 30
    if regexp_character_class_escape_exhaustive_timeout_path(path):
        return 60
    if regexp_literal_extended_timeout_path(path):
        return 20
    if regexp_character_class_escape_extended_timeout_path(path):
        return 30
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
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in TYPED_ARRAY_TO_LOCALE_STRING_FILES

def typed_array_to_locale_string_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return TYPED_ARRAY_TO_LOCALE_STRING_FEATURES.get(
        rel.as_posix(), frozenset()
    )

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

def weak_reference_scope_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    relative = rel.as_posix()
    return relative.startswith(WEAK_REF_PREFIXES) or relative.startswith(
        FINALIZATION_REGISTRY_PREFIXES
    )

def weak_ref_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    relative = rel.as_posix()
    return relative in WEAK_REFERENCE_FILES and relative.startswith(WEAK_REF_PREFIXES)

def finalization_registry_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    relative = rel.as_posix()
    return relative in WEAK_REFERENCE_FILES and relative.startswith(
        FINALIZATION_REGISTRY_PREFIXES
    )

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

def decorator_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in DECORATOR_FILES

def iterator_core_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    return rel.as_posix() in ITERATOR_CORE_FILES

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

def class_subclass_builtin_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return frozenset()
    return CLASS_SUBCLASS_BUILTIN_FEATURES_BY_FILE.get(
        rel.as_posix(), frozenset()
    )

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

def static_import_attributes_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in STATIC_IMPORT_ATTRIBUTES_FILES

def static_import_attributes_path_features(path):
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return frozenset()
    return static_import_attributes_features(rel.as_posix())

def intl_canonical_locales_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return False
    return rel.as_posix() in INTL_CANONICAL_LOCALES_FILES

def intl_canonical_locales_scope_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    relative = rel.as_posix()
    return relative == "intl402/Intl/builtin.js" or relative.startswith(
        ("intl402/Intl/toStringTag/", "intl402/Intl/getCanonicalLocales/")
    )

def intl_canonical_locales_path_features(path):
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return frozenset()
    return intl_canonical_locales_features(rel.as_posix())

def intl_supported_values_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return False
    return rel.as_posix() in INTL_SUPPORTED_VALUES_FILES

def intl_supported_values_scope_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix().startswith("intl402/Intl/supportedValuesOf/")

def intl_supported_values_path_features(path):
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return frozenset()
    return intl_supported_values_features(rel.as_posix())

def intl_locale_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return False
    return rel.as_posix() in INTL_LOCALE_FILES

def intl_locale_scope_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return False
    return rel.as_posix().startswith("intl402/Locale/")

def intl_locale_path_features(path):
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return frozenset()
    return intl_locale_features(rel.as_posix())

def intl_collator_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return False
    return rel.as_posix() in INTL_COLLATOR_FILES

def intl_collator_scope_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return False
    relative = rel.as_posix()
    return relative.startswith("intl402/Collator/") or relative.startswith(
        "intl402/String/prototype/localeCompare/"
    )

def intl_collator_path_features(path):
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError):
        return frozenset()
    return intl_collator_features(rel.as_posix())

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

def proxy_has_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in PROXY_HAS_FILES

def proxy_has_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return PROXY_HAS_FEATURES.get(rel.as_posix(), frozenset())

def proxy_delete_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in PROXY_DELETE_FILES

def proxy_delete_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return PROXY_DELETE_FEATURES.get(rel.as_posix(), frozenset())

def extensibility_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in EXTENSIBILITY_FILES

def extensibility_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return EXTENSIBILITY_FEATURES.get(rel.as_posix(), frozenset())

def extensibility_module_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in EXTENSIBILITY_MODULE_FILES

def prototype_internal_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in PROTOTYPE_INTERNAL_FILES

def prototype_internal_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return PROTOTYPE_INTERNAL_FEATURES.get(rel.as_posix(), frozenset())

def proxy_define_property_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in PROXY_DEFINE_PROPERTY_FILES

def proxy_define_property_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return PROXY_DEFINE_PROPERTY_FEATURES.get(rel.as_posix(), frozenset())

def proxy_set_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in PROXY_SET_FILES

def proxy_set_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return PROXY_SET_FEATURES.get(rel.as_posix(), frozenset())

def proxy_own_keys_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in PROXY_OWN_KEYS_FILES

def proxy_own_keys_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return PROXY_OWN_KEYS_FEATURES.get(rel.as_posix(), frozenset())

def proxy_for_in_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in PROXY_FOR_IN_FILES

def proxy_for_in_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return PROXY_FOR_IN_FEATURES.get(rel.as_posix(), frozenset())

def array_exotic_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in ARRAY_EXOTIC_FILES

def array_exotic_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return ARRAY_EXOTIC_FEATURES.get(rel.as_posix(), frozenset())

def array_concat_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in ARRAY_CONCAT_FILES

def array_concat_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return ARRAY_CONCAT_FEATURES.get(rel.as_posix(), frozenset())

def array_copy_within_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in ARRAY_COPY_WITHIN_FILES

def array_copy_within_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return ARRAY_COPY_WITHIN_FEATURES.get(rel.as_posix(), frozenset())

def array_fill_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in ARRAY_FILL_FILES

def array_fill_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return ARRAY_FILL_FEATURES.get(rel.as_posix(), frozenset())

def array_filter_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in ARRAY_FILTER_FILES

def array_filter_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return ARRAY_FILTER_FEATURES.get(rel.as_posix(), frozenset())

def array_map_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_MAP_FILES

def array_map_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_MAP_FEATURES.get(rel.as_posix(), frozenset())

def array_for_each_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_FOR_EACH_FILES

def array_for_each_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_FOR_EACH_FEATURES.get(rel.as_posix(), frozenset())

def array_reduce_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_REDUCE_FILES

def array_reduce_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_REDUCE_FEATURES.get(rel.as_posix(), frozenset())

def array_reduce_right_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_REDUCE_RIGHT_FILES

def array_reduce_right_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_REDUCE_RIGHT_FEATURES.get(rel.as_posix(), frozenset())

def array_reverse_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_REVERSE_FILES

def array_reverse_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_REVERSE_FEATURES.get(rel.as_posix(), frozenset())

def array_to_reversed_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_TO_REVERSED_FILES

def array_to_reversed_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_TO_REVERSED_FEATURES.get(rel.as_posix(), frozenset())

def array_to_spliced_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_TO_SPLICED_FILES

def array_to_spliced_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_TO_SPLICED_FEATURES.get(rel.as_posix(), frozenset())

def array_to_locale_string_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_TO_LOCALE_STRING_FILES

def array_to_locale_string_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_TO_LOCALE_STRING_FEATURES.get(rel.as_posix(), frozenset())

def array_join_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_JOIN_FILES

def array_join_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_JOIN_FEATURES.get(rel.as_posix(), frozenset())

def array_flat_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_FLAT_FILES

def array_flat_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_FLAT_FEATURES.get(rel.as_posix(), frozenset())

def array_flat_map_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return False
    return rel.as_posix() in ARRAY_FLAT_MAP_FILES

def array_flat_map_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError, TypeError):
        return frozenset()
    return ARRAY_FLAT_MAP_FEATURES.get(rel.as_posix(), frozenset())

def array_iterator_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in ARRAY_ITERATOR_FILES

def array_iterator_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return ARRAY_ITERATOR_FEATURES.get(rel.as_posix(), frozenset())

def reflect_set_has_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in REFLECT_SET_HAS_FILES

def reflect_set_has_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return REFLECT_SET_HAS_FEATURES.get(rel.as_posix(), frozenset())

def reflect_remaining_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in REFLECT_REMAINING_FILES

def reflect_remaining_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return REFLECT_REMAINING_FEATURES.get(rel.as_posix(), frozenset())

def reflect_call_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in REFLECT_CALL_FILES

def reflect_call_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return REFLECT_CALL_FEATURES.get(rel.as_posix(), frozenset())

def function_apply_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in FUNCTION_APPLY_FILES

def function_apply_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return FUNCTION_APPLY_FEATURES.get(rel.as_posix(), frozenset())

def function_bind_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, TypeError, ValueError):
        return False
    return rel.as_posix() in FUNCTION_BIND_FILES

def function_bind_features(path):
    if not function_bind_path(path):
        return frozenset()
    rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    return FUNCTION_BIND_FEATURES[rel.as_posix()]

def language_early_error_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except (OSError, TypeError, ValueError):
        return False
    return rel.as_posix() in LANGUAGE_EARLY_ERROR_FILES

def language_early_error_features(path):
    if not language_early_error_path(path):
        return frozenset()
    rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    return LANGUAGE_EARLY_ERROR_FEATURES[rel.as_posix()]

def language_early_error_module_path(path):
    if not language_early_error_path(path):
        return False
    rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    return rel.as_posix() in LANGUAGE_EARLY_ERROR_MODULE_FILES

def reference_primitive_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in REFERENCE_PRIMITIVE_FILES

def object_constructor_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in OBJECT_CONSTRUCTOR_FILES

def object_constructor_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return OBJECT_CONSTRUCTOR_FEATURES.get(rel.as_posix(), frozenset())

def object_from_entries_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in OBJECT_FROM_ENTRIES_FILES

def object_from_entries_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return OBJECT_FROM_ENTRIES_FEATURES.get(rel.as_posix(), frozenset())

def object_group_by_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in OBJECT_GROUP_BY_FILES

def object_group_by_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return OBJECT_GROUP_BY_FEATURES.get(rel.as_posix(), frozenset())

def map_group_by_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in MAP_GROUP_BY_FILES

def map_group_by_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return MAP_GROUP_BY_FEATURES.get(rel.as_posix(), frozenset())

def map_constructor_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in MAP_CONSTRUCTOR_FILES

def map_constructor_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return MAP_CONSTRUCTOR_FEATURES.get(rel.as_posix(), frozenset())

def set_constructor_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in SET_CONSTRUCTOR_FILES

def set_constructor_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return SET_CONSTRUCTOR_FEATURES.get(rel.as_posix(), frozenset())

def set_algebra_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in SET_ALGEBRA_FILES

def set_algebra_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return SET_ALGEBRA_FEATURES.get(rel.as_posix(), frozenset())

def weak_collection_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in WEAK_COLLECTION_FILES

def weak_collection_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return WEAK_COLLECTION_FEATURES.get(rel.as_posix(), frozenset())

def native_construct_path(path):
    if path is None:
        return False
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return False
    return rel.as_posix() in NATIVE_CONSTRUCT_FILES

def native_construct_features(path):
    if path is None:
        return frozenset()
    try:
        rel = Path(path).resolve().relative_to(Path(TEST262).resolve() / "test")
    except ValueError:
        return frozenset()
    return NATIVE_CONSTRUCT_FEATURES.get(rel.as_posix(), frozenset())

def object_prototype_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in OBJECT_PROTOTYPE_FILES

def object_prototype_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return OBJECT_PROTOTYPE_FEATURES_BY_FILE.get(rel.as_posix(), frozenset())

def promise_realm_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in PROMISE_REALM_FILES

def promise_realm_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return PROMISE_REALM_FEATURES.get(rel.as_posix(), frozenset())

def promise_combinator_close_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in PROMISE_COMBINATOR_CLOSE_FILES

def promise_combinator_close_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return PROMISE_COMBINATOR_CLOSE_FEATURES.get(rel.as_posix(), frozenset())

def promise_combinator_rejection_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in PROMISE_COMBINATOR_REJECTION_FILES

def promise_combinator_rejection_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return PROMISE_COMBINATOR_REJECTION_FEATURES.get(rel.as_posix(), frozenset())

def promise_keyed_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in PROMISE_KEYED_FILES

def promise_keyed_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return PROMISE_KEYED_FEATURES.get(rel.as_posix(), frozenset())

def promise_constructor_order_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in PROMISE_CONSTRUCTOR_ORDER_FILES

def promise_constructor_order_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return PROMISE_CONSTRUCTOR_ORDER_FEATURES.get(rel.as_posix(), frozenset())

def promise_finally_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in PROMISE_FINALLY_FILES

def promise_finally_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return PROMISE_FINALLY_FEATURES.get(rel.as_posix(), frozenset())

def regexp_match_indices_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return REGEXP_MATCH_INDICES_FEATURES.get(rel.as_posix(), frozenset())

def regexp_named_groups_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return REGEXP_NAMED_GROUPS_FEATURES.get(rel.as_posix(), frozenset())

def regexp_duplicate_named_groups_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return REGEXP_DUPLICATE_NAMED_GROUPS_FEATURES.get(rel.as_posix(), frozenset())

def regexp_unicode_sets_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return REGEXP_UNICODE_SETS_FEATURES.get(rel.as_posix(), frozenset())

def regexp_uv_flags_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return REGEXP_UV_FLAGS_FEATURES.get(rel.as_posix(), frozenset())

def regexp_logical_utf16_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return REGEXP_LOGICAL_UTF16_FEATURES.get(rel.as_posix(), frozenset())

def generator_function_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in GENERATOR_FUNCTION_FILES

def generator_function_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return GENERATOR_FUNCTION_FEATURES.get(rel.as_posix(), frozenset())

def async_generator_realm_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in ASYNC_GENERATOR_REALM_FILES

def async_generator_realm_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return ASYNC_GENERATOR_REALM_FEATURES.get(rel.as_posix(), frozenset())

def async_iterator_dispose_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in ASYNC_ITERATOR_DISPOSE_FILES

def async_iterator_dispose_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return ASYNC_ITERATOR_DISPOSE_FEATURES.get(rel.as_posix(), frozenset())

def async_from_sync_iterator_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in ASYNC_FROM_SYNC_ITERATOR_FILES

def async_from_sync_iterator_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return ASYNC_FROM_SYNC_ITERATOR_FEATURES.get(rel.as_posix(), frozenset())

def array_from_async_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return False
    return rel.as_posix() in ARRAY_FROM_ASYNC_FILES

def array_from_async_features(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except (OSError, ValueError):
        return frozenset()
    return ARRAY_FROM_ASYNC_FEATURES.get(rel.as_posix(), frozenset())

def should_skip(meta, path=None):
    feats = set(meta.get('features', []))
    if (
        path is not None
        and weak_reference_scope_path(path)
        and not weak_ref_path(path)
        and not finalization_registry_path(path)
    ):
        return True
    if (
        path is not None
        and intl_canonical_locales_scope_path(path)
        and not intl_canonical_locales_path(path)
        and not intl_locale_path(path)
    ):
        return True
    if (
        path is not None
        and intl_supported_values_scope_path(path)
        and not intl_supported_values_path(path)
    ):
        return True
    if path is not None and intl_locale_scope_path(path) and not intl_locale_path(path):
        return True
    if path is not None and intl_collator_scope_path(path) and not intl_collator_path(path):
        return True
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
    if path is not None and static_import_attributes_path(path):
        feats.difference_update(static_import_attributes_path_features(path))
    if path is not None and intl_canonical_locales_path(path):
        feats.difference_update(intl_canonical_locales_path_features(path))
    if path is not None and intl_supported_values_path(path):
        feats.difference_update(intl_supported_values_path_features(path))
    if path is not None and intl_locale_path(path):
        feats.difference_update(intl_locale_path_features(path))
    if path is not None and intl_collator_path(path):
        feats.difference_update(intl_collator_path_features(path))
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
    if path is not None and proxy_has_path(path):
        feats.difference_update(proxy_has_features(path))
    if path is not None and proxy_delete_path(path):
        feats.difference_update(proxy_delete_features(path))
    if path is not None and extensibility_path(path):
        feats.difference_update(extensibility_features(path))
    if path is not None and prototype_internal_path(path):
        feats.difference_update(prototype_internal_features(path))
    if path is not None and proxy_define_property_path(path):
        feats.difference_update(proxy_define_property_features(path))
    if path is not None and proxy_set_path(path):
        feats.difference_update(proxy_set_features(path))
    if path is not None and proxy_own_keys_path(path):
        feats.difference_update(proxy_own_keys_features(path))
    if path is not None and proxy_for_in_path(path):
        feats.difference_update(proxy_for_in_features(path))
    if path is not None and array_exotic_path(path):
        feats.difference_update(array_exotic_features(path))
    if path is not None and array_concat_path(path):
        feats.difference_update(array_concat_features(path))
    if path is not None and array_copy_within_path(path):
        feats.difference_update(array_copy_within_features(path))
    if path is not None and array_fill_path(path):
        feats.difference_update(array_fill_features(path))
    if path is not None and array_filter_path(path):
        feats.difference_update(array_filter_features(path))
    if path is not None and array_map_path(path):
        feats.difference_update(array_map_features(path))
    if path is not None and array_for_each_path(path):
        feats.difference_update(array_for_each_features(path))
    if path is not None and array_reduce_path(path):
        feats.difference_update(array_reduce_features(path))
    if path is not None and array_reduce_right_path(path):
        feats.difference_update(array_reduce_right_features(path))
    if path is not None and array_reverse_path(path):
        feats.difference_update(array_reverse_features(path))
    if path is not None and array_to_reversed_path(path):
        feats.difference_update(array_to_reversed_features(path))
    if path is not None and array_to_spliced_path(path):
        feats.difference_update(array_to_spliced_features(path))
    if path is not None and array_to_locale_string_path(path):
        feats.difference_update(array_to_locale_string_features(path))
    if path is not None and array_join_path(path):
        feats.difference_update(array_join_features(path))
    if path is not None and array_flat_path(path):
        feats.difference_update(array_flat_features(path))
    if path is not None and array_flat_map_path(path):
        feats.difference_update(array_flat_map_features(path))
    if path is not None and array_iterator_path(path):
        feats.difference_update(array_iterator_features(path))
    if path is not None and reflect_set_has_path(path):
        feats.difference_update(reflect_set_has_features(path))
    if path is not None and reflect_remaining_path(path):
        feats.difference_update(reflect_remaining_features(path))
    if path is not None and reflect_call_path(path):
        feats.difference_update(reflect_call_features(path))
    if path is not None and function_apply_path(path):
        feats.difference_update(function_apply_features(path))
    if path is not None and function_bind_path(path):
        feats.difference_update(function_bind_features(path))
    if path is not None and language_early_error_path(path):
        feats.difference_update(language_early_error_features(path))
    if path is not None and reference_primitive_path(path):
        feats.difference_update({"cross-realm", "Symbol", "Proxy"})
    if path is not None and object_constructor_path(path):
        feats.difference_update(object_constructor_features(path))
    if path is not None and object_from_entries_path(path):
        feats.difference_update(object_from_entries_features(path))
    if path is not None and object_group_by_path(path):
        feats.difference_update(object_group_by_features(path))
    if path is not None and map_group_by_path(path):
        feats.difference_update(map_group_by_features(path))
    if path is not None and map_constructor_path(path):
        feats.difference_update(map_constructor_features(path))
    if path is not None and set_constructor_path(path):
        feats.difference_update(set_constructor_features(path))
    if path is not None and set_algebra_path(path):
        feats.difference_update(set_algebra_features(path))
    if path is not None and weak_collection_path(path):
        feats.difference_update(weak_collection_features(path))
    if path is not None and native_construct_path(path):
        feats.difference_update(native_construct_features(path))
    if path is not None and object_prototype_path(path):
        feats.difference_update(object_prototype_features(path))
    if path is not None and promise_realm_path(path):
        feats.difference_update(promise_realm_features(path))
    if path is not None and promise_combinator_close_path(path):
        feats.difference_update(promise_combinator_close_features(path))
    if path is not None and promise_combinator_rejection_path(path):
        feats.difference_update(promise_combinator_rejection_features(path))
    if path is not None and promise_keyed_path(path):
        feats.difference_update(promise_keyed_features(path))
    if path is not None and promise_constructor_order_path(path):
        feats.difference_update(promise_constructor_order_features(path))
    if path is not None and promise_finally_path(path):
        feats.difference_update(promise_finally_features(path))
    if path is not None:
        feats.difference_update(regexp_match_indices_features(path))
        feats.difference_update(regexp_named_groups_features(path))
        feats.difference_update(regexp_duplicate_named_groups_features(path))
        feats.difference_update(regexp_unicode_sets_features(path))
        feats.difference_update(regexp_uv_flags_features(path))
        feats.difference_update(regexp_logical_utf16_features(path))
    if path is not None and generator_function_path(path):
        feats.difference_update(generator_function_features(path))
    if path is not None and async_generator_realm_path(path):
        feats.difference_update(async_generator_realm_features(path))
    if path is not None and async_iterator_dispose_path(path):
        feats.difference_update(async_iterator_dispose_features(path))
    if path is not None and async_from_sync_iterator_path(path):
        feats.difference_update(async_from_sync_iterator_features(path))
    if path is not None and array_from_async_path(path):
        feats.difference_update(array_from_async_features(path))
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
        feats.difference_update(typed_array_to_string_features(path))
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
        feats.difference_update(typed_array_join_features(path))
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
        feats.difference_update(typed_array_to_locale_string_features(path))
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
    if path is not None and decorator_path(path):
        feats.discard("decorators")
    if path is not None and iterator_core_path(path):
        feats.difference_update(ITERATOR_CORE_FEATURES)
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
    if path is not None:
        feats.difference_update(class_subclass_builtin_features(path))
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
        or async_iterator_dispose_path(path)
        or async_from_sync_iterator_path(path)
        or promise_combinator_close_path(path)
        or promise_combinator_rejection_path(path)
        or promise_keyed_path(path)
        or promise_finally_path(path)
        or array_from_async_path(path)
        or atomics_sync_path(path)
        or module_tla_runtime_path(path)
        or dynamic_import_path(path)
        or static_import_attributes_path(path)
        or import_meta_path(path)
    )
    module_admitted = path is not None and (
        module_core_path(path)
        or dynamic_import_path(path)
        or static_import_attributes_path(path)
        or import_meta_path(path)
        or extensibility_module_path(path)
        or language_early_error_module_path(path)
    )
    if ('module' in flags and not module_admitted) or (
        'async' in flags and not (RUN_ASYNC_TESTS or async_admitted)
    ):
        return True
    return False

# Harness files always loaded (the minimum test262 requires).
BASE_HARNESS = ['sta.js', 'assert.js']

def assemble_source(src, meta, strict=None):
    """Combine one parsed test with its harness for one execution variant."""
    flags = meta.get('flags', [])
    if 'raw' in flags:
        return src
    parts = []
    if strict is None:
        strict = 'onlyStrict' in flags
    # The directive must precede harness code so it governs the complete script.
    if strict:
        parts.append(STRICT_PREFIX)
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
    return "\n".join(parts)

def build_source(path, strict=None):
    """Build the full source: harness files + the test."""
    src = Path(path).read_text()
    meta = parse_meta(src)
    return assemble_source(src, meta, strict=strict), meta

def run_test(path):
    src = Path(path).read_text()
    meta = parse_meta(src)
    if should_skip(meta, path):
        return 'skip'
    timeout = test_timeout_seconds(path)
    source_path = path if "module" in meta.get("flags", []) or dynamic_import_path(path) else None
    results = []
    for label, strict in execution_variants(meta):
        full = assemble_source(src, meta, strict=strict)
        variant_status, diagnostic = execute_source(
            full, meta, RUJA, timeout=timeout, source_path=source_path
        )
        results.append((label, variant_status, diagnostic))
    status, _ = combine_variant_results(results)
    return status

def discover_test_files(base):
    """Return one requested test file or every JavaScript file below a directory."""
    if base.is_file():
        return [base] if base.suffix == ".js" else []
    return sorted(base.rglob("*.js"))

def main():
    dirs = sys.argv[1:] if len(sys.argv) > 1 else ['language/expressions']
    counts = {'pass': 0, 'fail': 0, 'skip': 0, 'timeout': 0, 'error': 0}
    total = 0
    for d in dirs:
        base = Path(TEST262) / 'test' / d
        if not base.exists():
            continue
        for f in discover_test_files(base):
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
        print(
            f"RATE=0.0 PASS={counts['pass']} FAIL={counts['fail']} "
            f"SKIP={counts['skip']} TOTAL={total} RAN=0"
        )

if __name__ == '__main__':
    main()
