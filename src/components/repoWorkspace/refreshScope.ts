// P86a (workstream B3) — Refresh scope taxonomy + slice matrix.
//
// Before P86a every refresh round ran all 11 slices regardless of what changed —
// so a one-ref `git branch` still paid the O(worktree) `get_status` scan and a
// full graph walk. A `RefreshScope` narrows a round to only the slices its change
// reason implies. Rust still owns all git work + the graph layout; this table
// only decides which of the container's refetch callbacks a round invokes.
//
// Matrix (contract §B3):
//   scope       | openRepo status graph branches remotes compare opState stashes submodules worktrees tagSync
//   full        |    ✓      ✓     ✓      ✓        ✓       ✓       ✓       ✓       ✓          ✓         ✓(forced/origin)
//   refsOnly    |    –      –     ✓      ✓        –       ✓       –       –       –          –         –
//   remoteMeta  |    –      –     ✓      ✓        ✓       ✓       –       –       –          –         ✓(non-forced)
//   worktree    |    –      ✓     –      –        –       –       ✓       –       –          –         –
//
// refsOnly / remoteMeta / worktree skip `openRepo` because they never move HEAD
// (HEAD-moving ops go through `full`) — the header HEAD label + watcher self-heal
// are unaffected. The matrix is conservative: an unsure handler uses `full`.

export type RefreshScope = 'full' | 'refsOnly' | 'remoteMeta' | 'worktree';

/** Which refetch callbacks a scoped `runRefreshRound` invokes. `tagSyncForcable`
 *  gates whether an origin-forced ls-remote drift check is allowed: only `full`
 *  honours the forcing origin (manual/focus/mutation); `remoteMeta` runs tagSync
 *  NON-forced regardless of origin. */
export interface RefreshSlices {
  openRepo: boolean;
  status: boolean;
  graph: boolean;
  branches: boolean;
  stashes: boolean;
  submodules: boolean;
  worktrees: boolean;
  remotes: boolean;
  opState: boolean;
  compare: boolean;
  tagSync: boolean;
  tagSyncForcable: boolean;
}

const NONE: RefreshSlices = {
  openRepo: false,
  status: false,
  graph: false,
  branches: false,
  stashes: false,
  submodules: false,
  worktrees: false,
  remotes: false,
  opState: false,
  compare: false,
  tagSync: false,
  tagSyncForcable: false,
};

const SLICES: Record<RefreshScope, RefreshSlices> = {
  full: {
    openRepo: true,
    status: true,
    graph: true,
    branches: true,
    stashes: true,
    submodules: true,
    worktrees: true,
    remotes: true,
    opState: true,
    compare: true,
    tagSync: true,
    tagSyncForcable: true,
  },
  refsOnly: { ...NONE, graph: true, branches: true, compare: true },
  remoteMeta: { ...NONE, graph: true, branches: true, remotes: true, compare: true, tagSync: true },
  worktree: { ...NONE, status: true, opState: true },
};

/** The slice set a scoped round runs. */
export function slicesForScope(scope: RefreshScope): RefreshSlices {
  return SLICES[scope];
}

/** Collapse a set of pending scopes (collected by the coalescer while a round was
 *  in flight) into the ONE scope its trailing round should run. `full` dominates;
 *  two distinct non-full scopes widen to `full` (their slice union is always a
 *  subset of `full`, so this is safe and keeps the recorded scope label honest).
 *  An empty set defaults to `full`. */
export function unionScopes(scopes: Iterable<RefreshScope>): RefreshScope {
  let result: RefreshScope | null = null;
  for (const scope of scopes) {
    if (scope === 'full') return 'full';
    if (result === null) result = scope;
    else if (result !== scope) return 'full';
  }
  return result ?? 'full';
}
