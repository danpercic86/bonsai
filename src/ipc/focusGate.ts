/** P124d — WebView2 delivers `onFocusChanged(true)` twice within ~22–47 ms, and
 *  each delivery ran a full refresh round. Shared by the Tauri adapter and the
 *  mock so both fire at most once per focus episode. A `false` may or may not
 *  interleave the duplicates, so an edge trigger alone is insufficient; a
 *  leading-edge time window is robust. */

/** Duplicate focus deliveries closer than this collapse into one. A genuine
 *  refocus sooner than this after the last accepted one is intentionally merged. */
export const FOCUS_COALESCE_MS = 250;

/** Leading-edge gate. The returned predicate is true for the first
 *  `focused === true` and false for any further `true` within `windowMs` of the
 *  last ACCEPTED one. `false` inputs return false and do NOT reset the window.
 *  Pure apart from `now`. */
export function createFocusGate(
  windowMs: number = FOCUS_COALESCE_MS,
  // Monotonic: a wall-clock step backwards (NTP, VM resume) must not wedge the gate.
  now: () => number = () => performance.now(),
): (focused: boolean) => boolean {
  let lastAccepted: number | null = null;
  return (focused: boolean): boolean => {
    if (!focused) return false;
    const t = now();
    if (lastAccepted !== null && t - lastAccepted < windowMs) return false;
    lastAccepted = t;
    return true;
  };
}
