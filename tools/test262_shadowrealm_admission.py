"""Frozen non-module ShadowRealm Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_shadowrealm_admission.txt")
SHADOWREALM_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES = {
    "built-ins/ShadowRealm/constructor.js": {"Reflect.construct"},
    "built-ins/ShadowRealm/prototype/Symbol.toStringTag.js": {"Symbol.toStringTag"},
    "built-ins/ShadowRealm/prototype/evaluate/globalthis-config-only-properties.js": {
        "Array.prototype.includes"
    },
    "built-ins/ShadowRealm/prototype/evaluate/not-constructor.js": {
        "Reflect.construct"
    },
    "built-ins/ShadowRealm/prototype/evaluate/throws-error-from-ctor-realm.js": {
        "cross-realm",
        "Reflect",
    },
    "built-ins/ShadowRealm/prototype/evaluate/wrapped-function-proto-from-caller-realm.js": {
        "cross-realm",
        "Reflect",
    },
    "built-ins/ShadowRealm/prototype/evaluate/wrapped-function-throws-typeerror-from-caller-realm.js": {
        "cross-realm",
        "Reflect",
    },
    "built-ins/ShadowRealm/prototype/importValue/not-constructor.js": {
        "Reflect.construct"
    },
}

SHADOWREALM_FEATURES = {
    path: frozenset({"ShadowRealm"} | _EXTRA_FEATURES.get(path, set()))
    for path in SHADOWREALM_FILES
}

if not frozenset(_EXTRA_FEATURES) <= SHADOWREALM_FILES:
    raise RuntimeError("ShadowRealm admission extras are outside the manifest")
