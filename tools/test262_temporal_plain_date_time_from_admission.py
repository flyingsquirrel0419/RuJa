"""Exact Test262 coverage for Temporal.PlainDateTime.from."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_time_from_admission.txt"
)
TEMPORAL_PLAIN_DATE_TIME_FROM_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    if path.endswith((
        "/argument-propertybag-calendar-wrong-type.js",
        "/argument-wrong-type.js",
        "/options-wrong-type.js",
    )):
        features.update({"BigInt", "Symbol"})
    if path.endswith((
        "/argument-propertybag-calendar-year-zero.js",
        "/argument-string-with-utc-designator.js",
        "/year-zero.js",
    )):
        features.add("arrow-function")
    if path.endswith((
        "/roundtrip-from-property-bag.js",
        "/roundtrip-from-string.js",
    )):
        features.add("Intl.Era-monthcode")
    if path.endswith("/not-a-constructor.js"):
        features.add("Reflect.construct")
    return frozenset(features)


TEMPORAL_PLAIN_DATE_TIME_FROM_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TIME_FROM_FILES
}

if len(TEMPORAL_PLAIN_DATE_TIME_FROM_FILES) != 69:
    raise RuntimeError("Temporal.PlainDateTime.from admission must contain 69 files")
