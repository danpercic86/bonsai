import type { FileHistoryEntry } from '../ipc';
import { relativeDate } from '../graph/draw';

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

export interface FileHistoryViewProps {
  path: string;
  entries: FileHistoryEntry[];
  loading: boolean;
  error: string | null;
  onClose(): void;
  /** Reveal (select + scroll) the clicked commit in the graph. */
  onRevealCommit(oid: string): void;
}

/** Read-only per-file commit history overlay (P23d §11.2). Layered over the
 *  graph pane like the diff overlay; row click reveals the commit in the graph. */
export function FileHistoryView({
  path,
  entries,
  loading,
  error,
  onClose,
  onRevealCommit,
}: FileHistoryViewProps) {
  const now = Math.floor(Date.now() / 1000);

  return (
    <div className="diff-overlay file-history-view" role="region" aria-label={`File history: ${path}`}>
      <div className="diff-overlay-header">
        <span className="diff-overlay-path mono" title={path}>
          {path}
        </span>
        <span className="diff-overlay-kind">File history</span>
        <button
          type="button"
          className="btn-icon diff-overlay-close"
          aria-label="Close file history"
          title="Close (Esc)"
          onClick={onClose}
        >
          {'×'}
        </button>
      </div>
      <div className="diff-overlay-body">
        {error !== null ? (
          <div className="diff-placeholder">{error}</div>
        ) : loading && entries.length === 0 ? (
          <div className="diff-slot-loading skeleton-group" aria-hidden="true">
            {Array.from({ length: 6 }, (_, i) => (
              <div key={i} className="skeleton-row" />
            ))}
          </div>
        ) : entries.length === 0 ? (
          <div className="diff-placeholder">No history for this file</div>
        ) : (
          <div className={loading ? 'diff-scroll diff-stale' : 'diff-scroll'}>
            <ul className="file-history-list">
              {entries.map((entry) => (
                <li key={entry.oid} className="file-history-row">
                  <button
                    type="button"
                    className="file-history-main"
                    title={`${shortOid(entry.oid)} — ${entry.summary}\nClick to reveal in graph`}
                    onClick={() => onRevealCommit(entry.oid)}
                  >
                    <span className="file-history-oid mono">{shortOid(entry.oid)}</span>
                    <span className="file-history-summary">{entry.summary}</span>
                    <span className="file-history-author">{entry.authorName}</span>
                    <span className="file-history-date">{relativeDate(entry.authorTs, now)}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}
