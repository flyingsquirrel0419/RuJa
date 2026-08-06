"""Exact Test262 coverage for Temporal.ZonedDateTime.prototype.startOfDay."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_zoned_date_time_start_of_day_admission.txt"
)
TEMPORAL_ZONED_DATE_TIME_START_OF_DAY_FILES = frozenset(
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


TEMPORAL_ZONED_DATE_TIME_START_OF_DAY_FEATURES = {
    path: _features(path) for path in TEMPORAL_ZONED_DATE_TIME_START_OF_DAY_FILES
}

if len(TEMPORAL_ZONED_DATE_TIME_START_OF_DAY_FILES) != 9:
    raise RuntimeError("Temporal.ZonedDateTime startOfDay admission must contain 9 files")
