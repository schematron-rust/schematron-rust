<script lang="ts">
  import SectionHeading from '$lib/lily/SectionHeading.svelte';
  import CodeBlock from '$lib/lily/CodeBlock.svelte';
  import Table from 'lily-design-system-svelte-headless/components/Table/Table.svelte';
  import TableHead from '$lib/lily/TableHead.svelte';
  import TableBody from '$lib/lily/TableBody.svelte';
  import TableRow from '$lib/lily/TableRow.svelte';
  import TableTH from '$lib/lily/TableTH.svelte';
  import TableTD from '$lib/lily/TableTD.svelte';
  import Alert from 'lily-design-system-svelte-headless/components/Alert/Alert.svelte';
  import InsetText from 'lily-design-system-svelte-headless/components/InsetText/InsetText.svelte';
  import InformationCallout from 'lily-design-system-svelte-headless/components/InformationCallout/InformationCallout.svelte';
  import WarningCallout from 'lily-design-system-svelte-headless/components/WarningCallout/WarningCallout.svelte';
  import Separator from 'lily-design-system-svelte-headless/components/Separator/Separator.svelte';
  import CallToAction from 'lily-design-system-svelte-headless/components/CallToAction/CallToAction.svelte';
  import { specUrl } from '$lib/site';

  type ElementRow = { element: string; emitted: string };

  const SVRL_ELEMENTS: ElementRow[] = [
    { element: 'svrl:active-pattern', emitted: 'A pattern begins running against a document. @documents is present only for @documents patterns.' },
    { element: 'svrl:fired-rule', emitted: 'A rule matches a node. Emitted once per matching node.' },
    { element: 'svrl:failed-assert', emitted: 'An assert whose test evaluated false.' },
    { element: 'svrl:successful-report', emitted: 'A report whose test evaluated true.' },
    { element: 'svrl:text', emitted: 'The instantiated human-readable message.' },
    { element: 'svrl:diagnostic-reference', emitted: 'Per @diagnostics reference on the assertion.' },
    { element: 'svrl:property-reference', emitted: 'Per @properties reference on the assertion.' },
    { element: 'svrl:ns-prefix-in-attribute-values', emitted: 'Once per schema ns, so a consumer can interpret @location and @test.' }
  ];
</script>

<svelte:head>
  <title>Reports — schematron</title>
  <meta
    name="description"
    content="One validation run, three renderings: SVRL for other Schematron tooling, JSON that keeps the tree structure, and text for a person. Plus reading SVRL back."
  />
</svelte:head>

<div class="page-header">
  <h1>Reports</h1>
  <p>
    A report is <strong>data</strong>, not formatted text. One run renders three
    ways, and can be queried directly instead of scraped back out of prose.
  </p>
</div>

<section class="section prose prose-wide">
  <CodeBlock label="Three renderings of one report">
    <pre><code>{`let svrl = report.to_svrl();   // SVRL, for other Schematron tooling
let json = report.to_json()?;  // JSON, keeping the tree structure
let text = report.to_text();   // for a person`}</code></pre>
  </CodeBlock>

  <CodeBlock label="The same, from the command line">
    <pre><code>{`schematron -s rules.sch -f text data.xml   # the default
schematron -s rules.sch -f svrl data.xml
schematron -s rules.sch -f json data.xml`}</code></pre>
  </CodeBlock>

  <InsetText>
    <p>
      Every output on this page is the real result of running
      <code>examples/invoice.sch</code> against
      <code>examples/invoice-bad.xml</code> under the <code>strict</code> phase.
      <a href="/example/">The worked example walks through that run &rarr;</a>
    </p>
  </InsetText>
</section>

<Separator label="Section break" />

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="For people" heading="Text" level={2} />

  <p>
    The first column is the finding's <code>@flag</code> when the schema sets
    one, and otherwise <code>error</code> for a failed assertion or
    <code>report</code> for a successful report. Lines beginning <code>-</code>
    are diagnostics.
  </p>

  <CodeBlock label="Text output">
    <pre><code>{`examples/invoice-bad.xml:
  error    /invoice[1]
           An invoice must have an id.
  error    /invoice[1]/line[1]
           Quantity must be positive, but is -2.
           - Quantity is the number of units ordered. It must be a positive number.
  error    /invoice[1]/line[2]
           A discount line must have a negative amount, but line has 5.00.
           - Amount is the line total in the invoice currency. Discounts are negative.
  warning  /invoice[1]
           Total is 99.00 but the lines plus tax come to 18.
  4 findings: 4 failed asserts, 0 reports`}</code></pre>
  </CodeBlock>
