/**
 * P74 — accessibility guarantees, locked in a real browser
 * (`docs/contracts/P74-a11y-toasts-hit-targets.md`, AC-1…AC-26;
 * `docs/contracts/ui-reference.md` §2 contrast, §3.1 hit-target floor,
 * §7 glyph vocabulary, §10.2 toast tones).
 *
 * WHY THIS SPEC EXISTS: 21 of P74's 26 acceptance criteria were verified once,
 * by hand, through harness computed-style reads. Contrast ratios and hit-target
 * sizes are the archetypal SILENT regression — nothing throws, nothing looks
 * broken, and the next CSS edit can undo them with the whole suite green. Every
 * assertion below is a computed style or a `getBoundingClientRect`, i.e. exactly
 * the class of check that only a real engine can answer (jsdom resolves neither
 * `color-mix()` nor `align-self: stretch`).
 *
 * THRESHOLDS, NOT MEASUREMENTS: contrast is asserted against the WCAG bars
 * (4.5:1 text, 3:1 non-text), never against the exact figures the contract
 * recorded. A spec pinned to 10.30:1 would fail the moment a token is
 * legitimately retuned, which is worse than no test. The contract's reference
 * values are carried in the failure messages instead.
 *
 * THEME SWITCHING: the four `?toasts=demo` tones include three NON-sticky ones
 * that auto-dismiss after 5 s, so both themes are measured inside a SINGLE
 * synchronous `page.evaluate` that flips `data-theme` on <html>, reads, and
 * flips back. `getComputedStyle` forces a style recalc synchronously, so this is
 * sound and immune to the auto-dismiss timer (a reload-per-theme would race it).
 * Geometry tests use the sticky `error` toast and so need no such care.
 * SPLIT NOTE: this file is the SIDEBAR HIT-TARGET half of the original single
 * P74 spec; the toast half is `26-a11y-toasts.spec.ts` and the shared
 * plumbing is `helpers/a11y.ts`. Pure move — no assertion, threshold or comment
 * was changed or dropped.
 */
import { test, expect } from './fixtures';
import { confirm, openBranchContextMenu, openRepo } from './helpers';
import { FLOOR, rect, sidebarReady, SLACK } from './helpers/a11y';

