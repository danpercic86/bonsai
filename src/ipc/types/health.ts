import type { RepoOpState } from './common';

// --- P29: repo health -------------------------------------------------------

/** P29. Per-section envelope: exactly one of `data`/`error` is set. Section
 *  failures never reject the whole getRepoHealth call (contract §D4). */
export interface Section<T> {
  data: T | null;
  error: string | null;
  elapsedMs: number;
}

/** P29. All four health sections in one payload. Wire mirror of the Rust
 *  `RepoHealth` (camelCase). */
export interface RepoHealth {
  stats: Section<StatsSection>;
  branches: Section<BranchesSection>;
  workingState: Section<WorkingStateSection>;
  structure: Section<StructureSection>;
  /** Epoch seconds. */
  generatedAt: number;
}

/** P29. History/ODB/disk metrics. Every `*Capped` flag means the number is a
 *  floor — the UI renders `≥`. */
export interface StatsSection {
  commitCount: number;
  commitCountCapped: boolean;
  commitsLast30d: number;
  authorsLast30d: number;
  authorsTotal: number;
  objectCount: number;
  objectScanCapped: boolean;
  /** Top 10 desc by size; blob→path mapping is out of scope (label "blob <shortOid>"). */
  largestBlobs: BlobStat[];
  workdirFileCount: number;
  workdirBytes: number;
  workdirScanCapped: boolean;
  /** Top 10 desc; forward-slash repo-relative paths. */
  largestFiles: FileStat[];
  /** Worktree files >= 10 MiB (warn when > 0). */
  largeFileCount: number;
  gitDirBytes: number;
  gitDirScanCapped: boolean;
}

export interface BlobStat {
  /** Full 40-hex; UI shortens to 7. */
  oid: string;
  size: number;
}

export interface FileStat {
  path: string;
  size: number;
}

/** P29. Ref counts + HEAD shape + stale rollup. */
export interface BranchesSection {
  localCount: number;
  remoteCount: number;
  tagCount: number;
  /** null = detached/unborn. */
  currentBranch: string | null;
  detached: boolean;
  unborn: boolean;
  /** vs upstream, best-effort; null when no upstream / lookup failed. */
  ahead: number | null;
  behind: number | null;
  upstream: string | null;
  /** null when the stale scan failed — see `staleError` (sub-metric, D9). */
  stale: StaleRollup | null;
  staleError: string | null;
}

export interface StaleRollup {
  base: string;
  mergedCount: number;
  goneUpstreamCount: number;
}

/** P29. Working-tree facts; reuses `RepoOpState` verbatim. */
export interface WorkingStateSection {
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
  opState: RepoOpState;
  stashCount: number;
  hasGitignore: boolean;
}

/** P29. Submodule/worktree/AI-asset rollups. */
export interface StructureSection {
  submoduleCount: number;
  submodulesUninitialized: number;
  submodulesOutOfSync: number;
  submodulesModified: number;
  /** Includes the synthesized main row. */
  worktreeCount: number;
  worktreesLocked: number;
  worktreesPrunable: number;
  worktreesInvalid: number;
  assetDriftedCount: number;
  assetsInSync: boolean;
}
