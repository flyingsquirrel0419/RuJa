#!/usr/bin/env python3
"""Regression tests for RuJa's shared test262 process support."""

import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import test262_analyze
import test262_runner
from test262_support import ASYNC_COMPLETE, ASYNC_PRINT_SHIM, execute_source


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


class HarnessAssemblyTests(unittest.TestCase):
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
                self.assertTrue(source.startswith("'use strict';"))
                positions = [
                    source.index("/* STA HARNESS */"),
                    source.index("/* ASSERT HARNESS */"),
                    source.index(ASYNC_PRINT_SHIM),
                    source.index("/* DONE HARNESS */"),
                    source.index("$DONE();"),
                ]
                self.assertEqual(positions, sorted(positions))


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
    def test_weak_ref_features_are_admitted_only_inside_builtin_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/WeakRef/prototype/deref/case.js"
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
                    self.assertTrue(tool.should_skip(meta, outside))
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

    def test_typed_array_to_string_features_are_frozen_to_audited_files(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/toString.js"
            detached = root / "test/built-ins/TypedArray/prototype/toString/detached-buffer.js"
            future = root / "test/built-ins/TypedArray/prototype/toString/future.js"
            outside = root / "test/built-ins/TypedArray/prototype/unsupported/detached-buffer.js"
            meta = {"flags": [], "features": ["TypedArray"]}
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(meta, inside))
                    self.assertFalse(tool.should_skip(meta, detached))
                    self.assertTrue(tool.should_skip(meta, future))
                    self.assertTrue(tool.should_skip(meta, outside))
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

    def test_join_features_are_admitted_only_on_join_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/join/case.js"
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

    def test_to_locale_string_features_are_admitted_only_on_its_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/toLocaleString/case.js"
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


class FinalizationRegistryAdmissionTests(unittest.TestCase):
    def test_finalization_registry_support_and_exact_path_exceptions(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = (
                root
                / "test/built-ins/FinalizationRegistry/prototype/register/case.js"
            )
            weak_ref_brand = (
                root / "test/built-ins/WeakRef/prototype/deref/brand.js"
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
                    self.assertTrue(tool.should_skip(combined, outside))
                finally:
                    tool.TEST262 = original_root


if __name__ == "__main__":
    unittest.main()
