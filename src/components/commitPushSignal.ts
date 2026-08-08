/**
 * Cancellation sentinel for the Commit & Push set-upstream flow.
 *
 * When the "Set upstream and push?" ConfirmDialog is dismissed (Cancel / Esc /
 * overlay-click), RepoWorkspace rejects the pending CommitBox submit promise
 * with this symbol instead of an error. CommitBox.runSubmit recognises it and
 * returns silently — leaving the typed message intact and showing no error
 * banner (nothing was committed).
 */
export const COMMIT_PUSH_CANCELED = Symbol('commitPushCanceled');

/**
 * Cancellation sentinel for the P59a hook-rejection flow.
 *
 * When a git hook BLOCKS a commit/amend/merge, the attempt is parked behind the
 * HookOutputDialog. Dismissing that dialog (Cancel / Esc / overlay-click)
 * rejects the parked submit promise with this symbol instead of an error, so
 * CommitBox.runSubmit (and the merge path) leave the typed message intact and
 * show no error banner — nothing was committed.
 */
export const COMMIT_HOOK_CANCELED = Symbol('commitHookCanceled');
