//! Integration tests for validation semantics.
//!
//! These exercise the behaviours specified in `spec/validation/` through
//! the public API only, so they also serve as a check that the public API is
//! sufficient to do real work.

use assertables::*;
use schematron::validate::{PhaseSelection, ValidateOptions};
use schematron::{Document, Schema};

/// Compiles a schema, validates a document, returns the messages of the
/// failed assertions in order.
fn failures(schema: &str, document: &str) -> Vec<String> {
    let schema = Schema::from_str(schema).expect("schema should compile");
    let document = Document::from_str(document).expect("document should parse");
    let report = schema.validate(&document).expect("validation should run");
    report
        .failures()
        .map(|f| f.text.trim().to_string())
        .collect()
}

/// As `failures`, but for successful reports.
fn reports(schema: &str, document: &str) -> Vec<String> {
    let schema = Schema::from_str(schema).expect("schema should compile");
    let document = Document::from_str(document).expect("document should parse");
    let report = schema.validate(&document).expect("validation should run");
    report
        .reports()
        .map(|r| r.text.trim().to_string())
        .collect()
}

/// Wraps pattern bodies in a schema envelope, to keep the tests readable.
fn schema_with(body: &str) -> String {
    format!(r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">{body}</schema>"#)
}

#[test]
fn assert_fires_when_its_test_is_false() {
    let schema = schema_with(
        r#"<pattern><rule context="a"><assert test="b">missing b</assert></rule></pattern>"#,
    );
    assert_eq!(failures(&schema, "<a/>"), vec!["missing b"]);
    assert_is_empty!(failures(&schema, "<a><b/></a>"));
}

#[test]
fn report_fires_when_its_test_is_true() {
    let schema = schema_with(
        r#"<pattern><rule context="a"><report test="b">has b</report></rule></pattern>"#,
    );
    assert_eq!(reports(&schema, "<a><b/></a>"), vec!["has b"]);
    assert_is_empty!(reports(&schema, "<a/>"));
}

#[test]
fn a_successful_report_does_not_make_a_document_invalid() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="a"><report test="b">has b</report></rule></pattern>"#,
    ))
    .unwrap();
    let report = schema
        .validate(&Document::from_str("<a><b/></a>").unwrap())
        .unwrap();
    assert!(report.is_valid());
    assert_eq!(report.reports().count(), 1);
}

#[test]
fn within_a_pattern_only_the_first_matching_rule_fires() {
    // The broad rule claims every element, so the specific rule never runs.
    let schema = schema_with(
        r#"<pattern>
             <rule context="*"><assert test="false()">broad</assert></rule>
             <rule context="a"><assert test="false()">specific</assert></rule>
           </pattern>"#,
    );
    let found = failures(&schema, "<a/>");
    assert_contains!(found, &"broad".to_string());
    assert_not_contains!(found, &"specific".to_string());
}

