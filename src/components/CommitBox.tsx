import { forwardRef, useImperativeHandle, useState } from 'react';
import { isAppError } from '../utils/errors';
import { ConfirmDialog } from './ConfirmDialog';

export interface CommitBoxProps {
  stagedCount: number;
  /** App-wide mutation in flight. */
  busy: boolean;
  /** Resolves on success (box clears its textarea); rejects with AppError on failure. */
  onCommit(message: string): Promise<void>;
  /** P3c §8.4: 'merge' repurposes the box as the merge-message editor —
   * prefilled once (App remounts via key on the merge transition), button
   * label "Commit merge", submit routed to commitMerge by the parent. */
  mode?: 'commit' | 'merge';
  /** Initial textarea contents (merge: opState.message). */
  initialMessage?: string;
  /** Merge mode: remaining conflicts gate submission. */
  conflictCount?: number;
  /** Non-merge op active (rebase/cherry-pick/revert): fully disabled. */
  blocked?: boolean;
  /** P20: amend mode. Button label "Amend"; a message-only amend is valid, so
   * `stagedCount === 0` does NOT disable submit. Merge mode is unaffected. */
  amend?: boolean;
  /** P15a: gates the "✨ Generate" button (aiEnabled && aiConsented && installed). */
  aiEligible?: boolean;
  /** P15a: asks the backend for a proposed message; resolves the text to insert,
   * rejects with AppError. Never commits. */
  onGenerate?(): Promise<string>;
}

/** Imperative submit hook so OpBanner's [Commit merge] triggers the same
 * submit path as the box's own button (P3c §8.1/§8.4). */
export interface CommitBoxHandle {
  submit(): void;
}

const SUMMARY_LIMIT = 72;

/** Pinned at the right-panel bottom: message textarea + Commit button (M3 §4.3). */
export const CommitBox = forwardRef<CommitBoxHandle, CommitBoxProps>(function CommitBox(
  {
    stagedCount,
    busy,
    onCommit,
    mode = 'commit',
    initialMessage,
    conflictCount = 0,
    blocked = false,
    amend = false,
    aiEligible = false,
    onGenerate,
  },
  ref,
) {
  const [message, setMessage] = useState(initialMessage ?? '');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<{ kind: string; text: string } | null>(null);
  // P15a: AI commit-message generation (proposal only — never commits).
  const [generating, setGenerating] = useState(false);
  const [replaceConfirmOpen, setReplaceConfirmOpen] = useState(false);

  const merge = mode === 'merge';
  const firstLineLen = (message.split('\n', 1)[0] ?? '').length;
  const disabled =
    blocked ||
    message.trim() === '' ||
    busy ||
    submitting ||
    generating ||
    (merge ? conflictCount > 0 : amend ? false : stagedCount === 0);

  // P15a: the "✨ Generate" affordance (commit mode only). Disabled per contract
  // §5.5 when AI is ineligible, nothing is staged, or a mutation/generation runs.
  const showGenerate = !merge && onGenerate !== undefined;
  const generateDisabled =
    blocked || !aiEligible || stagedCount === 0 || busy || generating || submitting;

  async function runGenerate() {
    if (onGenerate === undefined) return;
    setGenerating(true);
    try {
      const msg = await onGenerate();
      setMessage(msg); // REPLACES current text (contract §5.5)
      setError(null);
    } catch (e) {
      if (isAppError(e)) {
        setError({ kind: e.kind, text: e.message });
      } else {
        setError({ kind: 'other', text: e instanceof Error ? e.message : String(e) });
      }
    } finally {
      setGenerating(false);
    }
  }

  function onGenerateClick() {
    if (generateDisabled) return;
    // Non-empty box → confirm before replacing; empty → replace silently (§7.4).
    if (message.trim() !== '') {
      setReplaceConfirmOpen(true);
    } else {
      void runGenerate();
    }
  }

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

  useImperativeHandle(ref, () => ({ submit: () => void submit() }));

  return (
    <div className="commit-box">
      {showGenerate && (
        <div className="commit-box-header">
          <button
            type="button"
            className="btn-secondary commit-generate-button"
            disabled={generateDisabled}
            onClick={onGenerateClick}
            title={
              !aiEligible
                ? 'Enable AI features in settings to generate a commit message'
                : stagedCount === 0
                  ? 'Stage changes to generate a commit message'
                  : 'Generate a commit message from the staged changes'
            }
          >
            {generating ? 'Generating…' : '✨ Generate'}
          </button>
        </div>
      )}
      <textarea
        className="commit-message"
        rows={merge ? 5 : 3}
        placeholder={
          blocked ? 'An operation is in progress' : merge ? 'Merge commit message' : 'Commit message'
        }
        value={message}
        // P1 §4.4: only the in-flight commit locks the textarea — typing keeps
        // focus while stage/unstage runs (Windows focus-drop fix). The Commit
        // button below still gates on `busy`.
        disabled={submitting || blocked}
        onChange={(e) => setMessage(e.target.value)}
        onKeyDown={(e) => {
          if (e.ctrlKey && e.key === 'Enter') {
            e.preventDefault();
            void submit();
          }
        }}
      />
      {message.length > 0 && (
        <div
          className={
            firstLineLen > SUMMARY_LIMIT ? 'commit-counter commit-counter-over' : 'commit-counter'
          }
        >
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
        {submitting ? 'Committing…' : merge ? 'Commit merge' : amend ? 'Amend' : 'Commit'}
      </button>

      <ConfirmDialog
        open={replaceConfirmOpen}
        title="Replace the current message?"
        confirmLabel="Replace"
        busy={generating}
        onConfirm={() => {
          setReplaceConfirmOpen(false);
          void runGenerate();
        }}
        onCancel={() => setReplaceConfirmOpen(false)}
      >
        <div>The commit box already has text. Replace it with an AI-generated message?</div>
      </ConfirmDialog>
    </div>
  );
});
