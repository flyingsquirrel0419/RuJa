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
    take(bytes, &mut index, b'-')?;
    let month = digits(bytes, &mut index, 2)?;
    take(bytes, &mut index, b'-')?;
    let day = digits(bytes, &mut index, 2)?;
    if day == 0 || day > days_in_month(year, month)? {
        return None;
    }
    if !matches!(bytes.get(index), Some(b'T') | Some(b't') | Some(b' ')) {
        return None;
    }
    index += 1;
    let hour = digits(bytes, &mut index, 2)?;
    take(bytes, &mut index, b':')?;
    let minute = digits(bytes, &mut index, 2)?;
    if hour > 23 || minute > 59 {
        return None;
    }

    let mut second = 0;
    let mut fraction = 0_i128;
    if bytes.get(index) == Some(&b':') {
        index += 1;
        second = digits(bytes, &mut index, 2)?;
        if second > 60 {
            return None;
        }
        second = second.min(59);
        if matches!(bytes.get(index), Some(b'.') | Some(b',')) {
            index += 1;
            let start = index;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
            let count = index - start;
            if !(1..=9).contains(&count) {
                return None;
            }
            fraction = bytes[start..index]
                .iter()
                .fold(0_i128, |value, byte| value * 10 + i128::from(byte - b'0'))
                * 10_i128.pow((9 - count) as u32);
        }
    }

    let offset_seconds = match bytes.get(index) {
        Some(b'Z') | Some(b'z') => {
            index += 1;
            0
        }
        Some(b'+') | Some(b'-') => {
            let negative = bytes[index] == b'-';
            index += 1;
            let offset_hour = digits(bytes, &mut index, 2)?;
            take(bytes, &mut index, b':')?;
            let offset_minute = digits(bytes, &mut index, 2)?;
            if offset_hour > 23 || offset_minute > 59 {
                return None;
            }
            let value = offset_hour * 3_600 + offset_minute * 60;
            if negative {
                -value
            } else {
                value
            }
        }
        _ => return None,
    };
    if index != bytes.len() {
        return None;
    }

    let days = days_from_civil(year, month, day)?;
    let local_seconds = days
        .checked_mul(SECONDS_PER_DAY)?
        .checked_add(hour * 3_600 + minute * 60 + second)?;
    let epoch = local_seconds
        .checked_sub(offset_seconds)?
        .checked_mul(NS_PER_SECOND)?
        .checked_add(fraction)?;
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
}
