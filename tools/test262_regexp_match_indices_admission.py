"""Frozen Test262 RegExp match-indices named-group admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_regexp_match_indices_admission.txt")
REGEXP_MATCH_INDICES_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
REGEXP_MATCH_INDICES_FEATURES = {
    relative: frozenset({"regexp-named-groups"})
    for relative in REGEXP_MATCH_INDICES_FILES
}

if frozenset(REGEXP_MATCH_INDICES_FEATURES) != REGEXP_MATCH_INDICES_FILES:
    raise RuntimeError("RegExp match-indices admission manifest is out of sync")
if any(
    not relative.startswith("built-ins/RegExp/match-indices/")
    for relative in REGEXP_MATCH_INDICES_FILES
):
    raise RuntimeError("RegExp match-indices admission contains an unrelated path")
