//! The crate's error type.
//!
//! One enum covers everything, and every variant names both what failed and
//! where, because a schema error that says only "invalid XPath" costs the
//! reader more time than the crate saved them. See `spec/errors/`.
//!
//! The distinction that matters: a **finding** is a document breaking a rule,
//! and lives in a [`Report`](crate::Report). An **error** is the crate being
//! unable to do its job, and lives here. A false assertion is never an error,
//! and an error is never quietly downgraded into a false assertion — that
//! would let a broken schema report a clean bill of health.

use std::fmt;

/// The result type used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong loading a schema or validating a document.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    #[error("I/O error for {path}: {source}")]
    Io {
        /// The path involved.
        path: String,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// An XML document is not well-formed, or uses a construct the parser
    /// deliberately refuses, such as a DTD-defined entity.
    #[error("XML parse error at line {line}, column {column}: {message}")]
    XmlParse {
        /// One-based line number.
        line: usize,
        /// One-based column number.
        column: usize,
        /// What went wrong.
        message: String,
    },

    /// A schema is well-formed XML but is not a valid Schematron schema.
    #[error("schema error in <{element}>{}: {message}", OptionalAt(.location))]
    Schema {
        /// The Schematron element at fault.
        element: String,
        /// A path to the construct, such as `pattern[@id='p']/rule[1]`.
        location: Option<String>,
        /// What went wrong.
        message: String,
    },

    /// An XPath expression could not be parsed.
    ///
    /// Raised while compiling the schema, not while validating, so a typo in
    /// a `test` fails immediately rather than on somebody else's document.
    #[error(
        "XPath syntax error in {context}: {message}\n  expression: {}\n{caret}",
        excerpt(expression, *position).0
    )]
    XPathSyntax {
        /// A path to the schema construct holding the expression.
        context: String,
        /// The whole expression source. The message shows at most 60
        /// characters either side of `position`.
        expression: String,
        /// Byte offset of the offending character.
        position: usize,
        /// What went wrong.
        message: String,
        /// A pre-rendered caret line pointing at `position` within the
        /// excerpt the message shows.
        caret: String,
    },

    /// An XPath expression failed at evaluation time.
    #[error("XPath evaluation error in {context}: {message}")]
    XPathEval {
        /// A path to the schema construct holding the expression.
        context: String,
        /// What went wrong.
        message: String,
    },

    /// A phase was requested that the schema does not define.
    #[error("unknown phase {phase:?}; the schema defines: {available}")]
    UnknownPhase {
        /// The phase asked for.
        phase: String,
        /// The phases the schema actually has.
        available: String,
    },

    /// The schema declares a query language binding this crate does not
    /// implement.
    #[error(
        "unsupported query binding {binding:?}; this crate implements XPath 1.0, \
         so the supported bindings are \"xslt\" and \"xpath\". \
         Set allow_unknown_query_binding to compile it anyway."
    )]
    UnsupportedQueryBinding {
        /// The binding named by `schema/@queryBinding`.
        binding: String,
    },

    /// Includes form a cycle.
    #[error("include cycle: {chain}")]
    IncludeCycle {
        /// The chain of hrefs, joined by arrows.
        chain: String,
    },

    /// Includes nested deeper than the configured limit.
    #[error("include depth limit of {limit} exceeded at {href}")]
    IncludeDepth {
        /// The configured limit.
        limit: usize,
        /// The href that exceeded it.
        href: String,
    },

    /// A URI could not be fetched, or the resolver refused it.
    #[error("cannot resolve {href}: {message}")]
    Resolve {
        /// The URI.
        href: String,
        /// Why it failed, or why it was refused.
        message: String,
    },

    /// A streaming validation request could not be honoured: the schema or
    /// the document does not meet the constraints streaming validation
    /// requires. See `spec/streaming/`.
    ///
    /// Deliberately a hard error rather than a silent fallback to full
    /// materialization — someone asking for streaming likely did so
    /// because the document will not fit fully in memory, and guessing
    /// would risk exhausting it instead of failing cleanly.
    #[error("streaming validation is not possible: {message}")]
    Streaming {
        /// What specifically disqualified it.
        message: String,
    },
    /// A run option was supplied but is out of range or not a valid value.
    ///
    /// Raised before any validation starts; the option is never replaced by a
    /// default or clamped (RCA-IN-4).
    #[error("invalid option {option}: {message}")]
    InvalidOption {
        /// The option's name.
        option: &'static str,
        /// Why the value is refused.
        message: String,
    },
}

impl Error {
    /// Builds a schema error.
    pub(crate) fn schema(
        element: impl Into<String>,
        location: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Error::Schema {
            element: element.into(),
            location,
            message: message.into(),
        }
    }

