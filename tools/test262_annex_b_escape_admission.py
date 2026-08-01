"""Frozen Annex B global escape/unescape Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_annex_b_escape_admission.txt")
ANNEX_B_ESCAPE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_SYMBOL_FILES = {
    "annexB/built-ins/escape/to-string-err-symbol.js",
    "annexB/built-ins/unescape/to-string-err-symbol.js",
}

ANNEX_B_ESCAPE_FEATURES = {
    path: frozenset({"Symbol"} if path in _SYMBOL_FILES else {"Reflect.construct", "arrow-function"})
    for path in ANNEX_B_ESCAPE_FILES
}

if not _SYMBOL_FILES <= ANNEX_B_ESCAPE_FILES:
    raise RuntimeError("Annex B escape Symbol admission is outside the manifest")
