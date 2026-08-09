/**
 * T1 smoke spec (contract §2.3): boot against the mock harness, open the
 * fixture repo, prove the graph canvas renders and the sidebar shows the
 * fixture default branch (`main` — see src/ipc/mock/repoState.ts). A fresh
 * harness (empty localStorage) lands on the EmptyState, so the spec clicks
 * "Open repository" — the mock pickFolder always returns the fixture repo.
 * Console cleanliness is enforced automatically by the shared fixture.
 */
import { test, expect } from './fixtures';

test('app boots, opens the mock repo, renders graph + sidebar', async ({ page }) => {
  await page.goto('/');

  // Fresh session → first-run onboarding dialog (P43). Skip the tour.
  const welcome = page.getByRole('dialog', { name: 'Welcome to Bonsai' });
  await expect(welcome).toBeVisible();
  await welcome.getByRole('button', { name: 'Skip' }).click();
  await expect(welcome).toBeHidden();

  // Empty state with the primary open action.
  const openButton = page.getByRole('button', { name: 'Open repository' });
  await expect(openButton).toBeVisible();
  await openButton.click();

  // Graph canvas visible once the repo is open.
  await expect(page.locator('canvas').first()).toBeVisible();

  // Sidebar populated with the fixture default branch.
  await expect(page.getByText('main', { exact: true }).first()).toBeVisible();
});
