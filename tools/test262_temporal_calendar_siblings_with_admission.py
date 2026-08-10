"""Exact Test262 coverage for partial-date with methods."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_calendar_siblings_with_admission.txt"
)
TEMPORAL_CALENDAR_SIBLINGS_WITH_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name in {"branding.js", "options-wrong-type.js"} or path.endswith(
        "/PlainMonthDay/prototype/with/basic.js"
    ) or path.endswith("/PlainMonthDay/prototype/with/options-invalid.js"):
        features.add("Symbol")
    if name == "options-wrong-type.js" or path.endswith(
        "/PlainMonthDay/prototype/with/options-invalid.js"
    ):
        features.add("BigInt")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in {
        "basic.js",
        "copy-properties-not-undefined.js",
        "infinity-throws-rangeerror.js",
        "iso-year-used-only-for-overflow.js",
        "options-object.js",
        "options-read-before-algorithmic-validation.js",
        "order-of-operations.js",
        "overflow-undefined.js",
        "overflow-wrong-type.js",
        "subclassing-ignored.js",
    }:
        includes.add("temporalHelpers.js")
    if name in {
        "infinity-throws-rangeerror.js",
        "options-read-before-algorithmic-validation.js",
        "order-of-operations.js",
        "overflow-wrong-type.js",
    } or path.endswith("/PlainMonthDay/prototype/with/basic.js"):
        includes.add("compareArray.js")
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_CALENDAR_SIBLINGS_WITH_FEATURES = {
    path: _features(path) for path in TEMPORAL_CALENDAR_SIBLINGS_WITH_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_WITH_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_CALENDAR_SIBLINGS_WITH_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_WITH_FLAGS = {
    path: frozenset() for path in TEMPORAL_CALENDAR_SIBLINGS_WITH_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_WITH_NEGATIVE = {
    path: None for path in TEMPORAL_CALENDAR_SIBLINGS_WITH_FILES
}

if len(TEMPORAL_CALENDAR_SIBLINGS_WITH_FILES) != 43:
    raise RuntimeError("Temporal calendar sibling with admission must contain 43 files")
