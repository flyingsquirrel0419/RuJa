"""Frozen Test262 Object constructor and NewTarget files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_object_constructor_admission.txt")
OBJECT_CONSTRUCTOR_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
OBJECT_CONSTRUCTOR_FEATURES = {
    "built-ins/Object/is-a-constructor.js": frozenset({"Reflect.construct"}),
    "built-ins/Object/proto-from-ctor-realm.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
    "built-ins/Object/subclass-object-arg.js": frozenset(
        {"class", "Reflect", "Reflect.construct"}
    ),
}
