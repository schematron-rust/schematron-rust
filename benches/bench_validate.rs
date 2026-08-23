//! End-to-end validation benchmarks.
//!
//! The `compile_once_validate_many` benchmark exists to keep the crate's
//! central design claim honest: validating N documents must not re-parse each
//! `XPath` expression N times. If that ever regresses, the gap between the two
//! numbers here closes and the benchmark says so.

// `criterion_group!` generates undocumented functions.
#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use schematron::validate::ValidateOptions;
use schematron::{Document, Schema};

const SCHEMA: &str = r#"
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <let name="tax" value="0.2"/>
  <pattern id="structure">
    <rule context="order">
      <assert test="@id">An order needs an id.</assert>
      <assert test="count(line) &gt; 0">An order needs lines.</assert>
    </rule>
  </pattern>
  <pattern id="lines">
    <rule context="line[@type='discount']">
      <assert test="number(@amount) &lt; 0">A discount must be negative.</assert>
    </rule>
    <rule context="line">
      <let name="qty" value="number(@qty)"/>
      <assert test="$qty &gt; 0">Quantity must be positive, but is <value-of select="@qty"/>.</assert>
      <assert test="sku">A line needs a sku.</assert>
    </rule>
  </pattern>
</schema>
"#;

fn document_of(count: usize) -> String {
    let mut source = String::from("<order id=\"INV-1\">");
    for i in 0..count {
        // Every tenth line fails, so the reporting path is exercised too.
        let qty = if i % 10 == 0 { -1 } else { 1 };
        source.push_str(&format!(
            "<line qty=\"{qty}\" amount=\"{}\"><sku>SKU-{i}</sku></line>",
            i % 100
        ));
    }
    source.push_str("</order>");
    source
}

fn bench_compile_schema(c: &mut Criterion) {
    c.bench_function("schema_compile", |b| {
        b.iter(|| Schema::from_str(black_box(SCHEMA)).unwrap());
    });
}

fn bench_validate(c: &mut Criterion) {
    let schema = Schema::from_str(SCHEMA).unwrap();
    let mut group = c.benchmark_group("validate");
    for count in [10_usize, 1_000, 10_000] {
        let source = document_of(count);
        let document = Document::from_str(&source).unwrap();
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &document, |b, document| {
            b.iter(|| schema.validate(black_box(document)).unwrap());
        });
    }
    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    let source = document_of(1_000);
    c.bench_function("parse_and_validate_1k", |b| {
        b.iter(|| {
            let schema = Schema::from_str(black_box(SCHEMA)).unwrap();
            let document = Document::from_str(black_box(&source)).unwrap();
            schema.validate(&document).unwrap()
        });
    });
}

/// The payoff of compiling once: the same work, with and without recompiling
/// the schema for every document.
fn bench_compile_once_validate_many(c: &mut Criterion) {
    let documents: Vec<Document> = (0..20)
        .map(|_| Document::from_str(&document_of(100)).unwrap())
        .collect();

    let mut group = c.benchmark_group("compile_once_validate_many");

    group.bench_function("compile_once", |b| {
        let schema = Schema::from_str(SCHEMA).unwrap();
        b.iter(|| {
            for document in &documents {
                black_box(schema.validate(document).unwrap());
            }
        });
    });

    group.bench_function("recompile_each_time", |b| {
        b.iter(|| {
            for document in &documents {
                let schema = Schema::from_str(SCHEMA).unwrap();
                black_box(schema.validate(document).unwrap());
            }
        });
    });

    group.finish();
}

/// Recording every fired rule costs something on a large document; this shows
/// how much, so the option is a measured trade rather than a guess.
fn bench_fired_rule_recording(c: &mut Criterion) {
    let schema = Schema::from_str(SCHEMA).unwrap();
    let document = Document::from_str(&document_of(5_000)).unwrap();

    let mut group = c.benchmark_group("fired_rule_recording");
    group.bench_function("on", |b| {
        let options = ValidateOptions::new().with_record_fired_rules(true);
        b.iter(|| schema.validate_with(black_box(&document), &options).unwrap());
    });
    group.bench_function("off", |b| {
        let options = ValidateOptions::new().with_record_fired_rules(false);
        b.iter(|| schema.validate_with(black_box(&document), &options).unwrap());
    });
    group.finish();
}

