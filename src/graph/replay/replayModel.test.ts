import { describe, expect, it } from 'vitest';
import type { GraphLayout, GraphNode } from '../../ipc';
import {
  MAX_STEP_MS,
  MIN_STEP_MS,
  MIN_TOTAL_MS,
  buildReplayModel,
  cutoffForPlayhead,
  initialState,
  keyStepRows,
  pause,
  play,
  playheadForCutoff,
  scrubTo,
  setSpeed,
  stepRows,
  tick,
} from './replayModel';
import { PULSE_MS, prunePulses, pulseAlpha, pulseRingRadius } from './replayPulse';

/** Row 0 = newest. `gaps[k]` = seconds between chronological commits k-1 → k. */
function layoutOf(gapsSeconds: number[]): GraphLayout {
  const n = gapsSeconds.length + 1;
  let ts = 1_700_000_000;
  const chrono: number[] = [ts];
  for (const g of gapsSeconds) chrono.push((ts += g));
  const nodes: GraphNode[] = chrono
    .slice()
    .reverse()
    .map((t, row) => ({
      id: String(row).padStart(40, '0'),
      lane: 0,
      parents: row + 1 < n ? [row + 1] : [],
      summary: `c${row}`,
      author: 'a',
      ts: t,
      committerTs: t,
    }));
  return { nodes, edges: [], laneCount: 1, headIndex: 0, truncated: false };
}

describe('buildReplayModel', () => {
  it('builds a monotonic clamped narrative clock normalized to totalMs', () => {
    // Gaps: a burst (0s), a normal gap, and a huge real-world gap.
    const m = buildReplayModel(layoutOf([0, 1, 1_000_000]), false);
    expect(m.n).toBe(4);
    expect(m.canPlay).toBe(true);
    expect(m.cumTime[0]).toBe(0);
    for (let k = 1; k < m.n; k += 1) expect(m.cumTime[k]).toBeGreaterThan(m.cumTime[k - 1]);
    expect(m.cumTime[m.n - 1]).toBeCloseTo(m.totalMs, 6);
    // Clamp: the burst step and the huge step differ by at most MAX/MIN.
    const steps = [m.cumTime[1], m.cumTime[2] - m.cumTime[1], m.cumTime[3] - m.cumTime[2]];
    const ratio = Math.max(...steps) / Math.min(...steps);
    expect(ratio).toBeLessThanOrEqual(MAX_STEP_MS / MIN_STEP_MS + 1e-9);
  });

  it('End reveals row 0 even under inexact fp scaling (T[n-1] pinned to totalMs)', () => {
    // Gap sets chosen so `totalMs / raw` is non-terminating in binary — without
    // the pin, cum[n-1] can land one ulp above totalMs and cutoff sticks at 1.
    for (const gaps of [[1, 1, 1], [7, 13, 29, 3], [0, 61, 3600, 5]]) {
      const m = buildReplayModel(layoutOf(gaps), false);
      expect(m.cumTime[m.n - 1]).toBe(m.totalMs); // exact, not toBeCloseTo
      expect(cutoffForPlayhead(m, m.totalMs)).toBe(0);
      const end = scrubTo(m, initialState(m), 1);
      expect(end.cutoff).toBe(0);
      expect(end.status).toBe('finished');
    }
  });

  it('empty layout: canPlay false, cutoff stays 0-ranged', () => {
    const m = buildReplayModel({ ...layoutOf([]), nodes: [], headIndex: null }, false);
    expect(m.n).toBe(0);
    expect(m.canPlay).toBe(false);
    const s = initialState(m);
    expect(s.cutoff).toBe(0);
    expect(play(m, s)).toBe(s); // structural no-op
    expect(scrubTo(m, s, 0.5).status).toBe('paused'); // never 'finished' on n=0
  });

  it('single commit: reveals at playhead 0, totalMs floor applies', () => {
    const m = buildReplayModel(layoutOf([]), false);
    expect(m.n).toBe(1);
    expect(m.totalMs).toBe(MIN_TOTAL_MS);
    expect(cutoffForPlayhead(m, 0)).toBe(0); // T[0]=0, <= search → revealed
  });

  it('reduced motion: canPlay false and play() is a structural no-op', () => {
    const m = buildReplayModel(layoutOf([1, 1]), true);
    expect(m.canPlay).toBe(false);
    const s = initialState(m);
    expect(play(m, s)).toBe(s);
    // Scrubbing still works — the scrubber is the only control (§6).
    expect(scrubTo(m, s, 1).cutoff).toBe(0);
  });
});

