//! SVRL — Schematron Validation Report Language.
//!
//! SVRL is the standard XML vocabulary for a Schematron validation report.
//! Emitting it is what makes this crate's output consumable by existing
//! Schematron tooling. See `spec/svrl.md`.
//!
//! The output is deliberately *flat*: `active-pattern`, `fired-rule`,
//! `failed-assert`, and `successful-report` are siblings, and the structure is
//! implied by order — every `fired-rule` belongs to the most recent
//! `active-pattern`, and so on. That is what the reference implementation
//! emits, because it is what falls out of a streaming XSLT transform, and
//! consumers depend on it. The crate's own [`Report`] keeps the tree; it is
//! flattened here on the way out.
//!
//! # Examples
//!
//! ```
//! use schematron::{Document, Schema};
//!
//! let schema = Schema::from_str(r#"
//!     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
//!       <pattern><rule context="a"><assert test="b">Needs a b.</assert></rule></pattern>
//!     </schema>
//! "#)?;
//! let report = schema.validate(&Document::from_str("<a/>")?)?;
//!
//! let svrl = report.to_svrl();
//! assert!(svrl.contains("svrl:failed-assert"));
//! assert!(svrl.contains("<svrl:text>Needs a b.</svrl:text>"));
//! # Ok::<(), schematron::Error>(())
//! ```

use crate::validate::{AssertionResult, Report, ResultKind};
use crate::xml::{escape_attribute, escape_text};

/// The SVRL namespace URI.
pub const SVRL_NAMESPACE: &str = "http://purl.oclc.org/dsdl/svrl";

/// How much of the run to write out.
#[derive(Debug, Clone)]
pub struct SvrlOptions {
    /// Emit a `fired-rule` for every node a rule matched.
    ///
    /// On by default, which is what the standard specifies. A large document
    /// can produce far more `fired-rule` elements than findings, so turning
    /// this off keeps a report readable at the cost of conformance.
    pub include_fired_rules: bool,
    /// Emit `successful-report` elements.
    pub include_successful_reports: bool,
    /// Indent the output.
    pub indent: bool,
}

impl Default for SvrlOptions {
    fn default() -> Self {
        Self {
            include_fired_rules: true,
            include_successful_reports: true,
            indent: true,
        }
    }
}

impl SvrlOptions {
    /// Standard-conformant output: every event.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Findings only: no `fired-rule` events.
    #[must_use]
    pub fn findings_only() -> Self {
        Self {
            include_fired_rules: false,
            ..Self::default()
        }
    }

    /// Sets whether to emit `fired-rule` events.
    #[must_use]
    pub fn with_fired_rules(mut self, include: bool) -> Self {
        self.include_fired_rules = include;
        self
    }

    /// Sets whether to emit `successful-report` events.
    #[must_use]
    pub fn with_successful_reports(mut self, include: bool) -> Self {
        self.include_successful_reports = include;
        self
    }

    /// Sets whether to indent.
    #[must_use]
    pub fn with_indent(mut self, indent: bool) -> Self {
        self.indent = indent;
        self
    }
}

impl Report {
    /// Renders this report as SVRL, with default options.
    #[must_use]
    pub fn to_svrl(&self) -> String {
        self.to_svrl_with(&SvrlOptions::default())
    }

    /// Renders this report as SVRL.
    #[must_use]
    pub fn to_svrl_with(&self, options: &SvrlOptions) -> String {
        Writer {
            out: String::new(),
            options,
        }
        .run(self)
    }
}

struct Writer<'a> {
    out: String,
    options: &'a SvrlOptions,
}

