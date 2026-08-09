/**
 * T4 spec 15 — error-state injection (contract §5.15): each `?…fail` flag or
 * in-band sentinel surfaces its mapped error (toast / hook dialog / inline),
 * and the app remains usable afterwards. The search `#fail` case lives in
 * spec 09; the Composer `#fail` case is dropped per contract (the Composer is
 * AI-gated behind consent, so it is not reachable in this consent-off sweep).
 */
import { test, expect } from './fixtures';
import { errorToast, graphCanvas, openBranchContextMenu, openPalette, openRepo } from './helpers';
import type { Page } from '@playwright/test';

async function openWithStatus(page: Page, flags: Record<string, string>): Promise<void> {
  await openRepo(page, { flags });
  await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
}

test.describe('15 error injection', () => {
  test('remote=authfail: fetch surfaces authFailed, app stays usable', async ({ page }) => {
    await openWithStatus(page, { remote: 'authfail' });
    await page.getByRole('button', { name: '↓ Fetch' }).click();
    await expect(errorToast(page, /authentication failed for 'origin'/)).toBeVisible();
    await expect(graphCanvas(page)).toBeVisible();
    // Follow-up benign action still works.
    await page.getByRole('button', { name: 'Stage README.md' }).click();
    await expect(page.getByRole('button', { name: 'Unstage README.md' })).toBeVisible();
  });

  test('remote=network: pull surfaces networkError', async ({ page }) => {
    await openWithStatus(page, { remote: 'network' });
    await page.getByRole('button', { name: '⇣ Pull' }).click();
    await expect(errorToast(page, /network error talking to 'origin'/)).toBeVisible();
    await expect(graphCanvas(page)).toBeVisible();
  });

  test('remote=rejected: push surfaces the non-FF rejection', async ({ page }) => {
    await openWithStatus(page, { remote: 'rejected' });
    await page.getByRole('button', { name: '↑ Push' }).click();
    await expect(errorToast(page, /push rejected: the remote contains commits/)).toBeVisible();
    await expect(graphCanvas(page)).toBeVisible();
  });

  test('hooks=fail: commit blocked with hook output; skip-hooks retry succeeds', async ({
    page,
  }) => {
    await openWithStatus(page, { hooks: 'fail' });
    await page.getByPlaceholder('Commit message').fill('e2e: hook-gated commit');
    await page.getByRole('button', { name: 'Commit', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'A git hook blocked this commit' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(/gitleaks/)).toBeVisible(); // hook output verbatim
    await dialog.getByRole('button', { name: 'Commit anyway (skip hooks)' }).click();
    // Retry with skipHooks succeeds → the commit box clears.
    await expect(page.getByPlaceholder('Commit message')).toHaveValue('');
  });

  test('#hookfail sentinel triggers the same gate; cancel keeps the message', async ({
    page,
  }) => {
    await openWithStatus(page, {});
    await page.getByPlaceholder('Commit message').fill('feat: risky #hookfail');
    await page.getByRole('button', { name: 'Commit', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'A git hook blocked this commit' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toBeHidden();
    // No commit happened; the message is preserved for editing.
    await expect(page.getByPlaceholder('Commit message')).toHaveValue('feat: risky #hookfail');
  });

  test('hooks=failpush: push blocked; "Push anyway (skip hooks)" retries', async ({ page }) => {
    await openWithStatus(page, { hooks: 'failpush' });
    await page.getByRole('button', { name: '↑ Push' }).click();
    const dialog = page.getByRole('dialog', { name: 'A git hook blocked this push' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Push anyway (skip hooks)' }).click();
    await expect(dialog).toBeHidden();
    // main is ahead 0 → the skipped-hooks push lands as "Already up to date".
    await expect(page.locator('.toast-stack').getByText('Already up to date')).toBeVisible();
  });

  test('submodule=fail: deinit errors, row unchanged, app usable', async ({ page }) => {
    await openWithStatus(page, { submodule: 'fail' });
    // Sidebar sections stream in async — wait for the submodule + worktree rows
    // so the layout is stable before right-clicking (mirrors spec 14).
    await expect(page.getByText('stash@{0}', { exact: true })).toBeVisible();
    await expect(
      page.getByTitle('/mock/.worktrees/repo/release-1.2', { exact: true }),
    ).toBeVisible();
    const menu = await openBranchContextMenu(page, 'vendor/theme');
    await menu.getByRole('menuitem', { name: 'Deinitialize…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Deinitialize submodule' });
    await dialog.getByRole('button', { name: 'Deinitialize' }).click();
    await expect(errorToast(page, /Mock: submodule operation failed/)).toBeVisible();
    // The submodule is untouched (still listed, still initialized).
    await expect(page.getByTitle('vendor/theme', { exact: true })).toBeVisible();
    await expect(graphCanvas(page)).toBeVisible();
  });

  test('historyFail: index build errors, Ask-history overlay stays usable', async ({ page }) => {
    await openWithStatus(page, { historyFail: '1' });
    const palette = await openPalette(page);
    await palette.getByRole('combobox', { name: 'Command palette' }).fill('ask history');
    await palette.getByRole('option', { name: /Ask history…/ }).click();
    const overlay = page.getByRole('search', { name: 'Ask history' });
    await expect(overlay).toBeVisible();
    await overlay.getByRole('button', { name: 'Prepare history search' }).click();
    await expect(errorToast(page, /Mock: index build failed/)).toBeVisible();
    // The overlay and app survive; the prepare affordance is still offered.
    await expect(overlay.getByRole('button', { name: 'Prepare history search' })).toBeVisible();
    await expect(graphCanvas(page)).toBeVisible();
  });
});
