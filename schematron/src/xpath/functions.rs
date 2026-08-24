//! The XPath 1.0 core function library.
//!
//! All twenty-seven functions of XPath 1.0 section 4, plus `current()` from
//! the XSLT library, which Schematron's `xslt` query binding makes available
//! and which schemas genuinely use inside predicates.
//!
//! Arity is checked when the schema is compiled, not when a document is
//! validated, so `contains(@x)` is an error the moment the schema loads.

use super::context::EvalContext;
use super::eval::EvalError;
use super::temporal::{from_unix_seconds, Duration, DurationKind, Temporal, TemporalKind};
use super::value::{parse_number, Item, Value};
use super::version::XPathVersion;
use crate::xml::{Document, NodeId, NodeKind};

/// The name and permitted argument count of every supported function.
///
/// `None` as the maximum means unbounded, which only `concat` uses.
const SIGNATURES: &[(&str, usize, Option<usize>)] = &[
    ("last", 0, Some(0)),
    ("position", 0, Some(0)),
    ("count", 1, Some(1)),
    ("id", 1, Some(1)),
    ("local-name", 0, Some(1)),
    ("namespace-uri", 0, Some(1)),
    ("name", 0, Some(1)),
    ("string", 0, Some(1)),
    ("concat", 2, None),
    ("starts-with", 2, Some(2)),
    ("contains", 2, Some(2)),
    ("substring-before", 2, Some(2)),
    ("substring-after", 2, Some(2)),
    ("substring", 2, Some(3)),
    ("string-length", 0, Some(1)),
    ("normalize-space", 0, Some(1)),
    ("translate", 3, Some(3)),
    ("boolean", 1, Some(1)),
    ("not", 1, Some(1)),
    ("true", 0, Some(0)),
    ("false", 0, Some(0)),
    ("lang", 1, Some(1)),
    ("number", 0, Some(1)),
    ("sum", 1, Some(1)),
    ("floor", 1, Some(1)),
    ("ceiling", 1, Some(1)),
    ("round", 1, Some(1)),
    ("current", 0, Some(0)),
    ("document", 1, Some(2)),
    ("key", 2, Some(2)),
];

/// The XPath 2.0 functions this crate implements, with their arities.
///
/// Available only under an XPath 2.0 query binding. The set is deliberately
/// the one that needs no sequence type; see `spec/xpath2/`.
const SIGNATURES_V2: &[(&str, usize, Option<usize>)] = &[
    ("matches", 2, Some(3)),
    ("replace", 3, Some(4)),
    ("upper-case", 1, Some(1)),
    ("lower-case", 1, Some(1)),
    ("ends-with", 2, Some(2)),
    ("abs", 1, Some(1)),
    ("min", 1, Some(1)),
    ("max", 1, Some(1)),
    ("avg", 1, Some(1)),
    ("exists", 1, Some(1)),
    ("empty", 1, Some(1)),
    ("string-join", 2, Some(2)),
    ("tokenize", 2, Some(3)),
    ("distinct-values", 1, Some(1)),
    ("index-of", 2, Some(2)),
    ("current-date", 0, Some(0)),
    ("current-dateTime", 0, Some(0)),
    ("current-time", 0, Some(0)),
    ("year-from-date", 1, Some(1)),
    ("month-from-date", 1, Some(1)),
    ("day-from-date", 1, Some(1)),
    ("year-from-dateTime", 1, Some(1)),
    ("month-from-dateTime", 1, Some(1)),
    ("day-from-dateTime", 1, Some(1)),
    ("hours-from-dateTime", 1, Some(1)),
    ("minutes-from-dateTime", 1, Some(1)),
    ("seconds-from-dateTime", 1, Some(1)),
    ("hours-from-time", 1, Some(1)),
    ("minutes-from-time", 1, Some(1)),
    ("seconds-from-time", 1, Some(1)),
    ("xs:date", 1, Some(1)),
    ("xs:dateTime", 1, Some(1)),
    ("xs:time", 1, Some(1)),
    ("xs:dayTimeDuration", 1, Some(1)),
    ("xs:yearMonthDuration", 1, Some(1)),
    ("days-from-duration", 1, Some(1)),
    ("hours-from-duration", 1, Some(1)),
    ("minutes-from-duration", 1, Some(1)),
    ("seconds-from-duration", 1, Some(1)),
    ("years-from-duration", 1, Some(1)),
    ("months-from-duration", 1, Some(1)),
    ("timezone-from-date", 1, Some(1)),
    ("timezone-from-dateTime", 1, Some(1)),
    ("timezone-from-time", 1, Some(1)),
    ("implicit-timezone", 0, Some(0)),
];

/// XPath 2.0 functions this crate does **not** implement, and why.
///
/// Naming them turns "unknown function" into a message that says what is
/// actually wrong, and what it would take to support them.
const V2_FUNCTIONS_NEEDING_SEQUENCES: &[&str] = &[
    "subsequence",
    "insert-before",
    "remove",
    "reverse",
    "unordered",
    "for-each",
];

/// XPath 2.0 functions this crate does not implement because they need the
/// date and time types.
const V2_FUNCTIONS_NEEDING_DATES: &[&str] = &[
    "duration",
    "adjust-date-to-timezone",
    "adjust-dateTime-to-timezone",
    "adjust-time-to-timezone",
];

/// Other XPath 2.0 functions that are simply not implemented yet.
const V2_FUNCTIONS_NOT_IMPLEMENTED: &[&str] = &["data", "deep-equal", "trace", "resolve-uri"];

