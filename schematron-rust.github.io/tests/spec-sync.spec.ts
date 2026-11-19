import { test, expect } from '@playwright/test';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { SPEC_DOCS, VERSION } from '../src/lib/site';

// `site.ts` copies the crate's specification table of contents, and nothing
// checked that the copy was still true. The crate enforces its own index with
// `every_spec_document_is_linked_from_the_index`, but it knows nothing about
// this site, so drift here is invisible: the page just lists the wrong set.
const CRATE_SPEC = fileURLToPath(new URL('../../schematron/spec', import.meta.url));
const CRATE_MANIFEST = fileURLToPath(new URL('../../schematron/Cargo.toml', import.meta.url));

test.describe('SPEC_DOCS matches the crate', () => {
  // The site is published as a standalone repo split out of the monorepo, and
  // the crate is not beside it there. The check is meaningful exactly where
  // the two live together — which is also where the drift gets introduced.
  test.skip(!existsSync(CRATE_SPEC), 'crate not present; running outside the monorepo');

  test('lists every spec document, and only documents that exist', () => {
    const onDisk = readdirSync(CRATE_SPEC, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => `${entry.name}/index.md`)
      .concat('index.md')
      .sort();
    const listed = SPEC_DOCS.map((doc) => doc.file).sort();
    expect(listed).toEqual(onDisk);
  });
});

test('the advertised version matches the crate manifest', () => {
  test.skip(!existsSync(CRATE_MANIFEST), 'crate not present; running outside the monorepo');
  const manifest = readFileSync(CRATE_MANIFEST, 'utf8');
  const version = manifest.match(/^version = "([^"]+)"/m)?.[1];
  expect(VERSION).toBe(version);
});
