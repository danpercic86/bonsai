/**
 * P68e §13.2 — the AI activity dock in the mock-IPC browser harness.
 *
 * SCOPE, stated honestly: `pnpm dev:mock` composites at 0×0 under this harness, so
 * everything about APPEARANCE is native-only (whether the log reads as live, whether
 * 180px is comfortable, whether the `Needs you` tint catches the eye). What IS
 * machine-verifiable, and is what this spec asserts, is DOM structure, computed CSS
 * values, text content and state transitions.
 *
 * The user's complaint was "I clicked AI, nothing seemed to happen, and I had no
 * feedback", so the load-bearing assertions here are: the collapsed bar shows a
 * status word + subject + a ticking clock + the latest output line, and Cancel
 * reports `Stopping…` immediately without losing the output collected so far.
 */
import { test, expect } from './fixtures';
import { openRepo } from './helpers';
import type { Locator, Page } from '@playwright/test';

const AI = { onboardingSeen: true, aiConsented: true };

function dock(page: Page): Locator {
  return page.getByRole('region', { name: 'AI activity' });
}

/** Open the seeded paused merge with AI consented, and start a run on src/auth.ts. */
async function startRun(page: Page, flags: Record<string, string> = {}): Promise<void> {
  await openRepo(page, { flags: { op: 'merge', ...flags }, uiSettings: AI });
  const button = page.getByRole('button', { name: 'Resolve src/auth.ts with AI' });
  await expect(button).toBeEnabled();
  await button.click();
}

