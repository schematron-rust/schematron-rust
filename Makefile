# Publishing for the monorepo.
#
# The monorepo is the source of truth. `git subtree split` replays the history
# of one subdirectory onto a synthetic branch rooted at that subdirectory, and
# that branch is pushed to its standalone repo.
#
# Only committed files are published — a split reads history, not the working
# tree — so gitignored build output cannot leak into a public repo.
#
# `help` is the default target on purpose: a bare `make` must not push.
# Pass FORCE=1 to overwrite a target repo whose history did not come from a
# split of this monorepo.
#
# The recipe is factored through a recursive `_publish` because macOS ships
# GNU Make 3.81, which has neither .ONESHELL nor .RECIPEPREFIX.

FORCE_FLAG := $(if $(FORCE),--force,)
SPLIT      := _split_$(TARGET)

.PHONY: help publish publish-site publish-crate _publish

help:
	@echo "make publish         publish site and crate to their standalone repos"
	@echo "make publish-site    schematron-rust.github.io/ -> Pages repo"
	@echo "make publish-crate   schematron/                -> crate repo"
	@echo ""
	@echo "Add FORCE=1 to overwrite a target repo's history."

publish: publish-site publish-crate

publish-site:
	@$(MAKE) --no-print-directory _publish \
		  TARGET=site \
		  PREFIX=schematron-rust.github.io \
		  REMOTE=pages \
		  URL=git@github.com:schematron-rust/schematron-rust.github.io.git

publish-crate:
	@$(MAKE) --no-print-directory _publish \
		  TARGET=crate \
		  PREFIX=schematron \
		  REMOTE=crate \
		  URL=git@github.com:schematron-rust/schematron.git

_publish:
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
		  echo "has commits that did not come from a split of this monorepo — most" >&2; \
		  echo "often an initial README created with the repo. These repos are" >&2; \
		  echo "derived artifacts, so the fix is normally:" >&2; \
		  echo "" >&2; \
		  echo "    make publish-$(TARGET) FORCE=1" >&2; \
		  echo "" >&2; \
		  exit 1; \
		fi
