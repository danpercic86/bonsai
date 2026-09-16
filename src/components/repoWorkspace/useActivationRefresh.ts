/**
 * Activation self-heal (§7): on every flip TO active AFTER mount, refresh —
 * catches events missed while the tab was `display:none`. The mount run must NOT
 * refresh (RepoWorkspace's initial-load effect already covers first paint).
 *
 * Extracted from `RepoWorkspace.tsx` so the StrictMode behaviour is testable in
 * isolation (see `useActivationRefresh.test.tsx`).
 */
import { useEffect, useRef } from 'react';
import type { RefreshOrigin } from './useCoalescedRefresh';
import type { RefreshScope } from './refreshScope';

type Refresh = (origin: RefreshOrigin, scope: RefreshScope) => Promise<void>;

export function useActivationRefresh(active: boolean, refresh: Refresh): void {
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  // BUGFIX (2026-09-16): this was a "skip the first effect run" latch
  // (`if (!flip.current) { flip.current = true; return; }`), which does the
  // OPPOSITE of skipping the mount run under StrictMode: the simulated remount
  // re-runs the mount effect on the SAME hook instance (same ref object — see
  // `useAiRuns.ts`), so the second pass found the latch set and fired the
  // self-heal on mount. A real Dev-mode log caught it as two `origin: activation`,
  // `scope: full` rounds, both `round: 1`.
  //
  // The guard now tracks the LAST OBSERVED value rather than counting runs, so a
  // repeated run with an unchanged `active` is a no-op however often React
  // re-invokes the effect. Initialised from the first render's `active`, which is
  // what makes the mount pass (and its StrictMode repeat) an equality no-op.
  const seenActiveRef = useRef(active);
  useEffect(() => {
    if (seenActiveRef.current === active) return;
    seenActiveRef.current = active;
    // P81: activation ALWAYS refreshes (never echo-gated) — catches events
    // missed while the tab was display:none. Full scope for the self-heal.
    if (active) void refreshRef.current('activation', 'full');
  }, [active]);
}
