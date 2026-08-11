#!/usr/bin/env python3
"""Force the complete 243-file PlainDateTime arithmetic ownership surface."""

from pathlib import Path
import re
import sys

import test262_runner
from test262_temporal_plain_date_time_arithmetic_admission import (
    TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_COMPLETE_FILES,
    TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_DOWNSTREAM_FILES,
    TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES,
    audit_corpus,
)


SURFACE = TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_COMPLETE_FILES
DIRECT = TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES
DOWNSTREAM = TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_DOWNSTREAM_FILES
_SHARED_SHOULD_SKIP = test262_runner.should_skip
_CALENDAR_ERROR = "RangeError: Invalid Temporal calendar identifier"
_MISSING_METHOD_ERROR = "TypeError: undefined is not a function"


def _relative(path):
    try:
        return (
            Path(path)
            .resolve()
            .relative_to((Path(test262_runner.TEST262) / "test").resolve())
            .as_posix()
        )
    except (OSError, TypeError, ValueError):
        return None


def should_skip(meta, path=None):
    if path is not None and _relative(path) in SURFACE:
        return False
    return _SHARED_SHOULD_SKIP(meta, path)


def _run_with_diagnostics(path):
    source = test262_runner.read_source(path)
    metadata = test262_runner.parse_meta(source)
    timeout = test262_runner.test_timeout_seconds(path)
    results = []
    for label, strict in test262_runner.execution_variants(metadata):
        full = test262_runner.assemble_source(source, metadata, strict=strict)
        status, diagnostic = test262_runner.execute_source(
            full, metadata, test262_runner.RUJA, timeout=timeout
        )
        results.append((label, status, diagnostic))
    status, _ = test262_runner.combine_variant_results(results)
    return status, tuple(diagnostic for _, _, diagnostic in results)


def _normalize(message):
    return re.sub(r" \(at line \d+\)$", "", message)


def verify_expected_results(arguments):
    test_root = Path(test262_runner.TEST262) / "test"
    if not test_root.is_dir():
        raise FileNotFoundError(test_root)
    relative_arguments = [_relative(test_root / path) for path in arguments]
    requested = set(relative_arguments)
    if (
        len(arguments) != 243
        or None in requested
        or len(requested) != len(arguments)
        or requested != SURFACE
    ):
        raise RuntimeError(
            "complete PlainDateTime arithmetic diagnostic requires the exact frozen 243-file surface"
        )
    audit_corpus(test262_runner.TEST262, test262_runner.parse_meta)
    diagnostics = {
        path: _run_with_diagnostics(test_root / path) for path in sorted(SURFACE)
    }
    actual = {path: result[0] for path, result in diagnostics.items()}
    expected = {path: "pass" if path in DIRECT else "fail" for path in SURFACE}
    if actual != expected:
        raise RuntimeError(f"complete PlainDateTime arithmetic results drifted: {actual}")
    wrong_errors = {}
    for path, (status, messages) in diagnostics.items():
        if status == "pass":
            continue
        expected_error = _MISSING_METHOD_ERROR if path in DOWNSTREAM else _CALENDAR_ERROR
        if not messages or any(_normalize(message) != expected_error for message in messages):
            wrong_errors[path] = messages
    if wrong_errors:
        raise RuntimeError(
            f"complete PlainDateTime arithmetic failure reasons drifted: {wrong_errors}"
        )


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(sys.argv[1:])
    test262_runner.main()
