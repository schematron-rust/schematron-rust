# AI statement

| | |
|---|---|
| Version | 1.0.1 |
| Effective date | 2026-09-02 |
| Status | Active |
| Author and owner | Joel Parker Henderson, maintainer |
| Canonical location | `AI_STATEMENT.md` at the repository root |
| License | The same four-way choice as the crate — see §8 |
| Review | At every release that changes the practice described here, and on any trigger in §13 |

**Abstract.** This document discloses how artificial-intelligence tools are
used to develop schematron-rust, a monorepo holding a pure Rust ISO/IEC
19757-3 Schematron validator and its companion website. It states what the
tools do and do not touch, who is accountable, which controls bound the
work and how each is enforced, the licensing and data posture, the rules
for contributors, the uses that are prohibited, and the limitations that
survive all of it. It is a self-declaration by the maintainer, written for
evaluators and downstream adopters performing supplier due diligence, and
it changes in the same commit that changes the practice it describes.

The key words **shall**, **should**, and **may** are used as ISO/IEC
Directives Part 2 defines them: requirement, recommendation, permission.

## 1. Scope

This document covers the use of AI tools in developing everything in this
repository: the `schematron` crate's source, tests, fuzz targets and
benchmarks; its `spec/` (normative for the crate's behavior); the
`schematron-rust.github.io` website; the repository-level `spec/`
(governance, funding, publishing policy); the agent skills
(`schematron-skill`, `schematron-rust-maintainer-skill`); and this document
itself.

It does not cover an AI system in the product, because there is none:
**schematron-rust ships no AI.** No model is trained, embedded, or called
at run time; no inference happens anywhere in the published crate or the
site. The crate is `#![forbid(unsafe_code)]`, has no FFI and no C
dependency, and its default resolver refuses `http(s)` — including through
`document()` — so validation itself makes no network call
(`schematron/agents/invariants.md`). AI is used to *build* the software, in
the same sense a compiler and a linter are used to build it.

## 2. Which frameworks apply, and which do not

Stated plainly, because borrowed authority is worse than none. These are
the project's readings, not legal advice, and no legal review has been
performed.

- **The EU AI Act imposes no obligation on this project.** The Act binds
  providers and deployers of AI *systems*; this repository is not one, and
  Article 50's marking duties bind an AI tool's provider rather than the
  tool's user. This document is voluntary.
- **This crate is not a compliance or certification product.** It validates
  XML against Schematron schemas the caller supplies; conformance to
  ISO/IEC 19757-3 is a self-assessment
  ([`schematron/spec/conformance/`](schematron/spec/conformance/index.md)),
  not a certification. A downstream integrator who gives their own product
  a regulatory purpose brings *that* product into scope; that
  classification is theirs to make.
- **No standard is claimed as conformity.** No certification exists, no
  audit has occurred, and the words "certified", "audited", and "validated"
  appear in this document only in this sentence, to say they do not apply.

## 3. Terms

This document reuses the W3C AI Content Disclosure vocabulary rather than
inventing one: **none** (entirely human-authored), **ai-assisted**
(human-authored; AI edited, refined, or filled in boilerplate),
**ai-generated** (AI-generated with human prompting and review),
**autonomous** (AI-generated without meaningful human oversight). An
**agentic tool** is one that plans and executes multi-step work — editing
files, running builds and tests — under a human's direction, as opposed to
inline completion.

## 4. Accountability

One named human — the maintainer, Joel Parker Henderson (`Cargo.toml`'s
`authors` field; the sole human author in the git history, alongside
Dependabot's automated dependency-bump commits, which are a separate,
non-AI tool) — is the author of and accountable for every change in this
repository, whatever tool produced the bytes. A tool **shall not** be
recorded in git's `Author:` or `Committer:` field, and **shall not** sign
anything, because a tool cannot be responsible for accuracy, integrity, or
originality, and responsibility that cannot be borne cannot be assigned.

That is a narrower rule than "a tool shall not be named as a co-author,"
and the distinction matters enough to state precisely rather than let a
reader infer it: §10's `Co-Authored-By:` commit trailer **does** name the
tool, in the trailer, on every commit an agentic tool touched — that is
disclosure, not a violation of this section. What is prohibited is the
`Author:`/`Committer:` field itself, which git always records as the
human who ran the commit — `git log --format='%an <%ae>'` shows the
maintainer on every commit in this history — and the act of signing. A
trailer that discloses participation and a field that assigns
accountability are different things, and conflating them is exactly the
mistake this section exists to avoid.

## 5. Where AI is used, and at what level

The tooling is agentic AI coding assistance — Claude Code, by Anthropic —
in sessions the maintainer directs and reviews. The crate carries
[`schematron/AGENTS.md`](schematron/AGENTS.md) with
[`schematron/CLAUDE.md`](schematron/CLAUDE.md) pointing to it, and the site
carries its own `AGENTS.md`/`CLAUDE.md` pair; those files are the standing
instructions given to the tools, they are committed, and they are readable
by anyone evaluating this claim. `schematron-rust-maintainer-skill/` is a
third, agent-specific layer on top of them.

Levels below use the §3 vocabulary. Deliberately, no percentage appears
anywhere in this document: no defensible method exists for measuring one.

| Activity | Level | Notes |
|---|---|---|
| Crate source, tests, fuzz targets, benchmarks | ai-generated | Written in directed sessions against [`schematron/spec/`](schematron/spec/index.md); reviewed and committed by the maintainer |
| `schematron/spec/` and the repository-level `spec/` | ai-generated | The normative layer for the crate's behavior and the repository's governance, respectively |
| The website (`schematron-rust.github.io/`) | ai-generated | Held to its own `AGENTS.md`/`CLAUDE.md` |
| Documentation, `CHANGELOG.md`, `NEWS.md`, and this statement | ai-generated | Held to the repository's own prose conventions |
| What ISO/IEC 19757-3 requires where the standard is silent, conformance and portability findings, requirement adjudications | none | Decided by the maintainer, recorded in `schematron/spec/conformance/` or `schematron/spec/roadmap/` with reasoning |
| Deciding *that* a specific, already-landed change warrants a crates.io release, and executing `cargo publish` for it | autonomous | Adopted 2026-09-02, at the maintainer's direction, and bounded by [`spec/trusted-publishing/`](spec/trusted-publishing/index.md)'s "Governance" section and the release recipe in [`schematron/agents/tasks.md`](schematron/agents/tasks.md#release): only inside a live interactive session on the maintainer's own machine, never a scheduled or unattended one; only using the `cargo login` credential already configured there, never a separate one; and only after every step of that recipe holds — the version bump, the `CHANGELOG.md` entry, `cargo package --list`, the four-command gate, the differential suite, and `cargo bench`. §4's accountability is unchanged by this row |
| Accepting a contribution from someone else | none | Prohibited use; see §11 |

**autonomous** now appears in exactly the one row above, adopted
2026-09-02 — see §6.

## 6. Human oversight

The maintainer directs the work, reads the result, and commits every
change; nothing lands on its own authority. Until 2026-09-02, publishing to
crates.io was a manual step run outside any checklist an agent executed —
[`spec/trusted-publishing/`](spec/trusted-publishing/index.md) said so
plainly. That is now the one exception: within the bounds that document and
[`schematron/agents/tasks.md`](schematron/agents/tasks.md#release) state, an
agentic tool may decide a specific, already-committed change warrants a
release without the maintainer naming that release as a separate
instruction, and execute it. That is deliberately narrower than
"automated" in the scheduled-workflow sense — it still requires a
human-started, live session, the same environment every other agentic
action in this repository already runs in — and it is the one row in §5
where the decision is not the maintainer's alone. Every other decision with
consequences — what a conformance gap means, what a roadmap item is worth
building, what a release's `CHANGELOG.md` entry claims — remains the
maintainer's. A decision that exists only inside a tool session, outside
what §5's autonomous row bounds, is still not a decision this project made.

## 7. Quality controls, and what each one proves

AI-produced work is not a shortcut around engineering process. Every
change to the crate, whoever or whatever wrote it, passes the same gate,
run from `schematron/`:

```sh
cargo test --all-features                     # unit, integration, corpus, CLI, docs
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features            # must be warning-free
cargo +1.96 test --all-features               # the MSRV boundary
```

- **Spec authority.** [`schematron/spec/`](schematron/spec/index.md) is
  normative for the crate's behavior; code and spec disagreeing is a defect
  in one of them, not a judgment call. `tests/docs.rs` and `tests/cli.rs`
  mechanically enforce that several facts — the XPath function lists, CLI
  flags, exit codes, the lint table — cannot drift between the two.
- **A corpus of schema/document/expected-report triples**
  (`schematron/tests/corpus/`) is the primary way new Schematron semantics
  are tested, deliberately more than unit tests of engine internals.
- **Differential testing against the ISO reference implementation**
  (`schematron/tests/differential.rs`, needs `xsltproc` and a fetched
  skeleton) checks every corpus case, plus grammar-generated schema/document
  pairs, against the reference XSLT processor and records every known
  divergence in [`schematron/spec/conformance/`](schematron/spec/conformance/index.md).
- **Fuzz targets** (`schematron/fuzz/`) bound the XML parser, the XPath
  parser, `include`/`extends` resolution, and the validator itself, because
  "malformed input is an error, never a panic" is a non-negotiable
  (`schematron/agents/invariants.md`).
- **Criterion benchmarks**, run against a saved baseline before a release,
  so a performance claim is a measurement rather than an impression.
- **CI runs the crate, MSRV, and site jobs on every push and pull request**
  (`.github/workflows/ci.yml`); it does not itself publish anything.
- **Tests and expectations shall not be weakened to make a build pass.**
  That is a standing hard rule, for humans and tools alike.

What these controls do **not** prove is §12.

## 8. Licensing and provenance of AI output

`Cargo.toml` declares `license = "MIT OR Apache-2.0 OR GPL-2.0-only OR
GPL-3.0-only"`, matching [`CONTRIBUTING.md`](CONTRIBUTING.md)'s License
section. This repository does not yet carry the license text as a
`LICENSE.md` file at the root — that is a repository gap, not an AI-practice
one, and is named in §12 rather than glossed over here.

The position taken on AI output follows the reasoning the Apache Software
Foundation and LLVM have published rather than a wishful shortcut: an AI
tool's output does not launder anyone's copyright, the full provenance of
generated text is generally not knowable, and prompting alone is not
treated as authorship. In practice: contributions of substantially copied
third-party material are refused however they were produced; generated
code is held to the same originality expectations as human code, under the
same review; and if identifiable third-party material is found in the
tree, it is removed or licensed properly.

## 9. Data

This crate validates XML documents the caller supplies; it does not
collect, store, or transmit anything about who calls it or what they
validate, and its default resolver refuses `http(s)`
(`schematron/agents/invariants.md`), so a validation run makes no network
call to begin with. Every fixture in the repository — the corpus cases
under `schematron/tests/corpus/`, the `examples/invoice*.xml` and `.sch`
files, the fuzz corpus — is synthetic test data written for this project,
not a record about a real person or organization, and therefore not in any
prompt. This is a structural property a reader can check against the tree,
not a promise about tool behavior.

Vendor-side data handling is governed by the tool vendor's terms; this
document deliberately makes no claim on the vendor's behalf, because such
claims go stale silently.

## 10. Rules for contributors

Contributors **may** use AI tools. A contribution with **ai-generated**
content per §3 **should** say so in the pull-request description: which
tool, and what it did.

**This project records tool participation in commit trailers**, in the
form `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, plus a
`Claude-Session:` link where one exists, and the history carries them. That
is a deliberate choice and worth naming, because it is not universal — some
projects require such trailers and others forbid them. §4 governs how to
read one: the trailer records participation, and the `Author:` field
records accountability.

A contributor remains responsible for their submission in full, under the
same [`CONTRIBUTING.md`](CONTRIBUTING.md) bar as any other work: understood,
explained on request, tested, and honest.

## 11. Prohibited uses

In this project, AI **shall not**: merge a contribution from someone else
on its own authority; decide what a silent point in ISO/IEC 19757-3 means;
mark a conformance or portability finding without a divergence actually
established by running both implementations
(`schematron/spec/conformance/`); sign anything; or weaken a test, an
expectation, a spec rule, or a gate to make something pass. The last is a
standing hard rule for humans and tools alike.

## 12. Limitations and residual risks

This section exists because a disclosure without one is marketing.

- **The gates prove what they test, not correctness.** The corpus,
  differential, and fuzz suites demonstrate the behaviors and inputs they
  cover; coverage is real and ratchets upward, and it is still a boundary.
- **Review depth is one person's.** This repository has a single human
  maintainer, and no separate `MAINTAINERS.md` yet names that formally —
  `Cargo.toml`'s `authors` field and the git history are the checkable
  facts. "The maintainer understands and can explain every committed
  change" is the honest claim; "every line was independently re-derived"
  would not be. That claim is about the *content* of a change, made in a
  directed session per §5's other rows, and is unaffected by an agentic
  tool deciding the *timing* of a release under §5's autonomous row — the
  code being released was still reviewed and committed by the maintainer
  before the release decision was ever made.
- **No `LICENSE.md` file yet.** `Cargo.toml`'s `license` field and
  `CONTRIBUTING.md` state the four-way choice; the license texts themselves
  are not yet in the tree as a root file. Named here so it is not
  discovered instead of disclosed.
- **No `SECURITY.md` or `MAINTAINERS.md` yet.** §14's reporting route is
  therefore a public issue, not a private one; that is a real limitation,
  not a stylistic choice.
- **Retroactivity.** Commits predating this statement carry the trailers
  described in §10 but no other disclosure marker. This document describes
  the practice, not a per-commit audit trail.
- **Provenance uncertainty survives.** Whether any generated fragment
  echoes unlicensed training material is not fully knowable with current
  tools. §8 states the handling, not a guarantee.
- **The legal ground is unsettled.** Copyright in AI output is an open
  question in most jurisdictions. This document records positions, and
  positions may have to change.
- **This is a self-declaration.** No third party has audited it. The
  checkable artifacts — the specs, the tests, the trailers, `ci.yml` — are
  the counterweight: they can disagree with this document, and if they do,
  the document is wrong.

## 13. Review and change

This statement is reviewed at every release that changes the practice
described here, and revised off-cycle when any of these fires: the tooling
changes materially, a tool vendor's terms change in a way §8 or §9 relies
on, a binding rule emerges that touches this use, or a claim in this
document stops being true. The change lands as a commit like everything
else, and the version and the change log in Annex A update in the same
commit.

## 14. Reporting

A suspected provenance, licensing, or quality problem in this repository —
including a claim in this document that does not survive checking — is a
report this project wants. Open an issue and cite this file. There is no
private security route yet (see §12); that is itself a named limitation,
not an oversight papered over.

## 15. References

**Normative for this project:** [`CONTRIBUTING.md`](CONTRIBUTING.md);
[`schematron/spec/index.md`](schematron/spec/index.md) and the documents it
routes to, in particular
[`schematron/spec/conformance/`](schematron/spec/conformance/index.md) and
[`schematron/spec/roadmap/`](schematron/spec/roadmap/index.md);
[`spec/index.md`](spec/index.md) and
[`spec/trusted-publishing/`](spec/trusted-publishing/index.md);
[`schematron/AGENTS.md`](schematron/AGENTS.md) and
[`schematron/agents/`](schematron/agents/); `.github/workflows/ci.yml`.

**Informative:** the W3C AI Content Disclosure vocabulary, used for §3's
terms; the ISO/IEC Directives Part 2 verbal forms; the Apache Software
Foundation's and LLVM's generative-tooling positions; the structure of this
document follows the AI statement this maintainer's other Rust workspaces
(`fhir-rust`, `hl7-rust`, and siblings) already publish, reconciled here to
this repository's actual files rather than copied from theirs.

## Annex A. Change log

| Version | Date | Change |
|---|---|---|
| 1.0.0 | 2026-09-02 | First issue, written to reconcile this repository with the AI-disclosure practice already adopted in this maintainer's other Rust workspaces, and to record the same day's governance change: an agentic tool may now decide a landed change warrants a crates.io release and run `cargo publish` for it, bounded by [`spec/trusted-publishing/`](spec/trusted-publishing/index.md). |
| 1.0.1 | 2026-09-02 | §4 corrected: the first issue's "shall not be named as the author of, or a signer of" was imprecise about the `Co-Authored-By:` trailer §10 already documents, echoing wording an earlier sibling-workspace draft got wrong (a blanket "no co-author" rule that its own commit history contradicted). Restated precisely: git's `Author:`/`Committer:` field is always the human, and is what §4 prohibits a tool from occupying; a trailer naming the tool as co-author is disclosure, not a violation. `CONTRIBUTING.md` gained a matching "Using AI tools" section, so the same mistake cannot be reintroduced there either. |

## Annex B. Machine-readable summary

Levels per the W3C AI Content Disclosure vocabulary (§3); the prose above
is authoritative where the two could ever disagree.

```yaml
ai-statement:
  version: 1.0.1
  last-updated: 2026-09-02
  vocabulary: w3c-ai-content-disclosure
  disclosure-default: ai-generated
  tools:
    - name: Claude Code
      provider: Anthropic
  processes:
    design: ai-assisted
    implementation: ai-generated
    testing: ai-generated
    specification-text: ai-generated
    documentation: ai-generated
    review: none
    standards-adjudication: none
    release-decisions: autonomous
  commit-trailers: true
  ships-ai-system: false
  autonomous-use: release-publish-only
```
