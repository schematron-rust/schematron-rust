# Task recipes

Step-by-step for the jobs that come up. Each ends with the same gate: the four
commands in [`AGENTS.md`](../AGENTS.md).

## Add a Schematron semantics test

Almost always a corpus case, not Rust. See
[`testing/`](testing.md#adding-a-corpus-case). One directory, three files,
no code.

## Add an XPath function

1. `src/xpath/functions.rs` — add to `SIGNATURES` with its `(name, min, max)`
   arity. `None` as max means unbounded.
2. Add an arm to `call()`. Arity is already checked at entry, so you may index
   `args` up to the declared minimum — and no further.
3. Unit-test it in `src/xpath/eval.rs`, exercising the type conversions, not
   just the happy path.
4. Add it to the function list in [`spec/xpath/`](../spec/xpath/index.md).
   `tests/docs.rs::the_xpath_function_list_in_the_spec_matches_the_engine`
   fails if you forget.

For an **XPath 2.0** function, use `SIGNATURES_V2` instead. It becomes
available only under an `xslt2` or `xpath2` query binding, because
`check_function` gates on `XPathVersion`. Document it in the table in
[`spec/xpath2/`](../spec/xpath2/index.md);
`tests/docs.rs::the_xpath_two_function_list_in_the_spec_matches_the_engine`
fails if you forget.

If the function you're adding is a sequence-manipulating or date/time
function nobody has implemented yet — not blocked on the sequence or date
and time *types*, which both shipped (phases 2a and 2b) — do **not**
approximate it. Add it to `V2_FUNCTIONS_NEEDING_SEQUENCES` or
`V2_FUNCTIONS_NEEDING_DATES` so the error names the gap instead of saying
"unknown function".

## Add a new XPath syntax construct (not a function)

A new operator or expression form — `=>` and `||` were the last two of
these — touches more places than a function does, and some of them the
compiler will not catch for you if you miss one. Work through in this
order:

1. `src/xpath/lexer.rs` — a new `TokenKind` for any new punctuation, the
   byte(s) that produce it, and a `Display` arm. If the new token can be
   confused with an existing one by a shared leading character (`=` vs
   `=>`, `|` vs `||`), disambiguate by peeking the next byte, the way
   `<`/`<=`/`<<` already does. Add the new token to
   `previous_allows_operator`'s exclusion list too — otherwise a `*` or a
   keyword like `div` right after it misclassifies as multiply/operator
   instead of a wildcard/name starting a fresh operand.
2. `src/xpath/ast.rs` — a new `Expr` variant, or a new `BinaryOp` variant
   if it is a binary operator. Prefer desugaring into an *existing* node
   over inventing evaluation logic: `Expr::Arrow` carries no behavior of
   its own, only the version gate — the parser builds an ordinary
   `Expr::Function` or `Expr::DynamicCall` and evaluation just unwraps the
   wrapper. Less surface area, fewer places to get wrong.
3. `src/xpath/parser.rs` — a parse function at the right precedence. If it
   is a **repeating** binary operator (`Operand (Op Operand)*`), use
   `parse_binary_chain`, not a hand-rolled `while` loop — a loop that
   builds a nested `Expr::Binary` tree without charging the shared
   recursion budget for its own length parses instantly and then
   overflows the stack in `evaluate` on a long-enough chain of the *same*
   operator, found by fuzzing nothing more exotic than a few hundred `|`
   in a row. `MAX_RECURSION_DEPTH`'s doc comment and
   [`spec/testing/`](../spec/testing/index.md#what-fuzzing-found) have the
   detail.
4. `src/xpath/eval.rs` — the evaluation arm, if the node isn't pure sugar.
5. `src/schema/compile.rs` — three places, easy to under-count:
   - `check_expression`: the actual `require_v2`/`require_v3` gate, then
     recurse into the children.
   - `check_variables` (and `check_variables_v3` if the construct needs
     its own split, to keep `check_variables` under the line-count lint):
     recurse so a `$name` inside the new construct is still checked.
   - `calls_document_function`: recurse so a `document()` nested inside it
     is still found for the two-pass loading mechanism.
6. `src/lint.rs` — three *exhaustive* matches over `Expr` need a new arm:
   `looks_up_key`, `first_unprefixed_element_name`, `references_variable`.
   The compiler enforces this — `Expr` is not `#[non_exhaustive]` inside
   the crate — but check which kind of match you are editing before
   trusting that: `constant_test` and `following_from_an_attribute`, also
   in `src/lint.rs`, and `root_match_expression` in
   `src/validate/engine.rs`, use a catch-all `_ => …` arm instead, which
   compiles fine while silently *not* looking inside the new construct. A
   catch-all is the right shape for some of these (a lint asking "can this
   possibly be a constant test" should stay conservative), so decide on
   purpose rather than by what the compiler happens to accept.
7. Tests: a lexer unit test for the new token(s), a parser unit test for
   the shape and precedence (see `respects_operator_precedence` and its
   neighbors for the pattern), and integration tests in the matching
   `tests/xpath2.rs`/`tests/xpath3.rs` proving both that it **works** under
   its own binding and that it is a **hard error naming the construct**
   under an earlier one.
8. Document it: the version's own `spec/xpath2/` or `spec/xpath3/`, plus
   whichever of `spec/conformance/`, `spec/roadmap/`, `README.md`, and
   `llms.txt`/`llms.json` mention that version's feature list.

## Add a CLI flag

1. `src/main.rs` — a field on `Cli` with its `#[arg(...)]` and a doc comment,
   which becomes the help text.
2. Wire it through `run()`. Keep the binary thin: real behaviour belongs in
   the library, behind an option type.
3. Document it in the options table in [`spec/cli/`](../spec/cli/index.md).
   `tests/cli.rs::every_cli_flag_is_documented_and_every_documented_flag_exists`
   fails in **both** directions — an undocumented flag and a documented flag
   that does not exist are each a defect.
4. Test the flag's observable effect in `tests/cli.rs`, including its effect
   on the exit code if it has one.

## Add a Schematron element or attribute

1. `src/schema/model.rs` — the type, with rustdoc citing what the standard
   says it means, and `#[cfg_attr(feature = "serde", derive(...))]`.
2. `src/schema/parse.rs` — parse it, and **validate its content model here**:
   required attributes, mutually exclusive attributes. Later passes assume a
   well-formed model.
3. `src/schema/expand.rs` — if it contains expressions, make sure abstract
   pattern parameter substitution reaches them.
4. `src/schema/compile.rs` — if it contains expressions, add them to the
   collection walk so they are compiled once and checked.
5. `src/validate/engine.rs` — the behaviour.
6. `src/svrl.rs` — if it appears in a report.
7. `spec/data-model/` and `spec/conformance/`.
8. A corpus case.

## Change validation behaviour

Change [`spec/validation/`](../spec/validation/index.md) **first**, then the code
to match. The spec is the contract; a behaviour change that lands without it
leaves the two disagreeing and the next reader unable to tell which is right.

Add a corpus case that would fail under the old behaviour.

## Fix a fuzz crash

1. Reproduce: `cargo +nightly fuzz run <target> fuzz/artifacts/<target>/crash-<hash>`
2. Read the input — it is usually tiny and tells you the shape of the defect.
3. Write a **unit test** reproducing it, and name the fuzz target that found
   it in a comment. `cargo test` must cover it afterwards; a nightly-only
   regression test is not covered by CI.
4. Fix the defect. Ask whether the same class of bug exists elsewhere — the
   `not()` arity panic was one instance of "the library trusted a check that
   only one caller performs".
5. Re-run that target for at least 60 seconds.

Not every artifact is a defect, and the two false ones cost real time:

- **`slow-unit-*`**: a fuzz build carries a sanitizer and coverage and runs
  about ninety times slower than release, so an input at a documented limit —
  0.3 s released — is filed as slow at 27 s. Run with `-report_slow_units=30`,
  above the slowest input the limits permit. What matters is whether the work
  is *bounded*, not whether it is fast.
- **`oom-*`**: `-rss_limit_mb` measures the whole process, and the fuzzer's own
  corpus and coverage are most of it — 437 MB before a single iteration for
  `fuzz_validate`. A ceiling near that files OOMs that reproduce in a
  millisecond using 44 MB. Leave the default, or measure the baseline first.

A genuine resource defect looks different: the nested-range denial of service
was found *as* a slow unit, and the tell was that the input was 79 bytes and
the work grew without bound.

## Add a lint

1. `src/lint.rs` — a `LintKind` variant with a kebab-case `as_str()`, and the
   check itself.
2. Every lint needs a **location**, a message naming what is wrong, and a
   `help` line saying what to do. A lint without help is a complaint.
3. Unit-test both directions: that it fires on the bad shape, and that it does
   **not** fire on the legitimate shape that resembles it. The second matters
   more — a linter with false positives gets switched off, after which it
   catches nothing at all.
4. Document it in the table in [`spec/linting/`](../spec/linting/index.md).
5. Check `cargo run -- --schema examples/invoice.sch --lint` is still clean;
   `tests/cli.rs` asserts the bundled example models good practice.

Prefer a conservative check. `UnreachableRule` decides shadowing by pairwise
subsumption — the earlier rule must carry no predicates, and its steps must
generalise a suffix of the later one's — because deciding general subsumption
of XPath patterns is not practical and a guess would be a false positive. It
was three hardcoded contexts before that, and the hardcoded version had one:
it treated `@*` as claiming every node, so an element rule after it was called
unreachable although both fire.

**Is it a lint or a portability finding?** `lint()` is for constructs that are
probably *wrong*. `portability()` is for constructs that are *correct here*
and behave differently under another processor. A portability finding needs a
divergence recorded in [`spec/conformance/`](../spec/conformance/index.md),
established by running both implementations — not a suspicion — and its test
should point at the corpus case that demonstrates the divergence rather than
at a schema written to match. They are separate because a check that reports
correct code, mixed into the linter, teaches its reader to ignore the linter.

## Investigate "this schema does nothing"

Two causes account for nearly all of them:

```sh
schematron -s rules.sch --verbose data.xml   # did any rule fire at all?
schematron -s rules.sch --explain            # what will this schema do?
```

- **No rules fired** → a namespace problem. XPath 1.0 has no default
  namespace, so unprefixed contexts match elements in *no* namespace. The
  schema needs `<ns prefix="..." uri="..."/>` and prefixed contexts.
- **Rules fired, but not the one expected** → an earlier rule in the same
  pattern claimed the node. `--explain` marks every rule that can only see
  leftovers.

`--lint` detects both automatically and needs no document, so reach for it
first.

## Bump the MSRV

Follow [`spec/rust-msrv-n-minus-2/`](../spec/rust-msrv-n-minus-2/index.md)
exactly. The step that matters is actually running the boundary toolchain;
updating the numbers without it produces a value that looks maintained and is
not.

Expect clippy to surface **new** lints after a bump: raising the floor unlocks
standard-library APIs that `clippy::incompatible_msrv` was suppressing. That
is the policy working, not a problem. Apply them.

## Add a dependency

Usually: don't. The crate depends on `quick-xml`, `thiserror`, and `regex`,
plus `serde`/`serde_json`/`clap` behind default features, and that is a
feature of it.

If you must: check the dependency's own MSRV fits ours, state the reason in
the pull request, and confirm it does not pull in C.

## Release

1. Version in `Cargo.toml`. In 0.x, a breaking change is a **minor** bump:
   `^0.4` does not accept `0.5.0`.
2. [`CHANGELOG.md`](../CHANGELOG.md). Lead with what changes for someone who
   already depends on the crate — behaviour first, then additions, then fixes.
   A release that alters output says so plainly.
3. `cargo package --list` — check nothing unwanted is included and that
   `spec/` and `examples/` still are.
4. All four gates, plus the differential suite with `SCHEMATRON_SKELETON` set,
   plus `cargo bench` against a saved baseline.
5. `cargo publish --dry-run`, then `cargo publish`. An agent working in this
   repository is authorized to run the real publish, not only the dry run —
   see [`spec/trusted-publishing/`](../../spec/trusted-publishing/index.md).
   It is still irreversible (crates.io allows yanking, not deletion): do not
   skip steps 3–4 to get here faster.
6. Tag `v<version>` and push it alongside the commit.
