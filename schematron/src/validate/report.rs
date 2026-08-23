//! Validation results as data.
//!
//! A report is a value, not a pile of formatted strings, so the same run can
//! be rendered as SVRL for other Schematron tooling, as JSON for a pipeline,
//! or as text for a person. The tree here mirrors the run: patterns contain
//! the rules that fired, which contain the assertions that were reported.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::schema::Ns;

/// Which kind of finding an assertion produced.
///
/// The distinction is the whole difference between `assert` and `report`, and
/// conflating them is the most common way to misuse Schematron: a successful
/// report is an observation, not a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ResultKind {
    /// An `assert` whose test evaluated false. This is a validation failure.
    FailedAssert,
    /// A `report` whose test evaluated true. This is an observation.
    SuccessfulReport,
}

impl ResultKind {
    /// The SVRL element name for this kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ResultKind::FailedAssert => "failed-assert",
            ResultKind::SuccessfulReport => "successful-report",
        }
    }
}

/// One instantiated diagnostic attached to a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DiagnosticResult {
    /// The `diagnostic/@id` that was referenced.
    pub id: String,
    /// The message, instantiated against the firing node.
    pub text: String,
}

/// One instantiated property attached to a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PropertyResult {
    /// The `property/@id` that was referenced.
    pub id: String,
    /// The property's role.
    pub role: Option<String>,
    /// The property's scheme.
    pub scheme: Option<String>,
    /// The value, instantiated against the firing node.
    pub text: String,
}

/// One finding: a failed assertion or a successful report.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AssertionResult {
    /// Whether this is a failure or an observation.
    pub kind: ResultKind,
    /// The XPath test, verbatim from the schema.
    pub test: String,
    /// An absolute XPath identifying the subject node.
    pub location: String,
    /// The instantiated human-readable message.
    pub text: String,
    /// The assertion's `@id`.
    pub id: Option<String>,
    /// The resolved role: the assertion's, else the rule's.
    pub role: Option<String>,
    /// The resolved flag: the assertion's, else the rule's.
    pub flag: Option<String>,
    /// A URI for further reading.
    pub see: Option<String>,
    /// A URI of an icon.
    pub icon: Option<String>,
    /// A formal public identifier.
    pub fpi: Option<String>,
    /// The instantiated diagnostics.
    pub diagnostics: Vec<DiagnosticResult>,
    /// The instantiated properties.
    pub properties: Vec<PropertyResult>,
}

impl AssertionResult {
    /// Whether this finding is a validation failure.
    #[must_use]
    pub fn is_failure(&self) -> bool {
        self.kind == ResultKind::FailedAssert
    }
}

/// A rule that matched a node, and what it found there.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FiredRule {
    /// The rule's `@id`.
    pub id: Option<String>,
    /// The rule's `@context`, verbatim.
    pub context: String,
    /// The rule's `@role`.
    pub role: Option<String>,
    /// The rule's `@flag`.
    pub flag: Option<String>,
    /// An absolute XPath identifying the node the rule fired on.
    pub location: String,
    /// The findings, in assertion order.
    pub assertions: Vec<AssertionResult>,
}

/// A pattern that ran against a document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ActivePattern {
    /// The pattern's `@id`.
    pub id: Option<String>,
    /// The pattern's title, which SVRL carries as `@name`.
    pub name: Option<String>,
    /// The document this run of the pattern was against, when the pattern has
    /// a `@documents` attribute.
    pub documents: Option<String>,
    /// The rules that fired, in the order they fired.
    pub rules: Vec<FiredRule>,
}

/// The result of validating one document against one schema.
///
/// # Examples
///
/// ```
/// use schematron::{Document, Schema};
///
/// let schema = Schema::from_str(r#"
///     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
///       <pattern>
///         <rule context="a">
///           <assert test="b">Needs a b.</assert>
///           <report test="c">Has a c.</report>
///         </rule>
///       </pattern>
///     </schema>
/// "#)?;
///
/// let report = schema.validate(&Document::from_str("<a><c/></a>")?)?;
///
/// // The failed assert is a failure; the successful report is not.
/// assert_eq!(report.count_failures(), 1);
/// assert_eq!(report.reports().count(), 1);
/// assert!(!report.is_valid());
/// # Ok::<(), schematron::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Report {
    /// The schema's title.
    pub title: Option<String>,
    /// The phase that ran, when one was named.
    pub phase: Option<String>,
    /// The schema's `@schemaVersion`.
    pub schema_version: Option<String>,
    /// The schema's namespace bindings, so a consumer can interpret
    /// `@location` and `@test`.
    pub namespaces: Vec<Ns>,
    /// The patterns that ran, in order.
    pub patterns: Vec<ActivePattern>,
}

