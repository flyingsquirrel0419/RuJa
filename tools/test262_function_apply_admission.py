"""Frozen feature-gated Function.prototype.apply Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_function_apply_admission.txt")
FUNCTION_APPLY_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

FUNCTION_APPLY_FEATURES = {
    "built-ins/Function/prototype/apply/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Function/prototype/apply/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
}

if frozenset(FUNCTION_APPLY_FEATURES) != FUNCTION_APPLY_FILES:
    raise RuntimeError("Function.prototype.apply admission manifest is out of sync")