impl Writer<'_> {
    fn newline(&mut self, depth: usize) {
        if self.options.indent {
            self.out.push('\n');
            for _ in 0..depth {
                self.out.push_str("  ");
            }
        }
    }

    /// Writes ` name="value"`, or nothing when the value is absent.
    fn optional_attribute(&mut self, name: &str, value: Option<&str>) {
        if let Some(value) = value {
            self.out.push_str(&format!(" {name}=\"{}\"", escape_attribute(value)));
        }
    }

    fn text_element(&mut self, depth: usize, text: &str) {
        self.newline(depth);
        self.out
            .push_str(&format!("<svrl:text>{}</svrl:text>", escape_text(text)));
    }

    fn run(mut self, report: &Report) -> String {
        self.out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        self.out
            .push_str(&format!("<svrl:schematron-output xmlns:svrl=\"{SVRL_NAMESPACE}\""));
        self.optional_attribute("title", report.title.as_deref());
        self.optional_attribute("phase", report.phase.as_deref());
        self.optional_attribute("schemaVersion", report.schema_version.as_deref());
        self.out.push('>');

        // A consumer needs the schema's prefix bindings in order to interpret
        // the `@location` and `@test` attributes below.
        for ns in &report.namespaces {
            self.newline(1);
            self.out.push_str(&format!(
                "<svrl:ns-prefix-in-attribute-values prefix=\"{}\" uri=\"{}\"/>",
                escape_attribute(&ns.prefix),
                escape_attribute(&ns.uri)
            ));
        }

        for pattern in &report.patterns {
            self.newline(1);
            self.out.push_str("<svrl:active-pattern");
            self.optional_attribute("id", pattern.id.as_deref());
            self.optional_attribute("name", pattern.name.as_deref());
            self.optional_attribute("documents", pattern.documents.as_deref());
            self.out.push_str("/>");

            for rule in &pattern.rules {
                if self.options.include_fired_rules {
                    self.newline(1);
                    self.out.push_str("<svrl:fired-rule");
                    self.optional_attribute("id", rule.id.as_deref());
                    self.optional_attribute("context", Some(&rule.context));
                    self.optional_attribute("role", rule.role.as_deref());
                    self.optional_attribute("flag", rule.flag.as_deref());
                    self.out.push_str("/>");
                }
                for assertion in &rule.assertions {
                    if assertion.kind == ResultKind::SuccessfulReport
                        && !self.options.include_successful_reports
                    {
                        continue;
                    }
                    self.assertion(assertion);
                }
            }
        }

        self.out.push('\n');
        self.out.push_str("</svrl:schematron-output>");
        self.out.push('\n');
        self.out
    }

    fn assertion(&mut self, assertion: &AssertionResult) {
        let element = format!("svrl:{}", assertion.kind.as_str());
        self.newline(1);
        self.out.push_str(&format!("<{element}"));
        self.optional_attribute("id", assertion.id.as_deref());
        self.optional_attribute("location", Some(&assertion.location));
        self.optional_attribute("test", Some(&assertion.test));
        self.optional_attribute("role", assertion.role.as_deref());
        self.optional_attribute("flag", assertion.flag.as_deref());
        self.optional_attribute("see", assertion.see.as_deref());
        self.optional_attribute("icon", assertion.icon.as_deref());
        self.optional_attribute("fpi", assertion.fpi.as_deref());
        self.out.push('>');

        self.text_element(2, &assertion.text);

        for diagnostic in &assertion.diagnostics {
            self.newline(2);
            self.out.push_str(&format!(
                "<svrl:diagnostic-reference diagnostic=\"{}\">",
                escape_attribute(&diagnostic.id)
            ));
            self.text_element(3, &diagnostic.text);
            self.newline(2);
            self.out.push_str("</svrl:diagnostic-reference>");
        }

        for property in &assertion.properties {
            self.newline(2);
            self.out.push_str(&format!(
                "<svrl:property-reference property=\"{}\"",
                escape_attribute(&property.id)
            ));
            self.optional_attribute("role", property.role.as_deref());
            self.optional_attribute("scheme", property.scheme.as_deref());
            self.out.push('>');
            self.text_element(3, &property.text);
            self.newline(2);
            self.out.push_str("</svrl:property-reference>");
        }

        self.newline(1);
        self.out.push_str(&format!("</{element}>"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, Schema};

    fn svrl(schema: &str, document: &str) -> String {
        let schema = Schema::from_str(schema).unwrap();
        let document = Document::from_str(document).unwrap();
        schema.validate(&document).unwrap().to_svrl()
    }

    const SCHEMA: &str = r#"
        <schema xmlns="http://purl.oclc.org/dsdl/schematron" schemaVersion="1">
          <title>Orders</title>
          <ns prefix="p" uri="urn:p"/>
          <pattern id="lines">
            <title>Line rules</title>
            <rule context="line" flag="error" id="line-rule">
              <assert test="@qty" diagnostics="d1">Needs a qty.</assert>
              <report test="@free">Free line.</report>
            </rule>
          </pattern>
          <diagnostics><diagnostic id="d1">Add a qty attribute.</diagnostic></diagnostics>
        </schema>
    "#;

    #[test]
    fn emits_a_well_formed_document_that_reparses() {
        let output = svrl(SCHEMA, "<order><line free='1'/></order>");
        assert!(output.starts_with("<?xml"));
        // The strongest check available: feed it back to our own parser.
        let reparsed = Document::from_str(output.trim_start_matches("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(reparsed.is_ok(), "{output}");
    }

    #[test]
    fn carries_schema_metadata_on_the_root() {
        let output = svrl(SCHEMA, "<order><line qty='1'/></order>");
        assert!(output.contains("title=\"Orders\""), "{output}");
        assert!(output.contains("schemaVersion=\"1\""), "{output}");
        assert!(output.contains("ns-prefix-in-attribute-values prefix=\"p\""), "{output}");
    }

    #[test]
    fn emits_active_pattern_and_fired_rule() {
        let output = svrl(SCHEMA, "<order><line qty='1'/></order>");
        assert!(output.contains("<svrl:active-pattern id=\"lines\" name=\"Line rules\"/>"), "{output}");
        assert!(output.contains("<svrl:fired-rule id=\"line-rule\" context=\"line\" flag=\"error\"/>"), "{output}");
    }

    #[test]
    fn emits_failed_assert_with_location_and_test() {
        let output = svrl(SCHEMA, "<order><line/></order>");
        assert!(output.contains("<svrl:failed-assert"), "{output}");
        assert!(output.contains("location=\"/*:order[1]/*:line[1]\""), "{output}");
        assert!(output.contains("test=\"@qty\""), "{output}");
        assert!(output.contains("<svrl:text>Needs a qty.</svrl:text>"), "{output}");
    }

    #[test]
    fn emits_successful_report_separately() {
        let output = svrl(SCHEMA, "<order><line qty='1' free='1'/></order>");
        assert!(output.contains("<svrl:successful-report"), "{output}");
        assert!(!output.contains("<svrl:failed-assert"), "{output}");
    }

    #[test]
    fn emits_diagnostic_references() {
        let output = svrl(SCHEMA, "<order><line/></order>");
        assert!(
            output.contains("<svrl:diagnostic-reference diagnostic=\"d1\">"),
            "{output}"
        );
        assert!(output.contains("Add a qty attribute."), "{output}");
    }

    #[test]
    fn findings_only_omits_fired_rules() {
        let schema = Schema::from_str(SCHEMA).unwrap();
        let document = Document::from_str("<order><line/></order>").unwrap();
        let report = schema.validate(&document).unwrap();
        let output = report.to_svrl_with(&SvrlOptions::findings_only());
        assert!(!output.contains("fired-rule"), "{output}");
        assert!(output.contains("failed-assert"), "{output}");
    }

    #[test]
    fn escapes_markup_in_messages_and_tests() {
        let output = svrl(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a">
                   <assert test="b &lt; 1">Use &lt;b&gt; &amp; "quotes"</assert>
                 </rule></pattern>
               </schema>"#,
            "<a><b>5</b></a>",
        );
        assert!(output.contains("&lt;b&gt; &amp; \"quotes\""), "{output}");
        assert!(output.contains("test=\"b &lt; 1\""), "{output}");
    }
}
