// P118: the section-header action button (`+` / "Clean up branches…"), split out
// so that "disabled while an action is in flight" costs ONE leaf re-render per
// flip instead of a re-render of the whole memoised section (and, through it,
// every row). It reads `useSidebarBusy()` itself — the section that renders it
// must NOT take the flag as a prop.
import type { ReactNode } from 'react';
import { useSidebarBusy } from './sidebarBusyContext';

export interface SidebarActionButtonProps {
  label: string;
  onClick(): void;
  /** `sidebar-add-icon` for the glyph buttons; omit for the plain "+". */
  iconOnly?: boolean;
  children: ReactNode;
}

export function SidebarActionButton({
  label,
  onClick,
  iconOnly = false,
  children,
}: SidebarActionButtonProps) {
  const busy = useSidebarBusy();
  return (
    <button
      type="button"
      className={iconOnly ? 'sidebar-add sidebar-add-icon' : 'sidebar-add'}
      aria-label={label}
      title={label}
      disabled={busy}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
