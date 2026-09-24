/**
 * P120 §3.2 / A8 — our own input + resize trackers. The API's `hadRecentInput`
 * ignores wheel/scroll, but AC 11 counts a scroll gesture as input. The DOM
 * `scroll` event is deliberately NOT tracked: an app-initiated `scrollIntoView`
 * would otherwise mask a real shift.
 *
 * Rings hold `event.timeStamp` (same origin as `performance.now()`).
 */

const RING = 8;
const INPUT_EVENTS = ['pointerdown', 'keydown', 'wheel', 'touchstart'] as const;

let inputs: number[] = [];
let resizes: number[] = [];

function note(ring: number[], t: number): number[] {
  ring.push(t);
  return ring.length > RING ? ring.slice(-RING) : ring;
}

function within(ring: readonly number[], t: number, ms: number): boolean {
  return ring.some((tau) => tau >= t - ms && tau <= t);
}

/** True when an input landed at τ ∈ [t − ms, t]. */
export function inputWithin(t: number, ms: number): boolean {
  return within(inputs, t, ms);
}

/** True when a resize / dppx change landed at τ ∈ [t − ms, t]. */
export function resizeWithin(t: number, ms: number): boolean {
  return within(resizes, t, ms);
}

export function attachInputTracker(): () => void {
  const onInput = (e: Event): void => {
    inputs = note(inputs, e.timeStamp);
  };
  const opts: AddEventListenerOptions = { capture: true, passive: true };
  for (const t of INPUT_EVENTS) window.addEventListener(t, onInput, opts);
  return () => {
    for (const t of INPUT_EVENTS) window.removeEventListener(t, onInput, opts);
    inputs = [];
  };
}

export function attachResizeTracker(): () => void {
  const onResize = (e: Event): void => {
    resizes = note(resizes, e.timeStamp);
  };
  window.addEventListener('resize', onResize, { passive: true });
  // dppx change (monitor move / zoom): a `(resolution: Xdppx)` query flips once
  // the ratio changes, so re-arm against the new ratio after every change.
  let mql: MediaQueryList | null = null;
  const onDppx = (e: Event): void => {
    resizes = note(resizes, e.timeStamp);
    arm();
  };
  const arm = (): void => {
    mql?.removeEventListener('change', onDppx);
    mql = typeof window.matchMedia === 'function'
      ? window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`)
      : null;
    mql?.addEventListener('change', onDppx);
  };
  arm();
  return () => {
    window.removeEventListener('resize', onResize);
    mql?.removeEventListener('change', onDppx);
    mql = null;
    resizes = [];
  };
}
