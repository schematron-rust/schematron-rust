---
name: schematron-skill
description: Understand and use Schematron — the rule-based XML validation language — and its pure-Rust implementation in this repository (the `schematron` crate and CLI). Covers core concepts (pattern, rule, assert/report, phases, keys, abstract patterns), writing a schema, the library API, and the command line tool. Use when the user asks what Schematron is, how it differs from a grammar (DTD/XML Schema/RELAX NG), how to write or debug a schema, how to use the `schematron` crate or CLI, or asks about terms like assert/report/pattern/rule/phase/key/diagnostic.
---

# Schematron, and the `schematron` crate

You're helping someone **use** Schematron — write schemas, understand
results, or call the `schematron` crate/CLI. This is not the maintainer
skill for working on the crate's own source; see
`schematron-rust-maintainer-skill` for that.

## What Schematron is

DTD, XML Schema, and RELAX NG describe the **shape** a document may take.
Schematron describes the **conditions** a document must satisfy, written as
plain XPath expressions — so it expresses what a grammar structurally
cannot:

- co-occurrence rules ("if `@type` is `invoice`, `total` is required")
- value relationships ("`end` must not precede `start`")
- cross-references between distant parts of a document
- cardinality that depends on content, not position

Schematron is normally layered **on top of** a grammar, not used instead of
one. This repository's implementation is pure Rust: no `libxml2`, no XSLT
processor, no C toolchain, no FFI — it contains its own XML parser, its own
XPath engine, and its own validator, and interprets a schema directly rather
than transpiling it to XSLT.

## The shape of a schema

```xml
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="invoice">
      <assert test="total">An invoice must have a total.</assert>
      <report test="count(line) &gt; 100">This invoice has an unusual number of lines.</report>
    </rule>
  </pattern>
</schema>
```

Core vocabulary:

- **`pattern`** groups related rules; every pattern gets its own independent
  pass over the document (patterns never compete with each other).
- **`rule context="…"`** is an XPath expression selecting the nodes the rule
  applies to. Within one pattern, each node is processed by **at most one**
  rule — the first whose context matches. This is the single rule that
  explains almost every "why didn't my rule fire" surprise; rules in one
  pattern compete like the arms of a `match`, rules in different patterns do
  not.
- **`assert test="…"`** fires — reports a problem — when its test is
  **false**. A document breaking a rule.
- **`report test="…"`** fires when its test is **true**. An *observation*,
  not itself a failure — don't confuse the two; only failed `assert`s make a
  document invalid.
- **`let name="…" value="…"`** binds a variable, at schema, phase, pattern,
  or rule scope (innermost wins).
- **`phase`** selects which patterns run for a given validation, so the same
  schema can offer, say, a strict phase and a lenient one.
- **`key`** declares a named index (`<sch:key name="…" path="…" use="…"/>`)
  so a cross-reference check (`key('name', @ref)`) is linear instead of the
  quadratic cost of scanning the whole document per node.
- **Abstract patterns/rules** (`abstract="true"`, `is-a`, `param`) let a
  rule set be written once and instantiated with different parameters.

For the full eighteen-step walkthrough — namespaces, diagnostics, phases,
abstract patterns, dates, `=` vs `eq`, cross-references and keys, using the
library — see `spec/tutorial/index.md` in the crate (or
<https://schematron-rust.github.io/> for the same material as a website).

## XPath support: know which binding you're in

The default query binding is **XPath 1.0**, the standard's original target
and where the whole engine's semantics are proven. Schemas that declare
`queryBinding="xslt2"` or `"xpath2"` get a **subset** of XPath 2.0 layered on
top — sequences, `for`/`some`/`every`, dates/times/durations, value
comparisons (`eq`/`ne`/…), `instance of`/`cast as`/`castable as`, and a
numeric type hierarchy for `instance of` — but XPath 2.0 is a different
language with a different type system, not 1.0 plus extra functions, and the
crate is explicit about exactly where 2.0 still behaves like 1.0 (an
untyped-comparison quirk, the implicit timezone default, and a few others).
Anything genuinely unsupported is a **hard compile-time error naming the
construct**, never a silent wrong answer. See `spec/xpath2/index.md` before
relying on anything beyond the 1.0 core, and read its divergences section —
it's short and it matters.

Two comparison forms to know when writing `xslt2` schemas: `=` is
*existential* over node-sets/sequences ("does *some* pair match?") and
coerces an untyped value to the other operand's type; `eq` compares
*exactly one* value on each side and is stricter (comparing an untyped
attribute to a number with `eq` is a type error — that's `eq` telling you
`=` or `number(@n) eq 1` is what you meant).

## Using it

Library:

```rust
use schematron::{Document, Schema};

let schema = Schema::from_path("rules.sch")?;
let document = Document::from_path("data.xml")?;
let report = schema.validate(&document)?;

if !report.is_valid() {
    for failure in report.failures() {
        println!("{}: {}", failure.location, failure.text);
    }
}
```

A `Report` is data, not formatted text — render it as SVRL
(`report.to_svrl()`), JSON (`report.to_json()`), or human-readable text
(`report.to_text()`). See `spec/api/index.md`.

Command line:

```sh
cargo install schematron
schematron validate --schema rules.sch --document data.xml
```

Phase selection, output format, and flag filtering are all CLI options; see
`spec/cli/index.md` for the full flag reference.

## When a schema seems to do nothing

Reach for `schematron lint` (or `Schema::lint()`) first — it catches the
mistakes the model makes easy without needing a document at all: an
`assert`/`report` mixed up, a context that can never match, a phase that
activates nothing, a key that's declared but never looked up (or looked up
but never declared — the latter is a compile error naming it, not a
silently empty result). See `spec/linting/index.md`. The other common cause
is the "first matching rule wins" rule above: a later rule you expected to
fire never gets the chance because an earlier one in the same pattern
already claimed the node.

## Where the authoritative detail lives

This skill is a map, not the territory — the crate's `spec/` directory is
**normative**; if anything here and `spec/` disagree, trust `spec/`.

| Want to | Read |
|---|---|
| Learn by example, step by step | `spec/tutorial/index.md` |
| Every element and attribute | `spec/data-model/index.md` |
| The validation algorithm, exactly | `spec/validation/index.md` |
| The XPath 1.0 engine | `spec/xpath/index.md` |
| The XPath 2.0 subset and its limits | `spec/xpath2/index.md` |
| SVRL, read and written | `spec/svrl/index.md` |
| Keys and cross-references | `spec/keys/index.md` |
| Why a schema does nothing | `spec/linting/index.md` |
| Library API | `spec/api/index.md` |
| CLI flags and exit codes | `spec/cli/index.md` |
| Errors vs. findings | `spec/errors/index.md` |
| Limits and stated divergences | `spec/conformance/index.md` |

If the question is instead about *changing the crate's own source* — adding
an XPath function, fixing a fuzz crash, the release process — use
`schematron-rust-maintainer-skill`.
