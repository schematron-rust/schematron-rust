//! SVRL — Schematron Validation Report Language.
//!
//! SVRL is the standard XML vocabulary for a Schematron validation report.
//! Emitting it is what makes this crate's output consumable by existing
//! Schematron tooling. See `spec/svrl/`.
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

use crate::validate::{
    ActivePattern, AssertionResult, DiagnosticResult, FiredRule, PropertyResult, Report,
    ResultKind,
};
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

impl Report {
    /// Parses an SVRL document into a report.
    ///
    /// SVRL is flat — `active-pattern`, `fired-rule` and the findings are all
    /// siblings — so this rebuilds the tree the writer flattened: each rule
    /// belongs to the most recent pattern, each finding to the most recent
    /// rule.
    ///
    /// # Errors
    ///
    /// Returns [`Error::XmlParse`](crate::Error::XmlParse) if the input is not
    /// well-formed XML, and [`Error::Schema`](crate::Error::Schema) if it is
    /// not an SVRL document.
    ///
    /// # Examples
    ///
    /// ```
    /// use schematron::{Document, Report, Schema};
    ///
    /// let schema = Schema::from_str(r#"
    ///     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
    ///       <pattern><rule context="a"><assert test="b">Needs a b.</assert></rule></pattern>
    ///     </schema>
    /// "#)?;
    /// let original = schema.validate(&Document::from_str("<a/>")?)?;
    ///
    /// let parsed = Report::from_svrl(&original.to_svrl())?;
    /// assert_eq!(parsed.count_failures(), 1);
    /// assert_eq!(parsed.failures().next().unwrap().text, "Needs a b.");
    /// # Ok::<(), schematron::Error>(())
    /// ```
    pub fn from_svrl(source: &str) -> crate::Result<Report> {
        let document = crate::xml::Document::from_str(source)?;
        let root = document.document_element().ok_or_else(|| {
            crate::Error::schema("schematron-output", None, "the document has no root element")
        })?;

        let is_svrl = |node| {
            document.name(node).is_some_and(|name| {
                name.uri.as_deref() == Some(SVRL_NAMESPACE)
            })
        };
        if !is_svrl(root) || document.name(root).map(|n| n.local.as_str()) != Some("schematron-output")
        {
            return Err(crate::Error::schema(
                document
                    .name(root)
                    .map_or_else(String::new, crate::xml::QName::display_name),
                None,
                format!(
                    "the root element is not <svrl:schematron-output> in namespace                      {SVRL_NAMESPACE}"
                ),
            ));
        }

        let attribute = |node, wanted: &str| -> Option<String> {
            document
                .attributes(node)
                .iter()
                .copied()
                .find(|&a| {
                    document
                        .name(a)
                        .is_some_and(|n| n.uri.is_none() && n.local == wanted)
                })
                .map(|a| document.value(a).to_string())
        };

        let mut report = Report {
            title: attribute(root, "title"),
            phase: attribute(root, "phase"),
            schema_version: attribute(root, "schemaVersion"),
            namespaces: Vec::new(),
            patterns: Vec::new(),
        };

        for child in document.children(root).iter().copied().filter(|&n| is_svrl(n)) {
            let local = document
                .name(child)
                .map_or_else(String::new, |name| name.local.clone());

            match local.as_str() {
                "ns-prefix-in-attribute-values" => {
                    report.namespaces.push(crate::schema::Ns {
                        prefix: attribute(child, "prefix").unwrap_or_default(),
                        uri: attribute(child, "uri").unwrap_or_default(),
                    });
                }

                "active-pattern" => report.patterns.push(ActivePattern {
                    id: attribute(child, "id"),
                    name: attribute(child, "name"),
                    documents: attribute(child, "documents"),
                    rules: Vec::new(),
                }),

                "fired-rule" => {
                    let pattern = last_pattern(&mut report);
                    pattern.rules.push(FiredRule {
                        id: attribute(child, "id"),
                        context: attribute(child, "context").unwrap_or_default(),
                        role: attribute(child, "role"),
                        flag: attribute(child, "flag"),
                        // SVRL has nowhere to record which node the rule fired
                        // on; see `spec/svrl/`.
                        location: String::new(),
                        assertions: Vec::new(),
                    });
                }

                "failed-assert" | "successful-report" => {
                    let kind = if local == "failed-assert" {
                        ResultKind::FailedAssert
                    } else {
                        ResultKind::SuccessfulReport
                    };
                    let finding = read_finding(&document, child, kind);
                    last_rule(&mut report).assertions.push(finding);
                }

                _ => {}
            }
        }
        Ok(report)
    }
}

