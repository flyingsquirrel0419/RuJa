"""Frozen Test262 JSON raw-text and branding files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_json_raw_admission.txt")
JSON_RAW_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
