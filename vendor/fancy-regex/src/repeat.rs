use alloc::string::ToString;
use alloc::sync::Arc;
use core::cmp::Ordering;
use core::convert::TryFrom;

/// An exact finite regular-expression repetition count.
///
/// Values that fit in `u128` avoid allocation. Larger values retain their
/// canonical decimal spelling, so parsing and ordering do not depend on the
/// host pointer width.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepeatCount(RepeatCountRepr);

#[derive(Clone, Debug, Eq, PartialEq)]
enum RepeatCountRepr {
    Small(u128),
    Big(Arc<str>),
}

impl RepeatCount {
    /// Parses an unsigned decimal repetition count exactly.
    ///
    /// Leading zeroes are removed. Values larger than `u128::MAX` retain a
    /// canonical decimal representation instead of being truncated.
    pub fn from_decimal(digits: &str) -> Option<Self> {
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let canonical = digits.trim_start_matches('0');
        let canonical = if canonical.is_empty() { "0" } else { canonical };
        Some(Self(match canonical.parse::<u128>() {
            Ok(value) => RepeatCountRepr::Small(value),
            Err(_) => RepeatCountRepr::Big(Arc::from(canonical)),
        }))
    }

    pub(crate) fn parse(digits: &str) -> Option<Self> {
        Self::from_decimal(digits)
    }

    pub(crate) fn zero() -> Self {
        Self(RepeatCountRepr::Small(0))
    }

    pub(crate) fn one() -> Self {
        Self(RepeatCountRepr::Small(1))
    }

    pub(crate) fn is_zero(&self) -> bool {
        matches!(self.0, RepeatCountRepr::Small(0))
    }

    pub(crate) fn is_one(&self) -> bool {
        matches!(self.0, RepeatCountRepr::Small(1))
    }

    pub(crate) fn greater_than_one(&self) -> bool {
        !matches!(self.0, RepeatCountRepr::Small(0 | 1))
    }

    pub(crate) fn to_usize(&self) -> Option<usize> {
        match self.0 {
            RepeatCountRepr::Small(value) => usize::try_from(value).ok(),
            RepeatCountRepr::Big(_) => None,
        }
    }

    pub(crate) fn saturating_mul_usize(&self, factor: usize) -> usize {
        if factor == 0 {
            return 0;
        }
        self.to_usize()
            .and_then(|count| factor.checked_mul(count))
            .unwrap_or(usize::MAX)
    }

    pub(crate) fn write_decimal(&self, output: &mut alloc::string::String) {
        match &self.0 {
            RepeatCountRepr::Small(value) => output.push_str(&value.to_string()),
            RepeatCountRepr::Big(value) => output.push_str(value),
        }
    }
}

impl From<usize> for RepeatCount {
    fn from(value: usize) -> Self {
        Self(RepeatCountRepr::Small(value as u128))
    }
}

impl Ord for RepeatCount {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.0, &other.0) {
            (RepeatCountRepr::Small(left), RepeatCountRepr::Small(right)) => left.cmp(right),
            (RepeatCountRepr::Small(_), RepeatCountRepr::Big(_)) => Ordering::Less,
            (RepeatCountRepr::Big(_), RepeatCountRepr::Small(_)) => Ordering::Greater,
            (RepeatCountRepr::Big(left), RepeatCountRepr::Big(right)) => left
                .len()
                .cmp(&right.len())
                .then_with(|| left.as_bytes().cmp(right.as_bytes())),
        }
    }
}

impl PartialOrd for RepeatCount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A finite regular-expression repetition count or positive infinity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepeatBound {
    /// A finite upper bound.
    Finite(RepeatCount),
    /// An unbounded upper range.
    Infinity,
}

impl RepeatBound {
    pub(crate) fn finite(value: usize) -> Self {
        Self::Finite(value.into())
    }

    pub(crate) fn as_finite(&self) -> Option<&RepeatCount> {
        match self {
            Self::Finite(value) => Some(value),
            Self::Infinity => None,
        }
    }

    pub(crate) fn is_infinite(&self) -> bool {
        matches!(self, Self::Infinity)
    }
}

#[cfg(test)]
mod tests {
    use super::{RepeatBound, RepeatCount, RepeatCountRepr};
    use alloc::string::ToString;

    fn parse(value: &str) -> RepeatCount {
        RepeatCount::parse(value).unwrap()
    }

    #[test]
    fn parses_counts_without_host_width_limits() {
        assert_eq!(parse("0000000000"), RepeatCount(RepeatCountRepr::Small(0)));
        assert_eq!(
            parse("4294967295"),
            RepeatCount(RepeatCountRepr::Small(u32::MAX.into()))
        );
        assert_eq!(
            parse("4294967296"),
            RepeatCount(RepeatCountRepr::Small(1u128 << 32))
        );
        assert_eq!(
            parse("9007199254740991"),
            RepeatCount(RepeatCountRepr::Small((1u128 << 53) - 1))
        );
        assert_eq!(
            parse(&u128::MAX.to_string()),
            RepeatCount(RepeatCountRepr::Small(u128::MAX))
        );

        let above_u128 = "340282366920938463463374607431768211456";
        assert!(
            matches!(parse(above_u128).0, RepeatCountRepr::Big(value) if &*value == above_u128)
        );
        let thousand_digits = "9".repeat(1000);
        assert!(
            matches!(parse(&thousand_digits).0, RepeatCountRepr::Big(value) if &*value == thousand_digits)
        );
    }

    #[test]
    fn orders_canonical_decimal_counts_exactly() {
        let u128_max = parse(&u128::MAX.to_string());
        let above_u128 = parse("340282366920938463463374607431768211456");
        let larger_same_width = parse("340282366920938463463374607431768211457");
        let much_larger = parse(&"1".repeat(1000));

        assert!(u128_max < above_u128);
        assert!(above_u128 < larger_same_width);
        assert!(larger_same_width < much_larger);
    }

    #[test]
    fn host_reachability_and_analysis_are_saturating() {
        assert_eq!(parse("2").saturating_mul_usize(3), 6);
        assert_eq!(
            parse("18446744073709551616").saturating_mul_usize(1),
            usize::MAX
        );
        assert_eq!(parse("2").saturating_mul_usize(usize::MAX), usize::MAX);
        assert_eq!(
            parse("999999999999999999999999999999999999999").to_usize(),
            None
        );
        assert!(RepeatBound::Infinity.is_infinite());
        assert!(
            !RepeatBound::Finite(parse("999999999999999999999999999999999999999")).is_infinite()
        );
    }
}
