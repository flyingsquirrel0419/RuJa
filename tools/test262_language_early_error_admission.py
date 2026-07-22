"""Frozen Test262 residual language early-error metadata."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_language_early_error_admission.txt")
LANGUAGE_EARLY_ERROR_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
LANGUAGE_EARLY_ERROR_FEATURES = {
    "language/statements/labeled/decl-gen.js": frozenset({"generators"}),
    "language/statements/labeled/decl-async-function.js": frozenset(
        {"async-functions"}
    ),
    "language/statements/labeled/decl-async-generator.js": frozenset(
        {"async-iteration"}
    ),
    "language/expressions/class/class-name-ident-await-escaped-module.js": frozenset(),
    "language/statements/class/class-name-ident-await-escaped-module.js": frozenset(),
}
LANGUAGE_EARLY_ERROR_MODULE_FILES = frozenset(
    {
        "language/expressions/class/class-name-ident-await-escaped-module.js",
        "language/statements/class/class-name-ident-await-escaped-module.js",
    }
)

if frozenset(LANGUAGE_EARLY_ERROR_FEATURES) != LANGUAGE_EARLY_ERROR_FILES:
    raise RuntimeError("language early-error admission manifest and feature map differ")
if not LANGUAGE_EARLY_ERROR_MODULE_FILES <= LANGUAGE_EARLY_ERROR_FILES:
    raise RuntimeError("language early-error module files are not admitted")
