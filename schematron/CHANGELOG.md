# Changelog

Releases of the `schematron` crate. Earlier entries than 0.4.0 are in the
git history; this file starts where the first output-affecting change did.

## 0.15.0

### Added

- **EQNames, `Q{uri}local`, for node name tests.** `Q{http://example.com/ns}foo`
  names the same expanded name a declared prefix would
  (`<ns prefix="p" uri="http://example.com/ns"/>` plus `p:foo`), without
  needing that prefix declared — usable anywhere a node name test is
  written: `Q{uri}local`, `@Q{uri}local`, `child::Q{uri}local`, and so on.
  `Q{}local` — an empty braced URI literal — means no namespace, the same
  as writing `local` unprefixed. Recognized whatever the query binding, so
  a 1.0/2.0 schema that writes one gets a compile-time refusal naming
  "an EQName," not a confusing parse error about a stray `{`.
  - Deliberately scoped to node name tests, not variable names, type
    names, or function references, which real XPath 3.0 also allows an
    EQName to name. Node matching already resolved a prefix to a URI and
    compared by URI, so accepting a URI directly was a small, contained
    change; `Variables` keys a binding by the lexical spelling of its
    name, not by resolved URI, so making `$Q{uri}local` interchangeable
    with every prefixed spelling of the same expanded name would mean
    reworking that keying everywhere variables are bound — not worth that
    risk for a syntax real schemas essentially never write.

### Fixed

- **A stack-overflow regression in `MAX_RECURSION_DEPTH`'s safety margin.**
  Found by fuzzing immediately after EQNames were written, in `NameTest`
  — the type they extended — not in EQNames themselves: adding one field
  held inline in several `Expr` variants was enough to push the parser's
  already-thin margin above the real stack-overflow threshold negative,
  turning `refuses_absurd_nesting_instead_of_overflowing`'s expected clean
  parse error into a genuine crash. The true threshold on the toolchain
  this was measured on turned out to be in the low seventies, against a
  limit of 64 — a margin of about ten levels that one field consumed
  entirely. `MAX_RECURSION_DEPTH` is now 32, roughly half the
  last-measured danger zone rather than just under it; see its doc
  comment in `src/xpath/parser.rs` for how to re-measure if a future
  change needs to ask the same question again.

See [spec/xpath3/](spec/xpath3/index.md) and
[spec/testing/](spec/testing/index.md#what-fuzzing-found).

## 0.14.0

### Added

- **XPath 3.0 phase 4: the simple map operator, plus `let`.**
  - `E1 ! E2`: evaluates `E2` once per item of `E1`, with that item as the
    context item, and concatenates the results. `EvalContext` gained an
    optional `context_item` field for the one case a node can't represent
    — an atomic value or a function item — read in exactly one place, to
    resolve a bare `.`; anything wanting a real axis from a non-node item
    is a dynamic error naming what the item actually is. `EvalContext`
    gave up `Copy` for `Clone` to hold this, a cost paid only when `!` is
    mapping over a non-node item, not on the ordinary node-based path.
  - `let $v := E return E`: binds `E`'s whole value to `$v` — no
    iteration, unlike `for`. Found as a real gap while writing this
    release's own documentation (new to XPath 3.0's grammar, not its
    function library), not part of the original plan for this release,
    and fixed in the same sitting since it needed nothing `for` had not
    already built.
  - Two further gaps are now named rather than left implicit:
    `Q{uri}local` names (EQNames) and union types in casts and function
    signatures are real XPath 3.0 additions this crate does not implement.

