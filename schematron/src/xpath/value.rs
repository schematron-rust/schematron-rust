//! XPath 1.0 values and the conversions between them.
//!
//! XPath 1.0 has exactly four types, and the conversions between them are
//! total and lossy in ways that matter. They are implemented here literally,
//! including the number-to-string format, which is the detail engines most
//! often get wrong.

use super::ast::Expr;
use super::context::Variables;
use super::temporal::{Duration, Temporal};
use crate::xml::{Document, NodeId};
use std::sync::Arc;

/// An XPath 1.0 value.
///
/// # Examples
///
/// ```
/// use schematron::xpath::Value;
///
/// assert_eq!(Value::Number(3.0, Default::default()).to_xpath_string_scalar(), "3");
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
    /// A number, an IEEE 754 double as XPath 1.0 requires, tagged with the
    /// XPath 2.0 numeric type it statically carries. See [`NumericType`].
    Number(f64, NumericType),
    /// A string.
    String(String),
    /// An XPath 2.0 sequence: an ordered list of nodes and atomic values.
    ///
    /// **Unreachable under XPath 1.0.** Nothing in the 1.0 grammar or
    /// function library constructs one, so a 1.0 expression evaluates through
    /// exactly the code it did before this variant existed, with exactly the
    /// same results. See `spec/xpath2/`.
    ///
    /// Sequences do not nest: building one from others flattens them.
    Sequence(Vec<Item>),
}

/// Which XPath 2.0 numeric type a [`Value::Number`]/[`Item::Number`]
/// statically carries.
///
/// This tags a number's declared type; it never changes how the `f64` next
/// to it is computed. Arithmetic (`+ - * div mod`), every function in the
/// XPath function library, and `for`'s `to` ranges always produce
/// [`NumericType::Double`] — exactly the single-double behaviour the crate
/// had before this type existed. Only three things carry a more specific tag:
/// a numeric literal as written (`1` is [`NumericType::Integer`], `1.5` is
/// [`NumericType::Decimal`] — XPath 2.0 assigns these types lexically, not
/// by value, so `1.0` is a decimal even though it equals the integer `1`),
/// an explicit `cast as`/`castable as`, and the equivalent atomic constructor
/// such as `xs:decimal()`.
///
/// `xs:integer` derives from `xs:decimal` by restriction in XML Schema, so
/// `instance of xs:decimal` also matches [`NumericType::Integer`].
/// `xs:float` and `xs:double` are separate primitive types with no
/// derivation between them or `xs:decimal`, so they match nothing but
/// themselves. See `spec/xpath2/` for the full account, including the
/// deliberate limit that only literals, casts, and constructors are tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum NumericType {
    /// `xs:integer` (and its further restrictions `xs:long`, `xs:int`,
    /// `xs:short`, `xs:byte`, which this crate does not distinguish from it).
    Integer,
    /// `xs:decimal`.
    Decimal,
    /// `xs:float`.
    Float,
    /// `xs:double` — the type every number had before this enum existed,
    /// and what every computed number still gets.
    #[default]
    Double,
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
    /// A number, tagged with the numeric type it statically carries. See
    /// [`NumericType`].
    Number(f64, NumericType),
    /// A boolean.
    Boolean(bool),
    /// An `xs:date`, `xs:dateTime`, or `xs:time`.
    Temporal(Temporal),
    /// An `xs:dayTimeDuration` or `xs:yearMonthDuration`.
    Duration(Duration),
    /// An XPath 3.0 function item. See [`FunctionItem`].
    ///
    /// **Unreachable under XPath 1.0 and 2.0.** Neither grammar has a
    /// construct that produces one, matching how [`Value::Sequence`] is
    /// unreachable under 1.0. See `spec/xpath3/`.
    Function(FunctionItem),
}

