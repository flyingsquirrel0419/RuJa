//! Regression tests for the execution-fuel mechanism:
//!  - a tight loop is bounded by the fuel budget
//!  - fuel exhaustion is NOT catchable by a JS try/catch (a host-level abort)

use ruja::{Value, Vm};

#[test]
fn fuel_bounds_infinite_loop() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(10_000));
    let err = vm.run("var i=0; while(true){i++;}").unwrap_err();
    assert!(
        err.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        err
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn fuel_unbounded_by_default() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    assert_eq!(vm.fuel_remaining(), None);
    // a bounded loop completes fine without a fuel limit
    let v = vm
        .run("var s=0; for(let i=0;i<1000;i++){s+=i;} s;")
        .unwrap();
    assert_eq!(v, ruja::Value::Number(499500.0));
}

#[test]
fn temporal_instant_string_parsing_precharges_input_bytes() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.temporalShort = "1970-01-01T00:00Z[foo=a]";
        globalThis.temporalLong = "1970-01-01T00:00Z[foo=" + "a".repeat(512) + "]";
        "#,
    )
    .expect("Temporal fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.Instant.from(temporalShort);")
        .expect("short Temporal input should parse");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.Instant.from(temporalLong);")
        .expect("long Temporal input should parse");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("Temporal.Instant.from(temporalLong);")
        .expect_err("N-1 fuel must abort before parsing");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("Temporal.Instant.from(temporalLong);")
        .expect("exact measured fuel should parse successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_plain_date_time_from_precharges_input_bytes() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainFromShort = "1970-01-01T00:00[foo=a]";
        globalThis.plainFromLong = "1970-01-01T00:00[foo=" + "a".repeat(512) + "]";
        "#,
    )
    .expect("PlainDateTime.from fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDateTime.from(plainFromShort);")
        .expect("short PlainDateTime input should parse");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDateTime.from(plainFromLong);")
        .expect("long PlainDateTime input should parse");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("Temporal.PlainDateTime.from(plainFromLong);")
        .expect_err("N-1 fuel must abort before parsing");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("Temporal.PlainDateTime.from(plainFromLong);")
        .expect("exact measured fuel should parse successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_plain_date_time_equals_precharges_input_bytes() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainEquals = new Temporal.PlainDateTime(1970, 1, 1);
        globalThis.plainEqualsShort = "1970-01-01T00:00[foo=a]";
        globalThis.plainEqualsLong = "1970-01-01T00:00[foo=" + "a".repeat(512) + "]";
        "#,
    )
    .expect("PlainDateTime.equals fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("plainEquals.equals(plainEqualsShort);")
        .expect("short PlainDateTime input should parse");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("plainEquals.equals(plainEqualsLong);")
        .expect("long PlainDateTime input should parse");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("plainEquals.equals(plainEqualsLong);")
        .expect_err("N-1 fuel must abort before parsing");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("plainEquals.equals(plainEqualsLong);")
        .expect("exact measured fuel should compare successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_plain_date_equals_precharges_input_bytes() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainDateEquals = new Temporal.PlainDate(1970, 1, 1);
        globalThis.plainDateEqualsShort = "1970-01-01[foo=a]";
        globalThis.plainDateEqualsLong = "1970-01-01[foo=" + "a".repeat(512) + "]";
        "#,
    )
    .expect("PlainDate.equals fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("plainDateEquals.equals(plainDateEqualsShort);")
        .expect("short PlainDate input should parse");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("plainDateEquals.equals(plainDateEqualsLong);")
        .expect("long PlainDate input should parse");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("plainDateEquals.equals(plainDateEqualsLong);")
        .expect_err("N-1 fuel must abort before parsing");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("plainDateEquals.equals(plainDateEqualsLong);")
        .expect("exact measured fuel should parse successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_plain_date_to_plain_date_time_precharges_input_bytes() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainDateToDateTime = new Temporal.PlainDate(1970, 1, 1);
        globalThis.plainTimeShort = "12:34[foo=a]";
        globalThis.plainTimeLong = "12:34[foo=" + "a".repeat(512) + "]";
        "#,
    )
    .expect("PlainDate.toPlainDateTime fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("plainDateToDateTime.toPlainDateTime(plainTimeShort);")
        .expect("short PlainTime input should parse");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("plainDateToDateTime.toPlainDateTime(plainTimeLong);")
        .expect("long PlainTime input should parse");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("plainDateToDateTime.toPlainDateTime(plainTimeLong);")
        .expect_err("N-1 fuel must abort before parsing");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("plainDateToDateTime.toPlainDateTime(plainTimeLong);")
        .expect("exact measured fuel should parse successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_plain_time_from_precharges_input_bytes() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainTimeFromShort = "12:34[foo=a]";
        globalThis.plainTimeFromLong = "12:34[foo=" + "a".repeat(512) + "]";
        "#,
    )
    .expect("PlainTime.from fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainTime.from(plainTimeFromShort);")
        .expect("short PlainTime input should parse");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainTime.from(plainTimeFromLong);")
        .expect("long PlainTime input should parse");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("Temporal.PlainTime.from(plainTimeFromLong);")
        .expect_err("N-1 fuel must abort before parsing");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("Temporal.PlainTime.from(plainTimeFromLong);")
        .expect("exact measured fuel should parse successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_plain_time_compare_precharges_both_input_strings() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainTimeCompareShort = "12:34[foo=a]";
        globalThis.plainTimeCompareLong = "12:34[foo=" + "a".repeat(512) + "]";
        globalThis.plainTimeCompareBranded = new Temporal.PlainTime(12, 34);
        "#,
    )
    .expect("PlainTime.compare fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainTime.compare(plainTimeCompareShort, plainTimeCompareShort);")
        .expect("short PlainTime inputs should parse");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainTime.compare(plainTimeCompareShort, plainTimeCompareLong);")
        .expect("long second PlainTime input should parse");
    let second_long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainTime.compare(plainTimeCompareLong, plainTimeCompareShort);")
        .expect("long first PlainTime input should parse");
    let first_long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainTime.compare(plainTimeCompareBranded, plainTimeCompareShort);")
        .expect("branded first PlainTime input should use hidden slots");
    let branded_short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    assert!(second_long_work >= short_work + 500);
    assert!(first_long_work >= short_work + 500);
    assert!(short_work > branded_short_work);

    for (expression, exact_work) in [
        (
            "Temporal.PlainTime.compare(plainTimeCompareShort, plainTimeCompareLong);",
            second_long_work,
        ),
        (
            "Temporal.PlainTime.compare(plainTimeCompareLong, plainTimeCompareShort);",
            first_long_work,
        ),
    ] {
        vm.set_fuel(Some(exact_work - 1));
        let error = vm
            .run(expression)
            .expect_err("N-1 fuel must abort during input conversion");
        assert_eq!(error.kind, ruja::ErrorKind::Fuel);
        assert_eq!(vm.fuel_remaining(), Some(0));

        vm.set_fuel(Some(exact_work));
        vm.run(expression)
            .expect("exact measured fuel should parse both inputs");
        assert_eq!(vm.fuel_remaining(), Some(0));
    }
}

#[test]
fn temporal_plain_time_to_string_precharges_option_strings() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainTimeToStringFuel = new Temporal.PlainTime(12, 34, 56, 123, 456, 789);
        globalThis.plainTimeOptionShort = { toString() { return 'trunc'; } };
        globalThis.plainTimeOptionLong = { toString() { return 'x'.repeat(512); } };
        "#,
    )
    .expect("PlainTime.toString fuel fixtures should initialize");

    for property in ["fractionalSecondDigits", "roundingMode", "smallestUnit"] {
        vm.set_fuel(Some(BUDGET));
        let _ = vm.run(&format!(
            "try {{ plainTimeToStringFuel.toString({{ {property}: plainTimeOptionShort }}); }} catch (error) {{}}"
        ));
        let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

        vm.set_fuel(Some(BUDGET));
        let _ = vm.run(&format!(
            "try {{ plainTimeToStringFuel.toString({{ {property}: plainTimeOptionLong }}); }} catch (error) {{}}"
        ));
        let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
        assert!(
            long_work >= short_work + 500,
            "{property} conversion must charge the produced string"
        );
    }
}

