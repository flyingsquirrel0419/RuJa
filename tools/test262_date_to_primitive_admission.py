"""Frozen Test262 Date @@toPrimitive files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_date_to_primitive_admission.txt")
DATE_TO_PRIMITIVE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
