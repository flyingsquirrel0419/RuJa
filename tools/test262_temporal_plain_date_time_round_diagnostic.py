#!/usr/bin/env python3
"""Run the exact pinned Temporal.PlainDateTime.prototype.round surface."""

from pathlib import Path

import test262_runner
from test262_temporal_plain_date_time_round_admission import (
    TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES,
    audit_corpus,
)


SURFACE = TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES
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
    test_root = Path(test262_runner.TEST262) / "test"
    if not test_root.is_dir():
        raise FileNotFoundError(test_root)
    relative_arguments = [_relative(test_root / path) for path in arguments]
    requested = set(relative_arguments)
    if (
        len(arguments) != 45
        or None in requested
        or len(requested) != len(arguments)
        or requested != SURFACE
    ):
        raise RuntimeError(
            "PlainDateTime round diagnostic requires the exact frozen 45-file surface"
        )
    audit_corpus(test262_runner.TEST262, test262_runner.parse_meta)
    actual = {
        path: test262_runner.run_test(test_root / path) for path in sorted(SURFACE)
    }
    expected = {path: "pass" for path in SURFACE}
    if actual != expected:
        raise RuntimeError(f"PlainDateTime round results drifted: {actual}")


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(__import__("sys").argv[1:])
    test262_runner.main()
