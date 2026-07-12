"""Frozen Test262 JSON.parse files admitted after reviver/source auditing."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_json_parse_admission.txt")
JSON_PARSE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
