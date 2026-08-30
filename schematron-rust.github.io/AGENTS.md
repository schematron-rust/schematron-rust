# schematron-rust.github.io — contributor and agent guide

This is the **public site** for the `schematron` crate. It is documentation, not
implementation. See [index.md](index.md) for the human-oriented overview.

## Metadata

- **Package**: schematron-rust.github.io
- **Upstream crate**: <https://github.com/schematron-rust/schematron-rust>
- **Deploys to**: <https://schematron-rust.github.io/>
- **License**: MIT or Apache-2.0 or GPL-2.0-only or GPL-3.0-only
- **Contact**: Joel Parker Henderson (joel@joelparkerhenderson.com)

## What this is

A SvelteKit project (`@sveltejs/adapter-static`) that prerenders every route to
static HTML, deployed by GitHub Actions to GitHub Pages. It ships no Rust and
implements nothing: the crate is the product, and this site explains it.

## The single most important rule

**The crate's `spec/` directory is normative; this site is not.**

Every technical claim here — an option, an exit code, a conformance row, the
MSRV, the version — is *copied* from the crate repo. When the two disagree, the
crate is right and this site is stale. Never "fix" a discrepancy by editing the
crate to match the site.

Facts copied from the crate live in exactly two places:

- `src/lib/site.ts` — version, MSRV, licence, repo URLs, the `spec/` table of
  contents.
- Page-local `const` arrays — the CLI option table in `src/routes/cli/`, the
  conformance table in `src/routes/conformance/`.

Prefer linking to `spec/<file>.md` over restating it. Use the `specUrl()` helper
in `src/lib/site.ts` so the link shape stays in one place.

## Design system

Components come from `lily-design-system-svelte-headless` (the Lily Design
System). Lily components are **headless**: they render semantic HTML with
correct ARIA and one stable kebab-case class hook each, and ship no CSS.

- Import components as named exports from the package root, e.g.
  `import { Card } from 'lily-design-system-svelte-headless';` — the package's
  `exports` map only exposes `.`, so a deep path like
  `.../components/Card/Card.svelte` does not resolve. One `import { A, B, ... }`
  line per file is the house style; keep it together rather than one import
  per component.
- **All** styling lives in `static/assets/style.css`. There are no `<style>`
  blocks in components, and there should not be.
- Adding a Lily component to a page means adding a rule for its class hook to
  that stylesheet, in the "Lily component hooks" section, with the component
  named in a comment.
- Site-local layout classes are prefixed `site-`. Everything else in the
  stylesheet is a Lily hook.
- Prefer a Lily component over hand-written markup when one fits — that is what
  keeps the accessibility contract honest.

## Working rules

- Every route is prerendered. `src/routes/+layout.ts` sets
  `prerender = true` and `trailingSlash = 'always'`; internal links must
  therefore end in `/`.
- Adding a route means three edits: the `+page.svelte`, the `navLinks` array in
  `src/routes/+layout.svelte`, and a row in `static/sitemap.xml`. Add it to the
  `PAGES` array in `tests/site.spec.ts` too.
- Exactly one `<h1>` per page, inside `.page-header` (or `.hero` on the home
  page). Section headings use Lily's `SectionHeading`.
- Every page needs a `<svelte:head>` with a `<title>` containing "schematron"
  and a `<meta name="description">`. The tests check both.
- Code samples go in a Lily `CodeBlock` wrapping a `<pre><code>`. Put the sample
  in a Svelte expression holding a template literal, so Svelte does not read
  `{` as an expression and so `<` and `>` need no escaping:

  ```svelte
  <CodeBlock label="Validate a document">
    <pre><code>{`schematron --schema rules.sch data.xml`}</code></pre>
  </CodeBlock>
  ```
- No client-side JavaScript beyond what SvelteKit emits. The site works with
  scripting disabled; interactive disclosure uses native `<details>` via Lily's
  `Details`.

## Gates

```sh
pnpm check      # svelte-check, zero errors
pnpm build      # prerenders every route; a broken internal link fails here
pnpm test       # Playwright: per-page smoke tests and an internal link check
```

`pnpm build` is the strict one: `adapter-static` is configured with
`strict: true`, so a route that fails to prerender is a build failure, not a
warning.

## Deployment

`.github/workflows/deploy.yml` builds on every push to `main` and publishes
`build/` to GitHub Pages. There is no other deploy path, and no manual step.

Because this is an organisation Pages site (`schematron-rust.github.io`), the
base path is `/`. Do not add a `paths.base` to `svelte.config.js` — every
internal link is written root-relative and depends on it.
