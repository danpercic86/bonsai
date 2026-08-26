/** spec-002 — persistence + hydration of the two additive graph-theme prefs
 *  (`graphStyle` / `graphSeason`). Runs in the `dom` project (VITE_MOCK_IPC=1),
 *  so `ipc` IS `mockIpc` and localStorage is real jsdom storage.
 *
 *  Split across three layers, matching where the behavior actually lives:
 *   - mock IPC round-trip: a write survives a re-read AND lands in localStorage
 *     (that is the "reload" story — every read re-parses the blob);
 *   - legacy blob: a stored blob lacking both keys re-reads with them DEFAULTED
 *     to 'standard' / 'living', exactly like every other additive field (a
 *     missing/wrong-shape blob reads back as a complete UiSettings);
 *   - hook defaulting: hydrating a settings object without the fields yields
 *     'standard' / 'living', and a style patch does NOT bump metricsVersion. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { mockIpc } from '../ipc/mock';
import { HYDRATED, mountUiSettings as mount } from '../test/uiSettingsKit';

const UI_SETTINGS_KEY = 'bonsai.mockUiSettings';

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
  window.localStorage.clear();
});

describe('mock IPC round-trip (real timers — handlers await delay)', () => {
  beforeEach(() => window.localStorage.clear());

  it('persists graphStyle/graphSeason through setUiSettings → getUiSettings', async () => {
    await mockIpc.setUiSettings({ graphStyle: 'bonsai', graphSeason: 'autumn' });
    const reread = await mockIpc.getUiSettings();
    expect(reread.graphStyle).toBe('bonsai');
    expect(reread.graphSeason).toBe('autumn');
  });

  it('writes the values into localStorage so a reload (fresh read) recovers them', async () => {
    await mockIpc.setUiSettings({ graphStyle: 'bonsai', graphSeason: 'spring' });
    const raw = window.localStorage.getItem(UI_SETTINGS_KEY);
    expect(raw).not.toBeNull();
    const blob = JSON.parse(raw as string);
    expect(blob.graphStyle).toBe('bonsai');
    expect(blob.graphSeason).toBe('spring');
    // A completely fresh read (simulating relaunch) still sees them.
    const afterReload = await mockIpc.getUiSettings();
    expect(afterReload.graphStyle).toBe('bonsai');
    expect(afterReload.graphSeason).toBe('spring');
  });

  it('a stored blob missing both keys re-reads with them DEFAULTED (standard/living)', async () => {
    // Seed a minimal legacy blob lacking both keys.
    window.localStorage.setItem(
      UI_SETTINGS_KEY,
      JSON.stringify({ theme: 'light', paneWidths: { sidebar: 300, rightPanel: 400 } }),
    );
    const s = await mockIpc.getUiSettings();
    // Correct app behavior: read fills the keys from the shared defaults, exactly
    // like every other additive field — a legacy/missing blob reads back as a
    // complete UiSettings (the keys are pinned in the defaults oracle).
    expect(s.graphStyle).toBe('standard');
    expect(s.graphSeason).toBe('living');
    expect(s.theme).toBe('light');
  });
});

describe('hook defaulting (useUiSettings)', () => {
  it('hydrating a settings object without the fields defaults to standard/living', () => {
    // HYDRATED is a full UiSettings that omits graphStyle/graphSeason (optional).
    expect(HYDRATED.graphStyle).toBeUndefined();
    expect(HYDRATED.graphSeason).toBeUndefined();
    const { result } = mount();
    act(() => result.current.hydrateUiSettings(HYDRATED));
    expect(result.current.graphStyle).toBe('standard');
    expect(result.current.graphSeason).toBe('living');
  });

  it('hydrating explicit values seeds them', () => {
    const { result } = mount();
    act(() =>
      result.current.hydrateUiSettings({ ...HYDRATED, graphStyle: 'bonsai', graphSeason: 'autumn' }),
    );
    expect(result.current.graphStyle).toBe('bonsai');
    expect(result.current.graphSeason).toBe('autumn');
  });

  it('a style/season patch previews live but does NOT bump metricsVersion (palette-only)', () => {
    vi.useFakeTimers();
    vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result } = mount();
    const before = result.current.metricsVersion;

    act(() => result.current.handleSettingsChange({ graphStyle: 'bonsai' }));
    act(() => result.current.handleSettingsChange({ graphSeason: 'spring' }));

    expect(result.current.graphStyle).toBe('bonsai');
    expect(result.current.graphSeason).toBe('spring');
    expect(result.current.metricsVersion).toBe(before);
    act(() => vi.advanceTimersByTime(300));
    expect(result.current.metricsVersion).toBe(before);
  });
});
