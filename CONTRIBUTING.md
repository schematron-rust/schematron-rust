# Contributing

Thanks for considering it. This repository is a monorepo holding two things:

| Path | What it is |
|---|---|
| [`schematron/`](schematron/) | The `schematron` crate: a pure Rust ISO/IEC 19757-3 Schematron validator |
| [`schematron-rust.github.io/`](schematron-rust.github.io/) | The public site, a SvelteKit project deployed to GitHub Pages |

Most contributions are to the crate. Its own guide is more specific than
anything below and takes precedence for crate work:
[`schematron/AGENTS.md`](schematron/AGENTS.md) (`schematron/CLAUDE.md` points
to the same file). Read it before changing code — it covers the
specification-first workflow, the four commands that gate every change, and
the invariants that must never break.

## Where the truth lives

[`schematron/spec/`](schematron/spec/index.md) is **normative** for the
crate's behaviour. If code and spec disagree, that is a defect in one of
them — fix it and say which side you changed.

[`spec/`](spec/index.md), this file's sibling, is normative for the
*repository*: governance and process, not XPath semantics.

## Quick start

```sh
cd schematron
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
cargo +1.96 test --all-features   # the MSRV boundary
```

All four must pass before a change is done. See
[`schematron/AGENTS.md`](schematron/AGENTS.md) for the full gate, and
[`schematron-rust.github.io/README.md`](schematron-rust.github.io/README.md)
for the site's own `pnpm` commands.

## Making a change

1. Open an issue or a pull request describing what you want to change and
   why — for anything beyond a small fix, an issue first saves a rewritten
   PR later.
2. If the change affects behaviour, update the relevant `spec/` document in
   the same commit as the code. A behaviour change without a spec change is
   incomplete.
3. Run the gate above.
4. Open the pull request. CI runs the same checks; a red check is something
   to fix, not to explain away.

## Using AI tools

You **may** use AI tools to write a contribution. Say so in the pull
request description: which tool, and what it did.

**This project discloses tool participation in commit trailers**, not only
in the PR description — `Co-Authored-By: <tool> <email>` on any commit an
agentic tool touched. That names the tool as a co-author *in the trailer*;
it does not change who git records as the commit's `Author:`, which is
always the human who ran it, and it is not a claim of accountability —
[`AI_STATEMENT.md`](AI_STATEMENT.md) §4 states that distinction precisely,
because the two are easy to conflate and mean different things. You remain
responsible for your submission in full, understood, explained on request,
tested, and honest, whichever tool helped write it.

## Financial support

Contributing code and funding maintenance are two independent ways to help —
neither is a prerequisite for the other, and nothing here changes for who
has or hasn't sponsored. See
[`spec/free-open-source-funding/`](spec/free-open-source-funding/index.md)
for the full policy. The short version: you can sponsor the maintainer,
[Joel Parker Henderson](https://github.com/sponsors/joelparkerhenderson), on
GitHub Sponsors today; an Open Collective for companies that need an invoice
is planned but not yet live.

## License

By contributing, you agree your contribution is licensed under the same
terms as the crate: MIT, Apache-2.0, GPL-2.0-only, or GPL-3.0-only, at the
user's option.
