"""Exact Test262 coverage for partial-date toString formatters."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_calendar_siblings_to_string_admission.txt"
)
TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    if path.endswith("/branding.js") or path.endswith("/options-wrong-type.js"):
        features.add("Symbol")
    if path.endswith("/options-wrong-type.js"):
        features.add("BigInt")
    if path.endswith("/not-a-constructor.js"):
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = path.rsplit("/", 1)[-1]
    if name in {"calendarname-wrong-type.js", "order-of-operations.js"}:
        return frozenset({"compareArray.js", "temporalHelpers.js"})
    if name in {"length.js", "name.js", "prop-desc.js"}:
        return frozenset({"propertyHelper.js"})
    if name == "not-a-constructor.js":
        return frozenset({"isConstructor.js"})
    return frozenset()


TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FEATURES = {
    path: _features(path) for path in TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FLAGS = {
    path: frozenset() for path in TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FILES
}
TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_NEGATIVE = {
    path: None for path in TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FILES
}

if len(TEMPORAL_CALENDAR_SIBLINGS_TO_STRING_FILES) != 33:
    raise RuntimeError("Temporal calendar sibling toString admission must contain 33 files")
