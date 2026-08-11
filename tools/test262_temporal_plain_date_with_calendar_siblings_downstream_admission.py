"""Frozen downstream accounting for PlainDate/PlainDateTime withCalendar."""

from pathlib import Path

from test262_temporal_plain_year_month_to_plain_date_downstream_admission import (
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_FEATURES,
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_FLAGS,
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_INCLUDES,
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_NEGATIVE,
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_SURFACE,
)


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_with_calendar_siblings_downstream_blockers.txt"
)
_EXPLICIT_LINES = tuple(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_EXPLICIT = frozenset(
    _EXPLICIT_LINES
)
# Reuse this object, metadata, and its 87-file manifest. Do not duplicate paths.
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_REUSED = (
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_SURFACE
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_FILES = frozenset()
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_BLOCKERS = (
    TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_EXPLICIT
    | TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_REUSED
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_SURFACE = (
    TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_BLOCKERS
)


_TEMPORAL_ONLY = frozenset(
    {
        (
            "intl402/DateTimeFormat/prototype/formatRange/"
            "temporal-objects-throws-with-different-calendars.js"
        ),
        (
            "intl402/DateTimeFormat/prototype/formatRangeToParts/"
            "temporal-objects-throws-with-different-calendars.js"
        ),
        "intl402/Temporal/PlainDate/from/japanese-pre-meiji.js",
    }
)
_NO_HELPERS = frozenset(
    path
    for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_EXPLICIT
    if path.startswith("intl402/DateTimeFormat/") or path.endswith("/epoch-year.js")
)


def _explicit_features(path):
    features = {"Temporal"}
    if path not in _TEMPORAL_ONLY:
        features.add("Intl.Era-monthcode")
    return frozenset(features)


def _explicit_includes(path):
    if path in _NO_HELPERS:
        return frozenset()
    return frozenset({"temporalHelpers.js"})


TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_FEATURES = dict(
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_FEATURES
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_FEATURES.update(
    {
        path: _explicit_features(path)
        for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_EXPLICIT
    }
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_INCLUDES = dict(
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_INCLUDES
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_INCLUDES.update(
    {
        path: _explicit_includes(path)
        for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_EXPLICIT
    }
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_FLAGS = dict(
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_FLAGS
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_FLAGS.update(
    {
        path: frozenset()
        for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_EXPLICIT
    }
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_NEGATIVE = dict(
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_NEGATIVE
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_NEGATIVE.update(
    {
        path: None
        for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_EXPLICIT
    }
)


if (
    len(_EXPLICIT_LINES) != 50
    or tuple(sorted(_EXPLICIT_LINES)) != _EXPLICIT_LINES
    or len(TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_EXPLICIT) != 50
    or len(TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_REUSED) != 87
    or not TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_EXPLICIT.isdisjoint(
        TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_REUSED
    )
    or TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_FILES
    or len(TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_SURFACE) != 137
    or any(
        set(metadata) != TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_SURFACE
        for metadata in (
            TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_FEATURES,
            TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_INCLUDES,
            TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_FLAGS,
            TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_DOWNSTREAM_NEGATIVE,
        )
    )
):
    raise RuntimeError(
        "withCalendar sibling downstream must be a disjoint 50 + reused 87 = 137"
    )
