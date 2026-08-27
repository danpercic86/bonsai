/**
 * Spec-004 fold linear runs — UI-contract smoke (plan.md §Testing "Harness/e2e"
 * + UI contract §9): toggle via the filter popover, "⋯ N commits" pill row,
 * expand by click and keyboard (Enter), collapse via ArrowLeft, AC4 exact
 * restore, AC6 selection-inside-run, composition with first-parent, chip
 * `Folded` segment, persistence across reload, and the §3/§7 a11y contract
 * (aria-activedescendant resolves to a real row carrying the pill name).
 *
 * Fixture facts (fixtures/graph.ts + withStashNodes; mock graphFold.ts):
 *  - Full unfiltered model = 33 rows: 3 stash offshoots + 30 base rows.
 *  - Exactly ONE fold span: model rows 10..29 — the refs-free lane-0 chain
 *    ("core work 1" + "chore: history 19..1") — {start: 10, count: 20}.
 *    Rows 8–9 have multi-in-degree (merge bases + stash edges), row 30 is the
 *    root (no outgoing edge), so the run is exactly 20 rows → pill N = 20 and
 *    fold-on shrinks the graph by 19 display rows (20 hidden − 1 pill).
 *  - Display rows include the WIP row at display 0 when present; the offset is
 *    measured, never assumed (content extent = (rows [+ wip]) × rowHeight + 8).
 *  - Heights are read from the .graph-spacer element, NOT scroller.scrollHeight:
 *    the folded graph (14–15 rows ≈ 488px) is SHORTER than the scroller's
 *    viewport, and scrollHeight clamps to clientHeight — the shrink would be
 *    invisible. The spacer is the true content extent at any size.
 *  - Toggle-ON re-requests the stream (async) → every height change is polled.
 */
import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import { DEFAULT_ROW_HEIGHT, clickGraphRow, graphScroller, openRepo } from './helpers';

const FLAT = { onboardingSeen: true, listView: 'flat' };
const MODEL_ROWS = 33; // 3 stash nodes + 30 base rows
const SPAN_HIDDEN = 20; // model rows 10..29
const SPAN_START = 10;

function filterFab(page: Page) {
  return page.getByRole('button', { name: 'Graph filters', exact: true });
}
function chip(page: Page, summary: string) {
  return page.getByRole('button', { name: `Graph filters: ${summary}`, exact: true });
}

/** Open the popover (from the quiet fab or the active chip) and flip the
 *  "Fold linear runs" switch, then Esc-close so the graph is unobstructed. */
async function toggleFold(page: Page, opener: 'fab' | string): Promise<void> {
  if (opener === 'fab') await filterFab(page).click();
  else await chip(page, opener).click();
  const popover = page.getByRole('dialog', { name: 'Graph filters' });
  await expect(popover).toBeVisible();
  await popover.getByLabel('Fold linear runs').click();
  await page.keyboard.press('Escape');
  await expect(popover).toBeHidden();
}

/** True content extent — the virtualization spacer's height (see header note
 *  on why scroller.scrollHeight cannot be used here). */
async function graphContentHeight(page: Page): Promise<number> {
  return graphScroller(page)
    .locator('.graph-spacer')
    .evaluate((el) => parseFloat((el as HTMLElement).style.height));
}

/** Display rows above the graph's model row 0 (the WIP row, if the mock status
 *  produces one) — measured from the unfolded content extent, never assumed. */
async function wipOffset(page: Page): Promise<number> {
  const h = await graphContentHeight(page);
  return Math.round((h - 8) / DEFAULT_ROW_HEIGHT) - MODEL_ROWS;
}

/** The sr-only active-descendant row the scroller currently points at. */
async function activeRow(page: Page) {
  const id = await graphScroller(page).getAttribute('aria-activedescendant');
  expect(id).not.toBeNull();
  return page.locator(`#${id}`);
}

