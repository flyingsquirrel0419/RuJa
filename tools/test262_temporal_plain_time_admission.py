"""Exact Test262 boundaries for the supported Temporal.PlainTime surface."""

from pathlib import Path


def _read_manifest(name):
    return frozenset(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


TEMPORAL_PLAIN_TIME_CORE_FILES = _read_manifest(
    "test262_temporal_plain_time_core_admission.txt"
)
TEMPORAL_PLAIN_TIME_FROM_FILES = _read_manifest(
    "test262_temporal_plain_time_from_admission.txt"
)
TEMPORAL_PLAIN_TIME_VALUE_OF_FILES = _read_manifest(
    "test262_temporal_plain_time_value_of_admission.txt"
)
TEMPORAL_PLAIN_TIME_EQUALS_FILES = _read_manifest(
    "test262_temporal_plain_time_equals_admission.txt"
)
TEMPORAL_PLAIN_TIME_COMPARE_FILES = _read_manifest(
    "test262_temporal_plain_time_compare_admission.txt"
)
TEMPORAL_PLAIN_TIME_TO_STRING_FILES = _read_manifest(
    "test262_temporal_plain_time_to_string_admission.txt"
)

_CORE_TEMPORAL_HELPERS = frozenset({
    "argument-convert.js",
    "basic.js",
    "hour-undefined.js",
    "microsecond-undefined.js",
    "millisecond-undefined.js",
    "minute-undefined.js",
    "nanosecond-undefined.js",
    "negative-zero.js",
    "second-undefined.js",
    "subclass.js",
})
_FROM_TEMPORAL_HELPERS = frozenset({
    "argument-object-leap-second.js",
    "argument-object.js",
    "argument-plaintime.js",
    "argument-string-calendar-annotation.js",
    "argument-string-date-with-utc-offset.js",
    "argument-string-leap-second.js",
    "argument-string-time-designator-required-for-disambiguation.js",
    "argument-string-time-separators.js",
    "argument-string-time-zone-annotation.js",
    "argument-string-unknown-annotation.js",
    "argument-string-with-time-designator.js",
    "argument-zoneddatetime-balance-negative-time-units.js",
    "argument-zoneddatetime-negative-epochnanoseconds.js",
    "leap-second.js",
    "options-object.js",
    "overflow-constrain.js",
    "overflow-reject.js",
    "overflow-undefined.js",
    "plaintime-propertybag-no-time-units.js",
    "subclassing-ignored.js",
})
_FROM_COMPARE_ARRAY = frozenset({
    "argument-plaindatetime.js",
    "argument-string.js",
    "infinity-throws-rangeerror.js",
    "observable-get-overflow-argument-string-invalid.js",
    "options-read-before-algorithmic-validation.js",
    "order-of-operations.js",
    "overflow-wrong-type.js",
})
_FROM_ARROW = frozenset({
    "argument-string-invalid.js",
    "argument-string-no-implicit-midnight.js",
    "argument-string-time-designator-required-for-disambiguation.js",
    "argument-string-trailing-junk.js",
    "argument-string-with-time-designator.js",
    "argument-string-with-utc-designator.js",
    "year-zero.js",
})
_EQUALS_ARROW = frozenset({
    "argument-string-no-implicit-midnight.js",
    "argument-string-time-designator-required-for-disambiguation.js",
    "argument-string-with-time-designator.js",
    "argument-string-with-utc-designator.js",
    "year-zero.js",
})
_COMPARE_ARROW = frozenset({
    "argument-string-no-implicit-midnight.js",
    "argument-string-time-designator-required-for-disambiguation.js",
    "argument-string-with-time-designator.js",
    "argument-string-with-utc-designator.js",
})
_TO_STRING_COMPARE_ARRAY = frozenset({
    "fractionalseconddigits-wrong-type.js",
    "options-read-before-algorithmic-validation.js",
    "order-of-operations.js",
    "roundingmode-wrong-type.js",
    "smallestunit-wrong-type.js",
})


def _features(path, cohort):
    name = Path(path).name
    features = {"Temporal"}
    if cohort == "core" and name == "branding.js":
        features.add("Symbol")
    elif cohort == "from":
        if name in _FROM_ARROW:
            features.add("arrow-function")
        if name in {"argument-wrong-type.js", "options-wrong-type.js"}:
            features.update(("BigInt", "Symbol"))
        if name == "not-a-constructor.js":
            features.add("Reflect.construct")
    elif cohort == "valueOf":
        if name == "branding.js":
            features.add("Symbol")
        if name == "not-a-constructor.js":
            features.add("Reflect.construct")
    elif cohort == "equals":
        if name in _EQUALS_ARROW:
            features.add("arrow-function")
        if name == "argument-wrong-type.js":
            features.update(("BigInt", "Symbol"))
        elif name == "branding.js":
            features.add("Symbol")
        if name == "not-a-constructor.js":
            features.add("Reflect.construct")
    elif cohort == "compare":
        if name in _COMPARE_ARROW:
            features.add("arrow-function")
        if name == "argument-wrong-type.js":
            features.update(("BigInt", "Symbol"))
        if name == "not-a-constructor.js":
            features.add("Reflect.construct")
    elif cohort == "toString":
        if name == "branding.js":
            features.add("Symbol")
        elif name == "options-wrong-type.js":
            features.update(("BigInt", "Symbol"))
        if name == "not-a-constructor.js":
            features.add("Reflect.construct")
    return frozenset(features)


def _includes(path, cohort):
    name = Path(path).name
    includes = set()
    if cohort == "core":
        if path.count("/") == 3 and name in _CORE_TEMPORAL_HELPERS:
            includes.add("temporalHelpers.js")
        if name in {"infinity-throws-rangeerror.js", "negative-infinity-throws-rangeerror.js"}:
            includes.update(("compareArray.js", "temporalHelpers.js"))
        if path in {
            "built-ins/Temporal/PlainTime/length.js",
            "built-ins/Temporal/PlainTime/name.js",
            "built-ins/Temporal/PlainTime/prop-desc.js",
            "built-ins/Temporal/PlainTime/prototype/constructor.js",
            "built-ins/Temporal/PlainTime/prototype/prop-desc.js",
            "built-ins/Temporal/PlainTime/prototype/toStringTag/prop-desc.js",
        }:
            includes.add("propertyHelper.js")
    elif cohort == "from":
        if name in _FROM_TEMPORAL_HELPERS:
            includes.add("temporalHelpers.js")
        if name in _FROM_COMPARE_ARRAY:
            includes.update(("compareArray.js", "temporalHelpers.js"))
        if name in {"length.js", "name.js", "prop-desc.js"}:
            includes.add("propertyHelper.js")
        if name == "not-a-constructor.js":
            includes.add("isConstructor.js")
    elif cohort == "valueOf":
        if name in {"length.js", "name.js", "prop-desc.js"}:
            includes.add("propertyHelper.js")
        if name == "not-a-constructor.js":
            includes.add("isConstructor.js")
    elif cohort == "equals":
        if name == "argument-string-time-designator-required-for-disambiguation.js":
            includes.add("temporalHelpers.js")
        if name in {"length.js", "name.js", "prop-desc.js"}:
            includes.add("propertyHelper.js")
        if name == "not-a-constructor.js":
            includes.add("isConstructor.js")
    elif cohort == "compare":
        if name == "argument-string-time-designator-required-for-disambiguation.js":
            includes.add("temporalHelpers.js")
        if name in {"length.js", "name.js", "prop-desc.js"}:
            includes.add("propertyHelper.js")
        if name == "not-a-constructor.js":
            includes.add("isConstructor.js")
    elif cohort == "toString":
        if name in _TO_STRING_COMPARE_ARRAY:
            includes.update(("compareArray.js", "temporalHelpers.js"))
        elif name == "smallestunit-plurals-accepted.js":
            includes.add("temporalHelpers.js")
        if name in {"length.js", "name.js", "prop-desc.js"}:
            includes.add("propertyHelper.js")
        if name == "not-a-constructor.js":
            includes.add("isConstructor.js")
    return frozenset(includes)


def _metadata(files, cohort):
    return (
        {path: _features(path, cohort) for path in files},
        {path: _includes(path, cohort) for path in files},
        {path: frozenset() for path in files},
        {path: None for path in files},
    )


(
    TEMPORAL_PLAIN_TIME_CORE_FEATURES,
    TEMPORAL_PLAIN_TIME_CORE_INCLUDES,
    TEMPORAL_PLAIN_TIME_CORE_FLAGS,
    TEMPORAL_PLAIN_TIME_CORE_NEGATIVE,
) = _metadata(TEMPORAL_PLAIN_TIME_CORE_FILES, "core")
(
    TEMPORAL_PLAIN_TIME_FROM_FEATURES,
    TEMPORAL_PLAIN_TIME_FROM_INCLUDES,
    TEMPORAL_PLAIN_TIME_FROM_FLAGS,
    TEMPORAL_PLAIN_TIME_FROM_NEGATIVE,
) = _metadata(TEMPORAL_PLAIN_TIME_FROM_FILES, "from")
(
    TEMPORAL_PLAIN_TIME_VALUE_OF_FEATURES,
    TEMPORAL_PLAIN_TIME_VALUE_OF_INCLUDES,
    TEMPORAL_PLAIN_TIME_VALUE_OF_FLAGS,
    TEMPORAL_PLAIN_TIME_VALUE_OF_NEGATIVE,
) = _metadata(TEMPORAL_PLAIN_TIME_VALUE_OF_FILES, "valueOf")
(
    TEMPORAL_PLAIN_TIME_EQUALS_FEATURES,
    TEMPORAL_PLAIN_TIME_EQUALS_INCLUDES,
    TEMPORAL_PLAIN_TIME_EQUALS_FLAGS,
    TEMPORAL_PLAIN_TIME_EQUALS_NEGATIVE,
) = _metadata(TEMPORAL_PLAIN_TIME_EQUALS_FILES, "equals")
(
    TEMPORAL_PLAIN_TIME_COMPARE_FEATURES,
    TEMPORAL_PLAIN_TIME_COMPARE_INCLUDES,
    TEMPORAL_PLAIN_TIME_COMPARE_FLAGS,
    TEMPORAL_PLAIN_TIME_COMPARE_NEGATIVE,
) = _metadata(TEMPORAL_PLAIN_TIME_COMPARE_FILES, "compare")
(
    TEMPORAL_PLAIN_TIME_TO_STRING_FEATURES,
    TEMPORAL_PLAIN_TIME_TO_STRING_INCLUDES,
    TEMPORAL_PLAIN_TIME_TO_STRING_FLAGS,
    TEMPORAL_PLAIN_TIME_TO_STRING_NEGATIVE,
) = _metadata(TEMPORAL_PLAIN_TIME_TO_STRING_FILES, "toString")

for files, count, label in (
    (TEMPORAL_PLAIN_TIME_CORE_FILES, 40, "core"),
    (TEMPORAL_PLAIN_TIME_FROM_FILES, 51, "from"),
    (TEMPORAL_PLAIN_TIME_VALUE_OF_FILES, 7, "valueOf"),
    (TEMPORAL_PLAIN_TIME_EQUALS_FILES, 31, "equals"),
    (TEMPORAL_PLAIN_TIME_COMPARE_FILES, 32, "compare"),
    (TEMPORAL_PLAIN_TIME_TO_STRING_FILES, 40, "toString"),
):
    if len(files) != count:
        raise RuntimeError(f"Temporal.PlainTime {label} admission must contain {count} files")
