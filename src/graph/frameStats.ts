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

/** P7 §10 item 2: pure helpers exposed for the self-test harness (mock only). */
export interface P7DevHooks {
  initials(name: string): string;
  avatarColor(name: string): import('./draw').AvatarColor;
  groupRefs(refs: readonly import('../ipc').RefLabel[] | undefined): import('./draw').RefEntity[];
  layoutRefLabels(
    ctx: CanvasRenderingContext2D,
    entities: readonly import('./draw').RefEntity[],
    node: import('../ipc').GraphNode,
    theme: import('./colors').Theme,
    startX: number,
    budget: number,
  ): import('./draw').LaidRefLabel[];
  refColArea(m: import('./metrics').EffectiveMetrics): { startX: number; budget: number };
  avatarHit(
    px: number,
    py: number,
    cx: number,
    cy: number,
    m: import('./metrics').EffectiveMetrics,
  ): boolean;
  relativeDate(ts: number, now: number): string;
}

/** P7 §10 item 2: pure-fn self-test result the orchestrator reads. */
export interface P7SelfTestResult {
  pass: number;
  fail: number;
  failures: string[];
}

/** Dev hooks exposed on `window.__bonsai` in mock mode only (§4.7/§5.5). */
export interface BonsaiDevHooks {
  /** Animates scrollTop 0 → max → 0 over durationMs, recording every frame
   * delta; logs one `[bonsai] scroll-test {...}` line and resolves. */
  scrollSweep(durationMs?: number): Promise<FrameStats>;
  /** P7 §10: pure helpers for interactive inspection (mock only). */
  p7?: P7DevHooks;
  /** P7 §10: run all pure-fn assertions; returns pass/fail counts + names. */
  p7SelfTest?(): P7SelfTestResult;
  /** P12 §2.2: run the conflict-region helper assertions (mock/dev only).
   * Registered non-destructively by ConflictEditor's mount effect. */
  conflictSelfTest?(): P7SelfTestResult;
}

declare global {
  interface Window {
    __bonsai?: BonsaiDevHooks;
  }
}
