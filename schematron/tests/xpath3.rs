//! Integration tests for the XPath 3.0 subset this crate implements:
//! function items (inline function expressions, named function references,
//! dynamic calls, and `for-each()`, phase 1), the arrow operator `=>` and
//! the string concatenation operator `||` (phase 2), the rest of the
//! higher-order sequence function library plus function-item introspection
//! (`filter`, `fold-left`, `fold-right`, `for-each-pair`,
//! `function-lookup`, `function-arity`, `function-name`, phase 3), the
//! simple map operator `!` (phase 4), and the `let` expression and EQNames
//! (`Q{uri}local`, scoped to node name tests — see `spec/xpath3/`), both
//! found — and fixed in the same sitting — as accounting gaps while
//! writing phase 4's own documentation, so neither has a phase number of
//! its own.
//!
//! Same two properties as `xpath2.rs`, one level up: the additions must
//! **work** under an `xslt3`/`xpath3` binding, and everything outside the
//! implemented subset — `sort` (which is XPath 3.1, not 3.0), maps and
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

// Phase 2: the arrow operator `=>` and the string concatenation operator
// `||`.

#[test]
fn the_arrow_operator_pipes_the_left_operand_in_as_the_first_argument() {
    // `'abc' => string-length()` is sugar for `string-length('abc')`.
    assert!(check("'abc' => string-length() = 3", "<a/>"));
}

#[test]
fn arrow_calls_chain() {
    // Each `=>` pipes in the result of the one before it.
    assert!(check("'ABC' => lower-case() => string-length() = 3", "<a/>"));
}

#[test]
fn the_arrow_operator_can_call_a_dynamic_target() {
    assert!(check("for $f in string-length#1 return 'abcd' => $f() = 4", "<a/>"));
    assert!(check("'abcd' => (string-length#1)() = 4", "<a/>"));
}

#[test]
fn the_arrow_operator_passes_further_arguments_after_the_piped_in_one() {
    // `'a-b-c' => tokenize('-')` is sugar for `tokenize('a-b-c', '-')`.
    assert!(check("count('a-b-c' => tokenize('-')) = 3", "<a/>"));
}

#[test]
fn the_arrow_operator_is_refused_under_xpath_two() {
    let message = compile_error("xslt2", "'abc' => string-length()");
    assert_contains!(message, "XPath 3.0");
    assert_contains!(message, "arrow operator");
}

#[test]
fn string_concatenation_joins_the_string_values_of_both_operands() {
    assert!(check("('a' || 'b') = 'ab'", "<a/>"));
    // Each operand is atomized like `concat()`'s arguments are, so a
    // number and a node-set both convert.
    assert!(check("(1 || 2) = '12'", "<a/>"));
    assert!(check("(b || '!') = 'x!'", "<a><b>x</b></a>"));
}

#[test]
fn string_concatenation_binds_tighter_than_a_surrounding_comparison() {
    // `'a' || 'b' = 'ab'` groups as `('a' || 'b') = 'ab'`, not
    // `'a' || ('b' = 'ab')` — see the parser precedence test alongside it.
    assert!(check("'a' || 'b' = 'ab'", "<a/>"));
}

#[test]
fn string_concatenation_is_refused_under_xpath_two() {
    let message = compile_error("xslt2", "('a' || 'b') = 'ab'");
    assert_contains!(message, "XPath 3.0");
    assert_contains!(message, "||");
}

#[test]
fn string_concatenation_rejects_a_function_item() {
    let message = eval_error("(string-length#1 || 'x')", "<a/>");
    assert_contains!(message, "function item");
}

// Phase 3: the rest of the higher-order sequence function library, and
// introspection over function items.

#[test]
fn filter_keeps_items_the_predicate_holds_for() {
    assert!(check(
        "count(filter((1, 2, 3, 4, 5), function($x) { $x mod 2 = 0 })) = 2",
        "<a/>"
    ));
    assert!(check(
        "sum(filter((1, 2, 3, 4, 5), function($x) { $x mod 2 = 0 })) = 6",
        "<a/>"
    ));
    assert!(check("count(filter((), function($x) { true() })) = 0", "<a/>"));
}

#[test]
fn filter_is_refused_under_xpath_two() {
    let message = compile_error("xslt2", "filter((1, 2), function($x) { true() })");
    assert_contains!(message, "XPath 3.0");
}

#[test]
fn fold_left_combines_from_the_left() {
    assert!(check(
        "fold-left((1, 2, 3, 4), 0, function($acc, $x) { $acc + $x }) = 10",
        "<a/>"
    ));
    // Order matters: `((0 - 1) - 2) - 3 = -6`, not `1 - (2 - (3 - 0)) = 2`.
    assert!(check(
        "fold-left((1, 2, 3), 0, function($acc, $x) { $acc - $x }) = -6",
        "<a/>"
    ));
    assert!(check(
        "fold-left((), 'seed', function($acc, $x) { concat($acc, $x) }) = 'seed'",
        "<a/>"
    ));
}

