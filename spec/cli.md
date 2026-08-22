# Command line interface

The binary is a thin shell over the library. Built with `clap`.

```
schematron [OPTIONS] --schema <SCHEMA> [DOCUMENT]...
```

## Options

| Option | Description |
|---|---|
| `-s, --schema <PATH>` | Schematron schema file. Required. |
| `[DOCUMENT]...` | XML documents to validate. `-` reads stdin. |
| `-p, --phase <NAME>` | Phase to run. `#ALL`, `#DEFAULT`, or a phase id. |
| `-f, --format <FMT>` | `text` (default), `svrl`, or `json` |
| `-o, --output <PATH>` | Write the report here instead of stdout |
| `--flag <FLAG>` | Report only assertions with this flag. Repeatable. |
| `--max-failures <N>` | Stop after N findings |
| `--parallel` | Evaluate patterns on separate threads; see [validation.md](validation.md) |
| `--svrl-findings-only` | Omit `fired-rule` events from SVRL |
| `--allow-unknown-query-binding` | Compile an `xslt2`/`xslt3` schema anyway, best effort |
| `--list-phases` | Print the schema's phases and exit |
| `--explain` | Print the compiled schema: patterns, rules, contexts, tests |
| `--lint` | Check the schema for likely mistakes and exit; see [linting.md](linting.md) |
| `-q, --quiet` | Suppress the report; use the exit code only |
| `-v, --verbose` | Show the test and rule behind each finding, and rules that fired without finding anything |

There is no network flag. The tool never fetches over the network; vendor the
included schema next to the one that includes it, or use the library with your
own `Resolver`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Every document valid — no failed assertions; or, with `--lint`, no lints |
| 1 | At least one failed assertion; or, with `--lint`, at least one lint |
| 2 | Usage error — bad arguments |
| 3 | Schema error — the schema could not be compiled |
| 4 | Document error — an input document could not be parsed |

Successful reports never affect the exit code by themselves. `--flag` filters
what is *reported*, and therefore also what is counted for the exit code, so
`--flag error` gives "fail only on errors, show warnings elsewhere".

## Text output

The first column is the finding's `@flag` when the schema sets one, and
otherwise `error` for a failed assertion or `report` for a successful report.

```
examples/invoice-bad.xml:
  error    /invoice[1]
           An invoice must have an id.
  error    /invoice[1]/line[1]
           Quantity must be positive, but is -2.
           - Quantity is the number of units ordered. It must be a positive number.
  warning  /invoice[1]
           Total is 99.00 but the lines plus tax come to 18.
  3 findings: 3 failed asserts, 0 reports
```

Lines beginning `-` are diagnostics. `--verbose` adds the `test:` and `rule:`
behind each finding, which is how you find out why a rule fired — or, when the
list of fired rules is empty, that none did.

## Examples

```sh
# Validate one document
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

# Find out why a schema appears to do nothing
schematron -s rules.sch --verbose --phase '#ALL' data.xml
```

## Diagnosing a schema that does nothing

Two causes account for nearly all of them, and the tool can tell them apart:

1. **A missing `ns` prefix.** XPath 1.0 has no default namespace, so every
   context fails to match and no rule ever fires. `--verbose` shows no fired
   rules at all.
2. **An earlier rule in the same pattern claimed the nodes.** `--explain`
   marks every rule after the first in a pattern with a reminder that it only
   sees nodes no earlier rule claimed.

`--lint` detects both automatically, and needs no document:

```sh
schematron -s rules.sch --lint
```
