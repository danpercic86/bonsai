/**
 * T4 shared e2e flows (contract §3). Every seed goes through page.addInitScript
 * (runs on-origin BEFORE app code); seeds are written only when the key is
 * absent so the app's own persisted writes survive an in-test page.reload()
 * (e.g. the onboarding-persistence case).
 */
import type { Locator, Page } from '@playwright/test';
import { expect } from './fixtures';
import { shortcutKeys, shortcutLabel } from '../src/utils/platform';

export const FIXTURE_REPO = 'C:\\mock\\bonsai-fixture';

/** Timeout for the FIRST assertion after a page load - the one gated on the
 *  app's whole boot chain (mock session restore -> openRepo -> graph stream ->
 *  first React commit), not on any behaviour under test.
 *
 *  WHY IT IS NOT THE 5 s DEFAULT. Measured on this box (22 cores, msedge
 *  channel, dev-server mode) by instrumenting `openRepo` to record `page.goto`
 *  duration and load->canvas latency SEPARATELY, N = 414 boots over three runs:
 *
 *    idle full suite   (3.0 min, N=171)   first paint p50 949  p95 1466 max 1798
 *    loaded subset     (16 hog threads)   first paint p50 1202 p95 1667 max 1884
 *    loaded full suite (4.2x wall, N=171) first paint p50 1141 p99 1966 max 9203
 *
 *  The distribution is tight (p99 = 2.0 s) with a rare heavy tail: under CPU
 *  starvation ONE boot in 171 took 9203 ms. The 5 s default sits in the middle
 *  of that tail - which is exactly the observed ~3-failures-in-20-runs flake.
 *
 *  This is NOT cold-boot cost and NOT a stalled navigation. Both were ruled
 *  out by measurement, not assumed:
 *    - the 9203 ms boot had `goto` = 1023 ms, `loadEventEnd` = 986 ms,
 *      readyState 'complete', navType 'navigate' (so no Vite full-reload) and a
 *      slowest module transform of 116 ms. The page load was entirely healthy;
 *      all 9.2 s was spent AFTER load, inside the app's own async boot chain.
 *    - the first boot on each worker against a genuinely cold dev server
 *      (goto 565-863 ms, first paint 921-1410 ms) was no slower than the steady
 *      state, because Vite's dep cache in node_modules/.vite is already warm.
 *      There is no cold-start cliff to wait out.
 *  What stretches is the chain of mock-IPC `delay()` timers plus React commits,
 *  which is at the mercy of a starved renderer's task queue.
 *
 *  15 s = 1.6x the worst boot ever observed, under a synthetic load 2.6x
 *  heavier than the worst real gate run (12.5 min wall against a 3.0 min
 *  baseline, vs 4.2 min for the run that actually flaked), and 7.6x p99.
 *  Deliberately scoped to first-paint assertions rather than raised globally:
 *  every assertion about behaviour under test keeps the 5 s default, so a real
 *  regression still fails fast. A genuinely dead boot still fails HERE, with
 *  this name on it, well inside the 30 s test budget. */
export const FIRST_PAINT_TIMEOUT = 15_000;
export const DEFAULT_ROW_HEIGHT = 32; // GraphPrefs default (persistence.ts)

export interface HarnessOptions {
  /** URL query flags, e.g. { fixture: '20k', op: 'merge' }. */
  flags?: Record<string, string>;
  /** Partial UiSettings merged into bonsai.mockUiSettings BEFORE load.
   *  Defaults to { onboardingSeen: true } — pass {} to see onboarding. */
  uiSettings?: Record<string, unknown>;
  /** Seed bonsai.mockSession (auto-reopens repos on boot). */
  session?: { openRepos: string[]; activeRepo?: string | null };
  /** Seed bonsai.mockRecents paths (lastOpened = now). */
  recents?: string[];
}

