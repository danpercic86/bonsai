/**
 * T4 spec 10 — settings persistence across reload via bonsai.mockUiSettings
 * (contract §5.10) @smoke: theme, GraphPrefs toggles + rowHeight, list-view
 * toggle, corrupt-storage resilience. Pane-width persistence is exercised via
 * the same setUiSettings round-trip as listView; the divider drag itself is a
 * pointer-math flow left to the native checkpoint.
 */
import { test, expect } from './fixtures';
import { gotoHarness, graphCanvas, openPalette, openRepo, skipOnboarding, FIXTURE_REPO } from './helpers';
import type { Page } from '@playwright/test';

async function openSettings(page: Page) {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Settings' });
  await expect(dialog).toBeVisible();
  return dialog;
}

test.describe('10 settings persistence @smoke', () => {
  test('theme switches to light and survives a reload', async ({ page }) => {
    await openRepo(page);
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    await page.getByRole('button', { name: 'Switch to light theme' }).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    // The setUiSettings write is async (mock delay) — wait for the persisted
    // seed before reloading.
    await expect
      .poll(() =>
        page.evaluate(() => {
          const raw = window.localStorage.getItem('bonsai.mockUiSettings');
          return raw === null ? null : (JSON.parse(raw) as { theme?: string }).theme;
        }),
      )
      .toBe('light');
    await page.reload();
    await expect(graphCanvas(page)).toBeVisible();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await expect(page.getByRole('button', { name: 'Switch to dark theme' })).toBeVisible();
  });

  test('GraphPrefs toggles + row height persist across reload', async ({ page }) => {
    await openRepo(page);
    let dialog = await openSettings(page);
    await dialog.getByRole('checkbox', { name: 'Short SHA' }).uncheck();
    await dialog.getByRole('checkbox', { name: 'Compact rows' }).check();
    await dialog.getByRole('checkbox', { name: 'Author name' }).check();
    // Row height via the range slider's keyboard steps (32 → 40) — real key
    // events, so React onChange fires per step.
    const slider = dialog.getByRole('slider', { name: 'Row height' });
    await slider.focus();
    for (let i = 0; i < 8; i += 1) await page.keyboard.press('ArrowRight');
    await expect(dialog.locator('#settings-graph-row')).toHaveValue('40');
    await dialog.getByRole('button', { name: 'Close' }).click();
    await expect(dialog).toBeHidden();
    // The settings write is debounce-batched — wait for the persisted seed to
    // carry the last patch before reloading.
    await expect
      .poll(() =>
        page.evaluate(() => {
          const raw = window.localStorage.getItem('bonsai.mockUiSettings');
          if (raw === null) return null;
          const parsed = JSON.parse(raw) as { graph?: { rowHeight?: number } };
          return parsed.graph?.rowHeight ?? null;
        }),
      )
      .toBe(40);

    await page.reload();
    await expect(graphCanvas(page)).toBeVisible();
    dialog = await openSettings(page);
    await expect(dialog.getByRole('checkbox', { name: 'Short SHA' })).not.toBeChecked();
    await expect(dialog.getByRole('checkbox', { name: 'Compact rows' })).toBeChecked();
    await expect(dialog.getByRole('checkbox', { name: 'Author name' })).toBeChecked();
    await expect(dialog.locator('#settings-graph-row')).toHaveValue('40');
  });

  test('list-view toggle (tree ↔ flat) persists across reload', async ({ page }) => {
    await openRepo(page);
    const sidebar = page.getByRole('complementary');
    // Default tree view: the branches list shows a collapsed 'feature' folder
    // row (title = the dir prefix), so the full slashed name is absent.
    await expect(sidebar.getByTitle('feature', { exact: true }).first()).toBeVisible();
    await expect(sidebar.getByTitle('feature/sidebar', { exact: true })).toHaveCount(0);
    const dialog = await openPalette(page);
    await dialog.getByRole('combobox', { name: 'Command palette' }).fill('flat lists');
    await dialog.getByRole('option', { name: /Toggle tree \/ flat lists/ }).click();
    // Flat view: no folder rows; full slashed names render as single rows.
    await expect(sidebar.getByTitle('feature', { exact: true })).toHaveCount(0);
    await expect(sidebar.getByTitle('feature/sidebar', { exact: true })).toBeVisible();
    // The setUiSettings write is async (mock delay) — wait for the persisted
    // seed before reloading (same race as the theme case).
    await expect
      .poll(() =>
        page.evaluate(() => {
          const raw = window.localStorage.getItem('bonsai.mockUiSettings');
          return raw === null ? null : (JSON.parse(raw) as { listView?: string }).listView;
        }),
      )
      .toBe('flat');

    await page.reload();
    await expect(graphCanvas(page)).toBeVisible();
    await expect(sidebar.getByTitle('feature/sidebar', { exact: true })).toBeVisible();
    await expect(sidebar.getByTitle('feature', { exact: true })).toHaveCount(0);
  });

  test('corrupt bonsai.mockUiSettings boots on defaults with no errors', async ({ page }) => {
    // Registered BEFORE gotoHarness's init script, so its setIfAbsent is a no-op
    // and the app parses this garbage on boot (readUiSettings → defaults).
    await page.addInitScript(() => {
      window.localStorage.setItem('bonsai.mockUiSettings', 'garbage{');
    });
    await gotoHarness(page, { session: { openRepos: [FIXTURE_REPO] } });
    // Defaults ⇒ onboarding unseen → the Welcome overlay shows; skip it.
    await skipOnboarding(page);
    await expect(graphCanvas(page)).toBeVisible();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  });
});