/// Checks that a function exists and accepts this many arguments.
///
/// Called while compiling a schema, so that a typo or an XPath 2.0 function
/// is reported against the schema rather than against a document.
///
/// # Errors
///
/// Returns a message naming the function and, where it can, why it is absent.
///
/// # Examples
///
/// ```
/// use schematron::xpath::{check_function, XPathVersion};
///
/// assert!(check_function("contains", 2, XPathVersion::V1).is_ok());
/// assert!(check_function("contains", 1, XPathVersion::V1).is_err());
///
/// // An XPath 2.0 function needs an XPath 2.0 query binding.
/// assert!(check_function("matches", 2, XPathVersion::V1).is_err());
/// assert!(check_function("matches", 2, XPathVersion::V2).is_ok());
/// ```
pub fn check_function(name: &str, arity: usize, version: XPathVersion) -> Result<(), String> {
    let tables: &[&[(&str, usize, Option<usize>)]] = if version.is_v2() {
        &[SIGNATURES, SIGNATURES_V2]
    } else {
        &[SIGNATURES]
    };

    for (function, minimum, maximum) in tables.iter().copied().flatten() {
        if *function != name {
            continue;
        }
        if arity < *minimum {
            return Err(format!(
                "{name}() takes at least {minimum} argument(s), but {arity} were given"
            ));
        }
        if let Some(maximum) = maximum {
            if arity > *maximum {
                return Err(format!(
                    "{name}() takes at most {maximum} argument(s), but {arity} were given"
                ));
            }
        }
        return Ok(());
    }

    // A 2.0 function used under a 1.0 binding: say which binding it needs.
    if !version.is_v2() && SIGNATURES_V2.iter().any(|(f, _, _)| *f == name) {
        return Err(format!(
            "{name}() is an XPath 2.0 function, and this schema's query binding is \
             XPath 1.0. Set queryBinding=\"xslt2\" to use it; see spec/xpath2/."
        ));
    }
    if V2_FUNCTIONS_NEEDING_SEQUENCES.contains(&name) {
        return Err(format!(
            "{name}() returns or consumes an XPath 2.0 sequence, which this crate \
             does not implement yet; see spec/xpath2/"
        ));
    }
    if V2_FUNCTIONS_NEEDING_DATES.contains(&name) {
        return Err(format!(
            "{name}() needs the XPath 2.0 date and time types, which this crate \
             does not implement yet; see spec/xpath2/"
        ));
    }
    if V2_FUNCTIONS_NOT_IMPLEMENTED.contains(&name) {
        return Err(format!(
            "{name}() is an XPath 2.0 function this crate does not implement; \
             see spec/xpath2/"
        ));
    }
    Err(format!("unknown function {name}()"))
}

/// The names of every function this crate implements, sorted.
///
/// Useful for building a "did you mean" list, and for documentation tests
/// that assert the library has not silently shrunk.
#[must_use]
pub fn function_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = SIGNATURES.iter().map(|(name, _, _)| *name).collect();
    names.sort_unstable();
    names
}

/// Checks a regular expression literal and its flags, without a document.
///
/// Called while compiling a schema, so that a malformed pattern written into
/// the schema fails when the schema loads rather than part-way through
/// validating somebody's document. A pattern computed at runtime cannot be
/// checked here and is validated when it is evaluated.
///
/// # Errors
///
/// Returns a message naming the pattern and what is wrong with it.
///
/// # Examples
///
/// ```
/// use schematron::xpath::check_regex;
///
/// assert!(check_regex("^[0-9]+$", None).is_ok());
/// assert!(check_regex("^[0-9]+$", Some("i")).is_ok());
/// assert!(check_regex("[unclosed", None).is_err());
/// assert!(check_regex(".", Some("q")).is_err());
/// ```
pub fn check_regex(pattern: &str, flags: Option<&str>) -> Result<(), String> {
    build_regex(pattern, flags.unwrap_or_default()).map(|_| ())
}

/// The names of the XPath 2.0 functions this crate implements, sorted.
#[must_use]
pub fn function_names_v2() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = SIGNATURES_V2.iter().map(|(name, _, _)| *name).collect();
    names.sort_unstable();
    names
}

