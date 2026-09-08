//! Integration tests for streaming validation (`spec/streaming/`).
//!
//! The correctness bar is simple to state and easy to check mechanically:
//! for an eligible schema and a well-formed document, streaming validation
//! must produce the *same* findings, in the *same* order, at the *same*
//! locations, as ordinary whole-document validation. Most of this file is
//! exactly that property, exercised over a range of shapes that could
//! plausibly diverge — a rule on the document element itself, whitespace
//! and comments between records, deep descendants, rule-scoped `let`,
//! zero records — plus one test per way a schema can be *refused*, each
//! asserting the refusal names the actual construct responsible.

use assertables::*;
use schematron::validate::ValidateOptions;
use schematron::{Document, Schema};

fn schema_with(body: &str) -> String {
    format!(r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">{body}</schema>"#)
}

/// One finding, reduced to what a caller can compare across streamed and
/// full validation: its location and message, in report order. `kind` is
/// folded in as a prefix so an `assert` failure and a `report` finding
/// never compare equal by accident.
fn findings(report: &schematron::Report) -> Vec<String> {
    report
        .patterns
        .iter()
        .flat_map(|pattern| &pattern.rules)
        .flat_map(|rule| &rule.assertions)
        .map(|a| format!("{:?} {} {}", a.kind, a.location, a.text.trim()))
        .collect()
}

/// Validates `document` against `schema` both ways and asserts they agree
/// exactly — the property every other test in this file that isn't
/// specifically about *refusing* a schema ultimately reduces to.
#[track_caller]
fn assert_streaming_matches_full(schema_source: &str, document: &str) {
    let schema = Schema::from_str(schema_source).expect("schema should compile");
    schema
        .streaming_eligible()
        .unwrap_or_else(|reason| panic!("expected this schema to be streaming-eligible: {reason}"));

    let full_document = Document::from_str(document).expect("document should parse");
    let full = schema
        .validate(&full_document)
        .expect("full validation should run");

    let streamed = schema
        .validate_streaming(document.as_bytes(), &ValidateOptions::new())
        .expect("streaming validation should run");

    assert_eq!(
        findings(&streamed),
        findings(&full),
        "streaming and full validation disagreed for:\n{document}"
    );
}

#[test]
fn a_single_record_agrees_with_full_validation() {
    assert_streaming_matches_full(
        &schema_with(r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#),
        "<orders><order id='1'/></orders>",
    );
}

#[test]
fn many_records_mixed_pass_and_fail_agree_with_full_validation() {
    assert_streaming_matches_full(
        &schema_with(r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#),
        "<orders><order id='1'/><order/><order id='3'/><order/><order id='5'/></orders>",
    );
}

#[test]
fn whitespace_and_comments_between_records_do_not_affect_the_result() {
    assert_streaming_matches_full(
        &schema_with(r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#),
        "<orders>\n  <!-- first -->\n  <order id='1'/>\n\n  <order/>\n  <?pi data?>\n  <order id='3'/>\n</orders>",
    );
}

#[test]
fn zero_records_is_a_clean_empty_report_not_an_error() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#,
    ))
    .unwrap();
    let report = schema
        .validate_streaming("<orders></orders>".as_bytes(), &ValidateOptions::new())
        .unwrap();
    assert!(report.is_valid());
    assert_is_empty!(findings(&report));
}

#[test]
fn a_self_closing_record_is_the_same_as_an_explicit_open_and_close() {
    assert_streaming_matches_full(
        &schema_with(r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#),
        "<orders><order/></orders>",
    );
}

#[test]
fn a_rule_on_the_document_element_itself_fires_exactly_once() {
    // The document element (the "skeleton") is resident for every record,
    // so a rule matching it is the specific case that would refire once
    // per record under a naive implementation — see `run_pattern_scoped_to_record`.
    let schema = Schema::from_str(&schema_with(
        r#"<pattern>
            <rule context="orders"><assert test="@version">orders needs @version</assert></rule>
            <rule context="order"><assert test="@id">order needs id</assert></rule>
        </pattern>"#,
    ))
    .unwrap();
    let document = "<orders><order id='1'/><order id='2'/><order id='3'/></orders>";
    let report = schema
        .validate_streaming(document.as_bytes(), &ValidateOptions::new())
        .unwrap();
    let orders_failures: Vec<_> = findings(&report)
        .into_iter()
        .filter(|f| f.contains("orders needs"))
        .collect();
    assert_eq!(orders_failures.len(), 1, "the document-element rule fired {} times, not once", orders_failures.len());

    assert_streaming_matches_full(
        &schema_with(
            r#"<pattern>
                <rule context="orders"><assert test="@version">orders needs @version</assert></rule>
                <rule context="order"><assert test="@id">order needs id</assert></rule>
            </pattern>"#,
        ),
        document,
    );
}

#[test]
fn sibling_positions_in_locations_are_the_true_cumulative_count() {
    // The regression this guards: reusing one arena slot across records
    // means a record's parent only ever has *one* child attached when
    // `finalize_subtree` runs — a naive implementation numbers every
    // record `[1]`. See `Document::set_sibling_position`.
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#,
    ))
    .unwrap();
    let document = "<orders><order id='1'/><order/><order id='3'/><order/></orders>";
    let report = schema
        .validate_streaming(document.as_bytes(), &ValidateOptions::new())
        .unwrap();
    let locations: Vec<String> = report
        .failures()
        .map(|f| f.location.clone())
        .collect();
    assert_eq!(locations, vec!["/orders[1]/order[2]", "/orders[1]/order[4]"]);
}