#[test]
fn temporal_plain_time_round_precharges_option_strings() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainTimeRoundFuel = new Temporal.PlainTime(12, 34, 56, 123, 456, 789);
        globalThis.plainTimeRoundShortNumber = { valueOf() { return '1'; } };
        globalThis.plainTimeRoundLongNumber = { valueOf() { return '1' + ' '.repeat(512); } };
        globalThis.plainTimeRoundShortString = { toString() { return 'second'; } };
        globalThis.plainTimeRoundLongString = { toString() { return 'x'.repeat(512); } };
        "#,
    )
    .expect("PlainTime.round fuel fixtures should initialize");

    for (property, short, long) in [
        (
            "roundingIncrement",
            "plainTimeRoundShortNumber",
            "plainTimeRoundLongNumber",
        ),
        (
            "roundingMode",
            "plainTimeRoundShortString",
            "plainTimeRoundLongString",
        ),
        (
            "smallestUnit",
            "plainTimeRoundShortString",
            "plainTimeRoundLongString",
        ),
    ] {
        vm.set_fuel(Some(BUDGET));
        let _ = vm.run(&format!(
            "try {{ plainTimeRoundFuel.round({{ smallestUnit: 'second', {property}: {short} }}); }} catch (error) {{}}"
        ));
        let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

        vm.set_fuel(Some(BUDGET));
        let _ = vm.run(&format!(
            "try {{ plainTimeRoundFuel.round({{ smallestUnit: 'second', {property}: {long} }}); }} catch (error) {{}}"
        ));
        let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
        assert!(
            long_work >= short_work + 500,
            "{property} conversion must charge the produced string"
        );
    }
}

#[test]
fn temporal_plain_time_with_precharges_field_and_overflow_strings() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainTimeWithFuel = new Temporal.PlainTime(12, 34, 56);
        globalThis.plainTimeWithShortNumber = { valueOf() { return '1'; } };
        globalThis.plainTimeWithLongNumber = { valueOf() { return '1' + ' '.repeat(512); } };
        globalThis.plainTimeWithShortString = { toString() { return 'constrain'; } };
        globalThis.plainTimeWithLongString = { toString() { return 'x'.repeat(512); } };
        "#,
    )
    .expect("PlainTime.with fuel fixtures should initialize");

    for (expression_short, expression_long, label) in [
        (
            "plainTimeWithFuel.with({ hour: plainTimeWithShortNumber });",
            "plainTimeWithFuel.with({ hour: plainTimeWithLongNumber });",
            "field",
        ),
        (
            "try { plainTimeWithFuel.with({ hour: 1 }, { overflow: plainTimeWithShortString }); } catch (error) {}",
            "try { plainTimeWithFuel.with({ hour: 1 }, { overflow: plainTimeWithLongString }); } catch (error) {}",
            "overflow",
        ),
    ] {
        vm.set_fuel(Some(BUDGET));
        vm.run(expression_short).expect("short conversion should run");
        let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

        vm.set_fuel(Some(BUDGET));
        vm.run(expression_long).expect("long conversion should run");
        let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
        assert!(
            long_work >= short_work + 500,
            "{label} conversion must charge the produced string"
        );
    }
}

#[test]
fn temporal_plain_date_to_string_precharges_calendar_name_bytes() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainDateToString = new Temporal.PlainDate(1970, 1, 1);
        globalThis.plainDateToStringShort = "auto";
        globalThis.plainDateToStringLong = "x".repeat(512);
        "#,
    )
    .expect("PlainDate.toString fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("plainDateToString.toString({ calendarName: plainDateToStringShort });")
        .expect("short calendarName should format");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    let error = vm
        .run("plainDateToString.toString({ calendarName: plainDateToStringLong });")
        .expect_err("long invalid calendarName should reach option validation");
    assert_eq!(error.kind, ruja::ErrorKind::Range);
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("plainDateToString.toString({ calendarName: plainDateToStringLong });")
        .expect_err("N-1 fuel must abort during option conversion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    let error = vm
        .run("plainDateToString.toString({ calendarName: plainDateToStringLong });")
        .expect_err("exact measured fuel should reach invalid-option validation");
    assert_eq!(error.kind, ruja::ErrorKind::Range);
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_plain_date_time_compare_precharges_each_string_argument() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainCompareShort = "1970-01-01T00:00[foo=a]";
        globalThis.plainCompareLong =
          "1970-01-01T00:00:01[foo=" + "a".repeat(512) + "]";
        globalThis.plainCompareBranded = new Temporal.PlainDateTime(1970, 1, 1, 0, 0, 1);
        "#,
    )
    .expect("PlainDateTime compare fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDateTime.compare(plainCompareShort, plainCompareShort);")
        .expect("short PlainDateTime compare should parse both strings");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    let result = vm
        .run("Temporal.PlainDateTime.compare(plainCompareShort, plainCompareLong);")
        .expect("PlainDateTime compare should parse both strings");
    assert_eq!(result, Value::Number(-1.0));
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDateTime.compare(plainCompareLong, plainCompareShort);")
        .expect("PlainDateTime compare should precharge the first string");
    let first_long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDateTime.compare(plainCompareBranded, plainCompareShort);")
        .expect("branded first argument should leave only the second string to precharge");
    let branded_first_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(first_long_work >= branded_first_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("Temporal.PlainDateTime.compare(plainCompareShort, plainCompareLong);")
        .expect_err("N-1 fuel must abort ordered PlainDateTime conversion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("Temporal.PlainDateTime.compare(plainCompareShort, plainCompareLong);")
        .expect("exact measured fuel should compare successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_duration_numeric_string_conversion_precharges_input_bytes() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.temporalDurationShort = "1";
        globalThis.temporalDurationLong = "0".repeat(512) + "1";
        globalThis.temporalDurationObject = {
          valueOf: function () { return temporalDurationLong; }
        };
        "#,
    )
    .expect("Temporal.Duration fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.Duration(temporalDurationShort);")
        .expect("short Duration field should convert");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.Duration(temporalDurationLong);")
        .expect("long Duration field should convert");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.Duration(temporalDurationObject);")
        .expect("object-produced Duration field should convert");
    let object_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(object_work >= long_work);

    vm.set_fuel(Some(object_work - 1));
    let error = vm
        .run("new Temporal.Duration(temporalDurationObject);")
        .expect_err("N-1 fuel must abort object field conversion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(object_work));
    vm.run("new Temporal.Duration(temporalDurationObject);")
        .expect("exact measured fuel should construct Duration");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_duration_from_precharges_source_and_field_strings() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.durationFromShort = "PT1S";
        globalThis.durationFromLong = "PT" + "0".repeat(512) + "1S";
        globalThis.durationFromShortField = { valueOf() { return "1"; } };
        globalThis.durationFromLongField = { valueOf() { return "0".repeat(512) + "1"; } };
        "#,
    )
    .expect("Duration.from fuel fixtures should initialize");

    for (short, long, label) in [
        (
            "Temporal.Duration.from(durationFromShort);",
            "Temporal.Duration.from(durationFromLong);",
            "source",
        ),
        (
            "Temporal.Duration.from({ seconds: durationFromShortField });",
            "Temporal.Duration.from({ seconds: durationFromLongField });",
            "field",
        ),
    ] {
        vm.set_fuel(Some(BUDGET));
        vm.run(short)
            .expect("short Duration.from conversion should run");
        let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

        vm.set_fuel(Some(BUDGET));
        vm.run(long)
            .expect("long Duration.from conversion should run");
        let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
        assert!(
            long_work >= short_work + 500,
            "Duration.from {label} conversion must charge produced bytes"
        );

        vm.set_fuel(Some(long_work - 1));
        let error = vm
            .run(long)
            .expect_err("N-1 fuel must abort Duration.from conversion");
        assert_eq!(error.kind, ruja::ErrorKind::Fuel, "{label}");
        assert_eq!(vm.fuel_remaining(), Some(0), "{label}");

        vm.set_fuel(Some(long_work));
        vm.run(long)
            .expect("exact measured fuel should complete Duration.from conversion");
        assert_eq!(vm.fuel_remaining(), Some(0), "{label}");
    }
}

#[test]
fn temporal_plain_time_arithmetic_precharges_duration_strings_and_fields() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainTimeArithmeticFuel = new Temporal.PlainTime(12, 34, 56);
        globalThis.plainTimeArithmeticShort = "PT1S";
        globalThis.plainTimeArithmeticLong = "PT" + "0".repeat(512) + "1S";
        globalThis.plainTimeArithmeticShortField = { valueOf() { return "1"; } };
        globalThis.plainTimeArithmeticLongField = { valueOf() { return "0".repeat(512) + "1"; } };
        "#,
    )
    .expect("PlainTime arithmetic fuel fixtures should initialize");

    for (short, long, label) in [
        (
            "plainTimeArithmeticFuel.add(plainTimeArithmeticShort);",
            "plainTimeArithmeticFuel.add(plainTimeArithmeticLong);",
            "source",
        ),
        (
            "plainTimeArithmeticFuel.subtract({ seconds: plainTimeArithmeticShortField });",
            "plainTimeArithmeticFuel.subtract({ seconds: plainTimeArithmeticLongField });",
            "field",
        ),
    ] {
        vm.set_fuel(Some(BUDGET));
        vm.run(short)
            .expect("short PlainTime arithmetic conversion should run");
        let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

        vm.set_fuel(Some(BUDGET));
        vm.run(long)
            .expect("long PlainTime arithmetic conversion should run");
        let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
        assert!(
            long_work >= short_work + 500,
            "PlainTime arithmetic {label} conversion must charge produced bytes"
        );

        vm.set_fuel(Some(long_work - 1));
        let error = vm
            .run(long)
            .expect_err("N-1 fuel must abort PlainTime arithmetic conversion");
        assert_eq!(error.kind, ruja::ErrorKind::Fuel, "{label}");
        assert_eq!(vm.fuel_remaining(), Some(0), "{label}");

        vm.set_fuel(Some(long_work));
        vm.run(long)
            .expect("exact fuel must complete PlainTime arithmetic conversion");
        assert_eq!(vm.fuel_remaining(), Some(0), "{label}");
    }
}

