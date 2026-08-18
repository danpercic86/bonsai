/**
 * P68e — the container-side dock glue: store→props mapping, the once-per-run
 * auto-expand, the palette rows and the two explicit entry points.
 *
 * The auto-expand guard is the delicate half of U6: expanding is harmless and must
 * happen, but it must happen ONCE per run, or a user who deliberately collapsed the
 * dock is fighting it on every re-render.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { batch, ev, gatedStream, makeDeps, stubStream } from '../../test/aiRunsKit';
import { useAiDock } from './useAiDock';
import { useAiRuns } from './useAiRuns';
import type { AiActivityPanelHandle } from '../AiActivityPanel';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const EXPAND = { aiDockCollapsed: false };

function harness() {
  const onChange = vi.fn();
  const focusReply = vi.fn();
  const focusLog = vi.fn();
  // Created ONCE, outside the render callback: `useAiDock` documents these as stable
  // (they are `useCallback`s in `RepoWorkspace`), and fresh mocks per render would hide
  // the very memo churn the stability test below exists to catch.
  const onAskBonsai = vi.fn();
  const onChangelog = vi.fn();
  const view = renderHook(() => {
    const aiRuns = useAiRuns(makeDeps());
    const dock = useAiDock({
      aiRuns,
      height: 180,
      collapsed: false,
      onChange,
      density: 'cozy',
      streamLogEnabled: true,
      aiEligible: true,
      onAskBonsai,
      onChangelog,
    });
    // The panel is not rendered here, so stand in for its imperative handle.
    const ref = dock.panelProps.ref as { current: AiActivityPanelHandle | null };
    ref.current = { focusReply, focusLog };
    return { aiRuns, dock };
  });
  return { ...view, onChange, focusReply, focusLog };
}

describe('useAiDock', () => {
  it('maps the store onto ready-to-spread panel props, with a derived elapsedMs', () => {
    stubStream();
    const { result } = harness();
    act(() => result.current.aiRuns.startConflictRun('a.ts'));
    act(() => void vi.advanceTimersByTime(3_000));

    const props = result.current.dock.panelProps;
    expect(props.runs).toHaveLength(1);
    expect(props.runs[0]).toMatchObject({
      key: 'conflict:a.ts',
      label: 'a.ts',
      status: 'running',
    });
    expect(props.runs[0]?.elapsedMs).toBeGreaterThanOrEqual(3_000);
    // The cursor defaults to the newest run so the body always has content.
    expect(props.activeKey).toBe('conflict:a.ts');
    expect(props.density).toBe('cozy');
    expect(props.streamLogEnabled).toBe(true);
  });

  it('auto-expands ONCE when a run starts asking, and not again for the same run', () => {
    const stream = stubStream();
    const { result, onChange } = harness();
    act(() => result.current.aiRuns.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));
    expect(onChange).not.toHaveBeenCalled();

    act(() => stream.send(ev({ seq: 1, kind: 'awaitingInput', text: 'which plural?' })));
    expect(onChange).toHaveBeenCalledExactlyOnceWith(EXPAND);
    expect(result.current.dock.awaitingInput).toBe(true);

    act(() => void vi.advanceTimersByTime(2_000));
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it('focusDock expands and reaches the reply box while blocked, the log otherwise', () => {
    const stream = stubStream();
    const { result, onChange, focusReply, focusLog } = harness();
    act(() => result.current.aiRuns.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));

    act(() => result.current.dock.focusDock());
    act(() => void vi.advanceTimersByTime(1));
    expect(onChange).toHaveBeenLastCalledWith(EXPAND);
    expect(focusLog).toHaveBeenCalledTimes(1);
    expect(focusReply).not.toHaveBeenCalled();

    act(() => stream.send(ev({ seq: 1, kind: 'awaitingInput', text: 'q?' })));
    act(() => result.current.dock.focusDock());
    act(() => void vi.advanceTimersByTime(1));
    expect(focusReply).toHaveBeenCalledTimes(1);
  });

  it('focusDock is inert with no runs — an empty dock is never advertised', () => {
    const { result, onChange } = harness();
    act(() => result.current.dock.focusDock());
    expect(onChange).not.toHaveBeenCalled();
    expect(result.current.dock.paletteEntries.trail).toEqual([]);
  });

  it('offers the AI activity palette row once a run exists, and Answer only while blocked', () => {
    const stream = stubStream();
    const { result } = harness();
    // With no run, only the two P55c/P56b leads are offered.
    expect(result.current.dock.paletteEntries.lead.map((a) => a.id)).toEqual([
      'ai.ask',
      'ai.changelog',
    ]);

    act(() => result.current.aiRuns.startConflictRun('a.ts'));
    expect(result.current.dock.paletteEntries.trail.map((a) => a.id)).toEqual(['ai.activity']);

    act(() => stream.send(ev({ seq: 0, kind: 'started' })));
    act(() => stream.send(ev({ seq: 1, kind: 'awaitingInput', text: 'q?' })));
    expect(result.current.dock.paletteEntries.trail.map((a) => a.id)).toEqual([
      'ai.activity',
      'ai.answer',
    ]);
  });

  /**
   * S2 — the palette rows must NOT be rebuilt on every store commit.
   *
   * `paletteEntries` is spread into `RepoWorkspace`'s `paletteActions` memo, and
   * `CommandPalette` resets its highlighted row whenever the `actions` array identity
   * changes. `focusDock` used to close over `aiRuns.orderedRuns`, whose identity moves
   * on EVERY commit — i.e. roughly once a second while a run streams, so the palette's
   * selection jumped back to the first row about once a second.
   */
  it('keeps the palette rows referentially stable across store commits', () => {
    const stream = stubStream();
    const { result } = harness();
    act(() => result.current.aiRuns.startConflictRun('a.ts'));
    const first = result.current.dock.paletteEntries;
    const focus = result.current.dock.focusDock;

    // Three things that each commit the store: a log line, the 1 s elapsed tick, and a
    // metrics-only heartbeat. None of them changes WHICH rows the palette offers.
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));
    act(() => stream.send(ev({ seq: 1, kind: 'log', text: '⚙ Read(a.ts)' })));
    act(() => void vi.advanceTimersByTime(2_000));
    act(() => stream.send(ev({ seq: 2, kind: 'log', thinkingTokens: 300 })));
    act(() => void vi.advanceTimersByTime(2_000));

    expect(result.current.dock.focusDock).toBe(focus);
    expect(result.current.dock.paletteEntries).toBe(first);

    // It still rebuilds when the OFFER actually changes (a new blocked run).
    act(() => stream.send(ev({ seq: 3, kind: 'awaitingInput', text: 'q?' })));
    expect(result.current.dock.paletteEntries).not.toBe(first);
    // ...and `focusDock` is still the same function, reading runs through the ref.
    expect(result.current.dock.focusDock).toBe(focus);
  });

  it('revealForPath selects that path’s run and focuses the reply box only if blocked', () => {
    const stream = stubStream();
    const { result, focusReply } = harness();
    act(() => result.current.aiRuns.startConflictRun('b.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));

    act(() => result.current.dock.revealForPath('b.ts'));
    act(() => void vi.advanceTimersByTime(1));
    expect(result.current.dock.panelProps.activeKey).toBe('conflict:b.ts');
    expect(focusReply).not.toHaveBeenCalled();

    act(() => stream.send(ev({ seq: 1, kind: 'awaitingInput', text: 'q?' })));
    act(() => result.current.dock.revealForPath('b.ts'));
    act(() => void vi.advanceTimersByTime(1));
    expect(focusReply).toHaveBeenCalledTimes(1);
  });

  it('onRetryFile starts a fresh single run for one file of a bulk run', () => {
    const stream = stubStream();
    const { result } = harness();
    act(() => result.current.aiRuns.startBulkRun(['a.ts', 'b.ts']));
    act(() => result.current.dock.panelProps.onRetryFile('bulk:whatever', 'a.ts'));
    expect(stream.spy).toHaveBeenLastCalledWith(
      expect.any(String),
      ['a.ts'],
      expect.any(Function),
    );
  });

  it('onResizeHeight patches the persisted height, once per call', () => {
    stubStream();
    const { result, onChange } = harness();
    act(() => result.current.aiRuns.startConflictRun('a.ts'));
    act(() => result.current.dock.panelProps.onResizeHeight(240));
    expect(onChange).toHaveBeenLastCalledWith({ aiDockHeight: 240 });
  });

  it('drops the selection when its run is dismissed, so the body never points at nothing', async () => {
    const gate = gatedStream();
    const { result } = harness();
    act(() => result.current.aiRuns.startConflictRun('a.ts'));
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });
    act(() => result.current.dock.panelProps.onSelectRun('conflict:a.ts'));
    expect(result.current.dock.panelProps.activeKey).toBe('conflict:a.ts');
    act(() => result.current.aiRuns.dismissRun('conflict:a.ts'));
    expect(result.current.dock.panelProps.runs).toHaveLength(0);
    expect(result.current.dock.panelProps.activeKey).toBeNull();
  });
});