#[test]
fn a_deep_descendant_rule_agrees_with_full_validation() {
    assert_streaming_matches_full(
        &schema_with(r#"<pattern><rule context="order/line"><assert test="@amount">needs amount</assert></rule></pattern>"#),
        "<orders><order><line amount='1'/><line/></order><order><line/></order></orders>",
    );
}

#[test]
fn a_rule_scoped_let_is_record_local() {
    assert_streaming_matches_full(
        &schema_with(
            r#"<pattern><rule context="order">
                <let name="total" value="sum(line/@amount)"/>
                <assert test="$total = @total">line amounts must sum to @total</assert>
            </rule></pattern>"#,
        ),
        "<orders>\
            <order total='30'><line amount='10'/><line amount='20'/></order>\
            <order total='5'><line amount='1'/></order>\
         </orders>",
    );
}

#[test]
fn max_failures_stops_early_across_records() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#,
    ))
    .unwrap();
    // Ten failing records; --max-failures=2 should mean streaming never even
    // parses the later ones, not just that the report is truncated.
    let document = format!("<orders>{}</orders>", "<order/>".repeat(10));
    let options = ValidateOptions::new().with_max_failures(2);
    let report = schema
        .validate_streaming(document.as_bytes(), &options)
        .unwrap();
    assert_eq!(report.count_failures(), 2);
}

