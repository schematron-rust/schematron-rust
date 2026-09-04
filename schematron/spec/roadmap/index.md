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
  hierarchy phase 4 later added, needing only a node test rather than a
  type lattice
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
- XPath 2.0 phase 4: the numeric hierarchy — `instance of` and its
  companions now recognize `xs:integer`, `xs:decimal` and `xs:float` as well
  as `xs:double`, tracked for numeric literals (lexically: `1` is an
  integer, `1.0` a decimal) and for explicit `cast as`/`castable as`.
  Arithmetic, every function in the library, and `to` ranges are
  deliberately left untracked and stay `xs:double`, which is what keeps the
  XPath 1.0 arithmetic this crate's most-exercised code path — and an
  invariant in `agents/invariants.md` — untouched by a full numeric type
  lattice. This also corrected a pre-existing documentation mistake:
  `1 instance of xs:double` was recorded as agreeing with real XPath 2.0;
  it does not, since `1` is an `xs:integer` and does not derive from
  `xs:double`.
- XPath 2.0 phase 5: the remaining sequence-manipulating functions —
  `reverse()`, `subsequence()`, `insert-before()`, `remove()`, and
  `unordered()`. None were blocked on the sequence type, which phase 2a
  already shipped; they were simply unwritten. Auditing the list turned up
  one that had been filed alongside them by mistake: `for-each()` needs a
  function item, an XPath 3.0 feature this crate does not have, not merely a
  sequence — it moved to the "not implemented" table instead.
- XPath 2.0 phase 6: the cardinality assertions and atomization —
  `zero-or-one()`, `one-or-more()`, `exactly-one()`, and `data()`. The first
  three were an oversight rather than a deliberate gap: unlike every other
  unimplemented XPath 2.0 function, they were not named anywhere the crate
  could say so, and calling one reported "unknown function" — indistinguishable
  from a typo. `data()` atomizes a node to its typed value, which this
  schema-unaware crate always takes to be untyped atomic, represented as a
  plain string like every other untyped value in this engine.
- XPath 2.0 phase 7: `deep-equal()`. Structural equality over sequences:
  atomic items by value (`NaN` deep-equals `NaN`, and a mismatched type is
  simply unequal — both unlike `eq`) and nodes recursively, by kind,
  expanded name, an order-independent attribute set, and same-order
  children. Not atomizing — a node never deep-equals an atomic value, even
  one with an identical string value — is a deliberate choice, not a gap.
- XPath 2.0 phase 8: `resolve-uri()`. RFC 3986 URI-reference resolution,
  implemented by hand (`src/xpath/uri.rs`) rather than adding a dependency,
  and verified against the RFC's own worked examples — both the "normal"
  and "abnormal" sets in §5.4. The one-argument form falls back to the
  document's own base URI, the closest thing this crate has to a query's
  static base URI; it errors, naming what's missing, when the document has
  none. `trace()` — audited alongside it — stays unimplemented: its
  destination is implementation-defined, and this engine has no
  debug-output channel to point it at without a real architecture
  decision, which a single function shouldn't make unilaterally.
- XPath 2.0 phase 9: `adjust-date-to-timezone()`, `adjust-dateTime-to-timezone()`,
  and `adjust-time-to-timezone()`. Previously recorded as needing "a
  timezone-bearing cast, which nothing in the crate currently produces" —
  that turned out to already exist: `Temporal` has carried an optional
  offset since phase 2b. What was missing was the arithmetic, and one
  function serves all three types, because a `Date`'s time-of-day and a
  `Time`'s date are already fixed at their respective canonical values,
  which is exactly the "combine, adjust, extract" recipe F&O specifies for
  those two forms. Verified against the F&O reference material's own
  worked examples, including the one where converting a date's timezone
  rolls it to the adjacent day.
- **XPath 3.0 phase 1: function items.** The `xslt3`/`xpath3` bindings, a
  new `XPathVersion::V3`, and the machinery a real `for-each()` needed all
  along: inline function expressions (`function($x) { … }`, closures —
  genuinely lexical, capturing the environment where written, not where
  called), named function references (`name#arity`), and dynamic calls
  (`$f(1, 2)`, chainable). `for-each()` — misfiled for years as an
  unwritten XPath 2.0 sequence function, corrected in phase 5 — is now
  actually implemented, built on the same primitive a dynamic call uses.
  Function items are not atomizable: `string()`, `number()`, `concat()`,
  and every comparison operator reject one explicitly, naming it, rather
  than silently falling through to an empty string or `NaN`. A closure's
  captured environment is genuinely owned (`Item::Function`'s `Arc<Expr>`
  and cloned `Variables`, not a borrow), so a function item survives being
  passed to `for-each()`, bound to a variable, or crossing the thread
  boundary opt-in parallel pattern evaluation already uses — `Arc`, not
  `Rc`, is why. Not in this phase: `=>`, `||`, `!`, the rest of the
  higher-order function library (`filter`, `fold-left`, `fold-right`,
  `sort`, …), and maps and arrays, which are XPath 3.1 and stay behind the
  still-refused `xpath31`/`xslt31` bindings. See `spec/xpath3/`.
