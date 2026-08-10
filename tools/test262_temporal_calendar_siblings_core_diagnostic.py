#!/usr/bin/env python3
"""Force the frozen PlainMonthDay/PlainYearMonth constructor-core surface."""

from pathlib import Path

import test262_runner

from test262_temporal_calendar_siblings_core_admission import (
    TEMPORAL_CALENDAR_SIBLINGS_CORE_FILES,
)


def _read_manifest(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


BLOCKERS = _read_manifest("test262_temporal_calendar_siblings_core_blockers.txt")
SURFACE = TEMPORAL_CALENDAR_SIBLINGS_CORE_FILES | BLOCKERS
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


def verify_expected_results(arguments):
    requested = {_relative(path) for path in arguments}
    requested.discard(None)
    frozen = requested & SURFACE
    test_root = Path(test262_runner.TEST262) / "test"
    actual = {path: test262_runner.run_test(test_root / path) for path in frozen}
    expected = {path: "fail" if path in BLOCKERS else "pass" for path in frozen}
    if actual != expected:
        raise RuntimeError(f"Temporal calendar sibling core results drifted: {actual}")


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(__import__("sys").argv[1:])
    test262_runner.main()