/// Calls a function with already-evaluated arguments.
///
/// Re-checks arity here rather than trusting that [`check_function`] ran.
/// Schema compilation does call it, but [`evaluate`] is public and can be
/// handed an expression that never went through a schema — and the arms below
/// index into `args` directly, so an unchecked call would panic rather than
/// return an error.
///
/// [`evaluate`]: super::evaluate
pub(crate) fn call(
    name: &str,
    args: &[Value],
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    check_function(name, args.len(), context.version).map_err(EvalError::new)?;
    let document = context.document;
    match name {
        "last" => Ok(Value::Number(as_f64(context.size))),
        "position" => Ok(Value::Number(as_f64(context.position))),
        "count" => Ok(Value::Number(as_f64(items_of(name, args, 0, context.version)?.len()))),
        "id" => Ok(Value::NodeSet(id_function(args, context))),

        "local-name" => Ok(Value::String(
            node_argument(name, args, context)?
                .and_then(|n| document.name(n).map(|q| q.local.clone()))
                .unwrap_or_default(),
        )),
        "namespace-uri" => Ok(Value::String(
            node_argument(name, args, context)?
                .and_then(|n| document.name(n).and_then(|q| q.uri.clone()))
                .unwrap_or_default(),
        )),
        "name" => Ok(Value::String(
            node_argument(name, args, context)?
                .and_then(|n| document.name(n).map(crate::xml::QName::display_name))
                .unwrap_or_default(),
        )),

        "string" => Ok(Value::String(match args.first() {
            Some(value) => value.to_xpath_string(document),
            None => document.string_value(context.node),
        })),
        "concat" => Ok(Value::String(
            args.iter()
                .map(|a| a.to_xpath_string(document))
                .collect::<String>(),
        )),
        "starts-with" => {
            let (haystack, needle) = two_strings(args, document);
            Ok(Value::Boolean(haystack.starts_with(&needle)))
        }
        "contains" => {
            let (haystack, needle) = two_strings(args, document);
            Ok(Value::Boolean(haystack.contains(&needle)))
        }
        "substring-before" => {
            let (haystack, needle) = two_strings(args, document);
            Ok(Value::String(
                haystack
                    .find(&needle)
                    .map(|i| haystack[..i].to_string())
                    .unwrap_or_default(),
            ))
        }
        "substring-after" => {
            let (haystack, needle) = two_strings(args, document);
            Ok(Value::String(
                haystack
                    .find(&needle)
                    .map(|i| haystack[i + needle.len()..].to_string())
                    .unwrap_or_default(),
            ))
        }
        "substring" => Ok(Value::String(substring(args, document))),
        "string-length" => Ok(Value::Number(as_f64(
            string_argument(args, context).chars().count(),
        ))),
        "normalize-space" => Ok(Value::String(normalize_space(&string_argument(
            args, context,
        )))),
        "translate" => Ok(Value::String(translate(
            &args[0].to_xpath_string(document),
            &args[1].to_xpath_string(document),
            &args[2].to_xpath_string(document),
        ))),

        "boolean" => Ok(Value::Boolean(args[0].to_boolean())),
        "not" => Ok(Value::Boolean(!args[0].to_boolean())),
        "true" => Ok(Value::Boolean(true)),
        "false" => Ok(Value::Boolean(false)),
        "lang" => Ok(Value::Boolean(lang(
            &args[0].to_xpath_string(document),
            document,
            context.node,
        ))),

        "number" => Ok(Value::Number(match args.first() {
            Some(value) => value.to_number(document),
            None => parse_number(&document.string_value(context.node)),
        })),
        // Folded from `0.0` rather than `.sum()`: Rust's `Sum` for `f64` starts
        // from `-0.0`, which is the true additive identity — `-0.0 + x == x`
        // for every `x`, including `-0.0` itself, which `0.0` does not manage.
        // Correct for Rust, wrong here: `sum()` over an empty node-set must be
        // positive zero, as every other XPath processor gives. The sign is
        // invisible until something divides by it, and then `1 div sum(none)`
        // is -Infinity instead of Infinity.
        "sum" => {
            let items = items_of(name, args, 0, context.version)?;
            Ok(Value::Number(
                items.iter().map(|item| item.to_number(document)).fold(0.0, |a, b| a + b),
            ))
        }
        "floor" => Ok(Value::Number(args[0].to_number(document).floor())),
        "ceiling" => Ok(Value::Number(args[0].to_number(document).ceil())),
        "round" => Ok(Value::Number(round_half_up(args[0].to_number(document)))),

        "current" => Ok(Value::NodeSet(vec![context.current])),

        "document" => document_function(args, context),
        "key" => key_function(args, context),

        // XPath 2.0, phase 1. Reachable only under a 2.0 query binding,
        // because `check_function` above gates on the version.
        _ if context.version.is_v2() => call_v2(name, args, context),

        _ => Err(EvalError::new(format!("unknown function {name}()"))),
    }
}

/// The XPath 2.0 additions, split out so each dispatch stays readable and
/// mirrors its own signature table.
fn call_v2(name: &str, args: &[Value], context: &EvalContext<'_>) -> Result<Value, EvalError> {
    let document = context.document;
    match name {        "matches" => {
            let regex = compile_regex(&args[1].to_xpath_string(document), args.get(2), document)?;
            Ok(Value::Boolean(
                regex.is_match(&args[0].to_xpath_string(document)),
            ))
        }
        "replace" => {
            let regex = compile_regex(&args[1].to_xpath_string(document), args.get(3), document)?;
            let input = args[0].to_xpath_string(document);
            let replacement = translate_replacement(&args[2].to_xpath_string(document));
            Ok(Value::String(
                regex.replace_all(&input, replacement.as_str()).into_owned(),
            ))
        }
        "upper-case" => Ok(Value::String(args[0].to_xpath_string(document).to_uppercase())),
        "lower-case" => Ok(Value::String(args[0].to_xpath_string(document).to_lowercase())),
        "ends-with" => {
            let (haystack, needle) = two_strings(args, document);
            Ok(Value::Boolean(haystack.ends_with(&needle)))
        }
        "abs" => Ok(Value::Number(args[0].to_number(document).abs())),
        "min" => Ok(Value::Number(extreme(args, document, f64::min))),
        "max" => Ok(Value::Number(extreme(args, document, f64::max))),
        "avg" => {
            let numbers = numbers_of(args, document);
            if numbers.is_empty() {
                // XPath 2.0's avg() of an empty sequence is the empty
                // sequence; without sequences, NaN is the honest analogue and
                // behaves the same way in every comparison.
                return Ok(Value::Number(f64::NAN));
            }
            // Folded from `0.0` for the same reason as `sum()` above.
            let total: f64 = numbers.iter().fold(0.0, |a, b| a + b);
            #[allow(clippy::cast_precision_loss)]
            let count = numbers.len() as f64;
            Ok(Value::Number(total / count))
        }
        "exists" => Ok(Value::Boolean(!items_of(name, args, 0, context.version)?.is_empty())),
        "empty" => Ok(Value::Boolean(items_of(name, args, 0, context.version)?.is_empty())),
        "string-join" => {
            let items = items_of(name, args, 0, context.version)?;
            let separator = args[1].to_xpath_string(document);
            let parts: Vec<String> = items
                .iter()
                .map(|item| item.to_xpath_string(document))
                .collect();
            Ok(Value::String(parts.join(&separator)))
        }

        "tokenize" => {
            let regex = compile_regex(&args[1].to_xpath_string(document), args.get(2), document)?;
            let input = args[0].to_xpath_string(document);
            Ok(Value::Sequence(
                regex
                    .split(&input)
                    .map(|part| Item::String(part.to_string()))
                    .collect(),
            ))
        }

        "distinct-values" => {
            let items = items_of(name, args, 0, context.version)?;
            // Compared by string value, and first-seen order is kept, which
            // is what makes the result predictable. XPath 2.0 leaves the
            // order implementation-defined.
            let mut seen: Vec<String> = Vec::new();
            let mut out = Vec::new();
            for item in items {
                let key = item.to_xpath_string(document);
                if seen.iter().any(|existing| existing == &key) {
                    continue;
                }
                seen.push(key.clone());
                out.push(Item::String(key));
            }
            Ok(Value::Sequence(out))
        }

        _ => call_v2_temporal(name, args, context),
    }
}

