/**
 * P120 part a — the scripted interactions (spec 008, resolved decision 2;
 * the eighth, `background-refresh-diff-open`, is P124c).
 *
 * Each `run` does its boot and every locator lookup (plus any pointer move)
 * OUTSIDE the measured window, then hands `measure` an in-window `act` +
 * `settle` that use only raw input and DOM/counter predicates, advancing the
 * paused page clock via `stepUntil` / `stepQuiet` — see perfHelpers.ts.
 */
import type { Page } from '@playwright/test';
import {
  DEFAULT_ROW_HEIGHT,
  FIXTURE_REPO,
  MOCK_MODEL_ROWS,
  gotoHarness,
  openBranchContextMenu,
  openRepo,
} from '../helpers';
import { measure, snapshotPerf, stepQuiet, stepUntil, withCdpMetrics } from './perfHelpers';
import type { Metric, RunCounts } from './perfHelpers';

export interface Interaction {
  name: string;
  /** Metrics written as "nondeterministic" regardless of run agreement: ones
   *  observed to disagree often enough that 3-run agreement is itself a coin
   *  flip (which would make two consecutive counts files differ, AC 4). */
  alwaysNondeterministic?: readonly Metric[];
  /** One measured run on a fresh page. */
  run(page: Page): Promise<RunCounts>;
}

const SETTLE_TIMEOUT = 30_000;
export const SMALL_EXTENT = MOCK_MODEL_ROWS * DEFAULT_ROW_HEIGHT + 8;
const ROWS_20K = 20_000;
const EXTENT_20K = ROWS_20K * DEFAULT_ROW_HEIGHT * 0.9;
const FLAT = { onboardingSeen: true, listView: 'flat' };

interface SettledArg {
  min: number;
  status: boolean;
}

/** In-page predicate: Refresh enabled (graph stream + status round done), the
 *  graph spacer at its extent and — for fixtures with changes — the status
 *  panel's section headers rendered. Reads attributes/text/inline style only. */
