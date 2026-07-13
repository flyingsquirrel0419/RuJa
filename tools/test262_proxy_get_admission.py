"""Frozen Test262 Proxy and Reflect [[Get]] files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_proxy_get_admission.txt")
PROXY_GET_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Proxy/get/trap-is-not-callable-realm.js": {"cross-realm"},
    "built-ins/Proxy/get/trap-is-null-target-is-proxy.js": {"Symbol"},
    "built-ins/Reflect/get/not-a-constructor.js": {"Reflect.construct"},
    "built-ins/Reflect/get/return-value-from-symbol-key.js": {"Symbol"},
    "built-ins/Reflect/get/target-is-symbol-throws.js": {"Symbol"},
}
PROXY_GET_FEATURES = {
    path: frozenset(
        ({"Proxy"} if path.startswith("built-ins/Proxy/") else {"Reflect"})
        | _EXTRA_FEATURES.get(path, set())
    )
    for path in PROXY_GET_FILES
}