#[test]
fn fold_right_combines_from_the_right() {
    // The mirror image of the `fold-left` subtraction case above:
    // `1 - (2 - (3 - 0)) = 2`.
    assert!(check(
        "fold-right((1, 2, 3), 0, function($x, $acc) { $x - $acc }) = 2",
        "<a/>"
    ));
    assert!(check(
        "fold-right((), 'seed', function($x, $acc) { concat($x, $acc) }) = 'seed'",
        "<a/>"
    ));
}

#[test]
fn for_each_pair_stops_at_the_shorter_sequence() {
    assert!(check(
        "sum(for-each-pair((1, 2, 3), (10, 20, 30, 40), function($a, $b) { $a + $b })) = 66",
        "<a/>"
    ));
    assert!(check(
        "count(for-each-pair((1, 2, 3), (10, 20, 30, 40), function($a, $b) { $a + $b })) = 3",
        "<a/>"
    ));
}

#[test]
fn function_arity_reports_a_function_items_arity() {
    assert!(check("function-arity(string-length#1) = 1", "<a/>"));
    assert!(check("function-arity(concat#3) = 3", "<a/>"));
    assert!(check(
        "function-arity(function($a, $b) { $a }) = 2",
        "<a/>"
    ));
}

#[test]
fn function_name_reports_a_named_references_name_and_nothing_for_a_closure() {
    assert!(check("function-name(string-length#1) = 'string-length'", "<a/>"));
    assert!(check(
        "empty(function-name(function($x) { $x }))",
        "<a/>"
    ));
}

#[test]
fn function_lookup_finds_a_real_function_at_the_right_arity() {
    assert!(check("function-lookup('string-length', 1)('abcd') = 4", "<a/>"));
}

#[test]
fn function_lookup_is_the_empty_sequence_when_nothing_matches() {
    for test in [
        "empty(function-lookup('not-a-real-function', 1))",
        // A real function, but not at this arity.
        "empty(function-lookup('string-length', 9))",
    ] {
        assert!(check(test, "<a/>"), "{test}");
    }
}

#[test]
fn function_lookup_rejects_a_function_item_as_either_argument() {
    let message = eval_error("function-lookup(string-length#1, 1)", "<a/>");
    assert_contains!(message, "function item");
}

#[test]
fn higher_order_functions_are_refused_under_xpath_two() {
    for test in [
        "fold-left((1), 0, function($acc, $x) { $acc })",
        "fold-right((1), 0, function($x, $acc) { $acc })",
        "for-each-pair((1), (1), function($a, $b) { $a })",
        "function-lookup('string-length', 1)",
        "function-arity(string-length#1)",
        "function-name(string-length#1)",
    ] {
        let message = compile_error("xslt2", test);
        assert_contains!(message, "XPath 3.0", "{test}");
    }
}

#[test]
fn higher_order_functions_share_the_nested_construct_budget() {
    // The same shared budget `for-each` already draws on — see
    // `for_each_shares_the_nested_construct_budget` above — is what every
    // higher-order function in this phase spends against too, not a
    // separate one per function.
    let message = eval_error(
        "count(for-each(1 to 999, function($i) { \
           filter(1 to 999, function($j) { \
             count(for-each(1 to 999, function($k) { $k })) > 0 \
           }) \
         }))",
        "<a/>",
    );
    assert_contains!(message, "nested");
}

// Phase 4: the simple map operator `!`.

#[test]
fn simple_map_over_atomic_items_binds_dot_to_each_one() {
    assert!(check("sum((1, 2, 3) ! (. * 2)) = 12", "<a/>"));
    assert!(check("(('a', 'bb', 'ccc') ! string-length(.)) = (1, 2, 3)", "<a/>"));
}

#[test]
fn simple_map_over_a_node_set_binds_dot_to_each_node() {
    assert!(check(
        "sum(b ! string-length(.)) = 3",
        "<a><b>x</b><b>yy</b></a>"
    ));
    // `.` is a genuine node here, so ordinary axis steps from it work too,
    // exactly as they would in a predicate.
    assert!(check(
        "(b ! local-name(.)) = ('b', 'b')",
        "<a><b>x</b><b>yy</b></a>"
    ));
}

#[test]
fn simple_map_chains() {
    // Each `!` feeds the next: `((1 to 3) ! (. * 2)) ! (. + 1)`.
    //
    // The parentheses around `1 to 3` matter: `to` binds looser than `!` in
    // this grammar, exactly as real XPath 3.0's does (`RangeExpr` sits
    // above `SimpleMapExpr`), so the unparenthesised `1 to 3 ! (. * 2)`
    // would mean `1 to (3 ! (. * 2))` instead.
    assert!(check("sum((1 to 3) ! (. * 2) ! (. + 1)) = 15", "<a/>"));
}

