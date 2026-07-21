"""Frozen feature-gated Test262 Array.prototype.flat and flatMap files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_flat_admission.txt")
_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_FLAT_FILES = frozenset(path for path in _FILES if "/flat/" in path)
ARRAY_FLAT_MAP_FILES = _FILES - ARRAY_FLAT_FILES

ARRAY_FLAT_FEATURES = {
    "built-ins/Array/prototype/flat/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
}

ARRAY_FLAT_MAP_FEATURES = {
    "built-ins/Array/prototype/flatMap/array-like-objects-nested.js": frozenset(
        {"Array.prototype.flatMap", "Int32Array"}
    ),
    "built-ins/Array/prototype/flatMap/array-like-objects-typedarrays.js": frozenset(
        {"Array.prototype.flatMap", "Int32Array"}
    ),
    "built-ins/Array/prototype/flatMap/non-callable-argument-throws.js": frozenset(
        {"Array.prototype.flatMap", "Symbol"}
    ),
    "built-ins/Array/prototype/flatMap/not-a-constructor.js": frozenset(
        {"Reflect.construct", "Array.prototype.flatMap", "arrow-function"}
    ),
    "built-ins/Array/prototype/flatMap/this-value-ctor-non-object.js": frozenset(
        {"Array.prototype.flatMap", "Symbol"}
    ),
    "built-ins/Array/prototype/flatMap/this-value-ctor-object-species-bad-throws.js": frozenset(
        {"Array.prototype.flatMap", "Symbol", "Symbol.species"}
    ),
    "built-ins/Array/prototype/flatMap/this-value-ctor-object-species-custom-ctor-poisoned-throws.js": frozenset(
        {"Array.prototype.flatMap", "Symbol", "Symbol.species"}
    ),
    "built-ins/Array/prototype/flatMap/this-value-ctor-object-species-custom-ctor.js": frozenset(
        {"Array.prototype.flatMap", "Symbol", "Symbol.species"}
    ),
    "built-ins/Array/prototype/flatMap/this-value-ctor-object-species.js": frozenset(
        {"Array.prototype.flatMap", "Symbol", "Symbol.species"}
    ),
}

if frozenset(ARRAY_FLAT_FEATURES) != ARRAY_FLAT_FILES:
    raise RuntimeError("Array flat admission manifest and feature map differ")
if frozenset(ARRAY_FLAT_MAP_FEATURES) != ARRAY_FLAT_MAP_FILES:
    raise RuntimeError("Array flatMap admission manifest and feature map differ")
