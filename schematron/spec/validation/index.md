# Validation semantics

This is the core of the standard, and the core of the crate. Everything else
is plumbing.

## The algorithm

Given a schema `S`, an instance document `D`, and a phase `P`:

```
1. Resolve the active pattern set A from P.
2. Bind schema-level `let` variables, evaluated with D's root as context node.
3. Bind phase-level `let` variables (phase P only), same context.
4. For each pattern p in A, in document order of the schema:
     4a. Determine the target documents T:
           - if p has no @documents: T = [D]
           - else: T = each document named by evaluating @documents with the
             ROOT NODE of D as the context node. The root node, not the
             document element, which is what the ISO XSLT skeleton does — so
             the expression is written `catalog/ref/@href`, not `ref/@href`.
     4b. For each target document t in T:
           - Bind pattern-level `let`, context = root of t.
           - Record that p is active for t.
           - For each node n of t in document order
             (root, then elements, then their attributes, then their children,
              including text, comment, and processing-instruction nodes):
               - Find the FIRST rule r in p whose @context pattern matches n.
               - If no rule matches, n is skipped for this pattern.
               - If some rule matches:
                   - Record r as fired for n.
                   - Bind rule-level `let`, context = n.
                   - For each assertion a in r, in order:
                       - Evaluate a.test with context node n.
                       - assert : if the result is false -> failed assertion.
                       - report : if the result is true  -> successful report.
5. Emit the recorded events in the order they occurred.
```

## The one rule that matters most

> Within a single pattern, a node is processed by **at most one** rule: the
> first rule, in document order, whose context matches that node.

This is *not* "every matching rule fires". It is a switch statement, not a
list of independent checks. Rules later in the same pattern act as `else`
branches for nodes the earlier rules already claimed.

Two consequences that bite people:

1. To apply several independent rule sets to the same node, put them in
   **separate patterns**. Patterns do not compete with each other; every
   pattern gets its own pass over the document.
2. A broad rule placed early (`context="*"`) silently disables every narrower
   rule after it in the same pattern.

The crate reports which rule fired for which node, so this is observable
rather than mysterious — see `svrl:fired-rule` in [svrl/](../svrl/index.md).

## Context matching

`rule/@context` is an XSLT *pattern*, not a general XPath expression. A node
`n` matches pattern `c` when `n` is a member of the node-set that `c` selects
from some ancestor-or-self of `n`, or from the root.

The crate implements this by rewriting the pattern into an absolute
expression and evaluating it **once per document**, rather than testing every
rule against every node:

| Pattern | Rewritten as |
|---|---|
| `a/b` | `/descendant-or-self::node()/a/b` |
| `@qty` | `/descendant-or-self::node()/@qty` |
| `/a/b` | unchanged — already absolute |
| `a \| b` | each branch rewritten separately |

This is the classic XSLT pattern-matching reduction, and it gives the correct
answer for the pattern subset Schematron schemas use in practice: `name`,
`prefix:name`, `*`, `@attr`, `node()`, `text()`, `a/b`, `a//b`, `/a`,
`a[pred]`, and `|` unions of those. Predicates keep their proper meaning:
`line[1]` still means "the first `line` of its own parent", not "the first
`line` in the document".

Evaluating once per rule per document rather than once per rule per node is
what makes matching linear in document size. Each rule claims the nodes no
earlier rule in its pattern has already claimed, which is where
first-matching-rule-wins is actually implemented.

Patterns using the reverse axes (`ancestor::`, `preceding::`, …) at the top
level are not valid XSLT patterns and are rejected at schema-compile time,
with a message suggesting a predicate — `a[ancestor::b]` — instead.

## Node visiting order

Rules are offered nodes in document order. The crate's document order is:

1. The root node.
2. Each element, when its start tag is reached.
3. That element's namespace nodes, then its attribute nodes.
4. That element's children, recursively — elements, text, comments, and
   processing instructions interleaved as they appear.

Attribute and namespace nodes therefore precede the element's children, which
is what XPath specifies.

## Variable scoping

```
schema let      context = root of D          visible everywhere
  phase let     context = root of D          visible in that phase's patterns
    pattern let context = root of target doc  visible in that pattern
      rule let  context = the firing node     visible in that rule
```

Bindings are evaluated in document order within a scope, and an earlier
binding in the same scope is visible to a later one. An inner binding shadows
an outer binding of the same name for the rest of the inner scope. A reference
to a variable that nothing anywhere binds is caught when the schema loads; a
reference to one that exists but is out of reach here is an error at
validation, not an empty node-set. See [parsing/](../parsing/index.md).

`<let name="x" value="expr"/>` binds the XPath value of `expr`.
`<let name="x">content</let>` binds a string built from the rich content.

## Phases

| `@phase` argument | Meaning |
|---|---|
| absent | Use the schema's `defaultPhase`; if none, `#ALL` |
| `#ALL` | All patterns, including those no phase mentions |
| `#DEFAULT` | Same as absent |
| an id | That phase's `active/@pattern` set, in schema pattern order |

Naming a phase that the schema does not define is an error. Abstract patterns
are never active, even if a phase lists one.

## Message instantiation

An assertion's message is built by walking its content with the firing node as
the context node:

- text → itself, with XML whitespace preserved as authored.
- `<value-of select="e"/>` → the XPath string value of `e`. If `e` selects a
  node-set, the string value of the **first node in document order** is used,
  matching XSLT's `xsl:value-of`.
- `<name/>` → the qualified name of the context node.
- `<name path="e"/>` → the qualified name of the first node selected by `e`.
- `<emph>`, `<span>`, `<dir>` → their content, recursively.

