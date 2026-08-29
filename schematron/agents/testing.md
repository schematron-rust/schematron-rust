# Testing

How to prove a change works. The reference for what exists is
[`spec/testing/`](../spec/testing/index.md); this file is about *how to work*.

## The layers, and which one to reach for

| Layer | Location | Reach for it when |
|---|---|---|
| Unit | `#[cfg(test)]` in each module | One function's behaviour, especially an edge case |
| Integration | `tests/validation.rs`, `tests/schema.rs` | Behaviour through the public API |
| Corpus | `tests/corpus/<case>/` | **Any Schematron semantics** — prefer this |
| CLI | `tests/cli.rs` | Arguments, exit codes, output selection |
| Docs | `tests/docs.rs` | A documented claim that could rot |
| Doc examples | rustdoc | Anything a caller reaches for |
| Fuzz | `fuzz/fuzz_targets/` | Panic freedom on hostile input |
| Bench | `benches/` | Anything with a complexity claim |

**Prefer a corpus case** for validation semantics. It is a directory, not
Rust, so it stays readable, and it doubles as documentation of what the
implementation is supposed to do.

## Adding a corpus case

```
tests/corpus/<name>/
  schema.sch      the schema
  input.xml       the document
  expected.txt    expected findings, one per line
  phase           optional: the phase to run
```

`expected.txt` lines are `KIND | location | text`, where `KIND` is `assert`
for a failed assertion or `report` for a successful report. Whitespace in
`text` is normalised, so wrap freely. Blank lines and `#` comments are
ignored — **use them**, and explain what the case is demonstrating:

```
# The broad rule claims the node, so the narrower rule after it is dead code.
assert | /a[1] | claimed by the broad rule
```

No Rust changes are needed. `tests/corpus.rs` discovers directories, and a
second test checks each case is complete.

An empty `expected.txt` (comments only) asserts that a document is clean —
`tests/corpus/no-findings/` does this.

## Running things

```sh
cargo test                              # everything
cargo test --lib                        # unit tests only, fastest loop
cargo test --test corpus                # the conformance suite
cargo test --test docs                  # the documentation audit
cargo test --lib xpath                  # one module
```

`tests/cli.rs` spawns the built binary, so it is the slow one (~40 s). It is
worth its cost — exit codes are what a build pipeline depends on — but do not
put logic tests there that belong in the library.

## Benchmarks

```sh
cargo bench
cargo bench -- --save-baseline main     # then compare after a change
cargo bench --bench bench_validate -- validate
```

For a fast signal while iterating:

```sh
cargo bench --bench bench_validate -- --warm-up-time 1 --measurement-time 2 --sample-size 10
```

**Benchmarks here have caught real defects, not just tracked numbers.** The
`validate` group at 1 000 and 10 000 elements exposed a quadratic in SVRL
location generation; the fix was a 14× speedup. If you change anything that
runs per node, check that group's scaling ratio.

## Differential testing

`tests/differential.rs` runs every corpus schema through the ISO Schematron
reference implementation and compares the findings. It is `#[ignore]`d
because it needs `xsltproc` and third-party stylesheets:

```sh
sh tests/differential/fetch-skeleton.sh /tmp/skeleton
SCHEMATRON_SKELETON=/tmp/skeleton cargo test --test differential -- --ignored
```

Run it when changing validation semantics. If a case starts differing, decide
which implementation is right *before* adding it to `KNOWN_DIVERGENCES` — the
list is for differences that have been understood, not for silencing the test.
Both lists are checked in both directions, so an entry that no longer
describes reality fails just as loudly.

## Fuzzing

Requires nightly and `cargo-fuzz`.

```sh
cargo +nightly fuzz build
cargo +nightly fuzz run fuzz_xpath -- -max_total_time=60 -rss_limit_mb=4096
```

Four targets: `fuzz_xml`, `fuzz_xpath`, `fuzz_schema`, `fuzz_validate`. Each
asserts more than "did not crash" — tree self-consistency, total conversions,
SVRL reparsing. Seeds are copied from `tests/corpus/`.

A crash writes `fuzz/artifacts/<target>/crash-<hash>`. Reproduce with:

```sh
cargo +nightly fuzz run fuzz_xpath fuzz/artifacts/fuzz_xpath/crash-<hash>
```

`fuzz_xpath` has already found a genuine panic this way. When one fires, fix
the defect **and** add a unit test naming the fuzz target that found it, so
the regression is covered by `cargo test` and not only by a nightly run.

## The MSRV boundary

```sh
cargo +1.96 test --all-features
```

Must run on the boundary toolchain itself. A newer compiler accepts strictly
more and proves nothing. See
[`spec/rust-msrv-n-minus-2/`](../spec/rust-msrv-n-minus-2/index.md).

## What "done" means

All four gates in [`AGENTS.md`](../AGENTS.md) pass, plus:

- New behaviour has a test that **fails without the change**. Verify that by
  reverting the change, or by breaking the value the test asserts.
- New public items have rustdoc with an example.
- Any documented claim you added is machine-checked if it can be.
- `spec/` reflects the change, in the same edit.
