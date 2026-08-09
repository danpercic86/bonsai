/**
 * T4 spec 14 — destructive-confirm sweep (contract §5.14) @destructive: every
 * mock-reachable danger ConfirmDialog needs an explicit confirm; cancel is a
 * no-op; a stray Enter must never auto-confirm (ConfirmDialog focuses Cancel).
 * Covered elsewhere (referenced, not duplicated): branch delete (05), merge/
 * rebase abort (06/07), discard single file (04), stash drop (08).
 */
import { test, expect } from './fixtures';
import { confirm, openBranchContextMenu, openRepo, rightClickGraphRow } from './helpers';
import type { Locator, Page } from '@playwright/test';

const FLAT = { uiSettings: { onboardingSeen: true, listView: 'flat' } };

function row(page: Page, title: string): Locator {
  return page
    .locator('li')
    .filter({ has: page.getByTitle(title, { exact: true }) })
    .first();
}

/** The sidebar sections stream in asynchronously (stashes / submodules /
 *  worktrees each shift the layout as they land) — wait for the bottom-most
 *  rows before right-clicking, or the pointer can miss the moving row. */
async function sidebarSettled(page: Page): Promise<void> {
  await expect(page.getByText('stash@{0}', { exact: true })).toBeVisible();
  await expect(page.getByTitle('vendor/theme', { exact: true })).toBeVisible();
  await expect(
    page.getByTitle('/mock/.worktrees/repo/release-1.2', { exact: true }),
  ).toBeVisible();
}

