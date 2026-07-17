"""Frozen Test262 native [[Construct]] and constructor-entry files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_native_construct_admission.txt")
NATIVE_CONSTRUCT_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
NATIVE_CONSTRUCT_FEATURES = {
    "built-ins/BigInt/is-a-constructor.js": frozenset({"Reflect.construct"}),
    "built-ins/Symbol/is-constructor.js": frozenset(
        {"Symbol", "Reflect.construct"}
    ),
    "built-ins/Proxy/constructor.js": frozenset({"Proxy"}),
    "built-ins/Proxy/proxy-newtarget.js": frozenset({"Proxy"}),
    "built-ins/Proxy/proxy-undefined-newtarget.js": frozenset({"Proxy"}),
    "built-ins/String/is-a-constructor.js": frozenset({"Reflect.construct"}),
    "built-ins/String/proto-from-ctor-realm.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
    "built-ins/String/symbol-string-coercion.js": frozenset({"Symbol"}),
    "built-ins/String/symbol-wrapping.js": frozenset({"Symbol"}),
    "built-ins/Number/is-a-constructor.js": frozenset({"Reflect.construct"}),
    "built-ins/Number/proto-from-ctor-realm.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
    "built-ins/Number/return-abrupt-tonumber-value-symbol.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Boolean/is-a-constructor.js": frozenset({"Reflect.construct"}),
    "built-ins/Boolean/proto-from-ctor-realm.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
    "built-ins/Boolean/symbol-coercion.js": frozenset({"Symbol"}),
}
