/** T3.2b — useUpdateController: the idle→checking→available→downloading→
 *  readyToRestart machine, silent-check semantics, re-entrancy/double-download
 *  guards, error+Retry — plus the mock ?update= harness seam itself. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../ipc/mock';
import { useUpdateController } from './useUpdateController';
import { appErr } from '../test/actionHookKit';
import type { UpdateCheckResult, UpdateProgress } from '../ipc';

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const AVAILABLE: UpdateCheckResult = {
  available: true,
  currentVersion: '0.1.0',
  version: '0.2.0',
  notes: 'notes',
  date: '2026-08-01',
};
const NONE: UpdateCheckResult = {
  available: false,
  currentVersion: '0.1.0',
  version: null,
  notes: null,
  date: null,
};

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function mount() {
  return renderHook(() => useUpdateController());
}

describe('check', () => {
  it('loud check → checking → available; notification shows; dialog open/close', async () => {
    const d = deferred<UpdateCheckResult>();
    vi.spyOn(mockIpc, 'checkForUpdate').mockReturnValue(d.promise);
    const { result } = mount();
    expect(result.current.state).toEqual({ status: 'idle' });
    let p!: Promise<void>;
    act(() => {
      p = result.current.check();
    });
    expect(result.current.state).toEqual({ status: 'checking' });
    expect(result.current.notificationVisible).toBe(false);
    await act(async () => {
      d.resolve(AVAILABLE);
      await p;
    });
    expect(result.current.state).toEqual({ status: 'available', info: AVAILABLE });
    expect(result.current.currentVersion).toBe('0.1.0');
    expect(result.current.notificationVisible).toBe(true);
    act(() => result.current.openDialog());
    expect(result.current.dialogOpen).toBe(true);
    act(() => result.current.closeDialog());
    expect(result.current.dialogOpen).toBe(false);
    act(() => result.current.dismissNotification());
    expect(result.current.notificationVisible).toBe(false);
    expect(result.current.state.status).toBe('available'); // dismiss ≠ state change
  });

  it('a NEW available check un-dismisses the notification', async () => {
    vi.spyOn(mockIpc, 'checkForUpdate').mockResolvedValue(AVAILABLE);
    const { result } = mount();
    await act(async () => result.current.check());
    act(() => result.current.dismissNotification());
    await act(async () => result.current.check());
    expect(result.current.notificationVisible).toBe(true);
  });

  it('up to date: loud → upToDate; silent → idle (never shows checking)', async () => {
    const d = deferred<UpdateCheckResult>();
    vi.spyOn(mockIpc, 'checkForUpdate').mockReturnValue(d.promise);
    const { result } = mount();
    let p!: Promise<void>;
    act(() => {
      p = result.current.check(true); // silent launch check
    });
    expect(result.current.state).toEqual({ status: 'idle' }); // no 'checking'
    await act(async () => {
      d.resolve(NONE);
      await p;
    });
    expect(result.current.state).toEqual({ status: 'idle' });
    expect(result.current.currentVersion).toBe('0.1.0'); // still learned

    vi.spyOn(mockIpc, 'checkForUpdate').mockResolvedValue(NONE);
    await act(async () => result.current.check());
    expect(result.current.state).toEqual({ status: 'upToDate' });
  });

  it('check failure: loud → error state; silent → swallowed (idle)', async () => {
    vi.spyOn(mockIpc, 'checkForUpdate').mockRejectedValue(appErr('networkError', 'offline'));
    const { result } = mount();
    await act(async () => result.current.check());
    expect(result.current.state).toEqual({ status: 'error', message: 'offline' });
    await act(async () => result.current.check(true));
    expect(result.current.state).toEqual({ status: 'idle' });
  });

  it('a failed check clears the stale payload — download() after it is a no-op', async () => {
    vi.spyOn(mockIpc, 'checkForUpdate')
      .mockResolvedValueOnce(AVAILABLE)
      .mockRejectedValueOnce(appErr('networkError', 'offline'));
    const dl = vi.spyOn(mockIpc, 'downloadAndInstallUpdate').mockResolvedValue(undefined);
    const { result } = mount();
    await act(async () => result.current.check());
    await act(async () => result.current.check()); // fails → infoRef cleared
    expect(result.current.state.status).toBe('error');
    act(() => result.current.download()); // error state BUT no payload
    expect(dl).not.toHaveBeenCalled();
  });

  it('re-entrancy: overlapping checks issue ONE request', async () => {
    const d = deferred<UpdateCheckResult>();
    const spy = vi.spyOn(mockIpc, 'checkForUpdate').mockReturnValue(d.promise);
    const { result } = mount();
    let p1!: Promise<void>;
    let p2!: Promise<void>;
    act(() => {
      p1 = result.current.check();
      p2 = result.current.check();
    });
    await act(async () => {
      d.resolve(NONE);
      await Promise.all([p1, p2]);
    });
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('check is a no-op while downloading or readyToRestart (progress preserved)', async () => {
    vi.spyOn(mockIpc, 'checkForUpdate').mockResolvedValue(AVAILABLE);
    const dl = deferred<void>();
    vi.spyOn(mockIpc, 'downloadAndInstallUpdate').mockReturnValue(dl.promise);
    const { result } = mount();
    await act(async () => result.current.check());
    act(() => result.current.download());
    expect(result.current.state.status).toBe('downloading');
    const spy = vi.spyOn(mockIpc, 'checkForUpdate');
    spy.mockClear();
    await act(async () => result.current.check());
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.state.status).toBe('downloading');
    await act(async () => dl.resolve());
    expect(result.current.state.status).toBe('readyToRestart');
    await act(async () => result.current.check());
    expect(spy).not.toHaveBeenCalled();
  });
});

describe('download', () => {
  async function toAvailable() {
    vi.spyOn(mockIpc, 'checkForUpdate').mockResolvedValue(AVAILABLE);
    const h = mount();
    await act(async () => h.result.current.check());
    return h;
  }

  it('streams progress into state, then readyToRestart', async () => {
    const h = await toAvailable();
    let emit!: (p: UpdateProgress) => void;
    const dl = deferred<void>();
    vi.spyOn(mockIpc, 'downloadAndInstallUpdate').mockImplementation((onProgress) => {
      emit = onProgress;
      return dl.promise;
    });
    act(() => h.result.current.download());
    expect(h.result.current.state).toMatchObject({
      status: 'downloading',
      progress: { phase: 'started', downloadedBytes: 0 },
    });
    act(() => emit({ phase: 'downloading', downloadedBytes: 500, contentLength: 1000 }));
    expect(h.result.current.state).toMatchObject({
      status: 'downloading',
      info: AVAILABLE,
      progress: { downloadedBytes: 500 },
    });
    await act(async () => dl.resolve());
    expect(h.result.current.state).toEqual({ status: 'readyToRestart', info: AVAILABLE });
  });

  it('is a no-op from idle (no payload) and from downloading (double-click same tick)', async () => {
    const dlSpy = vi.spyOn(mockIpc, 'downloadAndInstallUpdate').mockReturnValue(
      deferred<void>().promise,
    );
    const cold = mount();
    act(() => cold.result.current.download());
    expect(dlSpy).not.toHaveBeenCalled();

    const h = await toAvailable();
    act(() => {
      h.result.current.download();
      h.result.current.download(); // same tick — eager statusRef guard
    });
    expect(dlSpy).toHaveBeenCalledTimes(1);
  });

  it('failure → error state; Retry (download from error) re-uses the retained payload', async () => {
    const h = await toAvailable();
    const dl = vi
      .spyOn(mockIpc, 'downloadAndInstallUpdate')
      .mockRejectedValueOnce(appErr('networkError', 'cut off'))
      .mockResolvedValueOnce(undefined);
    await act(async () => {
      h.result.current.download();
    });
    expect(h.result.current.state).toEqual({ status: 'error', message: 'cut off' });
    await act(async () => {
      h.result.current.download(); // Retry from error
    });
    expect(dl).toHaveBeenCalledTimes(2);
    expect(h.result.current.state).toEqual({ status: 'readyToRestart', info: AVAILABLE });
  });
});

describe('restart', () => {
  it('invokes relaunchApp; a rejection is swallowed', async () => {
    const spy = vi.spyOn(mockIpc, 'relaunchApp').mockRejectedValue(new Error('exiting'));
    const { result } = mount();
    act(() => result.current.restart());
    await act(async () => {});
    expect(spy).toHaveBeenCalledTimes(1); // and no unhandled rejection
  });
});

// ---------------------------------------------------------------------------
// The mock harness seam itself (?update= read once at module init) — exercised
// via vi.resetModules + a rewritten location, per mode.
// ---------------------------------------------------------------------------

describe('mock update seam (?update=)', () => {
  async function loadWithMode(mode?: string) {
    vi.resetModules();
    window.history.replaceState({}, '', mode === undefined ? '/' : `/?update=${mode}`);
    return (await import('../ipc/mock/handlers/update')).updateHandlers;
  }

  it('default (no flag) → up to date; download without a check rejects', async () => {
    vi.useFakeTimers();
    const h = await loadWithMode();
    const p = h.checkForUpdate();
    await vi.advanceTimersByTimeAsync(400);
    expect(await p).toMatchObject({ available: false, currentVersion: '0.1.0', version: null });
    await expect(h.downloadAndInstallUpdate(() => {})).rejects.toMatchObject({
      kind: 'noOperationInProgress',
    });
  });

  it('?update=available → 0.2.0 offered; download streams started→…→finished', async () => {
    vi.useFakeTimers();
    const h = await loadWithMode('available');
    const check = h.checkForUpdate();
    await vi.advanceTimersByTimeAsync(400);
    expect(await check).toMatchObject({ available: true, version: '0.2.0' });

    const ticks: UpdateProgress[] = [];
    const dl = h.downloadAndInstallUpdate((p) => ticks.push(p));
    await vi.advanceTimersByTimeAsync(15 * 120);
    await dl;
    expect(ticks[0]).toMatchObject({ phase: 'started', downloadedBytes: 0 });
    expect(ticks[ticks.length - 1]).toMatchObject({ phase: 'finished' });
    const last = ticks[ticks.length - 1];
    expect(last.downloadedBytes).toBe(last.contentLength);
    expect(ticks.filter((t) => t.phase === 'downloading').length).toBe(15);
    // Cumulative, monotonically non-decreasing bytes.
    for (let i = 1; i < ticks.length; i += 1)
      expect(ticks[i].downloadedBytes).toBeGreaterThanOrEqual(ticks[i - 1].downloadedBytes);
  });

  it('?update=error → check rejects networkError and download stays gated', async () => {
    vi.useFakeTimers();
    const h = await loadWithMode('error');
    const p = h.checkForUpdate();
    const assertion = expect(p).rejects.toMatchObject({ kind: 'networkError' });
    await vi.advanceTimersByTimeAsync(400);
    await assertion;
    await expect(h.downloadAndInstallUpdate(() => {})).rejects.toMatchObject({
      kind: 'noOperationInProgress',
    });
  });
});
