<script lang="ts">
  import { Details, Alert, WarningCallout, Separator, CallToAction, SectionHeading, CodeBlock } from 'lily-design-system-svelte-headless';
  import { REPO, specUrl } from '$lib/site';
</script>

<svelte:head>
  <title>Help — schematron</title>
  <meta
    name="description"
    content="Diagnose a Schematron schema that does nothing, understand assert versus report, XPath 1.0 node-set comparison, and where to ask for help."
  />
</svelte:head>

<div class="page-header">
  <h1>Help</h1>
  <p>
    The problems people actually hit, and the flag that tells each of them
    apart.
  </p>
</div>

<section class="section prose">
  <SectionHeading
    class="section-heading-start"
    eyebrow="The big one"
    heading="My schema runs, and finds nothing"
    level={2}
  />

  <WarningCallout label="Two causes account for nearly all of them">
    <p>
      A missing <code>ns</code> prefix, or an earlier rule in the same pattern
      that claimed the nodes. The tool can tell them apart, and
      <code>--lint</code> detects both without needing a document at all.
    </p>
  </WarningCallout>

  <CodeBlock label="Start here">
    <pre><code>{`schematron -s rules.sch --lint`}</code></pre>
  </CodeBlock>

  <h3>Cause 1: a missing namespace prefix</h3>
  <p>
    XPath 1.0 has no default namespace, so an unprefixed name matches elements
    in <em>no</em> namespace. Against a namespaced document every context fails
    to match and nothing fires.
  </p>
  <CodeBlock label="Confirm it">
    <pre><code>{`schematron -s rules.sch --verbose --phase '#ALL' data.xml`}</code></pre>
  </CodeBlock>
  <p>
    An empty list of fired rules is the signature. The fix is to declare a
    prefix with <code>&lt;ns&gt;</code> and use it everywhere:
  </p>
  <CodeBlock label="The fix">
    <pre><code>{`<ns prefix="inv" uri="http://example.com/invoice"/>
<rule context="inv:invoice">
  <assert test="inv:total">An invoice must have a total.</assert>
</rule>`}</code></pre>
  </CodeBlock>

  <h3>Cause 2: an earlier rule claimed the nodes</h3>
  <p>
    Within a single pattern, each node is processed by at most one rule: the
    first whose context matches it.
  </p>
  <CodeBlock label="Confirm it">
    <pre><code>{`schematron -s rules.sch --explain`}</code></pre>
  </CodeBlock>
  <p>
    <code>--explain</code> marks every rule after the first in a pattern with a
    reminder that it only sees nodes no earlier rule claimed. The fix is to move
    the independent checks into <strong>separate patterns</strong> — patterns do
    not compete, and each gets its own pass over the document.
  </p>

  <Alert type="info" role="status" heading="In your own tests, assert on it.">
    <p>
      <code>report.count_fired_rules()</code> returning zero is the programmatic
      form of this symptom. Check it and a broken schema fails loudly instead of
      passing silently.
    </p>
  </Alert>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Frequently asked" heading="Other things that surprise people" level={2} />

  <Details summary="My assert fires when the document looks fine.">
    <p>
      <code>assert</code> fires when its test is <strong>false</strong>;
      <code>report</code> fires when its test is <strong>true</strong>. If a
      check fires on the good document and stays quiet on the bad one, the two
      are swapped.
    </p>
  </Details>

  <Details summary="not(a = b) and a != b behave differently.">
    <p>
      Correct, and both are sometimes what you want. Comparing a node-set with
      <code>=</code> in XPath 1.0 is <em>existential</em>: true if any node
      compares true. So <code>not(line/@type = 'discount')</code> means "no line
      is a discount", while <code>line/@type != 'discount'</code> means "some
      line is not a discount".
    </p>
  </Details>

  <Details summary="A cross-reference check is slow, or does not work.">
    <p>
      A cross-reference needs a key. The crate implements <code>key</code> and
      <code>key()</code> as a documented non-ISO extension —
      <a href={specUrl('keys/index.md')}>spec/keys/</a> explains why a
      cross-reference check needs one, and what the alternative costs.
    </p>
  </Details>

  <Details summary="My schema uses an XPath 2.0 construct and the crate refuses it.">
    <p>
      By design: outside the implemented subset you get a hard error naming the
      construct, rather than a plausible wrong answer.
      <a href={specUrl('xpath2/index.md')}>spec/xpath2/</a> lists what is in and what
      is out.
    </p>
  </Details>

  <Details summary="An include over http: fails.">
    <p>
      The default resolver refuses <code>http:</code> and <code>https:</code>,
      and the command line tool has no network flag. Vendor the included schema
      next to the one that includes it, or use the library with your own
      <code>Resolver</code>.
    </p>
  </Details>

  <Details summary="Turning on --parallel made things slower.">
    <p>
      Likely, on a small document: the threads cost more than they save.
      Parallel patterns are a performance switch, not a behaviour switch — the
      report is identical either way, so measure and keep whichever is faster
      for your inputs.
    </p>
  </Details>

  <Details summary="My schema works here but behaves differently under another processor.">
    <p>
      Run <code>schematron -s rules.sch --portability</code>. It reports the
      constructs that are correct here and treated differently by the ISO
      reference implementation — a <code>let</code> that shadows an outer one, a
      rule on <code>comment()</code>, a rule's <code>@flag</code>.
    </p>
  </Details>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Still stuck?" heading="Where to ask" level={2} />

  <p>
    Open an issue on GitHub. A schema, an input document, and what you expected
    is almost always enough — and if the answer turns out to be a gap in the
    specification, that is a defect worth fixing too.
  </p>

  <p style="margin-top: 2rem;">
    <CallToAction class="button button-primary" href={REPO + '/issues'}>Open an issue</CallToAction>
    <CallToAction class="button button-secondary" href="/spec/">Read the specification</CallToAction>
  </p>
</section>
