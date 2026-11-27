# Production readiness tasks

## Completed in this assessment

- [x] Preserve nonbreaking spaces in XPath `normalize-space()` and `id()`.
  Exercise XML whitespace and multilingual text in the [shared corpus case](schematron/tests/corpus/xml-whitespace/schema.sch).
- [x] Accept both empty-wrapper spellings in streaming mode. Reject a second
  document element, misplaced CDATA and non-XML whitespace outside the root
  through both readers. Cover legal epilog content in the [streaming tests](schematron/tests/streaming.rs).
- [x] Refuse `preceding-sibling::` in streaming rules. Earlier records have
  already been discarded when a later record is evaluated.
- [x] Repair the root MSRV target and use Cargo's binary path in the
  differential harness so the reference comparison works with alternate
  test build layouts.
- [x] Return a named streaming error for wrapper child reads, cross-record
  parent paths and positional match contexts. Keep attribute-only wrapper
  checks and per-record child rules working. Cover an explicit single-pattern
  phase when a failure cap is set.
- [x] Keep recycling record storage after reaching a failure cap. Compare
  peak live arena slots and capacity for 10 and 10,000 trailing records.
  Verify that malformed trailing XML still produces a parse error and CLI
  exit 4.
- [x] Use one decoder for whole-document file loading. Compare UTF-8,
  UTF-16LE and UTF-16BE through schema compilation, a relative fragment,
  an external instance and CLI SVRL output. Check path-bearing decoding
  errors for truncated input.
- [x] Exercise a consumer of the crate archive with defaults, no defaults,
  `serde` only and `cli` only. Compare SVRL across all four. Run the shipped
  binary in its default and CLI-only builds. [Packaged feature test](bin/test-packaged-features)
- [x] Add native Windows and macOS CI alongside Linux and exercise paths
  containing spaces and non-ASCII characters. Let Rust choose the native
  debug-information format on each platform.
- [x] Use the toolchain installed by CI for tests, Clippy and docs. The
  hosted failure occurred when the checkout pin selected a different
  toolchain from the one on which setup had installed Clippy.
