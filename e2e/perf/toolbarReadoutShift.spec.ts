/**
 * P122b — the toolbar's "Fetched Xm ago" readout changing width moves nothing.
 *
 * The toolbar's remote-op group is centred, so before P122b every width change
 * of the readout ("2m" -> "<1m" at an auto-fetch, "<1m" -> "1m" on the next
 * clock tick) moved every neighbour — two unprompted shifts per auto-fetch in
 * the field. This drives the REAL flow on the mock harness: auto-fetch enabled
 * (the mock reads `intervalMinutes` as seconds), the page clock stepped past a
 * synthetic auto-fetch and then past the shared 30 s `useNow` ticks, with real
 * frames between steps so the browser actually paints each label. It asserts
 * the label really went through those widths AND that no `layout-shift` entry
 * had a source inside `[data-obs-region="toolbar"]`.
 *
 * A focused DOM test cannot prove this (jsdom has no layout), and it is not a
 * counts interaction: the auto-fetch round also refreshes the graph/sidebar, so
 * a whole-page shift count would measure more than the toolbar. Kept out of
 * interactions.spec.ts, so `test:perf:counts` does not pay for it;
 * `pnpm test:perf` runs it.
 */
import { openRepo } from '../helpers';
import { expect, test } from './perfFixtures';

/** Mock auto-fetch period: 60 "minutes" = 60 s of page time. */
const INTERVAL_S = 60;
const CHUNK_MS = 2_000;
const REAL_FRAME_WAIT_MS = 40;

interface ToolbarShift {
  value: number;
  sources: string[];
}

const OBSERVE = `
window.__toolbarShifts = [];
new PerformanceObserver((list) => {
  for (const e of list.getEntries()) {
    const bar = document.querySelector('[data-obs-region="toolbar"]');
    const hits = (e.sources || []).filter((s) => bar !== null && s.node && bar.contains(s.node));
    if (hits.length > 0) {
      window.__toolbarShifts.push({
        value: e.value,
        sources: hits.map((s) => (s.node.outerHTML || s.node.textContent || '').slice(0, 120)),
      });
    }
  }
}).observe({ type: 'layout-shift' });
`;

test.describe('perf toolbar readout shift', { tag: '@perf' }, () => {
  test('the auto-fetch readout changing width shifts nothing in the toolbar', async ({ freshPage }) => {
    test.setTimeout(90_000);
    const page = await freshPage();
    await openRepo(page, { uiSettings: { onboardingSeen: true, autoFetch: { enabled: true, intervalMinutes: INTERVAL_S } } });
    const readout = page.locator('[data-obs-region="toolbar"] .toolbar-fetch-age');
    await expect(readout).toHaveText('Fetched 2m ago');
    await page.evaluate(OBSERVE);
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 500));

    const seen = new Set<string>();
    // Past one auto-fetch (<= 60 s away) and two 30 s clock ticks after it.
    for (let t = 0; t < (INTERVAL_S + 70) * 1000; t += CHUNK_MS) {
      await page.clock.runFor(CHUNK_MS);
      await page.waitForTimeout(REAL_FRAME_WAIT_MS); // a real frame paints the label
      seen.add((await readout.textContent()) ?? '');
    }

    // The label genuinely changed width (else the assertion below is vacuous).
    expect([...seen]).toEqual(expect.arrayContaining(['Fetched 2m ago', 'Fetched <1m ago', 'Fetched 1m ago']));
    const shifts = await page.evaluate(
      () => (window as unknown as { __toolbarShifts: ToolbarShift[] }).__toolbarShifts,
    );
    expect(shifts, JSON.stringify(shifts)).toEqual([]);
  });
});
