"""Exact Test262 boundary for PlainMonthDay.from and PlainYearMonth.from."""

from pathlib import Path


TOOLS = Path(__file__).resolve().parent


def _read_manifest(name):
    return frozenset(
        line
        for raw_line in (TOOLS / name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_CALENDAR_SIBLINGS_FROM_FILES = _read_manifest(
    "test262_temporal_calendar_siblings_from_admission.txt"
)
TEMPORAL_CALENDAR_SIBLINGS_FROM_BLOCKER_FILES = _read_manifest(
    "test262_temporal_calendar_siblings_from_blockers.txt"
)
TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES = (
    TEMPORAL_CALENDAR_SIBLINGS_FROM_FILES
    | TEMPORAL_CALENDAR_SIBLINGS_FROM_BLOCKER_FILES
)
TEMPORAL_CALENDAR_SIBLINGS_FROM_FORMATTER_TRANSITIONS = _read_manifest(
    "test262_temporal_calendar_siblings_from_formatter_transitions.txt"
)

_PASSING_TEMPORAL_HELPER_FILES = frozenset(
    f"built-ins/Temporal/{kind}/from/{name}.js"
    for kind, names in {
        "PlainMonthDay": (
            "calendar-temporal-object",
            "infinity-throws-rangeerror",
            "observable-get-overflow-argument-string-invalid",
            "options-read-before-algorithmic-validation",
            "order-of-operations",
        ),
        "PlainYearMonth": (
            "argument-string-invalid",
            "calendar-temporal-object",
            "infinity-throws-rangeerror",
            "observable-get-overflow-argument-string-invalid",
            "options-read-before-algorithmic-validation",
            "order-of-operations",
        ),
    }.items()
    for name in names
)
_TEMPORAL_HELPERS = (
    TEMPORAL_CALENDAR_SIBLINGS_FROM_FORMATTER_TRANSITIONS
    | _PASSING_TEMPORAL_HELPER_FILES
)
_COMPARE_ARRAY_SUFFIXES = frozenset(
    {
        "calendar-temporal-object.js",
        "infinity-throws-rangeerror.js",
        "observable-get-overflow-argument-primitive.js",
        "observable-get-overflow-argument-string-invalid.js",
        "options-read-before-algorithmic-validation.js",
        "order-of-operations.js",
        "overflow-wrong-type.js",
    }
)
_COMPARE_ARRAY = frozenset(
    path
    for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES
    if path.rsplit("/", 1)[-1] in _COMPARE_ARRAY_SUFFIXES
    or "/PlainMonthDay/from/fields-" in path
    and not path.endswith("fields-missing-properties.js")
)
_PROPERTY_HELPER = frozenset(
    path
    for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES
    if path.rsplit("/", 1)[-1] in {"length.js", "name.js", "prop-desc.js"}
)
_IS_CONSTRUCTOR = frozenset(
    path
    for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES
    if path.endswith("/not-a-constructor.js")
)
_BIGINT_SYMBOL = frozenset(
    path
    for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES
    if path.endswith("/argument-propertybag-calendar-wrong-type.js")
    or path.endswith("/argument-wrong-type.js")
    or path.endswith("/options-wrong-type.js")
    or path.endswith("/PlainMonthDay/from/options-invalid.js")
)
_ARROW_FUNCTION = frozenset(
    path
    for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES
    if path.endswith("/argument-propertybag-calendar-year-zero.js")
    or path.endswith("/argument-string-with-utc-designator.js")
    or path.endswith("/year-zero.js")
)


def _features(path):
    features = {"Temporal"}
    if path in _BIGINT_SYMBOL:
        features.update({"BigInt", "Symbol"})
    if path in _ARROW_FUNCTION:
        features.add("arrow-function")
    if path in _IS_CONSTRUCTOR:
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


TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FEATURES = {
    path: _features(path) for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_FROM_FEATURES = {
    path: TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FEATURES[path]
    for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FLAGS = {
    path: frozenset() for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_NEGATIVE = {
    path: None for path in TEMPORAL_CALENDAR_SIBLINGS_FROM_ALL_FILES
}

if len(TEMPORAL_CALENDAR_SIBLINGS_FROM_FILES) != 123:
    raise RuntimeError("Temporal calendar sibling from admission must contain 123 files")
if TEMPORAL_CALENDAR_SIBLINGS_FROM_BLOCKER_FILES:
    raise RuntimeError("Temporal calendar sibling from blockers must be empty")
if (
    len(TEMPORAL_CALENDAR_SIBLINGS_FROM_FORMATTER_TRANSITIONS) != 49
    or not TEMPORAL_CALENDAR_SIBLINGS_FROM_FORMATTER_TRANSITIONS
    <= TEMPORAL_CALENDAR_SIBLINGS_FROM_FILES
):
    raise RuntimeError("Temporal calendar sibling from transitions must contain 49 admitted files")
