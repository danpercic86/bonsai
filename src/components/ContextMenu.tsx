import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { MenuList } from './contextMenu/MenuList';
import type { ContextMenuProps } from './contextMenu/types';

export type {
  ContextMenuItem,
  ContextMenuProps,
  ContextMenuState,
} from './contextMenu/types';

/** Small reusable right-click menu (P5 §5.1). Fixed-positioned at (x, y),
 *  clamped into the viewport. Dismisses on outside pointerdown, Escape, scroll
 *  (capture), resize, and window blur. Rows with `children` open a hover/keyboard
 *  flyout submenu. All colors come from CSS variables so both themes work. */
export function ContextMenu({ x, y, items, onClose, header, ariaLabel, busy }: ContextMenuProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  // P92 §1.5: focus returns to whatever had it when the menu opened (the graph
  // scroller, a sidebar row, …). Captured during the FIRST RENDER, not in an
  // effect: child effects run before parent effects, so MenuList's `autoFocus`
  // would already have moved focus into the menu by then.
  const [prevFocus] = useState<HTMLElement | null>(
    () => (document.activeElement as HTMLElement | null) ?? null,
  );
  useEffect(
    () => () => {
      // Only restore if focus is still ours (or was dropped to <body>) —
      // an outside click has already focused what the user aimed at.
      const active = document.activeElement;
      const ours =
        active === null ||
        active === document.body ||
        (rootRef.current !== null && rootRef.current.contains(active));
      if (ours && prevFocus !== null && prevFocus.isConnected) prevFocus.focus();
    },
    [prevFocus],
  );

  // Clamp into the viewport once the menu has measured itself.
  useLayoutEffect(() => {
    const el = rootRef.current;
    if (el === null) return;
    const { width, height } = el.getBoundingClientRect();
    const maxX = window.innerWidth - width - 4;
    const maxY = window.innerHeight - height - 4;
    setPos({
      x: Math.max(4, Math.min(x, maxX)),
      y: Math.max(4, Math.min(y, maxY)),
    });
  }, [x, y, items]);

  // Dismiss paths (§5.1): outside pointerdown, Escape, scroll (capture),
  // resize, window blur. The submenu lives inside rootRef, so its interactions
  // never count as "outside".
  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      if (rootRef.current !== null && !rootRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      } else if (e.key === 'Tab') {
        // Tab would move focus to an element behind the menu, orphaning it open.
        // Close it and let focus proceed normally to the next control.
        onClose();
      }
    };
    // P92 addendum A.1: `scroll` does not bubble, but a CAPTURE listener still
    // receives it from the menu's own (now height-clamped, scrollable) box — so
    // an unguarded handler closed the menu on the first wheel tick and on
    // `focusRow`'s `scrollIntoView` during arrow-key navigation. Ignore scrolls
    // that originate inside the menu, mirroring the pointerdown guard above.
    const onScroll = (e: Event) => {
      // `target` is `document` for a page scroll and `window` for a synthetic
      // one — neither is a Node the menu can contain, hence the instanceof.
      const t = e.target;
      if (rootRef.current !== null && t instanceof Node && rootRef.current.contains(t)) return;
      onClose();
    };
    const onResize = () => onClose();
    const onBlur = () => onClose();
    document.addEventListener('pointerdown', onPointerDown, true);
    window.addEventListener('keydown', onKeyDown, true);
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onResize);
    window.addEventListener('blur', onBlur);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown, true);
      window.removeEventListener('keydown', onKeyDown, true);
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', onResize);
      window.removeEventListener('blur', onBlur);
    };
  }, [onClose]);

  return (
    <MenuList
      containerRef={rootRef}
      items={items}
      onClose={onClose}
      autoFocus
      header={header}
      ariaLabel={ariaLabel}
      busy={busy}
      style={{ left: pos.x, top: pos.y }}
    />
  );
}
