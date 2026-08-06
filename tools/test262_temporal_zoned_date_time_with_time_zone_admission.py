"""Exact Test262 coverage for Temporal.ZonedDateTime.prototype.withTimeZone."""

from pathlib import Path

_MANIFEST = Path(__file__).with_name(
    "test262_temporal_zoned_date_time_with_time_zone_admission.txt"
)
TEMPORAL_ZONED_DATE_TIME_WITH_TIME_ZONE_FILES = frozenset(
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
    if path.endswith("/timezone-string-year-zero.js"):
        features.add("arrow-function")
    if path.endswith("/timezone-wrong-type.js"):
        features.update(("BigInt", "Symbol"))
    return frozenset(features)


TEMPORAL_ZONED_DATE_TIME_WITH_TIME_ZONE_FEATURES = {
    path: _features(path) for path in TEMPORAL_ZONED_DATE_TIME_WITH_TIME_ZONE_FILES
}

if len(TEMPORAL_ZONED_DATE_TIME_WITH_TIME_ZONE_FILES) != 16:
    raise RuntimeError(
        "Temporal.ZonedDateTime withTimeZone admission must contain 16 files"
    )
