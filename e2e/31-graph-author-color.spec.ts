/**
 * Spec-006 author coloring + parent highlight — UI-contract smoke
 * (plan.md §Testing "E2e smoke", spec-006-ui.md §1.3/§5):
 *  - "Graph colors" segmented row in Settings → Appearance toggles author
 *    mode, the canvas repaints without console errors (fixtures.ts fails any
 *    test on console.error automatically), and the pref survives a reload;
 *  - toggling back to Branch lanes restores the default;
 *  - hovering across graph rows (incl. mouseleave) is error-free — pixel
 *    assertions are a USER CHECKPOINT (headless canvas is opaque);
 *  - composition smoke: author mode + fold-linear (spec-004) + first-parent
 *    (spec-003) boot together without a crash and stay interactive.
 *
 * Fixture facts: display row 4 = 'Merge feat and exp' (octopus merge, 3
 * parents), display row 5 = 'feat: polish' — same map as 02-graph-interaction.
 */
import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import { clickGraphRow, graphCanvas, graphScroller, openRepo } from './helpers';

async function openSettings(page: Page) {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Settings' });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** The settings write is queued (~300 ms coalesce + mock write delay) — poll
 *  the persisted blob before any reload (spec-29/30 discipline). */
async function waitForPersistedColorMode(page: Page, mode: 'lane' | 'author'): Promise<void> {
  await page.waitForFunction(
    (expected) => {
      const raw = window.localStorage.getItem('bonsai.mockUiSettings');
      if (raw === null) return false;
      try {
        return (JSON.parse(raw) as { graphColorMode?: unknown }).graphColorMode === expected;
      } catch {
        return false;
      }
    },
    mode,
    { polling: 50, timeout: 10_000 },
  );
}

/** Sweep the pointer down the first rows of the graph, then fully off it —
 *  exercises hover-driven parent highlight + the mouseleave restore. Any
 *  console/page error fails via the shared fixture. */
async function hoverSweep(page: Page): Promise<void> {
  const box = await graphScroller(page).boundingBox();
  expect(box).not.toBeNull();
  if (box === null) return;
  for (const row of [4, 5, 6, 7]) {
    await page.mouse.move(box.x + box.width / 2, box.y + row * 32 + 16, { steps: 2 });
  }
  // Off the scroller entirely → mouseleave nulls the hover row.
  await page.mouse.move(box.x + box.width / 2, box.y - 10, { steps: 2 });
}

test.describe('31 graph author color + hover highlight @smoke', () => {
  test('Settings → Appearance "Graph colors: Author" recolors, persists across reload, and toggles back', async ({
    page,
  }) => {
    await openRepo(page);

    // Toggle to Author via the segmented row (native radios, UI §1.3).
    let dialog = await openSettings(page);
    await dialog.getByRole('tab', { name: 'Appearance' }).click();
    const authorRadio = dialog.getByRole('radio', { name: 'Author', exact: true });
    await expect(dialog.getByRole('radio', { name: 'Branch lanes', exact: true })).toBeChecked();
    await authorRadio.click();
    await expect(authorRadio).toBeChecked();
    await dialog.getByRole('button', { name: 'Close' }).click();
    await expect(dialog).toBeHidden();

    // Canvas stays alive after the recolor; hover in author mode is clean.
    await expect(graphCanvas(page)).toBeVisible();
    await hoverSweep(page);

    // Persistence: poll the blob, reload, re-open Settings — still Author.
    await waitForPersistedColorMode(page, 'author');
    await page.reload();
    await expect(graphCanvas(page)).toBeVisible();
    dialog = await openSettings(page);
    await dialog.getByRole('tab', { name: 'Appearance' }).click();
    await expect(dialog.getByRole('radio', { name: 'Author', exact: true })).toBeChecked();

    // Toggle back to Branch lanes; canvas repaints without errors.
    await dialog.getByRole('radio', { name: 'Branch lanes', exact: true }).click();
    await expect(
      dialog.getByRole('radio', { name: 'Branch lanes', exact: true }),
    ).toBeChecked();
    await dialog.getByRole('button', { name: 'Close' }).click();
    await expect(dialog).toBeHidden();
    await expect(graphCanvas(page)).toBeVisible();
    await waitForPersistedColorMode(page, 'lane');
  });

  test('hover + selection over merge/plain rows in lane mode produces no errors', async ({
    page,
  }) => {
    await openRepo(page);
    // Selection-driven highlight (keyboard path shares this code): select the
    // octopus merge (display row 4 once status settles → wait for the panel).
    await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
    await clickGraphRow(page, 4);
    await expect(
      page.getByTestId('commit-details').getByText('Merge feat and exp').first(),
    ).toBeVisible();
    await hoverSweep(page);
    await expect(graphCanvas(page)).toBeVisible();
  });

  test('composition smoke: author mode + fold-linear + first-parent boot and stay interactive', async ({
    page,
  }) => {
    await openRepo(page, {
      uiSettings: {
        onboardingSeen: true,
        graphColorMode: 'author',
        graphFoldLinear: true,
        graphFirstParent: true,
      },
    });
    await expect(graphCanvas(page)).toBeVisible();
    await hoverSweep(page);
    // Still selectable after the sweep (display row 4 = HEAD merge once the
    // status panel has landed the WIP row).
    await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
    await clickGraphRow(page, 4);
    await expect(
      page.getByTestId('commit-details').getByText('Merge feat and exp').first(),
    ).toBeVisible();
  });
});
