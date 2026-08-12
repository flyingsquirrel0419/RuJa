#!/usr/bin/env python3
"""Fail-closed tests for Duration add/compare Test262 ownership."""

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import test262_analyze
import test262_runner
import test262_temporal_duration_math_admission as admission
import test262_temporal_duration_math_diagnostic as diagnostic
import test262_temporal_plain_date_time_difference_admission as difference


class DurationMathToolingTests(unittest.TestCase):
    def test_manifests_are_exact_disjoint_and_shared(self):
        surfaces = (
            admission.TEMPORAL_DURATION_MATH_FILES,
            admission.TEMPORAL_DURATION_MATH_INTL_BLOCKERS,
            admission.TEMPORAL_DURATION_MATH_DOWNSTREAM_ADMISSION,
            admission.TEMPORAL_DURATION_MATH_DOWNSTREAM_BLOCKERS,
        )
        self.assertEqual(tuple(map(len, surfaces)), (84, 3, 2, 5))
        self.assertEqual(len(admission.TEMPORAL_DURATION_MATH_FALSE_POSITIVES), 4)
        for index, left in enumerate(surfaces):
            for right in surfaces[index + 1 :]:
                self.assertTrue(left.isdisjoint(right))
        self.assertEqual(len(admission.TEMPORAL_DURATION_MATH_ADMITTED), 86)
        self.assertEqual(len(admission.TEMPORAL_DURATION_MATH_COMPLETE), 94)
        self.assertEqual(
            admission.TEMPORAL_DURATION_MATH_FALSE_POSITIVES
            & admission.TEMPORAL_DURATION_MATH_ADMITTED,
            admission.TEMPORAL_DURATION_MATH_FALSE_POSITIVES,
        )
        self.assertEqual(
            admission.TEMPORAL_DURATION_MATH_DOWNSTREAM_ADMISSION,
            difference.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_TRANSITIONS,
        )
        self.assertEqual(
            set(admission.TEMPORAL_DURATION_MATH_FEATURES),
            admission.TEMPORAL_DURATION_MATH_ADMITTED,
        )

    def test_live_metadata_and_directory_identity_are_exact_when_available(self):
        test_root = Path(test262_runner.TEST262) / "test"
        corpus_required = "TEST262" in os.environ
        roots = (
            test_root / "built-ins/Temporal/Duration/compare",
            test_root / "built-ins/Temporal/Duration/prototype/add",
        )
        try:
            if not all(root.is_dir() for root in roots):
                if corpus_required:
                    raise FileNotFoundError(roots)
                return
            live = {
                path.relative_to(test_root).as_posix()
                for root in roots
                for path in root.glob("*.js")
                if "_FIXTURE" not in path.name
            }
        except OSError:
            if corpus_required:
                raise
            return
        self.assertEqual(live, admission.TEMPORAL_DURATION_MATH_FILES)
        self.assertEqual(
            len(admission.audit_metadata(test262_runner.TEST262, test262_runner.parse_meta)),
            94,
        )
        for relative in admission.TEMPORAL_DURATION_MATH_ADMITTED:
            path = test_root / relative
            metadata = test262_runner.parse_meta(path.read_text())
            self.assertEqual(
                frozenset(metadata.get("features", [])),
                admission.TEMPORAL_DURATION_MATH_FEATURES[relative],
                relative,
            )
            for tool in (test262_runner, test262_analyze):
                self.assertTrue(tool.temporal_duration_math_path(path), relative)
                self.assertEqual(
                    tool.temporal_duration_math_features(path),
                    admission.TEMPORAL_DURATION_MATH_FEATURES[relative],
                )
                self.assertFalse(tool.should_skip(metadata, path), relative)

    def test_runner_wiring_rejects_future_outside_and_inaccessible_paths(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            future = root / "test/built-ins/Temporal/Duration/compare/future.js"
            outside = root / "test/built-ins/Temporal/Duration/add/basic.js"
            malformed = future.with_name("basic.js.bak")
            for tool in (test262_runner, test262_analyze):
                original = tool.TEST262
                tool.TEST262 = str(root)
                try:
                    for relative in admission.TEMPORAL_DURATION_MATH_ADMITTED:
                        path = root / "test" / relative
                        self.assertTrue(tool.temporal_duration_math_path(path), relative)
                        self.assertEqual(
                            tool.temporal_duration_math_features(path),
                            admission.TEMPORAL_DURATION_MATH_FEATURES[relative],
                        )
                    for path in (future, outside, malformed, None, object()):
                        self.assertFalse(tool.temporal_duration_math_path(path))
                        self.assertEqual(
                            tool.temporal_duration_math_features(path), frozenset()
                        )
                    for path in (future, outside, malformed):
                        self.assertTrue(tool.should_skip({"features": ["Temporal"]}, path))
                    with patch("pathlib.Path.resolve", side_effect=PermissionError):
                        self.assertFalse(tool.temporal_duration_math_path(future))
                        self.assertEqual(
                            tool.temporal_duration_math_features(future), frozenset()
                        )
                finally:
                    tool.TEST262 = original

    def test_diagnostic_requires_exact_arguments_results_and_errors(self):
        arguments = sorted(admission.TEMPORAL_DURATION_MATH_COMPLETE)

        def expected(path):
            relative = path.name if isinstance(path, str) else path.as_posix()
            relative = next(
                item for item in admission.TEMPORAL_DURATION_MATH_COMPLETE
                if relative.endswith(item)
            )
            if relative in admission.TEMPORAL_DURATION_MATH_ADMITTED:
                return "pass", ("", "")
            if relative in admission.TEMPORAL_DURATION_MATH_INTL_BLOCKERS:
                error = diagnostic._INTL_ERRORS[relative]
            else:
                error = "TypeError: undefined is not a function"
            return "fail", (error, error)

        with (
            patch.object(diagnostic, "_relative", side_effect=lambda path: Path(path).as_posix().split("/test/")[-1]),
            patch.object(diagnostic, "_run", side_effect=expected),
            patch.object(diagnostic, "audit_metadata"),
            patch.object(diagnostic.test262_runner, "execute_source", return_value=("pass", "")),
        ):
            diagnostic.verify(arguments)
            with self.assertRaisesRegex(RuntimeError, "exact 94-file surface"):
                diagnostic.verify(arguments[:-1])

            target = min(admission.TEMPORAL_DURATION_MATH_INTL_BLOCKERS)

            def wrong_error(path):
                status, messages = expected(path)
                if path.as_posix().endswith(target):
                    messages = ("RangeError: different blocker",) * 2
                return status, messages

            with patch.object(diagnostic, "_run", side_effect=wrong_error):
                with self.assertRaisesRegex(RuntimeError, "Intl error drifted"):
                    diagnostic.verify(arguments)

            def empty_error(path):
                status, messages = expected(path)
                if path.as_posix().endswith(target):
                    messages = ()
                return status, messages

            with patch.object(diagnostic, "_run", side_effect=empty_error):
                with self.assertRaisesRegex(RuntimeError, "Intl error drifted"):
                    diagnostic.verify(arguments)

            with patch.object(
                diagnostic.test262_runner,
                "execute_source",
                return_value=("fail", "Error: Duration math methods are absent"),
            ):
                with self.assertRaisesRegex(RuntimeError, "method shape is absent"):
                    diagnostic.verify(arguments)


if __name__ == "__main__":
    unittest.main()
