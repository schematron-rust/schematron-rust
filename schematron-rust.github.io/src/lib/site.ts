// Facts about the crate that more than one page needs.
//
// Anything here that duplicates the crate repo — the version, the spec
// table of contents, the CLI options, the conformance rows — is copied from
// schematron-rust and must be kept in step with it. See AGENTS.md.

export const CRATE = 'schematron';
export const VERSION = '0.5.0';
export const MSRV = '1.94';
export const REPO = 'https://github.com/schematron-rust/schematron-rust';
export const CRATES_IO = 'https://crates.io/crates/schematron';
export const DOCS_RS = 'https://docs.rs/schematron';
export const LICENSE = 'MIT OR Apache-2.0 OR GPL-2.0-only OR GPL-3.0-only';

/** A file under `spec/` in the crate repo. `spec/` is normative. */
export type SpecDoc = { file: string; title: string; covers: string };

export const SPEC_DOCS: SpecDoc[] = [
  { file: 'index.md', title: 'Overview', covers: 'Design principles and a reading order.' },
  { file: 'tutorial.md', title: 'Tutorial', covers: 'Eighteen steps from one rule to a real schema.' },
  { file: 'data-model.md', title: 'Data model', covers: 'Every Schematron element and its Rust type.' },
  { file: 'validation.md', title: 'Validation', covers: 'The validation algorithm, exactly.' },
  { file: 'xpath.md', title: 'XPath 1.0', covers: 'The XPath 1.0 engine: axes, functions, conversions.' },
  { file: 'xpath2.md', title: 'XPath 2.0 subset', covers: 'What is in, what is out, and where 1.0 semantics still apply.' },
  { file: 'xml.md', title: 'XML', covers: 'The XML parser and its data model.' },
  { file: 'parsing.md', title: 'Parsing', covers: 'The five schema compilation passes.' },
  { file: 'svrl.md', title: 'SVRL', covers: 'The report format, read and written.' },
  { file: 'keys.md', title: 'Keys', covers: 'Keys, and why a cross-reference check needs one.' },
  { file: 'linting.md', title: 'Linting', covers: 'Catching schemas that silently do nothing.' },
  { file: 'api.md', title: 'API', covers: 'The library surface.' },
  { file: 'cli.md', title: 'CLI', covers: 'Every option, and every exit code.' },
  { file: 'errors.md', title: 'Errors', covers: 'The error taxonomy, and error versus finding.' },
  { file: 'conformance.md', title: 'Conformance', covers: 'Limits and divergences, stated up front.' },
  { file: 'testing.md', title: 'Testing', covers: 'Tests, fuzzing, benchmarks, lints.' },
  { file: 'rust-msrv-n-minus-3.md', title: 'MSRV policy', covers: 'Current stable minus three.' },
  { file: 'roadmap.md', title: 'Roadmap', covers: 'Shipped, next, and not planned.' }
];

export function specUrl(file: string): string {
  return `${REPO}/blob/main/spec/${file}`;
}
