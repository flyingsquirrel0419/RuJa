use num_bigint::BigInt;

const NS_PER_SECOND: i128 = 1_000_000_000;
const SECONDS_PER_DAY: i128 = 86_400;

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

fn parse_annotations(bytes: &[u8], index: &mut usize) -> Option<()> {
    let mut time_zone_seen = false;
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
        }
    }
    Some(())
}

fn leap_year(year: i128) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i128, month: i128) -> Option<i128> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => return None,
    })
}

fn days_from_civil(year: i128, month: i128, day: i128) -> Option<i128> {
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

pub(crate) fn parse_instant_string(source: &str) -> Option<BigInt> {
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
    if !matches!(bytes.get(index), Some(b'T') | Some(b't') | Some(b' ')) {
        return None;
    }
    index += 1;
    let hour = digits(bytes, &mut index, 2)?;
    let extended_time = bytes.get(index) == Some(&b':');
    let mut minute = 0;
    let mut second = 0;
    let mut fraction = 0_i128;

    let minute_present = if extended_time {
        index += 1;
        minute = digits(bytes, &mut index, 2)?;
        true
    } else if bytes.get(index).is_some_and(u8::is_ascii_digit) {
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

    let offset_nanoseconds = match bytes.get(index) {
        Some(b'Z') | Some(b'z') => {
            index += 1;
            0
        }
        Some(b'+') | Some(b'-') => parse_offset_nanoseconds(bytes, &mut index)?,
        _ => return None,
    };
    parse_annotations(bytes, &mut index)?;
    if index != bytes.len() {
        return None;
    }

    let days = days_from_civil(year, month, day)?;
    let local_seconds = days
        .checked_mul(SECONDS_PER_DAY)?
        .checked_add(hour * 3_600 + minute * 60 + second)?;
    let epoch = local_seconds
        .checked_mul(NS_PER_SECOND)?
        .checked_add(fraction)?
        .checked_sub(offset_nanoseconds)?;
    Some(BigInt::from(epoch))
}

#[cfg(test)]
mod tests {
    use super::parse_instant_string;
    use num_bigint::BigInt;

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
}
