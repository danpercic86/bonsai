/**
 * P91 increment 7d — the ACTIVATION wire.
 *
 * `useUiSettings` is the sole production caller of `configureObs` (a
 * `useEffect(() => configureObs(dev), [dev])`). Increments 1-7c built the whole
 * frontend pipeline but never turned it on; this suite is the regression guard
 * that hydrating / toggling the persisted `dev` settings actually flips capture.
 *
 * House pattern (uiSettingsKit): the `dom` vitest project runs with
 * VITE_MOCK_IPC=1, so `ipc` IS the instrumented `mockIpc` — a real call through
 * it exercises the proxy. We attach a CAPTURING sink after mount so the drained
 * records are inspectable without depending on the mock ring's own gate.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { ipc } from '../ipc';
import { obsEnabled } from '../obs/enabled';
import { resetObsConfigForTests } from '../obs/enabled';
import {
  attachSink,
  flushNow,
  pendingRestart,
  resetBatcherForTests,
} from '../obs/batcher';
import { resetIpcProxyForTests } from '../obs/ipcProxy';
import { clearSessionSalt } from '../obs/redact';
import { HYDRATED, mountUiSettings as mount } from '../test/uiSettingsKit';
import type { DevSettings } from '../ipc/types/settings';
import type { LogRecord } from '../obs/types';
import type { UiSettings } from '../ipc';

const SALT = '00112233445566778899aabbccddeeff';

const DEV_ON: DevSettings = {
  enabled: true,
  level: 'debug',
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

let sunk: LogRecord[] = [];

/** A capturing sink with a real salt, so the batcher's re-salt on enable
 *  resolves and `redactionReady()` becomes true. */
function attachCapture(): void {
  attachSink({
    async logAppend(records) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: SALT };
    },
  });
}

/** Hydrate the hook with a fully-enabled `dev` block (HYDRATED.dev has
 *  captureIpc:false, which would gate the proxy off). */
function settingsWithDev(dev: DevSettings): UiSettings {
  return { ...HYDRATED, dev };
}

const ipcCalls = () => sunk.filter((r) => r.kind === 'ipc.call');

beforeEach(() => {
  sunk = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  resetIpcProxyForTests();
  clearSessionSalt();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
  resetObsConfigForTests();
  resetBatcherForTests();
  clearSessionSalt();
});

describe('useUiSettings activates the obs pipeline (increment 7d)', () => {
  it('starts inert: default dev is disabled, so obsEnabled() is false after mount', () => {
    attachCapture();
    mount();
    // The mount-time effect ran configureObs(dev) with the disabled default.
    expect(obsEnabled()).toBe(false);
  });

  it('hydrating dev.enabled:true turns capture on and a real ipc call is recorded', async () => {
    attachCapture();
    const { result } = mount();
    expect(obsEnabled()).toBe(false);

    await act(async () => {
      result.current.hydrateUiSettings(settingsWithDev(DEV_ON));
      // The batcher re-salts on enable; wait for the fetch so the redactor is ready.
      await pendingRestart();
    });
    expect(obsEnabled()).toBe(true);

    await act(async () => {
      await ipc.getUiSettings();
      await flushNow();
    });
    const calls = ipcCalls();
    expect(calls.length).toBeGreaterThan(0);
    expect(calls.some((r) => r.kind === 'ipc.call' && r.cmd === 'getUiSettings')).toBe(true);
  });

  it('toggling dev.enabled true→false→true flips capture live (no reload)', async () => {
    attachCapture();
    const { result } = mount();

    // ON
    await act(async () => {
      result.current.handleSettingsChange({ dev: DEV_ON });
      await pendingRestart();
    });
    expect(obsEnabled()).toBe(true);

    // OFF — the effect refires because `dev` is a fresh object per patch.
    await act(async () => {
      result.current.handleSettingsChange({ dev: { ...DEV_ON, enabled: false } });
      await flushNow();
    });
    expect(obsEnabled()).toBe(false);
    sunk = [];
    await act(async () => {
      await ipc.getUiSettings();
      await flushNow();
    });
    expect(ipcCalls()).toHaveLength(0);

    // ON again
    await act(async () => {
      result.current.handleSettingsChange({ dev: DEV_ON });
      await pendingRestart();
    });
    expect(obsEnabled()).toBe(true);
    await act(async () => {
      await ipc.getUiSettings();
      await flushNow();
    });
    expect(ipcCalls().length).toBeGreaterThan(0);
  });
});
