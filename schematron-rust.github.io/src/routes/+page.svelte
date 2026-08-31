<script lang="ts">
  import { Card, Alert, Details, Tag, Separator, CallToAction, SectionHeading, CodeBlock } from 'lily-design-system-svelte-headless';
  import { VERSION, REPO, SPEC_DOCS } from '$lib/site';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta
    name="description"
    content="Rule-based XML validation to ISO/IEC 19757-3, in pure Rust: its own XML parser, its own XPath 1.0 engine, and no libxml2, no XSLT processor, no C toolchain, no FFI."
  />
</svelte:head>

<section class="hero">
  <p class="hero-eyebrow">ISO/IEC 19757-3 &middot; {VERSION}</p>
  <h1>Schematron in pure Rust.</h1>
  <p class="hero-tagline">
    Rule-based XML validation with no <code>libxml2</code>, no XSLT processor,
    no C toolchain, and no FFI. Its own XML parser, its own XPath engine, and a
    validator that interprets a schema directly instead of transpiling it.
  </p>
  <div class="button-row">
    <CallToAction class="button button-primary" href="/tutorial/">Start the tutorial</CallToAction>
    <CallToAction class="button button-secondary" href="/library/">Use the library</CallToAction>
    <CallToAction class="button button-secondary" href="/cli/">Use the CLI</CallToAction>
  </div>
  <div class="tag-row" style="justify-content: center;">
    <Tag label="No unsafe code">no <code>unsafe</code></Tag>
    <Tag label="Cross reference tables">XXE structurally impossible</Tag>
    <Tag label="Send and Sync schema"><code>Schema</code> is <code>Send + Sync</code></Tag>
    <Tag label="Report formats">SVRL, JSON, and text</Tag>
  </div>
</section>

<section class="section">
  <SectionHeading eyebrow="The problem" heading="Every other route to Schematron in Rust goes through C" />
  <div class="prose prose-center">
    <p>
      Bind to <code>libxml2</code>. Or shell out to Saxon. Or compile the schema
      into XSLT and find an XSLT engine to run it. Each choice drags a C
      toolchain, an FFI boundary, or a JVM into a project that had none.
    </p>
    <p>
      This crate does none of that. It contains its own XML parser, its own
      XPath 1.0 engine, and its own validator, and it <em>interprets</em> a
      schema directly rather than transpiling it. <code>cargo add schematron</code>
      is the whole install story, on every platform Rust targets.
    </p>
  </div>
</section>

<section class="section">
  <SectionHeading
    eyebrow="What Schematron is for"
    heading="Grammars describe shape. Schematron describes conditions."
  />
  <div class="prose prose-center">
    <p>
      DTD, XML Schema, and RELAX NG describe the <strong>shape</strong> a
      document may take. Schematron describes the <strong>conditions</strong> a
      document must satisfy, written as XPath expressions — so it can express
      what a grammar cannot:
    </p>
    <ul>
      <li>co-occurrence rules — "if <code>@type</code> is <code>invoice</code>, then <code>total</code> is required"</li>
      <li>value relationships — "<code>end</code> must not precede <code>start</code>"</li>
      <li>cross-references between distant parts of a document</li>
      <li>cardinality that depends on content rather than position</li>
    </ul>

    <CodeBlock label="A Schematron schema">
      <p class="code-block-caption">rules.sch</p>
      <pre><code>{`<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="invoice">
      <assert test="total">An invoice must have a total.</assert>
      <report test="count(line) > 100">This invoice has an unusual number of lines.</report>
    </rule>
  </pattern>
</schema>`}</code></pre>
    </CodeBlock>

    <p>
      Read that as: <em>for every <code>invoice</code> element, assert that it has
      a <code>total</code> child.</em> <code>assert</code> fires when its test is
      <strong>false</strong>; <code>report</code> fires when its test is
      <strong>true</strong>.
    </p>
    <p>
      Schematron is normally layered <em>on top of</em> a grammar, not used
      instead of one.
    </p>
  </div>
</section>

<Separator label="Section break" />

