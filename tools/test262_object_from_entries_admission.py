"""Frozen feature-gated Test262 Object.fromEntries files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_object_from_entries_admission.txt")
OBJECT_FROM_ENTRIES_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_SYMBOL_ITERATOR = frozenset({"Object.fromEntries", "Symbol.iterator"})
OBJECT_FROM_ENTRIES_FEATURES = {
    **{
        f"built-ins/Object/fromEntries/{name}": _SYMBOL_ITERATOR
        for name in (
            "evaluation-order.js",
            "iterator-closed-for-null-entry.js",
            "iterator-closed-for-string-entry.js",
            "iterator-closed-for-throwing-entry-key-accessor.js",
            "iterator-closed-for-throwing-entry-key-tostring.js",
            "iterator-closed-for-throwing-entry-value-accessor.js",
            "iterator-not-closed-for-next-returning-non-object.js",
            "iterator-not-closed-for-throwing-done-accessor.js",
            "iterator-not-closed-for-throwing-next.js",
            "iterator-not-closed-for-uncallable-next.js",
            "uses-keys-not-iterator.js",
        )
    },
    "built-ins/Object/fromEntries/not-a-constructor.js": frozenset(
        {"Object.fromEntries", "Reflect.construct", "arrow-function"}
    ),
    "built-ins/Object/fromEntries/supports-symbols.js": frozenset(
        {"Object.fromEntries", "Symbol"}
    ),
}

if frozenset(OBJECT_FROM_ENTRIES_FEATURES) != OBJECT_FROM_ENTRIES_FILES:
    raise RuntimeError("Object.fromEntries admission manifest and feature map differ")