/// An XPath 3.0 function item: something a dynamic call (`$f(...)`) can
/// invoke.
///
/// A function item is not atomizable: it has no string or numeric value,
/// and comparing one — with `=`, `eq`, or any value comparison — is a type
/// error, the same way comparing two nodes of incompatible types would be.
/// [`Item::to_xpath_string`] and [`Item::to_number`] cannot themselves
/// raise that error, because they are used throughout the engine as total
/// functions; see `spec/xpath3/` for exactly which call sites (`string()`,
/// `number()`, and value/general comparisons) reject a function item
/// explicitly, and what the few that don't fall back to.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FunctionItem {
    /// `name#arity` — a reference to one of this crate's own built-in
    /// functions, resolved by name at call time rather than closed over.
    Named {
        /// The function's name, unprefixed.
        name: String,
        /// The arity written after `#`.
        arity: usize,
    },
    /// `function($a, $b) { … }` — a closure over an inline function
    /// expression.
    ///
    /// `captured` is the variable environment where the expression was
    /// written, cloned rather than borrowed so the closure can outlive the
    /// call that created it — passed to `for-each()`, bound to a variable,
    /// returned from another function. [`Variables`] holds only owned
    /// [`Value`]s, so cloning it is exactly as cheap or expensive as
    /// cloning the values already in scope, no more.
    Inline {
        /// The parameter names, in the order they are bound on each call.
        params: Vec<String>,
        /// The body, evaluated once per call. `Arc`, not `Box`, because
        /// cloning an `Item` — routine throughout this engine — must not
        /// deep-copy the AST every time; `Arc` rather than `Rc` because
        /// opt-in parallel pattern evaluation sends `Value`s across a
        /// thread scope, and `Rc` is not `Send`.
        body: Arc<Expr>,
        /// The lexical environment at the point the expression was
        /// written.
        captured: Variables,
    },
}

impl FunctionItem {
    /// The arity this function item accepts.
    #[must_use]
    pub fn arity(&self) -> usize {
        match self {
            FunctionItem::Named { arity, .. } => *arity,
            FunctionItem::Inline { params, .. } => params.len(),
        }
    }
}

impl Item {
    /// The item's string value.
    ///
    /// A function item has none — atomizing one is a type error in real
    /// XPath 3.0 (`err:FOTY0013`) — but this method cannot itself raise:
    /// it is called throughout the engine as a total conversion. The call
    /// sites that matter (`string()`, `concat()`, and the value/general
    /// comparisons, via [`Item::type_name`] feeding their existing
    /// type-mismatch error) reject a function item explicitly before
    /// reaching here; this fallback exists only for the few that don't,
    /// and an empty string is the least surprising thing to return rather
    /// than inventing one. See `spec/xpath3/`.
    #[must_use]
    pub fn to_xpath_string(&self, document: &Document) -> String {
        match self {
            Item::Node(node) => document.string_value(*node),
            Item::String(text) => text.clone(),
            Item::Number(number, _) => format_number(*number),
            Item::Boolean(boolean) => if *boolean { "true" } else { "false" }.to_string(),
            Item::Temporal(temporal) => temporal.to_lexical(),
            Item::Duration(duration) => duration.to_lexical(),
            Item::Function(_) => String::new(),
        }
    }

    /// The item's numeric value.
    #[must_use]
    pub fn to_number(&self, document: &Document) -> f64 {
        match self {
            Item::Node(node) => parse_number(&document.string_value(*node)),
            Item::String(text) => parse_number(text),
            Item::Number(number, _) => *number,
            Item::Boolean(boolean) => f64::from(u8::from(*boolean)),
            // Neither a date, a duration, nor a function item has a numeric
            // value; NaN keeps every numeric comparison against one false
            // rather than inventing an answer. See `to_xpath_string`'s doc
            // for why a function item can reach here at all.
            Item::Temporal(_) | Item::Duration(_) | Item::Function(_) => f64::NAN,
        }
    }

