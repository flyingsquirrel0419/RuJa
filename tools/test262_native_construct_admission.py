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
}
