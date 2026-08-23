# Testing, benchmarking, and linting

## Layers

| Layer | Location | What it covers |
|---|---|---|
| Unit tests | `#[cfg(test)]` in each module | Lexer, parser, evaluator, model, one function at a time |
| Integration tests | `tests/` | Whole-crate behaviour through the public API |
| Document tests | `tests/documents.rs` | `document()`, cross-document node-sets, and the loading passes |
| Corpus tests | `tests/corpus/` | `.sch` + `.xml` + expected `.svrl` triples |
| Doc tests | rustdoc examples | Every public item's example actually runs |
| CLI tests | `tests/cli.rs` | Arguments, exit codes, output formats |
| Fuzz targets | `fuzz/` | Crash and panic freedom on hostile input |
| Differential | `tests/differential.rs` | Agreement with the ISO reference implementation |
| Benchmarks | `benches/` | Regression tracking with criterion |

Assertions use the `assertables` crate, matching the house style of the
sibling SixArm crates.

## Corpus tests

The corpus is the real conformance suite. Each case is a directory:

```
tests/corpus/<case>/
  schema.sch      the schema
  input.xml       the instance
  expected.txt    expected findings, one per line: KIND | location | text
  phase           optional: the phase to run
```

The runner discovers cases, runs each, and compares. Adding a case is adding a
directory — no Rust code. The cases, and what each pins down:

| Case | Covers |
|---|---|
| `assert-polarity` | An `assert` fires on **false**, and is silent on true |
| `report-polarity` | A `report` fires on **true** — the opposite polarity |
| `first-rule-wins` | A broad rule claims a node, leaving a later narrow rule dead |
| `rule-alternatives` | The same mechanism used deliberately, as `else` branches |
| `patterns-are-independent` | One node matched separately by two patterns |
| `namespaces` | `ns` prefix bindings against a namespaced document |
| `node-kinds` | Rules matching attributes, text, comments, and PIs |
| `absolute-context` | `/root/a` is not searched across the whole document |
| `union-context` | `a \| b` as a rule context |
| `predicate-context` | `line[1]` means "first of its own parent" |
| `let-scopes` | Schema, pattern, and rule scopes resolving together |
| `let-phase-scope` | The phase scope, shadowing a schema-level binding |
| `let-shadowing` | Innermost binding wins across all scopes |
| `abstract-patterns` | `is-a` and `param`, including `$x` inside a string literal |
| `abstract-rules` | `extends`, transitive, spliced at its own position |
| `include-fragment` | `include href="…#id"` splices the element; `extends href` splices its children |
| `node-set-boolean-comparison` | A node-set against a boolean converts rather than iterating, per XPath 1.0 section 3.4 |
| `following-axis-from-attribute` | `following` taken from an attribute node — a documented divergence from the reference |
| `namespaced-attribute-context` | `@x` and `@p:x` are different attributes — a second documented divergence |
| `namespaced-sibling-position` | A location counts position within its own namespace, not by local name alone |
| `message-inline-whitespace` | Whitespace between two inline elements in a message survives |
| `phase-default` | `@defaultPhase` applies when no phase is named |
| `phase-selection` | A named phase activating more patterns |
| `phase-all` | `#ALL` overrides the default and runs unlisted patterns |
| `diagnostics` | Multiple diagnostic references from one assertion |
| `message-interpolation` | `value-of`, `name`, and `name/@path` |
| `subject` | `@subject` on both rule and assert moving the location |
| `no-findings` | A clean document produces nothing at all |
| `rich-metadata` | Every optional attribute a finding can carry, so the SVRL round trip covers them |

Two things are deliberately **not** corpus cases, because the format has no
way to supply auxiliary files:

- `include` and nested includes — `tests/schema.rs`, using `MemoryResolver`.
- `pattern/@documents` — `tests/schema.rs`, using real temporary files.
- XPath `document()` — `tests/documents.rs`, using both.

## Differential testing

The strongest evidence available that this implementation is correct is that
it agrees with another one. `tests/differential.rs` compiles every corpus
schema with the **ISO Schematron reference implementation** — a set of XSLT
stylesheets run through `xsltproc` — and compares the findings.

