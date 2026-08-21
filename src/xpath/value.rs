//! XPath 1.0 values and the conversions between them.
//!
//! XPath 1.0 has exactly four types, and the conversions between them are
//! total and lossy in ways that matter. They are implemented here literally,
//! including the number-to-string format, which is the detail engines most
//! often get wrong.

use super::temporal::Temporal;
use crate::xml::{Document, NodeId};

/// An XPath 1.0 value.
///
/// # Examples
///
/// ```
/// use schematron::xpath::Value;
///
/// assert_eq!(Value::Number(3.0).to_xpath_string_scalar(), "3");
/// assert!(Value::String("x".into()).to_boolean_scalar());
/// assert!(!Value::String(String::new()).to_boolean_scalar());
/// ```
#[derive(Debug, Clone, PartialEq)]
// Variants will be added: XPath 2.0 phase 2b adds the date and time types.
// Marking it non-exhaustive now means that will not be a breaking change.
#[non_exhaustive]
pub enum Value {
    /// A set of nodes, kept sorted by document order and deduplicated.
    NodeSet(Vec<NodeId>),
    /// A boolean.
    Boolean(bool),
    /// A number, an IEEE 754 double as XPath 1.0 requires.
    Number(f64),
    /// A string.
    String(String),
    /// An XPath 2.0 sequence: an ordered list of nodes and atomic values.
    ///
    /// **Unreachable under XPath 1.0.** Nothing in the 1.0 grammar or
    /// function library constructs one, so a 1.0 expression evaluates through
    /// exactly the code it did before this variant existed, with exactly the
    /// same results. See `spec/xpath2.md`.
    ///
    /// Sequences do not nest: building one from others flattens them.
    Sequence(Vec<Item>),
}

/// One member of an XPath 2.0 [`Value::Sequence`].
///
/// A node or an atomic value — never a sequence, because sequences do not
/// nest.
#[derive(Debug, Clone, PartialEq)]
// Variants will be added: XPath 2.0 phase 2b adds the date and time types.
// Marking it non-exhaustive now means that will not be a breaking change.
#[non_exhaustive]
pub enum Item {
    /// A node.
    Node(NodeId),
    /// A string.
    String(String),
    /// A number.
    Number(f64),
    /// A boolean.
    Boolean(bool),
    /// An `xs:date`, `xs:dateTime`, or `xs:time`.
    Temporal(Temporal),
}

impl Item {
    /// The item's string value.
    #[must_use]
    pub fn to_xpath_string(&self, document: &Document) -> String {
        match self {
            Item::Node(node) => document.string_value(*node),
            Item::String(text) => text.clone(),
            Item::Number(number) => format_number(*number),
            Item::Boolean(boolean) => if *boolean { "true" } else { "false" }.to_string(),
            Item::Temporal(temporal) => temporal.to_lexical(),
        }
    }

    /// The item's numeric value.
    #[must_use]
    pub fn to_number(&self, document: &Document) -> f64 {
        match self {
            Item::Node(node) => parse_number(&document.string_value(*node)),
            Item::String(text) => parse_number(text),
            Item::Number(number) => *number,
            Item::Boolean(boolean) => f64::from(u8::from(*boolean)),
            // A date has no numeric value in XPath 2.0; NaN keeps every
            // numeric comparison against it false rather than inventing one.
            Item::Temporal(_) => f64::NAN,
        }
    }

    /// The name of this item's type, for error messages.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Item::Node(_) => "node",
            Item::String(_) => "string",
            Item::Number(_) => "number",
            Item::Boolean(_) => "boolean",
            Item::Temporal(temporal) => temporal.kind().as_str(),
        }
    }

    /// The temporal value, if this item is one.
    #[must_use]
    pub const fn as_temporal(&self) -> Option<&Temporal> {
        match self {
            Item::Temporal(temporal) => Some(temporal),
            _ => None,
        }
    }
}

/// Builds a sequence from values, flattening any that are themselves
/// sequences or node-sets.
///
/// This is what makes `(a, (b, c))` and `(a, b, c)` the same sequence, which
/// XPath 2.0 requires.
#[must_use]
pub fn flatten_into_sequence(values: Vec<Value>) -> Vec<Item> {
    let mut items = Vec::new();
    for value in values {
        match value {
            Value::Sequence(inner) => items.extend(inner),
            Value::NodeSet(nodes) => items.extend(nodes.into_iter().map(Item::Node)),
            Value::Boolean(boolean) => items.push(Item::Boolean(boolean)),
            Value::Number(number) => items.push(Item::Number(number)),
            Value::String(text) => items.push(Item::String(text)),
        }
    }
    items
}

