//! Integration tests for XPath `document()` and cross-document node-sets.
//!
//! A node-set is a list of indices into one arena, so returning nodes of
//! another document means merging that document into the same arena. And
//! because evaluation holds the tree immutably, a `document()` call cannot
//! load anything on the spot — it records a miss, and the validator runs
//! again with the document loaded. These tests pin down both halves.

use std::path::PathBuf;
use std::sync::Arc;

use assertables::*;
use schematron::schema::{MemoryResolver, SchemaOptions};
use schematron::{Document, Schema};

/// Compiles a schema whose `document()` targets are served from memory.
fn schema_with(source: &str, files: &[(&str, &str)]) -> Schema {
    let mut resolver = MemoryResolver::new();
    for (href, body) in files {
        resolver = resolver.with(*href, *body);
    }
    let options = SchemaOptions::new().with_resolver(Arc::new(resolver));
    Schema::from_str_with(source, &options).expect("schema should compile")
}

const CATALOGUE: &str = r#"
    <parts>
      <part sku="A-1"><name>Widget</name></part>
      <part sku="B-2"><name>Gadget</name></part>
    </parts>
"#;

#[test]
fn document_reads_an_external_file_by_literal_uri() {
    let schema = schema_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="line">
                 <assert test="document('parts.xml')/parts/part[@sku = current()/@sku]">
                   No part has sku <value-of select="@sku"/>.
                 </assert>
               </rule>
             </pattern>
           </schema>"#,
        &[("parts.xml", CATALOGUE)],
    );

    let document =
        Document::from_str(r#"<order><line sku="A-1"/><line sku="Z-9"/></order>"#).unwrap();
    let report = schema.validate(&document).unwrap();

    assert_eq!(report.count_failures(), 1);
    let failure = report.failures().next().unwrap();
    assert_eq!(failure.location, "/order[1]/line[2]");
    assert_contains!(failure.text, "Z-9");
}

#[test]
fn document_accepts_a_uri_computed_from_the_instance() {
    // `document(@href)` is the shape real schemas use: the URI is not known
    // until the document is being validated.
    let schema = schema_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="ref">
                 <assert test="document(@href)/doc/item"><value-of select="@href"/> has no item.</assert>
               </rule>
             </pattern>
           </schema>"#,
        &[
            ("a.xml", "<doc><item/></doc>"),
            ("b.xml", "<doc/>"),
        ],
    );

    let document =
        Document::from_str(r#"<catalog><ref href="a.xml"/><ref href="b.xml"/></catalog>"#).unwrap();
    let report = schema.validate(&document).unwrap();

    assert_eq!(report.count_failures(), 1);
    assert_contains!(report.failures().next().unwrap().text, "b.xml");
}

#[test]
fn a_node_set_argument_loads_every_document_it_names() {
    let schema = schema_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="catalog">
                 <assert test="count(document(ref/@href)/doc/item) = 3">
                   Expected three items, found <value-of select="count(document(ref/@href)/doc/item)"/>.
                 </assert>
               </rule>
             </pattern>
           </schema>"#,
        &[
            ("a.xml", "<doc><item/><item/></doc>"),
            ("b.xml", "<doc><item/></doc>"),
        ],
    );

    let document =
        Document::from_str(r#"<catalog><ref href="a.xml"/><ref href="b.xml"/></catalog>"#).unwrap();
    let report = schema.validate(&document).unwrap();
    assert!(report.is_valid(), "{}", report.to_text());
}

#[test]
fn document_calls_can_be_chained_across_passes() {
    // The URI of the second document is inside the first, so one pass cannot
    // discover both. The fixpoint resolves it.
    let schema = schema_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="start">
                 <assert test="document(document(@first)/hop/@next)/hop/@final = 'yes'">
                   The chain did not resolve.
                 </assert>
               </rule>
             </pattern>
           </schema>"#,
        &[
            ("hop1.xml", r#"<hop next="hop2.xml"/>"#),
            ("hop2.xml", r#"<hop final="yes"/>"#),
        ],
    );

    let document = Document::from_str(r#"<start first="hop1.xml"/>"#).unwrap();
    assert!(schema.validate(&document).unwrap().is_valid());
}

#[test]
fn an_absolute_path_inside_a_loaded_document_means_that_documents_root() {
    // `/` is the root of the document the context node belongs to, not the
    // root of the instance. Getting this wrong would silently search the
    // wrong tree.
    let schema = schema_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="order">
                 <assert test="document('other.xml')/other/marker">Loaded root is wrong.</assert>
                 <assert test="not(document('other.xml')/order)">Loaded document must not see the instance.</assert>
               </rule>
             </pattern>
           </schema>"#,
        &[("other.xml", "<other><marker/></other>")],
    );

    let document = Document::from_str("<order/>").unwrap();
    let report = schema.validate(&document).unwrap();
    assert!(report.is_valid(), "{}", report.to_text());
}

#[test]
fn a_missing_document_is_an_error_naming_the_uri() {
    let schema = schema_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="a"><assert test="document('gone.xml')/x">m</assert></rule>
             </pattern>
           </schema>"#,
        &[],
    );

    let document = Document::from_str("<a/>").unwrap();
    let error = schema.validate(&document).unwrap_err();
    assert_contains!(error.to_string(), "gone.xml");
}

