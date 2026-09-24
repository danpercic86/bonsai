/**
 * P120 part a (spec 008) — lab-count helpers for the `perf` Playwright project.
 *
 * Determinism rule for every measured window (reset -> act -> settle -> snapshot):
 * NO locator work inside it. Playwright's actionability checks, `toBeVisible`,
 * role-name resolution and `boundingBox()` all force style/layout, and their
 * poll counts vary run to run, so they would pollute `RecalcStyleCount` /
 * `LayoutCount` with harness noise. Inside the window, act with raw input
 * (`page.mouse`, `page.keyboard`) or an in-page call, and settle on predicates
 * that read only DOM attributes or the `__bonsai.perf` counters.
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Page } from '@playwright/test';
import type { PerfSnapshot } from '../../src/obs/stability/perfCounters';

/** The ratcheted metrics, in the order they are written. */
export const METRICS = [
  'reactCommits',
  'ipcCalls',
  'recalcStyle',
  'layout',
  'layoutShiftsUnprompted',
  'focusLosses',
] as const;
export type Metric = (typeof METRICS)[number];
export type MetricValue = number | 'nondeterministic';
export type InteractionCounts = Record<Metric, MetricValue>;
export type RunCounts = Record<Metric, number>;

/** The perf project's outputDir (gitignored); Playwright wipes it at the
 *  start of a perf run. Keep in sync with scripts/check-perf-baseline.mjs. */
export const COUNTS_PATH = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'test-results',
  'perf',
  'perf-counts.json',
);

/** A settled window: counters unchanged for this long (> the 300 ms watcher
 *  debounce and the 30 ms stream batch spacing, with margin). */
const QUIET_MS = 800;
const POLL_MS = 50;

interface PerfWindow {
  __bonsai?: { perf?: { reset(): void; snapshot(): PerfSnapshot } };
}

/** Wait for the mock dev hooks (installed when the graph canvas mounts). */
export async function waitForPerfHook(page: Page): Promise<void> {
  await page.waitForFunction(
    () => (window as unknown as PerfWindow).__bonsai?.perf !== undefined,
    undefined,
    { polling: POLL_MS },
  );
}

export async function resetPerf(page: Page): Promise<void> {
  await waitForPerfHook(page);
  await page.evaluate(() => (window as unknown as PerfWindow).__bonsai?.perf?.reset());
}

export async function snapshotPerf(page: Page): Promise<PerfSnapshot> {
  await waitForPerfHook(page);
  const snap = await page.evaluate(() => (window as unknown as PerfWindow).__bonsai?.perf?.snapshot() ?? null);
  if (snap === null) throw new Error('window.__bonsai.perf vanished mid-measurement');
  return snap;
}

/** Wait until the React-commit and IPC counters stop moving for QUIET_MS.
 *  Reads only the counters (never layout), so it adds nothing to the metrics. */
export async function quiesce(page: Page, timeoutMs = 20_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let last = '';
  let stableSince = Date.now();
  for (;;) {
    const s = await snapshotPerf(page);
    const key = `${s.reactCommits}/${s.ipcCalls}`;
    const now = Date.now();
    if (key !== last) {
      last = key;
      stableSince = now;
    } else if (now - stableSince >= QUIET_MS) {
      return;
    }
    if (now > deadline) throw new Error(`perf counters never went quiet (last ${key})`);
    await new Promise((r) => {
      setTimeout(r, POLL_MS);
    });
  }
}

/** Run `fn` between two CDP `Performance.getMetrics` reads and return the
 *  `RecalcStyleCount` / `LayoutCount` deltas. */
export async function withCdpMetrics(
  page: Page,
  fn: () => Promise<void>,
): Promise<{ recalcStyle: number; layout: number }> {
  const cdp = await page.context().newCDPSession(page);
  try {
    await cdp.send('Performance.enable');
    const read = async (): Promise<Map<string, number>> => {
      const { metrics } = await cdp.send('Performance.getMetrics');
      return new Map(metrics.map((m) => [m.name, m.value]));
    };
    const before = await read();
    await fn();
    const after = await read();
    const delta = (name: string): number => (after.get(name) ?? 0) - (before.get(name) ?? 0);
    return { recalcStyle: delta('RecalcStyleCount'), layout: delta('LayoutCount') };
  } finally {
    await cdp.detach();
  }
}

/** Fake-clock slice: one 60 Hz frame, so each slice runs exactly one faked
 *  rAF. Timers due within one slice fire in one task (so they batch the same
 *  way every run); timers due in different slices stay apart, so an
 *  unbatched-update regression still shows up as extra commits. */
export const STEP_MS = 16;

/** Slices advanced between two in-page checks. Checking is what costs real
 *  time (a round trip each), and real time matters: the browser's own
 *  `hadRecentInput` on layout-shift entries is a REAL 500 ms window, so a
 *  stepped window stretched past it would relabel input-driven shifts as
 *  unprompted. Checking every few slices keeps a 150 ms mock IPC well inside. */
