# Repository specification

This `spec/` directory specifies how the **repository** is run: governance,
community and process concerns that apply to the monorepo as a whole.

It is a separate tree from
[`schematron/spec/`](../schematron/spec/index.md), which specifies the
`schematron` crate itself — the XML parser, the XPath engine, and the
Schematron validator. A change to how XPath comparisons work belongs there.
A change to how the project accepts money belongs here.

## Status

| Document | Status |
|---|---|
| [Free and open source funding](free-open-source-funding/index.md) | In progress — GitHub Sponsors live, Open Collective pending |
| [Trusted publishing](trusted-publishing/index.md) | Planned — blocked on Codeberg/Forgejo support |
| [Dependabot](dependabot/index.md) | Implemented |
| [Rust MSRV policy](rust-msrv-n-minus-2/index.md) | Implemented — 1.96 |

Each document lives in its own directory as `index.md`, and every directory
under `spec/` must be linked from this file — the same convention
`schematron/spec/index.md` follows, enforced there by
`every_spec_document_is_linked_from_the_index`.
