// P73 §4: the submodule status → badge map. Its own module so SubmoduleRow.tsx
// exports components only (react-refresh/only-export-components) and the copy
// table stays readable next to the contract.
import type { SubmoduleStatus } from '../../ipc';

export interface SubmoduleBadge {
  label: string;
  intent: string;
  /** Explains WHY the state holds + what fixes it. Never the label again. */
  title: string;
  /** ui-reference §11: verdict pills carry an aria-hidden glyph so hue is never
   *  the sole carrier; hueless (informational) pills have none. */
  glyph: string | null;
}

/** P19 §6.2 + P73 §4: display-only status pill. Label + intent + title per status. */
export const SUBMODULE_BADGE: Record<SubmoduleStatus, SubmoduleBadge> = {
  uninitialized: {
    label: 'not checked out',
    intent: 'submodule-badge-muted',
    title: 'No files on disk yet. Right-click the row → Initialize and check out.',
    glyph: null,
  },
  upToDate: {
    label: 'up to date',
    intent: 'submodule-badge-ok',
    title: 'Files on disk match the commit the superproject pins.',
    glyph: '✓',
  },
  outOfSync: {
    label: 'out of sync',
    intent: 'submodule-badge-warn',
    title: 'Checked out at a different commit than the superproject pins. Update to fix.',
    glyph: '⚠',
  },
  modifiedWorkdir: {
    label: 'modified',
    intent: 'submodule-badge-warn',
    title: 'Uncommitted changes inside this submodule.',
    glyph: '⚠',
  },
};
