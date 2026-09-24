// @vitest-environment happy-dom
/** P120 T-F9 — turning Dev mode off flushes the open buckets ONCE, before the
 *  sink detaches, so the sink is clean for a re-enable (no stale bucket merges
 *  into the next session). `logRecord` is mocked here so the flush itself is
 *  observable; for real, `obsEnabled()` is already false at that point and the
 *  obs gate drops those records BY DESIGN — Rust closes its session on disable
 *  (`src-tauri/src/obs/mod.rs` `apply_dev_settings`). `install.test.ts` pins
 *  that nothing is enqueued. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DevSettings } from '../../ipc/types/settings';
import { configureObs, resetObsConfigForTests } from '../enabled';
import { logRecord } from '../log';
import { startStability, stopStability } from './install';
import { resetPhaseForTests } from './phase';
import { resetSinksForTests, stabilitySinkCountForTests } from './sinks';
import { FakePerformanceObserver, shiftEntry } from './testFakes';

vi.mock('../log', () => ({ logRecord: vi.fn() }));
const logged = vi.mocked(logRecord);

const DEV_ON: DevSettings = {
  enabled: true,
  level: 'trace',
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

beforeEach(() => {
  resetPhaseForTests();
  resetSinksForTests();
  resetObsConfigForTests();
  logged.mockClear();
  FakePerformanceObserver.instances = [];
  FakePerformanceObserver.supportedEntryTypes = ['layout-shift'];
  vi.stubGlobal('PerformanceObserver', FakePerformanceObserver);
});

afterEach(() => {
  stopStability();
  resetObsConfigForTests();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('Dev mode off → flush, then detach (T-F9)', () => {
  it('flushes each open bucket exactly once, then the sink count is 0', () => {
    startStability();
    configureObs(DEV_ON);
    const po = FakePerformanceObserver.instances[0];
    po?.deliver([shiftEntry({ startTime: 10 }), shiftEntry({ startTime: 20 })]);
    expect(logged).not.toHaveBeenCalled();
    let countAtFlush = -1;
    logged.mockImplementation(() => {
      countAtFlush = stabilitySinkCountForTests();
    });
    configureObs(null);
    expect(logged).toHaveBeenCalledTimes(1);
    expect(logged.mock.calls[0]?.[0]).toMatchObject({ kind: 'layout.shift', shifts: 2 });
    expect(countAtFlush).toBe(1); // flushed BEFORE unregistering
    expect(stabilitySinkCountForTests()).toBe(0);
    expect(po?.disconnected).toBe(true);
    // Nothing is left to flush on a later pagehide or re-disable.
    window.dispatchEvent(new Event('pagehide'));
    configureObs(null);
    expect(logged).toHaveBeenCalledTimes(1);
    // Re-enable starts from an empty bucket: the next flush carries only the new
    // shift (a re-attach is unbuffered, so the entry must post-date it).
    configureObs(DEV_ON);
    const later = performance.now() + 1000;
    FakePerformanceObserver.instances.at(-1)?.deliver([shiftEntry({ startTime: later })]);
    window.dispatchEvent(new Event('pagehide'));
    expect(logged).toHaveBeenCalledTimes(2);
    expect(logged.mock.calls[1]?.[0]).toMatchObject({ kind: 'layout.shift', shifts: 1 });
  });

  it('stopStability also flushes and unregisters', () => {
    startStability();
    configureObs(DEV_ON);
    FakePerformanceObserver.instances[0]?.deliver([shiftEntry({ startTime: 10 })]);
    stopStability();
    expect(logged).toHaveBeenCalledTimes(1);
    expect(stabilitySinkCountForTests()).toBe(0);
  });
});
