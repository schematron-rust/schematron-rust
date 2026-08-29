# schematron

Pure Rust ISO/IEC 19757-3 Schematron: rule-based XML validation, with no
`libxml2`, no XSLT processor, no C toolchain, no FFI, and no `unsafe`.

This page is the map. It routes you to the right document rather than
repeating what they say.

## Start here

| You want to | Go to |
|---|---|
| Understand what Schematron is and why | [README.md](README.md) |
| Read all of this as a website | <https://schematron-rust.github.io/> |
| Learn to write schemas | [spec/tutorial/](spec/tutorial/index.md) |
| Use the library | [spec/api/](spec/api/index.md) |
| Use the command line tool | [spec/cli/](spec/cli/index.md) |
| Work out why a schema does nothing | [spec/linting/](spec/linting/index.md) |
| Check a schema behaves the same elsewhere | [spec/linting/](spec/linting/index.md#portability) |
| See what changed between releases | [CHANGELOG.md](CHANGELOG.md) |
| Decide whether to depend on this | [spec/conformance/](spec/conformance/index.md) |
| Work on the crate | [AGENTS.md](AGENTS.md) |

## The specification

[`spec/`](spec/) is **normative**. If the code and the specification disagree,
that is a defect in one of them.

| Document | Covers |
|---|---|
| [spec/index.md](spec/index.md) | Overview, design principles, reading order |
| [spec/tutorial/](spec/tutorial/index.md) | Eighteen steps from one rule to a real schema |
| [spec/data-model/](spec/data-model/index.md) | Every Schematron element and its Rust type |
| [spec/validation/](spec/validation/index.md) | The validation algorithm, exactly |
| [spec/xpath/](spec/xpath/index.md) | The XPath 1.0 engine |
| [spec/xpath2/](spec/xpath2/index.md) | The XPath 2.0 subset, and its limits |
| [spec/xml/](spec/xml/index.md) | The XML parser and data model |
| [spec/parsing/](spec/parsing/index.md) | The five schema compilation passes |
| [spec/svrl/](spec/svrl/index.md) | The SVRL report format, read and written |
| [spec/keys/](spec/keys/index.md) | Keys, and why a cross-reference check needs one |
| [spec/linting/](spec/linting/index.md) | Catching schemas that silently do nothing |
| [spec/api/](spec/api/index.md) | Library API |
| [spec/cli/](spec/cli/index.md) | Command line interface |
| [spec/errors/](spec/errors/index.md) | Error taxonomy, and error versus finding |
| [spec/conformance/](spec/conformance/index.md) | Limits and divergences, stated up front |
| [spec/testing/](spec/testing/index.md) | Tests, fuzzing, benchmarks, lints |
| [spec/rust-msrv-n-minus-2/](spec/rust-msrv-n-minus-2/index.md) | MSRV policy: current stable minus two |
| [spec/roadmap/](spec/roadmap/index.md) | What is shipped, what is next, what is not planned |

## Contributor and agent documentation

| Document | Covers |
|---|---|
| [AGENTS.md](AGENTS.md) | Entry point: gates, non-negotiables, where facts live |
| [agents/architecture.md](agents/architecture.md) | The four layers and why they are separate |
| [agents/invariants.md](agents/invariants.md) | What must never break, and the test that catches it |
| [agents/conventions.md](agents/conventions.md) | Code and prose style |
| [agents/testing.md](agents/testing.md) | How to prove a change works |
| [agents/tasks.md](agents/tasks.md) | Recipes for common jobs |
| [CLAUDE.md](CLAUDE.md) | Claude Code mechanics; points at AGENTS.md |

## Runnable code

```sh
cargo run --example validate_file        # the shortest useful program
cargo run --example report_formats       # SVRL, JSON, and text from one run
cargo run --example embedded_schema      # includes served from memory
cargo run --example parallel_validation  # one schema, eight threads
cargo run --example xpath_engine         # the XPath engine on its own
```

`examples/invoice.sch` with `invoice-good.xml` and `invoice-bad.xml` is the
worked schema used throughout the tutorial and the CLI tests.

## Source map

| Path | Contents |
|---|---|
| `src/xml/` | XML parser and XPath 1.0 data model |
| `src/xpath/` | XPath 1.0 lexer, parser, evaluator, function library |
| `src/schema/` | Schematron model and its five-pass compiler |
| `src/validate/` | The validator and its report types |
| `src/svrl.rs`, `src/text.rs` | Report renderers |
| `src/main.rs` | The CLI, a thin shell over the library |
| `tests/corpus/` | The conformance suite: one directory per case |
| `fuzz/fuzz_targets/` | Four `cargo-fuzz` targets |
| `benches/` | Criterion benchmarks |

## Verifying the build

```sh
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
cargo +1.96 test --all-features
```
