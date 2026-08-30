<script lang="ts">
  import SectionHeading from '$lib/lily/SectionHeading.svelte';
  import Card from 'lily-design-system-svelte-headless/components/Card/Card.svelte';
  import InformationCallout from 'lily-design-system-svelte-headless/components/InformationCallout/InformationCallout.svelte';
  import CodeBlock from '$lib/lily/CodeBlock.svelte';
  import Separator from 'lily-design-system-svelte-headless/components/Separator/Separator.svelte';
  import { SPEC_DOCS, specUrl, REPO, specLabel, MSRV } from '$lib/site';
</script>

<svelte:head>
  <title>Specification — schematron</title>
  <meta
    name="description"
    content={`The normative specification for the schematron crate: ${SPEC_DOCS.length} documents covering the data model, the validation algorithm, both XPath engines, SVRL, linting, errors, and conformance.`}
  />
</svelte:head>

<div class="page-header">
  <h1>Specification</h1>
  <p>
    The <code>spec/</code> directory is <strong>normative</strong>, and written
    to be read. If the code and the specification disagree, that is a defect in
    one of them.
  </p>
</div>

<section class="section">
  <div class="prose">
    <InformationCallout label="Start here">
      <p>
        New to Schematron? Read <a href={specUrl('tutorial/index.md')}>spec/tutorial/</a>
        — eighteen steps from one rule to a real schema — or the
        <a href="/tutorial/">condensed tutorial on this site</a>.
      </p>
    </InformationCallout>
  </div>

  <SectionHeading class="section-heading-start" eyebrow={SPEC_DOCS.length + ' documents'} heading="The specification" level={2} />

  <div class="card-grid">
    {#each SPEC_DOCS as doc (doc.file)}
      <Card heading={doc.title} headingLevel={3} href={specUrl(doc.file)}>
        <p>{doc.covers}</p>
        <p class="card-meta"><code>{specLabel(doc.file)}</code></p>
      </Card>
    {/each}
  </div>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="How it stays true" heading="The documentation is machine-checked" level={2} />

  <p>
    <code>tests/docs.rs</code> compiles every schema shown in the docs, resolves
    every relative link, and ties duplicated facts — the MSRV, the CLI flags,
    the XPath function list — back to their single source. A document that
    drifts from the code fails the test suite.
  </p>

  <CodeBlock label="Verify the build">
    <pre><code>{`cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
cargo +${MSRV} test --all-features`}</code></pre>
  </CodeBlock>

  <p>
    The conformance suite lives in <code>tests/corpus/</code>. Each case is a
    directory holding <code>schema.sch</code>, <code>input.xml</code>, and
    <code>expected.txt</code>; adding a case means adding a directory, with no
    Rust to change.
  </p>

  <p style="margin-top: 2rem;">
    <a class="button button-primary" href={REPO}>Browse the repository</a>
    <a class="button button-secondary" href="/conformance/">Conformance summary</a>
    <a class="button button-secondary" href="/roadmap/">Roadmap</a>
  </p>
</section>
