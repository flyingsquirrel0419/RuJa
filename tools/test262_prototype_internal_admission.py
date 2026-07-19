"""Frozen Test262 Object and Proxy prototype internal-method files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_prototype_internal_admission.txt")
PROTOTYPE_INTERNAL_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Object/setPrototypeOf/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Object/setPrototypeOf/o-not-obj.js": {"Symbol"},
    "built-ins/Object/setPrototypeOf/proto-not-obj.js": {"Symbol"},
    "built-ins/Proxy/getPrototypeOf/trap-is-not-callable-realm.js": {
        "cross-realm"
    },
    "built-ins/Proxy/getPrototypeOf/trap-result-neither-object-nor-null-throws-symbol.js": {
        "Symbol"
    },
    "built-ins/Proxy/setPrototypeOf/internals-call-order.js": {
        "Reflect",
        "Reflect.setPrototypeOf",
    },
    "built-ins/Proxy/setPrototypeOf/not-extensible-target-not-same-target-prototype.js": {
        "Reflect",
        "Reflect.setPrototypeOf",
    },
    "built-ins/Proxy/setPrototypeOf/not-extensible-target-same-target-prototype.js": {
        "Reflect",
        "Reflect.setPrototypeOf",
    },
    "built-ins/Proxy/setPrototypeOf/toboolean-trap-result-false.js": {
        "Reflect",
        "Reflect.setPrototypeOf",
    },
    "built-ins/Proxy/setPrototypeOf/toboolean-trap-result-true-target-is-extensible.js": {
        "Reflect",
        "Reflect.setPrototypeOf",
        "Symbol",
    },
    "built-ins/Proxy/setPrototypeOf/trap-is-not-callable-realm.js": {
        "cross-realm"
    },
    "built-ins/Proxy/setPrototypeOf/trap-is-not-callable.js": {
        "Reflect",
        "Reflect.setPrototypeOf",
    },
}

PROTOTYPE_INTERNAL_FEATURES = {
    path: frozenset(
        (
            {"Proxy"}
            if path.startswith("built-ins/Proxy/")
            or path == "built-ins/Object/setPrototypeOf/set-error.js"
            else set()
        )
        | _EXTRA_FEATURES.get(path, set())
    )
    for path in PROTOTYPE_INTERNAL_FILES
}
