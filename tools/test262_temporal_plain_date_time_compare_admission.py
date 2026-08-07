"""Exact Test262 coverage for Temporal.PlainDateTime.compare."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_time_compare_admission.txt"
)
TEMPORAL_PLAIN_DATE_TIME_COMPARE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    if path.endswith((
        "/argument-propertybag-calendar-wrong-type.js",
        "/argument-wrong-type.js",
    )):
        features.update(("BigInt", "Symbol"))
    if path.endswith((
        "/argument-propertybag-calendar-year-zero.js",
        "/argument-string-with-utc-designator.js",
    )):
        features.add("arrow-function")
    if path.endswith("/not-a-constructor.js"):
        features.add("Reflect.construct")
    return frozenset(features)


TEMPORAL_PLAIN_DATE_TIME_COMPARE_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TIME_COMPARE_FILES
}

if len(TEMPORAL_PLAIN_DATE_TIME_COMPARE_FILES) != 40:
    raise RuntimeError("Temporal.PlainDateTime.compare admission must contain 40 files")
