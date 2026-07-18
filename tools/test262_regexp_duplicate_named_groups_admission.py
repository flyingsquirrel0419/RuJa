"""Frozen Test262 RegExp duplicate named-group admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_regexp_duplicate_named_groups_admission.txt"
)
REGEXP_DUPLICATE_NAMED_GROUPS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
REGEXP_DUPLICATE_NAMED_GROUPS_FEATURES = {
    relative: frozenset(
        {
            "regexp-named-groups"
            if relative.startswith("language/literals/regexp/named-groups/")
            else "regexp-duplicate-named-groups"
        }
    )
    for relative in REGEXP_DUPLICATE_NAMED_GROUPS_FILES
}

if (
    frozenset(REGEXP_DUPLICATE_NAMED_GROUPS_FEATURES)
    != REGEXP_DUPLICATE_NAMED_GROUPS_FILES
):
    raise RuntimeError("RegExp duplicate named-group admission manifest is out of sync")
if any(
    not relative.startswith(
        (
            "built-ins/RegExp/",
            "built-ins/String/prototype/match/",
            "language/literals/regexp/named-groups/",
        )
    )
    for relative in REGEXP_DUPLICATE_NAMED_GROUPS_FILES
):
    raise RuntimeError("RegExp duplicate named-group admission contains an unrelated path")
