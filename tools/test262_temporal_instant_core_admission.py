"""Frozen feature-gated Test262 Temporal.Instant core files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_instant_core_admission.txt")
TEMPORAL_INSTANT_CORE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_INSTANT_CORE_FEATURES = {
    path: frozenset(
        {"Temporal"}
        | ({"Symbol"} if path.endswith("argument.js") or path.endswith("branding.js") else set())
        | ({"BigInt"} if path.endswith("/basic.js") and "/prototype/epoch" in path else set())
    )
    for path in TEMPORAL_INSTANT_CORE_FILES
}

if len(TEMPORAL_INSTANT_CORE_FILES) != 19:
    raise RuntimeError("Temporal.Instant core admission must contain exactly 19 files")
