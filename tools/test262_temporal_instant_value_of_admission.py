"""Exact Test262 Temporal.Instant.prototype.valueOf coverage."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_instant_value_of_admission.txt")
TEMPORAL_INSTANT_VALUE_OF_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_INSTANT_VALUE_OF_FEATURES = {
    path: frozenset(
        {"Temporal"}
        | ({"Symbol"} if path.endswith("/branding.js") else set())
        | ({"Reflect.construct"} if path.endswith("/not-a-constructor.js") else set())
    )
    for path in TEMPORAL_INSTANT_VALUE_OF_FILES
}

if len(TEMPORAL_INSTANT_VALUE_OF_FILES) != 7:
    raise RuntimeError("Temporal.Instant valueOf admission must contain 7 files")
