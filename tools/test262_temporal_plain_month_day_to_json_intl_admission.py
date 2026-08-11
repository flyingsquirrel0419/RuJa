"""Exact Intl402 accounting for Temporal.PlainMonthDay.prototype.toJSON."""

from pathlib import Path


def _read(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_FILES = _read(
    "test262_temporal_plain_month_day_to_json_intl_admission.txt"
)
TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_BLOCKERS = _read(
    "test262_temporal_plain_month_day_to_json_intl_blockers.txt"
)
TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_SURFACE = (
    TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_FILES
    | TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_BLOCKERS
)

TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_FEATURES = {
    path: frozenset({"Temporal"})
    for path in TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_INCLUDES = {
    path: frozenset() for path in TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_SURFACE
}

if (
    TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_FILES
    or len(TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_BLOCKERS) != 2
    or len(TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INTL_SURFACE) != 2
):
    raise RuntimeError(
        "PlainMonthDay.toJSON Intl surface must contain 0 pass / 2 fail"
    )
