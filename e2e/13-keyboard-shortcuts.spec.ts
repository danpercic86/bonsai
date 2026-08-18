/**
 * T4 spec 13 — keyboard-shortcut spot-checks + the shortcut overlay
 * (contract §5.13). Binding source of truth: useWorkspaceKeyboard.ts (one
 * binding via ctrlKey||metaKey) and App.tsx ('?' overlay toggle behind the
 * typing guard). The overlay documents the search + palette rows (campaign fix).
 * LABEL source of truth: src/utils/platform.ts — the caps read 'Ctrl+F' on
 * Windows/Linux and '⌘F' on macOS, so expectations are derived, never spelled.
 */
import { test, expect } from './fixtures';
import {
  clickGraphRow,
  expectedOverlayCaps,
  graphCanvas,
  openPalette,
  openRepo,
} from './helpers';

test.describe('13 keyboard shortcuts', () => {
  test('Mod-K toggles the palette; Esc closes it', async ({ page }) => {
    await openRepo(page);
    const dialog = await openPalette(page);
    await page.keyboard.press('ControlOrMeta+k'); // second press closes
    await expect(dialog).toBeHidden();
    await openPalette(page);
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
  });

  test('? opens the shortcut overlay listing the search + palette rows', async ({ page }) => {
    await openRepo(page);
    await page.keyboard.press('?');
    const overlay = page.getByRole('dialog', { name: 'Keyboard shortcuts' });
    await expect(overlay).toBeVisible();
    await expect(overlay.getByText('Search commits')).toBeVisible();
    await expect(overlay.getByText('Open command palette')).toBeVisible();
    await expect(overlay.getByText('Commit staged changes')).toBeVisible();
    // The caps render for THIS platform (Ctrl… vs ⌘…) — expectation derived
    // from the app's own renderer so macOS runs assert glyphs, not words.
    const caps = (action: string) =>
      overlay.locator('.shortcut-row', { hasText: action }).locator('.shortcut-keys');
    await expect(caps('Search commits')).toHaveText(expectedOverlayCaps('Mod+F'));
    await expect(caps('Open command palette')).toHaveText(expectedOverlayCaps('Mod+K'));
    await expect(caps('Commit staged changes')).toHaveText(expectedOverlayCaps('Mod+Enter'));
    await expect(caps('Fetch all remotes')).toHaveText(expectedOverlayCaps('Mod+Shift+F'));
    // '?' toggles it off again; ✕ also closes.
    await page.keyboard.press('?');
    await expect(overlay).toBeHidden();
    await page.keyboard.press('?');
    await expect(overlay).toBeVisible();
    await overlay.getByRole('button', { name: 'Close' }).click();
    await expect(overlay).toBeHidden();
  });

  test('Mod-F opens commit search; Esc from the input closes it', async ({ page }) => {
    await openRepo(page);
    await page.keyboard.press('ControlOrMeta+f');
    // role=textbox: the closed-state floating affordance (a button) shares the
    // "Search commits" accessible name.
    const input = page.getByRole('textbox', { name: 'Search commits' });
    await expect(input).toBeVisible();
    await expect(input).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(input).toHaveCount(0);
  });

  test('Mod-R manual refresh fires without error', async ({ page }) => {
    await openRepo(page);
    await page.keyboard.press('ControlOrMeta+r');
    // Refresh is silent on success — the gate is "no console error, app alive"
    // (the fixture fails the test on any console/page error).
    await expect(graphCanvas(page)).toBeVisible();
    await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
  });

  test('arrow keys move the commit selection', async ({ page }) => {
    await openRepo(page);
    await clickGraphRow(page, 4); // a real commit row (below wip + stash nodes)
    const details = page.getByTestId('commit-details');
    await expect(details).toBeVisible();
    const before = await details.locator('.commit-summary').first().textContent();
    await page.keyboard.press('ArrowDown');
    await expect(details.locator('.commit-summary').first()).not.toHaveText(before ?? '');
    await page.keyboard.press('ArrowUp');
    await expect(details.locator('.commit-summary').first()).toHaveText(before ?? '');
  });

  test('shortcuts are suppressed while typing in the commit box', async ({ page }) => {
    await openRepo(page);
    const box = page.getByPlaceholder('Commit message');
    await box.click();
    await box.pressSequentially('why?');
    // The '?' stayed in the input; no overlay appeared.
    await expect(box).toHaveValue('why?');
    await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).toHaveCount(0);
  });
});
