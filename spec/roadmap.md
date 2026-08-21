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
- Fuzz targets, criterion benchmarks, clippy pedantic, corpus test suite,
  runnable examples, and this specification

## Next

1. **XPath 2.0 phase 3: the type system** — `instance of`, `cast as`,
   `castable as`, `treat as`, and the sequence types those need
   (`element()`, `item()*`, `node()?`). With a type system, the `adjust-*`
   functions and the general `xs:duration` follow, and so does closing the
   semantic divergences phase 1 documents — those exist only because shared
   constructs still evaluate on the XPath 1.0 engine.
2. **XSLT key emulation** — `<sch:key>` and the `key()` function, which some
   1.5-era schemas and some large industry schemas rely on for cross-reference
   checks that are otherwise quadratic.
3. **Streaming validation** — for patterns whose rules only need the subtree
   rooted at the context node, validate without materialising the whole
   document. Large-document memory is the main scaling limit today.
4. **More lints** — the current set is in [linting.md](linting.md). Candidates
   that need more than a syntactic check: a context that can never match the
   schema's own vocabulary, a `report` whose message reads like an assertion,
   and subsumption beyond the three certain cases.
5. **SVRL input** — parse an existing SVRL report back into a `Report`, so
   reports from other tools can be diffed against this crate's.
6. **`extends href`** — the rule-level counterpart of `include`. Low value
   while `include` can splice a `rule` element just as well, but it is in the
   standard.
7. **`no_std` core** — the XPath engine and validator have no intrinsic need
   for `std`; only I/O and the resolver do.

## Not planned

- **Compiling to XSLT.** That is the reference implementation's approach and
  the thing this crate exists to avoid.
- **FFI bindings to libxml2.** Same reason.
- **A general-purpose XSLT processor.** Out of scope; use the XPath engine.