test.describe('17 AI activity dock', () => {
  test('no dock exists until a run starts, then it is .workspace-host’s third child', async ({
    page,
  }) => {
    await openRepo(page, { flags: { op: 'merge' }, uiSettings: AI });
    await expect(dock(page)).toHaveCount(0);

    // §13.2-2: the graph pane's height must shrink by the dock's height — proof the
    // dock is in flow and nothing overlaps the canvas.
    const panesBefore = await page.locator('.panes').evaluate((el) => el.clientHeight);

    await page.getByRole('button', { name: 'Resolve src/auth.ts with AI' }).click();
    await expect(dock(page)).toBeVisible();

    const geometry = await page.evaluate(() => {
      const host = document.querySelector('.workspace-host:not([style*="display: none"])');
      const kids = [...(host?.children ?? [])];
      const node = document.querySelector('.ai-dock');
      return {
        index: node === null ? -1 : kids.indexOf(node),
        count: kids.length,
        flexGrow: node === null ? '' : getComputedStyle(node).flexGrow,
        dockHeight: node?.getBoundingClientRect().height ?? 0,
        panes: document.querySelector('.panes')?.clientHeight ?? 0,
      };
    });
    expect(geometry.index).toBe(2);
    expect(geometry.flexGrow).toBe('0');
    expect(geometry.dockHeight).toBeGreaterThan(180);
    expect(panesBefore - geometry.panes).toBe(geometry.dockHeight);
  });

  test('?aiSlow: the collapsed bar answers "is something happening?" on its own', async ({
    page,
  }) => {
    await startRun(page, { aiSlow: '1' });
    const bar = dock(page);
    await expect(bar.locator('.ai-dock-status')).toHaveText(/Running/);
    await expect(bar.locator('.ai-dock-subject')).toHaveText('src/auth.ts');

    // Collapse it: the bar alone must still carry status + clock + latest line.
    await bar.getByRole('button', { name: 'AI activity' }).click();
    await expect(page.locator('#ai-dock-body')).toHaveCount(0);
    await expect(bar.locator('.ai-dock-activity')).toContainText('Read(src/auth.ts)');

    // The clock ticks (>= 1.2s apart, per §13.2-3).
    const first = await bar.locator('.ai-dock-elapsed').textContent();
    await page.waitForTimeout(1_400);
    expect(await bar.locator('.ai-dock-elapsed').textContent()).not.toBe(first);

    // Cost is honest while unknown (U13) — never a guess.
    await expect(bar.locator('.ai-dock-cost')).toHaveText('$—');
    await expect(bar.locator('.ai-dock-cost')).toHaveAttribute(
      'title',
      'Cost appears when Claude finishes a turn',
    );

    // §12-B1: `$—` is only acceptable BECAUSE something else moves. The mock emits the
    // CLI's `thinking_tokens` heartbeat (metrics-only `log` events) every third tick, so
    // the estimate is the live spend signal during the first turn.
    const thinking = bar.locator('.ai-dock-thinking');
    await expect(thinking).toHaveText(/^~[\d,. ]+ tok$/, { timeout: 12_000 });
    await expect(thinking).toHaveAttribute('title', /not a price/);
    // NEVER priced: no dollar figure is derived from the token count.
    await expect(thinking).not.toContainText('$');
    // It grows while the run works, which is the whole point of showing it.
    const early = await thinking.textContent();
    await expect
      .poll(async () => (await thinking.textContent()) !== early, { timeout: 12_000 })
      .toBe(true);
  });

  test('?aiSlow: the log grows, then Cancel says Stopping… and keeps the output (D2)', async ({
    page,
  }) => {
    await startRun(page, { aiSlow: '1' });
    const lines = dock(page).locator('.ai-log-line');
    await expect.poll(async () => lines.count()).toBeGreaterThan(2);
    const before = await lines.count();

    const cancel = dock(page).getByRole('button', { name: 'Cancel the AI run' });
    await cancel.click();
    // Immediate, before any IPC resolves.
    await expect(dock(page).getByRole('button', { name: 'Stopping the AI run' })).toBeDisabled();
    await expect(dock(page).locator('.ai-dock-status')).toHaveAttribute('data-status', 'stopping');

    await expect(dock(page).locator('.ai-dock-status')).toHaveText(/Cancelled/);
    expect(await lines.count()).toBeGreaterThanOrEqual(before);
    // U7: the unfinished fragment is quarantined behind a closed disclosure with no
    // way to use it.
    const partial = dock(page).getByRole('button', { name: /Unfinished output/ });
    await expect(partial).toHaveAttribute('aria-expanded', 'false');
    await expect(dock(page).getByText(/Bonsai will not apply it/)).toBeVisible();
  });

  test('?aiAsk: the question is shown, Enter sends the answer, the run completes', async ({
    page,
  }) => {
    await startRun(page, { aiAsk: '1' });
    const ask = dock(page).getByRole('group', { name: 'Claude needs your answer' });
    await expect(ask).toBeVisible();
    await expect(ask.getByText(/German plural form/)).toBeVisible();
    // §4.1: the dock reads as "needs you" even collapsed, via the tint attribute.
    await expect(dock(page)).toHaveAttribute('data-attention', 'true');

    const box = ask.getByRole('textbox', { name: 'Your answer to Claude' });
    await box.fill('Einträge');
    await box.press('Enter');
    await expect(dock(page).locator('.ai-dock-status')).toHaveText(/Ready/);
    // The store's own `» answered (n bytes)` line landed — the UI invents nothing.
    await expect(dock(page).locator('.ai-log-line[data-kind="meta"]').last()).toContainText(
      'answered',
    );
  });

  test('?aiFlood: the 500-line cap, the trim note and the truncation chip', async ({ page }) => {
    await startRun(page, { aiFlood: '1' });
    const log = dock(page).locator('.ai-log');
    // D5, MEASURED. The store buffers log lines behind ONE 50 ms timer, so the dock
    // must land them in BATCHES. A MutationObserver callback fires once per
    // microtask checkpoint, i.e. roughly once per React commit — so counting
    // CALLBACKS (not records: React inserts nodes one at a time, so records track
    // lines) is the direct observation. ~700 lines arriving 1 ms apart must produce
    // an order of magnitude fewer commits, or `RepoWorkspace` repaints per line.
    await page.evaluate(() => {
      const node = document.querySelector('.ai-log');
      if (node === null) return;
      const w = window as unknown as { __logCommits?: number };
      w.__logCommits = 0;
      new MutationObserver(() => {
        w.__logCommits = (w.__logCommits ?? 0) + 1;
      }).observe(node, { childList: true });
    });
    await expect(dock(page).locator('.ai-log-dropped')).toBeVisible({ timeout: 30_000 });
    await expect(dock(page).locator('.ai-log-dropped')).toContainText('earlier lines trimmed');
    // The cap holds: never more than 500 retained lines.
    expect(await log.locator('.ai-log-line').count()).toBeLessThanOrEqual(500);
    await expect(dock(page).locator('.ai-log-trunc')).toHaveCount(1);

    const commits = await page.evaluate(
      () => (window as unknown as { __logCommits?: number }).__logCommits ?? 0,
    );
    expect(commits).toBeGreaterThan(0);
    expect(commits, `${commits} DOM commits for ~700 log lines`).toBeLessThan(120);
  });

  /** The queue itself needs a BULK run, whose only entry point is P68f's "Resolve
   *  all" — out of scope here (and covered by the jsdom suite). What IS reachable
   *  today is the single-file failure arm, which is the state the user actually hit. */
  test('?aiFail: the dock states the error verbatim and that nothing changed', async ({ page }) => {
    await openRepo(page, { flags: { op: 'merge', aiFail: '1' }, uiSettings: AI });
    await page.getByRole('button', { name: 'Resolve src/auth.ts with AI' }).click();
    await expect(dock(page).locator('.ai-dock-status')).toHaveText(/Failed/);
    await expect(dock(page).getByRole('alert')).toContainText('Claude exited without a result');
    await expect(
      dock(page).getByText('Nothing was changed. You can retry, or resolve this file by hand.'),
    ).toBeVisible();
  });

  test('?ai=off: the dock is never created', async ({ page }) => {
    await openRepo(page, { flags: { op: 'merge', ai: 'off' }, uiSettings: AI });
    await expect(page.getByRole('button', { name: 'Take our version of src/auth.ts' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Resolve src/auth.ts with AI' })).toBeDisabled();
    await expect(dock(page)).toHaveCount(0);
  });

  test('height and collapsed state persist across a reload', async ({ page }) => {
    await startRun(page, { aiSlow: '1' });
    const grip = page.locator('.pane-divider-ai-dock');
    await expect(grip).toHaveAttribute('aria-valuenow', '180');
    // Keyboard resize commits immediately (one settings write per keypress).
    await grip.focus();
    await grip.press('ArrowUp');
    await grip.press('ArrowUp');
    await expect(grip).toHaveAttribute('aria-valuenow', '196');
    await dock(page).getByRole('button', { name: 'AI activity' }).click();

    // Poll the persistence key rather than sleeping: the write is debounced AND the
    // mock handler delays ~150ms before storing, so any fixed wait is a race that
    // only loses under load. Mirrors 10-settings-persistence.spec.ts.
    await expect
      .poll(
        () =>
          page.evaluate(() => {
            const raw = window.localStorage.getItem('bonsai.mockUiSettings') ?? '{}';
            const s = JSON.parse(raw) as { aiDockHeight?: number; aiDockCollapsed?: boolean };
            return `${s.aiDockHeight ?? ''}:${s.aiDockCollapsed ?? ''}`;
          }),
        { timeout: 10_000 },
      )
      .toBe('196:true');
  });

  test('density and theme are driven by tokens only; --text-3 is used nowhere', async ({
    page,
  }) => {
    await startRun(page, { aiSlow: '1' });
    const read = () =>
      page.evaluate(() => {
        const node = document.querySelector('.ai-dock');
        if (node === null) return null;
        const style = getComputedStyle(node);
        const text3 = getComputedStyle(document.documentElement)
          .getPropertyValue('--text-3')
          .trim();
        const colours = [...node.querySelectorAll('*')].map((el) => getComputedStyle(el).color);
        return {
          logFont: style.getPropertyValue('--ai-dock-log-font').trim(),
          ctlH: style.getPropertyValue('--ai-dock-ctl-h').trim(),
          text3,
          usesText3: colours.some((c) => c !== '' && c === text3),
        };
      });

    const cozy = await read();
    expect(cozy?.logFont).toBe('12px');
    expect(cozy?.ctlH).toBe('28px');
    // U9: --text-3 is below AA as text; the dock's muted role is --text-2.
    expect(cozy?.usesText3).toBe(false);

    await page.evaluate(() => {
      document.querySelector('.ai-dock')?.setAttribute('data-density', 'compact');
    });
    const compact = await read();
    expect(compact?.logFont).toBe('11px');
    // §1.7: the AA hit-target floor, never below 24px.
    expect(compact?.ctlH).toBe('24px');
  });

  test('Mod+Shift+A reaches the dock even from the commit message box', async ({ page }) => {
    await startRun(page, { aiAsk: '1' });
    const reply = page.getByRole('textbox', { name: 'Your answer to Claude' });
    await expect(reply).toBeVisible();

    // THE POINT OF THE SHORTCUT (U6): the binding sits BEFORE the `typing` guard in
    // `useWorkspaceKeyboard`, so it must fire from inside a text field. Typing in the
    // commit box is the reported scenario — Claude's question lands 40 s into a run
    // while the user is mid-sentence — so the caret really is put there first.
    const commit = page.getByPlaceholder('Commit message');
    await commit.fill('wip: mid-sentence when Claude asked');
    await expect(commit).toBeFocused();

    await page.keyboard.press('ControlOrMeta+Shift+A');
    await expect(reply).toBeFocused();
    // Nothing was typed into the commit message and nothing was lost from it.
    await expect(commit).toHaveValue('wip: mid-sentence when Claude asked');
  });
});
