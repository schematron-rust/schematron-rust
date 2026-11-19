# Production readiness

Use whole-document validation for schemas that inspect a batch from its
wrapper or compare records. Use streaming when each rule can decide its
result from the current record and its wrapper attributes. The schema check
runs before input is read. The validator checks wrapper expressions when
a rule actually fires on the wrapper.

## Assessment

| Area | Evidence in this checkout | Production implication |
|---|---|---|
| Validation semantics | The corpus checks findings and locations. The differential suite compares these with the ISO skeleton and records known divergences. | Use both when changing the meaning of an expression. A validator can return a plausible report with the wrong result. |
| Streaming | A wrapper assertion that tests for an `order` child now raises a streaming error. Attribute-only wrapper checks agree with whole-document validation. Positional match contexts and paths that leave a record are refused. | Use whole-document mode for batch totals, cross-record constraints and rules outside the supported streaming subset. |
| International text | The French amount fixture exercises narrow nonbreaking space in a number and nonbreaking space before a currency symbol. It also checks an ID list through both XPath argument forms. | Preserve the author's code points wherever XML treats them as data. Compare exact results in fixtures. |
| Encodings | File input through the library, CLI and default resolver shares the UTF-8 and BOM-marked UTF-16 decoder. A fixture follows a relative include and an external instance through non-ASCII paths. | Schema authors can use either UTF-16 byte order. Streamed instance input requires UTF-8. |
| Distribution | An external application consumes the crate archive with each advertised feature combination. The locked all-feature suite, Clippy and documentation checks passed locally on Apple silicon macOS and in an ARM64 Linux container. Hosted Linux, Windows and macOS jobs passed on the readiness revision. | Keep platform-specific path and CLI checks in the required hosted matrix. |
| Test coverage | The complete all-feature Rust suite is measured with source-based LLVM coverage and CI rejects line coverage below 95%. | Treat the percentage as a regression alarm. Preserve semantic, corpus, differential and fuzz evidence rather than optimizing only for execution counts. |

## Next evidence to collect

1. **Streaming coverage.** When users need a currently refused but
   record-local pattern, capture the real schema and document. Extend the
   checker with a paired whole-document test covering findings, order and
   locations.

The streaming measurement isolates report retention in a million-order run.
Peak resident memory was 279 MiB for full SVRL, 24 MiB for findings-only
SVRL, and 5.1 MiB for findings-only SVRL capped at ten failures.
[Workload and measurements](schematron/spec/streaming/index.md#measured-peak-memory)

The [task list](tasks.md) gives acceptance checks. The [crate specification](schematron/spec/index.md)
defines runtime behavior and the [repository specification](spec/index.md)
defines project policy.
