# Invariants

Things that must never break. Each has a reason and, where possible, a test
that fails when it does.

## Safety and trust boundary

### No `unsafe`

`src/lib.rs` and `src/main.rs` both carry `#![forbid(unsafe_code)]`. A
validator is routinely pointed at input from outside the trust boundary; a
memory-safety bug here is a remote one. If a change appears to need `unsafe`,
the design is wrong.

### External entities are never resolved

The parser does not process DTD entity declarations at all. A `<!DOCTYPE>` is
skipped, and a reference to an entity it declares is an **error**, not a
silent empty expansion.

This makes XXE *structurally impossible* rather than merely switched off,
which is a stronger guarantee and a much easier one to keep. Do not add DTD
entity support "just for convenience".

- Test: `tests/schema.rs::external_entities_are_never_resolved`
- Test: `src/xml/parser.rs::tests::rejects_unknown_entity`

### No implicit network access

`FileResolver` refuses `http:` and `https:` with a message saying why. The
library never fetches on the caller's behalf; an application that wants that
supplies its own `Resolver` and owns the decision. There is no CLI flag to
switch this on.

- Test: `tests/schema.rs::the_default_resolver_refuses_network_access`

### `document()` is not a hole in the network rule

XPath `document()` fetches through the same `Resolver` as `include`, so the
default refusal of `http:` and `https:` applies to it too. A feature that
reads external documents is exactly where an implicit fetch would creep in.

- Test: `tests/documents.rs::the_default_resolver_still_refuses_the_network`

### `document()` without a registry is an error, not an empty node-set

Evaluating XPath directly, outside a validation run, has no document registry.
`document()` then fails rather than returning nothing, because an empty
node-set would make `assert test="document('x')/y"` *pass* on a lookup that
never happened.

- Test: `tests/documents.rs::calling_document_without_a_registry_is_an_error`

### Malformed input is an error, never a panic

Every parser and every evaluator returns `Result`. Depth is bounded in the
XPath parser (64 nested sub-expressions), the XML parser (1024 nested
elements), include resolution (64), and `extends` chains (64). Exceeding a
bound is an error.

Four `cargo-fuzz` targets enforce this over arbitrary input. This has already
caught a real panic: `not()` with no arguments indexed `args[0]`, because the
function library trusted that the schema compiler had checked arity — but
`evaluate` is public and that path never ran the check.

- Test: `src/xpath/eval.rs::tests::wrong_arity_is_an_error_not_a_panic`
- Targets: `fuzz/fuzz_targets/*.rs`

## Correctness

### An evaluation error is never a false assertion

If an XPath test cannot be evaluated — a variable out of scope, a type error —
that is
`Err`, and validation stops. It is **never** downgraded to "the test was
false", because a false `assert` means the document is invalid, and silently
turning a broken schema into a stream of failures, or worse into a pass, is
the single most dangerous thing a validator can do.

- Test: `tests/validation.rs::an_evaluation_error_is_an_error_not_a_silent_false`

### A finding is not an error

A document breaking a rule is a `Report` entry, not `Err`. Keep the two
categories separate; see [`spec/errors/`](../spec/errors/index.md).

### A successful report is not a failure

`report` fires when its test is **true** and is an *observation*.
`Report::is_valid()` counts only failed asserts, and the CLI exit code
follows. Conflating the two is the most common way to misuse Schematron.

- Test: `tests/validation.rs::a_successful_report_does_not_make_a_document_invalid`

### First matching rule wins

Within one pattern, a node is processed by exactly one rule: the first whose
context matches. Not "every matching rule".

- Test: `tests/validation.rs::within_a_pattern_only_the_first_matching_rule_fires`
- Corpus: `tests/corpus/first-rule-wins/`, `tests/corpus/rule-alternatives/`

### Patterns do not compete with each other

Every pattern gets its own pass over the document.

- Corpus: `tests/corpus/patterns-are-independent/`

### SVRL round-trips

Every report the writer emits must parse back identical, apart from
`FiredRule::location`, which SVRL has nowhere to record. The corpus round-trip
test checks this for every case — which is a far stronger check on the writer
than asserting on substrings of its output.

Keep it non-vacuous. The check passed for months' worth of cases while the
`flag` attribute could be deleted from the writer unnoticed, because no corpus
case set one; `tests/corpus/rich-metadata/` exists to exercise every optional
field. When adding a field to a report, add it there too.

- Test: `tests/corpus.rs::every_corpus_report_survives_an_svrl_round_trip`

### A misspelled variable fails when the schema loads

An unknown function and an undeclared namespace prefix are compile-time
errors, so a misspelled variable is one too — consistency matters more here
than the small extra analysis. The check is against the union of every name
any `let` binds, which catches every typo with no false positives; a variable
that exists but is out of reach stays a validation error. See
`spec/parsing/`.

- Test: `tests/validation.rs::a_variable_nothing_binds_is_caught_when_the_schema_loads`
- Test: `tests/validation.rs::an_evaluation_error_is_an_error_not_a_silent_false`

### A missing key is an error, not an empty result

