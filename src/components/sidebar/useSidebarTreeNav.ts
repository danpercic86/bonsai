// P-a11y §D.4: the sidebar tree's focus controller. Owns `activeKey` (roving
// tabindex), implements D.3 movement centrally on the tree root, and drives the
// D.6 context-menu focus restore. Returns `rootProps` to spread on the
// `role="tree"` wrapper plus the context value the treeitems consume.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent } from 'react';
import type { SidebarTreeContextValue, SidebarTreeNavOp } from './SidebarTreeContext';

/** Row height (px) — matches `.branch-row`/`.sidebar-section-header`. Page nav
 *  moves by one viewport of rows, mirroring the graph's PageUp/PageDown. */
const ROW_PX = 24;

interface UseSidebarTreeNavOptions {
  /** Default active row: the HEAD branch when checked out, else the first header. */
  currentBranch: string | null;
}

interface SidebarTreeNav {
  rootProps: {
    role: 'tree';
    'aria-label': string;
    ref: React.RefObject<HTMLDivElement | null>;
    onKeyDown(e: ReactKeyboardEvent<HTMLDivElement>): void;
  };
  context: SidebarTreeContextValue;
}

/** Visible treeitems in DOM order. Collapsed groups/dirs are not rendered at all
 *  (conditional render, not CSS), so a plain query already yields only the
 *  navigable rows — no layout-dependent `offsetParent` filter (which is always
 *  null in jsdom) is needed. */
function navItems(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>('[data-tree-item]'));
}

/** The focused treeitem, resolving inner focus (e.g. a dir's toggle button) up
 *  to its owning `[data-tree-item]`. */
function activeItem(root: HTMLElement): HTMLElement | null {
  const el = document.activeElement;
  if (!(el instanceof HTMLElement)) return null;
  const item = el.closest<HTMLElement>('[data-tree-item]');
  return item !== null && root.contains(item) ? item : null;
}

function pageSize(root: HTMLElement): number {
  const viewport = root.closest<HTMLElement>('.sidebar');
  const h = viewport?.clientHeight ?? 0;
  return Math.max(1, Math.floor(h / ROW_PX));
}

/** ArrowLeft on a leaf / collapsed group → the nearest preceding item with a
 *  smaller `aria-level` (its parent group). */
function parentOf(items: HTMLElement[], idx: number): HTMLElement | undefined {
  if (idx < 0) return undefined;
  const level = Number(items[idx].getAttribute('aria-level') ?? '1');
  for (let i = idx - 1; i >= 0; i--) {
    if (Number(items[i].getAttribute('aria-level') ?? '1') < level) return items[i];
  }
  return undefined;
}

export function useSidebarTreeNav({ currentBranch }: UseSidebarTreeNavOptions): SidebarTreeNav {
  const rootRef = useRef<HTMLDivElement>(null);
  const [activeKey, setActiveKeyState] = useState<string>(() =>
    currentBranch !== null ? `branch:${currentBranch}` : 'header:Branches',
  );
  const restoreObserverRef = useRef<MutationObserver | null>(null);

  const setActiveKey = useCallback((key: string) => setActiveKeyState(key), []);

  const move = useCallback((op: SidebarTreeNavOp, from?: HTMLElement) => {
    const root = rootRef.current;
    if (root === null) return;
    const items = navItems(root);
    if (items.length === 0) return;
    const current = from ?? activeItem(root);
    const idx = current !== null ? items.indexOf(current) : -1;
    const last = items.length - 1;
    let target: HTMLElement | undefined;
    switch (op) {
      case 'first':
        target = items[0];
        break;
      case 'last':
        target = items[last];
        break;
      case 'next':
        target = items[Math.min(last, (idx < 0 ? -1 : idx) + 1)];
        break;
      case 'firstChild':
        // Expanded group → its first child is the next visible item.
        target = items[Math.min(last, (idx < 0 ? -1 : idx) + 1)];
        break;
      case 'prev':
        target = items[Math.max(0, (idx < 0 ? 1 : idx) - 1)];
        break;
      case 'pageDown':
        target = items[Math.min(last, (idx < 0 ? 0 : idx) + pageSize(root))];
        break;
      case 'pageUp':
        target = items[Math.max(0, (idx < 0 ? 0 : idx) - pageSize(root))];
        break;
      case 'parent':
        target = parentOf(items, idx);
        break;
      default:
        return;
    }
    target?.focus();
  }, []);

  const navigate = useCallback(
    (op: SidebarTreeNavOp, from: HTMLElement) => move(op, from),
    [move],
  );

  // D.6: open a menu by keyboard and restore focus to the triggering row when the
  // shared ContextMenu unmounts. The menu is rendered far up the tree (by
  // RepoWorkspace) and never restores focus itself, so the Sidebar watches for
  // the `[role="menu"]` node to leave the DOM — covering every close path (Esc,
  // click-away, item activation) with no cross-component plumbing.
  const openRowMenu = useCallback((key: string, open: () => void) => {
    restoreObserverRef.current?.disconnect();
    const root = rootRef.current;
    open();
    if (root === null) return;
    let sawMenu = false;
    const observer = new MutationObserver(() => {
      if (document.querySelector('[role="menu"]') !== null) {
        sawMenu = true;
        return;
      }
      if (!sawMenu) return;
      observer.disconnect();
      if (restoreObserverRef.current === observer) restoreObserverRef.current = null;
      setActiveKeyState(key);
      const el = navItems(root).find((n) => n.dataset.treeKey === key);
      el?.focus();
    });
    restoreObserverRef.current = observer;
    observer.observe(document.body, { childList: true, subtree: true });
  }, []);

  // Disconnect a pending restore-watcher on unmount.
  useEffect(() => () => restoreObserverRef.current?.disconnect(), []);

  // Keep exactly one item in the Tab cycle: if `activeKey` isn't rendered (HEAD
  // filtered/collapsed away, section toggled), fall back to the first item so Tab
  // always lands somewhere. Deliberately dep-less — the rendered item set changes
  // via Sidebar state this hook can't observe (filters, collapse), so it must run
  // after every commit; the guard makes it a no-op (never loops) when valid.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    const root = rootRef.current;
    if (root === null) return;
    const items = navItems(root);
    if (items.length === 0) return;
    if (!items.some((el) => el.dataset.treeKey === activeKey)) {
      setActiveKeyState(items[0].dataset.treeKey ?? 'header:Branches');
    }
  });

  const onKeyDown = useCallback(
    (e: ReactKeyboardEvent<HTMLDivElement>) => {
      const root = rootRef.current;
      if (root === null || activeItem(root) === null) return; // focus not on a row
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          move('next');
          return;
        case 'ArrowUp':
          e.preventDefault();
          move('prev');
          return;
        case 'Home':
          e.preventDefault();
          move('first');
          return;
        case 'End':
          e.preventDefault();
          move('last');
          return;
        case 'PageDown':
          e.preventDefault();
          move('pageDown');
          return;
        case 'PageUp':
          e.preventDefault();
          move('pageUp');
          return;
        default:
          return;
      }
    },
    [move],
  );

  const context = useMemo<SidebarTreeContextValue>(
    () => ({ activeKey, setActiveKey, navigate, openRowMenu }),
    [activeKey, setActiveKey, navigate, openRowMenu],
  );

  return {
    rootProps: { role: 'tree', 'aria-label': 'Repository sidebar', ref: rootRef, onKeyDown },
    context,
  };
}
