// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DevSettings } from '../../ipc/types/settings';
import { enqueue } from '../batcher';
import { configureObs, resetObsConfigForTests } from '../enabled';
import { createDevSink } from './devSink';
import { startStability, stopStability } from './install';
import { resetPhaseForTests } from './phase';
import { resetSinksForTests, stabilitySinkCountForTests } from './sinks';
import { FakePerformanceObserver } from './testFakes';

vi.mock('../batcher', async (orig) => ({ ...(await orig<typeof import('../batcher')>()), enqueue: vi.fn() }));
const enqueued = vi.mocked(enqueue);

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

  it('turning Dev mode off detaches everything and removes the pagehide listener (T-F9)', () => {
    const remove = vi.spyOn(window, 'removeEventListener');
    startStability();
    configureObs(DEV_ON);
    const po = FakePerformanceObserver.instances[0];
    configureObs(null);
    expect(stabilitySinkCountForTests()).toBe(0);
    expect(po?.disconnected).toBe(true);
    expect(remove.mock.calls.filter(([t]) => t === 'pagehide')).toHaveLength(1);
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
