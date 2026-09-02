# Trusted Publishing

Trusted Publishing is a secure way to publish your Rust crates from CI/CD platforms like GitHub Actions and GitLab CI/CD without manually managing API tokens. It uses OpenID Connect (OIDC) to verify that your workflow is running from your repository, then provides a short-lived token for publishing.

Instead of storing long-lived API tokens in your repository secrets, Trusted Publishing allows your CI/CD platform to authenticate directly with crates.io using cryptographically signed tokens that prove the workflow's identity.

We intend to add "Trusted Publishing" when it is production-ready across all our code forges (GitHub.com, GitLab.com, Codeberg.org, etc.) and across all our target destinations (Rust crates.io, NPM npmjs.com, etc.).

## Current status

As of 2026-08-29, that condition is not met:

| Forge | crates.io Trusted Publishing |
|---|---|
| GitHub.com | Production-ready, via `rust-lang/crates-io-auth-action` |
| GitLab.com | Production-ready (GitLab.com only; self-hosted GitLab is not supported) |
| Codeberg.org | Not available. The tracking issue on the Forgejo side, [forgejo#9939](https://codeberg.org/forgejo/forgejo/issues/9939), was closed as "not actionable": a self-hosted, federated forge cannot establish the platform-level trust crates.io's OIDC model assumes, absent a concrete proposal from Forgejo instance operators. This is a structural blocker, not a scheduling one, so it may not resolve on any predictable timeline. |

Separately, npmjs.com is not actually a target destination for this
repository: [`schematron-rust.github.io`](../../schematron-rust.github.io/)
is `"private": true` in its `package.json` and is deployed to GitHub Pages
via `git subtree split` (see the root [`Makefile`](../../Makefile)), never
`npm publish`. crates.io, published manually today from the maintainer's
machine, is the only real destination this policy governs.

Given Codeberg's blocker looks structural rather than temporary, revisit
this policy — rather than only re-checking Codeberg's status — the next time
this document is read with intent to act on it.

## Governance: who may run the manual publish

As of 2026-09-02, an AI coding agent (Claude Code) acting on the
maintainer's behalf is authorized to run `cargo publish` itself, as the
final step of the release recipe in
[`schematron/agents/tasks.md`](../../schematron/agents/tasks.md#release) —
not only to prepare a release for a human to run it. This is still the
"published manually" arrangement above, credential-wise: the same
long-lived `cargo login` token on the maintainer's machine, not OIDC. What
changed is who is trusted to press the button, given the release recipe's
own gates (the four-command gate, the differential suite, and `cargo
package --list`) have already run first. It does not change the Trusted
Publishing question this document otherwise tracks, and does not extend to
any other publish destination without a separate decision.
