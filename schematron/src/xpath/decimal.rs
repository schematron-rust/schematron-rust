//! Exact `xs:integer` and `xs:decimal` arithmetic for XPath 2.0 and later.
//!
//! An exact number is held as a [`Decimal`] in its [`NumericType`], beside the
//! nearest `f64`, which the rest of the engine keeps reading unchanged. Exact
//! values exist only under an XPath 2.0 or later binding: under XPath 1.0 every
//! literal is evaluated as an `xs:double` (see `eval.rs`), so nothing here is
//! reachable from a 1.0 expression. See `spec/xpath2/`.

use std::cmp::Ordering;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use super::ast::BinaryOp;
use super::eval::EvalError;
use super::value::{Item, NumericType, Value};

/// An exact number and whether it is an `xs:integer` rather than an `xs:decimal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Exact {
    pub(crate) value: Decimal,
    pub(crate) integer: bool,
}

impl Exact {
    /// The exact number a numeric type carries, if it is one of the exact types.
    pub(crate) const fn of(numeric_type: NumericType) -> Option<Self> {
        match numeric_type {
            NumericType::Integer(value) => Some(Self {
                value,
                integer: true,
            }),
            NumericType::Decimal(value) => Some(Self {
                value,
                integer: false,
            }),
            NumericType::Float | NumericType::Double => None,
        }
    }

    /// The exact number a value holds, when it is a single exact number.
    pub(crate) fn of_value(value: &Value) -> Option<Self> {
        match value {
            Value::Number(_, numeric_type) => Self::of(*numeric_type),
            Value::Sequence(items) => match items.as_slice() {
                [item] => Self::of_item(item),
                _ => None,
            },
            _ => None,
        }
    }

    /// The exact number an item holds, when it is one.
    pub(crate) const fn of_item(item: &Item) -> Option<Self> {
        match item {
            Item::Number(_, numeric_type) => Self::of(*numeric_type),
            _ => None,
        }
    }

    /// The numeric type that carries this exact number.
    pub(crate) const fn numeric_type(self) -> NumericType {
        if self.integer {
            NumericType::Integer(self.value)
        } else {
            NumericType::Decimal(self.value)
        }
    }

    /// This exact number as a value, with its nearest double beside it.
    pub(crate) fn into_value(self) -> Value {
        Value::Number(to_f64(self.value), self.numeric_type())
    }

    /// This exact number as a sequence item, with its nearest double beside it.
    pub(crate) fn into_item(self) -> Item {
        Item::Number(to_f64(self.value), self.numeric_type())
    }
}

/// The nearest `f64` to an exact number.
///
/// Fallback: `NaN` — `rust_decimal` converts every value it can hold, whose
/// magnitude is at most about 7.9 × 10^28, far inside the `f64` range, so no
/// value reaches it; were one to, an unknown number must not pass for a known one.
pub(crate) fn to_f64(value: Decimal) -> f64 {
    value.to_f64().unwrap_or(f64::NAN)
}

