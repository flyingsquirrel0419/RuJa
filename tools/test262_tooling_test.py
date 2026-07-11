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
    def test_resizable_feature_is_admitted_only_on_typed_array_builtin_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            inside = root / "test/built-ins/TypedArray/prototype/byteLength/case.js"
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
            outside = root / "test/built-ins/TypedArray/prototype/map/case.js"
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
            outside = root / "test/built-ins/TypedArray/prototype/map/case.js"
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
            outside = root / "test/built-ins/TypedArray/prototype/map/case.js"
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
            outside = root / "test/built-ins/TypedArray/prototype/map/case.js"
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
            outside = root / "test/built-ins/TypedArray/prototype/map/case.js"
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
            outside = root / "test/built-ins/TypedArray/prototype/map/case.js"
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
