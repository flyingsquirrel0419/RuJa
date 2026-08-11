#!/usr/bin/env python3
"""Force the exact PlainDate sibling conversion directories through RuJa."""

from pathlib import Path
import sys

import test262_runner
from test262_temporal_plain_date_sibling_conversions_admission import (
    TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES,
)


SURFACE = TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES
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
        len(arguments) != len(SURFACE)
        or None in requested
        or len(requested) != len(arguments)
        or requested != SURFACE
    ):
        raise RuntimeError(
            "PlainDate sibling conversion diagnostic requires the exact frozen surface"
        )
    actual = {path: test262_runner.run_test(test_root / path) for path in SURFACE}
    expected = {path: "pass" for path in SURFACE}
    if actual != expected:
        raise RuntimeError(f"PlainDate sibling conversion results drifted: {actual}")


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results(sys.argv[1:])
    test262_runner.main()
