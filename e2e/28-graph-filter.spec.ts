/**
 * Spec-003 graph declutter — UI-contract smoke (plan.md §Testing "Harness/e2e"):
 * first-parent toggle via the chip popover, solo via the sidebar branch context
 * menu, clear-all restore, stale persisted `graphRefFilter` → warning chip, and
 * AC7 selection preserved/cleared across a filter change.
 *
 * Fixture facts (fixtures/graph.ts + branches.ts, mock filter graphFilter.ts):
 *  - Full graph: 30 base rows + 3 stash offshoot nodes; HEAD = row 0
 *    ("Merge feat and exp", octopus: parents [3, 1, 2]).
 *  - First-parent ALONE does not drop rows here — every side line keeps a live
 *    ref tip (feat/exp/origin/feat), mirroring the locked `--first-parent`
 *    semantics — so the row-shrink signal for first-parent is asserted ON TOP
 *    of a solo (dev = HEAD row): the feat/exp lines then have no seed and
 *    vanish (rows 1, 2, 4).
 *  - Solo dev drops the disconnected gh-pages component (2 rows) + the 3 stash
 *    offshoots (stash tips are never seeded under a restricted seed).
 *  - Row-count changes are observed via graphScrollHeight (rows × rowHeight —
 *    the canvas itself is opaque to DOM queries); filter reloads are async, so
 *    every change is polled.
 */
import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import {
  graphScrollHeight,
  openBranchContextMenu,
  openRepo,
} from './helpers';

const FLAT = { onboardingSeen: true, listView: 'flat' };

/** The inactive icon-only fab (chip collapses to it when nothing is active). */
function filterFab(page: Page) {
  return page.getByRole('button', { name: 'Graph filters', exact: true });
}
/** The active chip body, by its accessible summary label. */
function chip(page: Page, summary: string) {
  return page.getByRole('button', { name: `Graph filters: ${summary}`, exact: true });
}
function clearButton(page: Page) {
  return page.getByRole('button', { name: 'Clear graph filters' });
}

test.describe('28 graph declutter filters @smoke', () => {
  test('first-parent toggle via the chip popover shrinks a solo view further', async ({
    page,
  }) => {
    // Boot with solo:dev persisted (AC6 hydration is exercised for free) so the
    // first-parent toggle has rows to drop (see header note).
    await openRepo(page, {
      uiSettings: { ...FLAT, graphRefFilter: { mode: 'solo', refs: ['refs/heads/dev'] } },
    });
    await expect(chip(page, 'Solo: dev')).toBeVisible();
    const soloHeight = await graphScrollHeight(page);

    // Open the popover from the chip; toggle the first-parent switch.
    await chip(page, 'Solo: dev').click();
    const popover = page.getByRole('dialog', { name: 'Graph filters' });
    await expect(popover).toBeVisible();
    await popover.getByLabel('First-parent only').click();

    // Chip label gains the first-parent segment; the feat/exp side lines
    // (3 rows) drop out of the layout.
    await expect(chip(page, 'First-parent · Solo: dev')).toBeVisible();
    await expect.poll(() => graphScrollHeight(page)).toBeLessThan(soloHeight);

    // Esc closes the popover and restores focus to the chip (§2.1).
    await page.keyboard.press('Escape');
    await expect(popover).toBeHidden();
  });

  test('solo via the sidebar branch context menu, then clear-all restores', async ({
    page,
  }) => {
    await openRepo(page, { uiSettings: FLAT });
    const fullHeight = await graphScrollHeight(page);
    await expect(filterFab(page)).toBeVisible();

    // Solo `feat` from its sidebar row menu (§3.1).
    const menu = await openBranchContextMenu(page, 'feat');
    await menu.getByRole('menuitem', { name: 'Solo branch' }).click();

    // AC5 indicator + row shrink (gh-pages component + stash offshoots drop).
    await expect(chip(page, 'Solo: feat')).toBeVisible();
    await expect.poll(() => graphScrollHeight(page)).toBeLessThan(fullHeight);
    // §3.3 sidebar marker (sr-only text on the solo'd row).
    await expect(page.getByText(", solo'd in graph")).toBeAttached();

    // Clear-all (chip ✕) → full graph back, chip collapses to the quiet fab.
    await clearButton(page).click();
    await expect.poll(() => graphScrollHeight(page)).toBe(fullHeight);
    await expect(filterFab(page)).toBeVisible();
    await expect(page.getByText(", solo'd in graph")).not.toBeAttached();
  });

  test('stale persisted graphRefFilter shows the warning chip and full graph', async ({
    page,
  }) => {
    // A saved solo of a branch this repo doesn't have (global-persistence
    // leak case): backend reports seedRefsApplied:false → §2.2 warning chip,
    // graph falls back to the full walk.
    await openRepo(page, {
      uiSettings: {
        ...FLAT,
        graphRefFilter: { mode: 'solo', refs: ['refs/heads/zzz-gone'] },
      },
    });
    const staleChip = chip(page, 'Filter not applied');
    await expect(staleChip).toBeVisible();

    // Popover explains + offers "Clear saved filter".
    await staleChip.click();
    const popover = page.getByRole('dialog', { name: 'Graph filters' });
    await expect(
      popover.getByText(/saved branch filter doesn't match any branches/),
    ).toBeVisible();
    await popover.getByRole('button', { name: 'Clear saved filter' }).click();
    await expect(filterFab(page)).toBeVisible();
  });

  test('AC7: selection survives a filter it remains in, clears when filtered out', async ({
    page,
  }) => {
    await openRepo(page, { uiSettings: FLAT });

    // Select HEAD's commit via sidebar reveal (deterministic vs row math).
    await page
      .locator('li')
      .filter({ has: page.getByTitle('main', { exact: true }) })
      .first()
      .click();
    const details = page.getByTestId('commit-details');
    await expect(details.getByText('Merge feat and exp').first()).toBeVisible();

    // Solo dev (= HEAD row): the selected commit survives → still selected.
    let menu = await openBranchContextMenu(page, 'dev');
    await menu.getByRole('menuitem', { name: 'Solo branch' }).click();
    await expect(chip(page, 'Solo: dev')).toBeVisible();
    await expect(details.getByText('Merge feat and exp').first()).toBeVisible();

    // Clear, select the disconnected gh-pages tip, then solo dev again: the
    // gh-pages component is filtered out → selection clears.
    await clearButton(page).click();
    await expect(filterFab(page)).toBeVisible();
    await page
      .locator('li')
      .filter({ has: page.getByTitle('gh-pages', { exact: true }) })
      .first()
      .click();
    await expect(details.getByText('pages: update').first()).toBeVisible();

    menu = await openBranchContextMenu(page, 'dev');
    await menu.getByRole('menuitem', { name: 'Solo branch' }).click();
    await expect(chip(page, 'Solo: dev')).toBeVisible();
    await expect(details.getByText('pages: update')).not.toBeVisible();
  });
});