#[test]
fn later_rules_act_as_else_branches() {
    let schema = schema_with(
        r#"<pattern>
             <rule context="line[@type='discount']">
               <assert test="false()">discount branch</assert>
             </rule>
             <rule context="line">
               <assert test="false()">normal branch</assert>
             </rule>
           </pattern>"#,
    );
    let found = failures(&schema, r#"<o><line type="discount"/><line/></o>"#);
    assert_eq!(found, vec!["discount branch", "normal branch"]);
}

#[test]
fn patterns_do_not_compete_with_each_other() {
    // The same node is matched independently by both patterns.
    let schema = schema_with(
        r#"<pattern><rule context="*"><assert test="false()">one</assert></rule></pattern>
           <pattern><rule context="a"><assert test="false()">two</assert></rule></pattern>"#,
    );
    let found = failures(&schema, "<a/>");
    assert_contains!(found, &"one".to_string());
    assert_contains!(found, &"two".to_string());
}

#[test]
fn rules_can_match_attributes_text_comments_and_processing_instructions() {
    let schema = schema_with(
        r#"<pattern>
             <rule context="@x"><assert test="false()">attribute</assert></rule>
             <rule context="text()"><assert test="false()">text</assert></rule>
             <rule context="comment()"><assert test="false()">comment</assert></rule>
             <rule context="processing-instruction()"><assert test="false()">pi</assert></rule>
           </pattern>"#,
    );
    let found = failures(&schema, "<a x='1'>t<!--c--><?pi d?></a>");
    for expected in ["attribute", "text", "comment", "pi"] {
        assert_contains!(found, &expected.to_string());
    }
}

#[test]
fn messages_interpolate_value_of_and_name() {
    let schema = schema_with(
        r#"<pattern><rule context="line">
             <assert test="false()"><name/> has qty <value-of select="@qty"/></assert>
           </rule></pattern>"#,
    );
    assert_eq!(
        failures(&schema, "<o><line qty='7'/></o>"),
        vec!["line has qty 7"]
    );
}

#[test]
fn name_with_a_path_names_another_node() {
    let schema = schema_with(
        r#"<pattern><rule context="a">
             <assert test="false()">child is <name path="*[1]"/></assert>
           </rule></pattern>"#,
    );
    assert_eq!(failures(&schema, "<a><b/></a>"), vec!["child is b"]);
}

#[test]
fn namespaced_documents_need_a_prefix_binding() {
    let document = r#"<a xmlns="urn:n"><b/></a>"#;

    // Without a prefix, an unprefixed context matches nothing at all: XPath
    // 1.0 has no default namespace.
    let unbound = schema_with(
        r#"<pattern><rule context="a"><assert test="false()">never</assert></rule></pattern>"#,
    );
    assert_is_empty!(failures(&unbound, document));

    let bound = schema_with(
        r#"<ns prefix="n" uri="urn:n"/>
           <pattern><rule context="n:a"><assert test="n:b">fires</assert></rule></pattern>"#,
    );
    let schema = Schema::from_str(&bound).unwrap();
    let report = schema
        .validate(&Document::from_str(document).unwrap())
        .unwrap();
    assert_eq!(report.count_fired_rules(), 1);
    assert!(report.is_valid());
}

#[test]
fn let_variables_are_visible_at_every_inner_scope() {
    let schema = schema_with(
        r#"<let name="limit" value="10"/>
           <pattern>
             <let name="doubled" value="$limit * 2"/>
             <rule context="a">
               <let name="actual" value="number(@n)"/>
               <assert test="$actual &lt;= $doubled">
                 <value-of select="$actual"/> exceeds <value-of select="$doubled"/>
               </assert>
             </rule>
           </pattern>"#,
    );
    assert_is_empty!(failures(&schema, "<a n='5'/>"));
    assert_eq!(failures(&schema, "<a n='99'/>"), vec!["99 exceeds 20"]);
}

#[test]
fn an_inner_let_shadows_an_outer_one() {
    let schema = schema_with(
        r#"<let name="x" value="'outer'"/>
           <pattern>
             <rule context="a">
               <let name="x" value="'inner'"/>
               <assert test="false()"><value-of select="$x"/></assert>
             </rule>
           </pattern>"#,
    );
    assert_eq!(failures(&schema, "<a/>"), vec!["inner"]);
}

#[test]
fn a_rule_level_let_sees_the_firing_node() {
    let schema = schema_with(
        r#"<pattern><rule context="a">
             <let name="n" value="@n"/>
             <assert test="false()"><value-of select="$n"/></assert>
           </rule></pattern>"#,
    );
    assert_eq!(
        failures(&schema, "<r><a n='1'/><a n='2'/></r>"),
        vec!["1", "2"]
    );
}

#[test]
fn phases_select_which_patterns_run() {
    let source = r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="quick">
        <phase id="quick"><active pattern="one"/></phase>
        <phase id="full"><active pattern="one"/><active pattern="two"/></phase>
        <pattern id="one"><rule context="a"><assert test="false()">one</assert></rule></pattern>
        <pattern id="two"><rule context="a"><assert test="false()">two</assert></rule></pattern>
    </schema>"#;
    let schema = Schema::from_str(source).unwrap();
    let document = Document::from_str("<a/>").unwrap();

    let by_default = schema.validate(&document).unwrap();
    assert_eq!(by_default.count_failures(), 1);

    let full = schema
        .validate_with(
            &document,
            &ValidateOptions::new().with_phase(PhaseSelection::Named("full".into())),
        )
        .unwrap();
    assert_eq!(full.count_failures(), 2);

    let all = schema
        .validate_with(
            &document,
            &ValidateOptions::new().with_phase(PhaseSelection::All),
        )
        .unwrap();
    assert_eq!(all.count_failures(), 2);
}

#[test]
fn naming_an_unknown_phase_is_an_error() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="a"><assert test="b">m</assert></rule></pattern>"#,
    ))
    .unwrap();
    let document = Document::from_str("<a/>").unwrap();
    let error = schema
        .validate_with(
            &document,
            &ValidateOptions::new().with_phase(PhaseSelection::Named("nope".into())),
        )
        .unwrap_err();
    assert_contains!(error.to_string(), "unknown phase");
}

#[test]
fn diagnostics_are_instantiated_in_the_assertion_context() {
    let schema = schema_with(
        r#"<pattern><rule context="a">
             <assert test="false()" diagnostics="d">short</assert>
           </rule></pattern>
           <diagnostics>
             <diagnostic id="d">n is <value-of select="@n"/></diagnostic>
           </diagnostics>"#,
    );
    let schema = Schema::from_str(&schema).unwrap();
    let report = schema
        .validate(&Document::from_str("<a n='4'/>").unwrap())
        .unwrap();
    let failure = report.failures().next().unwrap();
    assert_eq!(failure.diagnostics.len(), 1);
    assert_eq!(failure.diagnostics[0].text.trim(), "n is 4");
}

