//! Validate a document against a schema, and report what was found.
//!
//! The shortest useful program you can write with this crate.
//!
//! ```sh
//! cargo run --example validate_file
//! cargo run --example validate_file -- examples/invoice.sch examples/invoice-bad.xml
//! ```

use std::process::ExitCode;

use schematron::validate::{PhaseSelection, ValidateOptions};
use schematron::{Document, Schema};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (schema_path, document_path) = match args.len() {
        0 => ("examples/invoice.sch", "examples/invoice-bad.xml"),
        2 => (args[0].as_str(), args[1].as_str()),
        _ => {
            eprintln!("usage: validate_file [SCHEMA.sch DOCUMENT.xml]");
            return ExitCode::from(2);
        }
    };

    // Compiling is the expensive step: it parses the schema, resolves its
    // includes, expands its abstractions, and parses every XPath expression
    // in it. Do it once and reuse the result.
    let schema = match Schema::from_path(schema_path) {
        Ok(schema) => schema,
        Err(error) => {
            eprintln!("schema error: {error}");
            return ExitCode::from(3);
        }
    };

    let document = match Document::from_path(document_path) {
        Ok(document) => document,
        Err(error) => {
            eprintln!("document error: {error}");
            return ExitCode::from(4);
        }
    };

    // `#ALL` runs every pattern, ignoring the schema's default phase.
    let options = ValidateOptions::new().with_phase(PhaseSelection::All);
    let report = match schema.validate_with(&document, &options) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("validation error: {error}");
            return ExitCode::from(3);
        }
    };

    println!("{document_path}");
    println!("  rules fired: {}", report.count_fired_rules());

    for failure in report.failures() {
        println!("  FAIL {}", failure.location);
        println!("       {}", failure.text.trim());
        // A diagnostic is the long explanation the schema attached to this
        // assertion, instantiated against the node that failed.
        for diagnostic in &failure.diagnostics {
            println!("       ({})", diagnostic.text.trim());
        }
    }

    // A successful report is an observation, not a failure: it does not make
    // the document invalid, and it does not belong in the exit code.
    for observation in report.reports() {
        println!("  NOTE {}", observation.location);
        println!("       {}", observation.text.trim());
    }

    if report.is_valid() {
        println!("  valid");
        ExitCode::SUCCESS
    } else {
        println!("  {} failed assertion(s)", report.count_failures());
        ExitCode::from(1)
    }
}
