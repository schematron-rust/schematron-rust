# Tutorial

A walk from "what is a rule" to "a schema that pulls its weight". Every schema
and document here is real and runnable; they are the files in `examples/`.

## 1. The smallest useful schema

```xml
<?xml version="1.0" encoding="UTF-8"?>
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="invoice">
      <assert test="total">An invoice must have a total.</assert>
    </rule>
  </pattern>
</schema>
```

Read it as: *for every `invoice` element, assert that it has a `total` child.*

- `rule/@context` selects the nodes to check.
- `assert/@test` is an XPath evaluated with that node as the context node.
- The text is the message shown when the test is **false**.

```sh
schematron --schema examples/invoice.sch examples/invoice-bad.xml
```

## 2. assert versus report

They are opposites, and the naming trips everyone up once.

```xml
<rule context="invoice">
  <assert test="total">An invoice must have a total.</assert>
  <report test="count(line) &gt; 100">This invoice has an unusual number of lines.</report>
</rule>
```

- `assert` fires when its test is **false**. Use it for "must".
- `report` fires when its test is **true**. Use it for "notice that".

A `report` is not a failure. `Report::is_valid()` ignores successful reports,
and the CLI's exit code ignores them too.

## 3. Messages that say what went wrong

`Quantity must be positive` is worse than `Quantity is -2 on line 3`. Use
`value-of` and `name`:

```xml
<rule context="line">
  <assert test="number(@qty) &gt; 0">
    <name/> quantity must be positive, but is <value-of select="@qty"/>.
  </assert>
</rule>
```

## 4. First matching rule wins

This is the one piece of Schematron semantics you must internalise.

```xml
<pattern>
  <rule context="*">
    <assert test="true()">never fails</assert>
  </rule>
  <rule context="invoice">
    <assert test="total">never runs at all</assert>
  </rule>
</pattern>
```

The second rule never fires, because within a pattern each node is claimed by
the **first** rule whose context matches it, and `*` matches everything.

The fix is to use separate patterns:

```xml
<pattern>
  <rule context="*"><assert test="true()">…</assert></rule>
</pattern>
<pattern>
  <rule context="invoice"><assert test="total">…</assert></rule>
</pattern>
```

Patterns do not compete. Each gets its own pass over the document. Use the
competition deliberately — it is how you write "otherwise" branches:

```xml
<pattern>
  <rule context="line[@type='discount']">
    <assert test="number(@amount) &lt; 0">A discount must be negative.</assert>
  </rule>
  <rule context="line">
    <assert test="number(@amount) &gt;= 0">A normal line must not be negative.</assert>
  </rule>
</pattern>
```

## 5. Namespaces

XPath 1.0 has no default namespace. An unprefixed name in a test matches
elements in **no** namespace. So a schema for a namespaced document must
declare a prefix with `ns` and use it everywhere:

```xml
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <ns prefix="inv" uri="http://example.com/invoice"/>
  <pattern>
    <rule context="inv:invoice">
      <assert test="inv:total">An invoice must have a total.</assert>
    </rule>
  </pattern>
</schema>
```

Forgetting the prefix is the single most common reason a schema "does
nothing": every context fails to match, and no rule ever fires. Run with
`--verbose` to see which rules fired — an empty list is the tell.

## 6. Variables

```xml
<pattern>
  <let name="tax-rate" value="0.2"/>
  <rule context="invoice">
    <let name="expected" value="sum(line/@amount) * (1 + $tax-rate)"/>
    <assert test="number(total) &gt;= $expected - 0.01 and number(total) &lt;= $expected + 0.01">
      Total <value-of select="total"/> should be <value-of select="$expected"/>.
    </assert>
  </rule>
</pattern>
```

Scopes nest: `schema` → `phase` → `pattern` → `rule`. A rule-level `let` is
evaluated with the firing node as context, so it can depend on the node.

Note the comparison: XPath 1.0 has no `abs()`, so a tolerance is written as a
pair of inequalities. If you reach for a function and the schema refuses to
compile, check [xpath.md](xpath.md) — the message will tell you whether the
function is an XPath 2.0 one.

## 7. Diagnostics

Keep the terse message on the assertion and the long explanation in a
diagnostic, so the same explanation can serve several assertions:

```xml
<pattern>
  <rule context="line">
    <assert test="number(@qty) &gt; 0" diagnostics="qty-help">Quantity must be positive.</assert>
  </rule>
</pattern>
<diagnostics>
  <diagnostic id="qty-help">
    Quantity is the number of units ordered and must be a positive integer.
    Found <value-of select="@qty"/> on line <value-of select="@id"/>.
  </diagnostic>
</diagnostics>
```

