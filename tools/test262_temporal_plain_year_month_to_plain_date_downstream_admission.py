"""Frozen downstream Test262 blockers for PlainYearMonth.prototype.toPlainDate."""

from pathlib import Path


# Metadata and paths are pinned to Test262 revision
# 9e61c12835c5e4a3bdba93850427e6742c4f64c4.
_BLOCKERS_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_year_month_to_plain_date_downstream_blockers.txt"
)
_BLOCKER_LINES = tuple(
    line
    for raw_line in _BLOCKERS_MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_FILES = frozenset()
TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_BLOCKERS = frozenset(
    _BLOCKER_LINES
)
TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_SURFACE = (
    TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_FILES
    | TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_BLOCKERS
)

_TEMPORAL_ONLY = frozenset(
    {
        "intl402/Temporal/PlainYearMonth/prototype/with/chinese-calendar-dates.js",
        "intl402/Temporal/PlainYearMonth/prototype/with/dangi-calendar-dates.js",
    }
)


def _features(path):
    features = {"Temporal"}
    if path not in _TEMPORAL_ONLY:
        features.add("Intl.Era-monthcode")
    return frozenset(features)


TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_FEATURES = {
    path: _features(path)
    for path in TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_SURFACE
}
TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_INCLUDES = {
    path: frozenset({"temporalHelpers.js"})
    for path in TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_SURFACE
}
TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_FLAGS = {
    path: frozenset()
    for path in TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_SURFACE
}
TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_NEGATIVE = {
    path: None
    for path in TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_SURFACE
}

_ALLOWED_PREFIXES = (
    "intl402/Temporal/PlainYearMonth/from/",
    "intl402/Temporal/PlainYearMonth/prototype/add/",
    "intl402/Temporal/PlainYearMonth/prototype/subtract/",
    "intl402/Temporal/PlainYearMonth/prototype/with/",
)
if (
    len(_BLOCKER_LINES) != 87
    or tuple(sorted(_BLOCKER_LINES)) != _BLOCKER_LINES
    or len(TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_BLOCKERS) != 87
    or TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_FILES
    or not _TEMPORAL_ONLY
    < TEMPORAL_PLAIN_YEAR_MONTH_TO_PLAIN_DATE_DOWNSTREAM_BLOCKERS
    or any(
        not path.endswith(".js")
        or not path.startswith(_ALLOWED_PREFIXES)
        for path in _BLOCKER_LINES
    )
):
    raise RuntimeError(
        "PlainYearMonth.toPlainDate downstream surface must contain 0 pass / 87 fail"
    )
