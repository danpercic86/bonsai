/** P119-ui §3.1 — `useMinimumBusy`: true on the rising edge, held until `minMs`
 *  after it, released immediately when the busy span already outlasted `minMs`. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useMinimumBusy } from './useMinimumBusy';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('useMinimumBusy', () => {
  it('is false while idle', () => {
    const { result } = renderHook(() => useMinimumBusy(false, 600));
    expect(result.current).toBe(false);
  });

  it('holds a short busy span for the minimum', () => {
    const { result, rerender } = renderHook(({ busy }) => useMinimumBusy(busy, 600), {
      initialProps: { busy: true },
    });
    expect(result.current).toBe(true);
    act(() => {
      vi.advanceTimersByTime(50);
    });
    rerender({ busy: false });
    expect(result.current).toBe(true);
    act(() => {
      vi.advanceTimersByTime(549);
    });
    expect(result.current).toBe(true);
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current).toBe(false);
  });

  it('releases at once when the span already exceeded the minimum', () => {
    const { result, rerender } = renderHook(({ busy }) => useMinimumBusy(busy, 600), {
      initialProps: { busy: true },
    });
    act(() => {
      vi.advanceTimersByTime(900);
    });
    rerender({ busy: false });
    expect(result.current).toBe(false);
  });

  it('a new rising edge during the hold restarts the span', () => {
    const { result, rerender } = renderHook(({ busy }) => useMinimumBusy(busy, 600), {
      initialProps: { busy: true },
    });
    rerender({ busy: false });
    act(() => {
      vi.advanceTimersByTime(300);
    });
    rerender({ busy: true });
    rerender({ busy: false });
    act(() => {
      vi.advanceTimersByTime(599);
    });
    expect(result.current).toBe(true);
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current).toBe(false);
  });
});
