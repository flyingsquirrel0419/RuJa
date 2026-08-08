"""Exact Test262 coverage for Temporal.PlainDate.prototype.toJSON."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_to_json_admission.txt"
)
TEMPORAL_PLAIN_DATE_TO_JSON_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_SYMBOL = "built-ins/Temporal/PlainDate/prototype/toJSON/branding.js"
_REFLECT_CONSTRUCT = (
    "built-ins/Temporal/PlainDate/prototype/toJSON/not-a-constructor.js"
)
_PROPERTY_HELPER = frozenset({
    "built-ins/Temporal/PlainDate/prototype/toJSON/length.js",
    "built-ins/Temporal/PlainDate/prototype/toJSON/name.js",
    "built-ins/Temporal/PlainDate/prototype/toJSON/prop-desc.js",
})


def _features(path):
    features = {"Temporal"}
    if path == _SYMBOL:
        features.add("Symbol")
    if path == _REFLECT_CONSTRUCT:
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    if path in _PROPERTY_HELPER:
        return frozenset({"propertyHelper.js"})
    if path == _REFLECT_CONSTRUCT:
        return frozenset({"isConstructor.js"})
    return frozenset()


TEMPORAL_PLAIN_DATE_TO_JSON_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TO_JSON_FILES
}
TEMPORAL_PLAIN_DATE_TO_JSON_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_DATE_TO_JSON_FILES
}
TEMPORAL_PLAIN_DATE_TO_JSON_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_TO_JSON_FILES
}
TEMPORAL_PLAIN_DATE_TO_JSON_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TO_JSON_FILES
}
TEMPORAL_PLAIN_DATE_TO_JSON_BLOCKER_FEATURES = {}
TEMPORAL_PLAIN_DATE_TO_JSON_BLOCKER_INCLUDES = {}
TEMPORAL_PLAIN_DATE_TO_JSON_BLOCKER_FLAGS = {}
TEMPORAL_PLAIN_DATE_TO_JSON_BLOCKER_NEGATIVE = {}

if len(TEMPORAL_PLAIN_DATE_TO_JSON_FILES) != 8:
    raise RuntimeError("Temporal.PlainDate.prototype.toJSON admission must contain 8 files")
