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
