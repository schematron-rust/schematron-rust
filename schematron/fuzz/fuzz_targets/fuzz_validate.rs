//! Fuzz a whole validation run: schema plus document.
//!
//! The property under test: for **any** pair of inputs, validation
//! terminates and returns `Ok` or `Err`, never a panic and never a hang. This
//! is the target that exercises the interaction between the two front ends —
//! a schema whose rule contexts are hostile against a document shaped to
//! provoke them.
//!
//! Rendering is fuzzed alongside, because a report that cannot be written out
//! is no more useful than one that cannot be produced.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use schematron::validate::{PhaseSelection, ValidateOptions};
use schematron::{Document, Schema};

/// A schema and a document to run it against.
///
/// Structured rather than a raw byte split, so the fuzzer can mutate the two
/// halves independently instead of shifting a boundary through both.
#[derive(Debug, Arbitrary)]
struct Input<'a> {
    schema: &'a str,
    document: &'a str,
    all_phases: bool,
}

fuzz_target!(|input: Input<'_>| {
    let Ok(schema) = Schema::from_str(input.schema) else {
        return;
    };
    let Ok(document) = Document::from_str(input.document) else {
        return;
    };

    let mut options = ValidateOptions::new();
    if input.all_phases {
        options = options.with_phase(PhaseSelection::All);
    }
    // A cap keeps a pathological schema from producing an unbounded report
    // and being reported as a hang rather than as the slow case it is.
    options = options.with_max_failures(1_000);

    let Ok(report) = schema.validate_with(&document, &options) else {
        return;
    };

    // Every renderer must survive every report.
    let svrl = report.to_svrl();
    let _ = report.to_text();
    let _ = report.to_json();

    // SVRL that this crate emits must be XML that this crate can read back.
    let body = svrl
        .strip_prefix("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")
        .unwrap_or(&svrl);
    assert!(
        Document::from_str(body).is_ok(),
        "emitted SVRL did not reparse:\n{svrl}"
    );
});
