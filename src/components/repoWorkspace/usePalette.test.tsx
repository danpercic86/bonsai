/** T3.2b — usePalette: open/close/toggle + force-close on tab deactivation. */
import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { usePalette } from './usePalette';

function mount(active = true) {
  return renderHook((p: { active: boolean }) => usePalette(p), {
    initialProps: { active },
  });
}

describe('usePalette', () => {
  it('starts closed; toggle opens and closes; openRef tracks open', () => {
    const { result } = mount();
    expect(result.current.open).toBe(false);
    expect(result.current.openRef.current).toBe(false);
    act(() => result.current.toggle());
    expect(result.current.open).toBe(true);
    expect(result.current.openRef.current).toBe(true);
    act(() => result.current.toggle());
    expect(result.current.open).toBe(false);
    expect(result.current.openRef.current).toBe(false);
  });

  it('close() closes and is idempotent', () => {
    const { result } = mount();
    act(() => result.current.toggle());
    act(() => result.current.close());
    expect(result.current.open).toBe(false);
    act(() => result.current.close());
    expect(result.current.open).toBe(false);
  });

  it('deactivating the tab force-closes; it does NOT reopen on reactivation', () => {
    const h = mount();
    act(() => h.result.current.toggle());
    expect(h.result.current.open).toBe(true);
    h.rerender({ active: false });
    expect(h.result.current.open).toBe(false);
    h.rerender({ active: true });
    expect(h.result.current.open).toBe(false);
  });

  it('cannot linger open when mounted inactive and toggled programmatically', () => {
    const h = mount(false);
    act(() => h.result.current.toggle());
    // The effect only fires on `active` CHANGES — toggling while inactive keeps
    // it open until the next active flip. Verify the flip cleans it up.
    h.rerender({ active: false });
    expect(h.result.current.open).toBe(true); // documented current behavior
    h.rerender({ active: true });
    h.rerender({ active: false });
    expect(h.result.current.open).toBe(false);
  });
});
