/** P111 — long ref names in pill-shaped chips.
 *
 *  The defect: a `border-radius: 999px` chip holding a long ref wrapped to two
 *  lines, and the radius then exceeded half the height, so it rendered as a
 *  lozenge rather than a pill.
 *
 *  These assertions are geometric on purpose. The visual bug is a *shape*, and a
 *  DOM-text assertion cannot see it — the text is identical whether the chip is
 *  one line or two. Height is the thing that actually regressed.
 */
import { expect, test } from './fixtures';
import { openRepo, rightClickGraphRow, waitForGraphSettled } from './helpers';

/** From `src/ipc/fixtures/branchNames.ts` — 86 chars, four segments. */
const LONG = 'feature/observability/rewrite-the-per-commit-blame-why-layer-behind-a-cached-lane-index-v2';
/** Same fixture — long, and with NO `/` at all, so the whole string is the leaf. */
const LONG_NO_SLASH = 'rewrite-the-cached-lane-index-behind-a-flag-without-a-slash-anywhere';

interface ChipGeometry {
  aria: string | null;
  chipHeight: number;
  headClientWidth: number | null;
  headScrollWidth: number | null;
  leafOverflowPx: number;
}

async function openSuggestions(page: Parameters<typeof waitForGraphSettled>[0], theme: 'dark' | 'light') {
  await openRepo(page, { uiSettings: { onboardingSeen: true, aiConsented: true, theme } });
  await waitForGraphSettled(page);
  const menu = await rightClickGraphRow(page, 2);
  await menu.getByRole('menuitem', { name: 'Create branch here' }).click();
  await page.getByRole('button', { name: 'Suggest name' }).click();
  await expect(page.getByRole('button', { name: LONG })).toBeVisible({ timeout: 10_000 });
}

function readChips(page: Parameters<typeof waitForGraphSettled>[0]): Promise<ChipGeometry[]> {
  return page.evaluate(() =>
    [...document.querySelectorAll('.branch-name-chip')].flatMap((chip) => {
      const wrap = chip.querySelector('.ref-label');
      const leaf = chip.querySelector('.ref-label-leaf');
      if (wrap === null || leaf === null) return [];
      const head = chip.querySelector('.ref-label-head');
      const wrapRect = wrap.getBoundingClientRect();
      return [
        {
          aria: chip.getAttribute('aria-label'),
          chipHeight: chip.getBoundingClientRect().height,
          headClientWidth: head === null ? null : head.clientWidth,
          headScrollWidth: head === null ? null : head.scrollWidth,
          leafOverflowPx: leaf.getBoundingClientRect().right - wrapRect.right,
        },
      ];
    }),
  );
}

for (const theme of ['dark', 'light'] as const) {
  test(`33 pill truncation @smoke — a long ref stays one line in ${theme}`, async ({ page }) => {
    await openSuggestions(page, theme);
    const chips = await readChips(page);
    expect(chips.length).toBeGreaterThanOrEqual(4);

    for (const chip of chips) {
      // AC1 — one line. The bug produced a ~50px two-line stadium; the pill is 24px.
      // Anything over 32 is a second line, whatever the exact metrics.
      expect(chip.chipHeight, `chip "${chip.aria}" must be one line`).toBeLessThan(32);

      // AC2 — nothing is HARD-clipped. The wrapper sets `overflow: hidden`, so a
      // leaf wider than the wrapper would be cut with no ellipsis: a truncated
      // ref that LOOKS complete. Measured at 174px before the leaf was allowed
      // to ellipsize as a last resort.
      expect(chip.leafOverflowPx, `leaf of "${chip.aria}" must not overflow`).toBeLessThanOrEqual(1);
    }
  });
}

test('33 pill truncation @smoke — the accessible name is the WHOLE ref, never the truncated text', async ({
  page,
}) => {
  await openSuggestions(page, 'dark');

  // The load-bearing claim of the contract: truncation is CSS-only, so the DOM
  // and the accessibility tree always hold the complete string. A screen reader
  // that receives a truncated branch name receives a *wrong* branch name.
  for (const name of [LONG, LONG_NO_SLASH]) {
    await expect(page.getByRole('button', { name })).toHaveAccessibleName(name);
  }
});

test('33 pill truncation @smoke — the leading segments are consumed before the leaf', async ({
  page,
}) => {
  await openSuggestions(page, 'dark');
  const chips = await readChips(page);

  const long = chips.find((c) => c.aria === LONG);
  expect(long, 'the long multi-segment chip must be present').toBeDefined();
  // Its head is fully consumed (client 0 against a real scroll width) — the ref
  // is identified by its TAIL, so that is the half that must survive.
  expect(long?.headClientWidth).toBe(0);
  expect(long?.headScrollWidth ?? 0).toBeGreaterThan(0);

  // A chip that FITS must not be truncated at all, or the rule would just be
  // "always ellipsize" and the priority would be untested.
  const short = chips.find((c) => c.aria === 'feat/ai-why-layer');
  expect(short, 'the short chip must be present').toBeDefined();
  expect(short?.headClientWidth).toBe(short?.headScrollWidth);
});
