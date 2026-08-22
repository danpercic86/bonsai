import { useEffect, useLayoutEffect, useRef, useState } from 'react';

export interface ContextMenuItem {
  label: string;
  /** Optional: a pure-submenu parent may omit a default action. */
  onSelect?(): void;
  disabled?: boolean;
  /** P10 T2: optional leading 16×16 monochrome glyph, inherits item text color. */
  icon?: React.ReactNode;
  /** Present ⇒ this row opens a flyout submenu of variants. */
  children?: ContextMenuItem[];
  /** 'danger' ⇒ red icon + label (destructive action). */
  tone?: 'default' | 'danger';
  /**
   * P69i (UI §4.4). Present ⇒ the row renders `role="menuitemradio"` with
   * `aria-checked`, and the whole list reserves a 16px leading check column
   * (a ✓ glyph in `--text-1`, never a background tint — so a checked row stays
   * legible in both themes and in forced-colours mode).
   *
   * Absent ⇒ `role="menuitem"` and no column: byte-identical to pre-P69i.
   * `menuitemradio` rather than `menuitemcheckbox` because these lists are
   * "at most one in effect" (the identity menu), not multi-select.
   */
  checked?: boolean;
  /**
   * P69i (UI §4.4). One secondary line under the label — 12px `--text-2`,
   * ellipsised, never focusable. The row grows 32 → 46px when present.
   */
  detail?: string;
}

export interface ContextMenuProps {
  /** clientX anchor. */
  x: number;
  /** clientY anchor. */
  y: number;
  items: ContextMenuItem[];
  /** Fired by every dismiss path AND after an enabled item activates. */
  onClose(): void;
  /**
   * P69i (UI §4.4). Non-interactive block rendered above the list inside
   * `.context-menu`, `role="presentation"`. Excluded from keyboard navigation
   * for free: the focus queries scope to the row buttons.
   */
  header?: React.ReactNode;
  /**
   * P69i. `aria-busy` on the menu root, for a menu that deliberately stays open
   * while an activated row's write settles (UI §4.5). Additive and generic.
   */
  busy?: boolean;
}

const HOVER_OPEN_MS = 120;
const HOVER_CLOSE_MS = 180;

interface MenuListProps {
  items: ContextMenuItem[];
  onClose(): void;
  /** true ⇒ render as an absolutely-positioned flyout (`.context-menu--sub`). */
  isSub?: boolean;
  /** Focus the first enabled row once mounted (root mount / keyboard-opened sub). */
  autoFocus?: boolean;
  /** Submenu only: ArrowLeft (or a leftward close request) hands focus back. */
  onCloseRequest?(): void;
  /** Extra inline style (root positioning). */
  style?: React.CSSProperties;
  /** Root only: the container ref the parent uses for clamp/dismiss. */
  containerRef?: React.RefObject<HTMLDivElement | null>;
  /** Root only: the §4.4 header block. */
  header?: React.ReactNode;
  /** Root only: `aria-busy` while an activated row is settling. */
  busy?: boolean;
}

/** Recursive menu list: renders one `.context-menu` (or `.context-menu--sub`)
 *  container of button rows. A row with `children` opens a nested MenuList
 *  flyout on hover (delayed) or ArrowRight. Focus queries are scoped to this
 *  list's own rows so root and submenu navigate independently. */
