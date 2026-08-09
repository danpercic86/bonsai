/**
 * T4 shared e2e flows (contract §3). Every seed goes through page.addInitScript
 * (runs on-origin BEFORE app code); seeds are written only when the key is
 * absent so the app's own persisted writes survive an in-test page.reload()
 * (e.g. the onboarding-persistence case).
 */
import type { Locator, Page } from '@playwright/test';
import { expect } from './fixtures';

export const FIXTURE_REPO = 'C:\\mock\\bonsai-fixture';
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

/** Clicks Skip on the Welcome dialog once visible (only spec 01 needs it). */
export async function skipOnboarding(page: Page): Promise<void> {
  const welcome = page.getByRole('dialog', { name: 'Welcome to Bonsai' });
  await expect(welcome).toBeVisible();
  await welcome.getByRole('button', { name: 'Skip' }).click();
  await expect(welcome).toBeHidden();
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
  await expect(graphCanvas(page)).toBeVisible();
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

/** Set scroller.scrollTop via evaluate; returns the resulting scrollTop. */
export async function scrollGraphTo(page: Page, px: number): Promise<number> {
  return graphScroller(page).evaluate((el, top) => {
    el.scrollTop = top;
    return el.scrollTop;
  }, px);
}

/** Ctrl+K (the app binds Ctrl on win32 UA; 'ControlOrMeta+K'). */
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
  await row.click({ button: 'right' });
  const menu = page.getByRole('menu');
  await expect(menu).toBeVisible();
  return menu;
}
