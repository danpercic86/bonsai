import { useCallback, useRef } from 'react';

export interface PaneDividerProps {
  /** Which pane this divider resizes ('sidebar' grows right-to-left drag = left edge of
   *  sidebar's right border; 'right-panel' grows left-to-right drag = left edge of the
   *  panel's left border). P68e adds 'ai-dock': a HORIZONTAL separator on the dock's top
   *  edge, where dragging UP grows the dock. */
  side: 'sidebar' | 'right-panel' | 'ai-dock';
  /** Called continuously during drag (pointermove) with a delta already normalized to
   *  "this pane's own growth direction" — the component negates internally for
   *  'right-panel' / 'ai-dock' so the caller never has to think about drag direction. */
  onResize(deltaPx: number): void;
  /** Called once on pointerup/pointercancel — the commit point (persist here). */
  onResizeEnd(): void;
  /** P68e: pointerdown, so a caller that accumulates its own live size can seed it
   *  from the current persisted value before the first move. */
  onResizeStart?(): void;
  /** P68e §8: double-click resets to the default size. */
  onReset?(): void;
  /** P68e §8: Home → smallest, End → largest. */
  onExtreme?(edge: 'min' | 'max'): void;
  /** Accessible name; defaults to none (the pane dividers are unlabelled today). */
  ariaLabel?: string;
  /** P68e §8: exposes the dock height on the separator role. */
  ariaValues?: { now: number; min: number; max: number };
  /** Keyboard nudge step; defaults to 8px. */
  nudgePx?: number;
}

const KEYBOARD_NUDGE_PX = 8;

/** 4px-wide invisible drag handle centered on a pane border. Purely an event
 * relay — no React state inside, so drag-frame cost lives in the parent's
 * `useState` setter, not a re-render of this component (P2a contract §3.1). */
export function PaneDivider(props: PaneDividerProps) {
  const { side, onResize, onResizeEnd, onResizeStart, onReset, onExtreme } = props;
  const horizontal = side === 'ai-dock';
  const nudge = props.nudgePx ?? KEYBOARD_NUDGE_PX;
  const lastRef = useRef(0);
  const draggingRef = useRef(false);

  // 'right-panel' grows as the pointer moves LEFT; 'ai-dock' grows as it moves UP.
  const normalize = useCallback(
    (delta: number) => (side === 'sidebar' ? delta : -delta),
    [side],
  );

  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      e.currentTarget.setPointerCapture(e.pointerId);
      lastRef.current = horizontal ? e.clientY : e.clientX;
      draggingRef.current = true;
      onResizeStart?.();
    },
    [horizontal, onResizeStart],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!draggingRef.current) return;
      const pos = horizontal ? e.clientY : e.clientX;
      const delta = pos - lastRef.current;
      lastRef.current = pos;
      onResize(normalize(delta));
    },
    [horizontal, onResize, normalize],
  );

  const endDrag = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!draggingRef.current) return;
      draggingRef.current = false;
      if (e.currentTarget.hasPointerCapture(e.pointerId)) {
        e.currentTarget.releasePointerCapture(e.pointerId);
      }
      onResizeEnd();
    },
    [onResizeEnd],
  );

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      const grow = horizontal ? 'ArrowUp' : 'ArrowRight';
      const shrink = horizontal ? 'ArrowDown' : 'ArrowLeft';
      if (e.key === shrink) {
        e.preventDefault();
        onResize(normalize(horizontal ? nudge : -nudge));
        onResizeEnd();
      } else if (e.key === grow) {
        e.preventDefault();
        onResize(normalize(horizontal ? -nudge : nudge));
        onResizeEnd();
      } else if (onExtreme !== undefined && (e.key === 'Home' || e.key === 'End')) {
        e.preventDefault();
        onExtreme(e.key === 'Home' ? 'min' : 'max');
      }
    },
    [horizontal, nudge, onExtreme, onResize, onResizeEnd, normalize],
  );

  return (
    <div
      className={`pane-divider pane-divider-${side}`}
      role="separator"
      aria-orientation={horizontal ? 'horizontal' : 'vertical'}
      aria-label={props.ariaLabel}
      aria-valuenow={props.ariaValues?.now}
      aria-valuemin={props.ariaValues?.min}
      aria-valuemax={props.ariaValues?.max}
      title={onReset === undefined ? undefined : 'Drag to resize · double-click to reset'}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onDoubleClick={onReset}
      onKeyDown={onKeyDown}
    />
  );
}
