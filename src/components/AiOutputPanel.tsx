// P15b: read-only, dismissible prose card for AI explain/review output. Purely
// presentational — RepoWorkspace owns the ipc.aiAnalyzeDiff call + open/loading/
// error state. Renders markdown-free prose (the backend strips code fences).
// Reuses the diff-overlay / error-banner / skeleton CSS; no new IPC.

export interface AiOutputPanelProps {
  /** Header title, e.g. "Explain commit a1b2c3d" / "Review staged changes". */
  title: string;
  /** Prose result; null while loading or on error. */
  text: string | null;
  loading: boolean;
  /** Human error string (null when none). */
  error: string | null;
  /** Optional call cost; shown in the header when present. */
  costUsd?: number | null;
  onClose(): void;
}

export function AiOutputPanel({
  title,
  text,
  loading,
  error,
  costUsd,
  onClose,
}: AiOutputPanelProps) {
  return (
    <div className="diff-overlay ai-output-panel" role="region" aria-label={title}>
      <div className="diff-overlay-header">
        <span className="ai-output-icon" aria-hidden="true">
          {'✨'}
        </span>
        <span className="diff-overlay-path" title={title}>
          {title}
        </span>
        {costUsd != null && (
          <span className="diff-overlay-kind ai-output-cost">${costUsd.toFixed(4)}</span>
        )}
        <button
          type="button"
          className="btn-icon diff-overlay-close"
          aria-label="Close AI output"
          title="Close (Esc)"
          onClick={onClose}
        >
          {'×'}
        </button>
      </div>
      <div className="diff-overlay-body">
        {loading ? (
          <div className="skeleton-group" aria-hidden="true">
            {Array.from({ length: 5 }, (_, i) => (
              <div key={i} className="skeleton-row" />
            ))}
          </div>
        ) : error !== null ? (
          <div className="error-banner error-banner-dismissible" role="alert">
            <span className="error-banner-text">{error}</span>
            <button
              type="button"
              className="error-dismiss"
              aria-label="Dismiss error"
              onClick={onClose}
            >
              {'×'}
            </button>
          </div>
        ) : (
          <pre className="ai-output-text">{text}</pre>
        )}
      </div>
    </div>
  );
}
