"""Frozen feature-gated Test262 Array.prototype.reduce files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_reduce_admission.txt")
ARRAY_REDUCE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_REDUCE_FEATURES = {
    "built-ins/Array/prototype/reduce/callbackfn-resize-arraybuffer.js": frozenset(
        {"TypedArray", "resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/reduce/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/reduce/resizable-buffer-grow-mid-iteration.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/reduce/resizable-buffer-shrink-mid-iteration.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/reduce/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
}

if frozenset(ARRAY_REDUCE_FEATURES) != ARRAY_REDUCE_FILES:
    raise RuntimeError("Array reduce admission manifest and feature map differ")