/** addInitScript(localStorage seeds) → page.goto('/?' + flags). */
export async function gotoHarness(page: Page, opts?: HarnessOptions): Promise<void> {
  const ui = opts?.uiSettings ?? { onboardingSeen: true };
  const session = opts?.session
    ? { activeRepo: null, ...opts.session }
    : null;
  const recents = opts?.recents ?? null;
  await page.addInitScript(
    (seed: { ui: Record<string, unknown>; session: unknown; recents: string[] | null }) => {
      const setIfAbsent = (key: string, value: string): void => {
        if (window.localStorage.getItem(key) === null) {
          window.localStorage.setItem(key, value);
        }
      };
      setIfAbsent('bonsai.mockUiSettings', JSON.stringify(seed.ui));
      if (seed.session !== null) {
        setIfAbsent('bonsai.mockSession', JSON.stringify(seed.session));
      }
      if (seed.recents !== null) {
        const now = Math.floor(Date.now() / 1000);
        setIfAbsent(
          'bonsai.mockRecents',
          JSON.stringify(seed.recents.map((path) => ({ path, lastOpened: now }))),
        );
      }
    },
    { ui, session, recents },
  );
  const qs = new URLSearchParams(opts?.flags ?? {}).toString();
  await page.goto(qs === '' ? '/' : `/?${qs}`);
}

/** Clicks Skip on the Welcome dialog, then waits for `onboardingSeen: true` to
 *  have actually LANDED in the mock's persisted UiSettings blob.
 *
 *  Why the extra wait: App.tsx's `closeOnboarding` hides the dialog
 *  synchronously but only QUEUES the flag (P69b: `queueSettingsWrite` — the
 *  shared ~300 ms coalescing window in useUiSettings, then the mock handler
 *  sleeps ~150 ms before `writeUiSettings`). So the dialog being hidden proves
 *  nothing about storage — a `page.reload()` right after can beat the write and boot with
 *  onboarding unseen again (flaky only under full-suite worker contention).
 *  Polling storage is the deterministic signal; never a fixed sleep.
 *
 *  Safe for every caller: all of them reach here by clicking Skip, which is
 *  exactly what fires the write. Specs that boot with onboarding already seen
 *  don't call this helper at all, so nothing can hang waiting on a write that
 *  never happens. */
export async function skipOnboarding(page: Page): Promise<void> {
  const welcome = page.getByRole('dialog', { name: 'Welcome to Bonsai' });
  // First paint - gated on the whole boot chain (see FIRST_PAINT_TIMEOUT).
  await expect(welcome).toBeVisible({ timeout: FIRST_PAINT_TIMEOUT });
  await welcome.getByRole('button', { name: 'Skip' }).click();
  await expect(welcome).toBeHidden();
  await page.waitForFunction(
    () => {
      const raw = window.localStorage.getItem('bonsai.mockUiSettings');
      if (raw === null) return false;
      try {
        return (JSON.parse(raw) as { onboardingSeen?: unknown }).onboardingSeen === true;
      } catch {
        return false;
      }
    },
    undefined,
    // rAF-independent polling (headless panes can stall rAF) + an explicit
    // timeout so a genuinely lost write fails here with a clear cause.
    { polling: 50, timeout: 10_000 },
  );
}

/** gotoHarness with the fixture repo seeded into bonsai.mockSession →
 *  auto-reopen on boot → wait for the graph canvas. THE standard entry for
 *  most tests. Session seeding (not the EmptyState click) is used because the
 *  launch-reopen effect clobbers a tab opened by an early click — see
 *  FINDINGS [T4.1] (App.tsx setTabs(opened) races user opens). The click path
 *  itself is covered by spec 01 + smoke.spec with an explicit boot-settle. */
export async function openRepo(page: Page, opts?: HarnessOptions): Promise<void> {
  const session = opts?.session ?? { openRepos: [FIXTURE_REPO] };
  await gotoHarness(page, { ...opts, session });
  await expect(graphCanvas(page)).toBeVisible({ timeout: FIRST_PAINT_TIMEOUT });
}

/** Graph scroll container + canvas locators (data-testid, contract §4). */
export function graphScroller(page: Page): Locator {
  return page.getByTestId('graph-scroller');
}
export function graphCanvas(page: Page): Locator {
  return page.getByTestId('graph-canvas');
}

