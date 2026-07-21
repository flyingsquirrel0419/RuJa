"""Frozen feature-gated Test262 Array.prototype.toSpliced files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_to_spliced_admission.txt")
ARRAY_TO_SPLICED_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_TO_SPLICED_FEATURES = {
    "built-ins/Array/prototype/toSpliced/not-a-constructor.js": frozenset(
        {"Reflect.construct"}
    ),
}

if frozenset(ARRAY_TO_SPLICED_FEATURES) != ARRAY_TO_SPLICED_FILES:
    raise RuntimeError("Array toSpliced admission manifest and feature map differ")