`key('prats', @ref)` on an undeclared key is an error naming it. Returning an
empty node-set would make the assertion quietly pass, so a typo in a key name
would silently disable the check it guards.

A key that matches *no nodes* is different, and is still declared: "no such
node" and "no such key" are distinct mistakes.

- Test: `tests/keys.rs::looking_up_an_undeclared_key_is_an_error`
- Test: `tests/keys.rs::a_key_matching_nothing_is_still_declared`

### `castable as` reports, `cast as` raises

The two run the same check and differ only in what they do when it fails.
That is deliberate: a schema needs to *report* a bad value as a finding, which
it cannot do if asking the question aborts the run.

Keep them in step. A cast that succeeds where `castable as` said false, or the
reverse, would make the guard pattern in `spec/tutorial/` unsound.

- Test: `tests/xpath2.rs::cast_as_errors_where_castable_as_reports_false`
- Test: `tests/xpath2.rs::castable_as_reports_instead_of_aborting`

### `is` is identity, not equality

`b = c` asks whether some pair of nodes has matching content; `b is c` asks
whether they are the same node. Conflating them would make the operator
pointless, since `=` already exists.

- Test: `tests/xpath2.rs::is_asks_about_identity_not_content`

### Arithmetic that has no meaning is an error

XPath 2.0 defines date and duration arithmetic narrowly: a date minus a date
is a duration, a date plus a duration is a date. A date plus a date, or a
yearMonthDuration plus a dayTimeDuration, has no defined result — and gets an
error naming the operands, never a number.

The two duration subtypes are kept apart for the same reason: whether one
month exceeds thirty days depends on the month, so `xs:duration` is only
partially ordered and this crate implements the subtypes instead.

- Test: `tests/xpath2.rs::adding_two_dates_is_an_error`
- Test: `tests/xpath2.rs::mixing_the_two_duration_subtypes_is_an_error`

### A value comparison reports rather than guesses

`eq` and its family exist to be stricter than `=`. Two or more items on either
side is a type error; mismatched types are a type error. Both are cases where
`=` quietly succeeds, and turning either into a silent answer would remove the
only reason to write `eq` at all.

Error messages here name the general counterpart — `=` for `eq`, `<` for `lt`
— so the reader is told what would have worked.

- Test: `tests/xpath2.rs::more_than_one_item_is_an_error_where_a_general_comparison_is_not`
- Test: `tests/xpath2.rs::mismatched_types_are_an_error`

### Validation is reproducible, including the clock

`current-date()` and its companions read the run's instant, not the system
clock. The instant is taken **once** per validation — XPath 2.0 requires that
much — and can be supplied through `ValidateOptions::with_current_time`, which
is what makes a schema with date rules testable at all.

Evaluating XPath directly, with no run, makes the clock functions an error
rather than an arbitrary time. A validator that quietly returns a different
answer tomorrow is worse than one that refuses.

- Test: `tests/xpath2.rs::the_clock_is_stable_across_a_whole_run`
- Test: `tests/xpath2.rs::calling_the_clock_without_a_run_instant_is_an_error`

### A malformed date is an error, not a false test

Casting an untyped value to a date is how `@signed &lt; current-date()` works.
A value that will not cast is an error naming it. A quietly false assertion
would report a document as broken for the wrong reason, or pass it for one.

- Test: `tests/xpath2.rs::a_malformed_date_is_an_error_not_a_false_test`

### `Value::Sequence` is unreachable under XPath 1.0

Nothing in the XPath 1.0 grammar or function library constructs a sequence, so
a 1.0 expression evaluates through exactly the code it did before sequences
existed. This is what makes the XPath 2.0 type additive rather than a rewrite
of the 1.0 engine.

Preserve it when adding to the engine: a new 2.0 construct must be gated by
`XPathVersion`, and a 1.0 code path must never see a `Sequence`. Where a match
arm can prove that, say so with `unreachable!` rather than inventing a
behaviour for it.

- Test: `tests/xpath2.rs::sequence_syntax_is_refused_under_a_one_point_zero_binding`

### XPath 1.0 semantics are exact, including the surprising parts

Do not "fix" any of these:

- Node-set comparison is existential, so `a = b` and `a != b` can both be true.
- Except against a boolean, which converts the node-set with `boolean()`
  instead of iterating it. So an empty node-set gives `missing >= false()`
  **true** — `0 >= 0` — while `missing = 'x'` and `missing != 'x'` are both
  false. This one was a real bug, found by differential testing against the
  reference; do not "simplify" it back into the bullet above.
- Relational operators always convert to number, so `'x' > 0` is false rather
  than an error.
- A rule context of the shape `/descendant-or-self::node()/child::TEST` —
  which is what a bare name or wildcard roots to, and so most contexts — takes
  a **fused walk** in `matched_nodes` rather than the general evaluator.
  Evaluating it generically materialises every node in the document and then
  filters, once per rule; the walk keeps only what matches. It cut the
  per-rule cost on a 20,000-element document from 1.33ms to 0.26ms.

  A `debug_assert_eq!` compares the fast path against the evaluator on every
  rule context, so the whole test suite and every generated differential case
  checks that the two agree. Do not remove it: a fast path that silently
  disagrees is worse than no fast path.
