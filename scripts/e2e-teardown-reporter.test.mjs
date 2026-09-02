/**
 * P104 — the teardown reporter's two safety properties, which are the only
 * places it could do harm:
 *   1. it must NEVER announce teardown while a test is still executing
 *      (a slow trailing test is not a teardown stall);
 *   2. its timer must be unref'd, or the diagnostic becomes the very bug it
 *      exists to report (a handle that keeps the runner alive).
 * Plus the happy path: a stall past the grace window is narrated, a fast
 * teardown stays silent.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import TeardownReporter from './e2e-teardown-reporter.mjs';

/** A minimal stand-in for Playwright's `Suite`. */
const suiteOf = (n) => ({ allTests: () => Array.from({ length: n }, (_, i) => ({ id: i })) });

function drive(reporter, n) {
  reporter.onBegin({ workers: 4 }, suiteOf(n));
  for (let i = 0; i < n; i++) {
    reporter.onTestBegin();
    reporter.onTestEnd();
  }
}

describe('e2e teardown reporter', () => {
  let logs;

  beforeEach(() => {
    vi.useFakeTimers();
    logs = [];
    vi.spyOn(console, 'log').mockImplementation((line) => logs.push(String(line)));
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('says nothing when teardown finishes inside the grace window', () => {
    const r = new TeardownReporter();
    drive(r, 3);
    vi.advanceTimersByTime(5_000);
    r.onEnd();
    expect(logs).toEqual([]);
  });

  it('names the phase, then ticks, once the stall passes the grace window', () => {
    const r = new TeardownReporter();
    drive(r, 3);

    vi.advanceTimersByTime(14_000);
    expect(logs).toEqual([]);

    vi.advanceTimersByTime(2_000); // past the 15s grace
    expect(logs.join('\n')).toContain('all 3 results are in');
    expect(logs.join('\n')).toContain('teardown, not a hung test');
    const afterFirst = logs.length;

    vi.advanceTimersByTime(14_000); // inside the 15s re-announce window
    expect(logs.length).toBe(afterFirst);

    vi.advanceTimersByTime(2_000);
    expect(logs.at(-1)).toContain('still tearing down');

    r.onEnd();
    expect(logs.at(-1)).toContain('teardown finished');
  });

  it('never announces teardown while a test is still running', () => {
    const r = new TeardownReporter();
    r.onBegin({ workers: 4 }, suiteOf(2));
    r.onTestBegin();
    r.onTestBegin();
    r.onTestEnd(); // one result in, one test still executing
    vi.advanceTimersByTime(120_000);
    expect(logs).toEqual([]);
  });

  it('re-arms rather than double-fires when a retry starts after the last result', () => {
    const r = new TeardownReporter();
    drive(r, 2);
    vi.advanceTimersByTime(20_000);
    const stalled = logs.length;
    expect(stalled).toBeGreaterThan(0);

    r.onTestBegin(); // a retry begins — this is no longer teardown
    vi.advanceTimersByTime(120_000);
    expect(logs.length).toBe(stalled);
  });

  it('holds no handle that could keep the runner alive', () => {
    const unrefs = [];
    const realSetInterval = globalThis.setInterval;
    vi.spyOn(globalThis, 'setInterval').mockImplementation((fn, ms) => {
      const t = realSetInterval(fn, ms);
      const spy = { unref: vi.fn(() => t) };
      unrefs.push(spy);
      return Object.assign(t, spy);
    });
    const r = new TeardownReporter();
    drive(r, 1);
    expect(unrefs).toHaveLength(1);
    expect(unrefs[0].unref).toHaveBeenCalledTimes(1);
    r.onEnd();
  });

  it('is fully silent under E2E_TEARDOWN_REPORTER=0', () => {
    vi.stubEnv('E2E_TEARDOWN_REPORTER', '0');
    const r = new TeardownReporter();
    drive(r, 1);
    vi.advanceTimersByTime(120_000);
    r.onEnd();
    expect(logs).toEqual([]);
    vi.unstubAllEnvs();
  });
});
