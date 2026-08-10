"""Exact relativeTo total boundary and external-fixture complement."""

from pathlib import Path


def _read_manifest(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_DURATION_TOTAL_FILES = _read_manifest(
    "test262_temporal_duration_total_admission.txt"
)
TEMPORAL_DURATION_TOTAL_FALSE_POSITIVES = _read_manifest(
    "test262_temporal_duration_total_false_positives.txt"
)
TEMPORAL_DURATION_TOTAL_BLOCKERS = _read_manifest(
    "test262_temporal_duration_total_blockers.txt"
)
TEMPORAL_DURATION_TOTAL_ALL_FILES = (
    TEMPORAL_DURATION_TOTAL_FILES
    | TEMPORAL_DURATION_TOTAL_FALSE_POSITIVES
    | TEMPORAL_DURATION_TOTAL_BLOCKERS
)

_BIGINT = frozenset({
    "built-ins/Temporal/Duration/prototype/total/options-wrong-type.js",
    "built-ins/Temporal/Duration/prototype/total/precision-exact-mathematical-values-5.js",
    "built-ins/Temporal/Duration/prototype/total/relativeto-propertybag-calendar-wrong-type.js",
    "built-ins/Temporal/Duration/prototype/total/relativeto-propertybag-timezone-wrong-type.js",
    "built-ins/Temporal/Duration/prototype/total/relativeto-wrong-type.js",
})
_SYMBOL = frozenset({
    "built-ins/Temporal/Duration/prototype/total/branding.js",
    "built-ins/Temporal/Duration/prototype/total/options-wrong-type.js",
    "built-ins/Temporal/Duration/prototype/total/relativeto-propertybag-calendar-wrong-type.js",
    "built-ins/Temporal/Duration/prototype/total/relativeto-propertybag-timezone-wrong-type.js",
    "built-ins/Temporal/Duration/prototype/total/relativeto-wrong-type.js",
})
_ARROW = frozenset({
    "built-ins/Temporal/Duration/prototype/total/relativeto-propertybag-timezone-string-year-zero.js",
    "built-ins/Temporal/Duration/prototype/total/relativeto-undefined-throw-on-calendar-units.js",
    "built-ins/Temporal/Duration/prototype/total/unit-disallowed-units-string.js",
    "built-ins/Temporal/Duration/prototype/total/unit-plurals-accepted-string.js",
    "built-ins/Temporal/Duration/prototype/total/unit-string-shorthand-string.js",
    "built-ins/Temporal/Duration/prototype/total/year-zero.js",
})
_PROPERTY_HELPER = frozenset({
    "built-ins/Temporal/Duration/prototype/total/length.js",
    "built-ins/Temporal/Duration/prototype/total/name.js",
    "built-ins/Temporal/Duration/prototype/total/prop-desc.js",
})
_COMPARE_ARRAY = frozenset({
    "built-ins/Temporal/Duration/prototype/total/calendar-temporal-object.js",
    "built-ins/Temporal/Duration/prototype/total/options-read-before-algorithmic-validation.js",
    "built-ins/Temporal/Duration/prototype/total/order-of-operations.js",
    "built-ins/Temporal/Duration/prototype/total/relativeto-infinity-throws-rangeerror.js",
    "built-ins/Temporal/Duration/prototype/total/unit-wrong-type.js",
})
_TEMPORAL_HELPERS = _COMPARE_ARRAY | frozenset({
    "built-ins/Temporal/Duration/prototype/total/unit-plurals-accepted-string.js",
    "built-ins/Temporal/Duration/prototype/total/unit-plurals-accepted.js",
})
_IS_CONSTRUCTOR = frozenset({
    "built-ins/Temporal/Duration/prototype/total/not-a-constructor.js",
})


def _features(path):
    features = {"Temporal"}
    if path in _BIGINT:
        features.add("BigInt")
    if path in _SYMBOL:
        features.add("Symbol")
    if path in _ARROW:
        features.add("arrow-function")
    if path in _IS_CONSTRUCTOR:
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    includes = set()
    if path in _PROPERTY_HELPER:
        includes.add("propertyHelper.js")
    if path in _COMPARE_ARRAY:
        includes.add("compareArray.js")
    if path in _TEMPORAL_HELPERS:
        includes.add("temporalHelpers.js")
    if path in _IS_CONSTRUCTOR:
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_DURATION_TOTAL_ALL_FEATURES = {
    path: _features(path) for path in TEMPORAL_DURATION_TOTAL_ALL_FILES
}
TEMPORAL_DURATION_TOTAL_ALL_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_DURATION_TOTAL_ALL_FILES
}
TEMPORAL_DURATION_TOTAL_ALL_FLAGS = {
    path: frozenset() for path in TEMPORAL_DURATION_TOTAL_ALL_FILES
}
TEMPORAL_DURATION_TOTAL_ALL_NEGATIVE = {
    path: None for path in TEMPORAL_DURATION_TOTAL_ALL_FILES
}
TEMPORAL_DURATION_TOTAL_FEATURES = {
    path: TEMPORAL_DURATION_TOTAL_ALL_FEATURES[path]
    for path in TEMPORAL_DURATION_TOTAL_FILES
}
TEMPORAL_DURATION_TOTAL_INCLUDES = {
    path: TEMPORAL_DURATION_TOTAL_ALL_INCLUDES[path]
    for path in TEMPORAL_DURATION_TOTAL_FILES
}
TEMPORAL_DURATION_TOTAL_FLAGS = {
    path: TEMPORAL_DURATION_TOTAL_ALL_FLAGS[path]
    for path in TEMPORAL_DURATION_TOTAL_FILES
}
TEMPORAL_DURATION_TOTAL_NEGATIVE = {
    path: TEMPORAL_DURATION_TOTAL_ALL_NEGATIVE[path]
    for path in TEMPORAL_DURATION_TOTAL_FILES
}

if len(TEMPORAL_DURATION_TOTAL_FILES) != 77:
    raise RuntimeError("Temporal.Duration.prototype.total admission must contain 77 files")
if TEMPORAL_DURATION_TOTAL_FALSE_POSITIVES:
    raise RuntimeError("Temporal.Duration.prototype.total false positives must be empty")
if len(TEMPORAL_DURATION_TOTAL_BLOCKERS) != 1:
    raise RuntimeError("Temporal.Duration.prototype.total blockers must contain 1 file")
if len(TEMPORAL_DURATION_TOTAL_ALL_FILES) != 78:
    raise RuntimeError("Temporal.Duration.prototype.total surface must contain 78 files")
