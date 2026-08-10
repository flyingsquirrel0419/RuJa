"""Exact Test262 coverage for Temporal.PlainDate.compare."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_plain_date_compare_admission.txt")
TEMPORAL_PLAIN_DATE_COMPARE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_ARROW_FUNCTION = frozenset({
    'built-ins/Temporal/PlainDate/compare/argument-propertybag-calendar-year-zero.js',
    'built-ins/Temporal/PlainDate/compare/argument-string-invalid.js',
    'built-ins/Temporal/PlainDate/compare/argument-string-with-utc-designator.js',
})

_BIGINT_SYMBOL = frozenset({
    'built-ins/Temporal/PlainDate/compare/argument-propertybag-calendar-wrong-type.js',
    'built-ins/Temporal/PlainDate/compare/argument-wrong-type.js',
})

_REFLECT_CONSTRUCT = frozenset({
    'built-ins/Temporal/PlainDate/compare/not-a-constructor.js',
})

_TEMPORAL_HELPERS = frozenset({
    'built-ins/Temporal/PlainDate/compare/calendar-temporal-object.js',
    'built-ins/Temporal/PlainDate/compare/argument-plaindatetime.js',
    'built-ins/Temporal/PlainDate/compare/infinity-throws-rangeerror.js',
})

_COMPARE_ARRAY = frozenset({
    'built-ins/Temporal/PlainDate/compare/calendar-temporal-object.js',
    'built-ins/Temporal/PlainDate/compare/argument-plaindatetime.js',
    'built-ins/Temporal/PlainDate/compare/argument-zoneddatetime-slots.js',
    'built-ins/Temporal/PlainDate/compare/infinity-throws-rangeerror.js',
})

_PROPERTY_HELPER = frozenset({
    'built-ins/Temporal/PlainDate/compare/length.js',
    'built-ins/Temporal/PlainDate/compare/name.js',
    'built-ins/Temporal/PlainDate/compare/prop-desc.js',
})

_IS_CONSTRUCTOR = frozenset({
    'built-ins/Temporal/PlainDate/compare/not-a-constructor.js',
})

def _features(path):
    features = {"Temporal"}
    if path in _ARROW_FUNCTION:
        features.add("arrow-function")
    if path in _BIGINT_SYMBOL:
        features.update({"BigInt", "Symbol"})
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


TEMPORAL_PLAIN_DATE_COMPARE_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_COMPARE_FILES
}
TEMPORAL_PLAIN_DATE_COMPARE_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_DATE_COMPARE_FILES
}
TEMPORAL_PLAIN_DATE_COMPARE_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_COMPARE_FILES
}
TEMPORAL_PLAIN_DATE_COMPARE_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_COMPARE_FILES
}

if len(TEMPORAL_PLAIN_DATE_COMPARE_FILES) != 42:
    raise RuntimeError("Temporal.PlainDate.compare admission must contain 42 files")
