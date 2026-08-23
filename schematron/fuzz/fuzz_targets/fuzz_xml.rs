//! Fuzz the XML parser.
//!
//! The property under test: for **any** input bytes, parsing returns `Ok` or
//! `Err`. It never panics, never overflows the stack, and never hangs. A
//! validator is often pointed at input from outside the trust boundary, so
//! "malformed input is an error, not a crash" has to hold unconditionally.
//!
//! When it does parse, the resulting tree must also be self-consistent, so
//! the target walks it: every node reachable, every string value computable,
//! every location renderable.

#![no_main]

use libfuzzer_sys::fuzz_target;
use schematron::xml::Document;

fuzz_target!(|data: &[u8]| {
    let Ok(document) = Document::from_bytes(data) else {
        // A rejected document is a correct outcome, not a failure.
        return;
    };

    // The tree parsed, so every accessor on it must hold up.
    for node in document.all_nodes_in_document_order() {
        let _ = document.kind(node);
        let _ = document.name(node);
        let _ = document.string_value(node);
        let _ = document.location(node);
        let _ = document.ancestors(node);

        // Document order must be internally consistent: a node's subtree
        // range must contain it and every one of its children.
        assert!(document.order(node) <= document.subtree_end(node));
        for &child in document.children(node) {
            assert!(document.is_descendant_of(child, node));
            assert_eq!(document.parent(child), Some(node));
        }
    }
});
