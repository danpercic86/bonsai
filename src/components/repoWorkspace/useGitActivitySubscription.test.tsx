/**
 * P123 V1-V3 — `useGitActivitySubscription` owns exactly ONE live subscription
 * per mount, through the REAL `ipc` (obs proxy → mock hub), including under
 * StrictMode's double mount and an unmount before the subscribe resolves.
 */
import { StrictMode } from 'react';
import type { ReactNode } from 'react';
import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { emitGitActivity, gitActivitySubscriberCount } from '../../ipc/mock/gitActivityHub';
import type { GitActivityEvent, GitActivityListener } from '../../ipc';
import { useGitActivitySubscription } from './useGitActivitySubscription';

const REPO = 'C:\\mock\\sub-repo';

const started = (id: string): GitActivityEvent => ({
  id,
  seq: 0,
  kind: 'started',
  elapsedMs: 0,
  category: 'fetch',
  repoId: REPO,
});

/** Let every pending subscribe/unsubscribe promise settle. */
async function settle(): Promise<void> {
  await act(async () => {
    for (let i = 0; i < 5; i += 1) await Promise.resolve();
  });
}

afterEach(() => vi.restoreAllMocks());

describe('useGitActivitySubscription (P123)', () => {
  it('V1: StrictMode double mount leaves exactly ONE live subscription; unmount drops it', async () => {
    const base = gitActivitySubscriberCount();
    const onEvent = vi.fn<GitActivityListener>();
    const view = renderHook(() => useGitActivitySubscription(REPO, onEvent), {
      wrapper: ({ children }: { children: ReactNode }) => <StrictMode>{children}</StrictMode>,
    });
    await settle();
    expect(gitActivitySubscriberCount() - base).toBe(1);
    act(() => emitGitActivity(REPO, started('v1-a')));
    expect(onEvent).toHaveBeenCalledTimes(1);
    view.unmount();
    await settle();
    expect(gitActivitySubscriberCount()).toBe(base);
  });

  it('V1b: N mounted hooks ⇒ N live subscriptions (one per tab)', async () => {
    const base = gitActivitySubscriberCount();
    const views = ['A', 'B', 'C'].map((r) => renderHook(() => useGitActivitySubscription(r, vi.fn())));
    await settle();
    expect(gitActivitySubscriberCount() - base).toBe(3);
    views[1]?.unmount();
    await settle();
    expect(gitActivitySubscriberCount() - base).toBe(2);
    for (const v of views) v.unmount();
    await settle();
    expect(gitActivitySubscriberCount()).toBe(base);
  });

  it('V2: a subscribe resolving AFTER unmount is unsubscribed at once', async () => {
    const base = gitActivitySubscriberCount();
    const real = mockIpc.gitActivitySubscribe.bind(mockIpc);
    let release!: () => void;
    const gate = new Promise<void>((res) => {
      release = res;
    });
    let heldId: string | null = null;
    vi.spyOn(mockIpc, 'gitActivitySubscribe').mockImplementation(async (repoId, cb) => {
      const id = await real(repoId, cb);
      heldId = id;
      await gate;
      return id;
    });
    const unsub = vi.spyOn(mockIpc, 'gitActivityUnsubscribe');
    const onEvent = vi.fn<GitActivityListener>();
    const view = renderHook(() => useGitActivitySubscription(REPO, onEvent));
    await settle();
    expect(gitActivitySubscriberCount() - base).toBe(1);
    view.unmount();
    expect(unsub).not.toHaveBeenCalled();
    release();
    await settle();
    expect(unsub).toHaveBeenCalledWith(heldId);
    expect(gitActivitySubscriberCount()).toBe(base);
    act(() => emitGitActivity(REPO, started('v2-a')));
    expect(onEvent).not.toHaveBeenCalled();
  });

  it('V3: an event landing after cleanup but before the unsubscribe is not delivered', async () => {
    let captured: GitActivityListener | null = null;
    vi.spyOn(mockIpc, 'gitActivitySubscribe').mockImplementation(async (_repoId, cb) => {
      captured = cb;
      return 'gas-mock-held';
    });
    vi.spyOn(mockIpc, 'gitActivityUnsubscribe').mockResolvedValue(undefined);
    const onEvent = vi.fn<GitActivityListener>();
    const view = renderHook(() => useGitActivitySubscription(REPO, onEvent));
    await settle();
    const deliver = captured as GitActivityListener | null;
    expect(deliver).not.toBeNull();
    deliver?.(started('v3-a'));
    expect(onEvent).toHaveBeenCalledTimes(1);
    view.unmount();
    deliver?.(started('v3-b'));
    expect(onEvent).toHaveBeenCalledTimes(1);
  });
});
