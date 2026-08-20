/**
 * P69k — settings search in a real browser (UI §3, §7.2).
 *
 * jsdom already covers matching, grouping, highlighting and the rail counts
 * (`src/components/settings/SettingsSearch.test.tsx`). Three claims it cannot
 * check live here:
 *   * TAB ORDER is pure DOM order — ✕ → search → rail → pane — while the grid
 *     puts the search bar to the RIGHT of the rail. Only a real layout engine
 *     can confirm that those two facts hold at the same time;
 *   * a result row is genuinely CLICKABLE (P69g shipped two pointer bugs jsdom
 *     could not see), and editing it applies immediately;
 *   * the zero-match block and its Clear search action are reachable.
 */
import { test, expect } from './fixtures';
import { openRepo } from './helpers';
import type { Locator, Page } from '@playwright/test';

function settingsDialog(page: Page): Locator {
  return page.getByRole('dialog', { name: 'Settings' });
}

async function openSettings(page: Page): Promise<Locator> {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const dialog = settingsDialog(page);
  await expect(dialog).toBeVisible();
  return dialog;
}

function searchBox(dialog: Locator): Locator {
  return dialog.getByRole('searchbox', { name: 'Search settings' });
}

test.describe('25 settings search @smoke', () => {
  test('opens focused in the search box, with ✕ before it and the rail after', async ({ page }) => {
    await openRepo(page);
    const dialog = await openSettings(page);
    const box = searchBox(dialog);
    await expect(box).toBeFocused();

    // Backwards: the close button is the only stop before the search field.
    await page.keyboard.press('Shift+Tab');
    await expect(dialog.getByRole('button', { name: 'Close' })).toBeFocused();

    // Forwards: ✕ → search → rail (ONE stop, roving tabindex) → pane content.
    await page.keyboard.press('Tab');
    await expect(box).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(dialog.getByRole('tab', { name: 'General' })).toBeFocused();
  });

  test('the search bar sits beside the rail, not above it, despite DOM order', async ({ page }) => {
    await openRepo(page);
    const dialog = await openSettings(page);
    const rail = dialog.getByRole('tablist', { name: 'Settings categories' });
    const railBox = await rail.boundingBox();
    const searchBar = await dialog.locator('.settings-search').boundingBox();
    expect(railBox).not.toBeNull();
    expect(searchBar).not.toBeNull();
    if (railBox === null || searchBar === null) return;
    // Right of the rail, and starting at the rail's own top edge.
    expect(searchBar.x).toBeGreaterThanOrEqual(railBox.x + railBox.width - 1);
    expect(Math.abs(searchBar.y - railBox.y)).toBeLessThan(2);
  });

  test('a cross-category result is live: editing it applies immediately', async ({ page }) => {
    await openRepo(page);
    const dialog = await openSettings(page);
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

    await searchBox(dialog).fill('theme');
    // The pane now shows Appearance's Theme row even though General is selected.
    await expect(dialog.getByRole('tab', { name: /^General/ })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await expect(dialog.locator('mark.settings-match').first()).toBeVisible();

    await dialog.getByRole('radio', { name: 'Light' }).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');

    // "Go to Appearance" leaves the search and lands on the real category.
    await dialog.getByRole('button', { name: 'Go to Appearance' }).click();
    await expect(searchBox(dialog)).toHaveValue('');
    await expect(dialog.getByRole('tab', { name: 'Appearance' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
  });

  test('zero matches offer a Clear search that restores the category pane', async ({ page }) => {
    await openRepo(page);
    const dialog = await openSettings(page);
    await searchBox(dialog).fill('zzzz');
    await expect(dialog.getByText('No settings match “zzzz”.')).toBeVisible();

    await dialog.getByRole('button', { name: 'Clear search' }).click();
    await expect(searchBox(dialog)).toHaveValue('');
    await expect(dialog.getByRole('heading', { name: 'General', level: 3 })).toBeVisible();
  });

  test('Escape clears a non-empty query before it closes the dialog', async ({ page }) => {
    await openRepo(page);
    const dialog = await openSettings(page);
    await searchBox(dialog).fill('graph');
    await page.keyboard.press('Escape');
    await expect(searchBox(dialog)).toHaveValue('');
    await expect(dialog).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
  });
});