#[test]
fn subject_moves_the_reported_location() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="a">
             <assert test="false()" subject="b">about b</assert>
           </rule></pattern>"#,
    ))
    .unwrap();
    let report = schema
        .validate(&Document::from_str("<a><b/></a>").unwrap())
        .unwrap();
    assert_eq!(report.failures().next().unwrap().location, "/a[1]/b[1]");
}

#[test]
fn flags_and_roles_fall_back_from_assertion_to_rule() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="a" flag="error" role="structure">
             <assert test="false()">inherits</assert>
             <assert test="false()" flag="warning">overrides</assert>
           </rule></pattern>"#,
    ))
    .unwrap();
    let report = schema
        .validate(&Document::from_str("<a/>").unwrap())
        .unwrap();
    let found: Vec<_> = report.failures().collect();
    assert_eq!(found[0].flag.as_deref(), Some("error"));
    assert_eq!(found[0].role.as_deref(), Some("structure"));
    assert_eq!(found[1].flag.as_deref(), Some("warning"));
}

#[test]
fn abstract_patterns_are_instantiated_per_parameter_set() {
    let schema = schema_with(
        r#"<pattern abstract="true" id="required">
             <rule context="$parent">
               <assert test="$child">a <name/> needs a child</assert>
             </rule>
           </pattern>
           <pattern is-a="required" id="i1">
             <param name="parent" value="invoice"/><param name="child" value="total"/>
           </pattern>
           <pattern is-a="required" id="i2">
             <param name="parent" value="order"/><param name="child" value="date"/>
           </pattern>"#,
    );
    let found = failures(&schema, "<r><invoice/><order/></r>");
    assert_eq!(
        found,
        vec!["a invoice needs a child", "a order needs a child"]
    );
}

#[test]
fn abstract_rules_splice_their_assertions_in_order() {
    let schema = schema_with(
        r#"<pattern>
             <rule abstract="true" id="dated">
               <assert test="@date">needs a date</assert>
             </rule>
             <rule context="invoice">
               <assert test="@id">needs an id</assert>
               <extends rule="dated"/>
               <assert test="total">needs a total</assert>
             </rule>
           </pattern>"#,
    );
    assert_eq!(
        failures(&schema, "<invoice/>"),
        vec!["needs an id", "needs a date", "needs a total"]
    );
}

#[test]
fn max_failures_stops_early() {
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="a"><assert test="false()">m</assert></rule></pattern>"#,
    ))
    .unwrap();
    let document = Document::from_str("<r><a/><a/><a/><a/><a/></r>").unwrap();

    let all = schema.validate(&document).unwrap();
    assert_eq!(all.count_failures(), 5);

    let limited = schema
        .validate_with(&document, &ValidateOptions::new().with_max_failures(2))
        .unwrap();
    assert_eq!(limited.count_failures(), 2);
}

#[test]
fn failure_flags_limit_what_counts_toward_the_cap() {
    // A warning comes first in document order. Counting it would stop the
    // run before the error is evaluated, and a caller keeping only errors
    // would then see a clean report for an invalid document.
    let schema = Schema::from_str(&schema_with(
        r#"<pattern>
             <rule context="a" flag="warning">
               <assert test="false()">rule-level warning</assert>
               <assert test="false()" flag="error">own error</assert>
             </rule>
           </pattern>"#,
    ))
    .unwrap();
    let document = Document::from_str("<r><a/><a/></r>").unwrap();

    let uncounted = schema
        .validate_with(&document, &ValidateOptions::new().with_max_failures(1))
        .unwrap();
    let texts: Vec<_> = uncounted.failures().map(|f| f.text.trim()).collect();
    assert_eq!(texts, vec!["rule-level warning"]);

    let options = ValidateOptions::new()
        .with_max_failures(1)
        .with_failure_flags(["error"]);
    assert_eq!(options.failure_flags, vec!["error".to_string()]);
    let counted = schema.validate_with(&document, &options).unwrap();
    let found: Vec<_> = counted
        .failures()
        .map(|f| (f.flag.as_deref(), f.text.trim()))
        .collect();
    // The warning is still reported; only the error counted, and the run
    // stopped at the first one.
    assert_eq!(
        found,
        vec![
            (Some("warning"), "rule-level warning"),
            (Some("error"), "own error")
        ]
    );
}

