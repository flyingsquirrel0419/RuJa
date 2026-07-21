"""Frozen feature-gated Test262 Array.prototype.forEach files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_for_each_admission.txt")
ARRAY_FOR_EACH_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_FOR_EACH_FEATURES = {
    "built-ins/Array/prototype/forEach/callbackfn-resize-arraybuffer.js": frozenset(
        {"TypedArray", "resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/forEach/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/forEach/resizable-buffer-grow-mid-iteration.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/forEach/resizable-buffer-shrink-mid-iteration.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/forEach/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
}

if frozenset(ARRAY_FOR_EACH_FEATURES) != ARRAY_FOR_EACH_FILES:
    raise RuntimeError("Array forEach admission manifest and feature map differ")
