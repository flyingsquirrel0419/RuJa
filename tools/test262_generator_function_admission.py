"""Frozen Test262 synchronous GeneratorFunction admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_generator_function_admission.txt")
GENERATOR_FUNCTION_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
GENERATOR_FUNCTION_FEATURES = {
    "built-ins/GeneratorFunction/proto-from-ctor-realm-prototype.js": frozenset(
        {"generators", "cross-realm", "Reflect"}
    ),
    "built-ins/GeneratorFunction/proto-from-ctor-realm.js": frozenset(
        {"generators", "cross-realm", "Reflect"}
    ),
    "language/expressions/generators/eval-body-proto-realm.js": frozenset(
        {"generators", "cross-realm"}
    ),
}
