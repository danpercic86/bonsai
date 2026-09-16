/**
 * The activation self-heal guard, under StrictMode.
 *
 * REGRESSION (2026-09-16): the original guard was `if (!flip.current) { flip.current
 * = true; return; }` — a "skip the first run" latch. React 19 StrictMode's simulated
 * remount re-runs the mount effect on the SAME hook instance (the same ref object, as
 * documented in `useAiRuns.ts` and `obs/react.test.tsx`), so the second pass
 * saw the latch already set and fired `refresh('activation', 'full')` ON MOUNT — the
 * exact double-refresh the guard exists to prevent. A real Dev-mode log showed two
 * `origin: activation`, `scope: full` refresh records, both at `round: 1`.
 *
 * The fix compares against the LAST OBSERVED `active` value instead of counting
 * effect runs, so a repeated run with an unchanged value is a no-op.
 */
import { StrictMode } from 'react';
import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { useActivationRefresh } from './useActivationRefresh';
import type { RefreshOrigin } from './useCoalescedRefresh';
import type { RefreshScope } from './refreshScope';

function mount(initialActive: boolean) {
  const refresh = vi.fn(async (_origin: RefreshOrigin, _scope: RefreshScope) => {});
  const view = renderHook(({ active }: { active: boolean }) => useActivationRefresh(active, refresh), {
    initialProps: { active: initialActive },
    wrapper: StrictMode,
  });
  return { refresh, view };
}

describe('useActivationRefresh', () => {
  it('does NOT refresh on mount under StrictMode (the initial load covers first paint)', () => {
    const { refresh } = mount(true);
    expect(refresh).not.toHaveBeenCalledWith('activation', 'full');
    expect(refresh).not.toHaveBeenCalled();
  });

  it('refreshes exactly once on a real flip to active', () => {
    const { refresh, view } = mount(true);
    view.rerender({ active: false });
    expect(refresh).not.toHaveBeenCalled();
    view.rerender({ active: true });
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(refresh).toHaveBeenCalledWith('activation', 'full');
  });

  it('does not refresh on a re-render that leaves `active` unchanged', () => {
    const { refresh, view } = mount(false);
    view.rerender({ active: false });
    view.rerender({ active: false });
    expect(refresh).not.toHaveBeenCalled();
  });

  it('never refreshes on a flip AWAY from active', () => {
    const { refresh, view } = mount(true);
    view.rerender({ active: false });
    expect(refresh).not.toHaveBeenCalled();
  });
});
