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

Components come from four Lily Design System packages: the general catalog,
`lily-design-system-svelte-headless`, plus three standalone widgets each
published separately — `lily-design-system-svelte-theme-picker`,
`-text-size-picker`, and `-share-picker` (all live in the header, see
`src/routes/+layout.svelte`). Lily components are **headless**: they render
semantic HTML with correct ARIA and one stable kebab-case class hook each,
and ship no CSS.

- Import components as named exports from the package root, e.g.
  `import { Card } from 'lily-design-system-svelte-headless';` — every Lily
  package's `exports` map only exposes `.`, so a deep path like
  `.../components/Card/Card.svelte` does not resolve. One `import { A, B, ... }`
  line per file is the house style; keep it together rather than one import
  per component. The three picker packages are separate npm packages, so they
  need their own import line each — they cannot be folded into the headless
  catalog's import.
- **All** styling lives in `static/assets/style.css`. There are no `<style>`
  blocks in components, and there should not be.
- Adding a Lily component to a page means adding a rule for its class hook to
  that stylesheet, in the "Lily component hooks" section, with the component
  named in a comment.
- Site-local layout classes are prefixed `site-`. Everything else in the
  stylesheet is a Lily hook.
- Prefer a Lily component over hand-written markup when one fits — that is what
  keeps the accessibility contract honest.
- `SharePicker`'s `targets` array (`+layout.svelte`) is real share/compose
  endpoints, not a generic template — each network's URL contract differs
  (LinkedIn's `share-offsite` ignores a title parameter entirely and reads
  Open Graph tags instead, which this site does not yet set; Bluesky's
  compose intent has one `text` field, not separate url/title; Mastodon is
  federated and has no single endpoint, hence the third-party
  `mastodonshare.com` redirector). Adding a network means reading that
  network's actual share-intent contract, not copying the shape of an
  existing entry.

## Theming

`static/assets/style.css` carries no colour of its own — every custom
property it consumes (`--rust`, `--page-bg`, `--border`, …) is defined by
whichever of `static/assets/themes/{light,dark}.css` is currently linked.
`ThemePicker` (in the header) swaps that `<link>`'s `href` and sets
`data-theme` on `<html>`; `src/app.html` carries a matching inline script
that resolves and applies the theme *before* any stylesheet loads, using the
same `storageKey` and the same `data-lily-theme-picker="theme"` selector as
the component, so `ThemePicker` finds and reuses that link on mount instead
of creating a second one. Changing the picker's `storageKey` or `name` prop
in `+layout.svelte` means updating `app.html` to match, or the two silently
stop agreeing and the page flashes the wrong theme on load.

- Both theme files must define the exact same set of custom properties —
  including the structural ones that never change (`--radius`,
  `--content-max`, `--font-sans`, …), since a theme file is the only
  stylesheet guaranteed linked at all times.
- `--rust`/`--rust-hover` are text/border-role colours; `--button-fill`/
  `--button-fill-hover` are a separate pair for filled surfaces with white
  text on top (`.button-primary`, `.skip-link`). dark.css's `--rust` is
  brightened for legibility as text on a dark page and would fail contrast
  as a fill behind white text, which is why the two pairs don't share values
  there. Don't collapse them back into one without rechecking contrast.
- New colour used anywhere in `style.css` must be a custom property defined
  in both theme files, never a literal hex value — with one deliberate
  exception: `pre` stays a fixed dark "terminal" block in both themes (see
  the comment on that rule).
- Text size (`small`/`medium`/`large`/`x-large`, via `TextSizePicker`) works
  by scaling `html[data-text-size="…"] { font-size: … }`, which rescales
  everything in the site's `rem`-based CSS. No anti-FOUC handling needed:
  the default (`medium`) renders identically to no attribute at all.

## Working rules

- Every route is prerendered. `src/routes/+layout.ts` sets
  `prerender = true` and `trailingSlash = 'always'`; internal links must
  therefore end in `/`.
- Adding a route means four edits: the `+page.svelte`, a `+page.ts` beside it
  (below), the `navLinks` array in `src/routes/+layout.svelte`, and a row in
  `static/sitemap.xml`. Add it to the `PAGES` array in `tests/site.spec.ts`
  too.
- Exactly one `<h1>` per page, inside `.page-header` (or `.hero` on the home
  page). Section headings use Lily's `SectionHeading`.
- **The page title is set once, in `+page.ts`.** Every route's `+page.ts`
  exports a `load` returning `{ title: 'X — schematron' }` — see
  `src/app.d.ts`'s `App.PageData`, which makes `title` a type error to
  forget. `+page.svelte`'s `<svelte:head>` reads it back with
  `<title>{data.title}</title>` (needs `let { data }: { data: PageData } =
  $props();`, imported from `./$types`) rather than repeating the string, and
  `+layout.svelte` reads `page.data.title` to pass to `SharePicker` — so a
  shared page's mailto subject and native-share-sheet title name the actual
  page, not a fixed site name. Don't hardcode a `<title>` string directly in
  a `+page.svelte` again; that's the drift this convention exists to
  prevent. `<meta name="description">` stays page-local in `+page.svelte`,
  not part of this convention.
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
