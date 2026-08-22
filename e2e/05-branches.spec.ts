/**
 * T4 spec 05 — branch management via sidebar + context menus
 * (contract §5.05) @destructive. All tests seed `listView: 'flat'` so slashed
 * branch names render as single rows (the default tree view collapses folders).
 */
import { test, expect } from './fixtures';
import { confirm, openBranchContextMenu, openRepo } from './helpers';
import type { Locator, Page } from '@playwright/test';

const FLAT = { uiSettings: { onboardingSeen: true, listView: 'flat' } };

/** Sidebar row containing the exactly-titled name span. */
function row(page: Page, name: string): Locator {
  return page
    .locator('li')
    .filter({ has: page.getByTitle(name, { exact: true }) })
    .first();
}

async function createBranch(page: Page, name: string): Promise<void> {
  await page.getByRole('button', { name: 'Create branch' }).click();
  const input = page.getByPlaceholder('new-branch-name');
  await input.fill(name);
  await input.press('Enter');
  await expect(page.getByTitle(name, { exact: true })).toBeVisible();
}

test.describe('05 branches @destructive', () => {
  test('create a branch; invalid and duplicate names surface errors', async ({ page }) => {
    await openRepo(page, FLAT);
    await createBranch(page, 'e2e/topic');

    // Invalid name (whitespace) → inline error, list unchanged.
    await page.getByRole('button', { name: 'Create branch' }).click();
    const input = page.getByPlaceholder('new-branch-name');
    await input.fill('bad name');
    await input.press('Enter');
    await expect(page.getByText(/invalid branch name/)).toBeVisible();

    // Duplicate → branchExists error.
    await input.fill('e2e/topic');
    await input.press('Enter');
    await expect(page.getByText(/already exists/)).toBeVisible();
    await input.press('Escape');
  });

  test('checkout via context menu: FF branch, then conflicted re-apply branch', async ({
    page,
  }) => {
    await openRepo(page, FLAT);
    // feature/merged-a is the deterministic FF fixture (ahead 0 / behind 3):
    // checkout fast-forwards silently and the behind badge clears.
    await expect(page.getByTitle('vs origin/feature/merged-a')).toHaveText('↓3');
    let menu = await openBranchContextMenu(page, 'feature/merged-a');
    await menu.getByRole('menuitem', { name: 'Checkout' }).click();
    await expect(row(page, 'feature/merged-a')).toHaveAttribute('aria-current', 'true');
    await expect(page.getByTitle('vs origin/feature/merged-a')).toHaveCount(0);

    // fix/watcher-debounce: dirty tree carried across with a conflicted
    // re-apply — surfaced as a toast + conflict row; the app stays usable.
    menu = await openBranchContextMenu(page, 'fix/watcher-debounce');
    await menu.getByRole('menuitem', { name: 'Checkout' }).click();
    await expect(
      page.locator('.toast-stack').getByText(/carried over with conflicts/),
    ).toBeVisible();
    await expect(row(page, 'fix/watcher-debounce')).toHaveAttribute('aria-current', 'true');
    await expect(page.getByTestId('status-panel').getByText(/Conflicts \(/)).toBeVisible();
  });

  test('rename a branch via context menu', async ({ page }) => {
    await openRepo(page, FLAT);
    await createBranch(page, 'e2e/topic');
    const menu = await openBranchContextMenu(page, 'e2e/topic');
    await menu.getByRole('menuitem', { name: 'Rename…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Rename branch' });
    await expect(dialog).toBeVisible();
    await dialog.getByLabel('New branch name').fill('e2e/topic2');
    await dialog.getByRole('button', { name: 'Rename' }).click();
    await expect(page.getByTitle('e2e/topic2', { exact: true })).toBeVisible();
    await expect(page.getByTitle('e2e/topic', { exact: true })).toHaveCount(0);
  });

  test('delete with confirm; unmerged branch delete is refused', async ({ page }) => {
    await openRepo(page, FLAT);
    await createBranch(page, 'e2e/doomed');
    let menu = await openBranchContextMenu(page, 'e2e/doomed');
    await menu.getByRole('menuitem', { name: 'Delete' }).click();
    await confirm(page, 'Delete branch', 'Delete branch');
    await expect(page.getByTitle('e2e/doomed', { exact: true })).toHaveCount(0);

    // experiment-unmerged → unmergedBranch error toast, branch remains.
    menu = await openBranchContextMenu(page, 'experiment-unmerged');
    await menu.getByRole('menuitem', { name: 'Delete' }).click();
    await confirm(page, 'Delete branch', 'Delete branch');
    // The delete error surfaces as an INLINE alert in the sidebar
    // (role=complementary), not a toast — RepoWorkspace routes branch-op
    // errors to the sidebar error slot.
    await expect(
      page.getByRole('complementary').getByRole('alert').filter({ hasText: /not fully merged/ }),
    ).toBeVisible();
    await expect(page.getByTitle('experiment-unmerged', { exact: true })).toBeVisible();
  });

  test('checkout a remote-only branch creates and switches to a local', async ({ page }) => {
    await openRepo(page, FLAT);
    const menu = await openBranchContextMenu(page, 'origin/release');
    await menu.getByRole('menuitem', { name: 'Checkout' }).click();
    await expect(row(page, 'release')).toHaveAttribute('aria-current', 'true');
  });
});
