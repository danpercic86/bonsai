import { describe, expect, it } from 'vitest';

import { createFrameRecorder } from './frameStats';

describe('createFrameRecorder', () => {
  it('flush with no frames → all zeros', () => {
    expect(createFrameRecorder().flushSummary()).toEqual({
      frames: 0,
      avgMs: 0,
      maxMs: 0,
      over33: 0,
      over100: 0,
      maxWindow5Avg: 0,
    });
  });

  it('counts frames and computes avg/max (rounded to 1 decimal)', () => {
    const r = createFrameRecorder();
    [10, 20, 30].forEach((d) => r.record(d));
    const s = r.flushSummary();
    expect(s.frames).toBe(3);
    expect(s.avgMs).toBe(20);
    expect(s.maxMs).toBe(30);
  });

  it('avg rounds half up at one decimal', () => {
    const r = createFrameRecorder();
    r.record(1);
    r.record(2.11);
    expect(r.flushSummary().avgMs).toBe(1.6); // 1.555 → 1.6
  });

  it('over33 / over100 are STRICT > thresholds (33 and 100 do not count)', () => {
    const r = createFrameRecorder();
    [33, 33.1, 100, 100.1, 5].forEach((d) => r.record(d));
    const s = r.flushSummary();
    expect(s.over33).toBe(3); // 33.1, 100, 100.1
    expect(s.over100).toBe(1); // 100.1
  });

  it('maxWindow5Avg stays 0 with fewer than 5 frames', () => {
    const r = createFrameRecorder();
    [50, 50, 50, 50].forEach((d) => r.record(d));
    expect(r.flushSummary().maxWindow5Avg).toBe(0);
  });

  it('maxWindow5Avg is the max over all sliding windows of 5', () => {
    const r = createFrameRecorder();
    // Windows: [1..5]=3, [2..6]=24 (2,3,4,5,106), ... spike drives the max.
    [1, 2, 3, 4, 5, 106, 1, 1, 1, 1].forEach((d) => r.record(d));
    const s = r.flushSummary();
    expect(s.maxWindow5Avg).toBe(24); // (2+3+4+5+106)/5
    expect(s.maxMs).toBe(106);
  });

  it('exactly 5 uniform frames → window avg equals the frame time', () => {
    const r = createFrameRecorder();
    [16, 16, 16, 16, 16].forEach((d) => r.record(d));
    expect(r.flushSummary().maxWindow5Avg).toBe(16);
  });

  it('flushSummary resets ALL state including the sliding window', () => {
    const r = createFrameRecorder();
    [200, 200, 200, 200, 200].forEach((d) => r.record(d));
    r.flushSummary();
    // After the reset, 4 small frames must not see leftovers from the old window.
    [1, 1, 1, 1].forEach((d) => r.record(d));
    const s = r.flushSummary();
    expect(s).toEqual({ frames: 4, avgMs: 1, maxMs: 1, over33: 0, over100: 0, maxWindow5Avg: 0 });
  });

  it('adversarial: zero and fractional durations are fine', () => {
    const r = createFrameRecorder();
    [0, 0.4, 0.6].forEach((d) => r.record(d));
    const s = r.flushSummary();
    expect(s.frames).toBe(3);
    expect(s.avgMs).toBe(0.3);
    expect(s.maxMs).toBe(0.6);
  });

  it('independent recorders do not share state', () => {
    const a = createFrameRecorder();
    const b = createFrameRecorder();
    a.record(50);
    expect(b.flushSummary().frames).toBe(0);
    expect(a.flushSummary().frames).toBe(1);
  });
});
