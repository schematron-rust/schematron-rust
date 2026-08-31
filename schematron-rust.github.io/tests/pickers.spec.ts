import { test, expect } from '@playwright/test';

// The three header pickers (ThemePicker, TextSizePicker, SharePicker) each
// come from a separate npm package; see AGENTS.md's "Theming" section for
// how ThemePicker's stylesheet swap and app.html's anti-FOUC bootstrap stay
// in step.

test.describe('ThemePicker', () => {
  test('defaults to light and offers Light/Dark', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await page.click('.theme-picker-button');
    const options = page.locator('.theme-picker-option');
    await expect(options).toHaveCount(2);
    await expect(options).toHaveText(['Light', 'Dark']);
  });

  test('switching to dark swaps the stylesheet, sets data-theme, and persists', async ({
    page
  }) => {
    await page.goto('/');
    await page.click('.theme-picker-button');
    await page.click('.theme-picker-option:has-text("Dark")');

    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    const link = page.locator('link[data-lily-theme-picker="theme"]');
    await expect(link).toHaveCount(1); // the app.html bootstrap link is reused, not duplicated
    await expect(link).toHaveAttribute('href', /themes\/dark\.css$/);

    // The swapped stylesheet actually takes effect, not just the attribute.
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
  test('offers Email and Copy link, and copying announces success', async ({
    page,
    context
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto('/');

    await page.click('.share-picker-button');
    const email = page.locator('.share-picker-target');
    await expect(email).toHaveText('Email');
    await expect(email).toHaveAttribute(
      'href',
      /^mailto:\?subject=schematron%20%E2%80%94%20Schematron%20in%20pure%20Rust&body=/
    );

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
    await expect(page.locator('.share-picker-target')).toHaveAttribute(
      'href',
      /^mailto:\?subject=Reports/
    );
  });
});
