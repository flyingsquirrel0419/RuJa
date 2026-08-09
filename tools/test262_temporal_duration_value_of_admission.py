"""Exact Test262 boundary for Temporal.Duration.prototype.valueOf."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_duration_value_of_admission.txt"
)
TEMPORAL_DURATION_VALUE_OF_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
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


TEMPORAL_DURATION_VALUE_OF_FEATURES = {
    path: _features(path) for path in TEMPORAL_DURATION_VALUE_OF_FILES
}
TEMPORAL_DURATION_VALUE_OF_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_DURATION_VALUE_OF_FILES
}
TEMPORAL_DURATION_VALUE_OF_FLAGS = {
    path: frozenset() for path in TEMPORAL_DURATION_VALUE_OF_FILES
}
TEMPORAL_DURATION_VALUE_OF_NEGATIVE = {
    path: None for path in TEMPORAL_DURATION_VALUE_OF_FILES
}

if len(TEMPORAL_DURATION_VALUE_OF_FILES) != 7:
    raise RuntimeError(
        "Temporal.Duration.prototype.valueOf admission must contain 7 files"
    )
