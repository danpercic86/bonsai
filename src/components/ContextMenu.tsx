import { useEffect, useLayoutEffect, useRef, useState } from 'react';

export interface ContextMenuItem {
  label: string;
  /** P92: native `title` tooltip on the row (picker rows carry the FULL ref
   *  name, since the label is the short/collapsed form and may ellipsise). */
  title?: string;
  /** P92: `true` ⇒ this entry renders as a non-interactive `role="separator"`
   *  rule instead of a row. Keyboard navigation skips it for free (the focus
   *  queries scope to `button[role="menuitem*"]`). `label` is ignored. */
  separator?: true;
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

/** P92: the open-menu state a container holds (position + items + the optional
 *  §4 header / accessible name). Shared so every ContextMenu owner spells it the
 *  same way. */
export interface ContextMenuState {
  x: number;
  y: number;
  items: ContextMenuItem[];
  header?: string;
  ariaLabel?: string;
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
  /** P92: accessible name for the menu root (e.g. `3 more refs on commit 4f2a91c`).
   *  Absent ⇒ no `aria-label` (byte-identical to pre-P92). */
  ariaLabel?: string;
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
  /** true ⇒ render as a viewport-fixed flyout (`.context-menu--sub`, P92 §8.1). */
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
  /** Root only: P92 accessible name for the menu root. */
  ariaLabel?: string;
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
  ariaLabel,
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
  // P92 §8.1 / addendum A.1: the flyout is `position: fixed`, so the pre-measure
  // pass must use viewport numbers (a `left: '100%'` would be 100% of the
  // VIEWPORT, not of the row) and stay invisible until measured.
  const [subStyle, setSubStyle] = useState<React.CSSProperties>(
    isSub ? { left: 0, top: 0, visibility: 'hidden' } : {},
  );
  const hoverTimer = useRef<number | undefined>(undefined);
  const closeTimer = useRef<number | undefined>(undefined);

  // Position + clamp the flyout (submenu only): open rightward by default, flip
  // leftward on right-edge overflow; raise it into view on bottom overflow.
  //
  // P92 addendum A.1: coordinates are VIEWPORT coordinates derived from the
  // anchor row's `getBoundingClientRect()`, because the flyout is now
  // `position: fixed`. The old `position: absolute` made it a descendant of the
  // parent's scroll box (`.context-menu { overflow-y: auto }`, which computes
  // `overflow-x` to `auto`), so every flyout was clipped and gave its parent a
  // spurious horizontal scrollbar. The trade-off — a fixed flyout no longer
  // tracks its row — is handled by closing it when the parent's box scrolls.
  useLayoutEffect(() => {
    if (!isSub) return;
    const el = localRef.current;
    const row = el?.parentElement;
    if (!el || !row) return;
    const rect = el.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    const left =
      rowRect.right + rect.width > window.innerWidth - 4
        ? Math.max(4, rowRect.left - rect.width) // flip leftward
        : rowRect.right;
    const top = Math.max(4, Math.min(rowRect.top, window.innerHeight - 4 - rect.height));
    setSubStyle({ left, top, visibility: 'visible' });
  }, [isSub, items, localRef]);

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

  /** P92: focus + keep the row inside the (now scrollable, §1.3) menu box. */
  const focusRow = (b: HTMLButtonElement) => {
    b.focus();
    b.scrollIntoView({ block: 'nearest' });
  };

  const focusFirst = () => {
    const b = scopedButtons().find((x) => !x.disabled);
    if (b !== undefined) focusRow(b);
  };

  const moveFocus = (index: number, step: number) => {
    const buttons = scopedButtons();
    const n = buttons.length;
    if (n === 0) return;
    // P92: `index` is the ITEMS index, which diverges from the button index once
    // a `separator` entry is present — resolve from the focused button first.
    const domIndex = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const from = domIndex >= 0 ? domIndex : Math.min(index, n - 1);
    for (let k = 1; k <= n; k++) {
      const j = (((from + step * k) % n) + n) % n;
      if (!buttons[j].disabled) {
        focusRow(buttons[j]);
        break;
      }
    }
  };

  /** P92 addendum A.1: a `position: fixed` flyout does not move with its row, so
   *  when THIS list's scroll box scrolls the open flyout would be left stranded
   *  next to a row that has moved. Close it (hover or ArrowRight reopens it at
   *  the row's new position). React's `onScroll` does not bubble, so this fires
   *  only for this list's own box — scrolling a submenu never closes it. */
  const onListScroll = () => {
    window.clearTimeout(hoverTimer.current);
    setOpenIndex(null);
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
      aria-label={ariaLabel}
      aria-busy={busy === true ? true : undefined}
      style={isSub ? subStyle : style}
      onScroll={onListScroll}
    >
      {header !== undefined && (
        <div className="context-menu-header" role="presentation">
          {header}
        </div>
      )}
      {items.map((item, i) => {
        if (item.separator === true) {
          return <div key={i} className="context-menu-sep" role="separator" />;
        }
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
              title={item.title}
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
