/**
 * P120 part a — detector determinism proofs (spec 008 AC 2 / AC 3).
 *
 *   AC 2: with `?perfInjectShift=<region>` every run detects the post-interactive
 *         shift (20/20); without it no run does (20/20).
 *   AC 3: with `?perfInjectFocusSteal=1` every run detects the focus loss
 *         (20/20); without it no run does (20/20).
 *
 * Each run boots on a PAUSED page clock (like `cold-open`), so the boot's
 * timers, the inject's `setTimeout` and the focus-settle timer all fire on the
 * same stepped timeline every run — the detector is proven deterministic, not
 * just usually right. Coarse steps: only ">= 1 vs 0" is asserted here. Unlike
 * the count runs, `performance.now()` stays REAL (`realNow`), because the
 * detector compares it against the browser's own shift/input timestamps.
 */
import type { Page } from '@playwright/test';
import { FIXTURE_REPO, gotoHarness } from '../helpers';
import { SMALL_EXTENT, appSettled, present } from './interactions';
import { expect, test } from './perfFixtures';
import { snapshotPerf, stepFor, stepUntil } from './perfHelpers';

const REPEAT = 20;
const STEP = 50;
/** Past the inject point (interactive + 750 ms) with margin. */
const AFTER_INJECT_MS = 1_500;

type Counter = 'layoutShiftsUnprompted' | 'focusLosses';

interface Case {
  name: string;
  flags: Record<string, string>;
  counter: Counter;
  /** In-page marker the inject leaves behind, if it persists (the shift
   *  spacer does; the focus-steal button is removed by design). */
  marker?: string;
}

const CASES: readonly Case[] = [
  { name: 'shift', flags: { perfInjectShift: 'sidebar' }, counter: 'layoutShiftsUnprompted', marker: '[data-perf-inject="shift"]' },
  { name: 'focus-steal', flags: { perfInjectFocusSteal: '1' }, counter: 'focusLosses' },
];

async function bootPaused(page: Page, flags: Record<string, string>): Promise<void> {
  await page.clock.pauseAt(Date.now() + 1000);
  await gotoHarness(page, { flags, session: { openRepos: [FIXTURE_REPO] } });
  await stepUntil(page, appSettled, { min: SMALL_EXTENT, status: true }, 30_000, STEP);
}

async function count(page: Page, counter: Counter): Promise<number> {
  return (await snapshotPerf(page))[counter];
}

test.describe('perf detector', { tag: '@perf' }, () => {
  // Real `performance.now()` under the fake timers — see perfFixtures.ts.
  test.use({ realNow: true });

  for (const c of CASES) {
    for (let i = 1; i <= REPEAT; i++) {
      test(`${c.name} injected is detected #${i}`, async ({ freshPage }) => {
        const page = await freshPage();
        await bootPaused(page, c.flags);
        if (c.marker !== undefined) {
          await stepUntil(page, present, { sel: c.marker, want: true }, 5_000, STEP);
        }
        await stepFor(page, AFTER_INJECT_MS, STEP);
        // PerformanceObserver / focus classification deliver asynchronously on
        // REAL time after the stepped timers ran: poll without advancing.
        await expect.poll(() => count(page, c.counter), { timeout: 5_000 }).toBeGreaterThanOrEqual(1);
      });
    }
  }

  // One clean boot proves BOTH negatives (AC 2 and AC 3): same boot, no flag.
  for (let i = 1; i <= REPEAT; i++) {
    test(`nothing injected: no shift, no focus loss #${i}`, async ({ freshPage }) => {
      const page = await freshPage();
      await bootPaused(page, {});
      await stepFor(page, AFTER_INJECT_MS, STEP);
      // Real-time grace for any observer delivery (page timers are paused).
      await page.waitForTimeout(250);
      const snap = await snapshotPerf(page);
      expect([snap.layoutShiftsUnprompted, snap.focusLosses]).toEqual([0, 0]);
    });
  }
});
