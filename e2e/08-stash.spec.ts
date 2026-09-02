/**
 * T4 spec 08 — stash journeys: list / save (scoped) / apply / pop / drop /
 * reserved-path recovery (contract §5.08) @destructive. The stash stack renders
 * in the sidebar "Stashes" section (rows titled `stash@{N}` + message); ops run
 * from the row's context menu; save runs from the StashSplitButton in the
 * status panel.
 */
import { test, expect } from './fixtures';
import { confirm, openBranchContextMenu, openRepo } from './helpers';
import type { Page } from '@playwright/test';

async function openWithStatus(page: Page): Promise<void> {
  await openRepo(page);
  await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
}

test.describe('08 stash @destructive', () => {
  test('seeded stack renders all three entries', async ({ page }) => {
    await openWithStatus(page);
    await expect(page.getByText('stash@{0}', { exact: true })).toBeVisible();
    await expect(page.getByTitle('WIP on main: polish sidebar', { exact: true })).toBeVisible();
    await expect(
      page.getByTitle('WIP on main: extract graph layout helpers', { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByTitle('WIP on main: aspire host scaffolding (reserved-name files)', {
        exact: true,
      }),
    ).toBeVisible();
  });

  // P80 §2b folded the stash scopes into the commit box's `⋯ Commit options`
  // overflow menu (CommitOptionsMenu.tsx); the old top-level 'Stash all' /
  // 'Stash options' buttons no longer exist.
  test('Stash: tracked AND untracked changes clear, a new stash@{0} appears', async ({ page }) => {
    await openWithStatus(page);
    await page.getByRole('button', { name: 'Commit options' }).click();
    await page.getByRole('menuitem', { name: 'Stash', exact: true }).click();
    await expect(page.locator('.toast-stack').getByText('Changes stashed')).toBeVisible();
    // New entry pushed on top; old entries re-indexed (+1) → stash@{3} exists.
    await expect(
      page.getByTitle('WIP on main: mock stashed changes', { exact: true }),
    ).toBeVisible();
    await expect(page.getByText('stash@{3}', { exact: true })).toBeVisible();
    // "Stash" is the WHOLE working directory: tracked changes AND brand-new
    // files leave the panel together (no silently-left-behind untracked rows).
    await expect(page.getByRole('button', { name: 'Stage README.md' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Stage notes/todo.txt' })).toHaveCount(0);
  });

  test('save staged-only via the split-button menu keeps unstaged rows', async ({ page }) => {
    await openWithStatus(page);
    await page.getByRole('button', { name: 'Commit options' }).click();
    await page.getByRole('menuitem', { name: 'Stash staged', exact: true }).click();
    await expect(page.locator('.toast-stack').getByText('Stashed staged changes')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Unstage src/app.rs' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Stage README.md' })).toBeVisible();
    await expect(page.getByText('stash@{3}', { exact: true })).toBeVisible();
  });

  test('apply leaves the stack unchanged', async ({ page }) => {
    await openWithStatus(page);
    const menu = await openBranchContextMenu(page, 'stash@{0}');
    await menu.getByRole('menuitem', { name: 'Apply' }).click();
    await expect(page.locator('.toast-stack').getByText('Applied stash@{0}')).toBeVisible();
    await expect(page.getByTitle('WIP on main: polish sidebar', { exact: true })).toBeVisible();
    await expect(page.getByText('stash@{2}', { exact: true })).toBeVisible();
  });

  test('pop removes the entry and re-indexes survivors', async ({ page }) => {
    await openWithStatus(page);
    const menu = await openBranchContextMenu(page, 'stash@{0}');
    await menu.getByRole('menuitem', { name: 'Pop' }).click();
    await expect(page.locator('.toast-stack').getByText('Popped stash@{0}')).toBeVisible();
    await expect(page.getByTitle('WIP on main: polish sidebar', { exact: true })).toHaveCount(0);
    // Two survivors re-indexed to 0/1 — stash@{2} no longer exists.
    await expect(page.getByText('stash@{2}', { exact: true })).toHaveCount(0);
    await expect(
      page.getByTitle('WIP on main: extract graph layout helpers', { exact: true }),
    ).toBeVisible();
    await expect(page.getByText('stash@{0}', { exact: true })).toBeVisible();
  });

  test('drop requires confirm; cancel keeps the entry', async ({ page }) => {
    await openWithStatus(page);
    // Cancel path.
    let menu = await openBranchContextMenu(page, 'stash@{1}');
    await menu.getByRole('menuitem', { name: 'Drop' }).click();
    const dialog = page.getByRole('dialog', { name: 'Drop stash' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toBeHidden();
    await expect(
      page.getByTitle('WIP on main: extract graph layout helpers', { exact: true }),
    ).toBeVisible();
    // Confirm path.
    menu = await openBranchContextMenu(page, 'stash@{1}');
    await menu.getByRole('menuitem', { name: 'Drop' }).click();
    await confirm(page, 'Drop stash', 'Drop stash');
    await expect(page.locator('.toast-stack').getByText('Dropped stash@{1}')).toBeVisible();
    await expect(
      page.getByTitle('WIP on main: extract graph layout helpers', { exact: true }),
    ).toHaveCount(0);
  });

  test('reserved-path stash: first apply blocked, skipping retry applies the rest', async ({
    page,
  }) => {
    await openWithStatus(page);
    const menu = await openBranchContextMenu(page, 'stash@{2}');
    await menu.getByRole('menuitem', { name: 'Apply' }).click();
    // First attempt → the reserved-paths recovery dialog (nothing applied yet).
    const dialog = page.getByRole('dialog', { name: "Skip files Windows can't restore?" });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText('src/Aspire.AppHost/NUL')).toBeVisible();
    await dialog.getByRole('button', { name: 'Apply the rest' }).click();
    await expect(
      page.locator('.toast-stack').getByText(/Applied stash@\{2\} — skipped 1 file\(s\)/),
    ).toBeVisible();
    // Apply never drops: the entry is still on the stack.
    await expect(page.getByText('stash@{2}', { exact: true })).toBeVisible();
  });

  // §5.08 downgrade: the conflicted-apply trigger is a stash whose MESSAGE
  // contains 'conflict' (stash.ts), but fixtures/stashes.ts seeds no such entry
  // and createStash ignores its message argument ('WIP on main: mock stashed
  // changes'), so the path is unreachable from the UI. Skip per contract §8.4.
  test.skip('conflicted apply surfaces the conflicts outcome (no seeded fixture)', () => {});
});
