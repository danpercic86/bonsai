/**
 * P120 perf fixtures: the shared console-clean `test` plus a `freshPage`
 * factory. Each measured run needs a FRESH CONTEXT, not just a fresh page —
 * `gotoHarness` seeds localStorage only when a key is absent, so a previous
 * run's persisted session/settings writes would otherwise leak into the next
 * run and change its counts. The console guard is re-attached to every page
 * the factory hands out, mirroring `../fixtures.ts`.
 */
import type { BrowserContext, Page } from '@playwright/test';
import { expect, test as base } from '../fixtures';

/** CSS transitions/animations recalc style once per FRAME they run, so their
 *  count tracks wall-clock frame timing, not the app's work. The lab freezes
 *  them (the counts measure the work, not the tween). */
const FREEZE_MOTION = `document.addEventListener('DOMContentLoaded', () => {
  const s = document.createElement('style');
  s.dataset.perfFreeze = '1';
  s.textContent = '*, *::before, *::after { transition: none !important; animation: none !important; }';
  document.head.appendChild(s);
});`;

/** The page clock fakes `performance` too. That is what the COUNT runs want
 *  (React's scheduler then sees no time pass inside a paused window, so its
 *  time-slicing cannot vary run to run — with a real `now()` stage-unstage
 *  and switch-branch drifted by one commit/shift across runs). The DETECTOR
 *  proof wants the opposite: it compares `performance.now()` (phase
 *  transitions, input windows) against browser timestamps that are never faked
 *  — `layout-shift` `entry.startTime`, `event.timeStamp` — and on a paused clock
 *  the two drift apart by seconds (observed: 1/20 injected shifts labelled
 *  `boot`). `realNow: true` keeps timers fake but `now()` real: stash the real
 *  one before the clock's init script, restore it after. */
const STASH_REAL_NOW = `{ const real = performance; window.__perfRealNow = () => real.now(); }`;
const RESTORE_REAL_NOW = `if (typeof window.__perfRealNow === 'function') performance.now = window.__perfRealNow;`;

interface PerfFixtures {
  realNow: boolean;
  freshPage: () => Promise<Page>;
  channel: string;
}

export const test = base.extend<PerfFixtures>({
  realNow: [false, { option: true }],
  freshPage: async ({ browser, baseURL, allowConsoleErrors, realNow }, use) => {
    const contexts: BrowserContext[] = [];
    const errors: string[] = [];
    const allowed = (text: string): boolean => allowConsoleErrors.some((a) => text.includes(a));
    await use(async () => {
      const ctx = await browser.newContext({ baseURL });
      contexts.push(ctx);
      await ctx.addInitScript(FREEZE_MOTION);
      if (realNow) await ctx.addInitScript(STASH_REAL_NOW);
      const page = await ctx.newPage();
      // Flowing fake clock: boot runs at real speed; `measure` pauses it.
      await page.clock.install();
      if (realNow) await page.addInitScript(RESTORE_REAL_NOW);
      page.on('console', (msg) => {
        if (msg.type() === 'error' && !allowed(msg.text())) errors.push(msg.text());
      });
      page.on('pageerror', (err) => {
        if (!allowed(err.message)) errors.push(err.message);
      });
      return page;
    });
    for (const ctx of contexts) await ctx.close();
    expect(errors, `console/page errors:\n${errors.join('\n')}`).toEqual([]);
  },
  // The baseline key: the browser channel (msedge locally on Windows), else
  // the browser name (bundled chromium on CI / macOS / Linux).
  channel: async ({ browserName }, use, testInfo) => {
    await use(testInfo.project.use.channel ?? browserName);
  },
});

export { expect };
