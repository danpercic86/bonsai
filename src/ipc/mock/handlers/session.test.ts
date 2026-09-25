// @vitest-environment happy-dom
/** P124d — the mock `onWindowFocus` applies the same leading-edge gate as the
 *  Tauri adapter, so the browser harness mirrors the native one-round-per-focus
 *  behaviour. Fake timers also fake `Date.now`, which the gate reads. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { sessionHandlers } from './session';

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe('mock onWindowFocus focus gate (P124d)', () => {
  it('collapses a double focus delivery, accepts a later one, and stops after unsubscribe', async () => {
    const cb = vi.fn();
    const unsub = await sessionHandlers.onWindowFocus(cb);

    window.dispatchEvent(new Event('focus'));
    vi.advanceTimersByTime(30);
    window.dispatchEvent(new Event('focus'));
    expect(cb).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(300);
    window.dispatchEvent(new Event('focus'));
    expect(cb).toHaveBeenCalledTimes(2);

    unsub();
    vi.advanceTimersByTime(300);
    window.dispatchEvent(new Event('focus'));
    expect(cb).toHaveBeenCalledTimes(2);
  });
});
