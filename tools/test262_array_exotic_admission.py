"""Frozen Test262 Array exotic and generic method files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_exotic_admission.txt")
ARRAY_EXOTIC_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_EXOTIC_FEATURES = {
    "built-ins/Array/prototype/Symbol.iterator.js": frozenset({"Symbol.iterator"}),
    "built-ins/Array/prototype/push/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/pop/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/shift/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/unshift/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/splice/create-proxy.js": frozenset(
        {"Proxy", "Symbol.species"}
    ),
    "built-ins/Array/prototype/splice/create-revoked-proxy.js": frozenset({"Proxy"}),
    "built-ins/Array/prototype/splice/create-species-non-ctor.js": frozenset(
        {"Symbol.species", "Reflect.construct"}
    ),
    "built-ins/Array/prototype/splice/create-species-undef-invalid-len.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Array/prototype/splice/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/splice/property-traps-order-with-species.js": frozenset(
        {"Proxy", "Symbol.species"}
    ),
    "built-ins/Array/prototype/slice/coerced-start-end-grow.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/slice/coerced-start-end-shrink.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/slice/create-proxied-array-invalid-len.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Array/prototype/slice/create-proxy.js": frozenset(
        {"Proxy", "Symbol.species"}
    ),
    "built-ins/Array/prototype/slice/create-revoked-proxy.js": frozenset({"Proxy"}),
    "built-ins/Array/prototype/slice/create-species-non-ctor.js": frozenset(
        {"Symbol.species", "Reflect.construct"}
    ),
    "built-ins/Array/prototype/slice/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/slice/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/with/not-a-constructor.js": frozenset(
        {"change-array-by-copy", "Reflect.construct"}
    ),
}

if frozenset(ARRAY_EXOTIC_FEATURES) != ARRAY_EXOTIC_FILES:
    raise RuntimeError("Array exotic admission manifest and feature map differ")
