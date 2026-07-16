"""Frozen Test262 Promise Realm admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_promise_realm_admission.txt")
PROMISE_REALM_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
PROMISE_REALM_FEATURES = {
    "built-ins/Promise/proto-from-ctor-realm.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
}
