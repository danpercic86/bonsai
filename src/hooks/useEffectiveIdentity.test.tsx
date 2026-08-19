/**
 * P69 §5.1 — the effective-identity store.
 *
 * The bug this module exists to kill: reading only the repo's LOCAL config and
 * calling that "the identity". Git resolves local-overrides-global, so the tests
 * below pin (a) precedence, (b) that ONE `getConfig(repoId, 'local')` answers both
 * levels (curated entries already carry effectiveValue/effectiveLevel), and (c) that
 * two consumers can never disagree — which is the whole reason the state is
 * module-level rather than per-component.
 */
import { describe, expect, it, vi, afterEach, beforeEach } from 'vitest';
import { act, render, renderHook, screen, waitFor } from '@testing-library/react';

import { mockIpc } from '../ipc/mock';
import type { ConfigLevelName, ConfigView, CuratedConfigEntry } from '../ipc';
import {
  invalidateEffectiveIdentity,
  resetEffectiveIdentityForTests,
  useEffectiveIdentity,
} from './useEffectiveIdentity';

function curated(
  key: string,
  effectiveValue: string | null,
  effectiveLevel: ConfigLevelName | null,
  targetValue: string | null = null,
): CuratedConfigEntry {
  return { key, kind: 'text', enumValues: [], effectiveValue, effectiveLevel, targetValue };
}

/** A ConfigView as `getConfig(repo, 'local')` returns it: curated entries hold the
 *  EFFECTIVE value + the level it came from, `targetValue` only the local one. */
function view(entries: CuratedConfigEntry[], advanced: ConfigView['advanced'] = []): ConfigView {
  return { targetLevel: 'local', curated: entries, advanced };
}

const LOCAL_VIEW = view([
  curated('user.name', 'Local Ada', 'local', 'Local Ada'),
  curated('user.email', 'ada@local.dev', 'local', 'ada@local.dev'),
]);
const GLOBAL_VIEW = view([
  curated('user.name', 'Global Ada', 'global'),
  curated('user.email', 'ada@global.dev', 'global'),
]);

beforeEach(() => {
  resetEffectiveIdentityForTests();
});
afterEach(() => {
  vi.restoreAllMocks();
  resetEffectiveIdentityForTests();
});

