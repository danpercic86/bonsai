import { ConfirmDialog } from './ConfirmDialog';

export interface HookOutputDialogProps {
  open: boolean;
  /** The `hookRejected` AppError message, shaped as
   *  `"<hook> hook failed:\n<combined stdout+stderr>"`. The first line becomes a
   *  heading; the remainder (the hook's own output) is rendered verbatim in a
   *  scrollable preformatted block. */
  message: string;
  /** True while the skip-hooks retry is in flight (disables the action). */
  busy: boolean;
  /** P59a-2: which operation the hook blocked — drives the primary-action label
   *  ("Commit anyway (skip hooks)" vs "Push anyway (skip hooks)") and the title.
   *  Default `'commit'`. When omitted it is inferred from `message`: a `pre-push`
   *  heading ⇒ `'push'` (the backend + mock both prefix push output that way). */
  opLabel?: 'commit' | 'push';
  /** "Commit/Push anyway (skip hooks)": re-run the SAME op with `skipHooks:true`. */
  onSkipRetry(): void;
  onCancel(): void;
}

/**
 * P59a: shown when a git hook (`pre-commit` / `commit-msg`) BLOCKS a
 * commit/amend/merge — the backend surfaces `AppError { kind:'hookRejected',
 * message }`. The hook's own output is rendered verbatim so the user can see WHY
 * it failed, with a "Commit anyway (skip hooks)" escape hatch that re-runs the
 * exact same commit with `skipHooks:true` (≡ `git commit --no-verify`).
 * PRESENTATIONAL — no IPC; the parent owns the retry. Built on the shared
 * `ConfirmDialog` (Esc / overlay cancel; initial focus = Cancel, so a stray
 * Enter never skips the hooks).
 */
export function HookOutputDialog({
  open,
  message,
  busy,
  opLabel,
  onSkipRetry,
  onCancel,
}: HookOutputDialogProps) {
  // The message is "<hook> hook failed:\n<output>" — peel the first line off as a
  // heading and render the remainder (the hook's stdout+stderr) preformatted.
  const nl = message.indexOf('\n');
  const heading = nl === -1 ? message : message.slice(0, nl);
  const output = nl === -1 ? '' : message.slice(nl + 1);

  // The primary action + title track the operation: a `pre-push` hook blocked a
  // push; anything else blocked a commit/amend/merge. `opLabel` overrides.
  const op = opLabel ?? (message.startsWith('pre-push') ? 'push' : 'commit');
  const verb = op === 'push' ? 'Push' : 'Commit';

  return (
    <ConfirmDialog
      open={open}
      title={`A git hook blocked this ${op}`}
      confirmLabel={`${verb} anyway (skip hooks)`}
      confirmVariant="primary"
      busy={busy}
      onConfirm={onSkipRetry}
      onCancel={onCancel}
    >
      <div className="hook-output">
        <p className="hook-output-heading">{heading}</p>
        {output !== '' && <pre className="hook-output-body">{output}</pre>}
        <p className="hook-output-note">
          Fix the reported issue and try again, or skip the hooks for this one {op}.
        </p>
      </div>
    </ConfirmDialog>
  );
}