#[test]
fn a_thousand_records_completes_and_agrees_with_full_validation() {
    let mut document = String::from("<orders>");
    for i in 1..=1000 {
        if i % 7 == 0 {
            document.push_str("<order/>");
        } else {
            document.push_str(&format!("<order id='{i}'/>"));
        }
    }
    document.push_str("</orders>");
    assert_streaming_matches_full(
        &schema_with(r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#),
        &document,
    );
}

// --- Ineligibility: each disqualifying construct is refused by name, at
// compile time, before any document is even read. ---

#[track_caller]
fn assert_ineligible(schema_source: &str, expected_substring: &str) {
    let schema = Schema::from_str(schema_source).expect("schema should compile");
    let reason = schema
        .streaming_eligible()
        .expect_err("expected this schema to be streaming-ineligible");
    assert_contains!(reason, expected_substring);

    // The same reason surfaces from `validate_streaming` itself, without
    // reading whatever document was passed.
    let err = schema
        .validate_streaming("<a><b/></a>".as_bytes(), &ValidateOptions::new())
        .expect_err("validate_streaming should also refuse it");
    assert_contains!(err.to_string(), expected_substring);
}

#[test]
fn a_declared_key_is_ineligible() {
    assert_ineligible(
        &schema_with(r#"<key name="k" match="a" use="@id"/><pattern><rule context="a"><assert test="1">x</assert></rule></pattern>"#),
        "<key>",
    );
}

#[test]
fn key_function_is_ineligible() {
    assert_ineligible(
        &schema_with(r#"<pattern><rule context="a"><assert test="key('k', @id)">x</assert></rule></pattern>"#),
        "key()",
    );
}

#[test]
fn id_function_is_ineligible() {
    assert_ineligible(
        &schema_with(r#"<pattern><rule context="a"><assert test="id('x')">x</assert></rule></pattern>"#),
        "id()",
    );
}

#[test]
fn document_function_is_ineligible() {
    assert_ineligible(
        &schema_with(r#"<pattern><rule context="a"><assert test="document('x.xml')">x</assert></rule></pattern>"#),
        "document()",
    );
}

#[test]
fn following_axis_is_ineligible() {
    assert_ineligible(
        &schema_with(r#"<pattern><rule context="a"><assert test="following::b">x</assert></rule></pattern>"#),
        "following:: axis",
    );
}

#[test]
fn preceding_axis_is_ineligible() {
    assert_ineligible(
        &schema_with(r#"<pattern><rule context="a"><assert test="preceding::b">x</assert></rule></pattern>"#),
        "preceding:: axis",
    );
}

#[test]
fn following_sibling_axis_is_ineligible() {
    assert_ineligible(
        &schema_with(r#"<pattern><rule context="a"><assert test="following-sibling::b">x</assert></rule></pattern>"#),
        "following-sibling:: axis",
    );
}

#[test]
fn a_following_axis_hidden_in_a_predicate_is_still_caught() {
    // `check_match_pattern` only restricts a rule context's own top-level
    // steps; an unsafe axis inside a *predicate* is legal XSLT and must
    // still be caught by walking into predicates, the way
    // `calls_document_function` already does for `document()`.
    assert_ineligible(
        &schema_with(r#"<pattern><rule context="a[preceding::b]"><assert test="1">x</assert></rule></pattern>"#),
        "preceding:: axis",
    );
}

#[test]
fn a_schema_scoped_let_is_ineligible() {
    assert_ineligible(
        &schema_with(r#"<let name="x" value="1"/><pattern><rule context="a"><assert test="1">x</assert></rule></pattern>"#),
        "schema-scoped <let>",
    );
}

#[test]
fn a_phase_scoped_let_is_ineligible() {
    assert_ineligible(
        &schema_with(
            r#"<phase id="p"><let name="x" value="1"/></phase>
               <pattern><rule context="a"><assert test="1">x</assert></rule></pattern>"#,
        ),
        "phase-scoped <let>",
    );
}

#[test]
fn a_pattern_scoped_let_is_ineligible() {
    assert_ineligible(
        &schema_with(
            r#"<pattern><let name="x" value="1"/><rule context="a"><assert test="1">x</assert></rule></pattern>"#,
        ),
        "pattern-scoped <let>",
    );
}

#[test]
fn a_documents_attribute_is_ineligible() {
    assert_ineligible(
        &schema_with(
            r#"<pattern documents="'x.xml'"><rule context="a"><assert test="1">x</assert></rule></pattern>"#,
        ),
        "@documents",
    );
}

#[test]
fn an_eligible_schema_reports_no_reason() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#,
    ))
    .unwrap();
    assert!(schema.streaming_eligible().is_ok());
}

#[test]
fn parallel_evaluation_is_refused_together_with_streaming() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#,
    ))
    .unwrap();
    let options = ValidateOptions::new().with_parallel_patterns(true);
    let err = schema
        .validate_streaming("<orders><order id='1'/></orders>".as_bytes(), &options)
        .expect_err("streaming + parallel should be refused");
    assert_contains!(err.to_string(), "parallel");
}

#[test]
fn a_document_with_no_element_children_to_stream_is_a_clean_error() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="order"><assert test="@id">needs id</assert></rule></pattern>"#,
    ))
    .unwrap();
    let err = schema
        .validate_streaming("not xml".as_bytes(), &ValidateOptions::new())
        .expect_err("malformed input should still be a clean error, never a panic");
    assert_contains!(err.to_string(), "XML parse error");
}