#[test]
fn the_default_resolver_still_refuses_the_network() {
    // `document()` must not become a hole in the no-implicit-network rule.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="a">
                 <assert test="document('https://example.com/x.xml')/x">m</assert>
               </rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();

    let document = Document::from_str("<a/>").unwrap();
    let error = schema.validate(&document).unwrap_err();
    assert_contains!(error.to_string(), "network");
}

#[test]
fn document_relative_uris_resolve_against_the_instance() {
    let directory = std::env::temp_dir().join("schematron-document-fn-test");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("parts.xml"), CATALOGUE).unwrap();
    let instance: PathBuf = directory.join("order.xml");
    std::fs::write(&instance, r#"<order><line sku="A-1"/></order>"#).unwrap();

    // The default file resolver, and a relative href in the schema.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="line">
                 <assert test="document('parts.xml')/parts/part[@sku = current()/@sku]">unknown sku</assert>
               </rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();

    let document = Document::from_path(&instance).unwrap();
    let report = schema.validate(&document).unwrap();
    assert!(report.is_valid(), "{}", report.to_text());

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn the_two_argument_document_resolves_against_the_node_it_is_given() {
    // XSLT 1.0 section 12.1: with a second argument, a relative URI resolves
    // against the base URI of that node-set's first node, not the instance's.
    //
    // The test only means something because the two answers are *different
    // files*: `parts.xml` exists both beside the instance and beside the
    // catalogue, with different contents. A one-argument call finds the
    // former, a two-argument call the latter.
    let directory = std::env::temp_dir().join("schematron-document-two-arg-test");
    let sub = directory.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("catalogue.xml"), r#"<catalogue><ref href="parts.xml"/></catalogue>"#)
        .unwrap();
    std::fs::write(sub.join("parts.xml"), r#"<parts><part id="A"/><part id="B"/></parts>"#).unwrap();
    std::fs::write(directory.join("parts.xml"), r#"<parts><part id="BESIDE-INSTANCE"/></parts>"#)
        .unwrap();
    let instance = directory.join("order.xml");
    std::fs::write(&instance, "<order><load>sub/catalogue.xml</load></order>").unwrap();

    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="order">
                 <let name="catalogue" value="document(load)"/>
                 <assert test="count($catalogue//ref) = 1">the catalogue loads</assert>

                 <!-- Resolved beside the catalogue: two parts. -->
                 <assert test="count(document($catalogue//ref/@href, $catalogue)//part) = 2">
                   the two-argument form resolves beside the catalogue
                 </assert>
                 <assert test="document($catalogue//ref/@href, $catalogue)//part[1]/@id = 'A'">
                   and so reads the parts file next to it
                 </assert>

                 <!-- The same href, resolved beside the instance: one part. -->
                 <assert test="count(document($catalogue//ref/@href)//part) = 1">
                   the one-argument form resolves beside the instance
                 </assert>
                 <assert test="document($catalogue//ref/@href)//part[1]/@id = 'BESIDE-INSTANCE'">
                   and so reads a different file entirely
                 </assert>
               </rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();

    let document = Document::from_path(&instance).unwrap();
    let report = schema.validate(&document).unwrap();
    assert!(report.is_valid(), "{}", report.to_text());

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn an_empty_second_argument_to_document_yields_nothing() {
    // Not an error: loading runs in passes, and on the first pass every
    // `document()` call returns empty, so a nested call's second argument is
    // empty until a later pass. Erroring would abort before the retry.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="a">
                 <report test="count(document('parts.xml', nothing)) = 0">nothing to resolve against</report>
               </rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();

    let document = Document::from_str("<a/>").unwrap();
    let report = schema.validate(&document).unwrap();
    assert_eq!(report.count_fired_rules(), 1);
    assert_eq!(report.reports().count(), 1);
}

#[test]
fn a_schema_without_document_is_unaffected() {
    // The zero-cost path: no working copy, no extra pass.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap();
    assert!(!schema.uses_document_function());

    let with_it = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern><rule context="a"><assert test="document('x')/y">m</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap();
    assert!(with_it.uses_document_function());
}

#[test]
fn document_is_detected_inside_predicates_and_arguments() {
    // The zero-cost decision is only safe if the detector is thorough.
    for test in [
        "document('x')/y",
        "count(document('x')/y) > 0",
        "a[document('x')/y]",
        "-count(document('x')/y)",
        "a | document('x')/y",
        "concat('a', document('x')/y)",
    ] {
        let source = format!(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a"><assert test="{test}">m</assert></rule></pattern>
               </schema>"#
        );
        let schema = Schema::from_str(&source).unwrap();
        assert!(
            schema.uses_document_function(),
            "document() not detected in {test:?}"
        );
    }
}

#[test]
fn calling_document_without_a_registry_is_an_error() {
    // Evaluating XPath directly, outside a validation run, has no registry.
    // An empty node-set would turn a broken lookup into a passing assertion.
    use schematron::xpath::{evaluate, parse, EvalContext, Namespaces, Variables};

    let document = Document::from_str("<a/>").unwrap();
    let expr = parse("document('x.xml')").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();
    let context = EvalContext::new(&document, document.root(), &variables, &namespaces);

    let error = evaluate(&expr, &context).unwrap_err();
    assert_contains!(error.message, "registry");
}
