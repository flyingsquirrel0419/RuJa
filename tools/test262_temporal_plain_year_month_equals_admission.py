"""Exact Test262 coverage for Temporal.PlainYearMonth.prototype.equals."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_year_month_equals_admission.txt"
)
TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    name = Path(path).name
    if name in {
        "argument-propertybag-calendar-wrong-type.js",
        "argument-wrong-type.js",
    }:
        features.update({"BigInt", "Symbol"})
    elif name == "branding.js":
        features.add("Symbol")
    if name in {
        "argument-propertybag-calendar-year-zero.js",
        "argument-string-with-utc-designator.js",
        "year-zero.js",
    }:
        features.add("arrow-function")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in {
        "argument-string-invalid.js",
        "argument-string.js",
        "calendar-temporal-object.js",
        "infinity-throws-rangeerror.js",
    }:
        includes.add("temporalHelpers.js")
    if name in {"calendar-temporal-object.js", "infinity-throws-rangeerror.js"}:
        includes.add("compareArray.js")
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_FILES
}
TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_FILES
}
TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_FILES
}
TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_FILES
}

if len(TEMPORAL_PLAIN_YEAR_MONTH_EQUALS_FILES) != 40:
    raise RuntimeError("PlainYearMonth.prototype.equals admission must contain 40 files")