test.describe('14 destructive confirms @destructive', () => {
  test('reset branch: cancel is a no-op, Enter does not confirm, confirm resets', async ({
    page,
  }) => {
    await openRepo(page);
    // Cancel path.
    let menu = await rightClickGraphRow(page, 9);
    await menu.getByRole('menuitem', { name: 'Reset main to here' }).click();
    const dialog = page.getByRole('dialog', { name: 'Reset branch' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toBeHidden();
    // Enter path: initial focus is Cancel → Enter dismisses, never resets.
    menu = await rightClickGraphRow(page, 9);
    await menu.getByRole('menuitem', { name: 'Reset main to here' }).click();
    await expect(dialog).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(dialog).toBeHidden();
    await expect(page.locator('.toast-stack').getByText(/Reset main to/)).toHaveCount(0);
    // Confirm path.
    menu = await rightClickGraphRow(page, 9);
    await menu.getByRole('menuitem', { name: 'Reset main to here' }).click();
    await confirm(page, 'Reset branch', 'Reset (mixed)');
    await expect(
      page.locator('.toast-stack').getByText(/Reset main to [0-9a-f]{7} \(mixed\)/),
    ).toBeVisible();
  });

  test('force-push with lease: cancel keeps state, confirm pushes', async ({ page }) => {
    await openRepo(page);
    await page.getByRole('button', { name: 'More push actions' }).click();
    await page.getByRole('menuitem', { name: 'Force-push with lease…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Force-push with lease?' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toBeHidden();
    await expect(page.locator('.toast-stack').getByRole('alert')).toHaveCount(0);
    await page.getByRole('button', { name: 'More push actions' }).click();
    await page.getByRole('menuitem', { name: 'Force-push with lease…' }).click();
    await confirm(page, 'Force-push with lease?', 'Force-push');
    await expect(
      page.locator('.toast-stack').getByText('Force-pushed main → origin/main'),
    ).toBeVisible();
  });

  test('stale-branches bulk delete goes through a nested confirm', async ({ page }) => {
    await openRepo(page, FLAT);
    // Scope sidebar asserts: the stale dialog lists the same names.
    const sidebar = page.getByRole('complementary');
    await expect(sidebar.getByTitle('feature/merged-a', { exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Clean up branches…' }).click();
    const stale = page.getByRole('dialog', { name: 'Clean up branches' });
    await expect(stale).toBeVisible();
    // Merged rows are pre-checked (feature/merged-a + feature/merged-b).
    const deleteSelected = stale.getByRole('button', { name: 'Delete selected (2)' });
    await deleteSelected.click();
    const nested = page.getByRole('dialog', { name: 'Delete branches' });
    await expect(nested).toBeVisible();
    await expect(nested.getByText('feature/merged-a')).toBeVisible();
    // Cancel writes nothing.
    await nested.getByRole('button', { name: 'Cancel' }).click();
    await expect(nested).toBeHidden();
    await expect(sidebar.getByTitle('feature/merged-a', { exact: true })).toBeVisible();
    // Confirm deletes both.
    await deleteSelected.click();
    await nested.getByRole('button', { name: 'Delete 2' }).click();
    // feature/gone (gone-upstream, unchecked) stays in the report, so the list
    // never empties — the outcome line + toast are the completion signal.
    await expect(page.getByText('Deleted 2 branches')).toBeVisible();
    await expect(stale.getByRole('button', { name: 'Delete selected (0)' })).toBeDisabled();
    await stale.getByRole('button', { name: 'Close', exact: true }).last().click();
    await expect(sidebar.getByTitle('feature/merged-a', { exact: true })).toHaveCount(0);
    await expect(sidebar.getByTitle('feature/merged-b', { exact: true })).toHaveCount(0);
  });

  test('discard all: cancel keeps rows, Enter never confirms, confirm clears them', async ({
    page,
  }) => {
    await openRepo(page);
    const discardAll = page.getByRole('button', { name: 'Discard all', exact: true });
    await expect(discardAll).toBeVisible();
    const dialog = page.getByRole('dialog', { name: 'Discard all changes' });
    // Cancel path.
    await discardAll.click();
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByRole('button', { name: 'Stage README.md' })).toBeVisible();
    // Enter path — must not discard.
    await discardAll.click();
    await expect(dialog).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(dialog).toBeHidden();
    await expect(page.getByRole('button', { name: 'Stage README.md' })).toBeVisible();
    // Confirm path: unstaged tracked rows revert, untracked rows are deleted.
    await discardAll.click();
    await confirm(page, 'Discard all changes', 'Discard all');
    await expect(page.getByRole('button', { name: 'Stage README.md' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Stage notes/todo.txt' })).toHaveCount(0);
  });

  test('tag delete requires confirm', async ({ page }) => {
    await openRepo(page, FLAT);
    await sidebarSettled(page);
    // The Tags section is collapsed by default — expand it first.
    await page.getByRole('button', { name: 'Tags' }).click();
    let menu = await openBranchContextMenu(page, 'v1.1.0');
    await menu.getByRole('menuitem', { name: 'Delete tag' }).click();
    const dialog = page.getByRole('dialog', { name: 'Delete tag' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByTitle('v1.1.0', { exact: true })).toBeVisible();
    menu = await openBranchContextMenu(page, 'v1.1.0');
    await menu.getByRole('menuitem', { name: 'Delete tag' }).click();
    await confirm(page, 'Delete tag', 'Delete tag');
    await expect(page.getByTitle('v1.1.0', { exact: true })).toHaveCount(0);
  });

  test('remote remove requires confirm', async ({ page }) => {
    await openRepo(page, FLAT);
    let menu = await openBranchContextMenu(page, 'origin');
    await menu.getByRole('menuitem', { name: 'Remove…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Remove remote' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByTitle('origin', { exact: true })).toBeVisible();
    menu = await openBranchContextMenu(page, 'origin');
    await menu.getByRole('menuitem', { name: 'Remove…' }).click();
    await confirm(page, 'Remove remote', 'Remove remote');
    await expect(page.getByTitle('origin', { exact: true })).toHaveCount(0);
  });

  test('worktree remove requires confirm', async ({ page }) => {
    await openRepo(page);
    await sidebarSettled(page);
    const wt = '/mock/.worktrees/repo/feature-login';
    let menu = await openBranchContextMenu(page, wt);
    await menu.getByRole('menuitem', { name: 'Remove…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Remove worktree' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByTitle(wt, { exact: true })).toBeVisible();
    menu = await openBranchContextMenu(page, wt);
    await menu.getByRole('menuitem', { name: 'Remove…' }).click();
    await confirm(page, 'Remove worktree', 'Remove worktree');
    await expect(page.getByTitle(wt, { exact: true })).toHaveCount(0);
  });

  test('submodule deinitialize requires confirm', async ({ page }) => {
    await openRepo(page);
    await sidebarSettled(page);
    const subRow = row(page, 'vendor/theme');
    await expect(subRow.getByText('up to date')).toBeVisible();
    let menu = await openBranchContextMenu(page, 'vendor/theme');
    await menu.getByRole('menuitem', { name: 'Deinitialize…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Deinitialize submodule' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(subRow.getByText('up to date')).toBeVisible();
    menu = await openBranchContextMenu(page, 'vendor/theme');
    await menu.getByRole('menuitem', { name: 'Deinitialize…' }).click();
    await confirm(page, 'Deinitialize submodule', 'Deinitialize');
    await expect(subRow.getByText('not initialized')).toBeVisible();
  });
});
