// P84 reveal-flash driver — extracted from GraphCanvas so the canvas component
// stays lean. Owns the two motion modes (UI contract §3):
//  - animated: a self-contained rAF loop that repaints each frame until the
//    duration elapses, then paints once more to clear.
//  - reduced-motion (§3.1): NO rAF loop — paint the static overlay once, hold
//    for the duration, then paint once more to clear (two paints total).
// Composites over normal paints (scroll during the flash simply repaints with
// the current alpha) and never blocks input; selection drives scroll-into-view.

import { flashDurationMs } from './revealFlash';

export interface FlashState {
  row: number;
  start: number;
}

export interface FlashRunnerRefs {
  /** Active flash descriptor (read by the paint path); `null` when idle. */
  state: { current: FlashState | null };
  /** Handle of the in-flight rAF, or 0. */
  raf: { current: number };
  /** Handle of the reduced-motion hold timeout, or 0. */
  timeout: { current: number };
}

/**
 * Start (or restart) a reveal flash on `row`. Returns a cleanup that cancels any
 * in-flight rAF/timeout and clears the flash state — call it on unmount or when
 * a new nonce supersedes this flash.
 */
export function startRevealFlash(
  row: number,
  reducedMotion: boolean,
  paint: () => void,
  refs: FlashRunnerRefs,
): () => void {
  refs.state.current = { row, start: performance.now() };
  const duration = flashDurationMs(reducedMotion);

  if (reducedMotion) {
    paint(); // "on" paint
    if (refs.timeout.current !== 0) clearTimeout(refs.timeout.current);
    refs.timeout.current = window.setTimeout(() => {
      refs.state.current = null;
      paint(); // "off" paint
      refs.timeout.current = 0;
    }, duration);
    return () => {
      if (refs.timeout.current !== 0) {
        clearTimeout(refs.timeout.current);
        refs.timeout.current = 0;
      }
      refs.state.current = null;
    };
  }

  const tick = () => {
    const fs = refs.state.current;
    if (fs === null) return;
    const elapsed = performance.now() - fs.start;
    paint();
    if (elapsed >= duration) {
      refs.state.current = null;
      paint(); // final clear paint
      refs.raf.current = 0;
      return;
    }
    refs.raf.current = requestAnimationFrame(tick);
  };
  if (refs.raf.current !== 0) cancelAnimationFrame(refs.raf.current);
  refs.raf.current = requestAnimationFrame(tick);
  return () => {
    if (refs.raf.current !== 0) {
      cancelAnimationFrame(refs.raf.current);
      refs.raf.current = 0;
    }
    refs.state.current = null;
  };
}
