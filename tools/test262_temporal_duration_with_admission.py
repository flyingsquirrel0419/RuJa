"""Exact Test262 boundary for Temporal.Duration.prototype.with."""

from pathlib import Path


def _read_manifest(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_DURATION_WITH_FILES = _read_manifest(
    "test262_temporal_duration_with_admission.txt"
)

_SYMBOL = frozenset({"argument-not-object.js", "branding.js"})
_TEMPORAL_HELPERS = frozenset({
    "all-negative.js",
    "all-positive.js",
    "blank-duration.js",
    "copy-properties-not-undefined.js",
    "order-of-operations.js",
    "partial-positive.js",
    "sign-replace.js",
    "subclassing-ignored.js",
})


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name in _SYMBOL:
        features.add("Symbol")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in _TEMPORAL_HELPERS:
        includes.add("temporalHelpers.js")
    if name == "order-of-operations.js":
        includes.add("compareArray.js")
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_DURATION_WITH_FEATURES = {
    path: _features(path) for path in TEMPORAL_DURATION_WITH_FILES
}
TEMPORAL_DURATION_WITH_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_DURATION_WITH_FILES
}
TEMPORAL_DURATION_WITH_FLAGS = {
    path: frozenset() for path in TEMPORAL_DURATION_WITH_FILES
}
TEMPORAL_DURATION_WITH_NEGATIVE = {
    path: None for path in TEMPORAL_DURATION_WITH_FILES
}

if len(TEMPORAL_DURATION_WITH_FILES) != 22:
    raise RuntimeError("Temporal.Duration.prototype.with admission must contain 22 files")
