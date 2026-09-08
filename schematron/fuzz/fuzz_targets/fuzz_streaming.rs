//! Fuzz streaming validation against ordinary validation.
//!
//! Two properties under test, for **any** pair of inputs:
//!
//! 1. `Schema::validate_streaming` terminates and returns `Ok` or `Err`,
//!    never a panic and never a hang — the same bar `fuzz_validate` holds
//!    ordinary validation to.
//! 2. When the schema is streaming-eligible, streaming and ordinary
//!    validation must agree on the report **exactly** — this is the
//!    stronger, more specific property this target exists for, and the one
//!    `tests/streaming.rs`'s hand-written cases cannot cover exhaustively.
//!    A schema `fuzz_schema` already shows compiles, run against a document
//!    `fuzz_xml` already shows parses, is exactly the input space a
//!    disagreement between the two `run_pattern` variants
//!    (`src/validate/engine.rs`) would show up in.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use schematron::validate::ValidateOptions;
use schematron::{Document, Schema};

/// A schema and a document to run it against — structured, like
/// `fuzz_validate`'s `Input`, so the fuzzer mutates the two halves
/// independently.
#[derive(Debug, Arbitrary)]
struct Input<'a> {
    schema: &'a str,
    document: &'a str,
}

fuzz_target!(|input: Input<'_>| {
    let Ok(schema) = Schema::from_str(input.schema) else {
        return;
    };

    let options = ValidateOptions::new().with_max_failures(1_000);

    let Ok(streamed) = schema.validate_streaming(input.document.as_bytes(), &options) else {
        return;
    };

    // `Document::from_bytes` strips a UTF-8 byte-order mark before
    // treating the rest as UTF-8 (`src/xml/parser.rs`); streaming does
    // not, since detecting one would mean buffering the start of the
    // input specially for a case real batch files essentially never have.
    // A BOM-prefixed document is exactly the case where the two can
    // legitimately part ways — streaming rejecting it as stray character
    // data outside the document element while whole-document parsing
    // accepts it — so that divergence is skipped here, not asserted away.
    let Ok(document) = Document::from_bytes(input.document.as_bytes()) else {
        return;
    };

    let Ok(full) = schema.validate_with(&document, &options) else {
        return;
    };

    // Reduce each report to what must match: kind, location, and message,
    // in the order findings occurred — the same comparison
    // `tests/streaming.rs`'s `assert_streaming_matches_full` makes by hand.
    let reduce = |report: &schematron::Report| -> Vec<String> {
        report
            .patterns
            .iter()
            .flat_map(|pattern| &pattern.rules)
            .flat_map(|rule| &rule.assertions)
            .map(|a| format!("{:?} {} {}", a.kind, a.location, a.text))
            .collect()
    };

    assert_eq!(
        reduce(&streamed),
        reduce(&full),
        "streaming and full validation disagreed\nschema: {:?}\ndocument: {:?}",
        input.schema,
        input.document,
    );
});
