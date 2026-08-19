/**
 * P69h — the Settings shell's Git-config half, in a real browser
 * (`docs/contracts/P69-settings-shell.md` §5.4 / §6 P69h, UI §1.1–§1.2, §7.2).
 *
 * jsdom cannot answer three of this increment's questions: whether `Ctrl/Cmd+,`
 * survives the app's real keydown stack, whether the scope switch and the empty
 * block are actually CLICKABLE (P69g shipped two pointer bugs that jsdom could
 * not see — a visible layer swallowing the click), and whether the deep link
 * lands when Settings is already open on another category.
 */
import { test, expect } from './fixtures';
import { gotoHarness, openPalette, openRepo, skipOnboarding } from './helpers';
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

test.describe('24 settings shell — Git config @smoke', () => {
  test('Ctrl/Cmd+, opens Settings and is a no-op while it is open', async ({ page }) => {
    await openRepo(page);
    await page.keyboard.press('ControlOrMeta+Comma');
    const dialog = settingsDialog(page);
    await expect(dialog).toBeVisible();
    // A shortcut that TOGGLES a modal is surprising (UI §7.2) — pressing it
    // again must leave it open, not close it.
    await page.keyboard.press('ControlOrMeta+Comma');
    await expect(dialog).toBeVisible();
  });

  test('the Git config pane names its scope, and the switch really switches', async ({ page }) => {
    await openRepo(page);
    const dialog = await openSettings(page);
    await dialog.getByRole('tab', { name: 'Git config, repository' }).click();

    // The rail says `repo`, and the pane header says WHICH file.
    await expect(dialog.getByText('Editing .git/config in bonsai-fixture')).toBeVisible();
    const scope = dialog.getByRole('radiogroup', { name: 'Scope' });
    await expect(scope).toBeVisible();
    await expect(dialog.getByRole('radio', { name: 'This repository' })).toBeChecked();

    // The layering jsdom cannot check: the transparent radio input is stretched
    // over the WHOLE segment, so a click on the visible label text lands on the
    // control instead of being swallowed by it (UI §7.4 hit target).
    const globalRadio = dialog.getByRole('radio', { name: 'Global' });
    const inputBox = await globalRadio.boundingBox();
    const labelBox = await dialog.getByText('Global', { exact: true }).boundingBox();
    const segmentBox = await dialog
      .locator('.settings-segment')
      .filter({ hasText: 'Global' })
      .boundingBox();
    expect(inputBox).not.toBeNull();
    expect(labelBox).not.toBeNull();
    expect(inputBox!.width).toBeGreaterThanOrEqual(labelBox!.width);
    expect(segmentBox!.height).toBeGreaterThanOrEqual(24); // UI §7.4

    await globalRadio.click();
    await expect(globalRadio).toBeChecked();
    await expect(dialog.getByText(/global Git config/)).toBeVisible();

    await dialog.getByRole('radio', { name: 'This repository' }).click();
    await expect(dialog.getByRole('radio', { name: 'This repository' })).toBeChecked();
  });

  test('the hooks switch and an identity field are both live in the pane', async ({ page }) => {
    await openRepo(page);
    const dialog = await openSettings(page);
    await dialog.getByRole('tab', { name: 'Git config, repository' }).click();

    const hooks = dialog.getByRole('checkbox', { name: 'Run git hooks for this repository' });
    await expect(hooks).toBeChecked();
    await hooks.uncheck();
    await expect(hooks).not.toBeChecked();
    await hooks.check();
    await expect(hooks).toBeChecked();

    const name = dialog.getByRole('textbox', { name: 'user.name', exact: true });
    await name.fill('E2E Ada');
    await name.blur();
    await expect(name).toHaveValue('E2E Ada');
  });

  test('with no repo open the pane offers a way OUT, not a dead form', async ({ page }) => {
    await gotoHarness(page, { uiSettings: {} });
    await skipOnboarding(page);
    const dialog = await openSettings(page);
    const tab = dialog.getByRole('tab', { name: 'Git config, repository' });
    await expect(tab).toBeEnabled(); // never a disabled rail item (UI §6.1)
    await tab.click();

    await expect(dialog.getByText('No repository open')).toBeVisible();
    await expect(dialog.getByRole('radiogroup', { name: 'Scope' })).toHaveCount(0);
    // The action is the point of the block — it must actually be clickable.
    await dialog.getByRole('button', { name: 'Open repository…' }).click();
    await expect(dialog.getByRole('radiogroup', { name: 'Scope' })).toBeVisible();
  });

  test('a configMissing deep link opens Settings on Git config, focused on user.name', async ({
    page,
  }) => {
    await openRepo(page, { flags: { fixture: 'noconfig' } });
    await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();

    await page.getByPlaceholder('Commit message').fill('e2e: should fail');
    await page.getByRole('button', { name: 'Commit', exact: true }).click();
    await page.getByRole('button', { name: 'Set identity…' }).click();

    await expect(
      settingsDialog(page).getByRole('tab', { name: 'Git config, repository' }),
    ).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByLabel('user.name', { exact: true })).toBeFocused();
  });

  test('a deep link re-targets the pane while Settings is ALREADY open', async ({ page }) => {
    await openRepo(page, { flags: { fixture: 'noconfig' } });
    await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
    await page.getByPlaceholder('Commit message').fill('e2e: should fail');
    await page.getByRole('button', { name: 'Commit', exact: true }).click();
    const setIdentity = page.getByRole('button', { name: 'Set identity…' });
    await expect(setIdentity).toBeVisible();

    // Park Settings on another category. This is the case a fresh mount cannot
    // cover and the only reason `requestSeq` exists (§5.4.3).
    const dialog = await openSettings(page);
    await dialog.getByRole('tab', { name: 'Appearance' }).click();
    await expect(dialog.getByRole('tab', { name: 'Appearance' })).toHaveAttribute(
      'aria-selected',
      'true',
    );

    // `dispatchEvent`, not `click`: the button is behind the modal's backdrop, so
    // no pointer can reach it — which is exactly why this path has no
    // click-through test. Everything downstream of the handler (App's
    // `openSettingsAt` → `requestSeq` → the shell's re-seed → the section's
    // focus effect) is the real thing, in a real browser.
    await setIdentity.dispatchEvent('click');

    await expect(dialog.getByRole('tab', { name: 'Git config, repository' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await expect(page.getByLabel('user.name', { exact: true })).toBeFocused();
  });

  test('the palette reaches Git config directly', async ({ page }) => {
    // The palette itself only exists with a repo open (RepoWorkspace owns it),
    // so `app.gitConfig`'s no-repo disabled state is a unit-level fact; what
    // needs a browser is that the entry really deep-links.
    await openRepo(page);
    const palette = await openPalette(page);
    await palette.getByRole('combobox', { name: 'Command palette' }).fill('git config');
    await palette.getByRole('option', { name: /Open Git config/ }).click();
    await expect(
      settingsDialog(page).getByRole('tab', { name: 'Git config, repository' }),
    ).toHaveAttribute('aria-selected', 'true');
  });
});
