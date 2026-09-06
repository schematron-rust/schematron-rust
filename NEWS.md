# News

Repository-level news — announcements about the project instead of the
crate's behavior. The crate's own release notes are
[`schematron/CHANGELOG.md`](schematron/CHANGELOG.md); this file covers
everything else that is worth a reader noticing, starting from when this file
was created.

## 2026-09-06

- **The autonomous release decision widened to cover the git side, not
  crates.io alone.** Since 2026-09-02, an agentic tool could decide a
  landed, gate-passed change warranted a crates.io release and run
  `cargo publish` for it; pushing the resulting commit and tag to
  `origin` — the step that makes a release actually *visible* — was
  still outside that authorization, and
  [`spec/release-process/`](spec/release-process/index.md)'s bounds
  said so explicitly. At the maintainer's direction, that changed:
  the same decision now also covers committing, merging into `main`,
  and pushing `main` and the release tag to `origin`, so a release
  isn't decided in a way that leaves half of it for a human to finish.
  `AI_STATEMENT.md` moved to 1.2.0 to match. Exercised for the first
  time the same day, for `schematron` 0.15.0 — see
  [`schematron/CHANGELOG.md`](schematron/CHANGELOG.md) for what that
  release actually contains.

## 2026-09-02

- **AI_STATEMENT.md published.** This repository now discloses how AI tools
  are used to build it, who is accountable, and what is and isn't
  automated — reconciled with the same practice this maintainer's other
  Rust workspaces already follow, adapted to this repository's actual
  files rather than copied from theirs. See
  [`AI_STATEMENT.md`](AI_STATEMENT.md). The same document records a
  governance change adopted the same day: an agentic tool may now decide a
  landed, gate-passed change warrants a crates.io release and run `cargo
  publish` for it, bounded by
  [`spec/trusted-publishing/`](spec/trusted-publishing/index.md)'s
  "Governance" section.
- **Keep Co-Authored-By trailers, clarify the distinction.** `AI_STATEMENT.md`
  §4 originally prohibited a tool being "named as the author of, or a
  signer of," anything here without saying precisely what that meant —
  and a sibling workspace's own statement gets it wrong outright, banning
  "author, co-author, or signer" in a project whose own history carries
  `Co-Authored-By:` trailers. Corrected in 1.0.1: the trailer is
  disclosure and stays; what's prohibited is git's `Author:`/`Committer:`
  field itself, which is always the human. `CONTRIBUTING.md` gained a
  matching "Using AI tools" section so the conflation can't come back.

## 2026-08-29

- **GitHub Sponsors is live.** You can now sponsor the maintainer,
  [@joelparkerhenderson](https://github.com/sponsors/joelparkerhenderson),
  directly from this repository's GitHub page. See
  [`spec/free-open-source-funding/`](spec/free-open-source-funding/index.md)
  for the funding policy — what sponsorship does and does not change, and the
  Open Collective channel planned for companies that need an invoice.
