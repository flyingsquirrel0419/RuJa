#!/usr/bin/env python3
"""Force the exact 36-file supported withCalendar sibling surface."""

from pathlib import Path
import sys

import test262_runner
from test262_temporal_plain_date_with_calendar_siblings_admission import (
    TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FEATURES,
    TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FILES,
    TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FLAGS,
    TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_INCLUDES,
    TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_NEGATIVE,
)


SURFACE = TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FILES
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


def _verify_corpus(test_root):
    for relative in sorted(SURFACE):
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
            TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FEATURES[relative],
            TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_INCLUDES[relative],
            TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FLAGS[relative],
            TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_NEGATIVE[relative],
        )
        if actual != expected:
            raise RuntimeError(
                f"withCalendar sibling corpus metadata drifted: {relative}: {actual}"
            )


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
            "withCalendar sibling diagnostic requires the exact frozen 36-file surface"
        )
    _verify_corpus(test_root)
    actual = {
        path: test262_runner.run_test(test_root / path) for path in sorted(SURFACE)
    }
    expected = {path: "pass" for path in SURFACE}
    if actual != expected:
        raise RuntimeError(f"withCalendar sibling results drifted: {actual}")


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(sys.argv[1:])
    test262_runner.main()
