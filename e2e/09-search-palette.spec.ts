/**
 * T4 spec 09 — commit search bar, Mod-K command palette, sidebar list
 * filtering (contract §5.09) @smoke. Search live-runs on cheap fields and
 * auto-reveals the first match; the palette dispatches pre-bound handlers
 * (options run on click / Enter on the highlighted row).
 */
import { test, expect } from './fixtures';
import { expectedShortcutLabel, graphScrollHeight, openPalette, openRepo } from './helpers';
import type { Page } from '@playwright/test';

/** Two consecutive identical scroll extents = the P65 graph stream has finished. */
async function settleGraph(page: Page): Promise<void> {
  let previous = -1;
  await expect
    .poll(
      async () => {
        const current = await graphScrollHeight(page);
        const stable = current === previous;
        previous = current;
        return stable;
      },
      { intervals: [200, 200, 200, 200, 200, 200] },
    )
    .toBe(true);
}

test.describe('09 search & palette @smoke', () => {
  test('search finds a fixture commit and reveals it; results list renders', async ({ page }) => {
    await openRepo(page);
    await page.keyboard.press('ControlOrMeta+f');
    // NOTE: role=textbox — the closed-state floating affordance (FAB button)
    // shares the "Search commits" accessible name.
    const input = page.getByRole('textbox', { name: 'Search commits' });
    await expect(input).toBeVisible();
    await input.fill('Merge feat');
    // Live search (debounced) → 1 match, auto-revealed → details panel updates.
    await expect(page.getByText('1/1')).toBeVisible();
    await expect(
      page.getByTestId('commit-details').getByText('Merge feat and exp').first(),
    ).toBeVisible();
    // Toggle the results list and pick the match explicitly.
    await page.getByRole('button', { name: '☰' }).click();
    const list = page.getByRole('listbox', { name: 'Search results' });
    await expect(list).toBeVisible();
    await list.getByRole('option').first().click();
    await expect(
      page.getByTestId('commit-details').getByText('Merge feat and exp').first(),
    ).toBeVisible();
  });

  test('palette: arrow keys move the highlight, Enter dispatches, Esc closes', async ({
    page,
  }) => {
    await openRepo(page);
    // PRE-EXISTING FLAKE, root cause: `CommandPalette` re-lands the highlight on the
    // first enabled row whenever the `actions` ARRAY IDENTITY changes, and
    // `RepoWorkspace.paletteActions` depends on `graph` — whose identity P65 bumps
    // once per streamed batch. Under parallel load a batch can land between the
    // ArrowDown and the assertion, silently resetting the highlight. Let the stream
    // settle first (two identical extents) so the test measures the keyboard, not
    // the streamer.
    await settleGraph(page);
    const dialog = await openPalette(page);
    // Empty query → full registry; the highlight starts on the first enabled row.
    const selected = dialog.getByRole('option', { selected: true });
    // The row hint is an INLINE one-string label ('Ctrl+Shift+F' / '⌘⇧F') —
    // derived from the app's own renderer so it holds on every platform.
    await expect(
      dialog
        .getByRole('option')
        .filter({ hasText: 'Fetch' })
        .first()
        .locator('.command-palette-option-hint'),
    ).toHaveText(expectedShortcutLabel('Mod+Shift+F'));
    const first = await selected.textContent();
    await page.keyboard.press('ArrowDown');
    await expect(selected).not.toHaveText(first ?? '');
    await page.keyboard.press('ArrowUp');
    await expect(selected).toHaveText(first ?? '');
    // Enter runs the highlighted row — first enabled = Fetch (repo.fetch).
    await page.keyboard.press('Enter');
    await expect(dialog).toBeHidden();
    await expect(page.locator('.toast-stack').getByText(/Fetched 1 remote/)).toBeVisible();
    // Esc closes a re-opened palette.
    const again = await openPalette(page);
    await page.keyboard.press('Escape');
    await expect(again).toBeHidden();
  });

  test('palette tag jump reveals the tagged commit', async ({ page }) => {
    await openRepo(page);
    const dialog = await openPalette(page);
    await dialog.getByRole('combobox', { name: 'Command palette' }).fill('v1.0');
    // v1.0 sits on graph row 0 → the jump selects/reveals it.
    await dialog.getByRole('option', { name: /^v1\.0 / }).click();
    await expect(dialog).toBeHidden();
    await expect(
      page.getByTestId('commit-details').getByText('Merge feat and exp').first(),
    ).toBeVisible();
  });

  test('palette command dispatch: toggle theme flips data-theme', async ({ page }) => {
    await openRepo(page);
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    const dialog = await openPalette(page);
    await dialog.getByRole('combobox', { name: 'Command palette' }).fill('toggle theme');
    await dialog.getByRole('option', { name: /Toggle theme/ }).click();
    await expect(dialog).toBeHidden();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  });

  test('sidebar tag filter narrows the list and clears back', async ({ page }) => {
    await openRepo(page);
    // The Tags section is collapsed by default — expand it first; with 7 seeded
    // tags (≥ threshold) the inline filter box renders.
    await page.getByRole('treeitem', { name: 'Tags' }).click();
    const filter = page.getByLabel('Filter tags');
    await expect(filter).toBeVisible();
    await filter.fill('v1');
    await expect(page.getByTitle('v1.0', { exact: true })).toBeVisible();
    await expect(page.getByTitle('v1.1.0', { exact: true })).toBeVisible();
    await expect(page.getByTitle('v0.2.0', { exact: true })).toHaveCount(0);
    await page.getByRole('button', { name: 'Clear filter' }).click();
    await expect(page.getByTitle('v0.2.0', { exact: true })).toBeVisible();
  });

  test('search error path: #fail surfaces a toast, app stays usable', async ({ page }) => {
    await openRepo(page);
    await page.keyboard.press('ControlOrMeta+f');
    const input = page.getByRole('textbox', { name: 'Search commits' });
    await input.fill('boom #fail');
    await expect(page.locator('.toast-stack').getByText('Mock: search failed')).toBeVisible();
    // App remains usable: a follow-up benign search succeeds.
    await input.fill('Merge feat');
    await expect(page.getByText('1/1')).toBeVisible();
  });
});
