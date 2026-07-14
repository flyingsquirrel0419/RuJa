"""Frozen Test262 computed public/static class-field files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_class_computed_field_admission.txt")
CLASS_COMPUTED_FIELD_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
