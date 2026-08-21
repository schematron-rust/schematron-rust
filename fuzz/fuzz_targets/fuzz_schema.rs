//! Fuzz schema compilation.
//!
//! The property under test: for **any** string, compiling a schema returns
//! `Ok` or `Err`, never a panic. This covers the whole front end at once —
//! XML parsing, the content-model checks, abstract pattern and rule
//! expansion, and expression compilation — including their cycle and depth
//! guards.
//!
//! A schema is often supplied by a user, or assembled from includes, so the
//! same "malformed is an error, not a crash" rule applies as for documents.

#![no_main]

use libfuzzer_sys::fuzz_target;
use schematron::Schema;

fuzz_target!(|data: &str| {
    let Ok(schema) = Schema::from_str(data) else {
        return;
    };

    // A schema that compiled must be inspectable without panicking.
    let _ = schema.id();
    let _ = schema.title();
    let _ = schema.default_phase();
    let _ = schema.phases().count();
    for pattern in schema.patterns() {
        for rule in &pattern.rules {
            let _ = rule.assertions().count();
        }
    }
});
