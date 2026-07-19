"""Frozen Test262 Reflect [[Set]] and [[HasProperty]] files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_reflect_set_has_admission.txt")
REFLECT_SET_HAS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/Reflect/has/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/has/return-abrupt-from-result.js": {"Proxy"},
    "built-ins/Reflect/has/symbol-property.js": {"Symbol"},
    "built-ins/Reflect/has/target-is-symbol-throws.js": {"Symbol"},
    "built-ins/Reflect/set/not-a-constructor.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "built-ins/Reflect/set/symbol-property.js": {"Symbol"},
    "built-ins/Reflect/set/target-is-symbol-throws.js": {"Symbol"},
}
REFLECT_SET_HAS_FEATURES = {
    path: frozenset(
        {"Reflect"}
        | (
            {"Reflect.set"}
            if path.startswith("built-ins/Reflect/set/")
            and path != "built-ins/Reflect/set/set.js"
            else set()
        )
        | _EXTRA_FEATURES.get(path, set())
    )
    for path in REFLECT_SET_HAS_FILES
}