## 8. Phases

Phases let one schema serve several strictnesses.

```xml
<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="basic">
  <phase id="basic">
    <active pattern="structure"/>
  </phase>
  <phase id="strict">
    <active pattern="structure"/>
    <active pattern="business-rules"/>
  </phase>

  <pattern id="structure">…</pattern>
  <pattern id="business-rules">…</pattern>
</schema>
```

```sh
schematron -s rules.sch -p strict data.xml
schematron -s rules.sch -p '#ALL' data.xml
```

## 9. Abstract patterns

When the same shape of rule applies to several element names, write it once:

```xml
<pattern abstract="true" id="required-child">
  <rule context="$parent">
    <assert test="$child">A <name/> must contain a <value-of select="'$child'"/>.</assert>
  </rule>
</pattern>

<pattern is-a="required-child" id="invoice-total">
  <param name="parent" value="invoice"/>
  <param name="child" value="total"/>
</pattern>

<pattern is-a="required-child" id="order-date">
  <param name="parent" value="order"/>
  <param name="child" value="date"/>
</pattern>
```

Substitution is textual: `$parent` becomes `invoice` before the expression is
parsed.

## 10. Abstract rules

Where abstract *patterns* parameterise a whole pattern, abstract *rules* share
assertions between rules in the same pattern:

```xml
<pattern>
  <rule abstract="true" id="dated">
    <assert test="@date">Must have a date.</assert>
    <assert test="string-length(@date) = 10">Date must be YYYY-MM-DD.</assert>
  </rule>
  <rule context="invoice">
    <extends rule="dated"/>
    <assert test="total">An invoice must have a total.</assert>
  </rule>
  <rule context="order">
    <extends rule="dated"/>
  </rule>
</pattern>
```

## 11. Reaching another document

Some constraints cannot be checked from one file: "every sku on this order
must exist in the catalogue". `document()` reads another document, and
`current()` refers back to the node the rule fired on, which is what lets the
two be correlated:

```xml
<pattern>
  <rule context="line">
    <assert test="document('catalogue.xml')/parts/part[@sku = current()/@sku]">
      No part in the catalogue has sku <value-of select="@sku"/>.
    </assert>
  </rule>
</pattern>
```

The URI can be computed, so `document(@href)` works, and a node-set argument
loads every document it names at once.

Two things to know. `document()` reads a document; it does not *validate* it —
no rule fires on the nodes it returns. To validate other documents, use
`pattern/@documents` instead, which runs the pattern's rules against each one:

```xml
<pattern id="parts" documents="catalog/ref/@href">
  <rule context="part">
    <assert test="name">Every part must have a name.</assert>
  </rule>
</pattern>
```

And URIs go through the resolver, which by default reads local files and
refuses `http:` and `https:`. Nothing fetches over the network unless your
application supplies a resolver that does.

## 12. Using the library

```rust
use schematron::{Document, Schema};

fn main() -> schematron::Result<()> {
    let schema = Schema::from_path("examples/invoice.sch")?;
    let doc = Document::from_path("examples/invoice-bad.xml")?;
    let report = schema.validate(&doc)?;

    for failure in report.failures() {
        println!("{} — {}", failure.location, failure.text);
    }
    println!("{}", report.to_svrl());
    Ok(())
}
```

Compile the schema once and reuse it. `Schema` is `Send + Sync`, so validating
a directory of documents in parallel needs no extra machinery.

## 13. When a schema seems to do nothing

This happens to everyone once. There are two causes, and the CLI distinguishes
them:

```sh
schematron -s rules.sch --verbose data.xml   # did any rule fire at all?
schematron -s rules.sch --explain            # what will this schema do?
```

- **No rules fired at all** → a namespace problem. The document is in a
  namespace and the schema's contexts are unprefixed. See step 5.
- **Rules fired, but not the one you wanted** → an earlier rule in the same
  pattern claimed the node. See step 4. `--explain` flags every rule that can
  only see leftovers.

## 14. Where to go next

- [validation.md](validation.md) — the exact algorithm, if a result surprises you
- [xpath.md](xpath.md) — what the expression language does and does not have
- [conformance.md](conformance.md) — the limits, stated up front
- [api.md](api.md) — the library API in full

Runnable code for everything above:

```sh
cargo run --example validate_file
cargo run --example report_formats
cargo run --example embedded_schema
cargo run --example parallel_validation
cargo run --example xpath_engine
```