The resulting plain-text string is what appears in `svrl:text`.

## Locations

Every reported assertion carries a **location**: an absolute XPath expression
that identifies the node the assertion is about, using positional predicates
so that it is unambiguous.

```
/invoice[1]/lines[1]/line[3]/@qty
```

**The location must be valid XPath 1.0**, because a location that a consumer
cannot evaluate does not do the one job it has. SVRL grew up in the XSLT 1.0
world and its consumers are XPath 1.0 engines, so the `*:local` wildcard is
not available — that is XPath 2.0 syntax, and libxml2 rejects it outright.

A name in no namespace is therefore written plainly, and a namespaced name
with the predicate form, which needs no prefix bound by the consumer:

```
/root[1]/*[local-name()='line' and namespace-uri()='urn:example'][3]/@qty
```

The ISO reference implementation writes namespaced names the same way, for
the same reason.

The node is the `@subject` node when the assertion or its rule names one,
otherwise the context node.

`tests/corpus.rs` checks every recorded location by evaluating
`count(LOCATION) = 1` under the XPath 1.0 binding, so a location that is not
valid 1.0, or that selects the wrong number of nodes, fails the suite.

Building one is linear in the node's depth: the chain is collected in a single
walk upwards and the steps written in one pass. Recursing and formatting the
parent's location at each level is quadratic, which a finding pays once per
location — measurably, on any deeply nested document. The `location_generation`
benchmarks cover the flat, namespaced and deep shapes.

## Matching a rule context

A context is an XSLT match pattern, rewritten to the rooted form described
above: `line` becomes `/descendant-or-self::node()/child::line`.

Evaluating that literally builds a node-set of the whole document and then
filters it — for **every rule**, so a fifty-rule schema builds it fifty times.
For the common shape, a bare name or wildcard with no predicate, the validator
instead walks the tree once and keeps only the nodes that match. The result is
identical by construction: `descendant-or-self::node()` from the root is every
node bar attributes and namespaces, and the children of that set are the same
minus the root.

Anything else — a predicate, a longer path, an explicit axis — takes the
general evaluator. In debug builds both are computed and compared on every
rule context, so the test suite and the generated differential cases check
that the fast path never disagrees.

## Parallel pattern evaluation

Patterns are independent by definition: each gets its own pass over the
document, and no pattern can observe another's results. That makes them
parallelisable without changing a single rule of the semantics above.

It is **opt-in**, and off by default:

```rust
let options = ValidateOptions::new().with_parallel_patterns(true);
```

Off by default because a library that spawns threads on its own is a
surprise. Many callers already parallelise across *documents* — `Schema` is
`Send + Sync`, so that needs nothing from this crate — and nesting a second
layer of threading inside that would oversubscribe the machine. The caller
knows which axis to parallelise; the crate should not guess.

### The report is identical either way

This is the guarantee that makes the option safe to turn on:

> Turning parallel evaluation on never changes the report. Same schema, same
> document, same findings, in the same order.

Three things make that true.

**Order is restored, not preserved.** Each pattern's result is collected into
its own slot and the slots are reassembled in schema order, so the report
reads the same whichever thread finished first.

**Variables are copied, not shared.** Schema-level and phase-level `let`
bindings are evaluated once, before any pattern runs, and each worker starts
from a clone of that scope. Pattern-level and rule-level bindings never
escape the pattern that made them, so there is nothing to synchronise.

**`max_failures` truncates afterwards.** Under sequential evaluation the cap
stops work early. Under parallel evaluation "the first N failures" is not
well-defined while patterns are still running, so every active pattern runs
to completion and the report is truncated to the same N findings the
sequential run would have produced. The cap costs more work in parallel than
it saves; determinism is worth more.

### When it helps, and when it does not

The ceiling is the number of active patterns: a two-pattern schema cannot go
more than twice as fast, and a one-pattern schema not at all. The work per
pattern must also be large enough to outweigh starting a thread, which for a
small document it is not.

It is worth turning on for a schema with many patterns over a large document,
and worth measuring rather than assuming — on the crate's own benchmark, an
eight-pattern schema is **slower** in parallel on a 100-element document and
about four times faster on a 5 000-element one. Figures and the benchmark
that produces them are in [testing/](../testing/index.md#benchmarks).

### Implementation

`std::thread::scope`, so there is no thread-pool dependency and no `'static`
requirement on the borrowed schema and document.

The active patterns are dealt out into at most
`std::thread::available_parallelism()` chunks — one thread per pattern would
be waste for a schema with fifty of them. There is no thread-count option;
if one is ever wanted, it belongs on `ValidateOptions`.

## Flags and roles

`@flag` and `@role` are opaque to the processor. Resolution for a reported
assertion: the assertion's own attribute if present, otherwise the rule's,
otherwise absent. `@flag` conventionally carries severity (`error`, `warning`,
`info`); the crate does not assign it meaning, but the CLI can filter on it
and derives its exit code from it — see [cli/](../cli/index.md).

## Diagnostics

`assert/@diagnostics="d1 d2"` references `diagnostic/@id`. Each referenced
diagnostic is instantiated in the same context as the assertion and attached
to the report. A reference to an undefined diagnostic id is a schema error,
raised at compile time, not at validation time.

## Errors during validation

An XPath expression that fails at runtime — a variable that is out of scope
here, a type error — is a hard error that aborts validation with a message
naming the schema construct that contains the expression. A misspelled
variable and an unknown function never reach this point; both are caught when
the schema loads. It is not silently
treated as false, because a silently-false assertion is a validation that
passes for the wrong reason.
