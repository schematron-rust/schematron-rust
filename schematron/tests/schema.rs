//! Integration tests for schema loading: includes, resolvers, errors, and the
//! compile-time checks that catch a broken schema before it is ever run.

use std::sync::Arc;

use assertables::*;
use schematron::schema::{MemoryResolver, SchemaOptions};
use schematron::{Document, Error, Schema};

fn resolver(pairs: &[(&str, &str)]) -> Arc<MemoryResolver> {
    let mut resolver = MemoryResolver::new();
    for (href, source) in pairs {
        resolver = resolver.with(*href, *source);
    }
    Arc::new(resolver)
}

fn compile_with(source: &str, files: &[(&str, &str)]) -> Result<Schema, Error> {
    let options = SchemaOptions::new().with_resolver(resolver(files));
    Schema::from_str_with(source, &options)
}

#[test]
fn include_splices_a_pattern_into_the_schema() {
    let schema = compile_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <include href="lines.sch"/>
           </schema>"#,
        &[(
            "lines.sch",
            r#"<pattern xmlns="http://purl.oclc.org/dsdl/schematron" id="lines">
                 <rule context="line"><assert test="@qty">needs qty</assert></rule>
               </pattern>"#,
        )],
    )
    .unwrap();

    assert_eq!(schema.patterns().len(), 1);
    let report = schema
        .validate(&Document::from_str("<order><line/></order>").unwrap())
        .unwrap();
    assert_eq!(report.count_failures(), 1);
}

#[test]
fn includes_nest() {
    let schema = compile_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <include href="outer.sch"/>
           </schema>"#,
        &[
            (
                "outer.sch",
                r#"<pattern xmlns="http://purl.oclc.org/dsdl/schematron" id="outer">
                     <include href="inner.sch"/>
                   </pattern>"#,
            ),
            (
                "inner.sch",
                r#"<rule xmlns="http://purl.oclc.org/dsdl/schematron" context="a">
                     <assert test="b">from the inner include</assert>
                   </rule>"#,
            ),
        ],
    )
    .unwrap();

    let report = schema.validate(&Document::from_str("<a/>").unwrap()).unwrap();
    assert_eq!(
        report.failures().next().unwrap().text,
        "from the inner include"
    );
}

#[test]
fn include_cycles_are_reported_rather_than_looping() {
    let error = compile_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <include href="a.sch"/>
           </schema>"#,
        &[
            (
                "a.sch",
                r#"<pattern xmlns="http://purl.oclc.org/dsdl/schematron"><include href="b.sch"/></pattern>"#,
            ),
            (
                "b.sch",
                r#"<pattern xmlns="http://purl.oclc.org/dsdl/schematron"><include href="a.sch"/></pattern>"#,
            ),
        ],
    )
    .unwrap_err();

    assert!(matches!(error, Error::IncludeCycle { .. }), "{error}");
    assert_contains!(error.to_string(), "a.sch");
}

#[test]
fn a_missing_include_names_the_href() {
    let error = compile_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <include href="nowhere.sch"/>
           </schema>"#,
        &[],
    )
    .unwrap_err();
    assert_contains!(error.to_string(), "nowhere.sch");
}

#[test]
fn the_default_resolver_refuses_network_access() {
    let error = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <include href="https://example.com/rules.sch"/>
           </schema>"#,
    )
    .unwrap_err();
    assert_contains!(error.to_string(), "network");
}

#[test]
fn a_broken_expression_fails_at_compile_time_not_validation_time() {
    let error = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern><rule context="a"><assert test="count(b">m</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap_err();
    assert!(matches!(error, Error::XPathSyntax { .. }), "{error}");
    // The message must locate the expression inside the schema.
    assert_contains!(error.to_string(), "@test");
    assert_contains!(error.to_string(), "count(b");
}

#[test]
fn an_xpath_two_construct_under_a_one_point_zero_binding_says_so_by_name() {
    let error = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt">
             <pattern><rule context="a">
               <assert test="matches(@x, '[0-9]+')">m</assert>
             </rule></pattern>
           </schema>"#,
    )
    .unwrap_err();
    assert_contains!(error.to_string(), "matches()");
    assert_contains!(error.to_string(), "XPath 2.0");
    // And it says what to do about it.
    assert_contains!(error.to_string(), "xslt2");
}

