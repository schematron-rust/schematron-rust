//! Integration tests for parallel pattern evaluation.
//!
//! Patterns are independent by definition, so evaluating them on separate
//! threads must not change anything a caller can observe. That is the whole
//! contract, and these tests exist to hold it: the report has to come back
//! identical, not merely equivalent.

use std::sync::Arc;

use assertables::*;
use schematron::schema::{MemoryResolver, SchemaOptions};
use schematron::validate::{PhaseSelection, ValidateOptions};
use schematron::{Document, Schema};

/// A schema with several patterns, so there is something to parallelise.
fn multi_pattern_schema() -> Schema {
    Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <let name="limit" value="10"/>
             <pattern id="structure">
               <rule context="order">
                 <assert test="@id">An order needs an id.</assert>
                 <assert test="line">An order needs lines.</assert>
               </rule>
             </pattern>
             <pattern id="quantities">
               <let name="floor" value="0"/>
               <rule context="line">
                 <let name="qty" value="number(@qty)"/>
                 <assert test="$qty &gt; $floor">
                   Quantity <value-of select="@qty"/> is not positive.
                 </assert>
               </rule>
             </pattern>
             <pattern id="amounts">
               <rule context="line">
                 <assert test="@amount">A line needs an amount.</assert>
                 <report test="number(@amount) &gt; $limit">A large amount.</report>
               </rule>
             </pattern>
             <pattern id="skus">
               <rule context="line">
                 <assert test="sku">A line needs a sku.</assert>
               </rule>
             </pattern>
           </schema>"#,
    )
    .expect("schema should compile")
}

/// A document in which a predictable share of lines break the rules.
fn document(lines: usize) -> Document {
    let mut source = String::from("<order>");
    for i in 0..lines {
        let qty = if i % 3 == 0 { -1 } else { 2 };
        if i % 5 == 0 {
            source.push_str(&format!("<line qty=\"{qty}\" amount=\"50\"/>"));
        } else {
            source.push_str(&format!("<line qty=\"{qty}\" amount=\"5\"><sku>S-{i}</sku></line>"));
        }
    }
    source.push_str("</order>");
    Document::from_str(&source).expect("document should parse")
}

fn sequential() -> ValidateOptions {
    ValidateOptions::new().with_phase(PhaseSelection::All)
}

fn parallel() -> ValidateOptions {
    sequential().with_parallel_patterns(true)
}

#[test]
fn the_report_is_identical_either_way() {
    let schema = multi_pattern_schema();
    let document = document(200);

    let one = schema.validate_with(&document, &sequential()).unwrap();
    let two = schema.validate_with(&document, &parallel()).unwrap();

    // Not "equivalent" — identical, including the order of every finding.
    assert_eq!(one, two);
}

#[test]
fn the_rendered_output_is_identical_too() {
    let schema = multi_pattern_schema();
    let document = document(120);

    let one = schema.validate_with(&document, &sequential()).unwrap();
    let two = schema.validate_with(&document, &parallel()).unwrap();

    assert_eq!(one.to_svrl(), two.to_svrl());
    assert_eq!(one.to_text(), two.to_text());
    assert_eq!(one.to_json().unwrap(), two.to_json().unwrap());
}

#[test]
fn repeated_parallel_runs_agree_with_each_other() {
    // A race would show up as run-to-run variation rather than as a
    // difference from the sequential result.
    let schema = multi_pattern_schema();
    let document = document(150);

    let first = schema.validate_with(&document, &parallel()).unwrap();
    for _ in 0..12 {
        let again = schema.validate_with(&document, &parallel()).unwrap();
        assert_eq!(first, again);
    }
}

#[test]
fn pattern_order_is_preserved() {
    let schema = multi_pattern_schema();
    let report = schema
        .validate_with(&document(10), &parallel())
        .unwrap();

    let ids: Vec<&str> = report
        .patterns
        .iter()
        .filter_map(|pattern| pattern.id.as_deref())
        .collect();
    assert_eq!(ids, vec!["structure", "quantities", "amounts", "skus"]);
}

