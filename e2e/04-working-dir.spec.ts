/**
 * T4 spec 04 — working-dir flow: stage / unstage / discard / commit
 * (contract §5.04) @smoke @destructive. All flows mutate the stateful mock
 * (`handlers/status.ts`); row moves are asserted via the rows' accessible
 * stage/unstage button labels (section membership drives the label).
 */
import { test, expect } from './fixtures';
import { clickGraphRow, confirm, graphScrollHeight, openRepo } from './helpers';
import type { Page } from '@playwright/test';

async function openWithStatus(page: Page, flags?: Record<string, string>): Promise<void> {
  await openRepo(page, flags ? { flags } : undefined);
  await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
}

test.describe('04 working-dir @smoke @destructive', () => {
  test('stage and unstage a tracked file', async ({ page }) => {
    await openWithStatus(page);
    await page.getByRole('button', { name: 'Stage README.md' }).click();
    await expect(page.getByRole('button', { name: 'Unstage README.md' })).toBeVisible();
    await page.getByRole('button', { name: 'Unstage README.md' }).click();
    await expect(page.getByRole('button', { name: 'Stage README.md' })).toBeVisible();
  });

  test('stage and unstage an untracked file (added ↔ untracked)', async ({ page }) => {
    await openWithStatus(page);
    await page.getByRole('button', { name: 'Stage notes/todo.txt' }).click();
    await expect(page.getByRole('button', { name: 'Unstage notes/todo.txt' })).toBeVisible();
    await page.getByRole('button', { name: 'Unstage notes/todo.txt' }).click();
    await expect(page.getByRole('button', { name: 'Stage notes/todo.txt' })).toBeVisible();
  });

  test('discard requires confirm; cancel is a no-op', async ({ page }) => {
    await openWithStatus(page);
    const discard = page.getByRole('button', { name: 'Discard changes to src/counter.ts' });

    // Cancel path first: the row stays.
    await discard.click();
    const dialog = page.getByRole('dialog', { name: 'Discard changes' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toBeHidden();
    await expect(page.getByRole('button', { name: 'Stage src/counter.ts' })).toBeVisible();

    // Confirm path: the row disappears (discardPaths).
    await discard.click();
    await confirm(page, 'Discard changes', 'Discard changes');
    await expect(page.getByRole('button', { name: 'Stage src/counter.ts' })).toHaveCount(0);
    await expect(
      page.getByRole('button', { name: 'Discard changes to src/counter.ts' }),
    ).toHaveCount(0);
  });

  test('commit is blocked while the message is empty', async ({ page }) => {
    // §5.04.4: staged entries exist by default, so the only gate left is the
    // empty message → the Commit button stays disabled (blocked, no toast).
    // The `nothingToCommit` branch is NOT reachable in the mock: src/main.rs is
    // model-derived (P17 three-way) and its full-row unstage is not modeled, so
    // the staged set can never be emptied — mock-layer limitation, not app bug.
    await openWithStatus(page);
    const commit = page.getByRole('button', { name: 'Commit', exact: true });
    await expect(commit).toBeVisible();
    await expect(commit).toBeDisabled();
    await page.getByPlaceholder('Commit message').fill('now enabled');
    await expect(commit).toBeEnabled();
  });

  test('happy commit: staged rows clear, graph gains a top row, ahead chip bumps', async ({
    page,
  }) => {
    await openWithStatus(page);
    const before = await graphScrollHeight(page);
    await page.getByPlaceholder('Commit message').fill('e2e: happy commit');
    await page.getByRole('button', { name: 'Commit', exact: true }).click();

    // CommitBox clears the message only after commit + refreshAll resolve.
    await expect(page.getByPlaceholder('Commit message')).toHaveValue('');
    // Fixture staged rows are gone (src/main.rs stays — model-derived).
    await expect(page.getByRole('button', { name: 'Unstage src/app.rs' })).toHaveCount(0);
    // Branch ahead-count chip bumps (main tracks origin/main).
    await expect(page.getByTitle('vs origin/main')).toHaveText('↑1');
    // The graph gained exactly one row; its details carry the new summary.
    await expect.poll(() => graphScrollHeight(page)).toBe(before + 32);
    await clickGraphRow(page, 4); // wip(1) + stashes(3) + new commit at layout row 3
    await expect(
      page.getByTestId('commit-details').getByText('e2e: happy commit').first(),
    ).toBeVisible();
  });

  test('identity gap: commit with no git identity surfaces configMissing', async ({ page }) => {
    await openWithStatus(page, { fixture: 'noconfig' });
    await page.getByPlaceholder('Commit message').fill('e2e: should fail');
    await page.getByRole('button', { name: 'Commit', exact: true }).click();
    await expect(page.getByRole('alert').filter({ hasText: /identity/i })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Set identity…' })).toBeVisible();
  });
});
