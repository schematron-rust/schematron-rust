//! Public XPath evaluation edges that compiled Schematron schemas normally
//! prevent from reaching the evaluator directly.

use schematron::xml::Document;
use schematron::xpath::{
    evaluate, parse, Documents, EvalContext, Expr, Keys, Namespaces, Value, Variables, XPathVersion,
};

fn evaluate_source(
    source: &str,
    document: &Document,
    variables: &Variables,
    namespaces: &Namespaces,
    version: XPathVersion,
) -> Result<Value, String> {
    let expression = parse(source).map_err(|error| error.to_string())?;
    let context = EvalContext::new(
        document,
        document.document_element().unwrap(),
        variables,
        namespaces,
    )
    .with_version(version)
    .with_current_time(0.0);
    evaluate(&expression, &context).map_err(|error| error.to_string())
}

#[test]
fn optional_core_arguments_and_runtime_errors_are_total() {
    let document = Document::from_str("<a>12</a>").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();

    assert!(evaluate_source(
        "boolean('x')",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V1,
    )
    .unwrap()
    .to_boolean());
    assert_eq!(
        evaluate_source(
            "number()",
            &document,
            &variables,
            &namespaces,
            XPathVersion::V1
        )
        .unwrap()
        .to_number(&document),
        12.0
    );
    assert_eq!(
        evaluate_source(
            "string()",
            &document,
            &variables,
            &namespaces,
            XPathVersion::V1
        )
        .unwrap()
        .to_xpath_string(&document),
        "12"
    );
    assert!(evaluate_source(
        "name('x')",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V1
    )
    .unwrap_err()
    .contains("node-set"));
    assert!(evaluate_source(
        "sum(1)",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V1
    )
    .unwrap_err()
    .contains("node-set"));
    assert!(evaluate_source(
        "count(1)",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V1
    )
    .unwrap_err()
    .contains("node-set"));

    let unknown = Expr::Function {
        name: "unknown-at-runtime".into(),
        args: vec![],
    };
    let context = EvalContext::new(
        &document,
        document.document_element().unwrap(),
        &variables,
        &namespaces,
    );
    assert!(evaluate(&unknown, &context)
        .unwrap_err()
        .to_string()
        .contains("unknown function"));
}

#[test]
fn xpath_two_atomic_deep_equality_covers_each_supported_type() {
    let document = Document::from_str("<a/>").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();
    for expression in [
        "deep-equal((true(), false()), (true(), false()))",
        "deep-equal((xs:date('2024-01-01')), (xs:date('2024-01-01')))",
        "deep-equal((xs:dayTimeDuration('PT1S')), (xs:dayTimeDuration('PT1S')))",
        "exists(true())",
        "timezone-from-time(xs:time('12:00:00Z')) instance of xs:dayTimeDuration",
    ] {
        assert!(
            evaluate_source(
                expression,
                &document,
                &variables,
                &namespaces,
                XPathVersion::V2,
            )
            .unwrap()
            .to_boolean(),
            "{expression}"
        );
    }
}

#[test]
fn document_and_key_report_bad_inputs_and_accept_scalar_lookups() {
    let document = Document::from_str("<a><b id='x'/></a>").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();
    let element = document.document_element().unwrap();
    let b = document.children(element)[0];

    let documents = Documents::new();
    let context =
        EvalContext::new(&document, element, &variables, &namespaces).with_documents(&documents);
    let expression = parse("document('parts.xml', 'not-a-node-set')").unwrap();
    assert!(evaluate(&expression, &context)
        .unwrap_err()
        .to_string()
        .contains("second argument"));

    let mut keys = Keys::new();
    keys.insert("parts", "x", b);
    let context = EvalContext::new(&document, element, &variables, &namespaces).with_keys(&keys);
    let result = evaluate(&parse("key('parts', 'x')").unwrap(), &context).unwrap();
    assert_eq!(result.as_node_set(), Some(&[b][..]));
}

#[test]
fn dynamic_calls_reject_every_invalid_target_cardinality_and_type() {
    let document = Document::from_str("<a/>").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();
    let context = EvalContext::new(
        &document,
        document.document_element().unwrap(),
        &variables,
        &namespaces,
    )
    .with_version(XPathVersion::V3);

    for function in [
        Expr::Sequence(vec![]),
        Expr::Sequence(vec![Expr::Literal("one".into())]),
        Expr::Sequence(vec![
            Expr::Literal("one".into()),
            Expr::Literal("two".into()),
        ]),
        Expr::Literal("scalar".into()),
    ] {
        let call = Expr::DynamicCall {
            function: Box::new(function),
            args: vec![],
        };
        assert!(evaluate(&call, &context)
            .unwrap_err()
            .to_string()
            .contains("dynamic call's target"));
    }
}