It is `#[ignore]`d, because it needs `xsltproc` and the reference
stylesheets, and those are third-party: they carry their own licence and
release cadence, and a vendored copy here would rot.

```sh
sh tests/differential/fetch-skeleton.sh /tmp/skeleton
SCHEMATRON_SKELETON=/tmp/skeleton cargo test --test differential -- --ignored
```

Every corpus case the reference can run agrees exactly. The exceptions are
listed in the test itself, each with its reason, and in
[conformance.md](conformance.md). Two lists drive it:

- `KNOWN_DIVERGENCES` — cases where the two legitimately differ.
- `REFERENCE_CANNOT_RUN` — cases the reference cannot compile at all.

Both are checked in **both directions**. A case that stops diverging, or that
the reference learns to run, fails the test just as an unexpected difference
does — because a list of known problems that no longer describes reality is
worse than no list. The test prints its own tally, so no count is repeated
here to go stale.

### Rule dispatch

A rule that fires but whose assertions all hold produces no finding, so
comparing findings cannot see it at all. That is a large blind spot: whether
a schema's tests happen to pass or fail has nothing to do with how hard it
exercises **first-matching-rule-wins**, which is the semantic the whole
language turns on.

SVRL records each firing as `svrl:fired-rule`, so the two implementations'
dispatch is compared directly: how many times each rule fired, in what order,
under which pattern. SVRL has nowhere to record *which node* a rule fired on,
so it is the sequence that is compared, not the nodes.

Breaking first-matching-rule-wins so that the *last* matching rule claims a
node is caught here at the first firing, in cases where the findings alone
still matched.

### Generated cases

The curated corpus covers constructs one idea at a time. It cannot cover their
*combinations*, and XPath 1.0's conversion rules are exactly where a
disagreement hides: a node-set against a number, an empty node-set coerced to
a boolean, a string that is not a number fed to a relational operator.

So `generated_cases_agree_with_the_reference_implementation` builds schema and
document pairs from a grammar, runs both implementations, and requires
identical findings. Each case comes from a seed, and the seed alone reproduces
it:

```sh
SCHEMATRON_FUZZ_SEED=25 SCHEMATRON_FUZZ_CASES=1 \
  cargo test --test differential generated -- --ignored --nocapture
```

This found a real bug: node-set compared to boolean was being evaluated
existentially like the string and number cases, when XPath 1.0 section 3.4
requires a `boolean()` conversion. `missing >= false()` is true, and this
crate said false. See [xpath.md](xpath.md).

**The generator is only as good as its grammar, and this is measured rather
than assumed.** Deliberately breaking a comparison rule and checking that the
generated cases notice is how the grammar was found wanting the first time:
an earlier version compared node-sets of one node almost exclusively, agreed
on 3487 findings, and failed to notice either of two sabotaged comparison
rules. It now generates repeated sibling names, so node-sets hold several
nodes, and compares them against booleans. The default case count is set the
same way — 200 was not enough to catch a sabotaged existential rule, so the
default is 500.

Coverage here is probabilistic, so it supplements the curated corpus and the
unit tests rather than replacing them: anything the generator finds gets a
named case of its own.

What the grammar now reaches, each verified by sabotage rather than assumed —
break the feature, and generated cases must notice:

