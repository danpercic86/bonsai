/**
 * T4 spec 06 — merge, conflicts, conflict editor (contract §5.06) @destructive.
 * The seeded paused merge (`?op=merge`, opStateSeed.ts) is the authoritative
 * editor flow; a FRESH conflicted merge only returns a conflicts outcome
 * WITHOUT seeding opState (merge.ts) — so that case is outcome-toast-only
 * (§5.06.7 downgrade).
 */
import { test, expect } from './fixtures';
import {
  clickGraphRow,
  confirm,
  graphScrollHeight,
  openBranchContextMenu,
  openRepo,
} from './helpers';
import type { Page } from '@playwright/test';

/** The P68d deep i18n conflict path (fixtures/conflicts.ts MERGE_DEEP_PATH). */
const DEEP =
  'src/features/internationalization/locales/de-DE/components/settings/advanced/notifications/messages.json';

const FLAT = { uiSettings: { onboardingSeen: true, listView: 'flat' } };

async function openPausedMerge(page: Page): Promise<void> {
  await openRepo(page, { flags: { op: 'merge' } });
  await expect(page.getByRole('status').getByText('Merging feature/login')).toBeVisible();
}

/** The op banner (role=status) — scopes its action buttons (the CommitBox also
 *  renders a "Commit merge" button in merge mode). */
function banner(page: Page) {
  return page.getByRole('status');
}