<section class="section">
  <SectionHeading eyebrow="Quick start" heading="Two ways in, both about a minute" />

  <div class="split">
    <div>
      <h3>As a library</h3>
      <CodeBlock label="Install the library">
        <pre><code>{`cargo add schematron`}</code></pre>
      </CodeBlock>
      <CodeBlock label="Validate a document in Rust">
        <pre><code>{`use schematron::{Document, Schema};

let schema = Schema::from_path("rules.sch")?;
let document = Document::from_path("data.xml")?;
let report = schema.validate(&document)?;

for failure in report.failures() {
    println!("{}: {}", failure.location, failure.text);
}`}</code></pre>
      </CodeBlock>
      <p><a href="/library/">The full library guide &rarr;</a></p>
    </div>

    <div>
      <h3>As a command line tool</h3>
      <CodeBlock label="Install the command line tool">
        <pre><code>{`cargo install schematron`}</code></pre>
      </CodeBlock>
      <CodeBlock label="Validate a document from the shell">
        <pre><code>{`schematron --schema rules.sch data.xml
schematron -s rules.sch -f svrl -o report.svrl data.xml
schematron -s rules.sch --flag error docs/*.xml
cat data.xml | schematron -s rules.sch -`}</code></pre>
      </CodeBlock>
      <p><a href="/cli/">Every option and exit code &rarr;</a></p>
    </div>
  </div>
</section>

<section class="section">
  <SectionHeading
    eyebrow="The one rule to internalise"
    heading="Within a pattern, the first matching rule claims the node"
  />
  <div class="prose prose-center">
    <p>
      Rules in one pattern compete like the arms of a match expression. This is
      a feature — it is how you write an "otherwise" branch:
    </p>
    <CodeBlock label="Alternative rules in one pattern">
      <pre><code>{`<pattern>
  <rule context="line[@type='discount']">
    <assert test="number(@amount) < 0">A discount must be negative.</assert>
  </rule>
  <rule context="line">
    <assert test="number(@amount) >= 0">A normal line must not be negative.</assert>
  </rule>
</pattern>`}</code></pre>
    </CodeBlock>

    <p>— and it is also the most common way to write a schema that silently does nothing:</p>

    <CodeBlock label="A shadowed rule that never runs">
      <pre><code>{`<pattern>
  <rule context="*">…</rule>
  <rule context="invoice">…</rule>  <!-- never runs: * claimed everything -->
</pattern>`}</code></pre>
    </CodeBlock>

    <Alert type="warning" role="status" heading="To apply independent checks to the same node, use separate patterns.">
      <p>
        Patterns do not compete; each gets its own pass over the document. The
        second-most common cause of a silent schema is a missing namespace
        prefix — XPath 1.0 has no default namespace, so an unprefixed name
        matches elements in <em>no</em> namespace.
        <a href="/help/">Both causes, and how to spot them &rarr;</a>
      </p>
    </Alert>
  </div>
</section>

<section class="section">
  <SectionHeading eyebrow="Beyond the standard" heading="A schema that does nothing is now a thing you can catch" />
  <div class="card-grid">
    <Card heading="Linting" headingLevel={3}>
      <p>
        <code>--lint</code> reports constructs that are legal but almost
        certainly wrong: a rule shadowed by an earlier one in the same pattern,
        an unprefixed name in a namespaced schema, a variable or key nothing
        uses, a rule that reports nothing.
      </p>
      <p class="card-meta">No document needed</p>
    </Card>
    <Card heading="Portability" headingLevel={3}>
      <p>
        <code>--portability</code> asks a different question: will this schema
        behave the same under another processor? What it reports is not wrong —
        it is correct here, and treated differently by the ISO reference
        implementation. Kept out of <code>--lint</code>, because a linter that
        reports correct code gets switched off.
      </p>
      <p class="card-meta">Seven checks, each backed by a measured divergence</p>
    </Card>
    <Card heading="Reports as data" headingLevel={3}>
      <p>
        A report is data, not formatted text, so one run renders three ways —
        SVRL, JSON, and prose — and can be queried directly instead of scraped
        back out of text.
      </p>
      <p class="card-meta"><code>to_svrl()</code>, <code>to_json()</code>, <code>to_text()</code></p>
    </Card>
    <Card heading="Explain and verbose" headingLevel={3}>
      <p>
        <code>--explain</code> prints the compiled schema — patterns, rules,
        contexts, tests — before it runs. <code>--verbose</code> shows which
        rules actually fired, which is how you learn that none did.
      </p>
      <p class="card-meta">Two flags worth knowing</p>
    </Card>
  </div>
