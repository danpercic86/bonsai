// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { attachInputTracker } from './input';
import { attachLayoutShift, layoutShiftSupported } from './layoutShift';
import { markInteractive, resetPhaseForTests } from './phase';
import { FakePerformanceObserver, shiftEntry } from './testFakes';
import type { StabilityEvent } from './types';

let now = 0;
function latest(): FakePerformanceObserver {
  const o = FakePerformanceObserver.instances[FakePerformanceObserver.instances.length - 1];
  if (o === undefined) throw new Error('no observer attached');
  return o;
}

beforeEach(() => {
  resetPhaseForTests();
  FakePerformanceObserver.instances = [];
  FakePerformanceObserver.supportedEntryTypes = ['layout-shift'];
  now = 1000;
  vi.spyOn(performance, 'now').mockImplementation(() => now);
  vi.stubGlobal('PerformanceObserver', FakePerformanceObserver);
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

describe('unsupported path (T-F3, WebKit)', () => {
  it('no PerformanceObserver → noop, never throws', () => {
    vi.stubGlobal('PerformanceObserver', undefined);
    expect(layoutShiftSupported()).toBe(false);
    const off = attachLayoutShift(() => {}, true);
    expect(() => off()).not.toThrow();
  });

  it("supportedEntryTypes without 'layout-shift' → installs nothing", () => {
    FakePerformanceObserver.supportedEntryTypes = ['paint', 'longtask'];
    expect(layoutShiftSupported()).toBe(false);
    attachLayoutShift(() => {}, true);
    expect(FakePerformanceObserver.instances).toHaveLength(0);
  });
});

describe('capture', () => {
  it('buffered only on the first attach while booting; later attaches drop stale entries (T-F4, A7)', () => {
    const got: StabilityEvent[] = [];
    const off1 = attachLayoutShift((e) => got.push(e), true);
    expect(latest().options).toEqual({ type: 'layout-shift', buffered: true });
    latest().deliver([shiftEntry({ startTime: 10 })]); // buffered boot replay kept
    expect(got).toHaveLength(1);
    off1();
    expect(FakePerformanceObserver.instances[0]?.disconnected).toBe(true);

    now = 5000;
    attachLayoutShift((e) => got.push(e), false);
    expect(latest().options).toEqual({ type: 'layout-shift', buffered: false });
    latest().deliver([shiftEntry({ startTime: 4000 }), shiftEntry({ startTime: 5100 })]);
    expect(got.map((e) => e.at - Math.round(performance.timeOrigin))).toEqual([10, 5100]);
  });

  it('first attach after interactive is not buffered', () => {
    markInteractive();
    attachLayoutShift(() => {}, true);
    expect(latest().options?.buffered).toBe(false);
  });

  it('hadRecentInput ORs the API flag with the wheel tracker; phase from startTime (T-F5, A6/A8)', () => {
    const detachInput = attachInputTracker();
    now = 100;
    const got: StabilityEvent[] = [];
    attachLayoutShift((e) => got.push(e), true);
    now = 2000;
    markInteractive();
    const wheel = new Event('wheel');
    Object.defineProperty(wheel, 'timeStamp', { value: 2500 });
    window.dispatchEvent(wheel);
    latest().deliver([
      shiftEntry({ startTime: 1500 }), // before interactive → boot
      shiftEntry({ startTime: 2700 }), // wheel 200 ms earlier → prompted
      shiftEntry({ startTime: 3200, hadRecentInput: true }), // API flag
      shiftEntry({ startTime: 4000 }), // unprompted
    ]);
    detachInput();
    expect(got.map((e) => (e.type === 'shift' ? [e.phase, e.hadRecentInput] : null))).toEqual([
      ['boot', false],
      ['interactive', true],
      ['interactive', true],
      ['interactive', false],
    ]);
  });

  it('two sources in two regions → two events with the same whole-entry value (A9)', () => {
    document.body.innerHTML =
      '<aside data-obs-region="sidebar"><p id="a"></p></aside><main data-obs-region="graph"><p id="b"></p><p id="c"></p></main>';
    const got: StabilityEvent[] = [];
    attachLayoutShift((e) => got.push(e), true);
    const node = (id: string): Node | null => document.getElementById(id);
    latest().deliver([
      shiftEntry({ startTime: 1200, value: 0.25, sources: [{ node: node('a') }, { node: node('b') }, { node: node('c') }] }),
      shiftEntry({ startTime: 1300, value: Number.NaN, sources: [] }),
    ]);
    expect(got.map((e) => (e.type === 'shift' ? [e.region, e.value] : null))).toEqual([
      ['sidebar', 0.25],
      ['graph', 0.25],
      ['other', 0],
    ]);
  });
});
