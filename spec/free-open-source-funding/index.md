# Free and open source funding

How this project accepts financial support, and what that support does and
does not change.

## Goal

Make it possible for people and companies who rely on `schematron` to fund
its maintenance, without turning that funding into an obligation. The project
is free and open source before and after this spec is carried out — funding
pays for continued maintenance, it does not buy priority, a roadmap seat, or
an SLA.

## Principles

- **No paywall.** Every release, every spec, every fix lands in the public
  repository regardless of who has or has not sponsored.
- **No influence for sale.** A sponsorship does not move an issue up the
  [roadmap](../../schematron/spec/roadmap/index.md) or grant a say in design
  decisions. [`conformance/`](../../schematron/spec/conformance/index.md) and
  the standard remain the contract, not a sponsor.
- **Individuals and companies both have a route in.** GitHub Sponsors suits
  an individual paying from a personal card. Some companies cannot use it —
  procurement wants an invoice and a legal entity on the other end — so a
  second channel exists for them.
- **Nothing is announced before it works.** A funding link that 404s or
  points at an unclaimed account is worse than no link. Each channel below is
  added to [`.github/FUNDING.yml`](#githubfundingyml) only once it is live.

## Channels

### GitHub Sponsors

Target: **the maintainer**, [@joelparkerhenderson](https://github.com/joelparkerhenderson),
not the `schematron-rust` organization. The org
(<https://github.com/schematron-rust>) is a single-seat free-plan org created
to hold this repository and its mirrors; enrolling an *organization* in
GitHub Sponsors is a separate, heavier application than enrolling an
individual account, and there is exactly one maintainer to pay today. This
can move to the org later if the project grows more maintainers — nothing
below depends on which account is chosen, only `.github/FUNDING.yml` would
change.

**Already live**: `joelparkerhenderson` has an approved Sponsors listing —
<https://github.com/sponsors/joelparkerhenderson> returns 200, and the
account's `hasSponsorsListing` is true. What is left is linking this specific
repository to it (step 4 below) and turning on the button, which
[`.github/FUNDING.yml`](#githubfundingyml) does.

Setup, done once, outside this repository:

1. ~~Apply at <https://github.com/sponsors> for the `joelparkerhenderson`
   account.~~ Done.
2. ~~Wait for approval.~~ Done.
3. Set tiers, or leave it as a custom-amount-only profile — either is valid;
   a fixed-cost recurring tier is not required to accept sponsorship.
4. Link this repository from the Sponsors profile so it appears as a
   "Sponsor this project" candidate.

### Open Collective

Target: a new collective, likely named `schematron-rust` to match the
organization and the crate.

Purpose: the route in for a company that cannot pay an individual through
GitHub Sponsors. Open Collective is a registered fiscal host, issues
invoices and receipts, and publishes every expense and every contribution —
which also keeps this channel consistent with the "no influence for sale"
principle above, since the ledger is public.

Setup, done once, outside this repository:

1. Create the collective at <https://opencollective.com>.
2. Choose a fiscal host (Open Collective's own host is the default; it takes
   a small percentage of funds).
3. Write the collective's "About" to match this document's Principles
   section, so the pitch on the funding side agrees with the pitch here.

## `.github/FUNDING.yml`

Lists only channels that are actually live. Today that is GitHub Sponsors
alone:

```yaml
github: joelparkerhenderson
```

Add `open_collective: schematron-rust` in the same file the day the
collective in the section above actually exists — not before, per the
"nothing is announced before it works" principle.

This file is what turns on the "Sponsor" button GitHub renders on the
repository page. It must live in `.github/FUNDING.yml` at the root of the
repository (not under `schematron/` or `schematron-rust.github.io/`), because
GitHub reads it relative to the repository root regardless of where the
crate or the site live inside the monorepo.

`FUNDING.yml` supports other platforms (`patreon`, `tidelift`,
`community_bridge`, `liberapay`, `issuehunt`, `ko_fi`, `custom`, and more).
None of those are added here: GitHub Sponsors and (eventually) Open
Collective already cover an individual payer and a company payer, and each
additional platform is one more account to keep current, one more place a
broken link can go stale, and one more thing a reader has to choose between.

## `CONTRIBUTING.md`

Carries a short "Financial support" section — a sentence or two making clear
that contributing code and funding maintenance are two independent ways to
help, neither a prerequisite for the other — linking to whichever of the
channels above are live at the time. Contributors are exactly the audience
most likely to also consider sponsoring, and least likely to find a funding
link that only lives on the GitHub sidebar. Update that section's links when
Open Collective goes live.

## `NEWS.md`

This repository does not yet have a root `NEWS.md` (the crate's own release
notes are [`schematron/CHANGELOG.md`](../../schematron/CHANGELOG.md)). When a
funding channel goes live, it should get one line in whichever file is
carrying repository-level news at the time, the same way any other
repository-visible change is recorded — not a special announcement, just an
honest entry.

## Status

| Task | Status |
|---|---|
| GitHub Sponsors approved for `joelparkerhenderson` | **Live** |
| Open Collective created for `schematron-rust` | Not started |
| `.github/FUNDING.yml` added | **Done** — lists `github` only, until Open Collective exists |
| `CONTRIBUTING.md` carries a funding section | **Done** |
| Funding channels recorded in `NEWS.md` | Not started — `NEWS.md` does not exist yet |

## Order of operations

The dependency runs in one direction, so this is the order, not a menu:

1. ~~Apply for GitHub Sponsors as `joelparkerhenderson` and wait for
   approval.~~ Done, ahead of this spec — the account already had an
   approved listing when this was written.
2. Create the Open Collective.
3. ~~Add `.github/FUNDING.yml` naming the channels that are actually
   live.~~ Done — `github: joelparkerhenderson` only, for now.
4. ~~Add `CONTRIBUTING.md`, linking the same channels.~~ Done.
5. Record the change in `NEWS.md`.
6. Once Open Collective exists, add `open_collective: schematron-rust` to
   `.github/FUNDING.yml` and update this table.

Step 2 has no dependency on anything else here and can happen at any time.
