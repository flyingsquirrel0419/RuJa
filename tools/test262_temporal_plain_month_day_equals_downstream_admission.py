"""Frozen downstream Test262 blockers for PlainMonthDay.prototype.equals."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_month_day_equals_downstream_blockers.txt"
)
_LINES = tuple(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_FILES = frozenset()
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_BLOCKERS = frozenset(_LINES)
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_SURFACE = (
    TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_FILES
    | TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_BLOCKERS
)

_ERA_MONTHCODE = frozenset(
    {
        "intl402/Temporal/PlainMonthDay/from/chinese-30-day-leap-months.js",
        "intl402/Temporal/PlainMonthDay/from/chinese-dangi-constrain-rare-leap-months.js",
    }
)


def _features(path):
    features = {"Temporal"}
    if path in _ERA_MONTHCODE:
        features.add("Intl.Era-monthcode")
    return frozenset(features)


TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_INCLUDES = {
    path: frozenset({"temporalHelpers.js"})
    for path in TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_SURFACE
}

if (
    len(_LINES) != 7
    or tuple(sorted(_LINES)) != _LINES
    or TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_FILES
    or len(TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_BLOCKERS) != 7
    or not _ERA_MONTHCODE < TEMPORAL_PLAIN_MONTH_DAY_EQUALS_DOWNSTREAM_BLOCKERS
):
    raise RuntimeError("PlainMonthDay.equals downstream must contain 0 pass / 7 fail")
