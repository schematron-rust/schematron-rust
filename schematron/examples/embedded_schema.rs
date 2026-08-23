//! Compile a schema whose includes live in memory rather than on disk.
//!
//! `MemoryResolver` lets a schema and its includes be embedded in the binary,
//! which is what you want for a tool that must run with no schema files
//! deployed alongside it. Writing your own `Resolver` is the same shape: one
//! method, from href to source text.
//!
//! ```sh
//! cargo run --example embedded_schema
//! ```

use std::sync::Arc;

use schematron::schema::{MemoryResolver, Resolver, SchemaOptions};
use schematron::{Document, Error, Schema};

/// The top-level schema, which pulls its patterns in by reference.
const MAIN: &str = r#"
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <title>Composed rules</title>
  <include href="structure.sch"/>
  <include href="lines.sch"/>
</schema>
"#;

const STRUCTURE: &str = r#"
<pattern xmlns="http://purl.oclc.org/dsdl/schematron" id="structure">
  <rule context="order">
    <assert test="@id">An order needs an id.</assert>
  </rule>
</pattern>
"#;

const LINES: &str = r#"
<pattern xmlns="http://purl.oclc.org/dsdl/schematron" id="lines">
  <rule context="line">
    <assert test="@qty">Every line needs a qty.</assert>
  </rule>
</pattern>
"#;

/// A resolver that refuses everything, to show what a custom one looks like.
///
/// Useful as a hard boundary: a schema compiled with this cannot pull in
/// anything at all, so an `include` in untrusted input is inert.
#[derive(Debug)]
struct NoIncludes;

impl Resolver for NoIncludes {
    fn resolve(&self, href: &str, _base: Option<&str>) -> schematron::Result<String> {
        Err(Error::Resolve {
            href: href.to_string(),
            message: "includes are disabled for this schema".to_string(),
        })
    }
}

fn main() -> schematron::Result<()> {
    let resolver = MemoryResolver::new()
        .with("structure.sch", STRUCTURE)
        .with("lines.sch", LINES);

    let options = SchemaOptions::new().with_resolver(Arc::new(resolver));
    let schema = Schema::from_str_with(MAIN, &options)?;

    println!("compiled {} pattern(s) from includes", schema.patterns().len());

    let document = Document::from_str("<order><line/></order>")?;
    let report = schema.validate(&document)?;
    for failure in report.failures() {
        println!("  {} — {}", failure.location, failure.text);
    }

    // The same schema, compiled with includes switched off.
    let locked = SchemaOptions::new().with_resolver(Arc::new(NoIncludes));
    match Schema::from_str_with(MAIN, &locked) {
        Ok(_) => println!("\nunexpected: includes were resolved"),
        Err(error) => println!("\nwith includes disabled: {error}"),
    }

    Ok(())
}
