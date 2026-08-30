# Rust MSRV — current N-2

This repository's **Minimum Supported Rust Version (MSRV)** is **the current
stable Rust release minus two**: if the current stable release is `1.N`, the
MSRV is `1.(N-2)`.

This is a project policy that governs the Rust toolchain the code in this
repository may assume.

## The rule

- Let `1.N` be the latest stable Rust release published by the Rust project.
- The MSRV MUST be `1.(N-2)`.
- Code, tests, benchmarks, fuzz targets, and examples MUST compile with the
  MSRV toolchain. A language or standard-library feature stabilized after the
  MSRV MUST NOT be used.
- Only the minor version is pinned. Any patch release of the MSRV minor
  version (`1.(N-2).x`) is acceptable, and the recorded value names the minor
  version only (`"1.96"`, not `"1.96.0"`), matching how `cargo` itself treats
  `rust-version`.
- Pre-release channels (beta, nightly) are never the MSRV and MUST NOT be
  required by any target this policy covers.

## Where the MSRV is recorded

This repository has one Rust crate, `schematron/`, and no root `Cargo.toml`
or Cargo workspace — there is nothing for a workspace-level setting to be
inherited from.

| Location                     | Form                                    |
| ----------------------------- | --------------------------------------- |
| `schematron/Cargo.toml`       | `[package] rust-version = "1.(N-2)"`    |
| `.github/workflows/ci.yml`    | an `msrv` job that reads the pin above  |

`schematron/Cargo.toml`'s `rust-version` is the single source of truth:
`cargo` refuses to build the crate with an older toolchain, and downstream
consumers see the value in the published crate metadata. The CI `msrv` job
does not hold a second, hand-maintained copy of the version — it reads
`rust-version` straight out of `Cargo.toml` at run time (see
`.github/workflows/ci.yml`'s `Read the declared MSRV` step), so there is only
ever one place to update.

`schematron/fuzz/` is a second, independent `Cargo.toml` — not a workspace
member (there is no workspace), and not referenced as a path dependency from
`schematron`'s own manifest. It is nightly-only (`cargo-fuzz` requires
nightly) and is therefore outside this policy entirely: nothing here asks the
fuzz crate to build under the MSRV toolchain.

## Maintenance

When a new stable Rust release `1.N` appears, the MSRV becomes `1.(N-2)`
**in the same change** that observes the release:

1. Set `rust-version` in `schematron/Cargo.toml` to `1.(N-2)`.
2. Run `cargo +1.(N-2) check --all-targets --all-features` from `schematron/`
   and fix anything the older toolchain rejects — the MSRV is a floor the
   code must meet, not a ceiling on what the code may need.
3. Nothing needs updating in CI: the `msrv` job reads the value set in step 1.

Raising the MSRV is therefore routine and expected, not a breaking change to
be avoided. Lowering it below N-2 to support an older consumer would be a
deliberate exception to this policy, not a convenience, and should be
justified in the commit that does it.

## CI enforcement

CI MUST verify the MSRV, not merely declare it. The `msrv` job installs the
exact toolchain named by `schematron/Cargo.toml`'s `rust-version` and runs
`cargo test --all-features` with it (`cargo check` is enough to prove the
MSRV question — "does this compile" — but this job runs the full test suite
so a toolchain-specific behavior difference cannot slip through either).

The `msrv` job is separate from the `crate` job (which tests, lints, and
builds docs on current stable) so a failure names the cause directly: `crate`
red means a regression on the toolchain the code is actually developed
against, `msrv` red means the code started requiring a newer toolchain than
the policy allows.

## Current value

As of the most recent update to this document, stable Rust is **1.98**, so the
MSRV is **1.96** — matching `schematron/Cargo.toml`. If stable has moved on
since, this document is stale in its example only — the rule above is what
binds, and `schematron/Cargo.toml` must be brought back in line with it.
