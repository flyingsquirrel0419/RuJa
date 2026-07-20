"""Frozen Test262 Proxy [[GetOwnProperty]] and for-in files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_proxy_for_in_admission.txt")
PROXY_FOR_IN_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Proxy/enumerate/removed-does-not-trigger.js": {
        "Symbol",
        "Symbol.iterator",
    },
    "built-ins/Proxy/getOwnPropertyDescriptor/result-type-is-not-object-nor-undefined-realm.js": {
        "cross-realm",
    },
    "built-ins/Proxy/getOwnPropertyDescriptor/result-type-is-not-object-nor-undefined.js": {
        "Symbol",
    },
    "built-ins/Proxy/getOwnPropertyDescriptor/resultdesc-is-not-configurable-not-writable-targetdesc-is-writable.js": {
        "proxy-missing-checks",
    },
    "built-ins/Proxy/getOwnPropertyDescriptor/trap-is-not-callable-realm.js": {
        "cross-realm",
    },
}
PROXY_FOR_IN_FEATURES = {
    path: frozenset({"Proxy"} | _EXTRA_FEATURES.get(path, set()))
    for path in PROXY_FOR_IN_FILES
}
