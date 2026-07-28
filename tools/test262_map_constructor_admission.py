"""Frozen feature-gated Test262 Map constructor files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_map_constructor_admission.txt")
MAP_CONSTRUCTOR_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_SYMBOL_ITERATOR_FILES = {
    "built-ins/Map/iterator-close-after-set-failure.js",
    "built-ins/Map/iterator-close-failure-after-set-failure.js",
    "built-ins/Map/iterator-is-undefined-throws.js",
    "built-ins/Map/iterator-item-first-entry-returns-abrupt.js",
    "built-ins/Map/iterator-item-second-entry-returns-abrupt.js",
    "built-ins/Map/iterator-next-failure.js",
    "built-ins/Map/iterator-value-failure.js",
}
MAP_CONSTRUCTOR_FEATURES = {
    **{path: frozenset({"Symbol.iterator"}) for path in _SYMBOL_ITERATOR_FILES},
    "built-ins/Map/iterator-items-are-not-object.js": frozenset({"Symbol"}),
    "built-ins/Map/proto-from-ctor-realm.js": frozenset({"cross-realm", "Reflect"}),
}

if frozenset(MAP_CONSTRUCTOR_FEATURES) != MAP_CONSTRUCTOR_FILES:
    raise RuntimeError("Map constructor admission manifest and feature map differ")
