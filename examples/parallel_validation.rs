//! Validate many documents in parallel with one compiled schema.
//!
//! A compiled `Schema` is immutable and `Send + Sync`, so sharing it across
//! threads needs nothing beyond an `Arc` — no locks, no per-thread copies,
//! and no re-parsing of the schema's XPath expressions.
//!
//! ```sh
//! cargo run --example parallel_validation
//! ```

use std::sync::Arc;
use std::thread;

use schematron::{Document, Schema};

const SCHEMA: &str = r#"
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="line">
      <assert test="number(@qty) &gt; 0">Quantity must be positive, but is <value-of select="@qty"/>.</assert>
    </rule>
  </pattern>
</schema>
"#;

/// Builds a document in which every `step`-th line is invalid.
fn document_source(index: usize) -> String {
    let mut source = String::from("<order>");
    for i in 0..100 {
        let qty = if i % (index + 2) == 0 { -1 } else { 1 };
        source.push_str(&format!("<line qty=\"{qty}\"/>"));
    }
    source.push_str("</order>");
    source
}

fn main() -> schematron::Result<()> {
    // Compile once. This is the only expensive step.
    let schema = Arc::new(Schema::from_str(SCHEMA)?);

    let handles: Vec<_> = (0..8)
        .map(|index| {
            let schema = Arc::clone(&schema);
            thread::spawn(move || -> schematron::Result<(usize, usize)> {
                let document = Document::from_str(&document_source(index))?;
                let report = schema.validate(&document)?;
                Ok((index, report.count_failures()))
            })
        })
        .collect();

    let mut total = 0;
    for handle in handles {
        let (index, failures) = handle.join().expect("worker thread should not panic")?;
        println!("document {index}: {failures} failure(s)");
        total += failures;
    }
    println!("total: {total} failure(s) across 8 documents on 8 threads");

    Ok(())
}
