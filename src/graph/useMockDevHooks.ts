/** Mock-mode dev hooks (`window.__bonsai`) — moved VERBATIM out of
 *  GraphCanvas.tsx (spec-004 size split): the programmatic scroll sweep with
 *  frame timing (§4.7) and the P7 pure-helper bag + self-test the orchestrator
 *  drives from the headless harness. Inert outside `VITE_MOCK_IPC=1`. */

import { useEffect } from 'react';
import type { RefObject } from 'react';
import { avatarColor, avatarHit, initials, refColArea } from './geometry';
import { relativeDate } from './dates';
import { groupRefs, layoutRefLabels } from './refLabels';
import { createFrameRecorder } from './frameStats';
import type { FrameStats, P7SelfTestResult } from './frameStats';
import { headGuide } from './viewport';
import { runP7SelfTest } from './selfTest';

const MOCK_MODE = import.meta.env.VITE_MOCK_IPC === '1';

export function useMockDevHooks(
  canvasRef: RefObject<HTMLCanvasElement | null>,
  scrollerRef: RefObject<HTMLDivElement | null>,
): void {
  useEffect(() => {
    if (!MOCK_MODE) return;
    const scrollSweep = (durationMs = 10000): Promise<FrameStats> =>
      new Promise((resolve) => {
        const scroller = scrollerRef.current;
        const rec = createFrameRecorder();
        if (scroller === null) {
          resolve(rec.flushSummary());
          return;
        }
        const maxTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
        const start = performance.now();
        let prev = start;
        const step = (ts: number): void => {
          rec.record(ts - prev);
          prev = ts;
          const t = Math.min(1, (ts - start) / durationMs);
          scroller.scrollTop = (t < 0.5 ? t * 2 : (1 - t) * 2) * maxTop;
          if (t < 1) {
            requestAnimationFrame(step);
          } else {
            const stats = rec.flushSummary();
            if (import.meta.env.DEV) console.log(`[bonsai] scroll-test ${JSON.stringify(stats)}`);
            resolve(stats);
          }
        };
        requestAnimationFrame(step);
      });

    // P7 §10 item 2: expose the pure helpers + a self-test (mock only, mirroring
    // scrollSweep). The orchestrator reads `window.__bonsai.p7SelfTest()`.
    // P67 §1.5: `headGuide` joins the bag — the only way to assert the guideline
    // geometry from a headless pane (no canvas pixel is ever produced there).
    const p7 = { initials, avatarColor, groupRefs, layoutRefLabels, refColArea, avatarHit, relativeDate, headGuide };
    const p7SelfTest = (): P7SelfTestResult => runP7SelfTest(canvasRef.current);

    window.__bonsai = { scrollSweep, p7, p7SelfTest };
    return () => {
      if (window.__bonsai?.scrollSweep === scrollSweep) delete window.__bonsai;
    };
  }, [canvasRef, scrollerRef]);
}
