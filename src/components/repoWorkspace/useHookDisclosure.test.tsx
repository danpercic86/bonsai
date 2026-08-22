/** useHookDisclosure: the first-time per-repo git-hook execution disclosure —
 *  no-hooks / already-acked short-circuits, the block-until-acknowledged dialog
 *  (confirm ⇒ true + ack; cancel ⇒ false), skip-hooks bypass, and the session
 *  cache that makes commit&push a single prompt. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useHookDisclosure } from './useHookDisclosure';
import { REPO } from '../../test/actionHookKit';

afterEach(() => vi.restoreAllMocks());

describe('ensureHooksDisclosed', () => {
  it('hasHooks:false → returns true with no dialog and no ack', async () => {
    const get = vi
      .spyOn(mockIpc, 'getRepoHooksDisclosure')
      .mockResolvedValue({ hasHooks: false, acknowledged: false });
    const ack = vi.spyOn(mockIpc, 'ackRepoHooks').mockResolvedValue(undefined);
    const { result } = renderHook(() => useHookDisclosure(REPO));

    let outcome!: boolean;
    await act(async () => {
      outcome = await result.current.ensureHooksDisclosed(false);
    });
    expect(outcome).toBe(true);
    expect(get).toHaveBeenCalledExactlyOnceWith(REPO);
    expect(ack).not.toHaveBeenCalled();
    expect(result.current.pendingHookDisclosure).toBe(false);
  });

  it('acknowledged:true → returns true with no dialog and no ack', async () => {
    vi.spyOn(mockIpc, 'getRepoHooksDisclosure').mockResolvedValue({
      hasHooks: true,
      acknowledged: true,
    });
    const ack = vi.spyOn(mockIpc, 'ackRepoHooks').mockResolvedValue(undefined);
    const { result } = renderHook(() => useHookDisclosure(REPO));

    let outcome!: boolean;
    await act(async () => {
      outcome = await result.current.ensureHooksDisclosed(false);
    });
    expect(outcome).toBe(true);
    expect(ack).not.toHaveBeenCalled();
    expect(result.current.pendingHookDisclosure).toBe(false);
  });

  it('skipHooks:true short-circuits — never queries the backend', async () => {
    const get = vi.spyOn(mockIpc, 'getRepoHooksDisclosure');
    const { result } = renderHook(() => useHookDisclosure(REPO));

    let outcome!: boolean;
    await act(async () => {
      outcome = await result.current.ensureHooksDisclosed(true);
    });
    expect(outcome).toBe(true);
    expect(get).not.toHaveBeenCalled();
  });

  it('hasHooks:true, not acked → opens the dialog; confirm resolves true and acks', async () => {
    vi.spyOn(mockIpc, 'getRepoHooksDisclosure').mockResolvedValue({
      hasHooks: true,
      acknowledged: false,
    });
    const ack = vi.spyOn(mockIpc, 'ackRepoHooks').mockResolvedValue(undefined);
    const { result } = renderHook(() => useHookDisclosure(REPO));

    let p!: Promise<boolean>;
    act(() => {
      p = result.current.ensureHooksDisclosed(false);
    });
    await act(async () => {}); // let getRepoHooksDisclosure settle
    expect(result.current.pendingHookDisclosure).toBe(true);

    act(() => result.current.onHookDiscloseConfirm());
    await act(async () => {
      expect(await p).toBe(true);
    });
    expect(ack).toHaveBeenCalledExactlyOnceWith(REPO);
    expect(result.current.pendingHookDisclosure).toBe(false);
  });

  it('hasHooks:true, not acked → cancel resolves false and never acks', async () => {
    vi.spyOn(mockIpc, 'getRepoHooksDisclosure').mockResolvedValue({
      hasHooks: true,
      acknowledged: false,
    });
    const ack = vi.spyOn(mockIpc, 'ackRepoHooks').mockResolvedValue(undefined);
    const { result } = renderHook(() => useHookDisclosure(REPO));

    let p!: Promise<boolean>;
    act(() => {
      p = result.current.ensureHooksDisclosed(false);
    });
    await act(async () => {});
    expect(result.current.pendingHookDisclosure).toBe(true);

    act(() => result.current.onHookDiscloseCancel());
    expect(await p).toBe(false);
    expect(ack).not.toHaveBeenCalled();
    expect(result.current.pendingHookDisclosure).toBe(false);
  });

  it('caches within a session: a no-hooks result is not re-queried on the next op', async () => {
    const get = vi
      .spyOn(mockIpc, 'getRepoHooksDisclosure')
      .mockResolvedValue({ hasHooks: false, acknowledged: false });
    const { result } = renderHook(() => useHookDisclosure(REPO));

    await act(async () => {
      await result.current.ensureHooksDisclosed(false);
    });
    await act(async () => {
      await result.current.ensureHooksDisclosed(false);
    });
    expect(get).toHaveBeenCalledTimes(1);
  });

  it('after confirm, a second op proceeds with no dialog and no re-query (commit&push = one prompt)', async () => {
    const get = vi
      .spyOn(mockIpc, 'getRepoHooksDisclosure')
      .mockResolvedValue({ hasHooks: true, acknowledged: false });
    vi.spyOn(mockIpc, 'ackRepoHooks').mockResolvedValue(undefined);
    const { result } = renderHook(() => useHookDisclosure(REPO));

    let p!: Promise<boolean>;
    act(() => {
      p = result.current.ensureHooksDisclosed(false);
    });
    await act(async () => {});
    act(() => result.current.onHookDiscloseConfirm());
    await act(async () => {
      expect(await p).toBe(true);
    });

    // The push gate (second op) sees the session cache the commit gate set.
    let outcome!: boolean;
    await act(async () => {
      outcome = await result.current.ensureHooksDisclosed(false);
    });
    expect(outcome).toBe(true);
    expect(get).toHaveBeenCalledTimes(1);
    expect(result.current.pendingHookDisclosure).toBe(false);
  });
});
