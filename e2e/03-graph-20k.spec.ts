/**
 * T4 spec 03 — 20k-commit fixture (contract §5.03) @smoke @slow.
 * No frame-budget assertion (headless timing is unreliable) — perf stays an
 * AI-gate/manual check. Console cleanliness is enforced by the shared fixture.
 */
import { test, expect } from './fixtures';
import {
  DEFAULT_ROW_HEIGHT,
  clickGraphRow,
  graphCanvas,
  graphScroller,
  graphScrollHeight,
  openRepo,
  scrollGraphTo,
} from './helpers';

test.describe('03 graph 20k @smoke @slow', () => {
  test('boots with 20k rows (scroll extent covers the fixture)', async ({ page }) => {
    await openRepo(page, { flags: { fixture: '20k' } });
    await expect(graphCanvas(page)).toBeVisible();
    const scrollHeight = await graphScrollHeight(page);
    expect(scrollHeight).toBeGreaterThan(20_000 * DEFAULT_ROW_HEIGHT * 0.9);
  });

  test('jump-scroll + rapid wheel scrolling keeps the canvas alive, zero errors', async ({
    page,
  }) => {
    await openRepo(page, { flags: { fixture: '20k' } });
    const scrollHeight = await graphScrollHeight(page);

    // Jump to the middle, then the end.
    const middle = await scrollGraphTo(page, Math.floor(scrollHeight / 2));
    expect(middle).toBeGreaterThan(0);
    await expect(graphCanvas(page)).toBeVisible();
    const end = await scrollGraphTo(page, scrollHeight);
    expect(end).toBeGreaterThan(middle);
    await expect(graphCanvas(page)).toBeVisible();

    // Rapid wheel scrolls ×10 (5 up from the end, 5 down again).
    await graphScroller(page).hover();
    for (let i = 0; i < 5; i++) await page.mouse.wheel(0, -1200);
    for (let i = 0; i < 5; i++) await page.mouse.wheel(0, 1200);
    await expect(graphCanvas(page)).toBeVisible();
    // Zero console errors throughout — enforced by the shared fixture teardown.
  });

  test('row click after a deep scroll still selects (details panel renders)', async ({ page }) => {
    await openRepo(page, { flags: { fixture: '20k' } });
    const target = 10_000 * DEFAULT_ROW_HEIGHT;
    const applied = await scrollGraphTo(page, target);
    expect(applied).toBe(target);
    // Click a row a few rows below the top of the current viewport.
    const displayRow = Math.ceil(applied / DEFAULT_ROW_HEIGHT) + 3;
    await clickGraphRow(page, displayRow);
    await expect(page.getByTestId('commit-details')).toBeVisible();
  });
});
