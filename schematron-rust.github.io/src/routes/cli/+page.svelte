<script lang="ts">
  import { Table, Alert, Separator, CallToAction, SectionHeading, CodeBlock, TableHead, TableBody, TableRow, TableTH, TableTD } from 'lily-design-system-svelte-headless';
  import { specUrl } from '$lib/site';

  type Option = { flag: string; description: string };

  const OPTIONS: Option[] = [
    { flag: '-s, --schema <PATH>', description: 'Schematron schema file. Required.' },
    { flag: '[DOCUMENT]...', description: 'XML documents to validate. "-" reads stdin.' },
    { flag: '-p, --phase <NAME>', description: 'Phase to run. #ALL, #DEFAULT, or a phase id.' },
    { flag: '-f, --format <FMT>', description: 'text (default), svrl, or json.' },
    { flag: '-o, --output <PATH>', description: 'Write the report here instead of stdout.' },
    { flag: '--flag <FLAG>', description: 'Report only assertions with this flag. Repeatable.' },
    { flag: '--max-failures <N>', description: 'Stop after N findings.' },
    { flag: '--parallel', description: 'Evaluate patterns on separate threads.' },
    { flag: '--svrl-findings-only', description: 'Omit fired-rule events from SVRL.' },
    { flag: '--allow-unknown-query-binding', description: 'Compile an xslt2/xslt3 schema anyway, best effort.' },
    { flag: '--list-phases', description: "Print the schema's phases and exit." },
    { flag: '--explain', description: 'Print the compiled schema: patterns, rules, contexts, tests.' },
    { flag: '--lint', description: 'Check the schema for likely mistakes and exit.' },
    { flag: '--portability', description: 'Report constructs other processors treat differently, and exit.' },
    { flag: '-q, --quiet', description: 'Suppress the report; use the exit code only.' },
    { flag: '-v, --verbose', description: 'Show the test and rule behind each finding, and rules that fired without finding anything.' }
  ];

  type ExitCode = { code: string; meaning: string };

  const EXIT_CODES: ExitCode[] = [
    { code: '0', meaning: 'Every document valid — no failed assertions; or, with --lint, no lints.' },
    { code: '1', meaning: 'At least one failed assertion; or, with --lint, at least one lint.' },
    { code: '2', meaning: 'Usage error — bad arguments.' },
    { code: '3', meaning: 'Schema error — the schema could not be compiled.' },
    { code: '4', meaning: 'Document error — an input document could not be parsed.' }
  ];
</script>

<svelte:head>
  <title>Command line — schematron</title>
  <meta
    name="description"
    content="The schematron command line tool: every option, every exit code, text and SVRL and JSON output, and the flags that diagnose a schema that does nothing."
  />
</svelte:head>

<div class="page-header">
  <h1>Command line</h1>
  <p>
    The binary is a thin shell over the library. The authoritative reference is
    <a href={specUrl('cli/index.md')}>spec/cli/</a>.
  </p>
</div>

<section class="section prose">
  <CodeBlock label="Install">
    <pre><code>{`cargo install schematron`}</code></pre>
  </CodeBlock>

  <CodeBlock label="Synopsis">
    <pre><code>{`schematron [OPTIONS] --schema <SCHEMA> [DOCUMENT]...`}</code></pre>
  </CodeBlock>
</section>

<section class="section">
  <SectionHeading class="section-heading-start" eyebrow="Reference" heading="Options" level={2} />

  <div class="table-scroll">
    <Table label="Command line options">
      <TableHead>
        <TableRow>
          <TableTH>Option</TableTH>
          <TableTH>Description</TableTH>
        </TableRow>
      </TableHead>
      <TableBody>
        {#each OPTIONS as option (option.flag)}
          <TableRow>
            <TableTH scope="row"><code>{option.flag}</code></TableTH>
            <TableTD>{option.description}</TableTD>
          </TableRow>
        {/each}
      </TableBody>
    </Table>
  </div>

  <div class="prose">
    <Alert type="info" role="status" heading="There is no network flag.">
      <p>
        The tool never fetches over the network. Vendor the included schema next
        to the one that includes it, or use the library with your own
        <code>Resolver</code>.
      </p>
    </Alert>
  </div>
</section>

<section class="section">
  <SectionHeading class="section-heading-start" eyebrow="Reference" heading="Exit codes" level={2} />

  <div class="table-scroll">
    <Table label="Exit codes">
      <TableHead>
        <TableRow>
          <TableTH>Code</TableTH>
          <TableTH>Meaning</TableTH>
        </TableRow>
      </TableHead>
      <TableBody>
        {#each EXIT_CODES as exit (exit.code)}
          <TableRow>
            <TableTH scope="row"><code>{exit.code}</code></TableTH>
            <TableTD>{exit.meaning}</TableTD>
          </TableRow>
        {/each}
      </TableBody>
    </Table>
  </div>

  <div class="prose">
    <p>
      Successful reports never affect the exit code by themselves.
      <code>--flag</code> filters what is <em>reported</em>, and therefore also
      what is counted for the exit code, so <code>--flag error</code> gives
      "fail only on errors, show warnings elsewhere".
    </p>
  </div>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Output" heading="Text output" level={2} />

  <p>
    The first column is the finding's <code>@flag</code> when the schema sets
    one, and otherwise <code>error</code> for a failed assertion or
    <code>report</code> for a successful report.
  </p>

  <CodeBlock label="Example text report">
    <pre><code>{`examples/invoice-bad.xml:
  error    /invoice[1]
           An invoice must have an id.
  error    /invoice[1]/line[1]
           Quantity must be positive, but is -2.
           - Quantity is the number of units ordered. It must be a positive number.
  warning  /invoice[1]
           Total is 99.00 but the lines plus tax come to 18.
  3 findings: 3 failed asserts, 0 reports`}</code></pre>
  </CodeBlock>

  <p>
    Lines beginning <code>-</code> are diagnostics. <code>--verbose</code> adds
    the <code>test:</code> and <code>rule:</code> behind each finding, which is
    how you find out why a rule fired — or, when the list of fired rules is
    empty, that none did.
  </p>
</section>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Recipes" heading="Examples" level={2} />

  <CodeBlock label="Common invocations">
    <pre><code>{`# Validate one document
schematron --schema rules.sch data.xml

# SVRL to a file, one phase only
schematron -s rules.sch -p strict -f svrl -o report.svrl data.xml

# Many documents; fail the build on errors but still show warnings
schematron -s rules.sch --flag error docs/*.xml

# Pipe from another tool
curl -s https://example.com/feed.xml | schematron -s feed.sch -

# Find out what a schema will actually do, before running it
schematron -s rules.sch --explain
schematron -s rules.sch --list-phases

# Check the schema itself for likely mistakes; no document needed
schematron -s rules.sch --lint

# Ask a different question: will this schema behave the same elsewhere?
schematron -s rules.sch --portability

# Find out why a schema appears to do nothing
schematron -s rules.sch --verbose --phase '#ALL' data.xml`}</code></pre>
  </CodeBlock>

  <p style="margin-top: 2rem;">
    <CallToAction class="button button-primary" href="/help/">Diagnosing a schema that does nothing</CallToAction>
    <CallToAction class="button button-secondary" href={specUrl('cli/index.md')}>spec/cli/</CallToAction>
  </p>
</section>
