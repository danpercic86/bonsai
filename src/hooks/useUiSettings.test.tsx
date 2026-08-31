/** useUiSettings — the persisted-UI-settings state machine extracted from App
 *  (P11c §3.2). This suite is the equivalence guard for that refactor and for
 *  every setting added to the hook afterwards: the 300 ms debounced COALESCING
 *  write, the pending-patch handoff across a flush, the `graph`-only
 *  `metricsVersion` bump, the save-failure toast copy, launch-time hydration,
 *  and the referential stability `handleSettingsChange` owes its children.
 *
 *  The P69b persist-path fixes (shared window for App-owned writers, retry of a
 *  failed write, unmount flush) live in `useUiSettings.writePath.test.tsx`;
 *  fixtures + mount helper are shared via `src/test/uiSettingsKit.ts`. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { mockIpc } from '../ipc/mock';
import { appErr } from '../test/actionHookKit';
import { deferred, GRAPH_PATCH, HYDRATED, mountUiSettings as mount } from '../test/uiSettingsKit';
import type { ToastTone } from '../components/Toasts';
import type { UiSettings } from '../ipc';

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

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
    // must be retained on its own rather than dropped or re-sent. (That empty
    // only sticks when the write SUCCEEDS — P69b merges a failed patch back;
    // see `useUiSettings.writePath.test.tsx`.)
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

describe('AI-run knobs (P68g)', () => {
  it('each knob patches on its own, previews live, and coalesces into ONE write', () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result } = mount();
    const before = result.current.metricsVersion;

    // The two LOCKED zeros are real values, not "unset": patching 0 must stick.
    act(() => result.current.handleSettingsChange({ aiConflictTools: 'none' }));
    act(() => result.current.handleSettingsChange({ aiIncludePartialMessages: true }));
    act(() => result.current.handleSettingsChange({ aiIdleTimeoutSecs: 0 }));
    act(() => result.current.handleSettingsChange({ aiHardCapSecs: 900 }));
    act(() => result.current.handleSettingsChange({ aiMaxTurns: 12 }));
    act(() => result.current.handleSettingsChange({ aiMaxBudgetUsd: 12.5 }));
    act(() => result.current.handleSettingsChange({ aiBulkMaxBytes: 800_000 }));

    expect(result.current.aiRun).toEqual({
      aiConflictTools: 'none',
      aiStreamLog: true,
      aiIncludePartialMessages: true,
      aiIdleTimeoutSecs: 0,
      aiHardCapSecs: 900,
      aiMaxTurns: 12,
      aiMaxBudgetUsd: 12.5,
      aiBulkMaxBytes: 800_000,
    });
    // None of them is a graph knob, so the canvas never re-measures.
    expect(result.current.metricsVersion).toBe(before);

    act(() => vi.advanceTimersByTime(300));
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0]).toEqual({
      aiConflictTools: 'none',
      aiIncludePartialMessages: true,
      aiIdleTimeoutSecs: 0,
      aiHardCapSecs: 900,
      aiMaxTurns: 12,
      aiMaxBudgetUsd: 12.5,
      aiBulkMaxBytes: 800_000,
    });
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
    // P68g: the eight AI-run knobs, exposed as one read-only struct. Asserted as a
    // whole so a field added to `AiRunPrefs` and forgotten in `hydrateUiSettings`
    // fails here instead of silently showing a default in Settings.
    expect(result.current.aiRun).toEqual({
      aiConflictTools: 'none',
      aiStreamLog: false,
      aiIncludePartialMessages: true,
      aiIdleTimeoutSecs: 120,
      aiHardCapSecs: 900,
      aiMaxTurns: 9,
      aiMaxBudgetUsd: 3,
      aiBulkMaxBytes: 200_000,
    });

    // Hydration is a read replay — it must never write back.
    expect(setSpy).not.toHaveBeenCalled();
  });
});

describe('referential stability', () => {
  it('handleSettingsChange and hydrateUiSettings survive a re-render unchanged', () => {
    const { result, rerender } = mount();
    const change = result.current.handleSettingsChange;
    const hydrate = result.current.hydrateUiSettings;
    // P69b: four App callbacks (toggleTheme, toggleListView, commitPaneWidths,
    // closeOnboarding) now list `queueSettingsWrite` in their deps, so its
    // identity churning would churn theirs — and their children's props.
    const queue = result.current.queueSettingsWrite;

    rerender();
    expect(result.current.handleSettingsChange).toBe(change);
    expect(result.current.hydrateUiSettings).toBe(hydrate);
    expect(result.current.queueSettingsWrite).toBe(queue);

    // ...and across a re-render caused by the hook's OWN state changing, since
    // children hold it as a prop for the whole session.
    vi.useFakeTimers();
    vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    act(() => result.current.handleSettingsChange({ graph: GRAPH_PATCH }));
    expect(result.current.metricsVersion).toBeGreaterThan(0); // did re-render
    expect(result.current.handleSettingsChange).toBe(change);
    expect(result.current.hydrateUiSettings).toBe(hydrate);
    expect(result.current.queueSettingsWrite).toBe(queue);
  });
});
