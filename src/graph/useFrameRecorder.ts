/** Frame-timing recorder for `GraphCanvas` — the paint/gap `frameStats`
 *  recorders plus the periodic `[bonsai] frames` summary log. Moved verbatim
 *  out of `GraphCanvas.tsx` (file-size ratchet).
 *
 *  `recordFrame` is a `useCallback(..., [])`, so its identity is stable for the
 *  component's lifetime exactly as before — the paint callbacks that list it in
 *  their dep arrays are recreated no more often than they were. */
import { useCallback, useRef } from 'react';
import { newGapRecorder, newPaintRecorder } from './graphObs';

/** Log a `[bonsai] frames` summary every this many recorded frames. */
const LOG_EVERY = 120;

export function useFrameRecorder() {
  const paintRecorderRef = useRef(newPaintRecorder());
  const paintCountRef = useRef(0);
  const gapRecorderRef = useRef(newGapRecorder());
  const gapCountRef = useRef(0);

  const recordFrame = useCallback((kind: 'paint' | 'gap', durMs: number) => {
    const rec = kind === 'paint' ? paintRecorderRef.current : gapRecorderRef.current;
    const countRef = kind === 'paint' ? paintCountRef : gapCountRef;
    rec.record(durMs);
    if (++countRef.current >= LOG_EVERY) {
      countRef.current = 0;
      const s = rec.flushSummary();
      if (import.meta.env.DEV) {
        console.log(
          `[bonsai] frames kind=${kind} n=${s.frames} avg=${s.avgMs.toFixed(1)}ms ` +
            `max=${s.maxMs.toFixed(1)}ms >33ms=${s.over33}`,
        );
      }
    }
  }, []);

  return { recordFrame };
}
