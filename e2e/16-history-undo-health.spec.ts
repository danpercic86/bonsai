/**
 * T4 spec 16 — reflog viewer, one-click undo (all ?undo= seams), repo-health
 * panel, palette action dispatch, file history + blame (contract §5.16).
 * Undo classification/wiring: describeLastUndo → UndoDialog → resetBranch.
 */
import { test, expect } from './fixtures';
import { graphCanvas, openPalette, openRepo } from './helpers';

test.describe('16 history, undo, health', () => {
  test('reflog viewer renders the seeded HEAD story', async ({ page }) => {
    await openRepo(page);
    await page.getByRole('button', { name: '↺ Reflog' }).click();
    const view = page.getByRole('region', { name: 'Reflog: HEAD' });
    await expect(view).toBeVisible();
    await expect(view.getByText('reset: moving to HEAD~1')).toBeVisible();
    await expect(view.getByText('commit (amend): tidy commit message')).toBeVisible();
    await expect(view.getByText('commit (initial): base')).toBeVisible();
    await page.getByRole('button', { name: 'Close reflog' }).click();
    await expect(view).toBeHidden();
  });

  test('undo (default seam): reset plan confirmed → branch moves back', async ({ page }) => {
    await openRepo(page);
    await page.getByRole('button', { name: '↶ Undo' }).click();
    const dialog = page.getByRole('dialog', { name: 'Undo reset' });
    await expect(dialog).toBeVisible();
    // RESET_HEAD.oldOid = fixture row 2 → target short 0202020 (mixed).
    await expect(dialog.getByText('0202020')).toBeVisible();
    await dialog.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect(
      page.locator('.toast-stack').getByText('Reset main to 0202020 (mixed)'),
    ).toBeVisible();
  });

  test('undo seams: switch / none / merge-on-dirty-tree are blocked', async ({ page }) => {
    // ?undo=switch → not undoable, with the branch-switch reason.
    await openRepo(page, { flags: { undo: 'switch' } });
    await page.getByRole('button', { name: '↶ Undo' }).click();
    let dialog = page.getByRole('dialog', { name: 'Undo branch switch' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(/switching branches isn't undone here/)).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
    await dialog.getByRole('button', { name: 'Cancel' }).click();

    // ?undo=none → empty reflog → nothing to undo.
    await openRepo(page, { flags: { undo: 'none' } });
    await page.getByRole('button', { name: '↶ Undo' }).click();
    dialog = page.getByRole('dialog', { name: 'Undo last operation' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText('nothing to undo')).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
    await dialog.getByRole('button', { name: 'Cancel' }).click();

    // ?undo=merge → hard-reset undo, blocked while the (default-dirty) tree
    // has tracked changes (requiresCleanWorktree).
    await openRepo(page, { flags: { undo: 'merge' } });
    await page.getByRole('button', { name: '↶ Undo' }).click();
    dialog = page.getByRole('dialog', { name: 'Undo merge' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText('Commit or stash your changes first.')).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
  });

  test('repo health panel renders sections and refreshes', async ({ page }) => {
    await openRepo(page);
    const palette = await openPalette(page);
    await palette.getByRole('combobox', { name: 'Command palette' }).fill('health');
    await palette.getByRole('option', { name: /Repository Health/ }).click();
    const dialog = page.getByRole('dialog', { name: 'Repo Health' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText('Stats')).toBeVisible();
    await expect(dialog.getByText('Branches')).toBeVisible();
    await expect(dialog.getByText('Working state')).toBeVisible();
    await dialog.getByRole('button', { name: 'Refresh', exact: true }).click();
    await expect(dialog.getByText('Stats')).toBeVisible(); // survives a refresh
    await dialog.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(dialog).toBeHidden();
  });

  test('palette dispatch: Open Settings and manual Refresh run their handlers', async ({
    page,
  }) => {
    await openRepo(page);
    let palette = await openPalette(page);
    await palette.getByRole('combobox', { name: 'Command palette' }).fill('open settings');
    await palette.getByRole('option', { name: /Open Settings/ }).click();
    const settings = page.getByRole('dialog', { name: 'Settings' });
    await expect(settings).toBeVisible();
    await settings.getByRole('button', { name: 'Close' }).click();
    await expect(settings).toBeHidden();

    palette = await openPalette(page);
    await palette.getByRole('combobox', { name: 'Command palette' }).fill('refresh');
    await palette.getByRole('option', { name: /^Refresh/ }).click();
    await expect(palette).toBeHidden();
    // Refresh is silent on success; the console-error gate + live canvas are
    // the assertion (matches spec 13's Mod+R case).
    await expect(graphCanvas(page)).toBeVisible();
    await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
  });

  test('file history and blame render for a fixture path', async ({ page }) => {
    await openRepo(page);
    await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
    // README.md (unstaged, tracked) carries both fixtures (BLAME_FIXTURE_PATHS).
    await page.getByRole('button', { name: 'Show history of README.md' }).click();
    const history = page.getByRole('region', { name: 'File history: README.md' });
    await expect(history).toBeVisible();
    await page.getByRole('button', { name: 'Close file history' }).click();
    await expect(history).toBeHidden();

    await page.getByRole('button', { name: 'Blame README.md' }).click();
    const blame = page.getByRole('region', { name: 'Blame: README.md' });
    await expect(blame).toBeVisible();
    await page.getByRole('button', { name: 'Close blame' }).click();
    await expect(blame).toBeHidden();
  });
});
