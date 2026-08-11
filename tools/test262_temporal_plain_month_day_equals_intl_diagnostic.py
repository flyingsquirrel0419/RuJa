#!/usr/bin/env python3
"""Force the four-file Intl402 PlainMonthDay.equals companion surface."""

from pathlib import Path
import re
import sys

import test262_runner
from test262_temporal_plain_month_day_equals_intl_admission import (
    TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_BLOCKERS,
    TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_SURFACE,
)


BLOCKERS = TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_BLOCKERS
SURFACE = TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_SURFACE
_SHARED_SHOULD_SKIP = test262_runner.should_skip


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


def verify_expected_results(arguments):
    test_root = Path(test262_runner.TEST262) / "test"
    if not test_root.is_dir():
        raise FileNotFoundError(test_root)
    relative_arguments = [_relative(test_root / path) for path in arguments]
    requested = set(relative_arguments)
    if (
        len(arguments) != len(SURFACE)
        or None in requested
        or len(requested) != len(arguments)
        or requested != SURFACE
    ):
        raise RuntimeError(
            "PlainMonthDay equals Intl diagnostic requires the exact frozen surface"
        )
    diagnostics = {path: _run_with_diagnostics(test_root / path) for path in SURFACE}
    actual = {path: result[0] for path, result in diagnostics.items()}
    expected = {path: "fail" if path in BLOCKERS else "pass" for path in SURFACE}
    if actual != expected:
        raise RuntimeError(f"PlainMonthDay equals Intl results drifted: {actual}")
    expected_error = "RangeError: Invalid Temporal calendar identifier"
    wrong_errors = {
        path: messages
        for path, (_, messages) in diagnostics.items()
        if path in BLOCKERS
        and (
            not messages
            or any(
                re.sub(r" \(at line \d+\)$", "", message) != expected_error
                for message in messages
            )
        )
    }
    if wrong_errors:
        raise RuntimeError(
            f"PlainMonthDay equals Intl failure reasons drifted: {wrong_errors}"
        )


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(sys.argv[1:])
    test262_runner.main()
