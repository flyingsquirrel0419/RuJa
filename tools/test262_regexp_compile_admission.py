"""Frozen Annex B RegExp.prototype.compile Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_regexp_compile_admission.txt")
REGEXP_COMPILE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_DUPLICATE_NAME_FILE = (
    "annexB/built-ins/RegExp/prototype/compile/"
    "duplicate-named-capturing-groups-syntax.js"
)
REGEXP_COMPILE_FEATURES = {
    path: frozenset(
        {"regexp-duplicate-named-groups"}
        if path == _DUPLICATE_NAME_FILE
        else {"Symbol"}
    )
    for path in REGEXP_COMPILE_FILES
}

if len(REGEXP_COMPILE_FILES) != 4:
    raise RuntimeError("RegExp compile admission must contain exactly four files")
if frozenset(REGEXP_COMPILE_FEATURES) != REGEXP_COMPILE_FILES:
    raise RuntimeError("RegExp compile admission manifest is out of sync")
if any(
    not path.startswith("annexB/built-ins/RegExp/prototype/compile/")
    for path in REGEXP_COMPILE_FILES
):
    raise RuntimeError("RegExp compile admission contains an unrelated path")
