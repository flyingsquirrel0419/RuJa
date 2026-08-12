#!/usr/bin/env python3
"""Force and verify the exact Duration add/compare ownership surface."""

from pathlib import Path
import re
import sys

import test262_runner
from test262_temporal_duration_math_admission import (
    TEMPORAL_DURATION_MATH_ADMITTED,
    TEMPORAL_DURATION_MATH_COMPLETE,
    TEMPORAL_DURATION_MATH_DOWNSTREAM_BLOCKERS,
    TEMPORAL_DURATION_MATH_INTL_BLOCKERS,
    audit_metadata,
)


SURFACE = TEMPORAL_DURATION_MATH_COMPLETE
_SHARED_SHOULD_SKIP = test262_runner.should_skip
_INTL_ERRORS = {
    "intl402/Temporal/Duration/compare/relativeto-hour.js": "RangeError: Invalid Temporal.ZonedDateTime string",
    "intl402/Temporal/Duration/compare/relativeto-sub-minute-offset.js": "RangeError: Named Temporal time zones are not available",
    "intl402/Temporal/Duration/compare/twenty-five-hour-day.js": "RangeError: Invalid Temporal time zone identifier",
}


def _relative(path):
    try:
        return (
            Path(path).resolve()
            .relative_to((Path(test262_runner.TEST262) / "test").resolve())
            .as_posix()
        )
    except (OSError, TypeError, ValueError):
        return None


def should_skip(meta, path=None):
    if path is not None and _relative(path) in SURFACE:
        return False
    return _SHARED_SHOULD_SKIP(meta, path)


def _run(path):
    source = test262_runner.read_source(path)
    metadata = test262_runner.parse_meta(source)
    results = []
    for _, strict in test262_runner.execution_variants(metadata):
        assembled = test262_runner.assemble_source(source, metadata, strict=strict)
        status, message = test262_runner.execute_source(
            assembled,
            metadata,
            test262_runner.RUJA,
            timeout=test262_runner.test_timeout_seconds(path),
        )
        results.append((status, re.sub(r" \(at line \d+\)$", "", message)))
    status, _ = test262_runner.combine_variant_results(
        [(str(index), status, message) for index, (status, message) in enumerate(results)]
    )
    return status, tuple(message for _, message in results)


def verify(arguments):
    requested = {_relative(Path(test262_runner.TEST262) / "test" / path) for path in arguments}
    requested.discard(None)
    if len(arguments) != 94 or requested != SURFACE:
        raise RuntimeError("Duration math diagnostic requires the exact 94-file surface")
    audit_metadata(test262_runner.TEST262, test262_runner.parse_meta)
    shape_source = """
if (typeof Temporal.Duration.prototype.add !== 'function' ||
    typeof Temporal.Duration.compare !== 'function') {
  throw new Error('Duration math methods are absent');
}
"""
    shape_status, shape_message = test262_runner.execute_source(
        shape_source, {}, test262_runner.RUJA,
        timeout=8,
    )
    if shape_status != "pass":
        raise RuntimeError(f"Duration math method shape is absent: {shape_message}")
    root = Path(test262_runner.TEST262) / "test"
    actual = {path: _run(root / path) for path in sorted(SURFACE)}
    expected_status = {
        path: "pass" if path in TEMPORAL_DURATION_MATH_ADMITTED else "fail"
        for path in SURFACE
    }
    if {path: result[0] for path, result in actual.items()} != expected_status:
        raise RuntimeError(f"Duration math results drifted: {actual}")
    for path in TEMPORAL_DURATION_MATH_INTL_BLOCKERS:
        if not actual[path][1] or any(
            message != _INTL_ERRORS[path] for message in actual[path][1]
        ):
            raise RuntimeError(f"Duration math Intl error drifted: {path}: {actual[path]}")
    for path in TEMPORAL_DURATION_MATH_DOWNSTREAM_BLOCKERS:
        if not actual[path][1] or any(
            message != "TypeError: undefined is not a function"
            for message in actual[path][1]
        ):
            raise RuntimeError(f"Duration math downstream error drifted: {path}: {actual[path]}")


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify(sys.argv[1:])
    test262_runner.main()
