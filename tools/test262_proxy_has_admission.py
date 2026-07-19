"""Frozen complete direct Test262 Proxy [[HasProperty]] files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_proxy_has_admission.txt")
PROXY_HAS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Proxy/has/trap-is-missing-target-is-proxy.js": {
        "Reflect",
        "Symbol.replace",
    },
    "built-ins/Proxy/has/trap-is-not-callable-realm.js": {"cross-realm"},
    "built-ins/Proxy/has/trap-is-null-target-is-proxy.js": {
        "Array.prototype.includes",
        "Reflect",
        "Symbol",
    },
    "built-ins/Proxy/has/trap-is-undefined-target-is-proxy.js": {"Reflect"},
}

PROXY_HAS_FEATURES = {
    path: frozenset({"Proxy"} | _EXTRA_FEATURES.get(path, set()))
    for path in PROXY_HAS_FILES
}
