//! Benchmarks for the `XPath` engine, split into compiling and evaluating.
//!
//! The split matters because the crate's central performance claim is that
//! compiling happens once per schema while evaluating happens once per node.
//! If the two ever drift together, these numbers say so.

// `criterion_group!` generates undocumented functions.
#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use schematron::xml::Document;
use schematron::xpath::{evaluate, parse, EvalContext, Namespaces, Variables};

/// Expressions from trivial to the kind of thing real schemas contain.
const EXPRESSIONS: &[(&str, &str)] = &[
    ("simple_name", "line"),
    ("attribute", "@qty"),
    ("descendant", ".//sku"),
    ("predicate", "line[@qty > 0]"),
    ("function", "count(line[number(@amount) > 10])"),
    (
        "compound",
        "count(line[@qty > 0 and starts-with(sku, 'SKU-')]) > 0 and sum(line/@amount) < 10000",
    ),
];

fn sample_document(count: usize) -> Document {
    let mut source = String::from("<order>");
    for i in 0..count {
        source.push_str(&format!(
            "<line qty=\"{}\" amount=\"{}\"><sku>SKU-{i}</sku></line>",
            i % 7 + 1,
            i % 100
        ));
    }
    source.push_str("</order>");
    Document::from_str(&source).unwrap()
}

fn bench_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("xpath_compile");
    for (name, expression) in EXPRESSIONS {
        group.bench_function(*name, |b| {
            b.iter(|| parse(black_box(expression)).unwrap());
        });
    }
    group.finish();
}

fn bench_evaluate(c: &mut Criterion) {
    let document = sample_document(1_000);
    let node = document.document_element().unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();

    let mut group = c.benchmark_group("xpath_evaluate_1k");
    for (name, expression) in EXPRESSIONS {
        let expr = parse(expression).unwrap();
        group.bench_function(*name, |b| {
            b.iter(|| {
                let context = EvalContext::new(&document, node, &variables, &namespaces);
                evaluate(black_box(&expr), &context).unwrap()
            });
        });
    }
    group.finish();
}

fn bench_axes(c: &mut Criterion) {
    let document = sample_document(500);
    let node = document.document_element().unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();

    // The axes differ enormously in cost: `child` is a slice, while
    // `following` and `preceding` scan the document.
    let mut group = c.benchmark_group("xpath_axes_500");
    for axis in [
        "child::*",
        "descendant::*",
        "descendant-or-self::node()",
        "line[1]/following-sibling::*",
        "line[250]/following::*",
        "line[250]/preceding::*",
        "line[250]/sku/ancestor::*",
    ] {
        let expr = parse(axis).unwrap();
        group.bench_function(axis, |b| {
            b.iter(|| {
                let context = EvalContext::new(&document, node, &variables, &namespaces);
                evaluate(black_box(&expr), &context).unwrap()
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_compile, bench_evaluate, bench_axes);
criterion_main!(benches);