- [x] Measure peak CLI memory for full SVRL, findings-only SVRL and capped
  findings. Skip retaining passing rule firings when output omits them.
  [Streaming measurements](schematron/spec/streaming/index.md#measured-peak-memory)
- [x] Run the locked all-feature crate suite, Clippy and documentation
  checks locally on macOS and ARM64 Linux.
- [x] Measure line coverage across the complete all-feature Rust suite, add
  behavior-focused CLI tests for standard input and incompatible execution
  modes, and enforce a 95% minimum locally and in hosted CI.
- [x] Raise the complete all-feature suite to exact source coverage. Exercise
  every function and merged executable region, document LLVM's duplicate
  Rust code-generation accounting, and enforce the 100% gate locally and in
  hosted CI without excluding files or narrowing the suite.
- [x] Confirm the Linux, Windows and macOS hosted jobs on the readiness
  revision, including the non-ASCII path fixture. All jobs passed in
  [hosted run 44513735145](https://github.com/schematron-rust/schematron-rust/actions/runs/44513735145).
- [x] Enforce repository specification discoverability from the existing
  documentation audit. Require every root `spec/*/` directory to contain an
  `index.md` linked from `spec/index.md`, while allowing the published crate
  archive to omit monorepo governance documents.
- [x] Close GitHub issue #25 by adding the XPath 2.0 atomic constructor
  functions already supported through `cast as`, including empty-sequence,
  cardinality, invalid lexical form, type-tag and XPath 1.0 refusal tests.
- [x] Finish the SvelteKit 3 migration audit. Confirm `page.url` is only read,
  confirm the site does not require cross-origin development assets, clear all
  migration markers, and pass typecheck, static build, and browser tests.
- [x] Close the disclosed repository-governance gaps with complete license
  texts included in crate archives, a root license index, a formal maintainer
  record, and a private vulnerability-reporting policy.
- [x] Remove website metadata drift: synchronize the displayed crate version
  with `Cargo.toml`, enforce it in the monorepo site tests, and publish
  canonical Open Graph title and URL metadata for share crawlers.

## Audit backlog (assessed 2026-11-26)

Sources: the GPT-6.2 Astra (high) production audit and the Mistral Medium 3.5
(high) improvement list. Each finding below was checked against this checkout
before it was recorded. The audit files have been removed; this section is
the record.

### Confirmed defects, in order

- [x] **A01 (P0) `--flag` with `--max-failures` approved invalid documents.**
  The cap counted every failed assertion before `--flag` filtered the report,
  so an earlier warning used it up. The CLI then printed `no findings` and
  exited 0. `ValidateOptions::failure_flags` now limits what counts toward
  the cap, and the CLI sets it from `--flag`. Covered by
  `tests/cli.rs::max_failures_counts_only_the_flags_being_reported` and
  `tests/validation.rs::failure_flags_limit_what_counts_toward_the_cap`.
- [x] **A02 (P0) Schema-level variables are re-read from `pattern/@documents`
  targets.** `<let name="v" value="/doc/item"/>` on the schema, read in a
  pattern with `documents="'ext.xml'"`, yields the external document's
  `/doc/item` ("external"), not the primary one's ("primary"). Bind
  schema-level and phase-level `let` once against the instance document and
  keep those values across targets. Acceptance: a passing and a failing
  cross-document corpus case and a differential case against the skeleton.
  Fixed by merging each target beside the instance in one arena, as
  `document()` does, and scoping the walk, `/`, keys and locations to the
  target root. Covered (sequential and parallel) by
  `tests/schema.rs::schema_variables_keep_the_instance_nodes_inside_a_documents_pattern`.
  A differential case still needs the skeleton (`SCHEMATRON_SKELETON`), which
  is not installed locally.
- [x] **A03 (P1) Duplicate attributes are accepted.** `<d a="1" a="2"/>`
  parses and `count(@*)` is 2. XML 1.0 WFC "Unique Att Spec" makes this a
  parse error (exit 4). Fix both the tree and streaming readers.
- [x] **A04 (P1) Attribute-value normalization is missing.** A literal tab in
  `a="x<TAB>y"` survives (`contains(@a, '&#9;')` is true). XML 1.0 §3.3.3
  replaces literal tab, CR and LF in attribute values with a space, but a
  character reference such as `&#9;` is kept as written. Test both forms in both readers.
- [x] **A05 (P1) End-of-line handling is missing.** `<d>a CR LF b</d>` has
  `string-length(.)` of 4. XML 1.0 §2.11 normalizes CRLF and lone CR to LF
  before parsing (3). Check text, attributes, comments, PIs and CDATA, and confirm
  locations (line numbers) still agree.
  A03–A05 are fixed in the shared `Builder`/`handle_event` used by both
  readers. Covered by
  `tests/streaming.rs::xml_conformance_is_the_same_in_both_readers`.
  Line numbers are unchanged because normalization applies to values, not to
  the positions in the source.
- [x] **A06 (P1) Distinct processing instructions share one SVRL location.**
  `<d><?a x?><?b y?></d>` reports both PIs at
  `/d[1]/processing-instruction()[1]`. Count the position among PI siblings
  (preferably `processing-instruction('a')[1]`). Check comments and text
  nodes for the same defect.
  Fixed: positions were already counted per target, but the step omitted
  the target. Comments and text have no name and were already correct. The
  `processing-instruction-locations` corpus case checks every location
  with `count(LOCATION) = 1`.
- [x] **A07 (P1) Multi-document CLI runs emit invalid SVRL/JSON.** Two inputs
  with `-f svrl` produce two concatenated XML documents (two XML
  declarations). JSON concatenates the same way. Choose one machine-readable shape per
  invocation, for example a JSON array and an SVRL wrapper element, or refuse
  more than one document for `svrl`/`json`. Specify it in `spec/cli/`, then
  test that standard readers parse the output.
  Resolved by refusal: with more than one document, `svrl` and `json` exit 2
  unless `--quiet` is given (`spec/cli/#several-documents`). A wrapper format
  (a JSON array, an SVRL container) remains possible later if users ask for
  one-run batch reports. Covered by
  `tests/cli.rs::machine_readable_reports_refuse_several_documents`.

The GPT audit said it recorded 12 tasks, but the file held only the summary
above, so A02–A07 are the confirmed findings it summarized. Its stated
mitigations for existing deployments still apply until each fix lands. Omit
the cap when filtering by flag (fixed now), emit one machine report per
invocation, and qualify cross-document rules against known passing and failing
inputs.

### Open GitHub issue

- [ ] **#28 Exact `xs:decimal`.** Under XPath 2.0, `0.1 + 0.2 = 0.3` is false
  because decimals are tagged `f64`. This is a representation change across
  arithmetic, comparison, `sum()`/`avg()` and formatting, not a local patch.
  Specify it in `spec/xpath2/` before writing code.

### Engineering gaps worth doing (from the Mistral list, verified absent)

- [x] **Format check in CI.** `.github/workflows/ci.yml` does not run
  `cargo fmt --check`.
  Done: the crate was reformatted in one mechanical sweep (49 files), and the
  `Format` step runs first in the crate job.
- [x] **Dependency audit in CI.** There is no `cargo deny` or `cargo audit`
  step. Dependabot (`.github/dependabot.yml`) covers updates but not license
  or advisory gating.
  Done: `schematron/deny.toml` and the `Dependency policy (cargo deny)` CI job.
  Locally it reports advisories, bans, licenses and sources all ok. Removing
  `Unicode-3.0` from the allow list rejects `unicode-ident`, so the license
  policy is live. Hosted run 45911111453 on `audit-fixes-2026-11` passed every
  job, the new Format step and cargo deny job included.
- [x] **Mutation testing (`src/validate`, `src/xml`).** Coverage is 100% by execution, and
  `cargo-mutants` would show whether the assertions catch changed behavior.
  Run it on `src/xpath` and `src/validate`, and record the survivors as tasks
  instead of gating on a score.
  First run (cargo-mutants 27.1, `-f 'src/validate/**' -f 'src/xml/**'`, 403
  mutants in 15 min at `-j 6`): 310 caught, 7 timeouts, 68 unviable, 18
  missed. Eleven tests now close 14 of the misses: parallel chunk offsets
  with 100 patterns, streaming with `record_fired_rules(false)` and a cap
  inside a record, the `simple_descendant_test` fast path,
  `count_fired_rules`, namespace and attribute order in a merged document,
  `is_descendant_of` on itself, namespace scope at an end tag, and the exact
  nesting limit. The four survivors are equivalent mutants, with no
  observable effect:
  - `engine.rs` `validate_streaming` `>=` → `<`. The break follows the
    only active pattern, because a cap with several patterns is refused.
  - `engine.rs` `validate_once` `active.len() > 1` → `<`. Parallel and
    sequential reports are identical by contract.
  - `document.rs` `subtree_capacity` → `0` or `1`. It is only a
    `Vec` capacity hint.

  The copied tree omits the repository root, so run with
  `-- --all-features -- --skip every_relative_link_resolves`, because that
  docs test checks links outside the crate.
- [x] **Mutation testing (`src/schema`).** 386 mutants in 27 min: 318
  caught, 5 timeouts, 33 unviable, 30 missed. New tests close 26 of the misses:
  - the kind-test name in a 1.0 error, and the `replace()` literal-regex check;
  - `document()` detection inside if, range, for, let, quantified, a named
    function reference and a dynamic call;
  - streaming refusals for a union, `text()`, `resolve-uri#1` and the pattern
    label, plus a wrapper comparison with a child;
  - parameter name characters, include fragments matching only by id, foreign
    elements in messages, `lang` without `xml:`, the exact extends depth limit,
    and `MemoryResolver::rebase`.

  The four survivors are equivalent:
  - `expand.rs` `substitute`: the early return (`||` → `&&`) is only a
    shortcut, and `index < len` → `<=` stops at the same place, because
    `is_char_boundary(len)` is true.
  - `include.rs` `next_order` → `0` or `1`: the rebuilt schema tree is
    read through child lists, never by document order.
- [ ] **Mutation testing (`src/xpath`).** 1,745 mutants, about two hours at
  `-j 6`. Run it in shards (`--shard k/4`) and triage the survivors the
  same way.
- [x] **Bound the echoed expression in errors.** An XPath nesting error prints
  the entire offending expression (200 KB for a hostile schema). Truncate it
  around the caret position. Done: 60 characters either side, with `…` at each cut.
  The full source stays in `Error::XPathSyntax::expression`.

### Already in place, or not actionable here (Mistral items not carried forward)

- Already done in this checkout: SECURITY.md and private reporting, coverage
  gating (100%), Linux/macOS/Windows CI plus MSRV, differential testing
  against the ISO skeleton, five fuzz targets, criterion benchmarks, a tutorial
  and "why" pages on the site, crates.io keywords and categories,
  contribution and maintainer guides, and XXE being structurally impossible.
  Both input-nesting limits exist (1024 XML levels, 32 XPath levels) and were
  confirmed with hostile inputs. XPath iteration has a shared budget.
- Already deferred with a recorded prerequisite: `no_std` (see
  [plan.md](plan.md)).
- Outside the repository, so the maintainer has to decide: awesome-list and Reddit
  submissions, social accounts, a chat server, workshops, videos, a logo or
  press kit, testimonials, analytics and download dashboards.
- Possible later, needing a design decision first: a WASM build and browser
  playground, property-based tests that complement the fuzzers, and
  XPath 3.1 maps and arrays (see `spec/xpath3/`).
