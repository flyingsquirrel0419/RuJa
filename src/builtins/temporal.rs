use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::fmt::Write;
use std::sync::Arc;

const NS_PER_SECOND: i128 = 1_000_000_000;
const SECONDS_PER_DAY: i128 = 86_400;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ParsedDuration {
    pub years: f64,
    pub months: f64,
    pub weeks: f64,
    pub days: f64,
    pub hours: f64,
    pub minutes: f64,
    pub seconds: f64,
    pub milliseconds: f64,
    pub microseconds: f64,
    pub nanoseconds: f64,
}

fn duration_number(bytes: &[u8], index: &mut usize) -> Option<(f64, Option<(i128, usize)>)> {
    let integer_start = *index;
    while bytes.get(*index).is_some_and(u8::is_ascii_digit) {
        *index += 1;
    }
    if *index == integer_start {
        return None;
    }
    let integer = std::str::from_utf8(&bytes[integer_start..*index])
        .ok()?
        .parse::<f64>()
        .ok()?;
    if !matches!(bytes.get(*index), Some(b'.') | Some(b',')) {
        return Some((integer, None));
    }
    *index += 1;
    let fraction_start = *index;
    while bytes.get(*index).is_some_and(u8::is_ascii_digit) {
        *index += 1;
    }
    let digits = *index - fraction_start;
    if !(1..=9).contains(&digits) {
        return None;
    }
    let numerator = bytes[fraction_start..*index]
        .iter()
        .fold(0_i128, |value, byte| value * 10 + i128::from(byte - b'0'));
    Some((integer, Some((numerator, digits))))
}

fn duration_fraction_nanoseconds(numerator: i128, digits: usize, unit: i128) -> Option<i128> {
    numerator
        .checked_mul(unit)?
        .checked_div(10_i128.pow(u32::try_from(digits).ok()?))
}

fn split_duration_nanoseconds(mut nanoseconds: i128) -> Option<(f64, f64, f64, f64, f64, f64)> {
    let hours = nanoseconds / (3_600 * NS_PER_SECOND);
    nanoseconds %= 3_600 * NS_PER_SECOND;
    let minutes = nanoseconds / (60 * NS_PER_SECOND);
    nanoseconds %= 60 * NS_PER_SECOND;
    let seconds = nanoseconds / NS_PER_SECOND;
    nanoseconds %= NS_PER_SECOND;
    let milliseconds = nanoseconds / 1_000_000;
    nanoseconds %= 1_000_000;
    let microseconds = nanoseconds / 1_000;
    let nanoseconds = nanoseconds % 1_000;
    Some((
        hours.to_f64()?,
        minutes.to_f64()?,
        seconds.to_f64()?,
        milliseconds.to_f64()?,
        microseconds.to_f64()?,
        nanoseconds.to_f64()?,
    ))
}

pub(crate) fn parse_duration_string(source: &str) -> Option<ParsedDuration> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let sign = match bytes.first() {
        Some(b'-') => {
            index += 1;
            -1.0
        }
        Some(b'+') => {
            index += 1;
            1.0
        }
        _ => 1.0,
    };
    if !bytes
        .get(index)
        .is_some_and(|byte| byte.eq_ignore_ascii_case(&b'p'))
    {
        return None;
    }
    index += 1;

    let mut fields = [0.0; 10];
    let mut any = false;
    let mut last_date_rank = None;
    while index < bytes.len()
        && !bytes
            .get(index)
            .is_some_and(|byte| byte.eq_ignore_ascii_case(&b't'))
    {
        let (value, fraction) = duration_number(bytes, &mut index)?;
        if fraction.is_some() {
            return None;
        }
        let unit = bytes.get(index)?.to_ascii_uppercase();
        index += 1;
        let (rank, slot) = match unit {
            b'Y' => (0, 0),
            b'M' => (1, 1),
            b'W' => (2, 2),
            b'D' => (3, 3),
            _ => return None,
        };
        if last_date_rank.is_some_and(|previous| rank <= previous) {
            return None;
        }
        last_date_rank = Some(rank);
        fields[slot] = value;
        any = true;
    }

    if bytes
        .get(index)
        .is_some_and(|byte| byte.eq_ignore_ascii_case(&b't'))
    {
        index += 1;
        let time_start = index;
        let mut last_time_rank = None;
        while index < bytes.len() {
            let (value, fraction) = duration_number(bytes, &mut index)?;
            let unit = bytes.get(index)?.to_ascii_uppercase();
            index += 1;
            let (rank, slot, unit_nanoseconds) = match unit {
                b'H' => (0, 4, 3_600 * NS_PER_SECOND),
                b'M' => (1, 5, 60 * NS_PER_SECOND),
                b'S' => (2, 6, NS_PER_SECOND),
                _ => return None,
            };
            if last_time_rank.is_some_and(|previous| rank <= previous) {
                return None;
            }
            last_time_rank = Some(rank);
            fields[slot] = value;
            any = true;

            if let Some((numerator, digits)) = fraction {
                if index != bytes.len() {
                    return None;
                }
                let fractional =
                    duration_fraction_nanoseconds(numerator, digits, unit_nanoseconds)?;
                let (hours, minutes, seconds, milliseconds, microseconds, nanoseconds) =
                    split_duration_nanoseconds(fractional)?;
                fields[4] += hours;
                fields[5] += minutes;
                fields[6] += seconds;
                fields[7] += milliseconds;
                fields[8] += microseconds;
                fields[9] += nanoseconds;
            }
        }
        if index == time_start {
            return None;
        }
    }
    if !any || index != bytes.len() {
        return None;
    }

    for field in &mut fields {
        *field *= sign;
        if *field == 0.0 {
            *field = 0.0;
        }
    }
    Some(ParsedDuration {
        years: fields[0],
        months: fields[1],
        weeks: fields[2],
        days: fields[3],
        hours: fields[4],
        minutes: fields[5],
        seconds: fields[6],
        milliseconds: fields[7],
        microseconds: fields[8],
        nanoseconds: fields[9],
    })
}

fn digits(bytes: &[u8], index: &mut usize, count: usize) -> Option<i128> {
    let end = index.checked_add(count)?;
    let slice = bytes.get(*index..end)?;
    if !slice.iter().all(u8::is_ascii_digit) {
        return None;
    }
    *index = end;
    Some(
        slice
            .iter()
            .fold(0_i128, |value, byte| value * 10 + i128::from(byte - b'0')),
    )
}

fn take(bytes: &[u8], index: &mut usize, expected: u8) -> Option<()> {
    if bytes.get(*index) != Some(&expected) {
        return None;
    }
    *index += 1;
    Some(())
}

fn fraction_nanoseconds(bytes: &[u8], index: &mut usize) -> Option<i128> {
    if !matches!(bytes.get(*index), Some(b'.') | Some(b',')) {
        return Some(0);
    }
    *index += 1;
    let start = *index;
    while bytes.get(*index).is_some_and(u8::is_ascii_digit) {
        *index += 1;
    }
    let count = *index - start;
    if !(1..=9).contains(&count) {
        return None;
    }
    Some(
        bytes[start..*index]
            .iter()
            .fold(0_i128, |value, byte| value * 10 + i128::from(byte - b'0'))
            * 10_i128.pow((9 - count) as u32),
    )
}

fn parse_offset_nanoseconds(bytes: &[u8], index: &mut usize) -> Option<i128> {
    let negative = bytes.get(*index) == Some(&b'-');
    if !negative && bytes.get(*index) != Some(&b'+') {
        return None;
    }
    *index += 1;

    let hour = digits(bytes, index, 2)?;
    let extended = bytes.get(*index) == Some(&b':');
    let mut minute = 0;
    let mut second = 0;
    let mut fraction = 0;

    let minute_present = if extended {
        *index += 1;
        minute = digits(bytes, index, 2)?;
        true
    } else if bytes.get(*index).is_some_and(u8::is_ascii_digit) {
        minute = digits(bytes, index, 2)?;
        true
    } else {
        false
    };

    let second_present = if extended && bytes.get(*index) == Some(&b':') {
        *index += 1;
        second = digits(bytes, index, 2)?;
        true
    } else if !extended && minute_present && bytes.get(*index).is_some_and(u8::is_ascii_digit) {
        second = digits(bytes, index, 2)?;
        true
    } else {
        false
    };
    if second_present {
        fraction = fraction_nanoseconds(bytes, index)?;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let value = (hour * 3_600 + minute * 60 + second)
        .checked_mul(NS_PER_SECOND)?
        .checked_add(fraction)?;
    Some(if negative { -value } else { value })
}

fn offset_has_sub_minute_syntax(bytes: &[u8], start: usize, end: usize) -> bool {
    let extended = bytes.get(start + 3) == Some(&b':');
    end > start + if extended { 6 } else { 5 }
}

fn valid_annotation_key(key: &[u8]) -> bool {
    key.first()
        .is_some_and(|byte| byte.is_ascii_lowercase() || *byte == b'_')
        && key.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn valid_annotation_value(value: &[u8]) -> bool {
    value
        .split(|byte| *byte == b'-')
        .all(|component| !component.is_empty() && component.iter().all(u8::is_ascii_alphanumeric))
}

fn valid_numeric_time_zone_annotation(value: &[u8]) -> bool {
    if !matches!(value.first(), Some(b'+') | Some(b'-')) {
        return false;
    }
    let mut index = 1;
    let Some(hour) = digits(value, &mut index, 2) else {
        return false;
    };
    let minute = if index == value.len() {
        0
    } else if value.get(index) == Some(&b':') {
        index += 1;
        let Some(minute) = digits(value, &mut index, 2) else {
            return false;
        };
        minute
    } else {
        let Some(minute) = digits(value, &mut index, 2) else {
            return false;
        };
        minute
    };
    index == value.len() && hour <= 23 && minute <= 59
}

fn valid_named_time_zone_annotation(value: &[u8]) -> bool {
    !value.is_empty()
        && value.split(|byte| *byte == b'/').all(|component| {
            component
                .first()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'.' | b'_'))
                && component[1..].iter().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-')
                })
        })
}

