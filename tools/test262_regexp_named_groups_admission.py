"""Frozen Test262 RegExp named-group admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_regexp_named_groups_admission.txt")
REGEXP_NAMED_GROUPS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
REGEXP_NAMED_GROUPS_FEATURES = {
    relative: frozenset({"regexp-named-groups"})
    for relative in REGEXP_NAMED_GROUPS_FILES
}

if frozenset(REGEXP_NAMED_GROUPS_FEATURES) != REGEXP_NAMED_GROUPS_FILES:
    raise RuntimeError("RegExp named-group admission manifest is out of sync")
if any(
    not relative.startswith(
        (
            "built-ins/RegExp/named-groups/",
            "built-ins/RegExp/prototype/Symbol.replace/",
            "language/literals/regexp/named-groups/",
        )
    )
    for relative in REGEXP_NAMED_GROUPS_FILES
):
    raise RuntimeError("RegExp named-group admission contains an unrelated path")
