"""Frozen feature-gated Test262 Array.prototype.copyWithin files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_copy_within_admission.txt")
ARRAY_COPY_WITHIN_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_COPY_WITHIN_FEATURES = {
    "built-ins/Array/prototype/copyWithin/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/copyWithin/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/copyWithin/return-abrupt-from-delete-proxy-target.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Array/prototype/copyWithin/return-abrupt-from-end-as-symbol.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Array/prototype/copyWithin/return-abrupt-from-has-start.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Array/prototype/copyWithin/return-abrupt-from-start-as-symbol.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Array/prototype/copyWithin/return-abrupt-from-target-as-symbol.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Array/prototype/copyWithin/return-abrupt-from-this-length-as-symbol.js": frozenset(
        {"Symbol"}
    ),
}

if frozenset(ARRAY_COPY_WITHIN_FEATURES) != ARRAY_COPY_WITHIN_FILES:
    raise RuntimeError("Array copyWithin admission manifest and feature map differ")
