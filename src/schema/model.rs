//! The Schematron document model.
//!
//! One Rust type per element of ISO/IEC 19757-3, as catalogued in
//! `spec/data-model.md`. XPath expressions are held as source strings here;
//! they are parsed once by the compiler and cached on the
//! [`Schema`](crate::Schema), which is why abstract-pattern parameter
//! substitution — which is textual — can happen before anything is parsed.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::xpath::XPathVersion;

/// The ISO Schematron namespace.
pub const SCHEMATRON_NAMESPACE: &str = "http://purl.oclc.org/dsdl/schematron";

/// The pre-ISO Schematron 1.5 namespace, accepted for compatibility.
pub const SCHEMATRON_1_5_NAMESPACE: &str = "http://www.ascc.net/xml/schematron";

/// The query language binding a schema declares.
///
/// This crate implements XPath 1.0, which is what `xslt` and `xpath` mean.
/// Later bindings are a different language with a different type system, so
/// they are refused rather than approximated. See `spec/conformance.md`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum QueryBinding {
    /// No `@queryBinding`; the standard's default, which is XPath 1.0.
    #[default]
    Default,
    /// `queryBinding="xslt"` — XPath 1.0 plus the XSLT function library.
    Xslt,
    /// `queryBinding="xpath"` — plain XPath 1.0.
    Xpath,
    /// `queryBinding="xslt2"` — XPath 2.0, phase 1; see `spec/xpath2.md`.
    Xslt2,
    /// `queryBinding="xpath2"` — XPath 2.0, phase 1.
    Xpath2,
    /// A binding this crate does not implement, such as `xslt3`.
    Other(String),
}

impl QueryBinding {
    /// Parses the attribute value.
    #[must_use]
    pub fn parse(value: &str) -> QueryBinding {
        match value.to_ascii_lowercase().as_str() {
            "xslt" => QueryBinding::Xslt,
            "xpath" => QueryBinding::Xpath,
            "xslt2" => QueryBinding::Xslt2,
            "xpath2" => QueryBinding::Xpath2,
            other => QueryBinding::Other(other.to_string()),
        }
    }

    /// The XPath version this binding selects.
    ///
    /// `xslt3` and later are not implemented and never reach this: they are
    /// refused at compile time unless the caller forces them, in which case
    /// they are treated as XPath 1.0 and every 2.0 construct in them is an
    /// error naming itself.
    #[must_use]
    pub const fn version(&self) -> XPathVersion {
        match self {
            QueryBinding::Xslt2 | QueryBinding::Xpath2 => XPathVersion::V2,
            _ => XPathVersion::V1,
        }
    }

    /// Whether this crate can run a schema with this binding.
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        matches!(
            self,
            QueryBinding::Default
                | QueryBinding::Xslt
                | QueryBinding::Xpath
                | QueryBinding::Xslt2
                | QueryBinding::Xpath2
        )
    }

    /// The attribute value, or `None` when the schema declared none.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            QueryBinding::Default => None,
            QueryBinding::Xslt => Some("xslt"),
            QueryBinding::Xpath => Some("xpath"),
            QueryBinding::Xslt2 => Some("xslt2"),
            QueryBinding::Xpath2 => Some("xpath2"),
            QueryBinding::Other(name) => Some(name),
        }
    }
}

/// A namespace prefix binding, from `<sch:ns>`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Ns {
    /// The prefix XPath expressions will use.
    pub prefix: String,
    /// The namespace URI it stands for.
    pub uri: String,
}

/// A variable binding, from `<sch:let>`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Let {
    /// The variable name, referenced as `$name`.
    pub name: String,
    /// What it binds to.
    pub value: LetValue,
}

/// The two forms a `let` value can take.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LetValue {
    /// `<let name="x" value="expr"/>` — binds the XPath value of `expr`.
    Expression(String),
    /// `<let name="x">…</let>` — binds a string built from rich content.
    Content(Vec<Content>),
}

/// A validation phase, from `<sch:phase>`.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Phase {
    /// The phase identifier, named by `--phase` or `ValidateOptions`.
    pub id: String,
    /// The `pattern` ids this phase activates.
    pub actives: Vec<String>,
    /// Phase-scoped variables.
    pub lets: Vec<Let>,
    /// Human-readable annotations.
    pub paragraphs: Vec<Paragraph>,
}

/// A group of competing rules, from `<sch:pattern>`.
///
/// The pattern is the unit of rule competition: within one pattern a node is
/// processed by at most one rule. See `spec/validation.md`.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Pattern {
    /// The pattern identifier.
    pub id: Option<String>,
    /// `abstract="true"`: a template, never run directly.
    pub is_abstract: bool,
    /// `is-a="ID"`: an instance of an abstract pattern.
    pub is_a: Option<String>,
    /// `@documents`: an XPath yielding URIs of external documents to validate.
    pub documents: Option<String>,
    /// The pattern's title.
    pub title: Option<String>,
    /// Pattern-scoped variables.
    pub lets: Vec<Let>,
    /// The rules, in the order they compete.
    pub rules: Vec<Rule>,
    /// Arguments, on an `is-a` instance pattern.
    pub params: Vec<Param>,
    /// Human-readable annotations.
    pub paragraphs: Vec<Paragraph>,
}

