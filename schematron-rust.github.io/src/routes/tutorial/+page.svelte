<script lang="ts">
  import { Alert, WarningCallout, Separator, CallToAction, SectionHeading, CodeBlock } from 'lily-design-system-svelte-headless';
  import { REPO, specUrl } from '$lib/site';
</script>

<svelte:head>
  <title>Tutorial — schematron</title>
  <meta
    name="description"
    content="Write your first Schematron schema: assert and report, first-matching-rule, namespaces, variables, diagnostics, phases, and the two ways a schema silently does nothing."
  />
</svelte:head>

<div class="page-header">
  <h1>Tutorial</h1>
  <p>
    Enough Schematron to write a real schema. The full eighteen-step version,
    with every worked file, is
    <a href={specUrl('tutorial/index.md')}>spec/tutorial/</a> in the repo.
  </p>
</div>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Step 1" heading="The smallest useful schema" level={2} />

  <p>
    A schema is patterns; a pattern is rules; a rule is a context plus
    assertions.
  </p>

  <CodeBlock label="A minimal schema">
    <p class="code-block-caption">rules.sch</p>
    <pre><code>{`<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="invoice">
      <assert test="total">An invoice must have a total.</assert>
    </rule>
  </pattern>
</schema>`}</code></pre>
  </CodeBlock>

  <p>
    <code>@context</code> is an XPath expression selecting the nodes the rule
    applies to. <code>@test</code> is an XPath expression evaluated with each
    such node as the context node.
  </p>

  <CodeBlock label="Run it">
    <pre><code>{`schematron --schema rules.sch data.xml`}</code></pre>
  </CodeBlock>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Step 2" heading="assert versus report" level={2} />

  <p>
    They are opposites, and the direction catches everyone once.
  </p>

  <ul>
    <li><code>assert</code> fires when its test is <strong>false</strong> — "this must be true".</li>
    <li><code>report</code> fires when its test is <strong>true</strong> — "tell me if this happens".</li>
  </ul>

  <CodeBlock label="assert and report side by side">
    <pre><code>{`<rule context="invoice">
  <assert test="total">An invoice must have a total.</assert>
  <report test="count(line) > 100">This invoice has an unusual number of lines.</report>
</rule>`}</code></pre>
  </CodeBlock>

  <p>
    An <code>assert</code> that fires is a failure. A <code>report</code> that
    fires is an observation. Both land in the report; only failed asserts make a
    document invalid.
  </p>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Step 3" heading="Messages that say what went wrong" level={2} />

  <p>
    Assertion text is mixed content, so it can interpolate the document with
    <code>value-of</code> and name the offending node with <code>name</code>:
  </p>

  <CodeBlock label="Interpolated message text">
    <pre><code>{`<assert test="number(@qty) > 0">
  Quantity must be positive, but <name/> has <value-of select="@qty"/>.
</assert>`}</code></pre>
  </CodeBlock>

  <p>
    Longer explanations belong in a <code>diagnostic</code>, referenced by
    <code>@diagnostics</code>, so the message stays short and the help stays
    available:
  </p>

  <CodeBlock label="A diagnostic">
    <pre><code>{`<assert test="number(@qty) > 0" diagnostics="qty-help">
  Quantity must be positive, but is <value-of select="@qty"/>.
</assert>

<diagnostics>
  <diagnostic id="qty-help">
    Quantity is the number of units ordered. It must be a positive number.
  </diagnostic>
</diagnostics>`}</code></pre>
  </CodeBlock>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Step 4" heading="First matching rule wins" level={2} />

  <WarningCallout label="The rule that explains most surprises">
    <h3>Within a single pattern, each node is processed by at most one rule: the first whose context matches it.</h3>
    <p>
      Rules in one pattern compete like the arms of a match expression. Rules in
      different patterns do not compete at all.
    </p>
  </WarningCallout>

  <p>Use it deliberately, to write an "otherwise" branch:</p>

  <CodeBlock label="Deliberate alternatives">
    <pre><code>{`<pattern id="lines">
  <rule context="line[@type='discount']">
    <assert test="number(@amount) < 0">A discount must be negative.</assert>
  </rule>
  <rule context="line">
    <assert test="number(@amount) >= 0">A normal line must not be negative.</assert>
  </rule>
</pattern>`}</code></pre>
  </CodeBlock>

  <p>And watch for it shadowing a rule you meant to run:</p>

  <CodeBlock label="An accidentally shadowed rule">
    <pre><code>{`<pattern>
  <rule context="*">…</rule>
  <rule context="invoice">…</rule>  <!-- never runs -->
</pattern>`}</code></pre>
  </CodeBlock>

  <p>
    To apply independent checks to the same node, put them in
    <strong>separate patterns</strong>. Each pattern gets its own pass over the
    document.
  </p>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Step 5" heading="Namespaces" level={2} />

  <p>
    XPath 1.0 has no default namespace. An unprefixed name in a test matches
    elements in <em>no</em> namespace — so against a namespaced document, every
    context fails to match and nothing fires.
  </p>

  <CodeBlock label="Declaring a prefix">
    <pre><code>{`<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <ns prefix="inv" uri="http://example.com/invoice"/>
  <pattern>
    <rule context="inv:invoice">
      <assert test="inv:total">An invoice must have a total.</assert>
    </rule>
  </pattern>
</schema>`}</code></pre>
  </CodeBlock>

  <p>
    Declare the prefix with <code>&lt;ns&gt;</code> and use it
    <strong>everywhere</strong> — in contexts, in tests, in
    <code>select</code> attributes.
  </p>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Steps 6–8" heading="Variables, diagnostics, phases" level={2} />

  <p>
    <code>let</code> binds a name to an expression, at schema, pattern, or rule
    scope. A rule-scope <code>let</code> is evaluated with the rule's context
    node:
  </p>

  <CodeBlock label="Variables">
    <pre><code>{`<let name="tax-rate" value="0.2"/>

<rule context="invoice">
  <let name="lines-total" value="sum(line/@amount)"/>
  <assert test="number(total) = $lines-total * (1 + $tax-rate)">
    Total is <value-of select="total"/> but the lines plus tax come to
    <value-of select="$lines-total * (1 + $tax-rate)"/>.
  </assert>
</rule>`}</code></pre>
  </CodeBlock>

  <p>
    <code>phase</code> selects which patterns run, so one schema can serve a
    quick structural check and a full audit:
  </p>

  <CodeBlock label="Phases">
    <pre><code>{`<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="basic">
  <phase id="basic">
    <active pattern="structure"/>
  </phase>
  <phase id="strict">
    <active pattern="structure"/>
    <active pattern="lines"/>
    <active pattern="totals"/>
  </phase>
  …
</schema>`}</code></pre>
  </CodeBlock>

  <CodeBlock label="Choosing a phase">
    <pre><code>{`schematron -s rules.sch --list-phases
schematron -s rules.sch --phase strict data.xml
schematron -s rules.sch --phase '#ALL' data.xml`}</code></pre>
  </CodeBlock>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Step 13" heading="= is not eq" level={2} />

  <p>
    In XPath 1.0, comparing a node-set with <code>=</code> is an
    <em>existential</em> comparison: it is true if <em>any</em> node in the set
    compares true. That is usually what you want in a Schematron test — and
    occasionally exactly what you did not want.
  </p>

  <Alert type="info" role="status" heading="Negation does not distribute over an existential comparison.">
    <p>
      <code>not(line/@type = 'discount')</code> means "no line is a discount".
      <code>line/@type != 'discount'</code> means "some line is not a discount".
      These are different tests, and both are sometimes correct.
    </p>
  </Alert>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Step 17" heading="When a schema seems to do nothing" level={2} />

  <p>Two causes account for nearly all of them, and the tool can tell them apart:</p>

  <ol>
    <li>
      <strong>A missing <code>ns</code> prefix.</strong> Every context fails to
      match, so no rule ever fires. <code>--verbose</code> shows no fired rules
      at all.
    </li>
    <li>
      <strong>An earlier rule in the same pattern claimed the nodes.</strong>
      <code>--explain</code> marks every rule after the first in a pattern with
      a reminder that it only sees nodes no earlier rule claimed.
    </li>
  </ol>

  <CodeBlock label="Diagnose without a document">
    <pre><code>{`schematron -s rules.sch --lint`}</code></pre>
  </CodeBlock>

  <p><a href="/help/">More on diagnosing a quiet schema &rarr;</a></p>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Step 18" heading="Where to go next" level={2} />

  <p>
    The repo ships a worked schema — <code>examples/invoice.sch</code> with
    <code>invoice-good.xml</code> and <code>invoice-bad.xml</code> — used
    throughout the full tutorial and the CLI tests. Clone it and run it:
  </p>

  <CodeBlock label="Run the worked example">
    <pre><code>{`git clone https://github.com/schematron-rust/schematron-rust
cd schematron-rust
cargo run -- -s examples/invoice.sch -p strict -v examples/invoice-bad.xml`}</code></pre>
  </CodeBlock>

  <p style="margin-top: 2rem;">
    <CallToAction class="button button-primary" href={specUrl('tutorial/index.md')}>The full eighteen-step tutorial</CallToAction>
    <CallToAction class="button button-secondary" href="/library/">Use the library</CallToAction>
  </p>
  <p><a class="back-link" href={REPO}>&larr; Everything else is in the repo</a></p>
</section>
