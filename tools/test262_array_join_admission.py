"""Frozen feature-gated Test262 Array.prototype.join files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_join_admission.txt")
ARRAY_JOIN_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_JOIN_FEATURES = {
    "built-ins/Array/prototype/join/coerced-separator-grow.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/join/coerced-separator-shrink.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/join/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/join/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
}

if frozenset(ARRAY_JOIN_FEATURES) != ARRAY_JOIN_FILES:
    raise RuntimeError("Array join admission manifest and feature map differ")
