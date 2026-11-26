# Dependabot

Enable GitHub Dependabot dependabot_security_updates at the repo level. 

Enable GitHub Dependabot .github/dependabot.yml for scheduled update PRs.

## Merge gate

Dependabot proposes updates, and `cargo deny check`
([`schematron/deny.toml`](../../schematron/deny.toml)) decides whether a
dependency may land. CI runs it on every push and pull request. It rejects a
crate with a RustSec advisory, a yanked version, a license outside the
allow list, a wildcard requirement, or a source other than crates.io. A new
license goes into the allow list in the same change as the dependency that
brings it in.