    /// Builds an XPath syntax error, rendering the caret line from the
    /// expression and the offset.
    pub(crate) fn xpath_syntax(
        context: impl Into<String>,
        expression: &str,
        position: usize,
        message: impl Into<String>,
    ) -> Self {
        let (_, prefix_width) = excerpt(expression, position);
        let caret = format!("{:width$}^", "  ", width = prefix_width + 14);
        Error::XPathSyntax {
            context: context.into(),
            expression: expression.to_string(),
            position,
            message: message.into(),
            caret,
        }
    }

    /// Builds an XPath evaluation error.
    pub(crate) fn xpath_eval(context: impl Into<String>, message: impl Into<String>) -> Self {
        Error::XPathEval {
            context: context.into(),
            message: message.into(),
        }
    }

    pub(crate) fn streaming(message: impl Into<String>) -> Self {
        Error::Streaming {
            message: message.into(),
        }
    }
}

/// How many characters of an expression a syntax error shows on each side
/// of the offending position.
///
/// A hostile or generated schema can hold an expression hundreds of
/// kilobytes long, and echoing all of it buries the one character that
/// matters. The whole source stays in the error's `expression` field.
const EXCERPT_CONTEXT: usize = 60;

/// The part of `expression` a syntax error message shows, and the caret's
/// column within it, counted in characters, not bytes, so the caret lands
/// under the right glyph for non-ASCII expressions. Each side is cut to
/// [`EXCERPT_CONTEXT`] characters, and `…` marks the cut.
fn excerpt(expression: &str, position: usize) -> (String, usize) {
    let prefix = expression
        .get(..position.min(expression.len()))
        .unwrap_or("");
    let before: Vec<char> = prefix.chars().collect();
    let after = expression.get(prefix.len()..).unwrap_or("");

    let mut out = String::new();
    let mut column = before.len();
    if before.len() > EXCERPT_CONTEXT {
        out.push('…');
        let skipped = before.len() - EXCERPT_CONTEXT;
        out.extend(&before[skipped..]);
        column = EXCERPT_CONTEXT + 1;
    } else {
        out.extend(&before);
    }
    let mut tail = after.chars();
    out.extend(tail.by_ref().take(EXCERPT_CONTEXT + 1));
    if tail.next().is_some() {
        out.push('…');
    }
    (out, column)
}

/// Renders `Some(location)` as ` at location`, and `None` as nothing.
struct OptionalAt<'a>(&'a Option<String>);

impl fmt::Display for OptionalAt<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(location) => write!(f, " at {location}"),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_error_includes_location_when_present() {
        let e = Error::schema("rule", Some("pattern[1]/rule[2]".into()), "no context");
        let text = e.to_string();
        assert!(text.contains("<rule>"), "{text}");
        assert!(text.contains("at pattern[1]/rule[2]"), "{text}");
    }

    #[test]
    fn schema_error_omits_location_when_absent() {
        let e = Error::schema("schema", None, "empty");
        assert_eq!(e.to_string(), "schema error in <schema>: empty");
    }

    #[test]
    fn xpath_syntax_error_renders_a_caret() {
        let e = Error::xpath_syntax("test", "count(a", 7, "expected ')'");
        let text = e.to_string();
        assert!(text.contains("expected ')'"), "{text}");
        assert!(text.contains('^'), "{text}");
    }

    /// The caret's column under the expression: the message's last line,
    /// less the caret itself and the 14-character `  expression: ` label.
    fn caret_column(error: &Error) -> usize {
        error.to_string().lines().last().unwrap().len() - 1 - 14
    }

    #[test]
    fn a_short_expression_is_shown_whole() {
        let e = Error::xpath_syntax("test", "é(a", 2, "unexpected");
        assert!(e.to_string().contains("  expression: é(a\n"), "{e}");
        // `é` is two bytes but one character, so the caret is at column 1.
        assert_eq!(caret_column(&e), 1);
    }

    #[test]
    fn a_long_expression_is_cut_around_the_error() {
        let source = format!("{}X{}", "a".repeat(500), "b".repeat(500));
        let e = Error::xpath_syntax("test", &source, 500, "unexpected X");
        let text = e.to_string();
        let shown = format!("…{}X{}…", "a".repeat(60), "b".repeat(60));
        assert!(text.contains(&format!("  expression: {shown}\n")), "{text}");
        assert_eq!(caret_column(&e), 61);
        // The full source is still there for a caller that wants it.
        assert!(matches!(&e, Error::XPathSyntax { expression, .. } if *expression == source));

        // Cut on one side only: near the start, and exactly at the limit.
        let near_start = Error::xpath_syntax("test", &source, 3, "unexpected");
        assert!(
            near_start.to_string().contains("  expression: aaa"),
            "{near_start}"
        );
        assert_eq!(caret_column(&near_start), 3);
        let exact = "c".repeat(121);
        let at_limit = Error::xpath_syntax("test", &exact, 60, "unexpected");
        assert!(at_limit
            .to_string()
            .contains(&format!("  expression: {exact}\n")));
    }
}