/// The XPath 2.0 date, time and duration functions, split out so that each
/// dispatch stays readable.
fn call_v2_temporal(
    name: &str,
    args: &[Value],
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let document = context.document;
    match name {
        "current-date" => clock(TemporalKind::Date, context),
        "current-dateTime" => clock(TemporalKind::DateTime, context),
        "current-time" => clock(TemporalKind::Time, context),

        "xs:date" => cast_temporal(args, TemporalKind::Date, document),
        "xs:dateTime" => cast_temporal(args, TemporalKind::DateTime, document),
        "xs:time" => cast_temporal(args, TemporalKind::Time, document),

        "xs:dayTimeDuration" => cast_duration(args, DurationKind::DayTime, document),
        "xs:yearMonthDuration" => cast_duration(args, DurationKind::YearMonth, document),

        _ if name.ends_with("-from-duration") => duration_component(name, args, document),

        "implicit-timezone" => Ok(Value::Sequence(vec![Item::Duration(
            Duration::from_seconds(f64::from(context.implicit_timezone) * 60.0),
        )])),

        _ if name.starts_with("timezone-from-") => timezone_of(name, args, document),

        _ if name.ends_with("-from-date")
            || name.ends_with("-from-dateTime")
            || name.ends_with("-from-time") =>
        {
            component_of(name, args, document)
        }

        _ => call_v2_rest(name, args, context),
    }
}

/// The remaining XPath 2.0 functions.
fn call_v2_rest(name: &str, args: &[Value], context: &EvalContext<'_>) -> Result<Value, EvalError> {
    let document = context.document;
    match name {
        "index-of" => {
            let items = items_of(name, args, 0, context.version)?;
            let wanted = args[1].to_xpath_string(document);
            let positions = items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.to_xpath_string(document) == wanted)
                // XPath positions are one-based.
                .map(|(index, _)| Item::Number(as_f64(index + 1)))
                .collect();
            Ok(Value::Sequence(positions))
        }

        _ => Err(EvalError::new(format!("unknown function {name}()"))),
    }
}

/// Converts a count to `f64` for XPath, which has only one numeric type.
///
/// A document large enough to lose precision here would need more than 2^53
/// nodes, which is far beyond what fits in memory.
#[allow(clippy::cast_precision_loss)]
fn as_f64(value: usize) -> f64 {
    value as f64
}

/// The items of an argument that may be a node-set or an XPath 2.0 sequence.
///
/// Under XPath 2.0 every value *is* a sequence, so a scalar counts as a
/// one-item one and `exists(1)` is true. Under XPath 1.0 a sequence is
/// unreachable and a scalar is an error, exactly as the node-set accessor
/// this replaced behaved — so the 1.0 functions are unaffected.
fn items_of(
    name: &str,
    args: &[Value],
    index: usize,
    version: XPathVersion,
) -> Result<Vec<Item>, EvalError> {
    match args.get(index) {
        Some(Value::NodeSet(nodes)) => Ok(nodes.iter().copied().map(Item::Node).collect()),
        Some(Value::Sequence(items)) => Ok(items.clone()),
        Some(other) if version.is_v2() => Ok(match other {
            Value::String(text) => vec![Item::String(text.clone())],
            Value::Number(number) => vec![Item::Number(*number)],
            Value::Boolean(boolean) => vec![Item::Boolean(*boolean)],
            Value::NodeSet(_) | Value::Sequence(_) => unreachable!("matched above"),
        }),
        _ => Err(EvalError::new(format!(
            "{name}() requires a node-set argument"
        ))),
    }
}

/// The node an optional-node-set function operates on: the first node of the
/// argument, or the context node when there is no argument.
fn node_argument(
    name: &str,
    args: &[Value],
    context: &EvalContext<'_>,
) -> Result<Option<NodeId>, EvalError> {
    match args.first() {
        None => Ok(Some(context.node)),
        Some(Value::NodeSet(nodes)) => Ok(nodes.first().copied()),
        Some(other) => Err(EvalError::new(format!(
            "{name}() requires a node-set argument, but was given a {}",
            other.type_name()
        ))),
    }
}

fn string_argument(args: &[Value], context: &EvalContext<'_>) -> String {
    match args.first() {
        Some(value) => value.to_xpath_string(context.document),
        None => context.document.string_value(context.node),
    }
}

fn two_strings(args: &[Value], document: &Document) -> (String, String) {
    (
        args[0].to_xpath_string(document),
        args[1].to_xpath_string(document),
    )
}

/// `current-date()` and its companions.
///
/// The instant comes from the run, not from the clock, so every call in a
/// validation agrees — which XPath 2.0 requires. With no instant supplied the
/// function is an error rather than an arbitrary time, so a caller evaluating
/// XPath directly cannot get a silently non-reproducible answer.
fn clock(kind: TemporalKind, context: &EvalContext<'_>) -> Result<Value, EvalError> {
    let Some(seconds) = context.current_time else {
        return Err(EvalError::new(
            "current-date() and its companions need the run's instant, which \
             the validator supplies and a direct call to evaluate() does not; \
             see EvalContext::with_current_time",
        ));
    };
    Ok(Value::Sequence(vec![Item::Temporal(from_unix_seconds(
        kind, seconds,
    ))]))
}