#[test]
fn temporal_plain_date_time_precharges_numeric_and_calendar_strings() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainYearShort = "2000";
        globalThis.plainYearLong = "0".repeat(512) + "2000";
        globalThis.plainCalendarShort = "x";
        globalThis.plainCalendarLong = "x".repeat(512);
        "#,
    )
    .expect("PlainDateTime fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.PlainDateTime(plainYearShort, 5, 2);")
        .expect("short PlainDateTime field should convert");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.PlainDateTime(plainYearLong, 5, 2);")
        .expect("long PlainDateTime field should convert");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("new Temporal.PlainDateTime(plainYearLong, 5, 2);")
        .expect_err("N-1 fuel must abort numeric field conversion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.PlainDateTime(2000, 5, 2, 0, 0, 0, 0, 0, 0, plainCalendarShort);")
        .expect_err("short invalid calendar should throw");
    let short_calendar_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.PlainDateTime(2000, 5, 2, 0, 0, 0, 0, 0, 0, plainCalendarLong);")
        .expect_err("long invalid calendar should throw");
    let long_calendar_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_calendar_work >= short_calendar_work + 500);
}

#[test]
fn temporal_plain_date_precharges_numeric_and_calendar_strings() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainDateYearShort = "2000";
        globalThis.plainDateYearLong = "0".repeat(512) + "2000";
        globalThis.plainDateCalendarShort = "x";
        globalThis.plainDateCalendarLong = "x".repeat(512);
        "#,
    )
    .expect("PlainDate fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.PlainDate(plainDateYearShort, 5, 2);")
        .expect("short PlainDate field should convert");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.PlainDate(plainDateYearLong, 5, 2);")
        .expect("long PlainDate field should convert");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("new Temporal.PlainDate(plainDateYearLong, 5, 2);")
        .expect_err("N-1 fuel must abort PlainDate numeric field conversion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.PlainDate(2000, 5, 2, plainDateCalendarShort);")
        .expect_err("short invalid PlainDate calendar should throw");
    let short_calendar_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.PlainDate(2000, 5, 2, plainDateCalendarLong);")
        .expect_err("long invalid PlainDate calendar should throw");
    let long_calendar_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_calendar_work >= short_calendar_work + 500);
}

#[test]
fn temporal_plain_date_from_precharges_input_bytes() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainDateFromShort = "1970-01-01[foo=a]";
        globalThis.plainDateFromLong = "1970-01-01[foo=" + "a".repeat(512) + "]";
        "#,
    )
    .expect("PlainDate.from fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDate.from(plainDateFromShort);")
        .expect("short PlainDate input should parse");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDate.from(plainDateFromLong);")
        .expect("long PlainDate input should parse");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("Temporal.PlainDate.from(plainDateFromLong);")
        .expect_err("N-1 fuel must abort PlainDate string parsing");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("Temporal.PlainDate.from(plainDateFromLong);")
        .expect("exact PlainDate fuel should succeed");
}

