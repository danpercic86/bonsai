/**
 * T4 spec 01 — boot, onboarding, empty states (contract §5.01) @smoke.
 * Fresh storage cases pass `uiSettings: {}` so the P43 Welcome overlay shows;
 * repo shapes are seeded via bonsai.mockSession (repoState.ts path substrings).
 */
import { test, expect } from './fixtures';
import { gotoHarness, graphCanvas, openRepo, skipOnboarding } from './helpers';

test.describe('01 boot & onboarding @smoke', () => {
  test('fresh boot shows Welcome, Skip lands on the EmptyState', async ({ page }) => {
    await gotoHarness(page, { uiSettings: {} });
    await skipOnboarding(page);
    await expect(page.getByRole('button', { name: 'Open repository' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Clone repository…' })).toBeVisible();
  });

  test('onboarding is persisted — no Welcome after reload', async ({ page }) => {
    await gotoHarness(page, { uiSettings: {} });
    await skipOnboarding(page);
    await page.reload();
    // EmptyState renders directly; the Welcome dialog never re-appears.
    await expect(page.getByRole('button', { name: 'Open repository' })).toBeVisible();
    await expect(page.getByRole('dialog', { name: 'Welcome to Bonsai' })).toBeHidden();
  });

  test('open repo shows graph, sidebar main, and working-dir status on the right', async ({
    page,
  }) => {
    // Explicit EmptyState-click path (openRepo seeds a session instead). A
    // non-usable recents seed makes the boot launch-reopen effect finish with
    // a visible signal (warning toast) BEFORE we click — clicking earlier
    // loses the tab to the boot effect's setTabs (FINDINGS [T4.1]).
    await gotoHarness(page, { recents: ['C:/mock/not-a-repo'] });
    await expect(page.locator('.toast-stack').getByText(/Could not reopen/)).toBeVisible();
    await page.getByRole('button', { name: 'Open repository' }).click();
    await expect(graphCanvas(page)).toBeVisible();
    await expect(page.getByText('main', { exact: true }).first()).toBeVisible();
    // No commit selected → the right panel is the working-dir status panel.
    const status = page.getByTestId('status-panel');
    await expect(status).toBeVisible();
    await expect(status.getByText(/Staged \(/)).toBeVisible();
    await expect(status.getByText(/Changes \(/)).toBeVisible();
  });

  test('unborn repo: empty graph, status panel usable for the first commit', async ({ page }) => {
    await gotoHarness(page, {
      session: { openRepos: ['C:/mock/unborn-repo'], activeRepo: 'C:/mock/unborn-repo' },
    });
    await expect(page.getByText('No commits yet')).toBeVisible();
    await expect(page.getByTestId('status-panel')).toBeVisible();
    await expect(page.getByPlaceholder('Commit message')).toBeVisible();
  });

  test('non-repo open: error surfaced, app stays usable', async ({ page }) => {
    await gotoHarness(page, {
      session: { openRepos: ['C:/mock/not-a-repo'], activeRepo: 'C:/mock/not-a-repo' },
    });
    // Boot reopen fails with a warning toast; the EmptyState remains usable.
    await expect(page.locator('.toast-stack').getByText(/Could not reopen/)).toBeVisible();
    const openButton = page.getByRole('button', { name: 'Open repository' });
    await expect(openButton).toBeVisible();
    await openButton.click();
    await expect(graphCanvas(page)).toBeVisible();
  });

  test('recents seed is listed on the EmptyState', async ({ page }) => {
    // A non-usable recents path keeps the EmptyState visible (the back-compat
    // boot path auto-reopens recents[0]; a usable path would open a tab).
    await gotoHarness(page, { recents: ['C:/mock/not-a-repo'] });
    await expect(page.getByRole('button', { name: 'Open repository' })).toBeVisible();
    await expect(page.getByRole('button', { name: /not-a-repo/ })).toBeVisible();
  });
});
