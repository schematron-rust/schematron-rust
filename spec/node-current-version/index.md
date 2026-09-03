# Node current version

Node.js® is a free, open-source, cross-platform JavaScript runtime environment that lets developers create servers, web apps, command line tools and scripts.

Node current version is 26.

- Any `.github/workflows/*.yml` file containing `node-version` — `deploy.yml`
  in the site directory and the root `ci.yml`'s site job are the two that
  exist today, and the second one drifted to a stale value once already
  because only the first was named here — set `node-version: 26`, then
  verify green (pnpm ci + pnpm run build success).
- If file `package.json` exists, then set: engines.node: "=26", then verify green locally.
- If file `.npmrc` exists, then set `engine-strict=true` so a gap can't occur, then verify failure (EBADENGINE) under node version 25, then verify success under node version 26.
- If file `.nvmrc` exists, then pin the exact local dev version, scoped to this project only.
- If file `.tool-versions` exists, then pin the exact local dev version, scoped to this project only.
