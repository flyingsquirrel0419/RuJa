"""Exact Test262 boundaries for Temporal.Duration unary sign transforms."""

from pathlib import Path


def _read_manifest(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_DURATION_ABS_FILES = _read_manifest(
    "test262_temporal_duration_abs_admission.txt"
)
TEMPORAL_DURATION_NEGATED_FILES = _read_manifest(
    "test262_temporal_duration_negated_admission.txt"
)

_TEMPORAL_HELPERS = frozenset({"basic.js", "subclassing-ignored.js"})


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name == "branding.js":
        features.add("Symbol")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if name in _TEMPORAL_HELPERS:
        includes.add("temporalHelpers.js")
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


def _metadata(files):
    return {
        "features": {path: _features(path) for path in files},
        "includes": {path: _includes(path) for path in files},
        "flags": {path: frozenset() for path in files},
        "negative": {path: None for path in files},
    }


_ABS_METADATA = _metadata(TEMPORAL_DURATION_ABS_FILES)
TEMPORAL_DURATION_ABS_FEATURES = _ABS_METADATA["features"]
TEMPORAL_DURATION_ABS_INCLUDES = _ABS_METADATA["includes"]
TEMPORAL_DURATION_ABS_FLAGS = _ABS_METADATA["flags"]
TEMPORAL_DURATION_ABS_NEGATIVE = _ABS_METADATA["negative"]

_NEGATED_METADATA = _metadata(TEMPORAL_DURATION_NEGATED_FILES)
TEMPORAL_DURATION_NEGATED_FEATURES = _NEGATED_METADATA["features"]
TEMPORAL_DURATION_NEGATED_INCLUDES = _NEGATED_METADATA["includes"]
TEMPORAL_DURATION_NEGATED_FLAGS = _NEGATED_METADATA["flags"]
TEMPORAL_DURATION_NEGATED_NEGATIVE = _NEGATED_METADATA["negative"]

if len(TEMPORAL_DURATION_ABS_FILES) != 9:
    raise RuntimeError("Temporal.Duration.prototype.abs admission must contain 9 files")
if len(TEMPORAL_DURATION_NEGATED_FILES) != 8:
    raise RuntimeError("Temporal.Duration.prototype.negated admission must contain 8 files")
if TEMPORAL_DURATION_ABS_FILES & TEMPORAL_DURATION_NEGATED_FILES:
    raise RuntimeError("Temporal.Duration unary admissions must be disjoint")