function MenuList({
  items,
  onClose,
  isSub = false,
  autoFocus = false,
  onCloseRequest,
  style,
  containerRef,
  header,
  busy,
}: MenuListProps) {
  const ownRef = useRef<HTMLDivElement>(null);
  const localRef = containerRef ?? ownRef;
  // The 16px check column belongs to the LIST: if only the rows that declare
  // `checked` reserved it, every plain row in the same menu (`Manage
  // identities…`) would hang 24px to the left of the labelled ones.
  const hasChecks = items.some((it) => it.checked !== undefined);

  const [openIndex, setOpenIndex] = useState<number | null>(null);
  const [focusSub, setFocusSub] = useState(false);
  const [subStyle, setSubStyle] = useState<React.CSSProperties>(
    isSub ? { left: '100%', top: 0, visibility: 'hidden' } : {},
  );
  const hoverTimer = useRef<number | undefined>(undefined);
  const closeTimer = useRef<number | undefined>(undefined);

  // Position + clamp the flyout (submenu only): open rightward by default, flip
  // leftward on right-edge overflow; raise it into view on bottom overflow.
  useLayoutEffect(() => {
    if (!isSub) return;
    const el = localRef.current;
    const row = el?.parentElement;
    if (!el || !row) return;
    const rect = el.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    const next: React.CSSProperties = { top: 0, visibility: 'visible' };
    if (rowRect.right + rect.width > window.innerWidth - 4) next.right = '100%';
    else next.left = '100%';
    const overflowY = rowRect.top + rect.height - (window.innerHeight - 4);
    if (overflowY > 0) next.top = -Math.min(overflowY, Math.max(0, rowRect.top - 4));
    setSubStyle(next);
  }, [isSub, items]);

  // Focus the first enabled row when requested (root mount / keyboard-opened sub).
  useEffect(() => {
    if (autoFocus) focusFirst();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoFocus]);

  useEffect(
    () => () => {
      window.clearTimeout(hoverTimer.current);
      window.clearTimeout(closeTimer.current);
    },
    [],
  );

  const scopedButtons = (): HTMLButtonElement[] => {
    const el = localRef.current;
    if (el === null) return [];
    return Array.from(
      el.querySelectorAll<HTMLButtonElement>(
        ':scope > .context-menu-row > button[role="menuitem"],' +
          ':scope > .context-menu-row > button[role="menuitemradio"]',
      ),
    );
  };

  const focusFirst = () => {
    const b = scopedButtons().find((x) => !x.disabled);
    b?.focus();
  };

  const moveFocus = (index: number, step: number) => {
    const buttons = scopedButtons();
    const n = buttons.length;
    if (n === 0) return;
    for (let k = 1; k <= n; k++) {
      const j = (((index + step * k) % n) + n) % n;
      if (!buttons[j].disabled) {
        buttons[j].focus();
        break;
      }
    }
  };

  const openSubmenu = (index: number, byKeyboard: boolean) => {
    setFocusSub(byKeyboard);
    setOpenIndex(index);
  };

  const activate = (item: ContextMenuItem, index: number) => {
    if (item.disabled === true) return;
    if (item.onSelect !== undefined) {
      item.onSelect();
      onClose();
      return;
    }
    if (item.children !== undefined) {
      setOpenIndex((prev) => (prev === index ? null : index));
      setFocusSub(true);
    }
  };

  const onRowEnter = (index: number, item: ContextMenuItem) => {
    window.clearTimeout(closeTimer.current);
    // Hovering a different row closes any currently-open flyout immediately.
    if (openIndex !== null && openIndex !== index) setOpenIndex(null);
    if (item.children !== undefined && item.disabled !== true) {
      window.clearTimeout(hoverTimer.current);
      hoverTimer.current = window.setTimeout(() => openSubmenu(index, false), HOVER_OPEN_MS);
    }
  };

  const onRowLeave = (index: number, item: ContextMenuItem) => {
    window.clearTimeout(hoverTimer.current);
    if (item.children !== undefined && openIndex === index) {
      closeTimer.current = window.setTimeout(() => setOpenIndex(null), HOVER_CLOSE_MS);
    }
  };

  const onItemKeyDown = (
    e: React.KeyboardEvent<HTMLButtonElement>,
    index: number,
    item: ContextMenuItem,
  ) => {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        moveFocus(index, 1);
        return;
      case 'ArrowUp':
        e.preventDefault();
        moveFocus(index, -1);
        return;
      case 'ArrowRight':
        if (item.children !== undefined && item.disabled !== true) {
          e.preventDefault();
          openSubmenu(index, true);
        }
        return;
      case 'ArrowLeft':
        if (isSub && onCloseRequest !== undefined) {
          e.preventDefault();
          onCloseRequest();
        }
        return;
      case 'Enter':
      case ' ':
        e.preventDefault();
        activate(item, index);
        return;
      default:
        return;
    }
  };

  return (
    <div
      ref={localRef}
      className={isSub ? 'context-menu context-menu--sub' : 'context-menu'}
      role="menu"
      aria-busy={busy === true ? true : undefined}
      style={isSub ? subStyle : style}
    >
      {header !== undefined && (
        <div className="context-menu-header" role="presentation">
          {header}
        </div>
      )}
      {items.map((item, i) => {
        const hasChildren = item.children !== undefined;
        const isOpen = openIndex === i;
        const isRadio = item.checked !== undefined;
        return (
          <div
            /* Index key, deliberately. Two identity profiles may share a label
               (both blank ⇒ both "Unnamed identity"), and — the reason it is not
               the label — a row whose label mutates in place (`… — Applying…`)
               must NOT remount: it holds keyboard focus in a menu that stays
               open for the whole write. These lists never reorder in place. */
            key={i}
            className="context-menu-row"
            onMouseEnter={() => onRowEnter(i, item)}
            onMouseLeave={() => onRowLeave(i, item)}
          >
            <button
              type="button"
              role={isRadio ? 'menuitemradio' : 'menuitem'}
              aria-checked={isRadio ? item.checked === true : undefined}
              className="context-menu-item"
              data-tone={item.tone === 'danger' ? 'danger' : undefined}
              disabled={item.disabled === true}
              aria-disabled={item.disabled === true}
              aria-haspopup={hasChildren ? 'menu' : undefined}
              aria-expanded={hasChildren ? isOpen : undefined}
              tabIndex={-1}
              onClick={() => activate(item, i)}
              onKeyDown={(e) => onItemKeyDown(e, i, item)}
            >
              {hasChecks && (
                <span className="context-menu-check" aria-hidden="true">
                  {item.checked === true ? '✓' : ''}
                </span>
              )}
              {item.icon !== undefined && (
                <span className="context-menu-icon" aria-hidden="true">
                  {item.icon}
                </span>
              )}
              {item.detail === undefined ? (
                <span className="context-menu-label">{item.label}</span>
              ) : (
                <span className="context-menu-lines">
                  <span className="context-menu-label">{item.label}</span>
                  <span className="context-menu-detail" title={item.detail}>
                    {item.detail}
                  </span>
                </span>
              )}
              {hasChildren && (
                <span className="context-menu-chevron" aria-hidden="true">
                  ▶
                </span>
              )}
            </button>
            {hasChildren && isOpen && (
              <MenuList
                items={item.children as ContextMenuItem[]}
                onClose={onClose}
                isSub
                autoFocus={focusSub}
                onCloseRequest={() => {
                  setOpenIndex(null);
                  scopedButtons()[i]?.focus();
                }}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

/** Small reusable right-click menu (P5 §5.1). Fixed-positioned at (x, y),
 *  clamped into the viewport. Dismisses on outside pointerdown, Escape, scroll
 *  (capture), resize, and window blur. Rows with `children` open a hover/keyboard
 *  flyout submenu. All colors come from CSS variables so both themes work. */
export function ContextMenu({ x, y, items, onClose, header, busy }: ContextMenuProps) {
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

  return (
    <MenuList
      containerRef={rootRef}
      items={items}
      onClose={onClose}
      autoFocus
      header={header}
      busy={busy}
      style={{ left: pos.x, top: pos.y }}
    />
  );
}
