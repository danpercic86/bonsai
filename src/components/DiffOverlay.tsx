import type { ConflictFile, FileStatus } from '../ipc';
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
  untracked: 'A',
  conflicted: 'C',
};

const KIND_LABEL: Record<DiffOverlayMeta['kind'], string> = {
  staged: 'Staged',
  unstaged: 'Unstaged',
  untracked: 'Untracked',
  commit: 'Commit',
  conflict: 'Conflict',
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
  kind: 'staged' | 'unstaged' | 'untracked' | 'commit' | 'conflict';
}

// P3c §8.3 (locked): the marker view is a plain highlighted <pre>, NOT
// DiffView — the marker text is one file body, not hunks.
const MARKER_RE = /^(<{7}|={7}|>{7})/;

function ConflictMarkerView({ file }: { file: ConflictFile }) {
  if (file.binary) return <div className="diff-placeholder">Binary file</div>;
  if (file.tooLarge) return <div className="diff-placeholder">File too large to display</div>;
  if (file.missing) return <div className="diff-placeholder">File was deleted</div>;
  return (
    <pre className="conflict-view">
      {file.text.split('\n').map((line, i) => (
        <div
          key={i}
          className={MARKER_RE.test(line) ? 'conflict-line conflict-marker-line' : 'conflict-line'}
        >
          {line === '' ? ' ' : line}
        </div>
      ))}
    </pre>
  );
}

/** Loading / error / ready body for a `conflict:<path>` slot — same state
 * recipe as DiffSlotView but rendering the ConflictFile marker view. */
function ConflictSlotView({ slot, onDismissError }: { slot: DiffSlot; onDismissError(): void }) {
  const file = slot.conflict ?? null;
  if (slot.state === 'loading' && file === null) {
    return (
      <div className="diff-slot-loading skeleton-group" aria-hidden="true">
        {Array.from({ length: 3 }, (_, i) => (
          <div key={i} className="skeleton-row" />
        ))}
      </div>
    );
  }
  if (slot.state === 'error') {
    return (
      <div className="error-banner error-banner-dismissible diff-slot-error" role="alert">
        <span className="error-banner-text">{slot.error}</span>
        <button
          type="button"
          className="error-dismiss"
          aria-label="Dismiss error"
          onClick={onDismissError}
        >
          {'×'}
        </button>
      </div>
    );
  }
  return file !== null ? (
    <div className={slot.state === 'loading' ? 'diff-scroll diff-stale' : 'diff-scroll'}>
      <ConflictMarkerView file={file} />
    </div>
  ) : null;
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
        {meta.kind === 'conflict' ? (
          <ConflictSlotView slot={slot} onDismissError={onClose} />
        ) : (
          <DiffSlotView slot={slot} onDismissError={onClose} />
        )}
      </div>
    </div>
  );
}
