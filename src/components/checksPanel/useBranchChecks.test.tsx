/** P90 — useBranchChecks: the ChecksState machine (idle/loading/noForge/connect/
 *  noChecks/error/loaded), the last-wins guard + 300 ms debounce, and force-refresh. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { mockIpc } from '../../ipc/mock';
import { FORGE_REPO_CONTEXT } from '../../ipc/fixtures/forge';
import type { CommitStatus, ForgeRepoContext, StatusContext } from '../../ipc';
import { useBranchChecks } from './useBranchChecks';
import type { ChecksTarget } from './checksTarget';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const TARGET: ChecksTarget = { name: 'main', tip: 'a'.repeat(40), hasUpstream: true };

function ctx(over: Partial<ForgeRepoContext> = {}): ForgeRepoContext {
  return { ...FORGE_REPO_CONTEXT, authenticated: true, ...over };
}
function stat(contexts: StatusContext[]): CommitStatus {
  return { sha: TARGET.tip, state: 'success', total: contexts.length, passed: 0, failed: 0, pending: 0, contexts };
}
const one: StatusContext = { name: 'build', state: 'success', description: null, targetUrl: null };

type Deps = Parameters<typeof useBranchChecks>[0];
function mount(over: Partial<Deps> = {}) {
  const deps: Deps = { repoId: '/mock/repo', target: TARGET, refreshSeq: 0, active: true, ...over };
  return renderHook((d: Deps) => useBranchChecks(d), { initialProps: deps });
}
async function settle() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(350);
  });
}

describe('useBranchChecks', () => {
  it('is idle with no target', async () => {
    const { result } = mount({ target: null });
    await settle();
    expect(result.current.state.kind).toBe('idle');
  });

  it('loads contexts into the loaded state', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue(ctx());
    vi.spyOn(mockIpc, 'forgeCommitStatuses').mockResolvedValue([stat([one])]);
    const { result } = mount();
    await settle();
    expect(result.current.state.kind).toBe('loaded');
    expect(result.current.lastUpdated).not.toBeNull();
  });

  it('reports noChecks when the fetched status has no contexts', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue(ctx());
    vi.spyOn(mockIpc, 'forgeCommitStatuses').mockResolvedValue([stat([])]);
    const { result } = mount();
    await settle();
    expect(result.current.state.kind).toBe('noChecks');
  });

  it('reports noForge for an unknown provider', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue(ctx({ provider: 'unknown' }));
    const { result } = mount();
    await settle();
    expect(result.current.state.kind).toBe('noForge');
  });

  it('reports connect when the known provider is not authenticated', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue(ctx({ authenticated: false }));
    const { result } = mount();
    await settle();
    expect(result.current.state.kind).toBe('connect');
  });

  it('surfaces a fetch error', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue(ctx());
    vi.spyOn(mockIpc, 'forgeCommitStatuses').mockRejectedValue(new Error('boom'));
    const { result } = mount();
    await settle();
    expect(result.current.state.kind).toBe('error');
    if (result.current.state.kind === 'error') expect(result.current.state.message).toContain('boom');
  });

  it('refresh() forces a refetch', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue(ctx());
    const spy = vi.spyOn(mockIpc, 'forgeCommitStatuses').mockResolvedValue([stat([one])]);
    const { result } = mount();
    await settle();
    expect(spy).toHaveBeenCalledTimes(1);
    act(() => result.current.refresh());
    await settle();
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('reports noChecks with reason no-upstream for an unpushed branch', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue(ctx());
    vi.spyOn(mockIpc, 'forgeCommitStatuses').mockResolvedValue([stat([])]);
    const local: ChecksTarget = { ...TARGET, hasUpstream: false };
    const { result } = mount({ target: local });
    await settle();
    expect(result.current.state.kind).toBe('noChecks');
    if (result.current.state.kind === 'noChecks')
      expect(result.current.state.reason).toBe('no-upstream');
  });

  it('keeps last-good rows on a failed refetch (stale-while-error)', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue(ctx());
    const spy = vi
      .spyOn(mockIpc, 'forgeCommitStatuses')
      .mockResolvedValue([stat([one])]);
    const { result } = mount();
    await settle();
    expect(result.current.state.kind).toBe('loaded');

    spy.mockRejectedValueOnce(new Error('offline'));
    act(() => result.current.refresh());
    await settle();
    expect(result.current.state.kind).toBe('error');
    if (result.current.state.kind === 'error') {
      expect(result.current.state.stale).not.toBeNull();
      expect(result.current.state.message).toContain('offline');
    }
    expect(result.current.failedRefreshAt).not.toBeNull();
  });

  it('does not fetch while inactive', async () => {
    const spy = vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue(ctx());
    mount({ active: false });
    await settle();
    expect(spy).not.toHaveBeenCalled();
  });
});
