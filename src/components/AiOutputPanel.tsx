// P15b: read-only, dismissible prose card for AI explain/review output. Purely
// presentational — RepoWorkspace owns the ipc.aiAnalyzeDiff call + open/loading/
// error state. Renders markdown-free prose (the backend strips code fences).
// Reuses the diff-overlay / error-banner / skeleton CSS; no new IPC.
//
// P56b: two ADDITIVE, backward-compatible affordances. (1) A Copy button in the
// header copies the rendered text — harmless for every existing read-only caller.
// (2) An OPT-IN `editable` mode swaps the read-only <pre> for a <textarea> so the
// changelog caller can tweak the Markdown before pasting; local draft state keeps
// typing self-contained (no parent re-render). Callers that pass neither prop get
// the unchanged <pre> render path.

import { useEffect, useState } from 'react';

import { SummarizeIcon } from './menuIcons';

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
  /** P56b (opt-in): render the body as an editable <textarea> instead of <pre>
   *  so users can tweak the output before copying. Existing callers omit this →
   *  unchanged read-only render. */
  editable?: boolean;
  /** P56b (opt-in): fired with the edited text on each change; pairs with
   *  `editable`. Draft is held locally regardless, so this is only needed by a
   *  caller that wants to persist edits. */
  onEdit?(next: string): void;
  onClose(): void;
}

export function AiOutputPanel({
  title,
  text,
  loading,
  error,
  costUsd,
  editable,
  onEdit,
  onClose,
}: AiOutputPanelProps) {
  // Local editable draft, seeded/reset whenever the underlying text changes (a
  // new generation). Unrelated parent re-renders keep the same `text` value → the
  // effect does not fire → in-progress edits survive.
  const [draft, setDraft] = useState(text ?? '');
  useEffect(() => {
    setDraft(text ?? '');
  }, [text]);

  // Transient "Copied" confirmation on the Copy button (self-cleaning timer).
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const id = window.setTimeout(() => setCopied(false), 1200);
    return () => window.clearTimeout(id);
  }, [copied]);

  const hasText = !loading && error === null && text !== null;
  const copyValue = editable ? draft : (text ?? '');
  const onCopy = () => {
    const p =
      navigator.clipboard?.writeText(copyValue) ??
      Promise.reject(new Error('Clipboard unavailable'));
    void p.then(() => setCopied(true)).catch(() => setCopied(false));
  };

  return (
    <div className="diff-overlay ai-output-panel" role="region" aria-label={title}>
      <div className="diff-overlay-header">
        <span className="ai-output-icon" aria-hidden="true">
          <SummarizeIcon />
        </span>
        <span className="diff-overlay-path" title={title}>
          {title}
        </span>
        {costUsd != null && (
          <span className="diff-overlay-kind ai-output-cost">${costUsd.toFixed(4)}</span>
        )}
        {hasText && (
          <button
            type="button"
            className="btn-icon ai-output-copy"
            aria-label="Copy to clipboard"
            title="Copy"
            onClick={onCopy}
          >
            {copied ? 'Copied' : 'Copy'}
          </button>
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
        ) : editable ? (
          <textarea
            className="ai-output-text ai-output-editable"
            aria-label={`${title} (editable)`}
            spellCheck={false}
            value={draft}
            onChange={(e) => {
              setDraft(e.target.value);
              onEdit?.(e.target.value);
            }}
          />
        ) : (
          <pre className="ai-output-text">{text}</pre>
        )}
      </div>
    </div>
  );
}
