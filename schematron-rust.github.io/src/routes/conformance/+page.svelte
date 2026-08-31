<script lang="ts">
  import { Table, Tag, Alert, InformationCallout, Separator, CallToAction, SectionHeading, CodeBlock, TableHead, TableBody, TableRow, TableTH, TableTD } from 'lily-design-system-svelte-headless';
  import { specUrl } from '$lib/site';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  type Row = { area: string; status: string; note?: string };

  const AREAS: Row[] = [
    { area: 'schema, pattern, rule, assert, report', status: 'Full' },
    { area: 'ns, let, phase, active, include, extends', status: 'Full' },
    { area: 'Abstract patterns (abstract, is-a, param)', status: 'Full' },
    { area: 'Abstract rules (rule/@abstract + extends)', status: 'Full' },
    { area: 'diagnostics, properties, value-of, name, emph, span, dir', status: 'Full' },
    { area: '@flag, @role, @subject, @see, @icon, @fpi', status: 'Full' },
    { area: 'pattern/@documents', status: 'Full' },
    { area: 'key and key()', status: 'Full', note: 'A non-ISO extension — see spec/keys/.' },
    { area: 'Phases, #ALL, #DEFAULT, @defaultPhase', status: 'Full' },
    { area: 'SVRL output', status: 'Full' },
    { area: 'XPath 1.0 — 13 axes, 27 core functions, exact conversion semantics', status: 'Full' },
    { area: 'XPath document(), with cross-document node-sets', status: 'Full' },
    { area: 'document(uri, base)', status: 'Full', note: "Resolves against the second argument's first node." },
    { area: 'XPath 2.0 kind tests as node tests — element(), attribute(id), document-node()', status: 'Full', note: 'Under an xslt2 binding.' },
    { area: 'XPath 2.0 sequences, dates, durations, type operators, value and node comparisons, for, some, every, ranges, regular expressions', status: 'Subset', note: 'See spec/xpath2/.' },
    { area: 'queryBinding="xslt", "xpath", or absent', status: 'Full' },
    { area: 'queryBinding="xslt2", "xpath2"', status: 'Subset', note: 'See spec/xpath2/.' },
    { area: 'queryBinding="xslt3" and later', status: 'Refused', note: 'By default.' },
    { area: 'extends rule and extends href, with #fragment identifiers', status: 'Full' }
  ];
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta
    name="description"
    content="What the schematron crate implements, what it does not, and every measured divergence from the ISO Schematron reference implementation."
  />
</svelte:head>

<div class="page-header">
  <h1>Conformance</h1>
  <p>
    A summary. <a href={specUrl('conformance/index.md')}>spec/conformance/</a> is
    authoritative, and states the limits and deliberate divergences in full.
  </p>
</div>

<section class="section">
  <SectionHeading class="section-heading-start" eyebrow="ISO/IEC 19757-3" heading="What is implemented" level={2} />

  <div class="table-scroll">
    <Table label="Implementation status by area">
      <TableHead>
        <TableRow>
          <TableTH>Area</TableTH>
          <TableTH>Status</TableTH>
          <TableTH>Note</TableTH>
        </TableRow>
      </TableHead>
      <TableBody>
        {#each AREAS as row (row.area)}
          <TableRow>
            <TableTH scope="row">{row.area}</TableTH>
            <TableTD><Tag class={'tag-' + row.status.toLowerCase()} label={'Status: ' + row.status}>{row.status}</Tag></TableTD>
            <TableTD>{row.note ?? ''}</TableTD>
          </TableRow>
        {/each}
      </TableBody>
    </Table>
  </div>

  <div class="prose">
    <InformationCallout label="Completeness under the XPath 1.0 binding">
      <p>
        With the two-argument <code>document(uri, base)</code> shipped in 0.5.0,
        every element of ISO/IEC 19757-3 is implemented under the XPath 1.0
        binding.
      </p>
    </InformationCallout>
  </div>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="XPath 2.0" heading="A subset, with a hard edge" level={2} />

  <p>
    XPath 2.0 is a different language, not XPath 1.0 with extra functions. The
    crate implements a documented subset of it — regular expressions,
    conditionals, and the string and numeric functions schemas actually use —
    and makes everything outside that subset a <strong>hard error naming the
    construct</strong>, never a wrong answer.
  </p>

  <Alert type="warning" role="status" heading="An xslt2 schema may still evaluate some things with XPath 1.0 semantics.">
    <p>
      <a href={specUrl('xpath2/index.md')}>spec/xpath2/</a> is explicit about what is
      in, what is out, and the handful of places where that happens. Read it
      before depending on an <code>xslt2</code> binding.
    </p>
  </Alert>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Measured" heading="Against the reference implementation" level={2} />

  <p>
    Beyond the test suite, the crate is compared against the ISO Schematron
    reference implementation — the XSLT stylesheets that compile a schema into a
    validator — over the whole corpus. Twenty of twenty-three cases agree
    exactly; the other three are documented, and in each the difference is the
    reference's rather than this crate's.
  </p>

  <CodeBlock label="Run the differential suite yourself">
    <pre><code>{`sh tests/differential/fetch-skeleton.sh /tmp/skeleton
SCHEMATRON_SKELETON=/tmp/skeleton cargo test --test differential -- --ignored`}</code></pre>
  </CodeBlock>

  <p>The documented divergences include:</p>
  <ul>
    <li>rules on <code>text()</code>, <code>comment()</code> and processing instructions, which the reference never fires;</li>
    <li>a <code>let</code> that shadows an outer one, which the reference refuses outright;</li>
    <li>a rule's <code>@flag</code>, which the reference drops.</li>
  </ul>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Two checks" heading="Lint asks one question; portability asks another" level={2} />

  <p>
    <code>--lint</code> reports constructs that are legal but almost certainly
    wrong — a shadowed rule, an unprefixed name in a namespaced schema, a
    variable or key nothing uses, a rule that reports nothing.
  </p>

  <CodeBlock label="Lint a schema">
    <pre><code>{`schematron --schema rules.sch --lint`}</code></pre>
  </CodeBlock>

  <p>
    <code>--portability</code> asks whether the schema will behave the same
    under another Schematron processor. What it reports is <strong>not
    wrong</strong> — it is correct, and works here, and the reference
    implementation treats it differently. Each of the seven checks is backed by
    a divergence in <code>spec/conformance/</code>, established by running
    both implementations against the same schema.
  </p>

  <CodeBlock label="Check portability">
    <pre><code>{`schematron --schema rules.sch --portability`}</code></pre>
  </CodeBlock>

  <p>
    They are separate on purpose, because a linter that reports correct code
    gets switched off.
  </p>

  <p style="margin-top: 2rem;">
    <CallToAction class="button button-primary" href={specUrl('conformance/index.md')}>The authoritative conformance document</CallToAction>
    <CallToAction class="button button-secondary" href="/roadmap/">What is next, and what is not planned</CallToAction>
    <CallToAction class="button button-secondary" href="/spec/">The whole specification</CallToAction>
  </p>
</section>
