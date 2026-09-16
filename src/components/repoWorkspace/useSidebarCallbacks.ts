// Render-storm fix (P91 follow-up) — the Sidebar's nine action callbacks, hoisted
// out of the JSX call site so their IDENTITY is stable across a container render.
//
// They used to be inline arrows (`onAddRemote={() => setPendingAddRemote(true)}`
// …), which meant every RepoWorkspace commit handed the Sidebar nine fresh
// functions. That alone defeats `React.memo` on the sections and on every row
// they reach, so it has to be fixed BEFORE memoising is worth anything.
//
// DEPS-BY-REF, deliberately: several of the dependencies are themselves rebuilt
// every render upstream (`useStashActions` returns `traced(...)`-wrapped
// handlers, not `useCallback`s). Capturing them in a ref and returning `[]`-dep
// callbacks makes the returned identities stable regardless — and is safe
// because every one of these is invoked from an event handler, never during
// render, so it always reads the current value.
import { useMemo, useRef } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import type { StashScope } from '../../ipc';
import { GESTURES, traced } from '../../obs/gesture';
import type { SidebarProps } from '../Sidebar';

export interface SidebarCallbackDeps {
  setBranchesError: Dispatch<SetStateAction<string | null>>;
  handleCheckoutBranch(name: string): Promise<void> | void;
  handleCreateBranch(name: string): Promise<void>;
  handleCreateStash(scope: StashScope): Promise<void> | void;
  refetchTagSync(): Promise<void> | void;
  setPendingAddSubmodule: Dispatch<SetStateAction<boolean>>;
  setNewWorktreeOpen: Dispatch<SetStateAction<boolean>>;
  setPendingAddRemote: Dispatch<SetStateAction<boolean>>;
  setStaleCleanupOpen: Dispatch<SetStateAction<boolean>>;
}

/** Exactly the Sidebar props this hook supplies — spread at the call site, so
 *  adding one here is a type error until `SidebarProps` names it. */
export type SidebarCallbacks = Pick<
  SidebarProps,
  | 'onDismissError'
  | 'onCheckout'
  | 'onCreateBranch'
  | 'onCreateStash'
  | 'onNewSubmodule'
  | 'onNewWorktree'
  | 'onTagsExpand'
  | 'onAddRemote'
  | 'onCleanupBranches'
>;

export function useSidebarCallbacks(deps: SidebarCallbackDeps): SidebarCallbacks {
  const ref = useRef(deps);
  ref.current = deps;
  return useMemo<SidebarCallbacks>(
    () => ({
      onDismissError: () => ref.current.setBranchesError(null),
      // Gesture traces are byte-preserved from the old call site: the checkout /
      // create arrows were `traced('click', …)` there, and `handleCreateStash`
      // arrives already wrapped as `traced('menu', …)` from useStashActions.
      onCheckout: traced('click', GESTURES.branchCheckout, (name: string) => {
        void ref.current.handleCheckoutBranch(name);
      }),
      onCreateBranch: traced('click', GESTURES.branchCreate, (name: string) =>
        ref.current.handleCreateBranch(name),
      ),
      onCreateStash: () => void ref.current.handleCreateStash('allWithUntracked'),
      onNewSubmodule: () => ref.current.setPendingAddSubmodule(true),
      onNewWorktree: () => ref.current.setNewWorktreeOpen(true),
      onTagsExpand: () => void ref.current.refetchTagSync(),
      onAddRemote: () => ref.current.setPendingAddRemote(true),
      onCleanupBranches: () => ref.current.setStaleCleanupOpen(true),
    }),
    [],
  );
}
