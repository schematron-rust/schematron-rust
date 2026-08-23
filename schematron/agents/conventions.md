# Conventions

How code and prose are written here. Match the surrounding file; when in
doubt, this document decides.

## Comments explain why, not what

The single most important rule. A comment that restates the code is noise
that will go stale and mislead someone. A comment that explains a decision is
the thing a reader cannot reconstruct.

Bad, and would be rejected:

```rust
// Increment the counter.
self.order += 1;
```

Good, and is real code from `src/xml/node.rs`:

```rust
/// One-based index among the siblings sharing this node's kind and
/// expanded name.
///
/// Precomputed because generating a location for every finding would
/// otherwise rescan the parent's child list once per finding, which is
/// quadratic on a document with many siblings — exactly the shape
/// Schematron is usually pointed at.
pub(crate) sibling_position: usize,
```

Comment density should match the surrounding file. Dense in the XPath
evaluator, where the specification's rules are unintuitive; sparse in the
model types, where the names carry the meaning.

Cite the standard where behaviour comes from it, and cite the *reason* where
behaviour comes from a decision. When a comment records something learned the
hard way — a benchmark result, a fuzz finding — say so; that is what stops it
being "simplified" away later.

## Rustdoc

`missing_docs` is a warning and `cargo doc` must stay clean, so every public
item is documented. Beyond that:

- Lead with what the item is **for**, not with its type signature restated.
- Give an `# Examples` block for anything a caller will actually reach for.
  Doc examples are compiled and run by `cargo test`, so they cannot rot.
- Use `# Errors` on anything returning `Result`, saying which variants and
  when.
- Where a surprise exists, document it at the point of surprise. `Value`
  documents XPath's number format; `Report::is_valid` documents that a
  successful report is not a failure.

## Naming

- Say the domain word: `Assertion`, `Pattern`, `Phase`, `Diagnostic` mean what
  ISO/IEC 19757-3 says they mean. Do not invent synonyms.
- Test names are sentences about behaviour, not labels:
  `within_a_pattern_only_the_first_matching_rule_fires`, not `test_rules`.
  A failing test name should tell you what broke without opening the file.
- `from_str` / `from_bytes` / `from_path` for constructors;
  `with_*` for builder methods; `to_*` for renderers.

## Errors

Every error names what failed **and where**. The `context` string in an XPath
error is a path into the schema:

```
pattern[@id='lines']/rule[@context='line']/assert[2]/@test
```

Where a message can teach, let it. `matches()` does not report "unknown
function" — it reports that it is an XPath 2.0 function and that this crate
implements XPath 1.0. An undeclared prefix error lists the prefixes the schema
*does* declare. That is the difference between an error that ends someone's
afternoon and one that ends their minute.

## Lints

`clippy::pedantic` is on crate-wide and CI runs it with `-D warnings`.

Four lints are allowed crate-wide in `Cargo.toml`, each with a written
reason — `doc_markdown`, `float_cmp`, `module_name_repetitions`,
`format_push_string`. **Do not add a fifth without a comment explaining why**,
and prefer a narrow `#[allow]` at the item with a one-line reason over a
crate-wide allow.

Existing narrow allows all carry their reason:

```rust
// `content` and `context` are both the right names for what they hold, and
// renaming either to satisfy the similarity heuristic would read worse.
#[allow(clippy::similar_names)]
```

## Prose style in `spec/` and `agents/`

- Second person, present tense, active voice.
- Tables for anything enumerable; prose for anything that needs a reason.
- State limits and gaps **up front**, not in a footnote. A reader deciding
  whether to depend on this crate should find the bad news early.
- Every code block that claims to be a schema must actually compile —
  `tests/docs.rs` checks this. Fence non-compiling sketches as ```text.
- No volatile numbers in prose. Test counts and line counts rot within a day;
  reference the command that produces them instead. Benchmark figures are the
  exception and are marked indicative.

## Formatting

`cargo fmt` defaults. Comment and doc text wraps at 79 columns to match the
Markdown in `spec/`; code wraps where rustfmt puts it.