#[test]
fn temporal_plain_date_compare_precharges_each_string_argument() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.plainDateCompareShort = "1970-01-01[foo=a]";
        globalThis.plainDateCompareLong = "1970-01-02[foo=" + "a".repeat(512) + "]";
        globalThis.plainDateCompareBranded = new Temporal.PlainDate(1970, 1, 1);
        "#,
    )
    .expect("PlainDate.compare fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDate.compare(plainDateCompareShort, plainDateCompareShort);")
        .expect("short PlainDate compare should parse both strings");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    let result = vm
        .run("Temporal.PlainDate.compare(plainDateCompareShort, plainDateCompareLong);")
        .expect("PlainDate compare should parse both strings");
    assert_eq!(result, Value::Number(-1.0));
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDate.compare(plainDateCompareLong, plainDateCompareShort);")
        .expect("PlainDate compare should precharge the first string");
    let first_long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.PlainDate.compare(plainDateCompareBranded, plainDateCompareShort);")
        .expect("branded first argument should leave only the second string to precharge");
    let branded_first_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(first_long_work >= branded_first_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("Temporal.PlainDate.compare(plainDateCompareShort, plainDateCompareLong);")
        .expect_err("N-1 fuel must abort ordered PlainDate conversion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("Temporal.PlainDate.compare(plainDateCompareShort, plainDateCompareLong);")
        .expect("exact PlainDate fuel should compare successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_instant_compare_precharges_each_string_argument() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.temporalCompareShort = "1970-01-01T00:00Z[foo=a]";
        globalThis.temporalCompareLong =
          "1970-01-01T00:00:01Z[foo=" + "a".repeat(512) + "]";
        globalThis.temporalCompareInstant = new Temporal.Instant(1000000000n);
        "#,
    )
    .expect("Temporal compare fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.Instant.compare(temporalCompareShort, temporalCompareShort);")
        .expect("short Temporal compare should parse both strings");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    let result = vm
        .run("Temporal.Instant.compare(temporalCompareShort, temporalCompareLong);")
        .expect("Temporal compare should parse both strings");
    assert_eq!(result, Value::Number(-1.0));
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(BUDGET));
    let result = vm
        .run("Temporal.Instant.compare(temporalCompareLong, temporalCompareShort);")
        .expect("Temporal compare should precharge the first string");
    assert_eq!(result, Value::Number(1.0));
    let first_long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.Instant.compare(temporalCompareInstant, temporalCompareShort);")
        .expect("branded first argument should leave only the second string to precharge");
    let branded_first_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(first_long_work >= branded_first_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("Temporal.Instant.compare(temporalCompareShort, temporalCompareLong);")
        .expect_err("N-1 fuel must abort while converting the ordered inputs");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("Temporal.Instant.compare(temporalCompareShort, temporalCompareLong);")
        .expect("exact measured fuel should compare successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_zoned_date_time_compare_precharges_each_string_argument() {
    const BUDGET: i64 = 30_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.zonedCompareShort = "1970-01-01T00:00Z[UTC][foo=a]";
        globalThis.zonedCompareLong =
          "1970-01-01T00:00:01Z[UTC][foo=" + "a".repeat(512) + "]";
        globalThis.zonedCompareBranded = new Temporal.ZonedDateTime(1000000000n, "UTC");
        "#,
    )
    .expect("ZonedDateTime compare fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.ZonedDateTime.compare(zonedCompareShort, zonedCompareShort);")
        .expect("short ZonedDateTime compare should parse both strings");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    let result = vm
        .run("Temporal.ZonedDateTime.compare(zonedCompareShort, zonedCompareLong);")
        .expect("ZonedDateTime compare should parse both strings");
    assert_eq!(result, Value::Number(-1.0));
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.ZonedDateTime.compare(zonedCompareLong, zonedCompareShort);")
        .expect("ZonedDateTime compare should precharge the first string");
    let first_long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("Temporal.ZonedDateTime.compare(zonedCompareBranded, zonedCompareShort);")
        .expect("branded first argument should leave only the second string to precharge");
    let branded_first_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(first_long_work >= branded_first_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("Temporal.ZonedDateTime.compare(zonedCompareShort, zonedCompareLong);")
        .expect_err("N-1 fuel must abort ordered ZonedDateTime conversion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("Temporal.ZonedDateTime.compare(zonedCompareShort, zonedCompareLong);")
        .expect("exact measured fuel should compare successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_zoned_date_time_start_of_day_obeys_exact_fuel_boundary() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run("globalThis.zonedStart = new Temporal.ZonedDateTime(1n, '+01:00');")
        .expect("startOfDay fuel fixture should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("zonedStart.startOfDay();")
        .expect("startOfDay should complete under the measurement budget");
    let work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(work - 1));
    let error = vm
        .run("zonedStart.startOfDay();")
        .expect_err("N-1 fuel must abort startOfDay");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(work));
    vm.run("zonedStart.startOfDay();")
        .expect("exact measured fuel should complete startOfDay");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_zoned_date_time_precharges_identifier_bytes() {
    const BUDGET: i64 = 20_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.temporalZoneInvalidShort = "x";
        globalThis.temporalZoneInvalidLong = "x".repeat(512);
        globalThis.temporalCalendarInvalidShort = "x";
        globalThis.temporalCalendarInvalidLong = "x".repeat(512);
        globalThis.temporalWithTimeZone = new Temporal.ZonedDateTime(0n, "UTC");
        globalThis.temporalWithTimeZoneString = "2021-08-19T17:30Z[UTC]";
        globalThis.temporalWithCalendar = new Temporal.ZonedDateTime(0n, "UTC");
        globalThis.temporalWithCalendarString = "2021-08-19T17:30[u-ca=iso8601]";
        "#,
    )
    .expect("Temporal ZonedDateTime fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.ZonedDateTime(0n, 'UTC', 'iso8601');")
        .expect("short ZonedDateTime identifiers should construct");
    let valid_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.ZonedDateTime(0n, temporalZoneInvalidShort);")
        .expect_err("short invalid time zone should throw");
    let short_zone_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.ZonedDateTime(0n, temporalZoneInvalidLong);")
        .expect_err("long invalid time zone should throw");
    let long_zone_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_zone_work >= short_zone_work + 500);

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.ZonedDateTime(0n, 'UTC', temporalCalendarInvalidShort);")
        .expect_err("short invalid calendar should throw");
    let short_calendar_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("new Temporal.ZonedDateTime(0n, 'UTC', temporalCalendarInvalidLong);")
        .expect_err("long invalid calendar should throw");
    let long_calendar_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_calendar_work >= short_calendar_work + 500);

    vm.set_fuel(Some(valid_work - 1));
    let error = vm
        .run("new Temporal.ZonedDateTime(0n, 'UTC', 'iso8601');")
        .expect_err("N-1 fuel must abort identifier processing");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(valid_work));
    vm.run("new Temporal.ZonedDateTime(0n, 'UTC', 'iso8601');")
        .expect("exact measured fuel should construct successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(BUDGET));
    vm.run("temporalWithTimeZone.withTimeZone(temporalWithTimeZoneString);")
        .expect("withTimeZone should parse its string argument");
    let with_time_zone_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(with_time_zone_work - 1));
    let error = vm
        .run("temporalWithTimeZone.withTimeZone(temporalWithTimeZoneString);")
        .expect_err("N-1 fuel must abort withTimeZone conversion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(with_time_zone_work));
    vm.run("temporalWithTimeZone.withTimeZone(temporalWithTimeZoneString);")
        .expect("exact measured fuel should complete withTimeZone");
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(BUDGET));
    vm.run("temporalWithCalendar.withCalendar(temporalWithCalendarString);")
        .expect("withCalendar should parse its string argument");
    let with_calendar_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("temporalWithCalendar.withCalendar(temporalCalendarInvalidShort);")
        .expect_err("short invalid withCalendar input should throw");
    let short_with_calendar_work =
        BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("temporalWithCalendar.withCalendar(temporalCalendarInvalidLong);")
        .expect_err("long invalid withCalendar input should throw");
    let long_with_calendar_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_with_calendar_work >= short_with_calendar_work + 500);

    vm.set_fuel(Some(with_calendar_work - 1));
    let error = vm
        .run("temporalWithCalendar.withCalendar(temporalWithCalendarString);")
        .expect_err("N-1 fuel must abort withCalendar conversion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(with_calendar_work));
    vm.run("temporalWithCalendar.withCalendar(temporalWithCalendarString);")
        .expect("exact measured fuel should complete withCalendar");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn temporal_instant_to_string_precharges_time_zone_bytes() {
    const BUDGET: i64 = 10_000;
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.temporalZoneShort = "2021-08-19T17:30Z[UTC][foo=a]";
        globalThis.temporalZoneLong =
          "2021-08-19T17:30Z[UTC][foo=" + "a".repeat(512) + "]";
        globalThis.temporalInstant = new Temporal.Instant(0n);
        "#,
    )
    .expect("Temporal time-zone fuel fixtures should initialize");

    vm.set_fuel(Some(BUDGET));
    vm.run("temporalInstant.toString({ timeZone: temporalZoneShort });")
        .expect("short Temporal time zone should format");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("temporalInstant.toString({ timeZone: temporalZoneLong });")
        .expect("long Temporal time zone should format");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(long_work >= short_work + 500);

    vm.set_fuel(Some(long_work - 1));
    let error = vm
        .run("temporalInstant.toString({ timeZone: temporalZoneLong });")
        .expect_err("N-1 fuel must abort before parsing");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(long_work));
    vm.run("temporalInstant.toString({ timeZone: temporalZoneLong });")
        .expect("exact measured fuel should format successfully");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn annex_b_escape_native_scans_consume_fuel_before_materialization() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.escapeCalls = 0;
        globalThis.escapeInput = {
          toString() {
            escapeCalls += 1;
            return "é".repeat(512);
          }
        };
        globalThis.unescapeInput = "%41".repeat(512);
        "#,
    )
    .expect("legacy escape fuel fixtures should initialize");

    vm.set_fuel(Some(50));
    let escape_error = vm
        .run("escape(escapeInput);")
        .expect_err("escape must meter its native UTF-16 scan");
    assert_eq!(escape_error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    vm.set_fuel(None);
    assert_eq!(
        vm.run("escapeCalls;")
            .expect("ToString side effect should remain observable"),
        Value::Number(1.0)
    );

    vm.set_fuel(Some(50));
    let unescape_error = vm
        .run("unescape(unescapeInput);")
        .expect_err("unescape must meter its native UTF-16 scan");
    assert_eq!(unescape_error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(Some(10_000));
    assert_eq!(
        vm.run("unescape(unescapeInput).length;")
            .expect("refilled fuel should allow the same operation"),
        Value::Number(512.0)
    );
}

#[test]
fn uri_decode_native_scan_consumes_fuel_before_materialization() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.uriDecodeCalls = 0;
        globalThis.uriDecodeInput = {
          toString() {
            uriDecodeCalls += 1;
            return "%41".repeat(512);
          }
        };
        "#,
    )
    .expect("URI decode fuel fixture should initialize");

    vm.set_fuel(Some(50));
    let error = vm
        .run("decodeURIComponent(uriDecodeInput);")
        .expect_err("URI decoding must meter its native byte scan");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(None);
    assert_eq!(
        vm.run("var decoded = decodeURIComponent(uriDecodeInput); uriDecodeCalls + '|' + decoded.length;")
            .expect("refilled URI decode should complete"),
        Value::String("2|512".into())
    );
}

