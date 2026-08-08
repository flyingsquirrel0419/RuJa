"""Exact Test262 coverage for Temporal.PlainDate.prototype.toString."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_to_string_admission.txt"
)
TEMPORAL_PLAIN_DATE_TO_STRING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_SYMBOL = frozenset({
    "built-ins/Temporal/PlainDate/prototype/toString/branding.js",
    "built-ins/Temporal/PlainDate/prototype/toString/options-wrong-type.js",
})
_BIGINT = frozenset({
    "built-ins/Temporal/PlainDate/prototype/toString/options-wrong-type.js",
})
_REFLECT_CONSTRUCT = frozenset({
    "built-ins/Temporal/PlainDate/prototype/toString/not-a-constructor.js",
})
_COMPARE_ARRAY_TEMPORAL_HELPERS = frozenset({
    "built-ins/Temporal/PlainDate/prototype/toString/calendarname-wrong-type.js",
    "built-ins/Temporal/PlainDate/prototype/toString/order-of-operations.js",
})
_PROPERTY_HELPER = frozenset({
    "built-ins/Temporal/PlainDate/prototype/toString/length.js",
    "built-ins/Temporal/PlainDate/prototype/toString/name.js",
    "built-ins/Temporal/PlainDate/prototype/toString/prop-desc.js",
})
_IS_CONSTRUCTOR = frozenset({
    "built-ins/Temporal/PlainDate/prototype/toString/not-a-constructor.js",
})


def _features(path):
    features = {"Temporal"}
    if path in _SYMBOL:
        features.add("Symbol")
    if path in _BIGINT:
        features.add("BigInt")
    if path in _REFLECT_CONSTRUCT:
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    if path in _COMPARE_ARRAY_TEMPORAL_HELPERS:
        return frozenset({"compareArray.js", "temporalHelpers.js"})
    if path in _PROPERTY_HELPER:
        return frozenset({"propertyHelper.js"})
    if path in _IS_CONSTRUCTOR:
        return frozenset({"isConstructor.js"})
    return frozenset()


TEMPORAL_PLAIN_DATE_TO_STRING_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TO_STRING_FILES
}
TEMPORAL_PLAIN_DATE_TO_STRING_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_DATE_TO_STRING_FILES
}
TEMPORAL_PLAIN_DATE_TO_STRING_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_TO_STRING_FILES
}
TEMPORAL_PLAIN_DATE_TO_STRING_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TO_STRING_FILES
}
TEMPORAL_PLAIN_DATE_TO_STRING_BLOCKER_FEATURES = {}
TEMPORAL_PLAIN_DATE_TO_STRING_BLOCKER_INCLUDES = {}
TEMPORAL_PLAIN_DATE_TO_STRING_BLOCKER_FLAGS = {}
TEMPORAL_PLAIN_DATE_TO_STRING_BLOCKER_NEGATIVE = {}

if len(TEMPORAL_PLAIN_DATE_TO_STRING_FILES) != 18:
    raise RuntimeError("Temporal.PlainDate.prototype.toString admission must contain 18 files")
