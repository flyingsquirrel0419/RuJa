"""Exact Test262 coverage for the Temporal.Duration hidden-slot core."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_duration_core_admission.txt")
TEMPORAL_DURATION_CORE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"Temporal"}
    if path.endswith("/branding.js"):
        features.add("Symbol")
    return frozenset(features)


TEMPORAL_DURATION_CORE_FEATURES = {
    path: _features(path) for path in TEMPORAL_DURATION_CORE_FILES
}

if len(TEMPORAL_DURATION_CORE_FILES) != 78:
    raise RuntimeError("Temporal.Duration core admission must contain 78 files")