/// `xs:date()`, `xs:dateTime()`, `xs:time()`.
fn cast_temporal(
    args: &[Value],
    kind: TemporalKind,
    document: &Document,
) -> Result<Value, EvalError> {
    // An argument that is already this type passes through unchanged.
    if let Some([Item::Temporal(temporal)]) = args[0].as_sequence() {
        if temporal.kind() == kind {
            return Ok(args[0].clone());
        }
    }
    let text = args[0].to_xpath_string(document);
    let parsed = Temporal::parse(&text, kind).map_err(EvalError::new)?;
    Ok(Value::Sequence(vec![Item::Temporal(parsed)]))
}

/// `xs:dayTimeDuration()` and `xs:yearMonthDuration()`.
fn cast_duration(
    args: &[Value],
    kind: DurationKind,
    document: &Document,
) -> Result<Value, EvalError> {
    if let Some([Item::Duration(duration)]) = args[0].as_sequence() {
        if duration.kind() == kind {
            return Ok(args[0].clone());
        }
    }
    let text = args[0].to_xpath_string(document);
    let parsed = Duration::parse(&text, kind).map_err(EvalError::new)?;
    Ok(Value::Sequence(vec![Item::Duration(parsed)]))
}

/// The `*-from-duration` accessors.
///
/// XPath 2.0 defines each as the component it would occupy in the normalised
/// lexical form, so the days of `P1DT36H` are two, not one.
fn duration_component(
    name: &str,
    args: &[Value],
    document: &Document,
) -> Result<Value, EvalError> {
    let component = name
        .strip_suffix("-from-duration")
        .ok_or_else(|| EvalError::new(format!("unknown function {name}()")))?;

    // Years and months belong to one subtype, the rest to the other.
    let kind = match component {
        "years" | "months" => DurationKind::YearMonth,
        _ => DurationKind::DayTime,
    };
    let duration = match args[0].as_sequence().and_then(|items| match items {
        [Item::Duration(duration)] => Some(*duration),
        _ => None,
    }) {
        Some(duration) => duration,
        None => Duration::parse(&args[0].to_xpath_string(document), kind)
            .map_err(EvalError::new)?,
    };

    if duration.kind() != kind {
        return Err(EvalError::new(format!(
            "{name}() takes a {}, not a {}",
            kind.as_str(),
            duration.kind().as_str()
        )));
    }

    // Each component is signed the same way as the duration as a whole.
    let sign = if duration.is_negative() { -1.0 } else { 1.0 };
    let value = match kind {
        DurationKind::YearMonth => {
            let months = duration.to_months().abs();
            match component {
                "years" => as_f64_signed(months / 12),
                _ => as_f64_signed(months % 12),
            }
        }
        DurationKind::DayTime => {
            let total = duration.to_seconds().abs();
            match component {
                "days" => (total / 86_400.0).floor(),
                "hours" => ((total % 86_400.0) / 3_600.0).floor(),
                "minutes" => ((total % 3_600.0) / 60.0).floor(),
                _ => total % 60.0,
            }
        }
    };
    Ok(Value::Number(value * sign))
}

/// `timezone-from-date()` and its companions.
///
/// Returns the value's **own** timezone as a dayTimeDuration, or the empty
/// sequence when it carries none — not the implicit timezone, which is a
/// property of the evaluation rather than of the value.
fn timezone_of(name: &str, args: &[Value], document: &Document) -> Result<Value, EvalError> {
    let type_name = name
        .strip_prefix("timezone-from-")
        .ok_or_else(|| EvalError::new(format!("unknown function {name}()")))?;
    let kind = match type_name {
        "date" => TemporalKind::Date,
        "dateTime" => TemporalKind::DateTime,
        "time" => TemporalKind::Time,
        other => return Err(EvalError::new(format!("unknown type {other:?}"))),
    };

    let temporal = match args[0].as_sequence().and_then(|items| match items {
        [Item::Temporal(temporal)] => Some(*temporal),
        _ => None,
    }) {
        Some(temporal) => temporal,
        None => Temporal::parse(&args[0].to_xpath_string(document), kind)
            .map_err(EvalError::new)?,
    };

    Ok(match temporal.offset_minutes() {
        None => Value::Sequence(Vec::new()),
        Some(minutes) => Value::Sequence(vec![Item::Duration(Duration::from_seconds(
            f64::from(minutes) * 60.0,
        ))]),
    })
}

/// The `*-from-date`, `*-from-dateTime` and `*-from-time` accessors.
fn component_of(
    name: &str,
    args: &[Value],
    document: &Document,
) -> Result<Value, EvalError> {
    let (component, type_name) = name
        .split_once("-from-")
        .ok_or_else(|| EvalError::new(format!("unknown function {name}()")))?;
    let kind = match type_name {
        "date" => TemporalKind::Date,
        "dateTime" => TemporalKind::DateTime,
        "time" => TemporalKind::Time,
        other => return Err(EvalError::new(format!("unknown component type {other:?}"))),
    };

    // The argument may already be typed, or be an untyped value to cast.
    let temporal = match args[0].as_sequence().and_then(|items| match items {
        [Item::Temporal(temporal)] => Some(*temporal),
        _ => None,
    }) {
        Some(temporal) => temporal,
        None => Temporal::parse(&args[0].to_xpath_string(document), kind)
            .map_err(EvalError::new)?,
    };

    let value = match component {
        "year" => as_f64_signed(temporal.year()),
        "month" => f64::from(temporal.month()),
        "day" => f64::from(temporal.day()),
        "hours" => f64::from(temporal.hour()),
        "minutes" => f64::from(temporal.minute()),
        "seconds" => temporal.second(),
        other => {
            return Err(EvalError::new(format!(
                "{other:?} is not a component of {}",
                kind.as_str()
            )))
        }
    };
    Ok(Value::Number(value))
}

/// Converts a year to `f64`; years are far below the precision limit.
#[allow(clippy::cast_precision_loss)]
fn as_f64_signed(value: i64) -> f64 {
    value as f64
}