#[test]
fn array_sort_native_index_scans_consume_fuel() {
    for method in ["sort", "toSorted"] {
        let mut vm = Vm::new().expect("failed to initialize VM");
        vm.run("globalThis.sparse = { length: 1000 };")
            .expect("sort fuel fixture should initialize");
        vm.set_fuel(Some(50));

        let error = vm
            .run(&format!("Array.prototype.{method}.call(sparse);"))
            .expect_err("the native indexed-property scan should consume fuel");
        assert_eq!(error.kind, ruja::ErrorKind::Fuel, "{method}");
        assert_eq!(vm.fuel_remaining(), Some(0), "{method}");
    }
}

#[test]
fn array_flat_infinite_cycles_consume_fuel_without_native_recursion() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run("globalThis.cycle = []; cycle[0] = cycle;")
        .expect("flat cycle fixture should initialize");
    vm.set_fuel(Some(100));

    let error = vm
        .run("cycle.flat(Infinity);")
        .expect_err("cyclic infinite-depth flattening must exhaust fuel");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));

    vm.set_fuel(None);
    let error = vm
        .run("cycle.flat(Infinity);")
        .expect_err("unmetered cyclic flattening must reach the cycle guard");
    assert_eq!(error.kind, ruja::ErrorKind::Range);
    assert!(error
        .to_string()
        .contains("Maximum cyclic Array flattening depth exceeded"));

    vm.run(
        r#"
        globalThis.proxyCycle = [];
        Object.defineProperty(proxyCycle, "0", {
          get: function() { return new Proxy(proxyCycle, {}); }
        });
        "#,
    )
    .expect("fresh-proxy cycle fixture should initialize");
    let error = vm
        .run("proxyCycle.flat(Infinity);")
        .expect_err("fresh wrappers around one Array must not bypass the cycle guard");
    assert_eq!(error.kind, ruja::ErrorKind::Range);
}

#[test]
fn regexp_symbol_split_native_loops_consume_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.longInput = "a".repeat(500);
        globalThis.noMatch = /z/;
        globalThis.manyCaptures = new RegExp("a" + "()".repeat(200));
        "#,
    )
    .expect("RegExp split fuel fixtures should initialize");

    vm.set_fuel(Some(50));
    let search_error = vm
        .run("noMatch[Symbol.split](longInput);")
        .expect_err("RegExp split search loop should consume fuel");
    assert_eq!(search_error.kind, ruja::ErrorKind::Fuel);

    vm.set_fuel(Some(50));
    let capture_error = vm
        .run("manyCaptures[Symbol.split]('a');")
        .expect_err("RegExp split capture loop should consume fuel");
    assert_eq!(capture_error.kind, ruja::ErrorKind::Fuel);
}

#[test]
fn regexp_symbol_match_global_collection_consumes_fuel_incrementally() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        "globalThis.longInput = 'a'.repeat(10000); globalThis.global = /./gu; RegExp.input = 'before';",
    )
        .expect("RegExp match fuel fixture should initialize");
    vm.set_fuel(Some(100_000));

    let error = vm
        .run("longInput.match(global);")
        .expect_err("global match collection must consume fuel per result");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
    vm.set_fuel(None);
    assert_eq!(
        vm.run(
            "RegExp.input === longInput && RegExp.lastMatch === 'a' && RegExp.leftContext.length > 0 && RegExp.rightContext.length < longInput.length - 1;",
        )
            .expect("completed built-in exec calls must publish legacy state"),
        Value::Bool(true)
    );
}

#[test]
fn regexp_symbol_replace_native_loops_consume_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.replaceInput = "a".repeat(500);
        globalThis.manyMatches = /a/g;
        globalThis.manyReplaceCaptures = new RegExp("a" + "()".repeat(200));
        "#,
    )
    .expect("RegExp replace fuel fixtures should initialize");

    vm.set_fuel(Some(50));
    let search_error = vm
        .run("manyMatches[Symbol.replace](replaceInput, 'x');")
        .expect_err("RegExp replace result loop should consume fuel");
    assert_eq!(search_error.kind, ruja::ErrorKind::Fuel);

    vm.set_fuel(Some(50));
    let capture_error = vm
        .run("manyReplaceCaptures[Symbol.replace]('a', 'x');")
        .expect_err("RegExp replace capture loop should consume fuel");
    assert_eq!(capture_error.kind, ruja::ErrorKind::Fuel);
}

#[test]
fn regexp_match_indices_materialization_consumes_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.manyIndexCaptures = new RegExp("a" + "()".repeat(200), "d");
        "#,
    )
    .expect("RegExp match-indices fuel fixture should initialize");

    vm.set_fuel(Some(50));
    let error = vm
        .run("manyIndexCaptures.exec('a');")
        .expect_err("match-indices pair materialization should consume fuel");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);

    vm.set_fuel(Some(1_000_000));
    assert_eq!(
        vm.run("manyIndexCaptures.exec('a').indices.length;")
            .expect("a sufficient fuel budget should finish match indices"),
        ruja::Value::Number(201.0)
    );
}

