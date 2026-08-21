import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { isAppError } from '../utils/errors';
import type { SigningStatus, StashScope } from '../ipc';
import { ConfirmDialog } from './ConfirmDialog';
import { CommitOptionsMenu } from './CommitOptionsMenu';
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
   * (and not merge/amend), the box renders a split control — Commit + Commit &
   * Push, with the emphasized one chosen by `primaryCommitAction`. Same
   * resolve/reject contract as onCommit (incl. the P59a `skipHooks` arg). */
  onCommitAndPush?: (message: string, sign: boolean | null, skipHooks: boolean) => Promise<void>;
  /** P3c §8.4: 'merge' repurposes the box as the merge-message editor —
   * prefilled once (App remounts via key on the merge transition), button
   * label "Commit merge", submit routed to commitMerge by the parent. */
  mode?: 'commit' | 'merge';
  /** Initial textarea contents. P80: merge message ONLY (amend now reseeds via
   * an internal effect from `amendMessage`, not through `initialMessage`). */
  initialMessage?: string;
  /** Merge mode: remaining conflicts gate submission. */
  conflictCount?: number;
  /** Non-merge op active (rebase/cherry-pick/revert): fully disabled. */
  blocked?: boolean;
  /** P20: amend mode. Button label "Amend"; a message-only amend is valid, so
   * `stagedCount === 0` does NOT disable submit. Merge mode is unaffected. */
  amend?: boolean;
  /** P80: the last commit's full message, for the amend reseed effect. */
  amendMessage?: string | null;
  /** P80: amend is offered (opState.kind==='none' && head && !head.unborn).
   * Gates the Amend menuitemcheckbox; when false the item is not rendered. */
  canAmend?: boolean;
  /** P80: toggle amend (owned upstream by RepoWorkspace; CommitBox only forwards
   * the menu checkbox change — it does NOT own amend state). */
  onToggleAmend?: (next: boolean) => void;
  /** P80: amend would rewrite already-pushed history → drives the note line. */
  showAmendPushWarning?: boolean;
  /** P80 D1: which commit button is emphasized in the split control. */
  primaryCommitAction?: 'commit' | 'commitPush';
  /** P15a: gates the "✨ Generate" button (aiEnabled && aiConsented && installed). */
  aiEligible?: boolean;
  /** P15a: asks the backend for a proposed message; resolves the text to insert,
   * rejects with AppError. Never commits. */
  onGenerate?(): Promise<string>;
  /** P54c: any working-tree change exists (staged/unstaged/untracked) — gates the
   * "✨ Compose commits" affordance (clean tree ⇒ nothing to compose). */
  workingDirty?: boolean;
  /** P54c: open the commit composer (proposes grouping the working tree into N
   * logical commits). WRITES NOTHING — the composer confirms before applying. */
  onCompose?: () => void;
  /** P80: request an AI review of the whole staged set (menu, staged scope). */
  onReviewStaged?: () => void;
  /** P80: request an AI review of the WHOLE working tree (menu, worktree scope). */
  onReviewWorktree?: () => void;
  /** P15b: true while an AI explain/review call is in flight — disables Review. */
  aiAnalyzing?: boolean;
  /** P34: stash the worktree per scope (absorbed from RightPanelActionsRow). */
  onStash?: (scope: StashScope) => void;
  /** P80: stash is only reachable in the normal working state (opState 'none',
   *  born HEAD) — never during a merge/rebase/etc, where `git stash` refuses on
   *  unmerged paths. When false the stash items are hidden from the menu. */
  canStash?: boolean;
  /** Per-scope stash enablement (P80, verbatim semantics). */
  hasTrackedChanges?: boolean;
  hasUntracked?: boolean;
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