/// The numeric values of a node-set argument, for `min`, `max`, and `avg`.
fn numbers_of(args: &[Value], document: &Document) -> Vec<f64> {
    match &args[0] {
        Value::NodeSet(nodes) => nodes
            .iter()
            .map(|&node| parse_number(&document.string_value(node)))
            .collect(),
        Value::Sequence(items) => items.iter().map(|item| item.to_number(document)).collect(),
        // A single scalar is a one-item sequence in XPath 2.0 terms.
        other => vec![other.to_number(document)],
    }
}

/// `min()` and `max()`, which differ only in the comparison.
fn extreme(args: &[Value], document: &Document, pick: fn(f64, f64) -> f64) -> f64 {
    let numbers = numbers_of(args, document);
    // A NaN anywhere makes the result NaN, matching XPath 2.0, where a
    // non-numeric value makes the whole call a type error and so never
    // yields a usable answer either.
    if numbers.is_empty() || numbers.iter().any(|n| n.is_nan()) {
        return f64::NAN;
    }
    numbers.into_iter().fold(f64::NAN, |accumulated, next| {
        if accumulated.is_nan() {
            next
        } else {
            pick(accumulated, next)
        }
    })
}

/// Compiles a regular expression, honouring XPath 2.0's flag argument.
///
/// The `regex` crate's syntax is close to the XML Schema regular expressions
/// XPath 2.0 specifies, but not identical; `spec/xpath2/` lists the
/// differences. A pattern that does not compile is an error naming the
/// pattern, never a silently false test.
fn compile_regex(
    pattern: &str,
    flags: Option<&Value>,
    document: &Document,
) -> Result<regex::Regex, EvalError> {
    let flags = flags
        .map(|value| value.to_xpath_string(document))
        .unwrap_or_default();
    build_regex(pattern, &flags).map_err(EvalError::new)
}

/// Builds a regular expression from an XPath 2.0 pattern and flag string.
fn build_regex(pattern: &str, flags: &str) -> Result<regex::Regex, String> {
    let mut builder = regex::RegexBuilder::new(pattern);
    for flag in flags.chars() {
        match flag {
            'i' => {
                builder.case_insensitive(true);
            }
            'm' => {
                builder.multi_line(true);
            }
            's' => {
                builder.dot_matches_new_line(true);
            }
            'x' => {
                builder.ignore_whitespace(true);
            }
            other => {
                return Err(format!(
                    "unknown regular expression flag {other:?}; XPath 2.0 defines \
                     i, m, s and x"
                ))
            }
        }
    }

    builder
        .build()
        .map_err(|error| format!("the regular expression {pattern:?} did not compile: {error}"))
}

/// Rewrites XPath 2.0's `$1` group references into the `regex` crate's form.
///
/// The two agree on `$1`, but XPath escapes a literal dollar as `\$` while
/// the crate uses `$$`.
fn translate_replacement(replacement: &str) -> String {
    replacement.replace("\\$", "$$$$")
}

/// `document()`: the root nodes of external documents, by URI.
///
/// The argument is converted the way XSLT converts it: a node-set yields one
/// URI per node, from each node's string value, so `document(ref/@href)`
/// loads every referenced document at once. Anything else yields a single URI
/// from its string value.
///
/// A URI that is not in the registry contributes no node and is recorded as a
/// miss, which is how the validator discovers what to load for the next pass.
/// A caller evaluating XPath directly, with no registry, gets an error —
/// silently returning an empty node-set would turn a broken lookup into a
/// passing assertion.
fn document_function(args: &[Value], context: &EvalContext<'_>) -> Result<Value, EvalError> {
    let Some(registry) = context.documents else {
        return Err(EvalError::new(
            "document() needs a document registry, which the validator supplies \
             and a direct call to evaluate() does not; see EvalContext::with_documents",
        ));
    };

    let uris: Vec<String> = match &args[0] {
        Value::NodeSet(nodes) => nodes
            .iter()
            .map(|&node| context.document.string_value(node))
            .collect(),
        other => vec![other.to_xpath_string(context.document)],
    };

    // XSLT 1.0 section 12.1: with a second argument, a relative URI resolves
    // against the base URI of the *first node* of that node-set, rather than
    // against the instance. That is how a document loaded by an earlier
    // `document()` call can name its own neighbours.
    let base: Option<String> = match args.get(1) {
        None => None,
        Some(Value::NodeSet(nodes)) => {
            let first = nodes
                .iter()
                .copied()
                .min_by_key(|&node| context.document.order(node));
            match first {
                // An empty node-set names no base, so there is nothing to
                // resolve and nothing to load.
                //
                // This must not be an error. Loading runs in passes, and on
                // the first pass every `document()` call returns empty — so
                // `document(x, document(y))` has an empty second argument
                // until the pass that loads `y` has run. Erroring here would
                // abort the validation before the retry that makes it work.
                None => return Ok(Value::NodeSet(Vec::new())),
                Some(node) => {
                    let root = context.document.root_of(node);
                    registry
                        .origin_of(root)
                        .map(ToString::to_string)
                        .or_else(|| context.document.base_uri().map(ToString::to_string))
                }
            }
        }
        Some(other) => {
            return Err(EvalError::new(format!(
                "document(): the second argument must be a node-set, not {}",
                other.type_name()
            )))
        }
    };

    let mut roots: Vec<NodeId> = uris
        .iter()
        .filter_map(|uri| registry.lookup(uri, base.as_deref()))
        .collect();
    // A node-set is sorted and deduplicated; two references to one URI are
    // one node.
    roots.sort_unstable_by_key(|&root| context.document.order(root));
    roots.dedup();
    Ok(Value::NodeSet(roots))
}

