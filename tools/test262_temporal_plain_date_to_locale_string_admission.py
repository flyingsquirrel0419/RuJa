"""Frozen direct blockers for Temporal.PlainDate.prototype.toLocaleString."""

from pathlib import Path


def _read_manifest(name):
    manifest = Path(__file__).with_name(name)
    return frozenset(
        line
        for raw_line in manifest.read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_FILES = _read_manifest(
    "test262_temporal_plain_date_to_locale_string_admission.txt"
)
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_FILES = _read_manifest(
    "test262_temporal_plain_date_to_locale_string_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_FEATURES = {}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INCLUDES = {}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_FLAGS = {}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_NEGATIVE = {}
_SYMBOL = "built-ins/Temporal/PlainDate/prototype/toLocaleString/branding.js"
_REFLECT_CONSTRUCT = (
    "built-ins/Temporal/PlainDate/prototype/toLocaleString/not-a-constructor.js"
)
_PROPERTY_HELPER = frozenset({
    "built-ins/Temporal/PlainDate/prototype/toLocaleString/length.js",
    "built-ins/Temporal/PlainDate/prototype/toLocaleString/name.js",
    "built-ins/Temporal/PlainDate/prototype/toLocaleString/prop-desc.js",
})


def _features(path):
    features = {"Temporal"}
    if path == _SYMBOL:
        features.add("Symbol")
    if path == _REFLECT_CONSTRUCT:
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    if path in _PROPERTY_HELPER:
        return frozenset({"propertyHelper.js"})
    if path == _REFLECT_CONSTRUCT:
        return frozenset({"isConstructor.js"})
    return frozenset()


TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_FEATURES = {
    path: _features(path)
    for path in TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_INCLUDES = {
    path: _includes(path)
    for path in TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_FLAGS = {
    path: frozenset()
    for path in TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_FILES
}

if TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_FILES:
    raise RuntimeError("Temporal.PlainDate.prototype.toLocaleString admission must be empty")
if len(TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_BLOCKER_FILES) != 7:
    raise RuntimeError("Temporal.PlainDate.prototype.toLocaleString blockers must contain 7 files")
