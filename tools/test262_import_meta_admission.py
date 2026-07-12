"""Frozen Test262 import.meta files admitted after parser/runtime auditing."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_import_meta_admission.txt")
IMPORT_META_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