impl Value {
    /// The name of this value's type, for error messages.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Value::NodeSet(_) => "node-set",
            Value::Boolean(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Sequence(_) => "sequence",
        }
    }

    /// Converts to boolean per XPath 1.0.
    ///
    /// A node-set is true when it is non-empty; a number when it is neither
    /// zero nor NaN; a string when it is non-empty.
    #[must_use]
    pub fn to_boolean(&self) -> bool {
        match self {
            Value::NodeSet(nodes) => !nodes.is_empty(),
            Value::Boolean(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::String(s) => !s.is_empty(),
            // A sequence uses XPath 2.0's effective boolean value, whose
            // "anything else is a type error" case cannot be expressed here;
            // `effective_boolean_value` reports it properly.
            Value::Sequence(items) => match items.as_slice() {
                [] => false,
                [Item::Boolean(boolean)] => *boolean,
                [Item::String(text)] => !text.is_empty(),
                [Item::Number(number)] => *number != 0.0 && !number.is_nan(),
                // A sequence starting with a node is true, and so is a lone
                // date. So is any other multi-item sequence here, but that
                // case is a type error rather than a value — see
                // `effective_boolean_value`.
                _ => true,
            },
        }
    }

    /// Converts to boolean, reporting XPath 2.0's type error.
    ///
    /// The effective boolean value of a sequence of two or more atomic items
    /// is a **type error** in XPath 2.0, not `true`. Raising it means
    /// `if (1, 2) then …` fails rather than quietly taking a branch.
    ///
    /// # Errors
    ///
    /// Returns a message naming the offending sequence.
    pub fn effective_boolean_value(&self) -> Result<bool, String> {
        if let Value::Sequence(items) = self {
            match items.as_slice() {
                [] | [Item::Node(_), ..] | [_] => {}
                _ => {
                    return Err(format!(
                        "a sequence of {} atomic items has no effective boolean value; \
                         XPath 2.0 makes this a type error",
                        items.len()
                    ))
                }
            }
        }
        Ok(self.to_boolean())
    }

    /// Converts to boolean without needing a document.
    ///
    /// Identical to [`Value::to_boolean`]; provided so that documentation
    /// examples can convert a scalar without constructing a tree.
    #[must_use]
    pub fn to_boolean_scalar(&self) -> bool {
        self.to_boolean()
    }

    /// Converts to string per XPath 1.0.
    ///
    /// A node-set becomes the string value of its first node in document
    /// order, or the empty string when it is empty.
    #[must_use]
    pub fn to_xpath_string(&self, document: &Document) -> String {
        match self {
            Value::NodeSet(nodes) => nodes
                .first()
                .map_or_else(String::new, |&n| document.string_value(n)),
            Value::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
            Value::Number(n) => format_number(*n),
            Value::String(s) => s.clone(),
            // The string value of a sequence is its first item's, matching
            // how a node-set behaves; XPath 2.0 makes a multi-item sequence a
            // type error here, which `spec/xpath2.md` records as a divergence.
            Value::Sequence(items) => items
                .first()
                .map_or_else(String::new, |item| item.to_xpath_string(document)),
        }
    }

    /// Converts a non-node-set to string.
    ///
    /// Returns the empty string for a node-set, which has no string value
    /// without a document to look nodes up in.
    #[must_use]
    pub fn to_xpath_string_scalar(&self) -> String {
        match self {
            Value::NodeSet(_) | Value::Sequence(_) => String::new(),
            Value::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
            Value::Number(n) => format_number(*n),
            Value::String(s) => s.clone(),
        }
    }

    /// Converts to number per XPath 1.0.
    ///
    /// Anything that is not a valid XPath number becomes NaN, rather than
    /// being an error, which is why a typo in a numeric comparison silently
    /// yields false instead of failing.
    #[must_use]
    pub fn to_number(&self, document: &Document) -> f64 {
        match self {
            Value::Number(n) => *n,
            Value::Boolean(b) => f64::from(u8::from(*b)),
            Value::String(s) => parse_number(s),
            Value::NodeSet(_) | Value::Sequence(_) => {
                parse_number(&self.to_xpath_string(document))
            }
        }
    }

    /// The sequence's items, or `None` for the other types.
    #[must_use]
    pub fn as_sequence(&self) -> Option<&[Item]> {
        match self {
            Value::Sequence(items) => Some(items),
            _ => None,
        }
    }

    /// The value as a sequence of items, converting a node-set or scalar.
    ///
    /// This is what lets `for`, `some`, `every`, and the sequence functions
    /// accept a node-set as readily as a sequence.
    #[must_use]
    pub fn into_items(self) -> Vec<Item> {
        flatten_into_sequence(vec![self])
    }

    /// Returns the node-set, or `None` for the other three types.
    #[must_use]
    pub fn as_node_set(&self) -> Option<&[NodeId]> {
        match self {
            Value::NodeSet(nodes) => Some(nodes),
            _ => None,
        }
    }
}

/// Parses a string as an XPath number.
///
/// XPath 1.0 accepts optional leading and trailing whitespace, an optional
/// sign, and a decimal number with no exponent. Rust's `f64::from_str` accepts
/// more than that — `1e5`, `inf`, `NaN`, `0x1p3` — so the input is screened
/// before it is handed over.
#[must_use]
pub fn parse_number(input: &str) -> f64 {
    let trimmed = input.trim_matches(|c: char| c.is_ascii_whitespace());
    if trimmed.is_empty() {
        return f64::NAN;
    }
    let body = trimmed.strip_prefix('-').unwrap_or(trimmed);
    let valid = !body.is_empty()
        && body.chars().all(|c| c.is_ascii_digit() || c == '.')
        && body.matches('.').count() <= 1
        && body != ".";
    if !valid {
        return f64::NAN;
    }
    trimmed.parse::<f64>().unwrap_or(f64::NAN)
}

