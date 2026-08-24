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
  { file: 'tutorial/index.md', title: 'Tutorial', covers: 'Eighteen steps from one rule to a real schema.' },
  { file: 'data-model/index.md', title: 'Data model', covers: 'Every Schematron element and its Rust type.' },
  { file: 'validation/index.md', title: 'Validation', covers: 'The validation algorithm, exactly.' },
  { file: 'xpath/index.md', title: 'XPath 1.0', covers: 'The XPath 1.0 engine: axes, functions, conversions.' },
  { file: 'xpath2/index.md', title: 'XPath 2.0 subset', covers: 'What is in, what is out, and where 1.0 semantics still apply.' },
  { file: 'xml/index.md', title: 'XML', covers: 'The XML parser and its data model.' },
  { file: 'parsing/index.md', title: 'Parsing', covers: 'The five schema compilation passes.' },
  { file: 'svrl/index.md', title: 'SVRL', covers: 'The report format, read and written.' },
  { file: 'keys/index.md', title: 'Keys', covers: 'Keys, and why a cross-reference check needs one.' },
  { file: 'linting/index.md', title: 'Linting', covers: 'Catching schemas that silently do nothing.' },
  { file: 'api/index.md', title: 'API', covers: 'The library surface.' },
  { file: 'cli/index.md', title: 'CLI', covers: 'Every option, and every exit code.' },
  { file: 'errors/index.md', title: 'Errors', covers: 'The error taxonomy, and error versus finding.' },
  { file: 'conformance/index.md', title: 'Conformance', covers: 'Limits and divergences, stated up front.' },
  { file: 'testing/index.md', title: 'Testing', covers: 'Tests, fuzzing, benchmarks, lints.' },
  { file: 'rust-msrv-n-minus-3/index.md', title: 'MSRV policy', covers: 'Current stable minus three.' },
  { file: 'agents-directory-name-is-lowercase/index.md', title: 'Agents directory naming', covers: 'Why the agent documentation directory is lowercase.' },
  { file: 'roadmap/index.md', title: 'Roadmap', covers: 'Shipped, next, and not planned.' }
];

/** How a spec document is written in prose: `spec/cli/`, not `spec/cli/index.md`. */
export function specLabel(file: string): string {
  return `spec/${file.replace(/index\.md$/, '')}`;
}

export function specUrl(file: string): string {
  // The crate lives in `schematron/` within the monorepo, so the path needs
  // that segment — without it every spec link 404s.
  return `${REPO}/blob/main/${CRATE}/spec/${file}`;
}
