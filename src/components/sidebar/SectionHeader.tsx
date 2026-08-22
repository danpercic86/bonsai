// Extracted from Sidebar.tsx (P77) so the shared collapsible section header can
// be reused by the split-out TagsSection without a circular import. Presentational
// only: a toggle button (the sole tab stop / expand control) plus an optional
// `extra` slot rendered right of the toggle (add buttons, the P77 rollup badge).
//
// P-a11y §D.2: the toggle button is a level-1 `role="treeitem"` in the sidebar
// tree — roving tabindex, ArrowLeft/Right collapse/expand, Enter/Space toggle
// (native to the `<button>`). The `extra` buttons stay separate Tab stops (§D.1).
import type { ReactNode } from 'react';
import { useSidebarTreeItem } from './useSidebarTreeItem';

export function SectionHeader({
  label,
  collapsed,
  onToggle,
  extra,
  treeKey,
}: {
  label: string;
  collapsed: boolean;
  onToggle(): void;
  extra?: ReactNode;
  /** P-a11y: treeitem key (`header:<label>`). Defaults to `header:<label>`. */
  treeKey?: string;
}) {
  const item = useSidebarTreeItem({
    treeKey: treeKey ?? `header:${label}`,
    level: 1,
    kind: 'group',
    nativeActivate: true,
    expanded: !collapsed,
    onToggle,
  });
  return (
    <div className="sidebar-section-header">
      <button
        {...item}
        type="button"
        role="treeitem"
        className="sidebar-section-toggle section-label"
        aria-expanded={!collapsed}
        onClick={onToggle}
      >
        <span className={`file-chevron${collapsed ? '' : ' file-chevron-open'}`}>{'›'}</span>
        {label}
      </button>
      {extra}
    </div>
  );
}
