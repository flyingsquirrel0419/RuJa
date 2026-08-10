"""Exact Test262 boundary for Temporal.Duration.prototype.toJSON."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_duration_to_json_admission.txt"
)
TEMPORAL_DURATION_TO_JSON_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_SYMBOL = "built-ins/Temporal/Duration/prototype/toJSON/branding.js"
_REFLECT_CONSTRUCT = (
    "built-ins/Temporal/Duration/prototype/toJSON/not-a-constructor.js"
)
_PROPERTY_HELPER = frozenset({
    "built-ins/Temporal/Duration/prototype/toJSON/length.js",
    "built-ins/Temporal/Duration/prototype/toJSON/name.js",
    "built-ins/Temporal/Duration/prototype/toJSON/prop-desc.js",
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


TEMPORAL_DURATION_TO_JSON_FEATURES = {
    path: _features(path) for path in TEMPORAL_DURATION_TO_JSON_FILES
}
TEMPORAL_DURATION_TO_JSON_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_DURATION_TO_JSON_FILES
}
TEMPORAL_DURATION_TO_JSON_FLAGS = {
    path: frozenset() for path in TEMPORAL_DURATION_TO_JSON_FILES
}
TEMPORAL_DURATION_TO_JSON_NEGATIVE = {
    path: None for path in TEMPORAL_DURATION_TO_JSON_FILES
}

if len(TEMPORAL_DURATION_TO_JSON_FILES) != 12:
    raise RuntimeError(
        "Temporal.Duration.prototype.toJSON admission must contain 12 files"
    )
