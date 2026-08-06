"""Exact Test262 coverage for Temporal.ZonedDateTime.prototype.equals."""

from pathlib import Path

_MANIFEST = Path(__file__).with_name(
    "test262_temporal_zoned_date_time_equals_admission.txt"
)
TEMPORAL_ZONED_DATE_TIME_EQUALS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_ARROW_FUNCTION_FILES = frozenset(
    path for path in TEMPORAL_ZONED_DATE_TIME_EQUALS_FILES if "year-zero" in path
)


def _features(path):
    features = {"Temporal"}
    if path.endswith("/argument-wrong-type.js") or path.endswith(
        (
            "/argument-propertybag-calendar-wrong-type.js",
            "/argument-propertybag-timezone-wrong-type.js",
        )
    ):
        features.update({"BigInt", "Symbol"})
    if path.endswith("/branding.js"):
        features.add("Symbol")
    if path.endswith("/not-a-constructor.js"):
        features.add("Reflect.construct")
    if path in _ARROW_FUNCTION_FILES:
        features.add("arrow-function")
    return frozenset(features)


TEMPORAL_ZONED_DATE_TIME_EQUALS_FEATURES = {
    path: _features(path) for path in TEMPORAL_ZONED_DATE_TIME_EQUALS_FILES
}

if len(TEMPORAL_ZONED_DATE_TIME_EQUALS_FILES) != 54:
    raise RuntimeError("Temporal.ZonedDateTime equals admission must contain 54 files")
