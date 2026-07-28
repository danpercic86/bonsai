import { useState } from 'react';
import { isAppError } from '../utils/errors';

export interface CommitBoxProps {
  stagedCount: number;
  /** App-wide mutation in flight. */
  busy: boolean;
  /** Resolves on success (box clears its textarea); rejects with AppError on failure. */
  onCommit(message: string): Promise<void>;
}

const SUMMARY_LIMIT = 72;

/** Pinned at the right-panel bottom: message textarea + Commit button (M3 §4.3). */
export function CommitBox({ stagedCount, busy, onCommit }: CommitBoxProps) {
  const [message, setMessage] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<{ kind: string; text: string } | null>(null);

  const firstLineLen = (message.split('\n', 1)[0] ?? '').length;
  const disabled = stagedCount === 0 || message.trim() === '' || busy || submitting;

  async function submit() {
    if (disabled) return;
    setSubmitting(true);
    try {
      await onCommit(message);
      setMessage('');
      setError(null);
    } catch (e) {
      if (isAppError(e)) {
        setError({ kind: e.kind, text: e.message });
      } else {
        setError({ kind: 'other', text: e instanceof Error ? e.message : String(e) });
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="commit-box">
      <textarea
        className="commit-message"
        rows={3}
        placeholder="Commit message"
        value={message}
        // P1 §4.4: only the in-flight commit locks the textarea — typing keeps
        // focus while stage/unstage runs (Windows focus-drop fix). The Commit
        // button below still gates on `busy`.
        disabled={submitting}
        onChange={(e) => setMessage(e.target.value)}
        onKeyDown={(e) => {
          if (e.ctrlKey && e.key === 'Enter') {
            e.preventDefault();
            void submit();
          }
        }}
      />
      {firstLineLen > SUMMARY_LIMIT && (
        <div className="commit-counter">
          {firstLineLen}/{SUMMARY_LIMIT}
        </div>
      )}
      {error !== null && (
        <div className="error-banner error-banner-dismissible commit-error" role="alert">
          <span className="error-banner-text">
            {error.kind === 'configMissing' ? `Set your Git identity: ${error.text}` : error.text}
          </span>
          <button
            type="button"
            className="error-dismiss"
            aria-label="Dismiss error"
            onClick={() => setError(null)}
          >
            {'×'}
          </button>
        </div>
      )}
      <button
        type="button"
        className="btn-primary commit-button"
        disabled={disabled}
        onClick={() => void submit()}
      >
        {submitting ? 'Committing…' : 'Commit'}
      </button>
    </div>
  );
}
