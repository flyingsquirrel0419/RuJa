"""Exact Test262 coverage for Temporal.PlainMonthDay.prototype.toPlainDate."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_month_day_to_plain_date_admission.txt"
)
TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    name = Path(path).name
    if name == "argument-not-object.js":
        features.update({"BigInt", "Symbol"})
    if name == "branding.js":
        features.add("Symbol")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in {
        "basic.js",
        "default-overflow-behaviour.js",
        "infinity-throws-rangeerror.js",
        "limits.js",
        "order-of-operations.js",
    }:
        includes.add("temporalHelpers.js")
    if name in {"infinity-throws-rangeerror.js", "order-of-operations.js"}:
        includes.add("compareArray.js")
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_FILES
}
TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_FILES
}
TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_FILES
}
TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_FILES
}

if len(TEMPORAL_PLAIN_MONTH_DAY_TO_PLAIN_DATE_FILES) != 12:
    raise RuntimeError("PlainMonthDay.prototype.toPlainDate admission must contain 12 files")
