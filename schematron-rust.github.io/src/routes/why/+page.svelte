<script lang="ts">
  import { InsetText, InformationCallout, Separator, CallToAction, SectionHeading, CodeBlock } from 'lily-design-system-svelte-headless';
  import { MSRV, REPO } from '$lib/site';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta
    name="description"
    content="Why a pure Rust Schematron implementation: no C toolchain, no FFI, no unsafe, a structurally impossible XXE, and a schema compiled once and validated in parallel."
  />
</svelte:head>

<div class="page-header">
  <h1>Why this crate</h1>
  <p>
    Schematron is a small standard with a large dependency footprint everywhere
    else. This page is the case for not having one.
  </p>
</div>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Dependencies" heading="No C, no XSLT, no JVM" level={2} />

  <p>
    The conventional way to run Schematron is to compile the schema into XSLT
    using the ISO reference stylesheets, then run that XSLT. That works, and it
    means a Rust project acquires either a <code>libxml2</code> build, an FFI
    boundary and a system package to install, or a Saxon process and a JVM to
    start.
  </p>
  <p>
    This crate implements the standard directly. The XML parser, the XPath 1.0
    engine, the schema compiler, and the validator are all Rust, in this crate,
    with no <code>unsafe</code>. The build story is <code>cargo add</code>, on
    every platform Rust targets — <code>musl</code> static binaries and
    cross-compiles included.
  </p>

  <InsetText>
    <p>
      Interpreting rather than transpiling has a second benefit: error messages
      point at your schema, not at generated XSLT. A mistake in a
      <code>@context</code> is reported as a mistake in a
      <code>@context</code>.
    </p>
  </InsetText>
</section>

<Separator label="Section break" />

<section class="section prose" id="security">
  <SectionHeading class="section-heading-start" eyebrow="Security" heading="XXE is structurally impossible" level={2} />

  <p>
    The XML parser never resolves an external entity and never processes a DTD's
    entity declarations. That is not a switch that defaults to off — the code
    path does not exist. A reference to a DTD-declared entity is an error.
  </p>
  <p>
    The default resolver reads local files and refuses <code>http:</code> and
    <code>https:</code> URIs. Network access is something an application opts
    into by supplying its own <code>Resolver</code>, never something the library
    does behind your back. The command line tool has no network flag at all.
  </p>
  <p>
    Parse depth, include depth, and expression nesting are all bounded, and
    exceeding a bound returns an error rather than exhausting the stack. Four
    <code>cargo-fuzz</code> targets exist to keep that true.
  </p>

  <InformationCallout label="Threat model note">
    <p>
      If you need a schema to pull an include over the network, that is an
      application decision, and the library makes you write it down: implement
      <code>Resolver</code>, decide what URIs you will honour, and pass it in.
    </p>
  </InformationCallout>
</section>

<Separator label="Section break" />

<section class="section prose" id="performance">
  <SectionHeading class="section-heading-start" eyebrow="Performance" heading="Compile once, validate many" level={2} />

  <p>
    Compiling is the expensive step: it resolves includes, expands abstractions,
    and parses every XPath expression in the schema. It happens once. The
    compiled schema is then reused across documents and across threads.
  </p>
  <p>
    Rule contexts are evaluated once per document rather than tested node by
    node, so matching is linear rather than quadratic in document size.
  </p>
  <p>
    <code>Schema</code> is immutable and <code>Send + Sync</code>:
  </p>

  <CodeBlock label="One schema, many threads">
    <pre><code>{`use std::sync::Arc;

let schema = Arc::new(Schema::from_path("rules.sch")?);

for path in paths {
    let schema = Arc::clone(&schema);
    std::thread::spawn(move || {
        let document = Document::from_path(path)?;
        schema.validate(&document)
    });
}`}</code></pre>
  </CodeBlock>

  <p>
    Patterns are independent, so a single schema with several of them can
    evaluate them on separate threads. The report is identical either way —
    this is a performance switch, not a behaviour switch:
  </p>

  <CodeBlock label="Parallel patterns">
    <pre><code>{`let options = ValidateOptions::new().with_parallel_patterns(true);
let report = schema.validate_with(&document, &options)?;`}</code></pre>
  </CodeBlock>

  <InsetText>
    <p>
      Measure before turning it on: on a small document the threads cost more
      than they save. Indicative figures and the benchmarks that produce them
      live in <code>spec/testing/</code>; run <code>cargo bench</code>
      yourself, because the numbers in any README are somebody else's hardware.
    </p>
  </InsetText>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Stability" heading="What you can depend on" level={2} />

  <ul>
    <li>
      <strong>MSRV of {MSRV}</strong>, under a stated policy: current stable
      minus two. Not a number that moves when a dependency feels like it.
    </li>
    <li>
      <strong>A normative specification.</strong> If the code and
      <code>spec/</code> disagree, that is a defect in one of them — and the
      test suite checks the docs against the code, so it is usually caught.
    </li>
    <li>
      <strong>Divergences stated up front.</strong> Every place this crate
      differs from the ISO reference implementation is written down, with the
      measurement that established it.
    </li>
    <li>
      <strong>Unsupported constructs error by name.</strong> Outside the
      implemented XPath 2.0 subset you get a hard error naming the construct,
      never a plausible wrong answer.
    </li>
  </ul>

  <p style="margin-top: 2rem;">
    <CallToAction class="button button-primary" href="/conformance/">See exactly what is implemented</CallToAction>
    <CallToAction class="button button-secondary" href={REPO}>Read the source</CallToAction>
  </p>
</section>
