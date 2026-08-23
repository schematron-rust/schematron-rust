# Rust MSRV policy: N-3

The Minimum Supported Rust Version is **current stable minus three**.

If the current stable release is `1.N`, then this crate supports `1.N-3` and
everything newer. The MSRV is declared in `Cargo.toml`:

```toml
[package]
rust-version = "1.94"
```

## Current value

| | |
|---|---|
| Current stable | 1.97 |
| Policy | N-3 |
| **MSRV** | **1.94** |
| Verified | `cargo +1.94 test --all-features`, full suite passing |

## What N-3 means in practice

Rust ships a stable release every six weeks, so three releases is roughly
**eighteen weeks** — a little over four months. A downstream project that
updates its toolchain even twice a year is never blocked by this crate.

Counting is by release number, not by date: when stable becomes 1.98, the MSRV
becomes 1.95, whether or not anything in the crate needed the change.

## Why N-3

It is a deliberate middle position between the two failure modes.

**Pinning an old MSRV forever** is the failure mode that costs the crate.
Every improvement to the standard library — `let`-`else`, `is_some_and`,
`is_none_or`, and their successors — stays out of reach, and the code
accumulates workarounds whose only purpose is to appease a compiler nobody
runs any more. Worse, the claim rots silently: an MSRV nothing tests against
is a guess, and it is usually wrong.

**Tracking stable exactly** is the failure mode that costs the users. It
breaks anyone whose toolchain is a week old, which includes most CI images,
every Linux distribution package, and anyone whose employer gates toolchain
upgrades.

N-3 gives roughly four months of slack — enough for the toolchain-lag cases
above — while keeping the language recent enough that the code can be written
the way the language is meant to be written today.

It also makes the number *decidable*. "What is our MSRV" has one answer that
anyone can compute from the current stable release, rather than being a
judgement call re-argued whenever someone wants a new standard library method.

## Consequences

1. **The MSRV is a supported version, not a promise of stability.** Raising it
   is routine maintenance and does not, on its own, constitute a breaking
   change requiring a major version bump. Callers who need a fixed toolchain
   should pin this crate's version.
2. **The MSRV must always be tested, never asserted.** See below.
3. **Dependencies are chosen to fit.** A dependency whose own MSRV is newer
   than ours cannot be used. This is checked by the same command that checks
   ours, because `cargo` resolves and builds the whole graph.
4. **`clippy::incompatible_msrv` enforces it during development.** Clippy
   reads `rust-version` from `Cargo.toml` and flags any standard library item
   that is newer than it, so using a too-new method fails the lint pass rather
   than surfacing later as a user's build failure.

## Verifying it

An MSRV that is written down but never exercised is a guess. Verify with the
exact toolchain, not with a newer one:

```sh
rustup toolchain install 1.94 --profile minimal
cargo +1.94 test --all-features
```

Testing on a *newer* toolchain proves nothing about the MSRV, because newer
compilers accept strictly more. The check has to run on the boundary version
itself.

Note that `cargo +1.94 clippy` needs the `clippy` component in that toolchain;
the `--profile minimal` install above omits it. Day-to-day linting runs on
current stable, where `clippy::incompatible_msrv` already catches the same
class of problem by reading `rust-version`.

## Updating when stable moves

When Rust `1.98` ships, the MSRV becomes `1.95`. Writing `NEW` for the new
value, so that nothing below can be copied as if it were a live command:

1. Set `rust-version` to `NEW` in `Cargo.toml`.
2. Update the **Current value** table above.
3. `rustup toolchain install NEW --profile minimal`
4. `cargo +NEW test --all-features`
5. If it fails, fix the code — do not lower the MSRV. The policy is the
   policy; a failure here means a dependency or a language feature needs
   attention, which is exactly what the policy is meant to surface.
6. Expect clippy to report **new** lints afterwards. Raising the floor unlocks
   standard-library APIs that `clippy::incompatible_msrv` was suppressing;
   applying them is the policy paying off, not a problem.

The step that matters is 4. Steps 1 and 2 without it produce a number that
looks maintained and is not.

## Relationship to the rest of the specification

This policy governs the toolchain only. It says nothing about the crate's own
public API stability, which is versioned separately, and nothing about which
version of Schematron is implemented — that is
[conformance.md](conformance.md).
