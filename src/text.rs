//! Human-readable report output.
//!
//! A person reading a validation failure needs three things: where it is,
//! what the rule said, and — when the message alone is not enough — which
//! test failed. This renderer gives those in that order, and keeps the
//! machine-oriented detail out of the way.

use crate::validate::{Report, ResultKind};

/// How to render a report as text.
// Independent on/off switches are what an options struct is; grouping them
// into an enum would force callers to spell out combinations they do not care
// about.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct TextOptions {
    /// Include successful reports, not only failures.
    pub include_reports: bool,
    /// Include the rules that fired without finding anything.
    ///
    /// Worth turning on when a schema appears to do nothing: an empty list of
    /// fired rules means no context matched, which is almost always a missing
    /// `ns` prefix binding.
    pub include_fired_rules: bool,
    /// Show the XPath test alongside each finding.
    pub include_tests: bool,
    /// Show instantiated diagnostics.
    pub include_diagnostics: bool,
    /// A label for the document, printed as a heading.
    pub label: Option<String>,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            include_reports: true,
            include_fired_rules: false,
            include_tests: false,
            include_diagnostics: true,
            label: None,
        }
    }
}

impl TextOptions {
    /// Default options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything, for diagnosing a schema that is not behaving.
    #[must_use]
    pub fn verbose() -> Self {
        Self {
            include_reports: true,
            include_fired_rules: true,
            include_tests: true,
            include_diagnostics: true,
            label: None,
        }
    }

    /// Sets the document label shown as a heading.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets whether to show the XPath test for each finding.
    #[must_use]
    pub fn with_tests(mut self, include: bool) -> Self {
        self.include_tests = include;
        self
    }

    /// Sets whether to show successful reports.
    #[must_use]
    pub fn with_reports(mut self, include: bool) -> Self {
        self.include_reports = include;
        self
    }
}

impl Report {
    /// Renders this report for a person to read.
    ///
    /// # Examples
    ///
    /// ```
    /// use schematron::{Document, Schema};
    ///
    /// let schema = Schema::from_str(r#"
    ///     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
    ///       <pattern><rule context="a"><assert test="b">Needs a b.</assert></rule></pattern>
    ///     </schema>
    /// "#)?;
    /// let report = schema.validate(&Document::from_str("<a/>")?)?;
    ///
    /// let text = report.to_text();
    /// assert!(text.contains("Needs a b."));
    /// assert!(text.contains("/a[1]"));
    /// # Ok::<(), schematron::Error>(())
    /// ```
    #[must_use]
    pub fn to_text(&self) -> String {
        self.to_text_with(&TextOptions::default())
    }

