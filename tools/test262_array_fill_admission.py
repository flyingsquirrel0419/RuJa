"""Frozen feature-gated Test262 Array.prototype.fill files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_fill_admission.txt")
ARRAY_FILL_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_FILL_FEATURES = {
    "built-ins/Array/prototype/fill/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/fill/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/fill/return-abrupt-from-end-as-symbol.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Array/prototype/fill/return-abrupt-from-start-as-symbol.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Array/prototype/fill/return-abrupt-from-this-length-as-symbol.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Array/prototype/fill/typed-array-resize.js": frozenset(
        {"resizable-arraybuffer"}
    ),
}

if frozenset(ARRAY_FILL_FEATURES) != ARRAY_FILL_FILES:
    raise RuntimeError("Array fill admission manifest and feature map differ")
