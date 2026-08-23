# Keys

A key is a named index over a document, declared once and looked up many
times. It exists for one reason: cross-reference checks are the most common
expensive thing a Schematron schema does, and without an index they are
quadratic.

```xml
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <key name="parts" match="part" use="@id"/>
  <pattern>
    <rule context="line">
      <assert test="key('parts', @ref)">
        No part has id <value-of select="@ref"/>.
      </assert>
    </rule>
  </pattern>
</schema>
```

## Why it is worth having

The same constraint without a key reads:

```xml
<assert test="//part[@id = current()/@ref]">…</assert>
```

That evaluates `//part` once for every `line`, and scans it. A document with
*n* lines and *n* parts does *n²* work. With a key, the index is built once
and each lookup is a hash probe: *n* work to build, and *n* probes.

Figures and the benchmark that produces them are in
[testing.md](testing.md#benchmarks).

## Declaring one

```xml
<key name="NAME" match="PATTERN" use="EXPRESSION"/>
```

| Attribute | Meaning |
|---|---|
| `name` | What `key()` will call it. Required, and unique within the schema. |
| `match` | Which nodes to index. An XSLT match pattern, exactly as `rule/@context` is. Required. |
| `use` | The key value, evaluated with each matched node as the context node. Required. |

`key` elements sit at the top level of the schema, beside `pattern`. A key is
global: every pattern and rule can use it.

If `use` selects several nodes, the matched node is indexed under **each** of
their string values, which is what XSLT does and what makes a
multiply-referenced node findable by any of its identifiers.

## Looking one up

```
key(NAME, VALUE)
```

Returns the nodes indexed under `VALUE`, in document order, or the empty
node-set when there are none — so `key('parts', @ref)` is directly usable as
an assertion test.

`VALUE` is compared as a string. When `VALUE` is a node-set, the result is
every node matching **any** of its string values, which is the existential
behaviour the rest of XPath uses.

## Where this sits in the standard

`<sch:key>` is Schematron 1.5. ISO/IEC 19757-3 dropped it, on the grounds
that the `xslt` query binding already gives access to XSLT's `key()` — but
ISO Schematron has no element for declaring one, so in practice a schema
cannot use keys at all without an extension.

This crate therefore accepts `<sch:key>` as an **extension**, in the
Schematron namespace, available under every query binding. A schema that uses
it is not portable to a processor that does not; that is the trade, and it is
stated here rather than discovered. Everything else in the schema remains
ISO-conformant.

## Limits

- **Keys index one document.** A key built for the instance document does not
  index documents loaded by `document()`, and `key()` inside a
  `pattern/@documents` run indexes the target document rather than the
  instance. This matches XSLT, where `key()` applies to the current document.
- **Indexes are built eagerly**, once per document per validation run, before
  any pattern runs. A declared key costs its build even if no expression uses
  it — a schema that declares a key it never looks up is paying for nothing,
  which the linter reports.
- **`match` is a match pattern**, so the same restriction applies as to
  `rule/@context`: a leading reverse axis is rejected. See
  [validation.md](validation.md).
