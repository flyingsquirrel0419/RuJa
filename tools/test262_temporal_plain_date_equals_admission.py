"""Exact Test262 coverage for Temporal.PlainDate.prototype.equals."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_plain_date_equals_admission.txt")
TEMPORAL_PLAIN_DATE_EQUALS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_BLOCKER = (
    "built-ins/Temporal/PlainDate/prototype/equals/calendar-temporal-object.js"
)
TEMPORAL_PLAIN_DATE_EQUALS_BLOCKER_FEATURES = {
    _BLOCKER: frozenset({"Temporal"})
}
TEMPORAL_PLAIN_DATE_EQUALS_BLOCKER_INCLUDES = {
    _BLOCKER: frozenset({"compareArray.js", "temporalHelpers.js"})
}
TEMPORAL_PLAIN_DATE_EQUALS_BLOCKER_FLAGS = {_BLOCKER: frozenset()}
TEMPORAL_PLAIN_DATE_EQUALS_BLOCKER_NEGATIVE = {_BLOCKER: None}

_DOWNSTREAM_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_equals_downstream.txt"
)
TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_FILES = frozenset(
    line
    for raw_line in _DOWNSTREAM_MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_FEATURES = {
    path: frozenset({"Temporal"})
    for path in TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_FILES
}
_DOWNSTREAM_TEMPORAL_HELPERS = frozenset({
    "intl402/Temporal/PlainDate/prototype/monthCode/chinese-calendar-dates.js",
    "intl402/Temporal/PlainDate/prototype/monthCode/dangi-calendar-dates.js",
})
TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_INCLUDES = {
    path: (
        frozenset({"temporalHelpers.js"})
        if path in _DOWNSTREAM_TEMPORAL_HELPERS
        else frozenset()
    )
    for path in TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_FILES
}
TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_FILES
}
TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_FILES
}

_ARROW_FUNCTION = frozenset({
    "built-ins/Temporal/PlainDate/prototype/equals/argument-propertybag-calendar-year-zero.js",
    "built-ins/Temporal/PlainDate/prototype/equals/argument-string-invalid.js",
    "built-ins/Temporal/PlainDate/prototype/equals/argument-string-with-utc-designator.js",
    "built-ins/Temporal/PlainDate/prototype/equals/year-zero.js",
})
_BIGINT_SYMBOL = frozenset({
    "built-ins/Temporal/PlainDate/prototype/equals/argument-propertybag-calendar-wrong-type.js",
    "built-ins/Temporal/PlainDate/prototype/equals/argument-wrong-type.js",
})
_SYMBOL = frozenset({
    "built-ins/Temporal/PlainDate/prototype/equals/branding.js",
})
_REFLECT_CONSTRUCT = frozenset({
    "built-ins/Temporal/PlainDate/prototype/equals/not-a-constructor.js",
})
_TEMPORAL_HELPERS = frozenset({
    "built-ins/Temporal/PlainDate/prototype/equals/argument-plaindatetime.js",
    "built-ins/Temporal/PlainDate/prototype/equals/infinity-throws-rangeerror.js",
})
_COMPARE_ARRAY = frozenset({
    "built-ins/Temporal/PlainDate/prototype/equals/argument-plaindatetime.js",
    "built-ins/Temporal/PlainDate/prototype/equals/argument-zoneddatetime-slots.js",
    "built-ins/Temporal/PlainDate/prototype/equals/infinity-throws-rangeerror.js",
})
_PROPERTY_HELPER = frozenset({
    "built-ins/Temporal/PlainDate/prototype/equals/length.js",
    "built-ins/Temporal/PlainDate/prototype/equals/name.js",
    "built-ins/Temporal/PlainDate/prototype/equals/prop-desc.js",
})
_IS_CONSTRUCTOR = frozenset({
    "built-ins/Temporal/PlainDate/prototype/equals/not-a-constructor.js",
})


def _features(path):
    features = {"Temporal"}
    if path in _ARROW_FUNCTION:
        features.add("arrow-function")
    if path in _BIGINT_SYMBOL:
        features.update({"BigInt", "Symbol"})
    if path in _SYMBOL:
        features.add("Symbol")
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


TEMPORAL_PLAIN_DATE_EQUALS_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_EQUALS_FILES
}
TEMPORAL_PLAIN_DATE_EQUALS_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_DATE_EQUALS_FILES
}
TEMPORAL_PLAIN_DATE_EQUALS_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_EQUALS_FILES
}
TEMPORAL_PLAIN_DATE_EQUALS_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_EQUALS_FILES
}

if len(TEMPORAL_PLAIN_DATE_EQUALS_FILES) != 39:
    raise RuntimeError("Temporal.PlainDate.prototype.equals admission must contain 39 files")
if len(TEMPORAL_PLAIN_DATE_EQUALS_DOWNSTREAM_FILES) != 4:
    raise RuntimeError("Temporal.PlainDate.prototype.equals downstream must contain four files")