`!` joins the eight repeating binary operators already sharing
`parse_binary_chain` (0.12.0's fix for a stack overflow), so its own chain
(`a ! b ! c`) is bounded the same way theirs are, with no new parser work
needed.

See [spec/xpath3/](spec/xpath3/index.md) for both additions in full, and
[spec/testing/](spec/testing/index.md#what-fuzzing-found) for a
real-looking OOM that fuzzing `!` turned up and that investigation showed
was legal, bounded work under the existing per-expression budget — not a
defect, and not fixed.

## 0.13.0

### Added

- **XPath 3.0 phase 3: the rest of the higher-order sequence functions,
  and function-item introspection.**
  - `filter(sequence, predicate)`: keeps the items `predicate`'s effective
    boolean value holds for.
  - `fold-left(sequence, zero, f)` and `fold-right(sequence, zero, f)`:
    combine a sequence into one value from the left or the right
    respectively — genuinely different, not mirror images written the
    same way: `fold-left`'s running value is `f`'s first parameter,
    `fold-right`'s is its second, and `zero` need not be a single item,
    so it is threaded through as a whole value.
  - `for-each-pair(sequence1, sequence2, f)`: applies `f` to corresponding
    items of both sequences and concatenates the results, stopping at the
    shorter one — a length mismatch is not an error.
  - `function-lookup(name, arity)`, `function-arity(f)`,
    `function-name(f)`: introspect a function item. `function-lookup` is
    the dynamic counterpart to `name#arity` — a computed name checked at
    call time, returning the empty sequence rather than an error when
    nothing matches, since F&O treats a dynamic lookup finding nothing as
    an ordinary outcome to test for, not a broken schema.
  - `filter`, `fold-left`, `fold-right`, and `for-each-pair` are
    multiplying constructs exactly like `for-each()`, and share its
    budget, not one of their own: a `filter` nested inside a `for-each` is
    bounded the same way two nested `for-each`es already are.
  - Not in this phase: the simple map operator `!`, still blocked on the
    same context-item foundation phase 2 recorded. `sort` is not listed
    as a gap at all: real XPath 3.0 F&O has no `fn:sort` — it is new in
    3.1, alongside maps and arrays, and was a factual error inherited
    from phase 1's own accounting, corrected here. See
    [spec/xpath3/](spec/xpath3/index.md).

## 0.12.0

### Added

- **XPath 3.0 phase 2: the arrow operator and string concatenation.**
  - `E => f(…)`: pipes `E` in as `f`'s first argument, ahead of whatever
    arguments are written — `'a-b-c' => tokenize('-')` means
    `tokenize('a-b-c', '-')`. Chainable (`E => f() => g()` means
    `g(f(E))`). The target can be a named function, a variable holding a
    function item (`E => $f(…)`), or a parenthesized expression
    evaluating to one (`E => (expr)(…)`).
  - `E || E`: string concatenation, atomizing each side the way
    `concat()`'s arguments already do, and rejecting a function item
    explicitly rather than silently stringifying it to nothing.
  - Both are pure syntax: `=>` desugars at parse time into an ordinary
    function call or dynamic call (a new `Expr::Arrow` exists only so a
    1.0/2.0 binding can reject the operator by name), and `||` is an
    ordinary `BinaryOp`. Neither needed new evaluation machinery.
  - Not in this phase: the simple map operator `!`. Unlike `for-each()`,
    whose action binds the mapped-over item to a named parameter, `!`'s
    right side can only see its item through `.` — and this crate's
    `EvalContext` holds a context *node*, not a context *item*, so
    implementing it needs that foundation to change first, not one more
    operator. See [spec/xpath3/](spec/xpath3/index.md).

### Fixed

- **A stack overflow in eight repeating binary operators.** `or`, `and`,
  the comparisons, `+`/`-`, `*`/`div`/`mod`, and `|` are each parsed by a
  loop, so a long chain of the same operator — `a|a|a|…` — cost the
  parser's own recursion budget nothing, but still built a left-degenerate
  `Expr::Binary` tree that `evaluate` walks recursively, one stack frame
  per repetition. Found by fuzzing `fuzz_xpath`: a few hundred `|` in a row
  crashed. Every one of the eight now shares a `parse_binary_chain` helper
  that counts a chain the same way a dynamic call's or the new arrow
  operator's own chaining already did, so this is a clean "nested deeper
  than the limit" parse error instead — see
  [spec/testing/](spec/testing/index.md#what-fuzzing-found).

## 0.11.0

### Added

- **XPath 3.0 phase 1: function items.** New `xslt3`/`xpath3` query
  bindings (`XPathVersion::V3`, a superset of `V2` the way `V2` is of
  `V1`). Adds inline function expressions (`function($x) { … }` —
  genuine closures, capturing the environment where written, not where
  called), named function references (`name#arity`), dynamic calls
  (`$f(1, 2)`, chainable), and `for-each($sequence, $action)` — the
  function all of the above exists for, previously misfiled as an
  unwritten XPath 2.0 sequence function.
  - A function item (`Item::Function`, publicly `FunctionItem`) is not
    atomizable: `string()`, `number()`, `concat()`, and every comparison
    operator (`=`, `eq`, `<`, …) reject one explicitly, naming it — not a
    silent empty string or `NaN`.
  - A closure's captured environment is owned, not borrowed
    (`FunctionItem::Inline`'s `Arc<Expr>` body and cloned `Variables`), so
    a function item survives being passed to `for-each()`, bound to a
    variable, or crossing opt-in parallel pattern evaluation's thread
    boundary — `Arc` rather than `Rc` is why.
  - `for-each()` is a genuine multiplying construct and shares the same
    budget nested `for`/`to`-ranges already do, so `for-each` inside
    `for-each` inside `for-each` is bounded the same way.
  - Not in this phase: the arrow operator `=>`, string concatenation
    `||`, the simple map operator `!`, the rest of the higher-order
    function library (`filter`, `fold-left`, `fold-right`, `sort`, …),
    and maps and arrays — XPath 3.1, which stays behind the still-refused
    `xpath31`/`xslt31` bindings. Each is a hard error naming the
    construct. See [spec/xpath3/](spec/xpath3/index.md).

## 0.10.0

### Added

- **XPath 2.0 phase 9: `adjust-date-to-timezone()`, `adjust-dateTime-to-timezone()`,
  and `adjust-time-to-timezone()`.** One function
  (`temporal::adjust_to_timezone`) serves all three types. Converts the
  instant a value denotes when it already has a timezone — which can roll
  a date to the adjacent day — or simply attaches the given timezone when
  it doesn't. Verified against the F&O reference material's own worked
  examples. See [spec/xpath2/](spec/xpath2/index.md).

## 0.9.0

### Added

- **XPath 2.0 phase 8: `resolve-uri()`.** RFC 3986 URI-reference
  resolution, implemented by hand (`src/xpath/uri.rs`) rather than adding
  a dependency, and verified against the RFC's own worked examples. The
  one-argument form falls back to the document's own base URI and errors,
  naming what's missing, when the document has none. See
  [spec/xpath2/](spec/xpath2/index.md).

## 0.8.0

### Added

- **XPath 2.0 phase 7: `deep-equal()`.** Structural equality over
  sequences: atomic items compare by value (`NaN` deep-equals `NaN`, and a
  mismatched type is simply unequal — both unlike `eq`, which errors on
  the second and treats `NaN` as never equal), and nodes compare
  recursively by kind, expanded name, an order-independent attribute set,
  and same-order children. This crate does not atomize for `deep-equal`: a
  node never compares equal to an atomic value. See
  [spec/xpath2/](spec/xpath2/index.md).

## 0.7.0

### Added

- **XPath 2.0 phase 6: the cardinality assertions and atomization** —
  `zero-or-one()`, `one-or-more()`, `exactly-one()`, and `data()`. The first
  three were an oversight rather than a deliberate gap: unlike every other
  unimplemented XPath 2.0 function, they weren't named anywhere the crate
  could say so, and calling one reported "unknown function" — indistinguishable
  from a typo. `data()` atomizes a node to its typed value, which this
  schema-unaware crate always takes to be untyped atomic, represented as a
  plain string like every other untyped value in this engine. See
  [spec/xpath2/](spec/xpath2/index.md).

## 0.6.0

### Changed

- **`Value::Number` and `Item::Number` gained a second field**, a
  `NumericType` tag (`Integer`, `Decimal`, `Float`, or `Double`). Both enums
  were already `#[non_exhaustive]`; this changes the shape of an existing
  variant, so any code constructing or matching `Value::Number(n)` /
  `Item::Number(n)` directly needs the tag added — `NumericType::Double`
  reproduces the old behaviour everywhere except numeric literals and casts.

### Added

- **XPath 2.0 phase 4: the numeric type hierarchy.** `instance of` and its
  companions now recognize `xs:integer`, `xs:decimal` and `xs:float` as well
  as `xs:double`. The type is tracked lexically for numeric literals (`1` is
  an integer, `1.0` a decimal, even though they are numerically equal) and
  for the result of an explicit `cast as`/`castable as`; `xs:integer`
  matches `instance of xs:decimal` too, since it derives from `xs:decimal`
  by restriction in XML Schema. Arithmetic, every function in the library,
  and `for`'s `to` ranges are deliberately left untracked and still produce
  `xs:double`, so `(1 + 1) instance of xs:integer` is `false` — this crate
  does not implement full XPath 2.0 numeric type promotion. See
  [spec/xpath2/](spec/xpath2/index.md).
- This also **corrects a documentation mistake**: `spec/xpath2/` previously
  recorded `1 instance of xs:double` as agreeing with real XPath 2.0. It
  does not — `1` is an `xs:integer`, which does not derive from `xs:double`
  — and both the documentation and the crate's behavior now say `false`.
- **XPath 2.0 phase 5: the remaining sequence-manipulating functions** —
  `reverse()`, `subsequence()`, `insert-before()`, `remove()`, and
  `unordered()`. See [spec/xpath2/](spec/xpath2/index.md).
- This also **corrects a misclassification**: `for-each()` was listed
  alongside these as merely unwritten, but it is not part of XPath 2.0 at
  all — it needs a function item, an XPath 3.0 feature this crate does not
  implement. It now reports that instead of "needs a sequence."

## 0.5.1

### Changed

- **MSRV raised to 1.96** (current stable minus two, up from minus three).
  Routine maintenance per
  [spec/rust-msrv-n-minus-2/](spec/rust-msrv-n-minus-2/index.md); not a
  breaking change. Verified with `cargo +1.96 test --all-features` on the
  boundary toolchain itself.

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
  [spec/conformance/](spec/conformance/index.md), established by running this
  crate and the ISO reference implementation against the same schema.

### Fixed

- **A denial of service in XPath 2.0 ranges and loops.** A limit on a single
  `to` range cannot see that nesting multiplies: each range in
  `for $i in 1 to 999 return for $j in 1 to 999 return for $k in 1 to 999
  return $k` is well inside it, and together they ask for close to a billion
  items — from a 90-byte expression. Found by fuzzing. A budget is now shared
  across every nested construct in one expression, so the product is bounded;
  both limits are in [spec/conformance/](spec/conformance/index.md).

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
  `@location`. See [spec/validation/](spec/validation/index.md).

- **A misspelled variable is an error when the schema loads**, rather than
  when the expression using it is first evaluated. `$naem` for `$name` used to
  abort a validation part-way through; it now fails at compile time, before
  any document is read.

### Added

- `<sch:key>` and the `key()` function, turning a quadratic cross-reference
  check into a linear one. A non-ISO extension — see [spec/keys/](spec/keys/index.md).
- `Report::from_svrl`, making SVRL bidirectional: reports can be read back,
  not only written.
- `extends href`, and fragment identifiers on both it and `include`:
  `lib.sch#dates` selects one element, `#dates` one from the document being
  read. `include` splices the element, `extends` its children.
- Six lints: unreferenced keys, unused variables, rules with no assertions,
  patterns with no rules, duplicate assertion tests, and phases that activate
  nothing. See [spec/linting/](spec/linting/index.md).

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
  [spec/conformance/](spec/conformance/index.md) — several of them cases where the
  reference is the one in the wrong. See [spec/testing/](spec/testing/index.md).