/// The text of the first `svrl:text` child, which is where every message
/// lives.
fn svrl_text(document: &crate::xml::Document, node: crate::xml::NodeId) -> String {
    let nested = document
        .children(node)
        .iter()
        .copied()
        .find(|&child| {
            document.name(child).is_some_and(|name| {
                name.uri.as_deref() == Some(SVRL_NAMESPACE) && name.local == "text"
            })
        })
        .map(|child| document.string_value(child));
    if let Some(nested) = nested {
        return nested;
    }

    // No `svrl:text` child. The ISO reference implementation writes the
    // message of a `diagnostic-reference` as bare character data instead, and
    // it is much the most common producer of SVRL, so a reader that returns
    // nothing here cannot read the output of the tool it is measured against.
    //
    // Only the element's *own* text nodes count, never its descendants': a
    // `failed-assert` holds its diagnostic references as children, and
    // sweeping the whole subtree would fold their text into the message.
    document
        .children(node)
        .iter()
        .copied()
        .filter(|&child| document.kind(child) == crate::xml::NodeKind::Text)
        .map(|child| document.string_value(child))
        .collect::<String>()
}

/// An attribute in no namespace, which is how SVRL spells all of its own.
fn svrl_attribute(
    document: &crate::xml::Document,
    node: crate::xml::NodeId,
    wanted: &str,
) -> Option<String> {
    document
        .attributes(node)
        .iter()
        .copied()
        .find(|&a| {
            document
                .name(a)
                .is_some_and(|n| n.uri.is_none() && n.local == wanted)
        })
        .map(|a| document.value(a).to_string())
}

/// Reads one `failed-assert` or `successful-report`, with its references.
fn read_finding(
    document: &crate::xml::Document,
    node: crate::xml::NodeId,
    kind: ResultKind,
) -> AssertionResult {
    let is_svrl = |child| {
        document
            .name(child)
            .is_some_and(|name| name.uri.as_deref() == Some(SVRL_NAMESPACE))
    };

    let mut diagnostics = Vec::new();
    let mut properties = Vec::new();
    for reference in document.children(node).iter().copied().filter(|&n| is_svrl(n)) {
        let name = document
            .name(reference)
            .map_or_else(String::new, |n| n.local.clone());
        match name.as_str() {
            "diagnostic-reference" => diagnostics.push(DiagnosticResult {
                id: svrl_attribute(document, reference, "diagnostic").unwrap_or_default(),
                text: svrl_text(document, reference),
            }),
            "property-reference" => properties.push(PropertyResult {
                id: svrl_attribute(document, reference, "property").unwrap_or_default(),
                role: svrl_attribute(document, reference, "role"),
                scheme: svrl_attribute(document, reference, "scheme"),
                text: svrl_text(document, reference),
            }),
            _ => {}
        }
    }

    AssertionResult {
        kind,
        test: svrl_attribute(document, node, "test").unwrap_or_default(),
        location: svrl_attribute(document, node, "location").unwrap_or_default(),
        text: svrl_text(document, node),
        id: svrl_attribute(document, node, "id"),
        role: svrl_attribute(document, node, "role"),
        flag: svrl_attribute(document, node, "flag"),
        see: svrl_attribute(document, node, "see"),
        icon: svrl_attribute(document, node, "icon"),
        fpi: svrl_attribute(document, node, "fpi"),
        diagnostics,
        properties,
    }
}

/// The pattern a `fired-rule` belongs to: the most recent one.
///
/// A rule before any pattern gets a synthetic one, so that reading a partial
/// or hand-written document loses nothing.
fn last_pattern(report: &mut Report) -> &mut ActivePattern {
    if report.patterns.is_empty() {
        report.patterns.push(ActivePattern {
            id: None,
            name: None,
            documents: None,
            rules: Vec::new(),
        });
    }
    report
        .patterns
        .last_mut()
        .expect("just ensured non-empty")
}

