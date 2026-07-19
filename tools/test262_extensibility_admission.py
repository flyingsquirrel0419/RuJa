"""Frozen Test262 Object and Proxy extensibility files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_extensibility_admission.txt")
EXTENSIBILITY_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Object/isExtensible/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Object/preventExtensions/abrupt-completion.js": {"Proxy"},
    "built-ins/Object/preventExtensions/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Object/preventExtensions/symbol-object-contains-symbol-properties-non-strict.js": {
        "Symbol"
    },
    "built-ins/Object/preventExtensions/symbol-object-contains-symbol-properties-strict.js": {
        "Symbol"
    },
    "built-ins/Proxy/isExtensible/trap-is-not-callable-realm.js": {"cross-realm"},
    "built-ins/Proxy/preventExtensions/return-false.js": {"Reflect"},
    "built-ins/Proxy/preventExtensions/return-true-target-is-not-extensible.js": {
        "Reflect"
    },
    "built-ins/Proxy/preventExtensions/trap-is-not-callable-realm.js": {
        "cross-realm"
    },
    "built-ins/Proxy/preventExtensions/trap-is-undefined-target-is-proxy.js": {
        "Reflect"
    },
    "built-ins/Proxy/preventExtensions/trap-is-undefined.js": {"Reflect"},
}
EXTENSIBILITY_FEATURES = {
    path: frozenset(
        ({"Proxy"} if path.startswith("built-ins/Proxy/") else set())
        | _EXTRA_FEATURES.get(path, set())
    )
    for path in EXTENSIBILITY_FILES
}

EXTENSIBILITY_MODULE_FILES = frozenset(
    {
        "built-ins/Proxy/preventExtensions/trap-is-undefined-target-is-proxy.js",
    }
)