#[test]
fn regexp_exec_materialization_has_an_exact_fuel_boundary() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.manyExecCaptures = new RegExp("a" + "()".repeat(200));
        "#,
    )
    .expect("RegExp exec fuel fixture should initialize");

    const BUDGET: i64 = 1_000_000;
    vm.set_fuel(Some(BUDGET));
    vm.run("manyExecCaptures.exec('a');")
        .expect("a large fuel budget should finish RegExp exec materialization");
    let required = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
    assert!(required > 200, "capture materialization must consume fuel");

    vm.set_fuel(Some(required - 1));
    let error = vm
        .run("manyExecCaptures.exec('a');")
        .expect_err("one unit below the measured boundary must fail");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);

    vm.set_fuel(Some(required));
    vm.run("manyExecCaptures.exec('a');")
        .expect("the exact measured fuel boundary should succeed");
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn regexp_replacement_materialization_has_exact_fuel_boundaries() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.replacementFuelInput = "ab".repeat(128);
        globalThis.replacementFuelCallback = function (match, capture) {
          return capture + match;
        };
        "#,
    )
    .expect("RegExp replacement fuel fixtures should initialize");

    const BUDGET: i64 = 10_000_000;
    for expression in [
        r#"/(a)(b)/g[Symbol.replace](replacementFuelInput, "$2$1$`$&$'");"#,
        r#"/(?<left>a)(b)/g[Symbol.replace](replacementFuelInput, "$<left>");"#,
        "/(a)(b)/g[Symbol.replace](replacementFuelInput, replacementFuelCallback);",
    ] {
        vm.set_fuel(Some(BUDGET));
        vm.run(expression)
            .expect("a large fuel budget should finish RegExp replacement");
        let required = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");
        assert!(required > 128, "replacement native work must consume fuel");

        vm.set_fuel(Some(required - 1));
        let error = vm
            .run(expression)
            .expect_err("one unit below the measured boundary must fail");
        assert_eq!(error.kind, ruja::ErrorKind::Fuel);

        vm.set_fuel(Some(required));
        vm.run(expression)
            .expect("the exact measured replacement fuel boundary should succeed");
        assert_eq!(vm.fuel_remaining(), Some(0));
    }
}

#[test]
fn regexp_named_group_hashing_consumes_name_length_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        globalThis.shortNamedExec = /(?<a>a)/d;
        globalThis.longNamedExec = new RegExp(
          "(?<" + "a".repeat(4096) + ">a)",
          "d"
        );
        "#,
    )
    .expect("named-group hashing fixtures should initialize");

    const BUDGET: i64 = 1_000_000;
    vm.set_fuel(Some(BUDGET));
    vm.run("shortNamedExec.exec('a');")
        .expect("short named-group exec should finish");
    let short_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    vm.set_fuel(Some(BUDGET));
    vm.run("longNamedExec.exec('a');")
        .expect("long named-group exec should finish");
    let long_work = BUDGET - vm.fuel_remaining().expect("fuel should remain enabled");

    assert!(
        long_work >= short_work + 8_000,
        "both string and indices groups must charge full name hashing"
    );
}

#[test]
fn fuel_can_be_refilled_between_runs() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(100));
    let _ = vm.run("while(true){}");
    // exhausted; refill and a fresh run succeeds
    vm.set_fuel(Some(1_000_000));
    let v = vm.run("1+1").unwrap();
    assert_eq!(v, ruja::Value::Number(2.0));
}

#[test]
fn fuel_exhaustion_is_uncatchable() {
    // Untrusted code must not be able to swallow a fuel exhaustion in a
    // try/catch and keep looping. The catch must not fire.
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(5_000));
    let src = "var n=0; for(;;){ try { while(true){} } catch(e){ n++; if(n>2){throw 'done';} } }";
    let err = vm.run(src).unwrap_err();
    // The script never reaches its own `throw 'done'`: fuel exhaustion aborts.
    assert!(
        err.to_string().contains("fuel exhausted"),
        "expected uncatchable fuel exhaustion, got: {}",
        err
    );
}

#[test]
fn promise_resolution_then_getter_does_not_suppress_fuel_exhaustion() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(10_000));
    let error = vm
        .run(
            r#"
            globalThis.inner = { get then() { while (true) {} } };
            globalThis.outer = [inner];
            outer.then = Array.prototype.map;
            globalThis.promise = Promise.resolve(outer);
            "#,
        )
        .expect_err("then getter fuel exhaustion must abort the host run");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn promise_reaction_does_not_suppress_resolution_fuel_exhaustion() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(10_000));
    let error = vm
        .run(
            r#"
            globalThis.inner = { get then() { while (true) {} } };
            globalThis.promise = Promise.resolve().then(function () {
              return Promise.resolve(inner);
            });
            "#,
        )
        .expect_err("Promise reaction must propagate resolution fuel exhaustion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn promise_and_async_boundaries_do_not_suppress_fuel_exhaustion() {
    let cases = [
        (
            "Promise executor",
            "new Promise(function () { while (true) {} });",
        ),
        (
            "thenable job",
            "Promise.resolve({ then: function () { while (true) {} } });",
        ),
        (
            "async function await",
            r#"
            globalThis.promise = (async function () {
              await { get then() { while (true) {} } };
            })();
            "#,
        ),
        (
            "Array.fromAsync value",
            r#"
            Array.fromAsync({
              0: { get then() { while (true) {} } },
              length: 1
            });
            "#,
        ),
        (
            "async generator yield",
            r#"
            globalThis.iterator = (async function* () {
              yield { get then() { while (true) {} } };
            })();
            globalThis.promise = iterator.next();
            "#,
        ),
        (
            "async iterator disposal",
            r#"
            globalThis.asyncIteratorPrototype = Object.getPrototypeOf(
              (async function* () {}).constructor.prototype.prototype
            );
            asyncIteratorPrototype[Symbol.asyncDispose].call({
              return: function () { while (true) {} }
            });
            "#,
        ),
    ];

    for (boundary, source) in cases {
        let mut vm = Vm::new().expect("failed to initialize VM");
        vm.set_fuel(Some(20_000));
        let error = match vm.run(source) {
            Err(error) => error,
            Ok(value) => panic!("{boundary} unexpectedly completed with {value:?}"),
        };
        assert_eq!(error.kind, ruja::ErrorKind::Fuel, "{boundary}");
        assert_eq!(vm.fuel_remaining(), Some(0), "{boundary}");
    }
}

#[test]
fn async_generator_recovers_after_a_host_fuel_abort() {
    for body in [
        "while (true) {}",
        "yield { get then() { while (true) {} } };",
    ] {
        let mut vm = Vm::new().expect("failed to initialize VM");
        vm.run(&format!(
            "globalThis.generator = (async function* () {{ {body} }})();"
        ))
        .expect("async generator should be created");

        vm.set_fuel(Some(20_000));
        let error = vm
            .run("generator.next();")
            .expect_err("async generator must propagate fuel exhaustion");
        assert_eq!(error.kind, ruja::ErrorKind::Fuel, "{body}");

        vm.set_fuel(None);
        vm.run(
            r#"
            globalThis.recovered = false;
            generator.next().then(
              function (result) { recovered = result.done; },
              function () { recovered = true; }
            );
            "#,
        )
        .expect("a later async generator request should settle");
        assert_eq!(
            vm.run("recovered")
                .expect("recovery marker should be readable"),
            ruja::Value::Bool(true),
            "{body}"
        );
    }

    let mut vm = Vm::new().expect("queued-request VM should initialize");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(ruja::Value::Undefined)
        },
        0,
    )
    .expect("GC hook should register");
    vm.run(
        r#"
        globalThis.generator = (async function* () {
          await 0;
          while (true) {}
        })();
        "#,
    )
    .expect("queued-request generator should be created");
    vm.set_fuel(Some(20_000));
    let error = vm
        .run(
            r#"
            globalThis.firstRequest = generator.next();
            globalThis.secondRequest = generator.next();
            "#,
        )
        .expect_err("resumed generator must propagate fuel exhaustion");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);

    vm.set_fuel(None);
    vm.run(
        r#"
        globalThis.secondSettled = false;
        secondRequest.then(
          function (result) { secondSettled = result.done; },
          function () { secondSettled = true; }
        );
        globalThis.generator = undefined;
        globalThis.firstRequest = undefined;
        forceGc();
        "#,
    )
    .expect("queued request should settle after recovery");
    assert_eq!(
        vm.run("secondSettled")
            .expect("queued recovery markers should be readable"),
        ruja::Value::Bool(true)
    );
}

