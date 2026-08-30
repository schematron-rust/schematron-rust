# Conformance and limits

What this crate does and does not implement, stated plainly, so that nobody
has to discover a gap by hitting it in production.

## Query language bindings

| `queryBinding` | Support |
|---|---|
| absent | Yes — XPath 1.0, the standard's default |
| `xslt` | Yes — XPath 1.0 plus `document()` and `current()` |
| `xpath` | Yes — XPath 1.0 |
| `xslt2`, `xpath2` | Partly — phases 1 through 4 of the subset in [xpath2/](../xpath2/index.md) |
| `xslt3`, `xpath3`, `xpath31` | No — rejected, unless `allow_unknown_query_binding` |

**Read [xpath2/](../xpath2/index.md) before declaring `xslt2`.** The subset covers
the function library, conditionals, sequences (`for`, `some`, `every`,
ranges), date and time types, value comparisons (`eq`/`ne`/`lt`/…), durations,
node comparisons, an implicit timezone, the type operators (`instance of`,
`cast as`, `castable as`, `treat as`), and the numeric type hierarchy
(`xs:integer`/`xs:decimal`/`xs:float`/`xs:double`); everything outside it is a
hard error naming the construct. But expressions still evaluate on the XPath
1.0 engine, so the handful of places where the two languages genuinely
disagree — chiefly, where XPath 2.0 raises a type error and XPath 1.0 yields
`NaN` — follow 1.0. That document lists them.

XPath 3.0 and later add more than this crate implements, so they stay refused:
accepting them would overclaim. `allow_unknown_query_binding` compiles such a
schema anyway, treating it as XPath 1.0 — it does **not** grant the XPath 2.0
subset, so a 2.0 construct in a forced 3.x schema is still an error.

Practically, under an XPath 1.0 binding, expressions like `current-date()`,
`if/then/else`, `for $x in ...`, `castable as`, and `matches()` are not
available; under an XPath 2.0 binding, all of them are. The error message
names the construct and says that it belongs to XPath 2.0, rather than merely
reporting an unknown function.

## Schematron elements

| Element | Status |
|---|---|
| `schema`, `pattern`, `rule`, `assert`, `report` | Full |
| `ns`, `let`, `phase`, `active`, `include`, `extends` | Full |
| `diagnostics`, `diagnostic` | Full |
| `properties`, `property` | Full |
| `key` | Full, as a **non-ISO extension** — see [keys/](../keys/index.md) |
| `param`, abstract patterns (`abstract`, `is-a`) | Full |
| abstract rules (`rule/@abstract` + `extends`) | Full |
| `title`, `p`, `emph`, `span`, `dir` | Full |
| `value-of`, `name` | Full |
| `extends rule="ID"` | Full, including transitive extension |
| `extends href="URI"` | Full, including fragment identifiers |
| `include`/`extends` `href="U#id"` | Full — `@id` or `@xml:id`, no DTD needed |
| XPath `document(uri)` | Full, including cross-document node-sets — see [xpath/](../xpath/index.md) |
| XPath `document(uri, base)` | Full — the URI resolves against the base URI of the second argument's first node |
| `pattern/@documents` | Full; the expression's context node is the **root node**, per the ISO XSLT skeleton, so write `catalog/ref/@href`, not `ref/@href` |
| `schema/@defaultPhase`, `#ALL`, `#DEFAULT` | Full |
| `@flag`, `@role`, `@subject`, `@see`, `@icon`, `@fpi` | Full |
| `@xml:lang`, `@xml:space` | Parsed and carried; no processor behaviour |

## Schematron 1.5 compatibility

The legacy namespace `http://www.ascc.net/xml/schematron` is accepted, so a
1.5-era schema whose vocabulary is otherwise ISO-compatible compiles and runs
unchanged.

