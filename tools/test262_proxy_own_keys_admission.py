"""Frozen Test262 Proxy and Reflect [[OwnPropertyKeys]] files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_proxy_own_keys_admission.txt")
PROXY_OWN_KEYS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Proxy/ownKeys/call-parameters-object-getownpropertysymbols.js": {"Symbol"},
    "built-ins/Proxy/ownKeys/return-duplicate-symbol-entries-throws.js": {"Symbol"},
    "built-ins/Proxy/ownKeys/return-not-list-object-throws.js": {"Symbol"},
    "built-ins/Proxy/ownKeys/return-not-list-object-throws-realm.js": {
        "Symbol",
        "cross-realm",
    },
    "built-ins/Proxy/ownKeys/trap-is-missing-target-is-proxy.js": {
        "Symbol",
        "Reflect",
    },
    "built-ins/Proxy/ownKeys/trap-is-not-callable-realm.js": {"cross-realm"},
    "built-ins/Proxy/ownKeys/trap-is-undefined-target-is-proxy.js": {
        "Symbol",
        "Reflect",
    },
    "built-ins/Reflect/ownKeys/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/ownKeys/order-after-define-property.js": {"Symbol"},
    "built-ins/Reflect/ownKeys/return-abrupt-from-result.js": {"Proxy"},
    "built-ins/Reflect/ownKeys/return-on-corresponding-order-large-index.js": {
        "Symbol",
        "computed-property-names",
    },
    "built-ins/Reflect/ownKeys/return-on-corresponding-order.js": {"Symbol"},
    "built-ins/Reflect/ownKeys/target-is-symbol-throws.js": {"Symbol"},
}
PROXY_OWN_KEYS_FEATURES = {
    path: frozenset(
        ({"Proxy"} if path.startswith("built-ins/Proxy/") else {"Reflect"})
        | _EXTRA_FEATURES.get(path, set())
    )
    for path in PROXY_OWN_KEYS_FILES
}
