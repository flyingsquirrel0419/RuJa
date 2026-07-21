"""Frozen feature-gated Test262 Array.prototype.reverse files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_reverse_admission.txt")
ARRAY_REVERSE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_REVERSE_FEATURES = {
    "built-ins/Array/prototype/reverse/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/reverse/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
}

if frozenset(ARRAY_REVERSE_FEATURES) != ARRAY_REVERSE_FILES:
    raise RuntimeError("Array reverse admission manifest and feature map differ")