test.describe('29 graph fold linear runs @smoke', () => {
  test('toggle on shows one ⋯ 20 commits span; toggle off restores exactly (AC1/AC4/AC7 chip)', async ({
    page,
  }) => {
    await openRepo(page, { uiSettings: FLAT });
    const fullHeight = await graphContentHeight(page);

    await toggleFold(page, 'fab');
    // Chip gains the Folded segment; the span collapses 20 rows into 1 pill.
    await expect(chip(page, 'Folded')).toBeVisible();
    await expect
      .poll(() => graphContentHeight(page))
      .toBe(fullHeight - (SPAN_HIDDEN - 1) * DEFAULT_ROW_HEIGHT);

    // AC4: off restores the exact pre-fold extent (no refetch needed, but poll
    // anyway — the paint is async) and the chip collapses to the quiet fab.
    await toggleFold(page, 'Folded');
    await expect.poll(() => graphContentHeight(page)).toBe(fullHeight);
    await expect(filterFab(page)).toBeVisible();
  });

  test('expand via pill click; keyboard: arrow onto pill, a11y name, Enter expands, ← collapses (AC2)', async ({
    page,
  }) => {
    await openRepo(page, { uiSettings: FLAT });
    const wip = await wipOffset(page);
    const fullHeight = await graphContentHeight(page);
    const foldedHeight = fullHeight - (SPAN_HIDDEN - 1) * DEFAULT_ROW_HEIGHT;

    await toggleFold(page, 'fab');
    await expect.poll(() => graphContentHeight(page)).toBe(foldedHeight);

    // Keyboard expand: select the commit just above the pill (model 9), arrow
    // down onto the pill row — active descendant moves, selection does NOT.
    await clickGraphRow(page, wip + SPAN_START - 1);
    const details = page.getByTestId('commit-details');
    await expect(details.getByText('core work 2').first()).toBeVisible();
    await page.keyboard.press('ArrowDown');

    // §3/§7 a11y: aria-activedescendant resolves to a REAL element carrying
    // the pill accessible name; the pill is never a selected commit.
    const pillRow = await activeRow(page);
    await expect(pillRow).toBeAttached();
    await expect(pillRow).toHaveText('20 commits folded. Press Enter to expand.');
    await expect(pillRow).toHaveAttribute('aria-expanded', 'false');
    await expect(pillRow).toHaveAttribute('aria-selected', 'false');
    await expect(details.getByText('core work 2').first()).toBeVisible();

    await page.keyboard.press('Enter');
    await expect.poll(() => graphContentHeight(page)).toBe(fullHeight);

    // Re-collapse via keyboard from inside the expanded run: select the run's
    // boundary row (model 10 = 'core work 1'), then ArrowLeft.
    await clickGraphRow(page, wip + SPAN_START);
    await expect(details.getByText('core work 1').first()).toBeVisible();
    await page.keyboard.press('ArrowLeft');
    await expect.poll(() => graphContentHeight(page)).toBe(foldedHeight);

    // Mouse: the whole pill row (display wip + SPAN_START) is the hit target.
    await clickGraphRow(page, wip + SPAN_START);
    await expect.poll(() => graphContentHeight(page)).toBe(fullHeight);
  });

  test('AC6: selection inside the run keeps its span expanded when fold turns on', async ({
    page,
  }) => {
    await openRepo(page, { uiSettings: FLAT });
    const wip = await wipOffset(page);
    const fullHeight = await graphContentHeight(page);

    // Select a mid-run commit (model row 15 = 'chore: history 15').
    await clickGraphRow(page, wip + 15);
    const details = page.getByTestId('commit-details');
    await expect(details.getByText('chore: history 15').first()).toBeVisible();

    await toggleFold(page, 'fab');
    await expect(chip(page, 'Folded')).toBeVisible();
    // The only span contains the selection → it boots expanded: the extent
    // stays FULL (poll through the toggle's async re-request settling).
    await expect.poll(() => graphContentHeight(page)).toBe(fullHeight);
    await expect(details.getByText('chore: history 15').first()).toBeVisible();
  });

  test('fold composes with first-parent (AC5 chip grammar + span intact)', async ({
    page,
  }) => {
    await openRepo(page, { uiSettings: FLAT });
    const fullHeight = await graphContentHeight(page);

    await filterFab(page).click();
    const popover = page.getByRole('dialog', { name: 'Graph filters' });
    await popover.getByLabel('First-parent only').click();
    await popover.getByLabel('Fold linear runs').click();
    await page.keyboard.press('Escape');

    // §4.2 grammar: First-parent before Folded. First-parent alone drops no
    // rows in this fixture (every side line keeps a ref tip — see e2e/28), so
    // the folded extent is the same 19-row shrink.
    await expect(chip(page, 'First-parent · Folded')).toBeVisible();
    await expect
      .poll(() => graphContentHeight(page))
      .toBe(fullHeight - (SPAN_HIDDEN - 1) * DEFAULT_ROW_HEIGHT);
  });

  test('AC7: fold toggle persists across reload; expansion state does not', async ({
    page,
  }) => {
    await openRepo(page, { uiSettings: FLAT });
    const wip = await wipOffset(page);
    const fullHeight = await graphContentHeight(page);
    const foldedHeight = fullHeight - (SPAN_HIDDEN - 1) * DEFAULT_ROW_HEIGHT;

    await toggleFold(page, 'fab');
    await expect(chip(page, 'Folded')).toBeVisible();
    await expect.poll(() => graphContentHeight(page)).toBe(foldedHeight);
    // Expand the run so the reload can prove expansion is transient.
    await clickGraphRow(page, wip + SPAN_START);
    await expect.poll(() => graphContentHeight(page)).toBe(fullHeight);

    // The settings write is queued (~300 ms coalesce + mock write delay) —
    // poll storage before reloading, same discipline as skipOnboarding().
    await page.waitForFunction(
      () => {
        const raw = window.localStorage.getItem('bonsai.mockUiSettings');
        if (raw === null) return false;
        try {
          return (JSON.parse(raw) as { graphFoldLinear?: unknown }).graphFoldLinear === true;
        } catch {
          return false;
        }
      },
      undefined,
      { polling: 50, timeout: 10_000 },
    );
    await page.reload();

    // Fold survives; the run returns collapsed (per-session expansion only).
    await expect(chip(page, 'Folded')).toBeVisible();
    await expect.poll(() => graphContentHeight(page)).toBe(foldedHeight);
  });
});
