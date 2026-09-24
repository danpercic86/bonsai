/**
 * P120 part a — mock-only deterministic triggers for the detector proof
 * (e2e/perf/detector.spec.ts):
 *   ?perfInjectShift=<region>  one post-interactive layout shift inside that region
 *   ?perfInjectFocusSteal=1    focus an element, then REMOVE it (never `.blur()` —
 *                              the removal path is what a refresh remount does, A3)
 * Each fires once per page, INJECT_DELAY_MS after the phase turns interactive, so
 * no boot-time input is inside the 500 ms input window.
 */
import { onPhaseChange, phaseAt } from './phase';
import { isRegion } from './regions';
import { INPUT_WINDOW_MS } from './types';

const INJECT_DELAY_MS = INPUT_WINDOW_MS + 250;
const SPACER_PX = 48;

function regionRoot(region: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`[data-obs-region="${region}"]`);
}

/** Insert an in-flow spacer as the region's first child: its siblings move. */
function injectShift(region: string): void {
  const root = regionRoot(region);
  if (root === null) return;
  const spacer = document.createElement('div');
  spacer.dataset.perfInject = 'shift';
  spacer.style.height = `${SPACER_PX}px`;
  spacer.style.flex = `0 0 ${SPACER_PX}px`;
  root.insertBefore(spacer, root.firstChild);
}

/** Focus a throwaway button inside a region, then detach it on the next frame. */
function injectFocusSteal(): void {
  const root = regionRoot('details') ?? regionRoot('graph') ?? document.body;
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.dataset.perfInject = 'focus';
  btn.textContent = 'perf inject';
  btn.style.position = 'absolute';
  btn.style.opacity = '0';
  root.appendChild(btn);
  btn.focus();
  requestAnimationFrame(() => btn.remove());
}

export function installPerfInject(search: string): () => void {
  const q = new URLSearchParams(search);
  const shiftParam = q.get('perfInjectShift');
  const shiftRegion = shiftParam !== null && isRegion(shiftParam) && shiftParam !== 'other' ? shiftParam : null;
  const steal = q.get('perfInjectFocusSteal') === '1';
  if (shiftRegion === null && !steal) return () => {};
  let timer: ReturnType<typeof setTimeout> | null = null;
  let done = false;
  const arm = (): void => {
    if (done) return;
    done = true;
    timer = setTimeout(() => {
      if (shiftRegion !== null) injectShift(shiftRegion);
      if (steal) injectFocusSteal();
    }, INJECT_DELAY_MS);
  };
  const off = onPhaseChange((p) => {
    if (p === 'interactive') arm();
  });
  if (phaseAt(performance.now()) === 'interactive') arm();
  return () => {
    off();
    if (timer !== null) clearTimeout(timer);
  };
}
