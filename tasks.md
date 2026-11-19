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