#[test]
fn an_evaluation_error_is_an_error_not_a_silent_false() {
    // `$elsewhere` is declared, so the schema compiles, but it is bound in
    // another pattern and so is out of reach here. The test cannot be
    // evaluated, and treating that as "false" would let a broken schema pass
    // a broken document.
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><let name="elsewhere" value="1"/>
             <rule context="a"><assert test="true()">m</assert></rule>
           </pattern>
           <pattern><rule context="a"><assert test="$elsewhere">m</assert></rule></pattern>"#,
    ))
    .unwrap();
    let error = schema
        .validate(&Document::from_str("<a/>").unwrap())
        .unwrap_err();
    assert_contains!(error.to_string(), "$elsewhere");
}

#[test]
fn a_variable_nothing_binds_is_caught_when_the_schema_loads() {
    // A misspelling should not wait until it aborts somebody's validation.
    let error = Schema::from_str(&schema_with(
        r#"<let name="limit" value="10"/>
           <pattern><rule context="a"><assert test="$lmit">m</assert></rule></pattern>"#,
    ))
    .unwrap_err();
    assert_contains!(error.to_string(), "$lmit");
    // And it says what the schema does declare.
    assert_contains!(error.to_string(), "limit");
}

#[test]
fn one_schema_validates_many_documents_concurrently() {
    use std::sync::Arc;
    use std::thread;

    let schema = Arc::new(
        Schema::from_str(&schema_with(
            r#"<pattern><rule context="a"><assert test="b">m</assert></rule></pattern>"#,
        ))
        .unwrap(),
    );

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let schema = Arc::clone(&schema);
            thread::spawn(move || {
                let source = if i % 2 == 0 { "<a/>" } else { "<a><b/></a>" };
                let document = Document::from_str(source).unwrap();
                schema.validate(&document).unwrap().count_failures()
            })
        })
        .collect();

    let counts: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        counts
            .iter()
            .try_fold(0_usize, |total, n| total.checked_add(*n))
            .unwrap(),
        4
    );
}

#[test]
fn report_order_is_pattern_then_document_then_assertion() {
    let schema = schema_with(
        r#"<pattern id="p1"><rule context="b">
             <assert test="false()">p1-b-1</assert>
             <assert test="false()">p1-b-2</assert>
           </rule></pattern>
           <pattern id="p2"><rule context="a">
             <assert test="false()">p2-a</assert>
           </rule></pattern>"#,
    );
    assert_eq!(
        failures(&schema, "<a><b/></a>"),
        vec!["p1-b-1", "p1-b-2", "p2-a"]
    );
}

/// RCA-TEST-8: a supplied out-of-range option is rejected, never clamped or defaulted.
#[test]
fn out_of_range_run_options_are_rejected() {
    use schematron::Error;
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="a"><assert test="true()">x</assert></rule></pattern>"#,
    ))
    .expect("schema should compile");
    let document = Document::from_str("<a/>").expect("document should parse");
    for (options, name) in [
        (
            ValidateOptions::default().with_implicit_timezone(841),
            "implicit_timezone",
        ),
        (
            ValidateOptions::default().with_implicit_timezone(-841),
            "implicit_timezone",
        ),
        (
            ValidateOptions::default().with_current_time(f64::NAN),
            "current_time",
        ),
        (
            ValidateOptions::default().with_current_time(f64::INFINITY),
            "current_time",
        ),
    ] {
        match schema.validate_with(&document, &options) {
            Err(Error::InvalidOption { option, .. }) => assert_eq!(option, name),
            other => panic!("expected InvalidOption for {name}, got {other:?}"),
        }
    }
    let edge = ValidateOptions::default()
        .with_implicit_timezone(840)
        .with_current_time(0.0);
    schema
        .validate_with(&document, &edge)
        .expect("the edge values are valid");
}

#[test]
fn an_absolute_context_is_not_mistaken_for_a_descendant_match() {
    // `/node()/b` has the same two-step shape as the rooted form of a bare
    // name, but its first step is `child::`, so it matches only children of
    // the document element: one `b` here, not both.
    let schema = Schema::from_str(&schema_with(
        r#"<pattern><rule context="/node()/b"><report test="true()">b</report></rule></pattern>
           <pattern><rule context="/descendant-or-self::x/b"><report test="true()">xb</report></rule></pattern>"#,
    ))
    .unwrap();
    let document = Document::from_str("<r><b/><x><b/></x></r>").unwrap();
    let report = schema.validate(&document).unwrap();
    let found: Vec<_> = report
        .reports()
        .map(|r| (r.location.as_str(), r.text.trim()))
        .collect();
    assert_eq!(found, vec![("/r[1]/b[1]", "b"), ("/r[1]/x[1]/b[1]", "xb")]);
    assert_eq!(report.count_fired_rules(), 2);
}
