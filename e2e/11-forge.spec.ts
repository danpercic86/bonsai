/**
 * T4 spec 11 — forge / PR journeys (contract §5.11) @forge. Written post-landing
 * (the LANDS-LAST parking condition lifted when the forge UI shipped); selectors
 * are roles/labels/visible text only, per the contract.
 *
 * Mock seams (src/ipc/mock/handlers/forge.ts):
 *   (default)            unauthenticated → ForgeConnect
 *   ?forge=auth          warm start (authenticated, PR list renders at once)
 *   ?forge=off           every forge command throws networkError (offline)
 *   ?forge=unsupported   provider 'unknown' → PrPanel unsupported empty state
 *   token containing 'bad' → authFailed; head containing '#fail' → aiFailed
 *
 * Deviations from the T4 sketch, decided against the shipped code:
 *   - "viewer identity shown" — the viewer login is not rendered anywhere in
 *     the shipped PrPanel; the contract's own success signal (connect flips
 *     `authenticated` → the PR list renders) is asserted instead.
 *   - "created PR appears in list" — the mock's forgeCreatePr echoes a #999
 *     detail but does not append to FORGE_PR_LIST, so (per contract flow 3)
 *     the assertion is: submit → the new PR's DETAIL renders + success toast.
 *   - P63 badges are canvas-drawn: they are asserted through the graph hover
 *     tooltips (role=tooltip) and the badge-click → PR-detail navigation, both
 *     of which share the exact hit-rect math with the draw pass.
 */
import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import {
  DEFAULT_ROW_HEIGHT,
  errorToast,
  graphScroller,
  openRepo,
} from './helpers';

/** openRepo + wait for status so the WIP row exists and row offsets are stable
 *  (same stabilization as spec 02). */
async function openWithStatus(
  page: Page,
  opts?: Parameters<typeof openRepo>[1],
): Promise<void> {
  await openRepo(page, opts);
  await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
}

