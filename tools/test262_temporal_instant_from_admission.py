"""Frozen feature-gated Temporal.Instant.from/string-conversion tests."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_instant_from_admission.txt")
TEMPORAL_INSTANT_FROM_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_INSTANT_FROM_FEATURES = {
    path: frozenset(
        {"Temporal"}
        | ({"Reflect.construct"} if path.endswith("/not-a-constructor.js") else set())
    )
    for path in TEMPORAL_INSTANT_FROM_FILES
}

if len(TEMPORAL_INSTANT_FROM_FILES) != 15:
    raise RuntimeError("Temporal.Instant.from admission must contain 15 files")