impl Report {
    /// Whether the document satisfied the schema.
    ///
    /// True when no `assert` failed. Successful reports are observations and
    /// do not make a document invalid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.count_failures() == 0
    }

    /// Every finding, in the order it was produced.
    pub fn assertions(&self) -> impl Iterator<Item = &AssertionResult> {
        self.patterns
            .iter()
            .flat_map(|pattern| pattern.rules.iter())
            .flat_map(|rule| rule.assertions.iter())
    }

    /// The failed assertions.
    pub fn failures(&self) -> impl Iterator<Item = &AssertionResult> {
        self.assertions()
            .filter(|a| a.kind == ResultKind::FailedAssert)
    }

    /// The successful reports.
    pub fn reports(&self) -> impl Iterator<Item = &AssertionResult> {
        self.assertions()
            .filter(|a| a.kind == ResultKind::SuccessfulReport)
    }

    /// How many assertions failed.
    #[must_use]
    pub fn count_failures(&self) -> usize {
        self.failures().count()
    }

    /// The findings carrying a given `@flag`.
    ///
    /// `@flag` conventionally holds a severity such as `error` or `warning`.
    /// The crate assigns it no meaning; this is how a caller applies its own.
    pub fn with_flag<'a>(&'a self, flag: &'a str) -> impl Iterator<Item = &'a AssertionResult> {
        self.assertions()
            .filter(move |a| a.flag.as_deref() == Some(flag))
    }

    /// The rules that fired, across every pattern.
    pub fn fired_rules(&self) -> impl Iterator<Item = &FiredRule> {
        self.patterns.iter().flat_map(|pattern| pattern.rules.iter())
    }

    /// How many rules fired.
    ///
    /// A count of zero on a non-empty document almost always means no rule
    /// context matched anything — most often a missing `ns` prefix binding.
    #[must_use]
    pub fn count_fired_rules(&self) -> usize {
        self.fired_rules().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(kind: ResultKind, flag: Option<&str>) -> AssertionResult {
        AssertionResult {
            kind,
            test: "t".into(),
            location: "/".into(),
            text: "m".into(),
            id: None,
            role: None,
            flag: flag.map(ToString::to_string),
            see: None,
            icon: None,
            fpi: None,
            diagnostics: vec![],
            properties: vec![],
        }
    }

    fn report_with(assertions: Vec<AssertionResult>) -> Report {
        Report {
            patterns: vec![ActivePattern {
                id: None,
                name: None,
                documents: None,
                rules: vec![FiredRule {
                    id: None,
                    context: "a".into(),
                    role: None,
                    flag: None,
                    location: "/".into(),
                    assertions,
                }],
            }],
            ..Report::default()
        }
    }

    #[test]
    fn an_empty_report_is_valid() {
        assert!(Report::default().is_valid());
        assert_eq!(Report::default().count_failures(), 0);
    }

    #[test]
    fn successful_reports_do_not_make_a_document_invalid() {
        let report = report_with(vec![finding(ResultKind::SuccessfulReport, None)]);
        assert!(report.is_valid());
        assert_eq!(report.reports().count(), 1);
        assert_eq!(report.failures().count(), 0);
    }

    #[test]
    fn failed_asserts_do() {
        let report = report_with(vec![finding(ResultKind::FailedAssert, None)]);
        assert!(!report.is_valid());
        assert_eq!(report.count_failures(), 1);
    }

    #[test]
    fn findings_can_be_filtered_by_flag() {
        let report = report_with(vec![
            finding(ResultKind::FailedAssert, Some("error")),
            finding(ResultKind::FailedAssert, Some("warning")),
        ]);
        assert_eq!(report.with_flag("error").count(), 1);
        assert_eq!(report.with_flag("warning").count(), 1);
        assert_eq!(report.with_flag("info").count(), 0);
    }

    #[test]
    fn fired_rules_are_counted_separately_from_findings() {
        let report = report_with(vec![]);
        assert_eq!(report.count_fired_rules(), 1);
        assert_eq!(report.assertions().count(), 0);
    }
}
