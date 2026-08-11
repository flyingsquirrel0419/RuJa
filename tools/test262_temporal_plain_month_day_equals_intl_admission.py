"""Exact Intl402 accounting for Temporal.PlainMonthDay.prototype.equals."""

from pathlib import Path


def _read(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_FILES = _read(
    "test262_temporal_plain_month_day_equals_intl_admission.txt"
)
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_BLOCKERS = _read(
    "test262_temporal_plain_month_day_equals_intl_blockers.txt"
)
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_SURFACE = (
    TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_FILES
    | TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_BLOCKERS
)


def _features(path):
    features = {"Temporal"}
    if Path(path).name == "future-calendar.js":
        features.add("Intl.Era-monthcode")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in {"future-calendar.js", "infinity-throws-rangeerror.js"}:
        includes.add("temporalHelpers.js")
    if name == "infinity-throws-rangeerror.js":
        includes.add("compareArray.js")
    return frozenset(includes)


TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_SURFACE
}

if (
    len(TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_FILES) != 1
    or len(TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_BLOCKERS) != 3
    or len(TEMPORAL_PLAIN_MONTH_DAY_EQUALS_INTL_SURFACE) != 4
):
    raise RuntimeError("PlainMonthDay.equals Intl surface must contain 1 pass / 3 fail")
