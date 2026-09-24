// @vitest-environment happy-dom
/** P120 T-F7 (coalescing + levels) and T-F13 (no layout reads) for the dev sink. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MockInstance } from 'vitest';
import { logRecord } from '../log';
import { createDevSink } from './devSink';
import type { DevSink } from './devSink';
import { markBoot, markInteractive, resetPhaseForTests } from './phase';
import { BOOT_BUCKET_MAX_MS, SHIFT_BUCKET_MS } from './types';
import type { ShiftEvent } from './types';

vi.mock('../log', () => ({ logRecord: vi.fn() }));
const logged = vi.mocked(logRecord);

let dev: DevSink;
let layoutReads: MockInstance[] = [];

function shift(p: Partial<ShiftEvent> & { at: number }): ShiftEvent {
  return { type: 'shift', region: 'graph', phase: 'interactive', hadRecentInput: false, resizing: false, value: 0.01, ...p };
}

beforeEach(() => {
  resetPhaseForTests(); // clears phase listeners — must precede createDevSink()
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  logged.mockClear();
  layoutReads = [vi.spyOn(Element.prototype, 'getBoundingClientRect'), vi.spyOn(window, 'getComputedStyle')];
  dev = createDevSink();
});

afterEach(() => {
  // T-F13: the dev sink never reads layout.
  for (const spy of layoutReads) expect(spy).not.toHaveBeenCalled();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('dev sink coalescing (T-F7)', () => {
  it('3 interactive unprompted shifts in one region within 1 s → ONE info record, shifts:3', () => {
    markInteractive();
    dev.sink(shift({ at: 1000, value: 0.01 }));
    dev.sink(shift({ at: 1200, value: 0.02 }));
    dev.sink(shift({ at: 1412, value: 0.01234 }));
    expect(logged).not.toHaveBeenCalled();
    vi.advanceTimersByTime(SHIFT_BUCKET_MS);
    expect(logged).toHaveBeenCalledTimes(1);
    expect(logged).toHaveBeenCalledWith({
      kind: 'layout.shift', lvl: 'info', region: 'graph', phase: 'interactive', hadRecentInput: false,
      resizing: false, shifts: 3, value: 0.0423, firstAt: 1000, spanMs: 412,
    });
  });

  it('prompted / resizing buckets never mix with unprompted ones and log at debug', () => {
    markInteractive();
    dev.sink(shift({ at: 1000 }));
    dev.sink(shift({ at: 1001, hadRecentInput: true }));
    dev.sink(shift({ at: 1002, resizing: true }));
    dev.sink(shift({ at: 1003, region: 'sidebar' }));
    vi.advanceTimersByTime(SHIFT_BUCKET_MS);
    const recs = logged.mock.calls.map(([r]) => r);
    expect(recs).toHaveLength(4);
    expect(recs.map((r) => r.lvl)).toEqual(['info', 'debug', 'debug', 'info']);
  });

  it('boot shifts → one debug record per key at markInteractive (the boot summary)', () => {
    dev.sink(shift({ at: 100, phase: 'boot' }));
    dev.sink(shift({ at: 300, phase: 'boot' }));
    dev.sink(shift({ at: 350, phase: 'boot', region: 'details' }));
    vi.advanceTimersByTime(SHIFT_BUCKET_MS * 5);
    expect(logged).not.toHaveBeenCalled();
    markInteractive();
    const recs = logged.mock.calls.map(([r]) => r);
    expect(recs).toEqual([
      expect.objectContaining({ kind: 'layout.shift', lvl: 'debug', region: 'graph', phase: 'boot', shifts: 2, spanMs: 200 }),
      expect.objectContaining({ kind: 'layout.shift', lvl: 'debug', region: 'details', phase: 'boot', shifts: 1, spanMs: 0 }),
    ]);
    // The summary is flushed once: a later markBoot/markInteractive cycle emits nothing more.
    markBoot();
    markInteractive();
    expect(logged).toHaveBeenCalledTimes(2);
  });

  it('a boot bucket that never sees markInteractive closes after BOOT_BUCKET_MAX_MS', () => {
    dev.sink(shift({ at: 100, phase: 'boot' }));
    vi.advanceTimersByTime(BOOT_BUCKET_MAX_MS - 1);
    expect(logged).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(logged).toHaveBeenCalledTimes(1);
  });

  it('flushAll emits every open bucket once and clears its timers', () => {
    dev.sink(shift({ at: 100, phase: 'boot' }));
    dev.sink(shift({ at: 200 }));
    dev.flushAll();
    expect(logged).toHaveBeenCalledTimes(2);
    vi.advanceTimersByTime(BOOT_BUCKET_MAX_MS);
    markInteractive();
    expect(logged).toHaveBeenCalledTimes(2);
  });

  it('focus-lost → one info focus.lost record, lostAt = event time', () => {
    dev.sink({ type: 'focus-lost', fromRegion: 'details', phase: 'interactive', via: 'detached', at: 5000 });
    expect(logged).toHaveBeenCalledWith({
      kind: 'focus.lost', lvl: 'info', fromRegion: 'details', phase: 'interactive', via: 'detached', lostAt: 5000,
    });
  });
});
