/**
 * P113 §13.1 / AC9 (phase 2) — the PRODUCER-SIDE guard, and its one exemption.
 *
 * The path lint (`eslint.config.js`, `no-restricted-imports`) is the cheap first
 * line and is not the guard: `no-restricted-imports` is import-graph-based, and
 * the five call sites phase 1 missed received `pushToast` as a PARAMETER from
 * `App.tsx`, importing no toast module at all. There is no import to ban, and
 * widening the `files` glob would accomplish nothing.
 *
 * So the guard lives inside `pushToast` itself and checks the condition that
 * actually matters — is the Settings overlay up — which catches a caller
 * anywhere in the tree, including §13.3's background toasts that no lint rule
 * can see.
 *
 * The second half is the part the orchestrator asked to be PROVEN rather than
 * assumed: `useUiSettings` keeps its `pushToast`, legitimately, because it also
 * serves density / sidebar / graph-filter writes made with Settings CLOSED. The
 * exemption is supposed to fall out of the assertion's own condition. These
 * cases check that it does — no console.error, no toast, banner state instead.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../ipc/mock';
import { appErr } from '../test/actionHookKit';
import { HYDRATED, mountUiSettings } from '../test/uiSettingsKit';
import { SETTINGS_SAVE_FAILURE_TEXT } from './useSettingsSaveFailure';
import { SETTINGS_TOAST_GUARD_MESSAGE, useToastQueue, type UseToastQueue } from './useToastQueue';

const OPEN = true;
const CLOSED = false;

/** The two consumers wired the way App wires them (§13.1: the **same signal**,
 *  wired once): `useToastQueue` takes the boolean, publishes the ONE
 *  `settingsOpen` ref, and `useUiSettings` reads THAT ref plus THAT `pushToast`.
 *
 *  Two hand-written literal refs and a bare `vi.fn()` pusher would leave both
 *  halves of the claim untested — the guard could never fire because a fake
 *  pusher cannot warn (so `expect(spy).not.toHaveBeenCalled()` would pass with
 *  the guard deleted), and App could hand `useUiSettings` a second, drifting
 *  copy of the signal with every case below still green. */
function mountWired(settingsOpenNow: boolean) {
  const queue = renderHook(() => useToastQueue(settingsOpenNow));
  const { pushToast, settingsOpen } = queue.result.current;
  return { queue, ui: mountUiSettings(pushToast, settingsOpen) };
}

/** What the real stack is holding, as `[tone, text]` pairs — `useUiSettings`
 *  pushes with a dedupe key, which is not part of what this suite claims. */
const toastPairs = (queue: { result: { current: UseToastQueue } }) =>
  queue.result.current.toasts.map((t) => [t.tone, t.text]);

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('P113 §13.1 — the dev-time reachability guard', () => {
  it('errors, naming the message, when a toast is raised while Settings is open', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { result } = renderHook(() => useToastQueue(OPEN));

    act(() => result.current.pushToast('error', 'Could not register: boom'));

    expect(spy).toHaveBeenCalledTimes(1);
    const text = String(spy.mock.calls[0][0]);
    expect(text).toContain(SETTINGS_TOAST_GUARD_MESSAGE);
    // Naming the text is what makes the console line actionable — otherwise the
    // developer knows a toast leaked but not which one.
    expect(text).toContain('Could not register: boom');
  });

  it('stays silent while Settings is closed', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { result } = renderHook(() => useToastQueue(CLOSED));

    act(() => result.current.pushToast('error', 'Could not reopen repo'));

    expect(spy).not.toHaveBeenCalled();
  });

  it('does not swallow the toast — the message must still reach somewhere', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const { result } = renderHook(() => useToastQueue(OPEN));

    act(() => result.current.pushToast('error', 'boom'));

    expect(result.current.toasts.map((t) => t.text)).toEqual(['boom']);
  });
});

describe("P113 §17.3 — useUiSettings's toast is exempt, and that falls out of the condition", () => {
  it('routes a failed write to the BANNER, silently, when Settings is open', async () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(mockIpc, 'setUiSettings').mockRejectedValue(appErr('io', 'disk on fire'));
    const { queue, ui } = mountWired(OPEN);

    act(() => ui.result.current.handleSettingsChange({ panelDensity: 'compact' }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });

    // Nothing reached the real stack, so the guard had nothing to warn about —
    // and the guard IS in the loop here, on the same open signal it would fire
    // on. That is what makes the silence below evidence of the exemption rather
    // than evidence of an unwired spy.
    expect(toastPairs(queue)).toEqual([]);
    expect(spy).not.toHaveBeenCalled();
    expect(ui.result.current.settingsSaveFailed).toBe(true);
  });

  it('routes a failed write to the TOAST, with no guard error, when Settings is closed', async () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(mockIpc, 'setUiSettings').mockRejectedValue(appErr('io', 'disk on fire'));
    const { queue, ui } = mountWired(CLOSED);

    act(() => ui.result.current.handleSettingsChange({ panelDensity: 'compact' }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });

    expect(toastPairs(queue)).toEqual([['error', SETTINGS_SAVE_FAILURE_TEXT]]);
    expect(spy).not.toHaveBeenCalled();
    // The condition is the same either way, so the banner state is set in both.
    expect(ui.result.current.settingsSaveFailed).toBe(true);
  });

  it('clears the banner only on a successful write', async () => {
    vi.useFakeTimers();
    const write = vi
      .spyOn(mockIpc, 'setUiSettings')
      .mockRejectedValue(appErr('io', 'disk on fire'));
    const { ui } = mountWired(OPEN);

    act(() => ui.result.current.handleSettingsChange({ panelDensity: 'compact' }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(ui.result.current.settingsSaveFailed).toBe(true);

    // A fresh user change resets the retry BUDGET but must not clear the banner:
    // the values are still not on disk at that moment, and clearing here would
    // blink it off for the debounce window and back on when the retry fails.
    act(() => ui.result.current.handleSettingsChange({ panelDensity: 'cozy' }));
    expect(ui.result.current.settingsSaveFailed).toBe(true);

    write.mockResolvedValue(HYDRATED);
    act(() => ui.result.current.retrySettingsSave());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(ui.result.current.settingsSaveFailed).toBe(false);
  });
});