/// Formats a number as XPath 1.0's `string()` function does.
///
/// The rules that differ from Rust's own formatting: NaN is `NaN`, infinities
/// are `Infinity` and `-Infinity`, negative zero is `0`, integers carry no
/// decimal point, and exponential notation is never used — so `1e21` prints
/// as twenty-two characters, not as `1e21`.
///
/// # Examples
///
/// ```
/// use schematron::xpath::format_number;
///
/// assert_eq!(format_number(3.0), "3");
/// assert_eq!(format_number(-0.0), "0");
/// assert_eq!(format_number(0.5), "0.5");
/// assert_eq!(format_number(f64::NAN), "NaN");
/// assert_eq!(format_number(1e21), "1000000000000000000000");
/// ```
#[must_use]
pub fn format_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if value == 0.0 {
        // Covers both zeroes; XPath has no negative zero in its lexical space.
        return "0".to_string();
    }
    if value.fract() == 0.0 && value.abs() < 1e21 {
        // Integral and small enough that no rounding is involved.
        return format!("{value:.0}");
    }

    // `{}` gives the shortest representation that round-trips, which is what
    // XPath wants, but it switches to exponential notation for extreme
    // magnitudes, which XPath forbids.
    let shortest = format!("{value}");
    if !shortest.contains('e') && !shortest.contains('E') {
        return shortest;
    }
    expand_exponential(&shortest)
}

/// Rewrites a Rust exponential rendering into plain decimal notation.
fn expand_exponential(input: &str) -> String {
    let (mantissa, exponent) = input
        .split_once(['e', 'E'])
        .expect("caller checked that an exponent is present");
    let exponent: i32 = exponent.parse().unwrap_or(0);

    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.trim_start_matches('-');
    let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits: String = format!("{int_part}{frac_part}");
    // Where the decimal point sits after shifting by the exponent.
    let point = i32::try_from(int_part.len()).unwrap_or(i32::MAX) + exponent;

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if point <= 0 {
        out.push_str("0.");
        for _ in 0..-point {
            out.push('0');
        }
        out.push_str(digits.trim_end_matches('0'));
    } else {
        let point = usize::try_from(point).unwrap_or(0);
        if point >= digits.len() {
            out.push_str(&digits);
            for _ in 0..point - digits.len() {
                out.push('0');
            }
        } else {
            out.push_str(&digits[..point]);
            let tail = digits[point..].trim_end_matches('0');
            if !tail.is_empty() {
                out.push('.');
                out.push_str(tail);
            }
        }
    }
    if out.is_empty() || out == "-" {
        out.push('0');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_integers_without_a_point() {
        assert_eq!(format_number(3.0), "3");
        assert_eq!(format_number(-3.0), "-3");
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "0");
    }

    #[test]
    fn format_fractions_shortest_round_trip() {
        assert_eq!(format_number(0.5), "0.5");
        assert_eq!(format_number(-1.25), "-1.25");
        assert_eq!(format_number(1.0 / 3.0), "0.3333333333333333");
    }

    #[test]
    fn format_specials() {
        assert_eq!(format_number(f64::NAN), "NaN");
        assert_eq!(format_number(f64::INFINITY), "Infinity");
        assert_eq!(format_number(f64::NEG_INFINITY), "-Infinity");
    }

    #[test]
    fn format_never_uses_exponential_notation() {
        assert_eq!(format_number(1e21), "1000000000000000000000");
        assert_eq!(format_number(1e-7), "0.0000001");
        assert!(!format_number(1e300).contains('e'));
    }

    #[test]
    fn parse_rejects_what_xpath_rejects() {
        assert!(parse_number("1e5").is_nan());
        assert!(parse_number("inf").is_nan());
        assert!(parse_number("NaN").is_nan());
        assert!(parse_number("0x10").is_nan());
        assert!(parse_number("").is_nan());
        assert!(parse_number("+1").is_nan());
    }

    #[test]
    fn parse_accepts_what_xpath_accepts() {
        assert_eq!(parse_number(" 42 "), 42.0);
        assert_eq!(parse_number("-1.5"), -1.5);
        assert_eq!(parse_number(".5"), 0.5);
        assert_eq!(parse_number("5."), 5.0);
    }

    #[test]
    fn boolean_conversion() {
        assert!(Value::Number(1.0).to_boolean());
        assert!(!Value::Number(0.0).to_boolean());
        assert!(!Value::Number(f64::NAN).to_boolean());
        assert!(!Value::NodeSet(vec![]).to_boolean());
        assert!(Value::String("0".into()).to_boolean());
    }
}
