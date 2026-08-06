"""Exact Test262 coverage for the Temporal.PlainDateTime hidden-slot core."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_time_core_admission.txt"
)
TEMPORAL_PLAIN_DATE_TIME_CORE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    if path.endswith("/branding.js"):
        features.add("Symbol")
    if path.endswith("/calendar-wrong-type.js"):
        features.update({"BigInt", "Symbol"})
    if path.endswith("/valueOf/not-a-constructor.js"):
        features.add("Reflect.construct")
    return frozenset(features)


TEMPORAL_PLAIN_DATE_TIME_CORE_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TIME_CORE_FILES
}

if len(TEMPORAL_PLAIN_DATE_TIME_CORE_FILES) != 101:
    raise RuntimeError("Temporal.PlainDateTime core admission must contain 101 files")
