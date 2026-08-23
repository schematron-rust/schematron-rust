# schematron-rust.github.io

The public site for the [`schematron`](https://github.com/schematron-rust/schematron-rust)
crate: Schematron in pure Rust, ISO/IEC 19757-3.

Live at <https://schematron-rust.github.io/>.

## What this is

A [SvelteKit](https://kit.svelte.dev/) project using
`@sveltejs/adapter-static`, prerendered to plain HTML and deployed to GitHub
Pages by GitHub Actions on every push to `main`. It documents and explains the
crate; it contains no Rust and implements nothing.

Components come from the [Lily Design System](https://lilydesignsystem.com/)
(`lily-design-system-svelte-headless`): headless Svelte components that render
semantic HTML and correct ARIA, carrying one stable kebab-case class hook each
and shipping no CSS. All styling therefore lives in one file,
`static/assets/style.css`, which targets those hooks.

## Routes

| Route | Covers |
|---|---|
| `/` | What the crate is, the quick start, and the one rule to internalise |
| `/why/` | No C toolchain, security posture, performance, stability |
| `/tutorial/` | A condensed form of `spec/tutorial.md` |
| `/library/` | Using the crate from Rust |
| `/cli/` | Every option and exit code |
| `/conformance/` | What is implemented, and every measured divergence |
| `/spec/` | A map of the crate's normative `spec/` directory |
| `/help/` | Diagnosing a schema that does nothing, and the FAQ |
| `/about/` | Crate facts, contributing, and how this site is built |

## Develop

```sh
pnpm install
pnpm dev        # http://localhost:5173
pnpm build      # prerender to build/
pnpm preview    # serve build/
pnpm check      # svelte-check
pnpm test       # Playwright, against a preview build
```

## Layout

| Path | Contents |
|---|---|
| `src/routes/` | One directory per route; `+layout.svelte` holds the header, nav, and footer |
| `src/lib/site.ts` | Facts copied from the crate: version, MSRV, the `spec/` table of contents |
| `static/assets/style.css` | Every visual decision on the site |
| `static/sitemap.xml` | Hand-maintained; add a row when you add a route |
| `tests/site.spec.ts` | Per-page smoke tests plus an internal link check |
| `.github/workflows/deploy.yml` | Build and deploy to GitHub Pages |

## License

Same terms as the crate: MIT, Apache-2.0, GPL-2.0-only, or GPL-3.0-only, at
your option.