#[test]
fn iterator_helper_close_does_not_suppress_fuel_exhaustion() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(20_000));
    let err = vm
        .run(
            r#"
            var source = {
              next: function() { return { value: 1, done: false }; },
              return: function() { while (true) {} }
            };
            Iterator.prototype.map.call(source, function() {
              throw "callback";
            }).next();
            "#,
        )
        .expect_err("close-time fuel exhaustion must abort the host run");
    assert!(
        err.to_string().contains("fuel exhausted"),
        "expected close-time fuel exhaustion, got: {}",
        err
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_helper_native_loops_consume_fuel() {
    for expression in [
        "Iterator.prototype.drop.call(source, Infinity).next()",
        "Iterator.prototype.filter.call(source, Boolean).next()",
        "Iterator.prototype.flatMap.call(source, function() { return [].values(); }).next()",
        "Iterator.prototype.reduce.call(source, function(memo) { return memo; }, 0)",
        "Iterator.prototype.forEach.call(source, function() {})",
        "Iterator.prototype.some.call(source, function() { return false; })",
        "Iterator.prototype.every.call(source, function() { return true; })",
        "Iterator.prototype.find.call(source, function() { return false; })",
    ] {
        let mut vm = Vm::new().expect("failed to initialize VM");
        vm.set_fuel(Some(100));
        let source = format!(
            r#"
            var source = {{
              next: Iterator.prototype[Symbol.iterator],
              done: false,
              value: 0
            }};
            {expression};
            "#
        );
        let error = vm
            .run(&source)
            .expect_err("native Iterator helper loop must exhaust fuel");
        assert!(
            error.to_string().contains("fuel exhausted"),
            "expected helper fuel exhaustion, got: {}",
            error
        );
        assert_eq!(vm.fuel_remaining(), Some(0));
    }
}

#[test]
fn iterator_concat_empty_sources_consume_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var empty = {
          [Symbol.iterator]: function() {
            return { next: function() { return { done: true }; } };
          }
        };
        var sources = [];
        for (var i = 0; i < 200; i++) sources.push(empty);
        var concatenated = Iterator.concat(...sources);
        "#,
    )
    .expect("failed to create concatenated iterator");
    vm.set_fuel(Some(100));
    let error = vm
        .run("concatenated.next()")
        .expect_err("empty concat scan should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_zip_eager_setup_consumes_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(1_000));
    let error = vm
        .run(
            r#"
            var inner = { next: function() { return { done: true }; } };
            Iterator.zip({
              [Symbol.iterator]: function() { return this; },
              next: function() { return { value: inner, done: false }; }
            });
            "#,
        )
        .expect_err("infinite Iterator.zip setup should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_zip_inactive_longest_slots_consume_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var inputs = [];
        for (var i = 0; i < 200; i++) {
          inputs.push({ next: function() { return { done: true }; } });
        }
        var liveStep = 0;
        inputs.push({
          next: function() {
            return liveStep++ === 0
              ? { value: 1, done: false }
              : { done: true };
          }
        });
        var zipped = Iterator.zip(inputs, { mode: "longest" });
        zipped.next();
        "#,
    )
    .expect("failed to create and start wide zip helper");
    vm.set_fuel(Some(100));
    let error = vm
        .run("zipped.next()")
        .expect_err("inactive longest slots should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_zip_return_fuel_abort_does_not_leave_helper_running() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var inputs = [];
        for (var i = 0; i < 200; i++) {
          inputs.push({
            next: function() { return { value: 1, done: false }; },
            return: function() { return {}; }
          });
        }
        var zipped = Iterator.zip(inputs);
        zipped.next();
        "#,
    )
    .expect("failed to create and suspend wide zip helper");
    vm.set_fuel(Some(100));
    let error = vm
        .run("zipped.return()")
        .expect_err("wide zip return should exhaust fuel while extracting inputs");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected fuel exhaustion, got: {}",
        error
    );

    vm.set_fuel(Some(10_000));
    let result = vm
        .run("zipped.next().done")
        .expect("fuel-aborted return must leave the helper completed");
    assert_eq!(result, ruja::Value::Bool(true));
}

#[test]
fn iterator_zip_keyed_eager_key_setup_consumes_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var keyedInputs = {};
        for (var i = 0; i < 200; i++) {
          keyedInputs["key" + i] = {
            next: function() { return { done: true }; }
          };
        }
        "#,
    )
    .expect("failed to create wide keyed input object");
    vm.set_fuel(Some(100));
    let error = vm
        .run("Iterator.zipKeyed(keyedInputs)")
        .expect_err("wide Iterator.zipKeyed setup should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected keyed setup fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_zip_keyed_proxy_own_keys_array_like_consumes_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var ownKeyReads = 0;
        var keyList = new Proxy({}, {
          get: function(_target, key) {
            if (key === "length") return 10000;
            ownKeyReads += 1;
            return "key" + key;
          }
        });
        var proxyInputs = new Proxy({}, {
          ownKeys: function() { return keyList; }
        });
        "#,
    )
    .expect("failed to create Proxy ownKeys array-like input");
    vm.set_fuel(Some(100));
    let error = vm
        .run("Iterator.zipKeyed(proxyInputs)")
        .expect_err("Proxy ownKeys array-like traversal should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected Proxy ownKeys fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    vm.set_fuel(None);
    assert_eq!(
        vm.run("ownKeyReads > 0 && ownKeyReads < 10000")
            .expect("failed to inspect bounded Proxy ownKeys traversal"),
        ruja::Value::Bool(true)
    );
}

