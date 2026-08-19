/**
 * T4 spec 07 — rebase: seeded pause, abort, clean, conflict route, interactive
 * (contract §5.07) @destructive.
 */
import { test, expect } from './fixtures';
import {
  clickGraphRow,
  confirm,
  graphScrollHeight,
  openBranchContextMenu,
  openRepo,
  rightClickGraphRow,
} from './helpers';
import type { Page } from '@playwright/test';

const FLAT = { uiSettings: { onboardingSeen: true, listView: 'flat' } };

/** Class + role: P70's always-mounted git-availability live region also carries
 *  `role="status"`, so a bare role query is no longer unique. */
function banner(page: Page) {
  return page.locator('.op-banner[role="status"]');
}

async function waitStatus(page: Page): Promise<void> {
  await expect(page.getByTestId('status-panel').getByText(/(Staged|Conflicts) \(/)).toBeVisible();
}

test.describe('07 rebase @destructive', () => {
  test('seeded pause at 2/3: resolve → Continue finishes with replayed commits', async ({
    page,
  }) => {
    await openRepo(page, { flags: { op: 'rebase' } });
    await expect(banner(page).getByText('Rebasing feature/topic')).toBeVisible();
    await expect(banner(page).getByText(/step 2\/3/)).toBeVisible();
    await expect(banner(page).getByRole('button', { name: 'Continue' })).toBeDisabled();
    const before = await graphScrollHeight(page);

    await page.getByRole('button', { name: 'Take our version of src/auth.ts' }).click();
    await expect(banner(page).getByText(/all conflicts resolved/)).toBeVisible();
    await banner(page).getByRole('button', { name: 'Continue' }).click();
    await expect(page.locator('.toast-stack').getByText('Rebase complete')).toBeVisible();
    await expect(banner(page)).toHaveCount(0);
    // 3 replayed rows appear atop the graph.
    await expect.poll(() => graphScrollHeight(page)).toBe(before + 3 * 32);
    await clickGraphRow(page, 4); // wip(1) + stashes(3) + newest replayed row
    await expect(
      page.getByTestId('commit-details').getByText('pick: replayed 3').first(),
    ).toBeVisible();
  });

  test('abort clears the paused rebase after confirm', async ({ page }) => {
    await openRepo(page, { flags: { op: 'rebase' } });
    await expect(banner(page).getByText('Rebasing feature/topic')).toBeVisible();
    const before = await graphScrollHeight(page);
    await banner(page).getByRole('button', { name: 'Abort' }).click();
    await confirm(page, 'Abort rebase?', 'Abort rebase');
    await expect(page.locator('.toast-stack').getByText('Rebase aborted')).toBeVisible();
    await expect(banner(page)).toHaveCount(0);
    // Nothing was replayed; the conflict row is gone.
    expect(await graphScrollHeight(page)).toBe(before);
    await expect(page.getByRole('button', { name: /Take our version/ })).toHaveCount(0);
  });

  test('clean rebase via branch context menu replays 3 commits', async ({ page }) => {
    await openRepo(page, FLAT);
    await waitStatus(page);
    const before = await graphScrollHeight(page);
    const menu = await openBranchContextMenu(page, 'feature/sidebar');
    await menu.getByRole('menuitem', { name: 'Rebase main onto feature/sidebar' }).click();
    await confirm(page, 'Rebase branch', 'Rebase');
    await expect(
      page.locator('.toast-stack').getByText('Rebased onto feature/sidebar (3 commit(s))'),
    ).toBeVisible();
    await expect.poll(() => graphScrollHeight(page)).toBe(before + 3 * 32);
    await clickGraphRow(page, 4);
    await expect(
      page.getByTestId('commit-details').getByText('pick: replayed 3').first(),
    ).toBeVisible();
  });

  test('plain-rebase conflict route pauses at 1/3, Skip completes it', async ({ page }) => {
    await openRepo(page, { ...FLAT, flags: { remote: 'rebaseconflict' } });
    await waitStatus(page);
    const menu = await openBranchContextMenu(page, 'feature/sidebar');
    await menu.getByRole('menuitem', { name: 'Rebase main onto feature/sidebar' }).click();
    await confirm(page, 'Rebase branch', 'Rebase');
    await expect(
      page.locator('.toast-stack').getByText(/Rebase paused at step 1\/3/),
    ).toBeVisible();
    await expect(banner(page).getByText('Rebasing main')).toBeVisible();
    await expect(banner(page).getByText(/1 conflict\(s\) remaining/)).toBeVisible();
    // Skip drops the offending commit and finishes.
    await banner(page).getByRole('button', { name: 'Skip' }).click();
    await expect(page.locator('.toast-stack').getByText('Rebase complete')).toBeVisible();
    await expect(banner(page)).toHaveCount(0);
  });

  test('interactive rebase from a graph row: plan renders, Start finishes', async ({ page }) => {
    await openRepo(page, FLAT);
    await waitStatus(page);
    const before = await graphScrollHeight(page);
    // Display row 9 is a ref-less 'core work …' fixture row (wip 1 + stashes 3
    // + fixture offset) → the commit context menu (not a branch pill menu).
    const menu = await rightClickGraphRow(page, 9);
    await menu.getByRole('menuitem', { name: 'Interactive rebase from here…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Interactive rebase' });
    await expect(dialog).toBeVisible();
    // The plan lists the 3 replayed todos (getInteractivePlan caps at 3).
    await expect(dialog.getByRole('combobox', { name: 'Action' })).toHaveCount(3);
    // An unchanged all-pick plan is a NO-OP: RebasePlanEditor disables Start
    // (canStart = !isNoOp). Reorder one row to make the plan meaningful.
    await dialog.getByRole('button', { name: 'Move down' }).first().click();
    await dialog.getByRole('button', { name: 'Start rebase' }).click();
    await expect(dialog).toBeHidden();
    await expect(
      page.locator('.toast-stack').getByText(/Rebased onto .+ \(3 commit\(s\)\)/),
    ).toBeVisible();
    // Rewritten commits land atop the graph (originals are fixture rows the
    // mock cannot remove — documented mock simplification).
    await expect.poll(() => graphScrollHeight(page)).toBe(before + 3 * 32);
  });

  test('interactive rebase conflict pause: abort restores', async ({ page }) => {
    await openRepo(page, { ...FLAT, flags: { rebase: 'conflict' } });
    await waitStatus(page);
    const before = await graphScrollHeight(page);
    const menu = await rightClickGraphRow(page, 9);
    await menu.getByRole('menuitem', { name: 'Interactive rebase from here…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Interactive rebase' });
    await expect(dialog).toBeVisible();
    // Same no-op guard as above: edit the plan so Start enables.
    await dialog.getByRole('button', { name: 'Move down' }).first().click();
    await dialog.getByRole('button', { name: 'Start rebase' }).click();
    await expect(dialog).toBeHidden();
    await expect(page.locator('.toast-stack').getByText(/Rebase paused at step 1\//)).toBeVisible();
    await expect(banner(page).getByText('Rebasing main')).toBeVisible();
    await banner(page).getByRole('button', { name: 'Abort' }).click();
    await confirm(page, 'Abort rebase?', 'Abort rebase');
    await expect(banner(page)).toHaveCount(0);
    expect(await graphScrollHeight(page)).toBe(before);
  });
});