/// The rule a finding belongs to: the most recent one.
///
/// A finding before any `fired-rule` — which is what `--svrl-findings-only`
/// output looks like — gets a synthetic rule for the same reason.
fn last_rule(report: &mut Report) -> &mut FiredRule {
    let pattern = last_pattern(report);
    if pattern.rules.is_empty() {
        pattern.rules.push(FiredRule {
            id: None,
            context: String::new(),
            role: None,
            flag: None,
            location: String::new(),
            assertions: Vec::new(),
        });
    }
    pattern.rules.last_mut().expect("just ensured non-empty")
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

    /// SVRL exactly as the ISO reference implementation writes it: the
    /// diagnostic message is bare character data, not a nested `svrl:text`.
    #[test]
    fn reads_a_diagnostic_written_the_reference_way() {
        let svrl = r#"<svrl:schematron-output xmlns:svrl="http://purl.oclc.org/dsdl/svrl">
  <svrl:failed-assert test="@qty" location="/order/line">
    <svrl:text>Needs a qty.</svrl:text>
    <svrl:diagnostic-reference diagnostic="qty-help">
Quantity is a positive count of units.</svrl:diagnostic-reference>
  </svrl:failed-assert>
</svrl:schematron-output>"#;
        let report = Report::from_svrl(svrl).expect("reference-shaped SVRL should parse");
        let assertion = report.assertions().next().expect("one assertion");

        // The message itself must not absorb the diagnostic's text.
        assert_eq!(assertion.text.trim(), "Needs a qty.");
        assert_eq!(assertion.diagnostics.len(), 1);
        assert_eq!(assertion.diagnostics[0].id, "qty-help");
        assert_eq!(
            assertion.diagnostics[0].text.trim(),
            "Quantity is a positive count of units."
        );
    }
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
        assert!(output.contains("location=\"/order[1]/line[1]\""), "{output}");
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
    fn reads_a_hand_written_document() {
        let report = Report::from_svrl(
            r#"<svrl:schematron-output xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                        title="T" phase="p" schemaVersion="2">
                 <svrl:ns-prefix-in-attribute-values prefix="ex" uri="urn:example"/>
                 <svrl:active-pattern id="one" name="One"/>
                 <svrl:fired-rule id="r" context="a" role="structure" flag="warning"/>
                 <svrl:failed-assert location="/a[1]" test="b" id="i" flag="error">
                   <svrl:text>message</svrl:text>
                   <svrl:diagnostic-reference diagnostic="d">
                     <svrl:text>detail</svrl:text>
                   </svrl:diagnostic-reference>
                 </svrl:failed-assert>
               </svrl:schematron-output>"#,
        )
        .unwrap();

        assert_eq!(report.title.as_deref(), Some("T"));
        assert_eq!(report.phase.as_deref(), Some("p"));
        assert_eq!(report.schema_version.as_deref(), Some("2"));
        assert_eq!(report.namespaces.len(), 1);
        assert_eq!(report.patterns.len(), 1);
        assert_eq!(report.patterns[0].name.as_deref(), Some("One"));

        let rule = &report.patterns[0].rules[0];
        assert_eq!(rule.context, "a");
        assert_eq!(rule.flag.as_deref(), Some("warning"));

        let finding = &rule.assertions[0];
        assert_eq!(finding.kind, ResultKind::FailedAssert);
        assert_eq!(finding.text, "message");
        assert_eq!(finding.location, "/a[1]");
        assert_eq!(finding.diagnostics[0].id, "d");
        assert_eq!(finding.diagnostics[0].text, "detail");
    }

    #[test]
    fn findings_only_output_reads_back() {
        // `--svrl-findings-only` emits no fired-rule elements, so the reader
        // must attach the findings to a synthetic one rather than lose them.
        let schema = Schema::from_str(SCHEMA).unwrap();
        let document = Document::from_str("<order><line/></order>").unwrap();
        let original = schema.validate(&document).unwrap();

        let svrl = original.to_svrl_with(&SvrlOptions::findings_only());
        assert!(!svrl.contains("fired-rule"), "{svrl}");

        let parsed = Report::from_svrl(&svrl).unwrap();
        assert_eq!(parsed.count_failures(), original.count_failures());
        assert_eq!(
            parsed.failures().next().unwrap().text,
            original.failures().next().unwrap().text
        );
    }

    #[test]
    fn a_successful_report_keeps_its_kind() {
        let schema = Schema::from_str(SCHEMA).unwrap();
        let document = Document::from_str("<order><line qty='1' free='1'/></order>").unwrap();
        let original = schema.validate(&document).unwrap();

        let parsed = Report::from_svrl(&original.to_svrl()).unwrap();
        assert_eq!(parsed.reports().count(), 1);
        assert_eq!(parsed.count_failures(), 0);
        assert!(parsed.is_valid());
    }

    #[test]
    fn a_document_that_is_not_svrl_is_refused() {
        let error = Report::from_svrl("<not-svrl/>").unwrap_err();
        assert!(error.to_string().contains("svrl:schematron-output"), "{error}");

        // And one that is not XML at all.
        assert!(Report::from_svrl("{\"json\": true}").is_err());
    }

    #[test]
    fn an_empty_report_round_trips() {
        let schema = Schema::from_str(SCHEMA).unwrap();
        let document = Document::from_str("<order/>").unwrap();
        let original = schema.validate(&document).unwrap();

        let parsed = Report::from_svrl(&original.to_svrl()).unwrap();
        assert!(parsed.is_valid());
        assert_eq!(parsed.assertions().count(), 0);
    }

    #[test]
    fn escaped_markup_survives_the_round_trip() {
        let schema = Schema::from_str(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a">
                   <assert test="b &lt; 1">Use &lt;b&gt; &amp; "quotes"</assert>
                 </rule></pattern>
               </schema>"#,
        )
        .unwrap();
        let document = Document::from_str("<a><b>5</b></a>").unwrap();
        let original = schema.validate(&document).unwrap();

        let parsed = Report::from_svrl(&original.to_svrl()).unwrap();
        let finding = parsed.failures().next().unwrap();
        assert_eq!(finding.text, "Use <b> & \"quotes\"");
        assert_eq!(finding.test, "b < 1");
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
