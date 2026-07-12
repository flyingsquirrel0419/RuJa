"""Frozen Test262 JSON.stringify files admitted after serialization auditing."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_json_stringify_admission.txt")
JSON_STRINGIFY_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
