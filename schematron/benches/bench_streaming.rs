//! Benchmarks for streaming validation.
//!
//! The point of `--stream`/`Schema::validate_streaming` is bounded *memory*,
//! not raw speed — criterion measures wall-clock, not resident set, so the
//! throughput numbers here show that streaming is not slower than ordinary
//! validation at any of these sizes, not that it uses less memory. Check
//! peak memory externally (`/usr/bin/time -l` on macOS, `valgrind --tool=massif`
//! elsewhere) if that is what a change here needs to justify.
//!
//! Measured once already, off a million-record document (`orders_of`, this
//! file): full validation held **2.51 GB** resident at the end; streaming
//! held **144 MB** — but naively trusting resident-set alone would have been
//! misleading in the other direction too. Resident set for *streaming*
//! crept up with record count as well (10k → 8.3 MB, 1M → 282 MB) even
//! though nothing in the design should scale that way, so before believing
//! "bounded" a custom global allocator counted bytes actually **live**
//! (allocated, not yet freed) at the moment `validate_streaming` returned:
//! identical — 6,275 bytes above baseline — at 10,000, 100,000, *and*
//! 1,000,000 records. The resident-set creep is the system allocator
//! retaining freed pages from many small alloc/free cycles rather than
//! returning them to the OS — ordinary allocator behavior under this
//! access pattern, not a leak — and it is exactly why "live bytes,
//! measured directly" is the claim this crate stands behind, not "resident
//! set, read off a process monitor."

// `criterion_group!` generates undocumented functions.
#![allow(missing_docs)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use schematron::validate::ValidateOptions;
use schematron::{Document, Schema};
use std::hint::black_box;

const SCHEMA: &str = r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="order">
      <assert test="@id">An order must have an id.</assert>
      <assert test="sum(line/@amount) = @total">The lines must sum to @total.</assert>
    </rule>
  </pattern>
</schema>"#;

/// An `<orders>` document of `count` `<order>` records, each with a few
/// `<line>` children — a plausible batch-file shape, and streaming's own
/// record boundary (`spec/streaming/`): direct children of the document
/// element.
fn orders_of(count: usize) -> String {
    let mut out = String::with_capacity(count * 96);
    out.push_str("<orders>");
    for i in 0..count {
        out.push_str(&format!(
            "<order id=\"{i}\" total=\"{}.00\"><line amount=\"{}.00\"/><line amount=\"{}.00\"/></order>",
            (i % 50) * 2,
            i % 50,
            i % 50,
        ));
    }
    out.push_str("</orders>");
    out
}

fn bench_full_validate_orders(c: &mut Criterion) {
    let schema = Schema::from_str(SCHEMA).unwrap();
    let options = ValidateOptions::new();
    let mut group = c.benchmark_group("orders_full_validate");
    for count in [10_usize, 1_000, 100_000] {
        let source = orders_of(count);
        let document = Document::from_str(&source).unwrap();
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &document, |b, document| {
            b.iter(|| schema.validate_with(black_box(document), &options).unwrap());
        });
    }
    group.finish();
}

fn bench_streaming_validate_orders(c: &mut Criterion) {
    let schema = Schema::from_str(SCHEMA).unwrap();
    let options = ValidateOptions::new();
    let mut group = c.benchmark_group("orders_streaming_validate");
    for count in [10_usize, 1_000, 100_000] {
        let source = orders_of(count);
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &source, |b, source| {
            b.iter(|| {
                schema
                    .validate_streaming(black_box(source.as_bytes()), &options)
                    .unwrap()
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_full_validate_orders, bench_streaming_validate_orders);
criterion_main!(benches);
