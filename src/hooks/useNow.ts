// P121c — ONE shared, boundary-aligned wall clock for every relative-age label.
//
// WHY A MODULE STORE. The 30 s ticker used to live in `useJobStatus`, i.e. in
// `RepoWorkspace` state, so every tick re-rendered the whole container in each
// of the (up to 6) mounted workspaces just to advance "Fetched 1m ago" and a few
// stash ages. Reading the clock through `useSyncExternalStore` re-renders ONLY
// the component that calls `useNow()` (a tiny leaf such as `RelativeAge`), and a
// module-level store needs no provider (App.tsx stays untouched and shared rows
// rendered outside a workspace get a working clock for free).
//
// One `setTimeout` chain for the whole app, armed on the first subscriber and
// cleared on the last unsubscribe. Each tick re-arms for the NEXT period
// boundary (not `setInterval`), so the clock never drifts off the boundary.
import { useSyncExternalStore } from 'react';

export const NOW_PERIOD_MS = 30_000;

const listeners = new Set<() => void>();
let timer: ReturnType<typeof setTimeout> | null = null;

/** Current time quantized to NOW_PERIOD_MS boundaries (ms). Stable within a
 *  period, so getSnapshot is consistent across React's double reads. */
function snapshot(): number {
  return Math.floor(Date.now() / NOW_PERIOD_MS) * NOW_PERIOD_MS;
}

function arm(): void {
  timer = setTimeout(tick, NOW_PERIOD_MS - (Date.now() % NOW_PERIOD_MS));
}

function tick(): void {
  // This timer has fired: clear the handle first so a mid-notify resubscribe
  // (which may arm its own timer) is not followed by a second, orphaned chain.
  timer = null;
  // Copy first: a listener's re-render may unsubscribe/resubscribe mid-notify.
  for (const cb of Array.from(listeners)) cb();
  // An early-firing timer is harmless: the snapshot is unchanged, so React bails
  // out, and the re-arm below targets the real boundary.
  if (listeners.size > 0 && timer === null) arm();
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  if (timer === null) arm();
  return () => {
    listeners.delete(cb);
    if (listeners.size === 0 && timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  };
}

/** Epoch ms, re-rendering ONLY the calling component once per period. Label
 *  error ≤ NOW_PERIOD_MS; `relativeDate`'s finest bucket is one minute. */
export function useNow(): number {
  return useSyncExternalStore(subscribe, snapshot);
}

/** Test-only: reset module state between tests. */
export function __resetNowForTests(): void {
  if (timer !== null) clearTimeout(timer);
  timer = null;
  listeners.clear();
}