    /// The name of this item's type, for error messages.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Item::Node(_) => "node",
            Item::String(_) => "string",
            Item::Number(_, _) => "number",
            Item::Boolean(_) => "boolean",
            Item::Temporal(temporal) => temporal.kind().as_str(),
            Item::Duration(duration) => duration.kind().as_str(),
            Item::Function(_) => "function item",
        }
    }

    /// The duration, if this item is one.
    #[must_use]
    pub const fn as_duration(&self) -> Option<&Duration> {
        match self {
            Item::Duration(duration) => Some(duration),
            _ => None,
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
            // Preserves the tag rather than resetting it — an integer
            // literal folded into a sequence is still an integer.
            Value::Number(number, ty) => items.push(Item::Number(number, ty)),
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
            Value::Number(_, _) => "number",
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
            Value::Number(n, _) => *n != 0.0 && !n.is_nan(),
            Value::String(s) => !s.is_empty(),
            // A sequence uses XPath 2.0's effective boolean value, whose
            // "anything else is a type error" case cannot be expressed here;
            // `effective_boolean_value` reports it properly.
            Value::Sequence(items) => match items.as_slice() {
                [] => false,
                [Item::Boolean(boolean)] => *boolean,
                [Item::String(text)] => !text.is_empty(),
                [Item::Number(number, _)] => *number != 0.0 && !number.is_nan(),
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
            Value::Number(n, _) => format_number(*n),
            Value::String(s) => s.clone(),
            // The string value of a sequence is its first item's, matching
            // how a node-set behaves; XPath 2.0 makes a multi-item sequence a
            // type error here, which `spec/xpath2/` records as a divergence.
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
            Value::Number(n, _) => format_number(*n),
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
            Value::Number(n, _) => *n,
            Value::Boolean(b) => f64::from(u8::from(*b)),
            Value::String(s) => parse_number(s),
            Value::NodeSet(_) | Value::Sequence(_) => parse_number(&self.to_xpath_string(document)),
        }
    }

    /// Whether a function item appears anywhere in this value.
    ///
    /// A node-set, a boolean, a number and a string obviously hold none; a
    /// sequence might, since [`Item::Function`] is unreachable any other
    /// way. Used to reject a function item explicitly at the handful of
    /// atomization call sites that matter — `string()`, `concat()`, and
    /// general comparison — rather than letting it reach
    /// [`Item::to_xpath_string`]'s or [`Item::to_number`]'s silent
    /// fallback. See those methods' doc comments for the fuller account.
    #[must_use]
    pub fn contains_function_item(&self) -> bool {
        matches!(self, Value::Sequence(items) if items.iter().any(|item| matches!(item, Item::Function(_))))
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
    // Request exponential notation at the magnitudes where a formatter may
    // choose it implicitly. Expanding that known shape is stable across Rust
    // releases; relying on Display to make the choice made this path dormant
    // when its formatting policy changed.
    let shortest = if value.abs() >= 1e21 || value.abs() < 1e-6 {
        format!("{value:e}")
    } else {
        format!("{value}")
    };
    if !shortest.contains('e') && !shortest.contains('E') {
        return shortest;
    }
    expand_exponential(&shortest)
}

/// Rewrites a Rust exponential rendering into plain decimal notation.
///
/// Fallback: `input`, unchanged — an exponential rendering is still a correct
/// rendering of the same number, so a rendering this function cannot expand
/// (an unparseable exponent, or a width that does not fit) loses nothing.
fn expand_exponential(input: &str) -> String {
    expand_exponential_checked(input).unwrap_or_else(|| input.to_string())
}

/// The most zeros an expansion writes. A rendered `f64` has an exponent within
/// about ±324, so a larger one is not a rendering of an `f64`, and writing its
/// zeros would be an unbounded allocation (RCA-LEN-4).
const MAX_EXPANDED_ZEROS: u64 = 400;

/// [`expand_exponential`], or `None` when the rendering cannot be expanded.
fn expand_exponential_checked(input: &str) -> Option<String> {
    let (mantissa, exponent) = input.split_once(['e', 'E'])?;
    let exponent: i64 = exponent.parse().ok()?;

    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.trim_start_matches('-');
    let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits: String = format!("{int_part}{frac_part}");
    // Where the decimal point sits after shifting by the exponent.
    let point = i64::try_from(int_part.len())
        .ok()
        .and_then(|integer_digits| integer_digits.checked_add(exponent))?;

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if point <= 0 {
        let zeros = point.unsigned_abs();
        if zeros > MAX_EXPANDED_ZEROS {
            return None;
        }
        out.push_str("0.");
        for _ in 0..zeros {
            out.push('0');
        }
        out.push_str(digits.trim_end_matches('0'));
    } else {
        // A positive `i64` always fits a `usize` where that is 64 bits wide; a
        // narrower one cannot hold the number, so the rendering is left as it is.
        let expanded = usize::try_from(point)
            .ok()
            .and_then(|point| expand_after_point(&digits, point))?;
        out.push_str(&expanded);
    }
    Some(out)
}

/// Places the decimal point `point` digits in, for a point after the start.
///
/// `None` when the zeros it would need exceed [`MAX_EXPANDED_ZEROS`], and
/// when `point` falls inside a multi-byte character of `digits`.
fn expand_after_point(digits: &str, point: usize) -> Option<String> {
    let mut out = String::new();
    if let Some(zeros) = point.checked_sub(digits.len()) {
        // The point is at or beyond the last digit: the number is whole.
        if u64::try_from(zeros)
            .ok()
            .is_none_or(|zeros| zeros > MAX_EXPANDED_ZEROS)
        {
            return None;
        }
        out.push_str(digits);
        out.push_str(&"0".repeat(zeros));
    } else {
        let (whole, tail) = digits.split_at_checked(point)?;
        out.push_str(whole);
        let tail = tail.trim_end_matches('0');
        if !tail.is_empty() {
            out.push('.');
            out.push_str(tail);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xpath::temporal::{DurationKind, TemporalKind};

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
        assert!(Value::Number(1.0, NumericType::Double).to_boolean());
        assert!(!Value::Number(0.0, NumericType::Double).to_boolean());
        assert!(!Value::Number(f64::NAN, NumericType::Double).to_boolean());
        assert!(!Value::NodeSet(vec![]).to_boolean());
        assert!(Value::String("0".into()).to_boolean());
    }

    #[test]
    fn scalar_boolean_strings_cover_both_values() {
        assert_eq!(Value::Boolean(true).to_xpath_string_scalar(), "true");
        assert_eq!(Value::Boolean(false).to_xpath_string_scalar(), "false");
    }

    #[test]
    fn every_item_conversion_preserves_xpath_semantics() {
        let document = Document::from_str("<a n=' 12 '>34</a>").unwrap();
        let element = document.document_element().unwrap();
        let attribute = document.attributes(element)[0];
        let date = Temporal::parse("2024-02-29", TemporalKind::Date).unwrap();
        let duration = Duration::parse("PT1M30S", DurationKind::DayTime).unwrap();
        let function = FunctionItem::Named {
            name: "string-length".to_string(),
            arity: 1,
        };

        for (item, text, number, name) in [
            (Item::Node(attribute), " 12 ", 12.0, "node"),
            (Item::String(" 7 ".into()), " 7 ", 7.0, "string"),
            (
                Item::Number(2.5, NumericType::Decimal),
                "2.5",
                2.5,
                "number",
            ),
            (Item::Boolean(true), "true", 1.0, "boolean"),
        ] {
            assert_eq!(item.to_xpath_string(&document), text);
            assert_eq!(item.to_number(&document), number);
            assert_eq!(item.type_name(), name);
            assert!(item.as_temporal().is_none());
            assert!(item.as_duration().is_none());
        }

        let temporal = Item::Temporal(date);
        assert_eq!(temporal.to_xpath_string(&document), "2024-02-29");
        assert!(temporal.to_number(&document).is_nan());
        assert_eq!(temporal.type_name(), "xs:date");
        assert_eq!(temporal.as_temporal(), Some(&date));

        let duration_item = Item::Duration(duration);
        assert_eq!(duration_item.to_xpath_string(&document), "PT1M30S");
        assert!(duration_item.to_number(&document).is_nan());
        assert_eq!(duration_item.type_name(), "xs:dayTimeDuration");
        assert_eq!(duration_item.as_duration(), Some(&duration));

        let function_item = Item::Function(function.clone());
        assert_eq!(function.arity(), 1);
        assert_eq!(function_item.to_xpath_string(&document), "");
        assert!(function_item.to_number(&document).is_nan());
        assert_eq!(function_item.type_name(), "function item");
        assert_eq!(Item::Boolean(false).to_xpath_string(&document), "false");
    }

    #[test]
    fn values_convert_and_flatten_without_losing_types_or_order() {
        let document = Document::from_str("<a>19</a>").unwrap();
        let element = document.document_element().unwrap();
        let function = FunctionItem::Named {
            name: "true".to_string(),
            arity: 0,
        };
        let values = vec![
            Value::NodeSet(vec![element]),
            Value::Boolean(false),
            Value::Number(3.5, NumericType::Float),
            Value::String("x".into()),
            Value::Sequence(vec![Item::Function(function.clone())]),
        ];
        let items = flatten_into_sequence(values);
        assert_eq!(items.len(), 5);
        assert!(matches!(items[0], Item::Node(node) if node == element));
        assert!(matches!(items[1], Item::Boolean(false)));
        assert!(matches!(items[2], Item::Number(3.5, NumericType::Float)));
        assert!(matches!(&items[3], Item::String(text) if text == "x"));
        assert!(matches!(&items[4], Item::Function(item) if item == &function));

        for (value, name, text, number) in [
            (Value::NodeSet(vec![element]), "node-set", "19", 19.0),
            (Value::Boolean(true), "boolean", "true", 1.0),
            (
                Value::Number(19.0, NumericType::Integer),
                "number",
                "19",
                19.0,
            ),
            (Value::String("19".into()), "string", "19", 19.0),
            (
                Value::Sequence(vec![Item::String("19".into())]),
                "sequence",
                "19",
                19.0,
            ),
        ] {
            assert_eq!(value.type_name(), name);
            assert_eq!(value.to_xpath_string(&document), text);
            assert_eq!(value.to_number(&document), number);
        }

        assert_eq!(Value::Boolean(false).to_xpath_string_scalar(), "false");
        assert_eq!(Value::Boolean(false).to_xpath_string(&document), "false");
        assert_eq!(
            Value::Number(2.0, NumericType::Double).to_xpath_string_scalar(),
            "2"
        );
        assert_eq!(Value::String("s".into()).to_xpath_string_scalar(), "s");
        assert_eq!(Value::NodeSet(vec![element]).to_xpath_string_scalar(), "");
        assert_eq!(Value::Sequence(vec![]).to_xpath_string_scalar(), "");
        assert_eq!(Value::Boolean(true).to_number(&document), 1.0);
        assert_eq!(Value::Boolean(false).to_number(&document), 0.0);
    }

    #[test]
    fn sequence_helpers_cover_empty_singleton_node_and_atomic_cases() {
        let document = Document::from_str("<a/>").unwrap();
        let element = document.document_element().unwrap();
        assert!(!Value::Sequence(vec![]).to_boolean());
        assert!(!Value::Sequence(vec![Item::String(String::new())]).to_boolean());
        assert!(Value::Sequence(vec![Item::String("x".into())]).to_boolean());
        assert!(!Value::Sequence(vec![Item::Number(f64::NAN, NumericType::Double)]).to_boolean());
        assert!(Value::Sequence(vec![Item::Node(element)]).to_boolean());
        assert!(Value::Sequence(vec![Item::Boolean(true)])
            .effective_boolean_value()
            .unwrap());
        assert!(Value::Sequence(vec![
            Item::Number(1.0, NumericType::Double),
            Item::Boolean(true)
        ])
        .effective_boolean_value()
        .unwrap_err()
        .contains("2 atomic items"));

        let sequence = Value::Sequence(vec![Item::String("x".into())]);
        assert_eq!(sequence.as_sequence().unwrap().len(), 1);
        assert!(sequence.as_node_set().is_none());
        assert!(!sequence.contains_function_item());
        assert_eq!(
            sequence.clone().into_items(),
            sequence.as_sequence().unwrap()
        );

        let nodes = Value::NodeSet(vec![element]);
        assert_eq!(nodes.as_node_set(), Some(&[element][..]));
        assert!(nodes.as_sequence().is_none());
        assert_eq!(nodes.into_items(), vec![Item::Node(element)]);

        let function = Value::Sequence(vec![Item::Function(FunctionItem::Named {
            name: "true".into(),
            arity: 0,
        })]);
        assert!(function.contains_function_item());
        assert!(!Value::String("true".into()).contains_function_item());
        assert!(Value::NodeSet(vec![element]).to_boolean_scalar());
        assert_eq!(Value::NodeSet(vec![]).to_xpath_string(&document), "");
    }

    #[test]
    fn formatting_expands_large_small_and_negative_exponents() {
        assert_eq!(format_number(-1e21), "-1000000000000000000000");
        assert_eq!(format_number(1.25e-7), "0.000000125");
        assert_eq!(format_number(-1.25e-7), "-0.000000125");
        assert_eq!(format_number(1.234e22), "12340000000000000000000");
    }

    #[test]
    fn exponential_expansion_covers_each_decimal_point_position() {
        // Rust's current Display implementation already chooses plain decimal
        // notation for finite f64 values. Exercise the normalizer directly so
        // a future formatter change cannot expose an untested conversion path.
        assert_eq!(expand_exponential("1e3"), "1000");
        assert_eq!(expand_exponential("12.5e-1"), "1.25");
        assert_eq!(expand_exponential("-1.25e-7"), "-0.000000125");
        assert_eq!(expand_exponential("1.20e1"), "12");
        assert_eq!(expand_exponential("1E2"), "100");
        // Fallback: the input unchanged, at the first exponent that cannot be expanded.
        assert_eq!(
            expand_exponential("1e9223372036854775807"),
            "1e9223372036854775807"
        );
        assert_eq!(
            expand_exponential("1e9223372036854775808"),
            "1e9223372036854775808"
        );
        assert_eq!(expand_exponential("1ex"), "1ex");
        assert_eq!(
            expand_exponential("1e-9223372036854775808"),
            "1e-9223372036854775808"
        );
        // The most zeros written is 400; one more keeps the exponential form.
        assert_eq!(expand_exponential("1e400").len(), 401);
        assert_eq!(expand_exponential("1e401"), "1e401");
        assert_eq!(expand_exponential("1e-401").len(), 403);
        assert_eq!(expand_exponential("1e-402"), "1e-402");
    }

    #[test]
    fn exponential_expansion_refuses_a_rendering_it_cannot_place() {
        // No exponent marker at all.
        assert_eq!(expand_exponential_checked("12"), None);
        // A point that lands inside a multi-byte character is not a decimal rendering.
        assert_eq!(expand_exponential_checked("\u{e9}1e-2"), None);
        // More than 400 zeros after the last digit.
        assert_eq!(expand_exponential_checked("1e500"), None);
        assert_eq!(
            expand_exponential_checked("1e400").map(|text| text.len()),
            Some(401)
        );
    }
}
