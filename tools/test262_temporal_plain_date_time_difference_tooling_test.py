#!/usr/bin/env python3
"""Regression tests for exact PlainDateTime until/since tooling."""

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import test262_runner
import test262_analyze
import test262_temporal_plain_date_time_difference_admission as admission
import test262_temporal_plain_date_time_difference_complete_diagnostic as complete
import test262_temporal_plain_date_time_difference_diagnostic as direct


class PlainDateTimeDifferenceToolingTests(unittest.TestCase):
    def test_manifests_are_exact_disjoint_and_shared(self):
        surfaces = (
            admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_FILES,
            admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_TRANSITIONS,
            admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DOWNSTREAM_FILES,
            admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_INTL_BLOCKERS,
            admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_HOMONYMS,
        )
        self.assertEqual(tuple(map(len, surfaces)), (191, 2, 1, 117, 11))
        for index, left in enumerate(surfaces):
            for right in surfaces[index + 1 :]:
                self.assertTrue(left.isdisjoint(right))
        self.assertEqual(
            len(admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES), 194
        )
        self.assertEqual(
            len(admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_COMPLETE_FILES),
            311,
        )
        self.assertEqual(
            set(admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_METADATA),
            admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_COMPLETE_FILES,
        )
        self.assertEqual(
            set(admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FEATURES),
            admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES,
        )
        self.assertTrue(
            admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_PREIMPLEMENTATION_FALSE_POSITIVES
            <= admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_FILES
        )

    def test_live_pinned_corpus_is_exact_when_available(self):
        corpus_root = Path(
            os.environ.get("TEST262", "/tmp/ruja-test262-difference-pinned")
        )
        required = "TEST262" in os.environ
        if not corpus_root.is_dir():
            if required:
                raise FileNotFoundError(corpus_root)
            self.skipTest("live pinned Test262 corpus is unavailable")
        candidates = admission.audit_corpus(corpus_root, test262_runner.parse_meta)
        self.assertEqual(len(candidates), 322)
        self.assertEqual(len(admission._ownership_rows(candidates)), 328)

    def test_corpus_unavailable_fails_closed(self):
        with self.assertRaises(FileNotFoundError):
            admission.audit_corpus(
                "/definitely/missing/test262", test262_runner.parse_meta
            )
        original_root = direct.test262_runner.TEST262
        direct.test262_runner.TEST262 = "/definitely/missing/test262"
        try:
            with self.assertRaises(FileNotFoundError):
                direct.verify_expected_results(sorted(direct.SURFACE))
        finally:
            direct.test262_runner.TEST262 = original_root

    def test_runner_wiring_is_exact_without_a_corpus(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            original_root = test262_runner.TEST262
            original_analyze_root = test262_analyze.TEST262
            test262_runner.TEST262 = str(root)
            test262_analyze.TEST262 = str(root)
            try:
                for relative in admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES:
                    path = root / "test" / relative
                    expected = admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FEATURES[
                        relative
                    ]
                    self.assertTrue(
                        test262_runner.temporal_plain_date_time_difference_path(path)
                    )
                    self.assertEqual(
                        test262_runner.temporal_plain_date_time_difference_features(path),
                        expected,
                    )
                    self.assertFalse(
                        test262_runner.should_skip({"features": list(expected)}, path)
                    )
                    self.assertTrue(
                        test262_analyze.temporal_plain_date_time_difference_path(path)
                    )
                    self.assertEqual(
                        test262_analyze.temporal_plain_date_time_difference_features(path),
                        expected,
                    )
                    self.assertEqual(
                        test262_runner.should_skip({"features": list(expected)}, path),
                        test262_analyze.should_skip({"features": list(expected)}, path),
                    )
                outside = admission.TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_INTL_BLOCKERS
                for relative in outside:
                    path = root / "test" / relative
                    self.assertFalse(
                        test262_runner.temporal_plain_date_time_difference_path(path)
                    )
                future = root / "test/built-ins/Temporal/PlainDateTime/prototype/until/future.js"
                for path in (future, None, object()):
                    self.assertFalse(
                        test262_runner.temporal_plain_date_time_difference_path(path)
                    )
                    self.assertEqual(
                        test262_runner.temporal_plain_date_time_difference_features(path),
                        frozenset(),
                    )
            finally:
                test262_runner.TEST262 = original_root
                test262_analyze.TEST262 = original_analyze_root

    def test_reference_and_call_audit_is_token_aware(self):
        source = r'''
const text = "ignored.until() and ignored.since()";
const regexp = /ignored\.until\(\)/;
// ignored.since();
const value = new Temporal.PlainDateTime(2000, 1, 1);
value.until(value);
value?.since?.(value);
value["until"](value);
value?.["since"]?.(value);
const untilAlias = value.until;
const sinceAlias = value["since"];
'''
        path = "built-ins/Temporal/PlainDateTime/prototype/until/synthetic.js"
        candidates = admission._candidate_stats({path: source})
        counts, tokens = candidates[path]
        self.assertEqual(admission._member_counts(tokens, "until"), (2, 1, 1, 1))
        self.assertEqual(admission._member_counts(tokens, "since"), (1, 2, 1, 1))
        self.assertEqual(counts[0], 1)
        with self.assertRaisesRegex(RuntimeError, "candidate surface drifted"):
            admission.verify_candidate_contract(candidates)

    def test_diagnostics_require_exact_arguments_results_and_locations(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            (root / "test").mkdir()
            original_direct_root = direct.test262_runner.TEST262
            original_complete_root = complete.test262_runner.TEST262
            direct.test262_runner.TEST262 = str(root)
            complete.test262_runner.TEST262 = str(root)
            try:
                direct_arguments = sorted(direct.SURFACE)
                with (
                    patch.object(direct, "audit_corpus"),
                    patch.object(direct.test262_runner, "run_test", return_value="pass"),
                ):
                    direct.verify_expected_results(direct_arguments)
                    with self.assertRaisesRegex(RuntimeError, "193-file direct surface"):
                        direct.verify_expected_results(direct_arguments[:-1])
                with (
                    patch.object(direct, "audit_corpus"),
                    patch.object(direct.test262_runner, "run_test", return_value="fail"),
                ):
                    with self.assertRaisesRegex(RuntimeError, "results drifted"):
                        direct.verify_expected_results(direct_arguments)

                complete_arguments = sorted(complete.SURFACE)

                def expected_result(path):
                    relative = path.relative_to(root / "test").as_posix()
                    if relative in complete.SUPPORTED:
                        return "pass", ("", "")
                    error = complete._CALENDAR_ERROR
                    return "fail", (
                        error + " (at line 100)",
                        error + " (at line 101)",
                    )

                expected = {
                    path: expected_result(root / "test" / path)
                    for path in complete.SURFACE
                }
                digest = complete._failure_diagnostic_digest(expected)
                with (
                    patch.object(complete, "audit_corpus"),
                    patch.object(
                        complete, "_EXPECTED_FAILURE_DIAGNOSTIC_DIGEST", digest
                    ),
                    patch.object(
                        complete, "_run_with_diagnostics", side_effect=expected_result
                    ),
                ):
                    complete.verify_expected_results(complete_arguments)
                    with self.assertRaisesRegex(RuntimeError, "311-file surface"):
                        complete.verify_expected_results(complete_arguments[:-1])

                def wrong_result(path):
                    status, messages = expected_result(path)
                    relative = path.relative_to(root / "test").as_posix()
                    if relative == min(complete.SUPPORTED):
                        return "fail", (
                            complete._CALENDAR_ERROR + " (at line 100)",
                            complete._CALENDAR_ERROR + " (at line 101)",
                        )
                    return status, messages

                with (
                    patch.object(complete, "audit_corpus"),
                    patch.object(
                        complete, "_run_with_diagnostics", side_effect=wrong_result
                    ),
                ):
                    with self.assertRaisesRegex(RuntimeError, "results drifted"):
                        complete.verify_expected_results(complete_arguments)

                def wrong_error(path):
                    status, messages = expected_result(path)
                    relative = path.relative_to(root / "test").as_posix()
                    if relative == min(complete.INTL_BLOCKERS):
                        messages = tuple(
                            message.replace(
                                complete._CALENDAR_ERROR,
                                "RangeError: different calendar dependency",
                            )
                            for message in messages
                        )
                    return status, messages

                with (
                    patch.object(complete, "audit_corpus"),
                    patch.object(
                        complete, "_run_with_diagnostics", side_effect=wrong_error
                    ),
                ):
                    with self.assertRaisesRegex(RuntimeError, "reasons drifted"):
                        complete.verify_expected_results(complete_arguments)

                def wrong_location(path):
                    status, messages = expected_result(path)
                    relative = path.relative_to(root / "test").as_posix()
                    if relative == min(complete.INTL_BLOCKERS):
                        messages = tuple(
                            message.replace("line 100", "line 102")
                            for message in messages
                        )
                    return status, messages

                with (
                    patch.object(complete, "audit_corpus"),
                    patch.object(
                        complete, "_EXPECTED_FAILURE_DIAGNOSTIC_DIGEST", digest
                    ),
                    patch.object(
                        complete, "_run_with_diagnostics", side_effect=wrong_location
                    ),
                ):
                    with self.assertRaisesRegex(RuntimeError, "locations drifted"):
                        complete.verify_expected_results(complete_arguments)
            finally:
                direct.test262_runner.TEST262 = original_direct_root
                complete.test262_runner.TEST262 = original_complete_root


if __name__ == "__main__":
    unittest.main()
