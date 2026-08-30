---
name: schematron-rust-maintainer-skill
description: Work on the schematron-rust repository itself — the schematron crate (pure Rust ISO/IEC 19757-3 Schematron validator) and its companion website. Covers the spec-first workflow, the four-command gate every change must pass, the invariants that must never break, task recipes for common changes (add an XPath function, add a lint, fix a fuzz crash, bump MSRV, release), and repository/testing conventions. Use when the user asks to implement a feature, fix a bug, add or change XPath/Schematron behavior, add a lint, review an invariant, or otherwise change code, tests, or spec/ in this repository.
---

# Maintaining schematron-rust

You're changing **this repository's own source** — the `schematron` crate,
its website, or the repository-level `spec/`. If the user instead wants help
*using* Schematron or the crate as a dependency, use `schematron-skill`.

## Repository shape

A monorepo of two peers, plus repository-level governance:

| Path | What it is |
|---|---|
| `schematron/` | The crate: pure Rust ISO/IEC 19757-3 Schematron — own XML parser, own XPath engine, own validator. `#![forbid(unsafe_code)]`. No `libxml2`, no XSLT processor, no C toolchain, no FFI. |
| `schematron-rust.github.io/` | The public site, a SvelteKit project deployed to GitHub Pages; see its own `AGENTS.md`. |
| `spec/` (repo root) | Normative for the **repository**: governance, funding, publishing policy — not XPath semantics. |
| `schematron/spec/` | Normative for the **crate**: every behavior. If code and spec disagree, that's a defect in one of them — decide which, fix it, say which you changed. |

Almost all work is in `schematron/`. Its own guide is authoritative and more
specific than anything here or in the root `CONTRIBUTING.md`:
**read `schematron/AGENTS.md` before changing code** (`schematron/CLAUDE.md`
just points to it — don't duplicate it, and don't trust a copy of it that
isn't that file, including this skill's summary below if the two ever
disagree).

## Before changing anything, read (in this order)

1. `schematron/spec/index.md` — what the project is and is not
2. `schematron/agents/architecture.md` — the four layers and why they're separate
3. `schematron/agents/invariants.md` — **the things that must never break**
4. `schematron/agents/conventions.md` — how code here is written
5. `schematron/agents/testing.md` — how to prove a change works
6. `schematron/agents/tasks.md` — recipes for the common jobs

## The gate — every change, no exceptions

Run from `schematron/`:

```sh
cargo test --all-features                     # unit, integration, corpus, CLI, docs
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features            # must be warning-free
cargo +1.96 test --all-features               # the MSRV boundary
```

Do not report a change as done on a partial run. If a gate fails for a
reason unrelated to your change, say so explicitly — don't describe the work
as complete or work around it silently.

Slower, run when relevant: `cargo bench` (criterion), `cargo +nightly fuzz
run <target> -- -max_total_time=60`, and the differential suite against the
ISO reference implementation (needs `xsltproc` and a skeleton; see
`schematron/spec/testing/index.md`).

## Non-negotiables

Full reasoning and tests for each are in `schematron/agents/invariants.md` —
read it, don't rely on this compressed list:

- **No `unsafe`, ever.**
- **No C dependencies, no FFI, no XSLT transpilation.** That's the point of
  the crate.
- **External entities are never resolved.** XXE must stay *structurally*
  impossible, not merely switched off — don't add DTD entity support "for
  convenience."
- **No implicit network access.** The default resolver refuses `http(s)`,
  including through `document()`.
- **An evaluation error is never silently a false assertion.** Downgrading a
  broken schema to "the test was false" can turn a real defect into a clean
  bill of health — the single most dangerous failure mode for a validator.
- **A finding is not an error**, and **a successful `report` is not a
  failure** — keep `assert`/`report` and `Err`/`Report` semantics distinct.
- **First matching rule wins**, within a pattern; patterns never compete.
- **Malformed input is an error, never a panic.** Depth is bounded
  everywhere (XPath parser, XML parser, includes, `extends` chains); four
  fuzz targets enforce this.
- **`castable as` reports, `cast as` raises** — same check, different
  failure behavior, deliberately.
- **Don't "fix" XPath 1.0's surprising-but-correct semantics** — existential
  node-set comparison, NaN-yielding conversions, the empty-node-set-vs-
  boolean case, `sum()`'s positive zero, no-exponent number formatting.
  Each is deliberate, tested, and matches every other conformant processor;
  "simplifying" one makes the engine wrong, not cleaner.

## Task recipes

`schematron/agents/tasks.md` has step-by-step recipes for: adding a
Schematron semantics test (almost always a corpus case, not Rust — see
below), adding an XPath function, adding a CLI flag, adding a Schematron
element/attribute, changing validation behavior, fixing a fuzz crash, adding
a lint, investigating "this schema does nothing," bumping the MSRV, adding a
dependency, and the release checklist. Use them rather than improvising the
shape of a change.

## Testing shape

`schematron/agents/testing.md` covers the layers and which to reach for.
The one most contributors miss: a new Schematron **semantics** test is
almost always a corpus case under `schematron/tests/corpus/` — one
directory, three files (schema, document, expected report), no Rust — not a
new `#[test]` function. Reach for Rust tests for engine internals (XPath
evaluation, parsing, lexing) instead.

## Where facts live (don't duplicate a fact across two places)

Enforced by `tests/docs.rs` and `tests/cli.rs` — if you add a fact that
exists in two places, add a test tying them together, the way these already
are:

| Fact | Lives in | Enforced by |
|---|---|---|
| MSRV | `Cargo.toml` | `the_msrv_spec_agrees_with_cargo_toml` |
| CLI flags | `src/main.rs` | `every_cli_flag_is_documented_and_every_documented_flag_exists` |
| Exit codes | `src/main.rs` | `every_documented_exit_code_is_described_consistently` |
| XPath functions | `src/xpath/functions.rs` | `the_xpath_function_list_in_the_spec_matches_the_engine` |
| Schema examples | the `.sch`/Markdown files | `every_documented_schema_compiles` |
| Conformance | `schematron/spec/conformance/` | reviewed by hand |

## Scope discipline

- Changing behavior means changing the relevant `schematron/spec/` document
  in the **same commit** as the code — not a follow-up.
- Don't add a dependency without a stated reason (today: `quick-xml`,
  `thiserror`, `regex`, plus `serde`/`serde_json`/`clap` behind default
  features).
- Don't raise the MSRV outside its policy —
  `schematron/spec/rust-msrv-n-minus-2/index.md`.
- Comments explain **why**, never what; a comment restating the code gets
  deleted, not kept. Every public item gets a rustdoc comment (`missing_docs`
  is a warning; `cargo doc` must stay clean).
- When a change is risky or touches an engine invariant (numeric
  representation, the fast-path/evaluator agreement in `matched_nodes`, rule
  claim storage), prefer the narrowest change that closes the actual gap
  over a more "complete" one that puts a proven invariant at risk — see how
  `schematron/spec/roadmap/index.md`'s "Next" section reasons about
  trade-offs it has deliberately not taken.

## Style precedent worth copying

The commit that lands a feature typically: implements it across
`ast.rs`/`lexer.rs`/`parser.rs`/`eval.rs` (or the relevant layer), adds
integration tests in the matching `tests/*.rs` file, updates the
`schematron/spec/` document(s) for the behavior, adds a `CHANGELOG.md` entry
naming what changed for someone who already depends on the crate, and states
its SemVer reasoning explicitly (in 0.x, a breaking change is a **minor**
bump) plus a verification line naming what was run. Look at a recent feature
commit's message before writing your own.