#[test]
fn for_in_native_snapshots_candidates_and_prototypes_consume_fuel() {
    let mut candidate_vm = Vm::new().expect("failed to initialize VM");
    candidate_vm
        .run(
            r#"
            var wideForIn = Object.create(null);
            for (var i = 0; i < 200; i += 1) {
              Object.defineProperty(wideForIn, "key" + i, {
                value: i,
                enumerable: false,
                configurable: true
              });
            }
            "#,
        )
        .expect("wide for-in fixture should initialize");
    let source = candidate_vm.get_global("wideForIn");
    let iterator = candidate_vm
        .make_for_in_keys(&source)
        .expect("wide for-in iterator should initialize");
    candidate_vm.set_fuel(Some(199));
    let error = candidate_vm
        .iterator_next(&iterator)
        .expect_err("N-1 fuel must abort before materializing N ordinary own keys");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(candidate_vm.fuel_remaining(), Some(0));

    candidate_vm.set_fuel(None);
    let iterator = candidate_vm
        .make_for_in_keys(&source)
        .expect("second wide for-in iterator should initialize");
    candidate_vm.set_fuel(Some(399));
    let error = candidate_vm
        .iterator_next(&iterator)
        .expect_err("snapshot plus N-1 skipped candidates must exhaust fuel");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(candidate_vm.fuel_remaining(), Some(0));

    candidate_vm.set_fuel(None);
    let iterator = candidate_vm
        .make_for_in_keys(&source)
        .expect("exact-budget wide for-in iterator should initialize");
    candidate_vm.set_fuel(Some(400));
    assert_eq!(
        candidate_vm
            .iterator_next(&iterator)
            .expect("exact snapshot and candidate fuel should complete"),
        (ruja::Value::Undefined, true)
    );
    assert_eq!(candidate_vm.fuel_remaining(), Some(0));

    let mut non_string_vm = Vm::new().expect("failed to initialize VM");
    non_string_vm
        .run(
            r#"
            var symbolOnlyForIn = Object.create(null);
            for (var i = 0; i < 200; i += 1) {
              symbolOnlyForIn[Symbol(i)] = i;
            }
            var typedForIn = new Uint8Array(200);
            var stringForIn = "x".repeat(200);
            "#,
        )
        .expect("non-string and exotic for-in fixtures should initialize");
    for name in ["symbolOnlyForIn", "typedForIn", "stringForIn"] {
        let source = non_string_vm.get_global(name);
        let iterator = non_string_vm
            .make_for_in_keys(&source)
            .expect("metered for-in iterator should initialize");
        non_string_vm.set_fuel(Some(0));
        let error = non_string_vm
            .iterator_next(&iterator)
            .expect_err("zero fuel must abort a non-empty own-key snapshot");
        assert_eq!(error.kind, ruja::ErrorKind::Fuel, "{name}");
        assert_eq!(non_string_vm.fuel_remaining(), Some(0), "{name}");
        non_string_vm.set_fuel(None);
    }

    let mut prototype_vm = Vm::new().expect("failed to initialize VM");
    prototype_vm
        .run(
            r#"
            var deepForIn = Object.create(null);
            for (var i = 0; i < 200; i += 1) {
              deepForIn = Object.create(deepForIn);
            }
            "#,
        )
        .expect("deep for-in prototype fixture should initialize");
    let source = prototype_vm.get_global("deepForIn");
    let iterator = prototype_vm
        .make_for_in_keys(&source)
        .expect("deep for-in iterator should initialize");
    prototype_vm.set_fuel(Some(100));
    let error = prototype_vm
        .iterator_next(&iterator)
        .expect_err("native prototype transitions should consume fuel");
    assert_eq!(error.kind, ruja::ErrorKind::Fuel);
    assert_eq!(prototype_vm.fuel_remaining(), Some(0));
}

#[test]
fn iterator_zip_keyed_padding_collection_consumes_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var keyedInputs = {};
        for (var i = 0; i < 200; i++) {
          keyedInputs["key" + i] = {
            next: function() { return { done: true }; }
          };
        }
        var paddingGets = 0;
        var keyedPadding = new Proxy({}, {
          get: function() {
            paddingGets += 1;
            return undefined;
          }
        });
        "#,
    )
    .expect("failed to create keyed padding inputs");
    vm.set_fuel(Some(700));
    let error = vm
        .run(
            r#"
            Iterator.zipKeyed(keyedInputs, {
              mode: "longest",
              padding: keyedPadding
            });
            "#,
        )
        .expect_err("wide Iterator.zipKeyed padding collection should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected keyed padding fuel exhaustion, got: {}",
        error
    );
    assert_eq!(vm.fuel_remaining(), Some(0));
    vm.set_fuel(None);
    assert_eq!(
        vm.run("paddingGets > 0 && paddingGets < 200")
            .expect("failed to inspect partial keyed padding collection"),
        ruja::Value::Bool(true)
    );
}

#[test]
fn iterator_zip_keyed_wide_step_consumes_fuel_and_completes() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var keyedInputs = {};
        for (var i = 0; i < 200; i++) {
          keyedInputs["key" + i] = {
            next: function() { return { value: 1, done: false }; },
            return: function() { return {}; }
          };
        }
        var keyedZip = Iterator.zipKeyed(keyedInputs);
        "#,
    )
    .expect("failed to create wide keyed zip helper");
    vm.set_fuel(Some(100));
    let error = vm
        .run("keyedZip.next()")
        .expect_err("wide Iterator.zipKeyed step should exhaust fuel");
    assert!(
        error.to_string().contains("fuel exhausted"),
        "expected keyed step fuel exhaustion, got: {}",
        error
    );
    vm.set_fuel(Some(10_000));
    assert_eq!(
        vm.run("keyedZip.next().done")
            .expect("fuel-aborted keyed step must leave the helper completed"),
        ruja::Value::Bool(true)
    );
}

#[test]
fn object_prototype_proxy_chain_walks_consume_fuel() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var cyclicProxy;
        cyclicProxy = new Proxy({}, {
          getOwnPropertyDescriptor: function() { return undefined; },
          getPrototypeOf: function() { return cyclicProxy; }
        });
        "#,
    )
    .expect("failed to create cyclic prototype proxy");

    for expression in [
        "Object.prototype.isPrototypeOf.call({}, cyclicProxy)",
        "Object.prototype.__lookupGetter__.call(cyclicProxy, 'value')",
    ] {
        vm.set_fuel(Some(100));
        let error = vm
            .run(expression)
            .expect_err("cyclic Proxy prototype walk should exhaust fuel");
        assert!(
            error.to_string().contains("fuel exhausted"),
            "expected fuel exhaustion for {expression}, got: {error}"
        );
    }
}

#[test]
fn normal_errors_remain_catchable() {
    // Fuel change must not break ordinary try/catch of catchable errors.
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.set_fuel(Some(1_000_000));
    let v = vm
        .run("var r; try { null.x; } catch(e) { r = 'caught ' + (e instanceof Error); } r;")
        .unwrap();
    assert_eq!(v, ruja::Value::String(std::sync::Arc::from("caught true")));
}

#[test]
fn heap_limit_is_catchable_not_a_panic() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    // Set a limit slightly above the current live count so only script
    // allocations push past the threshold.
    vm.set_max_heap_objects(Some(500));
    let result = vm.run("var a = []; while(true) { a.push({ x: 1, y: 2 }); }");
    assert!(
        result.is_err(),
        "heap limit should produce a catchable error, not a panic"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("heap limit"),
        "expected 'heap limit' in error, got: {}",
        err
    );
}

/// Heap limit must be enforced even when allocations happen inside builtins
/// (JSON.parse, Array.prototype.map/slice, RegExp). Previously, raw
/// `heap.allocate()` calls in builtins bypassed the limit entirely.
#[test]
fn heap_limit_enforced_in_json_parse() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    vm.set_max_heap_objects(Some(500));
    // Each element of the parsed array is a separate heap object.
    let big_json = format!(
        "[{}]",
        (0..10_000)
            .map(|i| format!("{{\"k\":{}}}", i))
            .collect::<Vec<_>>()
            .join(",")
    );
    let result = vm.run(&format!("JSON.parse({:?})", big_json));
    assert!(result.is_err(), "JSON.parse should hit heap limit");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("heap limit"),
        "expected 'heap limit' in error, got: {}",
        err
    );
}

#[test]
fn heap_limit_enforced_in_array_map() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    vm.set_max_heap_objects(Some(500));
    // map() creates a new array + result objects per element.
    let result = vm.run(
        "var a = [1,2,3,4,5,6,7,8,9,10]; \
         var b = a; \
         for (var i = 0; i < 10000; i++) { b = b.concat(a).map(function(x) { return { v: x }; }); }",
    );
    assert!(result.is_err(), "Array.map should hit heap limit");
}

#[test]
fn heap_limit_enforced_in_regexp() {
    let mut vm = ruja::Vm::new().expect("failed to initialize VM");
    vm.set_max_heap_objects(Some(500));
    // Each match creates a result array object on the heap.
    let result = vm.run(
        "var s = 'a'.repeat(10000); \
         var re = /a/g; \
         var matches = []; \
         while (re.exec(s) !== null) { matches.push(re.exec(s)); }",
    );
    assert!(result.is_err(), "RegExp should hit heap limit");
}
