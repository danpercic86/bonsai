/** useUiSettings — the persisted-UI-settings state machine extracted from App
 *  (P11c §3.2). This suite is the equivalence guard for that refactor and for
 *  every setting added to the hook afterwards: the 300 ms debounced COALESCING
 *  write, the pending-patch handoff across a flush, the `graph`-only
 *  `metricsVersion` bump, the save-failure toast copy, launch-time hydration,
 *  and the referential stability `handleSettingsChange` owes its children.
 *
 *  House pattern (see useUpdateController.test.tsx): the `dom` vitest project
 *  runs with VITE_MOCK_IPC=1, so `ipc` IS `mockIpc` — spy on it directly. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../ipc/mock';
import { useUiSettings } from './useUiSettings';
import { appErr } from '../test/actionHookKit';
import type { ToastTone } from '../components/Toasts';
import type { GraphPrefs, UiSettings } from '../ipc';

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** A complete non-default `UiSettings`, used both as the resolved value of the
 *  mocked write and as the hydration payload. Every field differs from the
 *  hook's own defaults so hydration assertions cannot pass vacuously. A new
 *  UiSettings field breaks this literal at compile time — on purpose. */
const HYDRATED: UiSettings = {
  theme: 'light',
  paneWidths: { sidebar: 300, rightPanel: 420 },
  listView: 'flat',
  panelDensity: 'compact',
  autoFetch: { enabled: true, intervalMinutes: 11 },
  healthRefresh: { enabled: true, intervalMinutes: 45 },
  graph: {
    avatarRadius: 14,
    rowHeight: 40,
    laneWidth: 22,
    showSha: false,
    showAuthor: true,
    showDate: false,
    dateBasis: 'committer',
    showAheadBehind: false,
    compact: true,
    showSignatureBadge: false,
    showPrBadge: true,
    showCiStatus: true,
  },
  aiEnabled: false,
  aiConflictAutonomy: 'autoResolve',
  aiConsented: true,
  mcpConsented: true,
  mcpWriteConsented: true,
  onboardingSeen: true,
  autoCheckUpdates: true,
  profiles: [
    { id: 'p1', label: 'Work', userName: 'A Dev', userEmail: 'dev@example.com', signingKey: null },
  ],
  terminalCommand: 'wt.exe -d {path}',
  editorCommand: 'code {path}',
  aiIdleTimeoutSecs: 120,
  aiHardCapSecs: 900,
  aiMaxTurns: 9,
  aiStreamLog: false,
  aiIncludePartialMessages: true,
  aiConflictTools: 'none',
  aiBulkMaxBytes: 200_000,
  aiMaxBudgetUsd: 3,
  aiDockHeight: 320,
  aiDockCollapsed: true,
};

/** A graph patch that differs from the hook's defaults in a few knobs. */
const GRAPH_PATCH: GraphPrefs = { ...HYDRATED.graph, rowHeight: 36 };

/** Stable toast pusher — identity must not churn, or behaviour 6 is untestable. */
function mount(push: (tone: ToastTone, text: string) => void = vi.fn()) {
  return { push, ...renderHook(() => useUiSettings(push)) };
}

describe('debounced coalescing write', () => {
  it('a burst of three patches produces ONE merged write after 300 ms', () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result } = mount();

    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    act(() => vi.advanceTimersByTime(150));
    act(() => result.current.handleSettingsChange({ aiDockHeight: 240 }));
    act(() => result.current.handleSettingsChange({ aiDockHeight: 260, aiStreamLog: false }));

    // Live preview is immediate — before any write has happened.
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.panelDensity).toBe('compact');
    expect(result.current.aiDockHeight).toBe(260);
    expect(result.current.aiStreamLog).toBe(false);

    // The window is re-armed by each call, so 299 ms after the LAST one: silent.
    act(() => vi.advanceTimersByTime(299));
    expect(spy).not.toHaveBeenCalled();

    act(() => vi.advanceTimersByTime(1));
    expect(spy).toHaveBeenCalledTimes(1);
    // Merged, last-write-wins, and nothing the user did not touch.
    expect(spy.mock.calls[0][0]).toEqual({
      panelDensity: 'compact',
      aiDockHeight: 260,
      aiStreamLog: false,
    });

    // The window does not fire twice.
    act(() => vi.advanceTimersByTime(1000));
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('a patch arriving while a write is in flight lands in a SECOND write, not the first', async () => {
    vi.useFakeTimers();
    const inFlight = deferred<UiSettings>();
    const spy = vi
      .spyOn(mockIpc, 'setUiSettings')
      .mockReturnValueOnce(inFlight.promise)
      .mockResolvedValue(HYDRATED);
    const { result } = mount();

    act(() => result.current.handleSettingsChange({ aiDockHeight: 200 }));
    act(() => vi.advanceTimersByTime(300)); // flush starts; write unresolved
    expect(spy).toHaveBeenCalledTimes(1);

    // Mid-flight patch: the pending ref was emptied before the call, so this
    // must be retained on its own rather than dropped or re-sent.
    act(() => result.current.handleSettingsChange({ aiDockCollapsed: true }));
    await act(async () => {
      inFlight.resolve(HYDRATED);
      await vi.advanceTimersByTimeAsync(300);
    });

    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[0][0]).toEqual({ aiDockHeight: 200 });
    expect(spy.mock.calls[1][0]).toEqual({ aiDockCollapsed: true }); // no stale re-send
    expect(result.current.aiDockHeight).toBe(200);
    expect(result.current.aiDockCollapsed).toBe(true);
  });

  it('unmount does NOT cancel a pending write (no cleanup — pre-existing)', () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result, unmount } = mount();

    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    unmount();
    act(() => vi.advanceTimersByTime(300));

    // Documents the shipped behaviour: the timer survives unmount, so the patch
    // still reaches disk. Only tearing down the JS context (app quit) inside the
    // 300 ms window loses it.
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0]).toEqual({ panelDensity: 'compact' });
  });
});

