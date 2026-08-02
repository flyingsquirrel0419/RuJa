"""Frozen feature-gated Test262 SuppressedError files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_suppressed_error_admission.txt")
SUPPRESSED_ERROR_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_BASE = frozenset({"explicit-resource-management"})
SUPPRESSED_ERROR_FEATURES = {
    "built-ins/SuppressedError/is-a-constructor.js": _BASE | {"Reflect.construct"},
    "built-ins/SuppressedError/length.js": _BASE,
    "built-ins/SuppressedError/message-method-prop-cast.js": _BASE,
    "built-ins/SuppressedError/message-method-prop.js": _BASE,
    "built-ins/SuppressedError/message-tostring-abrupt-symbol.js": _BASE
    | {"Symbol", "Symbol.toPrimitive"},
    "built-ins/SuppressedError/message-tostring-abrupt.js": _BASE
    | {"Symbol.toPrimitive"},
    "built-ins/SuppressedError/message-undefined-no-prop.js": _BASE,
    "built-ins/SuppressedError/name.js": _BASE,
    "built-ins/SuppressedError/newtarget-is-undefined.js": _BASE,
    "built-ins/SuppressedError/newtarget-proto-custom.js": _BASE
    | {"Reflect.construct"},
    "built-ins/SuppressedError/newtarget-proto-fallback.js": _BASE | {"Symbol"},
    "built-ins/SuppressedError/newtarget-proto.js": _BASE,
    "built-ins/SuppressedError/order-of-args-evaluation.js": _BASE
    | {"Symbol.iterator"},
    "built-ins/SuppressedError/prop-desc.js": _BASE,
    "built-ins/SuppressedError/proto-from-ctor-realm.js": _BASE
    | {"cross-realm", "Reflect", "Symbol"},
    "built-ins/SuppressedError/proto.js": _BASE,
    "built-ins/SuppressedError/prototype/constructor.js": _BASE,
    "built-ins/SuppressedError/prototype/errors-absent-on-prototype.js": _BASE,
    "built-ins/SuppressedError/prototype/message.js": _BASE,
    "built-ins/SuppressedError/prototype/name.js": _BASE,
    "built-ins/SuppressedError/prototype/prop-desc.js": _BASE,
    "built-ins/SuppressedError/prototype/proto.js": _BASE,
}

if frozenset(SUPPRESSED_ERROR_FEATURES) != SUPPRESSED_ERROR_FILES:
    raise RuntimeError("SuppressedError admission manifest and feature map differ")
