// Repo-health fixture for the browser-harness mock (P29 §7).

import type { RepoHealth } from '../types';

/** P29 §7 repo-health fixture: exercises EVERY warn state the panel renders —
 *  stale branches, ahead/behind, drifted assets, locked/prunable worktrees,
 *  out-of-sync/uninitialized submodules, large files, conflict + merge-op,
 *  stashes, and capped flags (commitCount + objectCount render `≥`). */
export function mockRepoHealth(): RepoHealth {
  return {
    stats: {
      data: {
        commitCount: 100_000,
        commitCountCapped: true,
        commitsLast30d: 342,
        authorsLast30d: 4,
        authorsTotal: 27,
        objectCount: 500_000,
        objectScanCapped: true,
        largestBlobs: [
          { oid: '9c1185a5c5e9fc54612808977ee8f548b2258d31', size: 50_331_648 },
          { oid: '3f786850e387550fdab836ed7e6dc881de23001b', size: 22_020_096 },
          { oid: '89e6c98d92887913cadf06b2adb97f26cde4849b', size: 9_437_184 },
        ],
        workdirFileCount: 12_873,
        workdirBytes: 734_003_200,
        workdirScanCapped: false,
        largestFiles: [
          { path: 'assets/design/master.psd', size: 96_468_992 },
          { path: 'fixtures/repo-20k.bundle', size: 25_165_824 },
          { path: 'docs/media/demo.mp4', size: 8_912_896 },
        ],
        largeFileCount: 2,
        gitDirBytes: 182_452_224,
        gitDirScanCapped: false,
      },
      error: null,
      elapsedMs: 412,
    },
    branches: {
      data: {
        localCount: 14,
        remoteCount: 22,
        tagCount: 9,
        currentBranch: 'main',
        detached: false,
        unborn: false,
        ahead: 2,
        behind: 5,
        upstream: 'origin/main',
        stale: { base: 'main', mergedCount: 3, goneUpstreamCount: 1 },
        staleError: null,
      },
      error: null,
      elapsedMs: 38,
    },
    workingState: {
      data: {
        staged: 2,
        unstaged: 3,
        untracked: 4,
        conflicted: 1,
        opState: { kind: 'merge', incoming: 'feature/x', message: "Merge branch 'feature/x'" },
        stashCount: 2,
        hasGitignore: true,
      },
      error: null,
      elapsedMs: 21,
    },
    structure: {
      data: {
        submoduleCount: 3,
        submodulesUninitialized: 1,
        submodulesOutOfSync: 1,
        submodulesModified: 0,
        worktreeCount: 4,
        worktreesLocked: 1,
        worktreesPrunable: 1,
        worktreesInvalid: 0,
        assetDriftedCount: 2,
        assetsInSync: false,
      },
      error: null,
      elapsedMs: 9,
    },
    generatedAt: Math.floor(Date.now() / 1000),
  };
}
