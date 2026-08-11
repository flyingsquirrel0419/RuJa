"""Exact Test262 coverage for PlainDate sibling conversions."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_plain_date_sibling_conversions_admission.txt"
)
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_PLAIN_DATE_TO_PLAIN_MONTH_DAY_FILES = frozenset(
    path
    for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES
    if "/toPlainMonthDay/" in path
)
TEMPORAL_PLAIN_DATE_TO_PLAIN_YEAR_MONTH_FILES = frozenset(
    path
    for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES
    if "/toPlainYearMonth/" in path
)


def _features(path):
    features = {"Temporal"}
    name = Path(path).name
    if name == "branding.js":
        features.add("Symbol")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in {"basic.js", "limits.js"}:
        includes.add("temporalHelpers.js")
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES
}
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES
}
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES
}
TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES
}

if (
    len(TEMPORAL_PLAIN_DATE_TO_PLAIN_MONTH_DAY_FILES) != 7
    or len(TEMPORAL_PLAIN_DATE_TO_PLAIN_YEAR_MONTH_FILES) != 8
    or len(TEMPORAL_PLAIN_DATE_SIBLING_CONVERSION_FILES) != 15
):
    raise RuntimeError("PlainDate sibling conversion admission must contain 7 + 8 files")
