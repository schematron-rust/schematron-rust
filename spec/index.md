# Schematron Rust crate — specification

`schematron` is a **pure Rust** implementation of ISO/IEC 19757-3 Schematron:
rule-based validation of XML documents using XPath assertions.

Pure Rust means: no `libxml2`, no `libxslt`, no Saxon, no XSLT transpilation
step, no C toolchain, no FFI. The crate contains its own XML parser, its own
XML data model, its own XPath 1.0 engine, and its own Schematron validator.

## Status

| Component | Spec | Status |
|---|---|---|
| XML data model and parser | [xml.md](xml.md) | Implemented |
| XPath 1.0 engine | [xpath.md](xpath.md) | Implemented |
| XPath 2.0 support | [xpath2.md](xpath2.md) | Phase 1 |
| Schematron data model | [data-model.md](data-model.md) | Implemented |
| Schema parsing, include, abstract expansion | [parsing.md](parsing.md) | Implemented, less `extends href` |
| Validation semantics | [validation.md](validation.md) | Implemented |
| SVRL report output | [svrl.md](svrl.md) | Implemented |
| Schema linting | [linting.md](linting.md) | Implemented |
| Library API | [api.md](api.md) | Implemented |
| Command line interface | [cli.md](cli.md) | Implemented |
| Errors | [errors.md](errors.md) | Implemented |
| Testing | [testing.md](testing.md) | Implemented |
| Conformance and limits | [conformance.md](conformance.md) | Implemented |
| Rust MSRV policy | [rust-msrv-n-minus-3.md](rust-msrv-n-minus-3.md) | Implemented |
| Agents directory naming | [agents-directory-name-is-lowercase.md](agents-directory-name-is-lowercase.md) | Implemented |
| Tutorial | [tutorial.md](tutorial.md) | — |
| Roadmap | [roadmap.md](roadmap.md) | — |

One gap is deliberate for this version and is recorded in
[conformance.md](conformance.md) rather than left to be discovered:
`extends href`, for which `include` serves. Query bindings above XPath 1.0 are
refused rather than approximated.

## What Schematron is

Schematron is a rule-based validation language for making assertions about
the presence or absence of patterns in XML documents. Unlike grammar-based
schema languages such as DTD, XML Schema, and RELAX NG, which describe what a
document's tree *may* contain, Schematron describes what a document *must*
satisfy, using arbitrary XPath expressions.

This makes Schematron able to express constraints that grammars cannot:

- Co-occurrence constraints ("if `@type` is `invoice` then `total` is required").
- Cross-references between distant parts of a document.
- Value relationships ("`end` must not precede `start`").
- Cardinality that depends on content, not position.
- Constraints across multiple documents.

Schematron is therefore usually layered *on top of* a grammar, not used
instead of one.

A schema that runs on this crate:

```xml
<schema xmlns="http://purl.oclc.org/dsdl/schematron">
   <pattern>
      <title>Contract rules</title>
      <rule context="Contract">
         <assert test="ContractDate">A contract must carry a date.</assert>
         <assert test="string-length(ContractDate) = 10">A contract date must be YYYY-MM-DD.</assert>
      </rule>
   </pattern>
</schema>
```

The example most often quoted for Schematron — the one in the Wikipedia
article — instead reads:

```text
<rule context="Contract">
   <assert test="ContractDate > current-date()">ContractDate should be
in the past because future contracts are not allowed.</assert>
</rule>
```

That one **does** now run, under an `xslt2` query binding — `current-date()`
and the date types arrived in XPath 2.0 phase 2b, see [xpath2.md](xpath2.md):

```xml
<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt2">
   <pattern>
      <rule context="Contract">
         <assert test="ContractDate &lt; current-date()">ContractDate should be
 in the past because future contracts are not allowed.</assert>
      </rule>
   </pattern>
</schema>
```

Note the comparison had to be turned around. As quoted, the example is
self-contradictory: it tests that the date is in the *future* while its
message says the date should be in the past. Schematron will happily let you
write an assertion whose message contradicts its test — nothing checks the
prose against the XPath. That is worth remembering every time you write one,
and it is why the crate's own linter reports an assertion with no message but
cannot report one with the wrong message.

## Standardisation history

Schematron was created by Rick Jelliffe in 1999. It became an international
standard as part of the Document Schema Definition Languages (DSDL) family:

| Edition | Notes |
|---|---|
| Schematron 1.5 (2000) | Pre-ISO; namespace `http://www.ascc.net/xml/schematron` |
| ISO/IEC 19757-3:2006 | First ISO edition; namespace `http://purl.oclc.org/dsdl/schematron` |
| ISO/IEC 19757-3:2016 | Adds `xslt2` query binding, `properties`/`property` |
| ISO/IEC 19757-3:2020 | Adds `xslt3` query binding |
| ISO/IEC 19757-3:2025 | Current edition |

This crate targets the ISO namespace and the `xslt` / `xpath` query language
bindings, which are XPath 1.0. See [conformance.md](conformance.md).

## Design principles

1. **Pure Rust, no C.** Everything from bytes to report is Rust.
2. **Interpretation, not transpilation.** The reference implementation of
   Schematron compiles a schema into XSLT and runs it. That requires an XSLT
   processor. This crate instead interprets the schema directly against an
   XPath engine, which removes the XSLT dependency entirely and makes error
   reporting far more direct.
3. **The standard is the contract.** Where the standard states a behaviour,
   the crate implements that behaviour, and the spec files here cite it.
   Where the crate deliberately diverges or does not yet reach, that is
   recorded in [conformance.md](conformance.md) rather than left implicit.
4. **Library first, CLI second.** The CLI is a thin shell over the library.
5. **Reports are data.** A validation result is a Rust value that can be
   rendered as SVRL, as JSON, or as human text — not a pile of strings.

## Reading order

New to Schematron: start with [tutorial.md](tutorial.md), which walks from one
rule to a schema that pulls its weight.

Working on the crate: read [data-model.md](data-model.md), then
[validation.md](validation.md). Those two carry the semantics of the language.
The rest is machinery.

Deciding whether to depend on it: [conformance.md](conformance.md) first.