describe('tick / play / pause', () => {
  const m = buildReplayModel(layoutOf(Array.from({ length: 99 }, () => 60)), false);

  it('cutoff decreases monotonically under tick and lands finished at the end', () => {
    let s = play(m, initialState(m));
    expect(s.status).toBe('playing');
    let prev = s.cutoff;
    for (let i = 0; i < 200 && s.status === 'playing'; i += 1) {
      s = tick(m, s, m.totalMs / 100);
      expect(s.cutoff).toBeLessThanOrEqual(prev);
      prev = s.cutoff;
    }
    expect(s.status).toBe('finished');
    expect(s.cutoff).toBe(0);
    expect(s.playheadMs).toBe(m.totalMs);
  });

  it('tick is inert unless playing; pause only affects playing', () => {
    const s = initialState(m);
    expect(tick(m, s, 1000)).toBe(s);
    expect(pause(s)).toBe(s);
    const finished = scrubTo(m, s, 1);
    expect(tick(m, finished, 1000)).toBe(finished);
  });

  it('speed multiplies the clock advance seamlessly mid-play', () => {
    let a = play(m, initialState(m));
    a = tick(m, a, 1000);
    let b = setSpeed(a, 2);
    b = tick(m, b, 500);
    const c = tick(m, a, 1000); // 1× for the same wall time
    expect(b.playheadMs).toBeCloseTo(c.playheadMs, 6);
  });

  it('play from finished restarts from the blank state (Replay again)', () => {
    const done = scrubTo(m, initialState(m), 1);
    expect(done.status).toBe('finished');
    const again = play(m, setSpeed(done, 4));
    expect(again.status).toBe('playing');
    expect(again.cutoff).toBe(m.n);
    expect(again.playheadMs).toBe(0);
    expect(again.speed).toBe(4); // speed survives the restart
    expect(again.follow).toBe(true);
  });
});

describe('scrub / step mapping', () => {
  const m = buildReplayModel(layoutOf(Array.from({ length: 49 }, (_, i) => (i % 7) * 30)), false);

  it('scrubTo maps fraction → playhead → cutoff consistently (round-trip)', () => {
    for (const f of [0, 0.1, 0.33, 0.5, 0.99, 1]) {
      const s = scrubTo(m, initialState(m), f);
      expect(s.playheadMs).toBeCloseTo(f * m.totalMs, 6);
      expect(s.cutoff).toBe(cutoffForPlayhead(m, s.playheadMs));
      // Round-trip: the cutoff's own narrative time never exceeds the playhead.
      expect(playheadForCutoff(m, s.cutoff)).toBeLessThanOrEqual(s.playheadMs + 1e-9);
    }
  });

  it('scrubbing while playing pauses; scrubbing to the end finishes', () => {
    const playing = play(m, initialState(m));
    expect(scrubTo(m, playing, 0.5).status).toBe('paused');
    expect(scrubTo(m, playing, 1).status).toBe('finished');
    expect(scrubTo(m, playing, 2).playheadMs).toBe(m.totalMs); // clamped
    expect(scrubTo(m, playing, -1).playheadMs).toBe(0);
  });

  it('stepRows is row-exact, clamped, and snaps the playhead', () => {
    const s0 = initialState(m); // cutoff = n (blank)
    const s1 = stepRows(m, s0, 3);
    expect(s1.cutoff).toBe(m.n - 3);
    expect(s1.status).toBe('paused');
    expect(s1.playheadMs).toBe(playheadForCutoff(m, s1.cutoff));
    expect(stepRows(m, s1, -1000).cutoff).toBe(m.n); // clamp high
    const end = stepRows(m, s1, 1000); // clamp low → full reveal
    expect(end.cutoff).toBe(0);
    expect(end.status).toBe('finished');
  });

  it('keyStepRows: max(1, n/500)', () => {
    expect(keyStepRows(10)).toBe(1);
    expect(keyStepRows(500)).toBe(1);
    expect(keyStepRows(100_000)).toBe(200);
  });
});

describe('frontier pulse math', () => {
  it('rises to the themed peak then eases out to 0 by 600ms', () => {
    expect(pulseAlpha(0, true)).toBe(0);
    expect(pulseAlpha(90, true)).toBeCloseTo(0.3, 6);
    expect(pulseAlpha(90, false)).toBeCloseTo(0.24, 6);
    expect(pulseAlpha(45, true)).toBeCloseTo(0.15, 6);
    expect(pulseAlpha(300, true)).toBeLessThan(0.3);
    expect(pulseAlpha(PULSE_MS, true)).toBe(0);
    expect(pulseAlpha(PULSE_MS + 1, false)).toBe(0);
  });

  it('ring grows +1 → +5 over the pulse; prune drops expired pulses', () => {
    expect(pulseRingRadius(0, 10)).toBe(11);
    expect(pulseRingRadius(PULSE_MS, 10)).toBe(15);
    const pulses = [
      { row: 1, start: 0 },
      { row: 2, start: 500 },
    ];
    expect(prunePulses(pulses, 100)).toBe(pulses); // oldest live → identity
    expect(prunePulses(pulses, 700)).toEqual([{ row: 2, start: 500 }]);
    expect(prunePulses([], 0)).toEqual([]);
  });
});
