"""Frozen Test262 Proxy and Reflect [[Delete]] files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_proxy_delete_admission.txt")
PROXY_DELETE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Proxy/deleteProperty/boolean-trap-result-boolean-false.js": {"Reflect"},
    "built-ins/Proxy/deleteProperty/boolean-trap-result-boolean-true.js": {"Reflect"},
    "built-ins/Proxy/deleteProperty/return-false-strict.js": {"Reflect"},
    "built-ins/Proxy/deleteProperty/targetdesc-is-configurable-target-is-not-extensible.js": {
        "Reflect",
        "proxy-missing-checks",
    },
    "built-ins/Proxy/deleteProperty/trap-is-missing-target-is-proxy.js": {"Reflect"},
    "built-ins/Proxy/deleteProperty/trap-is-not-callable-realm.js": {"cross-realm"},
    "built-ins/Proxy/deleteProperty/trap-is-null-target-is-proxy.js": {"Reflect"},
    "built-ins/Proxy/deleteProperty/trap-is-undefined-strict.js": {"Reflect"},
    "built-ins/Proxy/deleteProperty/trap-is-undefined-target-is-proxy.js": {"Reflect"},
    "built-ins/Reflect/deleteProperty/delete-symbol-properties.js": {"Symbol"},
    "built-ins/Reflect/deleteProperty/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/deleteProperty/return-abrupt-from-result.js": {"Proxy"},
    "built-ins/Reflect/deleteProperty/target-is-symbol-throws.js": {"Symbol"},
}
PROXY_DELETE_FEATURES = {
    path: frozenset(
        ({"Proxy"} if path.startswith("built-ins/Proxy/") else {"Reflect"})
        | _EXTRA_FEATURES.get(path, set())
    )
    for path in PROXY_DELETE_FILES
}
