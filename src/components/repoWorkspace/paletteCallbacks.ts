/** Palette-adjacent workspace callbacks, moved VERBATIM out of
 *  RepoWorkspace.tsx (spec-007 size offset — the container is over the cap and
 *  may only shrink). No behavior change.
 *
 *  - "New branch…" opens the shared create-branch PromptDialog seeded at HEAD
 *    (a dialog — never a raw mutation); disabled when detached/unborn or busy.
 *  - Dynamic palette rows: prefill + open the search bar, or jump to a commit
 *    by oid prefix — both reuse the non-mutating single-selection reveal path. */
import { useCallback } from 'react';
import type { GraphLayout } from '../../ipc';
import { findCommitByPrefix } from './searchHelpers';

export interface UsePaletteCallbacksDeps {
  headBranch: { tip: string } | null;
  setPendingCreateBranch(v: { oid: string }): void;
  setNewWorktreeOpen(open: boolean): void;
  /** `search.openSearch` — optional prefill text. */
  openSearch(text?: string): void;
  graphDataRef: { readonly current: GraphLayout | null };
  revealCommitByOid(oid: string): void;
  pushToast(kind: 'info', message: string): void;
}

export function usePaletteCallbacks(deps: UsePaletteCallbacksDeps) {
  const { headBranch, setPendingCreateBranch, setNewWorktreeOpen, openSearch, graphDataRef, revealCommitByOid, pushToast } = deps;
  const openNewBranch = useCallback(() => {
    if (headBranch !== null) setPendingCreateBranch({ oid: headBranch.tip });
  }, [headBranch, setPendingCreateBranch]);
  const openNewWorktree = useCallback(() => setNewWorktreeOpen(true), [setNewWorktreeOpen]);
  const openSearchEmpty = useCallback(() => openSearch(), [openSearch]);
  const paletteRunSearch = useCallback((t: string) => openSearch(t), [openSearch]);
  const paletteJumpToCommit = useCallback(
    (prefix: string) => {
      const oid = findCommitByPrefix(graphDataRef.current, prefix);
      if (oid !== null) revealCommitByOid(oid);
      else pushToast('info', `No commit matching ${prefix} in the current view`);
    },
    [graphDataRef, revealCommitByOid, pushToast],
  );
  return { openNewBranch, openNewWorktree, openSearchEmpty, paletteRunSearch, paletteJumpToCommit };
}
