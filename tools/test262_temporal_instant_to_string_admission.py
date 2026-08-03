"""Exact Test262 coverage for Temporal.Instant.prototype.toString."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_instant_to_string_admission.txt"
)
TEMPORAL_INSTANT_TO_STRING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_BIGINT = {
    "basic.js",
    "fractionalseconddigits-auto.js",
    "fractionalseconddigits-negative.js",
    "fractionalseconddigits-number.js",
    "options-wrong-type.js",
    "timezone-offset.js",
}


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name in _BIGINT or name == "argument-wrong-type.js":
        features.add("BigInt")
    if name in {"branding.js", "options-wrong-type.js", "argument-wrong-type.js"}:
        features.add("Symbol")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    if name == "timezone-string-year-zero.js":
        features.add("arrow-function")
    return frozenset(features)


TEMPORAL_INSTANT_TO_STRING_FEATURES = {
    path: _features(path) for path in TEMPORAL_INSTANT_TO_STRING_FILES
}

if len(TEMPORAL_INSTANT_TO_STRING_FILES) != 54:
    raise RuntimeError("Temporal.Instant toString admission must contain 54 files")
