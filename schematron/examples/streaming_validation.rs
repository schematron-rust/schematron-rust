//! Validate a document too large to comfortably hold fully in memory, one
//! repeating record at a time.
//!
//! Only for a schema whose active patterns are provably local to one
//! record's own subtree — `Schema::streaming_eligible` checks this once, at
//! compile time, and names the reason when it isn't. See spec/streaming/.
//!
//! ```sh
//! cargo run --example streaming_validation
//! ```

use schematron::{Schema, ValidateOptions};

const SCHEMA: &str = r#"
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="order">
      <assert test="@id">An order must have an id.</assert>
      <assert test="sum(line/@amount) = @total">The line amounts must sum to @total.</assert>
    </rule>
  </pattern>
</schema>
"#;

/// Builds an `<orders>` document with `count` `<order>` records — the
/// streaming record boundary is the document element's direct children —
/// where every seventh order is missing its id, so there is something to
/// find without materialising the whole thing to check.
fn orders_of(count: usize) -> String {
    let mut source = String::from("<orders>");
    for i in 0..count {
        if i % 7 == 0 {
            source.push_str("<order total=\"10.00\"><line amount=\"10.00\"/></order>");
        } else {
            source.push_str(&format!(
                "<order id=\"{i}\" total=\"10.00\"><line amount=\"10.00\"/></order>"
            ));
        }
    }
    source.push_str("</orders>");
    source
}

fn main() -> schematron::Result<()> {
    let schema = Schema::from_str(SCHEMA)?;

    // Checked once, before any document is read — a schema that isn't
    // streaming-eligible is a compile-time fact, not something discovered
    // partway through a large file.
    schema
        .streaming_eligible()
        .expect("this schema's rules are local to one record's own subtree");

    let document = orders_of(1_000);
    let report = schema.validate_streaming(document.as_bytes(), &ValidateOptions::new())?;

    println!(
        "{} record(s), {} failure(s), never holding the whole document in memory",
        1_000,
        report.count_failures()
    );
    for failure in report.failures().take(3) {
        println!("  {}: {}", failure.location, failure.text);
    }

    Ok(())
}
