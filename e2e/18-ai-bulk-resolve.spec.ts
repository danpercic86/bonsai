/**
 * P68f §9 — "Resolve all with AI" in the mock-IPC browser harness.
 *
 * This spec exists because P68e §13.2-6 could not be proved: the per-file `AiRunQueue`
 * had no way to appear, since a BULK run had no entry point yet. It has one now, so the
 * queue's per-file status, its `reason` column and the path from a finished file to its
 * proposal are all asserted here for the first time.
 *
 * SCOPE, stated honestly (same limitation as spec 17): this harness composites at 0×0,
 * so `requestAnimationFrame` is paused and nothing about APPEARANCE is verifiable.
 * What is verified is DOM structure, text, state transitions and the resulting IPC
 * effects — which is exactly where the "one run for all conflicts" contract lives.
 *
 * The `?op=merge` fixture pauses a merge with THREE conflicts, TWO of them AI-eligible
 * (`src/auth.ts` and the deep i18n JSON path); `README.md` is `deletedByThem` and must
 * never be dragged into a bulk run.
 */
import { test, expect } from './fixtures';
import { openRepo } from './helpers';
import { MERGE_DEEP_PATH } from '../src/ipc/fixtures/conflicts';
import type { Locator, Page } from '@playwright/test';

const AI = { onboardingSeen: true, aiConsented: true };
const BULK_NAME = 'Resolve all 2 conflicts with AI';
const CANCEL_NAME = 'Cancel the AI run for all 2 files';

function dock(page: Page): Locator {
  return page.getByRole('region', { name: 'AI activity' });
}
function banner(page: Page): Locator {
  return page.locator('.op-banner');
}
function conflictsHeader(page: Page): Locator {
  return page.locator('.section-header', { hasText: 'Conflicts (' });
}
function queueRows(page: Page): Locator {
  return dock(page).locator('.ai-run-queue-row');
}

/** Open the paused merge, click "Resolve all with AI" at `from`, and confirm. */
async function resolveAll(
  page: Page,
  from: 'banner' | 'section',
  flags: Record<string, string> = {},
  uiSettings: Record<string, unknown> = {},
): Promise<void> {
  await openRepo(page, { flags: { op: 'merge', ...flags }, uiSettings: { ...AI, ...uiSettings } });
  const host = from === 'banner' ? banner(page) : conflictsHeader(page);
  const button = host.getByRole('button', { name: BULK_NAME });
  await expect(button).toBeEnabled();
  await button.click();
  // Nothing has been spent yet: the confirm gate is in front.
  const dialog = page.getByRole('dialog', { name: 'Resolve all conflicts with AI' });
  await expect(dialog).toBeVisible();
  await expect(dock(page)).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Resolve all with AI' }).click();
}

