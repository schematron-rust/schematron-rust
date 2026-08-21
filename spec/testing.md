# Testing, benchmarking, and linting

## Layers

| Layer | Location | What it covers |
|---|---|---|
| Unit tests | `#[cfg(test)]` in each module | Lexer, parser, evaluator, model, one function at a time |
| Integration tests | `tests/` | Whole-crate behaviour through the public API |
| Document tests | `tests/documents.rs` | `document()`, cross-document node-sets, and the loading passes |
| Corpus tests | `tests/corpus/` | `.sch` + `.xml` + expected `.svrl` triples |
| Doc tests | rustdoc examples | Every public item's example actually runs |
| CLI tests | `tests/cli.rs` | Arguments, exit codes, output formats |
| Fuzz targets | `fuzz/` | Crash and panic freedom on hostile input |
| Benchmarks | `benches/` | Regression tracking with criterion |

Assertions use the `assertables` crate, matching the house style of the
sibling SixArm crates.

## Corpus tests

The corpus is the real conformance suite. Each case is a directory:

```
tests/corpus/<case>/
  schema.sch      the schema
  input.xml       the instance
  expected.txt    expected findings, one per line: KIND | location | text
  phase           optional: the phase to run
```

The runner discovers cases, runs each, and compares. Adding a case is adding a
directory — no Rust code. The cases, and what each pins down:

| Case | Covers |
|---|---|
| `assert-polarity` | An `assert` fires on **false**, and is silent on true |
| `report-polarity` | A `report` fires on **true** — the opposite polarity |
| `first-rule-wins` | A broad rule claims a node, leaving a later narrow rule dead |
| `rule-alternatives` | The same mechanism used deliberately, as `else` branches |
| `patterns-are-independent` | One node matched separately by two patterns |
| `namespaces` | `ns` prefix bindings against a namespaced document |
| `node-kinds` | Rules matching attributes, text, comments, and PIs |
| `absolute-context` | `/root/a` is not searched across the whole document |
| `union-context` | `a \| b` as a rule context |
| `predicate-context` | `line[1]` means "first of its own parent" |
| `let-scopes` | Schema, pattern, and rule scopes resolving together |
| `let-phase-scope` | The phase scope, shadowing a schema-level binding |
| `let-shadowing` | Innermost binding wins across all scopes |
| `abstract-patterns` | `is-a` and `param`, including `$x` inside a string literal |
| `abstract-rules` | `extends`, transitive, spliced at its own position |
| `phase-default` | `@defaultPhase` applies when no phase is named |
| `phase-selection` | A named phase activating more patterns |
| `phase-all` | `#ALL` overrides the default and runs unlisted patterns |
| `diagnostics` | Multiple diagnostic references from one assertion |
| `message-interpolation` | `value-of`, `name`, and `name/@path` |
| `subject` | `@subject` on both rule and assert moving the location |
| `no-findings` | A clean document produces nothing at all |

Two things are deliberately **not** corpus cases, because the format has no
way to supply auxiliary files:

- `include` and nested includes — `tests/schema.rs`, using `MemoryResolver`.
- `pattern/@documents` — `tests/schema.rs`, using real temporary files.
- XPath `document()` — `tests/documents.rs`, using both.

## Fuzzing

`cargo-fuzz` targets, all built on `libfuzzer-sys`:

| Target | Input | Property |
|---|---|---|
| `fuzz_xml` | arbitrary bytes | The XML parser never panics; it returns `Ok` or `Err`. |
| `fuzz_xpath` | arbitrary UTF-8 | The XPath lexer and parser never panic. |
| `fuzz_schema` | arbitrary UTF-8 | Schema compilation never panics. |
| `fuzz_validate` | structured: schema + document | Full validation never panics and always terminates. |

The invariant under test is the same for all four: **no panic, no hang, no
unbounded memory**. Recursion in the XPath parser and in include resolution is
depth-limited so that deeply nested input returns an error instead of blowing
the stack.

Each target also checks an invariant beyond "did not crash":

- `fuzz_xml` walks the parsed tree and asserts it is self-consistent — every
  child's parent points back, every subtree range contains its children.
- `fuzz_xpath` evaluates whatever parsed, and converts the result to all three
  scalar types, so the conversions are exercised too.
- `fuzz_validate` asserts that SVRL this crate emits is XML this crate can
  read back.

```sh
cargo +nightly fuzz run fuzz_xpath -- -max_total_time=60
```

Corpus seeds are copied from `tests/corpus/`, so the fuzzer starts from valid
input and mutates outward.

**This has already paid for itself.** `fuzz_xpath` found, within seconds, that
`not()` with no arguments panicked: the function library indexed `args[0]` on
the strength of the schema compiler having checked arity, but `evaluate` is
public and that path never ran the check. The fix was to check arity in the
library itself; the regression test is
`xpath::eval::tests::wrong_arity_is_an_error_not_a_panic`. Since then all four
targets have run clean for roughly eight million executions.

## Benchmarks

`criterion` benches in `benches/`:

| File | Measures |
|---|---|
| `bench_xml_parse` | Parsing 10 / 1 000 / 100 000 elements; deep nesting; string values; document order |
| `bench_xpath` | Compiling expressions, evaluating them, and each axis separately |
| `bench_validate` | Schema compilation, end-to-end validation, compile-once-validate-many, fired-rule recording, and report rendering |

```sh
cargo bench
cargo bench -- --save-baseline main
```

Two of these earn their keep specifically:

- **`compile_once_validate_many`** keeps the compiled-schema guarantee honest.
  Validating N documents must not re-parse each XPath expression N times, and
  if that regressed the gap between the two arms of this benchmark would close.
- **`validate` at 1 000 and 10 000 elements** is what caught a real quadratic:
  generating an SVRL location rescanned the parent's child list once per
  finding. Fixing it — by precomputing sibling positions and subtree ranges,
  see [xml.md](xml.md) — made the 10 000-element case fourteen times faster
  and restored linear scaling.

Indicative numbers on an M-series laptop:

| Benchmark | Time |
|---|---|
| `schema_compile` | ~19 µs |
| `validate/1000` | ~0.95 ms |
| `validate/10000` | ~10 ms |
| `report_render_1k/svrl` | ~120 µs |
| `parallel_patterns_100/sequential` | ~560 µs |
| `parallel_patterns_100/parallel` | ~670 µs — *slower*; threads cost more than they save here |
| `parallel_patterns_5000/sequential` | ~28 ms |
| `parallel_patterns_5000/parallel` | ~6.5 ms — about 4× faster |

The parallel pair is worth reading as a warning as much as a result: on a
small document, turning threading on makes validation slower. That is why
`parallel_patterns` is off by default and why the documentation says to
measure rather than assume.

## Lints

`clippy::pedantic` is enabled crate-wide in `Cargo.toml`:

```toml
[lints.clippy]
pedantic = { level = "warn", priority = -1 }
```

Individual pedantic lints are allowed only with a comment explaining why, at
the narrowest scope that works. `#![deny(missing_docs)]` is on: every public
item carries documentation, and CI fails without it.

## Commands

```sh
cargo test                        # unit + integration + corpus + doc tests
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps               # must be warning-free
cargo bench
cargo +nightly fuzz run fuzz_validate
cargo +1.94 test --all-features   # the MSRV boundary; see rust-msrv-n-minus-3.md
```

The last one has to run on the boundary toolchain itself. Testing on a newer
compiler proves nothing about the MSRV, because newer compilers accept
strictly more.
