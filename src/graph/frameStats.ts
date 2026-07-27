/** Frame-timing instrumentation (contract M2-graph.md §4.7). Only active when
 * `import.meta.env.DEV || import.meta.env.VITE_MOCK_IPC === '1'` — callers
 * gate on that check so release paths never record. */

export interface FrameStats {
  frames: number;
  avgMs: number;
  maxMs: number;
  over33: number;
  over100: number;
  /** Max over all windows of 5 consecutive frames of the window's average. */
  maxWindow5Avg: number;
}

export interface FrameRecorder {
  record(durMs: number): void;
  /** Returns the accumulated stats and resets the recorder. */
  flushSummary(): FrameStats;
}

const WINDOW = 5;

function round1(x: number): number {
  return Math.round(x * 10) / 10;
}

export function createFrameRecorder(): FrameRecorder {
  let frames = 0;
  let totalMs = 0;
  let maxMs = 0;
  let over33 = 0;
  let over100 = 0;
  let maxWindowAvg = 0;
  const window: number[] = [];
  let windowSum = 0;

  return {
    record(durMs: number): void {
      frames++;
      totalMs += durMs;
      if (durMs > maxMs) maxMs = durMs;
      if (durMs > 33) over33++;
      if (durMs > 100) over100++;
      window.push(durMs);
      windowSum += durMs;
      if (window.length > WINDOW) windowSum -= window.shift() ?? 0;
      if (window.length === WINDOW && windowSum / WINDOW > maxWindowAvg) {
        maxWindowAvg = windowSum / WINDOW;
      }
    },
    flushSummary(): FrameStats {
      const stats: FrameStats = {
        frames,
        avgMs: frames > 0 ? round1(totalMs / frames) : 0,
        maxMs: round1(maxMs),
        over33,
        over100,
        maxWindow5Avg: round1(maxWindowAvg),
      };
      frames = 0;
      totalMs = 0;
      maxMs = 0;
      over33 = 0;
      over100 = 0;
      maxWindowAvg = 0;
      window.length = 0;
      windowSum = 0;
      return stats;
    },
  };
}

/** Dev hooks exposed on `window.__bonsai` in mock mode only (§4.7/§5.5). */
export interface BonsaiDevHooks {
  /** Animates scrollTop 0 → max → 0 over durationMs, recording every frame
   * delta; logs one `[bonsai] scroll-test {...}` line and resolves. */
  scrollSweep(durationMs?: number): Promise<FrameStats>;
}

declare global {
  interface Window {
    __bonsai?: BonsaiDevHooks;
  }
}