/** Pinned at the right-panel bottom: message textarea + compact toolbar + Commit
 * button(s) (M3 §4.3 / P80 §2b). */
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
    amendMessage,
    canAmend = false,
    onToggleAmend,
    showAmendPushWarning = false,
    primaryCommitAction = 'commit',
    aiEligible = false,
    onGenerate,
    workingDirty = false,
    onCompose,
    onReviewStaged,
    onReviewWorktree,
    aiAnalyzing = false,
    onStash,
    canStash = false,
    hasTrackedChanges = false,
    hasUntracked = false,
    onOpenIdentitySettings,
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

  // P80: amend reseed effect. `amend` remains a prop (owned by RepoWorkspace);
  // the box no longer remounts on toggle, so we reseed `message` in place —
  // stashing a user-typed commit draft across an amend excursion.
  const commitDraftRef = useRef('');
  const prevAmendRef = useRef(amend);
  const messageRef = useRef(message);
  messageRef.current = message;
  const amendSeededRef = useRef(false);

  useEffect(() => {
    const wasAmend = prevAmendRef.current;
    if (wasAmend === amend) return;
    prevAmendRef.current = amend;
    if (amend) {
      commitDraftRef.current = messageRef.current;
      setMessage(amendMessage ?? '');
      amendSeededRef.current = amendMessage != null;
    } else {
      setMessage(commitDraftRef.current);
      amendSeededRef.current = false;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [amend, amendMessage]);

  // Async-race guard: amendMessage may resolve AFTER amend already flipped true.
  // Apply it only while the box is still untouched, so a fast typist is never
  // overwritten.
  useEffect(() => {
    if (amend && !amendSeededRef.current && amendMessage != null && messageRef.current === '') {
      setMessage(amendMessage);
      amendSeededRef.current = true;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [amend, amendMessage]);

  const merge = mode === 'merge';
  const firstLineLen = (message.split('\n', 1)[0] ?? '').length;
  const disabled =
    blocked ||
    message.trim() === '' ||
    busy ||
    submitting !== null ||
    generating ||
    (merge ? conflictCount > 0 : amend ? false : stagedCount === 0);

  // Normal commit mode only: the split Commit / Commit & Push control. Narrowed
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
  const generateTitle = !aiEligible
    ? 'Enable AI features in settings to generate a commit message'
    : stagedCount === 0
      ? 'Stage changes to generate a commit message'
      : 'Generate a commit message from the staged changes';

  // P54c: the "✨ Compose commits" affordance (commit mode only). Disabled when
  // AI is ineligible, the tree is clean, or a mutation/generation is in flight.
  const showCompose = !merge && !amend && onCompose !== undefined;
  const composeDisabled =
    blocked || !aiEligible || !workingDirty || busy || generating || submitting !== null;
  const composeTitle = !aiEligible
    ? 'Enable AI features in settings to compose commits'
    : !workingDirty
      ? 'No working-tree changes to compose'
      : 'Group the working tree into logical commits with AI';

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
      // P80: clear the stashed commit draft so a later amend excursion cannot
      // resurrect a message that was already committed.
      commitDraftRef.current = '';
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

  // P80 D1: which action carries `.btn-primary`. Only meaningful for the split
  // control (non-merge, non-amend, onCommitAndPush provided).
  const pushIsPrimary = primaryCommitAction === 'commitPush';

  const commitBtn = (primary: boolean) => (
    <button
      type="button"
      className={primary ? 'btn-primary commit-button' : 'btn-secondary commit-button-secondary'}
      disabled={disabled}
      onClick={() => submit()}
    >
      {submitting === 'commit' ? 'Committing…' : 'Commit'}
    </button>
  );
  const pushBtn = (primary: boolean, action: NonNullable<typeof splitAction>) => (
    <button
      type="button"
      className={primary ? 'btn-primary commit-button' : 'btn-secondary commit-button-secondary'}
      disabled={disabled}
      onClick={() => void runSubmit('commitPush', action)}
    >
      {submitting === 'commitPush' ? 'Committing & Pushing…' : 'Commit & Push'}
    </button>
  );

  return (
    <div className="commit-box">
      {/* P67 §2 item 4 / D9: `rows` is only the no-`field-sizing` FALLBACK size
          (macOS WKWebView / Linux webkit2gtk). On WebView2 the box auto-grows
          with content between --rp-msg-min and --rp-msg-max. */}
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
      {/* P80 §2b D2: one compact toolbar — generate icon, counter, options menu. */}
      <div className="commit-msg-toolbar">
        {showGenerate && (
          <button
            type="button"
            className="commit-msg-tool commit-generate-button"
            disabled={generateDisabled}
            aria-label="Generate commit message"
            aria-busy={generating}
            title={generateTitle}
            onClick={onGenerateClick}
          >
            {'✨'}
          </button>
        )}
        {message.length > 0 && (
          <span
            className={
              firstLineLen > SUMMARY_LIMIT ? 'commit-counter commit-counter-over' : 'commit-counter'
            }
          >
            {firstLineLen}/{SUMMARY_LIMIT}
          </span>
        )}
        <CommitOptionsMenu
          disabled={blocked}
          busy={busy || submitting !== null}
          aiEligible={aiEligible && (onReviewStaged !== undefined || onReviewWorktree !== undefined)}
          stagedCount={stagedCount}
          workingDirty={workingDirty}
          aiAnalyzing={aiAnalyzing}
          onReviewStaged={() => onReviewStaged?.()}
          onReviewWorktree={() => onReviewWorktree?.()}
          canAmend={canAmend && onToggleAmend !== undefined}
          amend={amend}
          onToggleAmend={(next) => onToggleAmend?.(next)}
          showSign={showSign}
          signChecked={signChecked}
          onChangeSign={setSignOverride}
          signFormatLabel={signFormatLabel}
          signingStatus={signingStatus}
          skipHooks={skipHooks}
          onChangeSkipHooks={setSkipHooks}
          showCompose={showCompose}
          composeDisabled={composeDisabled}
          composeTitle={composeTitle}
          onCompose={() => onCompose?.()}
          canStash={canStash}
          hasTrackedChanges={hasTrackedChanges}
          hasUntracked={hasUntracked}
          onStash={(scope) => onStash?.(scope)}
        />
      </div>
      {/* P80: single conditional note line below the toolbar (2c refines the
          glyph/copy; behavior preserved here). Priority: amend-pushed >
          sign no-key > sign will-sign > skip hooks. */}
      {showAmendPushWarning ? (
        <div className="amend-push-warning" role="note">
          This commit is already pushed — amending rewrites published history.
        </div>
      ) : showSign && signChecked && !(signingStatus?.hasKey ?? false) ? (
        <span className="commit-sign-warn" role="note">
          No signing key set — set user.signingkey
          {onOpenIdentitySettings !== undefined && (
            <button
              type="button"
              className="commit-sign-fix"
              onClick={() => onOpenIdentitySettings()}
            >
              Set key…
            </button>
          )}
        </span>
      ) : showSign && signChecked && (signingStatus?.hasKey ?? false) ? (
        <span className="commit-sign-hint">Commits will be signed ({signFormatLabel})</span>
      ) : skipHooks ? (
        <span className="commit-skip-hint" role="note">
          Git hooks (pre-commit, commit-msg) won’t run for this commit
        </span>
      ) : null}
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
          {pushIsPrimary ? (
            <>
              {pushBtn(true, splitAction)}
              {commitBtn(false)}
            </>
          ) : (
            <>
              {commitBtn(true)}
              {pushBtn(false, splitAction)}
            </>
          )}
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
