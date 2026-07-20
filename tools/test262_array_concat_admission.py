"""Frozen feature-gated Test262 Array.prototype.concat files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_concat_admission.txt")
ARRAY_CONCAT_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_CONCAT_FEATURES = {
    "built-ins/Array/prototype/concat/arg-length-exceeding-integer-limit.js": frozenset(
        {"Symbol.isConcatSpreadable", "Proxy"}
    ),
    "built-ins/Array/prototype/concat/create-proxy.js": frozenset(
        {"Proxy", "Symbol.species"}
    ),
    "built-ins/Array/prototype/concat/create-revoked-proxy.js": frozenset({"Proxy"}),
    "built-ins/Array/prototype/concat/create-species-non-ctor.js": frozenset(
        {"Symbol.species", "Reflect.construct"}
    ),
    "built-ins/Array/prototype/concat/is-concat-spreadable-is-array-proxy-revoked.js": frozenset(
        {"Proxy", "Symbol.isConcatSpreadable"}
    ),
    "built-ins/Array/prototype/concat/is-concat-spreadable-proxy-revoked.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Array/prototype/concat/is-concat-spreadable-proxy.js": frozenset(
        {"Proxy", "Symbol.isConcatSpreadable"}
    ),
    "built-ins/Array/prototype/concat/is-concat-spreadable-val-truthy.js": frozenset(
        {"Symbol", "Symbol.isConcatSpreadable"}
    ),
    "built-ins/Array/prototype/concat/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
}

if frozenset(ARRAY_CONCAT_FEATURES) != ARRAY_CONCAT_FILES:
    raise RuntimeError("Array concat admission manifest and feature map differ")