| Area | Sabotage that it catches |
|---|---|
| Node-set comparison | existential rule replaced by first-node |
| Node-set versus boolean | conversion replaced by iteration |
| Namespaces | (the reference's own defect found this one) |
| Comments | comment nodes dropped from the tree |
| CDATA | CDATA content dropped |
| Processing instructions | PI nodes dropped |
| Entity references | text left unescaped |
| `extends rule` | abstract rule splicing disabled |
| `is-a` and `param` | parameter substitution disabled |
| Phases and `active` | phase selection ignored, every pattern run |
| Schema-level `let` | the global bindings not bound |
| Assertion `@flag` | flag omitted from the SVRL |
| Assertion `@role` | role omitted from the SVRL |
| Diagnostics | diagnostic references omitted |
| Locations | sibling position off by one |
| Namespaced positions | siblings counted by local name alone |
| Rule dispatch | last matching rule wins instead of the first |
| `fired-rule` events | omitted from the SVRL |
| `include href="…#id"` | the include not resolved at all |
| Fragment identifiers | the `#id` ignored, whole document spliced |
| `extends href` | the element spliced instead of its children |
| `<name/>` in a message | contributes nothing |
| `<name path="…"/>` | the path ignored, always the context node |
| `emph`, `span`, `dir` | inline markup contributes nothing |

Assertion messages are generated as mixed content — `value-of`, `name`,
`name/@path`, and the inline markup `emph`, `span` and `dir` — because all of
it is instantiated into the message the comparison already checks. That covers
a whole element group for no extra comparison machinery.

Generated schemas are not flat either: they carry namespace declarations, a
schema-level `let`, abstract rules spliced in by `extends`, an abstract
pattern instantiated through `is-a` with `param` substitution, and a phase
that activates only some patterns. A second file, `lib.sch`, is written beside
the schema, which pulls parts out of it by fragment — `include` taking a whole
pattern and `extends href` taking a rule's children — so that relative
resolution against the schema's own location is exercised rather than assumed. Those exercise the compiler and expansion
passes rather than the XPath engine, which the expression grammar alone never
reaches.

Documents are generated with mixed content, comments, processing
instructions, CDATA, character references and whitespace, because a document
of bare elements never reaches the parser's interesting paths. Several of the
text pieces resolve to the same string by different routes — `foo`,
`&#102;oo`, `&#x66;oo`, `<![CDATA[foo]]>` — so a comparison against `'foo'`
tests resolution rather than only length.

## Fuzzing

`cargo-fuzz` targets, all built on `libfuzzer-sys`:

| Target | Input | Property |
|---|---|---|
| `fuzz_xml` | arbitrary bytes | The XML parser never panics; it returns `Ok` or `Err`. |
| `fuzz_xpath` | arbitrary UTF-8 | The XPath lexer and parser never panic. |
| `fuzz_schema` | arbitrary UTF-8 | Schema compilation never panics. |
| `fuzz_validate` | structured: schema + document | Full validation never panics and always terminates. |

The invariant under test is the same for all four: **no panic, no hang, no
unbounded memory**. Recursion in the XPath parser and in include resolution is
depth-limited so that deeply nested input returns an error instead of blowing
the stack.

Each target also checks an invariant beyond "did not crash":

- `fuzz_xml` walks the parsed tree and asserts it is self-consistent — every
  child's parent points back, every subtree range contains its children.
- `fuzz_xpath` evaluates whatever parsed, and converts the result to all three
  scalar types, so the conversions are exercised too.
- `fuzz_validate` asserts that SVRL this crate emits is XML this crate can
  read back.

```sh
cargo +nightly fuzz run fuzz_xpath -- -max_total_time=60 -report_slow_units=30
```

Corpus seeds are copied from `tests/corpus/`, so the fuzzer starts from valid
input and mutates outward.

**Raise `-report_slow_units`.** A fuzz build carries a sanitizer and coverage
instrumentation, and runs roughly ninety times slower than release: a `to`
range at the documented limit takes 0.3 s release and **27 s** instrumented.
libFuzzer's default threshold is 10 s, so it files that legal, bounded
expression as a slow unit on every run, and the next person spends their time
re-deciding it is not a defect. Thirty seconds sits above the slowest input
the limits permit, so what gets reported is work that is genuinely unbounded.

Memory is worth a word too. `-rss_limit_mb` measures the **whole process**,
and the fuzzer's own corpus and coverage tables are most of it — 437 MB before
a single iteration for `fuzz_validate`. Setting a ceiling near that produces
OOM artifacts that reproduce in a millisecond using 44 MB. Leave the default,
or measure the baseline first.

**This has already paid for itself.** `fuzz_xpath` found, within seconds, that
`not()` with no arguments panicked: the function library indexed `args[0]` on
the strength of the schema compiler having checked arity, but `evaluate` is
public and that path never ran the check. The fix was to check arity in the
library itself; the regression test is
`xpath::eval::tests::wrong_arity_is_an_error_not_a_panic`. Since then all four
targets have run clean for roughly eight million executions.

### What fuzzing found

The `not()` arity panic, and later a denial of service: nested ranges and
`for` expressions multiply, and a limit on *one* range cannot see it. Each
range in

```
for $i in 1 to 999 return for $j in 1 to 999 return for $k in 1 to 999 return $k
```

is comfortably inside the single-range limit, and together they ask for close
to a billion items — from a 79-byte expression. The fix is a budget shared by
every nested construct in one expression, so the product is what is bounded;
see [conformance.md](conformance.md) for both limits.

libFuzzer reports a **slow unit** long before the crate's limits are reached,
because its threshold is measured against a microsecond budget. A range of
938,020 items is legal, documented, and takes about a third of a second — it
will be reported as slow every time, and that is not a defect. Read the input
before acting on it: what matters is whether the work is *bounded*, not
whether it is fast.

## Benchmarks

`criterion` benches in `benches/`:

| File | Measures |
|---|---|
| `bench_xml_parse` | Parsing 10 / 1 000 / 100 000 elements; deep nesting; string values; document order |
| `bench_xpath` | Compiling expressions, evaluating them, and each axis separately |
| `bench_validate` | Schema compilation, end-to-end validation, compile-once-validate-many, fired-rule recording, and report rendering |

```sh
cargo bench
cargo bench -- --save-baseline main
```

Two of these earn their keep specifically:

- **`compile_once_validate_many`** keeps the compiled-schema guarantee honest.
  Validating N documents must not re-parse each XPath expression N times, and
  if that regressed the gap between the two arms of this benchmark would close.
- **`validate` at 1 000 and 10 000 elements** is what caught a real quadratic:
  generating an SVRL location rescanned the parent's child list once per
  finding. Fixing it — by precomputing sibling positions and subtree ranges,
  see [xml.md](xml.md) — made the 10 000-element case fourteen times faster
  and restored linear scaling.

Indicative numbers on an M-series laptop:

| Benchmark | Time |
|---|---|
| `schema_compile` | ~19 µs |
| `validate/1000` | ~0.95 ms |
| `validate/10000` | ~10 ms |
| `report_render_1k/svrl` | ~120 µs |
| `parallel_patterns_100/sequential` | ~560 µs |
| `parallel_patterns_100/parallel` | ~670 µs — *slower*; threads cost more than they save here |
| `parallel_patterns_5000/sequential` | ~28 ms |
| `parallel_patterns_5000/parallel` | ~6.5 ms — about 4× faster |
| `cross_reference_200/with_key` | ~166 µs |
| `cross_reference_200/without_key` | ~7.7 ms |
| `cross_reference_1000/with_key` | ~785 µs |
| `cross_reference_1000/without_key` | ~189 ms — about **240× slower** |

The cross-reference pair measures a complexity difference rather than a
constant factor, and the scaling shows it. Going from 200 references to 1 000
— five times the data — the keyed version takes 4.7 times as long, and the
unkeyed one 24.6 times, which is 5². That is the whole justification for
[keys.md](keys.md), and it is measured rather than asserted.

The parallel pair is worth reading as a warning as much as a result: on a
small document, turning threading on makes validation slower. That is why
`parallel_patterns` is off by default and why the documentation says to
measure rather than assume.

## Lints

`clippy::pedantic` is enabled crate-wide in `Cargo.toml`:

```toml
[lints.clippy]
pedantic = { level = "warn", priority = -1 }
```

Individual pedantic lints are allowed only with a comment explaining why, at
the narrowest scope that works. `#![deny(missing_docs)]` is on: every public
item carries documentation, and CI fails without it.

## Commands

```sh
cargo test                        # unit + integration + corpus + doc tests
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps               # must be warning-free
cargo bench
cargo +nightly fuzz run fuzz_validate
cargo +1.94 test --all-features   # the MSRV boundary; see rust-msrv-n-minus-3.md
```

The last one has to run on the boundary toolchain itself. Testing on a newer
compiler proves nothing about the MSRV, because newer compilers accept
strictly more.