fn parse_annotations<'a>(bytes: &'a [u8], index: &mut usize) -> Option<Option<&'a [u8]>> {
    let mut time_zone_seen = false;
    let mut time_zone_annotation = None;
    let mut annotation_seen = false;
    let mut calendar_count = 0_u32;
    let mut critical_calendar_seen = false;

    while bytes.get(*index) == Some(&b'[') {
        *index += 1;
        let start = *index;
        while bytes.get(*index).is_some_and(|byte| *byte != b']') {
            *index += 1;
        }
        let end = *index;
        take(bytes, index, b']')?;

        let content = bytes.get(start..end)?;
        let (critical, body) = if content.first() == Some(&b'!') {
            (true, content.get(1..)?)
        } else {
            (false, content)
        };
        if body.is_empty() {
            return None;
        }

        if let Some(separator) = body.iter().position(|byte| *byte == b'=') {
            let key = &body[..separator];
            let value = &body[separator + 1..];
            if !valid_annotation_key(key) || !valid_annotation_value(value) {
                return None;
            }
            if key == b"u-ca" {
                calendar_count += 1;
                critical_calendar_seen |= critical;
                if calendar_count > 1 && critical_calendar_seen {
                    return None;
                }
            } else if critical {
                return None;
            }
            annotation_seen = true;
        } else {
            if time_zone_seen || annotation_seen {
                return None;
            }
            let valid = if matches!(body.first(), Some(b'+') | Some(b'-')) {
                valid_numeric_time_zone_annotation(body)
            } else {
                valid_named_time_zone_annotation(body)
            };
            if !valid {
                return None;
            }
            time_zone_seen = true;
            time_zone_annotation = Some(body);
        }
    }
    Some(time_zone_annotation)
}

pub(crate) fn leap_year(year: i128) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub(crate) fn days_in_month(year: i128, month: i128) -> Option<i128> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => return None,
    })
}

pub(crate) fn days_from_civil(year: i128, month: i128, day: i128) -> Option<i128> {
    let year = year.checked_sub(i128::from(month <= 2))?;
    let era = if year >= 0 {
        year
    } else {
        year.checked_sub(399)?
    } / 400;
    let year_of_era = year.checked_sub(era.checked_mul(400)?)?;
    let shifted_month = month.checked_add(if month > 2 { -3 } else { 9 })?;
    let day_of_year = 153_i128
        .checked_mul(shifted_month)?
        .checked_add(2)?
        .checked_div(5)?
        .checked_add(day)?
        .checked_sub(1)?;
    let day_of_era = year_of_era
        .checked_mul(365)?
        .checked_add(year_of_era / 4)?
        .checked_sub(year_of_era / 100)?
        .checked_add(day_of_year)?;
    era.checked_mul(146_097)?
        .checked_add(day_of_era)?
        .checked_sub(719_468)
}

struct ParsedDateTime<'a> {
    local_nanoseconds: i128,
    offset_nanoseconds: Option<i128>,
    offset_has_sub_minute_syntax: bool,
    z: bool,
    time_zone_annotation: Option<&'a [u8]>,
}

fn parse_date_time(source: &str, allow_date_only: bool) -> Option<ParsedDateTime<'_>> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let year = if matches!(bytes.first(), Some(b'+') | Some(b'-')) {
        let negative = bytes[0] == b'-';
        index += 1;
        let magnitude = digits(bytes, &mut index, 6)?;
        if negative && magnitude == 0 {
            return None;
        }
        if negative {
            -magnitude
        } else {
            magnitude
        }
    } else {
        digits(bytes, &mut index, 4)?
    };
    let extended_date = bytes.get(index) == Some(&b'-');
    if extended_date {
        index += 1;
    }
    let month = digits(bytes, &mut index, 2)?;
    if extended_date {
        take(bytes, &mut index, b'-')?;
    }
    let day = digits(bytes, &mut index, 2)?;
    if day == 0 || day > days_in_month(year, month)? {
        return None;
    }
    let has_time = matches!(bytes.get(index), Some(b'T') | Some(b't') | Some(b' '));
    if !has_time && !allow_date_only {
        return None;
    }
    if !has_time && !matches!(bytes.get(index), None | Some(b'[')) {
        return None;
    }
    if has_time {
        index += 1;
    }
    let hour = if has_time {
        digits(bytes, &mut index, 2)?
    } else {
        0
    };
    let extended_time = has_time && bytes.get(index) == Some(&b':');
    let mut minute = 0;
    let mut second = 0;
    let mut fraction = 0_i128;

    let minute_present = if extended_time {
        index += 1;
        minute = digits(bytes, &mut index, 2)?;
        true
    } else if has_time && bytes.get(index).is_some_and(u8::is_ascii_digit) {
        minute = digits(bytes, &mut index, 2)?;
        true
    } else {
        false
    };

    let second_present = if extended_time && bytes.get(index) == Some(&b':') {
        index += 1;
        second = digits(bytes, &mut index, 2)?;
        true
    } else if !extended_time && minute_present && bytes.get(index).is_some_and(u8::is_ascii_digit) {
        second = digits(bytes, &mut index, 2)?;
        true
    } else {
        false
    };
    if second_present {
        fraction = fraction_nanoseconds(bytes, &mut index)?;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    second = second.min(59);

    let (offset_nanoseconds, offset_has_sub_minute_syntax, z) = match bytes.get(index) {
        Some(b'Z') | Some(b'z') => {
            index += 1;
            (Some(0), false, true)
        }
        Some(b'+') | Some(b'-') => {
            let start = index;
            let offset = parse_offset_nanoseconds(bytes, &mut index)?;
            (
                Some(offset),
                offset_has_sub_minute_syntax(bytes, start, index),
                false,
            )
        }
        _ => (None, false, false),
    };
    let time_zone_annotation = parse_annotations(bytes, &mut index)?;
    if index != bytes.len() {
        return None;
    }

    let days = days_from_civil(year, month, day)?;
    let local_seconds = days
        .checked_mul(SECONDS_PER_DAY)?
        .checked_add(hour * 3_600 + minute * 60 + second)?;
    let local_nanoseconds = local_seconds
        .checked_mul(NS_PER_SECOND)?
        .checked_add(fraction)?;
    Some(ParsedDateTime {
        local_nanoseconds,
        offset_nanoseconds,
        offset_has_sub_minute_syntax,
        z,
        time_zone_annotation,
    })
}

pub(crate) fn parse_instant_string(source: &str) -> Option<BigInt> {
    let parsed = parse_date_time(source, false)?;
    let offset_nanoseconds = parsed.offset_nanoseconds?;
    Some(BigInt::from(
        parsed.local_nanoseconds.checked_sub(offset_nanoseconds)?,
    ))
}

pub(crate) struct ParsedZonedDateTime {
    local_nanoseconds: i128,
    source_offset_nanoseconds: Option<i128>,
    z: bool,
    pub time_zone_identifier: Arc<str>,
    pub offset_minutes: i16,
    pub calendar_identifier: Arc<str>,
}

fn zoned_date_time_calendar_identifier(source: &str) -> Option<Arc<str>> {
    let mut first_calendar = None;
    for annotation in source.as_bytes().split(|byte| *byte == b'[').skip(1) {
        let body = annotation.split(|byte| *byte == b']').next()?;
        let body = body.strip_prefix(b"!").unwrap_or(body);
        let Some(value) = body.strip_prefix(b"u-ca=") else {
            continue;
        };
        if first_calendar.is_none() {
            first_calendar = Some(value);
        }
    }
    if first_calendar.is_some_and(|value| !value.eq_ignore_ascii_case(b"iso8601")) {
        return None;
    }
    Some(Arc::from("iso8601"))
}

pub(crate) fn parse_zoned_date_time_string(source: &str) -> Option<ParsedZonedDateTime> {
    let parsed = parse_date_time(source, true)?;
    let annotation = std::str::from_utf8(parsed.time_zone_annotation?).ok()?;
    let (time_zone_identifier, offset_minutes) = parse_time_zone_identifier(annotation)?;
    let calendar_identifier = zoned_date_time_calendar_identifier(source)?;
    Some(ParsedZonedDateTime {
        local_nanoseconds: parsed.local_nanoseconds,
        source_offset_nanoseconds: parsed.offset_nanoseconds,
        z: parsed.z,
        time_zone_identifier,
        offset_minutes,
        calendar_identifier,
    })
}

pub(crate) fn has_time_zone_annotation(source: &str) -> bool {
    parse_date_time(source, true).is_some_and(|parsed| parsed.time_zone_annotation.is_some())
}

pub(crate) struct ParsedPlainDateTime {
    pub fields: IsoDateTimeFields,
    pub calendar_identifier: Arc<str>,
}

pub(crate) struct ParsedPlainDate {
    pub year: i128,
    pub month: i128,
    pub day: i128,
    pub calendar_identifier: Arc<str>,
}

pub(crate) struct ParsedPlainMonthDay {
    pub reference_iso_year: i128,
    pub month: i128,
    pub day: i128,
    pub calendar_identifier: Arc<str>,
}

pub(crate) struct ParsedPlainYearMonth {
    pub year: i128,
    pub month: i128,
    pub reference_iso_day: i128,
    pub calendar_identifier: Arc<str>,
}

pub(crate) fn parse_plain_date_string(source: &str) -> Option<ParsedPlainDate> {
    let parsed = parse_date_time(source, true)?;
    if parsed.z {
        return None;
    }
    let calendar_identifier = zoned_date_time_calendar_identifier(source)?;
    let date_time = iso_date_time(&BigInt::from(parsed.local_nanoseconds), 0)?;
    Some(ParsedPlainDate {
        year: date_time.year,
        month: date_time.month,
        day: date_time.day,
        calendar_identifier,
    })
}

pub(crate) fn parse_plain_date_time_string(source: &str) -> Option<ParsedPlainDateTime> {
    let parsed = parse_date_time(source, true)?;
    if parsed.z {
        return None;
    }
    let calendar_identifier = zoned_date_time_calendar_identifier(source)?;
    let date_time = iso_date_time(&BigInt::from(parsed.local_nanoseconds), 0)?;
    Some(ParsedPlainDateTime {
        fields: IsoDateTimeFields {
            year: date_time.year,
            month: date_time.month,
            day: date_time.day,
            hour: date_time.hour,
            minute: date_time.minute,
            second: date_time.second,
            millisecond: date_time.millisecond,
            microsecond: date_time.microsecond,
            nanosecond: date_time.nanosecond,
        },
        calendar_identifier,
    })
}

pub(crate) fn parse_plain_month_day_string(source: &str) -> Option<ParsedPlainMonthDay> {
    if let Some(parsed) = parse_date_time(source, true) {
        if parsed.z {
            return None;
        }
        let calendar_identifier = zoned_date_time_calendar_identifier(source)?;
        let date_time = iso_date_time(&BigInt::from(parsed.local_nanoseconds), 0)?;
        return Some(ParsedPlainMonthDay {
            reference_iso_year: 1972,
            month: date_time.month,
            day: date_time.day,
            calendar_identifier,
        });
    }

    let (month, day, _) = parse_annotated_month_day_fields(source.as_bytes())?;
    Some(ParsedPlainMonthDay {
        reference_iso_year: 1972,
        month,
        day,
        calendar_identifier: zoned_date_time_calendar_identifier(source)?,
    })
}

