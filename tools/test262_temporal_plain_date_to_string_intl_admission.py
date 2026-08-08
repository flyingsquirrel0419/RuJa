"""Frozen Intl402 blockers for Temporal.PlainDate.prototype.toString."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_to_string_intl_admission.txt"
)
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_BLOCKERS = Path(__file__).with_name(
    "test262_temporal_plain_date_to_string_intl_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_FILES = frozenset(
    line
    for raw_line in _BLOCKERS.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_FEATURES = {}
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_INCLUDES = {}
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_FLAGS = {}
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_NEGATIVE = {}
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_FEATURES = {
    path: frozenset({"Temporal"})
    for path in TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_FILES
}
_WITH_HELPERS = (
    "intl402/Temporal/PlainDate/prototype/toString/calendarname-wrong-type.js"
)
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_INCLUDES = {
    path: (
        frozenset({"compareArray.js", "temporalHelpers.js"})
        if path == _WITH_HELPERS
        else frozenset()
    )
    for path in TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_FILES
}

if TEMPORAL_PLAIN_DATE_TO_STRING_INTL_FILES:
    raise RuntimeError("Temporal.PlainDate.prototype.toString Intl admission must be empty")
if len(TEMPORAL_PLAIN_DATE_TO_STRING_INTL_BLOCKER_FILES) != 8:
    raise RuntimeError("Temporal.PlainDate.prototype.toString Intl blockers must contain 8 files")