</section>

<Separator label="Section break" />

<section class="section">
  <div class="prose prose-wide">
    <SectionHeading class="section-heading-start" eyebrow="For other tooling" heading="SVRL" level={2} />

    <p>
      SVRL — Schematron Validation Report Language — is the standard XML
      vocabulary for a Schematron report, in namespace
      <code>http://purl.oclc.org/dsdl/svrl</code>, conventionally bound to
      <code>svrl</code>. Emitting it is what makes this crate's output
      consumable by existing Schematron tooling.
    </p>

    <CodeBlock label="SVRL output">
      <pre><code>{`<?xml version="1.0" encoding="UTF-8"?>
<svrl:schematron-output xmlns:svrl="http://purl.oclc.org/dsdl/svrl" title="Invoice rules" phase="strict">
  <svrl:active-pattern id="structure" name="Structure"/>
  <svrl:fired-rule context="invoice"/>
  <svrl:failed-assert location="/invoice[1]" test="@id" flag="error">
    <svrl:text>An invoice must have an id.</svrl:text>
  </svrl:failed-assert>
  <svrl:active-pattern id="lines" name="Line rules"/>
  <svrl:fired-rule context="line"/>
  <svrl:failed-assert location="/invoice[1]/line[1]" test="number(@qty) &gt; 0" flag="error">
    <svrl:text>Quantity must be positive, but is -2.</svrl:text>
    <svrl:diagnostic-reference diagnostic="qty-help">
      <svrl:text>Quantity is the number of units ordered. It must be a positive number.</svrl:text>
    </svrl:diagnostic-reference>
  </svrl:failed-assert>
  <svrl:fired-rule context="line[@type='discount']"/>
  <svrl:failed-assert location="/invoice[1]/line[2]" test="number(@amount) &lt; 0" flag="error">
    <svrl:text>A discount line must have a negative amount, but line has 5.00.</svrl:text>
    <svrl:diagnostic-reference diagnostic="amount-help">
      <svrl:text>Amount is the line total in the invoice currency. Discounts are negative.</svrl:text>
    </svrl:diagnostic-reference>
  </svrl:failed-assert>
  <svrl:active-pattern id="totals" name="Totals"/>
  <svrl:fired-rule context="invoice"/>
  <svrl:failed-assert location="/invoice[1]" test="number(total) &gt;= $expected - 0.01 and number(total) &lt;= $expected + 0.01" flag="warning">
    <svrl:text>Total is 99.00 but the lines plus tax come to 18.</svrl:text>
  </svrl:failed-assert>
</svrl:schematron-output>`}</code></pre>
    </CodeBlock>

    <WarningCallout label="SVRL is flat, not nested">
      <p>
        <code>active-pattern</code>, <code>fired-rule</code>,
        <code>failed-assert</code> and <code>successful-report</code> are all
        <strong>siblings</strong>. The structure is implied by order, not by
        nesting: every <code>fired-rule</code> belongs to the most recent
        <code>active-pattern</code>, and every finding belongs to the most
        recent <code>fired-rule</code>.
      </p>
      <p>
        That is what the reference implementation emits, because it is what
        falls out of an XSLT streaming transform — and consumers depend on it.
        This crate's internal report is a tree, and is flattened on the way out,
        which is how the JSON output keeps the structure while the SVRL output
        stays wire-compatible.
      </p>
    </WarningCallout>
  </div>

  <div class="table-scroll">
    <Table label="SVRL elements and when they are emitted">
      <TableHead>
        <TableRow>
          <TableTH>Element</TableTH>
          <TableTH>Emitted when</TableTH>
        </TableRow>
      </TableHead>
      <TableBody>
        {#each SVRL_ELEMENTS as row (row.element)}
          <TableRow>
            <TableTH scope="row"><code>{row.element}</code></TableTH>
            <TableTD>{row.emitted}</TableTD>
          </TableRow>
        {/each}
      </TableBody>
    </Table>
  </div>

  <div class="prose prose-wide">
    <p>
      On <code>failed-assert</code> and <code>successful-report</code>:
      <code>test</code> is the assertion's XPath source text verbatim,
      <code>location</code> is an absolute XPath to the subject node, and
      <code>role</code> and <code>flag</code> are resolved — the assertion's if
      set, otherwise the rule's. <code>see</code>, <code>icon</code> and
      <code>fpi</code> are passed through when present.
    </p>
    <p>
      <code>--svrl-findings-only</code> omits the <code>fired-rule</code> events
      when you want just the findings.
    </p>
  </div>
</section>

<Separator label="Section break" />

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="For programs" heading="JSON" level={2} />

  <p>
    The JSON rendering keeps the tree the SVRL rendering flattens: patterns
    contain rules, and rules contain assertions.
  </p>

  <CodeBlock label="JSON output, abridged">
    <pre><code>{`{
  "title": "Invoice rules",
  "phase": "strict",
  "schema_version": null,
  "namespaces": [],
  "patterns": [
    {
      "id": "structure",
      "name": "Structure",
      "documents": null,
      "rules": [
        {
          "id": null,
          "context": "invoice",
          "role": null,
          "flag": null,
          "location": "/invoice[1]",
          "assertions": [
            {
              "kind": "FailedAssert",
              "test": "@id",
              "location": "/invoice[1]",
              "text": "An invoice must have an id.",
              "id": null,
              "role": null,
              "flag": "error",
              "see": null,
              "icon": null,
              "fpi": null,
              "diagnostics": [],
              "properties": []
            }
          ]
        }
      ]
    }
  ]
}`}</code></pre>
  </CodeBlock>

  <p>
    JSON output needs the <code>serde</code> feature, which is on by default.
  </p>
</section>

<Separator label="Section break" />

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="Instead of parsing text" heading="Query the report directly" level={2} />

  <CodeBlock label="Querying a report">
    <pre><code>{`report.is_valid();                  // no assert failed
report.count_failures();            // how many did
report.with_flag("error").count();  // findings the schema flagged as errors
report.count_fired_rules();         // zero here means NO context matched`}</code></pre>
  </CodeBlock>

  <Alert type="info" role="status" heading="count_fired_rules() is the one to assert on in your own tests.">
    <p>
      Zero fired rules is the programmatic form of "my schema does nothing": no
      rule context matched any node. Check it and a broken schema fails loudly
      instead of passing silently.
    </p>
  </Alert>
</section>

<Separator label="Section break" />

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="Bidirectional" heading="Reading SVRL back" level={2} />

  <p>
    <code>Report::from_svrl</code> parses an SVRL document into a
    <code>Report</code>, which makes SVRL support bidirectional rather than
    write-only.
  </p>

  <CodeBlock label="Parse SVRL into a report">
    <pre><code>{`let report = Report::from_svrl(&svrl)?;
assert_eq!(report.count_failures(), 2);`}</code></pre>
  </CodeBlock>

  <p>Two things that buys:</p>
  <ul>
    <li>
      <strong>Round-trip testing.</strong> Every report the crate produces is
      parsed back and compared against the original, which checks the writer far
      more thoroughly than asserting on substrings of its output.
    </li>
    <li>
      <strong>Comparing processors.</strong> A report from another Schematron
      implementation can be read in and diffed against this one's — the
      strongest evidence available that the two agree.
    </li>
  </ul>

  <p>
    The reader rebuilds the tree the writer flattened. A finding appearing
    before any <code>fired-rule</code> — which is what
    <code>--svrl-findings-only</code> output looks like — is attached to a
    synthetic rule, so nothing is lost.
  </p>

  <InformationCallout label="What a round trip does not preserve">
    <p>
      <strong><code>FiredRule::location</code>.</strong> SVRL's
      <code>fired-rule</code> element carries <code>id</code>,
      <code>context</code>, <code>role</code> and <code>flag</code>, and has
      nowhere to put the node the rule fired on. That field therefore comes back
      empty. It is a real field — the text report uses it — so the loss is
      stated rather than papered over, and the round-trip test compares reports
      with those locations cleared.
    </p>
    <p>
      Everything else survives exactly: the schema title, phase and version, the
      namespace bindings, every pattern and rule, and every finding with its
      test, location, message, flags, diagnostics and properties.
    </p>
  </InformationCallout>

  <p style="margin-top: 2rem;">
    <CallToAction class="button button-primary" href={specUrl('svrl/index.md')}>spec/svrl/</CallToAction>
    <CallToAction class="button button-secondary" href="/example/">The run behind these outputs</CallToAction>
  </p>
</section>