- Rule claims in `run_pattern` are a **vector indexed by `NodeId`**, not a
  map keyed by one. `NodeId` is a dense arena index, so hashing it buys
  nothing: profiling put SipHash and its `RandomState` above the XPath
  evaluation the pattern exists to do. Replacing it took 10–19% off
  validation across every document size.
- `Document::location` is **linear in depth**. It walks up once collecting
  the chain and writes the steps in a single pass. The obvious recursive
  shape — `format!("{parent}/{step}")` at each level — re-copies the whole
  ancestor prefix per level, which is quadratic, and a finding pays it once
  per location. On a document nested 300 deep that cost 5.4x. There is a
  benchmark, `location_generation_deep`.
- `sum()` over an empty node-set is **positive** zero, so `1 div sum(none)`
  is Infinity. It is folded from `0.0` rather than written `.sum()`, because
  Rust's `Sum` for `f64` starts from `-0.0` — the true additive identity, and
  the wrong answer here. Do not "tidy" the fold back into `.sum()`.
- `string(number)` never uses exponential notation: `1e21` prints in full.
- An unprefixed name matches **no namespace**. There is no default namespace.

Each has a test in `src/xpath/`. A "simplification" here makes the engine
disagree with every other conformant processor.

The lexer's disambiguation rules are ordered, and the order is part of the
standard: the operator-position rule comes before the followed-by-`(` rule.
Reversing them breaks `a and (b)`, which shipped broken in 0.1.0.

- Test: `src/xpath/lexer.rs::tests::operator_names_beat_function_names`

### A number's tracked type never changes how it is computed

`Value::Number`/`Item::Number` carry a `NumericType` tag (`Integer`,
`Decimal`, `Float`, `Double`) so `instance of` can answer what a number's
declared type is — but every function that touches the `f64` next to the tag
(`to_number`, arithmetic, `format_number`, comparisons) strips the tag first
and is otherwise unchanged from before the tag existed. Only two things ever
set a tag more specific than `Double`: a numeric literal, tagged lexically at
the lexer (`1` is `Integer`, `1.0` is `Decimal`, regardless of value); and an
explicit `cast as`/`castable as`. Arithmetic, every function in the library,
and `for`'s `to` ranges always produce `Double`.

Do not extend the tag to track anything through arithmetic or a function
result — that is a deliberate, documented limit
(`spec/xpath2/`, "Numbers, and what `instance of` reports"), not an
oversight to "complete." Threading a numeric type lattice through every
arithmetic operation and function is exactly the risk weighed against, and
rejected, in `spec/roadmap/` before this tag was added.

- Test: `tests/xpath2.rs::a_literal_carries_the_type_it_is_written_as`
- Test: `tests/xpath2.rs::computed_numbers_are_still_double`

## Performance

### Expressions are parsed once per schema, not once per document

Compilation caches every expression keyed by source text. Validating N
documents must not parse N times.

- Bench: `benches/bench_validate.rs::bench_compile_once_validate_many`,
  reported by criterion under `compile_once_validate_many`. If this regressed,
  the gap between its two arms would close.

### Matching is linear, not quadratic

Rule contexts are evaluated once per document. Node locations use precomputed
sibling positions. Descendant tests use precomputed subtree ranges.

A benchmark caught the last one: SVRL location generation was rescanning the
parent's child list per finding, which made validation quadratic in the number
of sibling elements. Fixing it was a roughly fourteenfold speedup on a
10 000-element document. (Historical note, not a live figure — current numbers
come from `cargo bench`.)

- Bench: `benches/bench_validate.rs::bench_validate`, reported under
  `validate/1000` and `validate/10000`. The ratio must stay roughly linear.

### `Schema` is `Send + Sync`

Compile once, share across threads with an `Arc`, no locks.

- Test: `src/schema/compile.rs::tests::schema_is_send_and_sync`
- Test: `tests/validation.rs::one_schema_validates_many_documents_concurrently`

## Documentation

### Documented facts are machine-checked

Every schema in the docs compiles; every CLI flag is documented and every
documented flag exists; the MSRV in the spec matches `Cargo.toml`; every
relative link resolves.

- Tests: `tests/docs.rs`, and the two documentation tests in `tests/cli.rs`

### Absolute paths are per-document

`/` means the root of the document the context node belongs to, which is not
the instance root once `document()` has merged another document in. Every
place that needs a root uses `Document::root_of(node)`, never
`Document::root()`.

- Test: `tests/documents.rs::an_absolute_path_inside_a_loaded_document_means_that_documents_root`

### Rules never fire on nodes of a loaded document

`document()` makes another document *readable*, not *validated*. Node walking
starts from the primary root only.

- Test: `src/xml/document.rs::tests::appending_a_document_keeps_the_two_trees_separate`

### Trees must be finalized

Any code path that builds a `Document` must call `Document::finalize()`, or
`subtree_end` and `sibling_position` are wrong and the axes silently misbehave.
The parser and the include resolver both do.

- Test: `fuzz/fuzz_targets/fuzz_xml.rs` asserts subtree consistency on every
  parsed tree.