#[test]
fn an_unsupported_query_binding_is_refused_by_default_and_can_be_forced() {
    // XPath 3.0 and later remain refused: the crate implements 1.0 and the
    // phase-1 subset of 2.0, and accepting 3.x would overclaim.
    let source = r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt3">
                      <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
                    </schema>"#;

    let error = Schema::from_str(source).unwrap_err();
    assert!(matches!(error, Error::UnsupportedQueryBinding { .. }), "{error}");

    let forced = SchemaOptions::new().with_allow_unknown_query_binding(true);
    assert!(Schema::from_str_with(source, &forced).is_ok());
}

#[test]
fn a_missing_namespace_prefix_is_caught_before_validation() {
    let error = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern><rule context="inv:invoice"><assert test="b">m</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap_err();
    assert_contains!(error.to_string(), "inv");
    assert_contains!(error.to_string(), "not declared");
}

#[test]
fn schema_metadata_is_available_after_compiling() {
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron"
                   id="s1" schemaVersion="3" defaultPhase="quick">
             <title>My rules</title>
             <phase id="quick"><active pattern="p"/></phase>
             <pattern id="p"><rule context="a"><assert test="b">m</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap();

    assert_eq!(schema.id(), Some("s1"));
    assert_eq!(schema.title(), Some("My rules"));
    assert_eq!(schema.schema_version(), Some("3"));
    assert_eq!(schema.default_phase(), Some("quick"));
    assert_eq!(schema.phases().collect::<Vec<_>>(), vec!["quick"]);
}

#[test]
fn the_legacy_schematron_namespace_still_compiles() {
    let schema = Schema::from_str(
        r#"<schema xmlns="http://www.ascc.net/xml/schematron">
             <pattern><rule context="a"><assert test="b">legacy</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap();
    let report = schema.validate(&Document::from_str("<a/>").unwrap()).unwrap();
    assert_eq!(report.failures().next().unwrap().text, "legacy");
}

#[test]
fn a_document_that_is_not_well_formed_reports_line_and_column() {
    let error = Document::from_str("<a>\n  <b>\n</a>").unwrap_err();
    match error {
        Error::XmlParse { line, .. } => assert!(line >= 1),
        other => panic!("expected a parse error, got {other}"),
    }
}

#[test]
fn external_entities_are_never_resolved() {
    // No DTD processing means XXE is structurally impossible rather than
    // merely switched off.
    let error = Document::from_str(
        r#"<!DOCTYPE a [<!ENTITY x SYSTEM "file:///etc/passwd">]><a>&x;</a>"#,
    )
    .unwrap_err();
    assert_contains!(error.to_string(), "entity");
}

#[test]
fn pattern_documents_validates_external_files() {
    // `@documents` is Schematron's own mechanism for cross-document
    // validation: the expression names URIs, and the pattern's rules run
    // against each named document instead of against the instance.
    let directory = std::env::temp_dir().join("schematron-documents-test");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("good.xml"), "<part><name>Widget</name></part>").unwrap();
    std::fs::write(directory.join("bad.xml"), "<part/>").unwrap();
    let instance = directory.join("catalog.xml");
    std::fs::write(
        &instance,
        r#"<catalog><ref href="good.xml"/><ref href="bad.xml"/></catalog>"#,
    )
    .unwrap();

    // The context node for @documents is the root node, matching the ISO
    // XSLT skeleton, so the path starts at the document element.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern id="external" documents="catalog/ref/@href">
               <rule context="part">
                 <assert test="name">An external part must have a name.</assert>
               </rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();

    let document = Document::from_path(&instance).unwrap();
    let report = schema.validate(&document).unwrap();

    // One active-pattern run per external document.
    assert_eq!(report.patterns.len(), 2);
    assert_eq!(report.patterns[0].documents.as_deref(), Some("good.xml"));
    assert_eq!(report.patterns[1].documents.as_deref(), Some("bad.xml"));
    // Only the second document breaks the rule.
    assert_eq!(report.count_failures(), 1);

    let _ = std::fs::remove_dir_all(&directory);
}
