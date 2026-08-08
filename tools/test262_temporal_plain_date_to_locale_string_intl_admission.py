"""Frozen Intl402 blockers for Temporal.PlainDate.prototype.toLocaleString."""

from pathlib import Path


def _read_manifest(name):
    manifest = Path(__file__).with_name(name)
    return frozenset(
        line
        for raw_line in manifest.read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_FILES = _read_manifest(
    "test262_temporal_plain_date_to_locale_string_intl_admission.txt"
)
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_FILES = _read_manifest(
    "test262_temporal_plain_date_to_locale_string_intl_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_FEATURES = {}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_INCLUDES = {}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_FLAGS = {}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_NEGATIVE = {}
_CALENDAR_MISMATCH = (
    "intl402/Temporal/PlainDate/prototype/toLocaleString/calendar-mismatch.js"
)
_DATE_STYLE = "intl402/Temporal/PlainDate/prototype/toLocaleString/dateStyle.js"


def _features(path):
    features = {"Temporal"}
    if path == _CALENDAR_MISMATCH:
        features.add("Intl-enumeration")
    if path == _DATE_STYLE:
        features.add("Intl.DateTimeFormat-datetimestyle")
    return frozenset(features)


TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_FEATURES = {
    path: _features(path)
    for path in TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_INCLUDES = {
    path: frozenset()
    for path in TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_FLAGS = {
    path: frozenset()
    for path in TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_NEGATIVE = {
    path: None
    for path in TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_FILES
}

if TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_FILES:
    raise RuntimeError("Temporal.PlainDate.prototype.toLocaleString Intl admission must be empty")
if len(TEMPORAL_PLAIN_DATE_TO_LOCALE_STRING_INTL_BLOCKER_FILES) != 14:
    raise RuntimeError("Temporal.PlainDate.prototype.toLocaleString Intl blockers must contain 14 files")
