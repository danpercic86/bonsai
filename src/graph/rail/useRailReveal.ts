/** Spec-005: GraphCanvas-side rail reveal state — the 20px right-edge hover
 *  zone (transitions only, ref-guarded), the hover-revealed latch cleared by
 *  the rail's linger timer, and the drag pin (the rail must never unmount
 *  mid-thumb-drag with pointer capture live — review SHOULD-FIX #4).
 *  Extracted so GraphCanvas's delta stays within its size baseline. */
import { useCallback, useRef, useState } from 'react';
import { HOVER_ZONE_PX } from './railMath';

export interface RailReveal {
  /** Pointer currently inside the hover zone (drives the rail's linger). */
  pointerInZone: boolean;
  /** Mount keep-alive: hover-revealed OR a live thumb drag. */
  revealed: boolean;
  /** Call from the scroller's mousemove (x relative to the scroller left). */
  onZoneCheck(x: number, scroller: HTMLDivElement): void;
  /** Call from the scroller's mouseleave. */
  onLeave(): void;
  /** Stable — the rail's linger expired (clears the hover latch). */
  hide(): void;
  /** Stable — the rail reports drag start/end (pins the mount while true). */
  onDraggingChange(dragging: boolean): void;
  /** Stable — tab hidden / remount: drop hover state (a drag cannot be live). */
  reset(): void;
}

export function useRailReveal(): RailReveal {
  const [pointerInZone, setPointerInZone] = useState(false);
  const [hoverRevealed, setHoverRevealed] = useState(false);
  const [dragging, setDragging] = useState(false);
  const inZoneRef = useRef(false);

  const onZoneCheck = useCallback((x: number, scroller: HTMLDivElement) => {
    const inZone = x > scroller.offsetWidth - HOVER_ZONE_PX;
    if (inZone !== inZoneRef.current) {
      inZoneRef.current = inZone;
      setPointerInZone(inZone);
      if (inZone) setHoverRevealed(true);
    }
  }, []);

  const onLeave = useCallback(() => {
    // The rail's own pointerenter cancels the linger when the pointer moved
    // ONTO the rail; leaving the scroller otherwise exits the zone.
    if (inZoneRef.current) {
      inZoneRef.current = false;
      setPointerInZone(false);
    }
  }, []);

  const hide = useCallback(() => setHoverRevealed(false), []);
  const onDraggingChange = useCallback((d: boolean) => setDragging(d), []);
  const reset = useCallback(() => {
    inZoneRef.current = false;
    setPointerInZone(false);
    setHoverRevealed(false);
    setDragging(false);
  }, []);

  return {
    pointerInZone,
    revealed: hoverRevealed || dragging,
    onZoneCheck,
    onLeave,
    hide,
    onDraggingChange,
    reset,
  };
}
