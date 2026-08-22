import type { TagAutoSyncReport } from './branches';

export interface RemoteFetchResult {
  /** Remote name, e.g. "origin". */
  remote: string;
  receivedObjects: number;
  /** update_tips invocations where old != new (incl. newly created refs). */
  updatedRefs: number;
}

export interface FetchResult {
  /** One entry per configured remote, in remote-list order. */
  remotes: RemoteFetchResult[];
  /**
   * P84: best-effort automatic tag reconciliation performed after the fetch.
   * Absent when the fetch was a no-op or auto-sync did not run.
   */
  tagAutoSync?: TagAutoSyncReport;
}

export type PullResult =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; from: string; to: string }
  | {
      kind: 'wouldNotFastForward';
      branch: string;
      ahead: number;
      behind: number;
      /** P60b: resolved upstream shorthand ("origin/main") — the exact name the
       *  frontend hands to mergeBranch/rebaseBranch when reconciling. */
      upstream: string;
    };

export type PushResult =
  | { kind: 'upToDate'; remote: string; branch: string }
  | { kind: 'pushed'; remote: string; branch: string; setUpstream: boolean };
