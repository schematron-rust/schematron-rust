# Streaming validation

Validates a document one repeating record at a time — parsed, matched,
fired, and discarded before the next is parsed — so peak memory stays
bounded by a thin ancestor chain plus one record, instead of growing with
the document. It exists for exactly the shape [`spec/roadmap/`](../roadmap/index.md)
named: a wrapper element around many repeating children, too large to
comfortably hold fully in memory, validated by a schema whose rules never
need more than one record's own subtree.

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
with zero such children (`<orders/>` or `<orders></orders>`) is not an
error — it is simply an empty report, the same as it would be under
ordinary validation.

## What a schema must look like

Every one of these is checked once, at schema-compile time — before any
document is read — and refuses the schema by name, the same way an
unimplemented XPath construct already does, rather than guessing at
runtime:

| Disqualifies streaming | Why |
|---|---|
| Any `<key>` declared | A key indexes the *whole* document; a per-record index would silently answer `key()` differently than a real one would, whether or not an active pattern actually calls it |
| `key()`, `id()`, or `document()`, anywhere in any expression | Each needs the whole document, or another one entirely |
| The `following::`, `preceding::`, or `following-sibling::` axis, anywhere in any expression — including inside a predicate | Each needs either the whole document or a record's *parent's* complete child list; see below |
| A schema-scoped, phase-scoped, or pattern-scoped `<let>` | Each is evaluated exactly once, against the document root, **before any record has been parsed** — even one using only safe axes would silently see none of the document's actual content |
| `pattern/@documents` | Validates *other* whole documents against the same pattern; orthogonal to streaming the primary one |
| `--parallel` together with `--stream` | The arena streaming reuses across records is not safe to share across threads |

Rule-scoped and assertion-scoped `<let>` are unaffected — each is bound
against the node the rule actually fired on, which is always within the
current record, and is exactly as safe under streaming as it is otherwise.

This check is schema-wide, not scoped to whichever phase actually runs —
the same choice [`Schema::uses_document_function`](../xpath/index.md)
already made for `document()`: simpler, and conservative in the direction
that never lets an unsound schema through, at the cost of occasionally
refusing a schema whose only disqualifying construct sits in a pattern
that would never actually be active.

**Refused, never silently approximated.** A schema or document that does
not qualify is a named [`Error::Streaming`], not a quiet fallback to
ordinary validation — someone reaching for `--stream` most likely did so
because the document will not fit fully in memory, and a fallback that
might exhaust it defeats the reason to ask for this in the first place.

### Why `following::`/`preceding::`/`following-sibling::` specifically

Every other axis reads only a node's already-parsed ancestor chain plus
its own subtree — data streaming always has, because a parent is always
parsed before its children. These three are the exceptions:

- `following::`/`preceding::` need every node before or after the current
  one *anywhere in the document* (`collect_axis`, `src/xpath/eval.rs`) —
  categorically the whole tree.
- `following-sibling::` needs the *rest of the current record's parent's*
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
