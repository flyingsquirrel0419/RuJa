"""Exact Test262 boundary for Temporal.PlainDate.prototype.toPlainDateTime."""

from pathlib import Path


def _read_manifest(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_FILES = _read_manifest(
    "test262_temporal_plain_date_to_plain_date_time_admission.txt"
)
TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_BLOCKER_FILES = _read_manifest(
    "test262_temporal_plain_date_to_plain_date_time_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_DOWNSTREAM_FILES = _read_manifest(
    "test262_temporal_plain_date_to_plain_date_time_downstream.txt"
)

_ARROW = frozenset({
    "argument-string-no-implicit-midnight.js",
    "argument-string-time-designator-required-for-disambiguation.js",
    "argument-string-with-time-designator.js",
    "argument-string-with-utc-designator.js",
    "year-zero.js",
})
_TEMPORAL_HELPERS = frozenset({
    "argument-object.js",
    "argument-string-calendar-annotation.js",
    "argument-string-date-with-utc-offset.js",
    "argument-string-time-designator-required-for-disambiguation.js",
    "argument-string-time-separators.js",
    "argument-string-time-zone-annotation.js",
    "argument-string-unknown-annotation.js",
    "argument-string-with-time-designator.js",
    "argument-zoneddatetime-balance-negative-time-units.js",
    "argument-zoneddatetime-negative-epochnanoseconds.js",
    "basic.js",
    "leap-second.js",
    "limits.js",
    "plaintime-propertybag-no-time-units.js",
    "time-undefined.js",
})


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name == "argument-wrong-type.js":
        features.update(("BigInt", "Symbol"))
    elif name == "branding.js":
        features.add("Symbol")
    elif name == "not-a-constructor.js":
        features.add("Reflect.construct")
    if name in _ARROW:
        features.add("arrow-function")
    if path.startswith("intl402/"):
        features.add("Intl.Era-monthcode")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in _TEMPORAL_HELPERS:
        includes.add("temporalHelpers.js")
    if name == "order-of-operations.js":
        includes.update(("compareArray.js", "temporalHelpers.js"))
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


_ALL = (
    TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_FILES
    | TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_BLOCKER_FILES
    | TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_DOWNSTREAM_FILES
)
TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_FILES
}
TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_ALL_FEATURES = {
    path: _features(path) for path in _ALL
}
TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_ALL_INCLUDES = {
    path: _includes(path) for path in _ALL
}
TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_ALL_FLAGS = {
    path: frozenset() for path in _ALL
}
TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_ALL_NEGATIVE = {
    path: None for path in _ALL
}

if len(TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_FILES) != 32:
    raise RuntimeError("PlainDate.toPlainDateTime admission must contain 32 files")
if len(TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_BLOCKER_FILES) != 3:
    raise RuntimeError("PlainDate.toPlainDateTime blockers must contain 3 files")
if len(TEMPORAL_PLAIN_DATE_TO_PLAIN_DATE_TIME_DOWNSTREAM_FILES) != 1:
    raise RuntimeError("PlainDate.toPlainDateTime downstream must contain 1 file")
if len(_ALL) != 36:
    raise RuntimeError("PlainDate.toPlainDateTime manifests must be disjoint")
