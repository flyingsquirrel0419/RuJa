"""Frozen Test262 primitive-base Reference files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_reference_primitive_admission.txt")
REFERENCE_PRIMITIVE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
