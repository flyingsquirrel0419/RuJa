"""Frozen Test262 Array.fromAsync admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_from_async_admission.txt")
ARRAY_FROM_ASYNC_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
ARRAY_FROM_ASYNC_FEATURES = {
    relative: frozenset(
        {"Array.fromAsync"}
        | ({"Reflect.construct"} if relative.endswith("/not-a-constructor.js") else set())
        | ({"BigInt", "Symbol"} if relative.endswith("/mapfn-not-callable.js") else set())
    )
    for relative in ARRAY_FROM_ASYNC_FILES
}
