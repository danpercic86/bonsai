// P121c — ONE shared wall clock (read-phased, not wall-aligned) for every relative-age label.
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
// cleared on the last unsubscribe. The phase is RELATIVE TO THE FIRST READ, not
// to wall-clock :00/:30 boundaries: a wall-aligned tick could land inside any
// short window (the P120 perf lab measures interactions on a stepped page
// clock, and a boundary tick inside one added a toolbar commit
// nondeterministically). Same shape as the old mount-phase `setInterval`, but
// one chain for every consumer.
//
// Invariants: `subscribe` never changes `current` (no extra render on mount);
// a consumer's value is at most NOW_PERIOD_MS old; StrictMode's
// subscribe/unsubscribe/subscribe leaves exactly one timer.
import { useSyncExternalStore } from 'react';

export const NOW_PERIOD_MS = 30_000;

const listeners = new Set<() => void>();
let timer: ReturnType<typeof setTimeout> | null = null;
/** The value every consumer sees; refreshed by `tick`, or on read when the
 *  chain is idle and it has gone a full period stale. */
let current: number | null = null;

/** Stable between ticks, so getSnapshot is consistent across React's double
 *  reads. Only refreshes while NO chain is running (no mounted consumer could
 *  then disagree with the new value). */
function snapshot(): number {
  const now = Date.now();
  if (current === null || (timer === null && now - current >= NOW_PERIOD_MS)) current = now;
  return current;
}

function arm(delayMs: number): void {
  timer = setTimeout(tick, Math.max(0, delayMs));
}

function tick(): void {
  // This timer has fired: clear the handle first so a mid-notify resubscribe
  // (which may arm its own timer) is not followed by a second, orphaned chain.
  timer = null;
  current = Date.now();
  // Copy first: a listener's re-render may unsubscribe/resubscribe mid-notify.
  for (const cb of Array.from(listeners)) cb();
  if (listeners.size > 0 && timer === null) arm(NOW_PERIOD_MS);
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  if (timer === null) {
    // Fire when `current` becomes a full period old — never touch `current`
    // here (that would re-render the consumer right after mount).
    const age = current === null ? 0 : Date.now() - current;
    arm(NOW_PERIOD_MS - age);
  }
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
  current = null;
  listeners.clear();
}
