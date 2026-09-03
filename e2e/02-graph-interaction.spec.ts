/**
 * T4 spec 02 — graph interaction (contract §5.02) @smoke.
 *
 * Display-row map of the default fixture (repoState/layout.ts resolveLayout):
 *   row 0      — WIP row (default status is dirty; present once status loads)
 *   rows 1–3   — the 3 seeded stash offshoot nodes (withStashNodes)
 *   row 4      — 'Merge feat and exp' (HEAD, main pill)
 *   row 5      — 'feat: polish'
 * Tests wait for the status panel before row clicks so the WIP offset is
 * deterministic (contract §3 stabilization note).
 */
import { test, expect } from './fixtures';
import {
  clickGraphRow,
  graphCanvas,
  openRepo,
  scrollGraphTo,
  waitForGraphSettled,
} from './helpers';
import type { Page } from '@playwright/test';

/** openRepo + wait for BOTH graph-extent inputs — the stream's `meta` chunk and
 *  the first status round (⇒ the WIP row exists, display-row offsets are stable
 *  and the scroll extent covers the whole model). `openRepo` itself waits only
 *  for the canvas to be VISIBLE, which the pane renders before any data lands:
 *  the scroll assertion below needs an extent that already exceeds the
 *  viewport, and every `clickGraphRow(4)` here needs the WIP row counted. */
async function openWithStatus(page: Page, flags?: Record<string, string>): Promise<void> {
  await openRepo(page, flags ? { flags } : undefined);
  await waitForGraphSettled(page);
}

test.describe('02 graph interaction @smoke', () => {
  test('clicking a row opens commit details with message, author, date and files', async ({
    page,
  }) => {
    await openWithStatus(page);
    await clickGraphRow(page, 4);
    const details = page.getByTestId('commit-details');
    await expect(details).toBeVisible();
    await expect(details.getByText('Merge feat and exp').first()).toBeVisible();
    // .first(): the author line AND the signature-signer span both carry the
    // name (the fixture HEAD commit is signed) — strict mode needs the pick.
    await expect(details.getByText('Ada Lovelace').first()).toBeVisible();
    // getCommitDiff file list renders (fixture index 0 files).
    await expect(details.getByText('engine.rs')).toBeVisible();
    await expect(details.getByText('pipeline.rs')).toBeVisible();
  });

  test('clicking a different row updates the details panel', async ({ page }) => {
    await openWithStatus(page);
    await clickGraphRow(page, 4);
    await expect(
      page.getByTestId('commit-details').getByText('Merge feat and exp').first(),
    ).toBeVisible();
    await clickGraphRow(page, 5);
    const details = page.getByTestId('commit-details');
    await expect(details.getByText('feat: polish').first()).toBeVisible();
    await expect(details.getByText('Grace Hopper')).toBeVisible();
  });

  test('ref context: HEAD row selection matches the sidebar head branch', async ({ page }) => {
    // §5.02.3 [RENDER] downgrade: the canvas is opaque to DOM queries — assert
    // via sidebar head glyph + details instead of pixel-inspecting pills.
    await openWithStatus(page);
    const mainRow = page
      .locator('li')
      .filter({ has: page.getByTitle('main', { exact: true }) })
      .first();
    await expect(mainRow).toHaveAttribute('aria-current', 'true');
    // The HEAD display row shows the HEAD commit's details.
    await clickGraphRow(page, 4);
    await expect(
      page.getByTestId('commit-details').getByText('Merge feat and exp').first(),
    ).toBeVisible();
  });

  test('scrolling the graph applies scrollTop and keeps the canvas alive', async ({ page }) => {
    await openWithStatus(page);
    const applied = await scrollGraphTo(page, 2000); // clamps to the max extent
    expect(applied).toBeGreaterThan(0);
    await expect(graphCanvas(page)).toBeVisible();
    const backToTop = await scrollGraphTo(page, 0);
    expect(backToTop).toBe(0);
    await expect(graphCanvas(page)).toBeVisible();
  });

  test('detached fixture boots and the sidebar shows the detached indicator', async ({ page }) => {
    await openRepo(page, { flags: { fixture: 'detached' } });
    await expect(page.getByText(/HEAD detached @/)).toBeVisible();
    await expect(graphCanvas(page)).toBeVisible();
  });
});
