//! Integration tests for the XPath 2.0 phase-1 subset.
//!
//! Two properties matter here, and they pull in opposite directions.
//!
//! The additions must **work** under an XPath 2.0 query binding: regular
//! expressions, conditionals, and the string and numeric functions that
//! schemas declaring `xslt2` actually reach for.
//!
//! And everything outside the subset must be a **hard error naming the
//! construct** — never a wrong answer. That is what makes a partial
//! implementation of a different language honest rather than dangerous. See
//! `spec/xpath2.md`.

use assertables::*;
use schematron::{Document, Schema};

/// Wraps a body in a schema with the given query binding.
fn schema_with(binding: &str, body: &str) -> String {
    format!(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="{binding}">{body}</schema>"#
    )
}

/// Compiles an `xslt2` schema with one assertion, and validates `document`.
fn check(test: &str, document: &str) -> bool {
    let source = schema_with(
        "xslt2",
        &format!(r#"<pattern><rule context="a"><assert test="{test}">failed</assert></rule></pattern>"#),
    );
    let schema = Schema::from_str(&source)
        .unwrap_or_else(|e| panic!("schema with test {test:?} should compile: {e}"));
    let document = Document::from_str(document).expect("document should parse");
    schema.validate(&document).expect("validation should run").is_valid()
}

/// The compile error for a test under a binding, if it does not compile.
fn compile_error(binding: &str, test: &str) -> String {
    let source = schema_with(
        binding,
        &format!(r#"<pattern><rule context="a"><assert test="{test}">m</assert></rule></pattern>"#),
    );
    Schema::from_str(&source).map_or_else(
        |error| error.to_string(),
        |_| panic!("test {test:?} unexpectedly compiled under {binding}"),
    )
}

#[test]
fn matches_tests_a_regular_expression() {
    assert!(check("matches(@x, '^[0-9]+$')", r#"<a x="123"/>"#));
    assert!(!check("matches(@x, '^[0-9]+$')", r#"<a x="12a"/>"#));
    // Unanchored by default, as XPath 2.0 specifies.
    assert!(check("matches(@x, '[0-9]')", r#"<a x="ab1cd"/>"#));
}

#[test]
fn matches_honours_the_flags_argument() {
    assert!(!check("matches(@x, '^abc$')", r#"<a x="ABC"/>"#));
    assert!(check("matches(@x, '^abc$', 'i')", r#"<a x="ABC"/>"#));
}

#[test]
fn replace_rewrites_with_group_references() {
    assert!(check(
        "replace(@x, '([0-9]+)-([0-9]+)', '$2-$1') = '34-12'",
        r#"<a x="12-34"/>"#
    ));
}

#[test]
fn a_conditional_chooses_a_branch() {
    let test = "if (@type = 'credit') then number(@total) &lt; 0 else number(@total) &gt;= 0";
    assert!(check(test, r#"<a type="credit" total="-5"/>"#));
    assert!(!check(test, r#"<a type="credit" total="5"/>"#));
    assert!(check(test, r#"<a type="debit" total="5"/>"#));
    assert!(!check(test, r#"<a type="debit" total="-5"/>"#));
}

#[test]
fn a_conditional_evaluates_only_the_taken_branch() {
    // The untaken branch references an unbound variable, which would be an
    // error if it were evaluated.
    assert!(check("if (true()) then true() else $missing", "<a/>"));
    assert!(check("if (false()) then $missing else true()", "<a/>"));
}

#[test]
fn string_functions() {
    assert!(check("upper-case(@x) = 'ABC'", r#"<a x="abc"/>"#));
    assert!(check("lower-case(@x) = 'abc'", r#"<a x="ABC"/>"#));
    assert!(check("ends-with(@x, '.pdf')", r#"<a x="doc.pdf"/>"#));
    assert!(!check("ends-with(@x, '.pdf')", r#"<a x="doc.txt"/>"#));
}

#[test]
fn string_join_concatenates_a_node_set_in_document_order() {
    assert!(check(
        "string-join(b, ',') = 'x,y,z'",
        "<a><b>x</b><b>y</b><b>z</b></a>"
    ));
}

#[test]
fn numeric_functions() {
    assert!(check("abs(number(@x)) = 5", r#"<a x="-5"/>"#));
    assert!(check("min(b) = 1", "<a><b>3</b><b>1</b><b>2</b></a>"));
    assert!(check("max(b) = 3", "<a><b>3</b><b>1</b><b>2</b></a>"));
    assert!(check("avg(b) = 2", "<a><b>3</b><b>1</b><b>2</b></a>"));
}

#[test]
fn exists_and_empty_are_opposites() {
    assert!(check("exists(b)", "<a><b/></a>"));
    assert!(!check("exists(b)", "<a/>"));
    assert!(check("empty(b)", "<a/>"));
    assert!(!check("empty(b)", "<a><b/></a>"));
}

#[test]
fn two_point_zero_functions_are_refused_under_a_one_point_zero_binding() {
    for binding in ["xslt", "xpath"] {
        let message = compile_error(binding, "matches(@x, 'y')");
        assert_contains!(message, "matches()");
        assert_contains!(message, "XPath 2.0");
        // The message must say what to do about it.
        assert_contains!(message, "xslt2");
    }
}

#[test]
fn a_conditional_is_refused_under_a_one_point_zero_binding() {
    let message = compile_error("xslt", "if (b) then 1 else 2");
    assert_contains!(message, "XPath 2.0 syntax");
    assert_contains!(message, "xslt2");
}

#[test]
fn a_conditional_requires_both_branches() {
    let message = compile_error("xslt2", "if (b) then 1");
    assert_contains!(message, "else");
}

#[test]
fn constructs_needing_sequences_say_so() {
    for (test, expected) in [
        ("count(tokenize(@x, ','))", "sequence"),
        ("count(distinct-values(b))", "sequence"),
    ] {
        let message = compile_error("xslt2", test);
        assert_contains!(message, expected);
        assert_contains!(message, "spec/xpath2.md");
    }
}

#[test]
fn constructs_needing_dates_say_so() {
    let message = compile_error("xslt2", "@d &lt; current-date()");
    assert_contains!(message, "date and time");
    assert_contains!(message, "spec/xpath2.md");
}

#[test]
fn a_malformed_regular_expression_fails_when_the_schema_loads() {
    // Not part-way through validating somebody's document.
    let message = compile_error("xslt2", "matches(@x, '[unclosed')");
    assert_contains!(message, "did not compile");
    assert_contains!(message, "[unclosed");
}

#[test]
fn an_unknown_regular_expression_flag_fails_when_the_schema_loads() {
    let message = compile_error("xslt2", "matches(@x, 'y', 'q')");
    assert_contains!(message, "flag");
}

#[test]
fn a_computed_regular_expression_is_checked_at_evaluation_time() {
    // It cannot be checked earlier, so it must still be an error rather than
    // a silently false test.
    let source = schema_with(
        "xslt2",
        r#"<pattern><rule context="a"><assert test="matches('x', @pattern)">m</assert></rule></pattern>"#,
    );
    let schema = Schema::from_str(&source).expect("schema should compile");
    let document = Document::from_str(r#"<a pattern="[unclosed"/>"#).unwrap();

    let error = schema.validate(&document).unwrap_err();
    assert_contains!(error.to_string(), "did not compile");
}

#[test]
fn xpath_one_point_zero_still_works_under_a_two_point_zero_binding() {
    // The 2.0 binding adds to the language; it does not remove anything.
    assert!(check("count(b) = 2", "<a><b/><b/></a>"));
    assert!(check("b/@x = '1'", r#"<a><b x="1"/></a>"#));
    assert!(check("normalize-space(@x) = 'a b'", r#"<a x="  a   b "/>"#));
    assert!(check("substring(@x, 2, 3) = 'bcd'", r#"<a x="abcde"/>"#));
}

#[test]
fn bindings_above_two_point_zero_are_still_refused() {
    for binding in ["xslt3", "xpath3", "xpath31"] {
        let message = compile_error(binding, "true()");
        assert_contains!(message, "unsupported query binding");
    }
}

#[test]
fn the_documented_divergence_from_real_xpath_two_holds() {
    // spec/xpath2.md states that where XPath 2.0 raises a type error, this
    // crate produces NaN and the test is simply false. Asserting it here
    // means the documentation cannot drift away from the behaviour.
    assert!(!check("number(@x) + 1 &gt; 0", r#"<a x="not-a-number"/>"#));
    assert!(!check("'x' &gt; 0", "<a/>"));
}
