<script lang="ts">
  import { Card, Details, InsetText, InformationCallout, WarningCallout, Separator, CallToAction, SectionHeading } from 'lily-design-system-svelte-headless';
  import { REPO, VERSION, specUrl } from '$lib/site';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const SHIPPED: string[] = [
    'Pure Rust XML parser and XPath data model, with no external entity resolution and therefore no XXE',
    'Complete XPath 1.0 engine: all axes, all 27 core functions, XPath 1.0 comparison and conversion semantics',
    'Schematron model, parser, include resolution, abstract pattern and abstract rule expansion',
    'Validation with first-matching-rule-wins, phases, four let scopes, diagnostics, properties, subjects, flags, roles',
    'SVRL, JSON, and human-readable text reports — and SVRL reading, making the format bidirectional',
    'CLI with phase selection, output formats, flag filtering, and exit codes',
    'Cross-document node-sets and XPath document(), costing nothing for schemas that do not use it',
    'document(uri, base), closing the last ISO gap under the XPath 1.0 binding',
    'Schema linting: the mistakes the model makes easy, caught without a document',
    '--portability: constructs that behave differently under other processors, each backed by a measured divergence',
    'Keys: <sch:key> and key(), turning a quadratic cross-reference check into a linear one',
    'Static variable checking: a misspelled $name fails when the schema loads',
    'Opt-in parallel pattern evaluation, with a report identical to the sequential one',
    'XPath 2.0 phases 1 through 3: bindings, regular expressions, sequences, dates, durations, comparisons, and the type operators',
    'XPath 2.0 kind tests as path node tests — element(), attribute(id), document-node()',
    'XPath 2.0 phase 4: the numeric type hierarchy — instance of recognizes xs:integer, xs:decimal and xs:float as well as xs:double, tracked for literals and explicit casts',
    'Differential and generated testing against the ISO reference implementation',
    'Fuzz targets, criterion benchmarks, clippy pedantic, corpus test suite, runnable examples, and the specification'
  ];
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta
    name="description"
    content="What the schematron crate has shipped, what is next and why it is ordered that way, what was examined and abandoned, and what is not planned."
  />
</svelte:head>

<div class="page-header">
  <h1>Roadmap</h1>
  <p>
    What is shipped, what is next, and — just as usefully — what was considered
    and rejected. <a href={specUrl('roadmap/index.md')}>spec/roadmap/</a> is the
    source.
  </p>
</div>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow={'Shipped through ' + VERSION} heading="Done" level={2} />

  <InformationCallout label="The headline">
    <p>
      With <code>document(uri, base)</code> in 0.5.0, <strong>every element of
      ISO/IEC 19757-3 is implemented</strong> under the XPath 1.0 binding.
    </p>
  </InformationCallout>

  <ul>
    {#each SHIPPED as item}
      <li>{item}</li>
    {/each}
  </ul>

  <p>
    The <a href={REPO + '/blob/main/CHANGELOG.md'}>changelog</a> has the
    release-by-release detail.
  </p>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading
    class="section-heading-start"
    eyebrow="Next"
    heading="Ordered by value, not by phase number"
    level={2}
  />

  <p>
    Two items remain, and both are arguments for <em>not</em> doing the
    work yet. That is deliberate: a roadmap that only lists ambitions is a wish
    list.
  </p>

  <Details summary="1. Streaming validation — narrower than it looks">
    <p>
      For patterns whose rules only need the subtree rooted at the context node,
      validate without materialising the whole document.
    </p>
    <p>
      It pays off only when <em>every</em> active pattern is subtree-local: one
      <code>//</code>, one <code>key()</code>, one <code>ancestor::</code>
      forces the whole tree to be materialised anyway. Cross-node constraints
      are precisely what Schematron exists for, so most real schemas would fall
      back. Weighed against reworking the arena and <code>NodeId</code> model
      the entire engine rests on, that is a poor trade until someone has a
      document it actually blocks.
    </p>
  </Details>

  <Details summary="2. no_std core — blocked, and the earlier reasoning was wrong">
    <p>
      The claim was that only I/O and the resolver need <code>std</code>. They
      are not the obstacle: <code>quick-xml</code>, which this crate's XML
      parser is built on, declares no <code>no_std</code> support and reaches
      for <code>std::io</code> and <code>std::error</code> throughout. Nothing
      in the tree can be parsed without it, so a <code>no_std</code> build would
      validate nothing.
    </p>
    <p>
      <code>regex</code>, the dependency that looked likelier to block it, turns
      out to be <code>#![no_std]</code> already and would only need its
      <code>std</code> feature dropped.
    </p>
    <p>
      So the real price is a hand-written XML tokenizer replacing a
      heavily-fuzzed one — a correctness risk taken <em>in a validator</em>,
      whose whole value is being right — bought for a target that WebAssembly,
      the plausible use case, already reaches with <code>std</code>. Not worth
      it on today's evidence.
    </p>
  </Details>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading
    class="section-heading-start"
    eyebrow="Examined and abandoned"
    heading="Recorded so they are not proposed again"
    level={2}
  />

  <div class="card-grid">
    <Card heading="A lint for a context that can never match" headingLevel={3}>
      <p>
        There is no vocabulary to check against. Schematron declares no element
        names of its own; it is layered on a grammar — a DTD, XML Schema, RELAX
        NG — that it cannot see. Inferring one from the names the schema happens
        to mention is circular: a context naming an element no test mentions is
        entirely normal. This would need the grammar as a second input, which is
        a different feature.
      </p>
    </Card>
    <Card heading="A lint for a report that reads like a requirement" headingLevel={3}>
      <p>
        Confusing <code>assert</code> with <code>report</code> is the classic
        Schematron mistake, and catching it would be valuable. But the only
        available signal is the wording of English prose, and a lint that
        misfires on "this invoice must be reviewed manually" teaches its reader
        to ignore the linter — the one outcome worth avoiding above all.
      </p>
    </Card>
  </div>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Not planned" heading="Out of scope, on purpose" level={2} />

  <WarningCallout label="Not planned">
    <ul style="margin: 0;">
      <li>
        <strong>Compiling to XSLT.</strong> That is the reference
        implementation's approach, and the thing this crate exists to avoid.
      </li>
      <li><strong>FFI bindings to <code>libxml2</code>.</strong> Same reason.</li>
      <li>
        <strong>A general-purpose XSLT processor.</strong> Out of scope; use the
        XPath engine.
      </li>
    </ul>
  </WarningCallout>

  <InsetText>
    <p>
      A roadmap item here is not a refusal to discuss it — it is a record of the
      reasoning, so a new argument can be weighed against the old one instead of
      restarting it.
    </p>
  </InsetText>

  <p style="margin-top: 2rem;">
    <CallToAction class="button button-primary" href={REPO + '/issues'}>Make the case for something</CallToAction>
    <CallToAction class="button button-secondary" href={specUrl('roadmap/index.md')}>spec/roadmap/</CallToAction>
  </p>
</section>
