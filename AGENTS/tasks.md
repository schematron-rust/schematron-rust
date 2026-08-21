# Task recipes

Step-by-step for the jobs that come up. Each ends with the same gate: the four
commands in [`AGENTS.md`](../AGENTS.md).

## Add a Schematron semantics test

Almost always a corpus case, not Rust. See
[`testing.md`](testing.md#adding-a-corpus-case). One directory, three files,
no code.

## Add an XPath function

1. `src/xpath/functions.rs` — add to `SIGNATURES` with its `(name, min, max)`
   arity. `None` as max means unbounded.
2. Add an arm to `call()`. Arity is already checked at entry, so you may index
   `args` up to the declared minimum — and no further.
3. Unit-test it in `src/xpath/eval.rs`, exercising the type conversions, not
   just the happy path.
4. Add it to the function list in [`spec/xpath.md`](../spec/xpath.md).
   `tests/docs.rs::the_xpath_function_list_in_the_spec_matches_the_engine`
   fails if you forget.

For an **XPath 2.0** function, use `SIGNATURES_V2` instead. It becomes
available only under an `xslt2` or `xpath2` query binding, because
`check_function` gates on `XPathVersion`. Document it in the table in
[`spec/xpath2.md`](../spec/xpath2.md);
`tests/docs.rs::the_xpath_two_function_list_in_the_spec_matches_the_engine`
fails if you forget.

If the function needs something phase 1 does not have — the sequence type, or
dates — do **not** approximate it. Add it to `V2_FUNCTIONS_NEEDING_SEQUENCES`
or `V2_FUNCTIONS_NEEDING_DATES` so the error says what it would take, and see
roadmap item 1.

## Add a CLI flag

1. `src/main.rs` — a field on `Cli` with its `#[arg(...)]` and a doc comment,
   which becomes the help text.
2. Wire it through `run()`. Keep the binary thin: real behaviour belongs in
   the library, behind an option type.
3. Document it in the options table in [`spec/cli.md`](../spec/cli.md).
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
7. `spec/data-model.md` and `spec/conformance.md`.
8. A corpus case.

## Change validation behaviour

Change [`spec/validation.md`](../spec/validation.md) **first**, then the code
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

## Add a lint

1. `src/lint.rs` — a `LintKind` variant with a kebab-case `as_str()`, and the
   check itself.
2. Every lint needs a **location**, a message naming what is wrong, and a
   `help` line saying what to do. A lint without help is a complaint.
3. Unit-test both directions: that it fires on the bad shape, and that it does
   **not** fire on the legitimate shape that resembles it. The second matters
   more — a linter with false positives gets switched off, after which it
   catches nothing at all.
4. Document it in the table in [`spec/linting.md`](../spec/linting.md).
5. Check `cargo run -- --schema examples/invoice.sch --lint` is still clean;
   `tests/cli.rs` asserts the bundled example models good practice.

Prefer a conservative check. `UnreachableRule` reports only the three
contexts that certainly claim everything, because general subsumption of XPath
patterns is not practical to decide and a guess would be a false positive.

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

Follow [`spec/rust-msrv-n-minus-3.md`](../spec/rust-msrv-n-minus-3.md)
exactly. The step that matters is actually running the boundary toolchain;
updating the numbers without it produces a value that looks maintained and is
not.

Expect clippy to surface **new** lints after a bump: raising the floor unlocks
standard-library APIs that `clippy::incompatible_msrv` was suppressing. That
is the policy working, not a problem. Apply them.

## Add a dependency

Usually: don't. The crate depends on `quick-xml` and `thiserror`, plus
`serde`/`clap` behind default features, and that is a feature of it.

If you must: check the dependency's own MSRV fits ours, state the reason in
the pull request, and confirm it does not pull in C.

## Release

1. Version in `Cargo.toml`.
2. `cargo package --list` — check nothing unwanted is included and that
   `spec/` and `examples/` still are.
3. All four gates, plus `cargo bench` for a regression check.
4. `cargo publish --dry-run`.