/// An argument to an abstract pattern, from `<sch:param>`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Param {
    /// The name substituted for, written `$name` in the abstract pattern.
    pub name: String,
    /// The replacement text.
    pub value: String,
}

/// A context and the assertions that apply to it, from `<sch:rule>`.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Rule {
    /// The XSLT pattern selecting the nodes this rule applies to.
    ///
    /// Absent exactly when the rule is abstract.
    pub context: Option<String>,
    /// The rule identifier; required for an abstract rule.
    pub id: Option<String>,
    /// `abstract="true"`: a fragment to be spliced in by `extends`.
    pub is_abstract: bool,
    /// A label inherited by assertions that do not set their own.
    pub flag: Option<String>,
    /// A label describing the rule's role.
    pub role: Option<String>,
    /// An XPath naming the node the rule is about, when not the context node.
    pub subject: Option<String>,
    /// Rule-scoped variables.
    pub lets: Vec<Let>,
    /// The assertions and `extends` references, in document order.
    ///
    /// Order is preserved because it determines report order, and because an
    /// `extends` splices its assertions in at its own position.
    pub body: Vec<RuleChild>,
}

impl Rule {
    /// The assertions of this rule, in order.
    ///
    /// After compilation every `extends` has been expanded away, so this
    /// yields the rule's complete assertion list.
    pub fn assertions(&self) -> impl Iterator<Item = &Assertion> {
        self.body.iter().filter_map(|child| match child {
            RuleChild::Assertion(assertion) => Some(assertion),
            RuleChild::Extends(_) => None,
        })
    }
}

/// One child of a rule, in document order.
// `Assertion` is much wider than the `Extends` string, but boxing it would add
// an indirection to the hot path of validation in order to save bytes in a
// structure that exists once per schema, not once per node.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RuleChild {
    /// An `assert` or `report`.
    Assertion(Assertion),
    /// An `<sch:extends rule="ID"/>`, resolved during compilation.
    Extends(String),
}

/// Whether an assertion states a requirement or an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AssertionKind {
    /// `<assert>`: reported when its test is **false**.
    Assert,
    /// `<report>`: reported when its test is **true**.
    Report,
}

impl AssertionKind {
    /// The element name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            AssertionKind::Assert => "assert",
            AssertionKind::Report => "report",
        }
    }

    /// Whether a test result of `outcome` means this assertion is reported.
    ///
    /// This one line is the entire difference between the two elements.
    #[must_use]
    pub const fn is_reported(self, outcome: bool) -> bool {
        match self {
            AssertionKind::Assert => !outcome,
            AssertionKind::Report => outcome,
        }
    }
}

/// An `<sch:assert>` or `<sch:report>`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Assertion {
    /// Which of the two elements this is.
    pub kind: AssertionKind,
    /// The XPath test.
    pub test: String,
    /// The assertion identifier.
    pub id: Option<String>,
    /// A label, conventionally a severity; overrides the rule's.
    pub flag: Option<String>,
    /// A label describing the assertion's role; overrides the rule's.
    pub role: Option<String>,
    /// An XPath naming the node the assertion is about.
    pub subject: Option<String>,
    /// `diagnostic` ids to attach to the report.
    pub diagnostics: Vec<String>,
    /// `property` ids to attach to the report.
    pub properties: Vec<String>,
    /// A URI for further reading.
    pub see: Option<String>,
    /// A URI of an icon.
    pub icon: Option<String>,
    /// A formal public identifier.
    pub fpi: Option<String>,
    /// The human-readable message, as rich content.
    pub content: Vec<Content>,
}

/// A fragment of human-readable content.
///
/// The mixed content of `assert`, `report`, `diagnostic`, `title`, and `p` is
/// a sequence of these, instantiated against the firing node when a report is
/// produced.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Content {
    /// Literal text.
    Text(String),
    /// `<value-of select="…"/>`: the string value of an XPath result.
    ValueOf {
        /// The expression to evaluate.
        select: String,
    },
    /// `<name/>` or `<name path="…"/>`: a node's qualified name.
    Name {
        /// The node to name; the context node when absent.
        path: Option<String>,
    },
    /// `<emph>`: emphasised content.
    Emph(Vec<Content>),
    /// `<span>`: content with a class.
    Span {
        /// The `@class` value.
        class: Option<String>,
        /// The content.
        content: Vec<Content>,
    },
    /// `<dir>`: content with a text direction.
    Dir {
        /// `ltr` or `rtl`.
        value: Option<String>,
        /// The content.
        content: Vec<Content>,
    },
}

/// A reusable explanation, from `<sch:diagnostic>`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Diagnostic {
    /// The identifier referenced by `assert/@diagnostics`.
    pub id: String,
    /// The natural language of the text.
    pub lang: Option<String>,
    /// The message, as rich content.
    pub content: Vec<Content>,
}