/** Live rowHeight from the persisted GraphPrefs seed (default 32). */
async function liveRowHeight(page: Page): Promise<number> {
  return page.evaluate((fallback) => {
    try {
      const raw = window.localStorage.getItem('bonsai.mockUiSettings');
      if (raw === null) return fallback;
      const parsed = JSON.parse(raw) as { graph?: { rowHeight?: unknown } };
      return typeof parsed.graph?.rowHeight === 'number' ? parsed.graph.rowHeight : fallback;
    } catch {
      return fallback;
    }
  }, DEFAULT_ROW_HEIGHT);
}

/** Click a DISPLAY row (WIP row, if present, is display row 0):
 *  y = row*rowHeight + rowHeight/2 - scroller.scrollTop, x = midwidth. */
export async function clickGraphRow(page: Page, displayRow: number): Promise<void> {
  const scroller = graphScroller(page);
  const rowHeight = await liveRowHeight(page);
  const scrollTop = await scroller.evaluate((el) => el.scrollTop);
  const box = await scroller.boundingBox();
  if (box === null) throw new Error('graph scroller has no bounding box');
  const y = displayRow * rowHeight + rowHeight / 2 - scrollTop;
  if (y < 0 || y > box.height) {
    throw new Error(`display row ${displayRow} is outside the viewport (y=${y})`);
  }
  await scroller.click({ position: { x: box.width / 2, y } });
}

/** Click a DISPLAY row, then assert `target` became visible — retrying the
 *  CLICK if it did not. Use this instead of `clickGraphRow` + a bare
 *  `toBeVisible` whenever the click follows a graph MUTATION (commit, rebase,
 *  merge): the deterministic "graph gained N rows" signal is `scrollHeight`,
 *  but scrollHeight reaching its final value does not mean the display-row map
 *  has settled — the WIP row can still appear/disappear as the same refresh
 *  round's status slice lands, which shifts every row by one and makes the
 *  click select the neighbour. Under worker contention those two land far
 *  enough apart to lose the race (P94: e2e/07-rebase.spec.ts:64 reproduced
 *  ~1-in-6 this way).
 *
 *  This weakens NOTHING: the assertion is unchanged, and only a transient
 *  row-map shift is tolerated. If the mutation genuinely did not produce the
 *  expected commit, every retry misses and the helper still fails. */
export async function clickGraphRowUntilVisible(
  page: Page,
  displayRow: number,
  target: Locator,
): Promise<void> {
  await expect(async () => {
    await clickGraphRow(page, displayRow);
    await expect(target).toBeVisible({ timeout: 1000 });
  }).toPass({ timeout: 15_000 });
}

/** Right-click a DISPLAY row (same math as clickGraphRow) → the context menu. */
export async function rightClickGraphRow(page: Page, displayRow: number): Promise<Locator> {
  const scroller = graphScroller(page);
  const rowHeight = await liveRowHeight(page);
  const scrollTop = await scroller.evaluate((el) => el.scrollTop);
  const box = await scroller.boundingBox();
  if (box === null) throw new Error('graph scroller has no bounding box');
  const y = displayRow * rowHeight + rowHeight / 2 - scrollTop;
  await scroller.click({ button: 'right', position: { x: box.width / 2, y } });
  const menu = page.getByRole('menu');
  await expect(menu).toBeVisible();
  return menu;
}

/** Total scrollable extent of the graph — (rows + wip) * rowHeight + 8.
 *  Growth by N*rowHeight is THE deterministic "graph gained N rows" signal
 *  (commit/merge/rebase assertions poll this before clicking new rows). */
export async function graphScrollHeight(page: Page): Promise<number> {
  return graphScroller(page).evaluate((el) => el.scrollHeight);
}

/** Model rows every non-20k/non-detached mock fixture boots with: 3 stash
 *  offshoots + 30 base rows (`resolveLayout` = prependCommits(buildMockGraph(),
 *  []) + withStashNodes). The `op: merge` / `op: rebase` / `remote:` flags seed
 *  operation state, never extra layout rows — so this holds for those too. */
export const MOCK_MODEL_ROWS = 33;

