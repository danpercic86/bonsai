// Extracted from Sidebar.tsx (P77) so the shared collapsible section header can
// be reused by the split-out TagsSection without a circular import. Presentational
// only: a toggle button (the sole tab stop / expand control) plus an optional
// `extra` slot rendered right of the toggle (add buttons, the P77 rollup badge).
import type { ReactNode } from 'react';

export function SectionHeader({
  label,
  collapsed,
  onToggle,
  extra,
}: {
  label: string;
  collapsed: boolean;
  onToggle(): void;
  extra?: ReactNode;
}) {
  return (
    <div className="sidebar-section-header">
      <button
        type="button"
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
