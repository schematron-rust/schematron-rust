//! Benchmarks for the XML parser and data model.
//!
//! Parsing is the floor under every other number in this crate: nothing
//! validates faster than it parses. The sizes are chosen to show the shape of
//! the curve — a small document where fixed costs dominate, a mid-size one,
//! and a large one where allocation behaviour shows up.

// `criterion_group!` generates undocumented functions.
#![allow(missing_docs)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use schematron::xml::Document;

/// Builds a document with `count` `<line>` elements, each with attributes and
/// text, which is the shape of the data Schematron is usually pointed at.
fn document_of(count: usize) -> String {
    let mut out = String::with_capacity(count * 64);
    out.push_str("<order id=\"INV-1\">");
    for i in 0..count {
        out.push_str(&format!(
            "<line id=\"L{i}\" qty=\"{}\" amount=\"{}.00\"><sku>SKU-{i}</sku></line>",
            i % 7 + 1,
            i % 100
        ));
    }
    out.push_str("</order>");
    out
}

/// A deeply nested document, to show that depth is handled iteratively.
fn nested(depth: usize) -> String {
    let mut out = String::with_capacity(depth * 8);
    for _ in 0..depth {
        out.push_str("<n>");
    }
    out.push_str("leaf");
    for _ in 0..depth {
        out.push_str("</n>");
    }
    out
}

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("xml_parse");
    for count in [10_usize, 1_000, 100_000] {
        let source = document_of(count);
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &source, |b, source| {
            b.iter(|| Document::from_str(black_box(source)).unwrap());
        });
    }
    group.finish();
}

fn bench_nested(c: &mut Criterion) {
    let source = nested(500);
    c.bench_function("xml_parse_nested_500", |b| {
        b.iter(|| Document::from_str(black_box(&source)).unwrap());
    });
}

fn bench_string_value(c: &mut Criterion) {
    // String value walks every descendant text node, so it is the hot path
    // for any assertion that compares element content.
    let document = Document::from_str(&document_of(10_000)).unwrap();
    let root = document.document_element().unwrap();
    c.bench_function("xml_string_value_10k", |b| {
        b.iter(|| black_box(document.string_value(black_box(root))));
    });
}

fn bench_document_order(c: &mut Criterion) {
    let document = Document::from_str(&document_of(10_000)).unwrap();
    c.bench_function("xml_all_nodes_in_document_order_10k", |b| {
        b.iter(|| black_box(document.all_nodes_in_document_order().len()));
    });
}

criterion_group!(
    benches,
    bench_parse,
    bench_nested,
    bench_string_value,
    bench_document_order
);
criterion_main!(benches);
