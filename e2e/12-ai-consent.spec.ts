/**
 * T4 spec 12 — AI consent gating (contract §5.12). aiEligible = aiEnabled &&
 * aiConsented && CLI installed (availability probe). Declined ⇒ entry points
 * absent/disabled — asserting absence IS the contract's honest downgrade for
 * "zero AI IPC calls" (mock IPC is in-page; requests can't be counted).
 */
import { test, expect } from './fixtures';
import { clickGraphRow, openRepo, rightClickGraphRow } from './helpers';

const CONSENTED = { uiSettings: { onboardingSeen: true, aiConsented: true } };

test.describe('12 AI consent gating', () => {
  test('declined: AI entry points are absent or disabled', async ({ page }) => {
    await openRepo(page); // default seed: aiConsented=false
    // Toolbar settles (non-AI neighbors render) before the absence asserts.
    await expect(page.getByRole('button', { name: 'Reflog', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'What changed…', exact: true })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Ask…', exact: true })).toHaveCount(0);
    // Commit details: the "✨ Explain" affordance is gated off.
    await clickGraphRow(page, 4);
    await expect(page.getByTestId('commit-details')).toBeVisible();
    await expect(page.getByRole('button', { name: '✨ Explain' })).toHaveCount(0);
    // Graph-row context menu: "Explain this commit" renders but is disabled.
    const menu = await rightClickGraphRow(page, 9);
    await expect(menu.getByRole('menuitem', { name: 'Explain this commit' })).toBeDisabled();
    // No AI error toast ever appeared (nothing was invoked).
    await expect(page.locator('.toast-stack').getByRole('alert')).toHaveCount(0);
  });

  test('accepted: explain-commit renders the mock AI overlay', async ({ page }) => {
    await openRepo(page, CONSENTED);
    // Entry points light up once the availability probe resolves (installed).
    await expect(page.getByRole('button', { name: 'What changed…', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Ask…', exact: true })).toBeVisible();
    await clickGraphRow(page, 4);
    await page.getByRole('button', { name: '✨ Explain' }).click();
    // AiOutputPanel: role=region titled "Explain commit <short>", canned prose.
    const panel = page.getByRole('region', { name: /^Explain commit [0-9a-f]{7}/ });
    await expect(panel).toBeVisible();
    await expect(
      panel.getByText(/This change adds a "Summarize branch" context-menu action/),
    ).toBeVisible();
    // Esc peels the AI panel first.
    await page.keyboard.press('Escape');
    await expect(panel).toBeHidden();
  });

  test('accepted via context menu: Explain this commit is enabled and runs', async ({ page }) => {
    await openRepo(page, CONSENTED);
    await expect(page.getByRole('button', { name: 'Ask…', exact: true })).toBeVisible();
    const menu = await rightClickGraphRow(page, 9);
    const item = menu.getByRole('menuitem', { name: 'Explain this commit' });
    await expect(item).toBeEnabled();
    await item.click();
    await expect(page.getByRole('region', { name: /^Explain commit [0-9a-f]{7}/ })).toBeVisible();
  });

  test('?ai=off with consent: probe fails → entry points stay dark, Settings warns', async ({
    page,
  }) => {
    await openRepo(page, { ...CONSENTED, flags: { ai: 'off' } });
    await expect(page.getByRole('button', { name: 'Reflog', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'What changed…', exact: true })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Ask…', exact: true })).toHaveCount(0);
    // Settings surfaces the unavailable state (role=note warn line).
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Settings' });
    // P69g: the CLI availability line lives behind the "AI" rail tab now.
    await dialog.getByRole('tab', { name: 'AI' }).click();
    await expect(
      dialog.getByRole('note').filter({ hasText: 'Claude Code CLI not found on PATH' }),
    ).toBeVisible();
  });
});