`<sch:key>` **is** implemented, together with the `key()` function — as an
extension, because ISO/IEC 19757-3 dropped the element while leaving no other
way to declare a key. See [keys/](../keys/index.md), which states the portability
trade. 1.5's `pattern/@name` is not mapped onto `title`; use ISO spellings.

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
| A single `to` range | 1,000,000 items | A range is materialised |
| Nested ranges and `for`/`some`/`every` together | 10,000,000 items | Each may be within the limit while the product is not: three ranges of 999 ask for close to a billion |
| XML element nesting | 1024 | Same |
| Include, `@documents`, and `document()` fetches | Resolver-controlled | Disk and network access is opt-in, never implicit |
| `document()` loading passes | 8 | A schema deriving each URI from the document it just loaded |

Each returns an error when exceeded — never a panic and never a crash. Four
`cargo-fuzz` targets exist to keep that true; see [testing/](../testing/index.md).

## Security posture

- **No external entity resolution, ever.** The parser does not process DTD
  entity declarations at all, so XXE is structurally impossible rather than
  merely disabled. A reference to a DTD-declared entity is an error, not a
  silent empty expansion.
- **No implicit network access.** The default resolver reads local files and
  refuses `http:` and `https:` URIs with a message saying so. An application
  that wants network fetches supplies its own `Resolver` and owns that choice.
- **No unsafe code.** The crate is `#![forbid(unsafe_code)]`.

## Measured against the reference implementation

The crate is compared against the ISO Schematron reference implementation —
the XSLT stylesheets that compile a schema into a validator — over the whole
corpus in [testing/](../testing/index.md). The comparison is of *findings*: for each,
whether it is a failed assertion or a successful report, the test that
produced it, and the message.

**Twenty of twenty-three cases agree exactly.** The other three are below, and
in both directions the difference is the reference's, not this crate's.

### Rules on text, comments and processing instructions

The reference generates a template for a `context="text()"` rule and then
never visits a text node: its traversal recurses with `select="@*|*"`, so it
walks elements and attributes only. A rule written against `text()`,
`comment()` or `processing-instruction()` therefore cannot fire, silently.

This crate visits all seven node kinds, so such a rule works. See
[validation/](../validation/index.md), which documents the visiting order.

### A `let` that shadows another

The reference compiles every `let` into an `xsl:variable` in a single scope,
so a schema that binds the same name at two scopes is an XSLT error and does
not run at all. Two corpus cases hit this.

The standard describes four nested scopes — schema, phase, pattern, rule —
with an inner binding shadowing an outer one, and this crate implements that.
See [validation/](../validation/index.md).

### What is not compared

The `@location` attribute, textually. Both implementations emit an absolute
XPath 1.0 path, in different but equally valid syntax — `/list/item` against
`/list[1]/item[2]` — and SVRL prescribes neither. Comparing the strings would
report a difference on every finding and bury the real ones.

Comparing them *semantically* — resolving each and checking it picks the same
node — would be worth doing, and is the natural next step, but it would have
to record one reference-side defect first: **the reference emits `/@x` as the
location of every attribute**, whatever element carries it. That is not a
namespace artifact; it happens in a document with no namespaces at all, every
attribute in a document gets the identical location, and libxml2 itself
resolves `/@x` to zero nodes. A finding whose location cannot be resolved
does not do the one job a location has.

This crate emits `/root[1]/c[1]/@x`, and every location in the corpus is
checked to select exactly one node — see [validation/](../validation/index.md).

## Known divergences

1. **Rule context patterns** are matched by the rooted `//` reduction
   described in [validation/](../validation/index.md), not by a dedicated XSLT
   pattern matcher. For the pattern subset Schematron schemas use, the results
   agree. A context pattern with a leading reverse axis is rejected rather
   than guessed at.
2. **`value-of` on a node-set** uses the first node in document order, matching
   XSLT 1.0's `xsl:value-of`. Some XSLT 2.0-based Schematron implementations
   concatenate all nodes instead. If you depend on that, select explicitly.
3. **Report order** is pattern order, then document order of matched nodes,
   then assertion order within a rule. The standard does not mandate an order;
   this one is deterministic and matches the reference implementation.
