# schematron-rust

Pure Rust ISO/IEC 19757-3 Schematron: rule-based XML validation, with no libxml2, no XSLT processor, no C toolchain, no FFI, and no `unsafe`.

This repository is a monorepo holding the `schematron` crate and its companion website.

## Start here

- [Crate README](schematron/README.md): what Schematron is, install, library and CLI usage, and the feature list
- [Repository map](schematron/index.md): routes to the right document
- [Production readiness assessment](plan.md) and [prioritized tasks](tasks.md)
- [Contributing](CONTRIBUTING.md) and [Code of Conduct](CODE_OF_CONDUCT.md)

## Layout

| Path | What it is |
|---|---|
| [`schematron/`](schematron/) | The `schematron` crate |
| [`schematron-rust.github.io/`](schematron-rust.github.io/) | The public website |
| [`spec/`](spec/) | Repository-level specification: governance, funding, publishing policy |

The crate's own specification, `schematron/spec/`, is normative for its behavior.
