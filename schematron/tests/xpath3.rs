//! Integration tests for the XPath 3.0 phase-1 subset: function items,
//! inline function expressions, named function references, dynamic calls,
//! and `for-each()` — the one function that needed all of the above.
//!
//! Same two properties as `xpath2.rs`, one level up: the additions must
//! **work** under an `xslt3`/`xpath3` binding, and everything outside this
//! phase — `=>`, `||`, `filter`/`fold-left`/`fold-right`/`sort`, maps and
//! arrays — must be a **hard error naming the construct**. See
//! `spec/xpath3/`.

use assertables::*;
use schematron::{Document, Schema};

/// Wraps a body in a schema with the given query binding.
fn schema_with(binding: &str, body: &str) -> String {
    format!(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="{binding}">{body}</schema>"#
    )
}

/// Compiles an `xslt3` schema with one assertion, and validates `document`.
fn check(test: &str, document: &str) -> bool {
    let source = schema_with(
        "xslt3",
        &format!(r#"<pattern><rule context="a"><assert test="{test}">failed</assert></rule></pattern>"#),
    );
    let schema = Schema::from_str(&source)
        .unwrap_or_else(|e| panic!("schema with test {test:?} should compile: {e}"));
    let document = Document::from_str(document).expect("document should parse");
    schema.validate(&document).expect("validation should run").is_valid()
}

/// The compile error for a test under a binding, if it does not compile.
fn compile_error(binding: &str, test: &str) -> String {
    let source = schema_with(
        binding,
        &format!(r#"<pattern><rule context="a"><assert test="{test}">m</assert></rule></pattern>"#),
    );
    Schema::from_str(&source).map_or_else(
        |error| error.to_string(),
        |_| panic!("test {test:?} unexpectedly compiled under {binding}"),
    )
}

/// The evaluation error for a test under `xslt3`, if it produces one.
fn eval_error(test: &str, document: &str) -> String {
    let source = schema_with(
        "xslt3",
        &format!(r#"<pattern><rule context="a"><assert test="{test}">m</assert></rule></pattern>"#),
    );
    let schema = Schema::from_str(&source)
        .unwrap_or_else(|e| panic!("schema with test {test:?} should compile: {e}"));
    let document = Document::from_str(document).expect("document should parse");
    schema.validate(&document).map_or_else(
        |error| error.to_string(),
        |_| panic!("test {test:?} unexpectedly evaluated without error"),
    )
}

#[test]
fn xpath_three_functions_are_refused_under_earlier_bindings() {
    for binding in ["xslt", "xpath", "xslt2", "xpath2"] {
        let message = compile_error(binding, "count(for-each((1, 2), string-length#1))");
        assert_contains!(message, "XPath 3.0");
        assert_contains!(message, "xslt3");
    }
}

#[test]
fn an_inline_function_expression_is_refused_under_xpath_two() {
    let message = compile_error("xslt2", "count(function($x) { $x })");
    assert_contains!(message, "XPath 3.0");
    assert_contains!(message, "inline function");
}

#[test]
fn a_named_function_reference_is_refused_under_xpath_two() {
    let message = compile_error("xslt2", "count(string-length#1)");
    assert_contains!(message, "XPath 3.0");
    assert_contains!(message, "named function reference");
}

#[test]
fn a_dynamic_call_is_refused_under_xpath_two() {
    // `string-length#1` itself parses under any binding — the call syntax
    // is what's new. Wrapped in `(...)` so this is unambiguously a dynamic
    // call rather than a second, separate diagnosis.
    let message = compile_error("xslt2", "(true#0)()");
    assert_contains!(message, "XPath 3.0");
}

#[test]
fn for_each_maps_a_named_function_reference_over_a_sequence() {
    assert!(check(
        "sum(for-each(('a', 'bb', 'ccc'), string-length#1)) = 6",
        "<a/>"
    ));
    assert!(check("count(for-each((), string-length#1)) = 0", "<a/>"));
}

#[test]
fn for_each_maps_an_inline_function_over_a_sequence() {
    assert!(check(
        "sum(for-each((1, 2, 3), function($x) { $x * 2 })) = 12",
        "<a/>"
    ));
}

#[test]
fn for_each_works_over_a_node_set_too() {
    assert!(check(
        "sum(for-each(b, string-length#1)) = 3",
        "<a><b>x</b><b>yy</b></a>"
    ));
}

#[test]
fn an_inline_function_closes_over_its_enclosing_scope() {
    // `$n` is bound by the outer `for`, not by the inline function's own
    // parameter list — a closure sees the environment where it was
    // *written*, which is what makes it a closure rather than an ordinary
    // function.
    assert!(check(
        "sum(for $n in (10) return for-each((1, 2, 3), function($x) { $x * $n })) = 60",
        "<a/>"
    ));
}

#[test]
fn a_named_function_reference_can_be_called_directly() {
    assert!(check("string-length#1('abc') = 3", "<a/>"));
    assert!(check("(string-length#1)('abc') = 3", "<a/>"));
}

#[test]
fn dynamic_calls_chain() {
    // `function(){...}` returning another function item, called twice.
    assert!(check(
        "(function() { string-length#1 })()('abcd') = 4",
        "<a/>"
    ));
}

#[test]
fn calling_something_that_is_not_a_function_item_is_an_error() {
    let message = eval_error("(1)(2)", "<a/>");
    assert_contains!(message, "function item");
    assert_contains!(message, "number");
}

#[test]
fn calling_a_function_item_with_the_wrong_arity_is_an_error() {
    let message = eval_error("string-length#1('a', 'b')", "<a/>");
    assert_contains!(message, "1 parameter");
    assert_contains!(message, "2");
}

#[test]
fn a_function_item_cannot_be_compared() {
    for test in ["string-length#1 = 'x'", "string-length#1 &lt; 1"] {
        let message = eval_error(test, "<a/>");
        assert_contains!(message, "function item");
    }
}

#[test]
fn a_function_item_cannot_be_atomized() {
    for test in ["string(string-length#1)", "number(string-length#1)", "concat(string-length#1, 'x')"] {
        let message = eval_error(test, "<a/>");
        assert_contains!(message, "function item");
    }
}

#[test]
fn for_each_shares_the_nested_construct_budget() {
    // Same shape and same limit as the `for`/`to`-range budget test in
    // xpath2.rs: legitimate nesting keeps working, and a product asking for
    // close to a billion items is refused rather than attempted.
    assert!(check(
        "count(for-each(1 to 999, function($i) { for-each(1 to 999, function($j) { $j }) })) = 998001",
        "<a/>"
    ));
    let message = eval_error(
        "count(for-each(1 to 999, function($i) { \
           for-each(1 to 999, function($j) { for-each(1 to 999, function($k) { $k }) }) \
         }))",
        "<a/>",
    );
    assert_contains!(message, "nested");
}
