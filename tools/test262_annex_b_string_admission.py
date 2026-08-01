"""Frozen Annex B String legacy-method Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_annex_b_string_admission.txt")
ANNEX_B_STRING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_SYMBOL_FILES = {
    "annexB/built-ins/String/prototype/substr/length-to-int-err.js",
    "annexB/built-ins/String/prototype/substr/start-to-int-err.js",
}

ANNEX_B_STRING_FEATURES = {
    path: frozenset({"Symbol"} if path in _SYMBOL_FILES else {"Reflect.construct", "arrow-function"})
    for path in ANNEX_B_STRING_FILES
}

if not _SYMBOL_FILES <= ANNEX_B_STRING_FILES:
    raise RuntimeError("Annex B String Symbol admission is outside the manifest")
