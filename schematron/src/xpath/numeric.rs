//! Checked numeric conversions for the XPath engine (RCA-CONV-1 to RCA-CONV-4).
//!
//! XPath has one numeric type, the IEEE 754 double, while the engine counts in
//! `usize` and `i64`. Every conversion between them goes through here, so a value
//! that does not convert exactly is `None` at the point of conversion instead of
//! a silently truncated, saturated or rounded number.

/// 2^53: every integer of at most this magnitude is exactly an `f64`.
const MAX_EXACT_F64: u64 = 1 << 53;

/// 2^32, as an `f64`, to join the two halves of a 53-bit integer.
const TWO_POW_32: f64 = 4_294_967_296.0;

/// 2^63 is exactly representable as an `f64`; `i64::MAX` is not.
const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;

/// An unsigned integer as an `f64`, or `None` above 2^53 where it would round.
pub(crate) fn u64_to_f64(value: u64) -> Option<f64> {
    if value > MAX_EXACT_F64 {
        return None;
    }
    // Both halves fit `u32`, so `f64::from` is lossless and the sum is exact below 2^53.
    let high = u32::try_from(value >> 32).ok()?;
    let low = u32::try_from(value & u64::from(u32::MAX)).ok()?;
    Some(f64::from(high) * TWO_POW_32 + f64::from(low))
}

/// A length or count as an `f64`, or `None` above 2^53.
pub(crate) fn usize_to_f64(value: usize) -> Option<f64> {
    u64_to_f64(u64::try_from(value).ok()?)
}

/// A signed integer as an `f64`, or `None` when its magnitude exceeds 2^53.
pub(crate) fn i64_to_f64(value: i64) -> Option<f64> {
    let magnitude = u64_to_f64(value.unsigned_abs())?;
    Some(if value < 0 { -magnitude } else { magnitude })
}

/// An integral `f64` as an `i64`, or `None` for `NaN`, infinities, a fractional
/// part, or a value outside the `i64` range. Round or truncate first when the
/// caller wants that.
pub(crate) fn f64_to_i64(value: f64) -> Option<i64> {
    if !(value.is_finite() && value.fract() == 0.0 && (-TWO_POW_63..TWO_POW_63).contains(&value)) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "RCA-CONV-3: finite, integral and within [-2^63, 2^63), checked above"
    )]
    let converted = value as i64;
    Some(converted)
}

/// An integral `f64` as a `u64`; see [`f64_to_i64`].
pub(crate) fn f64_to_u64(value: f64) -> Option<u64> {
    u64::try_from(f64_to_i64(value)?).ok()
}

/// An integral `f64` as a `usize`; see [`f64_to_i64`].
pub(crate) fn f64_to_usize(value: f64) -> Option<usize> {
    usize::try_from(f64_to_i64(value)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_become_floats_up_to_2_pow_53() {
        assert_eq!(u64_to_f64(0), Some(0.0));
        assert_eq!(u64_to_f64(MAX_EXACT_F64), Some(9_007_199_254_740_992.0));
        // First failing input: 2^53 + 1 would round to 2^53.
        assert_eq!(u64_to_f64(MAX_EXACT_F64 + 1), None);
        assert_eq!(u64_to_f64(u64::MAX), None);
        assert_eq!(usize_to_f64(7), Some(7.0));
        assert_eq!(i64_to_f64(-5), Some(-5.0));
        assert_eq!(
            i64_to_f64(-9_007_199_254_740_992),
            Some(-9_007_199_254_740_992.0)
        );
        assert_eq!(i64_to_f64(-9_007_199_254_740_993), None);
        assert_eq!(i64_to_f64(i64::MIN), None);
        assert_eq!(i64_to_f64(i64::MAX), None);
    }

    #[test]
    fn floats_become_integers_only_when_exact_and_in_range() {
        assert_eq!(f64_to_i64(0.0), Some(0));
        assert_eq!(f64_to_i64(-0.0), Some(0));
        assert_eq!(f64_to_i64(-9_223_372_036_854_775_808.0), Some(i64::MIN));
        // First failing inputs: 2^63, NaN, infinities, a fraction.
        assert_eq!(f64_to_i64(9_223_372_036_854_775_808.0), None);
        assert_eq!(f64_to_i64(f64::NAN), None);
        assert_eq!(f64_to_i64(f64::INFINITY), None);
        assert_eq!(f64_to_i64(f64::NEG_INFINITY), None);
        assert_eq!(f64_to_i64(0.5), None);
        assert_eq!(f64_to_u64(-1.0), None);
        assert_eq!(f64_to_u64(3.0), Some(3));
        assert_eq!(f64_to_usize(-1.0), None);
        assert_eq!(f64_to_usize(4.0), Some(4));
    }
}