describe('metricsVersion', () => {
  it('a graph patch updates graph AND bumps metricsVersion', () => {
    vi.useFakeTimers();
    vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result } = mount();
    const before = result.current.metricsVersion;

    act(() => result.current.handleSettingsChange({ graph: GRAPH_PATCH }));

    expect(result.current.graph).toEqual(GRAPH_PATCH);
    expect(result.current.metricsVersion).toBe(before + 1);

    act(() => result.current.handleSettingsChange({ graph: { ...GRAPH_PATCH, laneWidth: 24 } }));
    expect(result.current.metricsVersion).toBe(before + 2); // once per graph change
  });

  it('a NON-graph patch leaves metricsVersion alone (no canvas re-measure)', () => {
    vi.useFakeTimers();
    vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result } = mount();
    const before = result.current.metricsVersion;

    act(() =>
      result.current.handleSettingsChange({
        panelDensity: 'compact',
        aiDockHeight: 240,
        aiEnabled: false,
        autoFetch: { enabled: true, intervalMinutes: 7 },
      }),
    );

    expect(result.current.panelDensity).toBe('compact');
    expect(result.current.metricsVersion).toBe(before);
    act(() => vi.advanceTimersByTime(300));
    expect(result.current.metricsVersion).toBe(before); // nor after the write
  });
});

describe('save failure', () => {
  it('a rejected write pushes an error toast with the exact copy prefix', async () => {
    vi.useFakeTimers();
    vi.spyOn(mockIpc, 'setUiSettings').mockRejectedValue(appErr('io', 'disk on fire'));
    const push = vi.fn();
    const { result } = mount(push);

    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });

    expect(push).toHaveBeenCalledTimes(1);
    const [tone, text] = push.mock.calls[0] as [ToastTone, string];
    expect(tone).toBe('error');
    expect(text.startsWith('Could not save settings: ')).toBe(true);
    expect(text).toBe('Could not save settings: disk on fire');
    // A failed write does not roll back the live preview.
    expect(result.current.panelDensity).toBe('compact');
  });

  it('a successful write pushes no toast', async () => {
    vi.useFakeTimers();
    vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const push = vi.fn();
    const { result } = mount(push);

    act(() => result.current.handleSettingsChange({ aiDockCollapsed: true }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });

    expect(push).not.toHaveBeenCalled();
  });
});

describe('hydrateUiSettings', () => {
  it('seeds the owned fields from the launch-time read and bumps metricsVersion', () => {
    const setSpy = vi.spyOn(mockIpc, 'setUiSettings');
    const { result } = mount();
    const before = result.current.metricsVersion;

    act(() => result.current.hydrateUiSettings(HYDRATED));

    // A representative spread: struct, enum, booleans, numbers, string, array.
    expect(result.current.graph).toEqual(HYDRATED.graph);
    expect(result.current.metricsVersion).toBe(before + 1);
    expect(result.current.panelDensity).toBe('compact');
    expect(result.current.aiConflictAutonomy).toBe('autoResolve');
    expect(result.current.aiEnabled).toBe(false);
    expect(result.current.aiConsented).toBe(true);
    expect(result.current.mcpConsented).toBe(true);
    expect(result.current.mcpWriteConsented).toBe(true);
    expect(result.current.autoCheckUpdates).toBe(true);
    expect(result.current.autoFetch).toEqual(HYDRATED.autoFetch);
    expect(result.current.healthRefresh).toEqual(HYDRATED.healthRefresh);
    expect(result.current.profiles).toEqual(HYDRATED.profiles);
    expect(result.current.terminalCommand).toBe('wt.exe -d {path}');
    expect(result.current.editorCommand).toBe('code {path}');
    expect(result.current.aiDockHeight).toBe(320);
    expect(result.current.aiDockCollapsed).toBe(true);
    expect(result.current.aiStreamLog).toBe(false);

    // Hydration is a read replay — it must never write back.
    expect(setSpy).not.toHaveBeenCalled();
  });
});

describe('referential stability', () => {
  it('handleSettingsChange and hydrateUiSettings survive a re-render unchanged', () => {
    const { result, rerender } = mount();
    const change = result.current.handleSettingsChange;
    const hydrate = result.current.hydrateUiSettings;

    rerender();
    expect(result.current.handleSettingsChange).toBe(change);
    expect(result.current.hydrateUiSettings).toBe(hydrate);

    // ...and across a re-render caused by the hook's OWN state changing, since
    // children hold it as a prop for the whole session.
    vi.useFakeTimers();
    vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    act(() => result.current.handleSettingsChange({ graph: GRAPH_PATCH }));
    expect(result.current.metricsVersion).toBeGreaterThan(0); // did re-render
    expect(result.current.handleSettingsChange).toBe(change);
    expect(result.current.hydrateUiSettings).toBe(hydrate);
  });
});
