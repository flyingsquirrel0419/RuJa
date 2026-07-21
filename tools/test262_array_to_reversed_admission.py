"""Frozen feature-gated Test262 Array.prototype.toReversed files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_to_reversed_admission.txt")
ARRAY_TO_REVERSED_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_TO_REVERSED_FEATURES = {
    "built-ins/Array/prototype/toReversed/not-a-constructor.js": frozenset(
        {"Reflect.construct"}
    ),
}

if frozenset(ARRAY_TO_REVERSED_FEATURES) != ARRAY_TO_REVERSED_FILES:
    raise RuntimeError("Array toReversed admission manifest and feature map differ")
