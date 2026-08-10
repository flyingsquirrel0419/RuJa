"""Exact Test262 coverage for PlainMonthDay/PlainYearMonth hidden-slot cores."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_calendar_siblings_core_admission.txt"
)
TEMPORAL_CALENDAR_SIBLINGS_CORE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_BLOCKER_MANIFEST = Path(__file__).with_name(
    "test262_temporal_calendar_siblings_core_blockers.txt"
)
TEMPORAL_CALENDAR_SIBLINGS_CORE_BLOCKER_FILES = frozenset(
    line
    for raw_line in _BLOCKER_MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_FILES = (
    TEMPORAL_CALENDAR_SIBLINGS_CORE_FILES
    | TEMPORAL_CALENDAR_SIBLINGS_CORE_BLOCKER_FILES
)

_TEMPORAL_HELPERS = frozenset(
    {
        "built-ins/Temporal/PlainMonthDay/argument-convert.js",
        "built-ins/Temporal/PlainMonthDay/basic.js",
        "built-ins/Temporal/PlainMonthDay/subclass.js",
        "built-ins/Temporal/PlainYearMonth/argument-convert.js",
        "built-ins/Temporal/PlainYearMonth/basic.js",
        "built-ins/Temporal/PlainYearMonth/limits.js",
        "built-ins/Temporal/PlainYearMonth/subclass.js",
    }
)
_COMPARE_ARRAY = frozenset(
    {
        "built-ins/Temporal/PlainMonthDay/calendar-invalid.js",
        "built-ins/Temporal/PlainMonthDay/infinity-throws-rangeerror.js",
        "built-ins/Temporal/PlainMonthDay/missing-arguments.js",
        "built-ins/Temporal/PlainMonthDay/negative-infinity-throws-rangeerror.js",
        "built-ins/Temporal/PlainYearMonth/calendar-invalid.js",
        "built-ins/Temporal/PlainYearMonth/infinity-throws-rangeerror.js",
        "built-ins/Temporal/PlainYearMonth/missing-arguments.js",
        "built-ins/Temporal/PlainYearMonth/negative-infinity-throws-rangeerror.js",
    }
)
_PROPERTY_HELPER_SUFFIXES = frozenset(
    {
        "length.js",
        "name.js",
        "prop-desc.js",
        "prototype/constructor.js",
        "prototype/prop-desc.js",
        "prototype/toStringTag/prop-desc.js",
        "prototype/valueOf/length.js",
        "prototype/valueOf/name.js",
        "prototype/valueOf/prop-desc.js",
    }
)
_PROPERTY_HELPER = frozenset(
    path
    for path in TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_FILES
    if path.split("/PlainMonthDay/", 1)[-1].split("/PlainYearMonth/", 1)[-1]
    in _PROPERTY_HELPER_SUFFIXES
)
_IS_CONSTRUCTOR = frozenset(
    {
        "built-ins/Temporal/PlainMonthDay/prototype/valueOf/not-a-constructor.js",
        "built-ins/Temporal/PlainYearMonth/prototype/valueOf/not-a-constructor.js",
    }
)


def _features(path):
    features = {"Temporal"}
    if path.endswith("/branding.js"):
        features.add("Symbol")
    if path.endswith("/calendar-wrong-type.js"):
        features.update({"BigInt", "Symbol"})
    if path.endswith("/valueOf/not-a-constructor.js"):
        features.add("Reflect.construct")
    return frozenset(features)


TEMPORAL_CALENDAR_SIBLINGS_CORE_FEATURES = {
    path: _features(path) for path in TEMPORAL_CALENDAR_SIBLINGS_CORE_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_FEATURES = {
    path: _features(path) for path in TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_FILES
}


def _includes(path):
    includes = set()
    if path in _TEMPORAL_HELPERS:
        includes.add("temporalHelpers.js")
    if path in _COMPARE_ARRAY:
        includes.add("compareArray.js")
        if not path.endswith("PlainYearMonth/missing-arguments.js"):
            includes.add("temporalHelpers.js")
    if path in _PROPERTY_HELPER:
        includes.add("propertyHelper.js")
    if path in _IS_CONSTRUCTOR:
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_FLAGS = {
    path: frozenset() for path in TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_NEGATIVE = {
    path: None for path in TEMPORAL_CALENDAR_SIBLINGS_CORE_ALL_FILES
}

if len(TEMPORAL_CALENDAR_SIBLINGS_CORE_FILES) != 89:
    raise RuntimeError("Temporal calendar sibling core admission must contain 89 files")
if len(TEMPORAL_CALENDAR_SIBLINGS_CORE_BLOCKER_FILES) != 15:
    raise RuntimeError("Temporal calendar sibling core blockers must contain 15 files")