/// Rendering is separate from validating, and cheap by comparison; keeping an
/// eye on it stops a report format from quietly becoming the bottleneck.
fn bench_report_rendering(c: &mut Criterion) {
    let schema = Schema::from_str(SCHEMA).unwrap();
    let document = Document::from_str(&document_of(1_000)).unwrap();
    let report = schema.validate(&document).unwrap();

    let mut group = c.benchmark_group("report_render_1k");
    group.bench_function("svrl", |b| b.iter(|| black_box(report.to_svrl())));
    group.bench_function("text", |b| b.iter(|| black_box(report.to_text())));
    group.bench_function("json", |b| b.iter(|| black_box(report.to_json().unwrap())));
    group.finish();
}

/// A schema with eight independent patterns, to show the ceiling on parallel
/// pattern evaluation: it can be no better than the number of patterns.
fn many_pattern_schema() -> String {
    let mut source = String::from(r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">"#);
    for i in 0..8 {
        source.push_str(&format!(
            r#"<pattern id="p{i}">
                 <rule context="line">
                   <let name="qty" value="number(@qty)"/>
                   <assert test="$qty &gt; -{i} - 1">Quantity <value-of select="@qty"/> is low.</assert>
                   <assert test="sku">A line needs a sku.</assert>
                 </rule>
               </pattern>"#
        ));
    }
    source.push_str("</schema>");
    source
}

/// Parallel pattern evaluation, against the sequential baseline.
///
/// The report is identical either way, so this measures only wall-clock time.
/// The ceiling is the pattern count, and the work per pattern has to outweigh
/// starting a thread — which is why the small-document case is here too.
fn bench_parallel_patterns(c: &mut Criterion) {
    let schema = Schema::from_str(&many_pattern_schema()).unwrap();

    for count in [100_usize, 5_000] {
        let document = Document::from_str(&document_of(count)).unwrap();
        let mut group = c.benchmark_group(format!("parallel_patterns_{count}"));

        group.bench_function("sequential", |b| {
            let options = ValidateOptions::new();
            b.iter(|| schema.validate_with(black_box(&document), &options).unwrap());
        });
        group.bench_function("parallel", |b| {
            let options = ValidateOptions::new().with_parallel_patterns(true);
            b.iter(|| schema.validate_with(black_box(&document), &options).unwrap());
        });

        group.finish();
    }
}

/// A cross-reference document: `count` parts, and `count` lines referencing
/// them. This is the shape that makes keys worth having.
fn cross_reference_document(count: usize) -> String {
    let mut source = String::from("<order><parts>");
    for i in 0..count {
        source.push_str(&format!("<part id=\"P{i}\"/>"));
    }
    source.push_str("</parts>");
    for i in 0..count {
        source.push_str(&format!("<line ref=\"P{i}\"/>"));
    }
    source.push_str("</order>");
    source
}

/// The same constraint with and without a key.
///
/// Without one, `//part[@id = current()/@ref]` re-scans every part for every
/// line, so the work is quadratic. With one, the index is built once and each
/// lookup is a hash probe. The gap is the entire justification for the
/// feature, so it is measured rather than asserted.
fn bench_keys(c: &mut Criterion) {
    const WITH_KEY: &str = r#"
    <schema xmlns="http://purl.oclc.org/dsdl/schematron">
      <key name="parts" match="part" use="@id"/>
      <pattern>
        <rule context="line">
          <assert test="key('parts', @ref)">No such part.</assert>
        </rule>
      </pattern>
    </schema>
    "#;

    const WITHOUT_KEY: &str = r#"
    <schema xmlns="http://purl.oclc.org/dsdl/schematron">
      <pattern>
        <rule context="line">
          <assert test="//part[@id = current()/@ref]">No such part.</assert>
        </rule>
      </pattern>
    </schema>
    "#;

    let with_key = Schema::from_str(WITH_KEY).unwrap();
    let without_key = Schema::from_str(WITHOUT_KEY).unwrap();

    for count in [200_usize, 1_000] {
        let document = Document::from_str(&cross_reference_document(count)).unwrap();
        let mut group = c.benchmark_group(format!("cross_reference_{count}"));

        group.bench_function("with_key", |b| {
            b.iter(|| with_key.validate(black_box(&document)).unwrap());
        });
        group.bench_function("without_key", |b| {
            b.iter(|| without_key.validate(black_box(&document)).unwrap());
        });

        group.finish();
    }
}

/// Locations, which every finding carries and nothing else pays for.
///
/// A name in no namespace is written plainly, `/order[1]/line[7]`, but a
/// namespaced one needs the XPath 1.0 predicate form,
/// `*[local-name()='line' and namespace-uri()='urn:example'][7]`, which is a
/// much longer string built per finding. Both are measured against a document
/// where *every* node fails, so the cost lands on the location path rather
/// than being diluted by nodes that report nothing.
fn bench_location_generation(c: &mut Criterion) {
    const N: usize = 10_000;
    const DEPTH: usize = 300;

    let plain_schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern><rule context="line"><assert test="false()">no</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap();
    let namespaced_schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <ns prefix="e" uri="urn:example"/>
             <pattern><rule context="e:line"><assert test="false()">no</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap();

    let mut plain = String::from("<order>");
    let mut namespaced = String::from("<order xmlns:e=\"urn:example\">");
    for _ in 0..N {
        plain.push_str("<line/>");
        namespaced.push_str("<e:line/>");
    }
    plain.push_str("</order>");
    namespaced.push_str("</order>");
    let plain = Document::from_str(&plain).unwrap();
    let namespaced = Document::from_str(&namespaced).unwrap();

    // Both must actually report on every node, or this measures nothing.
    assert_eq!(plain_schema.validate(&plain).unwrap().count_failures(), N);
    assert_eq!(
        namespaced_schema.validate(&namespaced).unwrap().count_failures(),
        N
    );

    // Depth is the interesting axis: a location names every ancestor, so
    // building one is linear in depth, and a naive implementation that
    // re-copies the prefix per level is quadratic.
    let mut deep = String::from("<order>");
    for _ in 0..DEPTH {
        deep.push_str("<line>");
    }
    for _ in 0..DEPTH {
        deep.push_str("</line>");
    }
    deep.push_str("</order>");
    let deep = Document::from_str(&deep).unwrap();
    assert_eq!(plain_schema.validate(&deep).unwrap().count_failures(), DEPTH);

    let mut group = c.benchmark_group("location_generation");
    group.throughput(Throughput::Elements(N as u64));
    group.bench_function("plain", |b| {
        b.iter(|| plain_schema.validate(black_box(&plain)).unwrap());
    });
    group.bench_function("namespaced", |b| {
        b.iter(|| namespaced_schema.validate(black_box(&namespaced)).unwrap());
    });
    group.finish();

    // Not in the group above: its throughput is set for N elements, and this
    // one has DEPTH findings, so sharing the group would misreport both.
    c.bench_function("location_generation_deep", |b| {
        b.iter(|| plain_schema.validate(black_box(&deep)).unwrap());
    });
}

criterion_group!(
    benches,
    bench_compile_schema,
    bench_validate,
    bench_location_generation,
    bench_end_to_end,
    bench_compile_once_validate_many,
    bench_fired_rule_recording,
    bench_report_rendering,
    bench_parallel_patterns,
    bench_keys
);
criterion_main!(benches);
