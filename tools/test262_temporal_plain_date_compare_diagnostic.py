#!/usr/bin/env python3
"""Force the frozen Temporal.PlainDate.compare surface through RuJa."""

from pathlib import Path

import test262_runner


TOOLS = Path(__file__).resolve().parent


def read_manifest(name):
    return frozenset(
        line
        for raw_line in (TOOLS / name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


SURFACE = read_manifest("test262_temporal_plain_date_compare_admission.txt") | read_manifest(
    "test262_temporal_plain_date_compare_blockers.txt"
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


if __name__ == "__main__":
    test262_runner.should_skip = should_skip
    test262_runner.main()
