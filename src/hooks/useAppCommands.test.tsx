/**
 * P69h — the App-level command-palette table, and the invariant its extraction
 * from `App.tsx` put at risk.
 *
 * `useAppCommands` feeds `RepoWorkspace`'s palette memo, whose result is
 * `CommandPalette`'s `actions` prop, and the palette re-runs
 * `setHighlight(firstEnabledIndex(flat))` whenever that array's identity
 * changes. A memo that recomputes on every App render therefore snaps the
 * highlight back to row 0 while the user is typing — so array identity is a
 * behavioural contract, not an optimisation, and it is pinned here in both
 * directions: stable across an unrelated re-render, and a negative control
 * proving the assertion can actually fail.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, renderHook, screen, waitFor } from '@testing-library/react';

import { useAppCommands, type AppCommandDeps } from './useAppCommands';
import type { PaletteAction } from '../components/paletteActions';

const captured = vi.hoisted(() => ({ appCommands: [] as PaletteAction[][] }));

vi.mock('../components/RepoWorkspace', () => ({
  RepoWorkspace: (props: { appCommands: PaletteAction[] }) => {
    captured.appCommands.push(props.appCommands);
    return <div className="workspace-host" data-testid="workspace-stub" />;
  },
}));

import App from '../App';
import { mockIpc } from '../ipc/mock';
import { DEFAULT_UI_SETTINGS } from '../ipc/mock/persistence';

function deps(over: Partial<AppCommandDeps> = {}): AppCommandDeps {
  return {
    activeRepo: '/repo',
    openRepository: vi.fn(async () => {}),
    cloneOpen: vi.fn(),
    initRepository: vi.fn(async () => {}),
    openSettingsAt: vi.fn(),
    setAiAssetsOpen: vi.fn(),
    setHealthOpen: vi.fn(),
    setOverlayOpen: vi.fn(),
    toggleTheme: vi.fn(),
    toggleListView: vi.fn(),
    gitRecheck: vi.fn(async () => null),
    pushToast: vi.fn(),
    ...over,
  };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  captured.appCommands = [];
});

describe('useAppCommands — identity', () => {
  it('is stable when re-rendered with a FRESH deps object of the same members', () => {
    // Exactly what App does: a new object literal every render, whose members are
    // all `useCallback`s / setState setters.
    const stable = deps();
    const { result, rerender } = renderHook((p: AppCommandDeps) => useAppCommands(p), {
      initialProps: { ...stable },
    });
    const first = result.current;
    rerender({ ...stable });
    rerender({ ...stable });
    expect(result.current).toBe(first);
  });

  it('NEGATIVE CONTROL: one inline arrow among the deps breaks it', () => {
    // The shape the extraction originally shipped. If this ever passes, the test
    // above has stopped proving anything.
    const stable = deps();
    const { result, rerender } = renderHook((p: AppCommandDeps) => useAppCommands(p), {
      initialProps: { ...stable, setHealthOpen: () => {} },
    });
    const first = result.current;
    rerender({ ...stable, setHealthOpen: () => {} });
    expect(result.current).not.toBe(first);
  });

  it('recomputes when the repo changes, and gates Git config on it', () => {
    const stable = deps({ activeRepo: null });
    const { result, rerender } = renderHook((p: AppCommandDeps) => useAppCommands(p), {
      initialProps: { ...stable },
    });
    const first = result.current;
    const gitConfig = (list: PaletteAction[]) => list.find((a) => a.id === 'app.gitConfig');
    expect(gitConfig(first)?.disabled).toBe(true);

    rerender({ ...stable, activeRepo: '/repo' });
    expect(result.current).not.toBe(first);
    expect(gitConfig(result.current)?.disabled).toBe(false);
  });

  it('deep-links Settings without opening it twice', () => {
    const openSettingsAt = vi.fn();
    const { result } = renderHook(() => useAppCommands(deps({ openSettingsAt })));
    result.current.find((a) => a.id === 'app.gitConfig')?.run();
    result.current.find((a) => a.id === 'app.identities')?.run();
    result.current.find((a) => a.id === 'app.settings')?.run();
    expect(openSettingsAt.mock.calls).toEqual([['git-config'], ['identities'], [null]]);
  });
});

describe('App wiring', () => {
  /** The regression the reviewer caught: App passed five freshly-allocated
   *  arrows into the hook, so `appCommands` was a new array on every render. */
  it('hands RepoWorkspace the SAME appCommands array across an unrelated re-render', async () => {
    vi.spyOn(mockIpc, 'getUiSettings').mockResolvedValue({
      ...structuredClone(DEFAULT_UI_SETTINGS),
      onboardingSeen: true,
    });
    vi.spyOn(mockIpc, 'getRecentRepos').mockResolvedValue([]);
    vi.spyOn(mockIpc, 'getSession').mockResolvedValue({
      openRepos: ['/mock/repo'],
      activeRepo: '/mock/repo',
    });
    vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(undefined as never);
    vi.spyOn(mockIpc, 'setSession').mockResolvedValue(undefined as never);

    render(<App />);
    await screen.findByTestId('workspace-stub');
    const before = captured.appCommands.length;
    const first = captured.appCommands[before - 1];

    // Opening Settings is App state the palette knows nothing about.
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));

    await waitFor(() => expect(captured.appCommands.length).toBeGreaterThan(before));
    for (const list of captured.appCommands) expect(list).toBe(first);
  });
});
