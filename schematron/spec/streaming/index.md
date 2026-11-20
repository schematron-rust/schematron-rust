# Streaming validation

Streaming validates one direct child of the document element at a time. Use it
for a large batch of independent records. It keeps the wrapper, the current
record and sibling-position counters. The returned report also retains its
findings and any requested rule-firing records.

```xml
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="order">
      <assert test="@id">An order must have an id.</assert>
    </rule>
  </pattern>
</schema>
```

```sh
schematron --schema rules.sch --stream orders.xml
```

```rust
use schematron::{Schema, ValidateOptions};

let schema = Schema::from_str(r#"..."#)?;
let file = std::fs::File::open("orders.xml")?;
let report = schema.validate_streaming(file, &ValidateOptions::new())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## The record boundary

The repeating unit is **the document element's direct children** —
`/root/record` — the single most common "batch file" shape: a wrapper
element (`<orders>`) around many repeating children (`<order>`). Deeper
nesting (`/root/group/record`) is not recognised; a document shaped that
way validates only under ordinary, non-streaming validation.

`--stream`/[`Schema::validate_streaming`] parses the document element
itself (the "skeleton") once, then parses and validates each of its direct
children in turn, discarding one before the next is parsed. A document
with zero such children (`<orders/>` or `<orders></orders>`) still runs
wrapper rules once. Record rules produce no findings. Both spellings
allow trailing XML whitespace, comments and processing instructions.

## What a schema must look like

The compiler checks the following schema constructs before reading the input.

| Disqualifies streaming | Why |
|---|---|
| Any `<key>` declared | A key indexes the *whole* document; a per-record index would silently answer `key()` differently than a real one would, whether or not an active pattern actually calls it |
| `key()`, `id()`, or `document()`, anywhere in any expression | Each needs the whole document, or another one entirely |
| `function-lookup()` or one-argument `resolve-uri()` | Dynamic lookup can hide a document-wide call. A streamed reader has no file base URI. |
| `following::`, `preceding::`, `following-sibling::` or `preceding-sibling::`, including inside predicates | Each can read a record the streaming reader has discarded or has yet to parse |
| Absolute paths in rule expressions, or parent and ancestor axes | A path can leave the current record and inspect other records. Absolute paths remain available as rule contexts. |
| Positional or non-attribute predicates in a rule context, or a filtered expression as its start | Matching a wrapper or numbering records needs input that has yet to arrive. Attribute predicates such as `order[@status="open"]` are supported. |
| Contexts ending in a node test that can select inter-record text, comments or processing instructions | These nodes fall outside the record subtrees. |
| A schema-scoped, phase-scoped, or pattern-scoped `<let>` | Each is evaluated exactly once, against the document root, **before any record has been parsed** — even one using only safe axes would silently see none of the document's actual content |
| `pattern/@documents` | Validates *other* whole documents against the same pattern; orthogonal to streaming the primary one |

Rule-scoped and assertion-scoped `<let>` use the node on which the rule
fires. Their expressions have the same record-locality requirement as
assertion tests.

The check covers the whole schema, including inactive patterns. A listed
construct produces `Error::Streaming`. The input wrapper and validation options determine the
additional checks below.

### Axes that can reach other records

The eligibility check rejects these axes wherever they occur:

- `following::`/`preceding::` need every node before or after the current
  one *anywhere in the document* (`collect_axis`, `src/xpath/eval.rs`) —
  categorically the whole tree.
- Both sibling axes can need the *rest of the current record's parent's*
  child list — but under streaming, a record's parent (the document
  element) only ever has the current record attached; every sibling that
  hasn't been parsed yet, or has already been discarded, is simply
  invisible.

## How it stays bounded: one reused arena slot, not a new node model

The document's arena (`Document`, `src/xml/document.rs`) is not
restructured to support this — `NodeId` stays exactly what it always was,
a plain index into a `Vec<NodeData>`. Instead, each record is parsed into
the *same* arena slot the previous one occupied: before parsing the next
record, the arena is truncated back to the skeleton's own length, and the
document element's stale child reference is cleared. A `NodeId` from the
previous record is simply never referenced again once this happens — the
same way a bump allocator reuses its arena, not a garbage collector
tracking liveness.

Two correctness details this depends on, both worth knowing before
touching the implementation (`src/xml/streaming.rs`, `src/validate/engine.rs`):

- **The document element's own rules fire exactly once, not once per
  record.** It stays resident across every record (it is part of the
  skeleton), so a rule matching it — `context="orders"`, checking a
  header attribute — is claimed and fired by one ordinary, whole-document
  `run_pattern` call over the bare skeleton, *before* the first record is
  parsed. The per-record pass that runs afterwards (`run_pattern_scoped_to_record`)
  explicitly discards any match outside the current record's own subtree,
  specifically so it never refires on the skeleton.
- **A finding's location needs the *true* cumulative sibling count, not
  "1st of 1."** `Document::finalize_subtree` numbers a node's children by
  counting whatever is currently attached to their parent — correct for a
  whole document built once, wrong here, since a record's parent only
  ever has one child attached at a time. The streaming reader tracks the
  real running count itself, per (kind, expanded name), and overwrites
  each record's own `sibling_position` after `finalize_subtree` runs, so
  `/orders/order[42]` still means the 42nd order, not always `[1]`.

## Limits

- **Wrapper rules.** When a rule fires on the wrapper, its tests, bindings,
  subjects and interpolations may use attributes, literals and operations on
  them, including comparisons and string functions such as `contains`.
  An unsupported expression raises `Error::Streaming`. For example, with
  `context="orders"`, `test="@version"` works and `test="order"` requires
  whole-document validation. A rule on an individual record can read its
  children.
- **Failure caps.** Streaming supports a cap with one active pattern.
  Multiple active patterns and a cap require whole-document pattern order.
  After reaching the cap, the reader recycles each remaining record and
  checks the XML through the epilog. A malformed tail is a parse error.
- **Report memory.** The report retains findings and, when requested, rule
  firings. The CLI also builds the rendered output in memory. For SVRL,
  `--svrl-findings-only` retains only rules with findings. When the exit
  code is sufficient, `--quiet` skips rendering and retains only rules
  with findings.

- **UTF-8 only.** [`Document::from_bytes`](../xml/index.md) detects a
  UTF-16 byte-order mark and transcodes; streaming does not, because doing
  so would mean decoding the whole input up front — exactly the cost this
  feature exists to avoid. A UTF-16 document needs ordinary, non-streaming
  validation.
- **Parse errors report a byte-tracked line and column, computed
  incrementally** rather than by scanning a buffered whole source, since
  streaming never has one; the numbers mean the same thing either way.
- **`--parallel` is refused together with `--stream`.** Parallelising
  *within* one record's own pattern set, across records, or both, is a
  possible future optimisation this phase does not attempt.

## Verifying the guarantee

The correctness bar this feature is held to: for an eligible schema and a
well-formed document, streaming and ordinary validation must produce
**the same findings, in the same order, at the same locations** —
`tests/streaming.rs` checks this directly, across a range of shapes
chosen because they could plausibly diverge (a rule on the document
element itself, whitespace and comments between records, deep
descendants, rule-scoped `let`, a self-closing record, zero records), not
merely that streaming runs without crashing.

### Measured peak memory

An Apple silicon macOS run used Rust 1.100.0 debug binaries and
`/usr/bin/time -l` to measure peak resident memory, including report
rendering. The generated XML had one `<orders>` wrapper, one `<line>`
per `<order>`, and one missing order ID in every 100 records. The schema
used one pattern and asserted `@id` on each `order`. Each row used a
fresh process with `--stream --format svrl`.

| Orders | Full SVRL | Findings only before the retention change | Findings only | Findings only with a cap of 10 |
|---:|---:|---:|---:|---:|
| 10,000 | 8.3 MiB | 8.5 MiB | 5.3 MiB | 5.1 MiB |
| 100,000 | 34.1 MiB | 26.6 MiB | 7.2 MiB | 5.1 MiB |
| 1,000,000 | 278.6 MiB | 205.4 MiB | 23.8 MiB | 5.1 MiB |

Both builds emitted the same number of failures and rule firings for each
mode. All runs returned exit code 1. The findings-only run still grew with
the number of failures. The cap held the report to ten failures throughout
this batch. Peak resident memory varies with workload and allocator.
