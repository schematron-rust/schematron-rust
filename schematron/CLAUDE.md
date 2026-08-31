# CLAUDE.md

Instructions for Claude Code in this repository.

**The instructions are in [`AGENTS.md`](AGENTS.md). Read that file.**

This file is deliberately a pointer, not a copy. Agent instructions split
across two files drift apart, and the drift is invisible until an agent acts
on the stale half. `AGENTS.md` is the single source of truth; everything below
is Claude-specific mechanics that do not belong there.

## Quick orientation

- **What this is** — a pure Rust ISO/IEC 19757-3 Schematron validator. No C,
  no XSLT, no FFI, no `unsafe`.
- **Where the truth lives** — [`spec/`](spec/) is normative. Code and spec
  disagreeing is a defect in one of them.
- **The gate** — the four commands in [`AGENTS.md`](AGENTS.md#the-commands-that-gate-a-change)
  must pass before any change is done. `cargo test --lib` is the fast loop
  while iterating (see [`agents/testing.md`](agents/testing.md) for the full
  layer-by-layer breakdown, including fuzzing and benchmarks); run the full
  gate before declaring done, not just the fast loop.

## Working notes

Mechanics specific to running this crate from *this* machine/session — not
duplicated from `AGENTS.md` or `agents/testing.md`, which cover what to run
and why:

- **The crate root is `schematron/`, not the repository root.** The repository
  holds the crate and the website as peers, so run `cargo` from here. Anything
  that reads the crate root at compile time — `env!("CARGO_MANIFEST_DIR")`, which
  five of the test files use — bakes in the path it was built with, and cargo
  does not rebuild on a directory move. A `target/` from before the move makes
  those tests look for `spec/` and `examples/` at the old path and fail with
  exit code 3 and empty output. `cargo clean` is the fix, not a source change.
- **`cargo clippy --fix` refuses on a dirty tree**, so commit or stash first
  rather than reaching for `--allow-dirty`; the point of the refusal is that
  the rewrite is otherwise unreviewable.
- **The MSRV toolchain `1.96` is installed.** If a bump is needed,
  `rustup toolchain install <version> --profile minimal` first, then actually
  run the tests on it.

## Things that look like bugs and are not

Do not "fix" a surprising XPath 1.0 semantic without reading
[`agents/invariants.md`](agents/invariants.md) first — existential node-set
comparison, NaN-yielding conversions, the empty-node-set-versus-boolean case,
and `sum()`'s positive zero are all correct, deliberate, and tested. This list
is deliberately not copied here; a second copy is exactly the kind of drift
this file's opening paragraph warns about.

## Before claiming a change is done

Do not report success on a partial run. If a gate fails for a reason unrelated
to the change, say so explicitly rather than describing the work as complete.
New behaviour needs a test that fails without the change — verify that, do not
assume it.
