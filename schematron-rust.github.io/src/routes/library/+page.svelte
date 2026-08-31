<script lang="ts">
  import { InsetText, Separator, SummaryList, SummaryListItem, CallToAction, SectionHeading, CodeBlock } from 'lily-design-system-svelte-headless';
  import { VERSION, MSRV, DOCS_RS, specUrl } from '$lib/site';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta
    name="description"
    content="Use the schematron crate from Rust: compile a schema once, validate many documents, render SVRL, JSON, or text, and query the report as data."
  />
</svelte:head>

<div class="page-header">
  <h1>Library</h1>
  <p>
    Compile a schema once; validate many documents. The full API surface is
    <a href={specUrl('api/index.md')}>spec/api/</a> and
    <a href={DOCS_RS}>docs.rs</a>.
  </p>
</div>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Install" heading="Add the crate" level={2} />

  <CodeBlock label="Add the dependency">
    <pre><code>{`cargo add schematron`}</code></pre>
  </CodeBlock>

  <SummaryList label="Crate facts">
    <SummaryListItem term="Version">{VERSION}</SummaryListItem>
    <SummaryListItem term="MSRV">{MSRV} — policy: current stable minus two</SummaryListItem>
    <SummaryListItem term="Default features"><code>serde</code>, <code>cli</code></SummaryListItem>
    <SummaryListItem term="Unsafe code">None</SummaryListItem>
    <SummaryListItem term="Licence">MIT, Apache-2.0, GPL-2.0-only, or GPL-3.0-only, at your option</SummaryListItem>
  </SummaryList>

  <InsetText>
    <p>
      Turning off the <code>cli</code> feature drops <code>clap</code>; turning
      off <code>serde</code> drops <code>serde</code>, <code>serde_json</code>,
      and <code>Report::to_json</code>. SVRL and text output are always
      available.
    </p>
  </InsetText>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="The shortest useful program" heading="Validate a document" level={2} />

  <CodeBlock label="Validate a document">
    <pre><code>{`use schematron::{Document, Schema};

fn main() -> schematron::Result<()> {
    // Compiling is the expensive step: it resolves includes, expands
    // abstractions, and parses every XPath expression in the schema.
    let schema = Schema::from_path("rules.sch")?;
    let document = Document::from_path("data.xml")?;
    let report = schema.validate(&document)?;

    if report.is_valid() {
        println!("valid");
    } else {
        for failure in report.failures() {
            println!("{}: {}", failure.location, failure.text);
        }
    }
    Ok(())
}`}</code></pre>
  </CodeBlock>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Reports" heading="A report is data, not text" level={2} />

  <p>One run renders three ways:</p>

  <CodeBlock label="Three renderings of one report">
    <pre><code>{`let svrl = report.to_svrl();   // SVRL, for other Schematron tooling
let json = report.to_json()?;  // JSON, keeping the tree structure
let text = report.to_text();   // for a person`}</code></pre>
  </CodeBlock>

  <p>And it can be queried directly, instead of scraped back out of text:</p>

  <CodeBlock label="Querying a report">
    <pre><code>{`report.is_valid();                  // no assert failed
report.count_failures();            // how many did
report.with_flag("error").count();  // findings the schema flagged as errors
report.count_fired_rules();         // zero here means NO context matched`}</code></pre>
  </CodeBlock>

  <InsetText>
    <p>
      <code>count_fired_rules()</code> returning zero is the programmatic form of
      the "my schema does nothing" symptom: no rule context matched any node.
      Assert on it in your own tests and a broken schema fails loudly instead of
      passing silently.
    </p>
  </InsetText>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Concurrency" heading="Compile once, validate in parallel" level={2} />

  <p><code>Schema</code> is immutable and <code>Send + Sync</code>:</p>

  <CodeBlock label="One schema across threads">
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
    Within one document, independent patterns can evaluate on separate threads.
    The report is identical either way:
  </p>

  <CodeBlock label="Parallel patterns">
    <pre><code>{`let options = ValidateOptions::new().with_parallel_patterns(true);
let report = schema.validate_with(&document, &options)?;`}</code></pre>
  </CodeBlock>

  <p>
    Measure before turning it on — on a small document the threads cost more
    than they save. See <a href={specUrl('validation/index.md')}>spec/validation/</a>.
  </p>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Resolvers" heading="Where includes come from" level={2} />

  <p>
    The default resolver reads local files and refuses <code>http:</code> and
    <code>https:</code>. Network access is something an application opts into by
    supplying its own <code>Resolver</code> — which is also how you serve
    includes from memory, from an embedded bundle, or from a database.
  </p>
  <p>
    <code>cargo run --example embedded_schema</code> in the repo is a worked
    example of exactly that.
  </p>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Runnable" heading="Examples in the repo" level={2} />

  <CodeBlock label="Runnable examples">
    <pre><code>{`cargo run --example validate_file        # the shortest useful program
cargo run --example report_formats       # SVRL, JSON, and text from one run
cargo run --example embedded_schema      # includes served from memory
cargo run --example parallel_validation  # one schema, eight threads
cargo run --example xpath_engine         # the XPath engine on its own`}</code></pre>
  </CodeBlock>

  <p style="margin-top: 2rem;">
    <CallToAction class="button button-primary" href={DOCS_RS}>API documentation on docs.rs</CallToAction>
    <CallToAction class="button button-secondary" href="/cli/">The command line tool</CallToAction>
  </p>
</section>
