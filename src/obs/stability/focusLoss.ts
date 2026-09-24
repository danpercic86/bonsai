/**
 * P120 §3.6 — unprompted focus-loss classifier (all OSes).
 *
 * Two triggers (A3): `focusout` with no `relatedTarget`, and a MutationObserver
 * noticing the tracked element was detached (Chromium does not reliably fire
 * `focusout` on removal). Either way the verdict is taken FOCUS_SETTLE_MS later,
 * so a dialog that autofocuses in `useEffect` (after paint) is not a loss.
 *
 * Reads only `isConnected`, `document.activeElement`, `hasFocus()` and
 * `visibilityState` — never layout.
 */
import { inputWithin } from './input';
import { phaseAt } from './phase';
import { regionOf } from './regions';
import { FOCUS_SETTLE_MS, INPUT_WINDOW_MS } from './types';
import type { FocusVia, StabilityRegion, StabilitySink } from './types';

interface Tracked {
  el: Element;
  region: StabilityRegion;
}

export function attachFocusLoss(emit: StabilitySink): () => void {
  let tracked: Tracked | null = null;
  const checking = new WeakSet<Element>();
  const timers = new Set<ReturnType<typeof setTimeout>>();
  let windowBlurAt = Number.NEGATIVE_INFINITY;

  const classify = (t: Tracked, at: number, via0: FocusVia): void => {
    const active = document.activeElement;
    const a = active === null || active === document.body;
    const w =
      document.hasFocus() && document.visibilityState === 'visible' && windowBlurAt < at - INPUT_WINDOW_MS;
    const i = inputWithin(at, INPUT_WINDOW_MS);
    const via: FocusVia = t.el.isConnected ? via0 : 'detached';
    if (a && w && !i) {
      emit({
        type: 'focus-lost',
        fromRegion: t.region,
        phase: phaseAt(at),
        via,
        at: Math.round(performance.timeOrigin + at),
      });
    }
    // The element lost focus either way; forget it so nothing double-reports.
    if ((a || !t.el.isConnected) && tracked === t) tracked = null;
    checking.delete(t.el);
  };

  const schedule = (via0: FocusVia): void => {
    const t = tracked;
    if (t === null || checking.has(t.el)) return;
    const at = performance.now();
    checking.add(t.el);
    const id = setTimeout(() => {
      timers.delete(id);
      classify(t, at, via0);
    }, FOCUS_SETTLE_MS);
    timers.add(id);
  };

  const onFocusIn = (e: FocusEvent): void => {
    const target = e.target;
    if (target instanceof Element && target !== document.body) {
      tracked = { el: target, region: regionOf(target) };
    }
  };
  const onFocusOut = (e: FocusEvent): void => {
    if (tracked !== null && tracked.el === e.target && e.relatedTarget === null) schedule('focusout');
  };
  const onWindowBlur = (): void => {
    windowBlurAt = performance.now();
  };
  // O(1) per callback: one `isConnected` read.
  const mo = new MutationObserver(() => {
    if (tracked !== null && !tracked.el.isConnected) schedule('detached');
  });

  document.addEventListener('focusin', onFocusIn, true);
  document.addEventListener('focusout', onFocusOut, true);
  window.addEventListener('blur', onWindowBlur);
  mo.observe(document.body, { childList: true, subtree: true });

  return () => {
    document.removeEventListener('focusin', onFocusIn, true);
    document.removeEventListener('focusout', onFocusOut, true);
    window.removeEventListener('blur', onWindowBlur);
    mo.disconnect();
    for (const id of timers) clearTimeout(id);
    timers.clear();
    tracked = null;
  };
}
