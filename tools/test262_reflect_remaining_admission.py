"""Frozen residual Test262 files for the direct Reflect surface."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_reflect_remaining_admission.txt")
REFLECT_REMAINING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Reflect/Symbol.toStringTag.js": {"Symbol.toStringTag"},
    "built-ins/Reflect/defineProperty/define-symbol-properties.js": {"Symbol"},
    "built-ins/Reflect/defineProperty/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/defineProperty/return-abrupt-from-result.js": {"Proxy"},
    "built-ins/Reflect/defineProperty/target-is-symbol-throws.js": {"Symbol"},
    "built-ins/Reflect/getOwnPropertyDescriptor/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/getOwnPropertyDescriptor/return-abrupt-from-result.js": {
        "Proxy"
    },
    "built-ins/Reflect/getOwnPropertyDescriptor/symbol-property.js": {"Symbol"},
    "built-ins/Reflect/getOwnPropertyDescriptor/target-is-symbol-throws.js": {
        "Symbol"
    },
    "built-ins/Reflect/getPrototypeOf/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/getPrototypeOf/return-abrupt-from-result.js": {"Proxy"},
    "built-ins/Reflect/getPrototypeOf/target-is-symbol-throws.js": {"Symbol"},
    "built-ins/Reflect/isExtensible/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/isExtensible/return-abrupt-from-result.js": {"Proxy"},
    "built-ins/Reflect/isExtensible/target-is-symbol-throws.js": {"Symbol"},
    "built-ins/Reflect/preventExtensions/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/preventExtensions/return-abrupt-from-result.js": {"Proxy"},
    "built-ins/Reflect/preventExtensions/return-boolean-from-proxy-object.js": {
        "Proxy"
    },
    "built-ins/Reflect/preventExtensions/target-is-symbol-throws.js": {"Symbol"},
    "built-ins/Reflect/setPrototypeOf/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/setPrototypeOf/proto-is-symbol-throws.js": {"Symbol"},
    "built-ins/Reflect/setPrototypeOf/return-abrupt-from-result.js": {"Proxy"},
    "built-ins/Reflect/setPrototypeOf/target-is-symbol-throws.js": {"Symbol"},
}
REFLECT_REMAINING_FEATURES = {
    path: frozenset(
        {"Reflect"}
        | (
            {"Reflect.setPrototypeOf"}
            if path.startswith("built-ins/Reflect/setPrototypeOf/")
            and path != "built-ins/Reflect/setPrototypeOf/setPrototypeOf.js"
            else set()
        )
        | _EXTRA_FEATURES.get(path, set())
    )
    for path in REFLECT_REMAINING_FILES
}
