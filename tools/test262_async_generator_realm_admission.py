"""Frozen Test262 asynchronous generator Realm admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_async_generator_realm_admission.txt")
ASYNC_GENERATOR_REALM_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
ASYNC_GENERATOR_REALM_FEATURES = {
    "built-ins/AsyncGeneratorFunction/proto-from-ctor-realm-prototype.js": frozenset(
        {"async-iteration", "cross-realm", "Reflect"}
    ),
    "built-ins/AsyncGeneratorFunction/proto-from-ctor-realm.js": frozenset(
        {"async-iteration", "cross-realm", "Reflect", "Symbol"}
    ),
    "language/expressions/async-generator/eval-body-proto-realm.js": frozenset(
        {"async-iteration", "cross-realm", "Symbol"}
    ),
}
