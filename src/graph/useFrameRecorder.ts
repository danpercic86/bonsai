/** Frame-timing recorder for `GraphCanvas` — the paint/gap `frameStats`
 *  recorders plus the periodic `[bonsai] frames` summary log. Moved verbatim
 *  out of `GraphCanvas.tsx` (file-size ratchet).
 *
 *  `recordFrame` is a `useCallback(..., [])`, so its identity is stable for the
 *  component's lifetime exactly as before — the paint callbacks that list it in
 *  their dep arrays are recreated no more often than they were. */
import { useCallback, useRef } from 'react';
import { newGapRecorder, newPaintRecorder } from './graphObs';

/** Log a `[bonsai] frames` summary every this many recorded frames.
 *
 *  P122e — this is ALSO the `frame` log-record cadence: `flushSummary()` is what
 *  fires the recorder's `onWindow` (`graphObs.ts`), so one record per dimension
 *  per GraphCanvas instance per 120 recorded frames. `paint` frames come from
 *  canvas repaints (scroll, data refresh, hover); `gap` frames only while
 *  scrolling. An hour of mostly-idle use (session `s229d042a`: one gesture —
 *  a fetch click — though graph scrolling is not a logged gesture, so a short
 *  scroll of <120 frames is not ruled out) therefore yields ~1 record, and that
 *  is expected, not a wiring fault. Known limit: the record's `ts` is the flush time, so `jank-trace` may
 *  attribute a window's `worstMs` to traces open up to 120 paints later. */
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
