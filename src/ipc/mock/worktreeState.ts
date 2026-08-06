// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import { CHEAP_TERSE_BODY, OPUS_RICH_BODY } from '../fixtures/aiAssets';
import { COMPARABLE_IDS, SINGLE_FILE_PATHS } from '../fixtures/profiles';
import { seedWorktrees } from '../fixtures/worktreesSeed';
import type { MockRepoState } from './repoState';
import type { AppError, WorktreeInfo } from '../types';

/** Module-level worktree list shared across all default-kind repo states —
 *  matches native, where every open tab views the same repository, so
 *  add/remove/lock/unlock are visible in every tab. Lazily seeded. */
let sharedWorktrees: WorktreeInfo[] | null = null;

/** The worktree list backing a repo state: the shared list for default repos,
 *  a throwaway empty list otherwise (non-default repos have no worktrees). */
export function worktreesFor(state: MockRepoState): WorktreeInfo[] {
  if (state.kind !== 'default' || state.graphFixture !== 'default') return [];
  if (sharedWorktrees === null) {
    sharedWorktrees = seedWorktrees('default', 'default');
  }
  return sharedWorktrees;
}

// --- P31 §6: per-worktree instruction files (worktreeKey → relPath → content).
// Module-level shared like `sharedWorktrees` — all default tabs view the same
// repository, so an activation into a linked worktree is visible from every
// tab's matrix. `@main` is NOT stored here: the main worktree's files live in
// each tab's `state.assetContent`/`state.inventory` (the P24 mock map), so
// `@main` activations keep flipping the AiAssetsPanel drift chips.
// Seeds: feature-login has a locally-tweaked (drifted) CLAUDE.md and is
// missing AGENTS.md; release-1.2 is locked (files never scanned);
// hotfix-stale is invalid (empty).
let sharedWorktreeFiles: Map<string, Record<string, string>> | null = null;

export function worktreeFilesFor(key: string): Record<string, string> {
  if (sharedWorktreeFiles === null) {
    sharedWorktreeFiles = new Map<string, Record<string, string>>([
      [
        'feature-login',
        {
          'CLAUDE.md': '# CLAUDE.md\n\nfeature-login local tweaks (drifted).\n',
          'GEMINI.md': CHEAP_TERSE_BODY,
        },
      ],
      ['release-1.2', { 'CLAUDE.md': OPUS_RICH_BODY }],
      ['hotfix-stale', {}],
    ]);
  }
  const seeded = sharedWorktreeFiles;
  let files = seeded.get(key);
  if (files === undefined) {
    files = {};
    seeded.set(key, files);
  }
  return files;
}

/** Drift/missing counts over a worktree's file map — the same math as
 *  `recomputeDrift` (canonical = first existing comparable doc), but comparing
 *  content directly instead of hashes (equivalent for the mock). */
export function worktreeDriftCounts(files: Record<string, string>): {
  drifted: number;
  missing: number;
} {
  const content = (id: string): string | undefined => files[SINGLE_FILE_PATHS[id]];
  const canonicalId = COMPARABLE_IDS.find((id) => content(id) !== undefined) ?? null;
  const canonical = canonicalId === null ? null : content(canonicalId);
  let drifted = 0;
  let missing = 0;
  for (const id of COMPARABLE_IDS) {
    const c = content(id);
    if (c === undefined) missing += 1;
    else if (canonical !== null && c !== canonical) drifted += 1;
  }
  return { drifted, missing };
}

/** The calling tab's own worktree key (D5): the linked row whose path this
 *  tab has open, else `"@main"`. */
export function tabWorktreeKey(state: MockRepoState): string {
  const row = worktreesFor(state).find((w) => !w.isMain && w.absPath === state.path);
  return row === undefined ? '@main' : row.name;
}

/** D6 eligibility guard for the worktree-targeted preview/activate mocks:
 *  throws the backend's refusal messages for unknown / invalid / prunable /
 *  locked worktrees; returns the row otherwise. `"@main"` maps to the main
 *  row, which is always eligible. */
export function requireEligibleWorktree(state: MockRepoState, worktreeKey: string): WorktreeInfo {
  const rows = worktreesFor(state);
  if (worktreeKey === '@main') {
    // The main worktree is always eligible — synthesize a row for fixtures
    // without a worktree list (mirrors the backend, where "@main" resolves on
    // any repo).
    return (
      rows.find((w) => w.isMain) ?? {
        name: 'repo',
        absPath: state.path,
        relPath: null,
        branch: state.headBranch,
        headOid: state.headOid,
        locked: false,
        lockReason: null,
        isMain: true,
        isCurrent: true,
        prunable: false,
        valid: true,
      }
    );
  }
  const row = rows.find((w) => !w.isMain && w.name === worktreeKey);
  if (row === undefined) {
    const err: AppError = { kind: 'git', message: `worktree '${worktreeKey}' not found` };
    throw err;
  }
  if (!row.valid) {
    const err: AppError = {
      kind: 'git',
      message: `worktree '${worktreeKey}' is invalid (its working directory is missing or broken)`,
    };
    throw err;
  }
  if (row.prunable) {
    const err: AppError = {
      kind: 'git',
      message: `worktree '${worktreeKey}' is stale (prunable); prune or repair it first`,
    };
    throw err;
  }
  if (row.locked) {
    const err: AppError = {
      kind: 'git',
      message: `worktree '${worktreeKey}' is locked; unlock it first`,
    };
    throw err;
  }
  return row;
}
