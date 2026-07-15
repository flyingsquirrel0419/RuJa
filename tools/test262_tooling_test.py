#!/usr/bin/env python3
"""Regression tests for RuJa's shared test262 process support."""

import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import test262_analyze
import test262_runner
from test262_class_computed_field_admission import CLASS_COMPUTED_FIELD_FILES
from test262_class_default_parameter_admission import CLASS_DEFAULT_PARAMETER_FILES
from test262_class_destructuring_admission import CLASS_DESTRUCTURING_FILES
from test262_decorator_admission import DECORATOR_FILES
from test262_class_private_admission import (
    CLASS_PRIVATE_FEATURES_BY_FILE,
    CLASS_PRIVATE_FILES,
    PRIVATE_CLASS_FEATURES,
)
from test262_class_public_field_admission import CLASS_PUBLIC_FIELD_FILES
from test262_date_to_primitive_admission import DATE_TO_PRIMITIVE_FILES
from test262_proxy_get_admission import PROXY_GET_FEATURES, PROXY_GET_FILES
from test262_reference_primitive_admission import REFERENCE_PRIMITIVE_FILES
from test262_dynamic_import_admission import DYNAMIC_IMPORT_FILES
from test262_import_meta_admission import IMPORT_META_FILES
from test262_iterator_admission import ITERATOR_CORE_FEATURES, ITERATOR_CORE_FILES
from test262_json_parse_admission import JSON_PARSE_FILES
from test262_json_raw_admission import JSON_RAW_FILES
from test262_json_stringify_admission import JSON_STRINGIFY_FILES
from test262_module_admission import (
    MODULE_STATIC_SEMANTICS_FILES,
    MODULE_TLA_RUNTIME_FILES,
    MODULE_TLA_SYNTAX_FILES,
)
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


class ModuleCoreAdmissionTests(unittest.TestCase):
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

    def test_reference_primitive_manifest_is_exact_and_shared(self):
        self.assertEqual(len(REFERENCE_PRIMITIVE_FILES), 3)
        admitted = "language/types/reference/put-value-prop-base-primitive-realm.js"
        outside = "built-ins/Array/isArray/proxy.js"
        self.assertIn(admitted, REFERENCE_PRIMITIVE_FILES)
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            admitted_path = root / "test" / admitted
            outside_path = root / "test" / outside
            for tool in (test262_runner, test262_analyze):
                original_root = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    self.assertFalse(tool.should_skip(
                        {"features": ["cross-realm", "Symbol", "Proxy"]},
                        admitted_path,
                    ))
                    self.assertTrue(tool.should_skip(
                        {"features": ["Proxy"]}, outside_path
                    ))
                    self.assertTrue(tool.reference_primitive_path(admitted_path))
                finally:
                    tool.TEST262 = original_root

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
        self.assertEqual(len(ITERATOR_CORE_FILES), 483)
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
                    self.assertTrue(
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