pub(crate) fn parse_plain_year_month_string(source: &str) -> Option<ParsedPlainYearMonth> {
    let (year, month) = if let Some(parsed) = parse_date_time(source, true) {
        if parsed.z {
            return None;
        }
        let date_time = iso_date_time(&BigInt::from(parsed.local_nanoseconds), 0)?;
        (date_time.year, date_time.month)
    } else {
        let (year, month, _) = parse_annotated_year_month_fields(source.as_bytes())?;
        (year, month)
    };
    if (year, month) < (-271_821, 4) || (year, month) > (275_760, 9) {
        return None;
    }
    Some(ParsedPlainYearMonth {
        year,
        month,
        reference_iso_day: 1,
        calendar_identifier: zoned_date_time_calendar_identifier(source)?,
    })
}

#[derive(Clone, Copy)]
pub(crate) enum ZonedDateTimeOffsetOption {
    Ignore,
    Prefer,
    Reject,
    Use,
}

pub(crate) struct IsoDateTimeFields {
    pub year: i128,
    pub month: i128,
    pub day: i128,
    pub hour: i128,
    pub minute: i128,
    pub second: i128,
    pub millisecond: i128,
    pub microsecond: i128,
    pub nanosecond: i128,
}

pub(crate) fn iso_date_time_to_local_nanoseconds(fields: IsoDateTimeFields) -> Option<i128> {
    let days = days_from_civil(fields.year, fields.month, fields.day)?;
    let seconds = days
        .checked_mul(SECONDS_PER_DAY)?
        .checked_add(fields.hour.checked_mul(3_600)?)?
        .checked_add(fields.minute.checked_mul(60)?)?
        .checked_add(fields.second)?;
    seconds
        .checked_mul(NS_PER_SECOND)?
        .checked_add(fields.millisecond.checked_mul(1_000_000)?)?
        .checked_add(fields.microsecond.checked_mul(1_000)?)?
        .checked_add(fields.nanosecond)
}

pub(crate) fn resolve_fixed_offset_epoch(
    local_nanoseconds: i128,
    source_offset_nanoseconds: Option<i128>,
    has_utc_designator: bool,
    time_zone_offset_minutes: i16,
    offset_option: ZonedDateTimeOffsetOption,
) -> Option<i128> {
    if has_utc_designator {
        return Some(local_nanoseconds);
    }
    let annotation_offset = i128::from(time_zone_offset_minutes) * 60 * NS_PER_SECOND;
    let selected_offset = match (source_offset_nanoseconds, offset_option) {
        (None, _) | (_, ZonedDateTimeOffsetOption::Ignore) => annotation_offset,
        (Some(source), ZonedDateTimeOffsetOption::Use) => source,
        (Some(source), ZonedDateTimeOffsetOption::Prefer) => {
            if source == annotation_offset {
                source
            } else {
                annotation_offset
            }
        }
        (Some(source), ZonedDateTimeOffsetOption::Reject) => {
            if source != annotation_offset {
                return None;
            }
            source
        }
    };
    let epoch_nanoseconds = local_nanoseconds.checked_sub(selected_offset)?;
    if source_offset_nanoseconds.is_some()
        && matches!(
            offset_option,
            ZonedDateTimeOffsetOption::Prefer | ZonedDateTimeOffsetOption::Reject
        )
    {
        let day_nanoseconds = SECONDS_PER_DAY * NS_PER_SECOND;
        let minimum = -100_000_000_i128 * day_nanoseconds;
        let maximum_local = 99_999_999_i128 * day_nanoseconds;
        let instant_limit = 100_000_000_i128 * day_nanoseconds;
        if local_nanoseconds < minimum
            || (local_nanoseconds > maximum_local && epoch_nanoseconds != instant_limit)
        {
            return None;
        }
    }
    Some(epoch_nanoseconds)
}

pub(crate) fn resolve_zoned_date_time_epoch(
    parsed: &ParsedZonedDateTime,
    offset_option: ZonedDateTimeOffsetOption,
) -> Option<BigInt> {
    resolve_fixed_offset_epoch(
        parsed.local_nanoseconds,
        parsed.source_offset_nanoseconds,
        parsed.z,
        parsed.offset_minutes,
        offset_option,
    )
    .map(BigInt::from)
}

fn minute_precision_offset(source: &[u8]) -> Option<i128> {
    let mut index = 0;
    let offset = parse_offset_nanoseconds(source, &mut index)?;
    (index == source.len()
        && !offset_has_sub_minute_syntax(source, 0, index)
        && offset % (60 * NS_PER_SECOND) == 0)
        .then_some(offset)
}

fn minute_offset_identifier(offset_nanoseconds: i128) -> Option<(Arc<str>, i16)> {
    if offset_nanoseconds % (60 * NS_PER_SECOND) != 0 {
        return None;
    }
    let offset_minutes = i16::try_from(offset_nanoseconds / (60 * NS_PER_SECOND)).ok()?;
    let sign = if offset_minutes < 0 { '-' } else { '+' };
    let magnitude = offset_minutes.unsigned_abs();
    Some((
        Arc::from(format!("{sign}{:02}:{:02}", magnitude / 60, magnitude % 60)),
        offset_minutes,
    ))
}

pub(crate) fn parse_time_zone_identifier(source: &str) -> Option<(Arc<str>, i16)> {
    if source.eq_ignore_ascii_case("UTC") {
        return Some((Arc::from("UTC"), 0));
    }
    let bytes = source.as_bytes();
    if !matches!(bytes.first(), Some(b'+') | Some(b'-')) {
        return None;
    }
    minute_offset_identifier(minute_precision_offset(bytes)?)
}

fn resolve_time_zone_syntax(
    offset_nanoseconds: Option<i128>,
    offset_has_sub_minute_syntax: bool,
    z: bool,
    annotation: Option<&[u8]>,
) -> Option<i128> {
    if let Some(annotation) = annotation {
        if annotation.eq_ignore_ascii_case(b"UTC") {
            return Some(0);
        }
        return minute_precision_offset(annotation);
    }
    if z {
        Some(0)
    } else {
        offset_nanoseconds
            .filter(|offset| !offset_has_sub_minute_syntax && offset % (60 * NS_PER_SECOND) == 0)
    }
}

struct ParsedTimeSyntax<'a> {
    hour: i128,
    minute: i128,
    second: i128,
    fraction_nanoseconds: i128,
    offset: Option<i128>,
    offset_has_sub_minute_syntax: bool,
    time_zone_annotation: Option<&'a [u8]>,
}

fn parse_time_syntax(source: &str) -> Option<ParsedTimeSyntax<'_>> {
    let bytes = source.as_bytes();
    let has_designator = matches!(bytes.first(), Some(b'T') | Some(b't'));
    let mut index = usize::from(has_designator);
    let hour = digits(bytes, &mut index, 2)?;
    let extended = bytes.get(index) == Some(&b':');
    let mut minute = 0;
    let mut second = 0;
    let minute_present = if extended {
        index += 1;
        minute = digits(bytes, &mut index, 2)?;
        true
    } else if bytes.get(index).is_some_and(u8::is_ascii_digit) {
        minute = digits(bytes, &mut index, 2)?;
        true
    } else {
        false
    };
    let second_present = if extended && bytes.get(index) == Some(&b':') {
        index += 1;
        second = digits(bytes, &mut index, 2)?;
        true
    } else if !extended && minute_present && bytes.get(index).is_some_and(u8::is_ascii_digit) {
        second = digits(bytes, &mut index, 2)?;
        true
    } else {
        false
    };
    let fraction_nanoseconds = if second_present {
        fraction_nanoseconds(bytes, &mut index)?
    } else {
        0
    };
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let (offset, sub_minute) = if matches!(bytes.get(index), Some(b'+') | Some(b'-')) {
        let start = index;
        let offset = parse_offset_nanoseconds(bytes, &mut index)?;
        (
            Some(offset),
            offset_has_sub_minute_syntax(bytes, start, index),
        )
    } else {
        (None, false)
    };
    if !has_designator
        && (parse_annotated_year_month(&bytes[..index]).is_some()
            || parse_annotated_month_day(&bytes[..index]).is_some())
    {
        return None;
    }
    let time_zone_annotation = parse_annotations(bytes, &mut index)?;
    (index == bytes.len()).then_some(ParsedTimeSyntax {
        hour,
        minute,
        second: second.min(59),
        fraction_nanoseconds,
        offset,
        offset_has_sub_minute_syntax: sub_minute,
        time_zone_annotation,
    })
}

pub(crate) struct ParsedPlainTime {
    pub hour: i128,
    pub minute: i128,
    pub second: i128,
    pub millisecond: i128,
    pub microsecond: i128,
    pub nanosecond: i128,
}

fn parsed_plain_time(
    hour: i128,
    minute: i128,
    second: i128,
    fraction_nanoseconds: i128,
) -> ParsedPlainTime {
    ParsedPlainTime {
        hour,
        minute,
        second,
        millisecond: fraction_nanoseconds / 1_000_000,
        microsecond: (fraction_nanoseconds / 1_000) % 1_000,
        nanosecond: fraction_nanoseconds % 1_000,
    }
}

pub(crate) fn parse_plain_time_string(source: &str) -> Option<ParsedPlainTime> {
    if let Some(parsed) = parse_time_syntax(source) {
        return Some(parsed_plain_time(
            parsed.hour,
            parsed.minute,
            parsed.second,
            parsed.fraction_nanoseconds,
        ));
    }

    let parsed = parse_date_time(source, false)?;
    if parsed.z {
        return None;
    }
    let date_time = iso_date_time(&BigInt::from(parsed.local_nanoseconds), 0)?;
    Some(parsed_plain_time(
        date_time.hour,
        date_time.minute,
        date_time.second,
        date_time.millisecond * 1_000_000 + date_time.microsecond * 1_000 + date_time.nanosecond,
    ))
}

fn parse_time_zone_from_time(source: &str) -> Option<i128> {
    let parsed = parse_time_syntax(source)?;
    resolve_time_zone_syntax(
        parsed.offset,
        parsed.offset_has_sub_minute_syntax,
        false,
        parsed.time_zone_annotation,
    )
}

fn parse_date_year(bytes: &[u8], index: &mut usize) -> Option<i128> {
    if matches!(bytes.get(*index), Some(b'+') | Some(b'-')) {
        let negative = bytes[*index] == b'-';
        *index += 1;
        let magnitude = digits(bytes, index, 6)?;
        if negative && magnitude == 0 {
            return None;
        }
        Some(if negative { -magnitude } else { magnitude })
    } else {
        digits(bytes, index, 4)
    }
}

