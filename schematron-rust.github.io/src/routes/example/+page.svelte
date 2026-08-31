<script lang="ts">
  import { Alert, InsetText, WarningCallout, Separator, CallToAction, SectionHeading, CodeBlock } from 'lily-design-system-svelte-headless';
  import { REPO, specUrl } from '$lib/site';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta
    name="description"
    content="One schema, two documents, and the real output: phases, first-matching-rule, diagnostics, flags, and what --explain and --verbose actually print."
  />
</svelte:head>

<div class="page-header">
  <h1>Worked example</h1>
  <p>
    One schema, two documents, and the real output of every command on this
    page. The files are <code>examples/invoice.sch</code>,
    <code>examples/invoice-good.xml</code> and
    <code>examples/invoice-bad.xml</code> in
    <a href={REPO}>the repository</a>.
  </p>
</div>

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="The schema" heading="Three patterns, two phases" level={2} />

  <p>
    The schema checks an invoice three ways, and each way is its own pattern —
    because patterns do not compete, while rules inside one pattern do.
  </p>

  <CodeBlock label="The invoice schema">
    <p class="code-block-caption">examples/invoice.sch (abridged)</p>
    <pre><code>{`<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="basic">

  <title>Invoice rules</title>

  <phase id="basic">
    <active pattern="structure"/>
  </phase>

  <phase id="strict">
    <active pattern="structure"/>
    <active pattern="lines"/>
    <active pattern="totals"/>
  </phase>

  <let name="tax-rate" value="0.2"/>

  <pattern id="structure">
    <title>Structure</title>
    <rule context="invoice">
      <assert test="@id" flag="error">An invoice must have an id.</assert>
      <assert test="total" flag="error">An invoice must have a total.</assert>
      <assert test="count(line) > 0" flag="error">An invoice must have at least one line.</assert>
      <report test="count(line) > 100" flag="info">
        This invoice has <value-of select="count(line)"/> lines, which is unusual.
      </report>
    </rule>
  </pattern>

  <pattern id="lines">
    <title>Line rules</title>
    <!-- These two rules are alternatives, on purpose: a discount line is
         checked by the first rule and never reaches the second. -->
    <rule context="line[@type='discount']">
      <assert test="number(@amount) < 0" flag="error" diagnostics="amount-help">
        A discount line must have a negative amount, but <name/> has <value-of select="@amount"/>.
      </assert>
    </rule>
    <rule context="line">
      <assert test="@qty" flag="error">Every line needs a qty.</assert>
      <assert test="number(@qty) > 0" flag="error" diagnostics="qty-help">
        Quantity must be positive, but is <value-of select="@qty"/>.
      </assert>
      <assert test="number(@amount) >= 0" flag="error">
        A normal line must not have a negative amount; use type="discount" for that.
      </assert>
    </rule>
  </pattern>

  <pattern id="totals">
    <title>Totals</title>
    <rule context="invoice">
      <let name="expected" value="sum(line/@amount) * (1 + $tax-rate)"/>
      <assert test="number(total) >= $expected - 0.01 and number(total) <= $expected + 0.01"
              flag="warning">
        Total is <value-of select="total"/> but the lines plus tax come to <value-of select="$expected"/>.
      </assert>
    </rule>
  </pattern>

  <diagnostics>
    <diagnostic id="qty-help">
      Quantity is the number of units ordered. It must be a positive number.
    </diagnostic>
    <diagnostic id="amount-help">
      Amount is the line total in the invoice currency. Discounts are negative.
    </diagnostic>
  </diagnostics>

</schema>`}</code></pre>
  </CodeBlock>

  <InsetText>
    <p>
      Note where <code>$tax-rate</code> and <code>$expected</code> live. A
      schema-level <code>let</code> is a constant; a rule-level <code>let</code>
      is evaluated with the rule's context node, so <code>$expected</code>
      means something different for each invoice.
    </p>
  </InsetText>
</section>

<Separator label="Section break" />

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="The documents" heading="One that passes, one that does not" level={2} />

  <CodeBlock label="A valid invoice">
    <p class="code-block-caption">examples/invoice-good.xml</p>
    <pre><code>{`<?xml version="1.0" encoding="UTF-8"?>
<invoice id="INV-1001">
  <line qty="2" amount="10.00"/>
  <line qty="1" amount="5.00"/>
  <line type="discount" qty="1" amount="-5.00"/>
  <total>12.00</total>
</invoice>`}</code></pre>
  </CodeBlock>

  <CodeBlock label="An invalid invoice">
    <p class="code-block-caption">examples/invoice-bad.xml</p>
    <pre><code>{`<?xml version="1.0" encoding="UTF-8"?>
<invoice>
  <line qty="-2" amount="10.00"/>
  <line type="discount" qty="1" amount="5.00"/>
  <total>99.00</total>
</invoice>`}</code></pre>
  </CodeBlock>

  <p>Four things are wrong with the second one. Count them before reading on.</p>
</section>

<Separator label="Section break" />

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="Run it" heading="The default phase checks structure only" level={2} />

  <CodeBlock label="Validate under the default phase">
    <pre><code>{`$ schematron -s examples/invoice.sch examples/invoice-bad.xml

examples/invoice-bad.xml:
  error    /invoice[1]
           An invoice must have an id.
  1 finding: 1 failed assert, 0 reports

$ echo $?
1`}</code></pre>
  </CodeBlock>

  <p>
    One finding — because <code>defaultPhase="basic"</code> activates only the
    <code>structure</code> pattern. The line and total problems are still
    there; nothing was asked to look for them.
  </p>

  <CodeBlock label="List the phases">
    <pre><code>{`$ schematron -s examples/invoice.sch --list-phases

Phases:
  basic  (default)
  strict

Also accepted: #ALL, #DEFAULT`}</code></pre>
  </CodeBlock>
