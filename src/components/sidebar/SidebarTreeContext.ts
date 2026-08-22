// P-a11y §D.4: shared state for the sidebar's composite `role="tree"`. One
// Tab stop, roving tabindex: exactly one treeitem carries `tabIndex=0` (the
// `activeKey`), all others `-1`. Movement is centralised on the tree root
// (`useSidebarTreeNav`); each treeitem owns its activation/structure keys and
// reads this context for its roving state + the shared helpers.
import { createContext, useContext } from 'react';

/** D.3 movement, resolved against the visible `[data-tree-item]` elements in DOM
 *  order at event time. Used both by the root controller (Arrow/Home/End/Page)
 *  and by per-item handlers (`firstChild` for ArrowRight-on-expanded-group,
 *  `parent` for ArrowLeft). */
export type SidebarTreeNavOp =
  | 'next'
  | 'prev'
  | 'first'
  | 'last'
  | 'pageDown'
  | 'pageUp'
  | 'firstChild'
  | 'parent';

export interface SidebarTreeContextValue {
  /** Key of the single treeitem in the Tab cycle (roving tabindex). */
  activeKey: string;
  /** Called from each treeitem's `onFocus` to keep roving state ↔ DOM in sync. */
  setActiveKey(key: string): void;
  /** Move focus relative to `from` (D.3). `from` is the event's currentTarget. */
  navigate(op: SidebarTreeNavOp, from: HTMLElement): void;
  /** D.6: open a row's context menu by keyboard, remembering the row so focus is
   *  restored to it when the shared ContextMenu closes (Esc / click-away /
   *  activation). `open` is the caller that actually opens the menu (via the
   *  Sidebar's existing `on*ContextMenu` prop). */
  openRowMenu(key: string, open: () => void): void;
}

export const SidebarTreeContext = createContext<SidebarTreeContextValue | null>(null);

export const SidebarTreeProvider = SidebarTreeContext.Provider;

/** Non-throwing accessor: returns `null` outside a provider so shared components
 *  (Tree, rendered both in the sidebar tree AND the status file tree) can opt out
 *  of the roving/context wiring when not embedded in the sidebar. */
export function useSidebarTree(): SidebarTreeContextValue | null {
  return useContext(SidebarTreeContext);
}
