"""Exact Test262 coverage for Temporal.PlainDateTime.prototype.equals."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_time_equals_admission.txt"
)
TEMPORAL_PLAIN_DATE_TIME_EQUALS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    if path.endswith((
        "/argument-propertybag-calendar-wrong-type.js",
        "/argument-wrong-type.js",
        "/branding.js",
    )):
        features.add("Symbol")
    if path.endswith((
        "/argument-propertybag-calendar-wrong-type.js",
        "/argument-wrong-type.js",
    )):
        features.add("BigInt")
    if path.endswith((
        "/argument-propertybag-calendar-year-zero.js",
        "/argument-string-with-utc-designator.js",
        "/year-zero.js",
    )):
        features.add("arrow-function")
    if path.endswith("/not-a-constructor.js"):
        features.add("Reflect.construct")
    return frozenset(features)


TEMPORAL_PLAIN_DATE_TIME_EQUALS_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TIME_EQUALS_FILES
}

if len(TEMPORAL_PLAIN_DATE_TIME_EQUALS_FILES) != 39:
    raise RuntimeError(
        "Temporal.PlainDateTime.prototype.equals admission must contain 39 files"
    )
