// P-a11y §D.2/§D.3/§D.5: per-treeitem wiring. Returns the props each row spreads
// onto its focusable element (the `<li>`, or the section header `<button>`): the
// roving `tabIndex`, `aria-level`/`aria-current`/`aria-disabled`,
// `data-tree-item`, `onFocus` (→ roving sync) and `onKeyDown` for the row's own
// activation/structure keys. Movement keys are left to bubble to the tree root.
//
// Null-safe: outside a `SidebarTreeProvider` (or with `enabled: false`, e.g. the
// status-panel file tree via `Tree` without `asGroup`) it returns `{}`, so the
// caller's element keeps its current, un-wired markup.
import type { FocusEventHandler, KeyboardEvent as ReactKeyboardEvent, KeyboardEventHandler } from 'react';
import { useSidebarTree } from './SidebarTreeContext';

export interface SidebarTreeItemOptions {
  /** Stable, namespaced key (`branch:<name>`, `header:<label>`, `dir:<prefix>`…). */
  treeKey: string;
  /** 1-based depth: section header = 1, its rows = 2, nested tree levels deeper. */
  level: number;
  kind: 'group' | 'leaf';
  /** Default true; false ⇒ inert (no wiring), for shared components not embedded
   *  in the sidebar tree. */
  enabled?: boolean;
  /** group: the element natively activates on Enter/Space (a `<button>`), so this
   *  hook must NOT also toggle (avoids a double-fire). */
  nativeActivate?: boolean;
  /** group: current expanded state (drives ArrowLeft/ArrowRight). */
  expanded?: boolean;
  /** HEAD branch row → `aria-current="true"`. */
  ariaCurrent?: boolean;
  /** Read-only info row (detached-HEAD) → `aria-disabled="true"`. */
  ariaDisabled?: boolean;
  /** group: expand/collapse toggle (Enter/Space on a non-button, Arrow keys). */
  onToggle?(): void;
  /** leaf: Enter/Space primary action (D.5), e.g. checkout. A disabled/HEAD row
   *  omits it ⇒ Enter/Space is a no-op (NOT a menu fallback). */
  onPrimary?(): void;
  /** leaf: the row's only affordance is its menu (remote/stash/submodule/…), so
   *  Enter/Space opens it (D.5). Distinct from a row with a disabled primary. */
  menuIsPrimary?: boolean;
  /** Opens this row's context menu at (x, y). Present ⇒ ContextMenu key / Shift+F10
   *  (and Enter on menu-only leaves) open it, with D.6 focus restore. */
  openMenuAt?(x: number, y: number): void;
}

export interface SidebarTreeItemProps {
  'aria-level'?: number;
  'aria-current'?: 'true';
  'aria-disabled'?: boolean;
  tabIndex?: number;
  'data-tree-item'?: string;
  'data-tree-key'?: string;
  onFocus?: FocusEventHandler;
  onKeyDown?: KeyboardEventHandler;
}

export function useSidebarTreeItem(opts: SidebarTreeItemOptions): SidebarTreeItemProps {
  const ctx = useSidebarTree();
  if (ctx === null || opts.enabled === false) return {};

  const { treeKey, level, kind, expanded, nativeActivate, onToggle, onPrimary, openMenuAt } = opts;
  const menuIsPrimary = opts.menuIsPrimary === true;

  const openMenu = (el: HTMLElement) => {
    if (openMenuAt === undefined) return;
    const r = el.getBoundingClientRect();
    // Row rect, not a cursor: 8px in from the left, flush under the row (D.6).
    ctx.openRowMenu(treeKey, () => openMenuAt(r.left + 8, r.bottom));
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLElement>) => {
    const el = e.currentTarget;
    if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
      if (openMenuAt !== undefined) {
        e.preventDefault();
        openMenu(el);
      }
      return;
    }
    if (kind === 'group') {
      switch (e.key) {
        case 'Enter':
        case ' ':
          // A `<button>` header toggles natively — don't double-fire.
          if (nativeActivate !== true) {
            e.preventDefault();
            onToggle?.();
          }
          return;
        case 'ArrowRight':
          e.preventDefault();
          if (expanded === true) ctx.navigate('firstChild', el);
          else onToggle?.();
          return;
        case 'ArrowLeft':
          e.preventDefault();
          if (expanded === true) onToggle?.();
          else ctx.navigate('parent', el); // top-level header → no parent → no-op
          return;
        default:
          return; // movement keys bubble to the tree root
      }
    }
    // leaf
    switch (e.key) {
      case 'Enter':
      case ' ':
        e.preventDefault(); // Space must not scroll the pane
        if (menuIsPrimary) openMenu(el);
        else onPrimary?.();
        return;
      case 'ArrowLeft':
        e.preventDefault();
        ctx.navigate('parent', el);
        return;
      default:
        return; // ArrowRight is a no-op on a leaf; movement keys bubble
    }
  };

  return {
    'aria-level': level,
    'aria-current': opts.ariaCurrent === true ? 'true' : undefined,
    'aria-disabled': opts.ariaDisabled === true ? true : undefined,
    tabIndex: ctx.activeKey === treeKey ? 0 : -1,
    'data-tree-item': '',
    'data-tree-key': treeKey,
    onFocus: () => ctx.setActiveKey(treeKey),
    onKeyDown,
  };
}
