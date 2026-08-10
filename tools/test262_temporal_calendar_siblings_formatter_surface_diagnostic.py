#!/usr/bin/env python3
"""Force the frozen 128-file partial-date formatter-dependent surface."""

from pathlib import Path
import sys

import test262_runner
from test262_temporal_calendar_siblings_from_admission import (
    TEMPORAL_CALENDAR_SIBLINGS_FROM_FORMATTER_TRANSITIONS,
)
from test262_temporal_calendar_siblings_to_string_admission import (
    TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FILES,
)


def _read_manifest(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


CORE_TRANSITIONS = _read_manifest(
    "test262_temporal_calendar_siblings_core_formatter_transitions.txt"
)
WITH_TRANSITIONS = _read_manifest(
    "test262_temporal_calendar_siblings_with_formatter_transitions.txt"
)
ARITHMETIC_TRANSITIONS = _read_manifest(
    "test262_temporal_plain_year_month_arithmetic_formatter_transitions.txt"
)
PASSING = (
    TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FILES
    | CORE_TRANSITIONS
    | TEMPORAL_CALENDAR_SIBLINGS_FROM_FORMATTER_TRANSITIONS
    | WITH_TRANSITIONS
    | ARITHMETIC_TRANSITIONS
)
SURFACE = PASSING
_SHARED_SHOULD_SKIP = test262_runner.should_skip

if len(ARITHMETIC_TRANSITIONS) != 22 or len(SURFACE) != 128:
    raise RuntimeError("Temporal calendar sibling formatter surface must be 128 pass")


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
            "Temporal calendar sibling formatter diagnostic requires the exact frozen surface"
        )
    actual = {path: test262_runner.run_test(test_root / path) for path in SURFACE}
    expected = {path: "pass" for path in SURFACE}
    if actual != expected:
        raise RuntimeError(f"Temporal calendar sibling formatter results drifted: {actual}")


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(sys.argv[1:])
    test262_runner.main()
