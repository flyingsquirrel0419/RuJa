"""Frozen Annex B legacy Date Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_annex_b_date_admission.txt")
ANNEX_B_DATE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_SYMBOL_FILES = {
    "annexB/built-ins/Date/prototype/setYear/year-nan.js",
    "annexB/built-ins/Date/prototype/setYear/year-to-number-err.js",
}

ANNEX_B_DATE_FEATURES = {
    path: frozenset({"Symbol"} if path in _SYMBOL_FILES else {"Reflect.construct", "arrow-function"})
    for path in ANNEX_B_DATE_FILES
}

if not _SYMBOL_FILES <= ANNEX_B_DATE_FILES:
    raise RuntimeError("Annex B Date Symbol admission is outside the manifest")
