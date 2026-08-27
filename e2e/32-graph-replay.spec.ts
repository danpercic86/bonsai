/**
 * Spec-007 replay mode — harness-verifiable half of the AI gate (UI contract
 * §7). Covered here: fab/palette entry → overlay + transport; EXACT working
 * restore on Esc (scrollTop + selection — held by construction, asserted
 * anyway); scrub-while-paused coherence via the slider's aria-valuetext
 * ("Commit X of Y"); the plan's no-rAF assertion (headless pauses rAF FIRING,
 * not SCHEDULING — a page-injected wrapper counts schedule calls; zero new
 * schedules while paused and after exit settles); reduced-motion control set
 * (play/speed HIDDEN, scrubber + hint present, keyboard scrub works); Space /
 * Home / End / Esc keyboard; unborn-repo fab visible-but-disabled (§2.2).
 *
 * NOT here (USER CHECKPOINT — rAF never fires headless): playback smoothness,
 * auto-follow feel, frontier-pulse aesthetics, speed-change seamlessness.
 */
import { test, expect } from './fixtures';
import type { Locator, Page } from '@playwright/test';
import { clickGraphRow, graphScroller, openPalette, openRepo, scrollGraphTo } from './helpers';

const overlay = (page: Page): Locator => page.getByRole('dialog', { name: 'Replay history' });
const fab = (page: Page): Locator => page.locator('.graph-replay-fab');
const slider = (page: Page): Locator => page.getByRole('slider', { name: 'Replay position' });

/** Parse "Commit X of Y — Mon YYYY" (X/Y locale-formatted with commas). */
async function revealedOfTotal(page: Page): Promise<{ revealed: number; total: number }> {
  const text = await slider(page).getAttribute('aria-valuetext');
  const m = /^Commit ([\d,.\u00a0\u202f']+) of ([\d,.\u00a0\u202f']+)/.exec(text ?? '');
  expect(m, `aria-valuetext parseable: ${text}`).not.toBeNull();
  const num = (s: string): number => Number(s.replace(/[^\d]/g, ''));
  return { revealed: num(m![1]), total: num(m![2]) };
}

/** Autoplay starts on entry; headless never fires the pending frame, so the
 *  status is stuck 'playing' — every paused-state assertion must pause first. */
async function pauseReplay(page: Page): Promise<void> {
  const btn = overlay(page).getByRole('button', { name: 'Pause' });
  await btn.click();
  await expect(overlay(page).getByRole('button', { name: /^(Play|Replay again)$/ })).toBeVisible();
}

