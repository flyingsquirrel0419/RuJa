"""Exact downstream accounting for PlainDate sibling conversions."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_sibling_conversions_downstream_blockers.txt"
)
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_BLOCKERS = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_FILES = frozenset()
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_SURFACE = (
    TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_BLOCKERS
)


def _features(path):
    name = Path(path).name
    feature = "Intl-enumeration" if name == "calendar-mismatch.js" else "Intl.Era-monthcode"
    return frozenset({"Temporal", feature})


TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_FEATURES = {
    path: _features(path)
    for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_SURFACE
}
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_INCLUDES = {
    path: frozenset()
    for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_SURFACE
}
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_FLAGS = {
    path: frozenset()
    for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_SURFACE
}
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_SURFACE
}

if len(TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_DOWNSTREAM_SURFACE) != 3:
    raise RuntimeError("PlainDate sibling conversion downstream surface must contain 3 blockers")
