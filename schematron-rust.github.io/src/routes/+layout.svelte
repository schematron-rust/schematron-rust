<script lang="ts">
  import { page } from '$app/state';
  import { SkipLink } from 'lily-design-system-svelte-headless';
  import { ThemePicker } from 'lily-design-system-svelte-theme-picker';
  import { TextSizePicker } from 'lily-design-system-svelte-text-size-picker';
  import { SharePicker } from 'lily-design-system-svelte-share-picker';
  import { REPO } from '$lib/site';

  let { children } = $props();

  const SHARE_TARGETS = [
    {
      id: 'email',
      label: 'Email',
      href: (url: string, title: string) =>
        `mailto:?subject=${encodeURIComponent(title)}&body=${encodeURIComponent(url)}`
    }
  ];

  type NavLink = { href: string; label: string };
  const navLinks: NavLink[] = [
    { href: '/', label: 'Home' },
    { href: '/why/', label: 'Why' },
    { href: '/tutorial/', label: 'Tutorial' },
    { href: '/example/', label: 'Example' },
    { href: '/library/', label: 'Library' },
    { href: '/cli/', label: 'CLI' },
    { href: '/reports/', label: 'Reports' },
    { href: '/conformance/', label: 'Conformance' },
    { href: '/spec/', label: 'Spec' },
    { href: '/help/', label: 'Help' },
    { href: '/about/', label: 'About' }
  ];

  function isCurrent(href: string): boolean {
    return page.url.pathname === href;
  }
</script>

<SkipLink href="#main" label="Skip to main content" />

<header class="site-header">
  <div class="site-header-inner">
    <a class="site-brand" href="/" aria-label="schematron home">
      <img class="site-brand-mark" src="/assets/favicon.svg" alt="" aria-hidden="true" />
      <span>schematron</span>
    </a>
    <nav class="site-nav" aria-label="Main">
      {#each navLinks as link (link.href)}
        <a href={link.href} aria-current={isCurrent(link.href) ? 'page' : undefined}>
          {link.label}
        </a>
      {/each}
      <a href={REPO}>GitHub</a>
    </nav>
    <div class="site-controls">
      <ThemePicker
        label="Theme"
        themesUrl="/assets/themes/"
        themes={['light', 'dark']}
        storageKey="schematron-theme"
        detectFromSystem
      />
      <TextSizePicker
        label="Text size"
        sizes={['small', 'medium', 'large', 'x-large']}
        storageKey="schematron-text-size"
      />
      <SharePicker
        label="Share this page"
        title={page.data.title}
        targets={SHARE_TARGETS}
        copyLabel="Copy link"
        copiedLabel="Link copied"
        copyFailedLabel="Could not copy link"
      />
    </div>
  </div>
</header>

<main id="main" class="site-main">
  {@render children()}
</main>

<footer class="site-footer">
  <div class="site-footer-inner">
    <p>
      schematron — ISO/IEC 19757-3 in pure Rust. MIT, Apache-2.0, GPL-2.0-only,
      or GPL-3.0-only, at your option.
    </p>
    <div class="site-footer-links">
      <a href={REPO}>GitHub</a>
      <a href="https://crates.io/crates/schematron">crates.io</a>
      <a href="https://docs.rs/schematron">docs.rs</a>
      <a href="/spec/">Specification</a>
      <a href="/roadmap/">Roadmap</a>
      <a href="/about/">About</a>
    </div>
  </div>
</footer>