/** Switch the right pane to the PR panel. */
async function openPrTab(page: Page): Promise<void> {
  await page.getByRole('tab', { name: 'Pull requests' }).click();
  await expect(page.getByRole('tab', { name: 'Pull requests' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
}

/** Hover-sweep the LEFT ref band of a display row until the graph tooltip
 *  matches `text`; returns the matching PAGE coordinates or null. The tooltip
 *  hit-rects come from the same pure layout the canvas draw pass uses, so a
 *  match proves the pill/badge is actually drawn there. */
async function sweepRefBand(
  page: Page,
  displayRow: number,
  text: RegExp,
): Promise<{ x: number; y: number } | null> {
  const box = await graphScroller(page).boundingBox();
  if (box === null) throw new Error('graph scroller has no bounding box');
  const y = box.y + displayRow * DEFAULT_ROW_HEIGHT + DEFAULT_ROW_HEIGHT / 2;
  const tip = page.getByRole('tooltip').filter({ hasText: text });
  // Park the cursor off the band first so re-sweeps re-trigger mousemove.
  await page.mouse.move(box.x + 400, box.y + 2);
  for (let x = 8; x < 178; x += 5) {
    await page.mouse.move(box.x + x, y);
    try {
      await expect(tip).toBeVisible({ timeout: 50 });
      return { x: box.x + x, y };
    } catch {
      // keep sweeping
    }
  }
  return null;
}

test.describe('11 forge @forge', () => {
  test('cold connect: prompt → bad token rejected → good token renders the PR list', async ({
    page,
  }) => {
    await openRepo(page); // default: unauthenticated
    await openPrTab(page);

    await test.step('connect prompt renders with the GitHub guidance', async () => {
      await expect(page.getByText('Connect to github.com')).toBeVisible();
      await expect(page.getByText('octo-org/bonsai')).toBeVisible();
      // No token yet → submit disabled.
      await expect(page.getByRole('button', { name: 'Connect' })).toBeDisabled();
    });

    const token = page.getByLabel('Personal access token');

    await test.step("token containing 'bad' → authFailed, still disconnected", async () => {
      await token.fill('ghp_bad_token');
      await page.getByRole('button', { name: 'Connect' }).click();
      await expect(
        page.getByRole('alert').filter({ hasText: 'token rejected by GET /user' }).first(),
      ).toBeVisible();
      await expect(errorToast(page, /Could not connect/)).toBeVisible();
      // The connect form is still there (nothing flipped).
      await expect(token).toBeVisible();
    });

    await test.step('valid token → authenticated → the fixture PR list renders', async () => {
      await token.fill('ghp_e2e_mock');
      await page.getByRole('button', { name: 'Connect' }).click();
      await expect(
        page.getByText('Render PR/CI status badges beside graph nodes'),
      ).toBeVisible();
      await expect(page.getByText('#128')).toBeVisible();
    });
  });

  test('warm start: PR list → filters → detail with labels, mergeable and comments', async ({
    page,
  }) => {
    await openRepo(page, { flags: { forge: 'auth' } });
    await openPrTab(page);

    await test.step('open filter (default) shows the 3 open fixture PRs', async () => {
      await expect(
        page.getByText('Render PR/CI status badges beside graph nodes'),
      ).toBeVisible();
      await expect(
        page.getByText('WIP: deterministic lane colors while scrolling'),
      ).toBeVisible();
      await expect(page.getByText('Fix scroll jank over 20k-commit histories')).toBeVisible();
      await expect(page.getByText('Draft', { exact: true })).toBeVisible(); // #127
      await expect(page.getByText('Right-pane working-directory status panel')).toHaveCount(0);
    });

    await test.step("'All' filter adds the merged PR", async () => {
      await page.getByRole('button', { name: 'All', exact: true }).click();
      await expect(
        page.getByText('Right-pane working-directory status panel'),
      ).toBeVisible();
      await expect(page.getByText('Merged', { exact: true })).toBeVisible();
    });

    await test.step('opening #128 renders the full detail + review comments', async () => {
      await page
        .getByRole('button', { name: /Render PR\/CI status badges beside graph nodes/ })
        .click();
      await expect(page.getByText('Open in browser ↗')).toBeVisible();
      await expect(page.getByText('No conflicts')).toBeVisible();
      await expect(page.getByText('enhancement')).toBeVisible(); // label
      await expect(page.getByText('+214')).toBeVisible();
      await expect(page.getByText('Comments (3)')).toBeVisible();
      await expect(
        page.getByText('Love this — does the badge cache invalidate on fetch?'),
      ).toBeVisible();
      await expect(page.getByText('crates/bonsai-forge/src/github/rollup.rs:42').first()).toBeVisible();
    });

    await test.step('back returns to the list', async () => {
      await page.getByRole('button', { name: '← Pull requests' }).click();
      await expect(page.getByRole('button', { name: 'New pull request' })).toBeVisible();
    });
  });

  test('create PR: form gating → submit → new detail renders + success toast', async ({
    page,
  }) => {
    await openRepo(page, { flags: { forge: 'auth' } });
    await openPrTab(page);
    await page.getByRole('button', { name: 'New pull request' }).click();
    await expect(page.getByText('Open a pull request')).toBeVisible();

    const base = page.getByPlaceholder('target branch (e.g. main)');
    const compare = page.getByPlaceholder('source branch');
    const title = page.getByPlaceholder('Add a title');
    const submit = page.getByRole('button', { name: /Create pull request/ });

    await test.step('submit stays disabled until base + compare + title are set', async () => {
      await base.fill('main');
      await compare.fill('feat');
      await expect(submit).toBeDisabled();
      await title.fill('e2e: created from spec 11');
      await expect(submit).toBeEnabled();
    });

    await test.step('submit → the created PR detail (#999) renders', async () => {
      await submit.click();
      // exact: the success toast repeats '#999' inside its longer message.
      await expect(page.getByText('#999', { exact: true })).toBeVisible();
      await expect(page.getByText('e2e: created from spec 11')).toBeVisible();
      await expect(page.getByText('feat → main')).toBeVisible();
      await expect(
        page.locator('.toast-stack').getByText(/Opened PR #999/),
      ).toBeVisible();
      await expect(page.getByText('No comments yet.')).toBeVisible();
    });
  });

  test('AI description: proposal fills the form and NEVER submits; #fail surfaces an error', async ({
    page,
  }) => {
    await openRepo(page, {
      flags: { forge: 'auth' },
      uiSettings: { onboardingSeen: true, aiConsented: true },
    });
    await openPrTab(page);
    await page.getByRole('button', { name: 'New pull request' }).click();

    const base = page.getByPlaceholder('target branch (e.g. main)');
    const compare = page.getByPlaceholder('source branch');
    const title = page.getByPlaceholder('Add a title');
    const body = page.getByPlaceholder('Describe the change (optional)');
    const generate = page.getByRole('button', { name: 'Generate description with AI' });

    await test.step('needs a resolvable range before it enables', async () => {
      await base.fill('main');
      await compare.fill('');
      await expect(generate).toBeDisabled();
      await compare.fill('feat');
      await expect(generate).toBeEnabled();
    });

    await test.step('generate fills title + body from base..head, form not submitted', async () => {
      await generate.click();
      await expect(title).toHaveValue('Add feat onto main');
      await expect(body).toHaveValue(/Bring the work on `feat` into `main`/);
      // Still on the create form — a proposal must never auto-submit.
      await expect(page.getByRole('button', { name: /Create pull request/ })).toBeVisible();
      await expect(page.getByText('#999')).toHaveCount(0);
    });

    await test.step("head containing '#fail' → aiFailed toast, form stays usable", async () => {
      await compare.fill('feat#fail');
      await generate.click();
      await expect(errorToast(page, /Could not generate a description/)).toBeVisible();
      await title.fill('still editable after the failure');
      await expect(title).toHaveValue('still editable after the failure');
    });
  });

  test('?ai=off with consent: the generate button is disabled (aiUnavailable gate)', async ({
    page,
  }) => {
    await openRepo(page, {
      flags: { forge: 'auth', ai: 'off' },
      uiSettings: { onboardingSeen: true, aiConsented: true },
    });
    await openPrTab(page);
    await page.getByRole('button', { name: 'New pull request' }).click();
    await page.getByPlaceholder('target branch (e.g. main)').fill('main');
    await page.getByPlaceholder('source branch').fill('feat');
    await expect(
      page.getByRole('button', { name: 'Generate description with AI' }),
    ).toBeDisabled();
  });

  test('PR + CI badges: graph tooltips and badge-click → PR detail (when enabled)', async ({
    page,
  }) => {
    test.setTimeout(90_000); // hover sweeps + the badge fetch debounce
    // Badges are OFF by default (GraphPrefs); enable both + warm forge auth.
    await openWithStatus(page, {
      flags: { forge: 'auth' },
      uiSettings: {
        onboardingSeen: true,
        graph: { showPrBadge: true, showCiStatus: true },
      },
    });

    // Display row 5 = 'feat: polish' (WIP 1 + stashes 3 + row 4 HEAD): the
    // `feat` branch tip, fixture PR #128 head, CI rollup success.
    let prHit: { x: number; y: number } | null = null;
    await test.step('hovering the PR badge shows the PR tooltip', async () => {
      await expect(async () => {
        prHit = await sweepRefBand(page, 5, /PR #128/);
        expect(prHit).not.toBeNull();
      }).toPass({ timeout: 30_000 });
      const tip = page.getByRole('tooltip');
      await expect(tip).toContainText('PR #128 (open)');
      await expect(tip).toContainText('Render PR/CI status badges beside graph nodes');
    });

    await test.step('hovering the CI dot shows the checks rollup', async () => {
      const ciHit = await sweepRefBand(page, 5, /Checks: 3 passed, 0 failed, 0 pending/);
      expect(ciHit).not.toBeNull();
    });

    await test.step('clicking the PR badge opens the PR detail in the right pane', async () => {
      expect(prHit).not.toBeNull();
      await page.mouse.click(prHit!.x, prHit!.y);
      await expect(page.getByRole('tab', { name: 'Pull requests' })).toHaveAttribute(
        'aria-selected',
        'true',
      );
      // Park the cursor off the graph so the hover tooltip (which repeats the
      // PR title) dismisses before the strict-mode text assert.
      await page.mouse.move(10, 10);
      await expect(page.getByText('#128')).toBeVisible();
      await expect(
        page.getByText('Render PR/CI status badges beside graph nodes'),
      ).toBeVisible();
    });
  });

  test('badges default OFF: the ref band shows pills but no PR/CI badge', async ({ page }) => {
    await openWithStatus(page, { flags: { forge: 'auth' } });
    // The branch-pill tooltip proves the sweep is hitting the ref band…
    const pill = await sweepRefBand(page, 5, /^feat$/);
    expect(pill).not.toBeNull();
    // …and the same sweep finds no PR badge anywhere on the row.
    const pr = await sweepRefBand(page, 5, /PR #/);
    expect(pr).toBeNull();
  });

  test('?forge=off: offline error state with a working (still-failing) Retry', async ({
    page,
  }) => {
    await openRepo(page, { flags: { forge: 'off' } });
    await openPrTab(page);
    const banner = page.getByRole('alert').filter({ hasText: 'forge is offline' });
    await expect(banner).toBeVisible();
    await test.step('Retry re-fetches (still failing) without crashing', async () => {
      await page.getByRole('button', { name: 'Retry' }).click();
      await expect(banner).toBeVisible();
      // The app is still alive: switch back to the working-dir tab and see status.
      await page.getByRole('tab', { name: 'Working' }).click();
      await expect(page.getByTestId('status-panel').getByText(/Staged \(/)).toBeVisible();
    });
  });

  test('?forge=unsupported: unknown provider shows the unsupported empty state', async ({
    page,
  }) => {
    await openRepo(page, { flags: { forge: 'unsupported' } });
    await openPrTab(page);
    await expect(
      page.getByText(/git\.example\.com isn't a supported forge yet/),
    ).toBeVisible();
    // No connect prompt and no PR list for an unsupported origin.
    await expect(page.getByLabel('Personal access token')).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'New pull request' })).toHaveCount(0);
  });
});
