#!/usr/bin/env python3
"""Regression tests for RuJa's shared test262 process support."""

import io
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import mock_open, patch

import test262_analyze
import test262_runner
import analyze_failures
from test262_class_computed_field_admission import CLASS_COMPUTED_FIELD_FILES
from test262_class_default_parameter_admission import CLASS_DEFAULT_PARAMETER_FILES
from test262_class_destructuring_admission import CLASS_DESTRUCTURING_FILES
from test262_decorator_admission import DECORATOR_FILES
from test262_class_private_admission import (
    CLASS_PRIVATE_FEATURES_BY_FILE,
    CLASS_PRIVATE_FILES,
    PRIVATE_CLASS_FEATURES,
)
from test262_class_subclass_builtin_admission import (
    CLASS_SUBCLASS_BUILTIN_FEATURES_BY_FILE,
    CLASS_SUBCLASS_BUILTIN_FILES,
)
from test262_class_public_field_admission import CLASS_PUBLIC_FIELD_FILES
from test262_date_to_primitive_admission import DATE_TO_PRIMITIVE_FILES
from test262_annex_b_string_admission import (
    ANNEX_B_STRING_FEATURES,
    ANNEX_B_STRING_FILES,
)
from test262_annex_b_escape_admission import (
    ANNEX_B_ESCAPE_FEATURES,
    ANNEX_B_ESCAPE_FILES,
)
from test262_annex_b_date_admission import (
    ANNEX_B_DATE_FEATURES,
    ANNEX_B_DATE_FILES,
)
from test262_regexp_compile_admission import (
    REGEXP_COMPILE_FEATURES,
    REGEXP_COMPILE_FILES,
)
from test262_regexp_legacy_accessors_admission import (
    REGEXP_LEGACY_ACCESSOR_FEATURES,
    REGEXP_LEGACY_ACCESSOR_FILES,
)
from test262_regexp_annex_b_admission import (
    REGEXP_ANNEX_B_FEATURES,
    REGEXP_ANNEX_B_FILES,
)
from test262_generator_function_admission import (
    GENERATOR_FUNCTION_FEATURES,
    GENERATOR_FUNCTION_FILES,
)
from test262_async_generator_realm_admission import (
    ASYNC_GENERATOR_REALM_FEATURES,
    ASYNC_GENERATOR_REALM_FILES,
)
from test262_async_iterator_dispose_admission import (
    ASYNC_ITERATOR_DISPOSE_FEATURES,
    ASYNC_ITERATOR_DISPOSE_FILES,
)
from test262_async_from_sync_iterator_admission import (
    ASYNC_FROM_SYNC_ITERATOR_FEATURES,
    ASYNC_FROM_SYNC_ITERATOR_FILES,
)
from test262_object_constructor_admission import (
    OBJECT_CONSTRUCTOR_FEATURES,
    OBJECT_CONSTRUCTOR_FILES,
)
from test262_suppressed_error_admission import (
    SUPPRESSED_ERROR_FEATURES,
    SUPPRESSED_ERROR_FILES,
)
from test262_temporal_namespace_admission import (
    TEMPORAL_NAMESPACE_FEATURES,
    TEMPORAL_NAMESPACE_FILES,
)
from test262_temporal_instant_core_admission import (
    TEMPORAL_INSTANT_CORE_FEATURES,
    TEMPORAL_INSTANT_CORE_FILES,
)
from test262_temporal_instant_compare_admission import (
    TEMPORAL_INSTANT_COMPARE_FEATURES,
    TEMPORAL_INSTANT_COMPARE_FILES,
)
from test262_temporal_instant_epoch_factories_admission import (
    TEMPORAL_INSTANT_EPOCH_FACTORY_FEATURES,
    TEMPORAL_INSTANT_EPOCH_FACTORY_FILES,
)
from test262_temporal_instant_equals_admission import (
    TEMPORAL_INSTANT_EQUALS_FEATURES,
    TEMPORAL_INSTANT_EQUALS_FILES,
)
from test262_temporal_instant_from_admission import (
    TEMPORAL_INSTANT_FROM_FEATURES,
    TEMPORAL_INSTANT_FROM_FILES,
)
from test262_temporal_instant_string_parser_admission import (
    TEMPORAL_INSTANT_STRING_PARSER_FEATURES,
    TEMPORAL_INSTANT_STRING_PARSER_FILES,
)
from test262_temporal_instant_to_string_admission import (
    TEMPORAL_INSTANT_TO_STRING_FEATURES,
    TEMPORAL_INSTANT_TO_STRING_FILES,
)
from test262_temporal_zoned_date_time_core_admission import (
    TEMPORAL_ZONED_DATE_TIME_CORE_FEATURES,
    TEMPORAL_ZONED_DATE_TIME_CORE_FILES,
)
from test262_temporal_zoned_date_time_fixed_offset_admission import (
    TEMPORAL_ZONED_DATE_TIME_FIXED_OFFSET_FEATURES,
    TEMPORAL_ZONED_DATE_TIME_FIXED_OFFSET_FILES,
)
from test262_temporal_zoned_date_time_equals_admission import (
    TEMPORAL_ZONED_DATE_TIME_EQUALS_FEATURES,
    TEMPORAL_ZONED_DATE_TIME_EQUALS_FILES,
)
from test262_temporal_zoned_date_time_compare_admission import (
    TEMPORAL_ZONED_DATE_TIME_COMPARE_FEATURES,
    TEMPORAL_ZONED_DATE_TIME_COMPARE_FILES,
)
from test262_temporal_zoned_date_time_with_time_zone_admission import (
    TEMPORAL_ZONED_DATE_TIME_WITH_TIME_ZONE_FEATURES,
    TEMPORAL_ZONED_DATE_TIME_WITH_TIME_ZONE_FILES,
)
from test262_temporal_zoned_date_time_with_calendar_admission import (
    TEMPORAL_ZONED_DATE_TIME_WITH_CALENDAR_FEATURES,
    TEMPORAL_ZONED_DATE_TIME_WITH_CALENDAR_FILES,
)
from test262_temporal_instant_value_of_admission import (
    TEMPORAL_INSTANT_VALUE_OF_FEATURES,
    TEMPORAL_INSTANT_VALUE_OF_FILES,
)
from test262_object_from_entries_admission import (
    OBJECT_FROM_ENTRIES_FEATURES,
    OBJECT_FROM_ENTRIES_FILES,
)
from test262_object_group_by_admission import (
    OBJECT_GROUP_BY_FEATURES,
    OBJECT_GROUP_BY_FILES,
)
from test262_map_group_by_admission import (
    MAP_GROUP_BY_FEATURES,
    MAP_GROUP_BY_FILES,
)
from test262_map_constructor_admission import (
    MAP_CONSTRUCTOR_FEATURES,
    MAP_CONSTRUCTOR_FILES,
)
from test262_set_constructor_admission import (
    SET_CONSTRUCTOR_FEATURES,
    SET_CONSTRUCTOR_FILES,
)
from test262_set_algebra_admission import SET_ALGEBRA_FEATURES, SET_ALGEBRA_FILES
from test262_weak_collection_admission import (
    WEAK_COLLECTION_FEATURES,
    WEAK_COLLECTION_FILES,
)
from test262_weak_reference_admission import (
    WEAK_REFERENCE_FEATURES,
    WEAK_REFERENCE_FILES,
    weak_reference_features,
)
from test262_native_construct_admission import (
    NATIVE_CONSTRUCT_FEATURES,
    NATIVE_CONSTRUCT_FILES,
)
from test262_object_prototype_admission import (
    OBJECT_PROTOTYPE_FEATURES_BY_FILE,
    OBJECT_PROTOTYPE_FILES,
)
from test262_promise_realm_admission import (
    PROMISE_REALM_FEATURES,
    PROMISE_REALM_FILES,
)
from test262_promise_combinator_close_admission import (
    PROMISE_COMBINATOR_CLOSE_FEATURES,
    PROMISE_COMBINATOR_CLOSE_FILES,
)
from test262_promise_combinator_rejection_admission import (
    PROMISE_COMBINATOR_REJECTION_FEATURES,
    PROMISE_COMBINATOR_REJECTION_FILES,
)
from test262_promise_keyed_admission import (
    PROMISE_KEYED_FEATURES,
    PROMISE_KEYED_FILES,
)
from test262_promise_constructor_order_admission import (
    PROMISE_CONSTRUCTOR_ORDER_FEATURES,
    PROMISE_CONSTRUCTOR_ORDER_FILES,
)
from test262_promise_finally_admission import (
    PROMISE_FINALLY_FEATURES,
    PROMISE_FINALLY_FILES,
)
from test262_regexp_match_indices_admission import (
    REGEXP_MATCH_INDICES_FEATURES,
    REGEXP_MATCH_INDICES_FILES,
)
from test262_regexp_named_groups_admission import (
    REGEXP_NAMED_GROUPS_FEATURES,
    REGEXP_NAMED_GROUPS_FILES,
)
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
    PROXY_DELETE_FEATURES,
    PROXY_DELETE_FILES,
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
    PROXY_OWN_KEYS_FEATURES,
    PROXY_OWN_KEYS_FILES,
)
from test262_proxy_for_in_admission import (
    PROXY_FOR_IN_FEATURES,
    PROXY_FOR_IN_FILES,
)
from test262_array_exotic_admission import (
    ARRAY_EXOTIC_FEATURES,
    ARRAY_EXOTIC_FILES,
)
from test262_array_concat_admission import (
    ARRAY_CONCAT_FEATURES,
    ARRAY_CONCAT_FILES,
)
from test262_array_copy_within_admission import (
    ARRAY_COPY_WITHIN_FEATURES,
    ARRAY_COPY_WITHIN_FILES,
)
from test262_array_fill_admission import ARRAY_FILL_FEATURES, ARRAY_FILL_FILES
from test262_array_filter_admission import ARRAY_FILTER_FEATURES, ARRAY_FILTER_FILES
from test262_array_map_admission import ARRAY_MAP_FEATURES, ARRAY_MAP_FILES
from test262_array_for_each_admission import (
    ARRAY_FOR_EACH_FEATURES,
    ARRAY_FOR_EACH_FILES,
)
from test262_array_reduce_admission import ARRAY_REDUCE_FEATURES, ARRAY_REDUCE_FILES
from test262_array_reduce_right_admission import (
    ARRAY_REDUCE_RIGHT_FEATURES,
    ARRAY_REDUCE_RIGHT_FILES,
)
from test262_array_reverse_admission import ARRAY_REVERSE_FEATURES, ARRAY_REVERSE_FILES
from test262_array_to_reversed_admission import (
    ARRAY_TO_REVERSED_FEATURES,
    ARRAY_TO_REVERSED_FILES,
)
from test262_array_to_spliced_admission import (
    ARRAY_TO_SPLICED_FEATURES,
    ARRAY_TO_SPLICED_FILES,
)
from test262_array_to_locale_string_admission import (
    ARRAY_TO_LOCALE_STRING_FEATURES,
    ARRAY_TO_LOCALE_STRING_FILES,
)
from test262_typed_array_to_locale_string_admission import (
    TYPED_ARRAY_TO_LOCALE_STRING_FEATURES,
    TYPED_ARRAY_TO_LOCALE_STRING_FILES,
)
from test262_typed_array_join_admission import (
    TYPED_ARRAY_JOIN_FEATURES,
    TYPED_ARRAY_JOIN_FILES,
)
from test262_typed_array_to_string_admission import (
    TYPED_ARRAY_TO_STRING_FEATURES,
    TYPED_ARRAY_TO_STRING_FILES,
)
from test262_array_join_admission import ARRAY_JOIN_FEATURES, ARRAY_JOIN_FILES
from test262_array_flat_admission import (
    ARRAY_FLAT_FEATURES,
    ARRAY_FLAT_FILES,
    ARRAY_FLAT_MAP_FEATURES,
    ARRAY_FLAT_MAP_FILES,
)
from test262_array_iterator_admission import (
    ARRAY_ITERATOR_FEATURES,
    ARRAY_ITERATOR_FILES,
)
from test262_reflect_set_has_admission import (
    REFLECT_SET_HAS_FEATURES,
    REFLECT_SET_HAS_FILES,
)
from test262_reflect_remaining_admission import (
    REFLECT_REMAINING_FEATURES,
    REFLECT_REMAINING_FILES,
)
from test262_reflect_call_admission import REFLECT_CALL_FEATURES, REFLECT_CALL_FILES
from test262_function_apply_admission import (
    FUNCTION_APPLY_FEATURES,
    FUNCTION_APPLY_FILES,
)
from test262_function_bind_admission import (
    FUNCTION_BIND_FEATURES,
    FUNCTION_BIND_FILES,
)
from test262_function_tostring_admission import (
    FUNCTION_TOSTRING_FEATURES,
    FUNCTION_TOSTRING_FILES,
)
from test262_shadowrealm_admission import (
    SHADOWREALM_FEATURES,
    SHADOWREALM_FILES,
    SHADOWREALM_MODULE_FILES,
)
from test262_language_early_error_admission import (
    LANGUAGE_EARLY_ERROR_FEATURES,
    LANGUAGE_EARLY_ERROR_FILES,
    LANGUAGE_EARLY_ERROR_MODULE_FILES,
)
from test262_reference_primitive_admission import (
    REFERENCE_PRIMITIVE_FEATURES,
    REFERENCE_PRIMITIVE_FILES,
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
from test262_intl_locale_admission import (
    INTL_LOCALE_BASE_FILES,
    INTL_LOCALE_FILES,
    INTL_LOCALE_INFO_FILES,
    intl_locale_features,
)
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
    MODULE_CLASS_ELEMENTS_FILES,
    MODULE_STATIC_SEMANTICS_FILES,
    MODULE_TLA_RUNTIME_FILES,
    MODULE_TLA_SYNTAX_FILES,
)
from test262_support import (
    ASYNC_COMPLETE,
    ASYNC_PRINT_SHIM,
    STRICT_PREFIX,
    combine_variant_results,
    execute_source,
    execution_variants,
)


class AsyncResultTests(unittest.TestCase):
    def execute(self, stdout="", stderr="", returncode=0):
        result = subprocess.CompletedProcess(
            ["ruja", "test.js"], returncode, stdout=stdout, stderr=stderr
        )
        with patch("test262_support.subprocess.run", return_value=result):
            return execute_source("", {"flags": ["async"]}, "ruja")

    def test_accepts_single_completion_marker(self):
        self.assertEqual(self.execute(stdout=f"{ASYNC_COMPLETE}\n"), ("pass", ""))

    def test_rejects_failure_marker(self):
        status, error = self.execute(
            stdout="Test262:AsyncTestFailure:Test262Error: failed\n"
        )
        self.assertEqual(status, "fail")
        self.assertIn("AsyncTestFailure", error)

    def test_rejects_missing_completion_marker(self):
        self.assertEqual(
            self.execute(),
            ("fail", "Test262 async completion marker missing"),
        )

    def test_rejects_process_error(self):
        self.assertEqual(
            self.execute(stderr="TypeError: failed\n", returncode=1),
            ("fail", "TypeError: failed"),
        )

    def test_reports_timeout(self):
        with patch(
            "test262_support.subprocess.run",
            side_effect=subprocess.TimeoutExpired("ruja", 8),
        ):
            self.assertEqual(
                execute_source("", {"flags": ["async"]}, "ruja"),
                ("timeout", ""),
            )

    def test_rejects_duplicate_or_unexpected_output(self):
        self.assertEqual(
            self.execute(stdout=f"{ASYNC_COMPLETE}\n{ASYNC_COMPLETE}\n"),
            ("fail", "Test262 async completion marker repeated"),
        )
        status, error = self.execute(stdout=f"debug\n{ASYNC_COMPLETE}\n")
        self.assertEqual(status, "fail")
        self.assertIn("debug", error)


class CanBlockEnvironmentTests(unittest.TestCase):
    def test_can_block_flags_are_forwarded_to_ruja(self):
        result = subprocess.CompletedProcess(
            ["ruja", "test.js"], 0, stdout="", stderr=""
        )
        for flag, expected in (("CanBlockIsTrue", "1"), ("CanBlockIsFalse", "0")):
            with patch(
                "test262_support.subprocess.run", return_value=result
            ) as run_process:
                self.assertEqual(
                    execute_source("", {"flags": [flag]}, "ruja"),
                    ("pass", ""),
                )
                self.assertEqual(
                    run_process.call_args.kwargs["env"]["RUJA_AGENT_CAN_BLOCK"],
                    expected,
                )


class UnsupportedFeatureTests(unittest.TestCase):
    def test_is_htmldda_is_skipped_by_runner_and_analyzer(self):
        meta = {"features": ["class", "IsHTMLDDA"]}
        path = Path("test/annexB/language/statements/class/is-htmldda.js")

        for tool in (test262_runner, test262_analyze):
            self.assertTrue(tool.should_skip(meta, path))


