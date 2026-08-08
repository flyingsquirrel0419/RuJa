#!/usr/bin/env python3
"""Force and identity-check the frozen PlainDate.toPlainDateTime surface."""

import sys
from pathlib import Path

import test262_runner
from test262_temporal_plain_date_to_plain_date_time_admission import (
    TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_BLOCKER_FILES,
    TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_DOWNSTREAM_FILES,
    TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_FILES,
)


SURFACE = (
    TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_FILES
    | TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_BLOCKER_FILES
    | TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_DOWNSTREAM_FILES
)
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
    requested = {_relative(Path(test262_runner.TEST262) / "test" / path) for path in arguments}
    requested.discard(None)
    frozen = requested & SURFACE
    test_root = Path(test262_runner.TEST262) / "test"
    actual = {path: test262_runner.run_test(test_root / path) for path in frozen}
    expected = {
        path: "pass" if path in TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_FILES else "fail"
        for path in frozen
    }
    if actual != expected:
        raise RuntimeError(f"PlainDate.toPlainDateTime forced results drifted: {actual}")


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(sys.argv[1:])
    test262_runner.main()
