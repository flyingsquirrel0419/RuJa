"""Exact Test262 coverage for the Temporal.ZonedDateTime hidden-slot core."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_zoned_date_time_core_admission.txt"
)
TEMPORAL_ZONED_DATE_TIME_CORE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_BIGINT_FILES = frozenset(
    {
        "built-ins/Temporal/ZonedDateTime/calendar-undefined.js",
        "built-ins/Temporal/ZonedDateTime/calendar-wrong-type.js",
        "built-ins/Temporal/ZonedDateTime/prototype/epochMilliseconds/basic.js",
        "built-ins/Temporal/ZonedDateTime/prototype/epochNanoseconds/basic.js",
        "built-ins/Temporal/ZonedDateTime/timezone-wrong-type.js",
    }
)
_SYMBOL_FILES = frozenset(
    {
        "built-ins/Temporal/ZonedDateTime/prototype/calendarId/branding.js",
        "built-ins/Temporal/ZonedDateTime/prototype/epochMilliseconds/branding.js",
        "built-ins/Temporal/ZonedDateTime/prototype/epochNanoseconds/branding.js",
        "built-ins/Temporal/ZonedDateTime/prototype/timeZoneId/branding.js",
        "built-ins/Temporal/ZonedDateTime/calendar-wrong-type.js",
        "built-ins/Temporal/ZonedDateTime/timezone-wrong-type.js",
    }
)


def _features(path):
    features = {"Temporal"}
    if path in _BIGINT_FILES:
        features.add("BigInt")
    if path in _SYMBOL_FILES:
        features.add("Symbol")
    return frozenset(features)


TEMPORAL_ZONED_DATE_TIME_CORE_FEATURES = {
    path: _features(path) for path in TEMPORAL_ZONED_DATE_TIME_CORE_FILES
}

if len(TEMPORAL_ZONED_DATE_TIME_CORE_FILES) != 37:
    raise RuntimeError("Temporal.ZonedDateTime core admission must contain 37 files")
