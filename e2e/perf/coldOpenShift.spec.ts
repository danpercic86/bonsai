/**
 * P120 — guard for cold-open's `layoutShiftsUnprompted: 0` ceiling.
 *
 * Cold-open's whole boot runs under the COUNT harness's faked
 * `performance.now()` (realNow=false), while the shift classifier compares the
 * phase marks stamped by that clock against the browser's real
 * `entry.startTime` — the same drift perfFixtures.ts documents for the
 * detector. A structurally blind ceiling would read 0 whatever happened, so
 * this proves the measurement itself (`runColdOpen`, unchanged) still counts
 * a post-interactive shift injected via `?perfInjectShift=sidebar`. If it ever
 * reads 0, the ceiling is meaningless: make cold-open's
 * `layoutShiftsUnprompted` always-nondeterministic and drop its ceiling.
 *
 * Kept out of interactions.spec.ts so the gate's `test:perf:counts` step does
 * not pay for it; `pnpm test:perf` runs it (~4 s per run).
 */
import { expect, test } from './perfFixtures';
import { runColdOpen } from './interactions';

const RUNS = 3;

test.describe('perf cold-open shift guard', { tag: '@perf' }, () => {
  test('cold-open counts an injected post-interactive shift', async ({ freshPage }) => {
    test.setTimeout(90_000);
    const seen: number[] = [];
    for (let i = 0; i < RUNS; i++) {
      const page = await freshPage();
      seen.push((await runColdOpen(page, { perfInjectShift: 'sidebar' })).layoutShiftsUnprompted);
      await page.context().close();
    }
    expect(seen.every((n) => n >= 1), `layoutShiftsUnprompted per run: ${JSON.stringify(seen)}`).toBe(true);
  });
});
