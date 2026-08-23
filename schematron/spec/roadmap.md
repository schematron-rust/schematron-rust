# Roadmap

## Shipped

- Pure Rust XML parser and XPath data model, with no external entity
  resolution and therefore no XXE
- Complete XPath 1.0 engine: all axes, all 27 core functions, XPath 1.0
  comparison and conversion semantics
- Schematron model, parser, include resolution, abstract pattern and abstract
  rule expansion
- Validation with first-matching-rule-wins, phases, four `let` scopes,
  diagnostics, properties, subjects, flags, roles
- SVRL, JSON, and human-readable text reports
- CLI with phase selection, output formats, flag filtering, exit codes
- Cross-document node-sets and XPath `document()`, with loading driven by the
  resolver and costing nothing for schemas that do not use it
- Schema linting: the mistakes the model makes easy, caught without a document
- Opt-in parallel pattern evaluation, with a report identical to the
  sequential one
- XPath 2.0 phase 1: the `xslt2` and `xpath2` bindings, regular expressions,
  conditionals, and the string and numeric functions that need no sequences
- XPath 2.0 phase 2a: the sequence type, and with it sequence construction,
  ranges, `for`, `some`, `every`, `tokenize()`, `distinct-values()` and
  `index-of()`
- XPath 2.0 phase 2b: the date, dateTime and time types, the `xs:` constructors
  and component accessors, and a clock that is captured once per run and can
  be supplied
- XPath 2.0 phase 2c: the value comparisons `eq`, `ne`, `lt`, `le`, `gt` and
  `ge`, which compare exactly two values and report when they cannot
- XPath 2.0 phase 2d: the `xs:dayTimeDuration` and `xs:yearMonthDuration`
  types, and the date arithmetic that produces and consumes them
- XPath 2.0 phase 2e: the node comparisons `is`, `<<` and `>>`; duration
  scaling; and a configurable implicit timezone, with `timezone-from-*`
- XPath 2.0 phase 3: the type operators `instance of`, `castable as`,
  `cast as` and `treat as`, with the sequence types they take
- Keys: `<sch:key>` and `key()`, turning a quadratic cross-reference check
  into a linear one
- Static variable checking: a misspelled `$name` fails when the schema loads,
  rather than aborting a validation part-way through
- SVRL reading, making the format bidirectional, and with it a round-trip
  check over every corpus case
- Differential testing against the ISO reference implementation: every corpus
  case the reference can run agrees exactly, and each documented divergence
  names its cause — the test itself reports the tally, so no count is
  repeated here to go stale
- Five more lints: unused variables, empty rules, empty patterns, duplicate
  assertion tests, and phases that activate nothing
- Rule shadowing generalised from three special cases to pairwise subsumption,
  which also removed a false positive
- `extends href`, and fragment identifiers on both it and `include`
- `document(uri, base)`, closing the last ISO gap: **every element of
  ISO/IEC 19757-3 is now implemented** under the XPath 1.0 binding
- XPath 2.0 kind tests as path node tests — `element()`, `attribute(id)`,
  `document-node()` — which turned out to be separable from the numeric
  hierarchy that keeps the rest of phase 4 last
- `--portability`: constructs that behave differently under other processors,
  each backed by a divergence established by running both
- A denial of service in nested ranges and `for` loops, found by fuzzing: a
  limit on one range cannot see that nesting multiplies
- Three optimisations found by profiling — linear location building, rule
  claims in a vector rather than a hash map, and a fused walk for the common
  rule context
- Generated differential testing: schema and document pairs drawn from a
  grammar and compared against the reference. It found two real XPath 1.0
  conformance bugs — node-set-to-boolean comparison, and `sum()` of an empty
  node-set returning negative zero — and two divergences where the reference
  is the one in the wrong, one of them a libxslt defect its own XPath engine
  contradicts. Generating comments and CDATA also turned up a parser that
  accepted `<!-- a -- b -->`, which XML forbids. The compiler passes —
  `extends`, `is-a` with `param`, phases, schema-level `let` — were generated
  too and agree throughout. Deepening the comparison to `@flag`, `@role` and
  diagnostic messages then found an SVRL *reader* that could not read the
  reference's own diagnostics, and comparing locations by resolving them
  found two more reference defects and a `@subject` case the reference
  documents but does not implement
