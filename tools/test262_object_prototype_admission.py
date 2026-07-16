"""Frozen Test262 Object.prototype feature admissions."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_object_prototype_admission.txt")
OBJECT_PROTOTYPE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

OBJECT_PROTOTYPE_FEATURES_BY_FILE = {
    "built-ins/Object/prototype/__defineGetter__/define-abrupt.js": frozenset(
        {"Proxy", "__getter__"}
    ),
    "built-ins/Object/prototype/__defineGetter__/getter-non-callable.js": frozenset(
        {"Symbol", "__getter__"}
    ),
    "built-ins/Object/prototype/__defineSetter__/define-abrupt.js": frozenset(
        {"Proxy", "__setter__"}
    ),
    "built-ins/Object/prototype/__defineSetter__/setter-non-callable.js": frozenset(
        {"Symbol", "__setter__"}
    ),
    "built-ins/Object/prototype/__lookupGetter__/lookup-own-get-err.js": frozenset(
        {"Proxy", "__getter__"}
    ),
    "built-ins/Object/prototype/__lookupGetter__/lookup-own-proto-err.js": frozenset(
        {"Proxy", "__getter__"}
    ),
    "built-ins/Object/prototype/__lookupGetter__/lookup-proto-get-err.js": frozenset(
        {"Proxy", "__getter__"}
    ),
    "built-ins/Object/prototype/__lookupGetter__/lookup-proto-proto-err.js": frozenset(
        {"Proxy", "__getter__"}
    ),
    "built-ins/Object/prototype/__lookupSetter__/lookup-own-get-err.js": frozenset(
        {"Proxy", "__setter__"}
    ),
    "built-ins/Object/prototype/__lookupSetter__/lookup-own-proto-err.js": frozenset(
        {"Proxy", "__setter__"}
    ),
    "built-ins/Object/prototype/__lookupSetter__/lookup-proto-get-err.js": frozenset(
        {"Proxy", "__setter__"}
    ),
    "built-ins/Object/prototype/__lookupSetter__/lookup-proto-proto-err.js": frozenset(
        {"Proxy", "__setter__"}
    ),
    "built-ins/Object/prototype/__proto__/get-abrupt.js": frozenset(
        {"Proxy", "__proto__"}
    ),
    "built-ins/Object/prototype/__proto__/set-abrupt.js": frozenset(
        {"Proxy", "__proto__"}
    ),
    "built-ins/Object/prototype/__proto__/set-invalid-value.js": frozenset(
        {"Symbol", "__proto__"}
    ),
    "built-ins/Object/prototype/__proto__/set-non-object.js": frozenset(
        {"Symbol", "__proto__"}
    ),
    "built-ins/Object/prototype/hasOwnProperty/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Object/prototype/hasOwnProperty/symbol_own_property.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Object/prototype/hasOwnProperty/symbol_property_toString.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Object/prototype/hasOwnProperty/symbol_property_valueOf.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Object/prototype/isPrototypeOf/arg-is-proxy.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Object/prototype/isPrototypeOf/builtin.js": frozenset(
        {"Reflect.construct"}
    ),
    "built-ins/Object/prototype/isPrototypeOf/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Object/prototype/isPrototypeOf/null-this-and-primitive-arg-returns-false.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Object/prototype/isPrototypeOf/undefined-this-and-primitive-arg-returns-false.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Object/prototype/propertyIsEnumerable/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Object/prototype/propertyIsEnumerable/symbol_own_property.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Object/prototype/propertyIsEnumerable/symbol_property_toString.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Object/prototype/propertyIsEnumerable/symbol_property_valueOf.js": frozenset(
        {"Symbol"}
    ),
    "built-ins/Object/prototype/toLocaleString/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Object/prototype/toString/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Object/prototype/toString/proxy-array.js": frozenset({"Proxy"}),
    "built-ins/Object/prototype/toString/proxy-function.js": frozenset(
        {"Proxy", "Symbol.toStringTag", "async-functions", "generators"}
    ),
    "built-ins/Object/prototype/toString/proxy-function-async.js": frozenset(
        {"Proxy", "Symbol.toStringTag", "async-functions"}
    ),
    "built-ins/Object/prototype/toString/proxy-revoked-during-get-call.js": frozenset(
        {"Proxy"}
    ),
    "built-ins/Object/prototype/toString/proxy-revoked.js": frozenset({"Proxy"}),
    "built-ins/Object/prototype/toString/symbol-tag-array-builtin.js": frozenset(
        {"Symbol.iterator", "Symbol.toStringTag", "iterator-helpers"}
    ),
    "built-ins/Object/prototype/toString/symbol-tag-generators-builtin.js": frozenset(
        {"Symbol.iterator", "Symbol.toStringTag", "generators"}
    ),
    "built-ins/Object/prototype/toString/symbol-tag-map-builtin.js": frozenset(
        {"Map", "Symbol.iterator", "Symbol.toStringTag", "iterator-helpers"}
    ),
    "built-ins/Object/prototype/toString/symbol-tag-non-str-proxy-function.js": frozenset(
        {"Proxy", "Symbol.toStringTag", "async-functions", "generators"}
    ),
    "built-ins/Object/prototype/toString/symbol-tag-promise-builtin.js": frozenset(
        {"Promise", "Symbol.toStringTag"}
    ),
    "built-ins/Object/prototype/toString/symbol-tag-set-builtin.js": frozenset(
        {"Set", "Symbol.iterator", "Symbol.toStringTag", "iterator-helpers"}
    ),
    "built-ins/Object/prototype/toString/symbol-tag-string-builtin.js": frozenset(
        {"Symbol.iterator", "Symbol.toStringTag", "iterator-helpers"}
    ),
    "built-ins/Object/prototype/toString/symbol-tag-weakmap-builtin.js": frozenset(
        {"Symbol.toStringTag", "WeakMap"}
    ),
    "built-ins/Object/prototype/toString/symbol-tag-weakset-builtin.js": frozenset(
        {"Symbol.toStringTag", "WeakSet"}
    ),
    "built-ins/Object/prototype/valueOf/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
}
