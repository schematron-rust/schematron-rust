# AGENTS.md

Instructions for AI agents working in this repository. Humans: this is a
useful orientation too, but [`index.md`](index.md) is the friendlier door.

`CLAUDE.md` is a pointer to this file. This file is the single source of
truth for agent instructions; do not duplicate its content elsewhere.

## What this repository is

`schematron` — a **pure Rust** implementation of ISO/IEC 19757-3 Schematron:
rule-based XML validation. No `libxml2`, no XSLT processor, no C toolchain,
no FFI, no `unsafe`. The crate contains its own XML parser, its own XPath 1.0
engine, and its own validator.

The specification lives in [`spec/`](spec/) and is **normative**. If code and
spec disagree, that is a defect in one of them — decide which, fix it, and say
which you changed.

## Before you change anything

Read, in this order:

1. [`spec/index.md`](spec/index.md) — what the project is and is not
2. [`agents/architecture.md`](agents/architecture.md) — the four layers and why they are separate
3. [`agents/invariants.md`](agents/invariants.md) — **the things that must never break**
4. [`agents/conventions.md`](agents/conventions.md) — how code here is written
5. [`agents/testing.md`](agents/testing.md) — how to prove a change works
6. [`agents/tasks.md`](agents/tasks.md) — recipes for the common jobs

## The commands that gate a change

Every one of these must pass before you claim a change is done:

```sh
cargo test --all-features                     # unit, integration, corpus, CLI, docs
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features            # must be warning-free
cargo +1.94 test --all-features               # the MSRV boundary
```

Slower, run when relevant:

```sh
cargo bench                                   # criterion; see agents/testing.md
cargo +nightly fuzz run fuzz_validate -- -max_total_time=60 -report_slow_units=30

# Agreement with the ISO reference implementation, curated and generated.
# Needs xsltproc and the skeleton; see spec/testing.md.
SCHEMATRON_SKELETON=/path/to/skeleton cargo test --test differential -- --ignored
```

If a command fails for a reason unrelated to your change, say so explicitly
rather than working around it silently.

## The one domain rule to internalise

> Within a single Schematron pattern, each node is processed by **at most one**
> rule: the first whose context matches it.

Rules in one pattern compete like the arms of a match expression; rules in
different patterns do not. Almost every confusing Schematron result traces
back to this, or to a missing namespace prefix. See
[`spec/validation.md`](spec/validation.md).

## Non-negotiables

These are in [`agents/invariants.md`](agents/invariants.md) with reasoning.
The short version:

- **No `unsafe`.** The crate is `#![forbid(unsafe_code)]`.
- **No C dependencies, no FFI, no XSLT.** That is the whole point of the crate.
- **No external entity resolution, ever.** XXE must stay structurally
  impossible, not merely disabled.
- **No implicit network access.** The default resolver refuses `http(s)`.
- **An evaluation error is never silently a false assertion.** That would let
  a broken schema report a clean bill of health.
- **A malformed input is an error, never a panic.** Four fuzz targets enforce
  this.
- **Every XPath expression is parsed once, at schema compile time.**

## How to write here

Match the surrounding code. Specifically:

- Comments explain **why**, never what. If a comment restates the code,
  delete it. See [`agents/conventions.md`](agents/conventions.md).
- Every public item has a rustdoc comment; `missing_docs` is a warning and
  `cargo doc` must stay clean.
- Errors name what failed **and where**. A message reading only "invalid
  XPath" costs the reader more time than the crate saved them.
- Prefer a test that fails on the real defect over a test that passes.

## Scope discipline

- The specification is the contract. Changing behaviour means changing
  `spec/` in the same edit, not later.
- Do not add dependencies without a stated reason. The crate currently depends
  on `quick-xml` and `thiserror`, plus `serde`/`clap` behind default features.
- Do not "fix" XPath 1.0's surprising semantics. Existential node-set
  comparison, NaN-yielding conversions, and the no-exponent number format are
  correct and deliberate.
- Do not raise the MSRV outside its policy. See
  [`spec/rust-msrv-n-minus-3.md`](spec/rust-msrv-n-minus-3.md).

## Where facts live

Single source of truth, enforced by `tests/docs.rs` and `tests/cli.rs`:

| Fact | Lives in | Enforced by |
|---|---|---|
| MSRV | `Cargo.toml` | `the_msrv_spec_agrees_with_cargo_toml` |
| CLI flags | `src/main.rs` | `every_cli_flag_is_documented_and_every_documented_flag_exists` |
| Exit codes | `src/main.rs` | `every_documented_exit_code_is_described_consistently` |
| XPath functions | `src/xpath/functions.rs` | `the_xpath_function_list_in_the_spec_matches_the_engine` |
| Schema examples | the `.sch` and Markdown files | `every_documented_schema_compiles` |
| Conformance | `spec/conformance.md` | reviewed by hand |

If you add a fact that appears in two places, add a test tying them together.
