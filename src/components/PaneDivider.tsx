import { useCallback, useRef } from 'react';

export interface PaneDividerProps {
  /** Which pane this divider resizes ('sidebar' grows right-to-left drag = left edge of
   *  sidebar's right border; 'right-panel' grows left-to-right drag = left edge of the
   *  panel's left border). */
  side: 'sidebar' | 'right-panel';
  /** Called continuously during drag (pointermove) with a delta already normalized to
   *  "this pane's own growth direction" — the component negates internally for
   *  'right-panel' so the caller never has to think about drag direction. */
  onResize(deltaPx: number): void;
  /** Called once on pointerup/pointercancel — the commit point (persist here). */
  onResizeEnd(): void;
}

const KEYBOARD_NUDGE_PX = 8;

/** 4px-wide invisible drag handle centered on a pane border. Purely an event
 * relay — no React state inside, so drag-frame cost lives in the parent's
 * `useState` setter, not a re-render of this component (P2a contract §3.1). */
export function PaneDivider(props: PaneDividerProps) {
  const { side, onResize, onResizeEnd } = props;
  const lastXRef = useRef(0);
  const draggingRef = useRef(false);

  const normalize = useCallback(
    (delta: number) => (side === 'right-panel' ? -delta : delta),
    [side],
  );

  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      e.currentTarget.setPointerCapture(e.pointerId);
      lastXRef.current = e.clientX;
      draggingRef.current = true;
    },
    [],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!draggingRef.current) return;
      const delta = e.clientX - lastXRef.current;
      lastXRef.current = e.clientX;
      onResize(normalize(delta));
    },
    [onResize, normalize],
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
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        onResize(normalize(-KEYBOARD_NUDGE_PX));
        onResizeEnd();
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        onResize(normalize(KEYBOARD_NUDGE_PX));
        onResizeEnd();
      }
    },
    [onResize, onResizeEnd, normalize],
  );

  return (
    <div
      className={`pane-divider pane-divider-${side}`}
      role="separator"
      aria-orientation="vertical"
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onKeyDown={onKeyDown}
    />
  );
}
