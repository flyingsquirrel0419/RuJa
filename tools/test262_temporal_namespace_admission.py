"""Frozen feature-gated Test262 Temporal namespace files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_temporal_namespace_admission.txt")
TEMPORAL_NAMESPACE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_NAMESPACE_FEATURES = frozenset({"Symbol.toStringTag", "Temporal"})

if len(TEMPORAL_NAMESPACE_FILES) != 4:
    raise RuntimeError("Temporal namespace admission must contain exactly four files")
