/** P70 §4.4 / UI §6–§7: the git-preflight hook — one probe on mount, the
 *  latch (including the probe it kicks), the 400 ms minimum `checking` window,
 *  and the rule that a failed probe produces NO state (and therefore no chrome). */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import { mockIpc } from '../ipc/mock';
import { useGitAvailability, MIN_CHECKING_MS } from './useGitAvailability';
import {
  noteGitNotFound,
  resetGitNotFoundLatchForTests,
  gitNotFoundLatched,
} from '../ipc/gitNotFound';
import type { GitAvailability } from '../ipc';

const FOUND: GitAvailability = {
  found: true,
  path: '/usr/bin/git',
  version: '2.47.1',
  source: 'path',
  detail: 'Git 2.47.1 — /usr/bin/git (path)',
};
const MISSING: GitAvailability = {
  found: false,
  path: null,
  version: null,
  source: 'fallback',
  detail: 'Git is not available. …',
};

beforeEach(() => {
  resetGitNotFoundLatchForTests();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
  resetGitNotFoundLatchForTests();
});

describe('useGitAvailability', () => {
  it('probes exactly once on mount and exposes the status', async () => {
    const spy = vi.spyOn(mockIpc, 'checkGitAvailability').mockResolvedValue(FOUND);
    const { result, rerender } = renderHook(() => useGitAvailability());

    await waitFor(() => expect(result.current.status).toEqual(FOUND));
    rerender();
    rerender();
    expect(spy).toHaveBeenCalledTimes(1);
    expect(result.current.checking).toBe(false);
  });

  it('a probe that throws leaves status null — a failed probe must not make chrome', async () => {
    const spy = vi
      .spyOn(mockIpc, 'checkGitAvailability')
      .mockRejectedValue(new Error('invoke exploded'));
    const { result } = renderHook(() => useGitAvailability());

    await waitFor(() => expect(spy).toHaveBeenCalled());
    await waitFor(() => expect(result.current.checking).toBe(false));
    expect(result.current.status).toBeNull();
  });

  it('recheck re-invokes and holds `checking` for the 400 ms minimum window', async () => {
    vi.spyOn(mockIpc, 'checkGitAvailability').mockResolvedValue(MISSING);
    const { result } = renderHook(() => useGitAvailability());
    await waitFor(() => expect(result.current.status).toEqual(MISSING));

    const started = Date.now();
    let resolved: GitAvailability | null = null;
    await act(async () => {
      resolved = await result.current.recheck();
    });
    expect(resolved).toEqual(MISSING);
    // The mock resolves immediately, so any elapsed time is the floor at work.
    expect(Date.now() - started).toBeGreaterThanOrEqual(MIN_CHECKING_MS - 20);
    await waitFor(() => expect(result.current.checking).toBe(false));
  });

  it('a successful recheck clears a latch set by an observed gitNotFound error', async () => {
    vi.spyOn(mockIpc, 'checkGitAvailability').mockResolvedValue(FOUND);
    const { result } = renderHook(() => useGitAvailability());
    await waitFor(() => expect(result.current.status).toEqual(FOUND));

    act(() => noteGitNotFound());
    expect(result.current.latched).toBe(true);

    await act(async () => {
      await result.current.recheck();
    });
    expect(gitNotFoundLatched()).toBe(false);
    expect(result.current.latched).toBe(false);
  });

  it('a latch fired before any status lands kicks a probe (ratified decision 4)', async () => {
    // First call never settles => `status` stays null while the latch fires.
    let settleFirst: ((v: GitAvailability) => void) | null = null;
    const spy = vi
      .spyOn(mockIpc, 'checkGitAvailability')
      .mockImplementationOnce(
        () =>
          new Promise<GitAvailability>((resolve) => {
            settleFirst = resolve;
          }),
      )
      .mockResolvedValue(MISSING);

    const { result } = renderHook(() => useGitAvailability());
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(result.current.status).toBeNull();

    act(() => noteGitNotFound());
    expect(result.current.latched).toBe(true);
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(result.current.status).toEqual(MISSING));

    // The stale first probe must not clobber the later answer.
    act(() => settleFirst?.(FOUND));
    await waitFor(() => expect(result.current.status).toEqual(MISSING));
  });

  it('a latch fired AFTER a healthy status re-probes — git can break mid-session', async () => {
    // MUST-FIX (round 2): the startup probe can land `found: true` via the PATH
    // rung and git can then be moved, uninstalled or quarantined. The latch's
    // rising edge must re-probe REGARDLESS of the status we hold, or the banner
    // (and its Re-check button) never appears and the user gets a bare toast.
    const spy = vi
      .spyOn(mockIpc, 'checkGitAvailability')
      .mockResolvedValueOnce(FOUND)
      .mockResolvedValue(MISSING);
    const { result } = renderHook(() => useGitAvailability());
    await waitFor(() => expect(result.current.status).toEqual(FOUND));

    act(() => noteGitNotFound());
    await waitFor(() => expect(result.current.status).toEqual(MISSING));
    expect(spy).toHaveBeenCalledTimes(2);
    expect(result.current.latched).toBe(true);

    // …and the re-probe must not re-arm itself off its own `setStatus` (the
    // effect must be an EDGE, not a level, or it loops forever).
    await new Promise<void>((r) => {
      setTimeout(r, 50);
    });
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('a latch that re-fires after being cleared probes again', async () => {
    const spy = vi.spyOn(mockIpc, 'checkGitAvailability').mockResolvedValue(FOUND);
    const { result } = renderHook(() => useGitAvailability());
    await waitFor(() => expect(result.current.status).toEqual(FOUND));

    act(() => noteGitNotFound());
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
    // The probe came back healthy, so the latch self-heals.
    await waitFor(() => expect(result.current.latched).toBe(false));

    act(() => noteGitNotFound());
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(3));
  });

  it('`checking` is true from the click onward, for the whole floor', async () => {
    vi.spyOn(mockIpc, 'checkGitAvailability').mockResolvedValue(MISSING);
    const { result } = renderHook(() => useGitAvailability());
    await waitFor(() => expect(result.current.status).toEqual(MISSING));

    let pending: Promise<GitAvailability | null> | null = null;
    act(() => {
      pending = result.current.recheck();
    });
    // The FLAG the button renders from — not just the promise's duration.
    expect(result.current.checking).toBe(true);
    await new Promise<void>((r) => {
      setTimeout(r, MIN_CHECKING_MS / 2);
    });
    expect(result.current.checking).toBe(true);
    await act(async () => {
      await pending;
    });
    await waitFor(() => expect(result.current.checking).toBe(false));
  });
});