/** True content extent of the graph — the virtualization spacer's height,
 *  `(displayRows) * rowHeight + 8`.
 *
 *  Prefer this over `graphScrollHeight` for any exact arithmetic: a graph
 *  SHORTER than the scroller's viewport has `scrollHeight === clientHeight`
 *  (the browser clamps), so a shrink below the viewport is invisible there.
 *  The spacer is the true extent at any size. */
export async function graphContentHeight(page: Page): Promise<number> {
  return graphScroller(page)
    .locator('.graph-spacer')
    .evaluate((el) => parseFloat((el as HTMLElement).style.height));
}

/** Wait until BOTH async inputs of the graph extent have landed.
 *
 *  `openRepo` waits for NEITHER — it waits only for the canvas to be *visible*,
 *  which the pane renders before any data arrives. The extent then moves twice:
 *    1. the graph stream's `meta` chunk supplies `totalRows` (GraphCanvas.tsx:
 *       `Math.max(layout.nodes.length, totalRows ?? 0)`), and
 *    2. the first working-dir status round supplies the WIP display row —
 *       RepoWorkspace derives `wip` from `status`, so display row 0 does not
 *       exist until that round lands.
 *  A baseline sampled inside that window is exactly one row short, and every
 *  assertion derived from it inherits the error (the recurring 32 px flake:
 *  `Expected 1064, Received 1096` — 1064 = 33 rows, 1096 = 33 rows + WIP).
 *  Waiting on both signals removes it at the source; raising a timeout cannot,
 *  because the baseline was already wrong when it was captured.
 *
 *  SCOPE — this proves the extent, and (for the small fixtures) the rows with
 *  it, but it is NOT a general "stream finished" signal. The extent is
 *  `max(loadedRows, meta.total)` and `meta` is the FIRST chunk. The mock emits
 *  `meta` and the first batch back-to-back with no await between them, so for a
 *  fixture that fits one batch (MOCK_MODEL_ROWS = 33 << the 512-row first
 *  batch) React applies both in the same render and a full extent does imply
 *  the rows. A MULTI-batch fixture (20k) is different: there the extent is full
 *  while only 512 rows exist — see e2e/03's `streamComplete`.
 *
 *  Status signal: StatusPanel renders its section headers only for a non-null
 *  snapshot, and that is the SAME React commit that hands GraphCanvas its `wip`
 *  prop (e2e/04's `openWithStatus` uses the same signal). It requires a fixture
 *  with working-dir changes — every mock fixture except an intentionally clean
 *  one, which renders "No changes" and has no WIP row to wait for anyway.
 *  Either section header proves the snapshot: a paused merge/rebase fixture
 *  leads with Conflicts (e2e/07's own waitStatus used the same alternation). */
