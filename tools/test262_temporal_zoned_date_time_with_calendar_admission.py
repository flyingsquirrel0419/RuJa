"""Exact Test262 coverage for Temporal.ZonedDateTime.prototype.withCalendar."""

from pathlib import Path

_MANIFEST = Path(__file__).with_name(
    "test262_temporal_zoned_date_time_with_calendar_admission.txt"
)
TEMPORAL_ZONED_DATE_TIME_WITH_CALENDAR_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    if path.endswith("/branding.js"):
        features.add("Symbol")
    if path.endswith("/not-a-constructor.js"):
        features.add("Reflect.construct")
    return frozenset(features)


TEMPORAL_ZONED_DATE_TIME_WITH_CALENDAR_FEATURES = {
    path: _features(path) for path in TEMPORAL_ZONED_DATE_TIME_WITH_CALENDAR_FILES
}

if len(TEMPORAL_ZONED_DATE_TIME_WITH_CALENDAR_FILES) != 14:
    raise RuntimeError("Temporal.ZonedDateTime withCalendar admission must contain 14 files")
