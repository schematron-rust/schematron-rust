//! Which version of XPath an expression is written in.
//!
//! Schematron's query binding decides this: `xslt` and `xpath` mean XPath
//! 1.0, `xslt2` and `xpath2` mean XPath 2.0, `xslt3` and `xpath3` mean XPath
//! 3.0. The version gates the syntax and the function library, so that a
//! schema declaring 1.0 cannot accidentally acquire 2.0 or 3.0 behaviour, and
//! a construct belonging to a version the crate does not implement is a hard
//! error rather than a wrong answer.
//!
//! See `spec/xpath2/` and `spec/xpath3/`, which are explicit about how much
//! of each version is implemented and about where a schema still evaluates
//! with an earlier version's semantics.

/// The XPath version an expression is evaluated as.
///
/// Ordered: `V1 < V2 < V3`, because XPath 3.0 is a superset of 2.0 the way
/// 2.0 is of 1.0 — every construct a lower version admits, a higher one
/// admits too. [`XPathVersion::is_v2`] reads as "at least 2.0" for exactly
/// that reason; [`XPathVersion::is_v3`] is the narrower "exactly 3.0-only
/// syntax is admitted" check a 3.0-specific construct needs.
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
    /// Only the phased subset in `spec/xpath2/` is implemented. Anything
    /// outside it is an error naming the construct.
    V2,
    /// XPath 3.0: the `xslt3` and `xpath3` query bindings.
    ///
    /// Only the phase-1 subset in `spec/xpath3/` is implemented — function
    /// items, inline function expressions, named function references,
    /// dynamic function calls, and `for-each()`. Everything XPath 2.0
    /// implements is available too, since 3.0 is a superset. Anything
    /// outside the 3.0 subset is an error naming the construct, exactly as
    /// for 2.0.
    V3,
}

impl XPathVersion {
    /// Whether this version admits the XPath 2.0 additions — that is,
    /// whether it is 2.0 or later.
    #[must_use]
    pub const fn is_v2(self) -> bool {
        matches!(self, XPathVersion::V2 | XPathVersion::V3)
    }

    /// Whether this version admits the XPath 3.0 additions.
    #[must_use]
    pub const fn is_v3(self) -> bool {
        matches!(self, XPathVersion::V3)
    }

    /// A human-readable name, for error messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            XPathVersion::V1 => "XPath 1.0",
            XPathVersion::V2 => "XPath 2.0",
            XPathVersion::V3 => "XPath 3.0",
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
        assert_eq!(XPathVersion::V3.as_str(), "XPath 3.0");
    }

    #[test]
    fn three_is_a_superset_of_two_which_is_a_superset_of_one() {
        assert!(XPathVersion::V3.is_v2());
        assert!(XPathVersion::V3.is_v3());
        assert!(XPathVersion::V2.is_v2());
        assert!(!XPathVersion::V2.is_v3());
        assert!(!XPathVersion::V1.is_v2());
        assert!(!XPathVersion::V1.is_v3());
    }

    #[test]
    fn versions_order_by_how_much_they_admit() {
        assert!(XPathVersion::V1 < XPathVersion::V2);
        assert!(XPathVersion::V2 < XPathVersion::V3);
    }
}
