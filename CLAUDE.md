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
- **The gate** — these four must pass before any change is done:

  ```sh
  cargo test --all-features
  cargo clippy --all-targets --all-features -- -D warnings
  cargo doc --no-deps --all-features
  cargo +1.94 test --all-features
  ```

## Working notes

- **`cargo test --test cli` takes ~40 s** because it spawns the binary per
  test. `cargo test --lib` is the fast loop; run the full suite before
  declaring done.
- **Fuzzing needs nightly**, which is installed:
  `cargo +nightly fuzz run fuzz_xpath -- -max_total_time=60`.
  Budget for it; the default run is unbounded.
- **Benchmarks are slow by default.** For a quick signal use
  `--warm-up-time 1 --measurement-time 2 --sample-size 10`.
- **`cargo clippy --fix` needs `--allow-no-vcs`** here, since the repository
  has no version control initialised. Do not run `git init` to work around
  that — it is not this project's decision to make.
- **The MSRV toolchain `1.94` is installed.** If a bump is needed,
  `rustup toolchain install <version> --profile minimal` first, then actually
  run the tests on it.

## Things that look like bugs and are not

- `a = b` and `a != b` can both be true. XPath 1.0 node-set comparison is
  existential. Correct, deliberate, tested.
- `'x' > 0` is false rather than an error. Relational operators convert to
  number; the string becomes NaN.
- An unprefixed name in a schema matches **no namespace**. XPath 1.0 has no
  default namespace. This is the most common reason a schema appears to do
  nothing.
- A `report` firing does not make a document invalid. It is an observation.
- `missing >= false()` is **true** for an empty node-set. A node-set compared
  to a boolean is converted with `boolean()`, not walked existentially, so it
  is `0 >= 0`. Meanwhile `missing = 'x'` and `missing != 'x'` are both false.
  This was a real bug once; do not fold the boolean case back in with the
  others.
- `sum()` of an empty node-set is **positive** zero, so `1 div sum(none)` is
  Infinity. It is folded from `0.0` rather than written `.sum()`, because
  Rust's `Sum` for `f64` starts from `-0.0`.

Full list with reasoning: [`agents/invariants.md`](agents/invariants.md).

## Before claiming a change is done

Do not report success on a partial run. If a gate fails for a reason unrelated
to the change, say so explicitly rather than describing the work as complete.
New behaviour needs a test that fails without the change — verify that, do not
assume it.