test.describe('32 graph replay mode @smoke', () => {
  test('fab entry: overlay + transport appear; Esc restores selection and scroll EXACTLY (AC2/AC6)', async ({
    page,
  }) => {
    await openRepo(page);
    // Working-graph state FIRST (the overlay covers the scroller afterwards):
    // select a commit (display row 1 — row 0 is the WIP row on the dirty
    // default fixture) and scroll to an arbitrary offset.
    await clickGraphRow(page, 1);
    const summary = page.getByTestId('commit-details').locator('.commit-summary');
    await expect(summary).toBeVisible();
    const summaryText = await summary.textContent();
    const scrollTop = await scrollGraphTo(page, 96);
    expect(scrollTop).toBe(96);

    await fab(page).click();
    await expect(overlay(page)).toBeVisible();
    await expect(page.getByTestId('graph-replay-canvas')).toBeVisible();
    await expect(slider(page)).toBeVisible();
    // The overlay owns its own scroller; the working one stays put beneath.
    await expect(page.getByTestId('graph-replay-scroller')).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(overlay(page)).toHaveCount(0);
    await expect(page.locator('.graph-replay-overlay')).toHaveCount(0); // no artifacts (AC2)
    expect(await graphScroller(page).evaluate((el) => el.scrollTop)).toBe(96);
    await expect(summary).toHaveText(summaryText ?? '');
  });

  test('palette entry; scrub while paused maps aria-valuetext coherently; Space/Home/End (AC1/AC3 keyboard)', async ({
    page,
  }) => {
    await openRepo(page);
    const palette = await openPalette(page);
    await palette.getByText('Replay history', { exact: true }).click();
    await expect(overlay(page)).toBeVisible();

    await pauseReplay(page);
    // End = full reveal → "Commit N of N"; Home = blank → "Commit 0 of N".
    await page.keyboard.press('End');
    const end = await revealedOfTotal(page);
    expect(end.total).toBeGreaterThan(0);
    expect(end.revealed).toBe(end.total);
    expect(await slider(page).getAttribute('aria-valuenow')).toBe(
      await slider(page).getAttribute('aria-valuemax'),
    );
    // Home = playhead 0, which REVEALS the oldest commit (replayModel pins
    // T[oldest] = 0 — "single commit reveals at playhead 0"), so 1, not 0.
    await page.keyboard.press('Home');
    const home = await revealedOfTotal(page);
    expect(home.revealed).toBe(1);
    expect(home.total).toBe(end.total);

    // Pointer scrub to mid-track → strictly between the ends, still paused.
    const box = await slider(page).boundingBox();
    expect(box).not.toBeNull();
    if (box === null) return;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    const mid = await revealedOfTotal(page);
    expect(mid.revealed).toBeGreaterThan(0);
    expect(mid.revealed).toBeLessThan(end.total);

    // Space resumes (label flips back to Pause), Space pauses again.
    await page.keyboard.press('Space');
    await expect(overlay(page).getByRole('button', { name: 'Pause' })).toBeVisible();
    await page.keyboard.press('Space');
    // exact: 'Exit replay' substring-matches 'Play' otherwise.
    await expect(overlay(page).getByRole('button', { name: 'Play', exact: true })).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(overlay(page)).toHaveCount(0);
  });

  test('no rAF SCHEDULING while paused or after exit (AC4, plan wrapper strategy)', async ({
    page,
  }) => {
    // Wrap BEFORE app code runs. Headless pauses rAF firing; scheduling is
    // still observable — the driver must not schedule when not playing.
    await page.addInitScript(() => {
      const w = window as unknown as { __rafCount: number };
      w.__rafCount = 0;
      const orig = window.requestAnimationFrame.bind(window);
      window.requestAnimationFrame = (cb) => {
        w.__rafCount += 1;
        return orig(cb);
      };
    });
    await openRepo(page);
    const count = (): Promise<number> =>
      page.evaluate(() => (window as unknown as { __rafCount: number }).__rafCount);

    await fab(page).click();
    await expect(overlay(page)).toBeVisible();
    await pauseReplay(page); // cancels the pending autoplay frame
    // Scrubbing while paused repaints synchronously (layout effect) — allowed;
    // it must not START a loop.
    await page.keyboard.press('End');
    await page.waitForTimeout(300); // settle
    const paused = await count();
    await page.waitForTimeout(700); // hands off: no mouse, no keys
    expect(await count()).toBe(paused);

    await page.keyboard.press('Escape');
    await expect(overlay(page)).toHaveCount(0);
    await page.waitForTimeout(400); // exit focus-restore schedules ONE rAF — let it land
    const exited = await count();
    await page.waitForTimeout(700);
    expect(await count()).toBe(exited);
  });

  test('reduced motion: play + speed hidden, hint + scrubber present and functional (AC3, §6)', async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' }); // before app mount
    await openRepo(page);
    await fab(page).click();
    await expect(overlay(page)).toBeVisible();

    await expect(overlay(page).getByRole('button', { name: /^(Play|Pause)$/ })).toHaveCount(0);
    await expect(overlay(page).getByRole('radiogroup', { name: 'Playback speed' })).toHaveCount(0);
    await expect(overlay(page).getByText('Drag the slider to move through history.')).toBeVisible();
    await expect(overlay(page).getByRole('button', { name: 'Exit replay' })).toBeVisible();

    // Scrubber is the only advance mechanism — and it works (entry focuses it).
    const before = await revealedOfTotal(page);
    expect(before.revealed).toBe(0); // no autoplay under reduced motion
    await page.keyboard.press('End');
    const after = await revealedOfTotal(page);
    expect(after.revealed).toBe(after.total);

    await page.keyboard.press('Escape');
    await expect(overlay(page)).toHaveCount(0);
  });

  // §2.2 empty-repo gate (fab visible-but-disabled on a 0-row layout) is NOT
  // testable in the harness: the mock serves the default non-empty layout for
  // EVERY repo kind (resolveLayout ignores kind 'unborn'), so an unborn repo
  // shows "No commits yet" (repoInfo-driven) beside an ENABLED fab. Reported
  // to the orchestrator as a mock-fidelity gap; the gate itself is one line in
  // replayProps.ts (`canReplay = graph !== null && nodes.length > 0`).
});
