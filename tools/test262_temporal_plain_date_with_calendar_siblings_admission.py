"""Frozen PlainDate/PlainDateTime withCalendar Test262 accounting."""

from pathlib import Path


# Paths and metadata are pinned to Test262 revision
# 9e61c12835c5e4a3bdba93850427e6742c4f64c4.
def _read_manifest(name):
    lines = tuple(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )
    if tuple(sorted(lines)) != lines or len(set(lines)) != len(lines):
        raise RuntimeError(f"withCalendar sibling manifest is not sorted and unique: {name}")
    return frozenset(lines)


TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FILES = _read_manifest(
    "test262_temporal_plain_date_with_calendar_siblings_admission.txt"
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_BLOCKERS = _read_manifest(
    "test262_temporal_plain_date_with_calendar_siblings_blockers.txt"
)
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_SURFACE = (
    TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FILES
    | TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_BLOCKERS
)


def _features(path):
    features = {"Temporal"}
    name = Path(path).name
    if name == "branding.js":
        features.add("Symbol")
    if name == "calendar-wrong-type.js":
        features.update(("BigInt", "Symbol"))
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    if name in {"extreme-dates.js", "future-calendar.js"}:
        features.add("Intl.Era-monthcode")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in {
        "argument-string.js",
        "basic.js",
        "extreme-dates.js",
        "future-calendar.js",
        "roundtrip-from-iso8601.js",
        "subclassing-ignored.js",
    }:
        includes.add("temporalHelpers.js")
    if name == "calendar-temporal-object.js":
        includes.add("compareArray.js")
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FEATURES = {
    path: _features(path)
    for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_SURFACE
}
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_INCLUDES = {
    path: _includes(path)
    for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_SURFACE
}
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FLAGS = {
    path: frozenset()
    for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_SURFACE
}
TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_SURFACE
}


_PLAIN_DATE_PREFIXES = (
    "built-ins/Temporal/PlainDate/prototype/withCalendar/",
    "intl402/Temporal/PlainDate/prototype/withCalendar/",
)
_PLAIN_DATE_TIME_PREFIXES = (
    "built-ins/Temporal/PlainDateTime/prototype/withCalendar/",
    "intl402/Temporal/PlainDateTime/prototype/withCalendar/",
)
if (
    len(TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FILES) != 36
    or len(TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_BLOCKERS) != 9
    or len(TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_SURFACE) != 45
    or not TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FILES.isdisjoint(
        TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_BLOCKERS
    )
    or sum(
        path.startswith(_PLAIN_DATE_PREFIXES)
        for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FILES
    )
    != 18
    or sum(
        path.startswith(_PLAIN_DATE_TIME_PREFIXES)
        for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_FILES
    )
    != 18
    or sum(
        path.startswith(_PLAIN_DATE_PREFIXES)
        for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_BLOCKERS
    )
    != 4
    or sum(
        path.startswith(_PLAIN_DATE_TIME_PREFIXES)
        for path in TEMPORAL_PLAIN_DATE_WITH_CALENDAR_SIBLING_BLOCKERS
    )
    != 5
):
    raise RuntimeError(
        "PlainDate/PlainDateTime withCalendar surface must contain 36 pass / 9 fail"
    )
