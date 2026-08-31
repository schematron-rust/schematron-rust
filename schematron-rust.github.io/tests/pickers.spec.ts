import { test, expect } from '@playwright/test';

// The three header pickers (ThemePicker, TextSizePicker, SharePicker) each
// come from a separate npm package; see AGENTS.md's "Theming" section for
// how ThemePicker's attribute-based, multi-stylesheet setup and app.html's
// anti-FOUC bootstrap stay in step.

test.describe('ThemePicker', () => {
  test('defaults to light and offers Light/Dark', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await page.click('.theme-picker-button');
    const options = page.locator('.theme-picker-option');
    await expect(options).toHaveCount(2);
    await expect(options).toHaveText(['Light', 'Dark']);
  });

  test('both theme stylesheets are always linked, not swapped', async ({ page }) => {
    await page.goto('/');
    const hrefs = await page
      .locator('link[rel="stylesheet"]')
      .evaluateAll((els) => els.map((e) => e.getAttribute('href')));
    expect(hrefs.some((h) => h?.endsWith('/themes/light.css'))).toBe(true);
    expect(hrefs.some((h) => h?.endsWith('/themes/dark.css'))).toBe(true);
  });

  test('switching to dark applies instantly (no network wait) and persists', async ({ page }) => {
    await page.goto('/');
    await page.click('.theme-picker-button');
    await page.click('.theme-picker-option:has-text("Dark")');

    // Attribute-based: no waitForFunction on the stylesheet — reading
    // immediately after the click proves it isn't waiting on a fetch.
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    expect(
      await page.evaluate(() =>
        getComputedStyle(document.documentElement).getPropertyValue('--page-bg').trim()
      )
    ).toBe('#0c0a09');
    await expect(page.locator('body')).toHaveCSS('background-color', 'rgb(12, 10, 9)');

    expect(await page.evaluate(() => localStorage.getItem('schematron-theme'))).toBe('dark');

    // A reload re-applies the persisted theme before first paint — no flash.
    await page.reload();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  });
});

test.describe('TextSizePicker', () => {
  test('offers four sizes and rescales the root font size', async ({ page }) => {
    await page.goto('/');
    const before = await page.evaluate(() => getComputedStyle(document.documentElement).fontSize);

    await page.click('.text-size-picker-button');
    const options = page.locator('.text-size-picker-option');
    await expect(options).toHaveCount(4);
    await expect(options).toHaveText(['Small', 'Medium', 'Large', 'X Large']);

    await page.click('.text-size-picker-option:has-text("X Large")');
    await expect(page.locator('html')).toHaveAttribute('data-text-size', 'x-large');
    const after = await page.evaluate(() => getComputedStyle(document.documentElement).fontSize);
    expect(parseFloat(after)).toBeGreaterThan(parseFloat(before));
  });
});

test.describe('SharePicker', () => {
  test('offers Email, LinkedIn, Mastodon, Bluesky, Reddit, and Copy link', async ({ page }) => {
    await page.goto('/');
    await page.click('.share-picker-button');

    const targets = page.locator('.share-picker-target');
    await expect(targets).toHaveCount(5);
    await expect(targets).toHaveText(['Email', 'LinkedIn', 'Mastodon', 'Bluesky', 'Reddit']);
    await expect(page.locator('.share-picker-copy')).toHaveText('Copy link');
  });

  test('every network builder targets the real endpoint with an encoded URL', async ({
    page
  }) => {
    // subject/body/text for the home page's title, "schematron —
    // Schematron in pure Rust", URL-encoded (%20%E2%80%94%20 is " — ").
    await page.goto('/');
    await page.click('.share-picker-button');

    const hrefOf = (label: string) =>
      page.locator('.share-picker-target', { hasText: label }).getAttribute('href');

    expect(await hrefOf('Email')).toMatch(
      /^mailto:\?subject=schematron%20%E2%80%94%20Schematron%20in%20pure%20Rust&body=http/
    );
    // LinkedIn's share-offsite endpoint ignores a title/summary parameter
    // (it reads Open Graph tags instead), so only `url` is meaningful here.
    expect(await hrefOf('LinkedIn')).toBe(
      'https://www.linkedin.com/sharing/share-offsite/?url=' +
        encodeURIComponent(await page.evaluate(() => location.href))
    );
    // No single Mastodon instance to target — mastodonshare.com asks the
    // visitor for theirs and remembers it; see the SHARE_TARGETS comment.
    expect(await hrefOf('Mastodon')).toMatch(/^https:\/\/mastodonshare\.com\/\?text=schematron/);
    expect(await hrefOf('Mastodon')).toContain(
      'url=' + encodeURIComponent(await page.evaluate(() => location.href))
    );
    // Bluesky's compose intent has one `text` field, not separate url/title.
    expect(await hrefOf('Bluesky')).toMatch(/^https:\/\/bsky\.app\/intent\/compose\?text=/);
    expect(await hrefOf('Reddit')).toMatch(/^https:\/\/www\.reddit\.com\/submit\?url=.*&title=/);
  });

  test('copying announces success and puts the URL on the clipboard', async ({
    page,
    context
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto('/');

    await page.click('.share-picker-button');
    await page.click('.share-picker-copy');
    await expect(page.locator('.share-picker-status')).toHaveText('Link copied');
    const clipboard = await page.evaluate(() => navigator.clipboard.readText());
    expect(clipboard).toBe(await page.evaluate(() => location.href));
  });

  test("uses the visited page's own title, from page.data.title — not a fixed site name", async ({
    page
  }) => {
    // See src/app.d.ts's App.PageData and each route's +page.ts: this is
    // what makes the mailto: subject (and the native share sheet) name the
    // actual page being shared, not always "schematron".
    await page.goto('/reports/');
    await expect(page).toHaveTitle('Reports — schematron');
    await page.click('.share-picker-button');
    await expect(
      page.locator('.share-picker-target', { hasText: 'Email' })
    ).toHaveAttribute('href', /^mailto:\?subject=Reports/);
  });
});
