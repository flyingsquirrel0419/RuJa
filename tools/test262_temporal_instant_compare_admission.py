"""Exact Test262 coverage for Temporal.Instant.compare."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_instant_compare_admission.txt")
TEMPORAL_INSTANT_COMPARE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name == "argument-wrong-type.js":
        features.update({"BigInt", "Symbol"})
    if name == "argument-string-invalid.js":
        features.add("arrow-function")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


TEMPORAL_INSTANT_COMPARE_FEATURES = {
    path: _features(path) for path in TEMPORAL_INSTANT_COMPARE_FILES
}

if len(TEMPORAL_INSTANT_COMPARE_FILES) != 29:
    raise RuntimeError("Temporal.Instant.compare admission must contain 29 files")
