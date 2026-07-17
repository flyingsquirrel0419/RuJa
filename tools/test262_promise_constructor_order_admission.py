"""Frozen Test262 Promise constructor ordering admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_promise_constructor_order_admission.txt"
)
PROMISE_CONSTRUCTOR_ORDER_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
PROMISE_CONSTRUCTOR_ORDER_FEATURES = {
    relative: frozenset({"Reflect", "Reflect.construct"})
    for relative in PROMISE_CONSTRUCTOR_ORDER_FILES
}
