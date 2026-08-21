//! Render one validation run in all three output formats.
//!
//! A report is data, not text, so the same run can be handed to another
//! Schematron tool as SVRL, to a pipeline as JSON, or to a person as text.
//!
//! ```sh
//! cargo run --example report_formats
//! ```

use schematron::svrl::SvrlOptions;
use schematron::text::TextOptions;
use schematron::{Document, Schema};

const SCHEMA: &str = r#"
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <title>Order rules</title>
  <pattern id="lines">
    <title>Line rules</title>
    <rule context="line" flag="error">
      <assert test="@qty" diagnostics="qty-help">Every line needs a qty.</assert>
      <report test="number(@qty) &gt; 1000" flag="info">This line has an unusually large qty.</report>
    </rule>
  </pattern>
  <diagnostics>
    <diagnostic id="qty-help">Quantity is the number of units ordered.</diagnostic>
  </diagnostics>
</schema>
"#;

const DOCUMENT: &str = r#"<order><line qty="5000"/><line/></order>"#;

fn main() -> schematron::Result<()> {
    let schema = Schema::from_str(SCHEMA)?;
    let document = Document::from_str(DOCUMENT)?;
    let report = schema.validate(&document)?;

    println!("=== text ===\n");
    print!("{}", report.to_text_with(&TextOptions::verbose().with_label("order.xml")));

    println!("\n=== SVRL, findings only ===\n");
    print!("{}", report.to_svrl_with(&SvrlOptions::findings_only()));

    println!("\n=== SVRL, standard-conformant ===\n");
    print!("{}", report.to_svrl());

    println!("\n=== JSON ===\n");
    println!("{}", report.to_json().expect("a report always serialises"));

    // The report is a value, so it can be queried directly rather than
    // scraped back out of formatted text.
    println!("\n=== queried directly ===\n");
    println!("valid: {}", report.is_valid());
    println!("failures: {}", report.count_failures());
    println!("errors: {}", report.with_flag("error").count());
    println!("info: {}", report.with_flag("info").count());

    Ok(())
}
