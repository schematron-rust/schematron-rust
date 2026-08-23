# SVRL — Schematron Validation Report Language

SVRL is the standard XML vocabulary for a Schematron validation report, in
namespace:

```
http://purl.oclc.org/dsdl/svrl
```

conventionally bound to `svrl`. Emitting it is what makes this crate's output
consumable by existing Schematron tooling.

## Document shape

```xml
<svrl:schematron-output
    xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
    title="..." phase="..." schemaVersion="...">

  <svrl:ns-prefix-in-attribute-values prefix="..." uri="..."/>*

  <svrl:active-pattern id="..." name="..." documents="..."/>
  <svrl:fired-rule id="..." context="..." role="..." flag="..."/>
  <svrl:failed-assert test="..." location="..." id="..." role="..." flag="...">
    <svrl:text>...</svrl:text>
    <svrl:diagnostic-reference diagnostic="..."><svrl:text>...</svrl:text></svrl:diagnostic-reference>*
    <svrl:property-reference property="..." role="..." scheme="..."><svrl:text>...</svrl:text></svrl:property-reference>*
  </svrl:failed-assert>
  <svrl:successful-report .../>

</svrl:schematron-output>
```

## Flat, not nested

`active-pattern`, `fired-rule`, `failed-assert`, and `successful-report` are
all siblings. The structure is implied by order, not by nesting: every
`fired-rule` belongs to the most recent `active-pattern`, and every
`failed-assert` and `successful-report` belongs to the most recent
`fired-rule`. This is what the reference implementation emits, because it is
what falls out of an XSLT streaming transform, and consumers depend on it.

The crate's internal report is a tree — `Report { patterns: [ { rules: [ { assertions } ] } ] }` —
and is flattened on the way out. That way the JSON output can keep the
structure while the SVRL output stays wire-compatible.

## Elements

| Element | Emitted when |
|---|---|
| `svrl:active-pattern` | A pattern begins running against a document. `@documents` is present only for `@documents` patterns. |
| `svrl:fired-rule` | A rule matches a node. Emitted once per matching node. |
| `svrl:failed-assert` | An `assert` whose test evaluated false. |
| `svrl:successful-report` | A `report` whose test evaluated true. |
| `svrl:text` | The instantiated human-readable message. |
| `svrl:diagnostic-reference` | Per `@diagnostics` reference on the assertion. |

When **reading** SVRL, a `diagnostic-reference` whose message is bare
character data rather than a nested `svrl:text` is accepted too. That is the
shape the ISO reference implementation writes, and it is much the most common
producer of SVRL; a reader that insisted on the nested form could not read the
output of the tool this crate is measured against. Only the element's own text
nodes are taken, never its descendants', so a `failed-assert` message never
absorbs the text of the diagnostics beneath it.
| `svrl:property-reference` | Per `@properties` reference on the assertion. |
| `svrl:ns-prefix-in-attribute-values` | Once per schema `ns`, so a consumer can interpret `@location` and `@test`. |

## Attributes on failed-assert / successful-report

| Attribute | Value |
|---|---|
| `test` | The assertion's XPath source text, verbatim |
| `location` | Absolute XPath to the subject node, see [validation.md](validation.md) |
| `id` | The assertion's `@id`, if any |
| `role` | Resolved role: assertion's, else rule's |
| `flag` | Resolved flag: assertion's, else rule's |
| `see`, `icon`, `fpi` | Passed through when present |

## Reading SVRL back

`Report::from_svrl` parses an SVRL document into a [`Report`], which makes the
crate's SVRL support bidirectional rather than write-only. Two things that
buys:

- **Round-trip testing.** Every report this crate produces is parsed back and
  compared against the original, which checks the writer far more thoroughly
  than asserting on substrings of its output.
- **Comparing processors.** A report from another Schematron implementation
  can be read in and diffed against this one's, which is the strongest
  evidence available that the two agree.

```rust
let report = Report::from_svrl(&svrl)?;
assert_eq!(report.count_failures(), 2);
```

### Reconstructing the tree from a flat document

SVRL is flat: `active-pattern`, `fired-rule`, `failed-assert` and
`successful-report` are all siblings, and the structure is implied by order.
The reader rebuilds the tree the writer flattened — each `fired-rule` belongs
to the most recent `active-pattern`, each finding to the most recent
`fired-rule`.

A finding that appears before any `fired-rule`, which is what
`--svrl-findings-only` output looks like, is attached to a synthetic rule so
that nothing is lost.

### What a round trip does not preserve

**`FiredRule::location`.** SVRL's `fired-rule` element carries `id`,
`context`, `role` and `flag`, and has nowhere to put the node the rule fired
on. That field therefore comes back empty. It is a real field — the text
report uses it — so the loss is stated here rather than papered over, and the
round-trip test compares reports with those locations cleared.

Everything else survives exactly: the schema title, phase and version, the
namespace bindings, every pattern and rule, and every finding with its test,
location, message, flags, diagnostics and properties.

## Verbosity

Emitting a `fired-rule` for every matching node is correct but verbose — a
large document can produce far more `fired-rule` elements than findings. The
writer therefore has two modes:

- **Full** (default, standard-conformant): every event.
- **Findings only**: `failed-assert` and `successful-report` only, plus the
  `active-pattern` elements that contain them.

`--svrl-findings-only` on the CLI selects the second.
