/** T3.2b — useHookGate: hookRejected parks behind the dialog; skip-retry,
 *  cancel, non-hook pass-through, and the cancel-during-retry race. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useHookGate } from './useHookGate';
import { COMMIT_HOOK_CANCELED } from '../commitPushSignal';
import { appErr } from '../../test/actionHookKit';

afterEach(() => vi.restoreAllMocks());

function mount() {
  return renderHook(() => useHookGate());
}

describe('runWithHookGate', () => {
  it('success passes straight through with the given skipHooks flag', async () => {
    const { result } = mount();
    const attempt = vi.fn(async () => {});
    await act(async () => result.current.runWithHookGate(attempt, false));
    expect(attempt).toHaveBeenCalledExactlyOnceWith(false);
    expect(result.current.pendingHook).toBeNull();

    await act(async () => result.current.runWithHookGate(attempt, true));
    expect(attempt).toHaveBeenLastCalledWith(true);
  });

  it('a non-hook error is rethrown unchanged and never opens the dialog', async () => {
    const { result } = mount();
    const err = appErr('git', 'nothing staged');
    const attempt = vi.fn(async () => {
      throw err;
    });
    await act(async () => {
      await expect(result.current.runWithHookGate(attempt, false)).rejects.toBe(err);
    });
    expect(result.current.pendingHook).toBeNull();
  });

  it('hookRejected parks: the promise stays pending and the dialog message shows', async () => {
    const { result } = mount();
    const attempt = vi.fn(async () => {
      throw appErr('hookRejected', 'pre-commit said no');
    });
    let settled = false;
    let p!: Promise<void>;
    act(() => {
      p = result.current.runWithHookGate(attempt, false).finally(() => {
        settled = true;
      });
    });
    await act(async () => {});
    expect(result.current.pendingHook).toBe('pre-commit said no');
    expect(result.current.hookRetrying).toBe(false);
    expect(settled).toBe(false);
    // cleanup: settle so vitest sees no dangling rejection
    act(() => result.current.onHookCancel());
    await expect(p).rejects.toBe(COMMIT_HOOK_CANCELED);
  });
});

describe('onHookSkipRetry', () => {
  it('re-runs the SAME attempt with skipHooks:true and resolves the parked promise', async () => {
    const { result } = mount();
    const attempt = vi.fn(async (skipHooks: boolean) => {
      if (!skipHooks) throw appErr('hookRejected', 'blocked');
    });
    let p!: Promise<void>;
    act(() => {
      p = result.current.runWithHookGate(attempt, false);
    });
    await act(async () => {});
    act(() => result.current.onHookSkipRetry());
    expect(result.current.hookRetrying).toBe(true);
    await act(async () => p);
    expect(attempt).toHaveBeenCalledTimes(2);
    expect(attempt).toHaveBeenLastCalledWith(true);
    expect(result.current.pendingHook).toBeNull();
    expect(result.current.hookRetrying).toBe(false);
  });

  it('a retry that fails with a non-hook error rejects the parked promise with it', async () => {
    const { result } = mount();
    const other = appErr('git', 'identity unset');
    const attempt = vi.fn(async (skipHooks: boolean) => {
      throw skipHooks ? other : appErr('hookRejected', 'blocked');
    });
    let p!: Promise<void>;
    act(() => {
      p = result.current.runWithHookGate(attempt, false);
    });
    await act(async () => {});
    act(() => result.current.onHookSkipRetry());
    await act(async () => {
      await expect(p).rejects.toBe(other);
    });
    expect(result.current.pendingHook).toBeNull();
    expect(result.current.hookRetrying).toBe(false);
  });

  it('is a no-op with no parked gate', () => {
    const { result } = mount();
    act(() => result.current.onHookSkipRetry());
    expect(result.current.hookRetrying).toBe(false);
  });
});

describe('onHookCancel', () => {
  it('rejects the parked promise with the sentinel and closes the dialog', async () => {
    const { result } = mount();
    const attempt = vi.fn(async () => {
      throw appErr('hookRejected', 'blocked');
    });
    let p!: Promise<void>;
    act(() => {
      p = result.current.runWithHookGate(attempt, false);
    });
    await act(async () => {});
    act(() => result.current.onHookCancel());
    await expect(p).rejects.toBe(COMMIT_HOOK_CANCELED);
    expect(result.current.pendingHook).toBeNull();
    expect(attempt).toHaveBeenCalledTimes(1); // nothing was committed
  });

  it('cancel DURING a skip-retry no-ops: the retry outcome wins (no double-settle)', async () => {
    const { result } = mount();
    let releaseRetry!: () => void;
    const retryDone = new Promise<void>((res) => {
      releaseRetry = res;
    });
    const attempt = vi.fn(async (skipHooks: boolean) => {
      if (!skipHooks) throw appErr('hookRejected', 'blocked');
      await retryDone; // slow skip-hooks commit
    });
    let p!: Promise<void>;
    act(() => {
      p = result.current.runWithHookGate(attempt, false);
    });
    await act(async () => {});
    act(() => result.current.onHookSkipRetry());
    act(() => result.current.onHookCancel()); // user mashes Cancel mid-retry
    await act(async () => {
      releaseRetry();
      await p; // resolves (commit landed) — NOT the cancel sentinel
    });
    expect(result.current.pendingHook).toBeNull();
    expect(result.current.hookRetrying).toBe(false);
  });

  it('is a no-op with no parked gate', () => {
    const { result } = mount();
    act(() => result.current.onHookCancel());
    expect(result.current.pendingHook).toBeNull();
  });
});
