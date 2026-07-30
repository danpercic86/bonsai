import { useEffect, useLayoutEffect, useRef, useState } from 'react';

export interface ContextMenuItem {
  label: string;
  onSelect(): void;
  disabled?: boolean;
  /** P10 T2: optional leading 16×16 monochrome glyph, inherits item text color. */
  icon?: React.ReactNode;
}

export interface ContextMenuProps {
  /** clientX anchor. */
  x: number;
  /** clientY anchor. */
  y: number;
  items: ContextMenuItem[];
  /** Fired by every dismiss path AND after an enabled item activates. */
  onClose(): void;
}

/** Small reusable right-click menu (P5 §5.1). Fixed-positioned at (x, y),
 *  clamped into the viewport. Dismisses on outside pointerdown, Escape, scroll
 *  (capture), resize, and window blur. All colors come from CSS variables so
 *  both themes work; no hard-coded hex. */
export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

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

  // Focus the first enabled item on mount.
  useEffect(() => {
    const el = rootRef.current;
    if (el === null) return;
    const first = el.querySelector<HTMLButtonElement>('button[role="menuitem"]:not([aria-disabled="true"])');
    first?.focus();
  }, []);

  // Dismiss paths (§5.1): outside pointerdown, Escape, scroll (capture),
  // resize, window blur.
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
      }
    };
    const onScroll = () => onClose();
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

  const activate = (item: ContextMenuItem) => {
    if (item.disabled === true) return;
    item.onSelect();
    onClose();
  };

  // ArrowUp/Down move focus, skipping disabled items; Enter/Space activate.
  const onItemKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const step = e.key === 'ArrowDown' ? 1 : -1;
      const n = items.length;
      for (let i = 1; i <= n; i++) {
        const j = (((index + step * i) % n) + n) % n;
        if (items[j].disabled !== true) {
          const buttons = rootRef.current?.querySelectorAll<HTMLButtonElement>(
            'button[role="menuitem"]',
          );
          buttons?.[j]?.focus();
          break;
        }
      }
      return;
    }
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      activate(items[index]);
    }
  };

  return (
    <div
      ref={rootRef}
      className="context-menu"
      role="menu"
      style={{ left: pos.x, top: pos.y }}
    >
      {items.map((item, i) => (
        <button
          key={item.label}
          type="button"
          role="menuitem"
          className="context-menu-item"
          disabled={item.disabled === true}
          aria-disabled={item.disabled === true}
          tabIndex={-1}
          onClick={() => activate(item)}
          onKeyDown={(e) => onItemKeyDown(e, i)}
        >
          {item.icon !== undefined && (
            <span className="context-menu-icon" aria-hidden="true">
              {item.icon}
            </span>
          )}
          {item.label}
        </button>
      ))}
    </div>
  );
}
