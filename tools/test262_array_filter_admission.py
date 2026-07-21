"""Frozen feature-gated Test262 Array.prototype.filter files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_filter_admission.txt")
ARRAY_FILTER_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_FILTER_FEATURES = {
    "built-ins/Array/prototype/filter/callbackfn-resize-arraybuffer.js": frozenset(
        {"TypedArray", "resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/filter/create-proxy.js": frozenset(
        {"Proxy", "Symbol.species"}
    ),
    "built-ins/Array/prototype/filter/create-revoked-proxy.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Array/prototype/filter/create-species-non-ctor.js": frozenset(
        {"Symbol.species", "Reflect.construct"}
    ),
    "built-ins/Array/prototype/filter/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/filter/resizable-buffer-grow-mid-iteration.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/filter/resizable-buffer-shrink-mid-iteration.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/filter/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
}

if frozenset(ARRAY_FILTER_FEATURES) != ARRAY_FILTER_FILES:
    raise RuntimeError("Array filter admission manifest and feature map differ")
