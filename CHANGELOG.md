# Changelog

Releases of the `schematron` crate. Earlier entries than 0.4.0 are in the
git history; this file starts where the first output-affecting change did.

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