export async function waitForGraphSettled(
  page: Page,
  modelRows: number = MOCK_MODEL_ROWS,
): Promise<void> {
  await expect(
    page.getByTestId('status-panel').getByText(/(Staged|Conflicts) \(/).first(),
  ).toBeVisible();
  await expect
    .poll(() => graphContentHeight(page))
    .toBeGreaterThanOrEqual(modelRows * DEFAULT_ROW_HEIGHT + 8);
}

/** THE settled baseline: `waitForGraphSettled`, then ONE measurement of the
 *  extent and the WIP offset derived from it.
 *
 *  Returning both from a single sample also closes the weaker sibling of the
 *  same defect — reading the offset and the height as two separate samples,
 *  where status landing in between yields `wip: 0` with a WIP-inclusive height.
 *  The offset stays MEASURED, never assumed: status landing is what makes the
 *  row possible, not proof that the fixture produced one. The sanity assert
 *  catches a wrong `modelRows` constant (which would otherwise silently under-
 *  wait). */
export async function settledGraph(
  page: Page,
  modelRows: number = MOCK_MODEL_ROWS,
): Promise<{ full: number; wip: number }> {
  await waitForGraphSettled(page, modelRows);
  const full = await graphContentHeight(page);
  const wip = Math.round((full - 8) / DEFAULT_ROW_HEIGHT) - modelRows;
  expect([0, 1]).toContain(wip);
  return { full, wip };
}

/** Set scroller.scrollTop via evaluate; returns the resulting scrollTop. */
export async function scrollGraphTo(page: Page, px: number): Promise<number> {
  return graphScroller(page).evaluate((el, top) => {
    el.scrollTop = top;
    return el.scrollTop;
  }, px);
}

/* ── Shortcut LABELS ───────────────────────────────────────────────────────
 * Key PRESSES are platform-neutral already ('ControlOrMeta+K'). The TEXT the
 * app renders is not: src/utils/platform.ts prints '⌘⇧F' on macOS and
 * 'Ctrl+Shift+F' elsewhere, so specs must never hardcode either spelling.
 * We reuse the app's own renderer (imported across the e2e/src boundary — the
 * same precedent as 18-ai-bulk-resolve.spec.ts importing src/ipc/fixtures) and
 * feed it the platform explicitly.
 *
 * CAVEAT: `process.platform` is the NODE process's platform (the machine
 * running Playwright), whereas the app's `isMac` comes from the BROWSER's
 * navigator. They agree only because this repo always drives a local
 * chromium/Edge on the same machine. A remote grid / BrowserStack / a mac-UA
 * emulation run would break that assumption — such a setup would have to pass
 * the browser's platform in instead. (We pass the platform explicitly rather than
 * relying on platform.ts's default `isMac`: that default is resolved from
 * whichever navigator the *importing* process has. Node does expose a
 * browser-shaped one — `navigator.platform` is 'Win32'/'MacIntel' — so it would
 * happen to be right here, but only by coincidence of running the tests on the
 * same machine as the browser. Being explicit makes the coupling visible.)
 */
export const E2E_IS_MAC = process.platform === 'darwin';

/** Inline one-string label for a spec: 'Mod+F' -> 'Ctrl+F' | '⌘F'. */
export function expectedShortcutLabel(spec: string): string {
  return shortcutLabel(spec, E2E_IS_MAC);
}

/** ShortcutOverlay caps text: one <kbd> per token, always '+'-joined by the
 *  component (`.shortcut-plus`) on every platform — so this is NOT the same as
 *  `expectedShortcutLabel`, which drops the separator on macOS. */
export function expectedOverlayCaps(spec: string): string {
  return shortcutKeys(spec, E2E_IS_MAC).join('+');
}

/** Open the command palette (Mod+K — bound as ctrlKey||metaKey by the app). */
export async function openPalette(page: Page): Promise<Locator> {
  await page.keyboard.press('ControlOrMeta+K');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** Locate the ConfirmDialog (role=dialog, aria-label = title). */
export function confirmDialog(page: Page, title: string | RegExp): Locator {
  return page.getByRole('dialog', { name: title });
}

/** Assert the dialog is visible, then click its confirm button by name. */
export async function confirm(
  page: Page,
  title: string | RegExp,
  button: string | RegExp,
): Promise<void> {
  const dialog = confirmDialog(page, title);
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: button }).click();
}

/** Error toast locator: role=alert inside .toast-stack (Toasts.tsx). */
export function errorToast(page: Page, text?: string | RegExp): Locator {
  const alerts = page.locator('.toast-stack').getByRole('alert');
  return text === undefined ? alerts : alerts.filter({ hasText: text });
}

/** Sidebar branch row (flat list-view) + its context menu (right-click).
 *  Callers seed uiSettings { listView: 'flat' } so slashed names render as
 *  single rows (the tree view collapses folders). */
export async function openBranchContextMenu(page: Page, name: string): Promise<Locator> {
  const row = page
    .locator('li')
    .filter({ has: page.getByTitle(name, { exact: true }) })
    .first();
  // ContextMenu dismisses on ANY capture-phase scroll — pre-scroll the row into
  // view so the click's own auto-scroll can't fire a trailing scroll event that
  // instantly closes the menu (flaky on bottom-of-sidebar rows), and retry the
  // right-click if a residual scroll still races the open.
  await row.scrollIntoViewIfNeeded();
  const menu = page.getByRole('menu');
  await expect(async () => {
    await row.click({ button: 'right' });
    await expect(menu).toBeVisible({ timeout: 1000 });
  }).toPass();
  return menu;
}