test.describe('27 a11y — sidebar hit targets @smoke', () => {
  /* ---------------------------------------------------------------- *
   * Item 2 — hit targets
   * ---------------------------------------------------------------- */

  test('EVERY sidebar control clears the 24px floor (AC-24 — the generic guard)', async ({
    page,
  }) => {
    await openRepo(page);
    await sidebarReady(page);

    // Reveal the one control that only exists with a filter typed (AC-19).
    const filter = page.locator('.sidebar .list-filter-input').first();
    await filter.fill('a');
    await expect(page.locator('.sidebar .list-filter-clear')).toBeVisible();

    // The point of sweeping rather than enumerating: a control added next year
    // is covered without editing this spec. WCAG 2.2 SC 2.5.8 = 24x24 CSS px.
    const undersized = await page.evaluate((slack) => {
      const nodes = Array.from(
        document.querySelectorAll('.sidebar button, .sidebar [role="button"], .sidebar input'),
      );
      return nodes
        .map((el) => {
          const r = el.getBoundingClientRect();
          return {
            sel: el.className || el.tagName,
            label: el.getAttribute('aria-label') ?? (el.textContent ?? '').trim().slice(0, 24),
            w: Math.round(r.width * 100) / 100,
            h: Math.round(r.height * 100) / 100,
          };
        })
        // Zero-size nodes are not rendered (collapsed/hidden) — not targets.
        .filter((m) => !(m.w === 0 && m.h === 0))
        .filter((m) => m.w < 24 - slack || m.h < 24 - slack);
    }, SLACK);

    expect(
      undersized,
      `sidebar controls below the 24x24 floor (ui-reference §3.1, WCAG 2.5.8):\n${JSON.stringify(undersized, null, 2)}`,
    ).toEqual([]);
  });

  test('the named sidebar controls measure exactly what P74 set (AC-15..20,22)', async ({
    page,
  }) => {
    await openRepo(page);
    await sidebarReady(page);

    const m = await page.evaluate(() => {
      const all = (sel: string) =>
        Array.from(document.querySelectorAll(sel)).map((el) => {
          const r = el.getBoundingClientRect();
          return { w: r.width, h: r.height };
        });
      return {
        toggles: all('.sidebar-section-toggle'),
        headers: all('.sidebar-section-header'),
        adds: all('.sidebar-add'),
        treeToggles: all('.sidebar .tree-dir-toggle'),
        treeRows: all('.sidebar .tree-dir-row'),
        branchRows: all('.branch-row'),
        filterInputs: all('.sidebar .list-filter-input'),
        sectionMarginBottom: getComputedStyle(
          document.querySelector('.sidebar-section') as HTMLElement,
        ).marginBottom,
        branchListMarginTop: getComputedStyle(
          document.querySelector('.branch-list') as HTMLElement,
        ).marginTop,
      };
    });

    // AC-15 / AC-17: six sections, header row and toggle both on the 24px module
    // (the toggles were 16px, and the Tags header was 16 while the rest were 20).
    expect(m.toggles.length).toBe(6);
    expect(m.headers.length).toBe(6);
    for (const t of m.toggles) expect(t.h).toBeGreaterThanOrEqual(FLOOR - SLACK);
    for (const h of m.headers) expect(h.h).toBeGreaterThanOrEqual(FLOOR - SLACK);
    for (const t of m.toggles) expect(t.h).toBeLessThan(FLOOR + 2);

    // AC-16: all six `+` / cleanup buttons are 24x24 boxes (were 20x20).
    expect(m.adds.length).toBe(6);
    for (const a of m.adds) {
      expect(a.w).toBeGreaterThanOrEqual(FLOOR - SLACK);
      expect(a.h).toBeGreaterThanOrEqual(FLOOR - SLACK);
      expect(a.w).toBeLessThan(FLOOR + 2);
      expect(a.h).toBeLessThan(FLOOR + 2);
    }

    // AC-20: the shared Tree toggle stretches to the sidebar's 24px row, and
    // the row rhythm itself did NOT move.
    expect(m.treeToggles.length).toBeGreaterThan(0);
    for (const t of m.treeToggles) expect(t.h).toBeGreaterThanOrEqual(FLOOR - SLACK);
    for (const r of m.treeRows) expect(r.h).toBeCloseTo(24, 1);

    // AC-23: no list geometry moved while the headers grew.
    expect(m.branchRows.length).toBeGreaterThan(0);
    for (const r of m.branchRows) expect(r.h).toBeCloseTo(24, 1);
    expect(m.sectionMarginBottom).toBe('16px');
    expect(m.branchListMarginTop).toBe('4px');
    for (const i of m.filterInputs) expect(i.h).toBeCloseTo(24, 1);

    // AC-19: the clear affordance only exists with a value, and it is 24x24.
    await page.locator('.sidebar .list-filter-input').first().fill('a');
    const clear = await rect(page, '.sidebar .list-filter-clear');
    expect(clear).not.toBeNull();
    expect(clear!.w).toBeCloseTo(24, 1);
    expect(clear!.h).toBeCloseTo(24, 1);
  });

  test('the painted glyphs did NOT grow with their boxes (AC-18)', async ({ page }) => {
    await openRepo(page);
    await sidebarReady(page);
    await page.locator('.sidebar .list-filter-input').first().fill('a');

    // The whole technique is "a transparent hit box larger than the painted
    // glyph" (house precedent: .btn-icon is 32x32 around a 14-16px glyph). A
    // future "fix" that scales the icon up with the box must fail HERE.
    const m = await page.evaluate(() => {
      const svg = document.querySelector('.sidebar-add-icon svg');
      const svgRect = svg?.getBoundingClientRect();
      const plus = Array.from(document.querySelectorAll('.sidebar-add')).find(
        (el) => (el.textContent ?? '').trim() === '+',
      );
      const clear = document.querySelector('.list-filter-clear');
      return {
        svg: svgRect ? { w: svgRect.width, h: svgRect.height } : null,
        plusFont: plus ? getComputedStyle(plus).fontSize : null,
        clearFont: clear ? getComputedStyle(clear).fontSize : null,
      };
    });

    expect(m.svg).not.toBeNull();
    expect(m.svg!.w).toBeCloseTo(14, 1);
    expect(m.svg!.h).toBeCloseTo(14, 1);
    expect(m.plusFont).toBe('14px');
    expect(m.clearFont).toBe('14px');
  });

  test('the section-toggle hover wash starts flush with .branch-row at x=12 (P74 SF-1)', async ({
    page,
  }) => {
    await openRepo(page);
    await sidebarReady(page);
    // OPEN-1's `margin-left: -4px` was WITHDRAWN after the post-implementation
    // design review: it made the wash the only element in the pane intruding
    // into the sidebar's 12px gutter. If it creeps back, the toggle's rect
    // starts at x = 8 and this fails.
    const toggle = await rect(page, '.sidebar-section-toggle');
    const row = await rect(page, '.branch-row');
    expect(toggle).not.toBeNull();
    expect(row).not.toBeNull();
    expect(toggle!.left, 'section toggle must not intrude into the 12px gutter').toBeCloseTo(
      12,
      1,
    );
    expect(toggle!.left, 'the hover wash must align with .branch-row:hover').toBeCloseTo(
      row!.left,
      1,
    );
  });

  test('the compact right-panel folder toggle stays 20px — a RECORDED exemption (AC-21)', async ({
    page,
  }) => {
    await openRepo(page);
    // The status tree loads after the graph canvas — wait for a real folder row.
    await expect(page.locator('.right-panel .tree-dir-row').first()).toBeVisible();

    // READ THIS BEFORE "FIXING" IT: 20px is DELIBERATE and documented
    // (P74 OPEN-2, ui-reference §3.1 compact-density exemption). The shared
    // `.tree-dir-toggle` uses `align-self: stretch` so it TRACKS `--rp-row-h`
    // instead of hardcoding 24px; in the right panel's opt-in `compact`
    // density that row is 20px, and raising it to 24 would delete the point of
    // `compact`. The assertion below therefore pins BOTH numbers: 24 in cozy
    // (the default) and 20 in compact. A change to either is a contract change,
    // not a bug fix.
    const measure = async (density: 'cozy' | 'compact') => {
      await page.evaluate((d) => {
        document.querySelector('.right-panel')?.setAttribute('data-density', d);
      }, density);
      return page.evaluate(() => {
        const t = document.querySelector('.right-panel .tree-dir-toggle');
        const r = document.querySelector('.right-panel .tree-dir-row');
        return {
          toggle: t ? t.getBoundingClientRect().height : null,
          row: r ? r.getBoundingClientRect().height : null,
        };
      });
    };

    const cozy = await measure('cozy');
    expect(cozy.toggle, 'a right-panel folder row must be present to measure').not.toBeNull();
    expect(cozy.toggle!).toBeCloseTo(24, 1);
    expect(cozy.row!).toBeCloseTo(24, 1);

    const compact = await measure('compact');
    expect(compact.toggle!).toBeCloseTo(20, 1);
    expect(compact.row!).toBeCloseTo(20, 1);
  });

  test('the sidebar error banner dismiss is a 24x24 target (AC-22)', async ({ page }) => {
    // The only reachable route to `.error-dismiss` in the sidebar: a refused
    // delete of the unmerged fixture branch, which RepoWorkspace routes to the
    // sidebar's inline error slot rather than a toast (see spec 05).
    await openRepo(page, { uiSettings: { onboardingSeen: true, listView: 'flat' } });
    const menu = await openBranchContextMenu(page, 'experiment-unmerged');
    await menu.getByRole('menuitem', { name: 'Delete' }).click();
    await confirm(page, 'Delete branch', 'Delete branch');
    await expect(page.locator('.sidebar-error')).toBeVisible();

    const dismiss = await rect(page, '.sidebar-error .error-dismiss');
    expect(dismiss).not.toBeNull();
    expect(dismiss!.w).toBeCloseTo(24, 1);
    expect(dismiss!.h).toBeCloseTo(24, 1);
  });
});
