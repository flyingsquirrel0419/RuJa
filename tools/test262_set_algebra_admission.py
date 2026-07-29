"""Frozen feature-gated Test262 Set algebra files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_set_algebra_admission.txt")
SET_ALGEBRA_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
SET_ALGEBRA_FEATURES = {
    path: frozenset({"Reflect.construct", "set-methods"})
    for path in SET_ALGEBRA_FILES
}
