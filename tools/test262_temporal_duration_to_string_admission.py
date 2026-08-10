"""Exact Test262 boundary for Temporal.Duration.prototype.toString."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_duration_to_string_admission.txt"
)
TEMPORAL_DURATION_TO_STRING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_SYMBOL = frozenset({"branding.js", "options-wrong-type.js"})
_BIGINT = frozenset({"options-wrong-type.js"})
_REFLECT_CONSTRUCT = frozenset({"not-a-constructor.js"})
_COMPARE_ARRAY = frozenset({
    "fractionalseconddigits-wrong-type.js",
    "options-read-before-algorithmic-validation.js",
    "order-of-operations.js",
    "roundingmode-wrong-type.js",
    "smallestunit-wrong-type.js",
})
_TEMPORAL_HELPERS = frozenset({
    "fractionalseconddigits-wrong-type.js",
    "options-read-before-algorithmic-validation.js",
    "order-of-operations.js",
    "roundingmode-wrong-type.js",
    "smallestunit-plurals-accepted.js",
    "smallestunit-wrong-type.js",
})
_PROPERTY_HELPER = frozenset({"length.js", "name.js", "prop-desc.js"})
_IS_CONSTRUCTOR = frozenset({"not-a-constructor.js"})


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name in _SYMBOL:
        features.add("Symbol")
    if name in _BIGINT:
        features.add("BigInt")
    if name in _REFLECT_CONSTRUCT:
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in _COMPARE_ARRAY:
        includes.add("compareArray.js")
    if name in _TEMPORAL_HELPERS:
        includes.add("temporalHelpers.js")
    if name in _PROPERTY_HELPER:
        includes.add("propertyHelper.js")
    if name in _IS_CONSTRUCTOR:
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_DURATION_TO_STRING_FEATURES = {
    path: _features(path) for path in TEMPORAL_DURATION_TO_STRING_FILES
}
TEMPORAL_DURATION_TO_STRING_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_DURATION_TO_STRING_FILES
}
TEMPORAL_DURATION_TO_STRING_FLAGS = {
    path: frozenset() for path in TEMPORAL_DURATION_TO_STRING_FILES
}
TEMPORAL_DURATION_TO_STRING_NEGATIVE = {
    path: None for path in TEMPORAL_DURATION_TO_STRING_FILES
}

if len(TEMPORAL_DURATION_TO_STRING_FILES) != 44:
    raise RuntimeError(
        "Temporal.Duration.prototype.toString admission must contain 44 files"
    )
