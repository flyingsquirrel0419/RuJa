"""Frozen feature-gated Test262 Set constructor files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_set_constructor_admission.txt")
SET_CONSTRUCTOR_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_SYMBOL_ITERATOR_FILES = {
    "built-ins/Set/set-iterator-close-after-add-failure.js",
    "built-ins/Set/set-iterator-next-failure.js",
    "built-ins/Set/set-iterator-value-failure.js",
}
SET_CONSTRUCTOR_FEATURES = {
    **{path: frozenset({"Symbol.iterator"}) for path in _SYMBOL_ITERATOR_FILES},
    "built-ins/Set/proto-from-ctor-realm.js": frozenset({"cross-realm", "Reflect"}),
}

if frozenset(SET_CONSTRUCTOR_FEATURES) != SET_CONSTRUCTOR_FILES:
    raise RuntimeError("Set constructor admission manifest and feature map differ")