- Fuzz targets, criterion benchmarks, clippy pedantic, corpus test suite,
  runnable examples, and this specification

## Next

Ordered by value, not by how the XPath 2.0 phases happened to be numbered.

1. **Streaming validation** — for patterns whose rules only need the subtree
   rooted at the context node, validate without materialising the whole
   document.

   **Narrower than it looks.** It pays off only when *every* active pattern is
   subtree-local: one `//`, one `key()`, one `ancestor::` forces the whole tree
   to be materialised anyway. Cross-node constraints are precisely what
   Schematron exists for, so most real schemas would fall back. Weighed against
   reworking the arena and `NodeId` model the entire engine rests on, that is a
   poor trade until someone has a document it actually blocks.
2. **`no_std` core** — **blocked, and the earlier reasoning here was wrong.**
   The claim was that only I/O and the resolver need `std`. They are not the
   obstacle: `quick-xml`, which this crate's XML parser is built on, declares
   no `no_std` support and reaches for `std::io` and `std::error` throughout.
   Nothing in the tree can be parsed without it, so a `no_std` build would
   validate nothing.

   `regex`, the dependency that looked likelier to block it, turns out to be
   `#![no_std]` already and would only need its `std` feature dropped.

   So the real price is a hand-written XML tokenizer replacing a
   heavily-fuzzed one — a correctness risk taken *in a validator*, whose
   whole value is being right — bought for a target that WASM, the plausible
   use case, already reaches with `std`. Not worth it on today's evidence.
3. **XPath 2.0 phase 4: the numeric hierarchy** — tracking whether a number
   arrived as `xs:integer`, `xs:decimal`, `xs:float` or `xs:double`, rather
   than holding every number as a double, which is what would make
   `1 instance of xs:integer` true, and closing the semantic divergences
   phase 1 documents.

   Kind tests as path node tests — `a/element()` — were part of this item and
   are **done**: they turned out to be orthogonal to the numeric hierarchy,
   needing only a node test rather than a type lattice, so they carried none
   of the risk that keeps the rest of phase 4 last.

   **Deliberately last.** It is the only remaining gap [xpath2.md](xpath2.md)
   records, and it is also the one worth least: a schema inspects untyped
   document data, where `castable as xs:integer` already gives the right
   answer, and the distinction between integer and double rarely decides
   anything. Against that, threading a numeric type lattice underneath every
   value would put the exactness of the XPath 1.0 arithmetic at risk — the
   crate's most-exercised code path and an invariant in
   `agents/invariants.md`.

## Examined and abandoned

Recorded so they are not proposed again.

- **A lint for a context that can never match the schema's own vocabulary.**
  There is no vocabulary to check against. Schematron declares no element
  names of its own; it is layered on a grammar — a DTD, XML Schema, RELAX NG —
  that it cannot see. Inferring one from the names the schema happens to
  mention is circular: a context naming an element no test mentions is
  entirely normal. This would need the grammar as a second input, which is a
  different feature.
- **A lint for a `report` whose message reads like a requirement.** Confusing
  `assert` with `report` is the classic Schematron mistake, and catching it
  would be valuable. But the only available signal is the wording of English
  prose, and a lint that misfires on "this invoice must be reviewed manually"
  teaches its reader to ignore the linter — which [linting.md](linting.md)
  argues is the one outcome worth avoiding above all.

## Not planned

- **Compiling to XSLT.** That is the reference implementation's approach and
  the thing this crate exists to avoid.
- **FFI bindings to libxml2.** Same reason.
- **A general-purpose XSLT processor.** Out of scope; use the XPath engine.
