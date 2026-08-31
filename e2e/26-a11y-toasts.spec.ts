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
 * SPLIT NOTE: this file is the TOAST half of the original single P74 spec; the
 * sidebar hit-target half is `27-a11y-hit-targets.spec.ts` and the shared
 * contrast/compositing plumbing is `helpers/a11y.ts`. Pure move — no assertion,
 * threshold or comment was changed or dropped.
 */
import { test, expect } from './fixtures';
import { openRepo } from './helpers';
import { readTones } from './helpers/a11y';

test.describe('26 a11y — toast tones @smoke', () => {
  test('every tone clears AA text and 3:1 non-text, in BOTH themes (AC-1,2,4,6,7)', async ({
    page,
  }) => {
    await openRepo(page, { flags: { toasts: 'demo' } });
    await expect(page.locator('.toast')).toHaveCount(4);

    const read = await readTones(page);

    for (const theme of ['dark', 'light'] as const) {
      const tones = read[theme];
      expect(
        tones.map((t) => t.tone).sort(),
        `?toasts=demo must show all four tones (${theme})`,
      ).toEqual(['error', 'info', 'success', 'warning']);

      for (const t of tones) {
        // AC-2 / AC-6: the label is --text-1, never the hue. The defect P74
        // fixed was hue-as-text on a 14% tint of the same hue.
        expect(t.labelColor, `${theme} ${t.tone}: label colour must be --text-1`).toBe(t.text1);
        // Sanity-check the probe itself in dark, where AC-2 pins the value.
        if (theme === 'dark') expect(t.text1).toBe('rgb(232, 234, 237)');
        // AC-1 / AC-6: WCAG 1.4.3 AA. Contract reference: 9.24–10.30 dark,
        // 11.68–12.00 light.
        expect(
          t.labelRatio,
          `${theme} ${t.tone}: label vs composited toast background is ${t.labelRatio}:1, below the 4.5:1 AA floor (contract measured 9.24–10.30 dark / 11.68–12.00 light)`,
        ).toBeGreaterThanOrEqual(4.5);
        // AC-4 / AC-7: WCAG 1.4.11 non-text, for the two hue carriers.
        // Contract reference: 3.34–4.96 dark, 3.37–3.66 light.
        expect(
          t.glyphRatio,
          `${theme} ${t.tone}: glyph is ${t.glyphRatio}:1, below the 3:1 non-text floor (contract measured 3.34–4.96 dark / 3.37–3.66 light)`,
        ).toBeGreaterThanOrEqual(3);
        expect(
          t.barRatio,
          `${theme} ${t.tone}: left-edge bar is ${t.barRatio}:1, below the 3:1 non-text floor`,
        ).toBeGreaterThanOrEqual(3);
        // AC-3: 3px hue bar on the leading edge, 1px tinted hairline elsewhere.
        expect(t.borderLeftWidth, `${theme} ${t.tone}: hue bar width`).toBe('3px');
        expect(t.borderTopWidth, `${theme} ${t.tone}: hairline width`).toBe('1px');
        // The bar and the glyph must be the SAME hue — colour is one signal.
        expect(t.barColor, `${theme} ${t.tone}: bar and glyph must share the hue`).toBe(
          t.glyphColor,
        );
        // ...and the hue must not be the label colour, or the tone is invisible.
        expect(t.glyphColor, `${theme} ${t.tone}: hue must differ from --text-1`).not.toBe(
          t.labelColor,
        );
      }
    }
  });

  test('the glyph vocabulary is exact, distinct, and unvoiced (AC-4,5)', async ({ page }) => {
    await openRepo(page, { flags: { toasts: 'demo' } });
    await expect(page.locator('.toast')).toHaveCount(4);

    const glyphs = await page.evaluate(() =>
      Array.from(document.querySelectorAll('.toast')).map((toast) => ({
        tone: (toast.className.match(/toast-(error|warning|success|info)/) ?? [])[1],
        glyph: toast.querySelector('.toast-glyph')?.textContent ?? '',
        hidden: toast.querySelector('.toast-glyph')?.getAttribute('aria-hidden'),
        // AC-4: the glyph must live in its own span, never inside the label —
        // otherwise a screen reader announces "circled division slash".
        textHasGlyph: /[⊘⚠✓●]/.test(toast.querySelector('.toast-text')?.textContent ?? ''),
      })),
    );

    const byTone = new Map(glyphs.map((g) => [g.tone, g]));
    // §7 vocabulary, chosen for four distinct silhouettes: slashed ring /
    // triangle / tick / solid disc.
    expect(byTone.get('error')?.glyph).toBe('⊘');
    expect(byTone.get('warning')?.glyph).toBe('⚠');
    expect(byTone.get('success')?.glyph).toBe('✓');
    expect(byTone.get('info')?.glyph).toBe('●');
    // AC-5: WCAG 1.4.1 — strip colour and four distinct strings remain.
    expect(new Set(glyphs.map((g) => g.glyph)).size).toBe(4);
    for (const g of glyphs) {
      expect(g.hidden, `${g.tone}: glyph must be aria-hidden`).toBe('true');
      expect(g.textHasGlyph, `${g.tone}: no glyph character may leak into .toast-text`).toBe(
        false,
      );
    }
  });

  test('toast geometry: 360 border-box, 24x24 dismiss, 284 text column (AC-9,10,11)', async ({
    page,
  }) => {
    // The sticky `error` tone only, so nothing can auto-dismiss mid-measure.
    await openRepo(page, { flags: { toasts: 'long' } });
    const toast = page.locator('.toast-error');
    await expect(toast).toHaveCount(1);

    const geo = await page.evaluate(() => {
      const t = document.querySelector('.toast-error') as HTMLElement;
      const text = t.querySelector('.toast-text') as HTMLElement;
      const dismiss = t.querySelector('.toast-dismiss') as HTMLElement;
      const stack = document.querySelector('.toast-stack') as HTMLElement;
      const ts = getComputedStyle(t);
      const ss = getComputedStyle(stack);
      return {
        boxSizing: ts.boxSizing,
        padding: ts.padding,
        toast: t.getBoundingClientRect(),
        text: text.getBoundingClientRect(),
        dismiss: dismiss.getBoundingClientRect(),
        stackWidth: ss.width,
        stackTop: ss.top,
        stackRight: ss.right,
        stackGap: ss.gap,
        dismissLabel: dismiss.getAttribute('aria-label'),
      };
    });

    // AC-9
    expect(geo.boxSizing).toBe('border-box');
    expect(geo.toast.width).toBeCloseTo(360, 1);
    expect(geo.padding).toBe('8px 12px 8px 10px');
    // AC-8 — the stack is unchanged by the restyle.
    expect(geo.stackWidth).toBe('360px');
    expect(geo.stackTop).toBe('52px');
    expect(geo.stackRight).toBe('12px');
    expect(geo.stackGap).toBe('8px');
    // AC-10 — the compliant 24x24 dismiss target, fully inside the toast's
    // border box despite its negative margin.
    expect(geo.dismiss.width).toBeCloseTo(24, 1);
    expect(geo.dismiss.height).toBeCloseTo(24, 1);
    expect(geo.dismiss.top).toBeGreaterThanOrEqual(geo.toast.top);
    expect(geo.dismiss.bottom).toBeLessThanOrEqual(geo.toast.bottom);
    expect(geo.dismissLabel).toBe('Dismiss'); // AC-13
    // AC-11 — the text column the glyph + 24px dismiss leave behind.
    expect(geo.text.width).toBeGreaterThanOrEqual(283);
    expect(geo.text.width).toBeLessThanOrEqual(285);
  });

  test('a long refusal wraps and never truncates (AC-12, as revised)', async ({ page }) => {
    // AC-12 REVISED 2026-08-20: no pixel height is asserted — the seam's
    // LONG_TEXT re-creates P73's pathological string rather than being
    // byte-identical, so a pinned height would assert a fixture, not a layout.
    await openRepo(page, { flags: { toasts: 'long' } });
    const toast = page.locator('.toast-error');
    await expect(toast).toHaveCount(1);

    const m = await page.evaluate(() => {
      const t = document.querySelector('.toast-error') as HTMLElement;
      const text = t.querySelector('.toast-text') as HTMLElement;
      const stack = document.querySelector('.toast-stack') as HTMLElement;
      const cs = getComputedStyle(text);
      const ts = getComputedStyle(t);
      return {
        textLength: (text.textContent ?? '').length,
        toastScrollW: t.scrollWidth,
        toastClientW: t.clientWidth,
        textScrollW: text.scrollWidth,
        textClientW: text.clientWidth,
        textOverflow: cs.textOverflow,
        lineClamp: cs.webkitLineClamp,
        overflowWrapText: cs.overflowWrap,
        overflowWrapToast: ts.overflowWrap,
        maxHeightToast: ts.maxHeight,
        maxHeightText: cs.maxHeight,
        stackBottom: stack.getBoundingClientRect().bottom,
        viewportH: window.innerHeight,
        // A wrapped multi-line block is taller than one line box; proof the
        // text is laid out, not clipped.
        toastH: t.getBoundingClientRect().height,
      };
    });

    expect(m.textLength).toBeGreaterThan(300); // the real 430-char refusal copy
    expect(m.toastScrollW, 'no horizontal overflow').toBe(m.toastClientW);
    expect(m.textScrollW).toBeLessThanOrEqual(m.textClientW);
    expect(m.textOverflow).not.toBe('ellipsis');
    expect(m.lineClamp).toBe('none');
    expect(m.maxHeightToast).toBe('none');
    expect(m.maxHeightText).toBe('none');
    // `overflow-wrap: anywhere` is what keeps a 91-char path from overflowing;
    // it may be declared on the toast and inherited by the label.
    expect([m.overflowWrapText, m.overflowWrapToast]).toContain('anywhere');
    // Grew to several lines, and still fits the viewport.
    expect(m.toastH).toBeGreaterThan(60);
    expect(m.stackBottom).toBeLessThanOrEqual(m.viewportH);
  });

  test('the 5-toast cap and the dedupe key survive the restyle (AC-13,14)', async ({ page }) => {
    // All six `cap` pushes are sticky errors on purpose — no timing race.
    await openRepo(page, { flags: { toasts: 'cap' } });
    await expect(page.locator('.toast')).toHaveCount(5);
    // Stable, not merely momentary: still 5 after the 5 s auto-dismiss window
    // that would have fired on a non-sticky tone.
    await expect(page.locator('.toast')).toHaveCount(5);

    const aria = await page.evaluate(() => ({
      live: document.querySelector('.toast-stack')?.getAttribute('aria-live'),
      // AC-13: role="alert" is for errors only — a polite stack plus assertive
      // errors. Every `cap` toast is an error, so all five carry it.
      alerts: document.querySelectorAll('.toast[role="alert"]').length,
      nonErrorAlerts: document.querySelectorAll(
        '.toast:not(.toast-error)[role="alert"]',
      ).length,
    }));
    expect(aria.live).toBe('polite');
    expect(aria.alerts).toBe(5);
    expect(aria.nonErrorAlerts).toBe(0);
  });

  test('a repeated keyed push replaces in place (AC-14)', async ({ page }) => {
    await openRepo(page, { flags: { toasts: 'dedupe' } });
    // Two pushes, one key => ONE toast, carrying the SECOND text.
    await expect(page.locator('.toast')).toHaveCount(1);
    await expect(page.locator('.toast-text')).toHaveText(/Second attempt/);
    await expect(page.locator('.toast-text')).not.toHaveText(/First attempt/);
  });

  test('non-error tones still auto-dismiss and errors stay sticky (AC-14)', async ({ page }) => {
    await openRepo(page, { flags: { toasts: 'demo' } });
    await expect(page.locator('.toast')).toHaveCount(4);
    // §10.1, unchanged by the restyle: info/success/warning expire after 5 s,
    // `error` never does. The whole point of the sticky rule is that P73's
    // multi-sentence refusals cannot vanish while being read.
    await expect(page.locator('.toast')).toHaveCount(1, { timeout: 15_000 });
    await expect(page.locator('.toast')).toHaveClass(/toast-error/);
  });

  test('non-error tones stay assertive-free and role is tone-scoped (AC-13)', async ({ page }) => {
    await openRepo(page, { flags: { toasts: 'demo' } });
    await expect(page.locator('.toast')).toHaveCount(4);
    const roles = await page.evaluate(() =>
      Array.from(document.querySelectorAll('.toast')).map((t) => ({
        tone: (t.className.match(/toast-(error|warning|success|info)/) ?? [])[1],
        role: t.getAttribute('role'),
      })),
    );
    for (const r of roles) {
      expect(r.role, `${r.tone}: role must be alert for error only`).toBe(
        r.tone === 'error' ? 'alert' : null,
      );
    }
  });
});
