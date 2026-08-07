"""Exact Intl402 Test262 coverage unlocked by Temporal.PlainDate.prototype.equals."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_plain_date_equals_intl_admission.txt")
TEMPORAL_PLAIN_DATE_EQUALS_INTL_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_DATE_EQUALS_INTL_FEATURES = {
    "intl402/Temporal/PlainDate/prototype/equals/future-calendar.js": frozenset(
        {"Intl.Era-monthcode", "Temporal"}
    )
}
TEMPORAL_PLAIN_DATE_EQUALS_INTL_INCLUDES = {
    "intl402/Temporal/PlainDate/prototype/equals/future-calendar.js": frozenset(
        {"temporalHelpers.js"}
    )
}
TEMPORAL_PLAIN_DATE_EQUALS_INTL_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_EQUALS_INTL_FILES
}
TEMPORAL_PLAIN_DATE_EQUALS_INTL_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_EQUALS_INTL_FILES
}
TEMPORAL_PLAIN_DATE_EQUALS_INTL_BLOCKER_FILES = frozenset({
    "intl402/Temporal/PlainDate/prototype/equals/argument-object-valid.js",
    "intl402/Temporal/PlainDate/prototype/equals/argument-string.js",
    "intl402/Temporal/PlainDate/prototype/equals/calendar-is-compared.js",
    "intl402/Temporal/PlainDate/prototype/equals/canonicalize-calendar.js",
    "intl402/Temporal/PlainDate/prototype/equals/infinity-throws-rangeerror.js",
})
TEMPORAL_PLAIN_DATE_EQUALS_INTL_BLOCKER_FEATURES = {
    path: frozenset({"Temporal"})
    for path in TEMPORAL_PLAIN_DATE_EQUALS_INTL_BLOCKER_FILES
}
_BLOCKER_WITH_HELPERS = (
    "intl402/Temporal/PlainDate/prototype/equals/infinity-throws-rangeerror.js"
)
TEMPORAL_PLAIN_DATE_EQUALS_INTL_BLOCKER_INCLUDES = {
    path: (
        frozenset({"compareArray.js", "temporalHelpers.js"})
        if path == _BLOCKER_WITH_HELPERS
        else frozenset()
    )
    for path in TEMPORAL_PLAIN_DATE_EQUALS_INTL_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_EQUALS_INTL_BLOCKER_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_EQUALS_INTL_BLOCKER_FILES
}
TEMPORAL_PLAIN_DATE_EQUALS_INTL_BLOCKER_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_EQUALS_INTL_BLOCKER_FILES
}

if set(TEMPORAL_PLAIN_DATE_EQUALS_INTL_FEATURES) != set(
    TEMPORAL_PLAIN_DATE_EQUALS_INTL_FILES
):
    raise RuntimeError("Temporal.PlainDate.prototype.equals Intl admission must contain one file")
