"""Frozen non-module ShadowRealm Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_shadowrealm_admission.txt")
SHADOWREALM_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

SHADOWREALM_MODULE_FILES = frozenset(
    {
        "built-ins/ShadowRealm/prototype/importValue/import-value.js",
        "built-ins/ShadowRealm/prototype/importValue/throws-if-import-value-does-not-exist.js",
        "built-ins/ShadowRealm/prototype/importValue/throws-typeerror-import-syntax-error.js",
        "built-ins/ShadowRealm/prototype/importValue/throws-typeerror-import-throws.js",
    }
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
if not SHADOWREALM_MODULE_FILES <= SHADOWREALM_FILES:
    raise RuntimeError("ShadowRealm Module admission is outside the manifest")