class ModuleStagingTests(unittest.TestCase):
    def test_module_graph_is_staged_without_writing_to_the_source_tree(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            source_dir = Path(temp_dir)
            entry = source_dir / "entry.js"
            fixture = source_dir / "dependency_FIXTURE.js"
            entry.write_text("import './dependency_FIXTURE.js';")
            fixture.write_text("export const value = 1;")
            before = set(source_dir.iterdir())
            observed = {}

            def run_process(command, **kwargs):
                staged_entry = Path(command[-1])
                observed["parent"] = staged_entry.parent
                observed["fixture_exists"] = (
                    staged_entry.parent / "dependency_FIXTURE.js"
                ).exists()
                return subprocess.CompletedProcess(
                    command, 0, stdout="", stderr=""
                )

            with patch(
                "test262_support.subprocess.run", side_effect=run_process
            ):
                self.assertEqual(
                    execute_source(
                        "import './dependency_FIXTURE.js';",
                        {"flags": ["module"]},
                        "ruja",
                        source_path=entry,
                    ),
                    ("pass", ""),
                )
                self.assertNotEqual(observed["parent"], source_dir)
                self.assertTrue(observed["fixture_exists"])
            self.assertEqual(set(source_dir.iterdir()), before)

    def test_negative_phases_select_distinct_cli_paths(self):
        result = subprocess.CompletedProcess(
            ["ruja", "test.js"], 1, stdout="", stderr="SyntaxError"
        )
        cases = [
            ({"phase": "parse", "type": "SyntaxError"}, "--parse"),
            (
                {"phase": "parse", "type": "SyntaxError"},
                "--module-parse",
                ["module"],
            ),
            (
                {"phase": "resolution", "type": "SyntaxError"},
                "--module-link",
                ["module"],
            ),
            ({"phase": "runtime", "type": "SyntaxError"}, "--module", ["module"]),
        ]
        for case in cases:
            negative, expected = case[:2]
            flags = case[2] if len(case) == 3 else []
            with patch(
                "test262_support.subprocess.run", return_value=result
            ) as run_process:
                execute_source("", {"negative": negative, "flags": flags}, "ruja")
                self.assertIn(expected, run_process.call_args.args[0])


class HarnessAssemblyTests(unittest.TestCase):
    def test_async_completion_harness_preserves_line_endings(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            harness = root / "harness"
            harness.mkdir()
            (harness / "sta.js").write_text("/* STA HARNESS */")
            (harness / "assert.js").write_text("/* ASSERT HARNESS */")
            done = "/* DONE\r\nHARNESS */"
            (harness / "doneprintHandle.js").write_bytes(done.encode("utf-8"))
            test = root / "test.js"
            test.write_text("/*---\nflags: [async]\n---*/\n$DONE();\n")

            for tool in (test262_runner, test262_analyze):
                with (
                    self.subTest(tool=tool.__name__),
                    patch.object(tool, "HARNESS", harness),
                ):
                    source, _ = tool.build_source(test)
                self.assertIn(done, source)

    def test_sync_tests_receive_the_host_print_binding_in_both_tools(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            harness = root / "harness"
            harness.mkdir()
            (harness / "sta.js").write_text("/* STA HARNESS */")
            (harness / "assert.js").write_text("/* ASSERT HARNESS */")
            test = root / "test.js"
            test.write_text("/*---\n---*/\nArray.print = print;\n")

            for tool in (test262_runner, test262_analyze):
                original = tool.HARNESS
                tool.HARNESS = harness
                try:
                    source, meta = tool.build_source(test)
                finally:
                    tool.HARNESS = original
                self.assertNotIn("async", meta.get("flags", []))
                self.assertIn(ASYNC_PRINT_SHIM, source)

    def test_strict_directive_precedes_async_harness_in_both_tools(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            harness = root / "harness"
            harness.mkdir()
            (harness / "sta.js").write_text("/* STA HARNESS */")
            (harness / "assert.js").write_text("/* ASSERT HARNESS */")
            (harness / "doneprintHandle.js").write_text("/* DONE HARNESS */")
            test = root / "test.js"
            test.write_text(
                "/*---\nflags: [onlyStrict, async]\n---*/\n$DONE();\n"
            )

            for tool in (test262_runner, test262_analyze):
                original = tool.HARNESS
                tool.HARNESS = harness
                try:
                    source, meta = tool.build_source(test)
                finally:
                    tool.HARNESS = original
                self.assertIn("async", meta["flags"])
                self.assertTrue(source.startswith(STRICT_PREFIX))
                positions = [
                    source.index("/* STA HARNESS */"),
                    source.index("/* ASSERT HARNESS */"),
                    source.index(ASYNC_PRINT_SHIM),
                    source.index("/* DONE HARNESS */"),
                    source.index("$DONE();"),
                ]
                self.assertEqual(positions, sorted(positions))


class ExecutionVariantTests(unittest.TestCase):
    @staticmethod
    def make_harness(root):
        harness = root / "harness"
        harness.mkdir(exist_ok=True)
        return harness

    def test_flags_select_the_required_test262_variants(self):
        cases = [
            ({}, (("sloppy", False), ("strict", True))),
            ({"flags": ["onlyStrict"]}, (("strict", True),)),
            ({"flags": ["noStrict"]}, (("sloppy", False),)),
            ({"flags": ["raw"]}, (("raw", False),)),
            ({"flags": ["module"]}, (("module", False),)),
        ]
        for meta, expected in cases:
            with self.subTest(meta=meta):
                self.assertEqual(execution_variants(meta), expected)

    def test_variant_results_preserve_file_count_and_label_failures(self):
        self.assertEqual(
            combine_variant_results(
                [("sloppy", "pass", ""), ("strict", "fail", "SyntaxError")]
            ),
            ("fail", "[strict] SyntaxError"),
        )
        status, diagnostic = combine_variant_results(
            [("sloppy", "error", "spawn failed"), ("strict", "timeout", "")]
        )
        self.assertEqual(status, "error")
        self.assertEqual(diagnostic, "[sloppy] spawn failed\n[strict] timeout")

    def test_all_file_status_precedence_combinations(self):
        precedence = {"pass": 0, "fail": 1, "timeout": 2, "error": 3}
        for left in precedence:
            for right in precedence:
                with self.subTest(left=left, right=right):
                    status, _ = combine_variant_results(
                        [("sloppy", left, left), ("strict", right, right)]
                    )
                    expected = max((left, right), key=precedence.get)
                    self.assertEqual(status, expected)

    def test_runner_and_analyzer_execute_default_tests_twice(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            harness = self.make_harness(root)
            test = root / "default.js"
            test.write_text("/*---\n---*/\nvar value = 1;\n")
            for tool in (test262_runner, test262_analyze):
                sources = []

                def execute(source, *args, **kwargs):
                    sources.append(source)
                    return "pass", ""

                with (
                    patch.object(tool, "HARNESS", harness),
                    patch.object(tool, "should_skip", return_value=False),
                    patch.object(tool, "execute_source", side_effect=execute),
                ):
                    result = tool.run_test(test)
                expected = "pass" if tool is test262_runner else ("pass", "")
                self.assertEqual(result, expected)
                self.assertEqual(len(sources), 2)
                self.assertFalse(sources[0].startswith(STRICT_PREFIX))
                self.assertTrue(sources[1].startswith(STRICT_PREFIX))

    def test_runner_and_analyzer_preserve_source_line_endings(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            test = Path(temp_dir) / "raw.js"
            expected = "/*---\r\nflags: [raw]\r\n---*/\r\nfunction\rf() {}\r"
            test.write_bytes(expected.encode("utf-8"))
            for tool in (test262_runner, test262_analyze):
                with self.subTest(tool=tool.__name__):
                    source, meta = tool.build_source(test)
                    self.assertEqual(meta.get("flags"), ["raw"])
                    self.assertEqual(source, expected)

    def test_each_default_variant_receives_the_full_timeout(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            harness = self.make_harness(root)
            test = root / "default.js"
            test.write_text("/*---\n---*/\n1;\n")
            for tool in (test262_runner, test262_analyze):
                with (
                    patch.object(tool, "HARNESS", harness),
                    patch.object(tool, "should_skip", return_value=False),
                    patch.object(tool, "test_timeout_seconds", return_value=37),
                    patch.object(
                        tool, "execute_source", return_value=("pass", "")
                    ) as execute,
                ):
                    tool.run_test(test)
                self.assertEqual(execute.call_count, 2)
                self.assertEqual(
                    [call.kwargs["timeout"] for call in execute.call_args_list],
                    [37, 37],
                )

    def test_single_variant_flags_execute_once(self):
        cases = [
            ("onlyStrict", True),
            ("noStrict", False),
            ("raw", False),
            ("module", False),
        ]
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            harness = self.make_harness(root)
            for flag, strict in cases:
                test = root / f"{flag}.js"
                test.write_text(f"/*---\nflags: [{flag}]\n---*/\nvar value = 1;\n")
                for tool in (test262_runner, test262_analyze):
                    with (
                        self.subTest(tool=tool.__name__, flag=flag),
                        patch.object(tool, "HARNESS", harness),
                        patch.object(tool, "should_skip", return_value=False),
                        patch.object(
                            tool, "execute_source", return_value=("pass", "")
                        ) as execute,
                    ):
                        tool.run_test(test)
                    self.assertEqual(execute.call_count, 1)
                    source = execute.call_args.args[0]
                    self.assertEqual(source.startswith(STRICT_PREFIX), strict)
                    if flag == "raw":
                        self.assertEqual(source, test.read_text())

    def test_module_raw_is_unmodified_and_runs_as_a_module_once(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            harness = self.make_harness(root)
            test = root / "module-raw.js"
            test.write_text("/*---\nflags: [module, raw]\n---*/\nexport {};\n")
            for tool in (test262_runner, test262_analyze):
                with (
                    patch.object(tool, "HARNESS", harness),
                    patch.object(tool, "should_skip", return_value=False),
                    patch.object(
                        tool, "execute_source", return_value=("pass", "")
                    ) as execute,
                ):
                    tool.run_test(test)
                self.assertEqual(execute.call_count, 1)
                self.assertEqual(execute.call_args.args[0], test.read_text())
                self.assertEqual(execute.call_args.kwargs["source_path"], test)

    def test_runner_main_counts_a_dual_execution_as_one_file(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            test_dir = root / "test/language/example"
            test_dir.mkdir(parents=True)
            test = test_dir / "default.js"
            test.write_text("/*---\n---*/\n1;\n")
            output = io.StringIO()
            with (
                patch.object(test262_runner, "TEST262", str(root)),
                patch.object(test262_runner, "run_test", return_value="pass"),
                patch.object(sys, "argv", ["test262_runner.py", "language/example"]),
                redirect_stdout(output),
            ):
                test262_runner.main()
            self.assertIn("PASS=1 FAIL=0 SKIP=0 TOTAL=1 RAN=1", output.getvalue())

    def test_analyzer_main_retains_timeout_and_error_files(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            test_dir = root / "test/language/example"
            test_dir.mkdir(parents=True)
            timeout_test = test_dir / "timeout.js"
            error_test = test_dir / "error.js"
            timeout_test.write_text("/*---\n---*/\n1;\n")
            error_test.write_text("/*---\n---*/\n1;\n")
            dumped = {}

            def capture_dump(value, *args, **kwargs):
                dumped.update(value)

            with (
                patch.object(test262_analyze, "TEST262", str(root)),
                patch.object(
                    test262_analyze,
                    "run_test",
                    side_effect=[
                        ("timeout", "[strict] timeout"),
                        ("error", "[sloppy] spawn failed"),
                    ],
                ),
                patch.object(
                    sys, "argv", ["test262_analyze.py", "language/example"]
                ),
                patch("builtins.open", mock_open()),
                patch.object(test262_analyze.json, "dump", side_effect=capture_dump),
                redirect_stdout(io.StringIO()),
            ):
                test262_analyze.main()
            retained = [item for values in dumped.values() for item in values]
            self.assertEqual(len(retained), 2)
            self.assertEqual(
                {Path(path).name for path, _ in retained},
                {"timeout.js", "error.js"},
            )


class AsyncAdmissionTests(unittest.TestCase):
    def test_async_flag_is_admitted_only_inside_exact_paths(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            object_method = (
                root
                / "test/language/expressions/object/method-definition/case.js"
            )
            async_arrow = (
                root / "test/language/expressions/async-arrow-function/case.js"
            )
            async_function_expression = (
                root / "test/language/expressions/async-function/case.js"
            )
            async_function_declaration = (
                root / "test/language/statements/async-function/case.js"
            )
            await_expression = root / "test/language/expressions/await/case.js"
            for_await_of = root / "test/language/statements/for-await-of/case.js"
            async_generator_expression = (
                root / "test/language/expressions/async-generator/case.js"
            )
            async_generator_declaration = (
                root / "test/language/statements/async-generator/case.js"
            )
            class_element_expression = (
                root / "test/language/expressions/class/elements/case.js"
            )
            class_element_declaration = (
                root / "test/language/statements/class/elements/case.js"
            )
            optional_chaining = (
                root / "test/language/expressions/optional-chaining/case.js"
            )
            class_definition = (
                root / "test/language/statements/class/definition/case.js"
            )
            outside = root / "test/language/expressions/async-arrow/case.js"
            meta = {"flags": ["async"], "features": []}
            feature_meta = {
                "flags": [],
                "features": ["async-functions", "default-parameters"],
            }
            async_generator_meta = {
                "flags": ["async"],
                "features": [
                    "async-functions",
                    "async-iteration",
                    "generators",
                    "Symbol.asyncIterator",
                ],
            }
            await_meta = {
                "flags": ["async"],
                "features": ["async-functions", "async-iteration", "generators"],
            }
            for_await_meta = {
                "flags": ["async"],
                "features": ["async-iteration", "Symbol.asyncIterator"],
            }
            class_element_meta = {
                "flags": ["async"],
                "features": [
                    "async-functions",
                    "class-methods-private",
                    "destructuring-binding",
                    "optional-chaining",
                    "Symbol",
                    "Symbol.asyncIterator",
                    "Symbol.iterator",
                ],
            }
            optional_chaining_meta = {
                "flags": ["async"],
                "features": ["optional-chaining"],
            }

            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                original_run_async = tool.RUN_ASYNC_TESTS
                tool.TEST262 = str(root)
                tool.RUN_ASYNC_TESTS = False
                try:
                    self.assertFalse(tool.should_skip(meta, object_method))
                    self.assertFalse(tool.should_skip(meta, async_arrow))
                    self.assertFalse(
                        tool.should_skip(meta, async_function_expression)
                    )
                    self.assertFalse(
                        tool.should_skip(meta, async_function_declaration)
                    )
                    self.assertFalse(tool.should_skip(await_meta, await_expression))
                    self.assertFalse(tool.should_skip(for_await_meta, for_await_of))
                    self.assertFalse(
                        tool.should_skip(async_generator_meta, async_generator_expression)
                    )
                    self.assertFalse(
                        tool.should_skip(async_generator_meta, async_generator_declaration)
                    )
                    self.assertFalse(
                        tool.should_skip(class_element_meta, class_element_expression)
                    )
                    self.assertFalse(
                        tool.should_skip(class_element_meta, class_element_declaration)
                    )
                    self.assertFalse(
                        tool.should_skip(optional_chaining_meta, optional_chaining)
                    )
                    self.assertFalse(tool.should_skip(meta, class_definition))
                    self.assertTrue(tool.should_skip(meta, outside))
                    self.assertTrue(tool.should_skip(async_generator_meta, outside))
                    self.assertTrue(tool.should_skip(class_element_meta, outside))
                    self.assertTrue(tool.should_skip(optional_chaining_meta, outside))
                    self.assertFalse(
                        tool.should_skip(feature_meta, async_function_expression)
                    )
                    self.assertFalse(
                        tool.should_skip(feature_meta, async_function_declaration)
                    )
                    self.assertTrue(tool.should_skip(feature_meta, outside))
                finally:
                    tool.TEST262 = original_root
                    tool.RUN_ASYNC_TESTS = original_run_async


class WeakRefAdmissionTests(unittest.TestCase):
    def test_weak_ref_features_are_admitted_only_for_exact_builtin_paths(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = (
                root
                / "test/built-ins/WeakRef/prototype/deref/return-object-target.js"
            )
            future = root / "test/built-ins/WeakRef/prototype/deref/future.js"
            outside = root / "test/built-ins/Other/case.js"
            meta = {
                "flags": [],
                "features": [
                    "WeakRef",
                    "Reflect",
                    "Reflect.construct",
                    "Symbol",
                    "Symbol.toStringTag",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, future))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root


class ModuleCoreAdmissionTests(unittest.TestCase):
    def test_language_early_error_admission_is_exact_and_shared(self):
        expected = {
            "language/statements/labeled/decl-gen.js": frozenset({"generators"}),
            "language/statements/labeled/decl-async-function.js": frozenset(
                {"async-functions"}
            ),
            "language/statements/labeled/decl-async-generator.js": frozenset(
                {"async-iteration"}
            ),
            "language/expressions/class/class-name-ident-await-escaped-module.js": frozenset(),
            "language/statements/class/class-name-ident-await-escaped-module.js": frozenset(),
            "language/expressions/prefix-increment/target-cover-yieldexpr.js": frozenset(
                {"generators"}
            ),
            "language/expressions/postfix-increment/target-cover-yieldexpr.js": frozenset(
                {"generators"}
            ),
            "language/expressions/prefix-decrement/target-cover-yieldexpr.js": frozenset(
                {"generators"}
            ),
            "language/expressions/postfix-decrement/target-cover-yieldexpr.js": frozenset(
                {"generators"}
            ),
        }
        expected_modules = frozenset(
            {
                "language/expressions/class/class-name-ident-await-escaped-module.js",
                "language/statements/class/class-name-ident-await-escaped-module.js",
            }
        )
        manifest = Path(__file__).with_name(
            "test262_language_early_error_admission.txt"
        )
        manifest_entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(manifest_entries, tuple(expected))
        manifest_files = frozenset(manifest_entries)
        self.assertEqual(manifest_files, frozenset(expected))
        self.assertEqual(LANGUAGE_EARLY_ERROR_FILES, manifest_files)
        self.assertEqual(LANGUAGE_EARLY_ERROR_FEATURES, expected)
        self.assertEqual(LANGUAGE_EARLY_ERROR_MODULE_FILES, expected_modules)

        admission_dir = Path(__file__).resolve().parent
        for other_manifest in admission_dir.glob("test262_*_admission.txt"):
            if other_manifest.name == "test262_language_early_error_admission.txt":
                continue
            other_files = {
                line
                for raw_line in other_manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                LANGUAGE_EARLY_ERROR_FILES.isdisjoint(other_files),
                other_manifest.name,
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("includes", []), [], relative)
                self.assertEqual(
                    metadata.get("flags", []),
                    ["module"] if relative in expected_modules else [],
                    relative,
                )
                self.assertEqual(
                    metadata.get("negative"),
                    {"phase": "parse", "type": "SyntaxError"},
                    relative,
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future_siblings = (
                root / "test/language/statements/labeled/decl-gen-future.js",
                root
                / "test/language/expressions/class/"
                "class-name-ident-await-escaped-module-future.js",
                root
                / "test/language/expressions/prefix-increment/"
                "target-cover-yieldexpr-future.js",
                root
                / "test/language/expressions/postfix-increment/"
                "target-cover-yieldexpr-future.js",
                root
                / "test/language/expressions/prefix-decrement/"
                "target-cover-yieldexpr-future.js",
                root
                / "test/language/expressions/postfix-decrement/"
                "target-cover-yieldexpr-future.js",
            )
            outside = root / "test/language/statements/if/decl-gen.js"
            for tool in (test262_runner, test262_analyze):
                self.assertIs(
                    tool.LANGUAGE_EARLY_ERROR_FILES, LANGUAGE_EARLY_ERROR_FILES
                )
                self.assertIs(
                    tool.LANGUAGE_EARLY_ERROR_FEATURES,
                    LANGUAGE_EARLY_ERROR_FEATURES,
                )
                self.assertIs(
                    tool.LANGUAGE_EARLY_ERROR_MODULE_FILES,
                    LANGUAGE_EARLY_ERROR_MODULE_FILES,
                )
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        is_module = relative in expected_modules
                        flags = ["module"] if is_module else []
                        self.assertTrue(tool.language_early_error_path(path), relative)
                        self.assertEqual(
                            tool.language_early_error_features(path), features, relative
                        )
                        self.assertEqual(
                            tool.language_early_error_module_path(path),
                            is_module,
                            relative,
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"flags": flags, "features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": flags,
                                    "features": sorted(features | {"decorators"}),
                                },
                                path,
                            ),
                            relative,
                        )
                        if not is_module:
                            self.assertTrue(
                                tool.should_skip(
                                    {
                                        "flags": ["module"],
                                        "features": sorted(features),
                                    },
                                    path,
                                ),
                                relative,
                            )

                    rejected = future_siblings + (outside, root / "outside.js")
                    for path in rejected:
                        self.assertFalse(tool.language_early_error_path(path))
                        self.assertEqual(
                            tool.language_early_error_features(path), frozenset()
                        )
                        self.assertFalse(tool.language_early_error_module_path(path))
                    self.assertTrue(
                        tool.should_skip(
                            {"flags": [], "features": ["generators"]},
                            future_siblings[0],
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"flags": ["module"], "features": []},
                            future_siblings[1],
                        )
                    )
                    for future_update in future_siblings[2:]:
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["generators"]},
                                future_update,
                            )
                        )
                    self.assertTrue(
                        tool.should_skip(
                            {"flags": [], "features": ["generators"]}, outside
                        )
                    )
                    for invalid in (None, object()):
                        self.assertFalse(tool.language_early_error_path(invalid))
                        self.assertEqual(
                            tool.language_early_error_features(invalid), frozenset()
                        )
                        self.assertFalse(tool.language_early_error_module_path(invalid))
                finally:
                    tool.TEST262 = original_root

    def test_module_core_is_frozen_to_the_audited_files(self):
        expected_names = {
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
        }
        expected_names.update({
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
        })
        expected = {f"language/module-code/{name}" for name in expected_names}
        expected.update(MODULE_CLASS_ELEMENTS_FILES)
        expected.update(MODULE_STATIC_SEMANTICS_FILES)
        expected.update(MODULE_TLA_SYNTAX_FILES)
        expected.update(MODULE_TLA_RUNTIME_FILES)
        meta = {"flags": ["module"], "features": []}
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/language/module-code/future.js"
            outside = root / "test/language/statements/future.js"
            for tool in (test262_runner, test262_analyze):
                self.assertEqual(tool.MODULE_CORE_FILES, expected)
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative in expected:
                        self.assertFalse(tool.should_skip(meta, root / "test" / relative))
                    namespace_meta = {
                        "flags": ["module"],
                        "features": [
                            "Symbol", "Symbol.iterator", "Symbol.toStringTag", "Reflect",
                            "export-star-as-namespace-from-module",
                        ],
                    }
                    namespace = root / "test/language/module-code/namespace/Symbol.iterator.js"
                    self.assertFalse(tool.should_skip(namespace_meta, namespace))
                    self.assertTrue(tool.should_skip(meta, future))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_module_class_elements_manifest_is_exact_and_shared(self):
        admitted = (
            "language/expressions/class/elements/"
            "class-name-static-initializer-default-export.js"
        )
        self.assertEqual(MODULE_CLASS_ELEMENTS_FILES, frozenset({admitted}))
        meta = {
            "flags": ["module"],
            "features": ["class-static-fields-public"],
        }
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            path = test_root / admitted
            self.assertTrue(path.is_file())
            live_meta = test262_runner.parse_meta(path.read_text())
            self.assertEqual(live_meta.get("flags", []), meta["flags"])
            self.assertEqual(live_meta.get("features", []), meta["features"])
            self.assertEqual(live_meta.get("includes", []), [])
            self.assertNotIn("negative", live_meta)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            path = root / "test" / admitted
            future = path.with_name(
                "class-name-static-initializer-default-export-future.js"
            )
            outside = root / "test/language/statements/class/elements" / path.name
            for tool in (test262_runner, test262_analyze):
                self.assertIs(
                    tool.MODULE_CLASS_ELEMENTS_FILES, MODULE_CLASS_ELEMENTS_FILES
                )
                self.assertTrue(MODULE_CLASS_ELEMENTS_FILES <= tool.MODULE_CORE_FILES)
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, path))
                    self.assertTrue(tool.should_skip(meta, future))
                    self.assertTrue(tool.should_skip(meta, outside))
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "flags": meta["flags"],
                                "features": meta["features"] + ["decorators"],
                            },
                            path,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

    def test_module_class_elements_live_metadata_tolerates_unavailable_root(self):
        with patch("pathlib.Path.is_dir", side_effect=PermissionError):
            self.test_module_class_elements_manifest_is_exact_and_shared()

    def test_module_static_semantics_manifest_is_exact_and_shared(self):
        self.assertEqual(len(MODULE_STATIC_SEMANTICS_FILES), 125)
        self.assertIn(
            "language/module-code/export-expname-binding-string.js",
            MODULE_STATIC_SEMANTICS_FILES,
        )
        self.assertIn(
            "language/module-code/parse-err-decl-pos-import-while.js",
            MODULE_STATIC_SEMANTICS_FILES,
        )
        malformed_upstream = (
            "language/module-code/ambiguous-export-bindings/"
            "namespace-unambiguous-if-export-star-as-from-and-import-star-as-and-export.js"
        )
        self.assertNotIn(malformed_upstream, MODULE_STATIC_SEMANTICS_FILES)
        for tool in (test262_runner, test262_analyze):
            self.assertTrue(MODULE_STATIC_SEMANTICS_FILES <= tool.MODULE_CORE_FILES)

    def test_module_tla_syntax_manifest_is_exact_and_shared(self):
        self.assertEqual(len(MODULE_TLA_SYNTAX_FILES), 213)
        admitted = (
            "language/module-code/top-level-await/syntax/"
            "top-level-await-expr-literal-number.js"
        )
        dynamic = "language/module-code/top-level-await/syntax/await-expr-dyn-import.js"
        dynamic_parameter = "language/module-code/top-level-await/syntax/catch-parameter.js"
        self.assertIn(admitted, MODULE_TLA_SYNTAX_FILES)
        self.assertNotIn(dynamic, MODULE_TLA_SYNTAX_FILES)
        self.assertNotIn(dynamic_parameter, MODULE_TLA_SYNTAX_FILES)
        self.assertIn(
            "language/module-code/top-level-await/no-operand.js",
            MODULE_TLA_SYNTAX_FILES,
        )
        self.assertIn(
            "language/module-code/top-level-await/new-await-script-code.js",
            MODULE_TLA_SYNTAX_FILES,
        )
        meta = {"flags": ["module"], "features": ["top-level-await"]}
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            path = root / "test" / admitted
            for tool in (test262_runner, test262_analyze):
                self.assertTrue(MODULE_TLA_SYNTAX_FILES <= tool.MODULE_CORE_FILES)
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, path))
                finally:
                    tool.TEST262 = original_root

    def test_module_tla_runtime_manifest_is_exact_and_shared(self):
        self.assertEqual(len(MODULE_TLA_RUNTIME_FILES), 27)
        sibling = (
            "language/module-code/top-level-await/"
            "async-module-does-not-block-sibling-modules.js"
        )
        dynamic = "language/module-code/top-level-await/dynamic-import-resolution.js"
        self.assertIn(sibling, MODULE_TLA_RUNTIME_FILES)
        self.assertNotIn(dynamic, MODULE_TLA_RUNTIME_FILES)
        self.assertNotIn(
            "language/module-code/top-level-await/no-operand.js",
            MODULE_TLA_RUNTIME_FILES,
        )
        self.assertNotIn(
            "language/module-code/top-level-await/new-await-script-code.js",
            MODULE_TLA_RUNTIME_FILES,
        )
        meta = {"flags": ["module", "async"], "features": ["top-level-await"]}
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            path = root / "test" / sibling
            for tool in (test262_runner, test262_analyze):
                self.assertTrue(MODULE_TLA_RUNTIME_FILES <= tool.MODULE_CORE_FILES)
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, path))
                finally:
                    tool.TEST262 = original_root

    def test_dynamic_import_manifest_is_exact_and_shared(self):
        self.assertEqual(len(DYNAMIC_IMPORT_FILES), 621)
        admitted = (
            "language/expressions/dynamic-import/usage/"
            "top-level-import-then-returns-thenable.js"
        )
        module_admitted = (
            "language/expressions/dynamic-import/"
            "reuse-namespace-object-from-import.js"
        )
        evaluation_rejection_admitted = (
            "language/expressions/dynamic-import/catch/"
            "nested-async-gen-return-await-eval-rqstd-abrupt-urierror.js"
        )
        missing_module_admitted = (
            "language/expressions/dynamic-import/catch/"
            "nested-while-import-catch-file-does-not-exist.js"
        )
        coercion_rejection_admitted = (
            "language/expressions/dynamic-import/catch/"
            "nested-async-function-specifier-tostring-abrupt-rejects.js"
        )
        assignment_expression_admitted = (
            "language/expressions/dynamic-import/assignment-expression/"
            "yield-star.js"
        )
        root_once_admitted = (
            "language/expressions/dynamic-import/eval-rqstd-once.js"
        )
        root_namespace_admitted = (
            "language/expressions/dynamic-import/reuse-namespace-object.js"
        )
        usage_live_binding_admitted = (
            "language/expressions/dynamic-import/usage/"
            "nested-async-gen-return-await-eval-gtbndng-indirect-update.js"
        )
        usage_host_resolution_admitted = (
            "language/expressions/dynamic-import/usage/"
            "nested-while-import-then-eval-script-code-host-resolves-module-code.js"
        )
        namespace_own_keys_admitted = (
            "language/expressions/dynamic-import/namespace/"
            "await-ns-own-property-keys-sort.js"
        )
        namespace_define_admitted = (
            "language/expressions/dynamic-import/namespace/"
            "promise-then-ns-define-own-property.js"
        )
        syntax_new_invalid_admitted = (
            "language/expressions/dynamic-import/syntax/invalid/"
            "top-level-no-new-call-expression.js"
        )
        syntax_new_covered_admitted = (
            "language/expressions/dynamic-import/syntax/valid/"
            "new-covered-expression-is-valid.js"
        )
        syntax_attributes_admitted = (
            "language/expressions/dynamic-import/syntax/valid/"
            "top-level-import-attributes-trailing-comma-second.js"
        )
        runtime_attributes_admitted = (
            "language/expressions/dynamic-import/import-attributes/"
            "2nd-param-with-enumeration-enumerable.js"
        )
        root_tla_cycle_admitted = (
            "language/expressions/dynamic-import/"
            "import-fulfilled-member-of-errored-cycle.js"
        )
        outside = (
            "language/expressions/dynamic-import/catch/"
            "top-level-import-catch-import-source-specifier-tostring.js"
        )
        self.assertIn(admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(module_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(evaluation_rejection_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(missing_module_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(coercion_rejection_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(assignment_expression_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(root_once_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(root_namespace_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(usage_live_binding_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(usage_host_resolution_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(namespace_own_keys_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(namespace_define_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(syntax_new_invalid_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(syntax_new_covered_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(syntax_attributes_admitted, DYNAMIC_IMPORT_FILES)
        self.assertIn(runtime_attributes_admitted, DYNAMIC_IMPORT_FILES)
        self.assertNotIn(outside, DYNAMIC_IMPORT_FILES)
        meta = {"flags": ["generated", "async"], "features": ["dynamic-import"]}
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            path = root / "test" / admitted
            module_path = root / "test" / module_admitted
            root_tla_cycle_path = root / "test" / root_tla_cycle_admitted
            namespace_define_path = root / "test" / namespace_define_admitted
            syntax_attributes_path = root / "test" / syntax_attributes_admitted
            runtime_attributes_path = root / "test" / runtime_attributes_admitted
            outside_path = root / "test" / outside
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, path))
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": ["module", "async"], "features": ["dynamic-import"]},
                            module_path,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {
                                "flags": ["async"],
                                "features": ["dynamic-import", "top-level-await"],
                            },
                            root_tla_cycle_path,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {
                                "flags": ["async"],
                                "features": [
                                    "dynamic-import",
                                    "Symbol",
                                    "Symbol.iterator",
                                    "Symbol.toStringTag",
                                    "Reflect",
                                ],
                            },
                            namespace_define_path,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {
                                "flags": ["generated"],
                                "features": ["dynamic-import", "import-attributes"],
                            },
                            syntax_attributes_path,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {
                                "flags": ["async"],
                                "features": [
                                    "dynamic-import",
                                    "import-attributes",
                                    "json-modules",
                                    "Symbol",
                                    "Proxy",
                                ],
                            },
                            runtime_attributes_path,
                        )
                    )
                    self.assertTrue(tool.should_skip(meta, outside_path))
                    self.assertTrue(tool.dynamic_import_path(path))
                finally:
                    tool.TEST262 = original_root

    def test_static_import_attributes_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(STATIC_IMPORT_ATTRIBUTES_FILES), 30)
        syntax = (
            "language/module-code/import-attributes/"
            "early-dup-attribute-key-export.js"
        )
        runtime = (
            "language/import/import-attributes/json-idempotency.js"
        )
        text = "language/import/import-attributes/text-self.js"
        outside = "language/import/import-attributes/unknown-future-test.js"
        self.assertIn(syntax, STATIC_IMPORT_ATTRIBUTES_FILES)
        self.assertIn(runtime, STATIC_IMPORT_ATTRIBUTES_FILES)
        self.assertIn(text, STATIC_IMPORT_ATTRIBUTES_FILES)
        self.assertNotIn(outside, STATIC_IMPORT_ATTRIBUTES_FILES)
        self.assertEqual(
            static_import_attributes_features(syntax),
            frozenset({"import-attributes"}),
        )
        self.assertEqual(
            static_import_attributes_features(runtime),
            frozenset({"import-attributes", "json-modules", "dynamic-import"}),
        )
        self.assertEqual(
            static_import_attributes_features(text),
            frozenset({"import-attributes", "import-text"}),
        )

        checkout = Path(test262_runner.TEST262) / "test"
        try:
            checkout_available = checkout.exists()
        except OSError:
            checkout_available = False
        if checkout_available:
            syntax_dir = checkout / "language/module-code/import-attributes"
            runtime_dir = checkout / "language/import/import-attributes"
            live = {
                path.relative_to(checkout).as_posix()
                for path in syntax_dir.glob("*.js")
                if "_FIXTURE" not in path.name
            }
            live.update(
                path.relative_to(checkout).as_posix()
                for path in runtime_dir.glob("*.js")
                if "_FIXTURE" not in path.name
                and (path.name.startswith("json-") or path.name.startswith("text-"))
            )
            self.assertEqual(live, STATIC_IMPORT_ATTRIBUTES_FILES)
            for relative in live:
                meta = test262_runner.parse_meta((checkout / relative).read_text())
                self.assertTrue(
                    static_import_attributes_features(relative)
                    <= set(meta.get("features", []))
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(
                        tool.should_skip(
                            {"features": ["import-attributes"]},
                            root / "test" / syntax,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {
                                "flags": ["module", "async"],
                                "features": [
                                    "import-attributes",
                                    "json-modules",
                                    "dynamic-import",
                                ],
                            },
                            root / "test" / runtime,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {
                                "flags": ["module"],
                                "features": ["import-attributes", "import-text"],
                            },
                            root / "test" / text,
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["import-attributes"]},
                            root / "test" / outside,
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["import-text"]},
                            root / "test" / outside,
                        )
                    )
                    self.assertTrue(tool.static_import_attributes_path(root / "test" / syntax))
                finally:
                    tool.TEST262 = original_root

    def test_intl_canonical_locales_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(INTL_CANONICAL_LOCALES_FILES), 40)
        root_file = "intl402/Intl/builtin.js"
        proxy_file = "intl402/Intl/getCanonicalLocales/has-property.js"
        tag_file = "intl402/Intl/toStringTag/toStringTag.js"
        locale_object = "intl402/Intl/getCanonicalLocales/Locale-object.js"
        outside = "intl402/Intl/getCanonicalLocales/future-test.js"
        self.assertIn(root_file, INTL_CANONICAL_LOCALES_FILES)
        self.assertIn(proxy_file, INTL_CANONICAL_LOCALES_FILES)
        self.assertIn(tag_file, INTL_CANONICAL_LOCALES_FILES)
        self.assertNotIn(locale_object, INTL_CANONICAL_LOCALES_FILES)
        self.assertNotIn(outside, INTL_CANONICAL_LOCALES_FILES)
        self.assertEqual(
            intl_canonical_locales_features(proxy_file), frozenset({"Proxy"})
        )
        self.assertEqual(
            intl_canonical_locales_features(tag_file),
            frozenset({"Symbol.toStringTag"}),
        )
        self.assertEqual(intl_canonical_locales_features(outside), frozenset())

        checkout = Path(test262_runner.TEST262) / "test"
        try:
            checkout_available = checkout.exists()
        except OSError:
            checkout_available = False
        if checkout_available:
            intl_root = checkout / "intl402/Intl"
            live = {
                path.relative_to(checkout).as_posix()
                for path in intl_root.glob("*.js")
                if path.name == "builtin.js"
            }
            live.update(
                path.relative_to(checkout).as_posix()
                for path in (intl_root / "toStringTag").glob("*.js")
            )
            live.update(
                path.relative_to(checkout).as_posix()
                for path in (intl_root / "getCanonicalLocales").glob("*.js")
                if path.name != "Locale-object.js"
            )
            self.assertEqual(live, INTL_CANONICAL_LOCALES_FILES)
            for relative in live:
                meta = test262_runner.parse_meta((checkout / relative).read_text())
                self.assertTrue(
                    intl_canonical_locales_features(relative)
                    <= set(meta.get("features", []))
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            original_roots = (test262_runner.TEST262, test262_analyze.TEST262)
            try:
                test262_runner.TEST262 = temp_dir
                test262_analyze.TEST262 = temp_dir
                admitted = Path(temp_dir) / "test" / proxy_file
                locale = Path(temp_dir) / "test" / locale_object
                future = Path(temp_dir) / "test" / outside
                for tool in (test262_runner, test262_analyze):
                    self.assertFalse(tool.should_skip({"features": ["Proxy"]}, admitted))
                    self.assertFalse(
                        tool.should_skip({"features": ["Intl.Locale"]}, locale)
                    )
                    self.assertTrue(tool.should_skip({"features": ["Proxy"]}, future))
                    self.assertTrue(tool.should_skip({"features": []}, future))
            finally:
                test262_runner.TEST262, test262_analyze.TEST262 = original_roots

    def test_intl_locale_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(INTL_LOCALE_BASE_FILES), 109)
        self.assertEqual(len(INTL_LOCALE_FILES), 161)
        self.assertEqual(len(INTL_LOCALE_INFO_FILES), 52)
        self.assertTrue(INTL_LOCALE_BASE_FILES.isdisjoint(INTL_LOCALE_INFO_FILES))
        locale_object = "intl402/Intl/getCanonicalLocales/Locale-object.js"
        symbol_file = "intl402/Locale/invalid-tag-throws-symbol.js"
        realm_file = "intl402/Locale/proto-from-ctor-realm.js"
        info_file = "intl402/Locale/prototype/getCollations/branding.js"
        outside = "intl402/Locale/future-test.js"
        self.assertIn(locale_object, INTL_LOCALE_FILES)
        self.assertIn(symbol_file, INTL_LOCALE_FILES)
        self.assertIn(realm_file, INTL_LOCALE_FILES)
        self.assertIn(info_file, INTL_LOCALE_FILES)
        self.assertIn(info_file, INTL_LOCALE_INFO_FILES)
        self.assertNotIn(outside, INTL_LOCALE_FILES)
        self.assertEqual(
            intl_locale_features(symbol_file),
            frozenset({"Intl.Locale", "Symbol"}),
        )
        self.assertEqual(
            intl_locale_features(realm_file),
            frozenset({"Intl.Locale", "Reflect", "Symbol", "cross-realm"}),
        )
        self.assertEqual(intl_locale_features(outside), frozenset())

        checkout = Path(test262_runner.TEST262) / "test"
        try:
            checkout_available = checkout.exists()
        except OSError:
            checkout_available = False
        if checkout_available:
            live = {
                path.relative_to(checkout).as_posix()
                for path in (checkout / "intl402/Locale").rglob("*.js")
            }
            adjacent = checkout / locale_object
            if adjacent.exists():
                live.add(locale_object)
            self.assertEqual(live, INTL_LOCALE_FILES)
            for relative in live:
                meta = test262_runner.parse_meta((checkout / relative).read_text())
                self.assertEqual(
                    intl_locale_features(relative),
                    frozenset(meta.get("features", [])),
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            original_roots = (test262_runner.TEST262, test262_analyze.TEST262)
            try:
                test262_runner.TEST262 = temp_dir
                test262_analyze.TEST262 = temp_dir
                admitted = Path(temp_dir) / "test" / symbol_file
                info = Path(temp_dir) / "test" / info_file
                future = Path(temp_dir) / "test" / outside
                adjacent = Path(temp_dir) / "test" / locale_object
                adjacent.parent.mkdir(parents=True)
                adjacent.write_text("/*---\nfeatures: [Intl.Locale]\n---*/")
                for tool in (test262_runner, test262_analyze):
                    self.assertEqual(tool.discover_test_files(adjacent), [adjacent])
                    self.assertFalse(
                        tool.should_skip(
                            {"features": ["Intl.Locale", "Symbol"]}, admitted
                        )
                    )
                    self.assertFalse(
                        tool.should_skip({"features": ["Intl.Locale"]}, adjacent)
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {"features": ["Intl.Locale", "Intl.Locale-info"]},
                            info,
                        )
                    )
                    self.assertTrue(tool.should_skip({"features": []}, future))
            finally:
                test262_runner.TEST262, test262_analyze.TEST262 = original_roots

    def test_intl_supported_values_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(INTL_SUPPORTED_VALUES_FILES), 16)
        builtin = "intl402/Intl/supportedValuesOf/builtin.js"
        calendar = "intl402/Intl/supportedValuesOf/calendars.js"
        formatter = (
            "intl402/Intl/supportedValuesOf/units-accepted-by-NumberFormat.js"
        )
        outside = "intl402/Intl/supportedValuesOf/future-test.js"
        self.assertIn(builtin, INTL_SUPPORTED_VALUES_FILES)
        self.assertIn(calendar, INTL_SUPPORTED_VALUES_FILES)
        self.assertNotIn(formatter, INTL_SUPPORTED_VALUES_FILES)
        self.assertNotIn(outside, INTL_SUPPORTED_VALUES_FILES)
        self.assertEqual(
            intl_supported_values_features(builtin),
            frozenset({"Intl-enumeration", "Reflect.construct"}),
        )
        self.assertEqual(intl_supported_values_features(outside), frozenset())

        checkout = Path(test262_runner.TEST262) / "test"
        try:
            checkout_available = checkout.exists()
        except OSError:
            checkout_available = False
        if checkout_available:
            root = checkout / "intl402/Intl/supportedValuesOf"
            live = {
                path.relative_to(checkout).as_posix()
                for path in root.glob("*.js")
                if "-accepted-by-" not in path.name
            }
            collator_file = root / "collations-accepted-by-Collator.js"
            if collator_file.exists():
                live.add(collator_file.relative_to(checkout).as_posix())
            self.assertEqual(live, INTL_SUPPORTED_VALUES_FILES)
            for relative in live:
                meta = test262_runner.parse_meta((checkout / relative).read_text())
                self.assertEqual(
                    intl_supported_values_features(relative),
                    frozenset(meta.get("features", [])),
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            original_roots = (test262_runner.TEST262, test262_analyze.TEST262)
            try:
                test262_runner.TEST262 = temp_dir
                test262_analyze.TEST262 = temp_dir
                admitted = Path(temp_dir) / "test" / calendar
                excluded = Path(temp_dir) / "test" / formatter
                future = Path(temp_dir) / "test" / outside
                broad_only = (
                    Path(temp_dir)
                    / "test/intl402/NumberFormat/supportedValuesOf-future.js"
                )
                for tool in (test262_runner, test262_analyze):
                    self.assertFalse(
                        tool.should_skip(
                            {
                                "features": [
                                    "Intl-enumeration",
                                    "Intl.Locale",
                                    "Array.prototype.includes",
                                ]
                            },
                            admitted,
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "features": [
                                    "Intl-enumeration",
                                    "Array.prototype.includes",
                                ]
                            },
                            excluded,
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["Intl-enumeration"]}, future
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["Intl-enumeration"]}, broad_only
                        )
                    )
            finally:
                test262_runner.TEST262, test262_analyze.TEST262 = original_roots

    def test_intl_collator_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(INTL_COLLATOR_FILES), 74)
        realm_file = "intl402/Collator/proto-from-ctor-realm.js"
        compare_builtin = "intl402/Collator/prototype/compare/builtin.js"
        locale_compare = (
            "intl402/String/prototype/localeCompare/taint-Intl-Collator.js"
        )
        excluded = "intl402/Collator/this-value-ignored.js"
        future = "intl402/Collator/future-test.js"
        self.assertIn(realm_file, INTL_COLLATOR_FILES)
        self.assertIn(compare_builtin, INTL_COLLATOR_FILES)
        self.assertIn(locale_compare, INTL_COLLATOR_FILES)
        self.assertNotIn(excluded, INTL_COLLATOR_FILES)
        self.assertNotIn(future, INTL_COLLATOR_FILES)
        self.assertEqual(
            intl_collator_features(realm_file),
            frozenset({"cross-realm", "Reflect", "Symbol"}),
        )
        self.assertEqual(
            intl_collator_features(compare_builtin),
            frozenset({"Reflect.construct"}),
        )
        self.assertEqual(intl_collator_features(future), frozenset())

        checkout = Path(test262_runner.TEST262) / "test"
        try:
            checkout_available = checkout.exists()
        except OSError:
            checkout_available = False
        if checkout_available:
            live = {
                path.relative_to(checkout).as_posix()
                for root in (
                    checkout / "intl402/Collator",
                    checkout / "intl402/String/prototype/localeCompare",
                )
                for path in root.rglob("*.js")
                if path.relative_to(checkout).as_posix() != excluded
            }
            self.assertEqual(live, INTL_COLLATOR_FILES)
            for relative in live:
                meta = test262_runner.parse_meta((checkout / relative).read_text())
                self.assertEqual(
                    intl_collator_features(relative),
                    frozenset(meta.get("features", [])),
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            original_roots = (test262_runner.TEST262, test262_analyze.TEST262)
            try:
                test262_runner.TEST262 = temp_dir
                test262_analyze.TEST262 = temp_dir
                admitted = Path(temp_dir) / "test" / realm_file
                held = Path(temp_dir) / "test" / excluded
                unknown = Path(temp_dir) / "test" / future
                for tool in (test262_runner, test262_analyze):
                    self.assertFalse(
                        tool.should_skip(
                            {"features": ["cross-realm", "Reflect", "Symbol"]},
                            admitted,
                        )
                    )
                    self.assertTrue(tool.should_skip({"features": []}, held))
                    self.assertTrue(tool.should_skip({"features": []}, unknown))
            finally:
                test262_runner.TEST262, test262_analyze.TEST262 = original_roots

    def test_import_meta_manifest_is_exact_and_shared(self):
        self.assertEqual(len(IMPORT_META_FILES), 23)
        admitted = "language/expressions/import.meta/same-object-returned.js"
        dynamic = (
            "language/expressions/dynamic-import/assignment-expression/import-meta.js"
        )
        outside = "language/expressions/import.meta/unknown-future-test.js"
        self.assertIn(admitted, IMPORT_META_FILES)
        self.assertIn(dynamic, IMPORT_META_FILES)
        self.assertNotIn(outside, IMPORT_META_FILES)
        meta = {"flags": ["module", "async"], "features": ["import.meta"]}
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted_path = root / "test" / admitted
            outside_path = root / "test" / outside
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.import_meta_path(admitted_path))
                    self.assertFalse(tool.should_skip(meta, admitted_path))
                    self.assertTrue(tool.should_skip(meta, outside_path))
                finally:
                    tool.TEST262 = original_root

    def test_json_parse_manifest_is_exact_and_shared(self):
        self.assertEqual(len(JSON_PARSE_FILES), 77)
        self.assertTrue(
            all(path.startswith("built-ins/JSON/parse/") for path in JSON_PARSE_FILES)
        )
        admitted = "built-ins/JSON/parse/reviver-context-source-primitive-literal.js"
        proxy_admitted = "built-ins/JSON/parse/revived-proxy.js"
        outside = "built-ins/JSON/stringify/replacer.js"
        self.assertIn(admitted, JSON_PARSE_FILES)
        self.assertIn(proxy_admitted, JSON_PARSE_FILES)
        self.assertNotIn(outside, JSON_PARSE_FILES)
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted_path = root / "test" / admitted
            proxy_path = root / "test" / proxy_admitted
            outside_path = root / "test" / outside
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(
                        tool.should_skip(
                            {"features": ["json-parse-with-source"]}, admitted_path
                        )
                    )
                    self.assertFalse(
                        tool.should_skip({"features": ["Proxy"]}, proxy_path)
                    )
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, outside_path)
                    )
                    self.assertTrue(tool.json_parse_path(admitted_path))
                finally:
                    tool.TEST262 = original_root

    def test_json_stringify_manifest_is_exact_and_shared(self):
        self.assertEqual(len(JSON_STRINGIFY_FILES), 66)
        self.assertTrue(
            all(path.startswith("built-ins/JSON/stringify/") for path in JSON_STRINGIFY_FILES)
        )
        admitted = "built-ins/JSON/stringify/value-object-proxy.js"
        outside = "built-ins/Array/isArray/proxy.js"
        self.assertIn(admitted, JSON_STRINGIFY_FILES)
        self.assertNotIn(outside, JSON_STRINGIFY_FILES)
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted_path = root / "test" / admitted
            outside_path = root / "test" / outside
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(
                        tool.should_skip({"features": ["Proxy"]}, admitted_path)
                    )
                    self.assertTrue(tool.should_skip({"features": ["Proxy"]}, outside_path))
                    self.assertTrue(tool.json_stringify_path(admitted_path))
                finally:
                    tool.TEST262 = original_root

    def test_json_raw_manifest_is_exact_and_shared(self):
        self.assertEqual(len(JSON_RAW_FILES), 17)
        admitted = "built-ins/JSON/rawJSON/basic.js"
        tag = "built-ins/JSON/Symbol.toStringTag.js"
        outside = "built-ins/Array/isArray/proxy.js"
        self.assertIn(admitted, JSON_RAW_FILES)
        self.assertIn(tag, JSON_RAW_FILES)
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted_path = root / "test" / admitted
            tag_path = root / "test" / tag
            outside_path = root / "test" / outside
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(
                        {"features": ["json-parse-with-source"]}, admitted_path
                    ))
                    self.assertFalse(tool.should_skip(
                        {"features": ["Symbol.toStringTag"]}, tag_path
                    ))
                    self.assertTrue(tool.should_skip(
                        {"features": ["Proxy"]}, outside_path
                    ))
                    self.assertTrue(tool.json_raw_path(admitted_path))
                finally:
                    tool.TEST262 = original_root

    def test_date_to_primitive_manifest_is_exact_and_shared(self):
        self.assertEqual(len(DATE_TO_PRIMITIVE_FILES), 18)
        admitted = "built-ins/Date/prototype/Symbol.toPrimitive/called-as-function.js"
        outside = "built-ins/Array/isArray/proxy.js"
        self.assertIn(admitted, DATE_TO_PRIMITIVE_FILES)
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted_path = root / "test" / admitted
            outside_path = root / "test" / outside
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(
                        {"features": ["Symbol"]}, admitted_path
                    ))
                    self.assertTrue(tool.should_skip(
                        {"features": ["Proxy"]}, outside_path
                    ))
                    self.assertTrue(tool.date_to_primitive_path(admitted_path))
                finally:
                    tool.TEST262 = original_root

    def test_annex_b_string_manifest_is_exact_live_disjoint_and_shared(self):
        manifest = Path(__file__).with_name(
            "test262_annex_b_string_admission.txt"
        )
        entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(len(entries), 16)
        self.assertEqual(entries, tuple(sorted(entries)))
        self.assertEqual(ANNEX_B_STRING_FILES, frozenset(entries))
        self.assertEqual(frozenset(ANNEX_B_STRING_FEATURES), ANNEX_B_STRING_FILES)

        for other_manifest in Path(__file__).parent.glob("test262_*_admission.txt"):
            if other_manifest == manifest:
                continue
            other_entries = {
                line
                for raw_line in other_manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                ANNEX_B_STRING_FILES.isdisjoint(other_entries),
                other_manifest.name,
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        for tool in (test262_runner, test262_analyze):
            original_root = tool.TEST262
            try:
                tool.TEST262 = str(test_root.parent)
                for relative, features in ANNEX_B_STRING_FEATURES.items():
                    path = test_root / relative
                    self.assertTrue(tool.annex_b_string_path(path), relative)
                    self.assertEqual(tool.annex_b_string_features(path), features)
                    self.assertFalse(
                        tool.should_skip({"features": sorted(features)}, path),
                        relative,
                    )
                    if test_root_available:
                        metadata = tool.parse_meta(path.read_text())
                        self.assertEqual(
                            frozenset(metadata.get("features", [])),
                            features,
                            relative,
                        )
                future = test_root / (
                    "annexB/built-ins/String/prototype/anchor/future.js"
                )
                self.assertFalse(tool.annex_b_string_path(future))
                self.assertEqual(tool.annex_b_string_features(future), frozenset())
                self.assertTrue(
                    tool.should_skip(
                        {"features": ["Reflect.construct", "arrow-function"]},
                        future,
                    )
                )
                html_dda = test_root / (
                    "annexB/built-ins/String/prototype/match/"
                    "custom-matcher-emulates-undefined.js"
                )
                self.assertTrue(
                    tool.should_skip(
                        {"features": ["Symbol.match", "IsHTMLDDA"]}, html_dda
                    )
                )
                for invalid in (None, object()):
                    self.assertFalse(tool.annex_b_string_path(invalid))
                    self.assertEqual(
                        tool.annex_b_string_features(invalid), frozenset()
                    )
            finally:
                tool.TEST262 = original_root

    def test_annex_b_escape_manifest_is_exact_live_disjoint_and_shared(self):
        manifest = Path(__file__).with_name(
            "test262_annex_b_escape_admission.txt"
        )
        entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(len(entries), 4)
        self.assertEqual(entries, tuple(sorted(entries)))
        self.assertEqual(ANNEX_B_ESCAPE_FILES, frozenset(entries))
        self.assertEqual(frozenset(ANNEX_B_ESCAPE_FEATURES), ANNEX_B_ESCAPE_FILES)

        for other_manifest in Path(__file__).parent.glob("test262_*_admission.txt"):
            if other_manifest == manifest:
                continue
            other_entries = {
                line
                for raw_line in other_manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                ANNEX_B_ESCAPE_FILES.isdisjoint(other_entries),
                other_manifest.name,
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        for tool in (test262_runner, test262_analyze):
            original_root = tool.TEST262
            try:
                tool.TEST262 = str(test_root.parent)
                for relative, features in ANNEX_B_ESCAPE_FEATURES.items():
                    path = test_root / relative
                    self.assertTrue(tool.annex_b_escape_path(path), relative)
                    self.assertEqual(tool.annex_b_escape_features(path), features)
                    self.assertFalse(
                        tool.should_skip({"features": sorted(features)}, path),
                        relative,
                    )
                    if test_root_available:
                        metadata = tool.parse_meta(path.read_text())
                        self.assertEqual(
                            frozenset(metadata.get("features", [])),
                            features,
                            relative,
                        )
                future = test_root / "annexB/built-ins/escape/future.js"
                self.assertFalse(tool.annex_b_escape_path(future))
                self.assertEqual(tool.annex_b_escape_features(future), frozenset())
                self.assertTrue(
                    tool.should_skip(
                        {"features": ["Reflect.construct", "arrow-function"]},
                        future,
                    )
                )
                for invalid in (None, object()):
                    self.assertFalse(tool.annex_b_escape_path(invalid))
                    self.assertEqual(
                        tool.annex_b_escape_features(invalid), frozenset()
                    )
            finally:
                tool.TEST262 = original_root

    def test_annex_b_date_manifest_is_exact_live_disjoint_and_shared(self):
        expected_features = {
            "annexB/built-ins/Date/prototype/getYear/not-a-constructor.js": frozenset(
                {"Reflect.construct", "arrow-function"}
            ),
            "annexB/built-ins/Date/prototype/setYear/not-a-constructor.js": frozenset(
                {"Reflect.construct", "arrow-function"}
            ),
            "annexB/built-ins/Date/prototype/setYear/year-nan.js": frozenset(
                {"Symbol"}
            ),
            "annexB/built-ins/Date/prototype/setYear/year-to-number-err.js": frozenset(
                {"Symbol"}
            ),
            "annexB/built-ins/Date/prototype/toGMTString/not-a-constructor.js": frozenset(
                {"Reflect.construct", "arrow-function"}
            ),
        }
        manifest = Path(__file__).with_name("test262_annex_b_date_admission.txt")
        entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(tuple(expected_features), entries)
        self.assertEqual(entries, tuple(sorted(entries)))
        self.assertEqual(ANNEX_B_DATE_FILES, frozenset(entries))
        self.assertEqual(ANNEX_B_DATE_FEATURES, expected_features)

        for other_manifest in Path(__file__).parent.glob("test262_*_admission.txt"):
            if other_manifest == manifest:
                continue
            other_entries = {
                line
                for raw_line in other_manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                ANNEX_B_DATE_FILES.isdisjoint(other_entries),
                other_manifest.name,
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        for tool in (test262_runner, test262_analyze):
            original_root = tool.TEST262
            try:
                tool.TEST262 = str(test_root.parent)
                for relative, features in ANNEX_B_DATE_FEATURES.items():
                    path = test_root / relative
                    self.assertTrue(tool.annex_b_date_path(path), relative)
                    self.assertEqual(tool.annex_b_date_features(path), features)
                    self.assertFalse(
                        tool.should_skip({"features": sorted(features)}, path),
                        relative,
                    )
                    if test_root_available:
                        metadata = tool.parse_meta(path.read_text())
                        self.assertEqual(
                            frozenset(metadata.get("features", [])),
                            features,
                            relative,
                        )
                future = test_root / "annexB/built-ins/Date/prototype/future.js"
                self.assertFalse(tool.annex_b_date_path(future))
                self.assertEqual(tool.annex_b_date_features(future), frozenset())
                self.assertTrue(
                    tool.should_skip(
                        {"features": ["Reflect.construct", "arrow-function"]},
                        future,
                    )
                )
                for invalid in (None, object()):
                    self.assertFalse(tool.annex_b_date_path(invalid))
                    self.assertEqual(tool.annex_b_date_features(invalid), frozenset())
            finally:
                tool.TEST262 = original_root

    def test_regexp_compile_manifest_is_exact_live_disjoint_and_shared(self):
        manifest = Path(__file__).with_name(
            "test262_regexp_compile_admission.txt"
        )
        entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(len(entries), 4)
        self.assertEqual(entries, tuple(sorted(entries)))
        self.assertEqual(REGEXP_COMPILE_FILES, frozenset(entries))
        self.assertEqual(frozenset(REGEXP_COMPILE_FEATURES), REGEXP_COMPILE_FILES)

        for other_manifest in Path(__file__).parent.glob("test262_*_admission.txt"):
            if other_manifest == manifest:
                continue
            other_entries = {
                line
                for raw_line in other_manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                REGEXP_COMPILE_FILES.isdisjoint(other_entries),
                other_manifest.name,
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        for tool in (test262_runner, test262_analyze):
            original_root = tool.TEST262
            try:
                tool.TEST262 = str(test_root.parent)
                for relative, features in REGEXP_COMPILE_FEATURES.items():
                    path = test_root / relative
                    self.assertTrue(tool.regexp_compile_path(path), relative)
                    self.assertEqual(tool.regexp_compile_features(path), features)
                    self.assertFalse(
                        tool.should_skip({"features": sorted(features)}, path),
                        relative,
                    )
                    if test_root_available:
                        metadata = tool.parse_meta(path.read_text())
                        self.assertEqual(
                            frozenset(metadata.get("features", [])),
                            features,
                            relative,
                        )
                future = test_root / (
                    "annexB/built-ins/RegExp/prototype/compile/future.js"
                )
                self.assertFalse(tool.regexp_compile_path(future))
                self.assertEqual(tool.regexp_compile_features(future), frozenset())
                self.assertTrue(tool.should_skip({"features": ["Symbol"]}, future))
                for invalid in (None, object()):
                    self.assertFalse(tool.regexp_compile_path(invalid))
                    self.assertEqual(
                        tool.regexp_compile_features(invalid), frozenset()
                    )
            finally:
                tool.TEST262 = original_root

    def test_regexp_legacy_accessor_manifest_is_exact_live_disjoint_and_shared(self):
        manifest = Path(__file__).with_name(
            "test262_regexp_legacy_accessors_admission.txt"
        )
        entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(len(entries), 24)
        self.assertEqual(entries, tuple(sorted(entries)))
        self.assertEqual(REGEXP_LEGACY_ACCESSOR_FILES, frozenset(entries))
        self.assertEqual(
            frozenset(REGEXP_LEGACY_ACCESSOR_FEATURES),
            REGEXP_LEGACY_ACCESSOR_FILES,
        )

        for other_manifest in Path(__file__).parent.glob("test262_*_admission.txt"):
            if other_manifest == manifest:
                continue
            other_entries = {
                line
                for raw_line in other_manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                REGEXP_LEGACY_ACCESSOR_FILES.isdisjoint(other_entries),
                other_manifest.name,
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        for tool in (test262_runner, test262_analyze):
            original_root = tool.TEST262
            try:
                tool.TEST262 = str(test_root.parent)
                for relative, features in REGEXP_LEGACY_ACCESSOR_FEATURES.items():
                    path = test_root / relative
                    self.assertTrue(tool.regexp_legacy_accessor_path(path), relative)
                    self.assertEqual(
                        tool.regexp_legacy_accessor_features(path), features
                    )
                    self.assertFalse(
                        tool.should_skip({"features": sorted(features)}, path),
                        relative,
                    )
                    if test_root_available:
                        metadata = tool.parse_meta(path.read_text())
                        self.assertEqual(
                            frozenset(metadata.get("features", [])),
                            features,
                            relative,
                        )
                future = test_root / (
                    "annexB/built-ins/RegExp/legacy-accessors/input/future.js"
                )
                self.assertFalse(tool.regexp_legacy_accessor_path(future))
                self.assertEqual(
                    tool.regexp_legacy_accessor_features(future), frozenset()
                )
                self.assertTrue(
                    tool.should_skip({"features": ["Reflect"]}, future)
                )
                for invalid in (None, object()):
                    self.assertFalse(tool.regexp_legacy_accessor_path(invalid))
                    self.assertEqual(
                        tool.regexp_legacy_accessor_features(invalid), frozenset()
                    )
            finally:
                tool.TEST262 = original_root

    def test_regexp_annex_b_manifest_is_exact_live_disjoint_and_shared(self):
        manifest = Path(__file__).with_name(
            "test262_regexp_annex_b_admission.txt"
        )
        entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(len(entries), 4)
        self.assertEqual(entries, tuple(sorted(entries)))
        self.assertEqual(REGEXP_ANNEX_B_FILES, frozenset(entries))
        self.assertEqual(
            frozenset(REGEXP_ANNEX_B_FEATURES), REGEXP_ANNEX_B_FILES
        )

        for other_manifest in Path(__file__).parent.glob("test262_*_admission.txt"):
            if other_manifest == manifest:
                continue
            other_entries = {
                line
                for raw_line in other_manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                REGEXP_ANNEX_B_FILES.isdisjoint(other_entries),
                other_manifest.name,
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        for tool in (test262_runner, test262_analyze):
            original_root = tool.TEST262
            try:
                tool.TEST262 = str(test_root.parent)
                for relative, features in REGEXP_ANNEX_B_FEATURES.items():
                    path = test_root / relative
                    self.assertTrue(tool.regexp_annex_b_path(path), relative)
                    self.assertEqual(tool.regexp_annex_b_features(path), features)
                    self.assertFalse(
                        tool.should_skip({"features": sorted(features)}, path),
                        relative,
                    )
                    if test_root_available:
                        metadata = tool.parse_meta(path.read_text())
                        self.assertEqual(
                            frozenset(metadata.get("features", [])),
                            features,
                            relative,
                        )
                future = test_root / "annexB/built-ins/RegExp/future.js"
                self.assertFalse(tool.regexp_annex_b_path(future))
                self.assertEqual(tool.regexp_annex_b_features(future), frozenset())
                self.assertTrue(
                    tool.should_skip({"features": ["regexp-named-groups"]}, future)
                )
                for invalid in (None, object()):
                    self.assertFalse(tool.regexp_annex_b_path(invalid))
                    self.assertEqual(tool.regexp_annex_b_features(invalid), frozenset())
            finally:
                tool.TEST262 = original_root

    def test_reference_primitive_manifest_is_exact_and_shared(self):
        expected = {
            "language/types/reference/get-value-prop-base-primitive.js": {"Symbol"},
            "language/types/reference/get-value-prop-base-primitive-realm.js": {
                "cross-realm",
                "Symbol",
            },
            "language/types/reference/put-value-prop-base-primitive.js": {
                "Symbol",
                "Proxy",
            },
            "language/types/reference/put-value-prop-base-primitive-realm.js": {
                "cross-realm",
                "Symbol",
                "Proxy",
            },
        }
        self.assertEqual(REFERENCE_PRIMITIVE_FILES, frozenset(expected))
        self.assertEqual(REFERENCE_PRIMITIVE_FEATURES, expected)
        outside = "built-ins/Array/isArray/proxy.js"
        future = "language/types/reference/get-value-prop-base-primitive-future.js"

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside_path = root / "test" / outside
            future_path = root / "test" / future
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertIs(
                        tool.REFERENCE_PRIMITIVE_FILES,
                        REFERENCE_PRIMITIVE_FILES,
                    )
                    self.assertIs(
                        tool.REFERENCE_PRIMITIVE_FEATURES,
                        REFERENCE_PRIMITIVE_FEATURES,
                    )
                    for relative, features in expected.items():
                        admitted_path = root / "test" / relative
                        self.assertTrue(
                            tool.reference_primitive_path(admitted_path), relative
                        )
                        self.assertEqual(
                            tool.reference_primitive_features(admitted_path),
                            features,
                            relative,
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features)}, admitted_path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": sorted(
                                        features | {"decorators"}
                                    )
                                },
                                admitted_path,
                            ),
                            relative,
                        )
                    self.assertTrue(tool.should_skip(
                        {"features": ["Proxy"]}, outside_path
                    ))
                    self.assertFalse(tool.reference_primitive_path(future_path))
                    self.assertEqual(
                        tool.reference_primitive_features(future_path), frozenset()
                    )
                    self.assertTrue(
                        tool.should_skip({"features": ["Symbol"]}, future_path)
                    )
                    for invalid in (None, object()):
                        self.assertFalse(tool.reference_primitive_path(invalid))
                        self.assertEqual(
                            tool.reference_primitive_features(invalid), frozenset()
                        )
                finally:
                    tool.TEST262 = original_root

    def test_object_constructor_manifest_is_exact_and_shared(self):
        expected = {
            "built-ins/Object/is-a-constructor.js": {"Reflect.construct"},
            "built-ins/Object/proto-from-ctor-realm.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/Object/subclass-object-arg.js": {
                "class", "Reflect", "Reflect.construct",
            },
        }
        self.assertEqual(OBJECT_CONSTRUCTOR_FILES, frozenset(expected))
        self.assertEqual(
            OBJECT_CONSTRUCTOR_FEATURES,
            {path: frozenset(features) for path, features in expected.items()},
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test" / "built-ins/Object/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.object_constructor_path(path))
                        self.assertEqual(
                            tool.object_constructor_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features)},
                                path,
                            )
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"Proxy"})},
                                path,
                            )
                        )
                    self.assertFalse(tool.object_constructor_path(outside))
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["Reflect.construct"]}, outside
                        )
                    )
                finally:
                    tool.TEST262 = original_root

    def test_object_from_entries_manifest_is_exact_live_disjoint_and_shared(self):
        iterator_names = {
            "evaluation-order.js",
            "iterator-closed-for-null-entry.js",
            "iterator-closed-for-string-entry.js",
            "iterator-closed-for-throwing-entry-key-accessor.js",
            "iterator-closed-for-throwing-entry-key-tostring.js",
            "iterator-closed-for-throwing-entry-value-accessor.js",
            "iterator-not-closed-for-next-returning-non-object.js",
            "iterator-not-closed-for-throwing-done-accessor.js",
            "iterator-not-closed-for-throwing-next.js",
            "iterator-not-closed-for-uncallable-next.js",
            "uses-keys-not-iterator.js",
        }
        expected = {
            **{
                f"built-ins/Object/fromEntries/{name}": {
                    "Object.fromEntries",
                    "Symbol.iterator",
                }
                for name in iterator_names
            },
            "built-ins/Object/fromEntries/not-a-constructor.js": {
                "Object.fromEntries",
                "Reflect.construct",
                "arrow-function",
            },
            "built-ins/Object/fromEntries/supports-symbols.js": {
                "Object.fromEntries",
                "Symbol",
            },
        }
        self.assertEqual(OBJECT_FROM_ENTRIES_FILES, frozenset(expected))
        self.assertEqual(
            OBJECT_FROM_ENTRIES_FEATURES,
            {path: frozenset(features) for path, features in expected.items()},
        )

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_object_from_entries_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(OBJECT_FROM_ENTRIES_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in OBJECT_FROM_ENTRIES_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Object/fromEntries/future.js"
            outside = root / "test/built-ins/Object/entries/evaluation-order.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.object_from_entries_path(path))
                        self.assertEqual(
                            tool.object_from_entries_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"Proxy"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.object_from_entries_path(path))
                        self.assertTrue(
                            tool.should_skip({"features": ["Symbol.iterator"]}, path)
                        )
                finally:
                    tool.TEST262 = original_root

    def test_suppressed_error_manifest_is_exact_live_disjoint_and_shared(self):
        expected = {
            "built-ins/SuppressedError/is-a-constructor.js": {
                "Reflect.construct",
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/length.js": {"explicit-resource-management"},
            "built-ins/SuppressedError/message-method-prop-cast.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/message-method-prop.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/message-tostring-abrupt-symbol.js": {
                "explicit-resource-management",
                "Symbol",
                "Symbol.toPrimitive",
            },
            "built-ins/SuppressedError/message-tostring-abrupt.js": {
                "explicit-resource-management",
                "Symbol.toPrimitive",
            },
            "built-ins/SuppressedError/message-undefined-no-prop.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/name.js": {"explicit-resource-management"},
            "built-ins/SuppressedError/newtarget-is-undefined.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/newtarget-proto-custom.js": {
                "explicit-resource-management",
                "Reflect.construct",
            },
            "built-ins/SuppressedError/newtarget-proto-fallback.js": {
                "explicit-resource-management",
                "Symbol",
            },
            "built-ins/SuppressedError/newtarget-proto.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/order-of-args-evaluation.js": {
                "explicit-resource-management",
                "Symbol.iterator",
            },
            "built-ins/SuppressedError/prop-desc.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/proto-from-ctor-realm.js": {
                "explicit-resource-management",
                "cross-realm",
                "Reflect",
                "Symbol",
            },
            "built-ins/SuppressedError/proto.js": {"explicit-resource-management"},
            "built-ins/SuppressedError/prototype/constructor.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/prototype/errors-absent-on-prototype.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/prototype/message.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/prototype/name.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/prototype/prop-desc.js": {
                "explicit-resource-management",
            },
            "built-ins/SuppressedError/prototype/proto.js": {
                "explicit-resource-management",
            },
        }
        expected = {
            relative: frozenset(features) for relative, features in expected.items()
        }
        self.assertEqual(len(expected), 22)
        self.assertEqual(SUPPRESSED_ERROR_FILES, frozenset(expected))
        self.assertEqual(SUPPRESSED_ERROR_FEATURES, expected)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_suppressed_error_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(SUPPRESSED_ERROR_FILES.isdisjoint(existing), manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/SuppressedError/future.js"
            outside = root / "test/built-ins/Other/suppressed-error.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.suppressed_error_path(path), relative)
                        self.assertEqual(
                            tool.suppressed_error_features(path), features, relative
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.suppressed_error_path(path))
                        self.assertEqual(
                            tool.suppressed_error_features(path), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": ["explicit-resource-management"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root

    def test_temporal_namespace_manifest_is_exact_live_disjoint_and_shared(self):
        expected = {
            "built-ins/Temporal/Now/toStringTag/prop-desc.js",
            "built-ins/Temporal/Now/toStringTag/string.js",
            "built-ins/Temporal/toStringTag/prop-desc.js",
            "built-ins/Temporal/toStringTag/string.js",
        }
        features = frozenset({"Symbol.toStringTag", "Temporal"})
        self.assertEqual(TEMPORAL_NAMESPACE_FILES, frozenset(expected))
        self.assertEqual(TEMPORAL_NAMESPACE_FEATURES, features)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_namespace_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(TEMPORAL_NAMESPACE_FILES.isdisjoint(existing), manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative in expected:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(frozenset(metadata.get("features", [])), features)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))
                expected_includes = (
                    ["propertyHelper.js"] if relative.endswith("prop-desc.js") else []
                )
                self.assertEqual(metadata.get("includes", []), expected_includes)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/toStringTag/future.js"
            outside = root / "test/built-ins/Other/toStringTag/string.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative in expected:
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_namespace_path(path), relative)
                        self.assertEqual(tool.temporal_namespace_features(path), features)
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_namespace_path(path))
                        self.assertEqual(tool.temporal_namespace_features(path), frozenset())
                        self.assertTrue(
                            tool.should_skip({"features": ["Temporal"]}, path)
                        )
                finally:
                    tool.TEST262 = original_root

    def test_temporal_instant_core_manifest_is_exact_live_disjoint_and_shared(self):
        expected = {
            "built-ins/Temporal/Instant/argument.js": {"Symbol", "Temporal"},
            "built-ins/Temporal/Instant/basic.js": {"Temporal"},
            "built-ins/Temporal/Instant/builtin.js": {"Temporal"},
            "built-ins/Temporal/Instant/constructor.js": {"Temporal"},
            "built-ins/Temporal/Instant/get-prototype-from-constructor-throws.js": {"Temporal"},
            "built-ins/Temporal/Instant/large-bigint.js": {"Temporal"},
            "built-ins/Temporal/Instant/length.js": {"Temporal"},
            "built-ins/Temporal/Instant/name.js": {"Temporal"},
            "built-ins/Temporal/Instant/prop-desc.js": {"Temporal"},
            "built-ins/Temporal/Instant/prototype/builtin.js": {"Temporal"},
            "built-ins/Temporal/Instant/prototype/constructor.js": {"Temporal"},
            "built-ins/Temporal/Instant/prototype/prop-desc.js": {"Temporal"},
            "built-ins/Temporal/Instant/prototype/toStringTag/prop-desc.js": {"Temporal"},
            "built-ins/Temporal/Instant/prototype/epochMilliseconds/basic.js": {"BigInt", "Temporal"},
            "built-ins/Temporal/Instant/prototype/epochMilliseconds/branding.js": {"Symbol", "Temporal"},
            "built-ins/Temporal/Instant/prototype/epochMilliseconds/prop-desc.js": {"Temporal"},
            "built-ins/Temporal/Instant/prototype/epochNanoseconds/basic.js": {"BigInt", "Temporal"},
            "built-ins/Temporal/Instant/prototype/epochNanoseconds/branding.js": {"Symbol", "Temporal"},
            "built-ins/Temporal/Instant/prototype/epochNanoseconds/prop-desc.js": {"Temporal"},
        }
        expected = {path: frozenset(features) for path, features in expected.items()}
        self.assertEqual(TEMPORAL_INSTANT_CORE_FILES, frozenset(expected))
        self.assertEqual(TEMPORAL_INSTANT_CORE_FEATURES, expected)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_instant_core_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(TEMPORAL_INSTANT_CORE_FILES.isdisjoint(existing), manifest.name)

        property_helper_files = {
            "built-ins/Temporal/Instant/length.js",
            "built-ins/Temporal/Instant/name.js",
            "built-ins/Temporal/Instant/prop-desc.js",
            "built-ins/Temporal/Instant/prototype/constructor.js",
            "built-ins/Temporal/Instant/prototype/prop-desc.js",
            "built-ins/Temporal/Instant/prototype/toStringTag/prop-desc.js",
        }
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(frozenset(metadata.get("features", [])), features)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))
                expected_includes = ["propertyHelper.js"] if relative in property_helper_files else []
                self.assertEqual(metadata.get("includes", []), expected_includes)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/Instant/future.js"
            outside = root / "test/built-ins/Other/Instant/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_instant_core_path(path), relative)
                        self.assertEqual(tool.temporal_instant_core_features(path), features)
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_instant_core_path(path))
                        self.assertEqual(tool.temporal_instant_core_features(path), frozenset())
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_instant_epoch_factory_manifest_is_exact_live_disjoint_and_shared(self):
        milliseconds = "built-ins/Temporal/Instant/fromEpochMilliseconds/"
        nanoseconds = "built-ins/Temporal/Instant/fromEpochNanoseconds/"
        expected = {
            milliseconds + name
            for name in (
                "argument.js",
                "basic.js",
                "builtin.js",
                "length.js",
                "limits.js",
                "name.js",
                "non-integer.js",
                "not-a-constructor.js",
                "prop-desc.js",
                "subclassing-ignored.js",
            )
        } | {
            nanoseconds + name
            for name in (
                "argument.js",
                "basic.js",
                "builtin.js",
                "length.js",
                "limits.js",
                "name.js",
                "not-a-constructor.js",
                "prop-desc.js",
                "subclassing-ignored.js",
            )
        }
        test_root = Path(test262_runner.TEST262) / "test"
        self.assertEqual(TEMPORAL_INSTANT_EPOCH_FACTORY_FILES, frozenset(expected))
        self.assertEqual(len(expected), 19)

        live_directories = tuple(
            test_root / prefix for prefix in (milliseconds, nanoseconds)
        )

        def live_test262_available():
            try:
                return all(path.is_dir() for path in live_directories)
            except OSError:
                return False

        test_root_available = live_test262_available()
        if test_root_available:
            live = {
                path.relative_to(test_root).as_posix()
                for prefix in (milliseconds, nanoseconds)
                for path in (test_root / prefix).glob("*.js")
            }
            self.assertEqual(live, expected)
            for relative in expected:
                metadata = test262_runner.parse_meta((test_root / relative).read_text())
                filename = Path(relative).name
                if filename in {"length.js", "name.js", "prop-desc.js"}:
                    expected_includes = ["propertyHelper.js"]
                elif filename == "not-a-constructor.js":
                    expected_includes = ["isConstructor.js"]
                elif filename in {"limits.js", "subclassing-ignored.js"}:
                    expected_includes = ["temporalHelpers.js"]
                else:
                    expected_includes = []
                self.assertEqual(
                    TEMPORAL_INSTANT_EPOCH_FACTORY_FEATURES[relative],
                    frozenset(metadata.get("features", [])),
                )
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))
                self.assertEqual(
                    metadata.get("includes", []),
                    expected_includes,
                )

        with patch("pathlib.Path.is_dir", side_effect=PermissionError):
            self.assertFalse(live_test262_available())

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_instant_epoch_factories_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                TEMPORAL_INSTANT_EPOCH_FACTORY_FILES.isdisjoint(existing), manifest.name
            )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/Instant/fromEpochMilliseconds/future.js"
            outside = root / "test/built-ins/Other/fromEpochMilliseconds/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in TEMPORAL_INSTANT_EPOCH_FACTORY_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_instant_epoch_factory_path(path), relative)
                        self.assertEqual(
                            tool.temporal_instant_epoch_factory_features(path), features
                        )
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_instant_epoch_factory_path(path))
                        self.assertEqual(
                            tool.temporal_instant_epoch_factory_features(path), frozenset()
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_zoned_date_time_core_manifest_is_exact_live_disjoint_and_shared(self):
        zoned_prefix = "built-ins/Temporal/ZonedDateTime/"
        root_names = {
            "argument-convert.js",
            "builtin.js",
            "calendar-case-insensitive.js",
            "calendar-invalid-iso-string.js",
            "calendar-string.js",
            "calendar-undefined.js",
            "constructor.js",
            "get-prototype-from-constructor-throws.js",
            "length.js",
            "limits.js",
            "missing-arguments.js",
            "name.js",
            "prop-desc.js",
            "subclass.js",
            "timezone-case-insensitive.js",
            "timezone-iso-string.js",
            "timezone-string.js",
            "timezone-wrong-type.js",
        }
        expected = {zoned_prefix + name for name in root_names}
        expected.update(
            {
                zoned_prefix + "prototype/constructor.js",
                zoned_prefix + "prototype/prop-desc.js",
                zoned_prefix + "prototype/toStringTag/prop-desc.js",
                "built-ins/Temporal/Instant/compare/argument-zoneddatetime.js",
                "built-ins/Temporal/Instant/from/argument-zoneddatetime.js",
                "built-ins/Temporal/Instant/prototype/equals/argument-zoneddatetime.js",
            }
        )
        for directory in ("calendarId", "epochMilliseconds", "epochNanoseconds", "timeZoneId"):
            expected.update(
                zoned_prefix + f"prototype/{directory}/{name}"
                for name in ("basic.js", "branding.js", "prop-desc.js")
            )

        bigint = {
            zoned_prefix + "calendar-undefined.js",
            zoned_prefix + "prototype/epochMilliseconds/basic.js",
            zoned_prefix + "prototype/epochNanoseconds/basic.js",
            zoned_prefix + "timezone-wrong-type.js",
        }
        symbol = {
            zoned_prefix + "prototype/calendarId/branding.js",
            zoned_prefix + "prototype/epochMilliseconds/branding.js",
            zoned_prefix + "prototype/epochNanoseconds/branding.js",
            zoned_prefix + "prototype/timeZoneId/branding.js",
            zoned_prefix + "timezone-wrong-type.js",
        }
        expected_features = {
            relative: frozenset(
                {"Temporal"}
                | ({"BigInt"} if relative in bigint else set())
                | ({"Symbol"} if relative in symbol else set())
            )
            for relative in expected
        }
        self.assertEqual(TEMPORAL_ZONED_DATE_TIME_CORE_FILES, frozenset(expected))
        self.assertEqual(TEMPORAL_ZONED_DATE_TIME_CORE_FEATURES, expected_features)
        self.assertEqual(len(expected), 36)

        test_root = Path(test262_runner.TEST262) / "test"
        root_directory = test_root / zoned_prefix
        try:
            live_available = root_directory.is_dir()
        except OSError:
            live_available = False
        if live_available:
            blockers = {
                zoned_prefix + "calendar-wrong-type.js": (
                    {"BigInt", "Symbol", "Temporal"},
                    [],
                ),
            }
            admitted_elsewhere = {
                zoned_prefix + "construction-and-properties.js",
            }
            live_root = {
                path.relative_to(test_root).as_posix()
                for path in root_directory.glob("*.js")
            }
            self.assertEqual(
                live_root,
                {zoned_prefix + name for name in root_names}
                | set(blockers)
                | admitted_elsewhere,
            )
            live_prototype_root = {
                path.relative_to(test_root).as_posix()
                for path in (root_directory / "prototype").glob("*.js")
            }
            self.assertEqual(
                live_prototype_root,
                {
                    zoned_prefix + "prototype/constructor.js",
                    zoned_prefix + "prototype/prop-desc.js",
                },
            )
            for directory in ("calendarId", "epochMilliseconds", "epochNanoseconds", "timeZoneId"):
                live = {
                    path.relative_to(test_root).as_posix()
                    for path in (root_directory / "prototype" / directory).glob("*.js")
                }
                self.assertEqual(
                    live,
                    {
                        zoned_prefix + f"prototype/{directory}/{name}"
                        for name in ("basic.js", "branding.js", "prop-desc.js")
                    },
                )

            property_helper = {
                zoned_prefix + "length.js",
                zoned_prefix + "name.js",
                zoned_prefix + "prop-desc.js",
                zoned_prefix + "prototype/constructor.js",
                zoned_prefix + "prototype/prop-desc.js",
                zoned_prefix + "prototype/toStringTag/prop-desc.js",
            }
            instant_helpers = {
                "built-ins/Temporal/Instant/compare/argument-zoneddatetime.js",
                "built-ins/Temporal/Instant/from/argument-zoneddatetime.js",
                "built-ins/Temporal/Instant/prototype/equals/argument-zoneddatetime.js",
            }
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(frozenset(metadata.get("features", [])), features)
                includes = (
                    ["compareArray.js", "temporalHelpers.js"]
                    if relative in instant_helpers
                    else ["propertyHelper.js"]
                    if relative in property_helper
                    else []
                )
                self.assertEqual(metadata.get("includes", []), includes)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))

            for relative, (features, includes) in blockers.items():
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(set(metadata.get("features", [])), features)
                self.assertEqual(metadata.get("includes", []), includes)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(tool.should_skip(metadata, path))

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_zoned_date_time_core_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                TEMPORAL_ZONED_DATE_TIME_CORE_FILES.isdisjoint(existing),
                manifest.name,
            )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test" / zoned_prefix / "future.js"
            outside = root / "test/built-ins/Temporal/ZonedDateTime/prototype/add/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_zoned_date_time_core_path(path), relative)
                        self.assertEqual(
                            tool.temporal_zoned_date_time_core_features(path), features
                        )
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_zoned_date_time_core_path(path))
                        self.assertEqual(
                            tool.temporal_zoned_date_time_core_features(path), frozenset()
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_zoned_date_time_fixed_offset_manifest_is_exact_live_disjoint_and_shared(self):
        files = TEMPORAL_ZONED_DATE_TIME_FIXED_OFFSET_FILES
        features_by_file = TEMPORAL_ZONED_DATE_TIME_FIXED_OFFSET_FEATURES
        blockers = {
            line
            for raw_line in (
                Path(__file__).with_name(
                    "test262_temporal_zoned_date_time_fixed_offset_blockers.txt"
                )
            ).read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        }
        self.assertEqual(len(files), 253)
        self.assertEqual(len(blockers), 13)
        self.assertEqual(set(features_by_file), set(files))
        self.assertTrue(files.isdisjoint(blockers))
        self.assertTrue(files.isdisjoint(TEMPORAL_ZONED_DATE_TIME_CORE_FILES))

        compare_helpers = {
            "built-ins/Temporal/ZonedDateTime/from/disambiguation-wrong-type.js",
            "built-ins/Temporal/ZonedDateTime/from/infinity-throws-rangeerror.js",
            "built-ins/Temporal/ZonedDateTime/from/observable-get-overflow-argument-primitive.js",
            "built-ins/Temporal/ZonedDateTime/from/observable-get-overflow-argument-string-invalid.js",
            "built-ins/Temporal/ZonedDateTime/from/offset-wrong-type.js",
            "built-ins/Temporal/ZonedDateTime/from/options-read-before-algorithmic-validation.js",
            "built-ins/Temporal/ZonedDateTime/from/order-of-operations.js",
            "built-ins/Temporal/ZonedDateTime/from/overflow-wrong-type.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toString/calendarname-wrong-type.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toString/fractionalseconddigits-wrong-type.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toString/offset-wrong-type.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toString/options-read-before-algorithmic-validation.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toString/order-of-operations.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toString/roundingmode-wrong-type.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toString/smallestunit-wrong-type.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toString/timezonename-wrong-type.js",
        }
        temporal_helpers = {
            "built-ins/Temporal/ZonedDateTime/from/argument-propertybag-function-object.js",
            "built-ins/Temporal/ZonedDateTime/from/argument-propertybag-ignores-incorrect-properties.js",
            "built-ins/Temporal/ZonedDateTime/from/argument-propertybag-monthcode-month.js",
            "built-ins/Temporal/ZonedDateTime/from/argument-string-basic-and-extended-format.js",
            "built-ins/Temporal/ZonedDateTime/from/argument-string-decimal-places.js",
            "built-ins/Temporal/ZonedDateTime/from/argument-string-negative-extended-year.js",
            "built-ins/Temporal/ZonedDateTime/from/argument-string-optional-parts.js",
            "built-ins/Temporal/ZonedDateTime/from/argument-string-variant-decimal-separator.js",
            "built-ins/Temporal/ZonedDateTime/from/overflow-options.js",
            "built-ins/Temporal/ZonedDateTime/from/subclassing-ignored.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toInstant/recent-date.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toInstant/year-less-than-1.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toInstant/year-less-than-99.js",
            "built-ins/Temporal/ZonedDateTime/prototype/toInstant/year-zero-leap-day.js",
        }
        property_helper_directories = {"from", "toInstant", "toString", "toJSON", "valueOf"}

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            live_available = (test_root / "built-ins/Temporal/ZonedDateTime").is_dir()
        except OSError:
            live_available = False
        if live_available:
            zoned_root = test_root / "built-ins/Temporal/ZonedDateTime"
            surface = {
                path.relative_to(test_root).as_posix()
                for directory in (
                    zoned_root / "from",
                    zoned_root / "prototype/toInstant",
                    zoned_root / "prototype/toString",
                    zoned_root / "prototype/toJSON",
                    zoned_root / "prototype/valueOf",
                    *(zoned_root / "prototype" / name for name in (
                        "year", "month", "monthCode", "day", "hour", "minute",
                        "second", "millisecond", "microsecond", "nanosecond",
                        "era", "eraYear", "dayOfWeek", "dayOfYear", "weekOfYear",
                        "yearOfWeek", "hoursInDay", "daysInWeek", "daysInMonth",
                        "daysInYear", "monthsInYear", "inLeapYear",
                        "offsetNanoseconds", "offset",
                    )),
                )
                for path in directory.rglob("*.js")
                if "_FIXTURE" not in path.name
            }
            surface.add(
                "built-ins/Temporal/ZonedDateTime/construction-and-properties.js"
            )
            self.assertEqual(surface, set(files) | blockers)
            for relative in files:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])),
                    features_by_file[relative],
                    relative,
                )
                parts = Path(relative).parts
                if relative in compare_helpers:
                    includes = ["compareArray.js", "temporalHelpers.js"]
                elif relative in temporal_helpers:
                    includes = ["temporalHelpers.js"]
                elif path.name == "not-a-constructor.js":
                    includes = ["isConstructor.js"]
                elif (
                    path.name in {"length.js", "name.js", "prop-desc.js"}
                    and parts[-2] in property_helper_directories
                ):
                    includes = ["propertyHelper.js"]
                else:
                    includes = []
                self.assertEqual(metadata.get("includes", []), includes, relative)
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(
                        tool.temporal_zoned_date_time_fixed_offset_path(path), relative
                    )
                    self.assertEqual(
                        tool.temporal_zoned_date_time_fixed_offset_features(path),
                        features_by_file[relative],
                    )
                    self.assertFalse(tool.should_skip(metadata, path), relative)

            for relative in blockers:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertIn("Temporal", metadata.get("features", []), relative)
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(tool.should_skip(metadata, path), relative)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if (
                manifest.name
                == "test262_temporal_zoned_date_time_fixed_offset_admission.txt"
            ):
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(files.isdisjoint(existing), manifest.name)

        code_only_admissions = (
            CLASS_SUBCLASS_BUILTIN_FILES
            | MODULE_CLASS_ELEMENTS_FILES
            | MODULE_STATIC_SEMANTICS_FILES
            | MODULE_TLA_SYNTAX_FILES
            | MODULE_TLA_RUNTIME_FILES
            | WEAK_COLLECTION_FILES
            | WEAK_REFERENCE_FILES
        )
        self.assertTrue(files.isdisjoint(code_only_admissions))

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/ZonedDateTime/prototype/year/future.js"
            outside = root / "test/built-ins/Temporal/ZonedDateTime/constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in features_by_file.items():
                        path = root / "test" / relative
                        self.assertTrue(
                            tool.temporal_zoned_date_time_fixed_offset_path(path), relative
                        )
                        self.assertEqual(
                            tool.temporal_zoned_date_time_fixed_offset_features(path),
                            features,
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path), relative
                        )
                    for path in (future, outside):
                        self.assertFalse(
                            tool.temporal_zoned_date_time_fixed_offset_path(path)
                        )
                        self.assertEqual(
                            tool.temporal_zoned_date_time_fixed_offset_features(path),
                            frozenset(),
                        )
                    self.assertTrue(
                        tool.should_skip({"features": ["Temporal"]}, future)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_temporal_zoned_date_time_equals_manifest_is_exact_live_disjoint_and_shared(self):
        files = TEMPORAL_ZONED_DATE_TIME_EQUALS_FILES
        features_by_file = TEMPORAL_ZONED_DATE_TIME_EQUALS_FEATURES
        blockers = {
            line
            for raw_line in Path(__file__).with_name(
                "test262_temporal_zoned_date_time_equals_blockers.txt"
            ).read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        }
        self.assertEqual(len(files), 52)
        self.assertEqual(len(blockers), 3)
        self.assertEqual(set(features_by_file), set(files))
        self.assertTrue(files.isdisjoint(blockers))

        test_root = Path(test262_runner.TEST262) / "test"
        equals_dir = test_root / "built-ins/Temporal/ZonedDateTime/prototype/equals"
        try:
            live_files = (
                {
                    path.relative_to(test_root).as_posix()
                    for path in equals_dir.glob("*.js")
                    if "_FIXTURE" not in path.name
                }
                if equals_dir.is_dir()
                else None
            )
        except OSError:
            live_files = None
        if live_files is not None:
            self.assertEqual(live_files, set(files) | blockers)
            for relative in files:
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])),
                    features_by_file[relative],
                    relative,
                )
                if path.name in {"infinity-throws-rangeerror.js", "order-of-operations.js"}:
                    includes = ["compareArray.js", "temporalHelpers.js"]
                elif path.name in {"length.js", "name.js", "prop-desc.js"}:
                    includes = ["propertyHelper.js"]
                elif path.name == "not-a-constructor.js":
                    includes = ["isConstructor.js"]
                else:
                    includes = []
                self.assertEqual(metadata.get("includes", []), includes, relative)
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(tool.temporal_zoned_date_time_equals_path(path), relative)
                    self.assertEqual(
                        tool.temporal_zoned_date_time_equals_features(path),
                        features_by_file[relative],
                    )
                    self.assertFalse(tool.should_skip(metadata, path), relative)
            for relative in blockers:
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(tool.should_skip(metadata, path), relative)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_zoned_date_time_equals_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(files.isdisjoint(existing), manifest.name)

        code_only_admissions = (
            CLASS_SUBCLASS_BUILTIN_FILES
            | MODULE_CLASS_ELEMENTS_FILES
            | MODULE_STATIC_SEMANTICS_FILES
            | MODULE_TLA_SYNTAX_FILES
            | MODULE_TLA_RUNTIME_FILES
            | WEAK_COLLECTION_FILES
            | WEAK_REFERENCE_FILES
        )
        self.assertTrue(files.isdisjoint(code_only_admissions))

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/ZonedDateTime/prototype/equals/future.js"
            outside = root / "test/built-ins/Other/prototype/equals/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in features_by_file.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_zoned_date_time_equals_path(path), relative)
                        self.assertEqual(tool.temporal_zoned_date_time_equals_features(path), features)
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_zoned_date_time_equals_path(path))
                        self.assertEqual(
                            tool.temporal_zoned_date_time_equals_features(path), frozenset()
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_zoned_date_time_with_time_zone_manifest_is_exact_live_disjoint_and_shared(self):
        files = TEMPORAL_ZONED_DATE_TIME_WITH_TIME_ZONE_FILES
        features_by_file = TEMPORAL_ZONED_DATE_TIME_WITH_TIME_ZONE_FEATURES
        blockers = {
            line
            for raw_line in Path(__file__).with_name(
                "test262_temporal_zoned_date_time_with_time_zone_blockers.txt"
            ).read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        }
        self.assertEqual(len(files), 14)
        self.assertEqual(len(blockers), 2)
        self.assertEqual(set(features_by_file), set(files))
        self.assertTrue(files.isdisjoint(blockers))

        test_root = Path(test262_runner.TEST262) / "test"
        method_dir = test_root / "built-ins/Temporal/ZonedDateTime/prototype/withTimeZone"
        try:
            live_files = (
                {
                    path.relative_to(test_root).as_posix()
                    for path in method_dir.glob("*.js")
                    if "_FIXTURE" not in path.name
                }
                if method_dir.is_dir()
                else None
            )
        except OSError:
            live_files = None
        if live_files is not None:
            self.assertEqual(live_files, set(files) | blockers)
            for relative in files:
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])),
                    features_by_file[relative],
                    relative,
                )
                if path.name in {"length.js", "name.js", "prop-desc.js"}:
                    includes = ["propertyHelper.js"]
                elif path.name == "not-a-constructor.js":
                    includes = ["isConstructor.js"]
                elif path.name == "subclassing-ignored.js":
                    includes = ["temporalHelpers.js"]
                else:
                    includes = []
                self.assertEqual(metadata.get("includes", []), includes, relative)
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(
                        tool.temporal_zoned_date_time_with_time_zone_path(path),
                        relative,
                    )
                    self.assertEqual(
                        tool.temporal_zoned_date_time_with_time_zone_features(path),
                        features_by_file[relative],
                    )
                    self.assertFalse(tool.should_skip(metadata, path), relative)
            for relative in blockers:
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertIn("Temporal", metadata.get("features", []), relative)
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(tool.should_skip(metadata, path), relative)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if (
                manifest.name
                == "test262_temporal_zoned_date_time_with_time_zone_admission.txt"
            ):
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(files.isdisjoint(existing), manifest.name)

        code_only_admissions = (
            CLASS_SUBCLASS_BUILTIN_FILES
            | MODULE_CLASS_ELEMENTS_FILES
            | MODULE_STATIC_SEMANTICS_FILES
            | MODULE_TLA_SYNTAX_FILES
            | MODULE_TLA_RUNTIME_FILES
            | WEAK_COLLECTION_FILES
            | WEAK_REFERENCE_FILES
        )
        self.assertTrue(files.isdisjoint(code_only_admissions))

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/ZonedDateTime/prototype/withTimeZone/future.js"
            outside = root / "test/built-ins/Other/prototype/withTimeZone/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in features_by_file.items():
                        path = root / "test" / relative
                        self.assertTrue(
                            tool.temporal_zoned_date_time_with_time_zone_path(path),
                            relative,
                        )
                        self.assertEqual(
                            tool.temporal_zoned_date_time_with_time_zone_features(path),
                            features,
                        )
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                    for path in (future, outside):
                        self.assertFalse(
                            tool.temporal_zoned_date_time_with_time_zone_path(path)
                        )
                        self.assertEqual(
                            tool.temporal_zoned_date_time_with_time_zone_features(path),
                            frozenset(),
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_zoned_date_time_with_calendar_manifest_is_exact_live_disjoint_and_shared(self):
        files = TEMPORAL_ZONED_DATE_TIME_WITH_CALENDAR_FILES
        features_by_file = TEMPORAL_ZONED_DATE_TIME_WITH_CALENDAR_FEATURES
        blockers = {
            line
            for raw_line in Path(__file__).with_name(
                "test262_temporal_zoned_date_time_with_calendar_blockers.txt"
            ).read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        }
        self.assertEqual(len(files), 14)
        self.assertEqual(len(blockers), 2)
        self.assertEqual(set(features_by_file), set(files))
        self.assertTrue(files.isdisjoint(blockers))

        test_root = Path(test262_runner.TEST262) / "test"
        method_dir = test_root / "built-ins/Temporal/ZonedDateTime/prototype/withCalendar"
        try:
            live_files = (
                {
                    path.relative_to(test_root).as_posix()
                    for path in method_dir.glob("*.js")
                    if "_FIXTURE" not in path.name
                }
                if method_dir.is_dir()
                else None
            )
        except OSError:
            live_files = None
        if live_files is not None:
            self.assertEqual(live_files, set(files) | blockers)
            for relative in files:
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])),
                    features_by_file[relative],
                    relative,
                )
                if path.name in {"length.js", "name.js", "prop-desc.js"}:
                    includes = ["propertyHelper.js"]
                elif path.name == "not-a-constructor.js":
                    includes = ["isConstructor.js"]
                elif path.name == "subclassing-ignored.js":
                    includes = ["temporalHelpers.js"]
                else:
                    includes = []
                self.assertEqual(metadata.get("includes", []), includes, relative)
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(
                        tool.temporal_zoned_date_time_with_calendar_path(path),
                        relative,
                    )
                    self.assertEqual(
                        tool.temporal_zoned_date_time_with_calendar_features(path),
                        features_by_file[relative],
                    )
                    self.assertFalse(tool.should_skip(metadata, path), relative)
            for relative in blockers:
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertIn("Temporal", metadata.get("features", []), relative)
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(tool.should_skip(metadata, path), relative)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if (
                manifest.name
                == "test262_temporal_zoned_date_time_with_calendar_admission.txt"
            ):
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(files.isdisjoint(existing), manifest.name)

        code_only_admissions = (
            CLASS_SUBCLASS_BUILTIN_FILES
            | MODULE_CLASS_ELEMENTS_FILES
            | MODULE_STATIC_SEMANTICS_FILES
            | MODULE_TLA_SYNTAX_FILES
            | MODULE_TLA_RUNTIME_FILES
            | WEAK_COLLECTION_FILES
            | WEAK_REFERENCE_FILES
        )
        self.assertTrue(files.isdisjoint(code_only_admissions))

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/ZonedDateTime/prototype/withCalendar/future.js"
            outside = root / "test/built-ins/Other/prototype/withCalendar/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in features_by_file.items():
                        path = root / "test" / relative
                        self.assertTrue(
                            tool.temporal_zoned_date_time_with_calendar_path(path),
                            relative,
                        )
                        self.assertEqual(
                            tool.temporal_zoned_date_time_with_calendar_features(path),
                            features,
                        )
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                    for path in (future, outside):
                        self.assertFalse(
                            tool.temporal_zoned_date_time_with_calendar_path(path)
                        )
                        self.assertEqual(
                            tool.temporal_zoned_date_time_with_calendar_features(path),
                            frozenset(),
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_zoned_date_time_compare_manifest_is_exact_live_disjoint_and_shared(self):
        files = TEMPORAL_ZONED_DATE_TIME_COMPARE_FILES
        features_by_file = TEMPORAL_ZONED_DATE_TIME_COMPARE_FEATURES
        blockers = {
            line
            for raw_line in Path(__file__).with_name(
                "test262_temporal_zoned_date_time_compare_blockers.txt"
            ).read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        }
        self.assertEqual(len(files), 46)
        self.assertEqual(len(blockers), 4)
        self.assertEqual(set(features_by_file), set(files))
        self.assertTrue(files.isdisjoint(blockers))

        test_root = Path(test262_runner.TEST262) / "test"
        method_dir = test_root / "built-ins/Temporal/ZonedDateTime/compare"
        try:
            live_files = (
                {
                    path.relative_to(test_root).as_posix()
                    for path in method_dir.glob("*.js")
                    if "_FIXTURE" not in path.name
                }
                if method_dir.is_dir()
                else None
            )
        except OSError:
            live_files = None
        if live_files is not None:
            self.assertEqual(live_files, set(files) | blockers)
            for relative in files:
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])),
                    features_by_file[relative],
                    relative,
                )
                if path.name in {"length.js", "name.js", "prop-desc.js"}:
                    includes = ["propertyHelper.js"]
                elif path.name == "not-a-constructor.js":
                    includes = ["isConstructor.js"]
                elif path.name in {"infinity-throws-rangeerror.js", "order-of-operations.js"}:
                    includes = ["compareArray.js", "temporalHelpers.js"]
                else:
                    includes = []
                self.assertEqual(metadata.get("includes", []), includes, relative)
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(tool.temporal_zoned_date_time_compare_path(path), relative)
                    self.assertEqual(
                        tool.temporal_zoned_date_time_compare_features(path),
                        features_by_file[relative],
                    )
                    self.assertFalse(tool.should_skip(metadata, path), relative)
            for relative in blockers:
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertIn("Temporal", metadata.get("features", []), relative)
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(tool.should_skip(metadata, path), relative)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_zoned_date_time_compare_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(files.isdisjoint(existing), manifest.name)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/ZonedDateTime/compare/future.js"
            outside = root / "test/built-ins/Other/compare/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in features_by_file.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_zoned_date_time_compare_path(path), relative)
                        self.assertEqual(
                            tool.temporal_zoned_date_time_compare_features(path), features
                        )
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_zoned_date_time_compare_path(path))
                        self.assertEqual(
                            tool.temporal_zoned_date_time_compare_features(path), frozenset()
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_instant_compare_manifest_is_exact_live_disjoint_and_shared(self):
        prefix = "built-ins/Temporal/Instant/compare/"
        names = {
            "argument-object-tostring.js",
            "argument-string-calendar-annotation-invalid-key.js",
            "argument-string-calendar-annotation.js",
            "argument-string-critical-unknown-annotation.js",
            "argument-string-date-with-utc-offset.js",
            "argument-string-invalid.js",
            "argument-string-limits.js",
            "argument-string-minus-sign.js",
            "argument-string-multiple-calendar.js",
            "argument-string-multiple-time-zone.js",
            "argument-string-time-separators.js",
            "argument-string-time-zone-annotation.js",
            "argument-string-too-many-decimals.js",
            "argument-string-unknown-annotation.js",
            "argument-string-with-offset-not-valid-epoch-nanoseconds.js",
            "argument-wrong-type.js",
            "builtin.js",
            "cross-epoch.js",
            "exhaustive.js",
            "instant-string-multiple-offsets.js",
            "instant-string-sub-minute-offset.js",
            "instant-string.js",
            "leap-second.js",
            "length.js",
            "name.js",
            "no-fractional-minutes-hours.js",
            "not-a-constructor.js",
            "prop-desc.js",
            "year-zero.js",
        }
        expected = {prefix + name for name in names}
        expected_features = {}
        for relative in expected:
            name = Path(relative).name
            features = {"Temporal"}
            if name == "argument-wrong-type.js":
                features.update({"BigInt", "Symbol"})
            if name == "argument-string-invalid.js":
                features.add("arrow-function")
            if name == "not-a-constructor.js":
                features.add("Reflect.construct")
            expected_features[relative] = frozenset(features)
        self.assertEqual(TEMPORAL_INSTANT_COMPARE_FILES, frozenset(expected))
        self.assertEqual(TEMPORAL_INSTANT_COMPARE_FEATURES, expected_features)
        self.assertEqual(len(expected), 29)

        test_root = Path(test262_runner.TEST262) / "test"
        directory = test_root / prefix
        try:
            live_available = directory.is_dir()
        except OSError:
            live_available = False
        if live_available:
            blocker = prefix + "argument-zoneddatetime.js"
            live = {
                path.relative_to(test_root).as_posix()
                for path in directory.glob("*.js")
            }
            self.assertEqual(live, expected | {blocker})
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(frozenset(metadata.get("features", [])), features)
                includes = (
                    ["propertyHelper.js"]
                    if path.name in {"length.js", "name.js", "prop-desc.js"}
                    else ["isConstructor.js"]
                    if path.name == "not-a-constructor.js"
                    else []
                )
                self.assertEqual(metadata.get("includes", []), includes)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))

            blocker_path = test_root / blocker
            metadata = test262_runner.parse_meta(blocker_path.read_text())
            self.assertEqual(set(metadata.get("features", [])), {"Temporal"})
            self.assertEqual(
                metadata.get("includes", []),
                ["compareArray.js", "temporalHelpers.js"],
            )
            self.assertEqual(metadata.get("flags", []), [])
            self.assertIsNone(metadata.get("negative"))
            for tool in (test262_runner, test262_analyze):
                self.assertFalse(tool.should_skip(metadata, blocker_path))

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_instant_compare_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(TEMPORAL_INSTANT_COMPARE_FILES.isdisjoint(existing), manifest.name)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test" / prefix / "future.js"
            outside = root / "test/built-ins/Temporal/Instant/prototype/compare/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_instant_compare_path(path), relative)
                        self.assertEqual(tool.temporal_instant_compare_features(path), features)
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_instant_compare_path(path))
                        self.assertEqual(
                            tool.temporal_instant_compare_features(path), frozenset()
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_instant_equals_manifest_is_exact_live_disjoint_and_shared(self):
        prefix = "built-ins/Temporal/Instant/prototype/equals/"
        expected = {
            prefix + name
            for name in (
                "basic.js",
                "branding.js",
                "builtin.js",
                "length.js",
                "name.js",
                "not-a-constructor.js",
                "prop-desc.js",
            )
        }
        expected_features = {
            path: frozenset(
                {"Temporal"}
                | ({"Symbol"} if path.endswith("/branding.js") else set())
                | (
                    {"Reflect.construct"}
                    if path.endswith("/not-a-constructor.js")
                    else set()
                )
            )
            for path in expected
        }
        self.assertEqual(TEMPORAL_INSTANT_EQUALS_FILES, frozenset(expected))
        self.assertEqual(TEMPORAL_INSTANT_EQUALS_FEATURES, expected_features)

        test_root = Path(test262_runner.TEST262) / "test"
        equals_dir = test_root / prefix
        try:
            live_metadata = (
                {
                    relative: test262_runner.parse_meta(
                        (test_root / relative).read_text()
                    )
                    for relative in expected_features
                }
                if equals_dir.is_dir()
                else None
            )
        except OSError:
            live_metadata = None
        if live_metadata is not None:
            for relative, features in expected_features.items():
                metadata = live_metadata[relative]
                self.assertEqual(frozenset(metadata.get("features", [])), features)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_instant_equals_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                TEMPORAL_INSTANT_EQUALS_FILES.isdisjoint(existing), manifest.name
            )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/Instant/prototype/equals/future.js"
            outside = root / "test/built-ins/Other/prototype/equals/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_instant_equals_path(path), relative)
                        self.assertEqual(tool.temporal_instant_equals_features(path), features)
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_instant_equals_path(path))
                        self.assertEqual(
                            tool.temporal_instant_equals_features(path), frozenset()
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_instant_value_of_manifest_is_exact_live_disjoint_and_shared(self):
        prefix = "built-ins/Temporal/Instant/prototype/valueOf/"
        expected = {
            prefix + name
            for name in (
                "basic.js",
                "branding.js",
                "builtin.js",
                "length.js",
                "name.js",
                "not-a-constructor.js",
                "prop-desc.js",
            )
        }
        expected_features = {
            path: frozenset(
                {"Temporal"}
                | ({"Symbol"} if path.endswith("/branding.js") else set())
                | (
                    {"Reflect.construct"}
                    if path.endswith("/not-a-constructor.js")
                    else set()
                )
            )
            for path in expected
        }
        self.assertEqual(TEMPORAL_INSTANT_VALUE_OF_FILES, frozenset(expected))
        self.assertEqual(TEMPORAL_INSTANT_VALUE_OF_FEATURES, expected_features)

        test_root = Path(test262_runner.TEST262) / "test"
        value_of_dir = test_root / prefix
        try:
            live_files = (
                {
                    path.relative_to(test_root).as_posix()
                    for path in value_of_dir.glob("*.js")
                }
                if value_of_dir.is_dir()
                else None
            )
        except OSError:
            live_files = None
        if live_files is not None:
            self.assertEqual(live_files, expected)
            for relative, features in expected_features.items():
                metadata = test262_runner.parse_meta((test_root / relative).read_text())
                filename = Path(relative).name
                if filename in {"length.js", "name.js", "prop-desc.js"}:
                    expected_includes = ["propertyHelper.js"]
                elif filename == "not-a-constructor.js":
                    expected_includes = ["isConstructor.js"]
                else:
                    expected_includes = []
                self.assertEqual(frozenset(metadata.get("features", [])), features)
                self.assertEqual(metadata.get("includes", []), expected_includes)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_instant_value_of_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                TEMPORAL_INSTANT_VALUE_OF_FILES.isdisjoint(existing), manifest.name
            )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/Instant/prototype/valueOf/future.js"
            outside = root / "test/built-ins/Other/prototype/valueOf/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_instant_value_of_path(path), relative)
                        self.assertEqual(
                            tool.temporal_instant_value_of_features(path), features
                        )
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_instant_value_of_path(path))
                        self.assertEqual(
                            tool.temporal_instant_value_of_features(path), frozenset()
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_instant_from_manifest_is_exact_live_disjoint_and_shared(self):
        expected = {
            "built-ins/Temporal/Instant/from/argument-instant.js",
            "built-ins/Temporal/Instant/from/argument-object-tostring.js",
            "built-ins/Temporal/Instant/from/argument-string-time-separators.js",
            "built-ins/Temporal/Instant/from/basic.js",
            "built-ins/Temporal/Instant/from/builtin.js",
            "built-ins/Temporal/Instant/from/leap-second.js",
            "built-ins/Temporal/Instant/from/length.js",
            "built-ins/Temporal/Instant/from/name.js",
            "built-ins/Temporal/Instant/from/not-a-constructor.js",
            "built-ins/Temporal/Instant/from/prop-desc.js",
            "built-ins/Temporal/Instant/from/subclassing-ignored.js",
            "built-ins/Temporal/Instant/prototype/equals/argument-object-tostring.js",
            "built-ins/Temporal/Instant/prototype/equals/argument-string-time-separators.js",
            "built-ins/Temporal/Instant/prototype/equals/cross-epoch.js",
            "built-ins/Temporal/Instant/prototype/equals/leap-second.js",
        }
        expected_features = {
            path: frozenset(
                {"Temporal"}
                | (
                    {"Reflect.construct"}
                    if path.endswith("/not-a-constructor.js")
                    else set()
                )
            )
            for path in expected
        }
        self.assertEqual(TEMPORAL_INSTANT_FROM_FILES, frozenset(expected))
        self.assertEqual(TEMPORAL_INSTANT_FROM_FEATURES, expected_features)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            live_metadata = {
                relative: test262_runner.parse_meta((test_root / relative).read_text())
                for relative in expected_features
            }
        except OSError:
            live_metadata = None
        if live_metadata is not None:
            for relative, features in expected_features.items():
                metadata = live_metadata[relative]
                self.assertEqual(frozenset(metadata.get("features", [])), features)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_instant_from_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(TEMPORAL_INSTANT_FROM_FILES.isdisjoint(existing), manifest.name)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/Instant/from/future.js"
            outside = root / "test/built-ins/Other/from/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_instant_from_path(path), relative)
                        self.assertEqual(tool.temporal_instant_from_features(path), features)
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_instant_from_path(path))
                        self.assertEqual(tool.temporal_instant_from_features(path), frozenset())
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_instant_string_parser_manifest_is_exact_live_disjoint_and_shared(self):
        from_prefix = "built-ins/Temporal/Instant/from/"
        equals_prefix = "built-ins/Temporal/Instant/prototype/equals/"
        common = {
            "argument-string-calendar-annotation-invalid-key.js",
            "argument-string-calendar-annotation.js",
            "argument-string-critical-unknown-annotation.js",
            "argument-string-date-with-utc-offset.js",
            "argument-string-invalid.js",
            "argument-string-limits.js",
            "argument-string-minus-sign.js",
            "argument-string-multiple-calendar.js",
            "argument-string-multiple-time-zone.js",
            "argument-string-time-zone-annotation.js",
            "argument-string-too-many-decimals.js",
            "argument-string-unknown-annotation.js",
            "instant-string-multiple-offsets.js",
            "instant-string-sub-minute-offset.js",
            "instant-string.js",
            "no-fractional-minutes-hours.js",
            "year-zero.js",
        }
        expected = {from_prefix + name for name in common} | {
            equals_prefix + name for name in common
        } | {
            from_prefix + "argument-string.js",
            from_prefix + "timezone-custom.js",
        }
        expected_features = {
            path: frozenset(
                {"Temporal"}
                | (
                    {"arrow-function"}
                    if path.endswith(("/argument-string-invalid.js", "/year-zero.js"))
                    else set()
                )
            )
            for path in expected
        }
        self.assertEqual(TEMPORAL_INSTANT_STRING_PARSER_FILES, frozenset(expected))
        self.assertEqual(TEMPORAL_INSTANT_STRING_PARSER_FEATURES, expected_features)
        self.assertEqual(len(expected), 36)

        test_root = Path(test262_runner.TEST262) / "test"
        directories = (test_root / from_prefix, test_root / equals_prefix)
        try:
            live_available = all(path.is_dir() for path in directories)
        except OSError:
            live_available = False
        if live_available:
            for relative, features in expected_features.items():
                metadata = test262_runner.parse_meta((test_root / relative).read_text())
                self.assertEqual(frozenset(metadata.get("features", [])), features)
                self.assertEqual(metadata.get("includes", []), [])
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))

            live = {
                path.relative_to(test_root).as_posix()
                for directory in directories
                for path in directory.glob("*.js")
            }
            blockers = {
                prefix + "argument-zoneddatetime.js"
                for prefix in (from_prefix, equals_prefix)
            }
            self.assertLessEqual(blockers, live)
            for relative in blockers:
                metadata = test262_runner.parse_meta((test_root / relative).read_text())
                self.assertEqual(set(metadata.get("features", [])), {"Temporal"})
                self.assertEqual(
                    metadata.get("includes", []),
                    ["compareArray.js", "temporalHelpers.js"],
                )
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))
                for tool in (test262_runner, test262_analyze):
                    self.assertFalse(tool.should_skip(metadata, test_root / relative))
            owned = (
                TEMPORAL_INSTANT_FROM_FILES
                | TEMPORAL_INSTANT_EQUALS_FILES
                | TEMPORAL_INSTANT_STRING_PARSER_FILES
                | TEMPORAL_INSTANT_TO_STRING_FILES
                | TEMPORAL_ZONED_DATE_TIME_CORE_FILES
            ) & live
            self.assertEqual(owned, live)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_instant_string_parser_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                TEMPORAL_INSTANT_STRING_PARSER_FILES.isdisjoint(existing),
                manifest.name,
            )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/Instant/from/future.js"
            outside = root / "test/built-ins/Other/from/argument-string.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_instant_string_parser_path(path))
                        self.assertEqual(
                            tool.temporal_instant_string_parser_features(path), features
                        )
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_instant_string_parser_path(path))
                        self.assertEqual(
                            tool.temporal_instant_string_parser_features(path), frozenset()
                        )
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_temporal_instant_to_string_manifest_is_exact_live_disjoint_and_shared(self):
        prefix = "built-ins/Temporal/Instant/prototype/toString/"
        names = {
            "basic.js",
            "branding.js",
            "builtin.js",
            "fractionalseconddigits-auto.js",
            "fractionalseconddigits-invalid-string.js",
            "fractionalseconddigits-nan.js",
            "fractionalseconddigits-negative.js",
            "fractionalseconddigits-non-integer.js",
            "fractionalseconddigits-number.js",
            "fractionalseconddigits-out-of-range.js",
            "fractionalseconddigits-undefined.js",
            "fractionalseconddigits-wrong-type.js",
            "get-timezone-throws.js",
            "length.js",
            "name.js",
            "negative-epochnanoseconds.js",
            "negative-instant-rounding.js",
            "not-a-constructor.js",
            "options-object.js",
            "options-read-before-algorithmic-validation.js",
            "options-undefined.js",
            "options-wrong-type.js",
            "order-of-operations.js",
            "precision.js",
            "prop-desc.js",
            "rounding-cross-midnight.js",
            "rounding-direction.js",
            "roundingmode-ceil.js",
            "roundingmode-expand.js",
            "roundingmode-floor.js",
            "roundingmode-halfCeil.js",
            "roundingmode-halfEven.js",
            "roundingmode-halfExpand.js",
            "roundingmode-halfFloor.js",
            "roundingmode-halfTrunc.js",
            "roundingmode-invalid-string.js",
            "roundingmode-trunc.js",
            "roundingmode-undefined.js",
            "roundingmode-wrong-type.js",
            "smallestunit-fractionalseconddigits.js",
            "smallestunit-invalid-string.js",
            "smallestunit-undefined.js",
            "smallestunit-valid-units.js",
            "smallestunit-wrong-type.js",
            "timezone-offset.js",
            "timezone-string-datetime.js",
            "timezone-string-leap-second.js",
            "timezone-string-multiple-offsets.js",
            "timezone-string-sub-minute-offset.js",
            "timezone-string-year-zero.js",
            "timezone-string.js",
            "year-format.js",
        }
        wrong_type_paths = {
            "built-ins/Temporal/Instant/from/argument-wrong-type.js",
            "built-ins/Temporal/Instant/prototype/equals/argument-wrong-type.js",
        }
        expected = {prefix + name for name in names} | wrong_type_paths
        bigint_names = {
            "basic.js",
            "fractionalseconddigits-auto.js",
            "fractionalseconddigits-negative.js",
            "fractionalseconddigits-number.js",
            "options-wrong-type.js",
            "timezone-offset.js",
        }
        expected_features = {}
        for relative in expected:
            name = Path(relative).name
            features = {"Temporal"}
            if name in bigint_names or name == "argument-wrong-type.js":
                features.add("BigInt")
            if name in {
                "branding.js",
                "options-wrong-type.js",
                "argument-wrong-type.js",
            }:
                features.add("Symbol")
            if name == "not-a-constructor.js":
                features.add("Reflect.construct")
            if name == "timezone-string-year-zero.js":
                features.add("arrow-function")
            expected_features[relative] = frozenset(features)
        self.assertEqual(TEMPORAL_INSTANT_TO_STRING_FILES, frozenset(expected))
        self.assertEqual(TEMPORAL_INSTANT_TO_STRING_FEATURES, expected_features)
        self.assertEqual(len(expected), 54)

        helper_includes = {
            "fractionalseconddigits-wrong-type.js",
            "options-read-before-algorithmic-validation.js",
            "order-of-operations.js",
            "roundingmode-wrong-type.js",
            "smallestunit-wrong-type.js",
        }
        property_includes = {"length.js", "name.js", "prop-desc.js"}
        test_root = Path(test262_runner.TEST262) / "test"
        directory = test_root / prefix
        try:
            live_available = directory.is_dir()
        except OSError:
            live_available = False
        if live_available:
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(frozenset(metadata.get("features", [])), features)
                name = path.name
                includes = (
                    ["compareArray.js", "temporalHelpers.js"]
                    if name in helper_includes
                    else ["propertyHelper.js"]
                    if name in property_includes
                    else ["isConstructor.js"]
                    if name == "not-a-constructor.js"
                    else []
                )
                self.assertEqual(metadata.get("includes", []), includes)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))

            blockers = {
                prefix + "smallestunit-plurals-accepted.js": (
                    {"Temporal"},
                    ["temporalHelpers.js"],
                ),
                prefix + "timezone-wrong-type.js": (
                    {"BigInt", "Symbol", "Temporal"},
                    [],
                ),
            }
            live = {
                path.relative_to(test_root).as_posix()
                for path in directory.glob("*.js")
            }
            self.assertEqual(live, {prefix + name for name in names} | set(blockers))
            for relative, (features, includes) in blockers.items():
                metadata = test262_runner.parse_meta((test_root / relative).read_text())
                self.assertEqual(set(metadata.get("features", [])), features)
                self.assertEqual(metadata.get("includes", []), includes)
                self.assertEqual(metadata.get("flags", []), [])
                self.assertIsNone(metadata.get("negative"))
                for tool in (test262_runner, test262_analyze):
                    self.assertTrue(tool.should_skip(metadata, test_root / relative))

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_temporal_instant_to_string_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(TEMPORAL_INSTANT_TO_STRING_FILES.isdisjoint(existing), manifest.name)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test" / prefix / "future.js"
            outside = root / "test/built-ins/Temporal/Instant/prototype/round/basic.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_instant_to_string_path(path), relative)
                        self.assertEqual(tool.temporal_instant_to_string_features(path), features)
                        self.assertFalse(tool.should_skip({"features": sorted(features)}, path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.temporal_instant_to_string_path(path))
                        self.assertEqual(tool.temporal_instant_to_string_features(path), frozenset())
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                finally:
                    tool.TEST262 = original_root

    def test_object_group_by_manifest_is_exact_live_disjoint_and_shared(self):
        expected = {
            "built-ins/Object/groupBy/iterator-next-throws.js": {
                "array-grouping",
                "Symbol.iterator",
            }
        }
        self.assertEqual(OBJECT_GROUP_BY_FILES, frozenset(expected))
        self.assertEqual(
            OBJECT_GROUP_BY_FEATURES,
            {path: frozenset(features) for path, features in expected.items()},
        )

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_object_group_by_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(OBJECT_GROUP_BY_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in OBJECT_GROUP_BY_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted = root / "test/built-ins/Object/groupBy/iterator-next-throws.js"
            future = root / "test/built-ins/Object/groupBy/future.js"
            outside = root / "test/built-ins/Set/groupBy/iterator-next-throws.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.object_group_by_path(admitted))
                    self.assertEqual(
                        tool.object_group_by_features(admitted),
                        expected["built-ins/Object/groupBy/iterator-next-throws.js"],
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {"features": sorted(OBJECT_GROUP_BY_FEATURES[next(iter(expected))])},
                            admitted,
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "features": sorted(
                                    OBJECT_GROUP_BY_FEATURES[next(iter(expected))]
                                    | {"Proxy"}
                                )
                            },
                            admitted,
                        )
                    )
                    for path in (future, outside):
                        self.assertFalse(tool.object_group_by_path(path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": ["Symbol.iterator"]}, path
                            )
                        )
                finally:
                    tool.TEST262 = original_root

    def test_map_group_by_manifest_is_exact_live_disjoint_and_shared(self):
        expected = {
            "built-ins/Map/groupBy/groupLength.js": {
                "array-grouping",
                "Map",
                "Symbol.iterator",
            },
            "built-ins/Map/groupBy/iterator-next-throws.js": {
                "array-grouping",
                "Map",
                "Symbol.iterator",
            },
        }
        self.assertEqual(MAP_GROUP_BY_FILES, frozenset(expected))
        self.assertEqual(
            MAP_GROUP_BY_FEATURES,
            {path: frozenset(features) for path, features in expected.items()},
        )

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_map_group_by_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(MAP_GROUP_BY_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in MAP_GROUP_BY_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Map/groupBy/future.js"
            outside = root / "test/built-ins/Set/groupBy/groupLength.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.map_group_by_path(path))
                        self.assertEqual(tool.map_group_by_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"Proxy"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.map_group_by_path(path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": ["Symbol.iterator"]}, path
                            )
                        )
                finally:
                    tool.TEST262 = original_root

    def test_map_constructor_manifest_is_exact_live_disjoint_and_shared(self):
        symbol_iterator = {
            "built-ins/Map/iterator-close-after-set-failure.js",
            "built-ins/Map/iterator-close-failure-after-set-failure.js",
            "built-ins/Map/iterator-is-undefined-throws.js",
            "built-ins/Map/iterator-item-first-entry-returns-abrupt.js",
            "built-ins/Map/iterator-item-second-entry-returns-abrupt.js",
            "built-ins/Map/iterator-next-failure.js",
            "built-ins/Map/iterator-value-failure.js",
        }
        expected = {
            **{path: {"Symbol.iterator"} for path in symbol_iterator},
            "built-ins/Map/iterator-items-are-not-object.js": {"Symbol"},
            "built-ins/Map/proto-from-ctor-realm.js": {"cross-realm", "Reflect"},
        }
        self.assertEqual(MAP_CONSTRUCTOR_FILES, frozenset(expected))
        self.assertEqual(
            MAP_CONSTRUCTOR_FEATURES,
            {path: frozenset(features) for path, features in expected.items()},
        )

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_map_constructor_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(MAP_CONSTRUCTOR_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in MAP_CONSTRUCTOR_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Map/future-constructor.js"
            outside = root / "test/built-ins/Set/iterator-next-failure.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.map_constructor_path(path))
                        self.assertEqual(tool.map_constructor_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"Proxy"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.map_constructor_path(path))
                        self.assertTrue(
                            tool.should_skip({"features": ["Symbol.iterator"]}, path)
                        )
                finally:
                    tool.TEST262 = original_root

    def test_set_constructor_manifest_is_exact_live_disjoint_and_shared(self):
        symbol_iterator = {
            "built-ins/Set/set-iterator-close-after-add-failure.js",
            "built-ins/Set/set-iterator-next-failure.js",
            "built-ins/Set/set-iterator-value-failure.js",
        }
        expected = {
            **{path: {"Symbol.iterator"} for path in symbol_iterator},
            "built-ins/Set/proto-from-ctor-realm.js": {"cross-realm", "Reflect"},
        }
        self.assertEqual(SET_CONSTRUCTOR_FILES, frozenset(expected))
        self.assertEqual(
            SET_CONSTRUCTOR_FEATURES,
            {path: frozenset(features) for path, features in expected.items()},
        )

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_set_constructor_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(SET_CONSTRUCTOR_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in SET_CONSTRUCTOR_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Set/future-constructor.js"
            outside = root / "test/built-ins/Map/set-iterator-next-failure.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.set_constructor_path(path))
                        self.assertEqual(tool.set_constructor_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"Proxy"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.set_constructor_path(path))
                        self.assertTrue(
                            tool.should_skip({"features": ["Symbol.iterator"]}, path)
                        )
                finally:
                    tool.TEST262 = original_root

    def test_set_algebra_manifest_is_exact_live_disjoint_and_shared(self):
        methods = {
            "difference",
            "intersection",
            "isDisjointFrom",
            "isSubsetOf",
            "isSupersetOf",
            "symmetricDifference",
            "union",
        }
        expected = {
            f"built-ins/Set/prototype/{method}/not-a-constructor.js": {
                "Reflect.construct",
                "set-methods",
            }
            for method in methods
        }
        self.assertEqual(SET_ALGEBRA_FILES, frozenset(expected))
        self.assertEqual(
            SET_ALGEBRA_FEATURES,
            {path: frozenset(features) for path, features in expected.items()},
        )

        for name, files in vars(test262_runner).items():
            if name == "SET_ALGEBRA_FILES" or not name.endswith("_FILES"):
                continue
            if isinstance(files, (set, frozenset)) and all(
                isinstance(path, str) for path in files
            ):
                self.assertFalse(SET_ALGEBRA_FILES & files, name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in SET_ALGEBRA_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Set/prototype/union/future.js"
            outside = root / "test/built-ins/Map/prototype/union/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.set_algebra_path(path))
                        self.assertEqual(tool.set_algebra_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"Proxy"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.set_algebra_path(path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": ["Reflect.construct", "set-methods"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root

    def test_weak_collection_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(WEAK_COLLECTION_FILES), 95)
        self.assertEqual(
            WEAK_COLLECTION_FILES, frozenset(WEAK_COLLECTION_FEATURES)
        )
        self.assertTrue(
            all(
                path.startswith(("built-ins/WeakMap/", "built-ins/WeakSet/"))
                for path in WEAK_COLLECTION_FILES
            )
        )

        tools_dir = Path(__file__).resolve().parent
        manifest = tools_dir / "test262_weak_collection_admission.data"
        frozen = {}
        for raw_line in manifest.read_text().splitlines():
            line = raw_line.strip()
            if not line or line.startswith("#"):
                continue
            path, separator, raw_features = line.partition("|")
            self.assertEqual(separator, "|", raw_line)
            self.assertNotIn(path, frozen)
            frozen[path] = frozenset(raw_features.split(","))
        self.assertEqual(frozen, WEAK_COLLECTION_FEATURES)

        for name, files in vars(test262_runner).items():
            if name == "WEAK_COLLECTION_FILES" or not name.endswith("_FILES"):
                continue
            if not isinstance(files, (set, frozenset)) or not all(
                isinstance(path, str) for path in files
            ):
                continue
            self.assertFalse(WEAK_COLLECTION_FILES & files, name)

        strict_path = (
            "built-ins/WeakMap/prototype/getOrInsertComputed/"
            "check-callback-fn-args.js"
        )
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in WEAK_COLLECTION_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("flags", []),
                    ["onlyStrict"] if relative == strict_path else [],
                    relative,
                )
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/WeakMap/future.js"
            outside = root / "test/built-ins/Map/weak.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in WEAK_COLLECTION_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.weak_collection_path(path))
                        self.assertEqual(
                            tool.weak_collection_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"Proxy"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.weak_collection_path(path))
                        self.assertTrue(
                            tool.should_skip({"features": ["WeakMap"]}, path)
                        )
                finally:
                    tool.TEST262 = original_root

    def test_weak_reference_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(WEAK_REFERENCE_FILES), 76)
        self.assertEqual(
            WEAK_REFERENCE_FILES, frozenset(WEAK_REFERENCE_FEATURES)
        )
        self.assertEqual(
            sum(path.startswith("built-ins/WeakRef/") for path in WEAK_REFERENCE_FILES),
            29,
        )
        self.assertEqual(
            sum(
                path.startswith("built-ins/FinalizationRegistry/")
                for path in WEAK_REFERENCE_FILES
            ),
            47,
        )

        for name, files in vars(test262_runner).items():
            if name == "WEAK_REFERENCE_FILES" or not name.endswith("_FILES"):
                continue
            if not isinstance(files, (set, frozenset)) or not all(
                isinstance(path, str) for path in files
            ):
                continue
            self.assertFalse(WEAK_REFERENCE_FILES & files, name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            live = {
                path.relative_to(test_root).as_posix()
                for area in ("WeakRef", "FinalizationRegistry")
                for path in (test_root / "built-ins" / area).rglob("*.js")
            }
            self.assertEqual(live, WEAK_REFERENCE_FILES)
            for relative, features in WEAK_REFERENCE_FEATURES.items():
                path = test_root / relative
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            weak = root / "test/built-ins/WeakRef/constructor.js"
            registry = root / "test/built-ins/FinalizationRegistry/constructor.js"
            weak_future = root / "test/built-ins/WeakRef/future.js"
            registry_future = (
                root / "test/built-ins/FinalizationRegistry/future.js"
            )
            outside = root / "test/built-ins/Map/weak-reference.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.weak_ref_path(weak))
                    self.assertTrue(tool.finalization_registry_path(registry))
                    self.assertFalse(
                        tool.should_skip({"features": ["WeakRef"]}, weak)
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {"features": ["FinalizationRegistry"]}, registry
                        )
                    )
                    for path in (weak_future, registry_future, outside):
                        self.assertFalse(tool.weak_ref_path(path))
                        self.assertFalse(tool.finalization_registry_path(path))
                    self.assertTrue(
                        tool.should_skip({"features": ["WeakRef"]}, weak_future)
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["FinalizationRegistry"]},
                            registry_future,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {"features": ["FinalizationRegistry"]}, outside
                        )
                    )
                finally:
                    tool.TEST262 = original_root

        self.assertEqual(
            weak_reference_features("built-ins/WeakRef/constructor.js"),
            frozenset({"WeakRef"}),
        )
        self.assertEqual(
            weak_reference_features(
                "built-ins/FinalizationRegistry/proto-from-ctor-realm.js"
            ),
            frozenset(
                {"FinalizationRegistry", "cross-realm", "Reflect", "Symbol"}
            ),
        )
        self.assertEqual(
            weak_reference_features("built-ins/WeakRef/future.js"), frozenset()
        )

    def test_native_construct_manifest_is_exact_and_shared(self):
        expected = {
            "built-ins/BigInt/is-a-constructor.js": {"Reflect.construct"},
            "built-ins/Symbol/is-constructor.js": {
                "Symbol", "Reflect.construct",
            },
            "built-ins/Proxy/constructor.js": {"Proxy"},
            "built-ins/Proxy/proxy-newtarget.js": {"Proxy"},
            "built-ins/Proxy/proxy-undefined-newtarget.js": {"Proxy"},
            "built-ins/String/is-a-constructor.js": {"Reflect.construct"},
            "built-ins/String/proto-from-ctor-realm.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/String/symbol-string-coercion.js": {"Symbol"},
            "built-ins/String/symbol-wrapping.js": {"Symbol"},
            "built-ins/Number/is-a-constructor.js": {"Reflect.construct"},
            "built-ins/Number/proto-from-ctor-realm.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/Number/return-abrupt-tonumber-value-symbol.js": {
                "Symbol",
            },
            "built-ins/Boolean/is-a-constructor.js": {"Reflect.construct"},
            "built-ins/Boolean/proto-from-ctor-realm.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/Boolean/symbol-coercion.js": {"Symbol"},
            "built-ins/Date/is-a-constructor.js": {"Reflect.construct"},
            "built-ins/Date/subclassing.js": {"Reflect"},
            "built-ins/Date/proto-from-ctor-realm-zero.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/Date/proto-from-ctor-realm-one.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/Date/proto-from-ctor-realm-two.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/RegExp/is-a-constructor.js": {
                "Reflect.construct",
            },
            "built-ins/RegExp/proto-from-ctor-realm.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/RegExp/from-regexp-like-flag-override.js": {
                "Symbol", "Symbol.match",
            },
            "built-ins/RegExp/from-regexp-like-get-ctor-err.js": {
                "Symbol", "Symbol.match",
            },
            "built-ins/RegExp/from-regexp-like-get-flags-err.js": {
                "Symbol", "Symbol.match",
            },
            "built-ins/RegExp/from-regexp-like-get-source-err.js": {
                "Symbol", "Symbol.match",
            },
            "built-ins/RegExp/from-regexp-like-short-circuit.js": {
                "Symbol", "Symbol.match",
            },
            "built-ins/RegExp/from-regexp-like.js": {
                "Symbol", "Symbol.match",
            },
            "built-ins/Function/is-a-constructor.js": {"Reflect.construct"},
            "built-ins/Function/proto-from-ctor-realm-prototype.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/Function/proto-from-ctor-realm.js": {
                "cross-realm", "Reflect",
            },
            "built-ins/AsyncFunction/is-a-constructor.js": {
                "Reflect.construct",
            },
            "built-ins/AsyncFunction/proto-from-ctor-realm.js": {
                "async-functions", "cross-realm", "Reflect", "Symbol",
            },
            "built-ins/GeneratorFunction/is-a-constructor.js": {
                "Reflect.construct",
            },
            "built-ins/AsyncGeneratorFunction/is-a-constructor.js": {
                "Reflect.construct",
            },
        }
        self.assertEqual(NATIVE_CONSTRUCT_FILES, frozenset(expected))
        self.assertEqual(
            NATIVE_CONSTRUCT_FEATURES,
            {path: frozenset(features) for path, features in expected.items()},
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test" / "built-ins/Proxy/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.native_construct_path(path))
                        self.assertEqual(
                            tool.native_construct_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features)},
                                path,
                            )
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            )
                        )
                    self.assertFalse(tool.native_construct_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_object_prototype_manifest_is_exact_and_shared(self):
        self.assertEqual(len(OBJECT_PROTOTYPE_FILES), 46)
        self.assertEqual(
            frozenset(OBJECT_PROTOTYPE_FEATURES_BY_FILE), OBJECT_PROTOTYPE_FILES
        )

        representative = {
            "built-ins/Object/prototype/__lookupGetter__/lookup-own-get-err.js": {
                "Proxy", "__getter__",
            },
            "built-ins/Object/prototype/propertyIsEnumerable/symbol_own_property.js": {
                "Symbol",
            },
            "built-ins/Object/prototype/toString/proxy-function-async.js": {
                "Proxy", "Symbol.toStringTag", "async-functions",
            },
            "built-ins/Object/prototype/toString/symbol-tag-promise-builtin.js": {
                "Promise", "Symbol.toStringTag",
            },
        }
        for path, features in representative.items():
            self.assertEqual(
                OBJECT_PROTOTYPE_FEATURES_BY_FILE[path], frozenset(features)
            )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test" / "built-ins/Object/prototype/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in OBJECT_PROTOTYPE_FEATURES_BY_FILE.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.object_prototype_path(path))
                        self.assertEqual(
                            tool.object_prototype_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features)}, path
                            )
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            )
                        )
                    self.assertFalse(tool.object_prototype_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_promise_realm_manifest_is_exact_and_shared(self):
        relative = "built-ins/Promise/proto-from-ctor-realm.js"
        features = frozenset({"cross-realm", "Reflect"})
        self.assertEqual(PROMISE_REALM_FILES, frozenset({relative}))
        self.assertEqual(PROMISE_REALM_FEATURES, {relative: features})
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted = root / "test" / relative
            outside = root / "test" / "built-ins/Promise/future-realm.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.promise_realm_path(admitted))
                    self.assertEqual(tool.promise_realm_features(admitted), features)
                    self.assertFalse(
                        tool.should_skip({"features": sorted(features)}, admitted)
                    )
                    self.assertFalse(tool.promise_realm_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": sorted(features)}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_promise_combinator_close_manifest_is_exact_and_shared(self):
        self.assertEqual(len(PROMISE_COMBINATOR_CLOSE_FILES), 12)
        self.assertEqual(
            frozenset(PROMISE_COMBINATOR_CLOSE_FEATURES),
            PROMISE_COMBINATOR_CLOSE_FILES,
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test/built-ins/Promise/all/future-close.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in PROMISE_COMBINATOR_CLOSE_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.promise_combinator_close_path(path))
                        self.assertEqual(
                            tool.promise_combinator_close_features(path), features
                        )
                        flags = ["async"] if "/any/" in relative else []
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features), "flags": flags},
                                path,
                            )
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": sorted(features | {"decorators"}),
                                    "flags": flags,
                                },
                                path,
                            )
                        )
                    self.assertFalse(tool.promise_combinator_close_path(outside))
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["Symbol.iterator"]},
                            outside,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in PROMISE_COMBINATOR_CLOSE_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

    def test_promise_combinator_rejection_manifest_is_exact_and_shared(self):
        self.assertEqual(len(PROMISE_COMBINATOR_REJECTION_FILES), 95)
        self.assertEqual(
            frozenset(PROMISE_COMBINATOR_REJECTION_FEATURES),
            PROMISE_COMBINATOR_REJECTION_FILES,
        )
        self.assertTrue(
            PROMISE_COMBINATOR_REJECTION_FILES.isdisjoint(
                PROMISE_COMBINATOR_CLOSE_FILES
            )
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test/built-ins/Promise/all/future-rejection.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in (
                        PROMISE_COMBINATOR_REJECTION_FEATURES.items()
                    ):
                        path = root / "test" / relative
                        self.assertTrue(tool.promise_combinator_rejection_path(path))
                        self.assertEqual(
                            tool.promise_combinator_rejection_features(path),
                            features,
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features), "flags": ["async"]},
                                path,
                            )
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": sorted(features | {"decorators"}),
                                    "flags": ["async"],
                                },
                                path,
                            )
                        )
                    self.assertFalse(
                        tool.promise_combinator_rejection_path(outside)
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "features": ["Symbol.iterator"],
                                "flags": ["async"],
                            },
                            outside,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in (
                PROMISE_COMBINATOR_REJECTION_FEATURES.items()
            ):
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), ["async"], relative)

    def test_promise_keyed_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(PROMISE_KEYED_FILES), 63)
        self.assertEqual(frozenset(PROMISE_KEYED_FEATURES), PROMISE_KEYED_FILES)
        self.assertTrue(
            PROMISE_KEYED_FILES.isdisjoint(PROMISE_COMBINATOR_CLOSE_FILES)
        )
        self.assertTrue(
            PROMISE_KEYED_FILES.isdisjoint(PROMISE_COMBINATOR_REJECTION_FILES)
        )
        self.assertTrue(PROMISE_KEYED_FILES.isdisjoint(PROMISE_FINALLY_FILES))

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Promise/allKeyed/future.js"
            outside = root / "test/built-ins/Promise/all/future-keyed.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in PROMISE_KEYED_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.promise_keyed_path(path), relative)
                        self.assertEqual(tool.promise_keyed_features(path), features)
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features), "flags": ["async"]},
                                path,
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": sorted(features | {"decorators"}),
                                    "flags": ["async"],
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.promise_keyed_path(path))
                        self.assertEqual(tool.promise_keyed_features(path), frozenset())
                        self.assertTrue(
                            tool.should_skip(
                                {"features": [], "flags": ["async"]},
                                path,
                            )
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": ["await-dictionary"],
                                    "flags": ["async"],
                                },
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in PROMISE_KEYED_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

    def test_promise_finally_manifest_is_exact_and_shared(self):
        self.assertEqual(len(PROMISE_FINALLY_FILES), 37)
        self.assertEqual(
            frozenset(PROMISE_FINALLY_FEATURES), PROMISE_FINALLY_FILES
        )
        self.assertEqual(
            len(
                {
                    relative
                    for relative in PROMISE_FINALLY_FILES
                    if "/prototype/finally/" in relative
                }
            ),
            29,
        )
        self.assertTrue(
            PROMISE_FINALLY_FILES.isdisjoint(PROMISE_COMBINATOR_CLOSE_FILES)
        )
        self.assertTrue(
            PROMISE_FINALLY_FILES.isdisjoint(PROMISE_COMBINATOR_REJECTION_FILES)
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test/built-ins/Promise/prototype/finally/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in PROMISE_FINALLY_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.promise_finally_path(path), relative)
                        self.assertEqual(
                            tool.promise_finally_features(path), features, relative
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features), "flags": ["async"]},
                                path,
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": sorted(features | {"decorators"}),
                                    "flags": ["async"],
                                },
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.promise_finally_path(outside))
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "features": ["Promise.prototype.finally"],
                                "flags": ["async"],
                            },
                            outside,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in PROMISE_FINALLY_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

    def test_promise_constructor_order_manifest_is_exact_and_shared(self):
        relative = (
            "built-ins/Promise/"
            "get-prototype-abrupt-executor-not-callable.js"
        )
        features = frozenset({"Reflect", "Reflect.construct"})
        self.assertEqual(PROMISE_CONSTRUCTOR_ORDER_FILES, frozenset({relative}))
        self.assertEqual(
            PROMISE_CONSTRUCTOR_ORDER_FEATURES,
            {relative: features},
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted = root / "test" / relative
            outside = root / "test/built-ins/Promise/get-prototype-abrupt.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.promise_constructor_order_path(admitted))
                    self.assertEqual(
                        tool.promise_constructor_order_features(admitted), features
                    )
                    self.assertFalse(
                        tool.should_skip({"features": sorted(features)}, admitted)
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": sorted(features | {"decorators"})},
                            admitted,
                        )
                    )
                    self.assertFalse(tool.promise_constructor_order_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": sorted(features)}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

        path = Path(test262_runner.TEST262) / "test" / relative
        try:
            path_available = path.is_file()
        except OSError:
            path_available = False
        if path_available:
            metadata = test262_runner.parse_meta(path.read_text())
            self.assertEqual(
                frozenset(metadata.get("features", [])), features
            )

    def test_generator_function_manifest_is_exact_and_shared(self):
        self.assertEqual(len(GENERATOR_FUNCTION_FILES), 3)
        self.assertEqual(
            frozenset(GENERATOR_FUNCTION_FEATURES), GENERATOR_FUNCTION_FILES
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test" / "built-ins/GeneratorFunction/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in GENERATOR_FUNCTION_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.generator_function_path(path))
                        self.assertEqual(
                            tool.generator_function_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            )
                        )
                    self.assertFalse(tool.generator_function_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["generators"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in GENERATOR_FUNCTION_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

    def test_async_generator_realm_manifest_is_exact_and_shared(self):
        self.assertEqual(len(ASYNC_GENERATOR_REALM_FILES), 3)
        self.assertEqual(
            frozenset(ASYNC_GENERATOR_REALM_FEATURES),
            ASYNC_GENERATOR_REALM_FILES,
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test" / "built-ins/AsyncGeneratorFunction/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ASYNC_GENERATOR_REALM_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.async_generator_realm_path(path))
                        self.assertEqual(
                            tool.async_generator_realm_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            )
                        )
                    self.assertFalse(tool.async_generator_realm_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["async-iteration"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ASYNC_GENERATOR_REALM_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

    def test_async_iterator_dispose_manifest_is_exact_and_shared(self):
        self.assertEqual(len(ASYNC_ITERATOR_DISPOSE_FILES), 9)
        self.assertEqual(
            frozenset(ASYNC_ITERATOR_DISPOSE_FEATURES),
            ASYNC_ITERATOR_DISPOSE_FILES,
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = (
                root
                / "test/built-ins/AsyncIteratorPrototype/Symbol.asyncDispose/future.js"
            )
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ASYNC_ITERATOR_DISPOSE_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.async_iterator_dispose_path(path))
                        self.assertEqual(
                            tool.async_iterator_dispose_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path)
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {
                                    "features": sorted(features),
                                    "flags": ["async"],
                                },
                                path,
                            )
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            )
                        )
                    self.assertFalse(tool.async_iterator_dispose_path(outside))
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "features": ["explicit-resource-management"],
                                "flags": ["async"],
                            },
                            outside,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ASYNC_ITERATOR_DISPOSE_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

    def test_async_from_sync_iterator_manifest_is_exact_and_shared(self):
        self.assertEqual(len(ASYNC_FROM_SYNC_ITERATOR_FILES), 38)
        self.assertEqual(
            frozenset(ASYNC_FROM_SYNC_ITERATOR_FEATURES),
            ASYNC_FROM_SYNC_ITERATOR_FILES,
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test/built-ins/AsyncFromSyncIteratorPrototype/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ASYNC_FROM_SYNC_ITERATOR_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.async_from_sync_iterator_path(path))
                        self.assertEqual(
                            tool.async_from_sync_iterator_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features), "flags": ["async"]},
                                path,
                            )
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": sorted(features | {"decorators"}),
                                    "flags": ["async"],
                                },
                                path,
                            )
                        )
                    self.assertFalse(tool.async_from_sync_iterator_path(outside))
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "features": ["async-iteration"],
                                "flags": ["async"],
                            },
                            outside,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ASYNC_FROM_SYNC_ITERATOR_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

    def test_proxy_get_manifest_is_exact_and_shared(self):
        self.assertEqual(len(PROXY_GET_FILES), 30)
        admitted = "built-ins/Proxy/get/trap-is-undefined-receiver.js"
        realm = "built-ins/Proxy/get/trap-is-not-callable-realm.js"
        symbolic = "built-ins/Proxy/get/trap-is-null-target-is-proxy.js"
        reflected = "built-ins/Reflect/get/return-value-from-symbol-key.js"
        outside = "built-ins/Array/isArray/proxy.js"
        self.assertIn(admitted, PROXY_GET_FILES)
        self.assertIn(reflected, PROXY_GET_FILES)
        self.assertEqual(PROXY_GET_FEATURES[admitted], {"Proxy"})
        self.assertEqual(PROXY_GET_FEATURES[realm], {"Proxy", "cross-realm"})
        self.assertEqual(PROXY_GET_FEATURES[symbolic], {"Proxy", "Symbol"})
        self.assertEqual(PROXY_GET_FEATURES[reflected], {"Reflect", "Symbol"})
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted_path = root / "test" / admitted
            realm_path = root / "test" / realm
            symbolic_path = root / "test" / symbolic
            reflected_path = root / "test" / reflected
            outside_path = root / "test" / outside
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(
                        {"features": ["Proxy"]}, admitted_path
                    ))
                    self.assertTrue(tool.should_skip(
                        {"features": ["Proxy", "Symbol"]}, admitted_path
                    ))
                    self.assertFalse(tool.should_skip(
                        {"features": ["Proxy", "cross-realm"]}, realm_path
                    ))
                    self.assertFalse(tool.should_skip(
                        {"features": ["Proxy", "Symbol"]}, symbolic_path
                    ))
                    self.assertFalse(tool.should_skip(
                        {"features": ["Reflect", "Symbol"]}, reflected_path
                    ))
                    self.assertTrue(tool.should_skip(
                        {"features": ["Proxy"]}, outside_path
                    ))
                    self.assertTrue(tool.proxy_get_path(admitted_path))
                    self.assertTrue(tool.proxy_get_path(reflected_path))
                finally:
                    tool.TEST262 = original_root

    def test_proxy_delete_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(PROXY_DELETE_FILES), 28)
        self.assertEqual(frozenset(PROXY_DELETE_FEATURES), PROXY_DELETE_FILES)
        proxy = (
            "built-ins/Proxy/deleteProperty/"
            "targetdesc-is-configurable-target-is-not-extensible.js"
        )
        realm = "built-ins/Proxy/deleteProperty/trap-is-not-callable-realm.js"
        reflected = "built-ins/Reflect/deleteProperty/not-a-constructor.js"
        outside = "built-ins/Proxy/deleteProperty/future.js"
        self.assertEqual(
            PROXY_DELETE_FEATURES[proxy],
            {"Proxy", "Reflect", "proxy-missing-checks"},
        )
        self.assertEqual(
            PROXY_DELETE_FEATURES[realm], {"Proxy", "cross-realm"}
        )
        self.assertEqual(
            PROXY_DELETE_FEATURES[reflected],
            {"Reflect", "Reflect.construct", "arrow-function"},
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative in PROXY_DELETE_FILES:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                features = set(
                    test262_runner.parse_meta(path.read_text()).get("features", [])
                )
                self.assertEqual(
                    features, set(PROXY_DELETE_FEATURES[relative]), relative
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative in PROXY_DELETE_FILES:
                        path = root / "test" / relative
                        features = set(PROXY_DELETE_FEATURES[relative])
                        self.assertTrue(tool.proxy_delete_path(path), relative)
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    outside_path = root / "test" / outside
                    self.assertFalse(tool.proxy_delete_path(outside_path))
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, outside_path)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_extensibility_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(EXTENSIBILITY_FILES), 31)
        self.assertEqual(frozenset(EXTENSIBILITY_FEATURES), EXTENSIBILITY_FILES)
        self.assertEqual(
            EXTENSIBILITY_MODULE_FILES,
            {
                "built-ins/Proxy/preventExtensions/"
                "trap-is-undefined-target-is-proxy.js"
            },
        )
        self.assertEqual(
            {
                family: sum(family in relative for relative in EXTENSIBILITY_FILES)
                for family in (
                    "Object/isExtensible/",
                    "Object/preventExtensions/",
                    "Reflect/preventExtensions/",
                    "Proxy/isExtensible/",
                    "Proxy/preventExtensions/",
                )
            },
            {
                "Object/isExtensible/": 1,
                "Object/preventExtensions/": 5,
                "Reflect/preventExtensions/": 1,
                "Proxy/isExtensible/": 12,
                "Proxy/preventExtensions/": 12,
            },
        )

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_extensibility_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(EXTENSIBILITY_FILES & existing, manifest.name)

        constructor = "built-ins/Object/isExtensible/not-a-constructor.js"
        symbol = (
            "built-ins/Object/preventExtensions/"
            "symbol-object-contains-symbol-properties-strict.js"
        )
        realm = "built-ins/Proxy/isExtensible/trap-is-not-callable-realm.js"
        reflected = "built-ins/Proxy/preventExtensions/return-false.js"
        variable_length_object = (
            "staging/built-ins/Object/preventExtensions/"
            "preventExtensions-variable-length-typed-arrays.js"
        )
        variable_length_reflect = (
            "staging/built-ins/Reflect/preventExtensions/"
            "preventExtensions-variable-length-typed-arrays.js"
        )
        variable_length_seal = (
            "staging/built-ins/Object/seal/"
            "seal-variable-length-typed-arrays.js"
        )
        self.assertEqual(
            EXTENSIBILITY_FEATURES[constructor],
            {"Reflect.construct", "arrow-function"},
        )
        self.assertEqual(EXTENSIBILITY_FEATURES[symbol], {"Symbol"})
        self.assertEqual(EXTENSIBILITY_FEATURES[realm], {"Proxy", "cross-realm"})
        self.assertEqual(EXTENSIBILITY_FEATURES[reflected], {"Proxy", "Reflect"})
        variable_length_features = {
            "ArrayBuffer",
            "SharedArrayBuffer",
            "resizable-arraybuffer",
        }
        self.assertEqual(
            EXTENSIBILITY_FEATURES[variable_length_object], variable_length_features
        )
        self.assertEqual(
            EXTENSIBILITY_FEATURES[variable_length_reflect], variable_length_features
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in EXTENSIBILITY_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                if relative.endswith("/not-a-constructor.js"):
                    expected_includes = ["isConstructor.js"]
                elif relative in {variable_length_object, variable_length_reflect}:
                    expected_includes = ["resizableArrayBufferUtils.js"]
                else:
                    expected_includes = []
                self.assertEqual(
                    metadata.get("includes", []), expected_includes, relative
                )
                if relative.endswith("symbol-object-contains-symbol-properties-strict.js"):
                    expected_flags = ["onlyStrict"]
                elif relative.endswith(
                    "symbol-object-contains-symbol-properties-non-strict.js"
                ):
                    expected_flags = ["noStrict"]
                elif relative in EXTENSIBILITY_MODULE_FILES:
                    expected_flags = ["module"]
                else:
                    expected_flags = []
                self.assertEqual(metadata.get("flags", []), expected_flags, relative)
                self.assertNotIn("negative", metadata, relative)

            for relative in (variable_length_object, variable_length_reflect):
                directory = (test_root / relative).parent
                self.assertEqual(
                    {
                        path.relative_to(test_root).as_posix()
                        for path in directory.rglob("*.js")
                    },
                    {relative},
                )

            seal_path = test_root / variable_length_seal
            self.assertTrue(seal_path.is_file(), variable_length_seal)
            seal_metadata = test262_runner.parse_meta(seal_path.read_text())
            self.assertEqual(
                frozenset(seal_metadata.get("features", [])),
                variable_length_features,
            )
            self.assertEqual(
                seal_metadata.get("includes", []), ["resizableArrayBufferUtils.js"]
            )
            for tool in (test262_runner, test262_analyze):
                self.assertFalse(tool.extensibility_path(seal_path))
                self.assertTrue(tool.should_skip(seal_metadata, seal_path))

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Proxy/preventExtensions/future.js"
            outside = root / "test/built-ins/Proxy/defineProperty/future.js"
            staging_future = (
                root / "test/staging/built-ins/Object/preventExtensions/future.js"
            )
            staging_seal = root / "test" / variable_length_seal
            non_module = (
                root / "test/built-ins/Proxy/preventExtensions/call-parameters.js"
            )
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in EXTENSIBILITY_FEATURES.items():
                        path = root / "test" / relative
                        flags = ["module"] if relative in EXTENSIBILITY_MODULE_FILES else []
                        self.assertTrue(tool.extensibility_path(path), relative)
                        self.assertEqual(tool.extensibility_features(path), features)
                        self.assertEqual(
                            tool.extensibility_module_path(path),
                            relative in EXTENSIBILITY_MODULE_FILES,
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features), "flags": flags}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": sorted(features | {"decorators"}),
                                    "flags": flags,
                                },
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.extensibility_path(future))
                    self.assertFalse(tool.extensibility_path(outside))
                    self.assertFalse(tool.extensibility_path(staging_future))
                    self.assertFalse(tool.extensibility_path(staging_seal))
                    self.assertTrue(tool.should_skip({"features": ["Proxy"]}, future))
                    self.assertTrue(tool.should_skip({"features": ["Proxy"]}, outside))
                    self.assertTrue(
                        tool.should_skip(
                            {"features": sorted(variable_length_features)}, staging_future
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": sorted(variable_length_features)}, staging_seal
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["Proxy"], "flags": ["module"]},
                            non_module,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

    def test_prototype_internal_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(PROTOTYPE_INTERNAL_FILES), 40)
        self.assertEqual(
            frozenset(PROTOTYPE_INTERNAL_FEATURES), PROTOTYPE_INTERNAL_FILES
        )
        self.assertEqual(
            {
                family: sum(family in relative for relative in PROTOTYPE_INTERNAL_FILES)
                for family in (
                    "Object/setPrototypeOf/",
                    "Proxy/getPrototypeOf/",
                    "Proxy/setPrototypeOf/",
                )
            },
            {
                "Object/setPrototypeOf/": 4,
                "Proxy/getPrototypeOf/": 19,
                "Proxy/setPrototypeOf/": 17,
            },
        )
        feature_counts = {}
        for features in PROTOTYPE_INTERNAL_FEATURES.values():
            feature_counts[features] = feature_counts.get(features, 0) + 1
        self.assertEqual(
            feature_counts,
            {
                frozenset({"Proxy"}): 28,
                frozenset({"Proxy", "cross-realm"}): 2,
                frozenset({"Proxy", "Symbol"}): 1,
                frozenset({"Proxy", "Reflect", "Reflect.setPrototypeOf"}): 5,
                frozenset(
                    {
                        "Proxy",
                        "Reflect",
                        "Reflect.setPrototypeOf",
                        "Symbol",
                    }
                ): 1,
                frozenset({"Reflect.construct", "arrow-function"}): 1,
                frozenset({"Symbol"}): 2,
            },
        )

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_prototype_internal_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(PROTOTYPE_INTERNAL_FILES & existing, manifest.name)

        constructor = "built-ins/Object/setPrototypeOf/not-a-constructor.js"
        object_proxy = "built-ins/Object/setPrototypeOf/set-error.js"
        realm = "built-ins/Proxy/getPrototypeOf/trap-is-not-callable-realm.js"
        symbol = (
            "built-ins/Proxy/getPrototypeOf/"
            "trap-result-neither-object-nor-null-throws-symbol.js"
        )
        reflected = "built-ins/Proxy/setPrototypeOf/internals-call-order.js"
        reflected_symbol = (
            "built-ins/Proxy/setPrototypeOf/"
            "toboolean-trap-result-true-target-is-extensible.js"
        )
        self.assertEqual(
            PROTOTYPE_INTERNAL_FEATURES[constructor],
            {"Reflect.construct", "arrow-function"},
        )
        self.assertEqual(PROTOTYPE_INTERNAL_FEATURES[object_proxy], {"Proxy"})
        self.assertEqual(
            PROTOTYPE_INTERNAL_FEATURES[realm], {"Proxy", "cross-realm"}
        )
        self.assertEqual(
            PROTOTYPE_INTERNAL_FEATURES[symbol], {"Proxy", "Symbol"}
        )
        self.assertEqual(
            PROTOTYPE_INTERNAL_FEATURES[reflected],
            {"Proxy", "Reflect", "Reflect.setPrototypeOf"},
        )
        self.assertEqual(
            PROTOTYPE_INTERNAL_FEATURES[reflected_symbol],
            {"Proxy", "Reflect", "Reflect.setPrototypeOf", "Symbol"},
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in PROTOTYPE_INTERNAL_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                expected_includes = (
                    ["isConstructor.js"] if relative == constructor else []
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertNotIn("negative", metadata, relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Proxy/setPrototypeOf/future.js"
            outside = root / "test/built-ins/Proxy/defineProperty/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in PROTOTYPE_INTERNAL_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.prototype_internal_path(path), relative)
                        self.assertEqual(
                            tool.prototype_internal_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.prototype_internal_path(future))
                    self.assertFalse(tool.prototype_internal_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, future)
                    )
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_proxy_define_property_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(PROXY_DEFINE_PROPERTY_FILES), 24)
        self.assertEqual(
            frozenset(PROXY_DEFINE_PROPERTY_FEATURES),
            PROXY_DEFINE_PROPERTY_FILES,
        )
        feature_counts = {}
        for features in PROXY_DEFINE_PROPERTY_FEATURES.values():
            feature_counts[features] = feature_counts.get(features, 0) + 1
        self.assertEqual(
            feature_counts,
            {
                frozenset({"Proxy"}): 10,
                frozenset({"Proxy", "cross-realm"}): 8,
                frozenset({"Proxy", "Reflect"}): 5,
                frozenset({"Proxy", "Reflect", "proxy-missing-checks"}): 1,
            },
        )

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_proxy_define_property_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(PROXY_DEFINE_PROPERTY_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            live_directory = {
                path.relative_to(test_root).as_posix()
                for path in (test_root / "built-ins/Proxy/defineProperty").glob("*.js")
            }
            self.assertEqual(PROXY_DEFINE_PROPERTY_FILES, live_directory)
            property_helper_files = {
                "built-ins/Proxy/defineProperty/return-boolean-and-define-target.js",
                "built-ins/Proxy/defineProperty/trap-is-null-target-is-proxy.js",
                "built-ins/Proxy/defineProperty/trap-is-undefined.js",
            }
            compare_array = (
                "built-ins/Proxy/defineProperty/trap-is-undefined-target-is-proxy.js"
            )
            for relative, features in PROXY_DEFINE_PROPERTY_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                if relative in property_helper_files:
                    expected_includes = ["propertyHelper.js"]
                elif relative == compare_array:
                    expected_includes = ["compareArray.js"]
                else:
                    expected_includes = []
                self.assertEqual(
                    metadata.get("includes", []), expected_includes, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertNotIn("negative", metadata, relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Proxy/defineProperty/future.js"
            outside = root / "test/built-ins/Object/defineProperty/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in PROXY_DEFINE_PROPERTY_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.proxy_define_property_path(path), relative)
                        self.assertEqual(
                            tool.proxy_define_property_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.proxy_define_property_path(future))
                    self.assertFalse(tool.proxy_define_property_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, future)
                    )
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_proxy_has_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(PROXY_HAS_FILES), 26)
        self.assertEqual(frozenset(PROXY_HAS_FEATURES), PROXY_HAS_FILES)
        feature_counts = {}
        for features in PROXY_HAS_FEATURES.values():
            feature_counts[features] = feature_counts.get(features, 0) + 1
        self.assertEqual(
            feature_counts,
            {
                frozenset({"Proxy"}): 22,
                frozenset({"Proxy", "Reflect", "Symbol.replace"}): 1,
                frozenset({"Proxy", "cross-realm"}): 1,
                frozenset(
                    {"Proxy", "Array.prototype.includes", "Reflect", "Symbol"}
                ): 1,
                frozenset({"Proxy", "Reflect"}): 1,
            },
        )

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_proxy_has_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(PROXY_HAS_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            live_directory = {
                path.relative_to(test_root).as_posix()
                for path in (test_root / "built-ins/Proxy/has").glob("*.js")
            }
            self.assertEqual(PROXY_HAS_FILES, live_directory)
            proxy_traps_helper_files = {
                "built-ins/Proxy/has/call-in-prototype-index.js",
                "built-ins/Proxy/has/call-in-prototype.js",
            }
            no_strict_files = {
                "built-ins/Proxy/has/call-with.js",
                "built-ins/Proxy/has/null-handler-using-with.js",
                "built-ins/Proxy/has/return-false-target-not-extensible-using-with.js",
                "built-ins/Proxy/has/return-false-target-prop-exists-using-with.js",
                "built-ins/Proxy/has/return-false-targetdesc-not-configurable-using-with.js",
                "built-ins/Proxy/has/return-is-abrupt-with.js",
                "built-ins/Proxy/has/return-true-target-prop-exists-using-with.js",
                "built-ins/Proxy/has/trap-is-not-callable-using-with.js",
                "built-ins/Proxy/has/trap-is-undefined-using-with.js",
            }
            self.assertEqual(len(no_strict_files), 9)
            for relative, features in PROXY_HAS_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []),
                    ["proxyTrapsHelper.js"]
                    if relative in proxy_traps_helper_files
                    else [],
                    relative,
                )
                self.assertEqual(
                    metadata.get("flags", []),
                    ["noStrict"] if relative in no_strict_files else [],
                    relative,
                )
                self.assertNotIn("negative", metadata, relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Proxy/has/future.js"
            outside = root / "test/built-ins/Object/has/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in PROXY_HAS_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.proxy_has_path(path), relative)
                        self.assertEqual(tool.proxy_has_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.proxy_has_path(future))
                    self.assertFalse(tool.proxy_has_path(outside))
                    self.assertTrue(tool.should_skip({"features": ["Proxy"]}, future))
                    self.assertTrue(tool.should_skip({"features": ["Proxy"]}, outside))
                finally:
                    tool.TEST262 = original_root

    def test_proxy_set_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(PROXY_SET_FILES), 27)
        self.assertEqual(frozenset(PROXY_SET_FEATURES), PROXY_SET_FILES)
        feature_counts = {}
        for features in PROXY_SET_FEATURES.values():
            feature_counts[features] = feature_counts.get(features, 0) + 1
        self.assertEqual(
            feature_counts,
            {
                frozenset({"Proxy"}): 11,
                frozenset({"Proxy", "Reflect", "Reflect.set"}): 9,
                frozenset({"Proxy", "Reflect"}): 5,
                frozenset({"Proxy", "__proto__"}): 1,
                frozenset({"Proxy", "cross-realm"}): 1,
            },
        )

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_proxy_set_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(PROXY_SET_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            live_directory = {
                path.relative_to(test_root).as_posix()
                for path in (test_root / "built-ins/Proxy/set").glob("*.js")
            }
            self.assertEqual(PROXY_SET_FILES, live_directory)
            proxy_traps_helper_files = {
                "built-ins/Proxy/set/call-parameters-prototype-dunder-proto.js",
                "built-ins/Proxy/set/call-parameters-prototype-index.js",
                "built-ins/Proxy/set/call-parameters-prototype.js",
            }
            compare_array_files = {
                "built-ins/Proxy/set/trap-is-missing-receiver-multiple-calls-index.js",
                "built-ins/Proxy/set/trap-is-missing-receiver-multiple-calls.js",
                "built-ins/Proxy/set/trap-is-null-target-is-proxy.js",
            }
            for relative, features in PROXY_SET_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                if relative in proxy_traps_helper_files:
                    expected_includes = ["proxyTrapsHelper.js"]
                elif relative in compare_array_files:
                    expected_includes = ["compareArray.js"]
                else:
                    expected_includes = []
                self.assertEqual(
                    metadata.get("includes", []), expected_includes, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertNotIn("negative", metadata, relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Proxy/set/future.js"
            outside = root / "test/built-ins/Object/set/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in PROXY_SET_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.proxy_set_path(path), relative)
                        self.assertEqual(tool.proxy_set_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.proxy_set_path(future))
                    self.assertFalse(tool.proxy_set_path(outside))
                    self.assertTrue(tool.should_skip({"features": ["Proxy"]}, future))
                    self.assertTrue(tool.should_skip({"features": ["Proxy"]}, outside))
                finally:
                    tool.TEST262 = original_root

    def test_reflect_call_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(REFLECT_CALL_FILES), 19)
        self.assertEqual(frozenset(REFLECT_CALL_FEATURES), REFLECT_CALL_FILES)
        self.assertEqual(
            sum("/apply/" in relative for relative in REFLECT_CALL_FILES),
            9,
        )
        self.assertEqual(
            sum("/construct/" in relative for relative in REFLECT_CALL_FILES),
            10,
        )

        symbolic = (
            "built-ins/Reflect/apply/"
            "arguments-list-is-not-array-like-but-still-valid.js"
        )
        construct = "built-ins/Reflect/construct/use-arguments-list.js"
        construct_base = "built-ins/Reflect/construct/construct.js"
        self.assertEqual(
            REFLECT_CALL_FEATURES[symbolic],
            {"Reflect", "Symbol", "arrow-function"},
        )
        self.assertEqual(
            REFLECT_CALL_FEATURES[construct],
            {"Reflect", "Reflect.construct"},
        )
        self.assertEqual(REFLECT_CALL_FEATURES[construct_base], {"Reflect"})

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Reflect/apply/future.js"
            outside = root / "test/built-ins/Reflect/get/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in REFLECT_CALL_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.reflect_call_path(path), relative)
                        self.assertEqual(tool.reflect_call_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.reflect_call_path(future))
                    self.assertFalse(tool.reflect_call_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["Reflect"]}, future)
                    )
                    self.assertTrue(
                        tool.should_skip({"features": ["Reflect"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in REFLECT_CALL_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

    def test_reflect_set_has_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(REFLECT_SET_HAS_FILES), 28)
        self.assertEqual(
            frozenset(REFLECT_SET_HAS_FEATURES), REFLECT_SET_HAS_FILES
        )
        self.assertEqual(
            sum("/set/" in relative for relative in REFLECT_SET_HAS_FILES), 18
        )
        self.assertEqual(
            sum("/has/" in relative for relative in REFLECT_SET_HAS_FILES), 10
        )
        plain_set = "built-ins/Reflect/set/set.js"
        constructor = "built-ins/Reflect/set/not-a-constructor.js"
        proxy_has = "built-ins/Reflect/has/return-abrupt-from-result.js"
        symbolic = "built-ins/Reflect/has/symbol-property.js"
        self.assertEqual(REFLECT_SET_HAS_FEATURES[plain_set], {"Reflect"})
        self.assertEqual(
            REFLECT_SET_HAS_FEATURES[constructor],
            {"Reflect", "Reflect.set", "Reflect.construct", "arrow-function"},
        )
        self.assertEqual(
            REFLECT_SET_HAS_FEATURES[proxy_has], {"Reflect", "Proxy"}
        )
        self.assertEqual(
            REFLECT_SET_HAS_FEATURES[symbolic], {"Reflect", "Symbol"}
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in REFLECT_SET_HAS_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Reflect/set/future.js"
            outside = root / "test/built-ins/Reflect/get/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in REFLECT_SET_HAS_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.reflect_set_has_path(path), relative)
                        self.assertEqual(
                            tool.reflect_set_has_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": sorted(features | {"decorators"})
                                },
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.reflect_set_has_path(future))
                    self.assertFalse(tool.reflect_set_has_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["Reflect"]}, future)
                    )
                    self.assertTrue(
                        tool.should_skip({"features": ["Reflect"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_reflect_remaining_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(REFLECT_REMAINING_FILES), 71)
        self.assertEqual(
            frozenset(REFLECT_REMAINING_FEATURES), REFLECT_REMAINING_FILES
        )
        counts = {
            family: sum(
                f"/{family}/" in relative
                for relative in REFLECT_REMAINING_FILES
            )
            for family in (
                "defineProperty",
                "getOwnPropertyDescriptor",
                "getPrototypeOf",
                "isExtensible",
                "preventExtensions",
                "setPrototypeOf",
            )
        }
        self.assertEqual(
            counts,
            {
                "defineProperty": 12,
                "getOwnPropertyDescriptor": 13,
                "getPrototypeOf": 10,
                "isExtensible": 8,
                "preventExtensions": 10,
                "setPrototypeOf": 14,
            },
        )
        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_reflect_remaining_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(
                REFLECT_REMAINING_FILES & existing,
                manifest.name,
            )

        tag = "built-ins/Reflect/Symbol.toStringTag.js"
        constructor = "built-ins/Reflect/getPrototypeOf/not-a-constructor.js"
        proxy = "built-ins/Reflect/preventExtensions/return-abrupt-from-result.js"
        set_proto = "built-ins/Reflect/setPrototypeOf/length.js"
        plain_set_proto = "built-ins/Reflect/setPrototypeOf/setPrototypeOf.js"
        self.assertEqual(
            REFLECT_REMAINING_FEATURES[tag], {"Reflect", "Symbol.toStringTag"}
        )
        self.assertEqual(
            REFLECT_REMAINING_FEATURES[constructor],
            {"Reflect", "Reflect.construct", "arrow-function"},
        )
        self.assertEqual(
            REFLECT_REMAINING_FEATURES[proxy], {"Reflect", "Proxy"}
        )
        self.assertEqual(
            REFLECT_REMAINING_FEATURES[set_proto],
            {"Reflect", "Reflect.setPrototypeOf"},
        )
        self.assertEqual(
            REFLECT_REMAINING_FEATURES[plain_set_proto], {"Reflect"}
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            property_helper_files = {
                "built-ins/Reflect/Symbol.toStringTag.js",
                "built-ins/Reflect/defineProperty/define-properties.js",
                "built-ins/Reflect/defineProperty/defineProperty.js",
                "built-ins/Reflect/defineProperty/length.js",
                "built-ins/Reflect/defineProperty/name.js",
                "built-ins/Reflect/getOwnPropertyDescriptor/getOwnPropertyDescriptor.js",
                "built-ins/Reflect/getOwnPropertyDescriptor/length.js",
                "built-ins/Reflect/getOwnPropertyDescriptor/name.js",
                "built-ins/Reflect/getPrototypeOf/getPrototypeOf.js",
                "built-ins/Reflect/getPrototypeOf/length.js",
                "built-ins/Reflect/getPrototypeOf/name.js",
                "built-ins/Reflect/isExtensible/isExtensible.js",
                "built-ins/Reflect/isExtensible/length.js",
                "built-ins/Reflect/isExtensible/name.js",
                "built-ins/Reflect/preventExtensions/length.js",
                "built-ins/Reflect/preventExtensions/name.js",
                "built-ins/Reflect/preventExtensions/preventExtensions.js",
                "built-ins/Reflect/prop-desc.js",
                "built-ins/Reflect/setPrototypeOf/length.js",
                "built-ins/Reflect/setPrototypeOf/name.js",
                "built-ins/Reflect/setPrototypeOf/setPrototypeOf.js",
            }
            compare_array_files = {
                "built-ins/Reflect/getOwnPropertyDescriptor/return-from-accessor-descriptor.js",
                "built-ins/Reflect/getOwnPropertyDescriptor/return-from-data-descriptor.js",
                "built-ins/Reflect/getOwnPropertyDescriptor/symbol-property.js",
            }
            for relative, features in REFLECT_REMAINING_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                if relative in property_helper_files:
                    expected_includes = ["propertyHelper.js"]
                elif relative in compare_array_files:
                    expected_includes = ["compareArray.js"]
                elif relative.endswith("/not-a-constructor.js"):
                    expected_includes = ["isConstructor.js"]
                else:
                    expected_includes = []
                self.assertEqual(
                    metadata.get("includes", []), expected_includes, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertNotIn("negative", metadata, relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Reflect/defineProperty/future.js"
            outside = root / "test/built-ins/Reflect/get/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in REFLECT_REMAINING_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.reflect_remaining_path(path), relative)
                        self.assertEqual(
                            tool.reflect_remaining_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "features": sorted(features | {"decorators"})
                                },
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.reflect_remaining_path(future))
                    self.assertFalse(tool.reflect_remaining_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["Reflect"]}, future)
                    )
                    self.assertTrue(
                        tool.should_skip({"features": ["Reflect"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_function_apply_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(FUNCTION_APPLY_FILES), 2)
        self.assertEqual(frozenset(FUNCTION_APPLY_FEATURES), FUNCTION_APPLY_FILES)
        self.assertEqual(
            FUNCTION_APPLY_FEATURES[
                "built-ins/Function/prototype/apply/not-a-constructor.js"
            ],
            {"Reflect.construct", "arrow-function"},
        )
        self.assertEqual(
            FUNCTION_APPLY_FEATURES[
                "built-ins/Function/prototype/apply/resizable-buffer.js"
            ],
            {"resizable-arraybuffer"},
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Function/prototype/apply/future.js"
            outside = root / "test/built-ins/Function/prototype/call/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in FUNCTION_APPLY_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.function_apply_path(path), relative)
                        self.assertEqual(tool.function_apply_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.function_apply_path(future))
                    self.assertFalse(tool.function_apply_path(outside))
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["resizable-arraybuffer"]}, future
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": ["resizable-arraybuffer"]}, outside
                        )
                    )
                finally:
                    tool.TEST262 = original_root

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in FUNCTION_APPLY_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

    def test_function_bind_admission_is_exact_live_disjoint_and_shared(self):
        base = "built-ins/Function/prototype/bind"
        expected = {
            f"{base}/instance-length-default-value.js": frozenset({"Symbol"}),
            f"{base}/instance-length-exceeds-int32.js": frozenset(),
            f"{base}/instance-length-prop-desc.js": frozenset(),
            f"{base}/instance-length-remaining-args.js": frozenset(),
            f"{base}/instance-length-tointeger.js": frozenset(),
            f"{base}/instance-name-chained.js": frozenset(),
            f"{base}/instance-name-error.js": frozenset(),
            f"{base}/instance-name-non-string.js": frozenset({"Symbol"}),
            f"{base}/instance-name.js": frozenset(),
        }
        manifest = Path(__file__).with_name("test262_function_bind_admission.txt")
        manifest_entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(manifest_entries, tuple(expected))
        self.assertEqual(FUNCTION_BIND_FILES, frozenset(expected))
        self.assertEqual(FUNCTION_BIND_FEATURES, expected)

        tools_dir = Path(__file__).resolve().parent
        for other_manifest in tools_dir.glob("test262_*_admission.txt"):
            if other_manifest.name == "test262_function_bind_admission.txt":
                continue
            other_files = {
                line
                for raw_line in other_manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                FUNCTION_BIND_FILES.isdisjoint(other_files), other_manifest.name
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            property_helper_files = {
                f"{base}/instance-length-prop-desc.js",
                f"{base}/instance-name-chained.js",
                f"{base}/instance-name-non-string.js",
                f"{base}/instance-name.js",
            }
            for relative, features in expected.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []),
                    ["propertyHelper.js"] if relative in property_helper_files else [],
                    relative,
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / f"test/{base}/instance-name-future.js"
            outside = root / "test/built-ins/Function/prototype/call/instance-name.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertIs(tool.FUNCTION_BIND_FEATURES, FUNCTION_BIND_FEATURES)
                    self.assertIs(tool.FUNCTION_BIND_FILES, FUNCTION_BIND_FILES)
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.function_bind_path(path), relative)
                        self.assertEqual(tool.function_bind_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    for rejected in (future, outside, root / "outside.js"):
                        self.assertFalse(tool.function_bind_path(rejected))
                        self.assertEqual(
                            tool.function_bind_features(rejected), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip({"features": ["Symbol"]}, rejected)
                        )
                    for invalid in (None, object()):
                        self.assertFalse(tool.function_bind_path(invalid))
                        self.assertEqual(
                            tool.function_bind_features(invalid), frozenset()
                        )
                finally:
                    tool.TEST262 = original_root

    def test_shadowrealm_admission_is_exact_live_and_shared(self):
        manifest = Path(__file__).with_name("test262_shadowrealm_admission.txt")
        entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(len(entries), 64)
        self.assertEqual(SHADOWREALM_FILES, frozenset(entries))
        self.assertEqual(frozenset(SHADOWREALM_FEATURES), SHADOWREALM_FILES)
        self.assertEqual(len(SHADOWREALM_MODULE_FILES), 4)
        self.assertTrue(SHADOWREALM_MODULE_FILES <= SHADOWREALM_FILES)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        for tool in (test262_runner, test262_analyze):
            original_root = tool.TEST262
            try:
                tool.TEST262 = str(test_root.parent)
                for relative, features in SHADOWREALM_FEATURES.items():
                    path = test_root / relative
                    self.assertTrue(tool.shadowrealm_path(path), relative)
                    self.assertEqual(tool.shadowrealm_features(path), features, relative)
                    self.assertFalse(
                        tool.should_skip({"features": sorted(features)}, path),
                        relative,
                    )
                    self.assertEqual(
                        tool.shadowrealm_module_path(path),
                        relative in SHADOWREALM_MODULE_FILES,
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"features": sorted(features | {"decorators"})},
                            path,
                        ),
                        relative,
                    )
                    if test_root_available:
                        metadata = tool.parse_meta(path.read_text())
                        self.assertEqual(
                            frozenset(metadata.get("features", [])),
                            features,
                            relative,
                        )
                future = test_root / "built-ins/ShadowRealm/future-sibling.js"
                self.assertFalse(tool.shadowrealm_path(future))
                self.assertEqual(tool.shadowrealm_features(future), frozenset())
                self.assertFalse(tool.shadowrealm_module_path(future))
                self.assertTrue(
                    tool.should_skip({"features": ["ShadowRealm"]}, future)
                )
                for invalid in (None, object()):
                    self.assertFalse(tool.shadowrealm_path(invalid))
                    self.assertEqual(tool.shadowrealm_features(invalid), frozenset())
                    self.assertFalse(tool.shadowrealm_module_path(invalid))
            finally:
                tool.TEST262 = original_root

    def test_function_tostring_admission_is_exact_live_disjoint_and_shared(self):
        manifest = Path(__file__).with_name(
            "test262_function_tostring_admission.txt"
        )
        manifest_entries = tuple(
            line
            for raw_line in manifest.read_text().splitlines()
            if (line := raw_line.strip()) and not line.startswith("#")
        )
        self.assertEqual(len(manifest_entries), 35)
        self.assertEqual(manifest_entries, tuple(FUNCTION_TOSTRING_FEATURES))
        self.assertEqual(FUNCTION_TOSTRING_FILES, frozenset(manifest_entries))

        tools_dir = Path(__file__).resolve().parent
        for other_manifest in tools_dir.glob("test262_*_admission.txt"):
            if other_manifest.name == manifest.name:
                continue
            other_files = {
                line
                for raw_line in other_manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                FUNCTION_TOSTRING_FILES.isdisjoint(other_files),
                other_manifest.name,
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in FUNCTION_TOSTRING_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = (
                root
                / "test/built-ins/Function/prototype/toString/future-feature.js"
            )
            outside = (
                root
                / "test/built-ins/Function/prototype/call/proxy-future.js"
            )
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertIs(
                        tool.FUNCTION_TOSTRING_FEATURES,
                        FUNCTION_TOSTRING_FEATURES,
                    )
                    self.assertIs(
                        tool.FUNCTION_TOSTRING_FILES, FUNCTION_TOSTRING_FILES
                    )
                    for relative, features in FUNCTION_TOSTRING_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.function_tostring_path(path), relative)
                        self.assertEqual(
                            tool.function_tostring_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.function_tostring_path(future))
                    self.assertEqual(
                        tool.function_tostring_features(future), frozenset()
                    )
                    for rejected in (future, outside, root / "outside.js"):
                        self.assertFalse(tool.function_tostring_path(rejected))
                        self.assertEqual(
                            tool.function_tostring_features(rejected), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip({"features": ["Proxy"]}, rejected)
                        )
                    for invalid in (None, object()):
                        self.assertFalse(tool.function_tostring_path(invalid))
                        self.assertEqual(
                            tool.function_tostring_features(invalid), frozenset()
                        )
                finally:
                    tool.TEST262 = original_root

    def test_proxy_own_keys_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(PROXY_OWN_KEYS_FILES), 40)
        proxy = "built-ins/Proxy/ownKeys/return-not-list-object-throws-realm.js"
        reflected = "built-ins/Reflect/ownKeys/not-a-constructor.js"
        outside = "built-ins/Proxy/ownKeys/future.js"
        self.assertEqual(
            PROXY_OWN_KEYS_FEATURES[proxy],
            {"Proxy", "Symbol", "cross-realm"},
        )
        self.assertEqual(
            PROXY_OWN_KEYS_FEATURES[reflected],
            {"Reflect", "Reflect.construct", "arrow-function"},
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative in PROXY_OWN_KEYS_FILES:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                features = set(test262_runner.parse_meta(path.read_text()).get("features", []))
                self.assertEqual(features, set(PROXY_OWN_KEYS_FEATURES[relative]), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative in PROXY_OWN_KEYS_FILES:
                        path = root / "test" / relative
                        features = list(PROXY_OWN_KEYS_FEATURES[relative])
                        self.assertTrue(tool.proxy_own_keys_path(path), relative)
                        self.assertFalse(
                            tool.should_skip({"features": features}, path),
                            relative,
                        )
                    outside_path = root / "test" / outside
                    self.assertFalse(tool.proxy_own_keys_path(outside_path))
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, outside_path)
                    )
                finally:
                    tool.TEST262 = original_root

    def test_proxy_for_in_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(PROXY_FOR_IN_FILES), 22)
        self.assertEqual(frozenset(PROXY_FOR_IN_FEATURES), PROXY_FOR_IN_FILES)
        self.assertEqual(
            sum("/getOwnPropertyDescriptor/" in path for path in PROXY_FOR_IN_FILES),
            21,
        )
        enumerate_path = "built-ins/Proxy/enumerate/removed-does-not-trigger.js"
        realm_path = (
            "built-ins/Proxy/getOwnPropertyDescriptor/"
            "result-type-is-not-object-nor-undefined-realm.js"
        )
        missing_checks_path = (
            "built-ins/Proxy/getOwnPropertyDescriptor/"
            "resultdesc-is-not-configurable-not-writable-targetdesc-is-writable.js"
        )
        self.assertEqual(
            PROXY_FOR_IN_FEATURES[enumerate_path],
            {"Proxy", "Symbol", "Symbol.iterator"},
        )
        self.assertEqual(
            PROXY_FOR_IN_FEATURES[realm_path], {"Proxy", "cross-realm"}
        )
        self.assertEqual(
            PROXY_FOR_IN_FEATURES[missing_checks_path],
            {"Proxy", "proxy-missing-checks"},
        )

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_proxy_for_in_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(PROXY_FOR_IN_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            property_helper_files = {
                "built-ins/Proxy/getOwnPropertyDescriptor/"
                "trap-is-missing-target-is-proxy.js",
                "built-ins/Proxy/getOwnPropertyDescriptor/"
                "trap-is-null-target-is-proxy.js",
                "built-ins/Proxy/getOwnPropertyDescriptor/"
                "trap-is-undefined-target-is-proxy.js",
                "built-ins/Proxy/getOwnPropertyDescriptor/trap-is-undefined.js",
            }
            for relative, features in PROXY_FOR_IN_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                expected_includes = []
                if relative in property_helper_files:
                    expected_includes = ["propertyHelper.js"]
                elif relative == enumerate_path:
                    expected_includes = ["compareArray.js"]
                self.assertEqual(metadata.get("includes", []), expected_includes, relative)
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future_descriptor = (
                root / "test/built-ins/Proxy/getOwnPropertyDescriptor/future.js"
            )
            future_enumerate = root / "test/built-ins/Proxy/enumerate/future.js"
            outside = root / "test/language/statements/for-in/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in PROXY_FOR_IN_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.proxy_for_in_path(path), relative)
                        self.assertEqual(tool.proxy_for_in_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    for path in (future_descriptor, future_enumerate, outside):
                        self.assertFalse(tool.proxy_for_in_path(path))
                        self.assertTrue(
                            tool.should_skip({"features": ["Proxy"]}, path)
                        )
                finally:
                    tool.TEST262 = original_root


class ArrayExoticAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(ARRAY_EXOTIC_FILES), 20)
        self.assertEqual(frozenset(ARRAY_EXOTIC_FEATURES), ARRAY_EXOTIC_FILES)
        self.assertEqual(
            {
                method: sum(f"/prototype/{method}/" in path for path in ARRAY_EXOTIC_FILES)
                for method in (
                    "push",
                    "pop",
                    "shift",
                    "unshift",
                    "splice",
                    "slice",
                    "with",
                )
            },
            {
                "push": 1,
                "pop": 1,
                "shift": 1,
                "unshift": 1,
                "splice": 6,
                "slice": 8,
                "with": 1,
            },
        )

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_exotic_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(ARRAY_EXOTIC_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ARRAY_EXOTIC_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test/built-ins/Array/prototype/slice/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_EXOTIC_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_exotic_path(path), relative)
                        self.assertEqual(tool.array_exotic_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    self.assertFalse(tool.array_exotic_path(outside))
                    self.assertTrue(
                        tool.should_skip({"features": ["Proxy"]}, outside)
                    )
                finally:
                    tool.TEST262 = original_root


class ArrayConcatAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(ARRAY_CONCAT_FILES), 9)
        self.assertEqual(frozenset(ARRAY_CONCAT_FEATURES), ARRAY_CONCAT_FILES)
        self.assertEqual(
            ARRAY_CONCAT_FILES,
            frozenset(
                {
                    "built-ins/Array/prototype/concat/arg-length-exceeding-integer-limit.js",
                    "built-ins/Array/prototype/concat/create-proxy.js",
                    "built-ins/Array/prototype/concat/create-revoked-proxy.js",
                    "built-ins/Array/prototype/concat/create-species-non-ctor.js",
                    "built-ins/Array/prototype/concat/is-concat-spreadable-is-array-proxy-revoked.js",
                    "built-ins/Array/prototype/concat/is-concat-spreadable-proxy-revoked.js",
                    "built-ins/Array/prototype/concat/is-concat-spreadable-proxy.js",
                    "built-ins/Array/prototype/concat/is-concat-spreadable-val-truthy.js",
                    "built-ins/Array/prototype/concat/not-a-constructor.js",
                }
            ),
        )

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_concat_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(ARRAY_CONCAT_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ARRAY_CONCAT_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/concat/future.js"
            outside = root / "test/built-ins/Array/prototype/slice/future-concat.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_CONCAT_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_concat_path(path), relative)
                        self.assertEqual(tool.array_concat_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_concat_path(path))
                        self.assertTrue(
                            tool.should_skip({"features": ["Proxy"]}, path)
                        )
                finally:
                    tool.TEST262 = original_root


class ArrayCopyWithinAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        expected = frozenset(
            {
                "built-ins/Array/prototype/copyWithin/not-a-constructor.js",
                "built-ins/Array/prototype/copyWithin/resizable-buffer.js",
                "built-ins/Array/prototype/copyWithin/return-abrupt-from-delete-proxy-target.js",
                "built-ins/Array/prototype/copyWithin/return-abrupt-from-end-as-symbol.js",
                "built-ins/Array/prototype/copyWithin/return-abrupt-from-has-start.js",
                "built-ins/Array/prototype/copyWithin/return-abrupt-from-start-as-symbol.js",
                "built-ins/Array/prototype/copyWithin/return-abrupt-from-target-as-symbol.js",
                "built-ins/Array/prototype/copyWithin/return-abrupt-from-this-length-as-symbol.js",
            }
        )
        self.assertEqual(ARRAY_COPY_WITHIN_FILES, expected)
        self.assertEqual(
            frozenset(ARRAY_COPY_WITHIN_FEATURES), ARRAY_COPY_WITHIN_FILES
        )

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_copy_within_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(ARRAY_COPY_WITHIN_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ARRAY_COPY_WITHIN_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = (
                root
                / "test/built-ins/Array/prototype/copyWithin/future-proxy.js"
            )
            outside = root / "test/built-ins/Array/prototype/fill/future-proxy.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_COPY_WITHIN_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_copy_within_path(path), relative)
                        self.assertEqual(
                            tool.array_copy_within_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_copy_within_path(path))
                        self.assertTrue(
                            tool.should_skip({"features": ["Proxy"]}, path)
                        )
                finally:
                    tool.TEST262 = original_root


class ArrayFillAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        expected = frozenset(
            {
                "built-ins/Array/prototype/fill/not-a-constructor.js",
                "built-ins/Array/prototype/fill/resizable-buffer.js",
                "built-ins/Array/prototype/fill/return-abrupt-from-end-as-symbol.js",
                "built-ins/Array/prototype/fill/return-abrupt-from-start-as-symbol.js",
                "built-ins/Array/prototype/fill/return-abrupt-from-this-length-as-symbol.js",
                "built-ins/Array/prototype/fill/typed-array-resize.js",
            }
        )
        self.assertEqual(ARRAY_FILL_FILES, expected)
        self.assertEqual(frozenset(ARRAY_FILL_FEATURES), ARRAY_FILL_FILES)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_fill_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(ARRAY_FILL_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ARRAY_FILL_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/fill/future-symbol.js"
            outside = root / "test/built-ins/Array/prototype/reverse/future-symbol.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_FILL_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_fill_path(path), relative)
                        self.assertEqual(tool.array_fill_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_fill_path(path))
                        self.assertTrue(tool.should_skip({"features": ["Symbol"]}, path))
                finally:
                    tool.TEST262 = original_root


class ArrayFilterAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        names = {
            "callbackfn-resize-arraybuffer.js",
            "create-proxy.js",
            "create-revoked-proxy.js",
            "create-species-non-ctor.js",
            "not-a-constructor.js",
            "resizable-buffer-grow-mid-iteration.js",
            "resizable-buffer-shrink-mid-iteration.js",
            "resizable-buffer.js",
        }
        expected = frozenset(
            f"built-ins/Array/prototype/filter/{name}" for name in names
        )
        self.assertEqual(ARRAY_FILTER_FILES, expected)
        self.assertEqual(frozenset(ARRAY_FILTER_FEATURES), ARRAY_FILTER_FILES)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_filter_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(ARRAY_FILTER_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ARRAY_FILTER_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/filter/future-proxy.js"
            outside = root / "test/built-ins/Array/prototype/map/future-proxy.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_FILTER_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_filter_path(path), relative)
                        self.assertEqual(tool.array_filter_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_filter_path(path))
                        self.assertTrue(tool.should_skip({"features": ["Proxy"]}, path))
                finally:
                    tool.TEST262 = original_root


class ArrayMapAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        names = {
            "callbackfn-resize-arraybuffer.js",
            "create-proxy.js",
            "create-revoked-proxy.js",
            "create-species-non-ctor.js",
            "create-species-undef-invalid-len.js",
            "not-a-constructor.js",
            "resizable-buffer-grow-mid-iteration.js",
            "resizable-buffer-shrink-mid-iteration.js",
            "resizable-buffer.js",
        }
        expected = frozenset(f"built-ins/Array/prototype/map/{name}" for name in names)
        self.assertEqual(ARRAY_MAP_FILES, expected)
        self.assertEqual(frozenset(ARRAY_MAP_FEATURES), ARRAY_MAP_FILES)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_map_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(ARRAY_MAP_FILES & existing, manifest.name)

        expected_includes = {
            "built-ins/Array/prototype/map/callbackfn-resize-arraybuffer.js": [
                "testTypedArray.js",
                "compareArray.js",
            ],
            "built-ins/Array/prototype/map/create-proxy.js": [],
            "built-ins/Array/prototype/map/create-revoked-proxy.js": [],
            "built-ins/Array/prototype/map/create-species-non-ctor.js": [
                "isConstructor.js",
            ],
            "built-ins/Array/prototype/map/create-species-undef-invalid-len.js": [],
            "built-ins/Array/prototype/map/not-a-constructor.js": [
                "isConstructor.js",
            ],
            "built-ins/Array/prototype/map/resizable-buffer-grow-mid-iteration.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
            "built-ins/Array/prototype/map/resizable-buffer-shrink-mid-iteration.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
            "built-ins/Array/prototype/map/resizable-buffer.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
        }
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ARRAY_MAP_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/map/future-proxy.js"
            outside = root / "test/built-ins/Array/prototype/filter/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_MAP_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_map_path(path), relative)
                        self.assertEqual(tool.array_map_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_map_path(path))
                        self.assertTrue(tool.should_skip({"features": ["Proxy"]}, path))
                finally:
                    tool.TEST262 = original_root


class ArrayReduceAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        expected_features = {
            "built-ins/Array/prototype/reduce/callbackfn-resize-arraybuffer.js": frozenset(
                {"TypedArray", "resizable-arraybuffer"}
            ),
            "built-ins/Array/prototype/reduce/not-a-constructor.js": frozenset(
                {"Reflect.construct", "arrow-function"}
            ),
            "built-ins/Array/prototype/reduce/resizable-buffer-grow-mid-iteration.js": frozenset(
                {"resizable-arraybuffer"}
            ),
            "built-ins/Array/prototype/reduce/resizable-buffer-shrink-mid-iteration.js": frozenset(
                {"resizable-arraybuffer"}
            ),
            "built-ins/Array/prototype/reduce/resizable-buffer.js": frozenset(
                {"resizable-arraybuffer"}
            ),
        }
        self.assertEqual(ARRAY_REDUCE_FILES, frozenset(expected_features))
        self.assertEqual(ARRAY_REDUCE_FEATURES, expected_features)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_reduce_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(ARRAY_REDUCE_FILES.isdisjoint(existing), manifest.name)

        expected_includes = {
            "built-ins/Array/prototype/reduce/callbackfn-resize-arraybuffer.js": [
                "testTypedArray.js",
                "compareArray.js",
            ],
            "built-ins/Array/prototype/reduce/not-a-constructor.js": [
                "isConstructor.js",
            ],
            "built-ins/Array/prototype/reduce/resizable-buffer-grow-mid-iteration.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
            "built-ins/Array/prototype/reduce/resizable-buffer-shrink-mid-iteration.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
            "built-ins/Array/prototype/reduce/resizable-buffer.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
        }
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/reduce/future.js"
            outside = root / "test/built-ins/Array/prototype/map/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.array_reduce_path(None))
                    self.assertEqual(tool.array_reduce_features(None), frozenset())
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_reduce_path(path), relative)
                        self.assertEqual(tool.array_reduce_features(path), features)
                        self.assertFalse(
                            tool.should_skip(
                                {"flags": [], "features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(features | {"decorators"}),
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_reduce_path(path))
                        self.assertEqual(tool.array_reduce_features(path), frozenset())
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["resizable-arraybuffer"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class ArrayReduceRightAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        expected_features = {
            "built-ins/Array/prototype/reduceRight/callbackfn-resize-arraybuffer.js": frozenset(
                {"TypedArray", "resizable-arraybuffer"}
            ),
            "built-ins/Array/prototype/reduceRight/not-a-constructor.js": frozenset(
                {"Reflect.construct", "arrow-function"}
            ),
            "built-ins/Array/prototype/reduceRight/resizable-buffer-grow-mid-iteration.js": frozenset(
                {"resizable-arraybuffer"}
            ),
            "built-ins/Array/prototype/reduceRight/resizable-buffer-shrink-mid-iteration.js": frozenset(
                {"resizable-arraybuffer"}
            ),
            "built-ins/Array/prototype/reduceRight/resizable-buffer.js": frozenset(
                {"resizable-arraybuffer"}
            ),
        }
        self.assertEqual(ARRAY_REDUCE_RIGHT_FILES, frozenset(expected_features))
        self.assertEqual(ARRAY_REDUCE_RIGHT_FEATURES, expected_features)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_reduce_right_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(ARRAY_REDUCE_RIGHT_FILES.isdisjoint(existing), manifest.name)

        expected_includes = {
            "built-ins/Array/prototype/reduceRight/callbackfn-resize-arraybuffer.js": [
                "testTypedArray.js",
                "compareArray.js",
            ],
            "built-ins/Array/prototype/reduceRight/not-a-constructor.js": [
                "isConstructor.js",
            ],
            "built-ins/Array/prototype/reduceRight/resizable-buffer-grow-mid-iteration.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
            "built-ins/Array/prototype/reduceRight/resizable-buffer-shrink-mid-iteration.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
            "built-ins/Array/prototype/reduceRight/resizable-buffer.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
        }
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/reduceRight/future.js"
            outside = root / "test/built-ins/Array/prototype/reduce/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.array_reduce_right_path(None))
                    self.assertEqual(tool.array_reduce_right_features(None), frozenset())
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_reduce_right_path(path), relative)
                        self.assertEqual(tool.array_reduce_right_features(path), features)
                        self.assertFalse(
                            tool.should_skip(
                                {"flags": [], "features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(features | {"decorators"}),
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_reduce_right_path(path))
                        self.assertEqual(tool.array_reduce_right_features(path), frozenset())
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["resizable-arraybuffer"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class ArrayReverseAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        expected_features = {
            "built-ins/Array/prototype/reverse/not-a-constructor.js": frozenset(
                {"Reflect.construct", "arrow-function"}
            ),
            "built-ins/Array/prototype/reverse/resizable-buffer.js": frozenset(
                {"resizable-arraybuffer"}
            ),
        }
        self.assertEqual(ARRAY_REVERSE_FILES, frozenset(expected_features))
        self.assertEqual(ARRAY_REVERSE_FEATURES, expected_features)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_reverse_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(ARRAY_REVERSE_FILES.isdisjoint(existing), manifest.name)

        expected_includes = {
            "built-ins/Array/prototype/reverse/not-a-constructor.js": [
                "isConstructor.js",
            ],
            "built-ins/Array/prototype/reverse/resizable-buffer.js": [
                "compareArray.js",
                "resizableArrayBufferUtils.js",
            ],
        }
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/reverse/future.js"
            outside = root / "test/built-ins/Array/prototype/reduceRight/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.array_reverse_path(None))
                    self.assertEqual(tool.array_reverse_features(None), frozenset())
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_reverse_path(path), relative)
                        self.assertEqual(tool.array_reverse_features(path), features)
                        self.assertFalse(
                            tool.should_skip(
                                {"flags": [], "features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(features | {"decorators"}),
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_reverse_path(path))
                        self.assertEqual(tool.array_reverse_features(path), frozenset())
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["resizable-arraybuffer"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class ArrayToReversedAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        expected_features = {
            "built-ins/Array/prototype/toReversed/not-a-constructor.js": frozenset(
                {"Reflect.construct"}
            ),
        }
        self.assertEqual(ARRAY_TO_REVERSED_FILES, frozenset(expected_features))
        self.assertEqual(ARRAY_TO_REVERSED_FEATURES, expected_features)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_to_reversed_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(ARRAY_TO_REVERSED_FILES.isdisjoint(existing), manifest.name)

        expected_includes = {
            "built-ins/Array/prototype/toReversed/not-a-constructor.js": [
                "isConstructor.js",
            ],
        }
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])),
                    features | {"change-array-by-copy"},
                    relative,
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/toReversed/future.js"
            outside = root / "test/built-ins/Array/prototype/unrelated/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.array_to_reversed_path(None))
                    self.assertEqual(tool.array_to_reversed_features(None), frozenset())
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_to_reversed_path(path), relative)
                        self.assertEqual(tool.array_to_reversed_features(path), features)
                        self.assertFalse(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(
                                        features | {"change-array-by-copy"}
                                    ),
                                },
                                path,
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(
                                        features
                                        | {"change-array-by-copy", "decorators"}
                                    ),
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_to_reversed_path(path))
                        self.assertEqual(
                            tool.array_to_reversed_features(path), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["Reflect.construct"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class ArrayToSplicedAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        expected_features = {
            "built-ins/Array/prototype/toSpliced/not-a-constructor.js": frozenset(
                {"Reflect.construct"}
            ),
        }
        self.assertEqual(ARRAY_TO_SPLICED_FILES, frozenset(expected_features))
        self.assertEqual(ARRAY_TO_SPLICED_FEATURES, expected_features)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_to_spliced_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(ARRAY_TO_SPLICED_FILES.isdisjoint(existing), manifest.name)

        expected_includes = {
            "built-ins/Array/prototype/toSpliced/not-a-constructor.js": [
                "isConstructor.js",
            ],
        }
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])),
                    features | {"change-array-by-copy"},
                    relative,
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/toSpliced/future.js"
            outside = root / "test/built-ins/Array/prototype/unrelatedCopy/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.array_to_spliced_path(None))
                    self.assertEqual(tool.array_to_spliced_features(None), frozenset())
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_to_spliced_path(path), relative)
                        self.assertEqual(tool.array_to_spliced_features(path), features)
                        self.assertFalse(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(
                                        features | {"change-array-by-copy"}
                                    ),
                                },
                                path,
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(
                                        features
                                        | {"change-array-by-copy", "decorators"}
                                    ),
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_to_spliced_path(path))
                        self.assertEqual(tool.array_to_spliced_features(path), frozenset())
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["Reflect.construct"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class ArrayToLocaleStringAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        base = "built-ins/Array/prototype/toLocaleString"
        expected_features = {
            f"{base}/not-a-constructor.js": frozenset(
                {"Reflect.construct", "arrow-function"}
            ),
            f"{base}/resizable-buffer.js": frozenset({"resizable-arraybuffer"}),
            f"{base}/user-provided-tolocalestring-grow.js": frozenset(
                {"resizable-arraybuffer"}
            ),
            f"{base}/user-provided-tolocalestring-shrink.js": frozenset(
                {"resizable-arraybuffer"}
            ),
        }
        self.assertEqual(
            ARRAY_TO_LOCALE_STRING_FILES, frozenset(expected_features)
        )
        self.assertEqual(ARRAY_TO_LOCALE_STRING_FEATURES, expected_features)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_to_locale_string_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                ARRAY_TO_LOCALE_STRING_FILES.isdisjoint(existing), manifest.name
            )

        expected_includes = {
            f"{base}/not-a-constructor.js": ["isConstructor.js"],
            f"{base}/resizable-buffer.js": ["resizableArrayBufferUtils.js"],
            f"{base}/user-provided-tolocalestring-grow.js": [
                "resizableArrayBufferUtils.js"
            ],
            f"{base}/user-provided-tolocalestring-shrink.js": [
                "resizableArrayBufferUtils.js"
            ],
        }
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / f"test/{base}/future.js"
            outside = (
                root
                / "test/built-ins/Array/prototype/unrelatedLocale/not-a-constructor.js"
            )
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.array_to_locale_string_path(None))
                    self.assertEqual(
                        tool.array_to_locale_string_features(None), frozenset()
                    )
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(
                            tool.array_to_locale_string_path(path), relative
                        )
                        self.assertEqual(
                            tool.array_to_locale_string_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"flags": [], "features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(features | {"decorators"}),
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_to_locale_string_path(path))
                        self.assertEqual(
                            tool.array_to_locale_string_features(path), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": ["resizable-arraybuffer"],
                                },
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class TypedArrayToLocaleStringAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        base = "built-ins/TypedArray/prototype/toLocaleString"
        bigint_names = {
            "calls-tolocalestring-from-each-value.js",
            "calls-tostring-from-each-value.js",
            "calls-valueof-from-each-value.js",
            "detached-buffer.js",
            "empty-instance-returns-empty-string.js",
            "get-length-uses-internal-arraylength.js",
            "return-abrupt-from-firstelement-tolocalestring.js",
            "return-abrupt-from-firstelement-tostring.js",
            "return-abrupt-from-firstelement-valueof.js",
            "return-abrupt-from-nextelement-tolocalestring.js",
            "return-abrupt-from-nextelement-tostring.js",
            "return-abrupt-from-nextelement-valueof.js",
            "return-abrupt-from-this-out-of-bounds.js",
            "return-result.js",
        }
        number_names = {
            "calls-tolocalestring-from-each-value.js",
            "calls-tostring-from-each-value.js",
            "calls-valueof-from-each-value.js",
            "detached-buffer.js",
            "empty-instance-returns-empty-string.js",
            "get-length-uses-internal-arraylength.js",
            "invoked-as-func.js",
            "invoked-as-method.js",
            "length.js",
            "name.js",
            "not-a-constructor.js",
            "prop-desc.js",
            "return-abrupt-from-firstelement-tolocalestring.js",
            "return-abrupt-from-firstelement-tostring.js",
            "return-abrupt-from-firstelement-valueof.js",
            "return-abrupt-from-nextelement-tolocalestring.js",
            "return-abrupt-from-nextelement-tostring.js",
            "return-abrupt-from-nextelement-valueof.js",
            "return-abrupt-from-this-out-of-bounds.js",
            "return-result.js",
            "this-is-not-object.js",
            "this-is-not-typedarray-instance.js",
        }
        rab_names = {
            "resizable-buffer.js",
            "user-provided-tolocalestring-grow.js",
            "user-provided-tolocalestring-shrink.js",
        }

        expected_features = {
            f"{base}/BigInt/{name}": frozenset({"BigInt", "TypedArray"})
            for name in bigint_names
        }
        expected_features.update(
            {
                f"{base}/{name}": frozenset({"TypedArray"})
                for name in number_names
            }
        )
        expected_features.update(
            {
                f"{base}/{name}": frozenset({"resizable-arraybuffer"})
                for name in rab_names
            }
        )
        for prefix in ("", "BigInt/"):
            relative = f"{base}/{prefix}return-abrupt-from-this-out-of-bounds.js"
            expected_features[relative] |= {
                "ArrayBuffer",
                "arrow-function",
                "resizable-arraybuffer",
            }
        expected_features[f"{base}/not-a-constructor.js"] |= {
            "Reflect.construct",
            "arrow-function",
        }
        expected_features[f"{base}/this-is-not-object.js"] |= {"Symbol"}

        self.assertEqual(
            TYPED_ARRAY_TO_LOCALE_STRING_FILES, frozenset(expected_features)
        )
        self.assertEqual(
            TYPED_ARRAY_TO_LOCALE_STRING_FEATURES, expected_features
        )

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_typed_array_to_locale_string_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                TYPED_ARRAY_TO_LOCALE_STRING_FILES.isdisjoint(existing),
                manifest.name,
            )

        expected_includes = {
            relative: ["testTypedArray.js"] for relative in expected_features
        }
        expected_includes[f"{base}/calls-tolocalestring-from-each-value.js"] = [
            "testTypedArray.js",
            "compareArray.js",
        ]
        expected_includes[
            f"{base}/BigInt/calls-tolocalestring-from-each-value.js"
        ] = ["testTypedArray.js", "compareArray.js"]
        for prefix in ("", "BigInt/"):
            expected_includes[f"{base}/{prefix}detached-buffer.js"] = [
                "testTypedArray.js",
                "detachArrayBuffer.js",
            ]
        for name in ("length.js", "name.js", "prop-desc.js"):
            expected_includes[f"{base}/{name}"] = [
                "propertyHelper.js",
                "testTypedArray.js",
            ]
        expected_includes[f"{base}/not-a-constructor.js"] = [
            "isConstructor.js",
            "testTypedArray.js",
        ]
        for name in rab_names:
            expected_includes[f"{base}/{name}"] = ["resizableArrayBufferUtils.js"]

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            actual_files = frozenset(
                path.relative_to(test_root).as_posix()
                for path in (test_root / base).rglob("*.js")
                if "_FIXTURE" not in path.name
            )
            self.assertEqual(
                actual_files, TYPED_ARRAY_TO_LOCALE_STRING_FILES
            )
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / f"test/{base}/future.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.typed_array_to_locale_string_path(None))
                    self.assertEqual(
                        tool.typed_array_to_locale_string_features(None), frozenset()
                    )
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(
                            tool.typed_array_to_locale_string_path(path), relative
                        )
                        self.assertEqual(
                            tool.typed_array_to_locale_string_features(path), features
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"flags": [], "features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(features | {"decorators"}),
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.typed_array_to_locale_string_path(path))
                        self.assertEqual(
                            tool.typed_array_to_locale_string_features(path),
                            frozenset(),
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["resizable-arraybuffer"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class TypedArrayJoinAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        base = "built-ins/TypedArray/prototype/join"
        bigint_names = {
            "custom-separator-result-from-tostring-on-each-simple-value.js",
            "detached-buffer-during-fromIndex-returns-single-comma.js",
            "detached-buffer.js",
            "empty-instance-empty-string.js",
            "get-length-uses-internal-arraylength.js",
            "result-from-tostring-on-each-simple-value.js",
            "return-abrupt-from-separator-symbol.js",
            "return-abrupt-from-separator.js",
            "return-abrupt-from-this-out-of-bounds.js",
        }
        number_names = {
            "custom-separator-result-from-tostring-on-each-simple-value.js",
            "custom-separator-result-from-tostring-on-each-value.js",
            "detached-buffer-during-fromIndex-returns-single-comma.js",
            "detached-buffer.js",
            "empty-instance-empty-string.js",
            "get-length-uses-internal-arraylength.js",
            "invoked-as-func.js",
            "invoked-as-method.js",
            "length.js",
            "name.js",
            "not-a-constructor.js",
            "prop-desc.js",
            "result-from-tostring-on-each-simple-value.js",
            "result-from-tostring-on-each-value.js",
            "return-abrupt-from-separator-symbol.js",
            "return-abrupt-from-separator.js",
            "return-abrupt-from-this-out-of-bounds.js",
            "separator-tostring-once-after-resized.js",
            "this-is-not-object.js",
            "this-is-not-typedarray-instance.js",
        }
        rab_names = {
            "coerced-separator-grow.js",
            "coerced-separator-shrink.js",
            "resizable-buffer.js",
        }

        expected_features = {
            f"{base}/BigInt/{name}": frozenset({"BigInt", "TypedArray"})
            for name in bigint_names
        }
        expected_features.update(
            {
                f"{base}/{name}": frozenset({"TypedArray"})
                for name in number_names
            }
        )
        expected_features.update(
            {
                f"{base}/{name}": frozenset({"resizable-arraybuffer"})
                for name in rab_names
            }
        )
        for prefix in ("", "BigInt/"):
            expected_features[
                f"{base}/{prefix}detached-buffer-during-fromIndex-returns-single-comma.js"
            ] |= {"align-detached-buffer-semantics-with-web-reality"}
            expected_features[
                f"{base}/{prefix}return-abrupt-from-separator-symbol.js"
            ] |= {"Symbol"}
            expected_features[
                f"{base}/{prefix}return-abrupt-from-this-out-of-bounds.js"
            ] |= {"resizable-arraybuffer"}
        expected_features[
            f"{base}/BigInt/return-abrupt-from-this-out-of-bounds.js"
        ] |= {"ArrayBuffer", "arrow-function"}
        expected_features[f"{base}/separator-tostring-once-after-resized.js"] |= {
            "resizable-arraybuffer"
        }
        expected_features[f"{base}/not-a-constructor.js"] |= {
            "Reflect.construct",
            "arrow-function",
        }
        expected_features[f"{base}/this-is-not-object.js"] |= {"Symbol"}

        self.assertEqual(TYPED_ARRAY_JOIN_FILES, frozenset(expected_features))
        self.assertEqual(TYPED_ARRAY_JOIN_FEATURES, expected_features)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_typed_array_join_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(TYPED_ARRAY_JOIN_FILES.isdisjoint(existing), manifest.name)

        expected_includes = {
            relative: ["testTypedArray.js"] for relative in expected_features
        }
        for prefix in ("", "BigInt/"):
            for name in (
                "detached-buffer-during-fromIndex-returns-single-comma.js",
                "detached-buffer.js",
            ):
                expected_includes[f"{base}/{prefix}{name}"] = [
                    "testTypedArray.js",
                    "detachArrayBuffer.js",
                ]
        for name in ("length.js", "name.js", "prop-desc.js"):
            expected_includes[f"{base}/{name}"] = [
                "propertyHelper.js",
                "testTypedArray.js",
            ]
        expected_includes[f"{base}/not-a-constructor.js"] = [
            "isConstructor.js",
            "testTypedArray.js",
        ]
        for name in rab_names:
            expected_includes[f"{base}/{name}"] = ["resizableArrayBufferUtils.js"]
        expected_includes[f"{base}/separator-tostring-once-after-resized.js"] = []

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            actual_files = frozenset(
                path.relative_to(test_root).as_posix()
                for path in (test_root / base).rglob("*.js")
                if "_FIXTURE" not in path.name
            )
            self.assertEqual(actual_files, TYPED_ARRAY_JOIN_FILES)
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / f"test/{base}/future.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.typed_array_join_path(None))
                    self.assertEqual(tool.typed_array_join_features(None), frozenset())
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.typed_array_join_path(path), relative)
                        self.assertEqual(
                            tool.typed_array_join_features(path), features, relative
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"flags": [], "features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(features | {"decorators"}),
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.typed_array_join_path(path))
                        self.assertEqual(
                            tool.typed_array_join_features(path), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["resizable-arraybuffer"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class ArrayForEachAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        names = {
            "callbackfn-resize-arraybuffer.js",
            "not-a-constructor.js",
            "resizable-buffer-grow-mid-iteration.js",
            "resizable-buffer-shrink-mid-iteration.js",
            "resizable-buffer.js",
        }
        expected = frozenset(
            f"built-ins/Array/prototype/forEach/{name}" for name in names
        )
        self.assertEqual(ARRAY_FOR_EACH_FILES, expected)
        self.assertEqual(frozenset(ARRAY_FOR_EACH_FEATURES), ARRAY_FOR_EACH_FILES)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_for_each_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(ARRAY_FOR_EACH_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ARRAY_FOR_EACH_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/forEach/future.js"
            outside = root / "test/built-ins/Array/prototype/map/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_FOR_EACH_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_for_each_path(path), relative)
                        self.assertEqual(tool.array_for_each_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": list(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": list(features | {"decorators"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_for_each_path(path))
                        self.assertTrue(tool.should_skip({"features": ["Symbol"]}, path))
                finally:
                    tool.TEST262 = original_root


class ArrayJoinAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        names = {
            "coerced-separator-grow.js",
            "coerced-separator-shrink.js",
            "not-a-constructor.js",
            "resizable-buffer.js",
        }
        expected = frozenset(
            f"built-ins/Array/prototype/join/{name}" for name in names
        )
        self.assertEqual(ARRAY_JOIN_FILES, expected)
        self.assertEqual(frozenset(ARRAY_JOIN_FEATURES), ARRAY_JOIN_FILES)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_join_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(ARRAY_JOIN_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ARRAY_JOIN_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/join/future.js"
            outside = root / "test/built-ins/Array/prototype/map/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_JOIN_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_join_path(path), relative)
                        self.assertEqual(tool.array_join_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": list(features)}, path)
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": list(features | {"decorators"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_join_path(path))
                        self.assertTrue(tool.should_skip({"features": ["Symbol"]}, path))
                finally:
                    tool.TEST262 = original_root


class ArrayFlatAdmissionTests(unittest.TestCase):
    def test_manifests_are_exact_live_disjoint_and_shared(self):
        expected_flat = frozenset(
            {"built-ins/Array/prototype/flat/not-a-constructor.js"}
        )
        flat_map_names = {
            "array-like-objects-nested.js",
            "array-like-objects-typedarrays.js",
            "non-callable-argument-throws.js",
            "not-a-constructor.js",
            "this-value-ctor-non-object.js",
            "this-value-ctor-object-species-bad-throws.js",
            "this-value-ctor-object-species-custom-ctor-poisoned-throws.js",
            "this-value-ctor-object-species-custom-ctor.js",
            "this-value-ctor-object-species.js",
        }
        expected_flat_map = frozenset(
            f"built-ins/Array/prototype/flatMap/{name}" for name in flat_map_names
        )
        self.assertEqual(ARRAY_FLAT_FILES, expected_flat)
        self.assertEqual(ARRAY_FLAT_MAP_FILES, expected_flat_map)
        self.assertEqual(frozenset(ARRAY_FLAT_FEATURES), ARRAY_FLAT_FILES)
        self.assertEqual(frozenset(ARRAY_FLAT_MAP_FEATURES), ARRAY_FLAT_MAP_FILES)
        self.assertFalse(ARRAY_FLAT_FILES & ARRAY_FLAT_MAP_FILES)

        admitted = ARRAY_FLAT_FILES | ARRAY_FLAT_MAP_FILES
        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_flat_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(admitted & existing, manifest.name)

        feature_maps = (ARRAY_FLAT_FEATURES, ARRAY_FLAT_MAP_FEATURES)
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for feature_map in feature_maps:
                for relative, features in feature_map.items():
                    path = test_root / relative
                    self.assertTrue(path.is_file(), relative)
                    metadata = test262_runner.parse_meta(path.read_text())
                    self.assertEqual(
                        frozenset(metadata.get("features", [])), features, relative
                    )
                    self.assertEqual(metadata.get("flags", []), [], relative)
                    self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Array/prototype/flat/future.js"
            outside = root / "test/built-ins/Array/prototype/map/not-a-constructor.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_FLAT_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_flat_path(path), relative)
                        self.assertEqual(tool.array_flat_features(path), features)
                        self.assertFalse(tool.should_skip({"features": list(features)}, path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": list(features | {"decorators"})}, path
                            )
                        )
                    for relative, features in ARRAY_FLAT_MAP_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_flat_map_path(path), relative)
                        self.assertEqual(tool.array_flat_map_features(path), features)
                        self.assertFalse(tool.should_skip({"features": list(features)}, path))
                        self.assertTrue(
                            tool.should_skip(
                                {"features": list(features | {"decorators"})}, path
                            )
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.array_flat_path(path))
                        self.assertFalse(tool.array_flat_map_path(path))
                        self.assertTrue(tool.should_skip({"features": ["Symbol"]}, path))
                finally:
                    tool.TEST262 = original_root


class ArrayIteratorAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        direct_names = (
            "not-a-constructor.js",
            "resizable-buffer-grow-mid-iteration.js",
            "resizable-buffer-shrink-mid-iteration.js",
            "resizable-buffer.js",
            "returns-iterator-from-object.js",
            "returns-iterator.js",
        )
        expected = {
            f"built-ins/Array/prototype/{method}/{name}"
            for method in ("entries", "keys", "values")
            for name in direct_names
        }
        expected.update(
            {
                "built-ins/ArrayIteratorPrototype/Symbol.toStringTag/property-descriptor.js",
                "built-ins/ArrayIteratorPrototype/Symbol.toStringTag/value-direct.js",
                "built-ins/ArrayIteratorPrototype/Symbol.toStringTag/value-from-to-string.js",
                "built-ins/ArrayIteratorPrototype/next/Float32Array.js",
                "built-ins/ArrayIteratorPrototype/next/Float64Array.js",
                "built-ins/ArrayIteratorPrototype/next/Int16Array.js",
                "built-ins/ArrayIteratorPrototype/next/Int32Array.js",
                "built-ins/ArrayIteratorPrototype/next/Int8Array.js",
                "built-ins/ArrayIteratorPrototype/next/Uint16Array.js",
                "built-ins/ArrayIteratorPrototype/next/Uint32Array.js",
                "built-ins/ArrayIteratorPrototype/next/Uint8Array.js",
                "built-ins/ArrayIteratorPrototype/next/Uint8ClampedArray.js",
                "built-ins/ArrayIteratorPrototype/next/args-mapped-expansion-after-exhaustion.js",
                "built-ins/ArrayIteratorPrototype/next/args-mapped-expansion-before-exhaustion.js",
                "built-ins/ArrayIteratorPrototype/next/args-mapped-iteration.js",
                "built-ins/ArrayIteratorPrototype/next/args-mapped-truncation-before-exhaustion.js",
                "built-ins/ArrayIteratorPrototype/next/args-unmapped-expansion-after-exhaustion.js",
                "built-ins/ArrayIteratorPrototype/next/args-unmapped-expansion-before-exhaustion.js",
                "built-ins/ArrayIteratorPrototype/next/args-unmapped-iteration.js",
                "built-ins/ArrayIteratorPrototype/next/args-unmapped-truncation-before-exhaustion.js",
                "built-ins/ArrayIteratorPrototype/next/detach-typedarray-in-progress.js",
                "built-ins/ArrayIteratorPrototype/next/iteration-mutable.js",
                "built-ins/ArrayIteratorPrototype/next/iteration.js",
                "built-ins/ArrayIteratorPrototype/next/length.js",
                "built-ins/ArrayIteratorPrototype/next/name.js",
                "built-ins/ArrayIteratorPrototype/next/non-own-slots.js",
                "built-ins/ArrayIteratorPrototype/next/property-descriptor.js",
                "language/arguments-object/mapped/Symbol.iterator.js",
                "language/arguments-object/unmapped/Symbol.iterator.js",
            }
        )
        self.assertEqual(ARRAY_ITERATOR_FILES, frozenset(expected))
        self.assertEqual(frozenset(ARRAY_ITERATOR_FEATURES), ARRAY_ITERATOR_FILES)

        admission_dir = Path(__file__).resolve().parent
        for manifest in admission_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_array_iterator_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertFalse(ARRAY_ITERATOR_FILES & existing, manifest.name)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative, features in ARRAY_ITERATOR_FEATURES.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                expected_flags = (
                    ["noStrict"]
                    if "/next/args-" in relative
                    or relative.startswith("language/arguments-object/")
                    else []
                )
                self.assertEqual(metadata.get("flags", []), expected_flags, relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future_paths = (
                root / "test/built-ins/Array/prototype/entries/future-proxy.js",
                root
                / "test/built-ins/ArrayIteratorPrototype/next/future-symbol.js",
                root
                / "test/language/arguments-object/mapped/future-symbol-iterator.js",
                root / "test/built-ins/Array/prototype/fill/future-proxy.js",
            )
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in ARRAY_ITERATOR_FEATURES.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.array_iterator_path(path), relative)
                        self.assertEqual(tool.array_iterator_features(path), features)
                        self.assertFalse(
                            tool.should_skip({"features": sorted(features)}, path),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": sorted(features | {"decorators"})},
                                path,
                            ),
                            relative,
                        )
                    for path in future_paths:
                        self.assertFalse(tool.array_iterator_path(path))
                        self.assertTrue(
                            tool.should_skip({"features": ["Proxy"]}, path)
                            if "proxy" in path.name
                            else tool.should_skip(
                                {"features": ["Symbol.iterator"]}, path
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class RegExpMatchIndicesAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(REGEXP_MATCH_INDICES_FILES), 7)
        self.assertEqual(
            frozenset(REGEXP_MATCH_INDICES_FEATURES),
            REGEXP_MATCH_INDICES_FILES,
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative in REGEXP_MATCH_INDICES_FILES:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                features = set(
                    test262_runner.parse_meta(path.read_text()).get("features", [])
                )
                self.assertIn("regexp-match-indices", features, relative)
                self.assertIn("regexp-named-groups", features, relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/RegExp/match-indices/future.js"
            outside = root / "test/built-ins/RegExp/future-indices.js"
            metadata = {
                "features": ["regexp-match-indices", "regexp-named-groups"]
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, admitted in REGEXP_MATCH_INDICES_FEATURES.items():
                        path = root / "test" / relative
                        self.assertEqual(
                            tool.regexp_match_indices_features(path), admitted, relative
                        )
                        self.assertFalse(tool.should_skip(metadata, path), relative)
                    for path in (future, outside):
                        self.assertEqual(
                            tool.regexp_match_indices_features(path), frozenset()
                        )
                        self.assertTrue(tool.should_skip(metadata, path))
                finally:
                    tool.TEST262 = original_root


class RegExpNamedGroupsAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        self.assertEqual(len(REGEXP_NAMED_GROUPS_FILES), 86)
        self.assertEqual(
            frozenset(REGEXP_NAMED_GROUPS_FEATURES),
            REGEXP_NAMED_GROUPS_FILES,
        )
        existing = (
            REGEXP_MATCH_INDICES_FILES
            | REGEXP_DUPLICATE_NAMED_GROUPS_FILES
            | REGEXP_UNICODE_SETS_FILES
            | REGEXP_UV_FLAGS_FILES
            | REGEXP_LOGICAL_UTF16_FILES
        )
        self.assertFalse(REGEXP_NAMED_GROUPS_FILES & existing)

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            positive_language = {
                "language/literals/regexp/named-groups/forward-reference.js",
                "language/literals/regexp/named-groups/invalid-lone-surrogate-groupname.js",
            }
            for relative in REGEXP_NAMED_GROUPS_FILES:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertIn(
                    "regexp-named-groups", metadata.get("features", []), relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                if relative.startswith("language/") and relative not in positive_language:
                    self.assertEqual(
                        metadata.get("negative"),
                        {"phase": "parse", "type": "SyntaxError"},
                        relative,
                    )
                else:
                    self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future_paths = (
                root / "test/built-ins/RegExp/named-groups/future.js",
                root / "test/built-ins/RegExp/prototype/Symbol.replace/future.js",
                root / "test/language/literals/regexp/named-groups/future.js",
            )
            poisoned = (
                root
                / "test/built-ins/RegExp/prototype/Symbol.replace/poisoned-stdlib.js"
            )
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, admitted in REGEXP_NAMED_GROUPS_FEATURES.items():
                        path = root / "test" / relative
                        self.assertEqual(
                            tool.regexp_named_groups_features(path), admitted, relative
                        )
                        self.assertFalse(
                            tool.should_skip({"features": list(admitted)}, path),
                            relative,
                        )
                    for path in future_paths:
                        self.assertEqual(
                            tool.regexp_named_groups_features(path), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": ["regexp-named-groups"]}, path
                            )
                        )
                    self.assertEqual(
                        tool.regexp_named_groups_features(poisoned), frozenset()
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "features": [
                                    "Symbol.iterator",
                                    "Symbol.replace",
                                    "regexp-named-groups",
                                ]
                            },
                            poisoned,
                        )
                    )
                finally:
                    tool.TEST262 = original_root


class RegExpDuplicateNamedGroupsAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(REGEXP_DUPLICATE_NAMED_GROUPS_FILES), 19)
        self.assertEqual(
            frozenset(REGEXP_DUPLICATE_NAMED_GROUPS_FEATURES),
            REGEXP_DUPLICATE_NAMED_GROUPS_FILES,
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative in REGEXP_DUPLICATE_NAMED_GROUPS_FILES:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                features = set(
                    test262_runner.parse_meta(path.read_text()).get("features", [])
                )
                self.assertTrue(
                    REGEXP_DUPLICATE_NAMED_GROUPS_FEATURES[relative] <= features,
                    relative,
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/RegExp/named-groups/future-duplicate.js"
            outside = root / "test/built-ins/String/prototype/search/future-duplicate.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, admitted in (
                        REGEXP_DUPLICATE_NAMED_GROUPS_FEATURES.items()
                    ):
                        path = root / "test" / relative
                        self.assertEqual(
                            tool.regexp_duplicate_named_groups_features(path),
                            admitted,
                            relative,
                        )
                        metadata = {"features": list(admitted)}
                        self.assertFalse(tool.should_skip(metadata, path), relative)
                    for path in (future, outside):
                        self.assertEqual(
                            tool.regexp_duplicate_named_groups_features(path),
                            frozenset(),
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": ["regexp-duplicate-named-groups"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class RegExpUnicodeSetsAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(REGEXP_UNICODE_SETS_FILES), 142)
        self.assertEqual(
            frozenset(REGEXP_UNICODE_SETS_FEATURES),
            REGEXP_UNICODE_SETS_FILES,
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative in REGEXP_UNICODE_SETS_FILES:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                features = set(
                    test262_runner.parse_meta(path.read_text()).get("features", [])
                )
                self.assertEqual(
                    REGEXP_UNICODE_SETS_FEATURES[relative],
                    features,
                    relative,
                )
                for tool in (test262_runner, test262_analyze):
                    self.assertFalse(
                        tool.should_skip({"features": list(features)}, path),
                        relative,
                    )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = (
                root
                / "test/built-ins/RegExp/unicodeSets/generated/character-union-future.js"
            )
            outside = (
                root
                / "test/built-ins/RegExp/unicodeSets/non-generated.js"
            )
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, admitted in REGEXP_UNICODE_SETS_FEATURES.items():
                        path = root / "test" / relative
                        self.assertEqual(
                            tool.regexp_unicode_sets_features(path),
                            admitted,
                            relative,
                        )
                        self.assertFalse(
                            tool.should_skip({"features": list(admitted)}, path),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertEqual(
                            tool.regexp_unicode_sets_features(path), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": ["regexp-v-flag"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class RegExpUvFlagsAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(REGEXP_UV_FLAGS_FILES), 2)
        self.assertEqual(
            frozenset(REGEXP_UV_FLAGS_FEATURES),
            REGEXP_UV_FLAGS_FILES,
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative in REGEXP_UV_FLAGS_FILES:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    REGEXP_UV_FLAGS_FEATURES[relative],
                    set(metadata.get("features", [])),
                    relative,
                )
                for tool in (test262_runner, test262_analyze):
                    self.assertFalse(tool.should_skip(metadata, path), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/RegExp/prototype/unicodeSets/uv-future.js"
            outside = root / "test/built-ins/RegExp/uv-flags.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, admitted in REGEXP_UV_FLAGS_FEATURES.items():
                        path = root / "test" / relative
                        self.assertEqual(
                            tool.regexp_uv_flags_features(path),
                            admitted,
                            relative,
                        )
                        self.assertFalse(
                            tool.should_skip({"features": list(admitted)}, path),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertEqual(
                            tool.regexp_uv_flags_features(path), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": ["regexp-v-flag"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class RegExpLogicalUtf16AdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_and_shared(self):
        self.assertEqual(len(REGEXP_LOGICAL_UTF16_FILES), 2)
        self.assertEqual(
            frozenset(REGEXP_LOGICAL_UTF16_FEATURES),
            REGEXP_LOGICAL_UTF16_FILES,
        )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            for relative in REGEXP_LOGICAL_UTF16_FILES:
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    REGEXP_LOGICAL_UTF16_FEATURES[relative],
                    set(metadata.get("features", [])),
                    relative,
                )
                for tool in (test262_runner, test262_analyze):
                    self.assertFalse(tool.should_skip(metadata, path), relative)
                    self.assertEqual(tool.test_timeout_seconds(path), 30)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/RegExp/property-escapes/generated/future.js"
            outside = root / "test/built-ins/RegExp/property-escapes/Surrogate.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, admitted in REGEXP_LOGICAL_UTF16_FEATURES.items():
                        path = root / "test" / relative
                        self.assertEqual(
                            tool.regexp_logical_utf16_features(path), admitted
                        )
                        self.assertFalse(
                            tool.should_skip({"features": list(admitted)}, path)
                        )
                    for path in (future, outside):
                        self.assertEqual(
                            tool.regexp_logical_utf16_features(path), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"features": ["regexp-unicode-property-escapes"]},
                                path,
                            )
                        )
                finally:
                    tool.TEST262 = original_root


class TypedArrayResizableAdmissionTests(unittest.TestCase):
    def test_typed_array_static_features_are_frozen_to_audited_files(self):
        expected = {
            f"built-ins/TypedArray/{name}"
            for name in (
                "Symbol.species/prop-desc.js", "Symbol.species/result.js",
                "invoked.js", "length.js", "name.js",
                "of/invoked-as-func.js", "of/invoked-as-method.js",
                "of/length.js", "of/name.js", "of/not-a-constructor.js",
                "of/prop-desc.js", "of/this-is-not-constructor.js",
                "prototype.js",
            )
        }
        meta = {
            "flags": [],
            "features": [
                "Reflect.construct", "Symbol.species", "TypedArray",
                "arrow-function",
            ],
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/TypedArray/of/future.js"
            outside = root / "test/built-ins/TypedArray/unsupported/name.js"
            for tool in (test262_runner, test262_analyze):
                self.assertEqual(tool.TYPED_ARRAY_STATIC_FILES, expected)
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative in expected:
                        self.assertFalse(tool.should_skip(
                            meta, root / "test" / relative
                        ))
                    self.assertTrue(tool.should_skip(meta, future))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_typed_array_from_features_are_frozen_to_all_audited_files(self):
        expected = {
            f"built-ins/TypedArray/from/{name}"
            for name in (
                "arylk-get-length-error.js", "arylk-to-length-error.js",
                "from-array-mapper-detaches-result.js",
                "from-array-mapper-makes-result-out-of-bounds.js",
                "from-typedarray-into-itself-mapper-detaches-result.js",
                "from-typedarray-into-itself-mapper-makes-result-out-of-bounds.js",
                "from-typedarray-mapper-detaches-result.js",
                "from-typedarray-mapper-makes-result-out-of-bounds.js",
                "invoked-as-func.js", "invoked-as-method.js",
                "iter-access-error.js", "iter-invoke-error.js",
                "iter-next-error.js", "iter-next-value-error.js",
                "iterated-array-changed-by-tonumber.js", "length.js",
                "mapfn-is-not-callable.js", "name.js", "not-a-constructor.js",
                "prop-desc.js", "this-is-not-constructor.js",
            )
        }
        meta = {
            "flags": [],
            "features": [
                "Reflect.construct", "Symbol", "Symbol.iterator", "TypedArray",
                "arrow-function", "resizable-arraybuffer",
            ],
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/TypedArray/from/future.js"
            outside = root / "test/built-ins/TypedArray/unsupported/length.js"
            for tool in (test262_runner, test262_analyze):
                self.assertEqual(tool.TYPED_ARRAY_FROM_FILES, expected)
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative in expected:
                        self.assertFalse(tool.should_skip(meta, root / "test" / relative))
                    self.assertTrue(tool.should_skip(meta, future))
                    self.assertTrue(tool.should_skip(meta, outside))
                    for feature in meta["features"]:
                        if feature in tool.SKIP_FEATURES:
                            self.assertTrue(tool.should_skip(
                                {"flags": [], "features": [feature]}, future
                            ))
                finally:
                    tool.TEST262 = original_root

    def test_typed_array_prototype_intrinsics_are_frozen_to_parent_files(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            iterator = root / "test/built-ins/TypedArray/prototype/Symbol.iterator.js"
            constructor = root / "test/built-ins/TypedArray/prototype/constructor.js"
            future = root / "test/built-ins/TypedArray/prototype/future.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(
                        {"flags": [], "features": ["Symbol.iterator"]}, iterator
                    ))
                    self.assertFalse(tool.should_skip(
                        {"flags": [], "features": ["TypedArray"]}, constructor
                    ))
                    self.assertTrue(tool.should_skip(
                        {"flags": [], "features": ["TypedArray"]}, future
                    ))
                    self.assertTrue(tool.should_skip(
                        {"flags": [], "features": ["Symbol.iterator"]}, future
                    ))
                    self.assertTrue(tool.should_skip(
                        {"flags": [], "features": ["Symbol.iterator", "TypedArray"]},
                        future,
                    ))
                finally:
                    tool.TEST262 = original_root

    def _assert_typed_array_to_string_features_are_frozen_to_audited_files(self):
        expected_features = {
            "built-ins/TypedArray/prototype/toString.js": frozenset({"TypedArray"}),
            "built-ins/TypedArray/prototype/toString/BigInt/detached-buffer.js": frozenset(
                {"BigInt", "TypedArray"}
            ),
            "built-ins/TypedArray/prototype/toString/detached-buffer.js": frozenset(
                {"TypedArray"}
            ),
            "built-ins/TypedArray/prototype/toString/not-a-constructor.js": frozenset(
                {"Reflect.construct", "TypedArray", "arrow-function"}
            ),
        }
        expected_includes = {
            "built-ins/TypedArray/prototype/toString.js": [
                "propertyHelper.js",
                "testTypedArray.js",
            ],
            "built-ins/TypedArray/prototype/toString/BigInt/detached-buffer.js": [
                "testTypedArray.js",
                "detachArrayBuffer.js",
            ],
            "built-ins/TypedArray/prototype/toString/detached-buffer.js": [
                "testTypedArray.js",
                "detachArrayBuffer.js",
            ],
            "built-ins/TypedArray/prototype/toString/not-a-constructor.js": [
                "isConstructor.js",
                "testTypedArray.js",
            ],
        }
        self.assertEqual(TYPED_ARRAY_TO_STRING_FILES, frozenset(expected_features))
        self.assertEqual(TYPED_ARRAY_TO_STRING_FEATURES, expected_features)

        tools_dir = Path(__file__).resolve().parent
        for manifest in tools_dir.glob("test262_*_admission.txt"):
            if manifest.name == "test262_typed_array_to_string_admission.txt":
                continue
            existing = {
                line
                for raw_line in manifest.read_text().splitlines()
                if (line := raw_line.strip()) and not line.startswith("#")
            }
            self.assertTrue(
                TYPED_ARRAY_TO_STRING_FILES.isdisjoint(existing), manifest.name
            )

        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if test_root_available:
            prototype = test_root / "built-ins/TypedArray/prototype"
            actual_files = {
                (prototype / "toString.js").relative_to(test_root).as_posix()
            }
            actual_files.update(
                path.relative_to(test_root).as_posix()
                for path in (prototype / "toString").rglob("*.js")
                if "_FIXTURE" not in path.name
            )
            self.assertEqual(frozenset(actual_files), TYPED_ARRAY_TO_STRING_FILES)
            for relative, features in expected_features.items():
                path = test_root / relative
                self.assertTrue(path.is_file(), relative)
                metadata = test262_runner.parse_meta(path.read_text())
                self.assertEqual(
                    frozenset(metadata.get("features", [])), features, relative
                )
                self.assertEqual(
                    metadata.get("includes", []), expected_includes[relative], relative
                )
                self.assertEqual(metadata.get("flags", []), [], relative)
                self.assertIsNone(metadata.get("negative"), relative)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/TypedArray/prototype/toString/future.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/detached-buffer.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.typed_array_to_string_path(None))
                    self.assertEqual(
                        tool.typed_array_to_string_features(None), frozenset()
                    )
                    for relative, features in expected_features.items():
                        path = root / "test" / relative
                        self.assertTrue(tool.typed_array_to_string_path(path), relative)
                        self.assertEqual(
                            tool.typed_array_to_string_features(path), features, relative
                        )
                        self.assertFalse(
                            tool.should_skip(
                                {"flags": [], "features": sorted(features)}, path
                            ),
                            relative,
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": sorted(features | {"decorators"}),
                                },
                                path,
                            ),
                            relative,
                        )
                    for path in (future, outside):
                        self.assertFalse(tool.typed_array_to_string_path(path))
                        self.assertEqual(
                            tool.typed_array_to_string_features(path), frozenset()
                        )
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["TypedArray"]}, path
                            )
                        )
                finally:
                    tool.TEST262 = original_root

    def test_typed_array_accessor_features_are_admitted_only_on_exact_paths(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": ["BigInt", "DataView", "Symbol", "TypedArray"],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for name, filename in (
                        ("byteLength", "return-bytelength.js"),
                        ("byteOffset", "return-byteoffset.js"),
                        ("length", "return-length.js"),
                    ):
                        inside = root / f"test/built-ins/TypedArray/prototype/{name}/{filename}"
                        unknown = root / f"test/built-ins/TypedArray/prototype/{name}/future.js"
                        self.assertFalse(tool.should_skip(meta, inside))
                        self.assertTrue(tool.should_skip(meta, unknown))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_resizable_feature_is_admitted_only_on_typed_array_builtin_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/out-of-bounds-has.js"
            outside = root / "test/built-ins/Other/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_at_features_are_admitted_only_on_at_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/at/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "TypedArray",
                    "TypedArray.prototype.at",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_fill_features_are_admitted_only_on_fill_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/fill/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "immutable-arraybuffer",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_subarray_features_are_admitted_only_on_subarray_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/subarray/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "Symbol.species",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_set_features_are_admitted_only_on_set_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/set/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "BigInt",
                    "Reflect.construct",
                    "SharedArrayBuffer",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "immutable-arraybuffer",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_values_features_are_admitted_only_on_values_paths(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            values = root / "test/built-ins/TypedArray/prototype/values/case.js"
            iterator = root / "test/built-ins/TypedArray/prototype/Symbol.iterator/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "Symbol.iterator",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, values))
                    self.assertFalse(tool.should_skip(meta, iterator))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_keys_entries_features_are_admitted_only_on_their_paths(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            keys = root / "test/built-ins/TypedArray/prototype/keys/case.js"
            entries = root / "test/built-ins/TypedArray/prototype/entries/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "Symbol.iterator",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, keys))
                    self.assertFalse(tool.should_skip(meta, entries))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_reverse_features_are_admitted_only_on_reverse_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/reverse/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "immutable-arraybuffer",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_to_reversed_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/toReversed/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "Reflect.construct",
                    "Symbol.species",
                    "TypedArray",
                    "change-array-by-copy",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_copy_within_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/copyWithin/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "immutable-arraybuffer",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_copy_within_extended_timeout_is_limited_to_stress_files(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            stress = root / (
                "test/built-ins/TypedArray/prototype/copyWithin/"
                "coerced-values-start-detached.js"
            )
            ordinary = root / "test/built-ins/TypedArray/prototype/copyWithin/reverts.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.typed_array_copy_within_extended_timeout_path(stress))
                    self.assertFalse(tool.typed_array_copy_within_extended_timeout_path(ordinary))
                    self.assertFalse(tool.typed_array_copy_within_extended_timeout_path(outside))
                finally:
                    tool.TEST262 = original_root

    def test_regexp_literal_extended_timeout_is_limited_to_known_slow_files(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            slow = root / "test/language/literals/regexp/S7.8.5_A1.1_T2.js"
            ordinary = root / "test/language/literals/regexp/7.8.5-1.js"
            outside = root / "test/language/expressions/regexp/case.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.regexp_literal_extended_timeout_path(slow))
                    self.assertEqual(tool.test_timeout_seconds(slow), 20)
                    self.assertFalse(tool.regexp_literal_extended_timeout_path(ordinary))
                    self.assertEqual(tool.test_timeout_seconds(ordinary), 8)
                    self.assertFalse(tool.regexp_literal_extended_timeout_path(outside))
                    self.assertEqual(tool.test_timeout_seconds(outside), 8)
                finally:
                    tool.TEST262 = original_root

    def test_annex_b_regexp_escape_timeout_is_limited_to_two_bmp_sweeps(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            relative_paths = {
                "annexB/built-ins/RegExp/RegExp-leading-escape-BMP.js",
                "annexB/built-ins/RegExp/RegExp-trailing-escape-BMP.js",
            }
            ordinary = root / (
                "test/annexB/built-ins/RegExp/RegExp-leading-escape.js"
            )
            outside = root / "test/built-ins/RegExp/RegExp-leading-escape-BMP.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertEqual(
                        tool.REGEXP_ANNEX_B_ESCAPE_EXTENDED_TIMEOUT_FILES,
                        relative_paths,
                    )
                    for relative_path in relative_paths:
                        slow = root / "test" / relative_path
                        self.assertTrue(
                            tool.regexp_annex_b_escape_extended_timeout_path(slow)
                        )
                        self.assertEqual(tool.test_timeout_seconds(slow), 30)
                    self.assertFalse(
                        tool.regexp_annex_b_escape_extended_timeout_path(ordinary)
                    )
                    self.assertEqual(tool.test_timeout_seconds(ordinary), 8)
                    self.assertFalse(
                        tool.regexp_annex_b_escape_extended_timeout_path(outside)
                    )
                    self.assertEqual(tool.test_timeout_seconds(outside), 8)
                finally:
                    tool.TEST262 = original_root

    def test_zero_execution_rate_preserves_skip_and_total(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            test_dir = root / "test" / "unsupported"
            test_dir.mkdir(parents=True)
            (test_dir / "skipped.js").write_text("1;", encoding="utf-8")
            output = io.StringIO()
            with (
                patch.object(test262_runner, "TEST262", str(root)),
                patch.object(test262_runner, "run_test", return_value="skip"),
                patch.object(
                    test262_runner.sys,
                    "argv",
                    ["test262_runner.py", "unsupported"],
                ),
                redirect_stdout(output),
            ):
                test262_runner.main()

            self.assertIn("Results over 1 tests (ran 0):", output.getvalue())
            self.assertIn(
                "RATE=0.0 PASS=0 FAIL=0 SKIP=1 TOTAL=1 RAN=0",
                output.getvalue(),
            )

    def test_character_class_escape_timeout_is_limited_to_generated_complements(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            relative_paths = {
                "built-ins/RegExp/CharacterClassEscapes/character-class-digit-class-escape-negative-cases.js",
                "built-ins/RegExp/CharacterClassEscapes/character-class-non-digit-class-escape-positive-cases.js",
                "built-ins/RegExp/CharacterClassEscapes/character-class-non-whitespace-class-escape-positive-cases.js",
                "built-ins/RegExp/CharacterClassEscapes/character-class-non-word-class-escape-positive-cases.js",
                "built-ins/RegExp/CharacterClassEscapes/character-class-whitespace-class-escape-negative-cases.js",
                "built-ins/RegExp/CharacterClassEscapes/character-class-word-class-escape-negative-cases.js",
            }
            ordinary = root / (
                "test/built-ins/RegExp/CharacterClassEscapes/"
                "character-class-digit-class-escape-positive-cases.js"
            )
            outside = root / (
                "test/built-ins/RegExp/other/"
                "character-class-digit-class-escape-negative-cases.js"
            )
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertEqual(
                        tool.REGEXP_CHARACTER_CLASS_ESCAPE_EXTENDED_TIMEOUT_FILES,
                        relative_paths,
                    )
                    for relative_path in relative_paths:
                        slow = root / "test" / relative_path
                        self.assertTrue(
                            tool.regexp_character_class_escape_extended_timeout_path(slow)
                        )
                        self.assertEqual(tool.test_timeout_seconds(slow), 30)
                    self.assertFalse(
                        tool.regexp_character_class_escape_extended_timeout_path(ordinary)
                    )
                    self.assertEqual(tool.test_timeout_seconds(ordinary), 8)
                    self.assertFalse(
                        tool.regexp_character_class_escape_extended_timeout_path(outside)
                    )
                    self.assertEqual(tool.test_timeout_seconds(outside), 8)
                finally:
                    tool.TEST262 = original_root

    def test_character_class_escape_exhaustive_timeout_is_limited_to_one_file(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            exhaustive = root / (
                "test/built-ins/RegExp/character-class-escape-non-whitespace.js"
            )
            ordinary = root / (
                "test/built-ins/RegExp/character-class-escape-non-whitespace-u180e.js"
            )
            outside = root / (
                "test/built-ins/String/character-class-escape-non-whitespace.js"
            )
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertEqual(
                        tool.REGEXP_CHARACTER_CLASS_ESCAPE_EXHAUSTIVE_TIMEOUT_FILES,
                        {"built-ins/RegExp/character-class-escape-non-whitespace.js"},
                    )
                    self.assertTrue(
                        tool.regexp_character_class_escape_exhaustive_timeout_path(
                            exhaustive
                        )
                    )
                    self.assertEqual(tool.test_timeout_seconds(exhaustive), 60)
                    self.assertFalse(
                        tool.regexp_character_class_escape_exhaustive_timeout_path(ordinary)
                    )
                    self.assertEqual(tool.test_timeout_seconds(ordinary), 8)
                    self.assertFalse(
                        tool.regexp_character_class_escape_exhaustive_timeout_path(outside)
                    )
                    self.assertEqual(tool.test_timeout_seconds(outside), 8)
                finally:
                    tool.TEST262 = original_root

    def test_legacy_failure_analyzer_reuses_runner_timeout_policy(self):
        self.assertIs(analyze_failures.run_test_capture, test262_analyze.run_test)
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            harness = root / "harness"
            harness.mkdir()
            path = root / "slow.js"
            path.write_text("/*---\nflags: [noStrict]\n---*/\n1;\n")
            with (
                patch.object(test262_analyze, "HARNESS", harness),
                patch.object(test262_analyze, "should_skip", return_value=False),
                patch.object(test262_analyze, "test_timeout_seconds", return_value=60),
                patch.object(
                    test262_analyze, "execute_source", return_value=("pass", "")
                ) as execute,
            ):
                status, output = analyze_failures.run_test_capture(path)
        self.assertEqual((status, output), ("pass", ""))
        self.assertEqual(execute.call_args.kwargs["timeout"], 60)

    def test_private_brand_realm_admission_is_exact(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted_names = (
                "private-getter-brand-check-multiple-evaluations-of-class-realm-function-ctor.js",
                "private-getter-brand-check-multiple-evaluations-of-class-realm.js",
                "private-method-brand-check-multiple-evaluations-of-class-realm-function-ctor.js",
                "private-method-brand-check-multiple-evaluations-of-class-realm.js",
                "private-setter-brand-check-multiple-evaluations-of-class-realm-function-ctor.js",
                "private-setter-brand-check-multiple-evaluations-of-class-realm.js",
            )
            static_case = root / (
                "test/language/expressions/class/"
                "private-static-method-brand-check-multiple-evaluations-of-class-realm.js"
            )
            unrelated = root / "test/language/expressions/class/private-method.js"
            meta = {"flags": [], "features": ["class-methods-private", "cross-realm"]}
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for admitted_name in admitted_names:
                        admitted = root / "test/language/expressions/class" / admitted_name
                        self.assertTrue(tool.class_private_brand_realm_path(admitted))
                        self.assertFalse(tool.should_skip(meta, admitted))
                    self.assertFalse(tool.class_private_brand_realm_path(static_case))
                    self.assertTrue(tool.should_skip(meta, static_case))
                    self.assertFalse(tool.class_private_brand_realm_path(unrelated))
                    self.assertTrue(tool.should_skip(meta, unrelated))
                finally:
                    tool.TEST262 = original_root

    def test_computed_public_field_admission_is_exact(self):
        self.assertEqual(len(CLASS_COMPUTED_FIELD_FILES), 120)
        self.assertTrue(all("-fields" in path for path in CLASS_COMPUTED_FIELD_FILES))
        self.assertTrue(all("await-expression" not in path for path in CLASS_COMPUTED_FIELD_FILES))
        prefixes = (
            "language/expressions/class/cpn-class-expr-fields-computed-property-name-from-",
            "language/expressions/class/cpn-class-expr-fields-methods-computed-property-name-from-",
            "language/statements/class/cpn-class-decl-fields-computed-property-name-from-",
            "language/statements/class/cpn-class-decl-fields-methods-computed-property-name-from-",
        )
        suffix_sets = []
        for prefix in prefixes:
            suffixes = {
                path.removeprefix(prefix)
                for path in CLASS_COMPUTED_FIELD_FILES
                if path.startswith(prefix)
            }
            self.assertEqual(len(suffixes), 30)
            suffix_sets.append(suffixes)
        self.assertTrue(all(suffixes == suffix_sets[0] for suffixes in suffix_sets))

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted = root / "test" / next(iter(CLASS_COMPUTED_FIELD_FILES))
            await_case = root / (
                "test/language/expressions/class/"
                "cpn-class-expr-fields-computed-property-name-from-await-expression.js"
            )
            unrelated = root / "test/language/expressions/class/field.js"
            meta = {
                "flags": ["generated"],
                "features": [
                    "computed-property-names",
                    "class-fields-public",
                    "class-static-fields-public",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.class_computed_field_path(admitted))
                    self.assertFalse(tool.should_skip(meta, admitted))
                    self.assertFalse(tool.class_computed_field_path(await_case))
                    self.assertTrue(tool.should_skip(meta, await_case))
                    self.assertFalse(tool.class_computed_field_path(unrelated))
                    self.assertTrue(tool.should_skip(meta, unrelated))
                finally:
                    tool.TEST262 = original_root

    def test_class_default_parameter_admission_is_exact(self):
        names = {
            "async-method/dflt-params-duplicates.js",
            "async-method/dflt-params-rest.js",
            "async-method-static/dflt-params-duplicates.js",
            "async-method-static/dflt-params-rest.js",
            "getter-param-dflt.js",
            "method/dflt-params-abrupt.js",
            "method/dflt-params-arg-val-not-undefined.js",
            "method/dflt-params-arg-val-undefined.js",
            "method/dflt-params-duplicates.js",
            "method/dflt-params-ref-later.js",
            "method/dflt-params-ref-prior.js",
            "method/dflt-params-ref-self.js",
            "method/dflt-params-rest.js",
            "method-length-dflt.js",
            "method-static/dflt-params-abrupt.js",
            "method-static/dflt-params-arg-val-not-undefined.js",
            "method-static/dflt-params-arg-val-undefined.js",
            "method-static/dflt-params-duplicates.js",
            "method-static/dflt-params-ref-later.js",
            "method-static/dflt-params-ref-prior.js",
            "method-static/dflt-params-ref-self.js",
            "method-static/dflt-params-rest.js",
            "params-dflt-meth-args-unmapped.js",
            "params-dflt-meth-ref-arguments.js",
            "params-dflt-meth-static-args-unmapped.js",
            "params-dflt-meth-static-ref-arguments.js",
            "setter-length-dflt.js",
            "static-method-length-dflt.js",
        }
        expected = frozenset(
            f"{prefix}{name}"
            for prefix in (
                "language/expressions/class/",
                "language/statements/class/",
            )
            for name in names
        )
        self.assertEqual(len(CLASS_DEFAULT_PARAMETER_FILES), 56)
        self.assertEqual(CLASS_DEFAULT_PARAMETER_FILES, expected)

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted = root / "test" / next(iter(expected))
            unrelated = root / (
                "test/language/expressions/class/method/"
                "dflt-params-not-admitted.js"
            )
            meta = {"flags": [], "features": ["default-parameters"]}
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.class_default_parameter_path(admitted))
                    self.assertFalse(tool.should_skip(meta, admitted))
                    self.assertFalse(tool.class_default_parameter_path(unrelated))
                    self.assertTrue(tool.should_skip(meta, unrelated))
                finally:
                    tool.TEST262 = original_root

    def test_class_default_parameter_admission_requires_live_metadata_feature(self):
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            checkout_available = test_root.is_dir()
        except OSError:
            checkout_available = False
        if not checkout_available:
            self.skipTest("live Test262 checkout is unavailable")
        for relative in CLASS_DEFAULT_PARAMETER_FILES:
            path = test_root / relative
            self.assertTrue(path.is_file(), relative)
            meta = test262_runner.parse_meta(path.read_text())
            self.assertEqual(meta.get("features"), ["default-parameters"], relative)

    def test_class_default_parameter_runner_analyzer_parity(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            original_runner_root = test262_runner.TEST262
            original_analyze_root = test262_analyze.TEST262
            test262_runner.TEST262 = str(root)
            test262_analyze.TEST262 = str(root)
            try:
                for relative in CLASS_DEFAULT_PARAMETER_FILES:
                    path = root / "test" / relative
                    meta = {"flags": [], "features": ["default-parameters"]}
                    self.assertEqual(
                        test262_runner.class_default_parameter_path(path),
                        test262_analyze.class_default_parameter_path(path),
                    )
                    self.assertEqual(
                        test262_runner.should_skip(meta, path),
                        test262_analyze.should_skip(meta, path),
                    )
                    self.assertFalse(test262_runner.should_skip(meta, path))
            finally:
                test262_runner.TEST262 = original_runner_root
                test262_analyze.TEST262 = original_analyze_root

    def test_decorator_admission_is_exact(self):
        self.assertEqual(len(DECORATOR_FILES), 24)
        self.assertEqual(
            sum(path.startswith("language/expressions/class/") for path in DECORATOR_FILES),
            10,
        )
        self.assertEqual(
            sum(path.startswith("language/statements/class/") for path in DECORATOR_FILES),
            14,
        )
        self.assertEqual(
            sum("/decorator/syntax/" in path for path in DECORATOR_FILES),
            20,
        )
        self.assertEqual(
            sum("field-definition-accessor-no-line-terminator.js" in path for path in DECORATOR_FILES),
            2,
        )
        self.assertEqual(
            sum("grammar-field-accessor.js" in path for path in DECORATOR_FILES),
            2,
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted = root / "test" / next(iter(DECORATOR_FILES))
            unrelated = root / "test/language/statements/class/decorator/not-admitted.js"
            meta = {"flags": ["generated"], "features": ["class", "decorators"]}
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.decorator_path(admitted))
                    self.assertFalse(tool.should_skip(meta, admitted))
                    self.assertFalse(tool.decorator_path(unrelated))
                    self.assertTrue(tool.should_skip(meta, unrelated))
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "flags": ["generated"],
                                "features": ["class", "decorators", "Intl"],
                            },
                            admitted,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

    def test_decorator_admission_requires_live_metadata(self):
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            checkout_available = test_root.is_dir()
        except (OSError, PermissionError):
            checkout_available = False
        if not checkout_available:
            self.skipTest("live Test262 checkout is unavailable")
        for relative in DECORATOR_FILES:
            path = test_root / relative
            try:
                self.assertTrue(path.is_file(), relative)
                meta = test262_runner.parse_meta(path.read_text())
            except (OSError, PermissionError):
                self.skipTest("live Test262 checkout is inaccessible")
            self.assertEqual(set(meta.get("features", [])), {"class", "decorators"}, relative)
            self.assertIn("generated", meta.get("flags", []), relative)

    def test_decorator_admission_runner_analyzer_parity(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            original_runner_root = test262_runner.TEST262
            original_analyze_root = test262_analyze.TEST262
            test262_runner.TEST262 = str(root)
            test262_analyze.TEST262 = str(root)
            try:
                meta = {"flags": ["generated"], "features": ["class", "decorators"]}
                for relative in DECORATOR_FILES:
                    path = root / "test" / relative
                    self.assertEqual(
                        test262_runner.decorator_path(path),
                        test262_analyze.decorator_path(path),
                    )
                    self.assertEqual(
                        test262_runner.should_skip(meta, path),
                        test262_analyze.should_skip(meta, path),
                    )
                    self.assertFalse(test262_runner.should_skip(meta, path))
            finally:
                test262_runner.TEST262 = original_runner_root
                test262_analyze.TEST262 = original_analyze_root

    def test_iterator_core_admission_is_exact(self):
        self.assertEqual(len(ITERATOR_CORE_FILES), 527)
        self.assertEqual(
            sum("/prototype/Symbol.dispose/" in path for path in ITERATOR_CORE_FILES),
            6,
        )
        self.assertEqual(
            sum("/prototype/Symbol.iterator/" in path for path in ITERATOR_CORE_FILES),
            11,
        )
        self.assertEqual(
            sum("/prototype/Symbol.toStringTag/" in path for path in ITERATOR_CORE_FILES),
            2,
        )
        self.assertEqual(
            sum("/prototype/constructor/" in path for path in ITERATOR_CORE_FILES),
            2,
        )
        self.assertEqual(
            sum("/GeneratorPrototype/" in path for path in ITERATOR_CORE_FILES),
            1,
        )
        self.assertEqual(
            sum("/StringIteratorPrototype/" in path for path in ITERATOR_CORE_FILES),
            7,
        )
        self.assertEqual(
            sum("/String/prototype/Symbol.iterator/" in path for path in ITERATOR_CORE_FILES),
            6,
        )
        self.assertEqual(
            sum("/Iterator/from/" in path for path in ITERATOR_CORE_FILES),
            19,
        )
        self.assertEqual(
            sum("/Iterator/prototype/toArray/" in path for path in ITERATOR_CORE_FILES),
            18,
        )
        self.assertEqual(
            sum("/Iterator/prototype/map/" in path for path in ITERATOR_CORE_FILES),
            36,
        )
        self.assertEqual(
            sum("/Iterator/prototype/filter/" in path for path in ITERATOR_CORE_FILES),
            37,
        )
        self.assertEqual(
            sum("/Iterator/prototype/take/" in path for path in ITERATOR_CORE_FILES),
            33,
        )
        self.assertEqual(
            sum("/Iterator/prototype/drop/" in path for path in ITERATOR_CORE_FILES),
            34,
        )
        self.assertEqual(
            sum("/Iterator/prototype/flatMap/" in path for path in ITERATOR_CORE_FILES),
            44,
        )
        self.assertEqual(
            sum("/Iterator/prototype/reduce/" in path for path in ITERATOR_CORE_FILES),
            30,
        )
        self.assertEqual(
            sum("/Iterator/prototype/forEach/" in path for path in ITERATOR_CORE_FILES),
            27,
        )
        self.assertEqual(
            sum("/Iterator/prototype/some/" in path for path in ITERATOR_CORE_FILES),
            33,
        )
        self.assertEqual(
            sum("/Iterator/prototype/every/" in path for path in ITERATOR_CORE_FILES),
            33,
        )
        self.assertEqual(
            sum("/Iterator/prototype/find/" in path for path in ITERATOR_CORE_FILES),
            32,
        )
        self.assertEqual(
            sum("/Iterator/concat/" in path for path in ITERATOR_CORE_FILES),
            32,
        )
        self.assertEqual(
            sum("/Iterator/zip/" in path for path in ITERATOR_CORE_FILES),
            38,
        )
        self.assertEqual(
            sum("/Iterator/zipKeyed/" in path for path in ITERATOR_CORE_FILES),
            44,
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted = root / "test/built-ins/Iterator/constructor.js"
            dispose = root / "test/built-ins/Iterator/prototype/Symbol.dispose/is-function.js"
            symbol_iterator = root / "test/built-ins/Iterator/prototype/Symbol.iterator/is-function.js"
            string_iterator = root / "test/built-ins/StringIteratorPrototype/next/next-iteration.js"
            iterator_from = root / "test/built-ins/Iterator/from/is-function.js"
            to_array = root / "test/built-ins/Iterator/prototype/toArray/is-function.js"
            iterator_map = root / "test/built-ins/Iterator/prototype/map/is-function.js"
            iterator_filter = root / "test/built-ins/Iterator/prototype/filter/is-function.js"
            iterator_take = root / "test/built-ins/Iterator/prototype/take/is-function.js"
            iterator_drop = root / "test/built-ins/Iterator/prototype/drop/is-function.js"
            iterator_flat_map = root / "test/built-ins/Iterator/prototype/flatMap/is-function.js"
            iterator_reduce = root / "test/built-ins/Iterator/prototype/reduce/is-function.js"
            iterator_for_each = root / "test/built-ins/Iterator/prototype/forEach/is-function.js"
            iterator_some = root / "test/built-ins/Iterator/prototype/some/is-function.js"
            iterator_every = root / "test/built-ins/Iterator/prototype/every/is-function.js"
            iterator_find = root / "test/built-ins/Iterator/prototype/find/is-function.js"
            iterator_concat = root / "test/built-ins/Iterator/concat/is-function.js"
            joint = root / "test/built-ins/Iterator/zip/is-function.js"
            joint_keyed = root / "test/built-ins/Iterator/zipKeyed/is-function.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.iterator_core_path(admitted))
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["iterator-helpers"]},
                            admitted,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["explicit-resource-management"]},
                            dispose,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["Symbol.iterator"]},
                            symbol_iterator,
                        )
                    )
                    self.assertTrue(tool.iterator_core_path(string_iterator))
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["Symbol.iterator"]},
                            string_iterator,
                        )
                    )
                    self.assertTrue(tool.iterator_core_path(iterator_from))
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["iterator-helpers"]},
                            iterator_from,
                        )
                    )
                    self.assertTrue(tool.iterator_core_path(to_array))
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["iterator-helpers"]},
                            to_array,
                        )
                    )
                    self.assertTrue(tool.iterator_core_path(iterator_map))
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["iterator-helpers"]},
                            iterator_map,
                        )
                    )
                    self.assertTrue(tool.iterator_core_path(iterator_filter))
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["iterator-helpers"]},
                            iterator_filter,
                        )
                    )
                    self.assertTrue(tool.iterator_core_path(iterator_take))
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["iterator-helpers"]},
                            iterator_take,
                        )
                    )
                    self.assertTrue(tool.iterator_core_path(iterator_drop))
                    self.assertTrue(tool.iterator_core_path(iterator_flat_map))
                    self.assertTrue(tool.iterator_core_path(iterator_reduce))
                    self.assertTrue(tool.iterator_core_path(iterator_for_each))
                    self.assertTrue(tool.iterator_core_path(iterator_some))
                    self.assertTrue(tool.iterator_core_path(iterator_every))
                    self.assertTrue(tool.iterator_core_path(iterator_find))
                    self.assertTrue(tool.iterator_core_path(iterator_concat))
                    self.assertTrue(tool.iterator_core_path(joint))
                    self.assertTrue(tool.iterator_core_path(joint_keyed))
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["iterator-helpers"]},
                            iterator_drop,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["iterator-sequencing"]},
                            iterator_concat,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["joint-iteration"]},
                            joint,
                        )
                    )
                    self.assertFalse(
                        tool.should_skip(
                            {"flags": [], "features": ["joint-iteration"]},
                            joint_keyed,
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "flags": [],
                                "features": ["iterator-helpers", "Intl"],
                            },
                            admitted,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

    def test_iterator_core_admission_requires_live_metadata(self):
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            test_root_available = test_root.is_dir()
        except OSError:
            test_root_available = False
        if not test_root_available:
            self.skipTest("live Test262 checkout is unavailable")
        allowed = set(ITERATOR_CORE_FEATURES) | {"cross-realm", "globalThis"}
        for relative in ITERATOR_CORE_FILES:
            path = test_root / relative
            self.assertTrue(path.is_file(), relative)
            features = set(test262_runner.parse_meta(path.read_text()).get("features", []))
            self.assertTrue(features <= allowed, relative)
            self.assertTrue(features & set(ITERATOR_CORE_FEATURES), relative)

    def test_iterator_core_admission_runner_analyzer_parity(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            original_runner_root = test262_runner.TEST262
            original_analyze_root = test262_analyze.TEST262
            test262_runner.TEST262 = str(root)
            test262_analyze.TEST262 = str(root)
            try:
                for relative in ITERATOR_CORE_FILES:
                    if "/Symbol.dispose/" in relative:
                        features = ["explicit-resource-management"]
                    elif "/Symbol.iterator/" in relative:
                        features = ["Symbol.iterator"]
                    elif relative.endswith("proto-from-ctor-realm.js"):
                        features = ["Reflect", "Symbol", "iterator-helpers"]
                    else:
                        features = ["iterator-helpers"]
                    path = root / "test" / relative
                    meta = {"flags": [], "features": features}
                    self.assertEqual(
                        test262_runner.iterator_core_path(path),
                        test262_analyze.iterator_core_path(path),
                    )
                    self.assertEqual(
                        test262_runner.should_skip(meta, path),
                        test262_analyze.should_skip(meta, path),
                    )
                    self.assertFalse(test262_runner.should_skip(meta, path))
            finally:
                test262_runner.TEST262 = original_runner_root
                test262_analyze.TEST262 = original_analyze_root

    def test_class_destructuring_admission_is_exact(self):
        names = {
            "dstr/meth-ary-name-iter-val.js",
            "dstr/meth-ary-ptrn-elem-ary-elem-init.js",
            "dstr/meth-ary-ptrn-elem-ary-elem-iter.js",
            "dstr/meth-ary-ptrn-elem-ary-empty-iter.js",
            "dstr/meth-ary-ptrn-elem-ary-rest-init.js",
            "dstr/meth-ary-ptrn-elem-ary-rest-iter.js",
            "dstr/meth-ary-ptrn-elem-ary-val-null.js",
            "dstr/meth-ary-ptrn-elem-id-init-exhausted.js",
            "dstr/meth-ary-ptrn-elem-id-init-fn-name-arrow.js",
            "dstr/meth-ary-ptrn-elem-id-init-fn-name-class.js",
            "dstr/meth-ary-ptrn-elem-id-init-fn-name-cover.js",
            "dstr/meth-ary-ptrn-elem-id-init-fn-name-fn.js",
            "dstr/meth-ary-ptrn-elem-id-init-hole.js",
            "dstr/meth-ary-ptrn-elem-id-init-skipped.js",
            "dstr/meth-ary-ptrn-elem-id-init-throws.js",
            "dstr/meth-ary-ptrn-elem-id-init-undef.js",
            "dstr/meth-ary-ptrn-elem-id-init-unresolvable.js",
            "dstr/meth-ary-ptrn-elem-id-iter-complete.js",
            "dstr/meth-ary-ptrn-elem-id-iter-done.js",
            "dstr/meth-ary-ptrn-elem-id-iter-val.js",
            "dstr/meth-ary-ptrn-elem-obj-id-init.js",
            "dstr/meth-ary-ptrn-elem-obj-id.js",
            "dstr/meth-ary-ptrn-elem-obj-prop-id-init.js",
            "dstr/meth-ary-ptrn-elem-obj-prop-id.js",
            "dstr/meth-ary-ptrn-elem-obj-val-null.js",
            "dstr/meth-ary-ptrn-elem-obj-val-undef.js",
            "dstr/meth-ary-ptrn-rest-ary-elem.js",
            "dstr/meth-ary-ptrn-rest-ary-rest.js",
            "dstr/meth-ary-ptrn-rest-id-direct.js",
            "dstr/meth-ary-ptrn-rest-id-elision.js",
            "dstr/meth-ary-ptrn-rest-id.js",
            "dstr/meth-ary-ptrn-rest-init-ary.js",
            "dstr/meth-ary-ptrn-rest-init-id.js",
            "dstr/meth-ary-ptrn-rest-init-obj.js",
            "dstr/meth-ary-ptrn-rest-not-final-ary.js",
            "dstr/meth-ary-ptrn-rest-not-final-id.js",
            "dstr/meth-ary-ptrn-rest-not-final-obj.js",
            "dstr/meth-ary-ptrn-rest-obj-id.js",
            "dstr/meth-ary-ptrn-rest-obj-prop-id.js",
            "dstr/meth-obj-init-null.js",
            "dstr/meth-obj-init-undefined.js",
            "dstr/meth-obj-ptrn-empty.js",
            "dstr/meth-obj-ptrn-id-get-value-err.js",
            "dstr/meth-obj-ptrn-id-init-fn-name-arrow.js",
            "dstr/meth-obj-ptrn-id-init-fn-name-class.js",
            "dstr/meth-obj-ptrn-id-init-fn-name-cover.js",
            "dstr/meth-obj-ptrn-id-init-fn-name-fn.js",
            "dstr/meth-obj-ptrn-id-init-skipped.js",
            "dstr/meth-obj-ptrn-id-init-throws.js",
            "dstr/meth-obj-ptrn-id-init-unresolvable.js",
            "dstr/meth-obj-ptrn-id-trailing-comma.js",
            "dstr/meth-obj-ptrn-list-err.js",
            "dstr/meth-obj-ptrn-prop-ary-init.js",
            "dstr/meth-obj-ptrn-prop-ary-trailing-comma.js",
            "dstr/meth-obj-ptrn-prop-ary-value-null.js",
            "dstr/meth-obj-ptrn-prop-ary.js",
            "dstr/meth-obj-ptrn-prop-eval-err.js",
            "dstr/meth-obj-ptrn-prop-id-get-value-err.js",
            "dstr/meth-obj-ptrn-prop-id-init-skipped.js",
            "dstr/meth-obj-ptrn-prop-id-init-throws.js",
            "dstr/meth-obj-ptrn-prop-id-init-unresolvable.js",
            "dstr/meth-obj-ptrn-prop-id-init.js",
            "dstr/meth-obj-ptrn-prop-id-trailing-comma.js",
            "dstr/meth-obj-ptrn-prop-id.js",
            "dstr/meth-obj-ptrn-prop-obj-init.js",
            "dstr/meth-obj-ptrn-prop-obj-value-null.js",
            "dstr/meth-obj-ptrn-prop-obj-value-undef.js",
            "dstr/meth-obj-ptrn-prop-obj.js",
            "dstr/meth-static-ary-name-iter-val.js",
            "dstr/meth-static-ary-ptrn-elem-ary-elem-init.js",
            "dstr/meth-static-ary-ptrn-elem-ary-elem-iter.js",
            "dstr/meth-static-ary-ptrn-elem-ary-empty-iter.js",
            "dstr/meth-static-ary-ptrn-elem-ary-rest-init.js",
            "dstr/meth-static-ary-ptrn-elem-ary-rest-iter.js",
            "dstr/meth-static-ary-ptrn-elem-ary-val-null.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-exhausted.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-fn-name-arrow.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-fn-name-class.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-fn-name-cover.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-fn-name-fn.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-hole.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-skipped.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-throws.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-undef.js",
            "dstr/meth-static-ary-ptrn-elem-id-init-unresolvable.js",
            "dstr/meth-static-ary-ptrn-elem-id-iter-complete.js",
            "dstr/meth-static-ary-ptrn-elem-id-iter-done.js",
            "dstr/meth-static-ary-ptrn-elem-id-iter-val.js",
            "dstr/meth-static-ary-ptrn-elem-obj-id-init.js",
            "dstr/meth-static-ary-ptrn-elem-obj-id.js",
            "dstr/meth-static-ary-ptrn-elem-obj-prop-id-init.js",
            "dstr/meth-static-ary-ptrn-elem-obj-prop-id.js",
            "dstr/meth-static-ary-ptrn-elem-obj-val-null.js",
            "dstr/meth-static-ary-ptrn-elem-obj-val-undef.js",
            "dstr/meth-static-ary-ptrn-rest-ary-elem.js",
            "dstr/meth-static-ary-ptrn-rest-ary-rest.js",
            "dstr/meth-static-ary-ptrn-rest-id-direct.js",
            "dstr/meth-static-ary-ptrn-rest-id-elision.js",
            "dstr/meth-static-ary-ptrn-rest-id.js",
            "dstr/meth-static-ary-ptrn-rest-init-ary.js",
            "dstr/meth-static-ary-ptrn-rest-init-id.js",
            "dstr/meth-static-ary-ptrn-rest-init-obj.js",
            "dstr/meth-static-ary-ptrn-rest-not-final-ary.js",
            "dstr/meth-static-ary-ptrn-rest-not-final-id.js",
            "dstr/meth-static-ary-ptrn-rest-not-final-obj.js",
            "dstr/meth-static-ary-ptrn-rest-obj-id.js",
            "dstr/meth-static-ary-ptrn-rest-obj-prop-id.js",
            "dstr/meth-static-obj-init-null.js",
            "dstr/meth-static-obj-init-undefined.js",
            "dstr/meth-static-obj-ptrn-empty.js",
            "dstr/meth-static-obj-ptrn-id-get-value-err.js",
            "dstr/meth-static-obj-ptrn-id-init-fn-name-arrow.js",
            "dstr/meth-static-obj-ptrn-id-init-fn-name-class.js",
            "dstr/meth-static-obj-ptrn-id-init-fn-name-cover.js",
            "dstr/meth-static-obj-ptrn-id-init-fn-name-fn.js",
            "dstr/meth-static-obj-ptrn-id-init-skipped.js",
            "dstr/meth-static-obj-ptrn-id-init-throws.js",
            "dstr/meth-static-obj-ptrn-id-init-unresolvable.js",
            "dstr/meth-static-obj-ptrn-id-trailing-comma.js",
            "dstr/meth-static-obj-ptrn-list-err.js",
            "dstr/meth-static-obj-ptrn-prop-ary-init.js",
            "dstr/meth-static-obj-ptrn-prop-ary-trailing-comma.js",
            "dstr/meth-static-obj-ptrn-prop-ary-value-null.js",
            "dstr/meth-static-obj-ptrn-prop-ary.js",
            "dstr/meth-static-obj-ptrn-prop-eval-err.js",
            "dstr/meth-static-obj-ptrn-prop-id-get-value-err.js",
            "dstr/meth-static-obj-ptrn-prop-id-init-skipped.js",
            "dstr/meth-static-obj-ptrn-prop-id-init-throws.js",
            "dstr/meth-static-obj-ptrn-prop-id-init-unresolvable.js",
            "dstr/meth-static-obj-ptrn-prop-id-init.js",
            "dstr/meth-static-obj-ptrn-prop-id-trailing-comma.js",
            "dstr/meth-static-obj-ptrn-prop-id.js",
            "dstr/meth-static-obj-ptrn-prop-obj-init.js",
            "dstr/meth-static-obj-ptrn-prop-obj-value-null.js",
            "dstr/meth-static-obj-ptrn-prop-obj-value-undef.js",
            "dstr/meth-static-obj-ptrn-prop-obj.js",
        }
        expected = frozenset(
            f"{prefix}{name}"
            for prefix in (
                "language/expressions/class/",
                "language/statements/class/",
            )
            for name in names
        )
        self.assertEqual(len(names), 136)
        self.assertEqual(len(CLASS_DESTRUCTURING_FILES), 272)
        self.assertEqual(CLASS_DESTRUCTURING_FILES, expected)
        self.assertTrue(
            all(
                path.startswith((
                    "language/expressions/class/dstr/",
                    "language/statements/class/dstr/",
                )) and path.endswith(".js")
                for path in CLASS_DESTRUCTURING_FILES
            )
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted = root / "test" / next(iter(expected))
            unrelated = root / (
                "test/language/expressions/class/dstr/not-admitted.js"
            )
            meta = {"flags": ["generated"], "features": ["destructuring-binding"]}
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertTrue(tool.class_destructuring_path(admitted))
                    self.assertFalse(tool.should_skip(meta, admitted))
                    self.assertFalse(tool.class_destructuring_path(unrelated))
                    self.assertTrue(tool.should_skip(meta, unrelated))
                    self.assertTrue(
                        tool.should_skip(
                            {
                                "flags": ["generated"],
                                "features": [
                                    "destructuring-binding",
                                    "class-fields-private",
                                ],
                            },
                            admitted,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

    def test_class_destructuring_admission_requires_live_metadata_feature(self):
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            checkout_available = test_root.is_dir()
        except (OSError, PermissionError):
            checkout_available = False
        if not checkout_available:
            self.skipTest("live Test262 checkout is unavailable")
        for relative in CLASS_DESTRUCTURING_FILES:
            path = test_root / relative
            try:
                self.assertTrue(path.is_file(), relative)
                meta = test262_runner.parse_meta(path.read_text())
            except (OSError, PermissionError):
                self.skipTest("live Test262 checkout is inaccessible")
            self.assertEqual(meta.get("features"), ["destructuring-binding"], relative)

    def test_class_destructuring_runner_analyzer_parity(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            original_runner_root = test262_runner.TEST262
            original_analyze_root = test262_analyze.TEST262
            test262_runner.TEST262 = str(root)
            test262_analyze.TEST262 = str(root)
            try:
                for relative in CLASS_DESTRUCTURING_FILES:
                    path = root / "test" / relative
                    meta = {
                        "flags": ["generated"],
                        "features": ["destructuring-binding"],
                    }
                    self.assertEqual(
                        test262_runner.class_destructuring_path(path),
                        test262_analyze.class_destructuring_path(path),
                    )
                    self.assertEqual(
                        test262_runner.should_skip(meta, path),
                        test262_analyze.should_skip(meta, path),
                    )
                    self.assertFalse(test262_runner.should_skip(meta, path))
            finally:
                test262_runner.TEST262 = original_runner_root
                test262_analyze.TEST262 = original_analyze_root

    def test_residual_public_field_admission_is_exact(self):
        expected = {
            "language/expressions/class/constructor-this-tdz-during-initializers.js": [
                "class-fields-public"
            ],
            "language/statements/class/classelementname-abrupt-completion.js": [
                "class",
                "class-fields-public",
            ],
            "language/statements/class/static-classelementname-abrupt-completion.js": [
                "class-static-fields-public"
            ],
            "language/statements/class/static-init-abrupt.js": [
                "class-static-fields-public",
                "class-static-block",
            ],
            "language/statements/class/static-init-sequence.js": [
                "class-static-fields-public",
                "class-static-block",
            ],
        }
        self.assertEqual(CLASS_PUBLIC_FIELD_FILES, frozenset(expected))

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            unrelated = root / "test/language/statements/class/unsupported-field.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        admitted = root / "test" / relative
                        meta = {"flags": [], "features": features}
                        self.assertTrue(tool.class_public_field_path(admitted))
                        self.assertFalse(tool.should_skip(meta, admitted))
                    self.assertFalse(tool.class_public_field_path(unrelated))
                    self.assertTrue(
                        tool.should_skip(
                            {"flags": [], "features": ["class-fields-public"]},
                            unrelated,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

    def test_private_class_boundary_admission_is_exact(self):
        expected = {
            "language/expressions/class/private-getter-brand-check-multiple-evaluations-of-class-eval-indirect.js": {"class-methods-private"},
            "language/expressions/class/private-getter-brand-check-multiple-evaluations-of-class-eval.js": {"class-methods-private"},
            "language/expressions/class/private-getter-brand-check-multiple-evaluations-of-class-factory.js": {"class-methods-private"},
            "language/expressions/class/private-getter-brand-check-multiple-evaluations-of-class-function-ctor.js": {"class-methods-private"},
            "language/expressions/class/private-method-brand-check-multiple-evaluations-of-class-eval-indirect.js": {"class-methods-private"},
            "language/expressions/class/private-method-brand-check-multiple-evaluations-of-class-eval.js": {"class-methods-private"},
            "language/expressions/class/private-method-brand-check-multiple-evaluations-of-class-factory.js": {"class-methods-private"},
            "language/expressions/class/private-method-brand-check-multiple-evaluations-of-class-function-ctor.js": {"class-methods-private"},
            "language/expressions/class/private-setter-brand-check-multiple-evaluations-of-class-eval-indirect.js": {"class-methods-private"},
            "language/expressions/class/private-setter-brand-check-multiple-evaluations-of-class-eval.js": {"class-methods-private"},
            "language/expressions/class/private-setter-brand-check-multiple-evaluations-of-class-factory.js": {"class-methods-private"},
            "language/expressions/class/private-setter-brand-check-multiple-evaluations-of-class-function-ctor.js": {"class-methods-private"},
            "language/expressions/class/private-static-field-multiple-evaluations-of-class-direct-eval.js": {"class-static-fields-private"},
            "language/expressions/class/private-static-field-multiple-evaluations-of-class-eval-indirect.js": {"class-static-fields-private"},
            "language/expressions/class/private-static-field-multiple-evaluations-of-class-factory.js": {"class-static-fields-private"},
            "language/expressions/class/private-static-field-multiple-evaluations-of-class-function-ctor.js": {"class-static-fields-private"},
            "language/expressions/class/private-static-field-multiple-evaluations-of-class-realm.js": {"class-static-fields-private"},
            "language/expressions/class/private-static-getter-multiple-evaluations-of-class-direct-eval.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-getter-multiple-evaluations-of-class-eval-indirect.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-getter-multiple-evaluations-of-class-factory.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-getter-multiple-evaluations-of-class-function-ctor.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-getter-multiple-evaluations-of-class-realm.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-method-brand-check-multiple-evaluations-of-class-direct-eval.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-method-brand-check-multiple-evaluations-of-class-eval-indirect.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-method-brand-check-multiple-evaluations-of-class-factory.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-method-brand-check-multiple-evaluations-of-class-function-ctor.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-method-brand-check-multiple-evaluations-of-class-realm.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-setter-multiple-evaluations-of-class-direct-eval.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-setter-multiple-evaluations-of-class-eval-indirect.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-setter-multiple-evaluations-of-class-factory.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-setter-multiple-evaluations-of-class-function-ctor.js": {"class-static-methods-private"},
            "language/expressions/class/private-static-setter-multiple-evaluations-of-class-realm.js": {"class-static-methods-private"},
            "language/statements/class/private-non-static-getter-static-setter-early-error.js": {"class-methods-private", "class-static-methods-private"},
            "language/statements/class/private-non-static-setter-static-getter-early-error.js": {"class-methods-private", "class-static-methods-private"},
            "language/statements/class/private-static-getter-non-static-setter-early-error.js": {"class-methods-private", "class-static-methods-private"},
            "language/statements/class/private-static-setter-non-static-getter-early-error.js": {"class-methods-private", "class-static-methods-private"},
            "language/statements/class/static-init-scope-private.js": {"class-fields-private"},
        }
        expected = {path: frozenset(features) for path, features in expected.items()}
        self.assertEqual(len(CLASS_PRIVATE_FILES), 37)
        self.assertEqual(CLASS_PRIVATE_FEATURES_BY_FILE, expected)
        self.assertEqual(CLASS_PRIVATE_FILES, frozenset(expected))
        self.assertTrue(
            all(
                path.startswith((
                    "language/expressions/class/",
                    "language/statements/class/",
                ))
                and path.endswith(".js")
                and "/elements/" not in path
                for path in CLASS_PRIVATE_FILES
            )
        )
        self.assertTrue(
            all(features <= PRIVATE_CLASS_FEATURES for features in expected.values())
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            unrelated = root / "test/language/expressions/class/private-method.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, private_features in expected.items():
                        admitted = root / "test" / relative
                        meta = {"flags": [], "features": ["class", *private_features]}
                        self.assertEqual(
                            tool.class_private_path_features(admitted),
                            private_features,
                        )
                        self.assertFalse(tool.should_skip(meta, admitted))
                        excluded = next(iter(PRIVATE_CLASS_FEATURES - private_features))
                        self.assertTrue(
                            tool.should_skip(
                                {"flags": [], "features": ["class", *private_features, excluded]},
                                admitted,
                            )
                        )
                    self.assertEqual(tool.class_private_path_features(unrelated), frozenset())
                    self.assertTrue(
                        tool.should_skip(
                            {"flags": [], "features": ["class", *PRIVATE_CLASS_FEATURES]},
                            unrelated,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

    def test_private_class_boundary_runner_analyzer_parity(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            original_runner_root = test262_runner.TEST262
            original_analyze_root = test262_analyze.TEST262
            test262_runner.TEST262 = str(root)
            test262_analyze.TEST262 = str(root)
            try:
                for relative, private_features in CLASS_PRIVATE_FEATURES_BY_FILE.items():
                    path = root / "test" / relative
                    meta = {"flags": [], "features": ["class", *private_features]}
                    self.assertEqual(
                        test262_runner.should_skip(meta, path),
                        test262_analyze.should_skip(meta, path),
                    )
            finally:
                test262_runner.TEST262 = original_runner_root
                test262_analyze.TEST262 = original_analyze_root

    def test_slice_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/slice/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
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
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_find_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/find/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_find_index_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/findIndex/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_find_last_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/findLast/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_find_last_index_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/findLastIndex/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_some_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/some/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Reflect.set",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_every_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/every/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Reflect.set",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_for_each_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/forEach/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Reflect.set",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_includes_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/includes/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "align-detached-buffer-semantics-with-web-reality",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_reduce_right_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/reduceRight/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Reflect.set",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_reduce_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/reduce/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "BigInt",
                    "Reflect.construct",
                    "Reflect.set",
                    "Symbol",
                    "TypedArray",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_map_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/map/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
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
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_filter_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/filter/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
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
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_index_of_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/indexOf/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "Array.prototype.includes",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "align-detached-buffer-semantics-with-web-reality",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_last_index_of_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/lastIndexOf/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "Array.prototype.includes",
                    "BigInt",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "align-detached-buffer-semantics-with-web-reality",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_with_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/with/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "BigInt",
                    "Reflect.construct",
                    "Symbol.species",
                    "TypedArray",
                    "change-array-by-copy",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_to_string_tag_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/Symbol.toStringTag/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": [
                    "BigInt",
                    "DataView",
                    "Symbol",
                    "Symbol.toStringTag",
                    "TypedArray",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_buffer_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/buffer/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/case.js"
            meta = {
                "flags": [],
                "features": ["BigInt", "DataView", "Symbol", "TypedArray"],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_sort_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/sort/case.js"
            outside = root / "test/built-ins/TypedArray/prototype/toSorted/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "Array.prototype.includes",
                    "Reflect.construct",
                    "Symbol",
                    "TypedArray",
                    "immutable-arraybuffer",
                    "stable-typedarray-sort",
                    "arrow-function",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root

    def test_to_sorted_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/toSorted/case.js"
            outside = root / "test/built-ins/Array/prototype/toSorted/case.js"
            meta = {
                "flags": [],
                "features": [
                    "Reflect.construct",
                    "Symbol.species",
                    "TypedArray",
                    "change-array-by-copy",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root


class TypedArrayToStringAdmissionTests(unittest.TestCase):
    def test_manifest_is_exact_live_disjoint_and_shared(self):
        TypedArrayResizableAdmissionTests._assert_typed_array_to_string_features_are_frozen_to_audited_files(
            self
        )


class ArrayBufferAdmissionTests(unittest.TestCase):
    def test_resizable_array_buffer_feature_is_admitted_only_inside_builtin_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/ArrayBuffer/prototype/resize/case.js"
            outside = root / "test/built-ins/Other/case.js"
            meta = {
                "flags": [],
                "features": [
                    "ArrayBuffer",
                    "DataView",
                    "Int8Array",
                    "Reflect.construct",
                    "SharedArrayBuffer",
                    "Symbol",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root


class SharedArrayBufferAdmissionTests(unittest.TestCase):
    def test_shared_array_buffer_features_are_admitted_only_inside_builtin_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/SharedArrayBuffer/prototype/slice/case.js"
            outside = root / "test/built-ins/Other/case.js"
            meta = {
                "flags": [],
                "features": [
                    "SharedArrayBuffer",
                    "ArrayBuffer",
                    "DataView",
                    "TypedArray",
                    "Int8Array",
                    "Reflect",
                    "Reflect.construct",
                    "Symbol",
                    "Symbol.species",
                    "Symbol.toStringTag",
                    "resizable-arraybuffer",
                ],
            }
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root


class AtomicsSyncAdmissionTests(unittest.TestCase):
    def test_supported_atomics_paths_exclude_async_wait_and_pause(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            operation = root / "test/built-ins/Atomics/compareExchange/case.js"
            surface = root / "test/built-ins/Atomics/Symbol.toStringTag.js"
            wait = root / "test/built-ins/Atomics/wait/case.js"
            notify = root / "test/built-ins/Atomics/notify/case.js"
            wait_async = root / "test/built-ins/Atomics/waitAsync/case.js"
            pause = root / "test/built-ins/Atomics/pause/case.js"
            outside = root / "test/built-ins/Other/case.js"
            meta = {
                "flags": [],
                "features": [
                    "Atomics",
                    "ArrayBuffer",
                    "SharedArrayBuffer",
                    "TypedArray",
                    "BigInt",
                    "Symbol",
                    "Symbol.toStringTag",
                    "resizable-arraybuffer",
                ],
            }
            wait_async_meta = {
                "flags": ["async"],
                "features": meta["features"]
                + ["Atomics.waitAsync", "async-functions", "destructuring-binding"],
            }

            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, operation))
                    self.assertFalse(tool.should_skip(meta, surface))
                    self.assertFalse(tool.should_skip(meta, wait))
                    self.assertFalse(tool.should_skip(meta, notify))
                    self.assertFalse(tool.should_skip(wait_async_meta, wait_async))
                    pause_meta = {
                        "flags": [],
                        "features": ["Atomics.pause", "Reflect.construct"],
                    }
                    self.assertFalse(tool.should_skip(pause_meta, pause))
                    self.assertTrue(tool.should_skip(meta, outside))
                finally:
                    tool.TEST262 = original_root


class ClassSubclassBuiltinAdmissionTests(unittest.TestCase):
    def test_residual_subclass_builtin_admission_is_exact(self):
        expected = {
            f"language/{goal}/class/subclass-builtins/subclass-{name}.js": frozenset(
                {name}
            )
            for goal in ("expressions", "statements")
            for name in ("SharedArrayBuffer", "WeakRef")
        }
        self.assertEqual(CLASS_SUBCLASS_BUILTIN_FEATURES_BY_FILE, expected)
        self.assertEqual(CLASS_SUBCLASS_BUILTIN_FILES, frozenset(expected))

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / (
                "test/language/expressions/class/subclass-builtins/"
                "subclass-SharedArrayBuffer-future.js"
            )
            outside = root / "test/language/expressions/class/subclass-SharedArrayBuffer.js"
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative, features in expected.items():
                        path = root / "test" / relative
                        meta = {"flags": [], "features": sorted(features)}
                        self.assertEqual(
                            tool.class_subclass_builtin_features(path), features
                        )
                        self.assertFalse(tool.should_skip(meta, path))
                        self.assertTrue(
                            tool.should_skip(
                                {
                                    "flags": [],
                                    "features": [*sorted(features), "decorators"],
                                },
                                path,
                            )
                        )
                    self.assertEqual(
                        tool.class_subclass_builtin_features(future), frozenset()
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"flags": [], "features": ["SharedArrayBuffer"]},
                            future,
                        )
                    )
                    self.assertTrue(
                        tool.should_skip(
                            {"flags": [], "features": ["SharedArrayBuffer"]},
                            outside,
                        )
                    )
                finally:
                    tool.TEST262 = original_root

    def test_residual_subclass_builtin_live_metadata(self):
        test_root = Path(test262_runner.TEST262) / "test"
        try:
            checkout_available = test_root.is_dir()
        except (OSError, PermissionError):
            checkout_available = False
        if not checkout_available:
            self.skipTest("live Test262 checkout is unavailable")
        for relative, expected_features in (
            CLASS_SUBCLASS_BUILTIN_FEATURES_BY_FILE.items()
        ):
            path = test_root / relative
            try:
                if not path.is_file():
                    self.skipTest("live Test262 checkout is incomplete")
                meta = test262_runner.parse_meta(path.read_text())
            except (OSError, PermissionError):
                self.skipTest("live Test262 checkout is inaccessible")
            self.assertEqual(
                frozenset(meta.get("features", [])), expected_features, relative
            )
            self.assertEqual(meta.get("flags", []), ["generated"], relative)
            self.assertFalse(test262_runner.should_skip(meta, path), relative)
            self.assertFalse(test262_analyze.should_skip(meta, path), relative)


class FinalizationRegistryAdmissionTests(unittest.TestCase):
    def test_finalization_registry_support_and_exact_path_exceptions(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = (
                root
                / "test/built-ins/FinalizationRegistry/prototype/register/custom-this.js"
            )
            weak_ref_brand = (
                root
                / "test/built-ins/WeakRef/prototype/deref/this-does-not-have-internal-target-throws.js"
            )
            future = (
                root
                / "test/built-ins/FinalizationRegistry/prototype/register/future.js"
            )
            outside = root / "test/built-ins/Other/case.js"
            feature_only = {"flags": [], "features": ["FinalizationRegistry"]}
            combined = {
                "flags": [],
                "features": [
                    "FinalizationRegistry",
                    "Reflect",
                    "Reflect.construct",
                    "Symbol",
                    "Symbol.toStringTag",
                    "WeakMap",
                    "WeakRef",
                    "WeakSet",
                ],
            }

            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(feature_only, outside))
                    self.assertFalse(tool.should_skip(combined, inside))
                    self.assertFalse(tool.should_skip(combined, weak_ref_brand))
                    self.assertTrue(tool.should_skip(combined, future))
                    self.assertTrue(tool.should_skip(combined, outside))
                finally:
                    tool.TEST262 = original_root


if __name__ == "__main__":
    unittest.main()
