// P83 — shared merge-method label/description maps (UI contract §2, §7).
// Colocated so PrMergeDialog and PrActionsBar agree. SUPPORTED_MERGE_METHODS
// lives in ipc/types (mirrors the Rust `supported_for`); re-exported here for a
// single import site.

import type { MergeMethod } from '../../ipc';
import { SUPPORTED_MERGE_METHODS } from '../../ipc';

export { SUPPORTED_MERGE_METHODS };

/** Short segment labels for the method picker. */
export const MERGE_METHOD_LABEL: Record<MergeMethod, string> = {
  merge: 'Merge',
  squash: 'Squash',
  rebase: 'Rebase',
  fastForward: 'Fast-forward',
};

/** One-line description under the picker, per selection. */
export const MERGE_METHOD_DESC: Record<MergeMethod, string> = {
  merge: 'Creates a merge commit.',
  squash: 'Combines all commits into one.',
  rebase: 'Replays commits onto the base, no merge commit.',
  fastForward: 'Moves the base to the source tip, no merge commit.',
};

/** Lowercased method word used in the merge-dialog summary copy. */
export const MERGE_METHOD_WORD: Record<MergeMethod, string> = {
  merge: 'merge',
  squash: 'squash',
  rebase: 'rebase',
  fastForward: 'fast-forward',
};

/** Whether a method carries an optional commit title/message (merge/squash).
 *  Rebase and fast-forward ignore them, so the fields are hidden. */
export function methodTakesCommitFields(method: MergeMethod): boolean {
  return method === 'merge' || method === 'squash';
}