test.describe('18 bulk "Resolve all with AI"', () => {
  test('both entry points offer it, and the confirm gate states the cost before anything runs', async ({
    page,
  }) => {
    await openRepo(page, { flags: { op: 'merge' }, uiSettings: AI });
    // OQ4: BOTH — the conflicts header and the merge banner.
    await expect(conflictsHeader(page).getByRole('button', { name: BULK_NAME })).toBeEnabled();
    await expect(banner(page).getByRole('button', { name: BULK_NAME })).toBeEnabled();

    await conflictsHeader(page).getByRole('button', { name: BULK_NAME }).click();
    const dialog = page.getByRole('dialog', { name: 'Resolve all conflicts with AI' });
    await expect(dialog).toContainText('Send 2 conflicted files to Claude');
    await expect(dialog).toContainText('one or more Claude runs');
    await expect(dialog).toContainText('using your Claude quota');
    await expect(dialog).toContainText('src/auth.ts');
    await expect(dialog).toContainText(MERGE_DEEP_PATH);
    // The ineligible deletion conflict is NOT part of it.
    await expect(dialog).not.toContainText('README.md');

    // Cancelling spends nothing and creates no run.
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
    await expect(dock(page)).toHaveCount(0);
  });

  test('is not offered with fewer than two eligible conflicts, and is inert with ?ai=off', async ({
    page,
  }) => {
    await openRepo(page, { flags: { op: 'merge', ai: 'off' }, uiSettings: AI });
    // Same treatment as the per-row ✨AI button: visible, disabled, and it says why.
    const off = conflictsHeader(page).getByRole('button', { name: BULK_NAME });
    await expect(off).toBeDisabled();
    await expect(off).toHaveAttribute('title', 'Enable AI features in Settings to use this');

    // Resolve one of the two eligible files by hand → one left → the affordance goes.
    await page.getByRole('button', { name: 'Take our version of src/auth.ts' }).click();
    await expect(banner(page).getByText('2 conflict(s) remaining')).toBeVisible();
    await expect(conflictsHeader(page).getByRole('button', { name: /Resolve all/ })).toHaveCount(0);
    await expect(banner(page).getByRole('button', { name: /Resolve all/ })).toHaveCount(0);
  });

  test('ONE run covers both files: one dock entry, a two-row queue, and Review opens a proposal', async ({
    page,
  }) => {
    await resolveAll(page, 'section');

    // ONE run: the dock's subject is the batch, and the multi-run strip (which only
    // renders for >1 run) never appears.
    await expect(dock(page).locator('.ai-dock-subject')).toHaveText('2 conflicts');
    await expect(dock(page).locator('.ai-dock-runs')).toHaveCount(0);

    // §13.2-6, unprovable until now: the per-file queue.
    await expect(queueRows(page)).toHaveCount(2);
    await expect(dock(page).locator('.ai-dock-status')).toHaveText(/Ready/);
    const rows = queueRows(page);
    await expect(rows.nth(0)).toContainText('auth.ts');
    await expect(rows.nth(0)).toContainText('Ready');
    await expect(rows.nth(1)).toContainText('messages.json');
    await expect(rows.nth(1)).toContainText('Ready');
    await expect(rows.nth(1)).toHaveAttribute('data-status', 'ready');

    // Getting from a finished file to reviewing its proposal — the only route to
    // proposal #2 (the row `Review` button, §5.1-4).
    await rows.nth(1).getByRole('button', { name: `Review AI proposal for ${MERGE_DEEP_PATH}` }).click();
    await expect(page.getByRole('region', { name: `Diff: ${MERGE_DEEP_PATH}` })).toBeVisible();
    await expect(page.getByTestId('conflict-editor')).toBeVisible();

    // Bulk never auto-steals the centre pane, so the dock is where the user is sent.
    await expect(page.locator('.toast-stack')).toContainText(
      'AI proposals ready for 2 files — review them from the AI activity dock',
    );
  });

  test('?aiFail: a per-file failure marks ONLY its own row and reports the reason', async ({
    page,
  }) => {
    await resolveAll(page, 'banner', { aiFail: '1' });
    await expect(queueRows(page)).toHaveCount(2);
    const ok = queueRows(page).nth(0);
    const bad = queueRows(page).nth(1);

    await expect(ok).toHaveAttribute('data-status', 'ready');
    await expect(ok.getByRole('button', { name: /^Review AI proposal/ })).toBeVisible();

    await expect(bad).toHaveAttribute('data-status', 'failed');
    await expect(bad).toContainText('Failed');
    await expect(bad.locator('.ai-run-queue-reason')).toHaveText('no result block returned');
    await expect(bad.getByRole('button', { name: /^Retry AI resolution/ })).toBeVisible();

    // The conflict rows agree: one reviewable, one failed — and D11 held (one bad file
    // never cost the other one its result).
    await expect(page.getByRole('button', { name: 'Resolve src/auth.ts with AI' })).toHaveText(
      '✓ review',
    );
    await expect(page.getByRole('button', { name: `Resolve ${MERGE_DEEP_PATH} with AI` })).toHaveText(
      '⚠',
    );
  });

  test('?aiMarkers + autoResolve: marker-free files are staged, markerful falls back to review', async ({
    page,
  }) => {
    await resolveAll(page, 'section', { aiMarkers: '1' }, { aiConflictAutonomy: 'autoResolve' });

    // THE SAFETY GATE. `settleBatch` demotes the markerful body BEFORE deciding what is
    // stageable, so the clean file stages and the markerful one is never presented as a
    // clean merge — it opens for review instead.
    await expect(page.locator('.toast-stack')).toContainText(
      'Resolved src/auth.ts with AI — review the staged result',
    );
    await expect(page.locator('.toast-stack')).toContainText(
      `AI left unresolved markers in ${MERGE_DEEP_PATH} — opened for review`,
    );
    // Staged ⇒ it left the conflicted list (ONE refresh does that for the whole batch).
    await expect(banner(page).getByText('2 conflict(s) remaining')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Take our version of src/auth.ts' })).toHaveCount(
      0,
    );
    // The markerful one is still conflicted, marked failed, and open in the centre pane.
    await expect(page.getByRole('button', { name: `Resolve ${MERGE_DEEP_PATH} with AI` })).toHaveText(
      '⚠',
    );
    await expect(page.getByRole('region', { name: `Diff: ${MERGE_DEEP_PATH}` })).toBeVisible();
    // The staged body carries no markers (proof it was the stripped one).
    await expect(page.getByTestId('conflict-editor')).toContainText('<<<<<<<');
  });

  test('?aiSlow: "Cancel all" stops the ONE run, and the button says so at once', async ({
    page,
  }) => {
    await resolveAll(page, 'banner', { aiSlow: '1' });
    await expect(dock(page).locator('.ai-dock-status')).toHaveText(/Running/);

    const cancel = banner(page).getByRole('button', { name: CANCEL_NAME });
    await expect(cancel).toHaveText('Cancel all');
    await cancel.click();
    // Immediate feedback, before any IPC settles (the store's `cancelRequested`).
    await expect(banner(page).getByRole('button', { name: /Resolve all|Cancel|Stopping/ })).toHaveText(
      'Stopping…',
    );

    await expect(dock(page).locator('.ai-dock-status')).toHaveText(/Cancelled/);
    // ONE run ⇒ ONE cancel: still a single dock entry, and both files are unresolved.
    await expect(dock(page).locator('.ai-dock-runs')).toHaveCount(0);
    await expect(banner(page).getByText('3 conflict(s) remaining')).toBeVisible();
  });
});
