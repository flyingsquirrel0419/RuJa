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
    "built-ins/Date/is-a-constructor.js": frozenset({"Reflect.construct"}),
    "built-ins/Date/subclassing.js": frozenset({"Reflect"}),
    "built-ins/Date/proto-from-ctor-realm-zero.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
    "built-ins/Date/proto-from-ctor-realm-one.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
    "built-ins/Date/proto-from-ctor-realm-two.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
    "built-ins/Function/is-a-constructor.js": frozenset({"Reflect.construct"}),
    "built-ins/Function/proto-from-ctor-realm-prototype.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
    "built-ins/Function/proto-from-ctor-realm.js": frozenset(
        {"cross-realm", "Reflect"}
    ),
    "built-ins/AsyncFunction/is-a-constructor.js": frozenset(
        {"Reflect.construct"}
    ),
    "built-ins/AsyncFunction/proto-from-ctor-realm.js": frozenset(
        {"async-functions", "cross-realm", "Reflect", "Symbol"}
    ),
    "built-ins/GeneratorFunction/is-a-constructor.js": frozenset(
        {"Reflect.construct"}
    ),
    "built-ins/AsyncGeneratorFunction/is-a-constructor.js": frozenset(
        {"Reflect.construct"}
    ),
}
