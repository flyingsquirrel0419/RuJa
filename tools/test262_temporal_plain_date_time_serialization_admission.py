"""Frozen PlainDateTime toString/toJSON Test262 accounting."""

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
        raise RuntimeError(
            f"PlainDateTime serialization manifest is not sorted and unique: {name}"
        )
    return frozenset(lines)


TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FILES = _read_manifest(
    "test262_temporal_plain_date_time_serialization_admission.txt"
)
TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_BLOCKERS = _read_manifest(
    "test262_temporal_plain_date_time_serialization_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_DOWNSTREAM_FILES = _read_manifest(
    "test262_temporal_plain_date_time_serialization_downstream_admission.txt"
)
TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FALSE_POSITIVES = _read_manifest(
    "test262_temporal_plain_date_time_serialization_false_positives.txt"
)
TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_SURFACE = (
    TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FILES
    | TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_BLOCKERS
)
TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_DOWNSTREAM_SURFACE = (
    TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_DOWNSTREAM_FILES
)


def _features(path):
    features = {"Temporal"}
    name = Path(path).name
    if name == "branding.js":
        features.add("Symbol")
    if path.endswith("/toString/options-wrong-type.js"):
        features.update(("BigInt", "Symbol"))
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    if path.endswith(
        (
            "/toString/calendarname-wrong-type.js",
            "/toString/fractionalseconddigits-wrong-type.js",
            "/toString/options-read-before-algorithmic-validation.js",
            "/toString/order-of-operations.js",
            "/toString/roundingmode-wrong-type.js",
            "/toString/smallestunit-wrong-type.js",
        )
    ):
        includes.update(("compareArray.js", "temporalHelpers.js"))
    if path.endswith("/toString/smallestunit-plurals-accepted.js"):
        includes.add("temporalHelpers.js")
    if path in TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_DOWNSTREAM_SURFACE:
        includes.add("temporalHelpers.js")
    return frozenset(includes)


TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_ALL_FILES = (
    TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_SURFACE
    | TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_DOWNSTREAM_SURFACE
)
TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FEATURES = {
    path: _features(path)
    for path in TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_ALL_FILES
}
TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_INCLUDES = {
    path: _includes(path)
    for path in TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_ALL_FILES
}
TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FLAGS = {
    path: frozenset()
    for path in TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_ALL_FILES
}
TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_ALL_FILES
}


if (
    len(TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FILES) != 57
    or len(TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_BLOCKERS) != 7
    or len(TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_SURFACE) != 64
    or len(TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_DOWNSTREAM_SURFACE) != 1
    or len(TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FALSE_POSITIVES) != 5
    or not TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FILES.isdisjoint(
        TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_BLOCKERS
    )
    or not TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_SURFACE.isdisjoint(
        TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_DOWNSTREAM_SURFACE
    )
    or not TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FALSE_POSITIVES.issubset(
        TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FILES
    )
    or sum("/toString/" in path for path in TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FILES)
    != 49
    or sum("/toJSON/" in path for path in TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FILES)
    != 8
    or set(TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_FEATURES)
    != TEMPORAL_PLAIN_DATE_TIME_SERIALIZATION_ALL_FILES
):
    raise RuntimeError(
        "PlainDateTime serialization must contain 57 pass / 7 fail / 1 passing downstream"
    )
