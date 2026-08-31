/**
 * P84 spec 20 — sidebar single-click reveals the ref/stash in the commit graph.
 *
 * Single-click = reveal (scroll + flash + select + aria-live announce);
 * double-click = checkout (covered elsewhere). Assertions lean on the two
 * DOM-observable signals: the `role=status` RevealAnnouncer live region
 * (RevealAnnouncer.tsx) and the resulting `selectedIndex` → commit-details panel.
 * The canvas flash itself is opaque to DOM queries, so it is not asserted here.
 *
 * Fixture facts (fixtures/graph.ts + branches.ts):
 *  - `main`, `v1.0` (tag) both label graph row oid(0) = 40 zeros ("Merge feat and exp").
 *  - stash@{0} is its own offshoot node ("WIP on main: polish sidebar").
 *  - `feature/merged-a` is a sidebar branch NOT present in the loaded graph refs
 *    → the graceful miss path.
 */
import { test, expect } from './fixtures';
import { openRepo } from './helpers';
import type { Locator, Page } from '@playwright/test';

const FLAT = { uiSettings: { onboardingSeen: true, listView: 'flat' } };

/** openRepo + wait for status so the layout (incl. stash offshoots) is stable. */
async function openWithStatus(page: Page): Promise<void> {
  await openRepo(page, FLAT);
  await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
}

/**
 * The always-mounted P84 reveal live region (RevealAnnouncer.tsx). The app mounts
 * several `role="status"` sr-only regions (sidebar reveal, graph-grid selection,
 * git-activity dock), so target this one by its accessible name ("Reveal status")
 * rather than a bare role/class selector that would resolve to multiple elements.
 */
function announcer(page: Page): Locator {
  return page.getByRole('status', { name: /reveal/i });
}

/** A sidebar tree row containing the exactly-titled name span. */
function treeRow(page: Page, name: string): Locator {
  return page
    .getByRole('treeitem')
    .filter({ has: page.getByTitle(name, { exact: true }) })
    .first();
}

test.describe('20 sidebar reveal-in-graph @smoke', () => {
  test('single-click a branch row reveals + selects its commit', async ({ page }) => {
    await openWithStatus(page);
    await treeRow(page, 'main').click();

    await expect(announcer(page)).toHaveText(/^Revealed main at commit \w{7}/);
    // Reveal sets selectedIndex → the right panel shows that commit.
    await expect(
      page.getByTestId('commit-details').getByText('Merge feat and exp').first(),
    ).toBeVisible();
  });

  test('single-click a tag row reveals its commit', async ({ page }) => {
    await openWithStatus(page);
    // Tags section starts collapsed (TagsSection.tsx) — expand it first.
    await page.getByRole('treeitem', { name: 'Tags' }).click();

    await treeRow(page, 'v1.0').click();

    await expect(announcer(page)).toHaveText(/^Revealed v1\.0 at commit \w{7}/);
    await expect(
      page.getByTestId('commit-details').getByText('Merge feat and exp').first(),
    ).toBeVisible();
  });

  test('single-click a stash row reveals its commit by oid', async ({ page }) => {
    await openWithStatus(page);
    const stashRow = page
      .getByRole('treeitem')
      .filter({ has: page.getByText('stash@{0}', { exact: true }) })
      .first();
    await stashRow.click();

    await expect(announcer(page)).toHaveText(/^Revealed stash@\{0\} at commit \w{7}/);
    await expect(
      page.getByTestId('commit-details').getByText('WIP on main: polish sidebar').first(),
    ).toBeVisible();
  });

  test('reveal of a ref not in the loaded graph → miss announce + info toast', async ({ page }) => {
    await openWithStatus(page);
    // `feature/merged-a` exists as a sidebar branch but is not labelled on any
    // loaded graph node → graceful degrade: no throw, announce + toast.
    await treeRow(page, 'feature/merged-a').click();

    await expect(announcer(page)).toHaveText(/^feature\/merged-a is not in the loaded history/);
    await expect(
      page.locator('.toast-stack').getByText(/isn't in the loaded history yet/),
    ).toBeVisible();
  });
});
