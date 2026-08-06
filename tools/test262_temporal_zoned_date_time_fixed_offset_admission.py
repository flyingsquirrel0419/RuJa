"""Exact Test262 coverage for the fixed-offset ZonedDateTime surface."""

from pathlib import Path

_MANIFEST = Path(__file__).with_name(
    "test262_temporal_zoned_date_time_fixed_offset_admission.txt"
)
TEMPORAL_ZONED_DATE_TIME_FIXED_OFFSET_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_BIGINT_FILES = frozenset(
    {
        "built-ins/Temporal/ZonedDateTime/from/argument-wrong-type.js",
        "built-ins/Temporal/ZonedDateTime/from/argument-propertybag-calendar-wrong-type.js",
        "built-ins/Temporal/ZonedDateTime/from/argument-propertybag-timezone-wrong-type.js",
        "built-ins/Temporal/ZonedDateTime/from/options-wrong-type.js",
        "built-ins/Temporal/ZonedDateTime/prototype/offset/basic.js",
        "built-ins/Temporal/ZonedDateTime/prototype/offsetNanoseconds/basic.js",
        "built-ins/Temporal/ZonedDateTime/prototype/toJSON/basic.js",
        "built-ins/Temporal/ZonedDateTime/prototype/toJSON/offset.js",
        "built-ins/Temporal/ZonedDateTime/prototype/toString/fractionalseconddigits-auto.js",
        "built-ins/Temporal/ZonedDateTime/prototype/toString/fractionalseconddigits-negative.js",
        "built-ins/Temporal/ZonedDateTime/prototype/toString/fractionalseconddigits-number.js",
        "built-ins/Temporal/ZonedDateTime/prototype/toString/offset.js",
        "built-ins/Temporal/ZonedDateTime/prototype/toString/options-wrong-type.js",
    }
)

_ARROW_FUNCTION_FILES = frozenset(
    {
        "built-ins/Temporal/ZonedDateTime/from/argument-propertybag-calendar-year-zero.js",
        "built-ins/Temporal/ZonedDateTime/from/argument-propertybag-timezone-string-year-zero.js",
        "built-ins/Temporal/ZonedDateTime/from/year-zero.js",
    }
)

_ERA_MONTHCODE_FILES = frozenset(
    {
        "built-ins/Temporal/ZonedDateTime/from/roundtrip-from-property-bag.js",
        "built-ins/Temporal/ZonedDateTime/from/roundtrip-from-string.js",
    }
)


def _features(path):
    features = {"Temporal"}
    name = Path(path).name
    if path in _BIGINT_FILES:
        features.add("BigInt")
    if (
        "/branding.js" in path
        or path.endswith("/from/argument-propertybag-calendar-wrong-type.js")
        or path.endswith("/from/argument-propertybag-timezone-wrong-type.js")
        or path.endswith("/from/argument-wrong-type.js")
        or path.endswith("/from/options-wrong-type.js")
        or path.endswith("/toString/options-wrong-type.js")
    ):
        features.add("Symbol")
    if name == "not-a-constructor.js":
        features.add("Reflect.construct")
    if path in _ARROW_FUNCTION_FILES:
        features.add("arrow-function")
    if path in _ERA_MONTHCODE_FILES:
        features.add("Intl.Era-monthcode")
    return frozenset(features)


TEMPORAL_ZONED_DATE_TIME_FIXED_OFFSET_FEATURES = {
    path: _features(path) for path in TEMPORAL_ZONED_DATE_TIME_FIXED_OFFSET_FILES
}

if len(TEMPORAL_ZONED_DATE_TIME_FIXED_OFFSET_FILES) != 259:
    raise RuntimeError(
        "Temporal.ZonedDateTime fixed-offset admission must contain 259 files"
    )
