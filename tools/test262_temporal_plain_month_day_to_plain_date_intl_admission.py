"""Exact Intl402 accounting for Temporal.PlainMonthDay.prototype.toPlainDate."""

from pathlib import Path


def _read(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_FILES = _read(
    "test262_temporal_plain_month_day_to_plain_date_intl_admission.txt"
)
TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_BLOCKERS = _read(
    "test262_temporal_plain_month_day_to_plain_date_intl_blockers.txt"
)
TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_SURFACE = (
    TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_FILES
    | TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_BLOCKERS
)

TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_FEATURES = {
    path: frozenset({"Temporal"})
    for path in TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_INCLUDES = {
    path: frozenset({"compareArray.js", "temporalHelpers.js"})
    for path in TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_SURFACE
}
TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_SURFACE
}

if (
    TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_FILES
    or len(TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_BLOCKERS) != 1
    or len(TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INTL_SURFACE) != 1
):
    raise RuntimeError(
        "PlainMonthDay.toPlainDate Intl surface must contain 0 pass / 1 fail"
    )