/// A machine-oriented value, from `<sch:property>`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Property {
    /// The identifier referenced by `assert/@properties`.
    pub id: String,
    /// A label describing the property's role.
    pub role: Option<String>,
    /// The naming scheme the value belongs to.
    pub scheme: Option<String>,
    /// The value, as rich content.
    pub content: Vec<Content>,
}

/// A human-readable paragraph, from `<sch:p>`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Paragraph {
    /// The paragraph identifier.
    pub id: Option<String>,
    /// A class label.
    pub class: Option<String>,
    /// A URI of an icon.
    pub icon: Option<String>,
    /// The text, as rich content.
    pub content: Vec<Content>,
}

/// The parsed form of a whole schema document.
///
/// This is the schema as written, after includes are resolved and abstract
/// patterns and rules are expanded, but before XPath expressions are parsed.
/// [`Schema`](crate::Schema) wraps it together with the compiled expressions.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SchemaModel {
    /// The schema identifier.
    pub id: Option<String>,
    /// The schema's title.
    pub title: Option<String>,
    /// The schema author's own version string, opaque to the processor.
    pub schema_version: Option<String>,
    /// The phase to run when the caller names none.
    pub default_phase: Option<String>,
    /// The query language binding.
    pub query_binding: QueryBinding,
    /// The natural language of the human-readable text.
    pub lang: Option<String>,
    /// Namespace prefix bindings for every XPath expression in the schema.
    pub namespaces: Vec<Ns>,
    /// Schema-scoped variables.
    pub lets: Vec<Let>,
    /// The phases.
    pub phases: Vec<Phase>,
    /// The patterns, in the order they run.
    pub patterns: Vec<Pattern>,
    /// The reusable diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// The reusable properties.
    pub properties: Vec<Property>,
    /// Human-readable annotations.
    pub paragraphs: Vec<Paragraph>,
}

impl SchemaModel {
    /// Finds a pattern by identifier.
    #[must_use]
    pub fn pattern(&self, id: &str) -> Option<&Pattern> {
        self.patterns
            .iter()
            .find(|p| p.id.as_deref() == Some(id))
    }

    /// Finds a phase by identifier.
    #[must_use]
    pub fn phase(&self, id: &str) -> Option<&Phase> {
        self.phases.iter().find(|p| p.id == id)
    }

    /// Finds a diagnostic by identifier.
    #[must_use]
    pub fn diagnostic(&self, id: &str) -> Option<&Diagnostic> {
        self.diagnostics.iter().find(|d| d.id == id)
    }

    /// Finds a property by identifier.
    #[must_use]
    pub fn property(&self, id: &str) -> Option<&Property> {
        self.properties.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_reports_on_false_and_report_on_true() {
        assert!(AssertionKind::Assert.is_reported(false));
        assert!(!AssertionKind::Assert.is_reported(true));
        assert!(AssertionKind::Report.is_reported(true));
        assert!(!AssertionKind::Report.is_reported(false));
    }

    #[test]
    fn query_binding_recognises_the_supported_bindings() {
        for binding in ["xslt", "xpath", "xslt2", "xpath2"] {
            assert!(
                QueryBinding::parse(binding).is_supported(),
                "{binding} should be supported"
            );
        }
        assert!(QueryBinding::Default.is_supported());
    }

    #[test]
    fn bindings_above_xpath_two_are_still_refused() {
        // Accepting them would overclaim: XPath 3.0 adds more than this
        // crate implements, and a wrong answer is worse than a refusal.
        for binding in ["xslt3", "xpath3", "xpath31"] {
            assert!(
                !QueryBinding::parse(binding).is_supported(),
                "{binding} should be refused"
            );
        }
    }

    #[test]
    fn the_binding_selects_the_xpath_version() {
        assert_eq!(QueryBinding::Default.version(), XPathVersion::V1);
        assert_eq!(QueryBinding::parse("xslt").version(), XPathVersion::V1);
        assert_eq!(QueryBinding::parse("xpath").version(), XPathVersion::V1);
        assert_eq!(QueryBinding::parse("xslt2").version(), XPathVersion::V2);
        assert_eq!(QueryBinding::parse("xpath2").version(), XPathVersion::V2);
    }

    #[test]
    fn query_binding_is_case_insensitive() {
        assert_eq!(QueryBinding::parse("XSLT"), QueryBinding::Xslt);
    }

    #[test]
    fn rule_assertions_skips_unexpanded_extends() {
        let rule = Rule {
            body: vec![
                RuleChild::Extends("other".into()),
                RuleChild::Assertion(Assertion {
                    kind: AssertionKind::Assert,
                    test: "true()".into(),
                    id: None,
                    flag: None,
                    role: None,
                    subject: None,
                    diagnostics: vec![],
                    properties: vec![],
                    see: None,
                    icon: None,
                    fpi: None,
                    content: vec![],
                }),
            ],
            ..Rule::default()
        };
        assert_eq!(rule.assertions().count(), 1);
    }
}
