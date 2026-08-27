/**
 * Spec-005 graph overview rail — UI-contract smoke (plan.md §Testing "e2e" +
 * UI contract §7): zero-idle-cost hidden default (AC3 — the rail is NOT in the
 * DOM at all), search-open reveal with match ticks + tick click jumps to and
 * selects the match (AC1), close hides it, the persisted "Always show overview
 * rail" toggle (AC5) survives reload, hover-reveal near the right edge with the
 * 300 ms linger, and thumb click/drag scrolls the graph (AC4, headless-safe
 * scrollTop assertion — painted pixels are the USER CHECKPOINT).
 *
 * Fixture facts (src/ipc/fixtures/graph.ts + withStashNodes = 33 model rows):
 *  - "pages" matches exactly 2 commits: 'pages: update' (model 31) and
 *    'pages: init' (model 32 — the LAST row). Its tick therefore sits at the
 *    very bottom rail pixel (y = railHeight - 1), deterministically clickable
 *    within the ±3 px hit slop; the two ticks are ~18 px apart at any sane
 *    viewport height, so the bottom click cannot hit the wrong one.
 *  - Search auto-reveals match 1/2 ('pages: update'); the tick click must move
 *    the counter to 2/2 and the details panel to 'pages: init'.
 */
import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import { graphScroller, openRepo } from './helpers';

const rail = (page: Page) => page.getByTestId('graph-rail');

async function openSearch(page: Page, query: string): Promise<void> {
  await page.keyboard.press('ControlOrMeta+f');
  const input = page.getByRole('textbox', { name: 'Search commits' });
  await expect(input).toBeVisible();
  await input.fill(query);
}

async function openSettings(page: Page) {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Settings' });
  await expect(dialog).toBeVisible();
  return dialog;
}

test.describe('30 graph overview rail @smoke', () => {
  test('hidden by default: no rail element in the DOM at all (AC3 zero idle cost)', async ({
    page,
  }) => {
    await openRepo(page);
    // Not display:none — NOT MOUNTED. Both the testid and the class must be
    // absent (the class assertion also guards a future testid rename).
    await expect(rail(page)).toHaveCount(0);
    await expect(page.locator('.graph-rail')).toHaveCount(0);
  });

  test('search reveals the rail; clicking a tick jumps to and selects that match (AC1); close hides it (AC3)', async ({
    page,
  }) => {
    await openRepo(page);
    await openSearch(page, 'pages');
    // 2 matches, auto-revealed 1/2 = 'pages: update'.
    await expect(page.getByText('1/2')).toBeVisible();
    const details = page.getByTestId('commit-details');
    await expect(details.getByText('pages: update').first()).toBeVisible();
    await expect(rail(page)).toBeVisible();

    // The LAST commit ('pages: init', model row 32 of 33) ticks at the bottom
    // rail pixel — click there; assert by selection state, never by pixels.
    const box = await rail(page).boundingBox();
    expect(box).not.toBeNull();
    if (box === null) return;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height - 2);
    await expect(page.getByText('2/2')).toBeVisible();
    await expect(details.getByText('pages: init').first()).toBeVisible();

    // Closing search unmounts the rail (no always-show, pointer not in zone).
    await page.getByTitle('Close (Esc)').click();
    await expect(rail(page)).toHaveCount(0);
  });

  test('"Always show overview rail" toggle mounts the rail without search and survives reload (AC5)', async ({
    page,
  }) => {
    await openRepo(page);
    await expect(rail(page)).toHaveCount(0);

    const dialog = await openSettings(page);
    await dialog.getByRole('tab', { name: 'Commit graph' }).click();
    await dialog.getByRole('checkbox', { name: 'Always show overview rail' }).check();
    await dialog.getByRole('button', { name: 'Close' }).click();
    await expect(dialog).toBeHidden();

    // Persistent rail, no search open.
    await expect(rail(page)).toBeVisible();

    // The settings write is queued (~300 ms coalesce + mock write delay) —
    // poll the persisted blob before reloading (spec-10/29 discipline).
    await page.waitForFunction(
      () => {
        const raw = window.localStorage.getItem('bonsai.mockUiSettings');
        if (raw === null) return false;
        try {
          return (
            (JSON.parse(raw) as { graphMinimapAlwaysShow?: unknown }).graphMinimapAlwaysShow ===
            true
          );
        } catch {
          return false;
        }
      },
      undefined,
      { polling: 50, timeout: 10_000 },
    );
    await page.reload();
    await expect(rail(page)).toBeVisible();
  });

  test('thumb: click low on the rail scrolls the graph there (AC4, scrollTop truth)', async ({
    page,
  }) => {
    // Seed always-show directly — this test exercises the thumb, not the
    // settings UI (covered above). No search open → no tick can intercept.
    await openRepo(page, { uiSettings: { onboardingSeen: true, graphMinimapAlwaysShow: true } });
    const r = rail(page);
    await expect(r).toBeVisible();
    const scroller = graphScroller(page);
    expect(await scroller.evaluate((el) => el.scrollTop)).toBe(0);

    const box = await r.boundingBox();
    expect(box).not.toBeNull();
    if (box === null) return;
    // Click at 90% height — outside the top-anchored thumb → centers the
    // viewport there (scrollTop jumps toward the bottom of the history).
    await page.mouse.click(box.x + box.width / 2, box.y + box.height * 0.9);
    await expect.poll(() => scroller.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);

    // Drag back to the very top clamps to 0 (pointer capture path).
    await page.mouse.move(box.x + box.width / 2, box.y + box.height * 0.9);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2, box.y + 1, { steps: 5 });
    await page.mouse.up();
    await expect.poll(() => scroller.evaluate((el) => el.scrollTop)).toBe(0);
  });

  test('hover near the right edge reveals the rail; moving away unmounts after the linger', async ({
    page,
  }) => {
    await openRepo(page);
    await expect(rail(page)).toHaveCount(0);
    const box = await graphScroller(page).boundingBox();
    expect(box).not.toBeNull();
    if (box === null) return;

    // Enter the 20 px right-edge hover zone (a real mousemove on the scroller).
    await page.mouse.move(box.x + box.width - 5, box.y + box.height / 2, { steps: 3 });
    await expect(rail(page)).toBeVisible();

    // Leave the zone (and the rail) → 300 ms linger, then full unmount.
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 3 });
    await expect(rail(page)).toHaveCount(0, { timeout: 5_000 });
  });
});
