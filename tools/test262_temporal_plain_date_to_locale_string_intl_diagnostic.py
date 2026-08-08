#!/usr/bin/env python3
"""Force the frozen Intl402 PlainDate.toLocaleString blocker surface."""

from pathlib import Path

import test262_runner


TOOLS = Path(__file__).resolve().parent
SURFACE = frozenset(
    line
    for raw_line in (
        TOOLS / "test262_temporal_plain_date_to_locale_string_intl_blockers.txt"
    ).read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
EXPECTED_PASSES = frozenset(
    line
    for raw_line in (
        TOOLS
        / "test262_temporal_plain_date_to_locale_string_intl_forced_passes.txt"
    ).read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_SHARED_SHOULD_SKIP = test262_runner.should_skip


def should_skip(meta, path=None):
    if path is not None:
        try:
            relative = (
                Path(path)
                .resolve()
                .relative_to((Path(test262_runner.TEST262) / "test").resolve())
                .as_posix()
            )
        except (OSError, TypeError, ValueError):
            pass
        else:
            if relative in SURFACE:
                return False
    return _SHARED_SHOULD_SKIP(meta, path)


def verify_expected_results():
    test_root = Path(test262_runner.TEST262) / "test"
    actual = {
        relative: test262_runner.run_test(test_root / relative)
        for relative in SURFACE
    }
    expected = {
        relative: "pass" if relative in EXPECTED_PASSES else "fail"
        for relative in SURFACE
    }
    if actual != expected:
        raise RuntimeError(f"Intl402 PlainDate.toLocaleString forced results drifted: {actual}")


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    verify_expected_results()
    test262_runner.main()