- **XPath 3.0 phase 2: the arrow operator and string concatenation.** `E =>
  f(…)` and `E || E`, the two remaining 3.0 operators that needed no new
  evaluation machinery. The arrow operator is pure sugar — the parser
  prepends the left operand as the target's first argument and builds an
  ordinary `Expr::Function` (a named target) or `Expr::DynamicCall` (`$f`,
  or a parenthesized expression); a new `Expr::Arrow` wraps that only so a
  1.0/2.0 binding can reject `=>` by name rather than accepting whatever
  call it desugars to. `||` atomizes both sides the way `concat()`'s
  arguments already do. The simple map operator `!` did not come with
  them: unlike `for-each()`, whose action binds the mapped-over item to a
  named parameter, `!`'s right side can only see its item through `.`, and
  this crate's `EvalContext` holds a context *node*, not a context *item*
  — approximating it would mean `.` silently pointing at the wrong thing
  whenever the item isn't a node, which is the one failure mode this
  crate refuses to ship. See `spec/xpath3/`.
- A stack overflow in eight repeating binary operators (`or`, `and`, the
  comparisons, `||`, `+`/`-`, `*`/`div`/`mod`, `|`), found by fuzzing:
  each was parsed by a loop that spent none of `MAX_RECURSION_DEPTH`'s
  budget on how long a chain of itself ran, but the tree it built is still
  walked recursively at evaluation time — unlike a location path's steps,
  which are exempt from the same limit because they are walked in a loop
  too. A shared `parse_binary_chain` helper closes the gap for all eight
  at once.
- **XPath 3.0 phase 3: the rest of the higher-order sequence functions,
  and function-item introspection.** `filter`, `fold-left`, `fold-right`,
  and `for-each-pair` are `for-each()`'s siblings, sharing its multiplying
  construct's budget (`SequenceScope`) so a `filter` nested inside a
  `for-each` is bounded the same way two nested `for-each`es already are.
  `function-lookup`, `function-arity`, and `function-name` introspect a
  function item itself. None needed new evaluation machinery beyond what
  phase 1 already built for `for-each()`; `fold-left`/`fold-right` needed
  only threading `$zero` through as a whole `Value` rather than a single
  `Item`, since it need not be one item. Auditing what remained also
  caught a factual error carried since phase 1: `sort` was listed as an
  unwritten XPath 3.0 function, but real F&O 3.0 has no `fn:sort` at all
  — it is new in 3.1, alongside maps and arrays, so it was never a gap in
  this phase to begin with. See `spec/xpath3/`.
- **XPath 3.0 phase 4: the simple map operator, and the foundation it
  needed.** `E1 ! E2` evaluates `E2` once per item of `E1`, with that item
  as the context item — `EvalContext` gained an optional `context_item`
  field beside `node` for the case a node can't represent: an atomic value
  or a function item, resolved only for a bare `.`, read in exactly one
  place (`Expr::Path`'s evaluation). A node item still moves `EvalContext::node`
  exactly the way it always did, via `focus`, which now also clears
  `context_item` so a stale one from an *enclosing* `!` can't leak into
  ordinary node evaluation. `EvalContext` had to give up `Copy` for
  `Clone` to hold it — a small, deliberate cost, paid only when `!` is
  actually mapping over a non-node item, not on the ordinary node-based
  hot path. Fuzzing `!` itself, immediately after writing it, found a real
  OOM: eight nested `!`s compounding roughly 7× per level over a
  seven-node document reached 5.76 million items, comfortably inside
  `MAX_SEQUENCE_WORK`'s budget but with several million-item `Vec`s alive
  at once across the nesting depth — legal and bounded (confirmed by
  raising the fuzzer's own memory ceiling and letting it run to
  completion), not a defect; see `spec/testing/`. Writing up phase 4's own
  documentation then found `let $v := E return E` — new to XPath 3.0's
  *grammar*, not merely its function library the way `sort` was — entirely
  missing from every earlier phase's accounting, and fixed in the same
  sitting: one more branch in the same "name directly followed by `$`"
  dispatch `for`/`some`/`every` already used, needing nothing `for` had
  not already built. The same check also turned up two gaps this crate
  still has, named rather than left implicit: `Q{uri}local` names
  (EQNames) and union types in casts and function signatures. See
  `spec/xpath3/`.
- **EQNames, `Q{uri}local`, for node name tests.** The first of the two
  gaps phase 4 named, closed the same way `let` was: node-name matching
  already resolved a prefix to a URI and compared by URI, so accepting a
  URI directly instead needed no new machinery, just a new lexer token
  (`Q` immediately followed by `{`, unambiguous — no other construct puts
  `{` right after a name character) and one more field on `NameTest`.
  Deliberately not extended to variable names, type names, or function
  references: `Variables` keys a binding by the *lexical* spelling of its
  name, not by resolved URI, so making `$Q{uri}local` interchangeable
  with every prefixed spelling of the same expanded name would mean
  reworking that keying everywhere variables are bound — a
  foundation-level change for a syntax real schemas essentially never
  write, the same trade this file already declines for streaming
  validation and `no_std`. Union types remain the one fully open gap.
  Fuzzing this immediately after writing it — same discipline as every
  construct above — found a real stack-overflow regression, not in
  EQNames but in `NameTest`, the type it grew: `MAX_RECURSION_DEPTH`'s
  margin above the real stack-overflow threshold turned out to be about
  ten levels, and one inline (unboxed) field was enough to spend all of
  it. `MAX_RECURSION_DEPTH` is now 32, roughly half of the last-measured
  danger zone rather than just under it. See `spec/xpath3/` and
  `spec/testing/`.

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
  teaches its reader to ignore the linter — which [linting/](../linting/index.md)
  argues is the one outcome worth avoiding above all.

## Not planned

- **Compiling to XSLT.** That is the reference implementation's approach and
  the thing this crate exists to avoid.
- **FFI bindings to libxml2.** Same reason.
- **A general-purpose XSLT processor.** Out of scope; use the XPath engine.