</section>

<Separator label="Section break" />

<section class="section">
  <SectionHeading eyebrow="Guarantees" heading="What the crate promises" />

  <div class="card-grid">
    <Card heading="Security" headingLevel={3} href="/why/#security">
      <p>
        The XML parser never resolves an external entity and never processes a
        DTD's entity declarations. XXE is <strong>structurally impossible</strong>
        rather than merely switched off. The default resolver reads local files
        and refuses <code>http:</code> and <code>https:</code>.
      </p>
    </Card>
    <Card heading="Correctness" headingLevel={3} href="/conformance/">
      <p>
        Compared against the ISO reference implementation over the whole corpus.
        Twenty of twenty-three cases agree exactly; the other three are
        documented, and in each the difference is the reference's.
      </p>
    </Card>
    <Card heading="Performance" headingLevel={3} href="/why/#performance">
      <p>
        Every XPath expression is parsed once, at compile time. Rule contexts
        are evaluated once per document rather than tested node by node, so
        matching is linear rather than quadratic. Patterns can evaluate in
        parallel.
      </p>
    </Card>
    <Card heading="Documentation" headingLevel={3} href="/spec/">
      <p>
        {SPEC_DOCS.length} normative specification documents, machine-checked:
        the test suite compiles every schema shown in the docs, resolves every
        relative link, and ties duplicated facts back to their single source.
      </p>
    </Card>
  </div>

  <div class="button-row">
    <CallToAction class="button button-secondary" href="/example/">Follow a real run, end to end &rarr;</CallToAction>
    <CallToAction class="button button-secondary" href="/reports/">See SVRL, JSON, and text output &rarr;</CallToAction>
    <CallToAction class="button button-secondary" href="/roadmap/">Read the roadmap &rarr;</CallToAction>
  </div>
</section>

<section class="section prose prose-center" aria-label="Common questions">
  <SectionHeading eyebrow="Questions?" heading="Good questions, quick answers" />

  <Details summary="Do I need libxml2, Saxon, or a JVM?">
    <p>
      No. That is the point of the crate. It is pure Rust with no
      <code>unsafe</code>, so it builds wherever <code>cargo</code> does —
      including <code>musl</code> static builds, cross-compiles, and WebAssembly.
    </p>
  </Details>

  <Details summary="Is XPath 2.0 supported?">
    <p>
      A documented subset — regular expressions, conditionals, sequences, dates,
      durations, type operators, <code>for</code>, <code>some</code>,
      <code>every</code>, and ranges. Everything outside that subset is a
      <strong>hard error naming the construct</strong>, never a wrong answer.
      <a href="/conformance/">The exact boundary &rarr;</a>
    </p>
  </Details>

  <Details summary="My schema runs but finds nothing. What happened?">
    <p>
      Almost always one of two things: a missing <code>ns</code> prefix, or an
      earlier rule in the same pattern that claimed the nodes.
      <code>--lint</code> detects both automatically, and needs no document.
      <a href="/help/">Walk through the diagnosis &rarr;</a>
    </p>
  </Details>

  <Details summary="How do I check my schema will work under other processors?">
    <p>
      <code>schematron -s rules.sch --portability</code>. It reports constructs
      other Schematron processors treat differently — a <code>let</code> that
      shadows an outer one, a rule on <code>comment()</code>, a rule's
      <code>@flag</code> — each backed by a divergence established by running
      both implementations against the same schema.
    </p>
  </Details>

  <Details summary="What is the licence?">
    <p>
      Your choice of MIT, Apache-2.0, GPL-2.0-only, or GPL-3.0-only.
    </p>
  </Details>

  <p style="text-align: center; margin-top: 2rem;">
    <CallToAction class="button button-secondary" href={REPO}>Read the source on GitHub &rarr;</CallToAction>
  </p>
</section>