fn annotations_at_end<'a>(bytes: &'a [u8], index: &mut usize) -> Option<Option<&'a [u8]>> {
    let annotation = parse_annotations(bytes, index)?;
    (*index == bytes.len()).then_some(annotation)
}

fn parse_annotated_full_date(bytes: &[u8]) -> Option<Option<&[u8]>> {
    let mut index = 0;
    let year = parse_date_year(bytes, &mut index)?;
    let extended = bytes.get(index) == Some(&b'-');
    if extended {
        index += 1;
    }
    let month = digits(bytes, &mut index, 2)?;
    if extended {
        take(bytes, &mut index, b'-')?;
    }
    let day = digits(bytes, &mut index, 2)?;
    if day == 0 || day > days_in_month(year, month)? {
        return None;
    }
    annotations_at_end(bytes, &mut index)
}

fn parse_annotated_year_month_fields(bytes: &[u8]) -> Option<(i128, i128, Option<&[u8]>)> {
    let mut index = 0;
    let year = parse_date_year(bytes, &mut index)?;
    if bytes.get(index) == Some(&b'-') {
        index += 1;
    }
    let month = digits(bytes, &mut index, 2)?;
    if !(1..=12).contains(&month) {
        return None;
    }
    Some((year, month, annotations_at_end(bytes, &mut index)?))
}

fn parse_annotated_year_month(bytes: &[u8]) -> Option<Option<&[u8]>> {
    parse_annotated_year_month_fields(bytes).map(|(_, _, annotation)| annotation)
}

fn parse_annotated_month_day_fields(bytes: &[u8]) -> Option<(i128, i128, Option<&[u8]>)> {
    let mut index = 0;
    if bytes.get(0..2) == Some(b"--") {
        index = 2;
    }
    let month = digits(bytes, &mut index, 2)?;
    if bytes.get(index) == Some(&b'-') {
        index += 1;
    }
    let day = digits(bytes, &mut index, 2)?;
    if day == 0 || day > days_in_month(1972, month)? {
        return None;
    }
    Some((month, day, annotations_at_end(bytes, &mut index)?))
}

fn parse_annotated_month_day(bytes: &[u8]) -> Option<Option<&[u8]>> {
    parse_annotated_month_day_fields(bytes).map(|(_, _, annotation)| annotation)
}

fn parse_time_zone_from_annotated_date(source: &str) -> Option<i128> {
    let bytes = source.as_bytes();
    let annotation = parse_annotated_full_date(bytes)
        .or_else(|| parse_annotated_year_month(bytes))
        .or_else(|| parse_annotated_month_day(bytes))??;
    resolve_time_zone_syntax(None, false, false, Some(annotation))
}

pub(crate) fn parse_time_zone_offset(source: &str) -> Option<i128> {
    if source.eq_ignore_ascii_case("UTC") {
        return Some(0);
    }
    if let Some(offset) = minute_precision_offset(source.as_bytes()) {
        return Some(offset);
    }

    if let Some(parsed) = parse_date_time(source, false) {
        if let Some(offset) = resolve_time_zone_syntax(
            parsed.offset_nanoseconds,
            parsed.offset_has_sub_minute_syntax,
            parsed.z,
            parsed.time_zone_annotation,
        ) {
            return Some(offset);
        }
    }
    parse_time_zone_from_time(source).or_else(|| parse_time_zone_from_annotated_date(source))
}

pub(crate) fn parse_time_zone_identifier_like(source: &str) -> Option<(Arc<str>, i16)> {
    if let Some(identifier) = parse_time_zone_identifier(source) {
        return Some(identifier);
    }
    if let Some(parsed) = parse_date_time(source, false) {
        if let Some(annotation) = parsed.time_zone_annotation {
            return parse_time_zone_identifier(std::str::from_utf8(annotation).ok()?);
        }
        if parsed.z {
            return Some((Arc::from("UTC"), 0));
        }
        if !parsed.offset_has_sub_minute_syntax {
            return minute_offset_identifier(parsed.offset_nanoseconds?);
        }
        return None;
    }
    minute_offset_identifier(
        parse_time_zone_from_time(source)
            .or_else(|| parse_time_zone_from_annotated_date(source))?,
    )
}

pub(crate) fn parse_offset_string(source: &str) -> Option<i128> {
    let mut index = 0;
    let offset = parse_offset_nanoseconds(source.as_bytes(), &mut index)?;
    (index == source.len()).then_some(offset)
}