#[test]
fn simple_map_resets_context_position_and_size_to_one_each_time() {
    // Unlike an axis step, `!` always presents a singleton to its right
    // side — `position()` is 1 for every item, never the item's index in
    // the sequence being mapped.
    assert!(check("sum((10, 20, 30) ! position()) = 3", "<a/>"));
    assert!(check("sum((10, 20, 30) ! last()) = 3", "<a/>"));
}

#[test]
fn simple_map_over_a_non_node_item_rejects_a_real_axis_step() {
    // `.` alone resolves to the atomic item, but a step wanting an axis to
    // walk has no node to walk it from.
    let message = eval_error("(1, 2, 3) ! (child::x)", "<a/>");
    assert_contains!(message, "node");
}

#[test]
fn the_simple_map_operator_is_refused_under_xpath_two() {
    let message = compile_error("xslt2", "(1, 2, 3) ! (. * 2)");
    assert_contains!(message, "XPath 3.0");
    assert_contains!(message, "simple map operator");
}

#[test]
fn simple_map_shares_the_nested_construct_budget() {
    let message = eval_error(
        "count(for-each(1 to 999, function($i) { \
           (1 to 999) ! for-each(1 to 999, function($k) { $k }) \
         }))",
        "<a/>",
    );
    assert_contains!(message, "nested");
}

// `let` — found as a documentation gap while writing up phase 4, not a
// phase of its own; see the file-level doc comment.

#[test]
fn let_binds_a_name_to_a_value() {
    assert!(check("let $x := 5 return $x + 1 = 6", "<a/>"));
}

#[test]
fn let_binds_the_whole_sequence_not_one_item_at_a_time() {
    // The defining difference from `for`: `for $x in (1,2,3) return
    // count($x)` is `(1, 1, 1)`, because `for` rebinds `$x` to each item;
    // `let` binds it once, to the sequence as a whole.
    assert!(check("let $x := (1, 2, 3) return count($x) = 3", "<a/>"));
}

#[test]
fn nested_let_expressions_shadow_and_can_reference_the_outer_binding() {
    assert!(check("(let $x := 1 return let $x := 2 return $x) = 2", "<a/>"));
    assert!(check(
        "(let $x := 1 return let $y := $x + 1 return $y) = 2",
        "<a/>"
    ));
}

#[test]
fn let_is_refused_under_xpath_two() {
    let message = compile_error("xslt2", "let $x := 1 return $x");
    assert_contains!(message, "XPath 3.0");
    assert_contains!(message, "let");
}

// EQNames: `Q{uri}local`, XPath 3.0's namespace-URI-direct name form —
// found alongside `let`, not a phase of its own. Scoped to node name tests
// (element/attribute steps); see `spec/xpath3/` for what that leaves out.

#[test]
fn eqname_selects_by_namespace_uri_directly() {
    let source = schema_with(
        "xslt3",
        r#"<ns prefix="p" uri="http://example.com/ns"/>
           <pattern><rule context="a">
             <assert test="count(Q{http://example.com/ns}foo) = 1">failed</assert>
           </rule></pattern>"#,
    );
    let schema = Schema::from_str(&source).expect("schema should compile");
    let document = Document::from_str(r#"<a xmlns:p="http://example.com/ns"><p:foo/></a>"#)
        .expect("document should parse");
    assert!(schema.validate(&document).expect("validation should run").is_valid());
}

#[test]
fn eqname_and_a_prefix_resolving_to_the_same_uri_select_the_same_node() {
    // The whole point of an EQName: it names the same expanded name a
    // declared prefix would, without needing that prefix declared.
    let source = schema_with(
        "xslt3",
        r#"<ns prefix="p" uri="http://example.com/ns"/>
           <pattern><rule context="a">
             <assert test="Q{http://example.com/ns}foo = p:foo">failed</assert>
           </rule></pattern>"#,
    );
    let schema = Schema::from_str(&source).expect("schema should compile");
    let document = Document::from_str(r#"<a xmlns:p="http://example.com/ns"><p:foo>x</p:foo></a>"#)
        .expect("document should parse");
    assert!(schema.validate(&document).expect("validation should run").is_valid());
}

#[test]
fn eqname_with_an_empty_uri_means_no_namespace() {
    // `Q{}local` means the same as an unprefixed `local` — no namespace —
    // not "resolve an empty prefix."
    assert!(check("count(Q{}b) = 1", "<a><b/></a>"));
    assert!(check("@Q{}id = '5'", "<a id='5'/>"));
}

#[test]
fn an_element_named_q_still_parses_as_a_name() {
    assert!(check("count(Q) = 1", "<a><Q/></a>"));
}

#[test]
fn eqname_is_refused_under_xpath_two() {
    let message = compile_error("xslt2", "Q{}foo");
    assert_contains!(message, "XPath 3.0");
    assert_contains!(message, "EQName");
}
