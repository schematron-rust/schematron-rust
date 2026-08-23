# Local Lily stand-ins

Components that exist in the Lily Design System upstream repository but are not
in the published `lily-design-system-svelte-headless@0.2.0` package.

Each file is a faithful copy of the upstream component: same props, same
markup, same class hook, and — like every Lily component — no CSS. Styling
lives in `static/assets/style.css` alongside every other Lily hook.

When a release of `lily-design-system-svelte-headless` ships these, delete the
file here and change the import path in the pages that use it. Nothing else
needs to change.

| File | Upstream path |
|---|---|
| `SectionHeading.svelte` | `components/SectionHeading/SectionHeading.svelte` |
| `CodeBlock.svelte` | `components/CodeBlock/CodeBlock.svelte` |
| `TableHead.svelte` | `components/TableHead/TableHead.svelte` |
| `TableBody.svelte` | `components/TableBody/TableBody.svelte` |
| `TableRow.svelte` | `components/TableRow/TableRow.svelte` |
| `TableTH.svelte` | `components/TableTH/TableTH.svelte` |
| `TableTD.svelte` | `components/TableTD/TableTD.svelte` |
