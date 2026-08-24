# The agents directory is named `agents`, lowercase

AI agent directories in this repository are named **`agents`**, in lowercase.

```
agents/            <- the directory: lowercase
  architecture.md
  conventions.md
  invariants.md
  tasks.md
  testing.md
```

## Directories are lowercase; the entry-point files are not

The rule applies to **directories**. It does not apply to the top-level
Markdown files that tools look for by exact name:

| Path | Case | Why |
|---|---|---|
| `agents/` | lowercase | A directory, and every other directory here is lowercase |
| `AGENTS.md` | uppercase | The cross-tool convention for the file agents read first |
| `CLAUDE.md` | uppercase | The name Claude Code looks for |
| `README.md` | uppercase | Long-standing convention |
| `spec/`, `src/`, `tests/`, `benches/`, `examples/`, `fuzz/` | lowercase | Same rule |

So `AGENTS.md` points into `agents/`, and the two differ in case on purpose.

## Why lowercase

**Consistency.** Every other directory in this repository is lowercase. An
uppercase `AGENTS/` sitting beside `spec/` and `src/` is an inconsistency a
reader has to remember rather than derive.

**Portability.** macOS and Windows filesystems are case-insensitive by
default; Linux is not. A directory whose name differs in case between a
reference and the filesystem works on a developer's laptop and fails in CI —
and the rename itself needs two steps on a case-insensitive filesystem,
because `mv AGENTS agents` is a no-op there. Lowercase everywhere removes the
whole class of problem.

**Shouting is for entry points.** `README.md` and `AGENTS.md` are uppercase to
stand out at the top of a listing, which is the point of them. A directory
does not need to shout; it needs to be findable and typable.

## Applying it

- New agent documentation goes in `agents/`.
- Links from `AGENTS.md`, `CLAUDE.md`, `index.md`, and `README.md` use
  `agents/…`.
- Anything walking the tree — including `tests/docs.rs`, which reads the
  directory to check documentation size, links, and orphans — uses the
  lowercase name.

## Related

- [index.md](../index.md) — the specification map
- [`AGENTS.md`](../../AGENTS.md) — the agent entry point, which links into `agents/`
