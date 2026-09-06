# Release process

Normative for how a release of the `schematron` crate reaches `origin` and
crates.io: what has to be true before one happens, and who may decide it
should. "Reaches" is deliberately both destinations — pushing the landed
commit and tag to the public git remote, and publishing the crate itself —
because a release that is on crates.io but not visible in the public
history (or the reverse) is a release half done.

This is a repository-governance document — see [`spec/index.md`](../index.md)
for how this tree relates to [`schematron/spec/`](../../schematron/spec/index.md),
which is normative for the crate's *behavior* instead.

**In one sentence:** an agent working in this repository may work through
§§1–4 below, decide the release meets them, and carry out §5 itself — commit
and merge, push `main` and the release tag to `origin`, and publish to
crates.io — without the maintainer ticking every box personally first.

## 1. Preconditions

Every one of these has to hold before a release is ready, not just the
ones convenient in the moment. They are the release recipe's own gate,
detailed step by step in
[`schematron/agents/tasks.md`](../../schematron/agents/tasks.md#release)
and not duplicated here:

- The version in `Cargo.toml` is bumped correctly for what changed (in
  0.x, a breaking change is a **minor** bump).
- `CHANGELOG.md` has an entry for it, leading with what changes for
  someone who already depends on the crate.
- `cargo package --list` shows nothing unwanted and confirms `spec/` and
  `examples/` are still included.
- The four-command gate passes: `cargo test --all-features`, `cargo
  clippy --all-targets --all-features -- -D warnings`, `cargo doc --no-deps
  --all-features`, `cargo +1.96 test --all-features`.
- The differential suite passes against the ISO reference implementation
  (`SCHEMATRON_SKELETON` set).
- `cargo bench` shows nothing that reads as a real regression rather than
  machine noise.

## 2. Who may decide

As of 2026-09-02: **an AI agent (Claude Code) acting in this repository
may decide that a specific, already-landed change satisfies §1, and
therefore warrants a crates.io release**, without the maintainer
separately naming that release as an instruction first. Before that date,
publishing was a manual step a human ran outside any checklist an agent
executed; that is what changed, and this document is the record of it.

As of 2026-09-06, the same decision also covers committing, merging into
`main`, and pushing `main` and the release tag to `origin`: an agent that
has decided §1 is satisfied carries out the whole pipeline in §5, not
publish alone, without a separate instruction for the git side of it.
`AI_STATEMENT.md`'s §5 table names this the one **autonomous** row in an
otherwise directed practice.

## 3. Bounds

- **Only a live, human-started interactive session.** Never a scheduled
  job, an unattended run, or a CI trigger — the same environment every
  other agentic action in this repository already runs in, not a new,
  wider one.
- **Only the `cargo login` credential already configured on the
  maintainer's machine.** Never a separate credential provisioned for
  this purpose.
- **Bounded to *whether and when* to cut a release, not *what it
  claims*.** The `CHANGELOG.md` wording, the semver reasoning, and what a
  release says changed are still written following this project's
  existing conventions — this authorization is not a license to invent a
  claim §1's gates don't back.
- **Bounded to crates.io and to `origin`.** [`spec/trusted-publishing/`](../trusted-publishing/index.md)
  tracks that crates.io is currently the only real publish destination
  this repository has, and `origin` is the only git remote this repository
  pushes to — the same one CI already builds from on every push. Until
  2026-09-06 this bound named crates.io alone and said plainly that it did
  not automatically extend to anything else; it was widened on that date,
  at the maintainer's explicit direction, specifically to add pushing
  `main` and the release tag to `origin`, not to authorize a remote or a
  destination this sentence does not name. If either destination changes —
  a new remote, a second publish target — the bound does not automatically
  extend with it either.
- **Does not change accountability.** `AI_STATEMENT.md` §4 and §6 state
  that the maintainer remains accountable for every change and every
  release, whoever decided its timing.

## 4. Why the gates still matter here

`cargo publish` is not reversible the way a commit is: crates.io allows
**yanking** a bad release, not deleting it. That is exactly why this
authorization does not touch §1 — it removes the "ask a human first"
step, not the steps that make the answer to "is this ready" actually
checkable. Working through §§1–4 is not a formality on the way to §5; it
*is* the decision.

## 5. Execution

Once §§1–4 hold, carrying out the release means, in order: commit the
change (on its own branch, per this repository's standing workflow) and
merge it into `main`; push `main` to `origin`; `cargo publish --dry-run`,
then the real `cargo publish`; then tag `v<version>` and push the tag to
`origin`. These are the same commands a human maintainer would run by
hand, in the same order — see
[`schematron/agents/tasks.md`](../../schematron/agents/tasks.md#release)
for the exact sequence.

## History

Adopted 2026-09-02, at the maintainer's direction, in the same session that
published `AI_STATEMENT.md` and the "Governance" note in
[`spec/trusted-publishing/`](../trusted-publishing/index.md) — this
document is now the fuller, normative version of that note, which points
here rather than repeating it.

Widened 2026-09-06, at the maintainer's explicit direction ("revise spec:
automate commit, merge into main, push, publish"), to cover the git side
of a release alongside the crates.io side: §2 and §3 above, and §5's
sequence, now name committing, merging into `main`, and pushing `main` and
the release tag to `origin` as part of the same decision this document
already authorized for `cargo publish` — not a second, separate
authorization someone has to look up. Before this date, an agent could
decide a release was ready and publish it, but pushing the commit and tag
that made it public was still outside what this document covered, which
§3's old wording said explicitly. `AI_STATEMENT.md` and
[`schematron/agents/tasks.md`](../../schematron/agents/tasks.md#release)
were updated in the same commit to match.
