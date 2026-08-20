/**
 * Shared plumbing for the P74 accessibility specs (`26-a11y-toasts.spec.ts`,
 * `27-a11y-hit-targets.spec.ts`). Split out of the single original spec purely
 * to keep both halves under the ~500-line file limit — no assertion, threshold
 * or comment changed in the move.
 *
 * Follows the flat `e2e/helpers.ts` convention: a plain module beside the specs,
 * not a `spec`-matching file, so Playwright never collects it as a test.
 */
import { expect } from '../fixtures';
import type { Page } from '@playwright/test';

const CONTRAST_SETUP = `
  function parseColor(input) {
    const s = String(input).trim();
    if (s === 'transparent') return [0, 0, 0, 0];
    let m = s.match(/^rgba?\\(([^)]+)\\)$/);
    if (m) {
      const p = m[1].split(/[\\s,/]+/).filter(Boolean).map(Number);
      return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1];
    }
    m = s.match(/^color\\(srgb ([^)]+)\\)$/);
    if (m) {
      const p = m[1].split(/[\\s/]+/).filter(Boolean).map(Number);
      return [p[0] * 255, p[1] * 255, p[2] * 255, p.length > 3 ? p[3] : 1];
    }
    throw new Error('unparsed colour: ' + s);
  }
  function over(fg, bg) {
    const a = fg[3];
    if (a >= 1) return [fg[0], fg[1], fg[2], 1];
    return [
      fg[0] * a + bg[0] * (1 - a),
      fg[1] * a + bg[1] * (1 - a),
      fg[2] * a + bg[2] * (1 - a),
      1,
    ];
  }
  /** The COMPOSITED background actually painted behind \`el\` — walks up through
   *  transparent/translucent ancestors, which is mandatory here because the
   *  toast tints are color-mix() layered on --bg-2 inside a fixed stack. */
  function bgOf(el) {
    let acc = null;
    let node = el;
    while (node) {
      const c = parseColor(getComputedStyle(node).backgroundColor);
      if (c[3] > 0) acc = acc === null ? c : over(acc, c);
      if (acc !== null && acc[3] >= 1) return acc;
      node = node.parentElement;
    }
    const page = parseColor(getComputedStyle(document.documentElement).backgroundColor);
    return acc === null ? (page[3] > 0 ? page : [255, 255, 255, 1]) : over(acc, [255, 255, 255, 1]);
  }
  function lum(c) {
    const f = (v) => {
      const x = v / 255;
      return x <= 0.03928 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4);
    };
    return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2]);
  }
  function ratio(fgRaw, el) {
    const bg = bgOf(el);
    const fg = over(parseColor(fgRaw), bg);
    const a = lum(fg);
    const b = lum(bg);
    const hi = Math.max(a, b);
    const lo = Math.min(a, b);
    return Math.round(((hi + 0.05) / (lo + 0.05)) * 100) / 100;
  }
`;

export interface ToneRead {
  tone: string;
  /** `--text-1` as resolved in the theme this read was taken in. */
  text1: string;
  labelColor: string;
  labelRatio: number;
  glyphColor: string;
  glyphRatio: number;
  barColor: string;
  barRatio: number;
  borderLeftWidth: string;
  borderTopWidth: string;
}

/** One synchronous pass over the four tones, per theme — see THEME SWITCHING.
 *  Evaluated as a SOURCE STRING (Playwright accepts an expression) so the
 *  shared contrast helpers above can be spliced in without shipping them as a
 *  page-side module. */
export async function readTones(page: Page): Promise<Record<string, ToneRead[]>> {
  return page.evaluate<Record<string, ToneRead[]>>(`(() => {
      ${CONTRAST_SETUP}
      const readAll = () => Array.from(document.querySelectorAll('.toast')).map((toast) => {
        const text = toast.querySelector('.toast-text');
        const glyph = toast.querySelector('.toast-glyph');
        const ts = getComputedStyle(toast);
        // The token's own resolved value, read through a probe so the assertion
        // never hardcodes a hex per theme (--text-1 is #e8eaed dark / #1c1f24
        // light; the contract only recorded the dark one).
        const probe = document.createElement('span');
        probe.style.color = 'var(--text-1)';
        document.body.appendChild(probe);
        const text1 = getComputedStyle(probe).color;
        probe.remove();
        return {
          text1,
          tone: (toast.className.match(/toast-(error|warning|success|info)/) || [])[1],
          labelColor: getComputedStyle(text).color,
          labelRatio: ratio(getComputedStyle(text).color, toast),
          glyphColor: getComputedStyle(glyph).color,
          glyphRatio: ratio(getComputedStyle(glyph).color, toast),
          barColor: ts.borderLeftColor,
          barRatio: ratio(ts.borderLeftColor, toast),
          borderLeftWidth: ts.borderLeftWidth,
          borderTopWidth: ts.borderTopWidth,
        };
      });
      const html = document.documentElement;
      const before = html.getAttribute('data-theme');
      html.setAttribute('data-theme', 'dark');
      const dark = readAll();
      html.setAttribute('data-theme', 'light');
      const light = readAll();
      if (before === null) html.removeAttribute('data-theme');
      else html.setAttribute('data-theme', before);
      return { dark, light };
    })()`);
}

/** Rect of the first match, or null when the element is absent. */
export async function rect(page: Page, selector: string) {
  return page.evaluate((sel) => {
    const el = document.querySelector(sel);
    if (el === null) return null;
    const r = el.getBoundingClientRect();
    return { w: r.width, h: r.height, left: r.left, top: r.top };
  }, selector);
}

/** `openRepo` resolves on the graph canvas, which paints BEFORE the sidebar's
 *  branch/remote/tag lists and the status tree have landed (measured: the
 *  section toggles and every `.tree-dir-row` are still absent at that point).
 *  Every geometry read below must wait for the rows themselves, or it measures
 *  an empty pane and reports `null`. */
export async function sidebarReady(page: Page): Promise<void> {
  await expect(page.locator('.sidebar-section-toggle').first()).toBeVisible();
  await expect(page.locator('.branch-row').first()).toBeVisible();
  await expect(page.locator('.sidebar .tree-dir-row').first()).toBeVisible();
}

export const FLOOR = 24;
/** Subpixel layout slack — a 24px box can measure 23.99 after fractional
 *  ancestor offsets. Anything genuinely 20px or 18.84px (the P74 "before"
 *  values) is nowhere near this tolerance. */
export const SLACK = 0.5;