test.describe('06 merge & conflicts @destructive', () => {
  // P68d seeded a THIRD conflicted path (the deep i18n JSON) so the AI dock/bulk
  // paths have two text-mergeable files; the counts below follow it.
  test('seeded paused merge shows the banner and all three conflicted rows', async ({ page }) => {
    await openPausedMerge(page);
    await expect(banner(page).getByText('3 conflict(s) remaining')).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Take our version of README.md' }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Take our version of src/auth.ts' }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: `Take our version of ${DEEP}` }),
    ).toBeVisible();
    // Commit merge is gated while conflicts remain.
    await expect(banner(page).getByRole('button', { name: 'Commit merge' })).toBeDisabled();
  });

  test('conflict editor: ours/theirs regions render, resolve stages the file', async ({
    page,
  }) => {
    await openPausedMerge(page);
    // Open src/auth.ts in the editor (the row's main button carries
    // aria-expanded; the quick-action buttons do not).
    await page.getByRole('button', { name: /auth\.ts/, expanded: false }).click();
    await expect(page.getByRole('region', { name: 'Diff: src/auth.ts' })).toBeVisible();
    const editor = page.getByTestId('conflict-editor');
    await expect(editor).toBeVisible();
    // The marker fixture renders one region with its widget toolbar + captions.
    await expect(editor.getByText('Ours (HEAD)')).toBeVisible();
    await expect(editor.getByText('Theirs (feature/login)')).toBeVisible();
    // Stage-resolved gates on unresolved markers until a choice is accepted.
    await expect(editor.getByRole('button', { name: 'Stage resolved' })).toBeDisabled();
    await editor.getByRole('button', { name: 'Accept Ours' }).click();
    await editor.getByRole('button', { name: 'Stage resolved' }).click();
    // resolveConflictText → the row leaves the conflicted list.
    await expect(
      page.locator('.toast-stack').getByText('Staged resolution for src/auth.ts'),
    ).toBeVisible();
    await expect(banner(page).getByText('2 conflict(s) remaining')).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Take our version of src/auth.ts' }),
    ).toHaveCount(0);
  });

  test('deletedByThem README.md resolves via quick action (accept their deletion)', async ({
    page,
  }) => {
    await openPausedMerge(page);
    await page.getByRole('button', { name: 'Take their version of README.md' }).click();
    await expect(
      page.getByRole('button', { name: 'Take their version of README.md' }),
    ).toHaveCount(0);
    await expect(banner(page).getByText('2 conflict(s) remaining')).toBeVisible();
  });

  test('commit merge: prefilled message, banner clears, graph gains the merge commit', async ({
    page,
  }) => {
    await openPausedMerge(page);
    const before = await graphScrollHeight(page);
    // Resolve all three conflicts via quick actions.
    await page.getByRole('button', { name: 'Take our version of src/auth.ts' }).click();
    await page.getByRole('button', { name: 'Take our version of README.md' }).click();
    await page.getByRole('button', { name: `Take our version of ${DEEP}` }).click();
    await expect(banner(page).getByText('All conflicts resolved')).toBeVisible();
    // Message prefilled from opState into the merge commit box.
    await expect(page.getByPlaceholder('Merge commit message')).toHaveValue(
      /Merge branch 'feature\/login'/,
    );
    await banner(page).getByRole('button', { name: 'Commit merge' }).click();
    await expect(page.locator('.toast-stack').getByText('Merge committed')).toBeVisible();
    await expect(banner(page)).toHaveCount(0); // OpBanner cleared
    // Graph gained one merge row; wait for the refreshed layout before clicking.
    await expect.poll(() => graphScrollHeight(page)).toBe(before + 32);
    await clickGraphRow(page, 4); // wip(1) + stashes(3) + merge commit at row 3
    await expect(
      page.getByTestId('commit-details').getByText("Merge branch 'feature/login'").first(),
    ).toBeVisible();
  });

  test('abort merge: confirm-gated, opState and conflicts clear', async ({ page }) => {
    await openPausedMerge(page);
    await banner(page).getByRole('button', { name: 'Abort' }).click();
    await confirm(page, 'Abort merge?', 'Abort merge');
    await expect(page.locator('.toast-stack').getByText('Merge aborted')).toBeVisible();
    await expect(banner(page)).toHaveCount(0);
    await expect(page.getByRole('button', { name: /Take our version/ })).toHaveCount(0);
  });

  test('fresh clean merge commits a new merge node', async ({ page }) => {
    await openRepo(page, FLAT);
    await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
    const before = await graphScrollHeight(page);
    await page.getByRole('button', { name: 'Create branch' }).click();
    const input = page.getByPlaceholder('new-branch-name');
    await input.fill('demo-clean');
    await input.press('Enter');
    await expect(page.getByTitle('demo-clean', { exact: true })).toBeVisible();
    const menu = await openBranchContextMenu(page, 'demo-clean');
    await menu.getByRole('menuitem', { name: 'Merge demo-clean into main' }).click();
    await expect(page.locator('.toast-stack').getByText(/Merged demo-clean/)).toBeVisible();
    await expect.poll(() => graphScrollHeight(page)).toBe(before + 32);
    await clickGraphRow(page, 4);
    await expect(
      page.getByTestId('commit-details').getByText("Merge branch 'demo-clean'").first(),
    ).toBeVisible();
  });

  test('fresh conflicted merge surfaces the paused outcome (render-only)', async ({ page }) => {
    // §5.06.7 [RENDER] downgrade: mergeBranch('…conflict') returns the
    // conflicts outcome without seeding opState (merge.ts:34) — assert the
    // outcome toast + app usability only.
    await openRepo(page, FLAT);
    await page.getByRole('button', { name: 'Create branch' }).click();
    const input = page.getByPlaceholder('new-branch-name');
    await input.fill('demo-conflict');
    await input.press('Enter');
    await expect(page.getByTitle('demo-conflict', { exact: true })).toBeVisible();
    const menu = await openBranchContextMenu(page, 'demo-conflict');
    await menu.getByRole('menuitem', { name: 'Merge demo-conflict into main' }).click();
    await expect(
      page.locator('.toast-stack').getByText(/Merge paused: 3 conflict\(s\) to resolve/),
    ).toBeVisible();
    // App stays usable.
    await expect(page.getByTestId('graph-canvas')).toBeVisible();
  });
});
