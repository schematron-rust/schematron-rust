//! Which version of XPath an expression is written in.
//!
//! Schematron's query binding decides this: `xslt` and `xpath` mean XPath
//! 1.0, `xslt2` and `xpath2` mean XPath 2.0. The version gates the syntax and
//! the function library, so that a schema declaring 1.0 cannot accidentally
//! acquire 2.0 behaviour, and a construct belonging to a version the crate
//! does not implement is a hard error rather than a wrong answer.
//!
//! See `spec/xpath2.md`, which is explicit about how much of XPath 2.0 is
//! implemented and about where a 2.0 schema still evaluates with 1.0
//! semantics.

/// The XPath version an expression is evaluated as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
// Variants will be added: further XPath versions are on the roadmap. Marking
// it non-exhaustive now means that will not be a breaking change.
#[non_exhaustive]
pub enum XPathVersion {
    /// XPath 1.0: the `xslt` and `xpath` query bindings, and the default.
    #[default]
    V1,
    /// XPath 2.0: the `xslt2` and `xpath2` query bindings.
    ///
    /// Only the phase-1 subset in `spec/xpath2.md` is implemented. Anything
    /// outside it is an error naming the construct.
    V2,
}

impl XPathVersion {
    /// Whether this version admits the XPath 2.0 additions.
    #[must_use]
    pub const fn is_v2(self) -> bool {
        matches!(self, XPathVersion::V2)
    }

    /// A human-readable name, for error messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            XPathVersion::V1 => "XPath 1.0",
            XPathVersion::V2 => "XPath 2.0",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_xpath_one() {
        assert_eq!(XPathVersion::default(), XPathVersion::V1);
        assert!(!XPathVersion::default().is_v2());
    }

    #[test]
    fn versions_are_named_for_error_messages() {
        assert_eq!(XPathVersion::V1.as_str(), "XPath 1.0");
        assert_eq!(XPathVersion::V2.as_str(), "XPath 2.0");
    }
}
