#!/usr/bin/env python3
"""Force the frozen Temporal.Duration.prototype.toString surface through RuJa."""

from pathlib import Path

import test262_runner

from test262_temporal_duration_to_string_admission import (
    TEMPORAL_DURATION_TO_STRING_FILES,
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
    if path is not None and _relative(path) in TEMPORAL_DURATION_TO_STRING_FILES:
        return False
    return _SHARED_SHOULD_SKIP(meta, path)


def verify_expected_results(arguments):
    requested = {
        _relative(Path(test262_runner.TEST262) / "test" / path) for path in arguments
    }
    requested.discard(None)
    frozen = requested & TEMPORAL_DURATION_TO_STRING_FILES
    test_root = Path(test262_runner.TEST262) / "test"
    actual = {path: test262_runner.run_test(test_root / path) for path in frozen}
    expected = {path: "pass" for path in frozen}
    if actual != expected:
        raise RuntimeError(
            f"Temporal.Duration.prototype.toString results drifted: {actual}"
        )


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(__import__("sys").argv[1:])
    test262_runner.main()
