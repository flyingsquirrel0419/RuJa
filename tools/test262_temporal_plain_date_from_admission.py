"""Exact Test262 coverage for Temporal.PlainDate.from."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_plain_date_from_admission.txt")
TEMPORAL_PLAIN_DATE_FROM_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_ARROW_FUNCTION = frozenset({
    'built-ins/Temporal/PlainDate/from/argument-propertybag-calendar-year-zero.js',
    'built-ins/Temporal/PlainDate/from/argument-string-invalid.js',
    'built-ins/Temporal/PlainDate/from/argument-string-trailing-junk.js',
    'built-ins/Temporal/PlainDate/from/argument-string-with-utc-designator.js',
    'built-ins/Temporal/PlainDate/from/out-of-range.js',
    'built-ins/Temporal/PlainDate/from/year-zero.js',
})

_BIGINT_SYMBOL = frozenset({
    'built-ins/Temporal/PlainDate/from/argument-propertybag-calendar-wrong-type.js',
    'built-ins/Temporal/PlainDate/from/argument-wrong-type.js',
    'built-ins/Temporal/PlainDate/from/options-wrong-type.js',
})

_INTL_ERA_MONTHCODE = frozenset({
    'built-ins/Temporal/PlainDate/from/roundtrip-from-iso.js',
    'built-ins/Temporal/PlainDate/from/roundtrip-from-property-bag.js',
    'built-ins/Temporal/PlainDate/from/roundtrip-from-string.js',
})

_REFLECT_CONSTRUCT = frozenset({
    'built-ins/Temporal/PlainDate/from/not-a-constructor.js',
})

_TEMPORAL_HELPERS = frozenset({
    'built-ins/Temporal/PlainDate/from/argument-leap-second.js',
    'built-ins/Temporal/PlainDate/from/argument-object-invalid.js',
    'built-ins/Temporal/PlainDate/from/argument-object-valid.js',
    'built-ins/Temporal/PlainDate/from/argument-plaindate.js',
    'built-ins/Temporal/PlainDate/from/argument-plaindatetime.js',
    'built-ins/Temporal/PlainDate/from/argument-propertybag-calendar-case-insensitive.js',
    'built-ins/Temporal/PlainDate/from/argument-propertybag-calendar-iso-string.js',
    'built-ins/Temporal/PlainDate/from/argument-propertybag-calendar-leap-second.js',
    'built-ins/Temporal/PlainDate/from/argument-propertybag-calendar-string.js',
    'built-ins/Temporal/PlainDate/from/argument-propertybag-calendar.js',
    'built-ins/Temporal/PlainDate/from/argument-string-calendar-annotation.js',
    'built-ins/Temporal/PlainDate/from/argument-string-date-with-utc-offset.js',
    'built-ins/Temporal/PlainDate/from/argument-string-time-separators.js',
    'built-ins/Temporal/PlainDate/from/argument-string-time-zone-annotation.js',
    'built-ins/Temporal/PlainDate/from/argument-string-unknown-annotation.js',
    'built-ins/Temporal/PlainDate/from/argument-string.js',
    'built-ins/Temporal/PlainDate/from/argument-zoneddatetime.js',
    'built-ins/Temporal/PlainDate/from/infinity-throws-rangeerror.js',
    'built-ins/Temporal/PlainDate/from/limits.js',
    'built-ins/Temporal/PlainDate/from/observable-get-overflow-argument-primitive.js',
    'built-ins/Temporal/PlainDate/from/observable-get-overflow-argument-string-invalid.js',
    'built-ins/Temporal/PlainDate/from/one-of-era-erayear-undefined.js',
    'built-ins/Temporal/PlainDate/from/options-object.js',
    'built-ins/Temporal/PlainDate/from/options-read-before-algorithmic-validation.js',
    'built-ins/Temporal/PlainDate/from/order-of-operations.js',
    'built-ins/Temporal/PlainDate/from/overflow-undefined.js',
    'built-ins/Temporal/PlainDate/from/overflow-wrong-type.js',
    'built-ins/Temporal/PlainDate/from/roundtrip-from-iso.js',
    'built-ins/Temporal/PlainDate/from/roundtrip-from-property-bag.js',
    'built-ins/Temporal/PlainDate/from/roundtrip-from-string.js',
    'built-ins/Temporal/PlainDate/from/subclassing-ignored.js',
    'built-ins/Temporal/PlainDate/from/with-year-month-day-need-constrain.js',
    'built-ins/Temporal/PlainDate/from/with-year-month-day.js',
    'built-ins/Temporal/PlainDate/from/with-year-monthCode-day-need-constrain.js',
    'built-ins/Temporal/PlainDate/from/with-year-monthCode-day.js',
})

_COMPARE_ARRAY = frozenset({
    'built-ins/Temporal/PlainDate/from/argument-plaindatetime.js',
    'built-ins/Temporal/PlainDate/from/argument-zoneddatetime-slots.js',
    'built-ins/Temporal/PlainDate/from/infinity-throws-rangeerror.js',
    'built-ins/Temporal/PlainDate/from/observable-get-overflow-argument-primitive.js',
    'built-ins/Temporal/PlainDate/from/observable-get-overflow-argument-string-invalid.js',
    'built-ins/Temporal/PlainDate/from/options-read-before-algorithmic-validation.js',
    'built-ins/Temporal/PlainDate/from/order-of-operations.js',
    'built-ins/Temporal/PlainDate/from/overflow-wrong-type.js',
})

_PROPERTY_HELPER = frozenset({
    'built-ins/Temporal/PlainDate/from/length.js',
    'built-ins/Temporal/PlainDate/from/name.js',
    'built-ins/Temporal/PlainDate/from/prop-desc.js',
})

_IS_CONSTRUCTOR = frozenset({
    'built-ins/Temporal/PlainDate/from/not-a-constructor.js',
})

def _features(path):
    features = {"Temporal"}
    if path in _ARROW_FUNCTION:
        features.add("arrow-function")
    if path in _BIGINT_SYMBOL:
        features.update({"BigInt", "Symbol"})
    if path in _INTL_ERA_MONTHCODE:
        features.add("Intl.Era-monthcode")
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


TEMPORAL_PLAIN_DATE_FROM_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_FROM_FILES
}
TEMPORAL_PLAIN_DATE_FROM_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_DATE_FROM_FILES
}
TEMPORAL_PLAIN_DATE_FROM_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_FROM_FILES
}
TEMPORAL_PLAIN_DATE_FROM_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_FROM_FILES
}

if len(TEMPORAL_PLAIN_DATE_FROM_FILES) != 70:
    raise RuntimeError("Temporal.PlainDate.from admission must contain 70 files")
