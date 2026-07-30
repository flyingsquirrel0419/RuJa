"""Exact residual Test262 class built-in subclass feature admissions."""


CLASS_SUBCLASS_BUILTIN_FEATURES_BY_FILE = {
    "language/expressions/class/subclass-builtins/subclass-SharedArrayBuffer.js": frozenset(
        {"SharedArrayBuffer"}
    ),
    "language/expressions/class/subclass-builtins/subclass-WeakRef.js": frozenset(
        {"WeakRef"}
    ),
    "language/statements/class/subclass-builtins/subclass-SharedArrayBuffer.js": frozenset(
        {"SharedArrayBuffer"}
    ),
    "language/statements/class/subclass-builtins/subclass-WeakRef.js": frozenset(
        {"WeakRef"}
    ),
}
CLASS_SUBCLASS_BUILTIN_FILES = frozenset(
    CLASS_SUBCLASS_BUILTIN_FEATURES_BY_FILE
)
