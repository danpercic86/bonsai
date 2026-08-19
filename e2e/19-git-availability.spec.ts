/**
 * P70 spec 19 — the "Git is not available" notice bar, in a REAL browser.
 *
 * Why e2e and not only jsdom: the two defects this spec pins are both about
 * what reaches the screen. H1 ("Re-check looks like a no-op") is a rendering +
 * real-timer question that a promise-timing unit test cannot answer, and the
 * §5.7 announcement guard is only meaningful when the announced text is
 * compared against the text actually painted in the bar. jsdom covers the same
 * ground (`GitBannerRecheck.test.tsx`, `GitMissingBanner.test.tsx`); this file
 * is the one that runs it through React's real commit/paint path.
 *
 * Harness rows: UI §11.4 (`?git=missing`, `?git=badpath`, `&gitDelay=`).
 */
import { test, expect } from './fixtures';
import { gotoHarness } from './helpers';

const BANNER = '.git-banner';
const BTN = '.git-banner-btn';

test.describe('19 git availability', () => {
  test('?git=missing shows Variant A; Re-check shows a perceptible pending state', async ({
    page,
  }) => {
    await gotoHarness(page, { flags: { git: 'missing', gitDelay: '0' } });
    await expect(page.locator(BANNER)).toBeVisible();
    await expect(page.locator('.git-banner-title')).toHaveText('Git is not available');

    const btn = page.locator(BTN);
    await expect(btn).toHaveText('Re-check');
    // Sample the label across the whole window from INSIDE the page: a
    // round-tripped poll cannot resolve a 400 ms state reliably.
    const samples: string[] = await page.evaluate(async () => {
      const el = document.querySelector('.git-banner-btn') as HTMLButtonElement;
      const out: string[] = [];
      el.click();
      for (let i = 0; i < 30; i++) {
        const cur = document.querySelector('.git-banner-btn');
        out.push(cur?.textContent ?? '(gone)');
        await new Promise((r) => setTimeout(r, 20));
      }
      return out;
    });
    const checking = samples.filter((s) => s.includes('Checking'));
    // ≥10 samples at 20 ms ⇒ the state was on screen for ≳200 ms. The floor is
    // 400 ms; the margin absorbs a slow CI frame without ever passing for the
    // sub-frame flash the floor exists to prevent.
    expect(checking.length).toBeGreaterThanOrEqual(10);
    // …and it ends: the button must come back, enabled.
    await expect(btn).toHaveText('Re-check');
    await expect(btn).toBeEnabled();
    // A failed re-check reports in-banner and NEVER toasts (UI §6).
    await expect(page.locator('.git-banner-checked')).toHaveText(/^Still not found — checked/);
    await expect(page.locator('.toast')).toHaveCount(0);
    await expect(page.locator('.git-banner-announce')).toHaveText('Git is still not available.');
  });

  // UI §11.4: run the announcement check for BOTH variants — Variant B is the
  // one that regressed (it announced the Variant A diagnosis, remedy dropped).
  for (const [flag, title] of [
    ['missing', 'Git is not available'],
    ['badpath', "Git couldn't be started"],
  ] as const) {
    test(`?git=${flag}: the announcement matches the visible copy`, async ({ page }) => {
      await gotoHarness(page, { flags: { git: flag } });
      await expect(page.locator(BANNER)).toBeVisible();
      await expect(page.locator('.git-banner-title')).toHaveText(title);

      const parts = await page.evaluate(() => ({
        announced: document.querySelector('.git-banner-announce')?.textContent ?? '',
        title: document.querySelector('.git-banner-title')?.textContent ?? '',
        remedy: document.querySelector('.git-banner-remedy')?.textContent ?? '',
        tried: document.querySelector('.git-banner-path')?.textContent ?? null,
      }));
      expect(parts.title.length).toBeGreaterThan(0);
      expect(parts.remedy.length).toBeGreaterThan(0);
      expect(parts.announced.startsWith(parts.title)).toBe(true);
      expect(parts.announced.endsWith(parts.remedy)).toBe(true);
      // The resolved path is shown, never read aloud.
      if (parts.tried !== null) expect(parts.announced).not.toContain('Tried:');
    });
  }
});
