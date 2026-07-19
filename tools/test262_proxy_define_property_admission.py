"""Frozen Test262 Proxy [[DefineOwnProperty]] files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_proxy_define_property_admission.txt")
PROXY_DEFINE_PROPERTY_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Proxy/defineProperty/return-boolean-and-define-target.js": {
        "Reflect"
    },
    "built-ins/Proxy/defineProperty/targetdesc-configurable-desc-not-configurable-realm.js": {
        "cross-realm"
    },
    "built-ins/Proxy/defineProperty/targetdesc-not-compatible-descriptor-not-configurable-target-realm.js": {
        "cross-realm"
    },
    "built-ins/Proxy/defineProperty/targetdesc-not-compatible-descriptor-realm.js": {
        "cross-realm"
    },
    "built-ins/Proxy/defineProperty/targetdesc-not-configurable-writable-desc-not-writable.js": {
        "Reflect",
        "proxy-missing-checks",
    },
    "built-ins/Proxy/defineProperty/targetdesc-undefined-not-configurable-descriptor-realm.js": {
        "cross-realm"
    },
    "built-ins/Proxy/defineProperty/trap-is-missing-target-is-proxy.js": {
        "Reflect"
    },
    "built-ins/Proxy/defineProperty/trap-is-not-callable-realm.js": {
        "cross-realm"
    },
    "built-ins/Proxy/defineProperty/trap-is-null-target-is-proxy.js": {
        "Reflect"
    },
    "built-ins/Proxy/defineProperty/trap-is-undefined-target-is-proxy.js": {
        "Reflect"
    },
    "built-ins/Proxy/defineProperty/trap-return-is-false.js": {"Reflect"},
}

PROXY_DEFINE_PROPERTY_FEATURES = {
    path: frozenset({"Proxy"} | _EXTRA_FEATURES.get(path, set()))
    for path in PROXY_DEFINE_PROPERTY_FILES
}