/// `key(name, value)`: the nodes a named index holds under a value.
///
/// A node-set second argument looks up **each** of its string values, which
/// is the existential behaviour the rest of XPath uses — so
/// `key('parts', line/@ref)` finds every referenced part at once.
fn key_function(args: &[Value], context: &EvalContext<'_>) -> Result<Value, EvalError> {
    let Some(keys) = context.keys else {
        return Err(EvalError::new(
            "key() needs the run's key indexes, which the validator supplies and \
             a direct call to evaluate() does not; see EvalContext::with_keys",
        ));
    };

    let name = args[0].to_xpath_string(context.document);
    if !keys.is_declared(&name) {
        // A missing declaration is a mistake in the schema, not an empty
        // result: returning nothing would make the assertion quietly pass.
        return Err(EvalError::new(format!(
            "no key named {name:?} is declared; add <key name=\"{name}\" \
             match=\"…\" use=\"…\"/> to the schema"
        )));
    }

    let wanted: Vec<String> = match &args[1] {
        Value::NodeSet(nodes) => nodes
            .iter()
            .map(|&node| context.document.string_value(node))
            .collect(),
        other => vec![other.to_xpath_string(context.document)],
    };

    let mut found: Vec<NodeId> = wanted
        .iter()
        .flat_map(|value| keys.lookup(&name, value))
        .collect();
    found.sort_unstable_by_key(|&node| context.document.order(node));
    found.dedup();
    Ok(Value::NodeSet(found))
}

