# Publishing for the monorepo.
#
# Only the site is published elsewhere, because GitHub Pages serves an
# organization site at <org>.github.io only from a repository of that exact
# name. That repository is derived output, not a second source: it is
# regenerated from here and never edited directly.
#
# The crate is deliberately NOT published to a repository of its own. It lives
# here and nowhere else, and `schematron/Cargo.toml` points crates.io and
# docs.rs at its subdirectory of this repository.
#
# `git subtree split` replays the history of the site subdirectory onto a
# branch rooted at that subdirectory, which is pushed to the Pages repo, where
# its own .github/workflows/deploy.yml builds and deploys it.
#
# Only committed files are published — a split reads history, not the working
# tree — so gitignored build output cannot leak into the public repo.
#
# `help` is the default target on purpose: a bare `make` must not push.
# Pass FORCE=1 to overwrite a Pages repo whose history did not come from a
# split of this monorepo.
#
# Two targets publish the same directory to the same repo, on purpose:
#
#   make publish       recommended for everyday use. `git subtree split`
#                       first, so a rejected push costs nothing to retry, and
#                       it refuses to run off `main` or with uncommitted
#                       changes under the site directory, or to overwrite
#                       history that didn't come from a split of this
#                       monorepo (without FORCE=1).
#   make github-pages  the plain `git subtree push` one-liner, with none of
#                       the above. It re-derives the split on every run
#                       instead of reusing one, which is slow on a history
#                       this repo's size and only gets slower as it grows.
#                       Kept for parity with generic git-subtree docs that
#                       describe exactly this command; `make publish` is the
#                       one to reach for otherwise.

FORCE_FLAG   := $(if $(FORCE),--force,)
PREFIX       := schematron-rust.github.io
REMOTE       := pages
PAGES_REMOTE := github-pages
URL          := git@github.com:schematron-rust/schematron-rust.github.io.git
SPLIT        := _split_site

.PHONY: help publish github-pages

help:
	@echo "make publish       publish $(PREFIX)/ to the Pages repo (recommended)"
	@echo "make github-pages  publish $(PREFIX)/ via a plain 'git subtree push'"
	@echo ""
	@echo "Add FORCE=1 to overwrite the Pages repo's history (make publish only)."

publish:
	@set -eu; \
		echo "== $(PREFIX)/ -> $(URL)"; \
		branch="$$(git branch --show-current)"; \
		if [ "$$branch" != "main" ]; then \
		  echo "error: on branch '$$branch'; publish from main." >&2; \
		  exit 1; \
		fi; \
		if [ -n "$$(git status --porcelain -- '$(PREFIX)')" ]; then \
		  echo "error: uncommitted changes under $(PREFIX)/ — commit them first." >&2; \
		  git status --short -- '$(PREFIX)' >&2; \
		  exit 1; \
		fi; \
		if ! git remote get-url '$(REMOTE)' >/dev/null 2>&1; then \
		  echo "Adding remote '$(REMOTE)' -> $(URL)"; \
		  git remote add '$(REMOTE)' '$(URL)'; \
		fi; \
		git branch -D '$(SPLIT)' >/dev/null 2>&1 || true; \
		git subtree split --prefix='$(PREFIX)' --branch='$(SPLIT)' >/dev/null; \
		sha="$$(git rev-parse '$(SPLIT)')"; \
		if git push $(FORCE_FLAG) '$(REMOTE)' '$(SPLIT):main'; then \
		  git branch -D '$(SPLIT)' >/dev/null; \
		  echo "Published $$sha to $(URL) (main)"; \
		else \
		  git branch -D '$(SPLIT)' >/dev/null; \
		  echo "" >&2; \
		  echo "Push rejected. $(URL)" >&2; \
		  echo "has commits that did not come from a split of this monorepo. That may" >&2; \
		  echo "be a README created with the repo, or real history from before the" >&2; \
		  echo "monorepo existed. Look before overwriting:" >&2; \
		  echo "" >&2; \
		  echo "    git fetch $(REMOTE) && git log --oneline $(REMOTE)/main" >&2; \
		  echo "" >&2; \
		  echo "To keep that history, archive it onto a branch first:" >&2; \
		  echo "" >&2; \
		  echo "    git push $(REMOTE) $(REMOTE)/main:refs/heads/pre-monorepo" >&2; \
		  echo "" >&2; \
		  echo "Then overwrite:" >&2; \
		  echo "" >&2; \
		  echo "    make publish FORCE=1" >&2; \
		  echo "" >&2; \
		  exit 1; \
		fi

github-pages:
	@if ! git remote get-url '$(PAGES_REMOTE)' >/dev/null 2>&1; then \
		echo "Adding remote '$(PAGES_REMOTE)' -> $(URL)"; \
		git remote add '$(PAGES_REMOTE)' '$(URL)'; \
	fi
	git subtree push --prefix=$(PREFIX) $(PAGES_REMOTE) main