4. **The `following` axis taken from an attribute node** includes the
   attribute's own element's children. XPath 1.0 orders an element's
   attributes after the element and before its children, and defines
   `following` as everything after the context node in document order barring
   the *context node's* descendants — and an attribute has none. So for
   `<e x="1"><a/><a/></e><a/>`, `@x/following::a` selects three nodes here.

   The ISO reference implementation selects one: libxml2 answers as though
   the attribute stood where its element stands. This crate is not guessing —
   Java's XPath engine, an independent implementation, gives three as well,
   which puts the reference in the minority rather than in the right. Pinned
   by `tests/corpus/following-axis-from-attribute/` and listed in the
   differential test's `KNOWN_DIVERGENCES`, so if libxml2 changes, the test
   says so.

   `preceding` from an attribute agrees everywhere, because an attribute's
   element is its ancestor and ancestors are excluded from `preceding` by
   every reading.
5. **A rule context of `@x` does not claim `@p:x`.** An unprefixed name test
   names the no-namespace name, for attributes exactly as for elements.

   The reference disagrees, and here it contradicts itself: the ISO
   stylesheets compile the context to `match="@x"`, and libxslt's template
   matcher then matches namespaced attributes of the same local name — while
   libxml2's XPath, the same library, counts `//@x` correctly as excluding
   them. Java's XPath agrees with libxml2. So a schema with rules on both
   `@x` and `@p:x` gets the first rule claiming everything under the
   reference, and correct behaviour here.

   Elements are unaffected in both, and `@p:x` correctly matches only itself.
   Pinned by `tests/corpus/namespaced-attribute-context/`.
6. **`@flag` and `@role` are inherited from the rule** by an assertion that
   sets neither. The reference emits neither on such a finding.

   The standard does not state inheritance either way, so this is a reading
   rather than a defect on either side — but it is not an arbitrary one. The
   ISO grammar permits `rule/@flag`, and under the reference that attribute
   has no observable effect whatsoever: it reaches neither the finding nor the
   `fired-rule` event. A permitted attribute that does nothing is the weaker
   reading. Flags exist to classify findings for filtering — `--flag error` is
   the point of them — and a rule saying "everything I match is a warning" is
   the natural way to write that.

   An assertion's own `@flag` or `@role` always wins over the rule's, in both
   implementations. Pinned by `tests/corpus/rich-metadata/`.
7. **`@subject` moves the reported location** to the node the assertion is
   about, rather than the rule's context node. The reference reports the
   context node.

   Its own source settles this one. The skeleton's `linkableParms` template
   says:

   > ISO SVRL does not have a subject attribute to match the Schematron
   > subject attribute. Instead, the Schematron subject attribute is folded
   > into the location attribute

   and then makes no use of the `$subject` parameter it has just declared. The
   reference states this crate's behaviour as the intended one and does not
   carry it out. Pinned by `tests/corpus/subject/`.
8. **A location counts position within its own namespace.** The reference
   counts among siblings sharing the *local name* alone, while emitting a
   predicate that filters on namespace too — so the number and the node set
   it indexes disagree.

   For `<root><a/><p:a/><a/><p:a/></root>` the reference reports the first
   `p:a` as `…[2]`, which resolves to the *second* one, and the second as
   `…[4]`, which resolves to nothing. Checked by resolving its own output with
   libxml2. This crate reports `[1]` and `[2]`.

   Pinned by `tests/corpus/namespaced-sibling-position/`. Because of this the
   differential test does not compare locations naming a namespaced element;
   it still requires this crate's own location to resolve to exactly one node
   in every case.
9. **Whitespace between two inline elements in a message is preserved.** A
   message is mixed content, and the text between `<name/>` and `<emph>` is
   character data like any other.

   The reference loses it, and structurally cannot do otherwise: the validator
   it generates is itself an XSLT stylesheet, and XSLT 1.0 strips
   whitespace-only text nodes from a stylesheet. So `<name/> <emph>e</emph>`
   reports `ae` there and `a e` here. Text with any non-whitespace content —
   `<name/> and <emph>e</emph>` — is preserved by both, which is why this
   almost never shows up in a real schema.

   Pinned by `tests/corpus/message-inline-whitespace/`.
