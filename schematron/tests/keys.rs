//! Integration tests for keys.
//!
//! A key is a named index over a document, declared once and looked up many
//! times. It exists because cross-reference checks are the most common
//! expensive thing a Schematron schema does, and without an index they are
//! quadratic. See `spec/keys/`.

use assertables::*;
use schematron::{Document, Schema};

/// Wraps a body in a schema envelope.
fn schema_with(body: &str) -> String {
    format!(r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">{body}</schema>"#)
}

/// The messages of the failed assertions, in order.
fn failures(body: &str, document: &str) -> Vec<String> {
    let schema = Schema::from_str(&schema_with(body)).expect("schema should compile");
    let document = Document::from_str(document).expect("document should parse");
    schema
        .validate(&document)
        .expect("validation should run")
        .failures()
        .map(|f| f.text.trim().to_string())
        .collect()
}

const CATALOGUE: &str = r#"
    <order>
      <parts><part id="A"/><part id="B"/></parts>
      <line ref="A"/>
      <line ref="Z"/>
    </order>
"#;

#[test]
fn a_key_resolves_a_cross_reference() {
    let found = failures(
        r#"<key name="parts" match="part" use="@id"/>
           <pattern><rule context="line">
             <assert test="key('parts', @ref)">no part <value-of select="@ref"/></assert>
           </rule></pattern>"#,
        CATALOGUE,
    );
    assert_eq!(found, vec!["no part Z"]);
}

#[test]
fn a_lookup_returns_the_indexed_nodes_themselves() {
    // Not merely a boolean: the result is a node-set a path can continue from.
    let found = failures(
        r#"<key name="parts" match="part" use="@id"/>
           <pattern><rule context="line">
             <assert test="key('parts', @ref)/@id = @ref">mismatch</assert>
           </rule></pattern>"#,
        r#"<order><parts><part id="A"/></parts><line ref="A"/></order>"#,
    );
    assert_is_empty!(found);
}

#[test]
fn a_use_selecting_several_nodes_indexes_under_each() {
    // A part with two identifiers is findable by either, which is what XSLT
    // does and what makes aliases work.
    let found = failures(
        r#"<key name="parts" match="part" use="alias"/>
           <pattern><rule context="line">
             <assert test="key('parts', @ref)">no part <value-of select="@ref"/></assert>
           </rule></pattern>"#,
        r#"<order>
             <part><alias>A</alias><alias>AA</alias></part>
             <line ref="A"/><line ref="AA"/><line ref="Z"/>
           </order>"#,
    );
    assert_eq!(found, vec!["no part Z"]);
}

#[test]
fn a_node_set_lookup_value_is_existential() {
    // `key('parts', line/@ref)` finds every referenced part at once.
    let found = failures(
        r#"<key name="parts" match="part" use="@id"/>
           <pattern><rule context="order">
             <assert test="count(key('parts', line/@ref)) = 2">
               found <value-of select="count(key('parts', line/@ref))"/>
             </assert>
           </rule></pattern>"#,
        r#"<order>
             <part id="A"/><part id="B"/><part id="C"/>
             <line ref="A"/><line ref="B"/>
           </order>"#,
    );
    assert_is_empty!(found);
}

#[test]
fn a_key_matching_nothing_is_still_declared() {
    // "No such node" and "no such key" are different mistakes, and only the
    // second should be an error.
    let found = failures(
        r#"<key name="parts" match="nothing" use="@id"/>
           <pattern><rule context="line">
             <assert test="key('parts', @ref)">no part</assert>
           </rule></pattern>"#,
        r#"<order><line ref="A"/></order>"#,
    );
    assert_eq!(found, vec!["no part"]);
}

#[test]
fn looking_up_an_undeclared_key_is_an_error() {
    // An empty result would make the assertion quietly pass, which is the
    // opposite of what a typo in the key name should do.
    let schema = Schema::from_str(&schema_with(
        r#"<key name="parts" match="part" use="@id"/>
           <pattern><rule context="line">
             <assert test="key('prats', @ref)">no part</assert>
           </rule></pattern>"#,
    ))
    .expect("schema should compile");
    let document = Document::from_str(CATALOGUE).unwrap();

    let error = schema.validate(&document).unwrap_err();
    assert_contains!(error.to_string(), "prats");
    assert_contains!(error.to_string(), "no key named");
}

#[test]
fn two_keys_may_not_share_a_name() {
    let error = Schema::from_str(&schema_with(
        r#"<key name="parts" match="part" use="@id"/>
           <key name="parts" match="item" use="@id"/>
           <pattern><rule context="line"><assert test="key('parts', @ref)">m</assert></rule></pattern>"#,
    ))
    .unwrap_err();
    assert_contains!(error.to_string(), "ambiguous");
}

#[test]
fn a_key_needs_all_three_attributes() {
    for body in [
        r#"<key match="part" use="@id"/>"#,
        r#"<key name="parts" use="@id"/>"#,
        r#"<key name="parts" match="part"/>"#,
    ] {
        let source = schema_with(&format!(
            r#"{body}<pattern><rule context="a"><assert test="b">m</assert></rule></pattern>"#
        ));
        assert!(Schema::from_str(&source).is_err(), "{body} should be rejected");
    }
}

#[test]
fn a_key_match_is_a_match_pattern() {
    // Same restriction as rule/@context: a leading reverse axis is rejected
    // rather than guessed at.
    let error = Schema::from_str(&schema_with(
        r#"<key name="parts" match="ancestor::part" use="@id"/>
           <pattern><rule context="line"><assert test="key('parts', @ref)">m</assert></rule></pattern>"#,
    ))
    .unwrap_err();
    assert_contains!(error.to_string(), "not allowed in a rule context");
}

#[test]
fn a_broken_key_expression_fails_when_the_schema_loads() {
    let error = Schema::from_str(&schema_with(
        r#"<key name="parts" match="part" use="count(@id"/>
           <pattern><rule context="line"><assert test="key('parts', @ref)">m</assert></rule></pattern>"#,
    ))
    .unwrap_err();
    assert_contains!(error.to_string(), "@use");
}

#[test]
fn keys_work_under_an_xpath_two_binding_too() {
    let source = r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt2">
             <key name="parts" match="part" use="@id"/>
             <pattern><rule context="line">
               <assert test="exists(key('parts', @ref))">no part</assert>
             </rule></pattern>
           </schema>"#;
    let schema = Schema::from_str(source).expect("schema should compile");
    let document = Document::from_str(CATALOGUE).unwrap();
    assert_eq!(schema.validate(&document).unwrap().count_failures(), 1);
}

#[test]
fn a_key_nothing_looks_up_is_linted() {
    // Its index is built on every validation regardless.
    let schema = Schema::from_str(&schema_with(
        r#"<key name="unused" match="part" use="@id"/>
           <pattern><rule context="line"><assert test="@ref">m</assert></rule></pattern>"#,
    ))
    .unwrap();

    let lints = schema.lint();
    assert_eq!(lints.len(), 1, "{lints:?}");
    assert_eq!(lints[0].kind.as_str(), "unreferenced-key");
    assert_contains!(lints[0].location, "unused");
}

#[test]
fn a_key_that_is_looked_up_is_not_linted() {
    let schema = Schema::from_str(&schema_with(
        r#"<key name="parts" match="part" use="@id"/>
           <pattern><rule context="line">
             <assert test="key('parts', @ref)">m</assert>
           </rule></pattern>"#,
    ))
    .unwrap();
    assert_is_empty!(schema.lint());
}

#[test]
fn calling_key_without_indexes_is_an_error() {
    // Evaluating XPath directly has no run, so there are no indexes; an
    // empty node-set would turn a missing index into a passing assertion.
    use schematron::xpath::{evaluate, parse, EvalContext, Namespaces, Variables};

    let document = Document::from_str("<a/>").unwrap();
    let expr = parse("key('parts', 'A')").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();
    let context = EvalContext::new(&document, document.root(), &variables, &namespaces);

    let error = evaluate(&expr, &context).unwrap_err();
    assert_contains!(error.message, "key indexes");
}

#[test]
fn keys_are_built_per_document_for_a_documents_pattern() {
    // A key indexes one document, as it does in XSLT — so a @documents
    // pattern gets indexes over its target, not over the instance.
    use std::sync::Arc;
    use schematron::schema::{MemoryResolver, SchemaOptions};

    let resolver = MemoryResolver::new().with(
        "external.xml",
        r#"<catalogue><part id="X"/><line ref="X"/><line ref="Y"/></catalogue>"#,
    );
    let options = SchemaOptions::new().with_resolver(Arc::new(resolver));
    let schema = Schema::from_str_with(
        &schema_with(
            r#"<key name="parts" match="part" use="@id"/>
               <pattern documents="root/ref/@href">
                 <rule context="line">
                   <assert test="key('parts', @ref)">no part <value-of select="@ref"/></assert>
                 </rule>
               </pattern>"#,
        ),
        &options,
    )
    .expect("schema should compile");

    let document =
        Document::from_str(r#"<root><ref href="external.xml"/></root>"#).unwrap();
    let report = schema.validate(&document).unwrap();

    // The index was built over the external document, where X exists and Y
    // does not.
    assert_eq!(report.count_failures(), 1);
    assert_contains!(report.failures().next().unwrap().text, "Y");
}