#[test]
fn variables_do_not_leak_between_patterns() {
    // `quantities` binds `$floor`; `amounts` must not see it, and both must
    // see the schema-level `$limit`. Running them concurrently from a shared
    // base scope is exactly where a leak would appear.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <let name="shared" value="'schema'"/>
             <pattern id="one">
               <let name="local" value="'one'"/>
               <rule context="a">
                 <assert test="false()"><value-of select="$shared"/>/<value-of select="$local"/></assert>
               </rule>
             </pattern>
             <pattern id="two">
               <let name="local" value="'two'"/>
               <rule context="a">
                 <assert test="false()"><value-of select="$shared"/>/<value-of select="$local"/></assert>
               </rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();

    let document = Document::from_str("<a/>").unwrap();
    let report = schema.validate_with(&document, &parallel()).unwrap();

    let texts: Vec<String> = report.failures().map(|f| f.text.clone()).collect();
    assert_eq!(texts, vec!["schema/one", "schema/two"]);
}

#[test]
fn max_failures_keeps_evaluation_sequential_and_deterministic() {
    let schema = multi_pattern_schema();
    let document = document(100);

    let options = parallel().with_max_failures(7);
    let one = schema.validate_with(&document, &options).unwrap();
    let two = schema.validate_with(&document, &options).unwrap();

    assert_eq!(one.count_failures(), 7);
    assert_eq!(one, two);

    // And it matches what a plainly sequential run with the same cap gives.
    let plain = schema
        .validate_with(&document, &sequential().with_max_failures(7))
        .unwrap();
    assert_eq!(one, plain);
}

#[test]
fn a_single_pattern_schema_still_works() {
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap();
    let document = Document::from_str("<a/>").unwrap();
    let report = schema.validate_with(&document, &parallel()).unwrap();
    assert_eq!(report.count_failures(), 1);
}

#[test]
fn an_error_in_one_pattern_still_surfaces() {
    // A worker that fails must not be swallowed by the join.
    // `$elsewhere` is declared, so the schema compiles, but it is bound in
    // another pattern's rule and so is out of reach here — which is the
    // runtime failure the compile-time check deliberately does not catch.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern id="fine">
               <rule context="a">
                 <let name="elsewhere" value="1"/>
                 <assert test="$elsewhere">m</assert>
               </rule>
             </pattern>
             <pattern id="broken">
               <rule context="a"><assert test="$elsewhere">m</assert></rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();
    let document = Document::from_str("<a/>").unwrap();
    let error = schema
        .validate_with(&document, &parallel())
        .unwrap_err();
    assert_contains!(error.to_string(), "$elsewhere");
}

#[test]
fn parallel_works_with_the_document_function() {
    // The document registry is shared across workers, so it must be Sync and
    // its miss recording must survive concurrent access.
    let resolver = MemoryResolver::new()
        .with("a.xml", "<doc><item/></doc>")
        .with("b.xml", "<doc/>");
    let options = SchemaOptions::new().with_resolver(Arc::new(resolver));
    let schema = Schema::from_str_with(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern id="one">
               <rule context="ref"><assert test="document(@href)/doc/item">no item in <value-of select="@href"/></assert></rule>
             </pattern>
             <pattern id="two">
               <rule context="catalog"><assert test="count(document(ref/@href)) = 2">both documents load</assert></rule>
             </pattern>
           </schema>"#,
        &options,
    )
    .unwrap();

    let document =
        Document::from_str(r#"<catalog><ref href="a.xml"/><ref href="b.xml"/></catalog>"#).unwrap();

    let one = schema.validate_with(&document, &sequential()).unwrap();
    let two = schema.validate_with(&document, &parallel()).unwrap();
    assert_eq!(one, two);
    assert_eq!(two.count_failures(), 1);
    assert_contains!(two.failures().next().unwrap().text, "b.xml");
}

#[test]
fn phases_are_honoured_under_parallel_evaluation() {
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="quick">
             <phase id="quick"><active pattern="one"/></phase>
             <pattern id="one"><rule context="a"><assert test="false()">one</assert></rule></pattern>
             <pattern id="two"><rule context="a"><assert test="false()">two</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap();
    let document = Document::from_str("<a/>").unwrap();

    let options = ValidateOptions::new().with_parallel_patterns(true);
    let report = schema.validate_with(&document, &options).unwrap();
    assert_eq!(report.count_failures(), 1);
    assert_eq!(report.failures().next().unwrap().text, "one");
}
