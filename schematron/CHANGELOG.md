# Changelog

Releases of the `schematron` crate. Earlier entries than 0.4.0 are in the
git history; this file starts where the first output-affecting change did.

## 0.5.0

### Changed

- **`Documents::insert`, `lookup` and `missing` take a base URI.** A request
  for a document is now the pair of the URI as written and what to resolve it
  against, because `document('a.xml')` and `document('a.xml', $node)` may name
  different files. Only affects code driving the XPath engine directly; a
  schema is unaffected.

### Added

- **`document(uri, base)`**, the two-argument form. A relative URI resolves
  against the base URI of the second argument's first node, per XSLT 1.0
  section 12.1, so a document that has itself been loaded can name its own
  neighbours. This closes the last gap against ISO/IEC 19757-3: every element
  of the standard is now implemented under the XPath 1.0 binding.
- **XPath 2.0 kind tests as path node tests** — `element()`, `element(name)`,
  `attribute()`, `attribute(id)`, `document-node()`. A kind test names the
  kind outright, so unlike `*` it does not depend on the axis. A step whose
  test is an attribute kind test defaults to the attribute axis, per XPath 2.0
  section 3.2.1.1. Under an XPath 1.0 binding these are refused by name.
- **`--portability`**, and `Schema::portability()`: constructs that behave
  differently under other Schematron processors. These are **not mistakes** —
  they are correct, and this crate implements them as the standard describes —
  so they are kept out of `--lint`, which exists to report likely errors. Each
  of the seven checks is backed by a divergence in
  [spec/conformance.md](spec/conformance.md), established by running this
  crate and the ISO reference implementation against the same schema.

### Fixed

- **A denial of service in XPath 2.0 ranges and loops.** A limit on a single
  `to` range cannot see that nesting multiplies: each range in
  `for $i in 1 to 999 return for $j in 1 to 999 return for $k in 1 to 999
  return $k` is well inside it, and together they ask for close to a billion
  items — from a 90-byte expression. Found by fuzzing. A budget is now shared
  across every nested construct in one expression, so the product is bounded;
  both limits are in [spec/conformance.md](spec/conformance.md).

### Performance

Each of these was found by profiling and is covered by a benchmark.

- **Building a report location is linear in the node's depth**, not quadratic.
  It recursed to the root and re-copied the whole ancestor prefix at every
  level, and a finding pays that once per location: 11.5 ms to 2.1 ms for 300
  findings on a 300-deep document, and 23–33% on flat ones.
- **Rule claims are a vector indexed by node**, not a map keyed by one.
  `NodeId` is a dense arena index, so hashing it bought nothing: SipHash and
  its `RandomState` were above the XPath evaluation itself in the profile.
  10–19% off validation at every document size.
- **A rule context of a bare name or wildcard takes a fused walk.** Evaluated
  generically it materialises every node in the document and then filters,
  once per rule. Per-rule cost on a 20,000-element document fell from 1.33 ms
  to 0.26 ms. A debug assertion compares the fast path against the evaluator
  on every rule context, so the test suite and every generated differential
  case check that the two agree.

## 0.4.0

### Changed

Two changes alter what you see for a schema that worked before.

- **Report locations are now valid XPath 1.0.** They were written
  `/*:invoice[1]/*:line[3]`, which uses the XPath 2.0 `*:local` wildcard —
  syntax an XPath 1.0 engine rejects outright, and SVRL's consumers are XPath
  1.0 engines. A location a consumer cannot evaluate cannot do the one job it
  has. Names in no namespace are now written plainly, `/invoice[1]/line[3]`,
  and namespaced names as
  `*[local-name()='line' and namespace-uri()='urn:example'][3]`, which needs
  no prefix bound by the reader. This affects both the text output and SVRL's
  `@location`. See [spec/validation.md](spec/validation.md).

- **A misspelled variable is an error when the schema loads**, rather than
  when the expression using it is first evaluated. `$naem` for `$name` used to
  abort a validation part-way through; it now fails at compile time, before
  any document is read.

### Added

- `<sch:key>` and the `key()` function, turning a quadratic cross-reference
  check into a linear one. A non-ISO extension — see [spec/keys.md](spec/keys.md).
- `Report::from_svrl`, making SVRL bidirectional: reports can be read back,
  not only written.
- `extends href`, and fragment identifiers on both it and `include`:
  `lib.sch#dates` selects one element, `#dates` one from the document being
  read. `include` splices the element, `extends` its children.
- Six lints: unreferenced keys, unused variables, rules with no assertions,
  patterns with no rules, duplicate assertion tests, and phases that activate
  nothing. See [spec/linting.md](spec/linting.md).

### Fixed

- A node-set compared to a boolean was evaluated existentially like the string
  and number cases. XPath 1.0 section 3.4 requires converting the node-set with
  `boolean()`, so `missing >= false()` is true — `0 >= 0` — where this crate
  said false.
- `sum()` over an empty node-set returned negative zero, so `1 div sum(none)`
  was `-Infinity` instead of `Infinity`. Rust's `Sum` for `f64` starts from
  `-0.0`, which is the correct additive identity and the wrong answer here.
- Comments containing `--`, and comments ending `--->`, were accepted. XML 1.0
  section 2.5 forbids both, and every other XML tool rejects them.
- `Report::from_svrl` silently lost diagnostics written as bare character data
  rather than a nested `svrl:text` — which is the shape the ISO reference
  implementation writes.
- The rule-shadowing lint reported a false positive: a rule on `@*` was treated
  as claiming every node, so an element rule after it was called unreachable
  although both fire. Shadowing is now decided by pairwise subsumption, which
  also catches cases the old check missed, such as `a` before `a[@x]`.

### Testing

- Differential testing against the ISO reference implementation, both over the
  curated corpus and over generated schema and document pairs. It found the
  first two fixes above, and the divergences it turned up are recorded in
  [spec/conformance.md](spec/conformance.md) — several of them cases where the
  reference is the one in the wrong. See [spec/testing.md](spec/testing.md).
