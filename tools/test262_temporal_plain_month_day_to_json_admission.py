"""Exact Test262 coverage for Temporal.PlainMonthDay.prototype.toJSON."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_month_day_to_json_admission.txt"
)
TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    name = Path(path).name
    if name == "branding.js":
        features.add("Symbol")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    if name in {"length.js", "name.js", "prop-desc.js"}:
        return frozenset({"propertyHelper.js"})
    if name == "not-a-constructor.js":
        return frozenset({"isConstructor.js"})
    return frozenset()


TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_FILES
}
TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_FILES
}
TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_FILES
}
TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_FILES
}

if len(TEMPORAL_PLAIN_MONTH_DAY_TO_JSON_FILES) != 7:
    raise RuntimeError("PlainMonthDay.prototype.toJSON admission must contain 7 files")
