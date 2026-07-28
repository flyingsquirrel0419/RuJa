"""Frozen feature-gated Test262 Map.groupBy files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_map_group_by_admission.txt")
MAP_GROUP_BY_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
MAP_GROUP_BY_FEATURES = {
    "built-ins/Map/groupBy/groupLength.js": frozenset(
        {"array-grouping", "Map", "Symbol.iterator"}
    ),
    "built-ins/Map/groupBy/iterator-next-throws.js": frozenset(
        {"array-grouping", "Map", "Symbol.iterator"}
    ),
}

if frozenset(MAP_GROUP_BY_FEATURES) != MAP_GROUP_BY_FILES:
    raise RuntimeError("Map.groupBy admission manifest and feature map differ")
