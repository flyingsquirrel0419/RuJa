"""Frozen Function.prototype.toString Test262 admission."""

from pathlib import Path


_PREFIX = "built-ins/Function/prototype/toString/"
_MANIFEST = Path(__file__).with_name("test262_function_tostring_admission.txt")
FUNCTION_TOSTRING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_FEATURES_BY_NAME = {
    "AsyncFunction.js": {"async-functions"},
    "AsyncGenerator.js": {"async-iteration"},
    "GeneratorFunction.js": {"generators"},
    "async-arrow-function.js": {"async-functions"},
    "async-function-declaration.js": {"async-functions"},
    "async-function-expression.js": {"async-functions"},
    "async-generator-declaration.js": {"async-iteration"},
    "async-generator-expression.js": {"async-iteration"},
    "async-generator-method-class-expression-static.js": {"async-iteration"},
    "async-generator-method-class-expression.js": {"async-iteration"},
    "async-generator-method-class-statement-static.js": {"async-iteration"},
    "async-generator-method-class-statement.js": {"async-iteration"},
    "async-generator-method-object.js": {"async-iteration"},
    "async-method-class-expression-static.js": {"async-functions"},
    "async-method-class-expression.js": {"async-functions"},
    "async-method-class-statement-static.js": {"async-functions"},
    "async-method-class-statement.js": {"async-functions"},
    "async-method-object.js": {"async-functions"},
    "built-in-function-object.js": {"arrow-function", "Reflect", "Array.prototype.includes"},
    "not-a-constructor.js": {"Reflect.construct", "arrow-function"},
    "private-method-class-expression.js": {"class-methods-private"},
    "private-method-class-statement.js": {"class-methods-private"},
    "private-static-method-class-expression.js": {"class-static-methods-private"},
    "private-static-method-class-statement.js": {"class-static-methods-private"},
    "proxy-arrow-function.js": {"arrow-function", "Proxy"},
    "proxy-async-function.js": {"async-functions", "Proxy"},
    "proxy-async-generator-function.js": {"async-iteration", "Proxy"},
    "proxy-async-generator-method-definition.js": {"async-iteration", "Proxy"},
    "proxy-async-method-definition.js": {"async-functions", "Proxy"},
    "proxy-bound-function.js": {"Proxy"},
    "proxy-class.js": {"class", "Proxy"},
    "proxy-function-expression.js": {"Proxy"},
    "proxy-generator-function.js": {"generators", "Proxy"},
    "proxy-method-definition.js": {"Proxy"},
    "proxy-non-callable-throws.js": {"Proxy"},
}
FUNCTION_TOSTRING_FEATURES = {
    f"{_PREFIX}{name}": frozenset(features)
    for name, features in _FEATURES_BY_NAME.items()
}

if frozenset(FUNCTION_TOSTRING_FEATURES) != FUNCTION_TOSTRING_FILES:
    raise RuntimeError("Function.prototype.toString admission manifest is out of sync")
