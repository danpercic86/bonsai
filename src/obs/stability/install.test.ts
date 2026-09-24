// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DevSettings } from '../../ipc/types/settings';
import { enqueue, flushNow } from '../batcher';
import { configureObs, resetObsConfigForTests } from '../enabled';
import { createDevSink } from './devSink';
import { startStability, stopStability } from './install';
import { resetPhaseForTests } from './phase';
import { resetSinksForTests, stabilitySinkCountForTests } from './sinks';
import { FakePerformanceObserver, shiftEntry } from './testFakes';

vi.mock('../batcher', async (orig) => ({
  ...(await orig<typeof import('../batcher')>()),
  enqueue: vi.fn(),
  flushNow: vi.fn(() => Promise.resolve()),
}));
const enqueued = vi.mocked(enqueue);
const flushed = vi.mocked(flushNow);

function setVisibility(state: DocumentVisibilityState): void {
  vi.spyOn(document, 'visibilityState', 'get').mockReturnValue(state);
}

const DEV_ON: DevSettings = {
  enabled: true,
  level: 'trace',
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

let mutationObservers = 0;

beforeEach(() => {
  resetPhaseForTests();
  resetSinksForTests();
  resetObsConfigForTests();
  enqueued.mockClear();
  flushed.mockClear();
  FakePerformanceObserver.instances = [];
  FakePerformanceObserver.supportedEntryTypes = ['layout-shift'];
  vi.stubGlobal('PerformanceObserver', FakePerformanceObserver);
  mutationObservers = 0;
  const RealMO = window.MutationObserver;
  vi.stubGlobal(
    'MutationObserver',
    class extends RealMO {
      constructor(cb: MutationCallback) {
        super(cb);
        mutationObservers += 1;
      }
    },
  );
});

afterEach(() => {
  stopStability();
  resetObsConfigForTests();
  resetSinksForTests();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('startStability outside mock mode', () => {
  it('Dev mode off: registers no sink, so no observer or listener is attached; idempotent (T-F8)', () => {
    expect(import.meta.env.VITE_MOCK_IPC).not.toBe('1');
    const add = vi.spyOn(window, 'addEventListener');
    const addDoc = vi.spyOn(document, 'addEventListener');
    startStability();
    startStability();
    expect(stabilitySinkCountForTests()).toBe(0);
    expect(add).not.toHaveBeenCalled();
    expect(addDoc).not.toHaveBeenCalled();
    expect(FakePerformanceObserver.instances).toHaveLength(0);
    expect(mutationObservers).toBe(0);
  });

  it('Dev mode on registers the dev sink once (repeat config changes do not re-register)', () => {
    const add = vi.spyOn(window, 'addEventListener');
    startStability();
    configureObs(DEV_ON);
    configureObs({ ...DEV_ON, captureIpc: false });
    expect(stabilitySinkCountForTests()).toBe(1);
    expect(add.mock.calls.filter(([t]) => t === 'pagehide')).toHaveLength(1);
    expect(FakePerformanceObserver.instances).toHaveLength(1);
  });

  it('Dev mode already on at start registers immediately', () => {
    configureObs(DEV_ON);
    startStability();
    expect(stabilitySinkCountForTests()).toBe(1);
  });

  // T-F9: the disable-flush runs (installFlush.test.ts observes it) so the sink
  // is clean for a re-enable, but `obsEnabled()` is already false, so the obs
  // gate in `logRecord` drops those records BY DESIGN — Rust closes its session
  // on disable (`src-tauri/src/obs/mod.rs` `apply_dev_settings`), so there is
  // no file for them to land in.
  it('turning Dev mode off detaches everything, removes both listeners and enqueues nothing (T-F9)', () => {
    const remove = vi.spyOn(window, 'removeEventListener');
    const removeDoc = vi.spyOn(document, 'removeEventListener');
    startStability();
    configureObs(DEV_ON);
    const po = FakePerformanceObserver.instances[0];
    po?.deliver([shiftEntry({ startTime: 10 }), shiftEntry({ startTime: 20 })]);
    configureObs(null);
    expect(enqueued).not.toHaveBeenCalled();
    expect(stabilitySinkCountForTests()).toBe(0);
    expect(po?.disconnected).toBe(true);
    expect(remove.mock.calls.filter(([t]) => t === 'pagehide')).toHaveLength(1);
    expect(removeDoc.mock.calls.filter(([t]) => t === 'visibilitychange')).toHaveLength(1);
    // Neither listener survives the disable.
    setVisibility('hidden');
    window.dispatchEvent(new Event('pagehide'));
    document.dispatchEvent(new Event('visibilitychange'));
    expect(flushed).not.toHaveBeenCalled();
  });

  it('pagehide flushes the open buckets while Dev mode is on', () => {
    startStability();
    configureObs(DEV_ON);
    const po = FakePerformanceObserver.instances[0];
    po?.deliver([{ entryType: 'layout-shift', startTime: 10, value: 0.05, hadRecentInput: false, sources: [] }]);
    expect(enqueued).not.toHaveBeenCalled();
    window.dispatchEvent(new Event('pagehide'));
    expect(enqueued).toHaveBeenCalledTimes(1);
    expect(enqueued.mock.calls[0]?.[0]).toMatchObject({ kind: 'layout.shift', shifts: 1, value: 0.05, phase: 'boot' });
    expect(flushed).toHaveBeenCalledTimes(1);
    expect(flushed.mock.invocationCallOrder[0]).toBeGreaterThan(enqueued.mock.invocationCallOrder[0] ?? Infinity);
  });

  it('visibilitychange → hidden flushes the buckets and the batcher; → visible does not', () => {
    const addDoc = vi.spyOn(document, 'addEventListener');
    startStability();
    configureObs(DEV_ON);
    expect(addDoc.mock.calls.filter(([t]) => t === 'visibilitychange')).toHaveLength(1);
    FakePerformanceObserver.instances[0]?.deliver([shiftEntry({ startTime: 10 })]);
    setVisibility('visible');
    document.dispatchEvent(new Event('visibilitychange'));
    expect(enqueued).not.toHaveBeenCalled();
    expect(flushed).not.toHaveBeenCalled();
    setVisibility('hidden');
    document.dispatchEvent(new Event('visibilitychange'));
    expect(enqueued).toHaveBeenCalledTimes(1);
    expect(flushed).toHaveBeenCalledTimes(1);
  });
});

describe('dev sink with obsEnabled() false (T-F8)', () => {
  it('produces zero log output', () => {
    const dev = createDevSink();
    dev.sink({ type: 'shift', region: 'graph', phase: 'interactive', hadRecentInput: false, resizing: false, value: 0.1, at: 1 });
    dev.sink({ type: 'focus-lost', fromRegion: 'details', phase: 'interactive', via: 'focusout', at: 2 });
    dev.flushAll();
    expect(enqueued).not.toHaveBeenCalled();
  });
});
