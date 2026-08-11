#!/usr/bin/env python3
"""Force the exact PlainDate sibling conversion downstream blockers."""

from pathlib import Path
import re
import sys

import test262_runner
from test262_temporal_plain_date_sibling_conversions_downstream_admission import (
    TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_BLOCKERS,
    TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_FEATURES,
    TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_FLAGS,
    TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_INCLUDES,
    TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_NEGATIVE,
    TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_SURFACE,
)


BLOCKERS = TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_BLOCKERS
SURFACE = TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_SURFACE
_SHARED_SHOULD_SKIP = test262_runner.should_skip
_EXPECTED_ERRORS = {
    (
        "intl402/Temporal/PlainMonthDay/from/"
        "chinese-dangi-leap-month-with-year-from-options-bag.js"
    ): "RangeError: Invalid Temporal calendar identifier",
    (
        "intl402/Temporal/PlainYearMonth/prototype/toLocaleString/"
        "calendar-mismatch.js"
    ): "TypeError: not a constructor",
    (
        "intl402/Temporal/PlainYearMonth/prototype/year/epoch-year.js"
    ): "RangeError: Invalid Temporal calendar identifier",
}


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


def _verify_corpus(test_root):
    for relative in SURFACE:
        path = test_root / relative
        if not path.is_file():
            raise FileNotFoundError(path)
        metadata = test262_runner.parse_meta(test262_runner.read_source(path))
        actual = (
            frozenset(metadata.get("features", [])),
            frozenset(metadata.get("includes", [])),
            frozenset(metadata.get("flags", [])),
            metadata.get("negative"),
        )
        expected = (
            TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_FEATURES[relative],
            TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_INCLUDES[relative],
            TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_FLAGS[relative],
            TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_NEGATIVE[relative],
        )
        if actual != expected:
            raise RuntimeError(
                f"PlainDate sibling conversion downstream metadata drifted: {relative}: {actual}"
            )


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
            "PlainDate sibling conversion downstream diagnostic requires the exact frozen surface"
        )
    _verify_corpus(test_root)
    diagnostics = {path: _run_with_diagnostics(test_root / path) for path in SURFACE}
    actual = {path: result[0] for path, result in diagnostics.items()}
    expected = {path: "fail" for path in SURFACE}
    if actual != expected:
        raise RuntimeError(
            f"PlainDate sibling conversion downstream results drifted: {actual}"
        )
    wrong_errors = {
        path: messages
        for path, (_, messages) in diagnostics.items()
        if not messages
        or any(
            re.sub(r" \(at line \d+\)$", "", message) != _EXPECTED_ERRORS[path]
            for message in messages
        )
    }
    if wrong_errors:
        raise RuntimeError(
            "PlainDate sibling conversion downstream failure reasons drifted: "
            f"{wrong_errors}"
        )


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(sys.argv[1:])
    test262_runner.main()
