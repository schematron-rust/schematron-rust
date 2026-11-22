# Repository specification

This `spec/` directory specifies how the **repository** is run: governance,
community and process concerns that apply to the monorepo as a whole.

It is a separate tree from
[`schematron/spec/`](../schematron/spec/index.md), which specifies the
`schematron` crate itself — the XML parser, the XPath engine, and the
Schematron validator. A change to how XPath comparisons work belongs there.
A change to how the project accepts money belongs here.

The [production readiness assessment](../plan.md) and [verification backlog](../tasks.md)
track cross-platform checks and remaining operational work.

## Status

| Document | Status |
|---|---|
| [Free and open source funding](free-open-source-funding/index.md) | In progress — GitHub Sponsors live, Open Collective pending |
| [Trusted publishing](trusted-publishing/index.md) | Planned — blocked on Codeberg/Forgejo support |
| [Release process](release-process/index.md) | Implemented — who decides a release is ready and executes it, and the bounds on that |
| [Dependabot](dependabot/index.md) | Implemented |
| [Licenses](licenses/index.md) | Implemented — any one of seven licenses, at the recipient's option |
| [Rust MSRV policy](rust-msrv-n-minus-2/index.md) | Implemented — 1.98 |
| [Rust checked arithmetic](rust-checked-arithmetic/index.md) | Implemented — banned patterns only; deny-level lints not yet enabled |
| [Agent skills](agent-skills/index.md) | Implemented — `schematron-skill`, `schematron-rust-maintainer-skill` |
| [llms.json and llms.txt](llms-json-and-llms-txt/index.md) | Implemented — at the repository root and the website root |
| [Node current version](node-current-version/index.md) | Implemented — 26, in `schematron-rust.github.io/` |
| [Monorepo GitHub Pages](monorepo-github-pages/index.md) | Implemented — `make publish` (root `Makefile`), sibling repo `schematron-rust/schematron-rust.github.io` |

Each document lives in its own directory as `index.md`, and every directory
under `spec/` must be linked from this file — the same convention
`schematron/spec/index.md` follows. The crate's documentation audit enforces
both trees: `every_spec_document_is_linked_from_the_index` covers the crate
and `every_repository_spec_document_is_linked_from_the_index` covers this
repository index whenever the crate is tested from the monorepo checkout.
