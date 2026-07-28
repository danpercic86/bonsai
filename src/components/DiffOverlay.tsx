import type { FileStatus } from '../ipc';
import { DiffSlotView } from './DiffView';
import type { DiffSlot } from './DiffView';

// P3a §2.2: full-pane diff overlay over the center graph pane. Purely
// presentational — App owns the slot state, meta derivation, and Esc handling.

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'U',
  conflicted: 'C',
};

const KIND_LABEL: Record<DiffOverlayMeta['kind'], string> = {
  staged: 'Staged',
  unstaged: 'Unstaged',
  untracked: 'Untracked',
  commit: 'Commit',
};

/** Display metadata for the overlay header, derived by App (P3a §2.3) from the
 * slot key + the current snapshot/commitDiff. Never stored — recomputed each
 * render so it can't go stale relative to the data that produced the slot. */
export interface DiffOverlayMeta {
  path: string;
  /** Rename: header shows "orig → path". */
  origPath: string | null;
  /** null = lookup failed (P3a §2.3 fallback): no badge. */
  status: FileStatus | null;
  /** Drives the header context label. */
  kind: 'staged' | 'unstaged' | 'untracked' | 'commit';
}

export interface DiffOverlayProps {
  /** Non-null by construction — App only mounts the overlay when a slot is open. */
  slot: DiffSlot;
  meta: DiffOverlayMeta;
  /** × button AND error-banner dismiss both call this. */
  onClose(): void;
}

export function DiffOverlay({ slot, meta, onClose }: DiffOverlayProps) {
  return (
    <div className="diff-overlay" role="region" aria-label={`Diff: ${meta.path}`}>
      <div className="diff-overlay-header">
        {meta.status !== null && <span className="file-badge mono">{BADGES[meta.status]}</span>}
        {meta.origPath !== null ? (
          <span
            className="diff-overlay-path mono file-rename"
            title={`${meta.origPath} → ${meta.path}`}
          >
            {meta.origPath} {'→'} {meta.path}
          </span>
        ) : (
          <span className="diff-overlay-path mono" title={meta.path}>
            {meta.path}
          </span>
        )}
        <span className="diff-overlay-kind">{KIND_LABEL[meta.kind]}</span>
        <button
          type="button"
          className="btn-icon diff-overlay-close"
          aria-label="Close diff"
          title="Close (Esc)"
          onClick={onClose}
        >
          {'×'}
        </button>
      </div>
      <div className="diff-overlay-body">
        <DiffSlotView slot={slot} onDismissError={onClose} />
      </div>
    </div>
  );
}
