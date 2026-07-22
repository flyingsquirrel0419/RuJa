"""Frozen Function.prototype.bind name/length Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_function_bind_admission.txt")
FUNCTION_BIND_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

FUNCTION_BIND_FEATURES = {
    "built-ins/Function/prototype/bind/instance-length-default-value.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Function/prototype/bind/instance-length-exceeds-int32.js": frozenset(),
    "built-ins/Function/prototype/bind/instance-length-prop-desc.js": frozenset(),
    "built-ins/Function/prototype/bind/instance-length-remaining-args.js": frozenset(),
    "built-ins/Function/prototype/bind/instance-length-tointeger.js": frozenset(),
    "built-ins/Function/prototype/bind/instance-name-chained.js": frozenset(),
    "built-ins/Function/prototype/bind/instance-name-error.js": frozenset(),
    "built-ins/Function/prototype/bind/instance-name-non-string.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Function/prototype/bind/instance-name.js": frozenset(),
}

if frozenset(FUNCTION_BIND_FEATURES) != FUNCTION_BIND_FILES:
    raise RuntimeError("Function.prototype.bind admission manifest is out of sync")
