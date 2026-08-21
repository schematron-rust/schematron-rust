# Conformance and limits

What this crate does and does not implement, stated plainly, so that nobody
has to discover a gap by hitting it in production.

## Query language bindings

| `queryBinding` | Support |
|---|---|
| absent | Yes — XPath 1.0, the standard's default |
| `xslt` | Yes — XPath 1.0 plus `document()` and `current()` |
| `xpath` | Yes — XPath 1.0 |
| `xslt2`, `xpath2` | Partly — the phase-1 subset in [xpath2.md](xpath2.md) |
| `xslt3`, `xpath3`, `xpath31` | No — rejected, unless `allow_unknown_query_binding` |

**Read [xpath2.md](xpath2.md) before declaring `xslt2`.** The subset covers
regular expressions, conditionals, and the string and numeric functions that
schemas actually use; everything outside it is a hard error naming the
construct. But expressions still evaluate on the XPath 1.0 engine, so the
handful of places where the two languages genuinely disagree — chiefly, where
XPath 2.0 raises a type error and XPath 1.0 yields `NaN` — follow 1.0. That
document lists them.

XPath 3.0 and later add more than this crate implements, so they stay refused:
accepting them would overclaim. `allow_unknown_query_binding` compiles such a
schema anyway, treating it as XPath 1.0 — it does **not** grant the XPath 2.0
subset, so a 2.0 construct in a forced 3.x schema is still an error.

Practically, under an XPath 1.0 binding, expressions like `current-date()`,
`if/then/else`, `for $x in ...`, `castable as`, and `matches()` are not
available; under an XPath 2.0 binding, `if/then/else` and `matches()` are, and
the rest are not. The error
message names the construct and says that it belongs to XPath 2.0, rather than
merely reporting an unknown function.

## Schematron elements

| Element | Status |
|---|---|
| `schema`, `pattern`, `rule`, `assert`, `report` | Full |
| `ns`, `let`, `phase`, `active`, `include`, `extends` | Full |
| `diagnostics`, `diagnostic` | Full |
| `properties`, `property` | Full |
| `param`, abstract patterns (`abstract`, `is-a`) | Full |
| abstract rules (`rule/@abstract` + `extends`) | Full |
| `title`, `p`, `emph`, `span`, `dir` | Full |
| `value-of`, `name` | Full |
| `extends rule="ID"` | Full, including transitive extension |
| `extends href="URI"` | **Not implemented** — use `include` instead |
| XPath `document(uri)` | Full, including cross-document node-sets — see [xpath.md](xpath.md) |
| XPath `document(uri, base)` | **Not implemented** — URIs resolve against the instance |
| `pattern/@documents` | Full; the expression's context node is the **root node**, per the ISO XSLT skeleton, so write `catalog/ref/@href`, not `ref/@href` |
| `schema/@defaultPhase`, `#ALL`, `#DEFAULT` | Full |
| `@flag`, `@role`, `@subject`, `@see`, `@icon`, `@fpi` | Full |
| `@xml:lang`, `@xml:space` | Parsed and carried; no processor behaviour |

## Schematron 1.5 compatibility

The legacy namespace `http://www.ascc.net/xml/schematron` is accepted, so a
1.5-era schema whose vocabulary is otherwise ISO-compatible compiles and runs
unchanged.

`<sch:key>` is **not** implemented: it needs XSLT keys, which this crate has
no equivalent for. A schema using it is rejected with a message saying so, and
the same applies to the `key()` function. 1.5's `pattern/@name` is not mapped
onto `title`; use ISO spellings.

## XML support

Supported: namespaces, all seven node kinds, CDATA, comments, processing
instructions, predefined and numeric entities, UTF-8 and UTF-16.

Not supported, by design:

- **External entity resolution.** Never performed. This makes XXE structurally
  impossible rather than merely disabled.
- **DTD-defined entities.** A `<!DOCTYPE>` declaration is skipped; a reference
  to an entity it declares is an error rather than a silent empty expansion.
- **DTD validation and defaulted attributes.** Out of scope; use a validating
  parser first if you need them.
- **`xml:id`.** The XPath `id()` function needs a DTD or schema to know which
  attributes are of type ID. With no DTD processing, `id()` matches `@xml:id`
  and attributes named `id`, which is what every other DTD-less processor does.

## Numeric behaviour

XPath 1.0 numbers are IEEE 754 doubles, and the crate uses `f64` directly, so
`0.1 + 0.2 != 0.3` here exactly as it does in every other conformant engine.
Number-to-string conversion follows XPath 1.0's format, not Rust's `Display`.

## Limits

| Limit | Value | Why |
|---|---|---|
| Include depth | 64, configurable | Cycles and expansion blow-up |
| `extends` chain depth | 64 | Same |
| XPath sub-expression nesting | 64 | Stack exhaustion on hostile input |
| XML element nesting | 1024 | Same |
| Include, `@documents`, and `document()` fetches | Resolver-controlled | Disk and network access is opt-in, never implicit |
| `document()` loading passes | 8 | A schema deriving each URI from the document it just loaded |

Each returns an error when exceeded — never a panic and never a crash. Four
`cargo-fuzz` targets exist to keep that true; see [testing.md](testing.md).

## Security posture

- **No external entity resolution, ever.** The parser does not process DTD
  entity declarations at all, so XXE is structurally impossible rather than
  merely disabled. A reference to a DTD-declared entity is an error, not a
  silent empty expansion.
- **No implicit network access.** The default resolver reads local files and
  refuses `http:` and `https:` URIs with a message saying so. An application
  that wants network fetches supplies its own `Resolver` and owns that choice.
- **No unsafe code.** The crate is `#![forbid(unsafe_code)]`.

## Known divergences

1. **Rule context patterns** are matched by the rooted `//` reduction
   described in [validation.md](validation.md), not by a dedicated XSLT
   pattern matcher. For the pattern subset Schematron schemas use, the results
   agree. A context pattern with a leading reverse axis is rejected rather
   than guessed at.
2. **`value-of` on a node-set** uses the first node in document order, matching
   XSLT 1.0's `xsl:value-of`. Some XSLT 2.0-based Schematron implementations
   concatenate all nodes instead. If you depend on that, select explicitly.
3. **Report order** is pattern order, then document order of matched nodes,
   then assertion order within a rule. The standard does not mandate an order;
   this one is deterministic and matches the reference implementation.
