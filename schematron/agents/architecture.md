# Architecture

Four layers, each usable on its own, each depending only on those above it.

```
bytes ──► xml ──► xpath ──► schema ──► validate ──► svrl / text / json
```

| Layer | Module | Job | Knows about Schematron? |
|---|---|---|---|
| XML | `src/xml/` | Bytes to an XPath 1.0 data model | No |
| XPath | `src/xpath/` | Evaluate XPath 1.0 against that model | No |
| Schema | `src/schema/` | `.sch` to a compiled, immutable `Schema` | Yes |
| Validate | `src/validate/` | Run a schema over a document, produce a `Report` | Yes |
| Output | `src/svrl.rs`, `src/text.rs` | Render a `Report` | Only the format |

The separation is load-bearing, not decorative. The XML and XPath layers have
no Schematron types in them at all, which is why `examples/xpath_engine.rs`
can use the engine standalone, and why the XPath test suite can be written
without constructing a schema.

## Why we do not transpile to XSLT

The reference implementation of Schematron compiles a schema into an XSLT
stylesheet and runs it. That is why every other route to Schematron needs a
C library or a JVM.

This crate **interprets** the schema directly against its own XPath engine.
The costs are that we must implement XPath ourselves, and that XSLT-specific
features (`key()`, `document()`) are not free. The benefits are that there is
no C dependency, error messages point at the schema rather than at generated
XSLT, and the compile-once-validate-many optimisation is straightforward.

Do not reintroduce a transpilation step.

## Layer notes

### `src/xml/`

An **arena**: `Document` owns `Vec<NodeData>` and a `NodeId` is an index.
Cheap `Copy` handles, O(1) parent access, no reference cycles to fight the
borrow checker with.

All seven XPath node kinds are modelled. Attribute and namespace nodes have
the element as parent but are *not* in its child list — they are reachable
only through their own axes, which is what makes `child::node()` correct.

Two precomputed fields exist purely to avoid quadratic behaviour, and both
were added in response to a benchmark, not a hunch:

- `subtree_end` — the highest document-order value in a node's subtree. Turns
  "is `x` a descendant of `y`" into an integer range check, which keeps the
  `following` and `preceding` axes linear.
- `sibling_position` — index among same-kind, same-name siblings. Generating
  an SVRL location would otherwise rescan the parent's child list per finding.

Both are computed in one pass by `Document::finalize()`. **Any code that
builds a tree must call it** — the parser does, and so does the include
resolver, which rebuilds the tree.

### `src/xpath/`

`lexer` → `parser` → `Expr` → `eval`. The lexer resolves XPath's
context-sensitive token classes (is `*` a wildcard or a multiply? is `div` a
name or an operator?) so the parser can be plain recursive descent.

The engine serves **two languages**. `XPathVersion` comes from the schema's
query binding and gates the XPath 2.0 additions: the parser accepts
`if (…) then … else …` unconditionally — it is not valid XPath 1.0 anyway —
and `Schema::check_expression` rejects it under a 1.0 binding, which keeps the
version out of the parser's signature. Functions are gated the same way, by
two signature tables.

Every XPath 2.0 function this crate does not implement is listed by name in
one of three tables — grouped by why it's missing — so the error can say
*why* a function is missing rather than merely that it is. That is the
difference between an honest subset and a dangerous one.

Recursion is depth-limited. Values are the four XPath 1.0 types and the
conversions between them are exact, including the number-to-string format,
which is the detail engines most often get wrong.

### `src/schema/`

Five passes; see [`spec/parsing/`](../spec/parsing/index.md). The important
structural decision: **XPath expressions are held as source strings through
passes 1–4, and compiled in pass 5**, cached on the `Schema` keyed by source
text. This is what lets abstract-pattern parameter substitution be textual,
which is how the standard defines it — `$parent` can stand for an element
name, not merely for a value.

`Schema` is immutable and `Send + Sync`. Compile once, validate anywhere.

### `src/validate/`

`engine.rs` implements the algorithm in
[`spec/validation/`](../spec/validation/index.md). The performance-critical
decision is that a rule's context is evaluated **once per document**, rewritten
into an absolute expression, rather than tested node by node:

| Context | Evaluated as |
|---|---|
| `a/b` | `/descendant-or-self::node()/a/b` |
| `/a/b` | unchanged |

Each rule then claims the nodes no earlier rule in its pattern has claimed,
which is where first-matching-rule-wins actually lives.

### Output

A `Report` is **data**. The internal shape is a tree — patterns contain rules
contain findings — and SVRL flattens it on the way out because that is what
the standard's consumers expect. JSON keeps the tree. Never make the report a
pile of pre-formatted strings.

## Cross-document node-sets

A node-set is `Vec<NodeId>` into **one** arena, so `document()` — which
returns nodes of a different document — needed that arena to be shared. A
loaded document is copied in beside the instance with its own parentless root,
and document order continues across the merge.

Two consequences ripple through the layers, and both are easy to get wrong:

- **`/` is per-document.** `Document::root_of(node)` gives the root of the
  document a node belongs to, and absolute paths, `following`, `preceding`,
  and `id()` all use it. Using `Document::root()` there would silently search
  the instance from inside a loaded document.
- **Rules never fire on loaded nodes.** `all_nodes_in_document_order()` walks
  from the primary root only, so a merged document is readable but not
  validated.

Loading itself cannot happen during evaluation, because the tree is held
immutably. A `document()` call records a miss; the validator loads what was
asked for and runs again. See [`spec/xpath/`](../spec/xpath/index.md).

`Schema::uses_document_function()` is computed at compile time so that the
common case — a schema that never calls it — takes an early return in
`validate()` and pays nothing: no working copy of the tree, no second pass.

## `Run`

`validate/engine.rs` threads a `Run` struct — schema, document, registry,
options — rather than four separate parameters. `Run::on(document)` points it
at a different tree, which is how a `@documents` pattern validates an external
document, and `Run::context(node, variables)` builds an evaluation context
already wired to the registry. Every helper destructures the fields it needs
at its first line.
