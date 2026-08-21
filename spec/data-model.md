# Schematron data model

The Schematron vocabulary lives in namespace:

```
http://purl.oclc.org/dsdl/schematron
```

conventionally bound to the prefix `sch`. The legacy Schematron 1.5 namespace
`http://www.ascc.net/xml/schematron` is recognised on input for compatibility.

Every element below maps to a Rust type in `schematron::schema`.

## `schema` — the root

```text
<schema id? schemaVersion? defaultPhase? queryBinding? xml:lang?>
  include*, title?, ns*, p*, let*, phase*, pattern+, p*, diagnostics?, properties?
</schema>
```

| Attribute | Meaning |
|---|---|
| `id` | Identifier for the schema |
| `schemaVersion` | Version of the *schema*, opaque to the processor |
| `defaultPhase` | `phase/@id` to use when the caller does not name one |
| `queryBinding` | Query language binding, see [conformance.md](conformance.md) |
| `xml:lang` | Natural language of the human-readable text |

Rust: `Schema { id, schema_version, default_phase, query_binding, lang, title, namespaces, lets, phases, patterns, diagnostics, properties, paragraphs }`

## `ns` — namespace binding

```xml
<ns prefix="..." uri="..."/>
```

Declares a prefix usable in every `@context`, `@test`, `@select`, and `@value`
XPath expression in the schema. Schematron does *not* inherit prefixes from the
schema document's own XML namespace declarations; `ns` is the only channel.
Both attributes are required.

Rust: `Ns { prefix, uri }`

## `phase` and `active` — validation phases

```text
<phase id="..."> <active pattern="..."/>* </phase>
```

A phase names a subset of patterns to run. Selecting a phase at validation
time restricts the run to the patterns its `active` children reference. Two
values of `@phase` are reserved:

- `#ALL` — every pattern is active. This is the default when the schema has no
  `defaultPhase` and the caller names no phase.
- `#DEFAULT` — use the schema's `defaultPhase`, falling back to `#ALL`.

Rust: `Phase { id, actives: Vec<Active>, lets, paragraphs }`, `Active { pattern }`

## `pattern` — a group of rules

```text
<pattern id? abstract? is-a? documents? title? >
  (let*, rule*) | param*
</pattern>
```

A pattern is the unit of rule competition: within one pattern, at most one
rule fires per node. See [validation.md](validation.md).

Three kinds:

- **Concrete** — no `@abstract`, no `@is-a`. Contains rules; runs directly.
- **Abstract** — `abstract="true"`. A template. Never runs. Its rules contain
  `$name` placeholders. Must have an `@id`. Must not have `@is-a`.
- **Instance** — `is-a="ID"` referencing an abstract pattern. Contains only
  `param` children. Expanded before validation.

`@documents` holds an XPath expression evaluated against the instance
document; each resulting node's string value is a URI of an *external*
document, and the pattern's rules run against each such document instead of
the instance. See [validation.md](validation.md).

Rust: `Pattern { id, is_abstract, is_a, documents, title, lets, rules, params, paragraphs }`

## `param` — abstract pattern argument

```xml
<param name="..." value="..."/>
```

Both attributes required. Inside an instance pattern, every occurrence of
`$name` in the abstract pattern's `@context`, `@test`, `@select`, `@value`,
and `@subject` attributes is replaced by `value` textually. See
[parsing.md](parsing.md) for the exact substitution rule.

Rust: `Param { name, value }`

## `rule` — a context and its assertions

```text
<rule context? id? abstract? flag? role? subject?>
  let*, (assert | report | extends)*
</rule>
```

| Attribute | Meaning |
|---|---|
| `context` | XPath pattern selecting the nodes this rule applies to. Required for concrete rules, forbidden for abstract ones. |
| `abstract` | `"true"` makes this a reusable fragment, referenced via `extends`. Requires `@id`, forbids `@context`. |
| `flag` | Free-form label propagated to every assertion that does not override it |
| `role` | Free-form label describing the rule's role |
| `subject` | XPath naming the node the rule is *about*, when that differs from the context node |

Rust: `Rule { context, id, is_abstract, flag, role, subject, lets, assertions, extends }`

## `assert` and `report` — the assertions

```text
<assert test="..." id? flag? role? subject? diagnostics? properties? see? icon? fpi?>
  mixed content
</assert>
```

Same attributes for both. They differ only in polarity:

- `assert` — the constraint holds when `test` is **true**. A **false** test is
  a failed assertion, and is reported.
- `report` — the constraint is an observation. A **true** test is a successful
  report, and is reported.

`@diagnostics` is a whitespace-separated list of `diagnostic/@id` references.
`@properties` is a whitespace-separated list of `property/@id` references.

Rust: `Assertion { kind: AssertionKind::{Assert,Report}, test, id, flag, role, subject, diagnostics, properties, see, icon, fpi, content: Vec<Content> }`

## Rich content

The mixed content of `assert`, `report`, `diagnostic`, `title`, and `p` is a
sequence of:

| Element | Meaning |
|---|---|
| text | Literal text |
| `<value-of select="..."/>` | XPath evaluated in the assertion's context; inserts its string value |
| `<name path?/>` | Name of the context node, or of the node selected by `@path` |
| `<emph>`, `<span class?>`, `<dir value?>` | Presentation hints; content is rendered, markup is kept in the model |

Rust: `Content::{Text(String), ValueOf{select}, Name{path}, Emph(Vec<Content>), Span{class, content}, Dir{value, content}}`

## `let` — variables

```xml
<let name="..." value="..."/>
<let name="...">node content</let>
```

Binds an XPath variable visible to all expressions in the enclosing scope and
below. Scopes, outermost first: `schema` → `phase` → `pattern` → `rule`. An
inner binding shadows an outer one of the same name.

Schema-level and phase-level `let` values are evaluated against the document
root. Pattern-level and rule-level `let` values are evaluated against the
context node of the firing rule.

Rust: `Let { name, value: LetValue::{Expression(String), Nodes(Vec<Content>)} }`

## `diagnostics` and `diagnostic`

```text
<diagnostics> <diagnostic id="..."> mixed content </diagnostic>* </diagnostics>
```

Reusable diagnostic messages, referenced by `assert/@diagnostics`. Rendered in
the assertion's context, exactly like assertion content.

Rust: `Diagnostic { id, lang, content: Vec<Content> }`

## `properties` and `property`

```text
<properties> <property id="..." role? scheme?> mixed content </property>* </properties>
```

Introduced in ISO/IEC 19757-3:2016. Machine-oriented values attached to a
report, referenced by `assert/@properties`.

Rust: `Property { id, role, scheme, content: Vec<Content> }`

## `include`

```xml
<include href="..."/>
```

Replaced by the element the `href` points to, before any other processing.
See [parsing.md](parsing.md).

## `extends`

```xml
<extends rule="..."/>
<extends href="..."/>
```

Inside a rule, splices in the assertions of the referenced abstract rule.

## `title` and `p`

Human-readable annotations. `p` carries optional `@id`, `@class`, `@icon`.
Neither affects validation.
