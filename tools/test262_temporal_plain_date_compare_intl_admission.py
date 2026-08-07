"""Exact Intl402 Test262 coverage unlocked by Temporal.PlainDate.compare."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_compare_intl_admission.txt"
)
TEMPORAL_PLAIN_DATE_COMPARE_INTL_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_DATE_COMPARE_INTL_FEATURES = {
    "intl402/Temporal/PlainDate/compare/future-calendar.js": frozenset(
        {"Intl.Era-monthcode", "Temporal"}
    )
}
TEMPORAL_PLAIN_DATE_COMPARE_INTL_INCLUDES = {
    "intl402/Temporal/PlainDate/compare/future-calendar.js": frozenset(
        {"temporalHelpers.js"}
    )
}
TEMPORAL_PLAIN_DATE_COMPARE_INTL_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_COMPARE_INTL_FILES
}
TEMPORAL_PLAIN_DATE_COMPARE_INTL_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_COMPARE_INTL_FILES
}

if set(TEMPORAL_PLAIN_DATE_COMPARE_INTL_FEATURES) != set(
    TEMPORAL_PLAIN_DATE_COMPARE_INTL_FILES
):
    raise RuntimeError("Temporal.PlainDate.compare Intl admission must contain one file")
