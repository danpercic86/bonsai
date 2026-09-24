/**
 * P119-ui §3.1 — hold a busy flag for a minimum visible span.
 *
 * `true` on the rising edge of `busy`; after `busy` falls it stays `true` until
 * `minMs` have passed since that rising edge. A 50 ms refresh therefore renders
 * one full 600 ms turn of the Refresh glyph instead of a 30° twitch. Pure timing
 * — no IPC. Cosmetic only: callers keep gating INPUT on the raw flag.
 */
import { useEffect, useRef, useState } from 'react';

export function useMinimumBusy(busy: boolean, minMs: number): boolean {
  const startedAt = useRef<number | null>(null);
  const [holding, setHolding] = useState(false);

  useEffect(() => {
    if (busy) {
      startedAt.current = Date.now();
      setHolding(true);
      return undefined;
    }
    const start = startedAt.current;
    startedAt.current = null;
    if (start === null) return undefined;
    const remaining = minMs - (Date.now() - start);
    if (remaining <= 0) {
      setHolding(false);
      return undefined;
    }
    const id = setTimeout(() => setHolding(false), remaining);
    return () => clearTimeout(id);
  }, [busy, minMs]);

  return busy || holding;
}