#[test]
fn public_evaluator_reports_mixed_value_comparisons_and_non_node_paths() {
    let document = Document::from_str("<a xmlns:p='urn:p'><p:b/></a>").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();

    for operator in ["ne", "lt", "le", "gt", "ge"] {
        let expression = format!("1 {operator} '1'");
        let error = evaluate_source(
            &expression,
            &document,
            &variables,
            &namespaces,
            XPathVersion::V2,
        )
        .unwrap_err();
        assert!(error.contains("same type"), "{expression}: {error}");
    }

    assert!(evaluate_source(
        "string('x')/child::*",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V2,
    )
    .unwrap_err()
    .contains("path can only continue from a node-set"));

    let namespaces_on_element = evaluate_source(
        "namespace::*",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V2,
    )
    .unwrap();
    assert!(!namespaces_on_element.as_node_set().unwrap().is_empty());

    assert!(evaluate_source(
        "missing:*",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V2,
    )
    .unwrap()
    .as_node_set()
    .unwrap()
    .is_empty());
}

#[test]
fn xpath_two_function_failures_preserve_argument_contracts() {
    let document =
        Document::from_str("<root><a x='1'>same</a><a x='2'>same</a><b>same</b></root>").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();

    for expression in [
        "function-lookup('concat', -1)",
        "adjust-date-to-timezone(xs:date('2024-01-01'), xs:dayTimeDuration('PT15H'))",
        "adjust-date-to-timezone(xs:date('2024-01-01'), xs:yearMonthDuration('P1M'))",
    ] {
        assert!(evaluate_source(
            expression,
            &document,
            &variables,
            &namespaces,
            XPathVersion::V3,
        )
        .is_err());
    }

    for expression in [
        "empty(timezone-from-date(xs:date('2024-01-01')))",
        "deep-equal(/, /)",
        "deep-equal(/root/a[1]/@x, /root/a[1]/@x)",
        "xs:date(xs:date('2024-01-01')) eq xs:date('2024-01-01')",
        "xs:dayTimeDuration(xs:dayTimeDuration('PT1S')) eq xs:dayTimeDuration('PT1S')",
        "not(deep-equal(/root/a[1], /root/b))",
        "not(deep-equal(/root/a[1], /root/a[2]))",
    ] {
        assert!(evaluate_source(
            expression,
            &document,
            &variables,
            &namespaces,
            XPathVersion::V2,
        )
        .unwrap()
        .to_boolean());
    }
}

#[test]
fn direct_function_dispatch_propagates_each_helper_error() {
    let document = Document::from_str("<a x='1'/>").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();

    for expression in [
        "local-name('not-a-node-set')",
        "namespace-uri('not-a-node-set')",
        "replace('a', '[', 'x')",
        "tokenize('a', '[')",
        "for-each((1), 1)",
        "filter((1), 1)",
        "fold-left((1), 0, 1)",
        "fold-right((1), 0, 1)",
        "for-each-pair((1), (2), 1)",
        "function-arity(1)",
        "function-name(1)",
        "function-lookup('string', string#1)",
        "years-from-duration('bad')",
        "timezone-from-date('bad')",
        "year-from-date('bad')",
    ] {
        assert!(
            evaluate_source(
                expression,
                &document,
                &variables,
                &namespaces,
                XPathVersion::V3,
            )
            .is_err(),
            "{expression} unexpectedly succeeded"
        );
    }

    assert!(!evaluate_source(
        "lang('en')",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V1,
    )
    .unwrap()
    .to_boolean());
}

#[test]
fn evaluator_propagates_errors_from_each_composite_position() {
    let document = Document::from_str("<a><b/></a>").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();

    for expression in [
        "true() and $missing",
        "false() or $missing",
        "1 + $missing",
        "xs:date('2024-01-01') eq 'bad'",
        ". is (., .)",
        "1 eq (1, 2)",
        "($missing)/a",
        "(.)[$missing]/a",
        "b[$missing]",
    ] {
        assert!(
            evaluate_source(
                expression,
                &document,
                &variables,
                &namespaces,
                XPathVersion::V3,
            )
            .is_err(),
            "{expression} unexpectedly succeeded"
        );
    }

    assert!(evaluate_source(
        "xs:yearMonthDuration('P3M') - xs:yearMonthDuration('P1M') eq xs:yearMonthDuration('P2M')",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V2,
    )
    .unwrap()
    .to_boolean());
    assert!(evaluate_source(
        "xs:yearMonthDuration('P1M') + xs:yearMonthDuration('P2M') eq xs:yearMonthDuration('P3M')",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V2,
    )
    .unwrap()
    .to_boolean());
    assert!(evaluate_source(
        ". is .",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V2,
    )
    .unwrap()
    .to_boolean());
    assert!(evaluate_source(
        ". is ()",
        &document,
        &variables,
        &namespaces,
        XPathVersion::V2,
    )
    .unwrap()
    .as_sequence()
    .unwrap()
    .is_empty());

    for expression in [
        "fold-left((1), 0, function($a, $b) { $missing })",
        "fold-right((1), 0, function($a, $b) { $missing })",
        "for-each-pair((1), (2), function($a, $b) { $missing })",
        "$missing ! .",
        "some $x in 1 satisfies $missing",
        "xs:date('2024-01-01') = 'bad'",
    ] {
        assert!(evaluate_source(
            expression,
            &document,
            &variables,
            &namespaces,
            XPathVersion::V3,
        )
        .is_err());
    }

    assert_eq!(
        evaluate_source(
            "(.)[true()]/b",
            &document,
            &variables,
            &namespaces,
            XPathVersion::V3,
        )
        .unwrap()
        .as_node_set()
        .unwrap()
        .len(),
        1
    );
}
