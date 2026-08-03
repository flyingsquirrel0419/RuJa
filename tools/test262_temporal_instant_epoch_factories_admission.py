"""Frozen feature-gated Test262 Temporal.Instant epoch factory files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_instant_epoch_factories_admission.txt"
)
TEMPORAL_INSTANT_EPOCH_FACTORY_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_INSTANT_EPOCH_FACTORY_FEATURES = {
    path: frozenset(
        {"Temporal"}
        | ({"BigInt"} if path.endswith("/basic.js") else set())
        | ({"Reflect.construct"} if path.endswith("/not-a-constructor.js") else set())
    )
    for path in TEMPORAL_INSTANT_EPOCH_FACTORY_FILES
}

if len(TEMPORAL_INSTANT_EPOCH_FACTORY_FILES) != 17:
    raise RuntimeError("Temporal.Instant epoch factory admission must contain 17 files")
