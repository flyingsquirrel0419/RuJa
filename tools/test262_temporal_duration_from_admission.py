"""Exact Test262 boundary for Temporal.Duration.from."""

from pathlib import Path


def _read_manifest(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_DURATION_FROM_FILES = _read_manifest(
    "test262_temporal_duration_from_admission.txt"
)
TEMPORAL_DURATION_FROM_BLOCKERS = _read_manifest(
    "test262_temporal_duration_from_blockers.txt"
)
TEMPORAL_DURATION_FROM_ALL_FILES = (
    TEMPORAL_DURATION_FROM_FILES | TEMPORAL_DURATION_FROM_BLOCKERS
)

_REFLECT_CONSTRUCT = frozenset({
    "built-ins/Temporal/Duration/from/not-a-constructor.js",
})
_TEMPORAL_HELPERS = frozenset({
    "built-ins/Temporal/Duration/from/argument-duration.js",
    "built-ins/Temporal/Duration/from/argument-existing-object.js",
    "built-ins/Temporal/Duration/from/argument-propertybag-optional-properties.js",
    "built-ins/Temporal/Duration/from/argument-propertybag.js",
    "built-ins/Temporal/Duration/from/argument-string-fractional-precision.js",
    "built-ins/Temporal/Duration/from/argument-string-fractional-units-rounding-mode.js",
    "built-ins/Temporal/Duration/from/argument-string-negative-fractional-units.js",
    "built-ins/Temporal/Duration/from/argument-string.js",
    "built-ins/Temporal/Duration/from/blank-duration.js",
    "built-ins/Temporal/Duration/from/lower-limit.js",
    "built-ins/Temporal/Duration/from/order-of-operations.js",
    "built-ins/Temporal/Duration/from/string-with-skipped-units.js",
    "built-ins/Temporal/Duration/from/subclassing-ignored.js",
})
_COMPARE_ARRAY = frozenset({
    "built-ins/Temporal/Duration/from/order-of-operations.js",
})
_PROPERTY_HELPER = frozenset({
    "built-ins/Temporal/Duration/from/length.js",
    "built-ins/Temporal/Duration/from/name.js",
    "built-ins/Temporal/Duration/from/prop-desc.js",
})
_IS_CONSTRUCTOR = frozenset({
    "built-ins/Temporal/Duration/from/not-a-constructor.js",
})


def _features(path):
    features = {"Temporal"}
    if path in _REFLECT_CONSTRUCT:
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    includes = set()
    if path in _TEMPORAL_HELPERS:
        includes.add("temporalHelpers.js")
    if path in _COMPARE_ARRAY:
        includes.add("compareArray.js")
    if path in _PROPERTY_HELPER:
        includes.add("propertyHelper.js")
    if path in _IS_CONSTRUCTOR:
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_DURATION_FROM_ALL_FEATURES = {
    path: _features(path) for path in TEMPORAL_DURATION_FROM_ALL_FILES
}
TEMPORAL_DURATION_FROM_ALL_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_DURATION_FROM_ALL_FILES
}
TEMPORAL_DURATION_FROM_ALL_FLAGS = {
    path: frozenset() for path in TEMPORAL_DURATION_FROM_ALL_FILES
}
TEMPORAL_DURATION_FROM_ALL_NEGATIVE = {
    path: None for path in TEMPORAL_DURATION_FROM_ALL_FILES
}
TEMPORAL_DURATION_FROM_FEATURES = {
    path: TEMPORAL_DURATION_FROM_ALL_FEATURES[path]
    for path in TEMPORAL_DURATION_FROM_FILES
}
TEMPORAL_DURATION_FROM_BLOCKER_FEATURES = {
    path: TEMPORAL_DURATION_FROM_ALL_FEATURES[path]
    for path in TEMPORAL_DURATION_FROM_BLOCKERS
}
TEMPORAL_DURATION_FROM_BLOCKER_INCLUDES = {
    path: TEMPORAL_DURATION_FROM_ALL_INCLUDES[path]
    for path in TEMPORAL_DURATION_FROM_BLOCKERS
}
TEMPORAL_DURATION_FROM_BLOCKER_FLAGS = {
    path: TEMPORAL_DURATION_FROM_ALL_FLAGS[path]
    for path in TEMPORAL_DURATION_FROM_BLOCKERS
}
TEMPORAL_DURATION_FROM_BLOCKER_NEGATIVE = {
    path: TEMPORAL_DURATION_FROM_ALL_NEGATIVE[path]
    for path in TEMPORAL_DURATION_FROM_BLOCKERS
}

if len(TEMPORAL_DURATION_FROM_FILES) != 31:
    raise RuntimeError("Temporal.Duration.from admission must contain 31 files")
if TEMPORAL_DURATION_FROM_BLOCKERS:
    raise RuntimeError("Temporal.Duration.from blockers must be empty")
if len(TEMPORAL_DURATION_FROM_ALL_FILES) != 31:
    raise RuntimeError("Temporal.Duration.from surface must contain 31 files")