describe('useEffectiveIdentity', () => {
  it('a local identity wins over the global one and reports source "local"', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(LOCAL_VIEW);
    const { result } = renderHook(() => useEffectiveIdentity('/repo/a'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.name).toBe('Local Ada');
    expect(result.current.email).toBe('ada@local.dev');
    expect(result.current.source).toBe('local');
    expect(result.current.error).toBeNull();
  });

  it('an empty local block still yields the GLOBAL identity (the D6 bug)', async () => {
    // targetValue null everywhere = nothing set locally, exactly the default
    // harness state — the old local-only logic reported "no identity" here.
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(GLOBAL_VIEW);
    const { result } = renderHook(() => useEffectiveIdentity('/repo/b'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.name).toBe('Global Ada');
    expect(result.current.email).toBe('ada@global.dev');
    expect(result.current.source).toBe('global');
  });

  it('unset everywhere (and blank values) reads as no identity, not as ""', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      view([curated('user.name', '   ', 'global'), curated('user.email', null, null)]),
    );
    const { result } = renderHook(() => useEffectiveIdentity('/repo/c'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.name).toBeNull();
    expect(result.current.email).toBeNull();
    expect(result.current.source).toBeNull();
  });

  it('falls back to user.email’s level when only the email is set', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      view([curated('user.name', null, null), curated('user.email', 'ada@global.dev', 'global')]),
    );
    const { result } = renderHook(() => useEffectiveIdentity('/repo/c2'));

    await waitFor(() => expect(result.current.source).toBe('global'));
    expect(result.current.name).toBeNull();
  });

  it('exposes a LOCAL user.signingkey from the advanced list', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      view(LOCAL_VIEW.curated, [
        { name: 'user.signingkey', value: 'ABC123', level: 'local' },
        { name: 'core.autocrlf', value: 'input', level: 'local' },
      ]),
    );
    const { result } = renderHook(() => useEffectiveIdentity('/repo/c3'));

    await waitFor(() => expect(result.current.signingKey).toBe('ABC123'));
  });

  it('a rejected getConfig yields the error state, never a confident wrong identity', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockRejectedValue(new Error('config unreadable'));
    const { result } = renderHook(() => useEffectiveIdentity('/repo/d'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toMatch(/config unreadable/);
    expect(result.current.name).toBeNull();
    expect(result.current.email).toBeNull();
    expect(result.current.source).toBeNull();
  });

  it('repoId === null makes no IPC call at all', async () => {
    const spy = vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(LOCAL_VIEW);
    const { result } = renderHook(() => useEffectiveIdentity(null));

    await Promise.resolve();
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.loading).toBe(false);
    expect(result.current.name).toBeNull();
  });

  it('calls getConfig ONCE per repo across re-renders, remounts and a second consumer', async () => {
    const spy = vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(LOCAL_VIEW);
    const first = renderHook(() => useEffectiveIdentity('/repo/e'));
    await waitFor(() => expect(first.result.current.name).toBe('Local Ada'));

    first.rerender();
    const second = renderHook(() => useEffectiveIdentity('/repo/e'));
    await waitFor(() => expect(second.result.current.name).toBe('Local Ada'));
    first.unmount();
    renderHook(() => useEffectiveIdentity('/repo/e'));

    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy).toHaveBeenCalledWith('/repo/e', 'local');
  });

  it('a second mount DURING the first fetch does not issue a second call', async () => {
    let resolve: ((v: ConfigView) => void) | null = null;
    const spy = vi
      .spyOn(mockIpc, 'getConfig')
      .mockReturnValue(
        new Promise<ConfigView>((r) => {
          resolve = r;
        }),
      );
    const a = renderHook(() => useEffectiveIdentity('/repo/f'));
    const b = renderHook(() => useEffectiveIdentity('/repo/f'));
    expect(a.result.current.loading).toBe(true);
    expect(spy).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolve?.(LOCAL_VIEW);
    });
    await waitFor(() => expect(b.result.current.name).toBe('Local Ada'));
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('a superseded reply does not release the in-flight marker of the NEWER read', async () => {
    // load (seq 0, fetch1) -> invalidate (seq 1, fetch2) -> fetch1 settles late. If the
    // stale reply cleared `inFlight`, the next mount would fire a THIRD getConfig while
    // fetch2 is still open — the dedupe §5.1 requires, defeated.
    const pending: ((v: ConfigView) => void)[] = [];
    const spy = vi.spyOn(mockIpc, 'getConfig').mockImplementation(
      () =>
        new Promise<ConfigView>((r) => {
          pending.push(r);
        }),
    );
    const a = renderHook(() => useEffectiveIdentity('/repo/k'));
    expect(spy).toHaveBeenCalledTimes(1);

    act(() => invalidateEffectiveIdentity('/repo/k'));
    expect(spy).toHaveBeenCalledTimes(2);

    // The FIRST (now superseded) fetch answers.
    await act(async () => {
      pending[0](GLOBAL_VIEW);
    });
    expect(a.result.current.loading).toBe(true);

    renderHook(() => useEffectiveIdentity('/repo/k'));
    expect(spy).toHaveBeenCalledTimes(2);

    await act(async () => {
      pending[1](LOCAL_VIEW);
    });
    await waitFor(() => expect(a.result.current.name).toBe('Local Ada'));
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('a different repo is a different cache entry (one call each)', async () => {
    const spy = vi
      .spyOn(mockIpc, 'getConfig')
      .mockImplementation((repoId: string) =>
        Promise.resolve(repoId === '/repo/g' ? LOCAL_VIEW : GLOBAL_VIEW),
      );
    const { result, rerender } = renderHook(({ id }: { id: string }) => useEffectiveIdentity(id), {
      initialProps: { id: '/repo/g' },
    });
    await waitFor(() => expect(result.current.name).toBe('Local Ada'));

    rerender({ id: '/repo/h' });
    await waitFor(() => expect(result.current.name).toBe('Global Ada'));
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('invalidation refetches and every subscriber sees the NEW value', async () => {
    const spy = vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(GLOBAL_VIEW);
    const a = renderHook(() => useEffectiveIdentity('/repo/i'));
    const b = renderHook(() => useEffectiveIdentity('/repo/i'));
    await waitFor(() => expect(a.result.current.name).toBe('Global Ada'));
    expect(b.result.current.name).toBe('Global Ada');

    spy.mockResolvedValue(LOCAL_VIEW);
    await act(async () => {
      invalidateEffectiveIdentity('/repo/i');
    });

    await waitFor(() => expect(a.result.current.name).toBe('Local Ada'));
    expect(b.result.current.name).toBe('Local Ada');
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('two consumers in ONE tree render the same value from one call', async () => {
    const spy = vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(GLOBAL_VIEW);
    function Probe({ tag }: { tag: string }) {
      const id = useEffectiveIdentity('/repo/j');
      return <span data-testid={tag}>{id.loading ? 'loading' : (id.name ?? 'none')}</span>;
    }
    render(
      <>
        <Probe tag="one" />
        <Probe tag="two" />
      </>,
    );

    await waitFor(() => expect(screen.getByTestId('one')).toHaveTextContent('Global Ada'));
    expect(screen.getByTestId('two')).toHaveTextContent('Global Ada');
    expect(spy).toHaveBeenCalledTimes(1);
  });
});