/// `id()` without a DTD.
///
/// Nothing tells the crate which attributes have type ID, because DTDs are not
/// processed, so `xml:id` and any attribute named `id` are treated as
/// identifiers. Every DTD-less processor does something equivalent.
fn id_function(args: &[Value], context: &EvalContext<'_>) -> Vec<NodeId> {
    let document = context.document;
    let wanted: Vec<String> = match args.first() {
        Some(Value::NodeSet(nodes)) => nodes
            .iter()
            .flat_map(|&n| {
                document
                    .string_value(n)
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .collect(),
        Some(value) => value
            .to_xpath_string(document)
            .split_whitespace()
            .map(ToString::to_string)
            .collect(),
        None => Vec::new(),
    };

    let mut found = Vec::new();
    // Scoped to the document the context node belongs to: `id()` must not
    // reach into a document merged in by `document()`.
    for node in document.descendants_or_self(document.root_of(context.node)) {
        if document.kind(node) != NodeKind::Element {
            continue;
        }
        for &attribute in document.attributes(node) {
            let Some(name) = document.name(attribute) else {
                continue;
            };
            // With no DTD there is no attribute typed ID, so `id` and
            // `xml:id` are what every DTD-less processor treats as one.
            let is_id = name.local == "id"
                && matches!(name.uri.as_deref(), None | Some(crate::xml::XML_NAMESPACE));
            if is_id && wanted.iter().any(|w| w == document.value(attribute)) {
                found.push(node);
                break;
            }
        }
    }
    found
}

/// `substring()`, with XPath 1.0's one-based, rounding, NaN-tolerant indexing.
///
/// The specification defines it in terms of `round()` and inequalities rather
/// than integer slicing, which is why `substring('12345', 1.5, 2.6)` is `'234'`
/// and why a NaN start yields the empty string instead of an error.
fn substring(args: &[Value], document: &Document) -> String {
    let text: Vec<char> = args[0].to_xpath_string(document).chars().collect();
    let start = round_half_up(args[1].to_number(document));
    let end = match args.get(2) {
        Some(length) => {
            let length = round_half_up(length.to_number(document));
            if length.is_nan() {
                return String::new();
            }
            start + length
        }
        None => f64::INFINITY,
    };
    if start.is_nan() {
        return String::new();
    }

    let mut out = String::new();
    for (index, c) in text.iter().enumerate() {
        // XPath positions are one-based.
        let position = as_f64(index + 1);
        if position >= start && position < end {
            out.push(*c);
        }
    }
    out
}

/// XPath's `round()`: halfway cases go towards positive infinity, so
/// `round(-0.5)` is `0`, not `-1`.
fn round_half_up(value: f64) -> f64 {
    if value.is_nan() || value.is_infinite() {
        return value;
    }
    (value + 0.5).floor()
}

/// Collapses internal whitespace runs and trims the ends.
fn normalize_space(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `translate()`: character-wise mapping, with characters beyond the end of
/// the replacement string removed rather than replaced.
fn translate(text: &str, from: &str, to: &str) -> String {
    let from: Vec<char> = from.chars().collect();
    let to: Vec<char> = to.chars().collect();
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match from.iter().position(|&f| f == c) {
            // First occurrence wins, and a character past the end of `to` is
            // deleted.
            Some(index) => {
                if let Some(&replacement) = to.get(index) {
                    out.push(replacement);
                }
            }
            None => out.push(c),
        }
    }
    out
}

/// `lang()`: walks up for the nearest `xml:lang`, then matches the language
/// tag case-insensitively, allowing a suffix after a hyphen.
fn lang(wanted: &str, document: &Document, node: NodeId) -> bool {
    let mut current = Some(node);
    while let Some(id) = current {
        for &attribute in document.attributes(id) {
            if let Some(name) = document.name(attribute) {
                if name.local == "lang" && name.uri.as_deref() == Some(crate::xml::XML_NAMESPACE) {
                    let actual = document.value(attribute).to_lowercase();
                    let wanted = wanted.to_lowercase();
                    return actual == wanted
                        || actual.strip_prefix(&wanted).is_some_and(|r| r.starts_with('-'));
                }
            }
        }
        current = document.parent(id);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arity_is_checked() {
        assert!(check_function("contains", 2, XPathVersion::V1).is_ok());
        assert!(check_function("contains", 1, XPathVersion::V1).is_err());
        assert!(check_function("concat", 5, XPathVersion::V1).is_ok());
        assert!(check_function("concat", 1, XPathVersion::V1).is_err());
        assert!(check_function("true", 1, XPathVersion::V1).is_err());
        assert!(check_function("string", 0, XPathVersion::V1).is_ok());
        assert!(check_function("string", 1, XPathVersion::V1).is_ok());
        assert!(check_function("string", 2, XPathVersion::V1).is_err());
    }

    #[test]
    fn xpath_two_functions_are_refused_under_a_one_point_zero_binding() {
        let message = check_function("matches", 2, XPathVersion::V1).unwrap_err();
        assert!(message.contains("XPath 2.0"), "{message}");
        assert!(message.contains("xslt2"), "{message}");
    }

    #[test]
    fn xpath_two_functions_are_accepted_under_a_two_point_zero_binding() {
        assert!(check_function("matches", 2, XPathVersion::V2).is_ok());
        assert!(check_function("matches", 3, XPathVersion::V2).is_ok());
        assert!(check_function("matches", 1, XPathVersion::V2).is_err());
        assert!(check_function("string-join", 2, XPathVersion::V2).is_ok());
    }

    #[test]
    fn unimplemented_two_point_zero_functions_say_what_they_need() {
        let sequences = check_function("subsequence", 2, XPathVersion::V2).unwrap_err();
        assert!(sequences.contains("sequence"), "{sequences}");

        let dates = check_function("adjust-date-to-timezone", 2, XPathVersion::V2).unwrap_err();
        assert!(dates.contains("date and time"), "{dates}");
    }

    #[test]
    fn the_date_functions_are_available_now_that_dates_exist() {
        for (name, arity) in [
            ("current-date", 0),
            ("current-dateTime", 0),
            ("year-from-date", 1),
            ("hours-from-time", 1),
            ("xs:date", 1),
        ] {
            assert!(
                check_function(name, arity, XPathVersion::V2).is_ok(),
                "{name}() should be available"
            );
        }
        // Still an XPath 2.0 function, so an XPath 1.0 binding refuses it.
        assert!(check_function("current-date", 0, XPathVersion::V1).is_err());
    }

    #[test]
    fn the_duration_functions_are_available_now_that_durations_exist() {
        for (name, arity) in [
            ("xs:dayTimeDuration", 1),
            ("xs:yearMonthDuration", 1),
            ("days-from-duration", 1),
            ("months-from-duration", 1),
        ] {
            assert!(
                check_function(name, arity, XPathVersion::V2).is_ok(),
                "{name}() should be available"
            );
        }
        assert!(check_function("xs:dayTimeDuration", 1, XPathVersion::V1).is_err());
    }

    #[test]
    fn the_sequence_functions_are_available_now_that_sequences_exist() {
        // These moved out of the "needs sequences" list in phase 2a.
        for (name, arity) in [("tokenize", 2), ("distinct-values", 1), ("index-of", 2)] {
            assert!(
                check_function(name, arity, XPathVersion::V2).is_ok(),
                "{name}() should be available"
            );
        }
    }

    #[test]
    fn the_two_point_zero_library_has_the_documented_names() {
        let names = function_names_v2();
        // Growing this number is expected; the point of the check is that the
        // list and the documentation move together, which
        // `tests/docs.rs::the_xpath_two_function_list_in_the_spec_matches_the_engine`
        // enforces.
        assert_eq!(names.len(), 45, "{names:?}");
        for expected in [
            "matches",
            "replace",
            "abs",
            "exists",
            "string-join",
            "tokenize",
            "distinct-values",
            "index-of",
            "current-date",
            "year-from-date",
            "xs:date",
            "xs:dayTimeDuration",
            "xs:yearMonthDuration",
            "days-from-duration",
            "years-from-duration",
            "timezone-from-date",
            "implicit-timezone",
        ] {
            assert!(names.contains(&expected), "{expected} is missing");
        }
    }

    #[test]
    fn unknown_functions_are_rejected() {
        assert!(check_function("nonsense", 0, XPathVersion::V1).is_err());
    }

    #[test]
    fn library_has_every_expected_name() {
        // The 27 core XPath 1.0 functions, plus the three from the XSLT
        // library that the `xslt` query binding makes available.
        let names = function_names();
        assert_eq!(names.len(), 30, "{names:?}");
        for from_xslt in ["current", "document", "key"] {
            assert!(names.contains(&from_xslt), "{from_xslt}() is missing");
        }
    }

    #[test]
    fn document_without_a_registry_is_an_error() {
        // Returning an empty node-set instead would turn a misconfigured
        // lookup into a quietly passing assertion.
        assert!(check_function("document", 1, XPathVersion::V1).is_ok());
        // XSLT 1.0 section 12.1 gives `document()` a second, optional
        // argument: the node-set whose first node supplies the base URI.
        assert!(check_function("document", 2, XPathVersion::V1).is_ok());
        assert!(check_function("document", 0, XPathVersion::V1).is_err());
        assert!(check_function("document", 3, XPathVersion::V1).is_err());
    }

    #[test]
    fn round_sends_halves_upwards() {
        assert_eq!(round_half_up(0.5), 1.0);
        assert_eq!(round_half_up(-0.5), 0.0);
        assert_eq!(round_half_up(1.5), 2.0);
        assert_eq!(round_half_up(-1.5), -1.0);
    }

    #[test]
    fn normalize_space_collapses_runs() {
        assert_eq!(normalize_space("  a   b \n c "), "a b c");
        assert_eq!(normalize_space("   "), "");
    }

    #[test]
    fn translate_deletes_unmapped_characters() {
        assert_eq!(translate("bar", "abc", "ABC"), "BAr");
        assert_eq!(translate("--aaa--", "abc-", "ABC"), "AAA");
        assert_eq!(translate("abc", "aa", "XY"), "Xbc");
    }

    #[test]
    fn substring_follows_the_specification_not_integer_slicing() {
        assert_eq!(round_half_up(1.5), 2.0);
        assert_eq!(normalize_space(" a "), "a");
    }
}
