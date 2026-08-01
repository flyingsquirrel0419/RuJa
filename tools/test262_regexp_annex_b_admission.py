"""Frozen residual Annex B RegExp Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_regexp_annex_b_admission.txt")
REGEXP_ANNEX_B_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    if path.endswith("non-unicode-malformed-lookbehind.js"):
        return frozenset({"regexp-named-groups", "regexp-lookbehind"})
    if "/named-groups/" in path:
        return frozenset({"regexp-named-groups"})
    return frozenset({"generators"})


REGEXP_ANNEX_B_FEATURES = {
    path: _features(path) for path in REGEXP_ANNEX_B_FILES
}

if len(REGEXP_ANNEX_B_FILES) != 4:
    raise RuntimeError("residual Annex B RegExp admission must contain four files")
if frozenset(REGEXP_ANNEX_B_FEATURES) != REGEXP_ANNEX_B_FILES:
    raise RuntimeError("residual Annex B RegExp admission manifest is out of sync")
if any(
    not path.startswith("annexB/built-ins/RegExp/")
    for path in REGEXP_ANNEX_B_FILES
):
    raise RuntimeError("residual Annex B RegExp admission contains an unrelated path")