</section>

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="Run it" heading="The strict phase finds all four" level={2} />

  <CodeBlock label="Validate under the strict phase">
    <pre><code>{`$ schematron -s examples/invoice.sch -p strict examples/invoice-bad.xml

examples/invoice-bad.xml:
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

  <p>
    The first column is each finding's <code>@flag</code>. Lines beginning
    <code>-</code> are the diagnostics the assertions referenced. And the good
    document, for contrast:
  </p>

  <CodeBlock label="The valid invoice under the same phase">
    <pre><code>{`$ schematron -s examples/invoice.sch -p strict examples/invoice-good.xml

examples/invoice-good.xml:
  no findings

$ echo $?
0`}</code></pre>
  </CodeBlock>

  <Alert type="info" role="status" heading="Look closely at line[2].">
    <p>
      The discount line has <code>qty="1"</code> and a positive amount. It was
      caught by the <em>discount</em> rule, not the general <code>line</code>
      rule — because <code>line[@type='discount']</code> comes first in the
      pattern and claimed the node. That is first-matching-rule-wins doing
      exactly what it was written to do.
    </p>
  </Alert>
</section>

<Separator label="Section break" />

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="Diagnose" heading="What the schema will do, before it runs" level={2} />

  <p>
    <code>--explain</code> prints the compiled schema. It needs no document,
    and it is the fastest way to see a rule that can never fire.
  </p>

  <CodeBlock label="Explain the compiled schema">
    <pre><code>{`$ schematron -s examples/invoice.sch --explain

Invoice rules

query binding: (default, XPath 1.0)
namespaces: none declared

pattern structure
  Structure
  rule 1 context invoice
    assert  @id
    assert  total
    assert  count(line) > 0
    report  count(line) > 100

pattern lines
  Line rules
  rule 1 context line[@type='discount']
    assert  number(@amount) < 0
  rule 2 context line
       (only fires on nodes no earlier rule claimed)
    assert  @qty
    assert  number(@qty) > 0
    assert  number(@amount) >= 0

pattern totals
  Totals
  rule 1 context invoice
    assert  number(total) >= $expected - 0.01 and number(total) <= $expected + 0.01`}</code></pre>
  </CodeBlock>

  <WarningCallout label="The line that matters">
    <p>
      <code>(only fires on nodes no earlier rule claimed)</code> under
      <code>rule 2</code>. Here that is deliberate. When it appears under a rule
      you expected to run on everything, you have found your bug.
    </p>
  </WarningCallout>

  <p>
    <code>--verbose</code> answers the other half — <em>why</em> did this fire?
    — by printing the test and the rule behind each finding:
  </p>

  <CodeBlock label="Verbose findings">
    <pre><code>{`$ schematron -s examples/invoice.sch -p strict -v examples/invoice-bad.xml

examples/invoice-bad.xml:
  error    /invoice[1]
           An invoice must have an id.
           test: @id
           rule: invoice
  error    /invoice[1]/line[1]
           Quantity must be positive, but is -2.
           test: number(@qty) > 0
           rule: line
           - Quantity is the number of units ordered. It must be a positive number.
  error    /invoice[1]/line[2]
           A discount line must have a negative amount, but line has 5.00.
           test: number(@amount) < 0
           rule: line[@type='discount']
           - Amount is the line total in the invoice currency. Discounts are negative.
  warning  /invoice[1]
           Total is 99.00 but the lines plus tax come to 18.
           test: number(total) >= $expected - 0.01 and number(total) <= $expected + 0.01
           rule: invoice
  4 findings: 4 failed asserts, 0 reports`}</code></pre>
  </CodeBlock>

  <p>
    An <em>empty</em> list of fired rules here is the signature of the other
    common failure: a missing namespace prefix.
    <a href="/help/">Diagnosing that &rarr;</a>
  </p>
</section>

<Separator label="Section break" />

<section class="section prose prose-wide">
  <SectionHeading class="section-heading-start" eyebrow="Filter" heading="Fail on errors, still show warnings" level={2} />

  <p>
    <code>--flag</code> filters what is reported, and therefore what counts
    toward the exit code. The total mismatch above is flagged
    <code>warning</code>, so it disappears here:
  </p>

  <CodeBlock label="Errors only">
    <pre><code>{`$ schematron -s examples/invoice.sch -p strict --flag error examples/invoice-bad.xml

examples/invoice-bad.xml:
  error    /invoice[1]
           An invoice must have an id.
  error    /invoice[1]/line[1]
           Quantity must be positive, but is -2.
           - Quantity is the number of units ordered. It must be a positive number.
  error    /invoice[1]/line[2]
           A discount line must have a negative amount, but line has 5.00.
           - Amount is the line total in the invoice currency. Discounts are negative.
  3 findings: 3 failed asserts, 0 reports

$ echo $?
1`}</code></pre>
  </CodeBlock>

  <p>
    That is the shape of a build gate: fail on <code>error</code>, and run a
    second unfiltered pass to surface warnings without breaking the build.
  </p>

  <p style="margin-top: 2rem;">
    <CallToAction class="button button-primary" href="/reports/">The same run as SVRL and JSON</CallToAction>
    <CallToAction class="button button-secondary" href={specUrl('tutorial/index.md')}>The full tutorial</CallToAction>
  </p>
</section>