export function appSettled({ min, status }: SettledArg): boolean {
  if (status) {
    const panel = document.querySelector('[data-testid="status-panel"]');
    if (panel === null || !/(Staged|Conflicts) \(/.test(panel.textContent ?? '')) return false;
  }
  const refresh = document.querySelector<HTMLButtonElement>('button[aria-label="Refresh"]');
  if (refresh === null || refresh.disabled) return false;
  const spacer = document.querySelector<HTMLElement>('[data-testid="graph-scroller"] .graph-spacer');
  return spacer !== null && parseFloat(spacer.style.height) >= min;
}

export function present({ sel, want }: { sel: string; want: boolean }): boolean {
  return (document.querySelector(sel) !== null) === want;
}

/** Real-clock (outside the window) settle. */
async function waitAppSettled(page: Page, min: number, status = true): Promise<void> {
  await page.waitForFunction(appSettled, { min, status }, { polling: 50, timeout: SETTLE_TIMEOUT });
}

/** Press + release at the CURRENT mouse position. The move happens in
 *  `prepare` (before the window): Playwright's `mouse.click` sends move, down
 *  and up as separate CDP messages, and whether React's scheduler flushes the
 *  hover update between them is a race — one React commit more or less. */
async function pressHere(page: Page): Promise<void> {
  await page.mouse.down();
  await page.mouse.up();
}

/** Programmatic focus (no layout, no input event), then a real key press. */
async function focusAndPress(page: Page, selector: string, key: string): Promise<void> {
  await page.evaluate((sel) => {
    const el = document.querySelector<HTMLElement>(sel);
    if (el === null) throw new Error(`no element for ${sel}`);
    el.focus();
  }, selector);
  await page.keyboard.press(key);
}

async function openSettled(page: Page, uiSettings?: Record<string, unknown>): Promise<void> {
  await openRepo(page, uiSettings ? { uiSettings } : undefined);
  await waitAppSettled(page, SMALL_EXTENT);
}

const coldOpen: Interaction = {
  name: 'cold-open',
  // Even post-load and on the paused clock, 1-2 runs in 6 recalc/lay out once
  // more: during boot, whether two commits share a REAL rendering frame
  // depends on round-trip timing. React commits and IPC calls stay exact.
  alwaysNondeterministic: ['recalcStyle', 'layout'],
  // The whole boot IS the interaction: counters start at zero on page load, so
  // there is no reset. The clock is paused BEFORE navigation, so every mock
  // `delay()` / stream batch / rAF of the boot fires on the stepped timeline.
  run: (page) => runColdOpen(page),
};

/** The cold-open measurement; `flags` are harness query flags (the shift
 *  guard in coldOpenShift.spec.ts passes `perfInjectShift`). */
export async function runColdOpen(page: Page, flags: Record<string, string> = {}): Promise<RunCounts> {
  await page.clock.pauseAt(Date.now() + 1000);
  try {
    await gotoHarness(page, { flags, session: { openRepos: [FIXTURE_REPO] } });
    // CDP brackets load -> settled only: before `load`, the dev server's
    // per-module CSS injection recalcs style once per arriving stylesheet,
    // which tracks network timing, not the app.
    const cdp = await withCdpMetrics(page, async () => {
      await stepUntil(page, appSettled, { min: SMALL_EXTENT, status: true });
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

const selectCommit: Interaction = {
  name: 'select-commit',
  async run(page) {
    await openSettled(page);
    // Display row 3: a plain commit below the WIP row, inside the first screen.
    const box = await page.getByTestId('graph-scroller').boundingBox();
    if (box === null) throw new Error('graph scroller has no bounding box');
    const x = box.x + box.width / 2;
    const y = box.y + 3 * DEFAULT_ROW_HEIGHT + DEFAULT_ROW_HEIGHT / 2;
    await page.mouse.move(x, y);
    return measure(
      page,
      () => pressHere(page),
      () => stepUntil(page, present, { sel: '[data-testid="commit-details"]', want: true }),
    );
  },
};

const stageUnstage: Interaction = {
  name: 'stage-unstage',
  // P124 (user ruling 2026-09-25): recalcStyle/layout vary in about half of the
  // runs (10/8 vs 9/6..8/6), just as much on the pre-P121 tree (5 of 12 at
  // 618c7f0) as at HEAD, so this is harness timing, not a regression. Whether
  // the two keyboard-driven section swaps share a real rendering frame depends
  // on timing. React commits and IPC calls stay exact and stay ratcheted.
  alwaysNondeterministic: ['recalcStyle', 'layout'],
  async run(page) {
    await openSettled(page);
    const stage = '[aria-label="Stage README.md"]';
    const unstage = '[aria-label="Unstage README.md"]';
    // Keyboard, not mouse: the Unstage button is a NEW element in another
    // section, and moving the pointer onto it mid-window would add a hover race.
    return measure(
      page,
      async () => {
        await focusAndPress(page, stage, 'Enter');
        await stepUntil(page, present, { sel: unstage, want: true });
        await stepQuiet(page);
        await focusAndPress(page, unstage, 'Enter');
      },
      () => stepUntil(page, present, { sel: stage, want: true }),
    );
  },
};

const switchBranch: Interaction = {
  name: 'switch-branch',
  // Shift classification uses the browser's own `hadRecentInput`, a REAL
  // 500 ms window after the click. Checkout chains ten mock IPC calls, and the
  // stepped clock stretches that chain across the window in ~1 run of 9, so a
  // late shift flips between prompted and unprompted.
  alwaysNondeterministic: ['layoutShiftsUnprompted'],
  async run(page) {
    await openSettled(page, FLAT);
    // feature/merged-a: the deterministic fast-forward fixture (no toast).
    const menu = await openBranchContextMenu(page, 'feature/merged-a');
    const box = await menu.getByRole('menuitem', { name: 'Checkout' }).boundingBox();
    if (box === null) throw new Error('Checkout menuitem has no bounding box');
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    return measure(
      page,
      () => pressHere(page),
      async () => {
        await stepUntil(
          page,
          (name: string) => {
            const rows = document.querySelectorAll('[role="tree"] li');
            const row = [...rows].find((li) => li.querySelector(`[title="${name}"]`) !== null);
            return row?.getAttribute('aria-current') === 'true';
          },
          'feature/merged-a',
        );
        await stepUntil(page, appSettled, { min: SMALL_EXTENT, status: true });
      },
    );
  },
};

const backgroundRefresh: Interaction = {
  name: 'background-refresh',
  async run(page) {
    await openSettled(page);
    // Programmatic focus (no pointer/key input), so a loss caused by the
    // refresh is unprompted by construction.
    await page.getByRole('button', { name: 'Stage README.md' }).focus();
    return measure(
      page,
      async () => {
        await page.evaluate(() => {
          const emit = (window as unknown as { __bonsaiEmitWatcher?: (r: string) => number }).__bonsaiEmitWatcher;
          if (emit === undefined || emit('fs') === 0) throw new Error('no mock watcher listener');
        });
      },
      async () => {
        await stepUntil(
          page,
          () => {
            const w = window as unknown as { __bonsai?: { perf?: { snapshot(): { ipcByCmd: Record<string, number> } } } };
            return (w.__bonsai?.perf?.snapshot().ipcByCmd.getStatus ?? 0) >= 1;
          },
          undefined,
        );
        await stepUntil(page, appSettled, { min: SMALL_EXTENT, status: true });
      },
    );
  },
};

/** In-page predicate: the center-pane diff overlay shows a loaded diff that is
 *  not stale (`DiffSlotView` dims a same-key refetch with `.diff-stale`). */
function diffReady(): boolean {
  return document.querySelector('.diff-overlay-body .diff-scroll:not(.diff-stale)') !== null;
}

/** P124c: `background-refresh` with the unstaged README.md diff OPEN. The
 *  worktree round refetches the open diff; an identical result must commit
 *  nothing beyond the status round itself. */
const backgroundRefreshDiffOpen: Interaction = {
  name: 'background-refresh-diff-open',
  async run(page) {
    await openSettled(page);
    // Outside the window, on the real clock (like every other pre-window step).
    await page
      .locator('li.file-row', { has: page.locator('[aria-label="Stage README.md"]') })
      .locator('button.file-row-main')
      .click();
    await page.waitForFunction(diffReady, undefined, { polling: 50, timeout: SETTLE_TIMEOUT });
    await waitAppSettled(page, SMALL_EXTENT);
    return measure(
      page,
      async () => {
        await page.evaluate(() => {
          const emit = (window as unknown as { __bonsaiEmitWatcher?: (r: string) => number }).__bonsaiEmitWatcher;
          if (emit === undefined || emit('fsWorktree') === 0) throw new Error('no mock watcher listener');
        });
      },
      async () => {
        await stepUntil(
          page,
          () => {
            const w = window as unknown as { __bonsai?: { perf?: { snapshot(): { ipcByCmd: Record<string, number> } } } };
            return (w.__bonsai?.perf?.snapshot().ipcByCmd.getWorkdirFileDiff ?? 0) >= 1;
          },
          undefined,
        );
        // The diff refetch is fire-and-forget from the status round; its mock
        // 150 ms delay lands inside `measure`'s trailing 800 ms quiet window.
        await stepUntil(page, appSettled, { min: SMALL_EXTENT, status: true });
      },
    );
  },
};

const scroll20k: Interaction = {
  name: 'scroll-20k',
  async run(page) {
    await openRepo(page, { flags: { fixture: '20k' } });
    await waitAppSettled(page, EXTENT_20K, false);
    return measure(
      page,
      async () => {
        // A fixed walk: 4 steps of 50 rows down, then one jump deep into
        // history. Each write resolves on the scroll EVENT (dispatched by the
        // real frame, after the app's own listener), then fake time advances
        // so the rAF-driven redraw runs.
        const tops = [...Array.from({ length: 4 }, (_, i) => (i + 1) * 50 * DEFAULT_ROW_HEIGHT), 15_000 * DEFAULT_ROW_HEIGHT];
        for (const top of tops) {
          await page.evaluate(
            (px) =>
              new Promise<void>((resolve, reject) => {
                const el = document.querySelector<HTMLElement>('[data-testid="graph-scroller"]');
                if (el === null) {
                  reject(new Error('no graph scroller'));
                  return;
                }
                el.addEventListener('scroll', () => resolve(), { once: true });
                el.scrollTop = px;
              }),
            top,
          );
          await stepQuiet(page);
        }
      },
      async () => {},
    );
  },
};

const palette: Interaction = {
  name: 'palette-open-close',
  async run(page) {
    await openSettled(page);
    const dialog = 'div[role="dialog"][aria-label="Command palette"]';
    return measure(
      page,
      async () => {
        await page.keyboard.press('ControlOrMeta+K');
        await stepUntil(page, present, { sel: dialog, want: true });
        await stepQuiet(page);
        await page.keyboard.press('Escape');
      },
      () => stepUntil(page, present, { sel: dialog, want: false }),
    );
  },
};

export const INTERACTIONS: readonly Interaction[] = [
  coldOpen,
  selectCommit,
  stageUnstage,
  switchBranch,
  backgroundRefresh,
  backgroundRefreshDiffOpen,
  scroll20k,
  palette,
];
