import { forwardRef, useImperativeHandle, useState } from 'react';
import { isAppError } from '../utils/errors';
import type { SigningStatus } from '../ipc';
import { ConfirmDialog } from './ConfirmDialog';
import { CommitOptionsRow } from './CommitOptionsRow';
import { COMMIT_HOOK_CANCELED, COMMIT_PUSH_CANCELED } from './commitPushSignal';

export interface CommitBoxProps {
  stagedCount: number;
  /** App-wide mutation in flight. */
  busy: boolean;
  /** Resolves on success (box clears its textarea); rejects with AppError on
   * failure. P58c: `sign` is the explicit checkbox value (OQ6) — true/false to
   * force, null when the toggle is hidden (merge / no signing status). P59a:
   * `skipHooks` is the "Skip hooks" checkbox (≡ `--no-verify`). */
  onCommit(message: string, sign: boolean | null, skipHooks: boolean): Promise<void>;
  /** Normal-commit-mode only: commit then push the current branch. When provided
   * (and not merge/amend), the box renders a split control — primary
   * "Commit & Push" (this) + secondary "Commit" (onCommit). Same resolve/reject
   * contract as onCommit (incl. the P59a `skipHooks` arg). */
  onCommitAndPush?: (message: string, sign: boolean | null, skipHooks: boolean) => Promise<void>;
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
  /** P54c: any working-tree change exists (staged/unstaged/untracked) — gates the
   * "Compose commits ✨" affordance (clean tree ⇒ nothing to compose). */
  workingDirty?: boolean;
  /** P54c: open the commit composer (proposes grouping the working tree into N
   * logical commits). WRITES NOTHING — the composer confirms before applying. */
  onCompose?: () => void;
  /** P40b: open Settings → Git config focused on Identity. When provided, a
   * "Set identity…" button appears beside a `configMissing` commit error. */
  onOpenIdentitySettings?: () => void;
  /** P58c: effective signing config (RepoWorkspace reads it once per repo).
   * Drives the "Sign commit" checkbox default + the will-sign / no-key hint.
   * null/undefined (unread or read failed) ⇒ the toggle is hidden and commits
   * follow `commit.gpgsign` (sign = null). Hidden in merge mode. */
  signingStatus?: SigningStatus | null;
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
    onCommitAndPush,
    mode = 'commit',
    initialMessage,
    conflictCount = 0,
    blocked = false,
    amend = false,
    aiEligible = false,
    onGenerate,
    onOpenIdentitySettings,
    workingDirty = false,
    onCompose,
    signingStatus,
  },
  ref,
) {
  const [message, setMessage] = useState(initialMessage ?? '');
  // P58c: signing toggle. `null` ⇒ follow signingStatus.enabled (config default)
  // until the user flips it; then the explicit bool sticks for the session.
  const [signOverride, setSignOverride] = useState<boolean | null>(null);
  // P59a: pre-emptive "Skip hooks" (≡ --no-verify). Default false; sent as
  // `skipHooks` to the commit action. The dialog's "Commit anyway" is the other
  // route to the same skip.
  const [skipHooks, setSkipHooks] = useState(false);
  // null = idle; otherwise which control is in flight (label + shared disable).
  const [submitting, setSubmitting] = useState<null | 'commit' | 'commitPush'>(null);
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
    submitting !== null ||
    generating ||
    (merge ? conflictCount > 0 : amend ? false : stagedCount === 0);

  // Normal commit mode only: the split Commit & Push / Commit control. Narrowed
  // to a defined action here so the render needs no redundant undefined guard.
  const splitAction = !merge && !amend ? onCommitAndPush : undefined;

  // P58c: the "Sign commit" toggle — shown in commit + amend (never merge) once
  // signingStatus is known. `signChecked` defaults to the effective
  // commit.gpgsign; the explicit value is sent as `sign` (OQ6). When the toggle
  // is hidden, `sign` is null so the commit follows config.
  const showSign = !merge && signingStatus != null;
  const signChecked = signOverride ?? (signingStatus?.enabled ?? false);
  const signArg: boolean | null = showSign ? signChecked : null;
  const signFormatLabel = signingStatus?.format === 'ssh' ? 'SSH' : 'GPG';

  // P15a: the "✨ Generate" affordance (commit mode only). Disabled per contract
  // §5.5 when AI is ineligible, nothing is staged, or a mutation/generation runs.
  const showGenerate = !merge && onGenerate !== undefined;
  const generateDisabled =
    blocked || !aiEligible || stagedCount === 0 || busy || generating || submitting !== null;

  // P54c: the "Compose commits ✨" affordance (commit mode only). Disabled when
  // AI is ineligible, the tree is clean, or a mutation/generation is in flight.
  const showCompose = !merge && !amend && onCompose !== undefined;
  const composeDisabled =
    blocked || !aiEligible || !workingDirty || busy || generating || submitting !== null;

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

  // Shared submit path: validation gate + message reset on success + error
  // surfacing (on reject the message is preserved so the user can retry).
  async function runSubmit(
    kind: 'commit' | 'commitPush',
    action: (m: string, sign: boolean | null, skipHooks: boolean) => Promise<void>,
  ) {
    if (disabled) return;
    setSubmitting(kind);
    try {
      await action(message, signArg, skipHooks);
      setMessage('');
      setError(null);
    } catch (e) {
      // Set-upstream / hook dialog dismissed: nothing was committed. Leave the
      // typed message + any existing error untouched, no new error banner.
      if (e === COMMIT_PUSH_CANCELED || e === COMMIT_HOOK_CANCELED) return;
      if (isAppError(e)) {
        setError({ kind: e.kind, text: e.message });
      } else {
        setError({ kind: 'other', text: e instanceof Error ? e.message : String(e) });
      }
    } finally {
      setSubmitting(null);
    }
  }

  function submit() {
    void runSubmit('commit', onCommit);
  }

  useImperativeHandle(ref, () => ({ submit: () => submit() }));

  return (
    <div className="commit-box">
      {(showGenerate || showCompose) && (
        <div className="commit-box-header">
          {showGenerate && (
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
          )}
          {showCompose && (
            <button
              type="button"
              className="btn-secondary commit-compose-button"
              disabled={composeDisabled}
              onClick={onCompose}
              title={
                !aiEligible
                  ? 'Enable AI features in settings to compose commits'
                  : !workingDirty
                    ? 'No working-tree changes to compose'
                    : 'Group the working tree into logical commits with AI'
              }
            >
              {'Compose commits ✨'}
            </button>
          )}
        </div>
      )}
      {/* P67 §2 item 4 / D9: `rows` is only the no-`field-sizing` FALLBACK size
          (macOS WKWebView / Linux webkit2gtk, floored by the `@supports not`
          rule in styles.css). On a supporting engine (WebView2 = evergreen
          Chromium) `rows` is IGNORED for sizing: the box auto-grows with its
          content between --rp-msg-min and --rp-msg-max. So `rows={5}` makes a
          merge message open tall only on the fallback engines; on Chromium a
          short `Merge branch 'x'` opens at the 48 px floor and grows as you
          type. Giving merge mode its own taller floor is OQ4 — a native
          checkpoint tuning call, deliberately not guessed here. */}
      <textarea
        className="commit-message"
        rows={merge ? 5 : 1}
        placeholder={
          blocked ? 'An operation is in progress' : merge ? 'Merge commit message' : 'Commit message'
        }
        value={message}
        // P1 §4.4: only the in-flight commit locks the textarea — typing keeps
        // focus while stage/unstage runs (Windows focus-drop fix). The Commit
        // button below still gates on `busy`.
        disabled={submitting !== null || blocked}
        onChange={(e) => setMessage(e.target.value)}
        onKeyDown={(e) => {
          // macOS: Cmd+Enter must commit too — same `ctrlKey || metaKey` idiom
          // as App.tsx and useWorkspaceKeyboard.ts.
          if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
            e.preventDefault();
            submit();
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
      {/* P67 §5.2: P58c sign + P59a skip-hooks (≡ --no-verify, offered in every
          commit-like mode) share ONE wrapping row. State stays here. */}
      <CommitOptionsRow
        showSign={showSign}
        signChecked={signChecked}
        onChangeSign={setSignOverride}
        signingStatus={signingStatus}
        signFormatLabel={signFormatLabel}
        onOpenIdentitySettings={onOpenIdentitySettings}
        skipHooks={skipHooks}
        onChangeSkipHooks={setSkipHooks}
        disabled={submitting !== null || blocked}
      />
      {error !== null && (
        <div className="error-banner error-banner-dismissible commit-error" role="alert">
          <span className="error-banner-text">
            {error.kind === 'configMissing' ? `Set your Git identity: ${error.text}` : error.text}
          </span>
          {error.kind === 'configMissing' && onOpenIdentitySettings !== undefined && (
            <button
              type="button"
              className="btn-secondary commit-error-action"
              onClick={() => onOpenIdentitySettings()}
            >
              Set identity…
            </button>
          )}
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
      {splitAction ? (
        <div className="commit-button-row">
          <button
            type="button"
            className="btn-primary commit-button"
            disabled={disabled}
            onClick={() => void runSubmit('commitPush', splitAction)}
          >
            {submitting === 'commitPush' ? 'Committing & Pushing…' : 'Commit & Push'}
          </button>
          <button
            type="button"
            className="btn-secondary commit-button-secondary"
            disabled={disabled}
            onClick={() => submit()}
          >
            {submitting === 'commit' ? 'Committing…' : 'Commit'}
          </button>
        </div>
      ) : (
        <button
          type="button"
          className="btn-primary commit-button"
          disabled={disabled}
          onClick={() => submit()}
        >
          {submitting !== null ? 'Committing…' : merge ? 'Commit merge' : amend ? 'Amend' : 'Commit'}
        </button>
      )}

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
