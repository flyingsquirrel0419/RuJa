"""Frozen Test262 dynamic-import files admitted after host-loader auditing."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_dynamic_import_admission.txt")
DYNAMIC_IMPORT_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
