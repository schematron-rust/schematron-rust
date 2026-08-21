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
fn constructs_still_needing_phase_two_b_say_so() {
    // What phase 2a did not add still names itself rather than misbehaving.
    for (test, expected) in [
        ("count(subsequence(b, 2))", "sequence"),
        ("deep-equal(b, c)", "does not implement"),
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

// ---------------------------------------------------------------------------
// Phase 2a: the sequence type.
// ---------------------------------------------------------------------------

#[test]
fn a_sequence_can_be_constructed_and_counted() {
    assert!(check("count((1, 2, 3)) = 3", "<a/>"));
    assert!(check("count(()) = 0", "<a/>"));
    // Sequences do not nest: building one from others flattens them.
    assert!(check("count((1, (2, 3), 4)) = 4", "<a/>"));
}

#[test]
fn a_sequence_can_mix_nodes_and_atomic_values() {
    assert!(check("count((b, 'x', 1)) = 4", "<a><b/><b/></a>"));
}

#[test]
fn a_range_counts_up_and_is_empty_when_descending() {
    assert!(check("count(1 to 5) = 5", "<a/>"));
    assert!(check("count(5 to 1) = 0", "<a/>"));
    assert!(check("count(3 to 3) = 1", "<a/>"));
}

#[test]
fn an_absurd_range_is_an_error_rather_than_an_allocation() {
    let source = schema_with(
        "xslt2",
        r#"<pattern><rule context="a"><assert test="count(1 to 100000000) > 0">m</assert></rule></pattern>"#,
    );
    let schema = Schema::from_str(&source).expect("schema should compile");
    let document = Document::from_str("<a/>").unwrap();
    let error = schema.validate(&document).unwrap_err();
    assert_contains!(error.to_string(), "exceeds the limit");
}

#[test]
fn for_iterates_and_concatenates() {
    assert!(check("count(for $b in b return $b) = 3", "<a><b/><b/><b/></a>"));
    // The bound variable is a node, so a path can continue from it.
    assert!(check(
        "count(for $b in b return $b/c) = 2",
        "<a><b><c/></b><b><c/></b><b/></a>"
    ));
    // Each iteration may yield several items; the results concatenate.
    assert!(check("count(for $n in (1, 2) return (1 to 3)) = 6", "<a/>"));
}

#[test]
fn some_and_every_quantify_over_a_sequence() {
    let doc = r#"<a><b n="1"/><b n="2"/><b n="3"/></a>"#;
    assert!(check("some $b in b satisfies number($b/@n) = 2", doc));
    assert!(!check("some $b in b satisfies number($b/@n) = 9", doc));
    assert!(check("every $b in b satisfies number($b/@n) > 0", doc));
    assert!(!check("every $b in b satisfies number($b/@n) > 1", doc));
}

#[test]
fn every_is_true_for_an_empty_input_and_some_is_false() {
    // Vacuous truth, as XPath 2.0 specifies.
    assert!(check("every $b in nothing satisfies false()", "<a/>"));
    assert!(!check("some $b in nothing satisfies true()", "<a/>"));
}

#[test]
fn quantifiers_work_over_a_constructed_sequence() {
    assert!(check("some $n in (1 to 10) satisfies $n = 7", "<a/>"));
    assert!(check("every $n in (2, 4, 6) satisfies $n mod 2 = 0", "<a/>"));
}

#[test]
fn a_bound_variable_does_not_escape_its_expression() {
    // `$b` is bound only inside the quantified expression. Referencing it
    // outside must be an unbound-variable error, not a stale binding.
    let source = schema_with(
        "xslt2",
        r#"<pattern><rule context="a">
             <assert test="(every $b in b satisfies true()) and $b">m</assert>
           </rule></pattern>"#,
    );
    let schema = Schema::from_str(&source).expect("schema should compile");
    let document = Document::from_str("<a><b/></a>").unwrap();
    let error = schema.validate(&document).unwrap_err();
    assert_contains!(error.to_string(), "$b");
}

#[test]
fn tokenize_splits_into_a_sequence() {
    assert!(check("count(tokenize(@x, ',')) = 3", r#"<a x="p,q,r"/>"#));
    assert!(check(
        "string-join(tokenize(@x, ',\\s*'), '-') = 'p-q-r'",
        r#"<a x="p, q,  r"/>"#
    ));
}

#[test]
fn distinct_values_removes_duplicates_keeping_order() {
    assert!(check(
        "string-join(distinct-values((1, 2, 1, 3, 2)), ',') = '1,2,3'",
        "<a/>"
    ));
    assert!(check("count(distinct-values(b)) = 2", "<a><b>x</b><b>y</b><b>x</b></a>"));
}

#[test]
fn index_of_reports_one_based_positions() {
    assert!(check("index-of(('a', 'b', 'c'), 'b') = 2", "<a/>"));
    assert!(check("count(index-of(('a', 'b', 'a'), 'a')) = 2", "<a/>"));
    assert!(check("count(index-of(('a', 'b'), 'z')) = 0", "<a/>"));
}

#[test]
fn aggregates_accept_sequences_as_well_as_node_sets() {
    assert!(check("sum((1, 2, 3)) = 6", "<a/>"));
    assert!(check("min((3, 1, 2)) = 1", "<a/>"));
    assert!(check("max((3, 1, 2)) = 3", "<a/>"));
    assert!(check("avg((2, 4)) = 3", "<a/>"));
    assert!(check("exists((1))", "<a/>"));
    assert!(check("empty(())", "<a/>"));
}

#[test]
fn a_sequence_compares_existentially_like_a_node_set() {
    assert!(check("(1, 2, 3) = 2", "<a/>"));
    assert!(!check("(1, 2, 3) = 9", "<a/>"));
    assert!(check("(1, 2, 3) > 2", "<a/>"));
    // Both can be true at once, as for node-sets.
    assert!(check("((1, 2) = 1) and ((1, 2) != 1)", "<a/>"));
}

#[test]
fn a_multi_item_sequence_has_no_effective_boolean_value() {
    // XPath 2.0 makes this a type error; guessing a branch would be worse.
    let source = schema_with(
        "xslt2",
        r#"<pattern><rule context="a"><assert test="if ((1, 2)) then true() else false()">m</assert></rule></pattern>"#,
    );
    let schema = Schema::from_str(&source).expect("schema should compile");
    let document = Document::from_str("<a/>").unwrap();
    let error = schema.validate(&document).unwrap_err();
    assert_contains!(error.to_string(), "effective boolean value");
}

#[test]
fn sequence_syntax_is_refused_under_a_one_point_zero_binding() {
    for (test, fragment) in [
        ("count((1, 2))", "sequence"),
        ("count(1 to 3)", "`to` range"),
        ("count(for $b in b return $b)", "for"),
        ("some $b in b satisfies true()", "some"),
        ("every $b in b satisfies true()", "every"),
    ] {
        let message = compile_error("xslt", test);
        assert_contains!(message, fragment);
        assert_contains!(message, "xslt2");
    }
}

#[test]
fn keywords_are_not_reserved_words() {
    // `in`, `to`, `return` and the rest are ordinary names in XPath, so a
    // document may legitimately use them as element names.
    assert!(check("count(to) = 1", "<a><to/></a>"));
    assert!(check("count(for) = 1", "<a><for/></a>"));
    assert!(check("count(some) = 1", "<a><some/></a>"));
    // And a keyword directly followed by `(` is still the keyword.
    assert!(check("some $n in (1 to 3) satisfies $n = 2", "<a/>"));
    assert!(check("count(for $n in (1, 2) return ($n)) = 2", "<a/>"));
}