/// Parses the XML Schema `xs:decimal` lexical form: an optional sign, digits
/// with at most one `.`, and at least one digit. No exponent and no
/// underscores, which `Decimal::from_str` would otherwise accept.
///
/// `None` for anything else, and for a magnitude too large to hold. A longer
/// fractional part is rounded to the 28 places the representation keeps.
pub(crate) fn parse_lexical(text: &str) -> Option<Decimal> {
    let text = text.trim_matches(crate::xml::is_xml_space);
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (integral, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let digits_only = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if integral.is_empty() && fraction.is_empty()
        || !digits_only(integral)
        || !digits_only(fraction)
    {
        return None;
    }
    // `Decimal` parses a leading `+` and a bare `.5` or `5.` itself.
    text.parse::<Decimal>().ok()
}

/// The canonical string of an exact number: no exponent, no trailing
/// fractional zeros, and no `.` when it is integral.
pub(crate) fn format(value: Decimal) -> String {
    if value.is_zero() {
        // Covers `-0` and `0.000` alike; XPath has no negative zero decimal.
        return "0".to_string();
    }
    value.normalize().to_string()
}

/// `a op b` for two exact operands, with XPath 2.0's numeric promotion.
///
/// Integer `+ - * mod` integer stays `xs:integer`; `div` is always
/// `xs:decimal`; anything involving a decimal is a decimal. Division or `mod`
/// by zero is `FOAR0001`, and a result too large to hold is `FOAR0002`.
pub(crate) fn arithmetic(op: BinaryOp, a: Exact, b: Exact) -> Result<Exact, EvalError> {
    let overflow = || {
        EvalError::new(format!(
            "FOAR0002: {} {} {} is outside the range of xs:decimal (about ±7.9e28)",
            format(a.value),
            op.as_str(),
            format(b.value)
        ))
    };
    let both_integers = a.integer && b.integer;
    let (value, integer) = match op {
        BinaryOp::Add => (a.value.checked_add(b.value), both_integers),
        BinaryOp::Subtract => (a.value.checked_sub(b.value), both_integers),
        BinaryOp::Multiply => (a.value.checked_mul(b.value), both_integers),
        BinaryOp::Divide | BinaryOp::Modulo if b.value.is_zero() => {
            return Err(EvalError::new(format!(
                "FOAR0001: {} {} 0 divides an exact number by zero",
                format(a.value),
                op.as_str()
            )));
        }
        BinaryOp::Divide => (a.value.checked_div(b.value), false),
        _ => (a.value.checked_rem(b.value), both_integers),
    };
    value
        .map(|value| Exact { value, integer })
        .ok_or_else(overflow)
}

/// The exact negation, keeping the operand's type.
pub(crate) fn negate(a: Exact) -> Exact {
    Exact {
        value: -a.value,
        integer: a.integer,
    }
}

/// The exact ordering of two values, when both are single exact numbers.
pub(crate) fn compare(a: &Value, b: &Value) -> Option<Ordering> {
    Some(Exact::of_value(a)?.value.cmp(&Exact::of_value(b)?.value))
}

/// The exact numbers of `items`, when every one of them is exact.
pub(crate) fn all_exact(items: &[Item]) -> Option<Vec<Exact>> {
    items.iter().map(Exact::of_item).collect()
}

/// The exact sum of exact numbers: an integer when every addend is one.
///
/// `None` when the sum overflows, which the caller reports as `FOAR0002`.
pub(crate) fn sum(numbers: &[Exact]) -> Option<Exact> {
    numbers.iter().try_fold(
        Exact {
            value: Decimal::ZERO,
            integer: true,
        },
        |total, next| {
            Some(Exact {
                value: total.value.checked_add(next.value)?,
                integer: total.integer && next.integer,
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(text: &str, integer: bool) -> Exact {
        Exact {
            value: parse_lexical(text).unwrap(),
            integer,
        }
    }

    #[test]
    fn the_lexical_form_is_xml_schemas() {
        for good in ["1", "-1", "+1.5", ".5", "5.", "0012.3400", " 7 "] {
            assert!(parse_lexical(good).is_some(), "{good}");
        }
        for bad in [
            "", ".", "+", "1e3", "1_000", "1.2.3", "--1", "1 2", "NaN", "INF",
        ] {
            assert!(parse_lexical(bad).is_none(), "{bad}");
        }
        // Too large to hold.
        assert!(parse_lexical("100000000000000000000000000000").is_none());
    }

    #[test]
    fn canonical_strings_have_no_trailing_zeros_or_exponent() {
        assert_eq!(format(parse_lexical("1.50").unwrap()), "1.5");
        assert_eq!(format(parse_lexical("2.000").unwrap()), "2");
        assert_eq!(format(parse_lexical("-0.0").unwrap()), "0");
        assert_eq!(format(parse_lexical("0.0000001").unwrap()), "0.0000001");
    }

    #[test]
    fn promotion_and_errors_follow_xpath_two() {
        let one = exact("1", true);
        let two = exact("2", true);
        let half = exact("0.5", false);
        assert_eq!(
            arithmetic(BinaryOp::Add, one, two).unwrap(),
            exact("3", true)
        );
        assert_eq!(
            arithmetic(BinaryOp::Multiply, two, half).unwrap(),
            exact("1", false)
        );
        assert_eq!(
            arithmetic(BinaryOp::Divide, two, one).unwrap(),
            exact("2", false)
        );
        assert_eq!(
            arithmetic(BinaryOp::Modulo, exact("7", true), two).unwrap(),
            exact("1", true)
        );
        assert_eq!(
            arithmetic(BinaryOp::Subtract, half, one).unwrap(),
            exact("-0.5", false)
        );
        assert!(arithmetic(BinaryOp::Divide, one, exact("0", true))
            .unwrap_err()
            .message
            .contains("FOAR0001"));
        assert!(arithmetic(BinaryOp::Modulo, one, exact("0.0", false)).is_err());
        let huge = Exact {
            value: Decimal::MAX,
            integer: true,
        };
        assert!(arithmetic(BinaryOp::Add, huge, one)
            .unwrap_err()
            .message
            .contains("FOAR0002"));
        assert_eq!(negate(half), exact("-0.5", false));
    }

    #[test]
    fn sums_stay_integers_only_when_every_addend_is_one() {
        assert_eq!(sum(&[]), Some(exact("0", true)));
        assert_eq!(
            sum(&[exact("1", true), exact("2", true)]),
            Some(exact("3", true))
        );
        assert_eq!(
            sum(&[exact("1", true), exact("0.5", false)]),
            Some(exact("1.5", false))
        );
        let huge = Exact {
            value: Decimal::MAX,
            integer: true,
        };
        assert_eq!(sum(&[huge, huge]), None);
    }
}
