/**
 * T4 spec 03 — 20k-commit fixture (contract §5.03) @smoke @slow.
 * No frame-budget assertion (headless timing is unreliable) — perf stays an
 * AI-gate/manual check. Console cleanliness is enforced by the shared fixture.
 */
import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import {
  DEFAULT_ROW_HEIGHT,
  clickGraphRow,
  graphCanvas,
  graphContentHeight,
  graphScroller,
  graphScrollHeight,
  openRepo,
  scrollGraphTo,
} from './helpers';

/** Rows the 20k fixture streams (fixtures/graph20k.ts). */
const ROWS_20K = 20_000;
/** The extent this fixture must reach — the same 0.9 tolerance the assertions
 *  below use, so waiting on it never masks a genuinely short graph. */
const MIN_EXTENT = ROWS_20K * DEFAULT_ROW_HEIGHT * 0.9;

/** `openRepo` resolves as soon as the canvas is VISIBLE — before the stream's
 *  `meta` chunk has supplied `totalRows`, and 20k rows also take several
 *  batches (512 then 4096, 30 ms apart). Every extent read here must therefore
 *  poll first; a bare read can catch an extent of 8 px. The WIP row (the other
 *  async input, from the first status round) is irrelevant at this scale: one
 *  32 px row against ~640,000 px, far inside the 10% tolerance. */
async function settled20k(page: Page): Promise<number> {
  await expect.poll(() => graphContentHeight(page)).toBeGreaterThan(MIN_EXTENT);
  return graphScrollHeight(page);
}

/** The extent is NOT proof that the rows exist. It is `max(loadedRows, meta
 *  .total)` (GraphCanvas.tsx), and `meta` is the FIRST chunk — so at 20k the
 *  scrollbar spans the whole history while only the first 512-row batch has
 *  been applied (batches: 512 then 4096, 30 ms apart — mock graphStream.ts).
 *  Clicking a row that has not arrived selects nothing, which is how
 *  `03:66` failed with "commit-details not found" while the extent was full.
 *
 *  The toolbar Refresh button is the observable completion signal: it is
 *  `disabled={refreshing || statusLoading || graphLoading || mutating}`, and
 *  `graphLoading` clears only in `refetchGraph`'s `finally`, i.e. after `done`.
 *  Ordering makes it sound rather than a lucky sample — `settled20k` already
 *  proved `meta` landed, so the stream is in flight and the button is disabled
 *  when we start waiting. */
async function streamComplete(page: Page): Promise<void> {
  await expect(page.getByRole('button', { name: 'Refresh' })).toBeEnabled({ timeout: 30_000 });
}

test.describe('03 graph 20k @smoke @slow', () => {
  test('boots with 20k rows (scroll extent covers the fixture)', async ({ page }) => {
    await openRepo(page, { flags: { fixture: '20k' } });
    await expect(graphCanvas(page)).toBeVisible();
    const scrollHeight = await settled20k(page);
    expect(scrollHeight).toBeGreaterThan(MIN_EXTENT);
  });

  test('jump-scroll + rapid wheel scrolling keeps the canvas alive, zero errors', async ({
    page,
  }) => {
    await openRepo(page, { flags: { fixture: '20k' } });
    const scrollHeight = await settled20k(page);

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
    await settled20k(page);
    // Row 10_003 must have STREAMED, not merely be inside the extent.
    await streamComplete(page);
    const target = 10_000 * DEFAULT_ROW_HEIGHT;
    const applied = await scrollGraphTo(page, target);
    expect(applied).toBe(target);
    // Click a row a few rows below the top of the current viewport.
    const displayRow = Math.ceil(applied / DEFAULT_ROW_HEIGHT) + 3;
    await clickGraphRow(page, displayRow);
    await expect(page.getByTestId('commit-details')).toBeVisible();
  });
});
