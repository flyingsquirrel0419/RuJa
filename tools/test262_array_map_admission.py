"""Frozen feature-gated Test262 Array.prototype.map files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_map_admission.txt")
ARRAY_MAP_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_MAP_FEATURES = {
    "built-ins/Array/prototype/map/callbackfn-resize-arraybuffer.js": frozenset(
        {"TypedArray", "resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/map/create-proxy.js": frozenset(
        {"Proxy", "Symbol.species"}
    ),
    "built-ins/Array/prototype/map/create-revoked-proxy.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Array/prototype/map/create-species-non-ctor.js": frozenset(
        {"Symbol.species", "Reflect.construct"}
    ),
    "built-ins/Array/prototype/map/create-species-undef-invalid-len.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Array/prototype/map/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/map/resizable-buffer-grow-mid-iteration.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/map/resizable-buffer-shrink-mid-iteration.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/map/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
}

if frozenset(ARRAY_MAP_FEATURES) != ARRAY_MAP_FILES:
    raise RuntimeError("Array map admission manifest and feature map differ")
