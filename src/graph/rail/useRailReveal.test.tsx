/** Spec-005 — useRailReveal: the GraphCanvas-side reveal state machine.
 *  Plan §Testing gap: linger latch, drag pin, zone transitions, reset.
 *  jsdom offsetWidth is always 0, so the scroller is a stub — the hook only
 *  reads `offsetWidth` (zone = x > offsetWidth - HOVER_ZONE_PX = 280 here). */
import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useRailReveal } from './useRailReveal';
import { HOVER_ZONE_PX } from './railMath';

const scroller = { offsetWidth: 300 } as HTMLDivElement;
const IN_ZONE = 300 - HOVER_ZONE_PX + 1; // 281 — just inside
const OUT_ZONE = 300 - HOVER_ZONE_PX; // 280 — boundary is OUTSIDE (strict >)

describe('useRailReveal', () => {
  it('starts hidden with the pointer out of zone', () => {
    const { result } = renderHook(() => useRailReveal());
    expect(result.current.pointerInZone).toBe(false);
    expect(result.current.revealed).toBe(false);
  });

  it('zone enter sets pointerInZone and latches revealed; the boundary pixel is outside', () => {
    const { result } = renderHook(() => useRailReveal());
    act(() => result.current.onZoneCheck(OUT_ZONE, scroller));
    expect(result.current.revealed).toBe(false);
    act(() => result.current.onZoneCheck(IN_ZONE, scroller));
    expect(result.current.pointerInZone).toBe(true);
    expect(result.current.revealed).toBe(true);
  });

  it('leaving the zone clears pointerInZone but keeps revealed latched until hide()', () => {
    const { result } = renderHook(() => useRailReveal());
    act(() => result.current.onZoneCheck(IN_ZONE, scroller));
    act(() => result.current.onZoneCheck(50, scroller));
    expect(result.current.pointerInZone).toBe(false);
    // The latch: the rail stays mounted through its own 300ms linger; only the
    // rail's expiry callback (hide) drops it.
    expect(result.current.revealed).toBe(true);
    act(() => result.current.hide());
    expect(result.current.revealed).toBe(false);
  });

  it('onLeave (scroller mouseleave) clears the zone flag only', () => {
    const { result } = renderHook(() => useRailReveal());
    act(() => result.current.onZoneCheck(IN_ZONE, scroller));
    act(() => result.current.onLeave());
    expect(result.current.pointerInZone).toBe(false);
    expect(result.current.revealed).toBe(true);
    // Idempotent when already out of zone.
    act(() => result.current.onLeave());
    expect(result.current.pointerInZone).toBe(false);
  });

  it('drag pin: revealed survives hide() while a drag is live, drops on drag end', () => {
    const { result } = renderHook(() => useRailReveal());
    act(() => result.current.onZoneCheck(IN_ZONE, scroller));
    act(() => result.current.onDraggingChange(true));
    // Pointer leaves + linger expires mid-drag — the mount must be pinned.
    act(() => result.current.onLeave());
    act(() => result.current.hide());
    expect(result.current.revealed).toBe(true);
    act(() => result.current.onDraggingChange(false));
    expect(result.current.revealed).toBe(false);
  });

  it('reset drops zone, hover latch and drag pin together', () => {
    const { result } = renderHook(() => useRailReveal());
    act(() => result.current.onZoneCheck(IN_ZONE, scroller));
    act(() => result.current.onDraggingChange(true));
    act(() => result.current.reset());
    expect(result.current.pointerInZone).toBe(false);
    expect(result.current.revealed).toBe(false);
    // A fresh zone enter works after reset (the inZone ref was cleared too).
    act(() => result.current.onZoneCheck(IN_ZONE, scroller));
    expect(result.current.revealed).toBe(true);
  });
});
