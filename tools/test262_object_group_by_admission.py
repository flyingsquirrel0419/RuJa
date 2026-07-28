"""Frozen feature-gated Test262 Object.groupBy files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_object_group_by_admission.txt")
OBJECT_GROUP_BY_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
OBJECT_GROUP_BY_FEATURES = {
    "built-ins/Object/groupBy/iterator-next-throws.js": frozenset(
        {"array-grouping", "Symbol.iterator"}
    ),
}

if frozenset(OBJECT_GROUP_BY_FEATURES) != OBJECT_GROUP_BY_FILES:
    raise RuntimeError("Object.groupBy admission manifest and feature map differ")