    /// Renders this report for a person to read, with options.
    #[must_use]
    #[allow(clippy::too_many_lines)] // One straight-line renderer; splitting it hides the layout.
    pub fn to_text_with(&self, options: &TextOptions) -> String {
        let mut out = String::new();

        if let Some(label) = &options.label {
            out.push_str(label);
            out.push_str(":\n");
        }

        let mut failures = 0;
        let mut reports = 0;

        for pattern in &self.patterns {
            for rule in &pattern.rules {
                if options.include_fired_rules && rule.assertions.is_empty() {
                    out.push_str(&format!(
                        "  fired  {}  (rule context {})\n",
                        rule.location, rule.context
                    ));
                }
                for assertion in &rule.assertions {
                    match assertion.kind {
                        ResultKind::FailedAssert => failures += 1,
                        ResultKind::SuccessfulReport => {
                            reports += 1;
                            if !options.include_reports {
                                continue;
                            }
                        }
                    }

                    // The flag column carries severity when the schema sets
                    // one; otherwise fall back to the kind of finding.
                    let label = assertion.flag.clone().unwrap_or_else(|| {
                        match assertion.kind {
                            ResultKind::FailedAssert => "error".to_string(),
                            ResultKind::SuccessfulReport => "report".to_string(),
                        }
                    });

                    out.push_str(&format!("  {label:<8} {}\n", assertion.location));
                    for line in assertion.text.lines() {
                        let line = line.trim();
                        if !line.is_empty() {
                            out.push_str(&format!("           {line}\n"));
                        }
                    }
                    if options.include_tests {
                        out.push_str(&format!("           test: {}\n", assertion.test));
                        out.push_str(&format!("           rule: {}\n", rule.context));
                    }
                    if options.include_diagnostics {
                        for diagnostic in &assertion.diagnostics {
                            for line in diagnostic.text.lines() {
                                let line = line.trim();
                                if !line.is_empty() {
                                    out.push_str(&format!("           - {line}\n"));
                                }
                            }
                        }
                    }
                    if let Some(see) = &assertion.see {
                        out.push_str(&format!("           see: {see}\n"));
                    }
                }
            }
        }

        if failures == 0 && reports == 0 {
            out.push_str("  no findings\n");
            return out;
        }

        out.push_str(&format!(
            "  {} finding{}: {failures} failed assert{}, {reports} report{}\n",
            failures + reports,
            plural(failures + reports),
            plural(failures),
            plural(reports),
        ));
        out
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(feature = "serde")]
impl Report {
    /// Renders this report as JSON.
    ///
    /// Unlike SVRL, the JSON keeps the report's tree structure: patterns
    /// contain rules, which contain findings.
    ///
    /// # Errors
    ///
    /// Returns an error only if serialisation fails, which for this type
    /// means an allocation failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use schematron::{Document, Schema};
    ///
    /// let schema = Schema::from_str(r#"
    ///     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
    ///       <pattern><rule context="a"><assert test="b">Needs a b.</assert></rule></pattern>
    ///     </schema>
    /// "#)?;
    /// let report = schema.validate(&Document::from_str("<a/>")?)?;
    ///
    /// let json = report.to_json()?;
    /// assert!(json.contains("\"FailedAssert\""));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, Schema};

    const SCHEMA: &str = r#"
        <schema xmlns="http://purl.oclc.org/dsdl/schematron">
          <pattern>
            <rule context="line">
              <assert test="@qty" flag="error">Needs a qty.</assert>
              <report test="@free">Free line.</report>
            </rule>
          </pattern>
        </schema>
    "#;

    fn text(document: &str, options: &TextOptions) -> String {
        let schema = Schema::from_str(SCHEMA).unwrap();
        let document = Document::from_str(document).unwrap();
        schema.validate(&document).unwrap().to_text_with(options)
    }

    #[test]
    fn a_clean_document_says_so() {
        let output = text("<order><line qty='1'/></order>", &TextOptions::new());
        assert!(output.contains("no findings"), "{output}");
    }

    #[test]
    fn a_failure_shows_flag_location_and_message() {
        let output = text("<order><line/></order>", &TextOptions::new());
        assert!(output.contains("error"), "{output}");
        assert!(output.contains("/order[1]/line[1]"), "{output}");
        assert!(output.contains("Needs a qty."), "{output}");
    }

    #[test]
    fn the_summary_counts_failures_and_reports_separately() {
        let output = text("<order><line free='1'/></order>", &TextOptions::new());
        assert!(output.contains("1 failed assert,"), "{output}");
        assert!(output.contains("1 report"), "{output}");
    }

    #[test]
    fn verbose_shows_the_test_and_the_rule() {
        let output = text("<order><line/></order>", &TextOptions::verbose());
        assert!(output.contains("test: @qty"), "{output}");
        assert!(output.contains("rule: line"), "{output}");
    }

    #[test]
    fn reports_can_be_suppressed() {
        let options = TextOptions::new().with_reports(false);
        let output = text("<order><line qty='1' free='1'/></order>", &options);
        assert!(!output.contains("Free line."), "{output}");
    }

    #[test]
    fn a_label_becomes_a_heading() {
        let options = TextOptions::new().with_label("data.xml");
        let output = text("<order><line/></order>", &options);
        assert!(output.starts_with("data.xml:\n"), "{output}");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn json_keeps_the_tree_structure() {
        let schema = Schema::from_str(SCHEMA).unwrap();
        let document = Document::from_str("<order><line/></order>").unwrap();
        let json = schema.validate(&document).unwrap().to_json().unwrap();
        assert!(json.contains("\"patterns\""), "{json}");
        assert!(json.contains("\"FailedAssert\""), "{json}");
    }
}
