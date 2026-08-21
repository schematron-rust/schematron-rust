# schematron

**Schematron in pure Rust.** Rule-based XML validation to ISO/IEC 19757-3,
with no `libxml2`, no XSLT processor, no C toolchain, and no FFI.

[![crates.io](https://img.shields.io/crates/v/schematron.svg)](https://crates.io/crates/schematron)
[![docs.rs](https://docs.rs/schematron/badge.svg)](https://docs.rs/schematron)
[![license](https://img.shields.io/crates/l/schematron.svg)](#license)

Every other route to Schematron in Rust goes through C: bind to `libxml2`, or
shell out to Saxon, or compile the schema into XSLT and find an XSLT engine to
run it. This crate does none of that. It contains its own XML parser, its own
XPath 1.0 engine, and its own validator, and it *interprets* a schema directly
rather than transpiling it.

---

## What Schematron is for

DTD, XML Schema, and RELAX NG describe the **shape** a document may take.
Schematron describes the **conditions** a document must satisfy, written as
XPath expressions — so it can express what a grammar cannot:

- co-occurrence rules — "if `@type` is `invoice`, then `total` is required"
- value relationships — "`end` must not precede `start`"
- cross-references between distant parts of a document
- cardinality that depends on content rather than position

Schematron is normally layered *on top of* a grammar, not used instead of one.

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

Read that as: *for every `invoice` element, assert that it has a `total`
child.* `assert` fires when its test is **false**; `report` fires when its
test is **true**.

---

## Install

```sh
cargo add schematron
```

Or the command line tool:

```sh
cargo install schematron
```

---

## Library

```rust
use schematron::{Document, Schema};

fn main() -> schematron::Result<()> {
    // Compiling is the expensive step: it resolves includes, expands
    // abstractions, and parses every XPath expression in the schema.
    let schema = Schema::from_path("rules.sch")?;
    let document = Document::from_path("data.xml")?;
    let report = schema.validate(&document)?;

    if report.is_valid() {
        println!("valid");
    } else {
        for failure in report.failures() {
            println!("{}: {}", failure.location, failure.text);
        }
    }
    Ok(())
}
```

A report is **data**, not formatted text, so one run renders three ways:

```rust
let svrl = report.to_svrl();   // SVRL, for other Schematron tooling
let json = report.to_json()?;  // JSON, keeping the tree structure
let text = report.to_text();   // for a person
```

It can also be queried directly, instead of scraped back out of text:

```rust
report.is_valid();                  // no assert failed
report.count_failures();            // how many did
report.with_flag("error").count();  // findings the schema flagged as errors
report.count_fired_rules();         // zero here means NO context matched
```

Patterns are independent, so a schema with several of them can evaluate them
on separate threads. The report is identical either way — this is a
performance switch, not a behaviour switch:

```rust
let options = ValidateOptions::new().with_parallel_patterns(true);
let report = schema.validate_with(&document, &options)?;
```

Measure before turning it on: on a small document the threads cost more than
they save. See [spec/validation.md](spec/validation.md).

`Schema` is immutable and `Send + Sync`. Compile once, validate in parallel:

```rust
use std::sync::Arc;

let schema = Arc::new(Schema::from_path("rules.sch")?);

for path in paths {
    let schema = Arc::clone(&schema);
    std::thread::spawn(move || {
        let document = Document::from_path(path)?;
        schema.validate(&document)
    });
}
```

---

## Command line

```sh
schematron --schema rules.sch data.xml
schematron -s rules.sch -p strict -f svrl -o report.svrl data.xml
schematron -s rules.sch --flag error docs/*.xml
cat data.xml | schematron -s rules.sch -
```

| Exit code | Meaning |
|---|---|
| 0 | every document valid |
| 1 | at least one failed assertion |
| 2 | usage error |
| 3 | schema error |
| 4 | document error |

Two flags worth knowing when a schema is misbehaving:

```sh
schematron -s rules.sch --explain          # what the compiled schema will do
schematron -s rules.sch --verbose data.xml # which rules actually fired
```

Full option list: [spec/cli.md](spec/cli.md).

---

## The one rule to internalise

> Within a single pattern, each node is processed by **at most one** rule: the
> first whose context matches it.

Rules in one pattern compete like the arms of a match expression. This is a
feature — it is how you write "otherwise" branches:

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

— and it is also the most common way to write a schema that silently does
nothing:

```xml
<pattern>
  <rule context="*">…</rule>
  <rule context="invoice">…</rule>  <!-- never runs: * claimed everything -->
</pattern>
```

To apply independent checks to the same node, put them in **separate
patterns**. Patterns do not compete; each gets its own pass over the document.

The second-most common cause of a schema that does nothing is a missing
namespace prefix. XPath 1.0 has no default namespace, so an unprefixed name
matches elements in *no* namespace. Declare a prefix with `<ns>` and use it
everywhere. `--verbose` shows an empty list of fired rules when this is what
has happened.

---

## What is implemented

A summary. [spec/conformance.md](spec/conformance.md) is authoritative, and
states the limits and deliberate divergences in full.

| Area | Status |
|---|---|
| `schema`, `pattern`, `rule`, `assert`, `report` | Full |
| `ns`, `let`, `phase`, `active`, `include`, `extends` | Full |
| Abstract patterns (`abstract`, `is-a`, `param`) | Full |
| Abstract rules (`rule/@abstract` + `extends`) | Full |
| `diagnostics`, `properties`, `value-of`, `name`, `emph`, `span`, `dir` | Full |
| `@flag`, `@role`, `@subject`, `@see`, `@icon`, `@fpi` | Full |
| `pattern/@documents` | Full |
| Phases, `#ALL`, `#DEFAULT`, `@defaultPhase` | Full |
| SVRL output | Full |
| XPath 1.0 — 13 axes, 27 core functions, exact conversion semantics | Full |
| XPath `document()`, with cross-document node-sets | Full |
| XPath 2.0 sequences, `for`, `some`, `every`, ranges, regular expressions | Subset — see [spec/xpath2.md](spec/xpath2.md) |
| `queryBinding="xslt"`, `"xpath"`, or absent | Supported |
| `queryBinding="xslt2"`, `"xpath2"` | Partly — see [spec/xpath2.md](spec/xpath2.md) |
| `queryBinding="xslt3"` and later | Refused by default |
| XSLT `key()`, `extends href`, `document(uri, base)` | Not implemented |

Beyond the standard, the crate lints a schema for constructs that are legal
but almost certainly wrong — a rule shadowed by an earlier one in the same
pattern, an unprefixed name in a namespaced schema — which are the two ways a
Schematron schema silently does nothing:

```sh
schematron --schema rules.sch --lint
```

See [spec/linting.md](spec/linting.md).

XPath 2.0 is a different language, not XPath 1.0 with extra functions. The
crate implements a documented subset of it — regular expressions,
conditionals, and the string and numeric functions schemas actually use — and
makes everything outside that subset a **hard error naming the construct**,
never a wrong answer. [spec/xpath2.md](spec/xpath2.md) is explicit about what
is in, what is out, and the handful of places where a `xslt2` schema still
evaluates with XPath 1.0 semantics.

---

## Security

The XML parser never resolves an external entity and never processes a DTD's
entity declarations. XXE is therefore **structurally impossible** here rather
than merely switched off; a reference to a DTD-declared entity is an error.

The default resolver reads local files and refuses `http:` and `https:` URIs.
Network access is something an application opts into by supplying its own
`Resolver`, never something the library does behind your back.

Parse depth, include depth, and expression nesting are all bounded, and
exceeding a bound returns an error rather than exhausting the stack. Four
`cargo-fuzz` targets exist to keep that true.

---

## Performance

Every XPath expression is parsed **once**, when the schema is compiled, and
the compiled schema is reused across documents and across threads. Rule
contexts are evaluated once per document rather than tested node by node, so
matching is linear rather than quadratic in document size. Patterns can
optionally evaluate in parallel.

Indicative figures, and the benchmarks that produce them, are in
[spec/testing.md](spec/testing.md#benchmarks). Run them yourself with
`cargo bench` — the numbers in any README are somebody else's hardware.

---

## Documentation

[`index.md`](index.md) is the full map. [`AGENTS.md`](AGENTS.md) is the
contributor and agent guide.

The specification is in [`spec/`](spec/), is **normative**, and is written to
be read:

| Document | What it covers |
|---|---|
| [spec/index.md](spec/index.md) | Overview and design principles |
| [spec/tutorial.md](spec/tutorial.md) | **Start here** — fourteen steps from first rule to real schema |
| [spec/data-model.md](spec/data-model.md) | Every Schematron element and its Rust type |
| [spec/validation.md](spec/validation.md) | The validation algorithm, exactly |
| [spec/xpath.md](spec/xpath.md) | The XPath 1.0 engine |
| [spec/xpath2.md](spec/xpath2.md) | The XPath 2.0 subset, and its limits |
| [spec/xml.md](spec/xml.md) | The XML parser and data model |
| [spec/parsing.md](spec/parsing.md) | The five compilation passes |
| [spec/svrl.md](spec/svrl.md) | The SVRL report format |
| [spec/linting.md](spec/linting.md) | Catching schemas that silently do nothing |
| [spec/api.md](spec/api.md) | Library API |
| [spec/cli.md](spec/cli.md) | Command line interface |
| [spec/errors.md](spec/errors.md) | Error taxonomy |
| [spec/conformance.md](spec/conformance.md) | Limits and divergences, stated up front |
| [spec/testing.md](spec/testing.md) | Tests, fuzzing, benchmarks, lints |
| [spec/rust-msrv-n-minus-3.md](spec/rust-msrv-n-minus-3.md) | MSRV policy: current stable minus three |
| [spec/roadmap.md](spec/roadmap.md) | What is next |

If the code and the specification disagree, that is a defect in one of them.

Runnable examples:

```sh
cargo run --example validate_file        # the shortest useful program
cargo run --example report_formats       # SVRL, JSON, and text from one run
cargo run --example embedded_schema      # includes served from memory
cargo run --example parallel_validation  # one schema, eight threads
cargo run --example xpath_engine         # the XPath engine on its own
```

---

## Development

```sh
cargo test --all-features                                   # everything
cargo clippy --all-targets --all-features -- -D warnings    # pedantic, and clean
cargo doc --no-deps --all-features                          # warning-free
cargo +1.94 test --all-features                             # the MSRV boundary
```

Slower, run when relevant: `cargo bench`, and
`cargo +nightly fuzz run fuzz_validate` (needs `cargo-fuzz`).

The conformance suite lives in `tests/corpus/`. Each case is a directory
holding `schema.sch`, `input.xml`, and `expected.txt`; adding a case means
adding a directory, with no Rust to change.

The documentation is machine-checked: `tests/docs.rs` compiles every schema
shown in the docs, resolves every relative link, and ties duplicated facts —
the MSRV, the CLI flags, the XPath function list — back to their single
source. See [agents/testing.md](agents/testing.md).

---

## License

Licensed under any of:

- MIT License
- Apache License, Version 2.0
- GNU General Public License version 2 only
- GNU General Public License version 3 only

at your option.
