/** P121c — the job-status hook no longer owns a clock: a 30 s tick must not
 *  re-render its host (RepoWorkspace, ×6 mounted workspaces). The relative
 *  labels tick on the shared `useNow` store in leaf components instead. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useJobStatus } from './useJobStatus';

beforeEach(() => {
  vi.useFakeTimers();
  // Isolate from the mock's latency/scheduler: resolve immediately, never emit.
  vi.spyOn(mockIpc, 'getJobStatus').mockResolvedValue([]);
  vi.spyOn(mockIpc, 'onJobStatusChanged').mockResolvedValue(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('useJobStatus (P121c)', () => {
  it('non-StrictMode: a 30 s tick does not re-render the host', async () => {
    let renders = 0;
    const pushToast = vi.fn();
    renderHook(() => {
      renders += 1;
      return useJobStatus('repo-1', pushToast);
    });
    // Flush the mount snapshot + subscription promises.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    const settled = renders;
    await act(async () => {
      vi.advanceTimersByTime(30_000);
      await Promise.resolve();
    });
    expect(renders).toBe(settled);
  });
});
