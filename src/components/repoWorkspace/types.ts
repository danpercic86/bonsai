import type { Dispatch, SetStateAction } from 'react';
import type { PushToast } from '../../ToastContext';
import type {
  BlameLine,
  FileHistoryEntry,
  LineSelection,
  ReflogEntry,
  ResetMode,
} from '../../ipc';
import type { RefreshScope } from './refreshScope';

/** Convenience alias for a `useState` setter. */
export type Setter<T> = Dispatch<SetStateAction<T>>;

/** Shared post-op refresh callbacks threaded from RepoWorkspace into the domain
 *  action hooks. These are the stable `useCallback` refetch/clear helpers +
 *  `refreshAll` defined once in the container. */
export interface RefreshDeps {
  refreshAll: (scope?: RefreshScope) => Promise<void>;
  refetchStatus: () => Promise<void>;
  refetchGraph: () => Promise<void>;
  refetchBranches: () => Promise<void>;
  refetchStashes: () => Promise<void>;
  refetchSubmodules: () => Promise<void>;
  refetchWorktrees: () => Promise<void>;
  refetchRemotes: () => Promise<void>;
}

/** P73 §6.1: the submodule with an op in flight, and the present-participle
 *  label its row badge shows meanwhile. null ⇒ no submodule op running. */
export interface SubmoduleBusy {
  name: string;
  label: string;
}

/** The most common trio every mutating handler needs. */
export interface BaseActionDeps {
  repoId: string;
  pushToast: PushToast;
  setMutating: Setter<boolean>;
}

// ----- pending-dialog state shapes (mirrors the inline shapes in RepoWorkspace) -----

export interface PendingReservedStash {
  index: number;
  op: 'apply' | 'pop';
  paths: string[];
  /** F-A6-B: the oid the UI rendered for this stack index, forwarded verbatim on
   *  the skip-reserved retry so it hits the same entry the user saw. Undefined
   *  only for legacy callers that never captured an oid. */
  oid?: string;
}

export interface PendingReset {
  oid: string;
  mode: ResetMode;
}

export interface PendingDiscardForce {
  paths: string[];
  modified: number;
  created: number;
  untracked: string[];
}

export interface PendingHunkDiscard {
  path: string;
  origPath: string | null;
  hunkIndex: number;
}

export interface PendingLineDiscard {
  path: string;
  origPath: string | null;
  selection: LineSelection[];
}

export interface PendingCherrypick {
  oid: string;
  initialMessage: string;
  loading: boolean;
}

export interface RebasePlan {
  ontoOid: string;
  ontoLabel: string;
  initialTodos: import('../../ipc').RebaseTodoOp[];
  summaries: Record<string, string>;
}

// ----- P23d/P38: read-overlay state shapes (blame / file-history / reflog) -----

export interface BlameState {
  path: string;
  lines: BlameLine[];
  loading: boolean;
  error: string | null;
}

export interface HistoryState {
  path: string;
  entries: FileHistoryEntry[];
  loading: boolean;
  error: string | null;
}

export interface ReflogState {
  refName: string;
  entries: ReflogEntry[];
  loading: boolean;
  error: string | null;
}
