"""Frozen complete direct Test262 Proxy [[Set]] files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_proxy_set_admission.txt")
PROXY_SET_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Proxy/set/boolean-trap-result-is-false-boolean-return-false.js": {
        "Reflect",
        "Reflect.set",
    },
    "built-ins/Proxy/set/boolean-trap-result-is-false-null-return-false.js": {
        "Reflect",
        "Reflect.set",
    },
    "built-ins/Proxy/set/boolean-trap-result-is-false-number-return-false.js": {
        "Reflect",
        "Reflect.set",
    },
    "built-ins/Proxy/set/boolean-trap-result-is-false-string-return-false.js": {
        "Reflect",
        "Reflect.set",
    },
    "built-ins/Proxy/set/boolean-trap-result-is-false-undefined-return-false.js": {
        "Reflect",
        "Reflect.set",
    },
    "built-ins/Proxy/set/call-parameters-prototype-dunder-proto.js": {"__proto__"},
    "built-ins/Proxy/set/return-true-target-property-accessor-is-configurable-set-is-undefined.js": {
        "Reflect",
        "Reflect.set",
    },
    "built-ins/Proxy/set/return-true-target-property-accessor-is-not-configurable.js": {
        "Reflect",
        "Reflect.set",
    },
    "built-ins/Proxy/set/return-true-target-property-is-not-configurable.js": {
        "Reflect",
        "Reflect.set",
    },
    "built-ins/Proxy/set/return-true-target-property-is-not-writable.js": {
        "Reflect",
        "Reflect.set",
    },
    "built-ins/Proxy/set/trap-is-missing-receiver-multiple-calls-index.js": {
        "Reflect"
    },
    "built-ins/Proxy/set/trap-is-missing-receiver-multiple-calls.js": {"Reflect"},
    "built-ins/Proxy/set/trap-is-missing-target-is-proxy.js": {"Reflect"},
    "built-ins/Proxy/set/trap-is-not-callable-realm.js": {"cross-realm"},
    "built-ins/Proxy/set/trap-is-null-target-is-proxy.js": {"Reflect"},
    "built-ins/Proxy/set/trap-is-undefined-target-is-proxy.js": {"Reflect"},
}

PROXY_SET_FEATURES = {
    path: frozenset({"Proxy"} | _EXTRA_FEATURES.get(path, set()))
    for path in PROXY_SET_FILES
}
