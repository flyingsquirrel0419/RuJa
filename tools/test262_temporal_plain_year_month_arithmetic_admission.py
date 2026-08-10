"""Exact Test262 coverage for PlainYearMonth add and subtract."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_year_month_arithmetic_admission.txt"
)
_BLOCKERS_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_year_month_arithmetic_blockers.txt"
)
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_BLOCKERS = frozenset(
    line
    for raw_line in _BLOCKERS_MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE = (
    TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_FILES
    | TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_BLOCKERS
)


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name in {"argument-not-object.js", "branding.js", "options-wrong-type.js"}:
        features.add("Symbol")
    if name == "options-wrong-type.js":
        features.add("BigInt")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in {
        "argument-duration-max.js",
        "argument-duration-object.js",
        "argument-object.js",
        "argument-string.js",
        "blank-duration.js",
        "options-object.js",
        "options-read-before-algorithmic-validation.js",
        "order-of-operations.js",
        "overflow-undefined.js",
        "overflow-wrong-type.js",
        "subclassing-ignored.js",
        "subtract-from-last-representable-month.js",
    }:
        includes.add("temporalHelpers.js")
    if name in {
        "options-read-before-algorithmic-validation.js",
        "order-of-operations.js",
        "overflow-wrong-type.js",
    }:
        includes.add("compareArray.js")
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_FILES
}
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_FILES
}
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_FILES
}
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_FILES
}
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE
}
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE
}
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE
}
TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE
}

if (
    len(TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_FILES) != 73
    or len(TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_BLOCKERS) != 0
    or len(TEMPORAL_PLAIN_YEAR_MONTH_ARITHMETIC_SURFACE) != 73
):
    raise RuntimeError("PlainYearMonth arithmetic surface must contain 73 pass / 0 fail")