const SLICES_PER_CHECK = 2;

async function advance(page: Page, slices: number, stepMs: number): Promise<void> {
  for (let i = 0; i < slices; i++) await page.clock.runFor(stepMs);
}

/** In-window settle on a PAUSED clock: advance fake time slice by slice until
 *  the in-page predicate holds. Nothing moves between slices except real
 *  (non-timer) work, so the timeline is the same every run. */
export async function stepUntil<A>(
  page: Page,
  pred: (arg: A) => boolean,
  arg: A,
  maxMs = 30_000,
  stepMs = STEP_MS,
): Promise<void> {
  for (let t = 0; t <= maxMs; t += stepMs * SLICES_PER_CHECK) {
    // Widened for Playwright's `Unboxed<Arg>` mapping (identity for the plain
    // JSON args used here).
    if (await page.evaluate(pred as (a: unknown) => boolean, arg as unknown)) return;
    await advance(page, SLICES_PER_CHECK, stepMs);
  }
  throw new Error(`predicate still false after ${maxMs} ms of fake time`);
}

/** Advance a PAUSED clock by `ms` in `stepMs` slices (real work between). */
export async function stepFor(page: Page, ms: number, stepMs = STEP_MS): Promise<void> {
  await advance(page, Math.ceil(ms / stepMs), stepMs);
}

/** In-window quiesce on a PAUSED clock: step until the counters have not
 *  moved for QUIET_MS of fake time. */
export async function stepQuiet(page: Page, maxMs = 30_000): Promise<void> {
  const chunk = STEP_MS * SLICES_PER_CHECK;
  let last = '';
  let quietFor = 0;
  for (let t = 0; t <= maxMs; t += chunk) {
    const key = await page.evaluate(() => {
      const s = (window as unknown as PerfWindow).__bonsai?.perf?.snapshot();
      return s === undefined ? 'none' : `${s.reactCommits}/${s.ipcCalls}`;
    });
    quietFor = key === last ? quietFor + chunk : 0;
    last = key;
    if (quietFor >= QUIET_MS) return;
    await advance(page, SLICES_PER_CHECK, STEP_MS);
  }
  throw new Error(`perf counters never went quiet in fake time (last ${last})`);
}

/** One measured run. Settles on the real clock first, then PAUSES the page
 *  clock (installed by the perf `freshPage` fixture) for reset -> act -> settle
 *  -> snapshot, so timer interleaving — which decides whether two async
 *  results share a React commit — is identical every run. `act`/`settle` must
 *  advance time only through `stepUntil` / `stepQuiet`. */
export async function measure(
  page: Page,
  act: () => Promise<void>,
  settle: () => Promise<void>,
): Promise<RunCounts> {
  await quiesce(page);
  // A little ahead of "now" so the call can never land in the past; the app
  // is quiet, so the jump fires nothing.
  await page.clock.pauseAt(await page.evaluate(() => Date.now() + 500));
  try {
    await resetPerf(page);
    const cdp = await withCdpMetrics(page, async () => {
      await act();
      await settle();
      await stepQuiet(page);
    });
    const s = await snapshotPerf(page);
    return {
      reactCommits: s.reactCommits,
      ipcCalls: s.ipcCalls,
      recalcStyle: cdp.recalcStyle,
      layout: cdp.layout,
      layoutShiftsUnprompted: s.layoutShiftsUnprompted,
      focusLosses: s.focusLosses,
    };
  } finally {
    await page.clock.resume();
  }
}

/** Collapse N runs: a metric every run agrees on keeps its value; any
 *  disagreement becomes `"nondeterministic"` (dropped, never loosened). */
export function agree(runs: readonly RunCounts[], always: readonly Metric[] = []): InteractionCounts {
  const first = runs[0];
  if (first === undefined) throw new Error('agree() needs at least one run');
  const out = {} as InteractionCounts;
  for (const m of METRICS) {
    out[m] = !always.includes(m) && runs.every((r) => r[m] === first[m]) ? first[m] : 'nondeterministic';
  }
  return out;
}

interface CountsFile {
  _comment: string;
  interactions: Record<string, Record<string, InteractionCounts>>;
}

/** Merge this channel's results into `test-results/perf/perf-counts.json`
 *  (`{ interactions: { [channel]: { [interaction]: counts } } }`). */
export function writeCounts(channel: string, results: Record<string, InteractionCounts>): void {
  let file: CountsFile = {
    _comment: 'P120 lab counts (pnpm test:perf). Compared by pnpm lint:perf against scripts/perf-baseline.json.',
    interactions: {},
  };
  try {
    file = JSON.parse(readFileSync(COUNTS_PATH, 'utf8')) as CountsFile;
  } catch {
    // First write of this run.
  }
  file.interactions[channel] = { ...file.interactions[channel], ...results };
  mkdirSync(dirname(COUNTS_PATH), { recursive: true });
  writeFileSync(COUNTS_PATH, `${JSON.stringify(file, null, 2)}\n`);
}