pub(crate) fn parse_calendar_identifier(source: &str) -> Option<Arc<str>> {
    if source.eq_ignore_ascii_case("iso8601") {
        return Some(Arc::from("iso8601"));
    }
    let bytes = source.as_bytes();
    let valid_iso_syntax = parse_date_time(source, false).is_some()
        || parse_time_syntax(source).is_some()
        || parse_annotated_full_date(bytes).is_some()
        || parse_annotated_year_month(bytes).is_some()
        || parse_annotated_month_day(bytes).is_some();
    valid_iso_syntax.then(|| zoned_date_time_calendar_identifier(source))?
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InstantPrecision {
    Auto,
    Minute,
    Digits(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InstantRoundingMode {
    Ceil,
    Expand,
    Floor,
    HalfCeil,
    HalfEven,
    HalfExpand,
    HalfFloor,
    HalfTrunc,
    Trunc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TimeUnit {
    Hour,
    Minute,
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
}

impl TimeUnit {
    pub(crate) fn maximum_rounding_increment(self) -> u32 {
        match self {
            Self::Hour => 24,
            Self::Minute | Self::Second => 60,
            Self::Millisecond | Self::Microsecond | Self::Nanosecond => 1_000,
        }
    }

    fn nanoseconds(self) -> i128 {
        match self {
            Self::Hour => 3_600 * NS_PER_SECOND,
            Self::Minute => 60 * NS_PER_SECOND,
            Self::Second => NS_PER_SECOND,
            Self::Millisecond => 1_000_000,
            Self::Microsecond => 1_000,
            Self::Nanosecond => 1,
        }
    }
}

fn rounding_increment(precision: InstantPrecision) -> Option<i128> {
    match precision {
        InstantPrecision::Auto => Some(1),
        InstantPrecision::Minute => Some(60 * NS_PER_SECOND),
        InstantPrecision::Digits(digits) if digits <= 9 => Some(10_i128.pow(u32::from(9 - digits))),
        InstantPrecision::Digits(_) => None,
    }
}

fn round_as_if_positive(value: i128, increment: i128, mode: InstantRoundingMode) -> Option<i128> {
    let quotient = value.div_euclid(increment);
    let remainder = value.rem_euclid(increment);
    if remainder == 0 {
        return Some(value);
    }
    let lower = quotient.checked_mul(increment)?;
    let upper = lower.checked_add(increment)?;
    let choose_upper = match mode {
        InstantRoundingMode::Ceil | InstantRoundingMode::Expand => true,
        InstantRoundingMode::Floor | InstantRoundingMode::Trunc => false,
        InstantRoundingMode::HalfCeil
        | InstantRoundingMode::HalfEven
        | InstantRoundingMode::HalfExpand
        | InstantRoundingMode::HalfFloor
        | InstantRoundingMode::HalfTrunc => {
            let doubled = remainder.checked_mul(2)?;
            if doubled < increment {
                false
            } else if doubled > increment {
                true
            } else {
                match mode {
                    InstantRoundingMode::HalfCeil | InstantRoundingMode::HalfExpand => true,
                    InstantRoundingMode::HalfFloor | InstantRoundingMode::HalfTrunc => false,
                    InstantRoundingMode::HalfEven => quotient.rem_euclid(2) != 0,
                    _ => unreachable!(),
                }
            }
        }
    };
    Some(if choose_upper { upper } else { lower })
}

pub(crate) fn round_signed_to_increment(
    value: i128,
    increment: i128,
    mode: InstantRoundingMode,
) -> Option<i128> {
    if increment <= 0 {
        return None;
    }
    if value >= 0 {
        return round_as_if_positive(value, increment, mode);
    }
    let unsigned_mode = match mode {
        InstantRoundingMode::Ceil => InstantRoundingMode::Trunc,
        InstantRoundingMode::Floor => InstantRoundingMode::Expand,
        InstantRoundingMode::HalfCeil => InstantRoundingMode::HalfTrunc,
        InstantRoundingMode::HalfFloor => InstantRoundingMode::HalfExpand,
        other => other,
    };
    round_as_if_positive(value.checked_neg()?, increment, unsigned_mode)?.checked_neg()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn round_plain_time(
    hour: u8,
    minute: u8,
    second: u8,
    millisecond: u16,
    microsecond: u16,
    nanosecond: u16,
    increment: u32,
    unit: TimeUnit,
    rounding_mode: InstantRoundingMode,
) -> Option<(u8, u8, u8, u16, u16, u16)> {
    let total = (((((i128::from(hour) * 60 + i128::from(minute)) * 60 + i128::from(second))
        * 1_000
        + i128::from(millisecond))
        * 1_000
        + i128::from(microsecond))
        * 1_000)
        + i128::from(nanosecond);
    let prefix = match unit {
        TimeUnit::Hour => 0,
        TimeUnit::Minute => i128::from(hour) * 3_600 * NS_PER_SECOND,
        TimeUnit::Second => (i128::from(hour) * 3_600 + i128::from(minute) * 60) * NS_PER_SECOND,
        TimeUnit::Millisecond => {
            (i128::from(hour) * 3_600 + i128::from(minute) * 60 + i128::from(second))
                * NS_PER_SECOND
        }
        TimeUnit::Microsecond => {
            (i128::from(hour) * 3_600 + i128::from(minute) * 60 + i128::from(second))
                * NS_PER_SECOND
                + i128::from(millisecond) * 1_000_000
        }
        TimeUnit::Nanosecond => {
            (i128::from(hour) * 3_600 + i128::from(minute) * 60 + i128::from(second))
                * NS_PER_SECOND
                + i128::from(millisecond) * 1_000_000
                + i128::from(microsecond) * 1_000
        }
    };
    let quantity = total.checked_sub(prefix)?;
    let increment_nanoseconds = i128::from(increment).checked_mul(unit.nanoseconds())?;
    let rounded = prefix.checked_add(round_as_if_positive(
        quantity,
        increment_nanoseconds,
        rounding_mode,
    )?)?;
    let day_nanoseconds = SECONDS_PER_DAY.checked_mul(NS_PER_SECOND)?;
    let mut remainder = rounded.rem_euclid(day_nanoseconds);

    let rounded_hour = remainder / (3_600 * NS_PER_SECOND);
    remainder %= 3_600 * NS_PER_SECOND;
    let rounded_minute = remainder / (60 * NS_PER_SECOND);
    remainder %= 60 * NS_PER_SECOND;
    let rounded_second = remainder / NS_PER_SECOND;
    remainder %= NS_PER_SECOND;
    let rounded_millisecond = remainder / 1_000_000;
    remainder %= 1_000_000;
    let rounded_microsecond = remainder / 1_000;
    let rounded_nanosecond = remainder % 1_000;

    Some((
        u8::try_from(rounded_hour).ok()?,
        u8::try_from(rounded_minute).ok()?,
        u8::try_from(rounded_second).ok()?,
        u16::try_from(rounded_millisecond).ok()?,
        u16::try_from(rounded_microsecond).ok()?,
        u16::try_from(rounded_nanosecond).ok()?,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn add_plain_time(
    hour: u8,
    minute: u8,
    second: u8,
    millisecond: u16,
    microsecond: u16,
    nanosecond: u16,
    duration_nanoseconds: i128,
) -> Option<(u8, u8, u8, u16, u16, u16)> {
    let time_nanoseconds =
        (((((i128::from(hour) * 60 + i128::from(minute)) * 60 + i128::from(second)) * 1_000
            + i128::from(millisecond))
            * 1_000
            + i128::from(microsecond))
            * 1_000)
            + i128::from(nanosecond);
    let day_nanoseconds = SECONDS_PER_DAY.checked_mul(NS_PER_SECOND)?;
    let mut remainder = time_nanoseconds
        .checked_add(duration_nanoseconds)?
        .rem_euclid(day_nanoseconds);

    let result_hour = remainder / (3_600 * NS_PER_SECOND);
    remainder %= 3_600 * NS_PER_SECOND;
    let result_minute = remainder / (60 * NS_PER_SECOND);
    remainder %= 60 * NS_PER_SECOND;
    let result_second = remainder / NS_PER_SECOND;
    remainder %= NS_PER_SECOND;
    let result_millisecond = remainder / 1_000_000;
    remainder %= 1_000_000;
    let result_microsecond = remainder / 1_000;
    let result_nanosecond = remainder % 1_000;

    Some((
        u8::try_from(result_hour).ok()?,
        u8::try_from(result_minute).ok()?,
        u8::try_from(result_second).ok()?,
        u16::try_from(result_millisecond).ok()?,
        u16::try_from(result_microsecond).ok()?,
        u16::try_from(result_nanosecond).ok()?,
    ))
}

pub(crate) fn civil_from_days(days: i128) -> Option<(i128, i128, i128)> {
    let shifted = days.checked_add(719_468)?;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted.checked_sub(146_096)?
    } / 146_097;
    let day_of_era = shifted.checked_sub(era.checked_mul(146_097)?)?;
    let year_of_era = day_of_era
        .checked_sub(day_of_era / 1_460)?
        .checked_add(day_of_era / 36_524)?
        .checked_sub(day_of_era / 146_096)?
        / 365;
    let mut year = year_of_era.checked_add(era.checked_mul(400)?)?;
    let day_of_year = day_of_era.checked_sub(
        year_of_era
            .checked_mul(365)?
            .checked_add(year_of_era / 4)?
            .checked_sub(year_of_era / 100)?,
    )?;
    let shifted_month = day_of_year.checked_mul(5)?.checked_add(2)? / 153;
    let day = day_of_year
        .checked_sub(shifted_month.checked_mul(153)?.checked_add(2)? / 5)?
        .checked_add(1)?;
    let month = shifted_month.checked_add(if shifted_month < 10 { 3 } else { -9 })?;
    year = year.checked_add(i128::from(month <= 2))?;
    Some((year, month, day))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IsoDateTime {
    pub epoch_days: i128,
    pub year: i128,
    pub month: i128,
    pub day: i128,
    pub hour: i128,
    pub minute: i128,
    pub second: i128,
    pub millisecond: i128,
    pub microsecond: i128,
    pub nanosecond: i128,
}

pub(crate) fn iso_date_time(
    epoch_nanoseconds: &BigInt,
    offset_nanoseconds: i128,
) -> Option<IsoDateTime> {
    let local = epoch_nanoseconds
        .to_i128()?
        .checked_add(offset_nanoseconds)?;
    let nanoseconds_per_day = SECONDS_PER_DAY.checked_mul(NS_PER_SECOND)?;
    let epoch_days = local.div_euclid(nanoseconds_per_day);
    let within_day = local.rem_euclid(nanoseconds_per_day);
    let (year, month, day) = civil_from_days(epoch_days)?;
    let second_of_day = within_day / NS_PER_SECOND;
    let fraction = within_day % NS_PER_SECOND;
    Some(IsoDateTime {
        epoch_days,
        year,
        month,
        day,
        hour: second_of_day / 3_600,
        minute: second_of_day % 3_600 / 60,
        second: second_of_day % 60,
        millisecond: fraction / 1_000_000,
        microsecond: fraction / 1_000 % 1_000,
        nanosecond: fraction % 1_000,
    })
}

pub(crate) fn fixed_offset_start_of_day_epoch(
    epoch_nanoseconds: &BigInt,
    offset_nanoseconds: i128,
) -> Option<BigInt> {
    let epoch_days = iso_date_time(epoch_nanoseconds, offset_nanoseconds)?.epoch_days;
    let nanoseconds_per_day = SECONDS_PER_DAY.checked_mul(NS_PER_SECOND)?;
    let start = epoch_days
        .checked_mul(nanoseconds_per_day)?
        .checked_sub(offset_nanoseconds)?;
    Some(BigInt::from(start))
}

pub(crate) fn iso_day_of_week(epoch_days: i128) -> i128 {
    (epoch_days + 3).rem_euclid(7) + 1
}

pub(crate) fn iso_day_of_year(date_time: IsoDateTime) -> Option<i128> {
    let first = days_from_civil(date_time.year, 1, 1)?;
    date_time.epoch_days.checked_sub(first)?.checked_add(1)
}

fn iso_weeks_in_year(year: i128) -> Option<i128> {
    let first_day = iso_day_of_week(days_from_civil(year, 1, 1)?);
    Some(if first_day == 4 || (first_day == 3 && leap_year(year)) {
        53
    } else {
        52
    })
}

pub(crate) fn iso_week_of_year(date_time: IsoDateTime) -> Option<(i128, i128)> {
    let day_of_year = iso_day_of_year(date_time)?;
    let day_of_week = iso_day_of_week(date_time.epoch_days);
    let mut week = (day_of_year - day_of_week + 10) / 7;
    let mut year = date_time.year;
    if week < 1 {
        year = year.checked_sub(1)?;
        week = iso_weeks_in_year(year)?;
    } else {
        let weeks = iso_weeks_in_year(year)?;
        if week > weeks {
            year = year.checked_add(1)?;
            week = 1;
        }
    }
    Some((week, year))
}

fn write_year(output: &mut String, year: i128) -> Option<()> {
    if (0..=9_999).contains(&year) {
        write!(output, "{year:04}").ok()?;
    } else if year < 0 {
        write!(output, "-{:06}", year.checked_neg()?).ok()?;
    } else {
        write!(output, "+{year:06}").ok()?;
    }
    Some(())
}

fn format_iso_date_time(
    epoch_nanoseconds: &BigInt,
    offset_nanoseconds: i128,
    precision: InstantPrecision,
    rounding_mode: InstantRoundingMode,
) -> Option<String> {
    let epoch_nanoseconds = epoch_nanoseconds.to_i128()?;
    let increment = rounding_increment(precision)?;
    let rounded = round_as_if_positive(epoch_nanoseconds, increment, rounding_mode)?;
    let local = rounded.checked_add(offset_nanoseconds)?;
    let nanoseconds_per_day = SECONDS_PER_DAY.checked_mul(NS_PER_SECOND)?;
    let days = local.div_euclid(nanoseconds_per_day);
    let within_day = local.rem_euclid(nanoseconds_per_day);
    let (year, month, day) = civil_from_days(days)?;
    let mut output = String::with_capacity(40);
    write_year(&mut output, year)?;
    write!(output, "-{month:02}-{day:02}T").ok()?;
    write_time(&mut output, within_day, precision)?;
    Some(output)
}

fn write_time(
    output: &mut String,
    within_day_nanoseconds: i128,
    precision: InstantPrecision,
) -> Option<()> {
    let second_of_day = within_day_nanoseconds / NS_PER_SECOND;
    let hour = second_of_day / 3_600;
    let minute = second_of_day % 3_600 / 60;
    let second = second_of_day % 60;
    let fraction = within_day_nanoseconds % NS_PER_SECOND;

    write!(output, "{hour:02}:{minute:02}").ok()?;
    if precision != InstantPrecision::Minute {
        write!(output, ":{second:02}").ok()?;
        match precision {
            InstantPrecision::Auto if fraction != 0 => {
                let fraction = format!("{fraction:09}");
                output.push('.');
                output.push_str(fraction.trim_end_matches('0'));
            }
            InstantPrecision::Digits(digits) if digits != 0 => {
                let divisor = 10_i128.pow(u32::from(9 - digits));
                write!(
                    output,
                    ".{:0width$}",
                    fraction / divisor,
                    width = usize::from(digits)
                )
                .ok()?;
            }
            _ => {}
        }
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn format_plain_time(
    hour: u8,
    minute: u8,
    second: u8,
    millisecond: u16,
    microsecond: u16,
    nanosecond: u16,
    precision: InstantPrecision,
    rounding_mode: InstantRoundingMode,
) -> Option<String> {
    let quantity = (((((i128::from(hour) * 60 + i128::from(minute)) * 60 + i128::from(second))
        * 1_000
        + i128::from(millisecond))
        * 1_000
        + i128::from(microsecond))
        * 1_000)
        + i128::from(nanosecond);
    let rounded = round_as_if_positive(quantity, rounding_increment(precision)?, rounding_mode)?;
    let nanoseconds_per_day = SECONDS_PER_DAY.checked_mul(NS_PER_SECOND)?;
    let within_day = rounded.rem_euclid(nanoseconds_per_day);
    let mut output = String::with_capacity(18);
    write_time(&mut output, within_day, precision)?;
    Some(output)
}

pub(crate) fn format_duration(fields: [i128; 10], precision: InstantPrecision) -> Option<String> {
    let sign = fields
        .iter()
        .copied()
        .find(|value| *value != 0)
        .map_or(0, i128::signum);
    let values = fields.map(i128::checked_abs);
    let [Some(years), Some(months), Some(weeks), Some(days), Some(hours), Some(minutes), Some(seconds), Some(milliseconds), Some(microseconds), Some(nanoseconds)] =
        values
    else {
        return None;
    };
    let seconds_nanoseconds = seconds
        .checked_mul(NS_PER_SECOND)?
        .checked_add(milliseconds.checked_mul(1_000_000)?)?
        .checked_add(microseconds.checked_mul(1_000)?)?
        .checked_add(nanoseconds)?;

    let mut output = String::with_capacity(64);
    if sign < 0 {
        output.push('-');
    }
    output.push('P');
    if years != 0 {
        write!(output, "{years}Y").ok()?;
    }
    if months != 0 {
        write!(output, "{months}M").ok()?;
    }
    if weeks != 0 {
        write!(output, "{weeks}W").ok()?;
    }
    if days != 0 {
        write!(output, "{days}D").ok()?;
    }

    let fixed_precision = !matches!(precision, InstantPrecision::Auto);
    let date_is_zero = years == 0 && months == 0 && weeks == 0 && days == 0;
    let has_time = hours != 0 || minutes != 0 || seconds_nanoseconds != 0;
    if date_is_zero || has_time || fixed_precision {
        output.push('T');
        if hours != 0 {
            write!(output, "{hours}H").ok()?;
        }
        if minutes != 0 {
            write!(output, "{minutes}M").ok()?;
        }
        let emit_seconds = seconds_nanoseconds != 0
            || (date_is_zero && hours == 0 && minutes == 0)
            || fixed_precision;
        if emit_seconds {
            let whole_seconds = seconds_nanoseconds / NS_PER_SECOND;
            let fraction = seconds_nanoseconds % NS_PER_SECOND;
            write!(output, "{whole_seconds}").ok()?;
            match precision {
                InstantPrecision::Auto if fraction != 0 => {
                    let digits = format!("{fraction:09}");
                    output.push('.');
                    output.push_str(digits.trim_end_matches('0'));
                }
                InstantPrecision::Digits(digits @ 1..=9) => {
                    let divisor = 10_i128.pow(u32::from(9 - digits));
                    write!(
                        output,
                        ".{:0width$}",
                        fraction / divisor,
                        width = usize::from(digits)
                    )
                    .ok()?;
                }
                InstantPrecision::Auto | InstantPrecision::Digits(0) => {}
                InstantPrecision::Minute | InstantPrecision::Digits(_) => return None,
            }
            output.push('S');
        }
    }
    Some(output)
}

fn write_offset(output: &mut String, offset_nanoseconds: i128) -> Option<()> {
    let sign = if offset_nanoseconds < 0 { '-' } else { '+' };
    let total_minutes = offset_nanoseconds.abs() / (60 * NS_PER_SECOND);
    write!(
        output,
        "{sign}{:02}:{:02}",
        total_minutes / 60,
        total_minutes % 60
    )
    .ok()
}

pub(crate) fn format_instant(
    epoch_nanoseconds: &BigInt,
    display_offset_nanoseconds: Option<i128>,
    precision: InstantPrecision,
    rounding_mode: InstantRoundingMode,
) -> Option<String> {
    let offset = display_offset_nanoseconds.unwrap_or(0);
    let mut output = format_iso_date_time(epoch_nanoseconds, offset, precision, rounding_mode)?;
    if let Some(offset) = display_offset_nanoseconds {
        write_offset(&mut output, offset)?;
    } else {
        output.push('Z');
    }
    Some(output)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AnnotationDisplay {
    Auto,
    Always,
    Critical,
    Never,
}

fn write_calendar_annotation(
    output: &mut String,
    calendar_identifier: &str,
    display: AnnotationDisplay,
) {
    let show = match display {
        AnnotationDisplay::Auto => calendar_identifier != "iso8601",
        AnnotationDisplay::Always | AnnotationDisplay::Critical => true,
        AnnotationDisplay::Never => false,
    };
    if show {
        output.push('[');
        if display == AnnotationDisplay::Critical {
            output.push('!');
        }
        output.push_str("u-ca=");
        output.push_str(calendar_identifier);
        output.push(']');
    }
}

pub(crate) fn format_plain_date(
    year: i32,
    month: u8,
    day: u8,
    calendar_identifier: &str,
    calendar_name: AnnotationDisplay,
) -> Option<String> {
    let mut output = String::with_capacity(32);
    write_year(&mut output, i128::from(year))?;
    write!(output, "-{month:02}-{day:02}").ok()?;
    write_calendar_annotation(&mut output, calendar_identifier, calendar_name);
    Some(output)
}

pub(crate) fn format_plain_month_day(
    reference_iso_year: i32,
    month: u8,
    day: u8,
    calendar_identifier: &str,
    calendar_name: AnnotationDisplay,
) -> Option<String> {
    let show_reference_year = matches!(
        calendar_name,
        AnnotationDisplay::Always | AnnotationDisplay::Critical
    ) || calendar_identifier != "iso8601";
    let mut output = String::with_capacity(32);
    if show_reference_year {
        write_year(&mut output, i128::from(reference_iso_year))?;
        output.push('-');
    }
    write!(output, "{month:02}-{day:02}").ok()?;
    write_calendar_annotation(&mut output, calendar_identifier, calendar_name);
    Some(output)
}

pub(crate) fn format_plain_year_month(
    year: i32,
    month: u8,
    reference_iso_day: u8,
    calendar_identifier: &str,
    calendar_name: AnnotationDisplay,
) -> Option<String> {
    let show_reference_day = matches!(
        calendar_name,
        AnnotationDisplay::Always | AnnotationDisplay::Critical
    ) || calendar_identifier != "iso8601";
    let mut output = String::with_capacity(32);
    write_year(&mut output, i128::from(year))?;
    write!(output, "-{month:02}").ok()?;
    if show_reference_day {
        write!(output, "-{reference_iso_day:02}").ok()?;
    }
    write_calendar_annotation(&mut output, calendar_identifier, calendar_name);
    Some(output)
}

#[derive(Clone, Copy)]
pub(crate) struct ZonedDateTimeFormatOptions {
    pub precision: InstantPrecision,
    pub rounding_mode: InstantRoundingMode,
    pub show_offset: bool,
    pub time_zone_name: AnnotationDisplay,
    pub calendar_name: AnnotationDisplay,
}

pub(crate) fn format_zoned_date_time(
    epoch_nanoseconds: &BigInt,
    offset_nanoseconds: i128,
    time_zone_identifier: &str,
    calendar_identifier: &str,
    options: ZonedDateTimeFormatOptions,
) -> Option<String> {
    let mut output = format_iso_date_time(
        epoch_nanoseconds,
        offset_nanoseconds,
        options.precision,
        options.rounding_mode,
    )?;
    if options.show_offset {
        write_offset(&mut output, offset_nanoseconds)?;
    }
    if options.time_zone_name != AnnotationDisplay::Never {
        output.push('[');
        if options.time_zone_name == AnnotationDisplay::Critical {
            output.push('!');
        }
        output.push_str(time_zone_identifier);
        output.push(']');
    }
    write_calendar_annotation(&mut output, calendar_identifier, options.calendar_name);
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::{
        add_plain_time, format_duration, format_instant, format_plain_date, format_plain_month_day,
        format_plain_time, format_plain_year_month, parse_calendar_identifier,
        parse_duration_string, parse_instant_string, parse_offset_string, parse_plain_date_string,
        parse_plain_date_time_string, parse_plain_month_day_string, parse_plain_time_string,
        parse_plain_year_month_string, parse_time_zone_identifier, parse_time_zone_identifier_like,
        parse_time_zone_offset, parse_zoned_date_time_string, resolve_zoned_date_time_epoch,
        round_signed_to_increment, AnnotationDisplay, InstantPrecision, InstantRoundingMode,
        ZonedDateTimeOffsetOption,
    };
    use num_bigint::BigInt;

    #[test]
    fn parses_duration_strings_with_exact_fractional_time_units() {
        let duration =
            parse_duration_string("-P1Y2M3W4DT5H6M7.987654321S").expect("duration should parse");
        assert_eq!(
            [
                duration.years,
                duration.months,
                duration.weeks,
                duration.days,
                duration.hours,
                duration.minutes,
                duration.seconds,
                duration.milliseconds,
                duration.microseconds,
                duration.nanoseconds,
            ],
            [-1.0, -2.0, -3.0, -4.0, -5.0, -6.0, -7.0, -987.0, -654.0, -321.0]
        );

        let hours = parse_duration_string("PT0.999999999H").expect("hours should parse");
        assert_eq!(
            [
                hours.hours,
                hours.minutes,
                hours.seconds,
                hours.milliseconds,
                hours.microseconds,
                hours.nanoseconds,
            ],
            [0.0, 59.0, 59.0, 999.0, 996.0, 400.0]
        );
        let minutes = parse_duration_string("PT0.000000011M").expect("minutes should parse");
        assert_eq!(
            [
                minutes.seconds,
                minutes.milliseconds,
                minutes.microseconds,
                minutes.nanoseconds,
            ],
            [0.0, 0.0, 0.0, 660.0]
        );
    }

    #[test]
    fn rejects_invalid_duration_string_structure() {
        for source in [
            "",
            "P",
            "PT",
            "P0.5Y",
            "P2H",
            "PT.1H",
            "PT2.H",
            "PT0.1H0M",
            "PT0.1M0S",
            "PT1S1M",
            "P1D1Y",
            "PT1.1234567890S",
        ] {
            assert!(parse_duration_string(source).is_none(), "{source}");
        }
    }

    #[test]
    fn adds_plain_time_with_exact_midnight_wrapping() {
        assert_eq!(
            add_plain_time(23, 59, 59, 999, 999, 999, 1),
            Some((0, 0, 0, 0, 0, 0))
        );
        assert_eq!(
            add_plain_time(0, 0, 0, 0, 0, 0, -1),
            Some((23, 59, 59, 999, 999, 999))
        );
        assert_eq!(
            add_plain_time(12, 34, 56, 123, 456, 789, 86_400_000_000_000),
            Some((12, 34, 56, 123, 456, 789))
        );
    }

    #[test]
    fn formats_plain_dates_with_shared_calendar_annotations() {
        assert_eq!(
            format_plain_date(2000, 5, 2, "gregory", AnnotationDisplay::Auto).as_deref(),
            Some("2000-05-02[u-ca=gregory]")
        );
        assert_eq!(
            format_plain_date(2000, 5, 2, "gregory", AnnotationDisplay::Never).as_deref(),
            Some("2000-05-02")
        );
        assert_eq!(
            format_plain_date(-1, 8, 7, "gregory", AnnotationDisplay::Critical).as_deref(),
            Some("-000001-08-07[!u-ca=gregory]")
        );
    }

    #[test]
    fn formats_partial_dates_with_reference_components_only_for_annotations() {
        assert_eq!(
            format_plain_month_day(1972, 5, 2, "iso8601", AnnotationDisplay::Auto).as_deref(),
            Some("05-02")
        );
        assert_eq!(
            format_plain_month_day(1960, 11, 16, "iso8601", AnnotationDisplay::Always).as_deref(),
            Some("1960-11-16[u-ca=iso8601]")
        );
        assert_eq!(
            format_plain_month_day(1972, 5, 2, "iso8601", AnnotationDisplay::Critical).as_deref(),
            Some("1972-05-02[!u-ca=iso8601]")
        );
        assert_eq!(
            format_plain_month_day(1960, 11, 16, "gregory", AnnotationDisplay::Never).as_deref(),
            Some("1960-11-16")
        );
        assert_eq!(
            format_plain_year_month(2000, 5, 7, "iso8601", AnnotationDisplay::Auto).as_deref(),
            Some("2000-05")
        );
        assert_eq!(
            format_plain_year_month(2000, 5, 7, "iso8601", AnnotationDisplay::Always).as_deref(),
            Some("2000-05-07[u-ca=iso8601]")
        );
        assert_eq!(
            format_plain_year_month(-1, 8, 1, "iso8601", AnnotationDisplay::Never).as_deref(),
            Some("-000001-08")
        );
        assert_eq!(
            format_plain_year_month(2000, 5, 7, "gregory", AnnotationDisplay::Never).as_deref(),
            Some("2000-05-07")
        );
        assert_eq!(
            format_plain_year_month(2000, 5, 7, "gregory", AnnotationDisplay::Auto).as_deref(),
            Some("2000-05-07[u-ca=gregory]")
        );
    }

    #[test]
    fn parses_exact_canonical_instants() {
        assert_eq!(
            parse_instant_string("1976-11-18T15:23:30.123456789Z"),
            Some(BigInt::from(217_178_610_123_456_789_i128))
        );
        assert_eq!(
            parse_instant_string("1963-02-13T09:36:29.123456789Z"),
            Some(BigInt::from(-217_175_010_876_543_211_i128))
        );
        assert_eq!(
            parse_instant_string("2016-12-31T23:59:60Z"),
            Some(BigInt::from(1_483_228_799_000_000_000_i128))
        );
    }

    #[test]
    fn rejects_normalized_or_incomplete_dates() {
        for source in [
            "2020-02-30T00:00Z",
            "2020-01-01T00:00",
            "-000000-01-01T00:00Z",
            "2020-01-01T00:00:00.1234567890Z",
        ] {
            assert!(parse_instant_string(source).is_none(), "{source}");
        }
    }

    #[test]
    fn preserves_gregorian_offset_and_instant_boundaries() {
        let cases = [
            (
                "-271821-04-20T00:00:00Z",
                -8_640_000_000_000_000_000_000_i128,
            ),
            (
                "+275760-09-13T00:00:00Z",
                8_640_000_000_000_000_000_000_i128,
            ),
            ("1970-01-01T00:30:00+01:00", -1_800_000_000_000_i128),
            ("2000-02-29T00:00:00Z", 951_782_400_000_000_000_i128),
            ("-000001-01-01T00:00:00Z", -62_198_755_200_000_000_000_i128),
        ];
        for (source, expected) in cases {
            assert_eq!(
                parse_instant_string(source),
                Some(BigInt::from(expected)),
                "{source}"
            );
        }
        assert!(parse_instant_string("1900-02-29T00:00:00Z").is_none());
    }

    #[test]
    fn parses_basic_time_and_nanosecond_offsets() {
        let cases = [
            ("19761118T152330.1+0000", 217_178_610_100_000_000_i128),
            ("1976-11-18T15Z", 217_177_200_000_000_000_i128),
            ("1970-01-01T00:19:32.37+00:19:32.37", 0_i128),
            (
                "-271821-04-19T00:00:00.000000001-23:59:59.999999999",
                -8_640_000_000_000_000_000_000_i128,
            ),
        ];
        for (source, expected) in cases {
            assert_eq!(
                parse_instant_string(source),
                Some(BigInt::from(expected)),
                "{source}"
            );
        }
    }

    #[test]
    fn validates_rfc_9557_annotations_without_resolving_them() {
        for source in [
            "1970-01-01T00:00Z[UTC][u-ca=gregory]",
            "1970-01-01T00:00Z[!Europe/Vienna][foo=bar]",
            "1970-01-01T00:00Z[u-ca=iso8601][u-ca=discord]",
            "1970-01-01T00:00Z[+12]",
            "1970-01-01T00:00Z[.][foo=Alpha-123]",
        ] {
            assert_eq!(
                parse_instant_string(source),
                Some(BigInt::from(0)),
                "{source}"
            );
        }

        for source in [
            "1970-01-01T00:00Z[UTC][UTC]",
            "1970-01-01T00:00Z[u-ca=iso8601][!u-ca=gregory]",
            "1970-01-01T00:00Z[!foo=bar]",
            "1970-01-01T00:00Z[U-CA=iso8601]",
            "1970-01-01T00:00Z[-07:00:00]",
            "1970-01-01T00:00Z[UTC/]",
            "1970-01-01T00:00Z[1UTC]",
            "1970-01-01T00:00Z[foo=bad_value]",
            "1970-01-01T00:00Z[foo=bad--value]",
            "1970-01-01T00:00Z[foo=bar][UTC]",
            "1970-01-01T00:00:00+00:0000",
        ] {
            assert!(parse_instant_string(source).is_none(), "{source}");
        }
    }

    #[test]
    fn resolves_fixed_and_utc_time_zone_identifiers() {
        let minute = 60_i128 * 1_000_000_000;
        for (source, expected) in [
            ("UTC", 0),
            ("-01:30", -90 * minute),
            ("2021-08-19T17:30Z", 0),
            ("2021-08-19T17:30-07:00", -420 * minute),
            ("2021-08-19T17:30-07:00[UTC]", 0),
            ("12:34+01:00", 60 * minute),
            ("12:34[UTC]", 0),
            ("T12:34+01:00", 60 * minute),
            ("t12:34[UTC]", 0),
            ("2021-08-19[UTC]", 0),
            ("2021-08[+01:30]", 90 * minute),
            ("--02-29[UTC]", 0),
            ("08-19[UTC]", 0),
            ("2021-08-19T17:30:45.123456789-12:12[+01:46]", 106 * minute),
        ] {
            assert_eq!(parse_time_zone_offset(source), Some(expected), "{source}");
        }
        for source in [
            "2021-08-19T17:30",
            "-12:12:59.9",
            "2021-08-19T17:30-07:00:00",
            "2021-08-19T17:30-07:00:01",
            "2021-08-19T17:30[Europe/Vienna]",
            "12:34Z",
            "2021-08",
            "08-19",
            "24:00+01:00",
            "2021-02-30[UTC]",
            "9999-13[UTC]",
            "02-30[UTC]",
            "-000000-01-01T00:00Z",
        ] {
            assert_eq!(parse_time_zone_offset(source), None, "{source}");
        }
    }

    #[test]
    fn parses_only_constructor_time_zone_identifiers() {
        for (source, identifier, offset_minutes) in [
            ("utc", "UTC", 0),
            ("-00", "+00:00", 0),
            ("-00:00", "+00:00", 0),
            ("+01", "+01:00", 60),
            ("-01:30", "-01:30", -90),
            ("+0130", "+01:30", 90),
        ] {
            let (actual_identifier, actual_offset) =
                parse_time_zone_identifier(source).expect("identifier should parse");
            assert_eq!(actual_identifier.as_ref(), identifier, "{source}");
            assert_eq!(actual_offset, offset_minutes, "{source}");
        }
        for source in [
            "",
            "Europe/Vienna",
            "1997-12-04T12:34[+01:00]",
            "+24:00",
            "+01:00:00",
        ] {
            assert!(parse_time_zone_identifier(source).is_none(), "{source}");
        }
    }

    #[test]
    fn parses_property_bag_calendar_time_zone_and_offset_strings() {
        for source in [
            "iso8601",
            "ISO8601",
            "2020-01-01",
            "2020-01-01T00:00[u-ca=iso8601]",
            "2020-01",
            "01-01",
            "15:23",
            "T152330.123456789",
            "152330.1-08",
            "15:23:30.12-02:00",
        ] {
            assert_eq!(
                parse_calendar_identifier(source).as_deref(),
                Some("iso8601"),
                "{source}"
            );
        }
        for source in [
            "gregory",
            "2020-01-01[u-ca=gregory]",
            "15:23[u-ca=gregory]",
            "24:00",
            "invalid",
        ] {
            assert!(parse_calendar_identifier(source).is_none(), "{source}");
        }

        for (source, identifier, minutes) in [
            ("2021-08-19T17:30Z", "UTC", 0),
            ("2021-08-19T17:30-07:00", "-07:00", -420),
            ("2021-08-19T17:30-07:00[UTC]", "UTC", 0),
            ("+0130", "+01:30", 90),
        ] {
            let (actual_identifier, actual_minutes) =
                parse_time_zone_identifier_like(source).expect("time zone should parse");
            assert_eq!(actual_identifier.as_ref(), identifier, "{source}");
            assert_eq!(actual_minutes, minutes, "{source}");
        }
        for source in ["2021-08-19T17:30", "2021-08-19T17:30-07:00:00"] {
            assert!(
                parse_time_zone_identifier_like(source).is_none(),
                "{source}"
            );
        }

        assert_eq!(
            parse_offset_string("+01:02:03.004005006"),
            Some(3_723_004_005_006)
        );
        assert_eq!(parse_offset_string("-00:00"), Some(0));
        assert!(parse_offset_string("+01:00junk").is_none());
    }

    #[test]
    fn zoned_date_time_strings_preserve_only_supported_iso_calendars() {
        let parsed = parse_zoned_date_time_string("1970-01-01T00:00Z[UTC][u-ca=ISO8601]")
            .expect("ISO calendar annotation should parse");
        assert_eq!(
            resolve_zoned_date_time_epoch(&parsed, ZonedDateTimeOffsetOption::Reject),
            Some(BigInt::from(0))
        );
        assert_eq!(parsed.time_zone_identifier.as_ref(), "UTC");
        assert_eq!(parsed.calendar_identifier.as_ref(), "iso8601");

        let date_only = parse_zoned_date_time_string("1970-01-01[UtC]")
            .expect("ZonedDateTime strings may omit the time");
        assert_eq!(
            resolve_zoned_date_time_epoch(&date_only, ZonedDateTimeOffsetOption::Reject),
            Some(BigInt::from(0))
        );
        assert_eq!(date_only.time_zone_identifier.as_ref(), "UTC");
        assert!(parse_instant_string("1970-01-01[UTC]").is_none());
        assert!(parse_zoned_date_time_string("1970-01-01Z[UTC]").is_none());
        assert!(parse_zoned_date_time_string("1970-01-01+00:00[UTC]").is_none());

        let exact =
            parse_zoned_date_time_string("1970-01-01T00:00Z[+01:00][u-ca=iso8601][u-ca=gregory]")
                .expect("Z and a later noncritical calendar should parse");
        assert_eq!(
            resolve_zoned_date_time_epoch(&exact, ZonedDateTimeOffsetOption::Reject),
            Some(BigInt::from(0))
        );

        let wall = parse_zoned_date_time_string("1970-01-01T00:00[+01:00]")
            .expect("wall time should parse");
        assert_eq!(
            resolve_zoned_date_time_epoch(&wall, ZonedDateTimeOffsetOption::Reject),
            Some(BigInt::from(-3_600_000_000_000_i64))
        );

        for source in [
            "1970-01-01T00:00Z[UTC][u-ca=gregory]",
            "1970-01-01T00:00Z[UTC][!u-ca=gregory]",
        ] {
            assert!(parse_zoned_date_time_string(source).is_none(), "{source}");
        }
    }

    #[test]
    fn plain_date_time_strings_ignore_optional_offsets_and_zones() {
        let date_only = parse_plain_date_time_string("1976-11-18")
            .expect("date-only PlainDateTime should parse");
        assert_eq!(date_only.fields.year, 1976);
        assert_eq!(date_only.fields.month, 11);
        assert_eq!(date_only.fields.day, 18);
        assert_eq!(date_only.fields.hour, 0);

        let with_optional_data = parse_plain_date_time_string(
            "1976-11-18T15:23:30.123456789+23:59[Custom][u-ca=ISO8601]",
        )
        .expect("offset and time-zone annotation should be ignored");
        assert_eq!(with_optional_data.fields.hour, 15);
        assert_eq!(with_optional_data.fields.minute, 23);
        assert_eq!(with_optional_data.fields.second, 30);
        assert_eq!(with_optional_data.fields.millisecond, 123);
        assert_eq!(with_optional_data.fields.microsecond, 456);
        assert_eq!(with_optional_data.fields.nanosecond, 789);
        assert_eq!(with_optional_data.calendar_identifier.as_ref(), "iso8601");

        assert!(parse_plain_date_time_string("1976-11-18T15:23Z").is_none());
        assert!(parse_plain_date_time_string("1976-11-18+00:00").is_none());
        assert!(parse_plain_date_time_string("1976-11-18[u-ca=gregory]").is_none());
        assert!(parse_plain_date_time_string("-000000-11-18").is_none());
    }

    #[test]
    fn parses_partial_date_strings_without_applying_offsets() {
        for source in [
            "05-02",
            "--0502[Asia/Tokyo][u-ca=ISO8601]",
            "2000-05-02T00-02:30[America/St_Johns]",
        ] {
            let parsed = parse_plain_month_day_string(source).expect("MonthDay should parse");
            assert_eq!(
                (parsed.reference_iso_year, parsed.month, parsed.day),
                (1972, 5, 2)
            );
            assert_eq!(parsed.calendar_identifier.as_ref(), "iso8601");
        }
        for source in ["09-15Z", "2022-09-15+00:00", "09-15[u-ca=chinese]"] {
            assert!(parse_plain_month_day_string(source).is_none(), "{source}");
        }

        for source in [
            "2019-12",
            "201912[Africa/Abidjan][u-ca=ISO8601]",
            "2019-12-15T00+01[Europe/Vienna]",
        ] {
            let parsed = parse_plain_year_month_string(source).expect("YearMonth should parse");
            assert_eq!(
                (parsed.year, parsed.month, parsed.reference_iso_day),
                (2019, 12, 1)
            );
            assert_eq!(parsed.calendar_identifier.as_ref(), "iso8601");
        }
        for source in [
            "2022-09Z",
            "2022-09-15+00:00",
            "2022-09[u-ca=hebrew]",
            "-271821-03-31",
            "+275760-10",
        ] {
            assert!(parse_plain_year_month_string(source).is_none(), "{source}");
        }
    }

    #[test]
    fn plain_time_strings_accept_time_context_grammar_and_ignore_optional_data() {
        for source in [
            "12:34:56.123456789",
            "T123456.123456789+23:59[Custom][u-ca=gregory]",
            "1976-11-18T12:34:56.123456789+23:59[Custom][u-ca=gregory]",
        ] {
            let parsed = parse_plain_time_string(source).expect("PlainTime should parse");
            assert_eq!(parsed.hour, 12, "{source}");
            assert_eq!(parsed.minute, 34, "{source}");
            assert_eq!(parsed.second, 56, "{source}");
            assert_eq!(parsed.millisecond, 123, "{source}");
            assert_eq!(parsed.microsecond, 456, "{source}");
            assert_eq!(parsed.nanosecond, 789, "{source}");
        }

        let leap =
            parse_plain_time_string("2016-12-31T23:59:60").expect("leap second should clamp");
        assert_eq!(leap.second, 59);
        for source in [
            "1976-11-18",
            "1976-11-18T12:34Z",
            "-000000-11-18T12:34",
            "2021-12",
        ] {
            assert!(parse_plain_time_string(source).is_none(), "{source}");
        }
    }

    #[test]
    fn plain_date_strings_extract_local_date_and_allow_offsets_with_time() {
        for source in [
            "1976-11-18",
            "1976-11-18T15:23+00:00",
            "1976-11-18T15:23+23:59[Custom][u-ca=ISO8601]",
        ] {
            let parsed = parse_plain_date_string(source).expect("PlainDate string should parse");
            assert_eq!(parsed.year, 1976, "{source}");
            assert_eq!(parsed.month, 11, "{source}");
            assert_eq!(parsed.day, 18, "{source}");
            assert_eq!(parsed.calendar_identifier.as_ref(), "iso8601", "{source}");
        }

        for source in [
            "1976-11-18Z",
            "1976-11-18T15:23Z",
            "1976-11-18+00:00",
            "1976-11-18[u-ca=gregory]",
            "-000000-11-18",
        ] {
            assert!(parse_plain_date_string(source).is_none(), "{source}");
        }
    }

    #[test]
    fn formats_instants_with_precision_offsets_and_as_if_positive_rounding() {
        let epoch = BigInt::from(217_175_010_123_456_789_i128);
        assert_eq!(
            format_instant(
                &epoch,
                None,
                InstantPrecision::Auto,
                InstantRoundingMode::Trunc,
            )
            .as_deref(),
            Some("1976-11-18T14:23:30.123456789Z")
        );
        assert_eq!(
            format_instant(
                &BigInt::from(0),
                Some(-90 * 60 * 1_000_000_000_i128),
                InstantPrecision::Digits(0),
                InstantRoundingMode::Trunc,
            )
            .as_deref(),
            Some("1969-12-31T22:30:00-01:30")
        );

        let negative = BigInt::from(-65_261_246_399_500_000_000_i128);
        for (mode, expected) in [
            (InstantRoundingMode::Floor, "-000099-12-15T12:00:00Z"),
            (InstantRoundingMode::Trunc, "-000099-12-15T12:00:00Z"),
            (InstantRoundingMode::Ceil, "-000099-12-15T12:00:01Z"),
            (InstantRoundingMode::HalfExpand, "-000099-12-15T12:00:01Z"),
        ] {
            assert_eq!(
                format_instant(&negative, None, InstantPrecision::Digits(0), mode).as_deref(),
                Some(expected),
                "{mode:?}"
            );
        }
    }

    #[test]
    fn formats_plain_times_with_precision_and_midnight_rollover() {
        assert_eq!(
            format_plain_time(
                12,
                34,
                56,
                123,
                456,
                789,
                InstantPrecision::Auto,
                InstantRoundingMode::Trunc,
            )
            .as_deref(),
            Some("12:34:56.123456789")
        );
        assert_eq!(
            format_plain_time(
                12,
                34,
                56,
                123,
                456,
                789,
                InstantPrecision::Minute,
                InstantRoundingMode::Trunc,
            )
            .as_deref(),
            Some("12:34")
        );
        assert_eq!(
            format_plain_time(
                23,
                59,
                59,
                999,
                999,
                999,
                InstantPrecision::Digits(0),
                InstantRoundingMode::Ceil,
            )
            .as_deref(),
            Some("00:00:00")
        );
        assert_eq!(
            format_plain_time(
                12,
                34,
                56,
                123,
                500,
                0,
                InstantPrecision::Digits(3),
                InstantRoundingMode::HalfEven,
            )
            .as_deref(),
            Some("12:34:56.124")
        );
    }

    #[test]
    fn formats_durations_with_exact_subseconds_and_precision() {
        assert_eq!(
            format_duration([1, 2, 3, 4, 5, 6, 7, 987, 650, 0], InstantPrecision::Auto,).as_deref(),
            Some("P1Y2M3W4DT5H6M7.98765S")
        );
        assert_eq!(
            format_duration(
                [0, 0, 0, 0, 0, 0, 0, 9_007_199_254_740_991, 2_000, 0],
                InstantPrecision::Auto,
            )
            .as_deref(),
            Some("PT9007199254740.993S")
        );
        assert_eq!(
            format_duration([3, 0, 0, 0, 0, 0, 0, 0, 0, 0], InstantPrecision::Digits(5)).as_deref(),
            Some("P3YT0.00000S")
        );
        assert_eq!(
            format_duration([0; 10], InstantPrecision::Auto).as_deref(),
            Some("PT0S")
        );
    }

    #[test]
    fn rounds_signed_values_in_every_direction() {
        for (mode, positive, negative) in [
            (InstantRoundingMode::Ceil, 2, -1),
            (InstantRoundingMode::Expand, 2, -2),
            (InstantRoundingMode::Floor, 1, -2),
            (InstantRoundingMode::Trunc, 1, -1),
            (InstantRoundingMode::HalfCeil, 2, -1),
            (InstantRoundingMode::HalfFloor, 1, -2),
            (InstantRoundingMode::HalfExpand, 2, -2),
            (InstantRoundingMode::HalfTrunc, 1, -1),
            (InstantRoundingMode::HalfEven, 2, -2),
        ] {
            assert_eq!(round_signed_to_increment(15, 10, mode), Some(positive * 10));
            assert_eq!(
                round_signed_to_increment(-15, 10, mode),
                Some(negative * 10)
            );
        }
    }
}
