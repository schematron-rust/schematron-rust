declare global {
  namespace App {
    // interface Error {}
    // interface Locals {}
    interface PageData {
      /**
       * The page's `<title>` text, e.g. "Reports — schematron". Each
       * route's `+page.ts` sets this; `+page.svelte`'s `<svelte:head>`
       * reads it back rather than repeating the string, and
       * `+layout.svelte` reads `page.data.title` to pass to SharePicker —
       * one value, not three copies. See AGENTS.md's "Working rules".
       */
      title: string;
    }
    // interface PageState {}
    // interface Platform {}
  }
}

export {};
