//! Fuzz the XPath lexer and parser.
//!
//! The property under test: for **any** string, `parse` returns `Ok` or
//! `Err`, never a panic and never a stack overflow. Deeply nested input is
//! the interesting case, because the parser is recursive descent — it is
//! guarded by `MAX_RECURSION_DEPTH`, and this target is what keeps that guard
//! honest.
//!
//! Anything that parses is then evaluated against a small document, so the
//! evaluator is exercised too: evaluation may fail, but it may not panic.

#![no_main]

use libfuzzer_sys::fuzz_target;
use schematron::xml::Document;
use schematron::xpath::{evaluate, parse, EvalContext, Namespaces, Variables};

fuzz_target!(|data: &str| {
    let Ok(expr) = parse(data) else {
        return;
    };

    let document = Document::from_str("<a x='1'>text<b y='2'><c/></b><!--c--><?pi d?></a>")
        .expect("the fixture document is well-formed");
    let variables = Variables::new();
    let namespaces = Namespaces::new();
    let context = EvalContext::new(
        &document,
        document.document_element().unwrap(),
        &variables,
        &namespaces,
    );

    // May legitimately fail — an unbound variable, an undeclared prefix — but
    // must not panic. Conversions on the result must be total, too.
    if let Ok(value) = evaluate(&expr, &context) {
        let _ = value.to_boolean();
        let _ = value.to_number(&document);
        let _ = value.to_xpath_string(&document);
    }
});
