// P60c one-click-undo mock. `describeLastUndo` classifies a HEAD reflog[0]
// entry with the SAME prefix table as the Rust core and returns a matching
// `UndoPlan`. Execution reuses the existing `resetBranch` mock (which already
// moves the mock HEAD / graph), so nothing new mutates here.
//
// Fixtures via `?undo=` seam so the harness can drive every branch:
//   (default) → the seeded HEAD story's newest entry: a `reset` (mixed).
//   ?undo=commit → a Commit (mixed) — "move your branch back to <short>".
//   ?undo=merge  → a Merge (hard) — hard-reset wording; BLOCKS while dirty.
//   ?undo=switch → a branch switch (not undoable, with a reason).
//   ?undo=none   → an empty reflog (nothing to undo).
import type { IpcApi, ReflogEntry, ResetMode, UndoKind, UndoPlan } from '../../types';
import { MOCK_HEAD_REFLOG } from '../../fixtures/reflog';
import { delay, query, requireRepo } from '../repoState';

const ZERO = '0'.repeat(40);

/** The seeded reset (mixed) entry — `MOCK_HEAD_REFLOG[0]` per the contract. */
const RESET_HEAD = MOCK_HEAD_REFLOG[0];
/** `commit: add feature` — a Commit (mixed). Its `oldOid` is a real graph node. */
const COMMIT_HEAD = MOCK_HEAD_REFLOG[3];
/** A Merge (hard); reuses a real graph-node `oldOid` so a hard reset updates the
 *  mock graph in the harness. */
const MERGE_HEAD: ReflogEntry = {
  ...COMMIT_HEAD,
  message: "merge feature: Merge made by the 'ort' strategy.",
};
/** A branch switch — not undoable in v1. */
const SWITCH_HEAD: ReflogEntry = {
  ...COMMIT_HEAD,
  message: 'checkout: moving from feature to main',
};

/** Pick the reflog[0] entry the seam selects; null → empty reflog. */
function pickHead(): ReflogEntry | null {
  switch (query('undo')) {
    case 'commit':
      return COMMIT_HEAD;
    case 'merge':
      return MERGE_HEAD;
    case 'switch':
      return SWITCH_HEAD;
    case 'none':
      return null;
    default:
      return RESET_HEAD;
  }
}

/** Mirror of the Rust `classify` prefix table (first match wins). */
function classify(message: string): UndoKind {
  if (message.startsWith('commit (amend)')) return 'amend';
  if (message.startsWith('commit')) return 'commit';
  if (message.startsWith('reset:')) return 'reset';
  if (message.startsWith('cherry-pick')) return 'cherryPick';
  if (message.startsWith('revert:')) return 'revert';
  if (message.startsWith('rebase')) return 'rebase';
  if (
    message.startsWith('pull: Fast-forward') ||
    message.startsWith('merge: Fast-forward') ||
    message.startsWith('pull ')
  ) {
    return 'fastForward';
  }
  if (message.startsWith('merge ') || message.startsWith('pull:')) return 'merge';
  if (message.startsWith('checkout: moving from ')) return 'branchSwitch';
  return 'unknown';
}

/** Mirror of the Rust `UndoKind::reset_mode`. */
function kindResetMode(kind: UndoKind): ResetMode | null {
  switch (kind) {
    case 'commit':
    case 'amend':
    case 'reset':
      return 'mixed';
    case 'merge':
    case 'rebase':
    case 'fastForward':
    case 'cherryPick':
    case 'revert':
      return 'hard';
    default:
      return null;
  }
}

/** Build an `UndoPlan` from a reflog[0] entry + worktree dirtiness — the exact
 *  shape/logic the Rust `describe_last_undo` produces. */
function buildPlan(entry: ReflogEntry | null, dirty: boolean): UndoPlan {
  if (entry === null) {
    return {
      kind: 'unknown',
      summary: '',
      targetOid: '',
      targetShort: '',
      resetMode: null,
      requiresCleanWorktree: false,
      worktreeDirty: dirty,
      undoable: false,
      reason: 'nothing to undo',
    };
  }
  const kind = classify(entry.message);
  const isRoot = entry.oldOid === ZERO;
  const targetOid = isRoot ? '' : entry.oldOid;
  const resetMode = kindResetMode(kind);

  let undoable: boolean;
  let reason: string | null;
  if (kind === 'branchSwitch') {
    undoable = false;
    reason = "switching branches isn't undone here — check out the previous branch instead";
  } else if (resetMode === null) {
    undoable = false;
    reason = "the last operation isn't one Bonsai can undo automatically";
  } else if (isRoot) {
    undoable = false;
    reason = 'cannot undo the initial commit';
  } else {
    undoable = true;
    reason = null;
  }

  return {
    kind,
    summary: entry.message,
    targetOid,
    targetShort: targetOid === '' ? '' : targetOid.slice(0, 7),
    resetMode: undoable ? resetMode : null,
    requiresCleanWorktree: undoable && resetMode === 'hard',
    worktreeDirty: dirty,
    undoable,
    reason,
  };
}

export const undoHandlers = {
  async describeLastUndo(repoId: string): Promise<UndoPlan> {
    await delay(120);
    const state = requireRepo(repoId);
    // TRACKED dirtiness (staged + unstaged) — mirrors the backend's is_dirty
    // (untracked files survive a hard reset, so they don't gate it).
    const dirty = state.status.staged.length > 0 || state.status.unstaged.length > 0;
    return buildPlan(pickHead(), dirty);
  },
} satisfies Partial<IpcApi>;
