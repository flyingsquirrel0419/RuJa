"""Exact Test262 coverage for Temporal.ZonedDateTime.prototype.toPlainDateTime."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_zoned_date_time_to_plain_date_time_admission.txt"
)
TEMPORAL_ZONED_DATE_TIME_TO_PLAIN_DATE_TIME_FILES = frozenset(
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


TEMPORAL_ZONED_DATE_TIME_TO_PLAIN_DATE_TIME_FEATURES = {
    path: _features(path)
    for path in TEMPORAL_ZONED_DATE_TIME_TO_PLAIN_DATE_TIME_FILES
}

if len(TEMPORAL_ZONED_DATE_TIME_TO_PLAIN_DATE_TIME_FILES) != 10:
    raise RuntimeError(
        "Temporal.ZonedDateTime toPlainDateTime admission must contain 10 files"
    )
