"""Exact Test262 coverage for Temporal.ZonedDateTime.compare."""

from pathlib import Path

_MANIFEST = Path(__file__).with_name(
    "test262_temporal_zoned_date_time_compare_admission.txt"
)
TEMPORAL_ZONED_DATE_TIME_COMPARE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    if path.endswith("calendar-year-zero.js") or path.endswith(
        "timezone-string-year-zero.js"
    ):
        features.add("arrow-function")
    if path.endswith("/argument-wrong-type.js"):
        features.update(("BigInt", "Symbol"))
    if path.endswith("/not-a-constructor.js"):
        features.add("Reflect.construct")
    return frozenset(features)


TEMPORAL_ZONED_DATE_TIME_COMPARE_FEATURES = {
    path: _features(path) for path in TEMPORAL_ZONED_DATE_TIME_COMPARE_FILES
}

if len(TEMPORAL_ZONED_DATE_TIME_COMPARE_FILES) != 46:
    raise RuntimeError("Temporal.ZonedDateTime compare admission must contain 46 files")
