/** useUiSettings — the PERSIST path (P69b). The state machine itself is covered
 *  by `useUiSettings.test.tsx`; this suite owns the three write-path defects
 *  P69b fixed, each of which was previously either unguarded or pinned as
 *  shipped behaviour:
 *    1. four App-owned writers (theme / listView / paneWidths / onboardingSeen)
 *       fired their own `ipc.setUiSettings` outside the debounced merge,
 *    2. a rejected write was dropped and never retried,
 *    3. a patch pending inside the 300 ms window was never flushed on unmount.
 *
 *  Fixtures + mount helper: `src/test/uiSettingsKit.ts`. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { mockIpc } from '../ipc/mock';
import { appErr } from '../test/actionHookKit';
import { deferred, GRAPH_PATCH, HYDRATED, mountUiSettings as mount } from '../test/uiSettingsKit';

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

// P69b defect 1: `theme`, `listView`, `paneWidths` and `onboardingSeen` are
// App-owned state, but their PERSIST calls used to bypass this hook entirely —
// four independent `ipc.setUiSettings` writes racing the debounced merge, benign
// only while their key sets stayed disjoint. They now ride `queueSettingsWrite`:
// same pending patch, same window, no state touched here.
describe('queueSettingsWrite (App-owned settings share the window)', () => {
  it('a burst mixing hook-owned and App-owned writers produces ONE merged write', () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result } = mount();

    // One of each writer, interleaved, all inside one 300 ms window.
    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    act(() => result.current.queueSettingsWrite({ theme: 'light' })); // toggleTheme
    act(() => vi.advanceTimersByTime(150));
    act(() => result.current.queueSettingsWrite({ listView: 'flat' })); // toggleListView
    act(() => result.current.queueSettingsWrite({ paneWidths: { sidebar: 300, rightPanel: 420 } })); // commitPaneWidths
    act(() => result.current.queueSettingsWrite({ onboardingSeen: true })); // closeOnboarding

    expect(spy).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(300));

    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0]).toEqual({
      panelDensity: 'compact',
      theme: 'light',
      listView: 'flat',
      paneWidths: { sidebar: 300, rightPanel: 420 },
      onboardingSeen: true,
    });
  });

  it('an overlapping key is last-write-wins across writers instead of racing', () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result } = mount();

    // The failure mode the fix removes: two writers touching the SAME key in one
    // burst. As separate writes their order was unguaranteed (either could land
    // last); merged, the later call deterministically wins and neither key is lost.
    act(() => result.current.queueSettingsWrite({ theme: 'light', listView: 'flat' }));
    act(() => result.current.queueSettingsWrite({ theme: 'dark' }));
    act(() => vi.advanceTimersByTime(300));

    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0]).toEqual({ theme: 'dark', listView: 'flat' });
  });

  it('is persist-only — it never touches this hook’s state or metricsVersion', () => {
    vi.useFakeTimers();
    vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result } = mount();
    const before = result.current.metricsVersion;

    // App owns the live preview for its four; the hook must only carry a patch to
    // disk. (Passing hook-owned keys here is not something App does — it proves
    // the separation.)
    act(() => result.current.queueSettingsWrite({ panelDensity: 'compact', graph: GRAPH_PATCH }));

    expect(result.current.panelDensity).toBe('cozy'); // hook default, unchanged
    expect(result.current.metricsVersion).toBe(before);
  });
});

// P69b defect 3 — DELIBERATE BEHAVIOUR CHANGE. A test here previously pinned the
// shipped behaviour ("unmount does NOT cancel a pending write"): the timer merely
// outlived unmount, so a patch survived an unmount but NOT the JS context dying
// inside the 300 ms window (app quit / window close right after a knob change),
// and the late write could outlive unmount and race a read.
describe('unmount flush', () => {
  it('unmount FLUSHES the pending patch immediately and lets no write escape after it', () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result, unmount } = mount();

    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    expect(spy).not.toHaveBeenCalled(); // still inside the window

    unmount();

    // Written AT unmount — not left to a timer that outlives the component.
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0]).toEqual({ panelDensity: 'compact' });

    // ...and the timer was cleared, so nothing fires a second time afterwards.
    act(() => vi.advanceTimersByTime(1000));
    expect(spy).toHaveBeenCalledTimes(1);
  });

  // P69b SHOULD-FIX 3: React cleanup does NOT run on window close, app quit or
  // `location.reload()`, and `App` is the root (`src/main.tsx`) so it never
  // unmounts in production — the unmount cleanup alone would only ever fire in
  // HMR and tests. These two listeners are what make the defect-3 claim true.
  it.each(['pagehide', 'beforeunload'] as const)(
    '%s flushes a patch that is still inside the debounce window',
    (eventName) => {
      vi.useFakeTimers();
      const spy = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
      const { result } = mount();

      act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
      expect(spy).not.toHaveBeenCalled();

      act(() => {
        window.dispatchEvent(new Event(eventName));
      });

      expect(spy).toHaveBeenCalledTimes(1);
      expect(spy.mock.calls[0][0]).toEqual({ panelDensity: 'compact' });

      // The window was cancelled, so the timer cannot write a second time.
      act(() => vi.advanceTimersByTime(1000));
      expect(spy).toHaveBeenCalledTimes(1);
    },
  );

  it('teardown listeners are removed on unmount (no write after the component is gone)', () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result, unmount } = mount();

    unmount();
    spy.mockClear();
    // Queue again through the (now detached) callback, so a listener that was
    // never removed would have something to flush.
    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    act(() => {
      window.dispatchEvent(new Event('pagehide'));
    });
    expect(spy).not.toHaveBeenCalled();
  });

  // P69b round-2 NIT 3: the forced teardown flush can still reject AFTER the
  // component is gone. Its catch must not arm a retry — a timer outliving unmount
  // is a future mystery flake (it fires into whatever spy is installed next).
  it('a forced teardown flush that fails arms no timer after the component is gone', async () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockRejectedValue(appErr('io', 'disk on fire'));
    const { result, unmount } = mount();

    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    unmount(); // forced flush leaves; it will reject

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000); // well past 300 + 600 + 1200
    });
    expect(spy).toHaveBeenCalledTimes(1); // no retry survived the unmount
  });

  it('unmount with nothing pending writes nothing (StrictMode double-mount safe)', () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(HYDRATED);
    const { result, unmount } = mount();

    // A burst that has already been flushed leaves an empty pending patch.
    act(() => result.current.handleSettingsChange({ aiDockCollapsed: true }));
    act(() => vi.advanceTimersByTime(300));
    expect(spy).toHaveBeenCalledTimes(1);

    unmount();
    act(() => vi.advanceTimersByTime(1000));
    expect(spy).toHaveBeenCalledTimes(1); // no empty-patch write
  });
});

// P69b defect 2 — DELIBERATE BEHAVIOUR CHANGE. The pending patch used to be
// emptied BEFORE the await, so a rejection dropped it for good: the toast said
// the save failed while the UI kept showing values the disk did not have, and
// nothing ever retried. The failed patch is now merged back, newest-wins.
describe('failed writes are retried', () => {
  it('a rejected write is retried with the next change instead of being dropped', async () => {
    vi.useFakeTimers();
    const spy = vi
      .spyOn(mockIpc, 'setUiSettings')
      .mockRejectedValueOnce(appErr('io', 'disk on fire'))
      .mockResolvedValue(HYDRATED);
    const push = vi.fn();
    const { result } = mount(push);

    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(spy).toHaveBeenCalledTimes(1);
    expect(push).toHaveBeenCalledTimes(1); // the failure toast still fires

    // The next knob change carries the lost patch back out with it.
    act(() => result.current.handleSettingsChange({ aiDockHeight: 240 }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });

    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[1][0]).toEqual({ panelDensity: 'compact', aiDockHeight: 240 });
    expect(push).toHaveBeenCalledTimes(1); // the retry succeeded — no second toast
  });

  it('a value changed while a doomed write was in flight beats the retried stale one', async () => {
    vi.useFakeTimers();
    const inFlight = deferred<typeof HYDRATED>();
    const spy = vi
      .spyOn(mockIpc, 'setUiSettings')
      .mockReturnValueOnce(inFlight.promise)
      .mockResolvedValue(HYDRATED);
    const { result } = mount();

    act(() => result.current.handleSettingsChange({ aiDockHeight: 200 }));
    act(() => vi.advanceTimersByTime(300)); // write 1 leaves, unresolved
    expect(spy).toHaveBeenCalledTimes(1);

    // The user moves the SAME knob again while write 1 is still in flight...
    act(() => result.current.handleSettingsChange({ aiDockHeight: 260 }));
    // ...and only then does write 1 fail. Merging it back must not resurrect the
    // stale 200 over the 260 the UI is showing: newer wins.
    await act(async () => {
      inFlight.reject(appErr('io', 'disk on fire'));
      await vi.advanceTimersByTimeAsync(300);
    });

    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[1][0]).toEqual({ aiDockHeight: 260 });
    expect(result.current.aiDockHeight).toBe(260);
  });

  // P69b round-2 NIT 2: when a write settles with a patch already pending, the
  // pump must re-arm the FULL window. Writing immediately would cut a burst that
  // happens to span a settled write into two writes — defeating the coalescing
  // this whole path exists to provide.
  it('a change made while a write was in flight still coalesces with the rest of its burst', async () => {
    vi.useFakeTimers();
    const writeA = deferred<typeof HYDRATED>();
    const spy = vi
      .spyOn(mockIpc, 'setUiSettings')
      .mockReturnValueOnce(writeA.promise)
      .mockResolvedValue(HYDRATED);
    const { result } = mount();

    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    act(() => vi.advanceTimersByTime(300)); // write A leaves, unresolved
    expect(spy).toHaveBeenCalledTimes(1);

    // Knob 2 of the next burst lands while A is still out, then A settles...
    act(() => result.current.handleSettingsChange({ aiDockHeight: 240 }));
    await act(async () => {
      writeA.resolve(HYDRATED);
      await vi.advanceTimersByTimeAsync(100);
    });
    // ...and knob 3 arrives after the settle. Both belong to ONE write.
    act(() => result.current.handleSettingsChange({ aiDockCollapsed: true }));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(299);
    });
    expect(spy).toHaveBeenCalledTimes(1); // the pump did not write early

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[1][0]).toEqual({ aiDockHeight: 240, aiDockCollapsed: true });
  });

  // P69b MUST-FIX 2: `{...merged, ...pending}` is only newest-wins while the newer
  // value is STILL pending. Two concurrent flushes broke that — the newer patch
  // had already left in write B, so A's rejection put the stale value back and the
  // next write pushed it to disk under a UI showing the newer one. Fixed by
  // allowing at most one write in flight.
  it('an overlapping flush cannot resurrect a stale value from a failed earlier write', async () => {
    vi.useFakeTimers();
    const writeA = deferred<typeof HYDRATED>();
    const spy = vi
      .spyOn(mockIpc, 'setUiSettings')
      .mockReturnValueOnce(writeA.promise)
      .mockResolvedValue(HYDRATED);
    const { result } = mount();

    act(() => result.current.handleSettingsChange({ aiDockHeight: 200 }));
    act(() => vi.advanceTimersByTime(300)); // write A leaves, unresolved
    expect(spy).toHaveBeenCalledTimes(1);

    // A newer value, and its whole window elapses while A is STILL in flight.
    act(() => result.current.handleSettingsChange({ aiDockHeight: 260 }));
    act(() => vi.advanceTimersByTime(300));
    expect(spy).toHaveBeenCalledTimes(1); // serialized — no second concurrent write

    // Now A fails. Its patch is merged back UNDER the still-pending 260.
    await act(async () => {
      writeA.reject(appErr('io', 'disk on fire'));
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[1][0]).toEqual({ aiDockHeight: 260 });

    // ...and nothing later re-sends the stale 200.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls.at(-1)?.[0]).toEqual({ aiDockHeight: 260 });
    expect(result.current.aiDockHeight).toBe(260);
  });

  // P69b SHOULD-FIX 4: the catch used to restore the patch without re-arming the
  // window, so recovery waited on the user. It now retries with bounded backoff
  // (300 / 600 / 1200 ms) and keeps a dead disk to ONE toast per streak.
  it('a persistent failure retries a bounded number of times with ONE toast', async () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(mockIpc, 'setUiSettings').mockRejectedValue(appErr('io', 'disk on fire'));
    const push = vi.fn();
    const { result } = mount(push);

    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300); // attempt 1
    });
    expect(spy).toHaveBeenCalledTimes(1);
    expect(push).toHaveBeenCalledTimes(1);

    // Backoff: +300 (2), +600 (3), +1200 (4) — then it stops on its own.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(spy).toHaveBeenCalledTimes(4); // 1 + SETTINGS_SAVE_MAX_RETRIES
    expect(push).toHaveBeenCalledTimes(1); // no toast storm

    // Every attempt carried the same unsaved patch, and it is still pending: the
    // next user change gets a fresh budget and takes it out with it.
    expect(spy.mock.calls.every((c) => c[0].panelDensity === 'compact')).toBe(true);
    spy.mockResolvedValue(HYDRATED);
    act(() => result.current.handleSettingsChange({ aiDockHeight: 240 }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(spy).toHaveBeenCalledTimes(5);
    expect(spy.mock.calls[4][0]).toEqual({ panelDensity: 'compact', aiDockHeight: 240 });
  });

  it('a patch lost to a failed write is still flushed at unmount', async () => {
    vi.useFakeTimers();
    const spy = vi
      .spyOn(mockIpc, 'setUiSettings')
      .mockRejectedValueOnce(appErr('io', 'disk on fire'))
      .mockResolvedValue(HYDRATED);
    const { result, unmount } = mount();

    act(() => result.current.handleSettingsChange({ panelDensity: 'compact' }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(spy).toHaveBeenCalledTimes(1);

    // Retry-on-next-change and flush-on-unmount are the same pending patch.
    unmount();
    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[1][0]).toEqual({ panelDensity: 'compact' });
  });
});
